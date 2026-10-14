//! Forwarding one MCP stdio session to the service over relay protocol 3 (ADR-0197 Decisions 2,
//! 11 and 12).
//!
//! The connector reads client lines from stdin and service lines from an `mcp` relay stream, and
//! passes each one on unchanged. It looks inside a line only to track outstanding request ids per
//! direction, to answer `ping` itself, and to know which requests to answer when the service is
//! cut off or does not come back in time. Those answers are the service's own failure templates;
//! the connector authors no words.
//!
//! A client response reaches the service only when it answers a request the live connection made.
//! A lone response that answers nothing outstanding is dropped, and a batch loses such elements
//! and is sent without them, or not at all when nothing is left. So a late answer to a service
//! that went away never reaches the next one, which may have reused the id.
//!
//! Everything runs on one loop over [`Event`]s. Threads only read: one for stdin, one per service
//! connection, and one per connection attempt. Time comes from an injected [`Clock`] so the bounds
//! can be tested without waiting for them.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::client::ConnectError;
use ghostlight_relay::frame;
use ghostlight_relay::log::Event as LogEvent;
use ghostlight_relay::opening::{McpFrame, Reply, Stream};
use ghostlight_relay::spawn::Backoff;
use ghostlight_relay::templates::{self, Failure, FailureTemplates};
use ghostlight_relay::MAX_FRAME_BYTES;
use serde_json::{json, Value};

use crate::hold::{HeldLine, Hold, Request};

/// The channel this connector names in its opening.
pub const CHANNEL: &str = "mcp";

/// The JSON-RPC member names and values the connector reads or writes.
mod rpc {
    /// The protocol version member.
    pub const JSONRPC: &str = "jsonrpc";
    /// The only protocol version.
    pub const VERSION: &str = "2.0";
    /// The request id member.
    pub const ID: &str = "id";
    /// The method member; its presence makes a message a request or notification.
    pub const METHOD: &str = "method";
    /// The success member of a response.
    pub const RESULT: &str = "result";
    /// The method the connector answers itself.
    pub const PING: &str = "ping";
}

/// The service the connector talks to: how to open a stream and how to start it.
pub trait Service: Send + Sync + 'static {
    /// Connect to the running service and present an opening for `stream`.
    fn connect(&self, stream: Stream) -> Result<(TcpStream, Reply), ConnectError>;

    /// Start the service without waiting for it to be ready.
    fn start(&self) -> io::Result<()>;
}

/// The source of the current time for every bound the connector keeps.
pub trait Clock: Send + Sync + 'static {
    /// The current instant.
    fn now(&self) -> Instant;
}

/// The real clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Where diagnostics go: one closed event and a short, content-free detail.
pub type Log = fn(LogEvent, &str);

/// Everything the forwarding loop needs from the outside world.
pub struct Environment {
    /// The service to connect to and start.
    pub service: Arc<dyn Service>,
    /// The clock behind the hold limit and the backoff.
    pub clock: Arc<dyn Clock>,
    /// The failure-template file, read each time a request must be answered. `None` when the
    /// home cannot be found, which answers like a missing file.
    pub templates: Option<PathBuf>,
    /// The diagnostics writer.
    pub log: Log,
    /// The longest the loop sleeps before looking at the clock again. `None` sleeps until the
    /// next deadline, which is right for the real clock.
    pub poll: Option<Duration>,
}

/// Something the forwarding loop reacts to.
pub enum Event {
    /// One client line, without its newline.
    Line(Vec<u8>),
    /// A client line longer than [`MAX_FRAME_BYTES`], which was skipped.
    LineTooLong,
    /// The client closed stdin.
    InputClosed,
    /// A connection attempt was accepted by a service.
    Connected(TcpStream),
    /// A connection attempt failed; the service was asked to start.
    AttemptFailed,
    /// A frame arrived on the connection of the given generation.
    Frame(u64, McpFrame),
    /// The connection of the given generation ended.
    Gone(u64),
    /// The process that started the connector went away.
    ParentGone,
}

