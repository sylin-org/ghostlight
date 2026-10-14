//! The physical browser port and authenticated relay-backed adapter implementation.

pub mod adapter;
pub mod recovery;
pub mod session;
pub mod transport;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::browser::adapter::{
    adapter_capability, Adapter, AdapterCapability, BrowserEvent, BrowserFrame, BrowserOutcome,
    BrowserPlatform, BrowserRequest, DiagnosticsState, RuntimeControlState, UnsupportedHello, Work,
    COMMAND_CHUNK_PAYLOAD_BYTES, COMMAND_TRANSFER_MAX_BYTES, COMMAND_TRANSFER_MAX_CHUNKS,
};
use crate::browser::transport::{SocketReader, SocketWriter, DELIVERY_TIMEOUT, EXCHANGE_TIMEOUT};
use base64::Engine as _;
use ghostlight_bridge::framing::{write_length_frame, write_native, FrameError};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

// Chromium accepts at most 1 MiB from a native host. Keep both direct frames
// and base64-wrapped chunks comfortably below that physical boundary.
const DIRECT_NATIVE_MESSAGE_BYTES: usize = 768 * 1024;
// A native message inside Chromium's ordinary worker-idle window keeps the browser shore
// observable without conflating a silent browser operation with a dead adapter.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(45);
// A dispatch probe's acknowledgement travels behind every earlier frame on the same ordered
// stream, including a large chunked screenshot or recording receipt. A probe unanswered for less
// than this proves nothing about the adapter, so a short deadline never quarantines it.
const PROBE_VERDICT_MIN: Duration = Duration::from_secs(5);
// How often a call waiting for its browser looks again.
const AVAILABILITY_POLL: Duration = Duration::from_millis(40);
// A product name is a label a person reads next to a browser they already recognize, not a
// place for an adapter to write prose.
const BROWSER_NAME_MAX_CHARS: usize = 40;
const MAX_BROWSER_CONNECTIONS: usize = 16;
const CONTROL_DELIVERY_TIMEOUT: Duration = Duration::from_millis(500);

/// How long a call waits for a browser that is on its way back before it says what is true.
///
/// It covers one relay reconnect attempt (500 ms), the page-runtime installation that keeps a new
/// connection private (the 5 s exchange budget), and a margin. Callers cap it by their own
/// invocation deadline.
pub const RETURN_GRACE: Duration = Duration::from_secs(7);

/// How long after the service publishes its endpoint a call with no browser still waits for the
/// relays that served an earlier service.
///
/// A waiting relay retries every 500 ms. This covers two retries and the hello. A relay that does
/// reconnect registers inside it, and its registered connection then carries the wait through the
/// page-runtime installation. A cold start with no browser loses only this much of its browser
/// startup budget.
pub(crate) const RELAY_RETURN_WINDOW: Duration = Duration::from_millis(1_500);

/// Liveness timing for one relay port. Production uses the defaults; tests shorten them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HeartbeatSettings {
    /// Time between timer heartbeats on an otherwise quiet connection.
    pub(crate) interval: Duration,
    /// Time without any acknowledgement after which the heartbeat thread ends the connection's
    /// availability and fails its pending calls.
    pub(crate) timeout: Duration,
    /// Minimum time a dispatch probe must go unanswered before a call's deadline may mark the
    /// adapter as not answering.
    pub(crate) verdict: Duration,
}

impl Default for HeartbeatSettings {
    fn default() -> Self {
        Self {
            interval: HEARTBEAT_INTERVAL,
            timeout: HEARTBEAT_TIMEOUT,
            verdict: PROBE_VERDICT_MIN,
        }
    }
}

/// Whether one browser can take work, as learned by waiting for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    /// The browser is connected and answering.
    Available,
    /// The browser holds a registered connection that is still initializing or stopped
    /// answering liveness probes.
    NotAnswering,
    /// No connection is registered for the browser.
    Absent,
}

/// A live application-owned admission check used at the actual transmission boundary.
pub struct BrowserDispatch<'a> {
    /// Invocation deadline, checked again after waiting for the writer.
    pub deadline: Instant,
    /// Live cancellation state for this invocation.
    pub cancelled: &'a AtomicBool,
    /// Product-owned runtime control decision; never held across a receipt wait.
    pub admit: &'a dyn Fn() -> Result<(), BrowserError>,
}

/// A synchronous physical primitive port used only by the orchestrator executor.
pub trait BrowserPort: Send + Sync {
    /// Dispatch one primitive to one exact browser and await a decisive receipt, cancellation,
    /// deadline, or disconnect.
    ///
    /// The browser is chosen before dispatch and never re-chosen here. A port that could pick a
    /// different browser after a failure would silently move a person's work between two
    /// different authenticated contexts.
    fn call(
        &self,
        browser: &str,
        workspace: &str,
        command: Work,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError>;

    /// Recheck live controls immediately before dispatch; real relays do this after writer wait.
    fn call_guarded(
        &self,
        browser: &str,
        workspace: &str,
        command: Work,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        (dispatch.admit)()?;
        self.call(
            browser,
            workspace,
            command,
            dispatch.deadline,
            dispatch.cancelled,
        )
    }

    /// Every connected browser, most recently attended first.
    fn browsers(&self) -> Vec<BrowserSummary>;

    /// Wait until one exact browser can take work, `until` passes, or the call is cancelled.
    ///
    /// The default reads `browsers()`, so a port that cannot tell a silent connection from an
    /// absent one reports every miss as absent.
    fn await_available(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &AtomicBool,
    ) -> Availability {
        loop {
            if self
                .browsers()
                .iter()
                .any(|connected| connected.id == browser)
            {
                return Availability::Available;
            }
            if cancelled.load(Ordering::SeqCst) || Instant::now() >= until {
                return Availability::Absent;
            }
            thread::sleep(
                until
                    .saturating_duration_since(Instant::now())
                    .min(AVAILABILITY_POLL),
            );
        }
    }

    /// Wait while a browser is evidently returning: until any browser can take work, the
    /// evidence lapses, `until` passes, or the call is cancelled. Return the connected browsers,
    /// none when the wait ended without one.
    ///
    /// The evidence is read again on every poll. Without it the wait ends at once, so a call
    /// that goes on to browser startup keeps that startup's budget.
    fn await_any(&self, until: Instant, cancelled: &AtomicBool) -> Vec<BrowserSummary> {
        loop {
            let connected = self.browsers();
            if !connected.is_empty()
                || cancelled.load(Ordering::SeqCst)
                || Instant::now() >= until
                || !self.browser_returning()
            {
                return connected;
            }
            thread::sleep(
                until
                    .saturating_duration_since(Instant::now())
                    .min(AVAILABILITY_POLL),
            );
        }
    }

    /// Whether there is evidence, right now, that a browser is about to connect. That evidence
    /// is what earns a call with no browser a bounded wait instead of an immediate recovery,
    /// and it must lapse with time: a browser that was merely seen at some point is not
    /// returning.
    fn browser_returning(&self) -> bool {
        false
    }

    /// Publish authoritative content-free runtime state to every browser without awaiting a
    /// receipt.
    fn publish_control_state(&self, _state: RuntimeControlState) -> Result<(), BrowserError> {
        Ok(())
    }
}

/// Choose the browser one invocation must use, or explain why no single browser is implied.
///
/// The order is the whole routing contract, and every step above the last is evidence rather
/// than a guess:
///
/// 1. an explicit selection the caller named;
/// 2. the browser this workspace is already pinned to;
/// 3. the most recently attended connected browser;
/// 4. the only connected browser;
/// 5. otherwise nothing, because two equally plausible browsers are two different user contexts
///    and picking one would be a coin flip with the person's session.
///
/// A pinned browser that is not connected never falls back to another one (ADR-0084 D4): the
/// work waits for the browser it belongs to, and says so.
///
/// An explicit selection outranks the automatic default but never an established binding: a
/// workspace with tabs open in one browser cannot be told to continue in another, because the
/// tabs it already owns would stay where they are.
///
/// # Errors
///
/// Returns the refusal that explains an unknown selection, a workspace that already works
/// elsewhere, a stopped browser, or an ambiguous bootstrap.
pub fn choose_browser(
    requested: Option<&str>,
    pinned: Option<&str>,
    connected: &[BrowserSummary],
) -> Result<String, BrowserError> {
    if let Some(requested) = requested {
        if pinned.is_some_and(|pinned| pinned != requested) {
            return Err(BrowserError::BrowserPinned);
        }
        if !connected.iter().any(|browser| browser.id == requested) {
            return Err(BrowserError::UnknownBrowser(requested.into()));
        }
        return Ok(requested.into());
    }
    if let Some(pinned) = pinned {
        if connected.iter().any(|browser| browser.id == pinned) {
            return Ok(pinned.into());
        }
        return Err(BrowserError::DisconnectedBeforeDispatch);
    }
    if let Some(attended) = connected.iter().find(|browser| browser.attended) {
        return Ok(attended.id.clone());
    }
    match connected {
        [] => Err(BrowserError::DisconnectedBeforeDispatch),
        [only] => Ok(only.id.clone()),
        several => Err(BrowserError::AmbiguousBrowser(
            several.iter().map(|browser| browser.id.clone()).collect(),
        )),
    }
}

/// Sink for asynchronous physical browser facts.
pub trait BrowserEventSink: Send + Sync {
    /// React to one adapter event without granting authority or fabricating completion.
    ///
    /// The browser that produced the event is always named. Physical tab ids are unique only
    /// inside one browser, so an unattributed event could be applied to a different browser's
    /// tab that happens to carry the same number.
    fn on_event(&self, browser: &str, event: BrowserEvent);
}

type PendingResult = Result<BrowserOutcome, BrowserError>;

#[derive(Debug)]
struct Connection {
    id: String,
    writer: Arc<SocketWriter>,
    pending: Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    adapter_version: String,
    adapter: Adapter,
    browser_id: String,
    browser_name: Option<String>,
    capabilities: HashMap<String, u16>,
    liveness: Option<Arc<Liveness>>,
    ready: bool,
}

impl Connection {
    /// Whether this connection can take work now: initialized, and answering when it
    /// negotiated liveness.
    fn available(&self, now: Instant, timeout: Duration) -> bool {
        self.ready
            && self
                .liveness
                .as_ref()
                .is_none_or(|liveness| liveness.state().is_available(now, timeout))
    }
}

/// What one browser's registered connection can do right now.
enum Reachability {
    /// No connection is registered for the browser.
    Absent,
    /// A connection is registered but has not finished initializing.
    Initializing,
    /// The connection is initialized but its liveness has lapsed.
    Silent {
        writer: Arc<SocketWriter>,
        liveness: Arc<Liveness>,
    },
    /// The connection can take work.
    Available,
}

/// One connected browser as the orchestrator, the workbench, and the model see it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserSummary {
    /// Persistent opaque browser identity, minted by the adapter and stable across reconnects.
    ///
    /// This doubles as the model-facing handle. It is already opaque and content-free, so a
    /// second mapping table would add a lookup without adding a guarantee.
    pub id: String,
    /// Bounded product name, when the adapter reports one.
    pub name: Option<String>,
    /// Adapter version currently serving this browser.
    pub adapter_version: String,
    /// Target browser platform/engine family.
    pub platform: BrowserPlatform,
    /// Whether this is the most recently attended connected browser.
    pub attended: bool,
}

