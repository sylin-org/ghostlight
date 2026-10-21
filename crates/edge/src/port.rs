//! Carrying one Chrome native-messaging port to one service connection (ADR-0197 Decisions 3, 4
//! and 11, ADR-0199 Decision 5).
//!
//! The edge reads native frames from the browser and writes them to a relay `browser`
//! stream, and does the same in the other direction, without parsing a byte of either. The
//! relay frame header is a little-endian `u32` length; Chrome writes the same four bytes in
//! native order, which is the same thing on every supported target.
//!
//! The first frame the browser sends is the extension's hello. The edge holds it, drops
//! every later frame until a service has accepted the stream, and sends the held hello first.
//! Before that first connection it starts the service once, as a `browser` start, and keeps
//! connecting on the [`Backoff`] schedule without starting it again. When no service has accepted
//! within [`ABSENT_AFTER`] of the edge's own start, whatever the cause, it says so with
//! [`BACKEND_UNAVAILABLE`]; the extension reads that as "not connected" and stops waiting for a
//! hello answer, where silence would have read as an outdated service. After the first
//! connection, and after that frame, the edge never starts the service: when the stream
//! ends it tells the browser once with the same frame, drops what the browser sends, and probes
//! with a read-only control opening. When a service answers it returns
//! [`Ended::ServiceReturned`] and the process exits, so the extension reconnects with a fresh
//! hello on a new port.
//!
//! The first time the edge has to wait for a service, before the start or after the loss, it
//! registers its wake endpoint ([`Service::watch`], ADR-0199 amendment of 2026-10-20). A service
//! that becomes ready wakes it, and the edge looks at once instead of at its next scheduled look.
//! The schedule stays as it was, so an edge that cannot register, or a wake that never comes,
//! costs what it always cost.
//!
//! A service that ends the stream on purpose says goodbye first (ADR-0199 Decision 10). The
//! goodbye is not the browser's frame and is never passed on. After `quit` and `replace` the
//! edge behaves as it does for a lost service. After `uninstall` it returns
//! [`Ended::ServiceUninstalled`] at once and the process exits. After `session_end`, and when its
//! own session listener hears that the session is ending, it returns [`Ended::SessionOver`] at
//! once, and nothing is started (ADR-0199 Decision 13).
//!
//! Two threads carry the frames, one per direction, and the calling thread decides what happens
//! next from the events they send. Correlation ids, command names and payloads are never read
//! here, so nothing in a frame can reach the diagnostics.

use std::cell::Cell;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::endpoint::{ConnectError, Connection};
use ghostlight_relay::frame;
use ghostlight_relay::log::Event as LogEvent;
use ghostlight_relay::protocol::{
    goodbye_in, Control, Edge, GoodbyeReason, Opening, Program, Reply, Role, StartReason,
};
use ghostlight_relay::spawn::Backoff;
use ghostlight_relay::OPENING_DEADLINE;

#[cfg(test)]
mod tests;

// A relay header and a Chrome header are the same four bytes only on a little-endian target.
const _: () = assert!(
    cfg!(target_endian = "little"),
    "the browser edge relies on native byte order matching the relay frame header"
);

/// The one frame the edge writes to the browser itself, sent once when no service is there:
/// the frame kind `backend_unavailable` with the field `service` set to `absent`. A bare
/// `backend_unavailable` is what a 1.x connector writes and says nothing about absence.
pub const BACKEND_UNAVAILABLE: &[u8] = br#"{"kind":"backend_unavailable","service":"absent"}"#;

/// How long the edge looks for a service before it tells the browser none is there. It is
/// below the extension's hello timeout, which would otherwise read the silence as an outdated
/// service.
pub const ABSENT_AFTER: Duration = Duration::from_secs(5);

/// The diagnostics detail for an edge that found no service within [`ABSENT_AFTER`].
const ABSENT_DETAIL: &str = "absent";

