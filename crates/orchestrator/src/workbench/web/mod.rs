//! The loopback server that serves the workbench to the Ghostlight window and to browser tabs.
//!
//! It is the inbound adapter over [`WorkbenchFacade`]. It binds 127.0.0.1 only, answers only when
//! the `Host` header is its own address, and decides access per route in three contexts: Control (the request carries this start's bearer), Watch (no bearer
//! while "Others can watch" is on) and Closed (no bearer while it is off). Each part of the state
//! is projected for its audience when the response is written (ADR-0199, amendment of 2026-10-15).
//!
//! The bearer is 256 random bits made at each start. It is held here, handed to the desktop role
//! and to nobody else, and never logged, put in a URL or sent in a response.

mod assets;
mod events;
mod http;
mod limits;
mod routes;
#[cfg(test)]
mod tests;

use std::fmt;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use base64::Engine as _;
use ghostlight_relay::endpoint::fnv1a32;
use zeroize::Zeroizing;

use super::{Watching, WebFacts, WorkbenchEventSink, WorkbenchFacade};

/// The lowest port a home's default can be.
const PORT_BASE: u16 = 41_000;

/// How many ports above [`PORT_BASE`] a default can fall on.
const PORT_SPAN: u32 = 1_000;

/// The port the workbench preview server uses, which a default never takes.
const PREVIEW_PORT: u16 = 41_737;

/// How many random bytes make a bearer.
const BEARER_BYTES: usize = 32;

/// The most connections served at once. A connection beyond that is closed at once.
const MAX_CONNECTIONS: usize = 32;

/// How many short requests without the bearer may be served beside a full set of watchers' streams,
/// so the next watcher is told why it cannot watch rather than dropped.
const SHORT_REQUESTS_WITHOUT_BEARER: usize = 4;

/// How many connections may be without this start's bearer: a watcher, a stranger, or a peer that
/// has not sent its request yet. The other 20 stay for the window, so peers that only hold
/// connections open never lock it out (U-D5).
const MAX_WITHOUT_BEARER: usize = events::MAX_WATCH_STREAMS + SHORT_REQUESTS_WITHOUT_BEARER;

/// How long a peer let in past [`MAX_WITHOUT_BEARER`] has to send a request that carries the
/// bearer. One that does not is closed unanswered.
const BEARER_DEADLINE: Duration = Duration::from_secs(1);

/// How long the accept loop rests after a failed accept.
const ACCEPT_RETRY: Duration = Duration::from_millis(50);

/// The port a home's workbench tries first: 41000 to 41999 from the home path, never the
/// preview server's.
#[must_use]
pub fn default_port(home: &Path) -> u16 {
    let lowered = home.to_string_lossy().to_ascii_lowercase();
    let offset = u16::try_from(fnv1a32(lowered.as_bytes()) % PORT_SPAN)
        .expect("the span is below the width of a port");
    let port = PORT_BASE + offset;
    if port == PREVIEW_PORT {
        PREVIEW_PORT + 1
    } else {
        port
    }
}

/// Whether two secrets are equal, in time that depends only on their length.
#[must_use]
pub fn same_secret(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

/// Where the workbench is served.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchAddress {
    url: String,
    port: u16,
    usual: bool,
}

impl WorkbenchAddress {
    /// The workbench address at `port`, `usual` when it is the port the home's default names.
    pub(crate) fn new(port: u16, usual: bool) -> Self {
        Self {
            url: format!("http://{}:{port}/", Ipv4Addr::LOCALHOST),
            port,
            usual,
        }
    }

    /// The address a person opens, ending in a slash.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Whether this is the port the home's default names. It is not when the default was taken.
    #[must_use]
    pub const fn usual(&self) -> bool {
        self.usual
    }
}

/// A bound listener that is not answering yet.
///
/// The service protects the address from agents between binding and serving, so no agent request
/// can reach the page before the rule holds.
pub struct PendingWorkbench {
    listener: TcpListener,
    address: WorkbenchAddress,
}