/// Every connected browser, and the reported order in which they were last attended.
///
/// Browsers are plural. One person routinely runs Chrome and Edge at once, or two profiles of
/// one browser, and each is a different user context rather than a redundant server.
#[derive(Debug, Default)]
struct AdapterRegistry {
    /// One live connection per browser identity.
    ///
    /// A second connection carrying an identity that is already registered is a duplicate
    /// transport for one adapter, never a second browser: the adapter mints its identity once
    /// and keeps it across reconnects and service-worker restarts (ADR-0061).
    connections: HashMap<String, Connection>,
    /// Move-to-front browser attention order, most recent first.
    ///
    /// Attention is reported by adapters and outlives any single connection, so a browser that
    /// reconnects keeps the place its last reported attention earned. Connection order never
    /// enters this list (ADR-0084 D2).
    attention: Vec<String>,
    /// When the last initialized connection ended without a successor.
    ///
    /// A browser that left moments ago, such as an extension worker restarting its native port,
    /// is likely to come back. One that left longer ago than the return grace is evidence of
    /// nothing.
    departed: Option<Instant>,
}

impl AdapterRegistry {
    /// Remove exactly the named connection, never one that has since replaced it, and remember
    /// when an initialized connection left.
    fn remove_exact(&mut self, browser: &str, connection_id: &str) -> Option<Connection> {
        if self
            .connections
            .get(browser)
            .is_none_or(|connection| connection.id != connection_id)
        {
            return None;
        }
        let removed = self.connections.remove(browser)?;
        if removed.ready {
            self.departed = Some(Instant::now());
        }
        Some(removed)
    }

    /// Whether a browser is evidently on its way back at `now` (ADR-0196 Decision 12).
    ///
    /// Each piece of evidence lapses on its own: a registered connection that is still
    /// initializing or silent lasts as long as the connection, a departure lasts the return
    /// grace, and a freshly opened port lasts the relay return window.
    fn returning(&self, now: Instant, opened: Instant, timeout: Duration) -> bool {
        let initializing_or_silent = self
            .connections
            .values()
            .any(|connection| !connection.available(now, timeout));
        let departed_recently = self
            .departed
            .is_some_and(|departed| now.saturating_duration_since(departed) < RETURN_GRACE);
        let relays_reconnecting = now.saturating_duration_since(opened) < RELAY_RETURN_WINDOW;
        initializing_or_silent || departed_recently || relays_reconnecting
    }

    /// Record that one browser was attended most recently.
    fn attend(&mut self, browser: &str) {
        self.attention.retain(|known| known != browser);
        self.attention.insert(0, browser.into());
    }

    /// The most recently attended browser that is currently connected.
    fn attended(&self) -> Option<&str> {
        self.attention
            .iter()
            .find(|browser| self.connections.contains_key(*browser))
            .map(String::as_str)
    }

    /// What one browser's connection can do right now.
    fn reachability(&self, browser: &str, now: Instant, timeout: Duration) -> Reachability {
        let Some(connection) = self.connections.get(browser) else {
            return Reachability::Absent;
        };
        if !connection.ready {
            return Reachability::Initializing;
        }
        match &connection.liveness {
            Some(liveness) if !connection.available(now, timeout) => Reachability::Silent {
                writer: Arc::clone(&connection.writer),
                liveness: Arc::clone(liveness),
            },
            _ => Reachability::Available,
        }
    }

    /// Every initialized connection whose liveness has lapsed, with what a probe needs.
    fn silent(
        &self,
        now: Instant,
        timeout: Duration,
    ) -> Vec<(String, Arc<SocketWriter>, Arc<Liveness>)> {
        self.connections
            .values()
            .filter(|connection| connection.ready && !connection.available(now, timeout))
            .filter_map(|connection| {
                connection.liveness.as_ref().map(|liveness| {
                    (
                        connection.browser_id.clone(),
                        Arc::clone(&connection.writer),
                        Arc::clone(liveness),
                    )
                })
            })
            .collect()
    }

    /// Content-free inventory of connected browsers, most recently attended first.
    fn summaries(&self, timeout: Duration) -> Vec<BrowserSummary> {
        let now = Instant::now();
        let attended = self.attended().map(str::to_owned);
        let mut summaries: Vec<_> = self
            .connections
            .values()
            .filter(|connection| connection.available(now, timeout))
            .map(|connection| BrowserSummary {
                id: connection.browser_id.clone(),
                name: connection.browser_name.clone(),
                adapter_version: connection.adapter_version.clone(),
                platform: connection.adapter.platform(),
                attended: attended.as_deref() == Some(connection.browser_id.as_str()),
            })
            .collect();
        summaries.sort_by(|left, right| {
            right
                .attended
                .cmp(&left.attended)
                .then_with(|| left.id.cmp(&right.id))
        });
        summaries
    }
}

#[derive(Debug)]
struct ConnectionLiveness {
    last_acknowledged_at: Instant,
    next_sequence: u32,
    last_acknowledged_sequence: u32,
    stale: bool,
    /// Whether the heartbeat timeout already ran out during the current silence.
    timed_out: bool,
}

impl ConnectionLiveness {
    fn new(now: Instant) -> Self {
        Self {
            last_acknowledged_at: now,
            next_sequence: 0,
            last_acknowledged_sequence: 0,
            stale: false,
            timed_out: false,
        }
    }

    fn begin_probe(&mut self) -> u32 {
        self.next_sequence = self.next_sequence.saturating_add(1).max(1);
        self.next_sequence
    }

    /// Record one acknowledgement. Returns whether it ended a stale period.
    fn acknowledge(&mut self, sequence: u32, now: Instant) -> bool {
        if sequence == 0 || sequence > self.next_sequence {
            return false;
        }
        self.last_acknowledged_at = now;
        self.last_acknowledged_sequence = self.last_acknowledged_sequence.max(sequence);
        self.timed_out = false;
        std::mem::replace(&mut self.stale, false)
    }

