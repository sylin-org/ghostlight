//! A minimal HTTP/1.1 request reader and response writer over one TCP stream.
//!
//! The workbench server answers one request per connection and closes it. This module reads the
//! request line and headers within a fixed time and size, and writes a response that always says
//! `Connection: close`. It parses no more than the server needs, and it refuses anything it cannot
//! read with a plain error the caller turns into a 400 (ADR-0077 Decision 3, ADR-0199 amendment of
//! 2026-10-15).

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

use super::events::Subscription;

/// The longest request line plus headers the server reads.
pub const HEAD_LIMIT: usize = 16 * 1024;

/// How long a peer has to send its request line and headers, and then its body.
pub const READ_DEADLINE: Duration = Duration::from_secs(10);

/// How long a finished connection keeps reading what the peer still sends, so the peer sees the
/// response instead of a reset.
const DRAIN_DEADLINE: Duration = Duration::from_millis(250);

/// The most a finished connection reads and discards.
const DRAIN_LIMIT: usize = 512 * 1024;

/// Header names. Requests are matched without regard to case.
pub mod header {
    /// The authority the client addressed.
    pub const HOST: &str = "Host";
    /// The bearer.
    pub const AUTHORIZATION: &str = "Authorization";
    /// The body's size.
    pub const CONTENT_LENGTH: &str = "Content-Length";
    /// The body's type.
    pub const CONTENT_TYPE: &str = "Content-Type";
    /// The page that made the request.
    pub const ORIGIN: &str = "Origin";
    /// How the browser classed the request's origin.
    pub const SEC_FETCH_SITE: &str = "Sec-Fetch-Site";
    /// A request to change protocol.
    pub const UPGRADE: &str = "Upgrade";
    /// A request body framing the server does not read.
    pub const TRANSFER_ENCODING: &str = "Transfer-Encoding";
    /// The precondition of a write.
    pub const IF_MATCH: &str = "If-Match";
    /// The tag of a page the client already holds.
    pub const IF_NONE_MATCH: &str = "If-None-Match";
    /// Where a redirect goes.
    pub const LOCATION: &str = "Location";
    /// The methods a path takes.
    pub const ALLOW: &str = "Allow";
    /// How to authenticate.
    pub const WWW_AUTHENTICATE: &str = "WWW-Authenticate";
    /// How long to wait before asking again.
    pub const RETRY_AFTER: &str = "Retry-After";
    /// The tag of a page.
    pub const ETAG: &str = "ETag";
    /// What caches may keep.
    pub const CACHE_CONTROL: &str = "Cache-Control";
    /// The connection ends with the response.
    pub const CONNECTION: &str = "Connection";
    /// Types the browser must not guess.
    pub const NOSNIFF: &str = "X-Content-Type-Options";
    /// What a request may reveal about where it came from.
    pub const REFERRER_POLICY: &str = "Referrer-Policy";
    /// Who may load a response.
    pub const RESOURCE_POLICY: &str = "Cross-Origin-Resource-Policy";
    /// Whether a page may be framed.
    pub const FRAME_OPTIONS: &str = "X-Frame-Options";
    /// What a page may load and do.
    pub const CONTENT_SECURITY_POLICY: &str = "Content-Security-Policy";
    /// The sequence number of the last change an act caused.
    pub const SEQ: &str = "Ghostlight-Seq";
}

/// The methods the server tells apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    /// Read.
    Get,
    /// Read the headers of a read.
    Head,
    /// Act.
    Post,
    /// Replace.
    Put,
    /// Remove.
    Delete,
    /// A browser's cross-origin preflight.
    Options,
    /// Anything else.
    Other,
}

impl Method {
    fn parse(text: &str) -> Self {
        match text {
            "GET" => Self::Get,
            "HEAD" => Self::Head,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            "OPTIONS" => Self::Options,
            _ => Self::Other,
        }
    }

    /// The token as it appears on the request line.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
            Self::Other => "",
        }
    }
}

/// Why a request could not be read.
#[derive(Debug, Eq, PartialEq)]
pub enum ReadError {
    /// The peer closed or stopped sending before the request was complete.
    Closed,
    /// The request is not one the server reads.
    Malformed,
}