/// Why this edge starts the service: a browser needs it.
const START_REASON: StartReason = StartReason::Browser;

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

/// Where diagnostics go: one closed event and a short, content-free detail.
pub type Log = fn(LogEvent, &str);

/// Everything the port needs from the outside world.
pub struct Environment {
    /// The service to connect to and start.
    pub service: Arc<dyn Service>,
    /// The origin of the extension that opened the port: the first argument Chrome passed.
    pub origin: String,
    /// How long to look for a first service before telling the browser none is there.
    pub absent_after: Duration,
    /// The diagnostics writer.
    pub log: Log,
}

/// Something the port reacts to.
pub enum Event {
    /// The first frame the browser sent: the extension's hello.
    Hello(Vec<u8>),
    /// The browser closed its end of the port.
    InputClosed,
    /// Reading from the browser failed.
    InputFailed(String),
    /// Writing to the browser failed, so it is gone.
    OutputClosed,
    /// The process that started the edge went away.
    ParentGone,
    /// The service connection ended and the browser has been told.
    ServiceLost,
    /// The service said goodbye because Ghostlight is being removed.
    ServiceUninstalled,
    /// The session is ending, whether the service said so or this process heard it.
    SessionEnded,
    /// A service that became ready woke the edge.
    ServiceReady,
}

/// Why [`run`] returned. Every one of them is a clean exit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ended {
    /// The browser closed the port, in either direction.
    BrowserGone,
    /// The process that started the edge went away.
    ParentGone,
    /// A service accepted an opening after the connection was lost.
    ServiceReturned,
    /// The service said goodbye because Ghostlight is being removed.
    ServiceUninstalled,
    /// The session is ending: the service said goodbye for it, or this process heard it.
    SessionOver,
}

impl Ended {
    /// The reason as a short word for the diagnostics line.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BrowserGone => "browser_gone",
            Self::ParentGone => "parent_gone",
            Self::ServiceReturned => "service_returned",
            Self::ServiceUninstalled => "service_uninstalled",
            Self::SessionOver => "session_ended",
        }
    }
}

/// Carry one browser port to the service until the browser or the parent goes away, or until a
/// service answers again after the connection was lost.
///
/// `events` and `inbox` are the two ends of the port's channel; the caller keeps a sender of its
/// own for [`Event::ParentGone`] and [`Event::SessionEnded`]. Only a failure to read from the
/// browser is an error.
pub fn run(
    environment: Environment,
    input: impl Read + Send + 'static,
    output: impl Write + Send + 'static,
    events: Sender<Event>,
    inbox: Receiver<Event>,
) -> io::Result<Ended> {
    let started = Instant::now();
    let shared = Arc::new(Shared::default());
    read_browser(input, Arc::clone(&shared), events.clone());
    let result = Port {
        environment,
        shared: Arc::clone(&shared),
        events,
        inbox,
        started,
        watching: Cell::new(false),
    }
    .carry(output);
    shared.end();
    result
}

/// The connection frames from the browser are written to, shared with the reading thread.
#[derive(Default)]
struct Shared {
    /// The write half of the live service connection. `None` before the first connection and
    /// after the service is lost; the browser's frames are dropped then.
    uplink: Mutex<Option<Connection>>,
    /// Set when the port is finished, so a connection that closes after that says nothing.
    ending: AtomicBool,
}

impl Shared {
    /// The uplink, whether or not a thread panicked while holding it.
    fn uplink(&self) -> MutexGuard<'_, Option<Connection>> {
        self.uplink.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Send `hello` on `socket` and make `socket` the uplink, in one step.
    ///
    /// The lock is held across the write, so the browser's next frame cannot overtake the hello.
    fn install(&self, socket: &Connection, hello: &[u8]) -> io::Result<()> {
        let mut uplink = self.uplink();
        let mut writer = socket.try_clone()?;
        frame::write_frame_raw(&mut writer, hello).map_err(io::Error::other)?;
        *uplink = Some(writer);
        Ok(())
    }