    fn acknowledged(&self, sequence: u32) -> bool {
        self.last_acknowledged_sequence >= sequence
    }

    fn is_available(&self, now: Instant, timeout: Duration) -> bool {
        !self.stale && now.saturating_duration_since(self.last_acknowledged_at) < timeout
    }

    /// Mark the adapter as not answering. Returns whether it was answering before.
    fn mark_stale(&mut self) -> bool {
        let changed = !self.stale;
        self.stale = true;
        changed
    }

    /// Apply the heartbeat timeout: when nothing was acknowledged for `timeout`, the adapter is
    /// not answering. Returns whether this call is the first to see the timeout run out during
    /// the current silence, and whether it made the adapter stale.
    ///
    /// The two differ when a dispatch probe already marked the adapter stale: the timeout still
    /// runs out once, and that is still the moment the connection's pending calls end.
    fn expire(&mut self, now: Instant, timeout: Duration) -> (bool, bool) {
        if self.timed_out || now.saturating_duration_since(self.last_acknowledged_at) < timeout {
            return (false, false);
        }
        self.timed_out = true;
        (true, self.mark_stale())
    }
}

/// One connection's liveness record, and the observer told when the adapter stops or resumes
/// answering.
///
/// Every stale transition goes through here, so the diagnostics log sees each one exactly once,
/// whichever thread noticed it.
struct Liveness {
    state: Mutex<ConnectionLiveness>,
    browser_id: String,
    lifecycle: Option<Arc<dyn AdapterLifecycleObserver>>,
}

impl std::fmt::Debug for Liveness {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Liveness")
            .field("state", &*self.state())
            .field("browser_id", &self.browser_id)
            .finish_non_exhaustive()
    }
}

impl Liveness {
    fn new(
        browser_id: String,
        lifecycle: Option<Arc<dyn AdapterLifecycleObserver>>,
        now: Instant,
    ) -> Self {
        Self {
            state: Mutex::new(ConnectionLiveness::new(now)),
            browser_id,
            lifecycle,
        }
    }

    fn state(&self) -> MutexGuard<'_, ConnectionLiveness> {
        lock(&self.state)
    }

    /// Record one acknowledgement, reporting the adapter answering again after a stale period.
    fn acknowledge(&self, sequence: u32, now: Instant) {
        let recovered = self.state().acknowledge(sequence, now);
        if recovered {
            if let Some(observer) = &self.lifecycle {
                observer.adapter_answering(&self.browser_id);
            }
        }
    }

    /// Mark the adapter as not answering, reporting the transition. Returns whether it changed.
    fn mark_stale(&self) -> bool {
        let changed = self.state().mark_stale();
        self.report_stale(changed);
        changed
    }

    /// Apply the heartbeat timeout, reporting a stale transition. Returns whether the timeout
    /// ran out for the first time during the current silence.
    fn expire(&self, now: Instant, timeout: Duration) -> bool {
        let (timed_out, changed) = self.state().expire(now, timeout);
        self.report_stale(changed);
        timed_out
    }

    fn report_stale(&self, changed: bool) {
        if changed {
            if let Some(observer) = &self.lifecycle {
                observer.adapter_unanswered(&self.browser_id);
            }
        }
    }
}

/// The liveness probe written right after one dispatched request.
struct DispatchProbe {
    liveness: Arc<Liveness>,
    sequence: u32,
    sent: Instant,
    verdict: Duration,
}

impl DispatchProbe {
    /// Judge the adapter at this call's deadline.
    ///
    /// Only a probe unanswered for the whole verdict window marks the adapter as not answering.
    /// A shorter deadline proves nothing, because the acknowledgement may still be queued behind
    /// another call's large receipt. Either way the judgement changes availability for new
    /// dispatch only: other calls in flight keep waiting for their own receipts, and ending the
    /// whole connection stays with the heartbeat thread and a real end-of-stream.
    fn judge(&self, now: Instant) {
        let unanswered = !self.liveness.state().acknowledged(self.sequence);
        if unanswered && now.saturating_duration_since(self.sent) >= self.verdict {
            self.liveness.mark_stale();
        }
    }
}

/// Observes adapter connection lifecycle at the physical port boundary. The port calls this
/// where connections are registered, replaced, and lost, and where an attached adapter stops or
/// resumes answering; observers log or publish.
pub trait AdapterLifecycleObserver: Send + Sync + 'static {
    /// A browser's connection finished initializing and can take work.
    fn adapter_attached(&self, browser_id: &str, replaced: bool);

    /// A browser's announced connection ended.
    fn adapter_detached(&self, browser_id: &str);

    /// A browser's connection is still attached but stopped acknowledging liveness probes.
    fn adapter_unanswered(&self, _browser_id: &str) {}

    /// A browser's connection that had stopped answering acknowledged a probe again.
    fn adapter_answering(&self, _browser_id: &str) {}
}

/// Authenticated loopback implementation of the physical browser port.
pub struct RelayBrowserPort {
    service_epoch: String,
    adapters: Arc<Mutex<AdapterRegistry>>,
    event_sink: Mutex<Option<Arc<dyn BrowserEventSink>>>,
    control_state: Mutex<RuntimeControlState>,
    diagnostics: Mutex<Option<DiagnosticsState>>,
    lifecycle: Mutex<Option<Arc<dyn AdapterLifecycleObserver>>>,
    heartbeat: HeartbeatSettings,
    /// When relays could last begin reconnecting to this port: its construction, then the moment
    /// the service published the endpoint relays find it by. Right after that, relays that served
    /// an earlier service are still on their way back.
    opened: Mutex<Instant>,
}

impl std::fmt::Debug for RelayBrowserPort {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RelayBrowserPort")
            .field("connected", &self.is_connected())
            .finish_non_exhaustive()
    }
}

impl RelayBrowserPort {
    /// Construct a disconnected relay port for one restart-local service epoch.
    #[must_use]
    pub fn new(service_epoch: String) -> Self {
        Self::with_settings(service_epoch, HeartbeatSettings::default())
    }

    /// Construct a relay port with shortened liveness timing for tests.
    #[cfg(test)]
    pub(crate) fn with_heartbeat_settings(
        service_epoch: String,
        heartbeat: HeartbeatSettings,
    ) -> Self {
        debug_assert!(heartbeat.interval < heartbeat.timeout);
        Self::with_settings(service_epoch, heartbeat)
    }

    fn with_settings(service_epoch: String, heartbeat: HeartbeatSettings) -> Self {
        Self {
            service_epoch,
            adapters: Arc::new(Mutex::new(AdapterRegistry::default())),
            event_sink: Mutex::new(None),
            control_state: Mutex::new(RuntimeControlState::Active),
            diagnostics: Mutex::new(None),
            lifecycle: Mutex::new(None),
            heartbeat,
            opened: Mutex::new(Instant::now()),
        }
    }

    /// Record that the service is publishing the endpoint relays find it by.
    ///
    /// Relays that served an earlier service reconnect within a retry or two of this moment, so
    /// it starts the short window in which a call with no browser waits for them. Construction
    /// is too early a start: the desktop may take a second or more to publish.
    pub fn endpoint_published(&self) {
        *lock(&self.opened) = Instant::now();
    }

    /// Move this port's evidence clock back, as if `elapsed` had passed since it opened and
    /// since its last browser left.
    #[cfg(test)]
    pub(crate) fn backdate(&self, elapsed: Duration) {
        let earlier = |instant: Instant| {
            instant
                .checked_sub(elapsed)
                .expect("the monotonic clock has run longer than the backdate")
        };
        {
            let mut opened = lock(&self.opened);
            *opened = earlier(*opened);
        }
        let mut adapters = lock(&self.adapters);
        adapters.departed = adapters.departed.map(earlier);
    }

    /// Install the direct typed event reaction target.
    pub fn set_event_sink(&self, sink: Arc<dyn BrowserEventSink>) {
        *lock(&self.event_sink) = Some(sink);
    }

    /// Install the adapter lifecycle observer.
    pub fn set_lifecycle_observer(&self, observer: Arc<dyn AdapterLifecycleObserver>) {
        *lock(&self.lifecycle) = Some(observer);
    }

