// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The local web API (ADR-0030 Decision 9): HTTP/1.1 + WebSocket over TCP, a SECOND session
//! SOURCE into the same Hub a local app (the ADR's ".NET Automate") drives the browser through.
//! It reuses the UNCHANGED multiplex (Decision 2), identity (Decision 4), and isolation
//! (Decision 6) by calling the SAME `transport::mcp::server::serve_session` every MCP adapter
//! session calls -- it invents no parallel dispatch path. It has its OWN non-sacred, versioned
//! REST/WS vocabulary and NEVER re-serializes the 13 trained schemas
//! (`transport::mcp::tools::TOOLS_JSON`).
//!
//! The listener BINDS PER RESOLVED POLICY (Decision 9 + Decision 5): the web adapter's builtin
//! default policy fragment is `channels.webapi.from: [allow: "localhost"]` (the ADR-0019 builtin
//! layer, contributed per-adapter), so with no overlay it binds `127.0.0.1` explicitly, never
//! `0.0.0.0`; a remote bind happens ONLY because a user/org layer opened it
//! ([`resolve_bind`] is a PURE function of the resolved allowlist -- no other input). This task
//! does not yet wire a `ConfigStore`-resolved override for `channels.webapi.from`/`webapi.bind`
//! (deferred -- see the H8 ledger entry); today the running service always resolves to the
//! builtin default, which is the ONLY case any pinned test exercises.
//!
//! Authorization is the `channels.webapi.from` policy, decided by
//! [`crate::governance::channels::ChannelsPdp`] on the connecting SOURCE (Origin, or the peer's
//! classified address when no Origin is presented); authentication is optional and anonymous is
//! a first-class principal (Decision 5). The WS upgrade also rejects an unexpected `Host` header
//! (DNS-rebind defense, Decision 9).
//!
//! The WebSocket framing here is a deliberately minimal RFC 6455 subset: unfragmented text/binary
//! data frames are tunneled as a raw byte stream (message boundaries carry no meaning -- exactly
//! like the stdio/pipe streams `serve_session` already speaks over), close frames end the read
//! side cleanly, and ping/pong control frames are parsed and discarded rather than answered. This
//! is sufficient for a governed JSON-RPC tool-call channel; no pinned test in this batch
//! exercises the wire beyond the handshake, so this scope is a deliberate, documented
//! limitation, not a gap discovered later.

use crate::governance::channels::ChannelsPdp;
use crate::governance::ports::{Decision, PolicyDecisionPoint};
use crate::hub::session::SessionGuid;
use crate::hub::ServiceContext;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};

/// The web adapter's BUILTIN default policy fragment (ADR-0030 Decision 5; PINS.md SS7):
/// `channels.webapi.from: { allow: ["localhost"] }`. Contributed by this adapter, exactly as
/// Decision 5 describes ("each adapter ships a default policy fragment").
pub fn builtin_webapi_from() -> Vec<String> {
    vec!["localhost".to_string()]
}

/// Loopback bind (PINNED default, `docs/tasks/hub/PINS.md` SS7): bound EXPLICITLY, never
/// `0.0.0.0`.
pub const DEFAULT_WEBAPI_BIND: &str = "127.0.0.1";

/// The remote-open bind this batch uses once the resolved allowlist opens beyond `"localhost"`.
pub const REMOTE_WEBAPI_BIND: &str = "0.0.0.0";

/// PINNED default port, `docs/tasks/hub/PINS.md` SS7.
pub const DEFAULT_WEBAPI_PORT: u16 = 4180;

/// The pure "resolved allowlist -> bind address" function (ADR-0030 Decision 9, H8 Required
/// behavior item 2; `tests/webapi_auth.rs`). Its ONLY input is the resolved
/// `channels.webapi.from` allowlist -- there is no separate boolean/flag/env gate: a remote bind
/// happens only because the policy layer changed (Decision 5).
pub fn resolve_bind(allowlist: &[String]) -> &'static str {
    let opens_remote = allowlist.iter().any(|pattern| pattern != "localhost");
    if opens_remote {
        REMOTE_WEBAPI_BIND
    } else {
        DEFAULT_WEBAPI_BIND
    }
}

/// Classify a connecting peer's address into the `channels.webapi.from` source vocabulary
/// (`"localhost"` for a loopback peer, else its literal address) -- the same vocabulary
/// `builtin_webapi_from`'s `"localhost"` member matches against.
pub fn classify_source(addr: IpAddr) -> String {
    if addr.is_loopback() {
        "localhost".to_string()
    } else {
        addr.to_string()
    }
}