/// One request's line and headers.
pub struct Head {
    /// The method.
    pub method: Method,
    /// The path, without its query.
    pub path: String,
    /// The query string, without the question mark.
    pub query: Option<String>,
    headers: Vec<(String, String)>,
    /// The body's declared size.
    pub content_length: usize,
}

/// Prints the request without any header value: one of them is the bearer.
impl std::fmt::Debug for Head {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("Head")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query", &self.query)
            .field("header_names", &names)
            .field("header_values", &"[redacted]")
            .field("content_length", &self.content_length)
            .finish()
    }
}

impl Head {
    /// The value of a header, matched without regard to case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Headers whose value decides access or framing. A request that repeats one is not read.
const SINGLE_VALUED: [&str; 8] = [
    header::HOST,
    header::AUTHORIZATION,
    header::CONTENT_LENGTH,
    header::CONTENT_TYPE,
    header::ORIGIN,
    header::SEC_FETCH_SITE,
    header::IF_MATCH,
    header::IF_NONE_MATCH,
];

/// A connection and the bytes read past the headers.
pub struct Incoming {
    stream: TcpStream,
    buffered: Vec<u8>,
}

impl Incoming {
    /// Read one request's line and headers, which must arrive within `within`.
    pub fn read_head(stream: TcpStream, within: Duration) -> (Self, Result<Head, ReadError>) {
        let mut incoming = Self {
            stream,
            buffered: Vec::new(),
        };
        let head = incoming.head(within);
        (incoming, head)
    }

    fn head(&mut self, within: Duration) -> Result<Head, ReadError> {
        let deadline = Instant::now() + within;
        let mut chunk = [0_u8; 2048];
        let end = loop {
            if let Some(end) = find_head_end(&self.buffered) {
                break end;
            }
            if self.buffered.len() >= HEAD_LIMIT {
                return Err(ReadError::Malformed);
            }
            let count = self.read_some(&mut chunk, deadline)?;
            self.buffered.extend_from_slice(&chunk[..count]);
        };
        if end > HEAD_LIMIT {
            return Err(ReadError::Malformed);
        }
        let text = std::str::from_utf8(&self.buffered[..end]).map_err(|_| ReadError::Malformed)?;
        let head = parse_head(text)?;
        self.buffered.drain(..end + HEAD_END.len());
        Ok(head)
    }

    fn read_some(&mut self, buffer: &mut [u8], deadline: Instant) -> Result<usize, ReadError> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .ok_or(ReadError::Closed)?;
        self.stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| ReadError::Closed)?;
        match self.stream.read(buffer) {
            Ok(0) | Err(_) => Err(ReadError::Closed),
            Ok(count) => Ok(count),
        }
    }

    /// Read exactly `length` body bytes.
    pub fn read_body(&mut self, length: usize) -> Result<Vec<u8>, ReadError> {
        let deadline = Instant::now() + READ_DEADLINE;
        let mut chunk = [0_u8; 4096];
        while self.buffered.len() < length {
            let count = self.read_some(&mut chunk, deadline)?;
            self.buffered.extend_from_slice(&chunk[..count]);
        }
        let rest = self.buffered.split_off(length);
        Ok(std::mem::replace(&mut self.buffered, rest))
    }

    /// Write `response` and close the connection.
    pub fn finish(mut self, response: &Response) {
        let _ = self.stream.set_write_timeout(Some(READ_DEADLINE));
        let _ = response.write_to(&mut self.stream);
        let _ = self.stream.flush();
        let _ = self.stream.shutdown(Shutdown::Write);
        self.drain();
    }

    /// Write the head of `response` and keep the connection open for a body that has no length.
    ///
    /// The body ends when the connection closes. Each write to the returned writer gives up after
    /// `deadline`.
    pub fn begin_stream(
        mut self,
        response: &Response,
        deadline: Duration,
    ) -> io::Result<EventWriter> {
        self.stream.set_write_timeout(Some(deadline))?;
        response.write_stream_head(&mut self.stream)?;
        self.stream.flush()?;
        Ok(EventWriter {
            stream: self.stream,
        })
    }

    /// Read and discard what the peer still sends, so closing does not reset the response.
    fn drain(&mut self) {
        let deadline = Instant::now() + DRAIN_DEADLINE;
        let mut chunk = [0_u8; 4096];
        let mut total = 0;
        while total < DRAIN_LIMIT {
            match self.read_some(&mut chunk, deadline) {
                Ok(count) => total += count,
                Err(_) => break,
            }
        }
    }
}