    /// Whether at least one compatible adapter is currently connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        !self.connected_browsers().is_empty()
    }

    /// Every connected browser, most recently attended first.
    #[must_use]
    pub fn connected_browsers(&self) -> Vec<BrowserSummary> {
        lock(&self.adapters).summaries(self.heartbeat.timeout)
    }

    /// Whether one browser still holds a registered connection, available or not.
    ///
    /// An adapter that stops acknowledging is unavailable but still attached: its socket is open
    /// and it may answer again. That is a different fact from being connected, and the difference
    /// is what stops a quarantined adapter from being mistaken for a departed one.
    #[cfg(test)]
    pub(crate) fn is_registered(&self, browser: &str) -> bool {
        lock(&self.adapters).connections.contains_key(browser)
    }

    /// Return the adapter version of the most recently attended browser for diagnostics.
    #[must_use]
    pub fn adapter_version(&self) -> Option<String> {
        self.connected_browsers()
            .into_iter()
            .next()
            .map(|browser| browser.adapter_version)
    }

    /// Return the persistent installation id of the most recently attended browser.
    #[must_use]
    pub fn browser_id(&self) -> Option<String> {
        self.connected_browsers()
            .into_iter()
            .next()
            .map(|browser| browser.id)
    }

    /// Negotiate and attach one already-authenticated browser-relay stream.
    pub fn attach(&self, stream: TcpStream) -> Result<(), BrowserError> {
        stream
            .set_nonblocking(false)
            .map_err(|error| BrowserError::Protocol(error.to_string()))?;
        stream
            .set_nodelay(true)
            .map_err(|error| BrowserError::Protocol(error.to_string()))?;
        let reader = SocketReader::new(
            stream
                .try_clone()
                .map_err(|error| BrowserError::Protocol(error.to_string()))?,
        );
        self.attach_reader(stream, reader, Instant::now() + EXCHANGE_TIMEOUT)
    }

    /// Continue authentication using the reader that may already hold coalesced adapter bytes.
    pub(crate) fn attach_reader(
        &self,
        stream: TcpStream,
        mut reader: SocketReader,
        opening: Instant,
    ) -> Result<(), BrowserError> {
        let Some(BrowserFrame::Hello {
            major,
            adapter_version,
            browser_id,
            adapter_epoch,
            browser_name,
            platform,
            attended,
            capabilities,
        }) = reader
            .native(Some(opening))
            .map_err(|error| BrowserError::Protocol(error.to_string()))?
        else {
            return Err(BrowserError::Authentication);
        };
        let adapter = Adapter::negotiate(platform, major).map_err(
            |UnsupportedHello { offered, required }| BrowserError::Incompatible {
                offered,
                required,
            },
        )?;
        if !browser_id.starts_with("browser_")
            || browser_id.len() <= "browser_".len()
            || browser_id.len() > 80
            || !browser_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(BrowserError::Authentication);
        }
        if !valid_opaque_id(&adapter_epoch, "adapter_") {
            return Err(BrowserError::Authentication);
        }
        let capabilities = validated_capabilities(capabilities)?;
        let browser_name = validated_browser_name(browser_name)?;
        let reports_attention = capabilities
            .get(adapter_capability::ADAPTER_ATTENTION)
            .copied()
            .unwrap_or_default()
            >= 1;
        let lifecycle = lock(&self.lifecycle).clone();
        let liveness = (capabilities
            .get(adapter_capability::ADAPTER_LIVENESS)
            .copied()
            .unwrap_or_default()
            >= 1)
            .then(|| {
                Arc::new(Liveness::new(
                    browser_id.clone(),
                    lifecycle.clone(),
                    Instant::now(),
                ))
            });

        let writer = Arc::new(
            SocketWriter::new(stream).map_err(|error| BrowserError::Protocol(error.to_string()))?,
        );
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let connection_id = format!("connection_{}", Uuid::new_v4().simple());
        let connection = Connection {
            id: connection_id.clone(),
            writer: Arc::clone(&writer),
            pending: Arc::clone(&pending),
            adapter_version,
            adapter,
            browser_id: browser_id.clone(),
            browser_name,
            capabilities,
            liveness: liveness.clone(),
            ready: false,
        };
        let replaced = {
            let mut adapters = lock(&self.adapters);
            if !adapters.connections.contains_key(&browser_id)
                && adapters.connections.len() >= MAX_BROWSER_CONNECTIONS
            {
                writer.close();
                return Err(BrowserError::Protocol(
                    "browser connection capacity reached".into(),
                ));
            }
            let previous = adapters.connections.insert(browser_id.clone(), connection);
            if let Some(previous) = &previous {
                retire(previous);
            }
            if reports_attention && attended {
                adapters.attend(&browser_id);
            }
            previous.is_some()
        };
        if let Err(error) = writer.native(&BrowserFrame::HelloAccepted {
            major: adapter.level().major(),
            service_version: env!("CARGO_PKG_VERSION").into(),
            service_epoch: self.service_epoch.clone(),
            control_state: *lock(&self.control_state),
            diagnostics: *lock(&self.diagnostics),
        }) {
            self.detach_registered(&browser_id, &connection_id);
            return Err(BrowserError::Protocol(error.to_string()));
        }
        let sink = lock(&self.event_sink).clone();
        let tag = ConnectionTag {
            browser_id,
            connection_id,
        };
        let announced = Arc::new(AtomicBool::new(false));
        let reader_adapters = Arc::clone(&self.adapters);
        let reader_tag = tag.clone();
        let reader_liveness = liveness.clone();
        let reader_notifications = AdapterNotifications {
            sink,
            lifecycle,
            announced: Arc::clone(&announced),
        };
        let heartbeat_writer = Arc::clone(&writer);
        let heartbeat_pending = Arc::clone(&pending);
        if let Err(error) = thread::Builder::new()
            .name("ghostlight-browser-reader".into())
            .spawn(move || {
                read_adapter(
                    reader,
                    writer,
                    pending,
                    reader_notifications,
                    reader_adapters,
                    reader_tag,
                    reader_liveness,
                );
            })
        {
            // The connection was registered before this thread could prove it would ever run.
            // Leaving the registration in place on a spawn failure is a permanent zombie: a
            // connection with no liveness capability negotiated reports "always available"
            // forever (nothing else ever marks it stale), so every future call() to this
            // browser_id would insert into pending, write successfully (the socket is still
            // open), and simply time out, repeatedly, until this exact browser reconnects.
            // Remove it, matched by connection id so a legitimate concurrent replacement is
            // never the one torn down instead.
            self.detach_registered(&tag.browser_id, &tag.connection_id);
            return Err(BrowserError::Protocol(error.to_string()));
        }

        let installing = {
            let adapters = lock(&self.adapters);
            adapters
                .connections
                .get(&tag.browser_id)
                .filter(|connection| connection.id == tag.connection_id)
                .filter(|connection| connection.adapter.installs_assets(&connection.capabilities))
                .map(|connection| connection.adapter)
        };
        if let Some(adapter) = installing {
            let cancelled = AtomicBool::new(false);
            let installed = self.call(
                &tag.browser_id,
                "system",
                adapter.install_command(),
                Instant::now() + EXCHANGE_TIMEOUT,
                &cancelled,
            );
            match installed {
                Ok(outcome) if adapter.acknowledged(&outcome) => {}
                Ok(_) => {
                    self.detach_registered(&tag.browser_id, &tag.connection_id);
                    return Err(BrowserError::Protocol(
                        "adapter acknowledged a different page runtime".into(),
                    ));
                }
                Err(error) => {
                    self.detach_registered(&tag.browser_id, &tag.connection_id);
                    return Err(error);
                }
            }
        }
        if !self.publish_ready(&tag, &announced, replaced) {
            return Err(BrowserError::DisconnectedBeforeDispatch);
        }

        if let Some(liveness) = liveness {
            let heartbeat_adapters = Arc::clone(&self.adapters);
            let settings = self.heartbeat;
            let heartbeat_tag = tag.clone();
            if let Err(error) = thread::Builder::new()
                .name("ghostlight-browser-heartbeat".into())
                .spawn(move || {
                    heartbeat_adapter(
                        heartbeat_writer,
                        heartbeat_pending,
                        liveness,
                        heartbeat_adapters,
                        heartbeat_tag,
                        settings,
                    );
                })
            {
                // The reader thread is already live at this point. Losing only the heartbeat is
                // not the same zombie: a liveness-negotiated connection with nobody updating it
                // goes stale on the normal schedule and is then correctly treated as
                // unavailable, so this is tidiness rather than a silent-forever failure -- still
                // worth cleaning up rather than leaving two threads disagreeing about the same
                // connection.
                self.detach_registered(&tag.browser_id, &tag.connection_id);
                return Err(BrowserError::Protocol(error.to_string()));
            }
        }
        Ok(())
    }

    /// Publish one exact initialized connection to callers. A replaced connection can never
    /// make its successor ready.
    fn publish_ready(&self, tag: &ConnectionTag, announced: &AtomicBool, replaced: bool) -> bool {
        let lifecycle = lock(&self.lifecycle).clone();
        let mut adapters = lock(&self.adapters);
        let Some(connection) = adapters.connections.get_mut(&tag.browser_id) else {
            return false;
        };
        if connection.id != tag.connection_id {
            return false;
        }
        connection.ready = true;
        announced.store(true, Ordering::SeqCst);
        if let Some(observer) = lifecycle.as_ref() {
            observer.adapter_attached(&tag.browser_id, replaced);
        }
        true
    }

    /// Remove a registered connection, but only the exact one named -- never a connection that
    /// has since replaced it. `attach` registers a connection before either of its threads is
    /// proven to actually run, so a spawn failure must undo exactly that registration, and
    /// nothing else: a concurrent `attach` for the same `browser_id` may already have replaced it
    /// with a newer, healthy connection by the time this runs, and that one must be left alone.
    fn detach_registered(&self, browser_id: &str, connection_id: &str) {
        let removed = lock(&self.adapters).remove_exact(browser_id, connection_id);
        if let Some(connection) = removed {
            let was_ready = connection.ready;
            retire(&connection);
            if was_ready {
                if let Some(observer) = lock(&self.lifecycle).clone().as_ref() {
                    observer.adapter_detached(browser_id);
                }
            }
        }
    }

    fn call_inner(
        &self,
        browser: &str,
        workspace: &str,
        command: Work,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        let BrowserDispatch {
            deadline,
            cancelled,
            admit,
        } = dispatch;
        if cancelled.load(Ordering::SeqCst) {
            return Err(BrowserError::CancelledBeforeDispatch);
        }
        if Instant::now() >= deadline {
            return Err(BrowserError::DeadlineBeforeDispatch);
        }
        let correlation = format!("physical_{}", Uuid::new_v4().simple());
        let (sender, receiver) = mpsc::channel();
        let required_capability = command.required_capability();
        let (writer, pending, chunked_commands, liveness) = {
            let adapters = lock(&self.adapters);
            let Some(connection) = adapters.connections.get(browser) else {
                return Err(BrowserError::DisconnectedBeforeDispatch);
            };
            if !connection.ready && !matches!(&command, Work::InstallPageRuntime { .. }) {
                return Err(BrowserError::NotAnsweringBeforeDispatch);
            }
            if connection.liveness.as_ref().is_some_and(|liveness| {
                !liveness
                    .state()
                    .is_available(Instant::now(), self.heartbeat.timeout)
            }) {
                return Err(BrowserError::NotAnsweringBeforeDispatch);
            }
            let advertised = connection
                .capabilities
                .get(required_capability)
                .copied()
                .unwrap_or_default();
            if advertised < command.required_revision() {
                return Err(BrowserError::CapabilityVersion {
                    capability: required_capability.to_string(),
                    required: command.required_revision(),
                    advertised,
                });
            }
            if let Work::InDocuments { primitive, .. } = &command {
                if matches!(
                    **primitive,
                    Work::InDocuments { .. } | Work::DescribeDocuments { .. }
                ) {
                    return Err(BrowserError::Protocol(
                        "nested document execution is invalid".into(),
                    ));
                }
                let capability = primitive.required_capability();
                let advertised = connection
                    .capabilities
                    .get(capability)
                    .copied()
                    .unwrap_or_default();
                if advertised < primitive.required_revision() {
                    return Err(BrowserError::CapabilityVersion {
                        capability: capability.into(),
                        required: primitive.required_revision(),
                        advertised,
                    });
                }
            }
            (
                Arc::clone(&connection.writer),
                Arc::clone(&connection.pending),
                connection
                    .capabilities
                    .get(adapter_capability::CHUNKED_COMMANDS)
                    .copied()
                    .unwrap_or_default()
                    >= 1,
                connection.liveness.clone(),
            )
        };
        let frame = BrowserFrame::Request {
            request: BrowserRequest {
                correlation: correlation.clone(),
                workspace: workspace.into(),
                command,
            },
        };
        let payload = serde_json::to_vec(&frame)
            .map_err(|error| BrowserError::Protocol(error.to_string()))?;
        if payload.len() > COMMAND_TRANSFER_MAX_BYTES {
            return Err(BrowserError::Primitive(format!(
                "browser request exceeds the {COMMAND_TRANSFER_MAX_BYTES}-byte transfer bound"
            )));
        }
        if payload.len() > DIRECT_NATIVE_MESSAGE_BYTES && !chunked_commands {
            return Err(BrowserError::Primitive(
                "adapter does not support chunked command transfers".into(),
            ));
        }
        let mut output = writer
            .until_unless(deadline.min(Instant::now() + DELIVERY_TIMEOUT), || {
                cancelled.load(Ordering::SeqCst)
            })
            .map_err(|_| {
                if cancelled.load(Ordering::SeqCst) {
                    BrowserError::CancelledBeforeDispatch
                } else if Instant::now() >= deadline {
                    BrowserError::DeadlineBeforeDispatch
                } else {
                    // Another call held the writer for the whole delivery budget. The connection
                    // is busy, not gone, and nothing of this call was written.
                    BrowserError::BusyBeforeDispatch
                }
            })?;
        if cancelled.load(Ordering::SeqCst) {
            return Err(BrowserError::CancelledBeforeDispatch);
        }
        if Instant::now() >= deadline {
            return Err(BrowserError::DeadlineBeforeDispatch);
        }
        admit()?;
        lock(&pending).insert(correlation.clone(), sender);
        let probe = liveness
            .as_ref()
            .map(|liveness| liveness.state().begin_probe());
        if write_request_payload(&mut output, &payload, &correlation).is_err() {
            lock(&pending).remove(&correlation);
            mark_stale(&liveness);
            return Err(BrowserError::DisconnectedAfterDispatch);
        }
        if let Some(sequence) = probe {
            if write_native(&mut output, &BrowserFrame::Heartbeat { sequence }).is_err() {
                lock(&pending).remove(&correlation);
                mark_stale(&liveness);
                return Err(BrowserError::DisconnectedAfterDispatch);
            }
        }
        drop(output);
        let sent = Instant::now();
        let probe = liveness
            .zip(probe)
            .map(|(liveness, sequence)| DispatchProbe {
                liveness,
                sequence,
                sent,
                verdict: self.heartbeat.verdict,
            });
        await_receipt(
            receiver,
            &correlation,
            &writer,
            &pending,
            deadline,
            cancelled,
            probe,
        )
    }

    /// Wait for one browser's connection, sending a silent connection one immediate probe.
    fn await_registered(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &AtomicBool,
    ) -> Availability {
        let mut probed = false;
        loop {
            let reachability =
                lock(&self.adapters).reachability(browser, Instant::now(), self.heartbeat.timeout);
            let availability = match reachability {
                Reachability::Available => return Availability::Available,
                Reachability::Absent => Availability::Absent,
                Reachability::Initializing => Availability::NotAnswering,
                Reachability::Silent { writer, liveness } => {
                    if !probed {
                        probed = true;
                        probe_now(&writer, &liveness, until, cancelled);
                    }
                    Availability::NotAnswering
                }
            };
            if cancelled.load(Ordering::SeqCst) || Instant::now() >= until {
                return availability;
            }
            thread::sleep(
                until
                    .saturating_duration_since(Instant::now())
                    .min(AVAILABILITY_POLL),
            );
        }
    }

    /// Wait for any browser while one is evidently returning, sending each silent connection
    /// one immediate probe. The evidence is read again on every poll, so the wait ends as soon
    /// as it lapses.
    fn await_first(&self, until: Instant, cancelled: &AtomicBool) -> Vec<BrowserSummary> {
        let mut probed = Vec::new();
        loop {
            let connected = self.connected_browsers();
            if !connected.is_empty() {
                return connected;
            }
            let silent = lock(&self.adapters).silent(Instant::now(), self.heartbeat.timeout);
            for (browser, writer, liveness) in silent {
                if !probed.contains(&browser) {
                    probe_now(&writer, &liveness, until, cancelled);
                    probed.push(browser);
                }
            }
            if cancelled.load(Ordering::SeqCst)
                || Instant::now() >= until
                || !self.browser_returning()
            {
                return connected;
            }
            thread::sleep(
                until
                    .saturating_duration_since(Instant::now())
                    .min(AVAILABILITY_POLL),
            );
        }
    }
}

