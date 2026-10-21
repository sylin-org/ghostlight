//! The physical browser port and authenticated relay-backed adapter implementation.

pub mod adapter;
pub mod recovery;
pub mod session;
pub mod transport;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::browser::adapter::chromium::protocol4::{self, link::Line, link::Link, wire};
use crate::browser::adapter::{BrowserEvent, BrowserOutcome, RuntimeControlState, Work};
use crate::browser::transport::{SocketReader, SocketWriter, EXCHANGE_TIMEOUT};
use crate::install::last_browser;
use crate::install::native_host::browser_id_named;
use ghostlight_relay::endpoint::Connection as RelayConnection;
use ghostlight_relay::protocol::{goodbye_frame, GoodbyeReason};
use serde_json::Value;
use thiserror::Error;

// A native message inside Chromium's ordinary worker-idle window keeps the browser shore
// observable without conflating a silent browser operation with a dead adapter.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(45);
// How often a call waiting for its browser looks again.
const AVAILABILITY_POLL: Duration = Duration::from_millis(40);
// A product name is a label a person reads next to a browser they already recognize, not a
// place for an adapter to write prose.
const BROWSER_NAME_MAX_CHARS: usize = 40;
// A protocol-4 extension names the most specific brand among the browser's user-agent brands and
// leaves out the generic Chromium brand, so a browser that reports no name calls itself Chromium.
const UNBRANDED_BROWSER_NAME: &str = "Chromium";
const MAX_BROWSER_CONNECTIONS: usize = 16;
// Every browser identity an adapter mints starts with this prefix.
const BROWSER_ID_PREFIX: &str = "browser_";
// Every extension life an adapter mints starts with this prefix.
const LIFE_PREFIX: &str = "life_";
const CONTROL_DELIVERY_TIMEOUT: Duration = Duration::from_millis(500);

/// How long a call waits for a browser that is on its way back before it says what is true.
///
/// It covers one relay reconnect attempt (500 ms), the hello exchange of the new connection (the
/// 5 s exchange budget), and a margin. Callers cap it by their own invocation deadline.
pub const RETURN_GRACE: Duration = Duration::from_secs(7);

/// How long after the service publishes its endpoint a call with no browser still waits for the
/// relays that served an earlier service.
///
/// A waiting relay retries every 500 ms. This covers two retries and the hello. A relay that does
/// reconnect registers inside it. A cold start with no browser loses only this much of its
/// browser startup budget.
pub(crate) const RELAY_RETURN_WINDOW: Duration = Duration::from_millis(1_500);

/// Liveness timing for one relay port. Production uses the defaults; tests shorten them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HeartbeatSettings {
    /// Time between timer heartbeats on an otherwise quiet connection.
    pub(crate) interval: Duration,
    /// Time without any acknowledgement after which the connection is hidden from new work.
    /// Calls already waiting keep waiting for their own receipts or a closed socket.
    pub(crate) timeout: Duration,
}

impl Default for HeartbeatSettings {
    fn default() -> Self {
        Self {
            interval: HEARTBEAT_INTERVAL,
            timeout: HEARTBEAT_TIMEOUT,
        }
    }
}

/// Whether one browser can take work, as learned by waiting for it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    /// The browser is connected and answering.
    Available,
    /// The browser holds a registered connection that stopped answering liveness probes.
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
    /// Set by the port when it starts writing the command's first frame to the browser, and
    /// never cleared. Once set, the browser took part in the work whatever the outcome; while
    /// clear, nothing of the command left the service. Reaching the browser says nothing about
    /// effects: a read the browser held when its deadline ran out reached it and changed
    /// nothing.
    pub written: &'a AtomicBool,
}