/// The body of a stream, written as it happens. Dropping it closes the connection.
pub struct EventWriter {
    stream: TcpStream,
}

impl EventWriter {
    /// Write `text` and send it now.
    pub fn write(&mut self, text: &str) -> io::Result<()> {
        self.stream.write_all(text.as_bytes())?;
        self.stream.flush()
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

const HEAD_END: &[u8] = b"\r\n\r\n";

fn find_head_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(HEAD_END.len())
        .position(|window| window == HEAD_END)
}

fn parse_head(text: &str) -> Result<Head, ReadError> {
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or(ReadError::Malformed)?;
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(ReadError::Malformed);
    };
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(ReadError::Malformed);
    }
    if !target.starts_with('/') {
        return Err(ReadError::Malformed);
    }
    if target
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte == b' ')
    {
        return Err(ReadError::Malformed);
    }
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, Some(query.to_owned())),
        None => (target, None),
    };
    let mut headers = Vec::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(ReadError::Malformed)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(ReadError::Malformed);
        }
        let value = value.trim_matches(|c| c == ' ' || c == '\t');
        if value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(ReadError::Malformed);
        }
        headers.push((name.to_owned(), value.to_owned()));
    }
    for name in SINGLE_VALUED {
        if headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .count()
            > 1
        {
            return Err(ReadError::Malformed);
        }
    }
    let content_length = match headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(header::CONTENT_LENGTH))
    {
        None => 0,
        Some((_, value)) => {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(ReadError::Malformed);
            }
            value.parse().map_err(|_| ReadError::Malformed)?
        }
    };
    Ok(Head {
        method: Method::parse(method),
        path: path.to_owned(),
        query,
        headers,
        content_length,
    })
}

/// The statuses the server answers with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// 200.
    Ok,
    /// 202.
    Accepted,
    /// 304.
    NotModified,
    /// 308.
    PermanentRedirect,
    /// 400.
    BadRequest,
    /// 401.
    Unauthorized,
    /// 403.
    Forbidden,
    /// 404.
    NotFound,
    /// 405.
    MethodNotAllowed,
    /// 409.
    Conflict,
    /// 412.
    PreconditionFailed,
    /// 413.
    PayloadTooLarge,
    /// 415.
    UnsupportedMediaType,
    /// 421.
    Misdirected,
    /// 422.
    Unprocessable,
    /// 428.
    PreconditionRequired,
    /// 429.
    TooManyRequests,
    /// 503.
    ServiceUnavailable,
}

impl Status {
    /// The numeric code.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::Accepted => 202,
            Self::NotModified => 304,
            Self::PermanentRedirect => 308,
            Self::BadRequest => 400,
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::Conflict => 409,
            Self::PreconditionFailed => 412,
            Self::PayloadTooLarge => 413,
            Self::UnsupportedMediaType => 415,
            Self::Misdirected => 421,
            Self::Unprocessable => 422,
            Self::PreconditionRequired => 428,
            Self::TooManyRequests => 429,
            Self::ServiceUnavailable => 503,
        }
    }

    const fn reason(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Accepted => "Accepted",
            Self::NotModified => "Not Modified",
            Self::PermanentRedirect => "Permanent Redirect",
            Self::BadRequest => "Bad Request",
            Self::Unauthorized => "Unauthorized",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::Conflict => "Conflict",
            Self::PreconditionFailed => "Precondition Failed",
            Self::PayloadTooLarge => "Payload Too Large",
            Self::UnsupportedMediaType => "Unsupported Media Type",
            Self::Misdirected => "Misdirected Request",
            Self::Unprocessable => "Unprocessable Content",
            Self::PreconditionRequired => "Precondition Required",
            Self::TooManyRequests => "Too Many Requests",
            Self::ServiceUnavailable => "Service Unavailable",
        }
    }
}

/// One response. Every response says `Connection: close`. It carries its `Content-Length`, unless
/// its body is a stream of events, which ends when the connection does.
#[derive(Debug)]
pub struct Response {
    /// The status.
    pub status: Status,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
    head_only: bool,
    events: Option<Subscription>,
}