const MAX_HANDSHAKE_BYTES: usize = 16 * 1024;

/// Run the local web API listener for the life of the service (ADR-0030 Decision 9). Binds per
/// [`resolve_bind`] over the builtin default allowlist. A bind failure (e.g. the port is already
/// in use by another process, or by another Ghostlight service instance in a test run) is
/// LOGGED, never fatal: the web API is simply unavailable for this service instance, exactly like
/// the extension endpoint's `SessionBusy` handling in `run_service_loop` -- MCP/adapter
/// multiplexing must never be affected by this second, optional session source.
pub async fn run(ctx: ServiceContext) {
    let allowlist = builtin_webapi_from();
    let bind = resolve_bind(&allowlist);
    let addr = format!("{bind}:{DEFAULT_WEBAPI_PORT}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::warn!(
                error = %e,
                addr,
                "local web API TCP listener failed to bind; the web API is unavailable for this \
                 service instance"
            );
            return;
        }
    };
    tracing::info!(addr, "local web API listening");
    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(error = %e, "local web API accept failed");
                continue;
            }
        };
        let ctx = ctx.clone();
        let allowlist = allowlist.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, peer_addr, ctx, allowlist, bind).await {
                tracing::debug!(error = %e, "local web API connection ended with an error");
            }
        });
    }
}

/// One accepted TCP connection: read and validate the HTTP/1.1 WebSocket upgrade request,
/// authorize its connecting source against `channels.webapi.from` (Decision 5), validate `Host`
/// (DNS-rebind defense, Decision 9), complete the handshake, then hand off to the UNCHANGED
/// `serve_session` -- the SAME governance chokepoint every MCP adapter session enters (Decision
/// 2/4/6; H8 Required behavior item 1).
async fn handle_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    ctx: ServiceContext,
    allowlist: Vec<String>,
    bind: &'static str,
) -> crate::Result<()> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let (request, consumed) = loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Ok(()); // peer closed before completing the handshake request
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(parsed) = parse_http_request(&buf) {
            break parsed;
        }
        if buf.len() > MAX_HANDSHAKE_BYTES {
            return Ok(()); // refuse an oversized/never-terminating handshake
        }
    };

    let Some(client_key) = header(&request.headers, "Sec-WebSocket-Key") else {
        write_http_error(&mut stream, 400, "Bad Request").await?;
        return Ok(());
    };
    let client_key = client_key.to_string();
    let is_upgrade = request.method.eq_ignore_ascii_case("GET")
        && header(&request.headers, "Upgrade")
            .map(|v| v.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false);
    if !is_upgrade {
        write_http_error(&mut stream, 400, "Bad Request").await?;
        return Ok(());
    }

    // DNS-rebind defense (ADR-0030 Decision 9, Required behavior item 5): reject an unexpected
    // Host before doing anything else with this connection.
    let host_ok = header(&request.headers, "Host")
        .map(|h| host_is_expected(h, bind))
        .unwrap_or(false);
    if !host_ok {
        write_http_error(&mut stream, 400, "Bad Request").await?;
        return Ok(());
    }

    // Origin validated against the resolved channels.webapi.from policy (Required behavior item
    // 5); a non-browser caller with no Origin falls back to the classified peer source so the
    // channels decision still runs (Required behavior item 3: anonymous is a first-class
    // principal, never a hardcoded gate).
    let peer_source = classify_source(peer_addr.ip());
    let source = header(&request.headers, "Origin")
        .and_then(origin_hostname)
        .unwrap_or(peer_source);

    let manifest_hash = ctx
        .initial_policy
        .manifest
        .as_ref()
        .map(|m| m.hash.clone())
        .unwrap_or_default();
    let pdp = ChannelsPdp::new(allowlist);
    let decision_req = channel_decision_request(source.clone(), manifest_hash);
    match pdp.decide(&decision_req) {
        Decision::Allow { .. } => {}
        other => {
            tracing::info!(source = %source, decision = ?other, "web API connection refused by channels.webapi.from");
            write_http_error(&mut stream, 403, "Forbidden").await?;
            return Ok(());
        }
    }

    let accept = compute_accept_key(&client_key);
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(response.as_bytes()).await?;

    // Any bytes received after the header block are already-pipelined WS frame bytes; they must
    // not be dropped.
    let leftover = buf[consumed..].to_vec();
    let guid = SessionGuid::mint();
    let ws = WsStream::new(stream, leftover);
    crate::transport::mcp::server::serve_session(ws, ctx, guid).await
}