impl PendingWorkbench {
    /// Bind 127.0.0.1 on the home's default port, or on a port the system picks when that is taken.
    pub fn bind(home: &Path) -> io::Result<Self> {
        Self::bind_preferring(default_port(home))
    }

    /// Bind 127.0.0.1 on `preferred`, or on a port the system picks when that is taken.
    pub fn bind_preferring(preferred: u16) -> io::Result<Self> {
        let (listener, usual) = match TcpListener::bind((Ipv4Addr::LOCALHOST, preferred)) {
            Ok(listener) => (listener, true),
            Err(_) => (TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?, false),
        };
        let port = listener.local_addr()?.port();
        Ok(Self {
            listener,
            address: WorkbenchAddress::new(port, usual),
        })
    }

    /// Where the workbench will be served.
    #[must_use]
    pub const fn address(&self) -> &WorkbenchAddress {
        &self.address
    }

    /// Start answering, and tell the facade what it needs to know of this server.
    ///
    /// # Panics
    ///
    /// Panics when the operating system has no random bytes to make a bearer from.
    pub fn serve(
        self,
        facade: &WorkbenchFacade,
        instance: String,
        watching: Arc<Watching>,
    ) -> WorkbenchServer {
        facade.attach_web(WebFacts {
            address: self.address.clone(),
            instance: instance.clone(),
            watching: Arc::clone(&watching),
        });
        let events = Arc::new(events::EventHub::new(
            instance.clone(),
            Arc::clone(&watching),
        ));
        facade.attach_events(Arc::clone(&events) as Arc<dyn WorkbenchEventSink>);
        let mut random = Zeroizing::new([0_u8; BEARER_BYTES]);
        getrandom::getrandom(&mut *random).expect("the operating system supplies random bytes");
        let bearer =
            Zeroizing::new(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&random[..]));
        let shared = Arc::new(Shared {
            facade: facade.clone(),
            events,
            instance,
            bearer,
            watching,
            port: self.address.port,
            limits: limits::Limits::default(),
            places: Arc::default(),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let thread = spawn_accept(self.listener, Arc::clone(&shared), Arc::clone(&stop));
        WorkbenchServer {
            shared,
            address: self.address,
            stop,
            thread: Mutex::new(thread),
        }
    }
}

/// What every connection of one server shares.
struct Shared {
    facade: WorkbenchFacade,
    events: Arc<events::EventHub>,
    instance: String,
    bearer: Zeroizing<String>,
    watching: Arc<Watching>,
    port: u16,
    limits: limits::Limits,
    places: Arc<Places>,
}

impl fmt::Debug for Shared {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Shared")
            .field("port", &self.port)
            .field("bearer", &"[redacted]")
            .finish_non_exhaustive()
    }
}

/// A running workbench server. Dropping it stops it.
pub struct WorkbenchServer {
    shared: Arc<Shared>,
    address: WorkbenchAddress,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl fmt::Debug for WorkbenchServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkbenchServer")
            .field("address", &self.address)
            .field("bearer", &"[redacted]")
            .finish_non_exhaustive()
    }
}

impl WorkbenchServer {
    /// Bind and serve in one step, for a caller that has no rule to put between the two.
    pub fn start(
        home: &Path,
        facade: &WorkbenchFacade,
        instance: String,
        watching: Arc<Watching>,
    ) -> io::Result<Self> {
        Ok(PendingWorkbench::bind(home)?.serve(facade, instance, watching))
    }

    /// Where the workbench is served.
    #[must_use]
    pub const fn address(&self) -> &WorkbenchAddress {
        &self.address
    }

    /// The bearer of this start, for the desktop role. It goes nowhere else.
    #[must_use]
    pub fn bearer(&self) -> &str {
        &self.shared.bearer
    }