/// Forward one MCP session between `input`, `output` and the service until the input closes or
/// the parent goes away.
///
/// `events` and `inbox` are the two ends of the loop's channel; the caller keeps a sender of its
/// own for [`Event::ParentGone`]. Only a failure to write to `output` is an error.
pub fn run(
    environment: Environment,
    input: impl Read + Send + 'static,
    output: impl Write,
    events: Sender<Event>,
    inbox: Receiver<Event>,
) -> io::Result<()> {
    read_input(input, events.clone());
    let mut forwarder = Forwarder::new(environment, output, events);
    loop {
        forwarder.tick()?;
        let event = match forwarder.wait() {
            Some(wait) => match inbox.recv_timeout(wait) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            },
            None => match inbox.recv() {
                Ok(event) => event,
                Err(_) => return Ok(()),
            },
        };
        if forwarder.handle(event)? == Flow::Exit {
            return Ok(());
        }
    }
}

/// Whether the loop goes on after an event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Flow {
    /// Keep forwarding.
    Continue,
    /// Stop: the input closed or the parent went away.
    Exit,
}

/// The state of one forwarded session.
struct Forwarder<W: Write> {
    environment: Environment,
    output: W,
    events: Sender<Event>,
    /// The live service connection, for writing.
    link: Option<TcpStream>,
    /// Counts connections, so events from an ended one are ignored.
    generation: u64,
    /// Whether any service has accepted this session yet.
    ever_connected: bool,
    /// Whether a connection attempt is running.
    attempting: bool,
    /// When the next connection attempt may run. Read only while there is no link and no
    /// attempt, and set whenever a link ends, so a held request always has one coming.
    next_attempt: Instant,
    backoff: Backoff,
    /// The resume token the service last handed this session.
    resume: Option<String>,
    hold: Hold,
    /// Client requests delivered to the service and not yet answered, by id key.
    client_requests: BTreeMap<String, Request>,
    /// Requests the live connection wrote to the client and that are not yet answered, by id
    /// key. Only answers to these reach the service.
    service_requests: BTreeMap<String, Request>,
}

impl<W: Write> Forwarder<W> {
    /// A session that has not connected yet and tries at once.
    fn new(environment: Environment, output: W, events: Sender<Event>) -> Self {
        let now = environment.clock.now();
        Self {
            environment,
            output,
            events,
            link: None,
            generation: 0,
            ever_connected: false,
            attempting: false,
            next_attempt: now,
            backoff: Backoff::new(),
            resume: None,
            hold: Hold::default(),
            client_requests: BTreeMap::new(),
            service_requests: BTreeMap::new(),
        }
    }

    /// Whether the session needs a service now: before its first connection, and while a
    /// request is held. Never while idle.
    fn wants_service(&self) -> bool {
        !self.ever_connected || self.hold.has_request()
    }

    /// Expire held lines and start a due connection attempt.
    fn tick(&mut self) -> io::Result<()> {
        let now = self.environment.clock.now();
        for expired in self.hold.expire(now) {
            self.answer(&expired.requests, Failure::Expired)?;
        }
        if self.next_attempt <= now
            && self.link.is_none()
            && !self.attempting
            && self.wants_service()
        {
            self.attempt();
        }
        Ok(())
    }

    /// How long to wait for the next event, or `None` to wait without a deadline.
    fn wait(&self) -> Option<Duration> {
        let attempt = Some(self.next_attempt)
            .filter(|_| self.link.is_none() && !self.attempting && self.wants_service());
        let deadline = [attempt, self.hold.next_expiry()]
            .into_iter()
            .flatten()
            .min()?;
        let wait = deadline.saturating_duration_since(self.environment.clock.now());
        Some(self.environment.poll.map_or(wait, |poll| wait.min(poll)))
    }

    /// React to one event.
    fn handle(&mut self, event: Event) -> io::Result<Flow> {
        match event {
            Event::Line(bytes) => self.client_line(&bytes)?,
            Event::LineTooLong => {}
            Event::InputClosed => {
                for held in self.hold.drain() {
                    self.answer(&held.requests, Failure::Expired)?;
                }
                return Ok(Flow::Exit);
            }
            Event::Connected(socket) => self.connected(socket)?,
            Event::AttemptFailed => {
                self.attempting = false;
                self.next_attempt = self.environment.clock.now() + self.backoff.next_wait();
            }
            Event::Frame(generation, frame) => {
                if self.is_current(generation) {
                    self.service_frame(frame)?;
                }
            }
            Event::Gone(generation) => {
                if self.is_current(generation) {
                    (self.environment.log)(LogEvent::ServiceDisconnected, "");
                    self.disconnect()?;
                }
            }
            Event::ParentGone => {
                (self.environment.log)(LogEvent::ParentGone, "");
                return Ok(Flow::Exit);
            }
        }
        Ok(Flow::Continue)
    }

