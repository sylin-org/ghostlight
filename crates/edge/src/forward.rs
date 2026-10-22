//! Forwarding one MCP stdio session to the service over relay protocol 4 (ADR-0197 Decisions 2,
//! 11 and 12).
//!
//! The edge reads client lines from stdin and service lines from an `mcp` relay stream, and
//! passes each one on unchanged. It looks inside a line only to track outstanding request ids per
//! direction, to answer `ping` itself, and to know which requests to answer when the service is
//! cut off or does not come back in time, or when a line is too large to forward. Those answers
//! are the failure templates the service handed over in its last accepted reply; the edge authors
//! no words, and before any reply, or with no template for the failure, it answers the bare
//! internal error.
//!
//! A line over [`MAX_FRAME_BYTES`] is skipped up to its newline without being kept, and each
//! request it holds is answered from the `too_large` template ([`scan`], ADR-0197 amendment of
//! 2026-10-21); a shorter line whose frame would pass the bound is answered the same way. Either
//! way nothing reaches the service, and the next line is read as usual.
//!
//! Two kinds of request are held for the next service instead of being answered as cut off
//! (ADR-0197 amendment of 2026-10-17). A line whose write failed never reached the service, since a
//! service acts only on a whole frame, so it is held like any line sent while the service was
//! away. And a lone handshake or catalog listing ([`rpc::SENT_AGAIN`]) changes nothing, so when a
//! lost connection or a `replace` goodbye cuts it off it is sent again to the next service, and
//! the agent gets its answer instead of an error. Every other cut-off request may have taken
//! effect and is answered, never sent again.
//!
//! A client response reaches the service only when it answers a request the live connection made.
//! A lone response that answers nothing outstanding is dropped, and a batch loses such elements
//! and is sent without them, or not at all when nothing is left. So a late answer to a service
//! that went away never reaches the next one, which may have reused the id.
//!
//! The service says goodbye before it closes the stream on purpose, and the edge acts on the
//! reason (ADR-0199 Decision 10). After `quit` every outstanding request is answered as cut off
//! and nothing is held: the next request starts the service again, as an `agent` start. After
//! `replace` lines are held and the edge reconnects without starting the service for up to
//! [`HOLD_LIMIT`], and then behaves as after a lost connection. After `uninstall` outstanding
//! requests are answered and the edge exits. After `session_end` the edge exits too, and
//! it exits the same way when its own session listener hears that the session is ending
//! ([`Event::SessionEnded`]): nothing is started then (ADR-0199 Decision 13). A stream that ends
//! with no goodbye is a crash and behaves as a lost connection.
//!
//! The first time the edge is without a service it registers its wake endpoint
//! ([`Service::watch`], ADR-0199 amendment of 2026-10-20). When a service that became ready wakes
//! it, an edge with no connection and no attempt running connects at once and never starts the
//! service, so an edge left idle by a `quit` comes back too. The schedule stays as it was.
//!
//! Everything runs on one loop over [`Event`]s. Threads only read: one for stdin, one per service
//! connection, and one per connection attempt. Time comes from an injected [`Clock`] so the bounds
//! can be tested without waiting for them.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::endpoint::{ConnectError, Connection};
use ghostlight_relay::frame;
use ghostlight_relay::log::Event as LogEvent;
use ghostlight_relay::protocol::{
    failure_response, Edge, Failure, GoodbyeReason, McpFrame, Opening, Program, Reply, Role,
    StartReason, Templates, MCP_CHANNEL,
};
use ghostlight_relay::spawn::Backoff;
use ghostlight_relay::{HOLD_LIMIT, MAX_FRAME_BYTES};
use serde_json::{json, Value};

use crate::hold::{HeldLine, Hold, Request};

mod scan;

/// The JSON-RPC member names and values the edge reads or writes.
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
    /// The method the edge answers itself.
    pub const PING: &str = "ping";
    /// The requests that change nothing: the handshake and the catalog listings. Cut off by a
    /// lost service, a lone one is sent again to the next service rather than answered.
    pub const SENT_AGAIN: [&str; 5] = [
        "initialize",
        "tools/list",
        "prompts/list",
        "resources/list",
        "resources/templates/list",
    ];
}

