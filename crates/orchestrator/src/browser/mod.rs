//! The browser port: the one way the service reaches a browser (ARCHITECTURE, "Dispatch physical
//! effects only through the browser port").
//!
//! It holds the port contract (the [`BrowserPort`] trait, [`BrowserDispatch`], [`BrowserSummary`],
//! [`Availability`], [`BrowserError`] and the routing in [`choose_browser`]) and its relay-backed
//! implementation, [`RelayBrowserPort`]: the registry of every connected browser by its identity
//! (ADR-0114), each connection's liveness and heartbeat, the attention order, the browser starts
//! the launch guard records, and the last-browser record recovery reads. Browsers are plural; one
//! person often runs several at once.
//!
//! The wire is not here. A browser family's adapter (`adapter/`, today Chromium's protocol 4)
//! validates a hello, writes every frame, and registers each connection it accepts through
//! `RelayBrowserPort::admit`, which hands back the `Registration` its reader reports through.
//! `recovery` starts a browser when none answers, `graphical_session` finds the display a started
//! browser needs, and `transport` is the framed socket both sides use.

pub mod adapter;
pub mod graphical_session;
pub mod recovery;
pub mod transport;

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use crate::browser::adapter::chromium::protocol4::{self, link::Line, link::Link, wire};
use crate::browser::adapter::{BrowserCommand, BrowserEvent, BrowserOutcome, RuntimeControlState};
use crate::browser::adapter::{
    PhysicalClick, PhysicalDiagnosticsRead, PhysicalDrag, PhysicalFill, PhysicalPointClick,
    PhysicalPointDrag, PhysicalPointDrop, PhysicalPointWheel, PhysicalPressKey, PhysicalScroll,
    PhysicalSemantic, PhysicalTypeFocused, PhysicalTypeText, PhysicalViewPoint, PhysicalWait,
};
use crate::browser::transport::{SocketReader, SocketWriter, EXCHANGE_TIMEOUT};
use crate::install::last_browser;
use crate::install::native_host::browser_id_named;
use crate::sync::lock;
use ghostlight_relay::endpoint::Connection as RelayConnection;
use ghostlight_relay::protocol::{goodbye_frame, GoodbyeReason};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

// A native message inside Chromium's ordinary worker-idle window keeps the browser shore
// observable without conflating a silent browser operation with a dead adapter.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(45);
// How often a call waiting for its browser looks again.
const AVAILABILITY_POLL: Duration = Duration::from_millis(40);
// A protocol-4 extension names the most specific brand among the browser's user-agent brands and
// leaves out the generic Chromium brand, so a browser that reports no name calls itself Chromium.
pub(crate) const UNBRANDED_BROWSER_NAME: &str = "Chromium";
/// The most browser connections the port holds at once; a new browser past it is refused.
const MAX_BROWSER_CONNECTIONS: usize = 16;
/// Every connection id the port mints starts with this prefix.
const CONNECTION_PREFIX: &str = "connection_";
/// How long a runtime-control frame may take to reach one browser, and a registration to answer.
const CONTROL_DELIVERY_TIMEOUT: Duration = Duration::from_millis(500);

/// How long a call waits for a browser that is on its way back before it says what is true.
///
/// It covers one relay reconnect attempt (500 ms), the hello exchange of the new connection (the
/// 5 s exchange budget), and a margin. Callers cap it by their own invocation deadline. After a
/// replacement, a call with no browser waits this long for the browsers of the service that ended
/// ([`RelayBrowserPort::replacement_published`]).
pub const RETURN_GRACE: Duration = Duration::from_secs(7);