    /// Whether `generation` names the live connection.
    fn is_current(&self, generation: u64) -> bool {
        self.link.is_some() && generation == self.generation
    }

    /// Start one connection attempt on its own thread. A failed attempt starts the service.
    fn attempt(&mut self) {
        self.attempting = true;
        let service = Arc::clone(&self.environment.service);
        let events = self.events.clone();
        let log = self.environment.log;
        let stream = Stream::Mcp {
            channel: CHANNEL.to_owned(),
            session: None,
            resume: self.resume.clone(),
        };
        thread::spawn(move || {
            let event = match service.connect(stream) {
                Ok((socket, Reply::Accepted { .. })) => Event::Connected(socket),
                Ok((_, reply)) => {
                    let detail = match reply {
                        Reply::Refused { code, .. } => format!("{code:?}"),
                        other => format!("{other:?}"),
                    };
                    log(LogEvent::OpeningRefused, &detail);
                    start(service.as_ref(), log);
                    Event::AttemptFailed
                }
                Err(error) => {
                    log(LogEvent::ConnectFailed, &error.to_string());
                    start(service.as_ref(), log);
                    Event::AttemptFailed
                }
            };
            let _ = events.send(event);
        });
    }

    /// Take a newly accepted connection and deliver everything held, in order.
    fn connected(&mut self, socket: TcpStream) -> io::Result<()> {
        self.attempting = false;
        self.backoff.reset();
        self.ever_connected = true;
        let reader = match socket.try_clone() {
            Ok(reader) => reader,
            Err(error) => {
                (self.environment.log)(LogEvent::ConnectFailed, &error.to_string());
                self.next_attempt = self.environment.clock.now();
                return Ok(());
            }
        };
        self.generation += 1;
        read_service(self.generation, reader, self.events.clone());
        self.link = Some(socket);
        (self.environment.log)(LogEvent::ServiceConnected, "");
        let now = self.environment.clock.now();
        for expired in self.hold.expire(now) {
            self.answer(&expired.requests, Failure::Expired)?;
        }
        while self.link.is_some() {
            let Some(held) = self.hold.pop() else {
                break;
            };
            self.deliver(held.line, held.requests, &[])?;
        }
        Ok(())
    }

    /// One line from the client: answer a ping, drop answers the live connection never asked
    /// for, then deliver while connected, otherwise hold.
    fn client_line(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut line = String::from_utf8_lossy(bytes).into_owned();
        if line.ends_with('\r') {
            line.pop();
        }
        if line.trim().is_empty() {
            return Ok(());
        }
        let inspection = Inspection::of(&line);
        if let Some(id) = inspection.ping {
            return self.write_value(&json!({
                rpc::JSONRPC: rpc::VERSION,
                rpc::ID: id,
                rpc::RESULT: {},
            }));
        }
        let (answered, unasked): (Vec<String>, Vec<String>) = inspection
            .responses
            .into_iter()
            .partition(|key| self.service_requests.contains_key(key));
        let line = if unasked.is_empty() {
            line
        } else {
            match without_responses(&line, &unasked) {
                Some(line) => line,
                None => return Ok(()),
            }
        };
        if self.link.is_some() {
            return self.deliver(line, inspection.requests, &answered);
        }
        let held = HeldLine {
            line,
            requests: inspection.requests,
            held_at: self.environment.clock.now(),
        };
        match self.hold.push(held) {
            Ok(()) => Ok(()),
            Err(refused) => self.answer(&refused.requests, Failure::Expired),
        }
    }