/// Send one liveness probe now instead of waiting up to a full heartbeat interval for the
/// timer. An acknowledgement makes a silent connection available again at once.
///
/// A writer busy with another call's transfer is left alone. Closing it over a probe would end
/// every call on the connection.
fn probe_now(writer: &SocketWriter, liveness: &Liveness, until: Instant, cancelled: &AtomicBool) {
    let delivery = until.min(Instant::now() + DELIVERY_TIMEOUT);
    let Ok(mut output) = writer.until_unless(delivery, || cancelled.load(Ordering::SeqCst)) else {
        return;
    };
    let sequence = liveness.state().begin_probe();
    let _ = write_native(&mut output, &BrowserFrame::Heartbeat { sequence });
}

impl BrowserPort for RelayBrowserPort {
    fn call(
        &self,
        browser: &str,
        workspace: &str,
        command: Work,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.call_inner(
            browser,
            workspace,
            command,
            BrowserDispatch {
                deadline,
                cancelled,
                admit: &|| Ok(()),
            },
        )
    }

    fn call_guarded(
        &self,
        browser: &str,
        workspace: &str,
        command: Work,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.call_inner(browser, workspace, command, dispatch)
    }

    fn browsers(&self) -> Vec<BrowserSummary> {
        self.connected_browsers()
    }