    /// Pass one browser frame to the service, or drop it when there is no connection.
    fn forward(&self, payload: &[u8]) {
        let mut uplink = self.uplink();
        let Some(writer) = uplink.as_mut() else {
            return;
        };
        if frame::write_frame_raw(writer, payload).is_err() {
            writer.shutdown();
            *uplink = None;
        }
    }

    /// Close the uplink, which ends the service reader too.
    fn lose(&self) {
        if let Some(writer) = self.uplink().take() {
            writer.shutdown();
        }
    }

    /// Mark the port finished and close the connection.
    fn end(&self) {
        self.ending.store(true, Ordering::SeqCst);
        self.lose();
    }

    /// Whether the port is finished.
    fn is_ending(&self) -> bool {
        self.ending.load(Ordering::SeqCst)
    }
}

/// Read browser frames on their own thread: hand over the first, forward the rest.
fn read_browser(mut input: impl Read + Send + 'static, shared: Arc<Shared>, events: Sender<Event>) {
    thread::spawn(move || {
        let mut first = true;
        loop {
            let event = match frame::read_frame_raw(&mut input) {
                Ok(Some(payload)) if first => {
                    first = false;
                    Event::Hello(payload)
                }
                Ok(Some(payload)) => {
                    shared.forward(&payload);
                    continue;
                }
                Ok(None) => Event::InputClosed,
                Err(error) => Event::InputFailed(error.to_string()),
            };
            let last = !matches!(event, Event::Hello(_));
            if events.send(event).is_err() || last {
                return;
            }
        }
    });
}

/// What the port was woken for.
enum Waited {
    /// The wait ran out.
    Elapsed,
    /// The browser's hello arrived.
    Hello(Vec<u8>),
    /// The service connection ended.
    Lost,
    /// A service that became ready woke the edge.
    Ready,
    /// The port is over.
    Ended(Ended),
}

/// The state of one carried port.
struct Port {
    environment: Environment,
    shared: Arc<Shared>,
    events: Sender<Event>,
    inbox: Receiver<Event>,
    /// When the edge started, which [`Environment::absent_after`] counts from.
    started: Instant,
    /// Whether the edge has asked to be woken, which it does once.
    watching: Cell<bool>,
}

impl Port {
    /// The whole life of the port: hold the hello, connect, carry, wait for a returning service.
    /// A first connection that does not come in time is announced to the browser instead.
    fn carry(&self, mut output: impl Write + Send + 'static) -> io::Result<Ended> {
        let hello = match self.await_hello()? {
            Ok(hello) => hello,
            Err(ended) => return Ok(ended),
        };
        let absent_at = self.started + self.environment.absent_after;
        let mut backoff = Backoff::new();
        let mut started = false;
        let socket = loop {
            if let Some(socket) = self.open_stream(&hello) {
                break Some(socket);
            }
            if !started {
                started = true;
                self.watch();
                self.start_service();
            }
            let wait = backoff
                .next_wait()
                .min(absent_at.saturating_duration_since(Instant::now()));
            if let Some(ended) = self.pause(wait)? {
                return Ok(ended);
            }
            if Instant::now() >= absent_at {
                break None;
            }
        };
        match socket {
            Some(socket) => {
                (self.environment.log)(LogEvent::ServiceConnected, "");
                self.carry_service(socket, output);
                if let Some(ended) = self.await_loss()? {
                    return Ok(ended);
                }
            }
            None => {
                (self.environment.log)(LogEvent::ServiceDisconnected, ABSENT_DETAIL);
                if frame::write_frame_raw(&mut output, BACKEND_UNAVAILABLE).is_err() {
                    return Ok(Ended::BrowserGone);
                }
            }
        }
        self.watch();
        let mut backoff = Backoff::new();
        loop {
            if let Some(ended) = self.pause(backoff.next_wait())? {
                return Ok(ended);
            }
            if self.service_has_returned() {
                return Ok(Ended::ServiceReturned);
            }
        }
    }

