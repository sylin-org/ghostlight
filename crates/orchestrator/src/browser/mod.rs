//! The physical browser port and authenticated relay-backed adapter implementation.

pub mod recovery;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use ghostlight_bridge::browser::{
    adapter_capability, AdapterCapability, BrowserCommand, BrowserEvent, BrowserFrame,
    BrowserOutcome, BrowserRequest, RuntimeControlState, ADAPTER_PROTOCOL_MAJOR,
    COMMAND_CHUNK_PAYLOAD_BYTES, COMMAND_TRANSFER_MAX_BYTES, COMMAND_TRANSFER_MAX_CHUNKS,
};
use ghostlight_bridge::framing::{read_native, write_length_frame, write_native, FrameError};
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
// A product name is a label a person reads next to a browser they already recognize, not a
// place for an adapter to write prose.
const BROWSER_NAME_MAX_CHARS: usize = 40;

#[derive(Clone, Copy, Debug)]
struct HeartbeatSettings {
    interval: Duration,
    timeout: Duration,
}

impl Default for HeartbeatSettings {
    fn default() -> Self {
        Self {
            interval: HEARTBEAT_INTERVAL,
            timeout: HEARTBEAT_TIMEOUT,
        }
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
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError>;

    /// Every connected browser, most recently attended first.
    fn browsers(&self) -> Vec<BrowserSummary>;

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
    writer: Arc<Mutex<TcpStream>>,
    pending: Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    adapter_version: String,
    browser_id: String,
    browser_name: Option<String>,
    capabilities: HashMap<String, u16>,
    liveness: Option<Arc<Mutex<ConnectionLiveness>>>,
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
}

impl AdapterRegistry {
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

    /// Content-free inventory of connected browsers, most recently attended first.
    fn summaries(&self, timeout: Duration) -> Vec<BrowserSummary> {
        let now = Instant::now();
        let attended = self.attended().map(str::to_owned);
        let mut summaries: Vec<_> = self
            .connections
            .values()
            .filter(|connection| {
                connection
                    .liveness
                    .as_ref()
                    .is_none_or(|liveness| lock(liveness).is_available(now, timeout))
            })
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
    last_acknowledged_sequence: u32,
    stale: bool,
}

impl ConnectionLiveness {
    fn new(now: Instant) -> Self {
        Self {
            last_acknowledged_at: now,
            next_sequence: 0,
            last_acknowledged_sequence: 0,
            stale: false,
        }
    }

    fn begin_probe(&mut self) -> u32 {
        self.next_sequence = self.next_sequence.saturating_add(1).max(1);
        self.next_sequence
    }

    fn acknowledge(&mut self, sequence: u32, now: Instant) {
        if sequence == 0 || sequence > self.next_sequence {
            return;
        }
        self.last_acknowledged_at = now;
        self.last_acknowledged_sequence = self.last_acknowledged_sequence.max(sequence);
        self.stale = false;
    }

    fn acknowledged(&self, sequence: u32) -> bool {
        self.last_acknowledged_sequence >= sequence
    }

    fn is_available(&self, now: Instant, timeout: Duration) -> bool {
        !self.stale && now.saturating_duration_since(self.last_acknowledged_at) < timeout
    }

    fn mark_stale(&mut self) -> bool {
        let changed = !self.stale;
        self.stale = true;
        changed
    }
}

/// Authenticated loopback implementation of the physical browser port.
pub struct RelayBrowserPort {
    service_epoch: String,
    adapters: Arc<Mutex<AdapterRegistry>>,
    event_sink: Mutex<Option<Arc<dyn BrowserEventSink>>>,
    control_state: Mutex<RuntimeControlState>,
    heartbeat: HeartbeatSettings,
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
        Self {
            service_epoch,
            adapters: Arc::new(Mutex::new(AdapterRegistry::default())),
            event_sink: Mutex::new(None),
            control_state: Mutex::new(RuntimeControlState::Active),
            heartbeat: HeartbeatSettings::default(),
        }
    }