    /// Stop answering and free the port. A second call does nothing.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.shared.events.close();
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            // One connection to ourselves wakes the accept that waits.
            let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.address.port));
            let _ = thread.join();
        }
    }
}

impl Drop for WorkbenchServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The connections being served, and how many of them are without the bearer.
#[derive(Debug, Default)]
struct Places {
    served: AtomicUsize,
    without_bearer: AtomicUsize,
}

/// One connection's place, freed when the connection ends. A peer starts without the bearer, in
/// the share [`MAX_WITHOUT_BEARER`] bounds while it has room; past it, the peer must show the
/// bearer within [`BEARER_DEADLINE`].
struct Place {
    places: Arc<Places>,
    without_bearer: bool,
}

impl Place {
    fn take(places: &Arc<Places>) -> Option<Self> {
        if places.served.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            places.served.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        let without_bearer =
            places.without_bearer.fetch_add(1, Ordering::AcqRel) < MAX_WITHOUT_BEARER;
        if !without_bearer {
            places.without_bearer.fetch_sub(1, Ordering::AcqRel);
        }
        Some(Self {
            places: Arc::clone(places),
            without_bearer,
        })
    }

    /// How long the peer has to send its request.
    const fn deadline(&self) -> Duration {
        if self.without_bearer {
            http::READ_DEADLINE
        } else {
            BEARER_DEADLINE
        }
    }

    /// The peer showed the bearer: its place leaves the share without one.
    fn bearer_shown(&mut self) {
        if std::mem::take(&mut self.without_bearer) {
            self.places.without_bearer.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        if self.without_bearer {
            self.places.without_bearer.fetch_sub(1, Ordering::AcqRel);
        }
        self.places.served.fetch_sub(1, Ordering::AcqRel);
    }
}

fn spawn_accept(
    listener: TcpListener,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
) -> Option<JoinHandle<()>> {
    thread::Builder::new()
        .name("ghostlight-workbench-listener".into())
        .spawn(move || {
            for stream in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else {
                    thread::sleep(ACCEPT_RETRY);
                    continue;
                };
                if !from_loopback(&stream) {
                    continue;
                }
                let Some(mut place) = Place::take(&shared.places) else {
                    continue;
                };
                let shared = Arc::clone(&shared);
                let _ = thread::Builder::new()
                    .name("ghostlight-workbench-connection".into())
                    .spawn(move || serve_connection(&shared, stream, &mut place));
            }
        })
        .ok()
}

/// A peer that is not on this computer's loopback is closed unanswered (ADR-0077 Decision 3).
fn from_loopback(stream: &TcpStream) -> bool {
    stream
        .peer_addr()
        .is_ok_and(|peer: SocketAddr| peer.ip().is_loopback())
}

fn serve_connection(shared: &Shared, stream: TcpStream, place: &mut Place) {
    let _ = stream.set_nodelay(true);
    let (mut incoming, head) = http::Incoming::read_head(stream, place.deadline());
    if matches!(&head, Ok(head) if routes::carries_bearer(shared, head)) {
        place.bearer_shown();
    } else if !place.without_bearer {
        // Let in past the share without a bearer, it did not show one in time (U-D5).
        return;
    }
    let mut response = match head {
        // A HEAD answer carries no body, a refusal's included (RFC 9110 9.3.2; U-D6): every
        // route answers it as it answers a read, and only the headers go out.
        Ok(head) if head.method == http::Method::Head => {
            let mut response = routes::answer(shared, &head, &mut incoming);
            drop(response.take_events());
            response.head_only()
        }
        Ok(head) => routes::answer(shared, &head, &mut incoming),
        Err(http::ReadError::Closed) => return,
        Err(http::ReadError::Malformed) => routes::unreadable(),
    };
    match response.take_events() {
        Some(subscription) => events::serve(incoming, &response, &subscription),
        None => incoming.finish(&response),
    }
}