    /// A connection still installing its page runtime is waited on until it is ready. A silent
    /// one gets one immediate probe rather than the next timer heartbeat, up to 20 s away.
    fn await_available(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &AtomicBool,
    ) -> Availability {
        self.await_registered(browser, until, cancelled)
    }

    fn await_any(&self, until: Instant, cancelled: &AtomicBool) -> Vec<BrowserSummary> {
        self.await_first(until, cancelled)
    }

    /// The evidence is any of: a registered connection that is still initializing or silent;
    /// an initialized connection that ended less than the return grace ago, as when an
    /// extension worker restarts its native port; or an endpoint published less than the relay
    /// return window ago, while relays that served an earlier service reconnect. Attention a
    /// browser reported at some point is not evidence: it never lapses, and a browser closed long
    /// ago would then spend the whole wait that browser startup needs.
    fn browser_returning(&self) -> bool {
        let opened = *lock(&self.opened);
        lock(&self.adapters).returning(Instant::now(), opened, self.heartbeat.timeout)
    }

    /// Runtime control is a property of Ghostlight, not of one browser, so every connected
    /// adapter learns the new state. One unreachable browser does not hide the state from the
    /// rest.
    fn publish_control_state(&self, state: RuntimeControlState) -> Result<(), BrowserError> {
        *lock(&self.control_state) = state;
        self.broadcast_control(state)
    }
}

impl RelayBrowserPort {
    /// Publish process-diagnostics state to every connected adapter and remember it so the
    /// next hello carries it too.
    pub fn publish_diagnostics(&self, diagnostics: DiagnosticsState) -> Result<(), BrowserError> {
        *lock(&self.diagnostics) = Some(diagnostics);
        self.broadcast_control(*lock(&self.control_state))
    }

    fn broadcast_control(&self, state: RuntimeControlState) -> Result<(), BrowserError> {
        let frame = BrowserFrame::ControlState {
            state,
            diagnostics: *lock(&self.diagnostics),
        };
        let writers: Vec<_> = lock(&self.adapters)
            .connections
            .values()
            .map(|connection| Arc::clone(&connection.writer))
            .collect();
        let mut published = Ok(());
        let deadline = Instant::now() + CONTROL_DELIVERY_TIMEOUT;
        for writer in writers {
            if writer.native_until(&frame, deadline).is_err() {
                published = Err(BrowserError::DisconnectedAfterDispatch);
            }
        }
        published
    }
}

fn write_request_payload(
    writer: &mut impl std::io::Write,
    payload: &[u8],
    correlation: &str,
) -> Result<(), FrameError> {
    if payload.len() <= DIRECT_NATIVE_MESSAGE_BYTES {
        return write_length_frame(writer, payload);
    }
    if payload.len() > COMMAND_TRANSFER_MAX_BYTES {
        return Err(FrameError::TooLarge);
    }

    let count = payload.len().div_ceil(COMMAND_CHUNK_PAYLOAD_BYTES);
    let count = u16::try_from(count).map_err(|_| FrameError::TooLarge)?;
    if count > COMMAND_TRANSFER_MAX_CHUNKS {
        return Err(FrameError::TooLarge);
    }
    let total_bytes = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge)?;
    let transfer_id = format!("transfer_{}", Uuid::new_v4().simple());
    let mut sha256 = String::with_capacity(64);
    for byte in Sha256::digest(payload) {
        write!(&mut sha256, "{byte:02x}").expect("writing to a string cannot fail");
    }

    for (index, chunk) in payload.chunks(COMMAND_CHUNK_PAYLOAD_BYTES).enumerate() {
        let chunk_frame = BrowserFrame::CommandChunk {
            transfer_id: transfer_id.clone(),
            correlation: correlation.into(),
            index: u16::try_from(index).map_err(|_| FrameError::TooLarge)?,
            count,
            total_bytes,
            sha256: sha256.clone(),
            data: base64::engine::general_purpose::STANDARD.encode(chunk),
        };
        write_native(writer, &chunk_frame)?;
    }
    Ok(())
}

fn await_receipt(
    receiver: Receiver<PendingResult>,
    correlation: &str,
    writer: &Arc<SocketWriter>,
    pending: &Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
    probe: Option<DispatchProbe>,
) -> PendingResult {
    loop {
        if cancelled.load(Ordering::SeqCst) {
            send_cancel(writer, correlation);
            lock(pending).remove(correlation);
            return Err(BrowserError::CancelledAfterDispatch);
        }
        let now = Instant::now();
        if now >= deadline {
            send_cancel(writer, correlation);
            lock(pending).remove(correlation);
            if let Some(probe) = &probe {
                probe.judge(now);
            }
            return Err(BrowserError::DeadlineAfterDispatch);
        }
        let wait = deadline
            .saturating_duration_since(now)
            .min(Duration::from_millis(20));
        match receiver.recv_timeout(wait) {
            Ok(result) => return result,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(BrowserError::DisconnectedAfterDispatch)
            }
        }
    }
}

/// Ask the adapter to stop one call's work. Best effort.
///
/// A writer busy with another call's transfer is skipped rather than closed: one call's deadline
/// must never end the connection every other call on it is using.
fn send_cancel(writer: &Arc<SocketWriter>, correlation: &str) {
    let frame = BrowserFrame::Request {
        request: BrowserRequest {
            correlation: format!("cancel_{}", Uuid::new_v4().simple()),
            workspace: "system".into(),
            command: Work::Cancel {
                correlation: correlation.into(),
            },
        },
    };
    if let Ok(mut output) = writer.until(Instant::now() + CONTROL_DELIVERY_TIMEOUT) {
        let _ = write_native(&mut output, &frame);
    }
}

/// The notification targets one adapter reader reports to: the typed event sink and the
/// lifecycle observer.
struct AdapterNotifications {
    sink: Option<Arc<dyn BrowserEventSink>>,
    lifecycle: Option<Arc<dyn AdapterLifecycleObserver>>,
    announced: Arc<AtomicBool>,
}

fn read_adapter(
    mut reader: SocketReader,
    writer: Arc<SocketWriter>,
    pending: Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    notifications: AdapterNotifications,
    adapters: Arc<Mutex<AdapterRegistry>>,
    tag: ConnectionTag,
    liveness: Option<Arc<Liveness>>,
) {
    let ConnectionTag { browser_id, .. } = &tag;
    loop {
        match reader.native::<BrowserFrame>(None) {
            Ok(Some(BrowserFrame::Receipt { receipt })) => {
                let correlation = receipt.correlation.clone();
                if let Some(sender) = lock(&pending).remove(&correlation) {
                    let _ = sender.send(Ok(receipt.result));
                }
                acknowledge(&writer, correlation);
            }
            Ok(Some(BrowserFrame::Error {
                correlation: Some(correlation),
                code,
                message,
                effect_unknown,
            })) => {
                if let Some(sender) = lock(&pending).remove(&correlation) {
                    let error = adapter_error(&code, message, effect_unknown);
                    let _ = sender.send(Err(error));
                }
                acknowledge(&writer, correlation);
            }
            Ok(Some(BrowserFrame::Event { event })) => {
                if !is_current(&adapters, &tag) {
                    continue;
                }
                if matches!(event, BrowserEvent::Attended) {
                    lock(&adapters).attend(browser_id);
                    continue;
                }
                if let Some(sink) = &notifications.sink {
                    sink.on_event(browser_id, event);
                }
            }
            Ok(Some(BrowserFrame::HeartbeatAck { sequence })) => {
                if let Some(liveness) = &liveness {
                    liveness.acknowledge(sequence, Instant::now());
                }
            }
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                writer.close();
                fail_pending(&pending, BrowserError::DisconnectedAfterDispatch);
                // A replaced connection must not evict the connection that replaced it. Its
                // attention place survives, so the same browser reconnecting resumes where it
                // was rather than starting behind every other browser.
                let was_current = lock(&adapters)
                    .remove_exact(browser_id, &tag.connection_id)
                    .is_some();
                if was_current {
                    if let Some(sink) = &notifications.sink {
                        sink.on_event(browser_id, BrowserEvent::Disconnected);
                    }
                    if notifications.announced.load(Ordering::SeqCst) {
                        if let Some(observer) = &notifications.lifecycle {
                            observer.adapter_detached(browser_id);
                        }
                    }
                }
                return;
            }
        }
    }
}