/// Build the minimal [`crate::governance::ports::DecisionRequest`] a channels decision needs
/// (every OTHER field is irrelevant to [`ChannelsPdp::decide`], which reads only
/// `channel_source`/`manifest_hash`).
fn channel_decision_request(
    source: String,
    manifest_hash: String,
) -> crate::governance::ports::DecisionRequest {
    use crate::governance::ports::{DecisionRequest, EffectiveMode, GoverningResource};
    DecisionRequest {
        grants: Vec::new(),
        tool: String::new(),
        action: None,
        requires: Vec::new(),
        resource: GoverningResource::None,
        manifest_mode: None,
        config_mode: EffectiveMode::Enforce,
        manifest_hash,
        channel_source: Some(source),
    }
}

/// `true` iff `host_header`'s hostname (the part before an optional trailing `:port`) is an
/// expected loopback alias when `bind` is the loopback default; a remote bind imposes no further
/// restriction here (an operator already opened remote deliberately, Decision 5).
fn host_is_expected(host_header: &str, bind: &str) -> bool {
    if bind != DEFAULT_WEBAPI_BIND {
        return true;
    }
    let hostname = host_header.rsplit_once(':').map_or(host_header, |(h, _)| h);
    hostname == "127.0.0.1" || hostname == "localhost"
}

/// Extract the hostname portion of an `Origin` header (`scheme://host[:port]`), or `None` if it
/// does not parse as `scheme://host...`.
fn origin_hostname(origin: &str) -> Option<String> {
    let after_scheme = origin.split_once("://")?.1;
    let host_port = after_scheme
        .split(['/', '\\'])
        .next()
        .unwrap_or(after_scheme);
    let hostname = host_port.rsplit_once(':').map_or(host_port, |(h, _)| h);
    if hostname.is_empty() {
        None
    } else if hostname == "127.0.0.1" || hostname == "[::1]" {
        Some("localhost".to_string())
    } else {
        Some(hostname.to_string())
    }
}

struct HttpRequest {
    method: String,
    headers: Vec<(String, String)>,
}

/// Parse the request line + headers of an HTTP/1.1 request out of `buf`. Returns `None` until a
/// full `\r\n\r\n` header terminator has been received. The returned `usize` is the number of
/// bytes the header block consumed (any trailing bytes in `buf` belong to the next protocol
/// layer -- WS frames, once upgraded).
fn parse_http_request(buf: &[u8]) -> Option<(HttpRequest, usize)> {
    let text = std::str::from_utf8(buf).ok()?;
    let header_end = text.find("\r\n\r\n")?;
    let head = &text[..header_end];
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    Some((HttpRequest { method, headers }, header_end + 4))
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

async fn write_http_error(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
) -> std::io::Result<()> {
    let response =
        format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
    stream.write_all(response.as_bytes()).await
}

// --- RFC 6455 handshake primitives (hand-rolled: no new crate for a well-defined public
// standard's fixed constants/algorithms) ---

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// `base64(SHA1(client_key + WS_GUID))` (RFC 6455 section 1.3).
fn compute_accept_key(client_key: &str) -> String {
    let mut combined = client_key.as_bytes().to_vec();
    combined.extend_from_slice(WS_GUID.as_bytes());
    base64_encode(&sha1(&combined))
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Textbook SHA-1 (FIPS 180-1), hand-rolled: needed only for the WS handshake's
/// `Sec-WebSocket-Accept` computation, a well-defined public algorithm, not a project decision.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

// --- Minimal RFC 6455 data-frame tunnel (byte-stream semantics; see module doc) ---

const OP_CONTINUATION: u8 = 0x0;
const OP_TEXT: u8 = 0x1;
const OP_BINARY: u8 = 0x2;
const OP_CLOSE: u8 = 0x8;
const OP_PING: u8 = 0x9;
const OP_PONG: u8 = 0xA;

/// Cap on a single declared frame payload length, guarding against a hostile/garbled length
/// field forcing an unbounded allocation. Well under `native::host::MAX_MESSAGE_LEN`; this is the
/// web API's own vocabulary, never the frozen extension wire.
const MAX_FRAME_LEN: u64 = 64 * 1024 * 1024;

/// Decode one frame from the front of `buf`. `Ok(None)` means "not enough bytes yet"; `Err(())`
/// means the frame is malformed or declares an oversized length (the caller treats this as a
/// closed connection). Client frames MUST be masked (RFC 6455 section 5.1); this is enforced.
fn decode_frame(buf: &[u8]) -> Result<Option<(u8, Vec<u8>, usize)>, ()> {
    if buf.len() < 2 {
        return Ok(None);
    }
    let opcode = buf[0] & 0x0F;
    let masked = buf[1] & 0x80 != 0;
    let len7 = buf[1] & 0x7F;

    let (payload_len, mut offset): (u64, usize) = match len7 {
        126 => {
            if buf.len() < 4 {
                return Ok(None);
            }
            (u16::from_be_bytes([buf[2], buf[3]]) as u64, 4)
        }
        127 => {
            if buf.len() < 10 {
                return Ok(None);
            }
            let mut len_bytes = [0u8; 8];
            len_bytes.copy_from_slice(&buf[2..10]);
            (u64::from_be_bytes(len_bytes), 10)
        }
        n => (n as u64, 2),
    };
    if payload_len > MAX_FRAME_LEN {
        return Err(());
    }
    if !masked {
        return Err(());
    }
    if buf.len() < offset + 4 {
        return Ok(None);
    }
    let mask = [
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ];
    offset += 4;
    let payload_len = payload_len as usize;
    if buf.len() < offset + payload_len {
        return Ok(None);
    }
    let mut payload = buf[offset..offset + payload_len].to_vec();
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
    Ok(Some((opcode, payload, offset + payload_len)))
}

/// Encode one unmasked server-to-client frame (RFC 6455 section 5.2: a server MUST NOT mask its
/// frames). Always sent with FIN=1; see the module doc for why fragmentation is irrelevant here.
fn encode_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 10);
    out.push(0x80 | opcode);
    let len = payload.len();
    if len < 126 {
        out.push(len as u8);
    } else if len <= 0xFFFF {
        out.push(126);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        out.push(127);
        out.extend_from_slice(&(len as u64).to_be_bytes());
    }
    out.extend_from_slice(payload);
    out
}