    /// Wait for the browser's first frame, or for the port to end first.
    fn await_hello(&self) -> io::Result<Result<Vec<u8>, Ended>> {
        loop {
            match self.wait(None)? {
                Waited::Hello(hello) => return Ok(Ok(hello)),
                Waited::Ended(ended) => return Ok(Err(ended)),
                Waited::Elapsed | Waited::Lost | Waited::Ready => {}
            }
        }
    }

    /// Wait out `wait`, returning early when a service wakes the edge, so it looks at once, and
    /// when the port ends.
    fn pause(&self, wait: Duration) -> io::Result<Option<Ended>> {
        let limit = Instant::now() + wait;
        loop {
            match self.wait(Some(limit))? {
                Waited::Elapsed => return Ok(None),
                Waited::Ready => {
                    (self.environment.log)(LogEvent::ServiceReady, "");
                    return Ok(None);
                }
                Waited::Ended(ended) => return Ok(Some(ended)),
                Waited::Hello(_) | Waited::Lost => {}
            }
        }
    }

    /// Wait for the service connection to end, or for the port to end first.
    fn await_loss(&self) -> io::Result<Option<Ended>> {
        loop {
            match self.wait(None)? {
                Waited::Lost => return Ok(None),
                Waited::Ended(ended) => return Ok(Some(ended)),
                Waited::Elapsed | Waited::Hello(_) | Waited::Ready => {}
            }
        }
    }

    /// The next thing that happens: an event, or `limit` passing when there is one.
    fn wait(&self, limit: Option<Instant>) -> io::Result<Waited> {
        let received = match limit {
            Some(limit) => {
                let left = limit.saturating_duration_since(Instant::now());
                match self.inbox.recv_timeout(left) {
                    Ok(event) => Ok(event),
                    Err(RecvTimeoutError::Timeout) => return Ok(Waited::Elapsed),
                    Err(RecvTimeoutError::Disconnected) => Err(()),
                }
            }
            None => self.inbox.recv().map_err(|_| ()),
        };
        let Ok(event) = received else {
            return Ok(Waited::Ended(Ended::BrowserGone));
        };
        match event {
            Event::Hello(hello) => Ok(Waited::Hello(hello)),
            Event::ServiceLost => Ok(Waited::Lost),
            Event::ServiceReady => Ok(Waited::Ready),
            Event::ServiceUninstalled => Ok(Waited::Ended(Ended::ServiceUninstalled)),
            Event::SessionEnded => {
                (self.environment.log)(LogEvent::SessionEnded, "");
                Ok(Waited::Ended(Ended::SessionOver))
            }
            Event::InputClosed | Event::OutputClosed => Ok(Waited::Ended(Ended::BrowserGone)),
            Event::ParentGone => {
                (self.environment.log)(LogEvent::ParentGone, "");
                Ok(Waited::Ended(Ended::ParentGone))
            }
            Event::InputFailed(detail) => Err(io::Error::other(detail)),
        }
    }

    /// Try once to open the `browser` stream and send the held hello on it.
    fn open_stream(&self, hello: &[u8]) -> Option<Connection> {
        let log = self.environment.log;
        let mut opening = Opening::new(Role::Browser, edge());
        opening.origin = Some(self.environment.origin.clone());
        match self.environment.service.connect(&opening) {
            Ok((socket, reply)) if reply.accepted => match self.shared.install(&socket, hello) {
                Ok(()) => Some(socket),
                Err(error) => {
                    log(LogEvent::ConnectFailed, &error.to_string());
                    None
                }
            },
            Ok((_, reply)) => {
                log(LogEvent::OpeningRefused, &describe(&reply));
                None
            }
            Err(error) => {
                log(LogEvent::ConnectFailed, &error.to_string());
                None
            }
        }
    }