/// The one physical tab `command` acts on or reads, when it names one.
///
/// A command that names no tab, or several (a listing, an open, a diagnostics clear, the
/// recording's own commands and presentation), has none.
#[must_use]
pub fn addressed_tab(command: &Work) -> Option<u64> {
    match command {
        Work::InDocuments { primitive, .. } => addressed_tab(primitive),
        Work::DescribeDocuments { tab_id, .. }
        | Work::FocusTab { tab_id }
        | Work::AdoptTab { tab_id, .. }
        | Work::Navigate { tab_id, .. }
        | Work::TraverseHistory { tab_id, .. }
        | Work::Reload { tab_id, .. }
        | Work::CloseTab { tab_id, .. }
        | Work::NavigateDiscardingBeforeUnload { tab_id, .. }
        | Work::ReadText { tab_id, .. }
        | Work::ReadDocument { tab_id, .. }
        | Work::Inspect { tab_id, .. }
        | Work::InspectTree { tab_id, .. }
        | Work::Find { tab_id, .. }
        | Work::Screenshot { tab_id, .. }
        | Work::ScreenshotRegion { tab_id, .. }
        | Work::DescribeTargets { tab_id, .. }
        | Work::QuerySemantic { tab_id, .. }
        | Work::Activate { tab_id, .. }
        | Work::ActivatePoint { tab_id, .. }
        | Work::WheelAt { tab_id, .. }
        | Work::Scroll { tab_id, .. }
        | Work::SetZoom { tab_id, .. }
        | Work::ResizeWindow { tab_id, .. }
        | Work::Hover { tab_id, .. }
        | Work::HoverPoint { tab_id, .. }
        | Work::Fill { tab_id, .. }
        | Work::TypeText { tab_id, .. }
        | Work::DescribeFocused { tab_id }
        | Work::TypeFocused { tab_id, .. }
        | Work::PressKey { tab_id, .. }
        | Work::Drag { tab_id, .. }
        | Work::DragPoints { tab_id, .. }
        | Work::UploadFiles { tab_id, .. }
        | Work::DropImageAt { tab_id, .. }
        | Work::EvaluateScript { tab_id, .. }
        | Work::Observe { tab_id, .. }
        | Work::InspectDialog { tab_id }
        | Work::HandleDialog { tab_id, .. }
        | Work::ReadDiagnostics { tab_id, .. }
        | Work::StartRecording { tab_id } => Some(*tab_id),
        Work::ListTabs
        | Work::OpenTab { .. }
        | Work::ClearDiagnostics { .. }
        | Work::StatusRecording
        | Work::StopRecording
        | Work::ExportRecording { .. }
        | Work::DiscardRecording
        | Work::Present { .. } => None,
    }
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
    /// The port sets `dispatch.written` when it starts writing the command to the browser.
    ///
    /// This default has no write boundary of its own to observe, so it counts the command as
    /// written once runtime control admits it and it hands the command to [`Self::call`].
    fn call_guarded(
        &self,
        browser: &str,
        workspace: &str,
        command: Work,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        (dispatch.admit)()?;
        dispatch.written.store(true, Ordering::SeqCst);
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

    /// The native-host id (`chrome`, `edge`, ...) of the installed browser that last connected,
    /// as this port remembers it across service starts, or `None` when it remembers none. The
    /// default remembers nothing.
    fn last_connected_browser(&self) -> Option<String> {
        None
    }

    /// The native-host id (`chrome`, `edge`, ...) of the installed browser that `browser`, a
    /// browser identity, reported when it connected to this service, or `None` when it never
    /// connected or its reported name maps to no single supported browser. Kept for the
    /// service's life, so it answers for a browser that has since closed. The default knows none.
    fn installed_browser_of(&self, _browser: &str) -> Option<String> {
        None
    }

    /// How many connections reporting the installed browser `installed` (a native-host id) this
    /// port has registered in its life. Two readings tell recovery whether a browser it started
    /// has connected in between. The default counts none.
    fn arrivals_of(&self, _installed: &str) -> u64 {
        0
    }

    /// The extension life the registered connection of `browser` reported in its hello, or
    /// `None` when no connection of that browser is registered or its hello named no life.
    ///
    /// One life names one browser run, in which tab ids are never reused, so a pending binding
    /// remembered in this life is still the tab with its id (ADR-0203 Decision 1). A life read
    /// after a listing answered names the run that listing came from or a later one, never an
    /// earlier one, because a life never comes back once the browser has left it. The default
    /// knows none, so every pending binding is confirmed by its URL.
    fn life_of(&self, _browser: &str) -> Option<String> {
        None
    }

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

    /// Wait like [`await_available`](Self::await_available), but stop as soon as `browser` is
    /// absent and no browser is evidently returning ([`browser_returning`](Self::browser_returning)).
    ///
    /// A session that Ghostlight can start its browser for waits only while that browser may be
    /// on its way back, so a browser the person closed keeps the call's budget for its start.
    fn await_returning(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &AtomicBool,
    ) -> Availability {
        loop {
            let slice = (Instant::now() + AVAILABILITY_POLL).min(until);
            let availability = self.await_available(browser, slice, cancelled);
            if availability == Availability::Available
                || cancelled.load(Ordering::SeqCst)
                || Instant::now() >= until
                || (availability == Availability::Absent && !self.browser_returning())
            {
                return availability;
            }
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

/// One registered browser connection.
#[derive(Debug)]
struct Connection {
    id: String,
    /// What each command carried over the connection uses: its writer, its awaited receipts and
    /// its recordings.
    line: Line,
    adapter_version: String,
    browser_id: String,
    browser_name: Option<String>,
    /// The extension life the connection's hello reported, if it reported one.
    life: Option<String>,
    liveness: Arc<Liveness>,
}

impl Connection {
    /// Whether this connection is answering liveness probes now.
    fn available(&self, now: Instant, timeout: Duration) -> bool {
        self.liveness.state().is_available(now, timeout)
    }
}

/// What one browser's registered connection can do right now.
enum Reachability {
    /// No connection is registered for the browser.
    Absent,
    /// The connection's liveness has lapsed.
    Silent,
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
    /// Whether this is the most recently attended connected browser.
    pub attended: bool,
}

/// The name a connected browser goes by when its adapter reports no product name: the browser
/// family every adapter this service serves belongs to.
const UNNAMED_BROWSER: &str = "Chromium";

impl BrowserSummary {
    /// The name a person and the history know this browser by: its reported product name, or
    /// its family when it reports none.
    #[must_use]
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| UNNAMED_BROWSER.into())
    }
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
    /// When the last connection ended without a successor.
    ///
    /// A browser that left moments ago, such as an extension worker restarting its native port,
    /// is likely to come back. One that left longer ago than the return grace is evidence of
    /// nothing.
    departed: Option<Instant>,
    /// The installed browser (`chrome`, `edge`, ...) each browser identity reported when it
    /// connected, kept for the service's life in memory only, so a session whose browser closed
    /// can start that same browser again.
    installed: HashMap<String, &'static str>,
    /// How many connections reported each installed browser, for the service's life.
    arrivals: HashMap<&'static str, u64>,
}

impl AdapterRegistry {
    /// Remove exactly the named connection, never one that has since replaced it, and remember
    /// when it left.
    fn remove_exact(&mut self, browser: &str, connection_id: &str) -> Option<Connection> {
        if self
            .connections
            .get(browser)
            .is_none_or(|connection| connection.id != connection_id)
        {
            return None;
        }
        let removed = self.connections.remove(browser)?;
        self.departed = Some(Instant::now());
        Some(removed)
    }

    /// Whether a browser is evidently on its way back at `now` (ADR-0196 Decision 12).
    ///
    /// Each piece of evidence lapses on its own: a registered connection that is silent lasts as
    /// long as the connection, a departure lasts the return grace, and a freshly opened port
    /// lasts the relay return window.
    fn returning(&self, now: Instant, opened: Instant, timeout: Duration) -> bool {
        let silent = self
            .connections
            .values()
            .any(|connection| !connection.available(now, timeout));
        let departed_recently = self
            .departed
            .is_some_and(|departed| now.saturating_duration_since(departed) < RETURN_GRACE);
        let relays_reconnecting = now.saturating_duration_since(opened) < RELAY_RETURN_WINDOW;
        silent || departed_recently || relays_reconnecting
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
        match self.connections.get(browser) {
            None => Reachability::Absent,
            Some(connection) if !connection.available(now, timeout) => Reachability::Silent,
            Some(_) => Reachability::Available,
        }
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
    stale: bool,
}

impl ConnectionLiveness {
    fn new(now: Instant) -> Self {
        Self {
            last_acknowledged_at: now,
            next_sequence: 0,
            stale: false,
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
        std::mem::replace(&mut self.stale, false)
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
    /// not answering. Returns whether this made the adapter stale.
    fn expire(&mut self, now: Instant, timeout: Duration) -> bool {
        now.saturating_duration_since(self.last_acknowledged_at) >= timeout && self.mark_stale()
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

    /// Begin one liveness probe and return the heartbeat frame that carries it.
    fn probe(&self) -> wire::Outbound {
        wire::Outbound::Heartbeat {
            sequence: self.state().begin_probe(),
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

    /// Mark the adapter as not answering, reporting the transition.
    fn mark_stale(&self) {
        let changed = self.state().mark_stale();
        self.report_stale(changed);
    }

    /// Apply the heartbeat timeout, reporting a stale transition.
    fn expire(&self, now: Instant, timeout: Duration) {
        let changed = self.state().expire(now, timeout);
        self.report_stale(changed);
    }

    fn report_stale(&self, changed: bool) {
        if changed {
            if let Some(observer) = &self.lifecycle {
                observer.adapter_unanswered(&self.browser_id);
            }
        }
    }
}

/// Observes adapter connection lifecycle at the physical port boundary. The port calls this
/// where connections are registered, replaced, and lost, and where an attached adapter stops or
/// resumes answering; observers log or publish.
pub trait AdapterLifecycleObserver: Send + Sync + 'static {
    /// A browser's connection was accepted and can take work.
    fn adapter_attached(&self, browser_id: &str, replaced: bool);

    /// A browser's connection ended.
    fn adapter_detached(&self, browser_id: &str);

    /// A browser's connection is still attached but stopped acknowledging liveness probes.
    fn adapter_unanswered(&self, _browser_id: &str) {}

    /// A browser's connection that had stopped answering acknowledged a probe again.
    fn adapter_answering(&self, _browser_id: &str) {}
}

/// Relay-backed implementation of the physical browser port.
pub struct RelayBrowserPort {
    service_epoch: String,
    adapters: Arc<Mutex<AdapterRegistry>>,
    event_sink: Mutex<Option<Arc<dyn BrowserEventSink>>>,
    control_state: Mutex<RuntimeControlState>,
    lifecycle: Mutex<Option<Arc<dyn AdapterLifecycleObserver>>>,
    /// The page assets every protocol-4 connection receives in `hello_accepted`.
    assets: Arc<wire::Assets>,
    /// The last-browser record this port writes when a connection is accepted and reads for
    /// recovery. Unset, the port remembers nothing.
    last_browser: Mutex<Option<PathBuf>>,
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
    /// Construct a disconnected relay port for one restart-local service epoch, shipping
    /// `assets` to every protocol-4 connection.
    #[must_use]
    pub fn new(service_epoch: String, assets: Arc<wire::Assets>) -> Self {
        Self::with_settings(service_epoch, assets, HeartbeatSettings::default())
    }

    /// Construct a relay port with shortened liveness timing for tests.
    #[cfg(test)]
    pub(crate) fn with_heartbeat_settings(
        service_epoch: String,
        assets: Arc<wire::Assets>,
        heartbeat: HeartbeatSettings,
    ) -> Self {
        debug_assert!(heartbeat.interval < heartbeat.timeout);
        Self::with_settings(service_epoch, assets, heartbeat)
    }

    fn with_settings(
        service_epoch: String,
        assets: Arc<wire::Assets>,
        heartbeat: HeartbeatSettings,
    ) -> Self {
        Self {
            service_epoch,
            adapters: Arc::new(Mutex::new(AdapterRegistry::default())),
            event_sink: Mutex::new(None),
            control_state: Mutex::new(RuntimeControlState::Active),
            lifecycle: Mutex::new(None),
            assets,
            last_browser: Mutex::new(None),
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

    /// Remember in `record` which installed browser last connected, and recall it from there.
    ///
    /// Each accepted connection whose reported name maps to exactly one supported browser makes
    /// the record name that browser's native-host id ([`last_browser`]). A connection that maps
    /// to none leaves the record as it was.
    pub fn remember_browsers_in(&self, record: PathBuf) {
        *lock(&self.last_browser) = Some(record);
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
    /// and it may answer again. That is a different fact from being connected.
    #[cfg(test)]
    pub(crate) fn is_registered(&self, browser: &str) -> bool {
        lock(&self.adapters).connections.contains_key(browser)
    }

    /// Tell every attached browser stream that the service is closing it on purpose.
    ///
    /// The goodbye is the relay's fixed bytes, not an adapter frame: the browser edge reads it
    /// and never passes it to the extension. A stream that cannot take it is closed.
    pub fn say_goodbye(&self, reason: GoodbyeReason) {
        let writers: Vec<Arc<SocketWriter>> = lock(&self.adapters)
            .connections
            .values()
            .map(|connection| Arc::clone(&connection.line.writer))
            .collect();
        let farewell = goodbye_frame(reason);
        for writer in writers {
            let _ = writer.raw(&farewell);
        }
    }

    /// Negotiate and attach one already-authenticated browser-relay stream.
    pub fn attach(&self, stream: RelayConnection) -> Result<(), BrowserError> {
        let reader = SocketReader::new(
            stream
                .try_clone()
                .map_err(|error| BrowserError::Protocol(error.to_string()))?,
        );
        self.attach_reader(stream, reader, Instant::now() + EXCHANGE_TIMEOUT)
    }

    /// Continue authentication using the reader that may already hold coalesced adapter bytes.
    ///
    /// The first frame must be a protocol-4 hello. Anything else is told to update and closed.
    pub(crate) fn attach_reader(
        &self,
        stream: RelayConnection,
        mut reader: SocketReader,
        opening: Instant,
    ) -> Result<(), BrowserError> {
        let Some(hello) = reader
            .native::<Value>(Some(opening))
            .map_err(|error| BrowserError::Protocol(error.to_string()))?
        else {
            return Err(BrowserError::Authentication);
        };
        protocol4::connection::attach(self, stream, reader, hello)
    }

    /// Register one accepted connection under its browser identity, retiring the connection it
    /// replaces, and tell the lifecycle observer.
    ///
    /// Retiring a connection ends its stream, so the event sink hears that browser disconnect,
    /// with the life the retired connection reported, before the new connection is registered.
    /// Its own end then goes unreported, because it is no longer the current connection, and a
    /// browser whose old stream's end was never seen may have restarted all the same (ADR-0203
    /// Decision 1).
    ///
    /// # Errors
    ///
    /// Refuses a new browser once the port holds its connection capacity.
    fn register(&self, connection: Connection, attended: bool) -> Result<(), BrowserError> {
        let browser_id = connection.browser_id.clone();
        let installed = connection
            .browser_name
            .as_deref()
            .and_then(browser_id_named);
        let replaced = lock(&self.adapters)
            .connections
            .get(&browser_id)
            .map(|previous| previous.life.clone());
        if let Some(life) = replaced {
            if let Some(sink) = lock(&self.event_sink).clone() {
                sink.on_event(&browser_id, BrowserEvent::Disconnected { life });
            }
        }
        let lifecycle = lock(&self.lifecycle).clone();
        {
            let mut adapters = lock(&self.adapters);
            if !adapters.connections.contains_key(&browser_id)
                && adapters.connections.len() >= MAX_BROWSER_CONNECTIONS
            {
                connection.line.writer.close();
                return Err(BrowserError::Protocol(
                    "browser connection capacity reached".into(),
                ));
            }
            if let Some(installed) = installed {
                adapters.installed.insert(browser_id.clone(), installed);
                *adapters.arrivals.entry(installed).or_default() += 1;
            }
            let previous = adapters.connections.insert(browser_id.clone(), connection);
            if let Some(previous) = &previous {
                retire(previous);
            }
            if attended {
                adapters.attend(&browser_id);
            }
            if let Some(observer) = lifecycle.as_ref() {
                observer.adapter_attached(&browser_id, previous.is_some());
            }
        }
        if let Some(installed) = installed {
            self.remember(installed);
        }
        Ok(())
    }

    /// Make the last-browser record, when this port keeps one, name `installed`. The record only
    /// lets recovery start the right browser later, so a home that cannot take the write keeps
    /// the connection all the same.
    fn remember(&self, installed: &str) {
        let record = lock(&self.last_browser).clone();
        if let Some(record) = record {
            let _ = last_browser::record(&record, installed);
        }
    }

    /// The notification targets a new connection's reader reports to.
    fn notifications(&self) -> AdapterNotifications {
        AdapterNotifications {
            sink: lock(&self.event_sink).clone(),
            lifecycle: lock(&self.lifecycle).clone(),
        }
    }

    /// Start the timer side of one connection's liveness.
    ///
    /// # Errors
    ///
    /// Detaches the connection and reports the failure when the thread cannot start.
    fn spawn_heartbeat(
        &self,
        tag: &ConnectionTag,
        writer: Arc<SocketWriter>,
        liveness: Arc<Liveness>,
    ) -> Result<(), BrowserError> {
        let adapters = Arc::clone(&self.adapters);
        let settings = self.heartbeat;
        let heartbeat_tag = tag.clone();
        if let Err(error) = thread::Builder::new()
            .name("ghostlight-browser-heartbeat".into())
            .spawn(move || {
                heartbeat_adapter(&writer, &liveness, &adapters, &heartbeat_tag, settings);
            })
        {
            // A connection with nobody probing it would go stale and stay registered, so undo
            // the registration rather than leave a connection no thread watches.
            self.detach_registered(&tag.browser_id, &tag.connection_id);
            return Err(BrowserError::Protocol(error.to_string()));
        }
        Ok(())
    }

    /// Remove a registered connection, but only the exact one named -- never a connection that
    /// has since replaced it. A concurrent `attach` for the same `browser_id` may already have
    /// replaced it with a newer, healthy connection, and that one must be left alone.
    fn detach_registered(&self, browser_id: &str, connection_id: &str) {
        let removed = lock(&self.adapters).remove_exact(browser_id, connection_id);
        if let Some(connection) = removed {
            retire(&connection);
            if let Some(observer) = lock(&self.lifecycle).clone().as_ref() {
                observer.adapter_detached(browser_id);
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
        if dispatch.cancelled.load(Ordering::SeqCst) {
            return Err(BrowserError::CancelledBeforeDispatch);
        }
        if Instant::now() >= dispatch.deadline {
            return Err(BrowserError::DeadlineBeforeDispatch);
        }
        let line = {
            let adapters = lock(&self.adapters);
            let Some(connection) = adapters.connections.get(browser) else {
                return Err(BrowserError::DisconnectedBeforeDispatch);
            };
            if !connection.available(Instant::now(), self.heartbeat.timeout) {
                return Err(BrowserError::NotAnsweringBeforeDispatch);
            }
            connection.line.clone()
        };
        protocol4::run(command, &mut Link::new(line, workspace, dispatch))
    }

    /// Wait for one browser's connection to be registered and answering.
    fn await_registered(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &AtomicBool,
    ) -> Availability {
        loop {
            let reachability =
                lock(&self.adapters).reachability(browser, Instant::now(), self.heartbeat.timeout);
            let availability = match reachability {
                Reachability::Available => return Availability::Available,
                Reachability::Absent => Availability::Absent,
                Reachability::Silent => Availability::NotAnswering,
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
}

/// Tell an extension that does not speak adapter protocol 4 to update, then close its socket.
/// Best effort: the extension may already be gone.
fn refuse_incompatible(stream: RelayConnection) {
    let Ok(writer) = SocketWriter::new(stream) else {
        return;
    };
    let _ = writer.native_until(
        &wire::Outbound::Error {
            code: wire::error_code::ADAPTER_INCOMPATIBLE.into(),
            message: crate::language::adapter::ADAPTER_INCOMPATIBLE.into(),
        },
        Instant::now() + CONTROL_DELIVERY_TIMEOUT,
    );
    writer.close();
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
                written: &AtomicBool::new(false),
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

    fn last_connected_browser(&self) -> Option<String> {
        let record = lock(&self.last_browser).clone()?;
        last_browser::read(&record)
    }

    fn installed_browser_of(&self, browser: &str) -> Option<String> {
        lock(&self.adapters)
            .installed
            .get(browser)
            .map(|installed| (*installed).to_owned())
    }

    fn life_of(&self, browser: &str) -> Option<String> {
        lock(&self.adapters)
            .connections
            .get(browser)
            .and_then(|connection| connection.life.clone())
    }

    fn arrivals_of(&self, installed: &str) -> u64 {
        lock(&self.adapters)
            .arrivals
            .get(installed)
            .copied()
            .unwrap_or(0)
    }

    /// A silent connection is waited on until it answers a timer heartbeat again.
    fn await_available(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &AtomicBool,
    ) -> Availability {
        self.await_registered(browser, until, cancelled)
    }

    /// The evidence is any of: a registered connection that is silent; a connection that ended
    /// less than the return grace ago, as when an extension worker restarts its native port; or
    /// an endpoint published less than the relay return window ago, while relays that served an
    /// earlier service reconnect. Attention a browser reported at some point is not evidence: it
    /// never lapses, and a browser closed long ago would then spend the whole wait that browser
    /// startup needs.
    fn browser_returning(&self) -> bool {
        let opened = *lock(&self.opened);
        lock(&self.adapters).returning(Instant::now(), opened, self.heartbeat.timeout)
    }

    /// Runtime control is a property of Ghostlight, not of one browser, so every connected
    /// adapter learns the new state. One unreachable browser does not hide the state from the
    /// rest.
    fn publish_control_state(&self, state: RuntimeControlState) -> Result<(), BrowserError> {
        *lock(&self.control_state) = state;
        let frame = wire::Outbound::ControlState {
            state: state.into(),
        };
        let writers: Vec<_> = lock(&self.adapters)
            .connections
            .values()
            .map(|connection| Arc::clone(&connection.line.writer))
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

/// The notification targets one adapter reader reports to: the typed event sink and the
/// lifecycle observer.
struct AdapterNotifications {
    sink: Option<Arc<dyn BrowserEventSink>>,
    lifecycle: Option<Arc<dyn AdapterLifecycleObserver>>,
}

/// Forget one connection whose stream ended, and tell the sink and the lifecycle observer.
///
/// A replaced connection must not evict the connection that replaced it. Its attention place
/// survives, so the same browser reconnecting resumes where it was rather than starting behind
/// every other browser.
fn end_connection(
    adapters: &Mutex<AdapterRegistry>,
    tag: &ConnectionTag,
    notifications: &AdapterNotifications,
) {
    let browser_id = &tag.browser_id;
    let removed = lock(adapters).remove_exact(browser_id, &tag.connection_id);
    if let Some(removed) = removed {
        if let Some(sink) = &notifications.sink {
            sink.on_event(
                browser_id,
                BrowserEvent::Disconnected { life: removed.life },
            );
        }
        if let Some(observer) = &notifications.lifecycle {
            observer.adapter_detached(browser_id);
        }
    }
}

/// The timer side of liveness. A full heartbeat timeout without any acknowledgement hides the
/// connection from new work; only a closed socket fails the calls already waiting.
fn heartbeat_adapter(
    writer: &SocketWriter,
    liveness: &Liveness,
    adapters: &Mutex<AdapterRegistry>,
    tag: &ConnectionTag,
    settings: HeartbeatSettings,
) {
    loop {
        if writer.wait_closed(settings.interval) {
            return;
        }
        if !is_current(adapters, tag) {
            return;
        }
        liveness.expire(Instant::now(), settings.timeout);
        if writer.native(&liveness.probe()).is_err() {
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

/// End a connection that has left the registry: a fresher connection from the same browser
/// replaced it, or it was detached.
///
/// Failing its waiting calls is not enough. An abandoned but still-open socket keeps its relay
/// process alive and keeps the browser's stale native port alive with it, so nothing on either
/// shore ever learns the connection is finished. Closing the stream is what makes the duplicate
/// collapse: the relay reads end-of-stream and exits, and the browser observes its port
/// disconnect.
fn retire(previous: &Connection) {
    previous
        .line
        .receipts
        .fail_all(&BrowserError::DisconnectedAfterDispatch);
    previous.line.writer.close();
}

/// Validate the bounded product name an adapter reports for itself. A browser that reports none
/// is Chromium ([`UNBRANDED_BROWSER_NAME`]), so its connection is named and remembered like any
/// other browser's.
fn validated_browser_name(name: Option<String>) -> Result<Option<String>, BrowserError> {
    let Some(name) = name else {
        return Ok(Some(UNBRANDED_BROWSER_NAME.to_owned()));
    };
    let acceptable = !name.trim().is_empty()
        && name.chars().count() <= BROWSER_NAME_MAX_CHARS
        && !name.chars().any(char::is_control);
    if !acceptable {
        return Err(BrowserError::Authentication);
    }
    Ok(Some(name))
}

fn valid_opaque_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
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
    /// The page refused the call's control before acting on it: it is hidden, disabled,
    /// read-only or takes no text. Raised before any effect.
    #[error("the page refused the target: {}", .0.as_str())]
    PageRefused(crate::language::page_refusal::PageRefusal),
    /// The page holds no control for a locator, and says why. Raised before any effect.
    #[error("stale browser locator: {}", reason.word())]
    StaleLocator {
        /// Why the locator names nothing.
        reason: crate::workspace::StaleReason,
        /// What the page found when it looked again for the control, when it did. Only a
        /// control the page found removed is looked for.
        lookup: Option<crate::browser::adapter::StaleLookup>,
    },
    /// The page refused a fill before any edit because fields cannot take their values.
    #[error("form fields cannot take their values")]
    FieldsRejected {
        /// Every refused field, by its position in the call, with what would succeed.
        problems: Vec<crate::language::field_problem::FieldProblem>,
    },
    /// A fill was applied, but the page changed some fields back before their values settled.
    #[error("the page changed filled fields back")]
    FieldsReverted {
        /// How many of the call's fields held their values.
        set: usize,
        /// Every field the page changed, by its position in the call, with what it holds now.
        problems: Vec<crate::language::field_problem::FieldProblem>,
    },
    /// A JavaScript dialog blocks the page, so the renderer cannot take the work.
    #[error("a JavaScript dialog blocks the page")]
    DialogOpen,
    /// Back or forward found no entry that way in the tab's history, so nothing was sent.
    #[error("the tab has no history entry that way")]
    NoHistoryEntry,
    /// The tab is not the one its window shows. Chromium draws only that one, so a capture of
    /// this tab would wait for a frame that never comes. Nothing was captured.
    #[error("the tab is not showing in its window")]
    TabNotShowing,
    /// The page's view moved on from the screenshot the point was chosen on, and nothing was sent.
    #[error("the view moved since the screenshot was taken")]
    StaleView,
    /// The tab the command names is no longer one its workspace controls: after a gap in what
    /// the service knew of the browser, the browser did not confirm it (ADR-0203 Decision 1).
    /// Nothing was sent.
    #[error("the tab is no longer one the workspace controls")]
    StaleTab,
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
    /// The extension does not speak the adapter protocol this service serves.
    #[error("browser adapter protocol {offered} is incompatible with required {required}")]
    Incompatible {
        /// Protocol the extension's first frame named, or zero when it named none.
        offered: u16,
        /// Protocol this service serves.
        required: u16,
    },
    /// The extension does not know a primitive this command needs: it is older than the service.
    #[error("the browser extension does not support the {primitive} primitive")]
    Unsupported {
        /// The primitive the extension answered as unsupported.
        primitive: String,
    },
}

impl BrowserError {
    /// A stale locator for a reason, with nothing found by looking again.
    #[must_use]
    pub const fn stale(reason: crate::workspace::StaleReason) -> Self {
        Self::StaleLocator {
            reason,
            lookup: None,
        }
    }

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
mod contract_tests;

#[cfg(test)]
pub(crate) mod testing {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, MutexGuard};
    use std::time::Instant;

    use crate::browser::adapter::{BrowserOutcome, RuntimeControlState, Work};

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
        outcomes: Mutex<VecDeque<Queued>>,
        control_states: Mutex<Vec<RuntimeControlState>>,
        connected: Mutex<Vec<BrowserSummary>>,
        returning: Mutex<bool>,
        last_connected: Mutex<Option<String>>,
        installed: Mutex<std::collections::HashMap<String, String>>,
        arrivals: Mutex<std::collections::HashMap<String, u64>>,
        lives: Mutex<std::collections::HashMap<String, String>>,
    }

    /// One answer the fake will give, and whether it wrote the command to the browser first.
    #[derive(Debug)]
    struct Queued {
        outcome: Result<BrowserOutcome, BrowserError>,
        written: bool,
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
                last_connected: Mutex::new(None),
                installed: Mutex::default(),
                arrivals: Mutex::default(),
                lives: Mutex::default(),
            }
        }
    }

    /// One connected browser, as a test wants to describe it.
    pub fn summary(id: &str, attended: bool) -> BrowserSummary {
        BrowserSummary {
            id: id.into(),
            name: None,
            adapter_version: "1.0.0".into(),
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
        /// Queue the next answer of a command the fake writes to the browser, whatever the
        /// answer says.
        pub fn push(&self, outcome: Result<BrowserOutcome, BrowserError>) {
            lock(&self.outcomes).push_back(Queued {
                outcome,
                written: true,
            });
        }
        /// Queue the next answer of a command the fake fails before writing any of it, as a port
        /// does when the connection is gone before its first frame.
        pub fn push_unwritten(&self, outcome: Result<BrowserOutcome, BrowserError>) {
            lock(&self.outcomes).push_back(Queued {
                outcome,
                written: false,
            });
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
        /// Replace the connected inventory this port reports. Each listed browser that was
        /// reported as an installed browser counts as one arrival of that installed browser.
        pub fn connect(&self, browsers: Vec<BrowserSummary>) {
            {
                let installed = lock(&self.installed);
                let mut arrivals = lock(&self.arrivals);
                for browser in &browsers {
                    if let Some(installed) = installed.get(&browser.id) {
                        *arrivals.entry(installed.clone()).or_default() += 1;
                    }
                }
            }
            *lock(&self.connected) = browsers;
        }
        /// Report evidence that a browser is about to connect, as a relay does after a service
        /// start or while a connection initializes.
        pub fn set_returning(&self, returning: bool) {
            *lock(&self.returning) = returning;
        }
        /// Remember `browser`, a native-host id, as the installed browser that last connected.
        pub fn remember_last_connected(&self, browser: &str) {
            *lock(&self.last_connected) = Some(browser.into());
        }
        /// Report that the browser identity `browser` connected as the installed browser
        /// `installed`, a native-host id.
        pub fn remember_installed(&self, browser: &str, installed: &str) {
            lock(&self.installed).insert(browser.into(), installed.into());
        }
        /// Report `life` as the extension life the connection of `browser` named in its hello,
        /// or no life for `None`.
        pub fn set_life(&self, browser: &str, life: Option<&str>) {
            let mut lives = lock(&self.lives);
            match life {
                Some(life) => lives.insert(browser.into(), life.into()),
                None => lives.remove(browser),
            };
        }
    }

    impl FakeBrowser {
        /// Answer one command, with whether the fake wrote it to the browser. Every command is
        /// written except one answered from [`Self::push_unwritten`].
        fn answer(
            &self,
            browser: &str,
            command: Work,
        ) -> (Result<BrowserOutcome, BrowserError>, bool) {
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
                return (
                    Ok(BrowserOutcome::Documents {
                        tab_id: *tab_id,
                        inventory,
                    }),
                    true,
                );
            }
            if let Work::InDocuments { scope, primitive } = command {
                lock(&self.scopes).push(scope.clone());
                let (outcome, written) = self.answer(browser, *primitive);
                return (
                    outcome.map(|result| BrowserOutcome::InDocuments {
                        observation: DocumentObservation {
                            visited: scope.allowed,
                            ..DocumentObservation::default()
                        },
                        result: Box::new(result),
                    }),
                    written,
                );
            }
            if matches!(
                &command,
                Work::Observe {
                    condition: crate::language::PageCondition::VisualSettle,
                    ..
                }
            ) {
                let tab_id = match &command {
                    Work::Observe { tab_id, .. } => *tab_id,
                    _ => unreachable!(),
                };
                lock(&self.routed).push(browser.into());
                lock(&self.calls).push(command);
                if let Some(queued) = lock(&self.outcomes).pop_front() {
                    return (queued.outcome, queued.written);
                }
                return (
                    Ok(BrowserOutcome::Observed {
                        tab_id,
                        satisfied: true,
                        elapsed_ms: 0,
                        readiness: crate::browser::adapter::BrowserReadiness::Complete,
                    }),
                    true,
                );
            }
            lock(&self.routed).push(browser.into());
            lock(&self.calls).push(command);
            let Queued {
                outcome: result,
                written,
            } = lock(&self.outcomes).pop_front().unwrap_or_else(|| Queued {
                outcome: Err(BrowserError::Primitive("no fake outcome".into())),
                written: true,
            });
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
            (result, written)
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
            self.answer(browser, command).0
        }

        fn call_guarded(
            &self,
            browser: &str,
            _workspace: &str,
            command: Work,
            dispatch: super::BrowserDispatch<'_>,
        ) -> Result<BrowserOutcome, BrowserError> {
            (dispatch.admit)()?;
            let (outcome, written) = self.answer(browser, command);
            if written {
                dispatch.written.store(true, Ordering::SeqCst);
            }
            outcome
        }

        fn browsers(&self) -> Vec<BrowserSummary> {
            lock(&self.connected).clone()
        }

        fn browser_returning(&self) -> bool {
            *lock(&self.returning)
        }

        fn last_connected_browser(&self) -> Option<String> {
            lock(&self.last_connected).clone()
        }

        fn installed_browser_of(&self, browser: &str) -> Option<String> {
            lock(&self.installed).get(browser).cloned()
        }

        fn arrivals_of(&self, installed: &str) -> u64 {
            lock(&self.arrivals).get(installed).copied().unwrap_or(0)
        }

        fn life_of(&self, browser: &str) -> Option<String> {
            lock(&self.lives).get(browser).cloned()
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