/// Adapts an accepted, already-upgraded TCP connection into the `AsyncRead + AsyncWrite` byte
/// stream `serve_session` expects, tunneling bytes through minimal RFC 6455 data frames (see the
/// module doc for the documented framing-scope limitation). `leftover` seeds already-received,
/// pipelined bytes from the handshake read.
struct WsStream {
    inner: TcpStream,
    read_raw: Vec<u8>,
    read_ready: std::collections::VecDeque<u8>,
    read_eof: bool,
    write_buf: Vec<u8>,
}

impl WsStream {
    fn new(inner: TcpStream, leftover: Vec<u8>) -> Self {
        Self {
            inner,
            read_raw: leftover,
            read_ready: std::collections::VecDeque::new(),
            read_eof: false,
            write_buf: Vec::new(),
        }
    }
}

impl AsyncRead for WsStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        loop {
            if !this.read_ready.is_empty() {
                let n = buf.remaining().min(this.read_ready.len());
                let chunk: Vec<u8> = this.read_ready.drain(..n).collect();
                buf.put_slice(&chunk);
                return Poll::Ready(Ok(()));
            }
            if this.read_eof {
                return Poll::Ready(Ok(()));
            }
            match decode_frame(&this.read_raw) {
                Ok(Some((opcode, payload, consumed))) => {
                    this.read_raw.drain(..consumed);
                    match opcode {
                        OP_TEXT | OP_BINARY | OP_CONTINUATION => {
                            this.read_ready.extend(payload);
                        }
                        OP_CLOSE => {
                            this.read_eof = true;
                        }
                        OP_PING | OP_PONG => {
                            // Ignored (documented limitation): no pong reply is sent.
                        }
                        _ => {}
                    }
                    continue;
                }
                Ok(None) => {}
                Err(()) => {
                    this.read_eof = true;
                    continue;
                }
            }