    /// Write one client line to the live connection, once.
    ///
    /// Its requests become outstanding before the write, so a write that fails leaves them to be
    /// answered as cut off rather than sent again. A line too large to frame never reaches the
    /// service and its requests are answered as expired.
    fn deliver(
        &mut self,
        line: String,
        requests: Vec<Request>,
        responses: &[String],
    ) -> io::Result<()> {
        let payload = serde_json::to_vec(&McpFrame::Message { line }).map_err(io::Error::other)?;
        if payload.len() > MAX_FRAME_BYTES {
            return self.answer(&requests, Failure::Expired);
        }
        for request in requests {
            self.client_requests.insert(request.key.clone(), request);
        }
        for key in responses {
            self.service_requests.remove(key);
        }
        let Some(link) = self.link.as_mut() else {
            return Ok(());
        };
        if let Err(error) = frame::write_frame_raw(link, &payload) {
            (self.environment.log)(LogEvent::ServiceDisconnected, &error.to_string());
            self.disconnect()?;
        }
        Ok(())
    }

    /// One frame from the service: pass a line to the client, or keep a resume token.
    fn service_frame(&mut self, frame: McpFrame) -> io::Result<()> {
        match frame {
            McpFrame::Message { line } => {
                let inspection = Inspection::of(&line);
                for request in inspection.requests {
                    self.service_requests.insert(request.key.clone(), request);
                }
                for key in &inspection.responses {
                    self.client_requests.remove(key);
                }
                self.write_line(line.as_bytes())
            }
            McpFrame::Resume { token } => {
                self.resume = Some(token);
                Ok(())
            }
            McpFrame::Unknown => Ok(()),
        }
    }

    /// Drop the connection, answer every outstanding client request as cut off, and let the next
    /// attempt run as soon as a held request wants one.
    ///
    /// Requests the service made of the client are forgotten, so the client's late answers to them
    /// are dropped rather than sent to whichever service comes next.
    fn disconnect(&mut self) -> io::Result<()> {
        if let Some(link) = self.link.take() {
            let _ = link.shutdown(Shutdown::Both);
        }
        self.next_attempt = self.environment.clock.now();
        self.service_requests.clear();
        let cut_off: Vec<Request> = std::mem::take(&mut self.client_requests)
            .into_values()
            .collect();
        self.answer(&cut_off, Failure::CutOff)
    }

    /// Answer each request with the service's template for `failure`, or the bare error.
    fn answer(&mut self, requests: &[Request], failure: Failure) -> io::Result<()> {
        if requests.is_empty() {
            return Ok(());
        }
        let templates = self
            .environment
            .templates
            .as_deref()
            .and_then(|path| FailureTemplates::read(path).ok().flatten());
        for request in requests {
            let response = templates::failure_response(
                templates.as_ref(),
                failure,
                &request.method,
                &request.id,
            );
            self.write_value(&response)?;
        }
        Ok(())
    }

    /// Write one JSON value as a line to the client.
    fn write_value(&mut self, value: &Value) -> io::Result<()> {
        let line = serde_json::to_vec(value).map_err(io::Error::other)?;
        self.write_line(&line)
    }

    /// Write one line to the client and flush it.
    fn write_line(&mut self, line: &[u8]) -> io::Result<()> {
        self.output.write_all(line)?;
        self.output.write_all(b"\n")?;
        self.output.flush()
    }
}

/// Start the service after a failed attempt, logging the outcome.
fn start(service: &dyn Service, log: Log) {
    log(LogEvent::ServiceStartAttempted, "");
    if let Err(error) = service.start() {
        log(LogEvent::ServiceStartFailed, &error.to_string());
    }
}

/// What one line says about request ids.
#[derive(Debug, Default)]
struct Inspection {
    /// Requests the line makes: elements with a method and an id.
    requests: Vec<Request>,
    /// Id keys the line answers: elements with an id and no method.
    responses: Vec<String>,
    /// The id of a lone `ping` request. The connector answers a client's ping itself; a service's
    /// ping is an ordinary request and is also in `requests`.
    ping: Option<Value>,
}