/// The timer side of liveness, and the one owner of whole-connection failure short of a real
/// end-of-stream: only a full heartbeat timeout without any acknowledgement fails the
/// connection's pending calls.
fn heartbeat_adapter(
    writer: Arc<SocketWriter>,
    pending: Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    liveness: Arc<Liveness>,
    adapters: Arc<Mutex<AdapterRegistry>>,
    tag: ConnectionTag,
    settings: HeartbeatSettings,
) {
    loop {
        if writer.wait_closed(settings.interval) {
            return;
        }
        if !is_current(&adapters, &tag) {
            return;
        }

        if liveness.expire(Instant::now(), settings.timeout) {
            fail_pending(&pending, BrowserError::DisconnectedAfterDispatch);
        }
        let sequence = liveness.state().begin_probe();
        if writer
            .native(&BrowserFrame::Heartbeat { sequence })
            .is_err()
        {
            liveness.mark_stale();
            return;
        }
    }
}

/// Which connection, of which browser, one background thread is serving.
///
/// Both halves are needed together: the browser says where its work belongs, and the connection
/// says whether this particular socket is still the one carrying it.
#[derive(Clone, Debug)]
struct ConnectionTag {
    browser_id: String,
    connection_id: String,
}

/// Whether this connection is still the one serving its browser.
fn is_current(adapters: &Mutex<AdapterRegistry>, tag: &ConnectionTag) -> bool {
    lock(adapters)
        .connections
        .get(&tag.browser_id)
        .is_some_and(|connection| connection.id == tag.connection_id)
}

/// End a connection that a fresher connection from the same browser has replaced.
///
/// Failing its pending work is not enough. An abandoned but still-open socket keeps its relay
/// process alive and keeps the browser's stale native port alive with it, so nothing on either
/// shore ever learns the connection is finished, and a request written into it is dropped in
/// silence rather than refused. Closing the stream is what makes the duplicate collapse: the
/// relay reads end-of-stream and exits, and the browser observes its port disconnect.
fn retire(previous: &Connection) {
    fail_pending(&previous.pending, BrowserError::DisconnectedAfterDispatch);
    previous.writer.close();
}

/// Validate the optional bounded product name an adapter reports for itself.
fn validated_browser_name(name: Option<String>) -> Result<Option<String>, BrowserError> {
    let Some(name) = name else {
        return Ok(None);
    };
    let acceptable = !name.trim().is_empty()
        && name.chars().count() <= BROWSER_NAME_MAX_CHARS
        && !name.chars().any(char::is_control);
    if !acceptable {
        return Err(BrowserError::Authentication);
    }
    Ok(Some(name))
}

fn mark_stale(liveness: &Option<Arc<Liveness>>) {
    if let Some(liveness) = liveness {
        liveness.mark_stale();
    }
}

fn acknowledge(writer: &Arc<SocketWriter>, correlation: String) {
    let _ = writer.native(&BrowserFrame::Acknowledge { correlation });
}

fn adapter_error(code: &str, message: String, effect_unknown: bool) -> BrowserError {
    if effect_unknown {
        BrowserError::EffectUnknown(message)
    } else if code == "local_interlock" {
        BrowserError::LocalInterlock(message)
    } else if code == "document_scope_changed" {
        BrowserError::DocumentUnavailable
    } else {
        BrowserError::Primitive(message)
    }
}

fn valid_opaque_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn validated_capabilities(
    capabilities: Vec<AdapterCapability>,
) -> Result<HashMap<String, u16>, BrowserError> {
    let mut validated = HashMap::new();
    for capability in capabilities {
        let valid_name = !capability.name.is_empty()
            && capability.name.len() <= 64
            && capability
                .name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
        if !valid_name || capability.revision == 0 {
            return Err(BrowserError::Authentication);
        }
        if validated
            .insert(capability.name, capability.revision)
            .is_some()
        {
            return Err(BrowserError::Authentication);
        }
    }
    Ok(validated)
}

fn fail_pending(pending: &Mutex<HashMap<String, Sender<PendingResult>>>, error: BrowserError) {
    let senders: Vec<_> = lock(pending).drain().map(|(_, sender)| sender).collect();
    for sender in senders {
        let _ = sender.send(Err(error.clone()));
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Truth-preserving physical-browser failure classes.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BrowserError {
    /// The authority refused the actual embedded-document subject before access.
    #[error("document access denied: {0:?}")]
    DocumentAccess(crate::governance::Decision),
    /// Current document scope could not be established before access.
    #[error("document scope unavailable before access")]
    DocumentUnavailable,
    /// Application control refused before this physical dispatch began.
    #[error("runtime control refused browser dispatch: {0:?}")]
    RuntimeControl(crate::governance::ReasonCode),
    /// No adapter existed before dispatch.
    #[error("browser adapter is disconnected before dispatch")]
    DisconnectedBeforeDispatch,
    /// The browser holds a registered connection that could not take work before dispatch: it
    /// is still initializing, or it stopped acknowledging liveness probes.
    #[error("browser adapter is not answering before dispatch")]
    NotAnsweringBeforeDispatch,
    /// Other work held the connection's writer for the whole delivery budget, so nothing of this
    /// request was written. The connection is busy, not gone.
    #[error("browser connection was busy before dispatch")]
    BusyBeforeDispatch,
    /// The browser this workspace works in is not connected, while a different browser is.
    #[error("the workspace's browser is not connected, but a different browser is")]
    WrongProfile,
    /// The connection ended after a request could have reached the adapter.
    #[error("browser adapter disconnected after dispatch")]
    DisconnectedAfterDispatch,
    /// Cancellation was observed before dispatch.
    #[error("invocation was cancelled before dispatch")]
    CancelledBeforeDispatch,
    /// Cancellation was observed after dispatch.
    #[error("invocation was cancelled after dispatch")]
    CancelledAfterDispatch,
    /// Deadline expired before dispatch.
    #[error("deadline expired before dispatch")]
    DeadlineBeforeDispatch,
    /// Deadline expired after dispatch.
    #[error("deadline expired after dispatch")]
    DeadlineAfterDispatch,
    /// Adapter decisively rejected a primitive without an effect.
    #[error("browser primitive failed: {0}")]
    Primitive(String),
    /// A browser-local human safety choice refused the primitive without an effect.
    #[error("browser local interlock refused the primitive: {0}")]
    LocalInterlock(String),
    /// Adapter explicitly reported an uncertain effect.
    #[error("browser effect is unknown: {0}")]
    EffectUnknown(String),
    /// Browser bridge framing or message state failed.
    #[error("browser bridge protocol failed: {0}")]
    Protocol(String),
    /// Adapter identity or capability negotiation was invalid.
    #[error("browser adapter negotiation failed")]
    Authentication,
    /// The caller named a browser that is not connected.
    #[error("browser {0} is not connected")]
    UnknownBrowser(String),
    /// The caller named a browser other than the one this workspace already works in.
    #[error("workspace is already working in another browser")]
    BrowserPinned,
    /// Several browsers are connected and nothing implies which one the work belongs to.
    #[error("several browsers are connected and none was selected")]
    AmbiguousBrowser(Vec<String>),
    /// Browser readiness recovery reached a useful manual-mode outcome.
    #[error("browser startup is manual")]
    RecoveryManual {
        /// Installed browser names with current Ghostlight native-host registration.
        browsers: Vec<String>,
    },
    /// Browser readiness recovery reached one exact closed failure.
    #[error("browser recovery failed: {reason:?}")]
    RecoveryFailed {
        /// Exact failure class.
        reason: recovery::RecoveryFailure,
        /// Candidate names or package diagnoses.
        details: Vec<String>,
    },
    /// Browser adapter protocol major is incompatible.
    #[error("browser adapter protocol major {offered} is incompatible with required {required}")]
    Incompatible { offered: u16, required: u16 },
    /// The adapter advertises an older revision of a physical capability than the command requires.
    #[error(
        "adapter supports {capability} revision {advertised}, command requires revision {required}"
    )]
    CapabilityVersion {
        /// Physical capability family that is too old.
        capability: String,
        /// Minimum revision the command requires.
        required: u16,
        /// Highest revision the adapter advertised.
        advertised: u16,
    },
}