/// How long after the service publishes its endpoint a call with no browser still waits for
/// relays that may be looking for it, when the service replaced none.
///
/// A browser relay looks for the service on its own doubling backoff, counted from when it began
/// looking: 0.5, 1.5, 3.5 and 7.5 s after, then later ([`ghostlight_relay::spawn::Backoff`]).
/// The relay of a browser that started this service registers inside the window when the service
/// publishes before that relay's second look. A cold start with no browser loses only this much
/// of its browser startup budget.
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
pub fn addressed_tab(command: &BrowserCommand) -> Option<u64> {
    match command {
        BrowserCommand::InDocuments { primitive, .. } => addressed_tab(primitive),
        BrowserCommand::DescribeDocuments { tab_id, .. }
        | BrowserCommand::FocusTab { tab_id }
        | BrowserCommand::AdoptTab { tab_id, .. }
        | BrowserCommand::Navigate { tab_id, .. }
        | BrowserCommand::TraverseHistory { tab_id, .. }
        | BrowserCommand::Reload { tab_id, .. }
        | BrowserCommand::CloseTab { tab_id, .. }
        | BrowserCommand::NavigateDiscardingBeforeUnload { tab_id, .. }
        | BrowserCommand::ReadText { tab_id, .. }
        | BrowserCommand::ReadDocument { tab_id, .. }
        | BrowserCommand::Inspect { tab_id, .. }
        | BrowserCommand::InspectTree { tab_id, .. }
        | BrowserCommand::Find { tab_id, .. }
        | BrowserCommand::Screenshot { tab_id, .. }
        | BrowserCommand::ScreenshotRegion { tab_id, .. }
        | BrowserCommand::DescribeTargets { tab_id, .. }
        | BrowserCommand::QuerySemantic(PhysicalSemantic { tab_id, .. })
        | BrowserCommand::Activate(PhysicalClick { tab_id, .. })
        | BrowserCommand::ActivatePoint(PhysicalPointClick {
            at: PhysicalViewPoint { tab_id, .. },
            ..
        })
        | BrowserCommand::WheelAt(PhysicalPointWheel {
            at: PhysicalViewPoint { tab_id, .. },
            ..
        })
        | BrowserCommand::Scroll(PhysicalScroll { tab_id, .. })
        | BrowserCommand::SetZoom { tab_id, .. }
        | BrowserCommand::ResizeWindow { tab_id, .. }
        | BrowserCommand::Hover { tab_id, .. }
        | BrowserCommand::HoverPoint(PhysicalViewPoint { tab_id, .. })
        | BrowserCommand::Fill(PhysicalFill { tab_id, .. })
        | BrowserCommand::TypeText(PhysicalTypeText { tab_id, .. })
        | BrowserCommand::DescribeFocused { tab_id }
        | BrowserCommand::TypeFocused(PhysicalTypeFocused { tab_id, .. })
        | BrowserCommand::PressKey(PhysicalPressKey { tab_id, .. })
        | BrowserCommand::Drag(PhysicalDrag { tab_id, .. })
        | BrowserCommand::DragPoints(PhysicalPointDrag { tab_id, .. })
        | BrowserCommand::UploadFiles { tab_id, .. }
        | BrowserCommand::DropImageAt(PhysicalPointDrop {
            at: PhysicalViewPoint { tab_id, .. },
            ..
        })
        | BrowserCommand::EvaluateScript { tab_id, .. }
        | BrowserCommand::Observe(PhysicalWait { tab_id, .. })
        | BrowserCommand::InspectDialog { tab_id }
        | BrowserCommand::HandleDialog { tab_id, .. }
        | BrowserCommand::ReadDiagnostics(PhysicalDiagnosticsRead { tab_id, .. })
        | BrowserCommand::StartRecording { tab_id } => Some(*tab_id),
        BrowserCommand::ListTabs
        | BrowserCommand::OpenTab { .. }
        | BrowserCommand::ClearDiagnostics { .. }
        | BrowserCommand::StatusRecording { .. }
        | BrowserCommand::StopRecording { .. }
        | BrowserCommand::ExportRecording { .. }
        | BrowserCommand::DiscardRecording { .. }
        | BrowserCommand::Present { .. } => None,
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
        command: BrowserCommand,
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
        command: BrowserCommand,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        (dispatch.admit)()?;
        dispatch.written.store(true, Ordering::SeqCst);
        self.call(browser, command, dispatch.deadline, dispatch.cancelled)
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

    /// Note that Ghostlight is about to start the installed browser `installed` (a native-host
    /// id) at `now`, or learn that it must not: a start of it noted less than `window` ago has seen
    /// no arrival of that browser since. Returns whether this start was noted. Every starter notes
    /// its start here, beside the arrivals it is measured against, so no two starters start one
    /// browser twice (ADR-0203 C3c). The default notes every start.
    fn note_start(&self, _installed: &str, _now: Instant, _window: Duration) -> bool {
        true
    }

    /// Forget the start of `installed` noted last, after it began no process.
    fn forget_start(&self, _installed: &str) {}

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
        poll_until(until, cancelled, || {
            if self
                .browsers()
                .iter()
                .any(|connected| connected.id == browser)
            {
                ControlFlow::Break(Availability::Available)
            } else {
                ControlFlow::Continue(Availability::Absent)
            }
        })
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
                || (availability == Availability::Absent && !self.returning_of(browser))
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
        poll_until(until, cancelled, || {
            let connected = self.browsers();
            if connected.is_empty() && self.browser_returning() {
                ControlFlow::Continue(connected)
            } else {
                ControlFlow::Break(connected)
            }
        })
    }

    /// Whether there is evidence, right now, that a browser is about to connect. That evidence
    /// is what earns a call with no browser a bounded wait instead of an immediate recovery,
    /// and it must lapse with time: a browser that was merely seen at some point is not
    /// returning.
    fn browser_returning(&self) -> bool {
        false
    }

    /// Whether `browser`'s own installed browser is evidently on its way back: one of its
    /// connections is silent, one left within the return grace, or an earlier service's relays are
    /// still expected. Another browser's evidence never holds a pinned wait (C3b), while another
    /// profile of the same installed browser does. A browser whose installed kind is unknown
    /// reads every browser's evidence.
    fn returning_of(&self, browser: &str) -> bool {
        let _ = browser;
        self.browser_returning()
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
    browser_name: String,
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

/// One connected browser as the orchestrator, the workbench, and the model see it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserSummary {
    /// Persistent opaque browser identity, minted by the adapter and stable across reconnects.
    ///
    /// This doubles as the model-facing handle. It is already opaque and content-free, so a
    /// second mapping table would add a lookup without adding a guarantee.
    pub id: String,
    /// The product name a person and the history know this browser by: the bounded name its
    /// adapter reports, or Chromium ([`UNBRANDED_BROWSER_NAME`]) when it reports none.
    pub name: String,
    /// Adapter version currently serving this browser.
    pub adapter_version: String,
    /// Whether this is the most recently attended connected browser.
    pub attended: bool,
}

/// Every connected browser, and the reported order in which they were last attended.
///
/// Browsers are plural. One person routinely runs Chrome and Edge at once, or two profiles of
/// one browser, and each is a different user context rather than a redundant server.
/// One browser start Ghostlight made, as the launch guard holds it.
#[derive(Clone, Copy, Debug)]
struct PendingStart {
    /// When the start was made, by the starter's clock.
    at: Instant,
    /// The port's count of arrivals of that installed browser when the start was made; a higher
    /// count means a connection of it has arrived since.
    arrivals: u64,
}

/// The launch guard: the latest start of each installed browser, by native-host id.
#[derive(Debug, Default)]
pub(crate) struct LaunchGuard {
    starts: HashMap<String, PendingStart>,
}

impl LaunchGuard {
    /// [`BrowserPort::note_start`] against `arrivals`, the port's count of `installed` now.
    pub(crate) fn note(
        &mut self,
        installed: &str,
        arrivals: u64,
        now: Instant,
        window: Duration,
    ) -> bool {
        let pending = self.starts.get(installed).is_some_and(|start| {
            start.arrivals == arrivals && now.saturating_duration_since(start.at) < window
        });
        if !pending {
            self.starts
                .insert(installed.into(), PendingStart { at: now, arrivals });
        }
        !pending
    }

    /// [`BrowserPort::forget_start`].
    pub(crate) fn forget(&mut self, installed: &str) {
        self.starts.remove(installed);
    }
}

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
    /// When the last connection of each installed browser ended without a successor, keyed by the
    /// installed browser it reported (`None` for one that reported no single supported browser).
    ///
    /// A browser that left moments ago, such as an extension worker restarting its native port,
    /// is likely to come back. One that left longer ago than the return grace is evidence of
    /// nothing.
    departed: HashMap<Option<&'static str>, Instant>,
    /// The installed browser (`chrome`, `edge`, ...) each browser identity reported when it
    /// connected, kept for the service's life in memory only, so a session whose browser closed
    /// can start that same browser again.
    installed: HashMap<String, &'static str>,
    /// How many connections reported each installed browser, for the service's life.
    arrivals: HashMap<&'static str, u64>,
    /// The starts Ghostlight made of each installed browser, measured against `arrivals`.
    launches: LaunchGuard,
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
        let installed = self.installed.get(browser).copied();
        self.departed.insert(installed, Instant::now());
        Some(removed)
    }

    /// Whether a browser is evidently on its way back at `now` (ADR-0196 Decision 12).
    ///
    /// Each piece of evidence lapses on its own: a registered connection that is silent lasts as
    /// long as the connection, a departure lasts the return grace, and the relays of an earlier
    /// service are expected until `relays_until`.
    /// With `installed`, only that installed browser's connections and departures count.
    fn returning(
        &self,
        now: Instant,
        relays_until: Instant,
        timeout: Duration,
        installed: Option<&str>,
    ) -> bool {
        let ours = |kind: Option<&str>| installed.is_none_or(|wanted| kind == Some(wanted));
        let silent = self.connections.iter().any(|(browser, connection)| {
            ours(self.installed.get(browser).copied()) && !connection.available(now, timeout)
        });
        let departed_recently = self.departed.iter().any(|(kind, departed)| {
            ours(*kind) && now.saturating_duration_since(*departed) < RETURN_GRACE
        });
        let relays_reconnecting = now < relays_until;
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
    fn reachability(&self, browser: &str, now: Instant, timeout: Duration) -> Availability {
        match self.connections.get(browser) {
            None => Availability::Absent,
            Some(connection) if !connection.available(now, timeout) => Availability::NotAnswering,
            Some(_) => Availability::Available,
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

    /// Begin one liveness probe and return the sequence it carries.
    fn begin_probe(&self) -> u32 {
        self.state().begin_probe()
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
    /// Until when relays that served an earlier service may still be on their way back to this
    /// port: the relay return window from its construction, then from the moment the service
    /// published the endpoint relays find it by, or the return grace from that moment after a
    /// replacement.
    relays_until: Mutex<Instant>,
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
            relays_until: Mutex::new(Instant::now() + RELAY_RETURN_WINDOW),
        }
    }

    /// Record that the service is publishing the endpoint relays find it by.
    ///
    /// Relays that served an earlier service find it from this moment, so it starts the short
    /// window in which a call with no browser waits for them (`RELAY_RETURN_WINDOW`).
    /// Construction is too early a start: the desktop may take a second or more to publish.
    pub fn endpoint_published(&self) {
        self.expect_relays_for(RELAY_RETURN_WINDOW);
    }

    /// Record that the service is publishing the endpoint relays find it by in place of a
    /// service that just ended, as an update or the shell's recovery does.
    ///
    /// The browsers that service served left with it, and each one's relay looks for this one on
    /// its own doubling backoff, counted from that loss: 0.5, 1.5, 3.5 and 7.5 s after it. A
    /// replacement that publishes just after one look waits nearly the whole next wait for the
    /// look that finds it, which outlasts the short window once the replacement takes 1.5 s. So
    /// these browsers count as on their way back for the return grace, as any browser that left
    /// moments ago does. When a replacement publishes before the relay's fourth look, the relay
    /// looks again within 4 s, which leaves 3 s of the grace for it to hand its browser to a new
    /// native port.
    ///
    /// A home that remembers no browser ([`BrowserPort::last_connected_browser`]), as on a first
    /// install, had none to hand over, so there this is [`Self::endpoint_published`], and a call
    /// with no browser keeps its browser startup budget.
    pub fn replacement_published(&self) {
        let window = if self.last_connected_browser().is_some() {
            RETURN_GRACE
        } else {
            RELAY_RETURN_WINDOW
        };
        self.expect_relays_for(window);
    }

    /// Expect relays that served an earlier service for `window` from now.
    fn expect_relays_for(&self, window: Duration) {
        *lock(&self.relays_until) = Instant::now() + window;
    }

    /// Move this port's evidence clock back, as if `elapsed` had passed since it published its
    /// endpoint and since its last browser left.
    #[cfg(test)]
    pub(crate) fn backdate(&self, elapsed: Duration) {
        let earlier = |instant: Instant| {
            instant
                .checked_sub(elapsed)
                .expect("the monotonic clock has run longer than the backdate")
        };
        {
            let mut relays_until = lock(&self.relays_until);
            *relays_until = earlier(*relays_until);
        }
        let mut adapters = lock(&self.adapters);
        for departed in adapters.departed.values_mut() {
            *departed = earlier(*departed);
        }
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
        let installed = browser_id_named(&connection.browser_name);
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
                connection.line.end();
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

    /// The service epoch a protocol's acceptance names.
    pub(crate) fn service_epoch(&self) -> &str {
        &self.service_epoch
    }

    /// The page assets every protocol-4 connection receives when it is accepted.
    pub(crate) fn assets(&self) -> &wire::Assets {
        &self.assets
    }

    /// The runtime control state, locked: a protocol that must not let a later publish overtake
    /// the state it writes holds this while it writes.
    pub(crate) fn control_state(&self) -> MutexGuard<'_, RuntimeControlState> {
        lock(&self.control_state)
    }

    /// Register one connection a protocol accepted, under the browser identity its hello named,
    /// and hand back what that protocol's reader reports through.
    ///
    /// # Errors
    ///
    /// Refuses a new browser once the port holds its connection capacity, and closes the line.
    pub(crate) fn admit(&self, admitted: Admitted) -> Result<Registration, BrowserError> {
        let tag = ConnectionTag {
            browser_id: admitted.browser_id,
            connection_id: format!("{CONNECTION_PREFIX}{}", Uuid::new_v4().simple()),
        };
        let liveness = Arc::new(Liveness::new(
            tag.browser_id.clone(),
            lock(&self.lifecycle).clone(),
            Instant::now(),
        ));
        self.register(
            Connection {
                id: tag.connection_id.clone(),
                line: admitted.line,
                adapter_version: admitted.adapter_version,
                browser_id: tag.browser_id.clone(),
                browser_name: admitted.browser_name,
                life: admitted.life,
                liveness: Arc::clone(&liveness),
            },
            admitted.attended,
        )?;
        Ok(Registration {
            adapters: Arc::clone(&self.adapters),
            notifications: self.notifications(),
            tag,
            liveness,
        })
    }

    /// Start the timer side of one admitted connection's liveness, probing over `line`.
    ///
    /// # Errors
    ///
    /// Withdraws the connection and reports the failure when the thread cannot start.
    pub(crate) fn watch(
        &self,
        registration: &Registration,
        line: Line,
    ) -> Result<(), BrowserError> {
        let adapters = Arc::clone(&self.adapters);
        let settings = self.heartbeat;
        let tag = registration.tag.clone();
        let liveness = Arc::clone(&registration.liveness);
        if let Err(error) = thread::Builder::new()
            .name("ghostlight-browser-heartbeat".into())
            .spawn(move || {
                heartbeat_adapter(&line, &liveness, &adapters, &tag, settings);
            })
        {
            // A connection with nobody probing it would go stale and stay registered, so undo
            // the registration rather than leave a connection no thread watches.
            self.withdraw(registration);
            return Err(BrowserError::Protocol(error.to_string()));
        }
        Ok(())
    }

    /// Withdraw an admitted connection whose protocol could not start serving it.
    pub(crate) fn withdraw(&self, registration: &Registration) {
        self.detach_registered(
            &registration.tag.browser_id,
            &registration.tag.connection_id,
        );
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
        command: BrowserCommand,
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
        protocol4::run(command, &mut Link::new(line, dispatch))
    }

    /// Wait for one browser's connection to be registered and answering.
    fn await_registered(
        &self,
        browser: &str,
        until: Instant,
        cancelled: &AtomicBool,
    ) -> Availability {
        poll_until(until, cancelled, || {
            match lock(&self.adapters).reachability(browser, Instant::now(), self.heartbeat.timeout)
            {
                Availability::Available => ControlFlow::Break(Availability::Available),
                waiting => ControlFlow::Continue(waiting),
            }
        })
    }
}

impl BrowserPort for RelayBrowserPort {
    fn call(
        &self,
        browser: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.call_inner(
            browser,
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
        command: BrowserCommand,
        dispatch: BrowserDispatch<'_>,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.call_inner(browser, command, dispatch)
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

    fn note_start(&self, installed: &str, now: Instant, window: Duration) -> bool {
        let mut adapters = lock(&self.adapters);
        let arrivals = adapters.arrivals.get(installed).copied().unwrap_or(0);
        adapters.launches.note(installed, arrivals, now, window)
    }

    fn forget_start(&self, installed: &str) {
        lock(&self.adapters).launches.forget(installed);
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
    /// an endpoint published less than the relay return window ago, or less than the return
    /// grace ago in place of a service that just ended, while relays that served an earlier
    /// service reconnect. Attention a browser reported at some point is not evidence: it never
    /// lapses, and a browser closed long ago would then spend the whole wait that browser startup
    /// needs.
    fn browser_returning(&self) -> bool {
        let relays_until = *lock(&self.relays_until);
        lock(&self.adapters).returning(Instant::now(), relays_until, self.heartbeat.timeout, None)
    }

    fn returning_of(&self, browser: &str) -> bool {
        let relays_until = *lock(&self.relays_until);
        let adapters = lock(&self.adapters);
        let installed = adapters.installed.get(browser).copied();
        adapters.returning(
            Instant::now(),
            relays_until,
            self.heartbeat.timeout,
            installed,
        )
    }

    /// Runtime control is a property of Ghostlight, not of one browser, so every connected
    /// adapter learns the new state. One unreachable browser does not hide the state from the
    /// rest.
    fn publish_control_state(&self, state: RuntimeControlState) -> Result<(), BrowserError> {
        *lock(&self.control_state) = state;
        let lines: Vec<_> = lock(&self.adapters)
            .connections
            .values()
            .map(|connection| connection.line.clone())
            .collect();
        let mut published = Ok(());
        let deadline = Instant::now() + CONTROL_DELIVERY_TIMEOUT;
        for line in lines {
            if line.control_state(state, deadline).is_err() {
                published = Err(BrowserError::DisconnectedAfterDispatch);
            }
        }
        published
    }
}

/// What a protocol tells the port about a connection whose hello it accepted.
pub(crate) struct Admitted {
    /// What each command carried over the connection uses.
    pub(crate) line: Line,
    /// The adapter version the hello reported.
    pub(crate) adapter_version: String,
    /// The browser identity the hello named.
    pub(crate) browser_id: String,
    /// The browser's product name, as the protocol validated it.
    pub(crate) browser_name: String,
    /// The extension life the hello reported, if it reported a valid one.
    pub(crate) life: Option<String>,
    /// Whether the browser holds a focused window now.
    pub(crate) attended: bool,
}

/// One admitted connection as the protocol that reads it reports to the port: whether it still
/// serves its browser, that browser's attention, its events, its acknowledgements and its end.
#[derive(Clone)]
pub(crate) struct Registration {
    adapters: Arc<Mutex<AdapterRegistry>>,
    notifications: AdapterNotifications,
    tag: ConnectionTag,
    liveness: Arc<Liveness>,
}

impl Registration {
    /// Whether this connection is still the one serving its browser.
    pub(crate) fn is_current(&self) -> bool {
        is_current(&self.adapters, &self.tag)
    }

    /// The browser now holds a focused window.
    pub(crate) fn attended(&self) {
        lock(&self.adapters).attend(&self.tag.browser_id);
    }

    /// Hand one event from the browser to the event sink.
    pub(crate) fn event(&self, event: BrowserEvent) {
        if let Some(sink) = &self.notifications.sink {
            sink.on_event(&self.tag.browser_id, event);
        }
    }

    /// The browser acknowledged the liveness probe `sequence`.
    pub(crate) fn acknowledge(&self, sequence: u32) {
        self.liveness.acknowledge(sequence, Instant::now());
    }

    /// The connection's stream ended: forget it, and tell the sink and the lifecycle observer.
    pub(crate) fn end(&self) {
        end_connection(&self.adapters, &self.tag, &self.notifications);
    }
}

/// The notification targets one adapter reader reports to: the typed event sink and the
/// lifecycle observer.
#[derive(Clone)]
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
    line: &Line,
    liveness: &Liveness,
    adapters: &Mutex<AdapterRegistry>,
    tag: &ConnectionTag,
    settings: HeartbeatSettings,
) {
    loop {
        if line.writer.wait_closed(settings.interval) {
            return;
        }
        if !is_current(adapters, tag) {
            return;
        }
        liveness.expire(Instant::now(), settings.timeout);
        if line.heartbeat(liveness.begin_probe()).is_err() {
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
    previous.line.end();
}

/// Look with `look` until it is done, `until` passes or `cancelled` is set, sleeping at most
/// [`AVAILABILITY_POLL`] between looks. `look` breaks with the answer, or continues with the one a
/// wait that ends now gives.
fn poll_until<T>(
    until: Instant,
    cancelled: &AtomicBool,
    mut look: impl FnMut() -> ControlFlow<T, T>,
) -> T {
    loop {
        let pending = match look() {
            ControlFlow::Break(answer) => return answer,
            ControlFlow::Continue(pending) => pending,
        };
        if cancelled.load(Ordering::SeqCst) || Instant::now() >= until {
            return pending;
        }
        thread::sleep(
            until
                .saturating_duration_since(Instant::now())
                .min(AVAILABILITY_POLL),
        );
    }
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
    /// Adapter decisively rejected a primitive without an effect. `cause` is the closed kind of
    /// the refusal the receipt named; `detail` keeps the browser's or the adapter's own words.
    #[error("browser primitive failed: {detail}")]
    Primitive {
        detail: String,
        cause: crate::language::outcome::PrimitiveCause,
    },
    /// No capture in the budget fitted its size limit. Raised before any effect; `detail` keeps
    /// the browser's words.
    #[error("no capture fitted its size limit: {detail}")]
    CaptureTooLarge { detail: String },
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
    /// The page threw while running the script the call sent, with the page's own description
    /// of what it threw. The script may have acted before it threw, so its effect is unknown.
    #[error("the page script threw: {0}")]
    ScriptThrew(String),
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
    /// A primitive the adapter or the browser refused in words of its own.
    #[must_use]
    pub fn primitive(detail: impl Into<String>) -> Self {
        Self::Primitive {
            detail: detail.into(),
            cause: crate::language::outcome::PrimitiveCause::Refused,
        }
    }

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
                | Self::ScriptThrew(_)
        )
    }
}

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
pub(crate) mod testing;