impl Inspection {
    /// Inspect a line. A batch array is inspected element by element; a line that is not JSON
    /// says nothing.
    fn of(line: &str) -> Self {
        let mut inspection = Self::default();
        match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(elements)) => {
                elements.iter().for_each(|element| inspection.note(element))
            }
            Ok(element) => {
                if element.get(rpc::METHOD).and_then(Value::as_str) == Some(rpc::PING) {
                    inspection.ping = element.get(rpc::ID).cloned();
                }
                inspection.note(&element);
            }
            Err(_) => {}
        }
        inspection
    }

    /// Record one message: a request when it has a method and an id, a response when it has an
    /// id and no method.
    fn note(&mut self, element: &Value) {
        let Some(id) = element.get(rpc::ID) else {
            return;
        };
        let key = id.to_string();
        match element.get(rpc::METHOD) {
            Some(method) => self.requests.push(Request {
                key,
                id: id.clone(),
                method: method.as_str().unwrap_or_default().to_owned(),
            }),
            None => self.responses.push(key),
        }
    }
}

/// `line` without the responses whose id key is in `keys`, or `None` when nothing is left.
///
/// Only a batch keeps other elements; it is written back as compact JSON without the dropped
/// ones. A lone response in `keys` leaves nothing.
fn without_responses(line: &str, keys: &[String]) -> Option<String> {
    let Ok(Value::Array(mut elements)) = serde_json::from_str::<Value>(line) else {
        return None;
    };
    elements.retain(|element| {
        let unasked = element.get(rpc::METHOD).is_none()
            && element
                .get(rpc::ID)
                .is_some_and(|id| keys.contains(&id.to_string()));
        !unasked
    });
    if elements.is_empty() {
        return None;
    }
    serde_json::to_string(&elements).ok()
}

/// One line read from the client.
enum InputLine {
    /// A line within the bound, without its newline.
    Line(Vec<u8>),
    /// A line over the bound; its bytes were skipped up to its newline.
    TooLong,
}

/// Read client lines on their own thread and post them as events, then [`Event::InputClosed`].
fn read_input(input: impl Read + Send + 'static, events: Sender<Event>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(input);
        loop {
            let event = match read_line(&mut reader, MAX_FRAME_BYTES) {
                Ok(Some(InputLine::Line(bytes))) => Event::Line(bytes),
                Ok(Some(InputLine::TooLong)) => Event::LineTooLong,
                Ok(None) | Err(_) => {
                    let _ = events.send(Event::InputClosed);
                    return;
                }
            };
            if events.send(event).is_err() {
                return;
            }
        }
    });
}

/// Read one newline-terminated line of at most `limit` bytes, or `None` at the end of input.
///
/// A longer line is consumed to its end without being kept, so the next read starts on a line
/// boundary.
fn read_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Option<InputLine>> {
    let mut line = Vec::new();
    let mut started = false;
    let mut too_long = false;
    loop {
        let buffer = match reader.fill_buf() {
            Ok(buffer) => buffer,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if buffer.is_empty() {
            if !started {
                return Ok(None);
            }
            break;
        }
        started = true;
        let newline = buffer.iter().position(|&byte| byte == b'\n');
        let chunk = &buffer[..newline.unwrap_or(buffer.len())];
        if !too_long {
            if line.len() + chunk.len() > limit {
                too_long = true;
                line = Vec::new();
            } else {
                line.extend_from_slice(chunk);
            }
        }
        let used = newline.map_or(buffer.len(), |position| position + 1);
        reader.consume(used);
        if newline.is_some() {
            break;
        }
    }
    Ok(Some(if too_long {
        InputLine::TooLong
    } else {
        InputLine::Line(line)
    }))
}

/// Read frames from one service connection on their own thread, then [`Event::Gone`].
///
/// A frame that is not an MCP frame is skipped; a stream that ends or cannot be framed ends the
/// connection.
fn read_service(generation: u64, mut socket: TcpStream, events: Sender<Event>) {
    thread::spawn(move || loop {
        match frame::read_frame_raw(&mut socket) {
            Ok(Some(payload)) => {
                let Ok(frame) = serde_json::from_slice::<McpFrame>(&payload) else {
                    continue;
                };
                if events.send(Event::Frame(generation, frame)).is_err() {
                    return;
                }
            }
            Ok(None) | Err(_) => {
                let _ = events.send(Event::Gone(generation));
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests;