            let mut tmp = [0u8; 4096];
            let mut tmp_buf = ReadBuf::new(&mut tmp);
            match Pin::new(&mut this.inner).poll_read(cx, &mut tmp_buf) {
                Poll::Ready(Ok(())) => {
                    if tmp_buf.filled().is_empty() {
                        this.read_eof = true;
                    } else {
                        this.read_raw.extend_from_slice(tmp_buf.filled());
                    }
                    continue;
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl AsyncWrite for WsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        while !this.write_buf.is_empty() {
            match Pin::new(&mut this.inner).poll_write(cx, &this.write_buf) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "failed to write WS frame bytes",
                    )))
                }
                Poll::Ready(Ok(n)) => {
                    this.write_buf.drain(..n);
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        this.write_buf = encode_frame(OP_TEXT, buf);
        while !this.write_buf.is_empty() {
            match Pin::new(&mut this.inner).poll_write(cx, &this.write_buf) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "failed to write WS frame bytes",
                    )))
                }
                Poll::Ready(Ok(n)) => {
                    this.write_buf.drain(..n);
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => break,
            }
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        while !this.write_buf.is_empty() {
            match Pin::new(&mut this.inner).poll_write(cx, &this.write_buf) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "failed to write WS frame bytes",
                    )))
                }
                Poll::Ready(Ok(n)) => {
                    this.write_buf.drain(..n);
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        while !this.write_buf.is_empty() {
            match Pin::new(&mut this.inner).poll_write(cx, &this.write_buf) {
                Poll::Ready(Ok(0)) => break,
                Poll::Ready(Ok(n)) => {
                    this.write_buf.drain(..n);
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_default_is_loopback_only() {
        let allowlist = builtin_webapi_from();
        assert_eq!(allowlist, vec!["localhost".to_string()]);
        assert_eq!(resolve_bind(&allowlist), DEFAULT_WEBAPI_BIND);
        assert_ne!(resolve_bind(&allowlist), REMOTE_WEBAPI_BIND);
    }

    #[test]
    fn a_remote_allowlist_resolves_to_a_remote_bind() {
        let allowlist = vec!["*".to_string()];
        assert_eq!(resolve_bind(&allowlist), REMOTE_WEBAPI_BIND);
    }

    #[test]
    fn classify_source_recognizes_loopback() {
        assert_eq!(classify_source("127.0.0.1".parse().unwrap()), "localhost");
        assert_eq!(classify_source("::1".parse().unwrap()), "localhost");
        assert_eq!(
            classify_source("203.0.113.7".parse().unwrap()),
            "203.0.113.7"
        );
    }

    /// RFC 6455 section 1.3's own worked example: pinned by the published standard, not
    /// authored by this batch.
    #[test]
    fn accept_key_matches_the_rfc6455_worked_example() {
        assert_eq!(
            compute_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn ws_frame_round_trips_through_encode_and_decode() {
        let payload = b"hello ghostlight";
        // A masked "client" frame: reuse encode_frame's length-prefix logic, then mask it by
        // hand (encode_frame itself only ever emits UNMASKED server frames).
        let mut framed = encode_frame(OP_TEXT, payload);
        // framed[0] = FIN|opcode, framed[1] = length byte (payload is short, no extended length).
        let header_len = 2;
        let mask = [0x11, 0x22, 0x33, 0x44];
        framed[1] |= 0x80;
        let mut masked_payload: Vec<u8> = payload.to_vec();
        for (i, b) in masked_payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
        let mut client_frame = framed[..header_len].to_vec();
        client_frame.extend_from_slice(&mask);
        client_frame.extend_from_slice(&masked_payload);

        let (opcode, decoded, consumed) = decode_frame(&client_frame).unwrap().unwrap();
        assert_eq!(opcode, OP_TEXT);
        assert_eq!(decoded, payload);
        assert_eq!(consumed, client_frame.len());
    }

    #[test]
    fn decode_frame_needs_more_bytes_reports_none() {
        assert_eq!(decode_frame(&[0x81]), Ok(None));
    }

    #[test]
    fn decode_frame_rejects_an_unmasked_client_frame() {
        // FIN|TEXT, length 5, no mask bit set: a client frame MUST be masked.
        let frame = [0x81, 0x05, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(decode_frame(&frame), Err(()));
    }

    #[test]
    fn host_is_expected_accepts_loopback_aliases_and_rejects_others_under_the_default_bind() {
        assert!(host_is_expected("127.0.0.1:4180", DEFAULT_WEBAPI_BIND));
        assert!(host_is_expected("localhost:4180", DEFAULT_WEBAPI_BIND));
        assert!(!host_is_expected("evil.example.com", DEFAULT_WEBAPI_BIND));
        assert!(host_is_expected("evil.example.com", REMOTE_WEBAPI_BIND));
    }

    #[test]
    fn origin_hostname_normalizes_loopback_forms() {
        assert_eq!(
            origin_hostname("http://localhost:4180"),
            Some("localhost".to_string())
        );
        assert_eq!(
            origin_hostname("http://127.0.0.1:4180"),
            Some("localhost".to_string())
        );
        assert_eq!(
            origin_hostname("https://evil.example.com"),
            Some("evil.example.com".to_string())
        );
        assert_eq!(origin_hostname("not-a-url"), None);
    }
}