/// Why this edge starts the service: an agent needs it.
const START_REASON: StartReason = StartReason::Agent;

/// The service the edge talks to: how to open a stream and how to start it.
pub trait Service: Send + Sync + 'static {
    /// Connect to the running service and present `opening`.
    fn connect(&self, opening: &Opening) -> Result<(Connection, Reply), ConnectError>;

    /// Start the service for `reason` without waiting for it to be ready.
    fn start(&self, reason: StartReason) -> io::Result<()>;

    /// Call `ready` each time a service that became ready wakes this edge, for as long as the
    /// process runs. Fails when the edge cannot register its wake endpoint.
    fn watch(&self, ready: Box<dyn Fn() + Send>) -> io::Result<()>;
}

/// The source of the current time for every bound the edge keeps.
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
    /// A client line longer than [`MAX_FRAME_BYTES`], which was skipped, with the requests it
    /// held.
    LineTooLong(Vec<Request>),
    /// The client closed stdin.
    InputClosed,
    /// A connection attempt was accepted by a service, with the failure templates its reply
    /// carried.
    Connected(Connection, Option<Templates>),
    /// A connection attempt failed; the service was asked to start.
    AttemptFailed,
    /// A frame arrived on the connection of the given generation.
    Frame(u64, McpFrame),
    /// The connection of the given generation ended.
    Gone(u64),
    /// The process that started the edge went away.
    ParentGone,
    /// The session is ending.
    SessionEnded,
    /// A service that became ready woke the edge.
    ServiceReady,
}

/// Forward one MCP session between `input`, `output` and the service until the input closes, the
/// parent goes away or the session ends.
///
/// `events` and `inbox` are the two ends of the loop's channel; the caller keeps a sender of its
/// own for [`Event::ParentGone`] and [`Event::SessionEnded`]. Only a failure to write to `output` is an error.
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
    /// Stop: the input closed, the parent went away, the session is ending or the service said
    /// goodbye for good.
    Exit,
}

/// What a dropped connection does with its outstanding handshakes and listings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Again {
    /// Hold them once for the next service: the service was lost or is being replaced.
    Listings,
    /// Answer them as cut off like every other request: the service said it is not coming back
    /// on its own (`quit`, `uninstall`, `session_end`).
    Nothing,
}

/// Whether `held` is a lone request the edge may send again after a lost service: a handshake
/// or a catalog listing, which changes nothing. A batch is never sent again, because its other
/// elements may.
fn sent_again(held: &HeldLine) -> bool {
    let [request] = held.requests.as_slice() else {
        return false;
    };
    held.line.trim_start().starts_with('{') && rpc::SENT_AGAIN.contains(&request.method.as_str())
}