impl BrowserError {
    /// Whether dispatch could have caused a physical effect.
    #[must_use]
    pub const fn effect_unknown(&self) -> bool {
        matches!(
            self,
            Self::DisconnectedAfterDispatch
                | Self::CancelledAfterDispatch
                | Self::DeadlineAfterDispatch
                | Self::EffectUnknown(_)
        )
    }
}

#[cfg(test)]
mod contract_tests {
    mod controls;
    mod relay;

    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use crate::browser::adapter::{
        adapter_capability, AdapterCapability, BrowserCommand, BrowserFrame, BrowserPlatform,
        ADAPTER_PROTOCOL_MAJOR,
    };
    use ghostlight_bridge::framing::{read_native, write_native};

    use super::{BrowserError, BrowserPort, HeartbeatSettings, RelayBrowserPort};

    fn capability(name: &str) -> AdapterCapability {
        AdapterCapability {
            name: name.into(),
            revision: 1,
        }
    }

    const TEST_BROWSER: &str = "browser_test";

    fn announce_adapter(stream: &mut TcpStream, capabilities: Vec<AdapterCapability>) {
        announce_browser(stream, TEST_BROWSER, false, capabilities);
    }

    fn announce_browser(
        stream: &mut TcpStream,
        browser_id: &str,
        attended: bool,
        capabilities: Vec<AdapterCapability>,
    ) {
        write_native(
            stream,
            &BrowserFrame::Hello {
                major: ADAPTER_PROTOCOL_MAJOR,
                adapter_version: "1.0.0".into(),
                browser_id: browser_id.into(),
                adapter_epoch: format!("adapter_{}", browser_id.replace("browser_", "")),
                browser_name: None,
                platform: Some(BrowserPlatform::Chromium),
                attended,
                capabilities,
            },
        )
        .unwrap();
        assert!(matches!(
            read_native::<BrowserFrame>(stream).unwrap(),
            Some(BrowserFrame::HelloAccepted { .. })
        ));
    }

    fn short_heartbeat() -> HeartbeatSettings {
        HeartbeatSettings {
            interval: Duration::from_millis(10),
            timeout: Duration::from_millis(50),
            verdict: Duration::from_millis(50),
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Mutex, MutexGuard};
    use std::time::Instant;

    use crate::browser::adapter::{BrowserOutcome, BrowserPlatform, RuntimeControlState, Work};

    use super::{BrowserError, BrowserPort, BrowserSummary};

    /// The browser a fake stands in for when a test does not care which one it is.
    pub const FAKE_BROWSER: &str = "browser_fake";

    /// Deterministic browser port for executor contract tests.
    #[derive(Debug)]
    pub struct FakeBrowser {
        documents: Mutex<
            std::collections::HashMap<u64, crate::browser::adapter::documents::DocumentInventory>,
        >,
        scopes: Mutex<Vec<crate::browser::adapter::documents::DocumentScope>>,
        calls: Mutex<Vec<Work>>,
        routed: Mutex<Vec<String>>,
        outcomes: Mutex<VecDeque<Result<BrowserOutcome, BrowserError>>>,
        control_states: Mutex<Vec<RuntimeControlState>>,
        connected: Mutex<Vec<BrowserSummary>>,
        returning: Mutex<bool>,
    }

    impl Default for FakeBrowser {
        fn default() -> Self {
            Self {
                documents: Mutex::default(),
                scopes: Mutex::default(),
                calls: Mutex::default(),
                routed: Mutex::default(),
                outcomes: Mutex::default(),
                control_states: Mutex::default(),
                connected: Mutex::new(vec![summary(FAKE_BROWSER, true)]),
                returning: Mutex::new(false),
            }
        }
    }

    /// One connected browser, as a test wants to describe it.
    pub fn summary(id: &str, attended: bool) -> BrowserSummary {
        BrowserSummary {
            id: id.into(),
            name: None,
            adapter_version: "1.0.0".into(),
            platform: BrowserPlatform::Chromium,
            attended,
        }
    }

    impl FakeBrowser {
        /// Supply explicit document evidence for a governed executor fixture.
        pub fn set_documents(
            &self,
            tab: u64,
            inventory: crate::browser::adapter::documents::DocumentInventory,
        ) {
            lock(&self.documents).insert(tab, inventory);
        }
        /// Document constraints sent to this browser, separate from physical operation calls.
        pub fn scopes(&self) -> Vec<crate::browser::adapter::documents::DocumentScope> {
            lock(&self.scopes).clone()
        }
        pub fn push(&self, outcome: Result<BrowserOutcome, BrowserError>) {
            lock(&self.outcomes).push_back(outcome);
        }
        pub fn calls(&self) -> Vec<Work> {
            lock(&self.calls).clone()
        }
        pub fn control_states(&self) -> Vec<RuntimeControlState> {
            lock(&self.control_states).clone()
        }
        /// Which browser each dispatch was routed to, in order.
        pub fn routed(&self) -> Vec<String> {
            lock(&self.routed).clone()
        }
        /// Replace the connected inventory this port reports.
        pub fn connect(&self, browsers: Vec<BrowserSummary>) {
            *lock(&self.connected) = browsers;
        }
        /// Report evidence that a browser is about to connect, as a relay does after a service
        /// start or while a connection initializes.
        pub fn set_returning(&self, returning: bool) {
            *lock(&self.returning) = returning;
        }
    }

    impl BrowserPort for FakeBrowser {
        fn call(
            &self,
            browser: &str,
            _workspace: &str,
            command: Work,
            _deadline: Instant,
            _cancelled: &AtomicBool,
        ) -> Result<BrowserOutcome, BrowserError> {
            use crate::browser::adapter::documents::{
                DocumentInventory, DocumentObservation, PhysicalDocument,
            };
            if let Work::DescribeDocuments {
                tab_id,
                locators,
                points,
                focused,
                ..
            } = &command
            {
                let mut inventory = lock(&self.documents)
                    .get(tab_id)
                    .cloned()
                    .unwrap_or_default();
                if locators.is_empty() && points.is_empty() && !focused {
                    inventory.subjects.clear();
                } else if inventory.subjects.is_empty() {
                    inventory.subjects = inventory
                        .documents
                        .first()
                        .map(|document| vec![document.id.clone()])
                        .unwrap_or_default();
                }
                return Ok(BrowserOutcome::Documents {
                    tab_id: *tab_id,
                    inventory,
                });
            }
            if let Work::InDocuments { scope, primitive } = command {
                lock(&self.scopes).push(scope.clone());
                return self
                    .call(browser, _workspace, *primitive, _deadline, _cancelled)
                    .map(|result| BrowserOutcome::InDocuments {
                        observation: DocumentObservation {
                            visited: scope.allowed,
                            ..DocumentObservation::default()
                        },
                        result: Box::new(result),
                    });
            }
            if matches!(&command, Work::Observe { condition, .. } if condition == "visual_settle") {
                let tab_id = match &command {
                    Work::Observe { tab_id, .. } => *tab_id,
                    _ => unreachable!(),
                };
                lock(&self.routed).push(browser.into());
                lock(&self.calls).push(command);
                if let Some(outcome) = lock(&self.outcomes).pop_front() {
                    return outcome;
                }
                return Ok(BrowserOutcome::Observed {
                    tab_id,
                    satisfied: true,
                    elapsed_ms: 0,
                    readiness: crate::browser::adapter::BrowserReadiness::Complete,
                });
            }
            lock(&self.routed).push(browser.into());
            lock(&self.calls).push(command);
            let result = lock(&self.outcomes)
                .pop_front()
                .unwrap_or_else(|| Err(BrowserError::Primitive("no fake outcome".into())));
            if let Ok(
                BrowserOutcome::TabOpened { tab, .. }
                | BrowserOutcome::Navigated { tab, .. }
                | BrowserOutcome::Typed { tab, .. },
            ) = &result
            {
                lock(&self.documents).insert(
                    tab.tab_id,
                    DocumentInventory {
                        documents: vec![PhysicalDocument {
                            id: format!("document_{}", tab.tab_id),
                            url: tab.url.clone(),
                            parent: None,
                            supported: true,
                        }],
                        ..DocumentInventory::default()
                    },
                );
            }
            result
        }

        fn browsers(&self) -> Vec<BrowserSummary> {
            lock(&self.connected).clone()
        }

        fn browser_returning(&self) -> bool {
            *lock(&self.returning)
        }

        fn publish_control_state(&self, state: RuntimeControlState) -> Result<(), BrowserError> {
            lock(&self.control_states).push(state);
            Ok(())
        }
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