    #[cfg(test)]
    fn with_heartbeat_settings(service_epoch: String, heartbeat: HeartbeatSettings) -> Self {
        debug_assert!(heartbeat.interval < heartbeat.timeout);
        Self {
            service_epoch,
            adapters: Arc::new(Mutex::new(AdapterRegistry::default())),
            event_sink: Mutex::new(None),
            control_state: Mutex::new(RuntimeControlState::Active),
            heartbeat,
        }
    }

    /// Install the direct typed event reaction target.
    pub fn set_event_sink(&self, sink: Arc<dyn BrowserEventSink>) {
        *lock(&self.event_sink) = Some(sink);
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
    fn is_registered(&self, browser: &str) -> bool {
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
        let mut reader = stream
            .try_clone()
            .map_err(|error| BrowserError::Protocol(error.to_string()))?;
        let Some(BrowserFrame::Hello {
            major,
            adapter_version,
            browser_id,
            adapter_epoch,
            browser_name,
            attended,
            capabilities,
        }) = read_native(&mut reader).map_err(|error| BrowserError::Protocol(error.to_string()))?
        else {
            return Err(BrowserError::Authentication);
        };
        if major != ADAPTER_PROTOCOL_MAJOR {
            return Err(BrowserError::Incompatible {
                offered: major,
                required: ADAPTER_PROTOCOL_MAJOR,
            });
        }
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
        let liveness = (capabilities
            .get(adapter_capability::ADAPTER_LIVENESS)
            .copied()
            .unwrap_or_default()
            >= 1)
            .then(|| Arc::new(Mutex::new(ConnectionLiveness::new(Instant::now()))));

        let writer = Arc::new(Mutex::new(stream));
        write_native(
            &mut *lock(&writer),
            &BrowserFrame::HelloAccepted {
                major: ADAPTER_PROTOCOL_MAJOR,
                service_version: env!("CARGO_PKG_VERSION").into(),
                service_epoch: self.service_epoch.clone(),
                control_state: *lock(&self.control_state),
            },
        )
        .map_err(|error| BrowserError::Protocol(error.to_string()))?;
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let connection_id = format!("connection_{}", Uuid::new_v4().simple());
        let connection = Connection {
            id: connection_id.clone(),
            writer: Arc::clone(&writer),
            pending: Arc::clone(&pending),
            adapter_version,
            browser_id: browser_id.clone(),
            browser_name,
            capabilities,
            liveness: liveness.clone(),
        };
        {
            let mut adapters = lock(&self.adapters);
            if let Some(previous) = adapters.connections.insert(browser_id.clone(), connection) {
                retire(&previous);
            }
            if reports_attention && attended {
                adapters.attend(&browser_id);
            }
        }
        let sink = lock(&self.event_sink).clone();
        let tag = ConnectionTag {
            browser_id,
            connection_id,
        };
        let reader_adapters = Arc::clone(&self.adapters);
        let reader_tag = tag.clone();
        let reader_liveness = liveness.clone();
        let heartbeat_writer = Arc::clone(&writer);
        let heartbeat_pending = Arc::clone(&pending);
        if let Err(error) = thread::Builder::new()
            .name("ghostlight-browser-reader".into())
            .spawn(move || {
                read_adapter(
                    reader,
                    writer,
                    pending,
                    sink,
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

    /// Remove a registered connection, but only the exact one named -- never a connection that
    /// has since replaced it. `attach` registers a connection before either of its threads is
    /// proven to actually run, so a spawn failure must undo exactly that registration, and
    /// nothing else: a concurrent `attach` for the same `browser_id` may already have replaced it
    /// with a newer, healthy connection by the time this runs, and that one must be left alone.
    fn detach_registered(&self, browser_id: &str, connection_id: &str) {
        let removed = {
            let mut adapters = lock(&self.adapters);
            match adapters.connections.get(browser_id) {
                Some(connection) if connection.id == connection_id => {
                    adapters.connections.remove(browser_id)
                }
                _ => None,
            }
        };
        if let Some(connection) = removed {
            retire(&connection);
        }
    }

    fn call_inner(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
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
            if connection.liveness.as_ref().is_some_and(|liveness| {
                !lock(liveness).is_available(Instant::now(), self.heartbeat.timeout)
            }) {
                return Err(BrowserError::DisconnectedBeforeDispatch);
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
        lock(&pending).insert(correlation.clone(), sender);
        let probe = liveness
            .as_ref()
            .map(|liveness| lock(liveness).begin_probe());
        let mut output = lock(&writer);
        if write_request_payload(&mut *output, &payload, &correlation).is_err() {
            lock(&pending).remove(&correlation);
            mark_stale(&liveness);
            return Err(BrowserError::DisconnectedAfterDispatch);
        }
        if let Some(sequence) = probe {
            if write_native(&mut *output, &BrowserFrame::Heartbeat { sequence }).is_err() {
                lock(&pending).remove(&correlation);
                mark_stale(&liveness);
                return Err(BrowserError::DisconnectedAfterDispatch);
            }
        }
        drop(output);
        await_receipt(
            receiver,
            &correlation,
            &writer,
            &pending,
            deadline,
            cancelled,
            liveness.zip(probe),
        )
    }
}

impl BrowserPort for RelayBrowserPort {
    fn call(
        &self,
        browser: &str,
        workspace: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.call_inner(browser, workspace, command, deadline, cancelled)
    }

    fn browsers(&self) -> Vec<BrowserSummary> {
        self.connected_browsers()
    }

    /// Runtime control is a property of Ghostlight, not of one browser, so every connected
    /// adapter learns the new state. One unreachable browser does not hide the state from the
    /// rest.
    fn publish_control_state(&self, state: RuntimeControlState) -> Result<(), BrowserError> {
        *lock(&self.control_state) = state;
        let writers: Vec<_> = lock(&self.adapters)
            .connections
            .values()
            .map(|connection| Arc::clone(&connection.writer))
            .collect();
        let mut published = Ok(());
        for writer in writers {
            if write_native(&mut *lock(&writer), &BrowserFrame::ControlState { state }).is_err() {
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
    writer: &Arc<Mutex<TcpStream>>,
    pending: &Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
    liveness_probe: Option<(Arc<Mutex<ConnectionLiveness>>, u32)>,
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
            if let Some((liveness, sequence)) = &liveness_probe {
                let unanswered = !lock(liveness).acknowledged(*sequence);
                if unanswered {
                    lock(liveness).mark_stale();
                    fail_pending(pending, BrowserError::DisconnectedAfterDispatch);
                }
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

fn send_cancel(writer: &Arc<Mutex<TcpStream>>, correlation: &str) {
    let frame = BrowserFrame::Request {
        request: BrowserRequest {
            correlation: format!("cancel_{}", Uuid::new_v4().simple()),
            workspace: "system".into(),
            command: BrowserCommand::Cancel {
                correlation: correlation.into(),
            },
        },
    };
    let _ = write_native(&mut *lock(writer), &frame);
}

fn read_adapter(
    mut reader: TcpStream,
    writer: Arc<Mutex<TcpStream>>,
    pending: Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    sink: Option<Arc<dyn BrowserEventSink>>,
    adapters: Arc<Mutex<AdapterRegistry>>,
    tag: ConnectionTag,
    liveness: Option<Arc<Mutex<ConnectionLiveness>>>,
) {
    let ConnectionTag { browser_id, .. } = &tag;
    loop {
        match read_native::<BrowserFrame>(&mut reader) {
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
                if let Some(sink) = &sink {
                    sink.on_event(browser_id, event);
                }
            }
            Ok(Some(BrowserFrame::HeartbeatAck { sequence })) => {
                if let Some(liveness) = &liveness {
                    lock(liveness).acknowledge(sequence, Instant::now());
                }
            }
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                fail_pending(&pending, BrowserError::DisconnectedAfterDispatch);
                // A replaced connection must not evict the connection that replaced it. Its
                // attention place survives, so the same browser reconnecting resumes where it
                // was rather than starting behind every other browser.
                let was_current = {
                    let mut adapters = lock(&adapters);
                    let current = adapters
                        .connections
                        .get(browser_id)
                        .is_some_and(|connection| connection.id == tag.connection_id);
                    if current {
                        adapters.connections.remove(browser_id);
                    }
                    current
                };
                if was_current {
                    if let Some(sink) = &sink {
                        sink.on_event(browser_id, BrowserEvent::Disconnected);
                    }
                }
                return;
            }
        }
    }
}

fn heartbeat_adapter(
    writer: Arc<Mutex<TcpStream>>,
    pending: Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    liveness: Arc<Mutex<ConnectionLiveness>>,
    adapters: Arc<Mutex<AdapterRegistry>>,
    tag: ConnectionTag,
    settings: HeartbeatSettings,
) {
    loop {
        thread::sleep(settings.interval);
        if !is_current(&adapters, &tag) {
            return;
        }

        let now = Instant::now();
        let (sequence, became_stale) = {
            let mut state = lock(&liveness);
            let became_stale =
                if now.saturating_duration_since(state.last_acknowledged_at) >= settings.timeout {
                    state.mark_stale()
                } else {
                    false
                };
            (state.begin_probe(), became_stale)
        };
        if became_stale {
            fail_pending(&pending, BrowserError::DisconnectedAfterDispatch);
        }
        if write_native(&mut *lock(&writer), &BrowserFrame::Heartbeat { sequence }).is_err() {
            lock(&liveness).mark_stale();
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
    let _ = lock(&previous.writer).shutdown(Shutdown::Both);
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

fn mark_stale(liveness: &Option<Arc<Mutex<ConnectionLiveness>>>) {
    if let Some(liveness) = liveness {
        lock(liveness).mark_stale();
    }
}

fn acknowledge(writer: &Arc<Mutex<TcpStream>>, correlation: String) {
    let _ = write_native(
        &mut *lock(writer),
        &BrowserFrame::Acknowledge { correlation },
    );
}

fn adapter_error(code: &str, message: String, effect_unknown: bool) -> BrowserError {
    if effect_unknown {
        BrowserError::EffectUnknown(message)
    } else if code == "local_interlock" {
        BrowserError::LocalInterlock(message)
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
    /// No adapter existed before dispatch.
    #[error("browser adapter is disconnected before dispatch")]
    DisconnectedBeforeDispatch,
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
        /// Unique installed browser name, when one was found.
        browser: Option<String>,
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
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use ghostlight_bridge::browser::{
        adapter_capability, AdapterCapability, BrowserCommand, BrowserFrame, BrowserOutcome,
        BrowserReadiness, BrowserReceipt, PhysicalTab, ADAPTER_PROTOCOL_MAJOR,
    };
    use ghostlight_bridge::framing::{read_native, write_native};

    use super::{
        adapter_error, choose_browser, testing, AdapterRegistry, BrowserError, BrowserPort,
        HeartbeatSettings, RelayBrowserPort,
    };

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
        }
    }

    #[test]
    fn adapter_local_interlock_is_a_decisive_typed_refusal() {
        assert_eq!(
            adapter_error("local_interlock", "preserved".into(), false),
            BrowserError::LocalInterlock("preserved".into())
        );
        assert_eq!(
            adapter_error("local_interlock", "unknown".into(), true),
            BrowserError::EffectUnknown("unknown".into())
        );
    }

    #[test]
    fn incompatible_browser_bridge_fails_during_hello() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            write_native(
                &mut stream,
                &BrowserFrame::Hello {
                    major: ADAPTER_PROTOCOL_MAJOR + 1,
                    adapter_version: "future".into(),
                    browser_id: "browser_test".into(),
                    adapter_epoch: "adapter_test".into(),
                    browser_name: None,
                    attended: false,
                    capabilities: vec![],
                },
            )
            .unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        let port = RelayBrowserPort::new("service_test".into());
        assert_eq!(
            port.attach(stream),
            Err(BrowserError::Incompatible {
                offered: ADAPTER_PROTOCOL_MAJOR + 1,
                required: ADAPTER_PROTOCOL_MAJOR
            })
        );
        client.join().unwrap();
    }

    #[test]
    fn an_older_capability_revision_refuses_before_dispatch() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (release, hold) = mpsc::channel();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            announce_adapter(
                &mut stream,
                vec![
                    capability(adapter_capability::TABS),
                    AdapterCapability {
                        name: adapter_capability::SCRIPT.into(),
                        revision: 1,
                    },
                ],
            );
            hold.recv().unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        let port =
            RelayBrowserPort::with_heartbeat_settings("service_test".into(), short_heartbeat());
        port.attach(stream).unwrap();

        assert_eq!(
            port.call(
                TEST_BROWSER,
                "workspace_test",
                BrowserCommand::EvaluateScript {
                    tab_id: 1,
                    script: "1+1".into(),
                    max_result_chars: 1000,
                },
                Instant::now() + Duration::from_millis(200),
                &AtomicBool::new(false),
            ),
            Err(BrowserError::CapabilityVersion {
                capability: adapter_capability::SCRIPT.into(),
                required: adapter_capability::SCRIPT_REVISION_REPL,
                advertised: 1,
            })
        );

        // A revision-1 command still dispatches against the same connection,
        // so the refusal is per command, not per adapter.
        assert!(matches!(
            port.call(
                TEST_BROWSER,
                "workspace_test",
                BrowserCommand::ListTabs,
                Instant::now() + Duration::from_millis(100),
                &AtomicBool::new(false),
            ),
            Err(BrowserError::DeadlineAfterDispatch)
        ));
        release.send(()).unwrap();
        client.join().unwrap();
    }

    #[test]
    fn attachment_without_adapter_acknowledgement_becomes_unavailable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (release, hold) = mpsc::channel();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            announce_adapter(
                &mut stream,
                vec![
                    capability(adapter_capability::TABS),
                    capability(adapter_capability::ADAPTER_LIVENESS),
                ],
            );
            hold.recv().unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        let port =
            RelayBrowserPort::with_heartbeat_settings("service_test".into(), short_heartbeat());
        port.attach(stream).unwrap();
        assert!(port.is_connected());

        let deadline = Instant::now() + Duration::from_millis(500);
        while port.is_connected() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }

        assert!(!port.is_connected());
        assert!(
            port.is_registered(TEST_BROWSER),
            "the relay socket is still attached"
        );
        assert_eq!(
            port.call(
                TEST_BROWSER,
                "workspace_test",
                BrowserCommand::ListTabs,
                Instant::now() + Duration::from_millis(100),
                &AtomicBool::new(false),
            ),
            Err(BrowserError::DisconnectedBeforeDispatch)
        );
        release.send(()).unwrap();
        client.join().unwrap();
    }

    #[test]
    fn an_adapter_without_liveness_keeps_its_compatible_attachment_semantics() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (release, hold) = mpsc::channel();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            announce_adapter(&mut stream, vec![capability(adapter_capability::TABS)]);
            hold.recv().unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        let port =
            RelayBrowserPort::with_heartbeat_settings("service_test".into(), short_heartbeat());
        port.attach(stream).unwrap();

        thread::sleep(Duration::from_millis(75));

        assert!(port.is_connected());
        release.send(()).unwrap();
        client.join().unwrap();
    }

    #[test]
    fn an_unanswered_dispatch_probe_quarantines_the_adapter_at_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (release, hold) = mpsc::channel();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            announce_adapter(
                &mut stream,
                vec![
                    capability(adapter_capability::TABS),
                    capability(adapter_capability::ADAPTER_LIVENESS),
                ],
            );
            hold.recv().unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        let port = RelayBrowserPort::with_heartbeat_settings(
            "service_test".into(),
            HeartbeatSettings {
                interval: Duration::from_secs(1),
                timeout: Duration::from_secs(2),
            },
        );
        port.attach(stream).unwrap();

        assert_eq!(
            port.call(
                TEST_BROWSER,
                "workspace_test",
                BrowserCommand::ListTabs,
                Instant::now() + Duration::from_millis(75),
                &AtomicBool::new(false),
            ),
            Err(BrowserError::DeadlineAfterDispatch)
        );
        assert!(!port.is_connected());
        assert!(
            port.is_registered(TEST_BROWSER),
            "the relay socket is still attached"
        );
        release.send(()).unwrap();
        client.join().unwrap();
    }

    #[test]
    fn heartbeat_acknowledgements_keep_a_silent_operation_available() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (release, hold) = mpsc::channel();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            announce_adapter(
                &mut stream,
                vec![
                    capability(adapter_capability::TABS),
                    capability(adapter_capability::ADAPTER_LIVENESS),
                ],
            );
            let mut pending = None;
            loop {
                let frame = read_native::<BrowserFrame>(&mut stream).unwrap().unwrap();
                match frame {
                    BrowserFrame::Request { request }
                        if matches!(request.command, BrowserCommand::ListTabs) =>
                    {
                        pending = Some((request.correlation, Instant::now()));
                    }
                    BrowserFrame::Heartbeat { sequence } => {
                        write_native(&mut stream, &BrowserFrame::HeartbeatAck { sequence })
                            .unwrap();
                    }
                    _ => {}
                }
                if pending
                    .as_ref()
                    .is_some_and(|(_, started)| started.elapsed() >= Duration::from_millis(1_500))
                {
                    let (correlation, _) = pending.take().unwrap();
                    write_native(
                        &mut stream,
                        &BrowserFrame::Receipt {
                            receipt: BrowserReceipt {
                                correlation,
                                result: BrowserOutcome::Tabs { tabs: vec![] },
                            },
                        },
                    )
                    .unwrap();
                    break;
                }
            }
            hold.recv().unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        let port = RelayBrowserPort::with_heartbeat_settings(
            "service_test".into(),
            HeartbeatSettings {
                interval: Duration::from_millis(100),
                timeout: Duration::from_secs(1),
            },
        );
        port.attach(stream).unwrap();

        assert_eq!(
            port.call(
                TEST_BROWSER,
                "workspace_test",
                BrowserCommand::ListTabs,
                Instant::now() + Duration::from_secs(4),
                &AtomicBool::new(false),
            ),
            Ok(BrowserOutcome::Tabs { tabs: vec![] })
        );
        assert!(port.is_connected());
        release.send(()).unwrap();
        client.join().unwrap();
    }

    #[test]
    fn two_browsers_are_two_adapters_and_each_keeps_its_own_work() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let port = RelayBrowserPort::new("service_test".into());

        let mut adapters = Vec::new();
        for browser in ["browser_chrome", "browser_edge"] {
            let (ready, wait) = mpsc::channel();
            let client = thread::spawn(move || {
                let mut stream = TcpStream::connect(address).unwrap();
                announce_browser(
                    &mut stream,
                    browser,
                    false,
                    vec![capability(adapter_capability::TABS)],
                );
                ready.send(()).unwrap();
                // Answer exactly one request, naming which browser answered it.
                let Some(BrowserFrame::Request { request }) =
                    read_native::<BrowserFrame>(&mut stream).unwrap()
                else {
                    panic!("the adapter is asked for one primitive");
                };
                write_native(
                    &mut stream,
                    &BrowserFrame::Receipt {
                        receipt: BrowserReceipt {
                            correlation: request.correlation,
                            result: BrowserOutcome::Tabs {
                                tabs: vec![PhysicalTab {
                                    tab_id: 5,
                                    title: browser.into(),
                                    url: "about:blank".into(),
                                    active: true,
                                    readiness: BrowserReadiness::Complete,
                                }],
                            },
                        },
                    },
                )
                .unwrap();
            });
            let (stream, _) = listener.accept().unwrap();
            port.attach(stream).unwrap();
            wait.recv().unwrap();
            adapters.push(client);
        }

        // Both are connected at once. Neither replaced the other, because they are two browsers.
        let connected: Vec<_> = port
            .connected_browsers()
            .into_iter()
            .map(|browser| browser.id)
            .collect();
        assert_eq!(connected, vec!["browser_chrome", "browser_edge"]);

        // A request reaches the browser it named, and each browser's tab 5 is its own.
        for browser in ["browser_chrome", "browser_edge"] {
            let Ok(BrowserOutcome::Tabs { tabs }) = port.call(
                browser,
                "workspace_test",
                BrowserCommand::ListTabs,
                Instant::now() + Duration::from_millis(500),
                &AtomicBool::new(false),
            ) else {
                panic!("each browser answers its own request");
            };
            assert_eq!(tabs[0].title, browser);
        }
        for adapter in adapters {
            adapter.join().unwrap();
        }
    }

    #[test]
    fn a_second_connection_from_one_browser_collapses_onto_the_first() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let port = RelayBrowserPort::new("service_test".into());

        // One browser opening two native ports is the failure this design exists to make
        // impossible: the browser mints its identity once, so the second connection is the same
        // adapter arriving twice, not a second browser.
        let (closed, retired) = mpsc::channel();
        let first = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            announce_browser(
                &mut stream,
                "browser_one",
                false,
                vec![capability(adapter_capability::TABS)],
            );
            let mut drained = Vec::new();
            // Returns only when the service closes this connection.
            let _ = std::io::Read::read_to_end(&mut stream, &mut drained);
            closed.send(()).unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        port.attach(stream).unwrap();

        let (ready, holding) = mpsc::channel();
        let (release, hold) = mpsc::channel();
        let second = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            announce_browser(
                &mut stream,
                "browser_one",
                false,
                vec![capability(adapter_capability::TABS)],
            );
            ready.send(()).unwrap();
            hold.recv().unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        port.attach(stream).unwrap();
        holding.recv().unwrap();

        // One browser is still one browser, served by its newest connection.
        assert_eq!(port.connected_browsers().len(), 1);

        // The replaced connection is closed rather than abandoned. Left open, it would keep its
        // relay process alive and the browser's stale native port with it, and every request
        // written into it would be dropped in silence instead of refused.
        assert!(
            retired.recv_timeout(Duration::from_secs(5)).is_ok(),
            "the replaced connection reads end-of-stream instead of hanging open"
        );
        first.join().unwrap();
        release.send(()).unwrap();
        second.join().unwrap();
    }

    #[test]
    fn attention_is_reported_move_to_front_and_never_routes_to_an_absent_browser() {
        let mut registry = AdapterRegistry::default();
        assert_eq!(registry.attended(), None);

        registry.attend("browser_chrome");
        registry.attend("browser_edge");
        registry.attend("browser_chrome");
        assert_eq!(registry.attention, ["browser_chrome", "browser_edge"]);

        // Attention outlives connections, so a browser that reconnects keeps the place it earned.
        // Until one of them is actually connected, it routes nothing.
        assert_eq!(registry.attended(), None);
    }

    #[test]
    fn routing_prefers_selection_then_binding_then_attention() {
        let chrome = testing::summary("browser_chrome", false);
        let edge = testing::summary("browser_edge", true);
        let connected = vec![chrome.clone(), edge.clone()];

        // An explicit selection wins over the attended default.
        assert_eq!(
            choose_browser(Some("browser_chrome"), None, &connected).unwrap(),
            "browser_chrome"
        );
        // An established binding wins over the attended default, so work stays where it started.
        assert_eq!(
            choose_browser(None, Some("browser_chrome"), &connected).unwrap(),
            "browser_chrome"
        );
        // With nothing else to go on, the browser the person last attended gets the work.
        assert_eq!(
            choose_browser(None, None, &connected).unwrap(),
            "browser_edge"
        );
        // A sole browser needs no evidence at all.
        assert_eq!(
            choose_browser(None, None, &[testing::summary("browser_only", false)]).unwrap(),
            "browser_only"
        );
    }

    #[test]
    fn routing_refuses_rather_than_guessing_or_failing_over() {
        let unattended = vec![
            testing::summary("browser_chrome", false),
            testing::summary("browser_edge", false),
        ];
        // Two browsers and no evidence: name the candidates, choose nothing.
        assert_eq!(
            choose_browser(None, None, &unattended),
            Err(BrowserError::AmbiguousBrowser(vec![
                "browser_chrome".into(),
                "browser_edge".into()
            ]))
        );
        // A bound browser that stopped never fails over to the other one.
        assert_eq!(
            choose_browser(None, Some("browser_gone"), &unattended),
            Err(BrowserError::DisconnectedBeforeDispatch)
        );
        // A workspace cannot be told to continue somewhere its tabs are not.
        assert_eq!(
            choose_browser(Some("browser_edge"), Some("browser_chrome"), &unattended),
            Err(BrowserError::BrowserPinned)
        );
        // A selection nobody is serving is a refusal, not a substitution.
        assert_eq!(
            choose_browser(Some("browser_absent"), None, &unattended),
            Err(BrowserError::UnknownBrowser("browser_absent".into()))
        );
        assert_eq!(
            choose_browser(None, None, &[]),
            Err(BrowserError::DisconnectedBeforeDispatch)
        );
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Mutex, MutexGuard};
    use std::time::Instant;

    use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome, RuntimeControlState};

    use super::{BrowserError, BrowserPort, BrowserSummary};

    /// The browser a fake stands in for when a test does not care which one it is.
    pub const FAKE_BROWSER: &str = "browser_fake";

    /// Deterministic browser port for executor contract tests.
    #[derive(Debug)]
    pub struct FakeBrowser {
        calls: Mutex<Vec<BrowserCommand>>,
        routed: Mutex<Vec<String>>,
        outcomes: Mutex<VecDeque<Result<BrowserOutcome, BrowserError>>>,
        control_states: Mutex<Vec<RuntimeControlState>>,
        connected: Mutex<Vec<BrowserSummary>>,
    }

    impl Default for FakeBrowser {
        fn default() -> Self {
            Self {
                calls: Mutex::default(),
                routed: Mutex::default(),
                outcomes: Mutex::default(),
                control_states: Mutex::default(),
                connected: Mutex::new(vec![summary(FAKE_BROWSER, true)]),
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
        pub fn push(&self, outcome: Result<BrowserOutcome, BrowserError>) {
            lock(&self.outcomes).push_back(outcome);
        }
        pub fn calls(&self) -> Vec<BrowserCommand> {
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
    }

    impl BrowserPort for FakeBrowser {
        fn call(
            &self,
            browser: &str,
            _workspace: &str,
            command: BrowserCommand,
            _deadline: Instant,
            _cancelled: &AtomicBool,
        ) -> Result<BrowserOutcome, BrowserError> {
            lock(&self.routed).push(browser.into());
            lock(&self.calls).push(command);
            lock(&self.outcomes)
                .pop_front()
                .unwrap_or_else(|| Err(BrowserError::Primitive("no fake outcome".into())))
        }

        fn browsers(&self) -> Vec<BrowserSummary> {
            lock(&self.connected).clone()
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