/// The state of one forwarded session.
struct Forwarder<W: Write> {
    environment: Environment,
    output: W,
    events: Sender<Event>,
    /// The live service connection, for writing.
    link: Option<Connection>,
    /// Counts connections, so events from an ended one are ignored.
    generation: u64,
    /// Whether any service has accepted this session yet.
    ever_connected: bool,
    /// Whether a connection attempt is running.
    attempting: bool,
    /// Whether the edge has asked to be woken, which it does once.
    watching: bool,
    /// While a service that said `replace` is expected back, until when. Attempts made before
    /// then do not start the service.
    replacing_until: Option<Instant>,
    /// When the next connection attempt may run. Read only while there is no link and no
    /// attempt, and set whenever a link ends, so a held request always has one coming.
    next_attempt: Instant,
    backoff: Backoff,
    /// The resume token the service last handed this session.
    resume: Option<String>,
    /// The failure templates of the last accepted reply. None until a service has accepted.
    templates: Option<Templates>,
    hold: Hold,
    /// Client requests delivered to the service and not yet answered, by id key.
    client_requests: BTreeMap<String, Request>,
    /// The delivered lines among those that are a lone request the edge may send again
    /// ([`rpc::SENT_AGAIN`]), oldest first.
    again: Vec<HeldLine>,
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
            watching: false,
            replacing_until: None,
            next_attempt: now,
            backoff: Backoff::new(),
            resume: None,
            templates: None,
            hold: Hold::default(),
            client_requests: BTreeMap::new(),
            again: Vec::new(),
            service_requests: BTreeMap::new(),
        }
    }

    /// Whether the session needs a service now: before its first connection, while a request is
    /// held and while a replacement is expected. Never while idle.
    fn wants_service(&self) -> bool {
        !self.ever_connected || self.hold.has_request() || self.replacement_expected()
    }

    /// Whether a service that said `replace` is still expected back.
    fn replacement_expected(&self) -> bool {
        self.replacing_until
            .is_some_and(|until| self.environment.clock.now() < until)
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
            self.attempt(true);
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
            Event::LineTooLong(requests) => self.answer(&requests, Failure::TooLarge)?,
            Event::InputClosed => {
                for held in self.hold.drain() {
                    self.answer(&held.requests, Failure::Expired)?;
                }
                return Ok(Flow::Exit);
            }
            Event::Connected(socket, templates) => self.connected(socket, templates)?,
            Event::AttemptFailed => {
                self.attempting = false;
                self.next_attempt = self.environment.clock.now() + self.backoff.next_wait();
                self.watch();
            }
            Event::ServiceReady => {
                (self.environment.log)(LogEvent::ServiceReady, "");
                if self.link.is_none() && !self.attempting {
                    self.attempt(false);
                }
            }
            Event::Frame(generation, frame) => {
                if self.is_current(generation) {
                    return self.service_frame(frame);
                }
            }
            Event::Gone(generation) => {
                if self.is_current(generation) {
                    (self.environment.log)(LogEvent::ServiceDisconnected, "");
                    self.disconnect(Again::Listings, None)?;
                    self.watch();
                }
            }
            Event::ParentGone => {
                (self.environment.log)(LogEvent::ParentGone, "");
                return Ok(Flow::Exit);
            }
            Event::SessionEnded => {
                (self.environment.log)(LogEvent::SessionEnded, "");
                return Ok(Flow::Exit);
            }
        }
        Ok(Flow::Continue)
    }

    /// Whether `generation` names the live connection.
    fn is_current(&self, generation: u64) -> bool {
        self.link.is_some() && generation == self.generation
    }

    /// Ask to be woken by the next service that becomes ready, once for the life of the edge.
    fn watch(&mut self) {
        if std::mem::replace(&mut self.watching, true) {
            return;
        }
        let events = self.events.clone();
        let ready = Box::new(move || {
            let _ = events.send(Event::ServiceReady);
        });
        if let Err(error) = self.environment.service.watch(ready) {
            (self.environment.log)(LogEvent::WakeFailed, &error.to_string());
        }
    }

    /// Start one connection attempt on its own thread. A failed attempt starts the service when
    /// `may_start`, except while a replacement is expected: the installer starts that one.
    fn attempt(&mut self, may_start: bool) {
        self.attempting = true;
        let starts = may_start && !self.replacement_expected();
        let service = Arc::clone(&self.environment.service);
        let events = self.events.clone();
        let log = self.environment.log;
        let opening = opening(self.resume.clone());
        thread::spawn(move || {
            let event = match service.connect(&opening) {
                Ok((socket, reply)) if reply.accepted => Event::Connected(socket, reply.templates),
                Ok((_, reply)) => {
                    let detail = reply
                        .code
                        .map_or_else(String::new, |code| format!("{code:?}"));
                    log(LogEvent::OpeningRefused, &detail);
                    if starts {
                        start(service.as_ref(), log);
                    }
                    Event::AttemptFailed
                }
                Err(error) => {
                    log(LogEvent::ConnectFailed, &error.to_string());
                    if starts {
                        start(service.as_ref(), log);
                    }
                    Event::AttemptFailed
                }
            };
            let _ = events.send(event);
        });
    }

    /// Take a newly accepted connection and deliver everything held, in order.
    fn connected(&mut self, socket: Connection, templates: Option<Templates>) -> io::Result<()> {
        self.attempting = false;
        if templates.is_some() {
            self.templates = templates;
        }
        self.backoff.reset();
        self.ever_connected = true;
        self.replacing_until = None;
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
            self.deliver(held, &[])?;
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
        let held = HeldLine {
            line,
            requests: inspection.requests,
            held_at: self.environment.clock.now(),
            sent_again: false,
        };
        if self.link.is_some() {
            return self.deliver(held, &answered);
        }
        match self.hold.push(held) {
            Ok(()) => Ok(()),
            Err(refused) => self.answer(&refused.requests, Failure::Expired),
        }
    }

    /// Write one client line to the live connection, once. `responses` are the keys of the
    /// line's answers to requests this connection made.
    ///
    /// Its requests become outstanding before the write. A write that fails delivered nothing,
    /// because the service acts only on a whole frame, so the line goes back to the hold without
    /// its answers to this connection, and the connection is dropped. A line too large to frame
    /// never reaches the service and its requests are answered as too large.
    fn deliver(&mut self, held: HeldLine, responses: &[String]) -> io::Result<()> {
        let payload = serde_json::to_vec(&McpFrame::Message {
            line: held.line.clone(),
        })
        .map_err(io::Error::other)?;
        if payload.len() > MAX_FRAME_BYTES {
            return self.answer(&held.requests, Failure::TooLarge);
        }
        for request in &held.requests {
            self.client_requests
                .insert(request.key.clone(), request.clone());
        }
        for key in responses {
            self.service_requests.remove(key);
        }
        let Some(link) = self.link.as_mut() else {
            return Ok(());
        };
        if let Err(error) = frame::write_frame_raw(link, &payload) {
            (self.environment.log)(LogEvent::ServiceDisconnected, &error.to_string());
            for request in &held.requests {
                self.client_requests.remove(&request.key);
            }
            let undelivered = if responses.is_empty() {
                Some(held)
            } else {
                without_responses(&held.line, responses).map(|line| HeldLine { line, ..held })
            };
            self.disconnect(Again::Listings, undelivered)?;
            self.watch();
            return Ok(());
        }
        if !held.sent_again && sent_again(&held) {
            self.again.push(held);
        }
        Ok(())
    }

    /// One frame from the service: pass a line to the client, keep a resume token, or act on a
    /// goodbye.
    fn service_frame(&mut self, frame: McpFrame) -> io::Result<Flow> {
        match frame {
            McpFrame::Message { line } => {
                let inspection = Inspection::of(&line);
                for request in inspection.requests {
                    self.service_requests.insert(request.key.clone(), request);
                }
                for key in &inspection.responses {
                    self.client_requests.remove(key);
                }
                self.again.retain(|held| {
                    held.requests
                        .iter()
                        .all(|request| !inspection.responses.contains(&request.key))
                });
                self.write_line(line.as_bytes())?;
            }
            McpFrame::Resume { token } => self.resume = Some(token),
            McpFrame::Goodbye { reason } => return self.goodbye(reason),
            McpFrame::Unknown => {}
        }
        Ok(Flow::Continue)
    }

    /// The service is closing this stream on purpose.
    ///
    /// Every outstanding request is answered as cut off in each case. `quit` and an unknown
    /// reason then leave the edge idle until a request needs the service. `replace` expects
    /// the next service for [`HOLD_LIMIT`]. `uninstall` and `session_end` end the edge.
    fn goodbye(&mut self, reason: GoodbyeReason) -> io::Result<Flow> {
        (self.environment.log)(LogEvent::ServiceDisconnected, reason.as_str());
        let again = match reason {
            GoodbyeReason::Replace => Again::Listings,
            _ => Again::Nothing,
        };
        self.disconnect(again, None)?;
        match reason {
            GoodbyeReason::Replace => {
                self.replacing_until = Some(self.environment.clock.now() + HOLD_LIMIT);
                self.watch();
                Ok(Flow::Continue)
            }
            GoodbyeReason::Uninstall | GoodbyeReason::SessionEnd => Ok(Flow::Exit),
            GoodbyeReason::Quit | GoodbyeReason::Unknown => {
                self.watch();
                Ok(Flow::Continue)
            }
        }
    }

    /// Drop the connection and settle what it left outstanding, then let the next attempt run as
    /// soon as a held request wants one.
    ///
    /// With [`Again::Listings`], each outstanding lone handshake or listing goes back to the hold
    /// once, ahead of `undelivered` (a line whose write failed), which goes ahead of every line
    /// already held. Every other outstanding client request is answered as cut off. A line the
    /// hold cannot take back is answered at once: as cut off when it was sent, as expired when
    /// it never was.
    ///
    /// Requests the service made of the client are forgotten, so the client's late answers to them
    /// are dropped rather than sent to whichever service comes next.
    fn disconnect(&mut self, again: Again, undelivered: Option<HeldLine>) -> io::Result<()> {
        if let Some(link) = self.link.take() {
            link.shutdown();
        }
        let now = self.environment.clock.now();
        self.next_attempt = now;
        self.service_requests.clear();
        let mut outstanding = std::mem::take(&mut self.client_requests);
        let mut resend = std::mem::take(&mut self.again);
        if again == Again::Nothing {
            resend.clear();
        }
        for held in &mut resend {
            held.held_at = now;
            held.sent_again = true;
            for request in &held.requests {
                outstanding.remove(&request.key);
            }
        }
        let cut_off: Vec<Request> = outstanding.into_values().collect();
        self.answer(&cut_off, Failure::CutOff)?;
        for refused in self.hold.put_back(undelivered.into_iter().collect()) {
            self.answer(&refused.requests, Failure::Expired)?;
        }
        for refused in self.hold.put_back(resend) {
            self.answer(&refused.requests, Failure::CutOff)?;
        }
        Ok(())
    }

    /// Answer each request with the service's template for `failure`, or the bare error.
    fn answer(&mut self, requests: &[Request], failure: Failure) -> io::Result<()> {
        if requests.is_empty() {
            return Ok(());
        }
        for request in requests {
            let response = failure_response(
                self.templates.as_ref(),
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

/// The opening this edge sends: an `mcp` stream on the MCP channel, with the session's last
/// resume token when it has one.
fn opening(resume: Option<String>) -> Opening {
    let mut opening = Opening::new(
        Role::Mcp,
        Edge::new(Program::Edge, env!("CARGO_PKG_VERSION").to_owned()),
    );
    opening.channel = Some(MCP_CHANNEL.to_owned());
    opening.resume = resume;
    opening
}

/// Start the service after a failed attempt, logging the outcome.
fn start(service: &dyn Service, log: Log) {
    log(LogEvent::ServiceStartAttempted, START_REASON.as_str());
    if let Err(error) = service.start(START_REASON) {
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
    /// The id of a lone `ping` request. The edge answers a client's ping itself; a service's
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
    /// A line over the bound: its bytes were skipped up to its newline, and these are the
    /// requests it held.
    TooLong(Vec<Request>),
}

/// Read client lines on their own thread and post them as events, then [`Event::InputClosed`].
fn read_input(input: impl Read + Send + 'static, events: Sender<Event>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(input);
        loop {
            let event = match read_line(&mut reader, MAX_FRAME_BYTES) {
                Ok(Some(InputLine::Line(bytes))) => Event::Line(bytes),
                Ok(Some(InputLine::TooLong(requests))) => Event::LineTooLong(requests),
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
/// boundary; a [`scan::Scanner`] reads its bytes on the way for the requests it holds.
fn read_line(reader: &mut impl BufRead, limit: usize) -> io::Result<Option<InputLine>> {
    let mut line = Vec::new();
    let mut started = false;
    let mut skipped: Option<scan::Scanner> = None;
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
        match skipped.as_mut() {
            Some(scanner) => scanner.feed(chunk),
            None if line.len() + chunk.len() > limit => {
                let mut scanner = scan::Scanner::default();
                scanner.feed(&std::mem::take(&mut line));
                scanner.feed(chunk);
                skipped = Some(scanner);
            }
            None => line.extend_from_slice(chunk),
        }
        let used = newline.map_or(buffer.len(), |position| position + 1);
        reader.consume(used);
        if newline.is_some() {
            break;
        }
    }
    Ok(Some(match skipped {
        Some(scanner) => InputLine::TooLong(scanner.finish()),
        None => InputLine::Line(line),
    }))
}

/// Read frames from one service connection on their own thread, then [`Event::Gone`].
///
/// A frame that is not an MCP frame is skipped; a stream that ends or cannot be framed ends the
/// connection.
fn read_service(generation: u64, mut socket: Connection, events: Sender<Event>) {
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