impl Response {
    /// A response with no body yet.
    #[must_use]
    pub const fn new(status: Status) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            head_only: false,
            events: None,
        }
    }

    /// Make the body a stream of events, which the caller writes after the head.
    #[must_use]
    pub fn with_events(mut self, subscription: Subscription) -> Self {
        self.events = Some(subscription);
        self
    }

    /// Take the stream this response's body is, if it is one.
    pub fn take_events(&mut self) -> Option<Subscription> {
        self.events.take()
    }

    /// Add a header.
    #[must_use]
    pub fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    /// Set the body and its type.
    #[must_use]
    pub fn with_body(mut self, content_type: &str, body: Vec<u8>) -> Self {
        self.headers
            .push((header::CONTENT_TYPE, content_type.to_owned()));
        self.body = body;
        self
    }

    /// Send the headers a read would send, without the body.
    #[must_use]
    pub fn head_only(mut self) -> Self {
        self.head_only = true;
        self
    }

    /// The value of a header already set.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn head(&self, length: Option<usize>) -> String {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\n{}: close\r\n",
            self.status.code(),
            self.status.reason(),
            header::CONNECTION,
        );
        if let Some(length) = length {
            head.push_str(&format!("{}: {length}\r\n", header::CONTENT_LENGTH));
        }
        for (name, value) in &self.headers {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        head
    }

    fn write_to(&self, out: &mut impl Write) -> io::Result<()> {
        out.write_all(self.head(Some(self.body.len())).as_bytes())?;
        if !self.head_only {
            out.write_all(&self.body)?;
        }
        Ok(())
    }

    /// Write the head of a body that has no length: it ends when the connection closes.
    fn write_stream_head(&self, out: &mut impl Write) -> io::Result<()> {
        out.write_all(self.head(None).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_line_and_headers_are_read() {
        let head = parse_head(
            "POST /api/search?x=1 HTTP/1.1\r\nHost: 127.0.0.1:1\r\ncontent-length: 7\r\nX-A:  b ",
        )
        .unwrap();
        assert_eq!(head.method, Method::Post);
        assert_eq!(head.path, "/api/search");
        assert_eq!(head.query.as_deref(), Some("x=1"));
        assert_eq!(head.content_length, 7);
        assert_eq!(head.header("host"), Some("127.0.0.1:1"));
        assert_eq!(head.header("x-a"), Some("b"));
    }

    #[test]
    fn a_request_prints_without_its_bearer() {
        let head = parse_head(
            "GET /api/context HTTP/1.1\r\nHost: 127.0.0.1:1\r\nAuthorization: Bearer s3cr3t-bearer",
        )
        .unwrap();
        let printed = format!("{head:?}");
        assert!(!printed.contains("s3cr3t-bearer"), "{printed}");
        assert!(printed.contains("Authorization"), "{printed}");
        assert!(printed.contains("[redacted]"), "{printed}");
    }

    #[test]
    fn what_the_server_cannot_read_is_malformed() {
        for text in [
            "GET / HTTP/2\r\nHost: a",
            "GET\r\nHost: a",
            "GET / HTTP/1.1 extra\r\nHost: a",
            "GET http://evil.example/ HTTP/1.1\r\nHost: a",
            "GET /a b HTTP/1.1\r\nHost: a",
            "GET / HTTP/1.1\r\nHost a",
            "GET / HTTP/1.1\r\n folded: a",
            "GET / HTTP/1.1\r\nHost: a\r\nHost: b",
            "GET / HTTP/1.1\r\nContent-Length: -1",
            "GET / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 1",
            "GET / HTTP/1.1\r\nX-A: a\u{0001}b",
        ] {
            assert_eq!(
                parse_head(text).unwrap_err(),
                ReadError::Malformed,
                "{text}"
            );
        }
    }

    #[test]
    fn a_response_says_close_and_its_length() {
        let response = Response::new(Status::Ok)
            .with_header(header::ETAG, "\"x\"")
            .with_body("text/plain", b"hi".to_vec());
        let mut out = Vec::new();
        response.write_to(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n"));
        assert!(text.ends_with("\r\n\r\nhi"));
    }

    #[test]
    fn a_head_response_has_the_length_and_no_body() {
        let response = Response::new(Status::Ok)
            .with_body("text/plain", b"hi".to_vec())
            .head_only();
        let mut out = Vec::new();
        response.write_to(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Content-Length: 2"));
        assert!(text.ends_with("\r\n\r\n"));
    }
}