    /// Ask to be woken by the next service that becomes ready, once for the life of the port.
    fn watch(&self) {
        if self.watching.replace(true) {
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

    /// Ask for the service to start after the first failed attempt, logging the outcome.
    fn start_service(&self) {
        let log = self.environment.log;
        log(LogEvent::ServiceStartAttempted, START_REASON.as_str());
        if let Err(error) = self.environment.service.start(START_REASON) {
            log(LogEvent::ServiceStartFailed, &error.to_string());
        }
    }

    /// Whether a service accepts a read-only control opening. Nothing is started or changed.
    fn service_has_returned(&self) -> bool {
        let log = self.environment.log;
        let mut opening = Opening::new(Role::Control, edge());
        opening.control = Some(Control::Readiness);
        match self.environment.service.connect(&opening) {
            Ok((mut socket, reply)) if reply.accepted => {
                // The service sends one result and closes; take it so it is not written to a
                // closed connection. Whether it arrives does not matter.
                let _ = socket.set_read_timeout(Some(OPENING_DEADLINE));
                let _ = frame::read_frame_raw(&mut socket);
                true
            }
            Ok((_, reply)) => {
                log(LogEvent::OpeningRefused, &describe(&reply));
                false
            }
            Err(error) => {
                log(LogEvent::ConnectFailed, &error.to_string());
                false
            }
        }
    }

    /// Carry service frames to the browser on their own thread. When the connection ends the
    /// thread tells the browser once and then tells the port. A goodbye is read, not carried: an
    /// `uninstall` ends the port at once, and any other reason is logged with the end of the
    /// stream.
    fn carry_service(&self, mut socket: Connection, mut output: impl Write + Send + 'static) {
        let shared = Arc::clone(&self.shared);
        let events = self.events.clone();
        let log = self.environment.log;
        thread::spawn(move || {
            let mut goodbye = None;
            let cause = loop {
                match frame::read_frame_raw(&mut socket) {
                    Ok(Some(payload)) => {
                        if let Some(reason) = goodbye_in(&payload) {
                            goodbye = Some(reason);
                            if let Some(ending) = ending_event(reason) {
                                socket.shutdown();
                                shared.lose();
                                let _ = events.send(ending);
                                return;
                            }
                        } else if frame::write_frame_raw(&mut output, &payload).is_err() {
                            let _ = events.send(Event::OutputClosed);
                            return;
                        }
                    }
                    Ok(None) => break String::new(),
                    Err(error) => break error.to_string(),
                }
            };
            // Unblock a browser frame that is mid-write, then close the uplink.
            socket.shutdown();
            shared.lose();
            if shared.is_ending() {
                return;
            }
            let cause = goodbye.map_or(cause, |reason| reason.as_str().to_owned());
            log(LogEvent::ServiceDisconnected, &cause);
            let event = match frame::write_frame_raw(&mut output, BACKEND_UNAVAILABLE) {
                Ok(()) => Event::ServiceLost,
                Err(_) => Event::OutputClosed,
            };
            let _ = events.send(event);
        });
    }
}

/// The event that ends the port at once for a goodbye with `reason`, or `None` when the service
/// is expected back and the port waits for it.
fn ending_event(reason: GoodbyeReason) -> Option<Event> {
    match reason {
        GoodbyeReason::Uninstall => Some(Event::ServiceUninstalled),
        GoodbyeReason::SessionEnd => Some(Event::SessionEnded),
        GoodbyeReason::Quit | GoodbyeReason::Replace | GoodbyeReason::Unknown => None,
    }
}

/// The program this edge presents itself as, at this build.
fn edge() -> Edge {
    Edge::new(Program::Edge, env!("CARGO_PKG_VERSION").to_owned())
}

/// A refusal as a short, content-free detail.
fn describe(reply: &Reply) -> String {
    reply
        .code
        .map_or_else(String::new, |code| format!("{code:?}"))
}
