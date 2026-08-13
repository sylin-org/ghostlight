//! The physical browser port and authenticated relay-backed adapter implementation.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::net::TcpStream;
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
    /// Dispatch one primitive and await a decisive receipt, cancellation, deadline, or disconnect.
    fn call(
        &self,
        workspace: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError>;

    /// Publish authoritative content-free runtime state without awaiting a receipt.
    fn publish_control_state(&self, _state: RuntimeControlState) -> Result<(), BrowserError> {
        Ok(())
    }
}

/// Sink for asynchronous physical browser facts.
pub trait BrowserEventSink: Send + Sync {
    /// React to one adapter event without granting authority or fabricating completion.
    fn on_event(&self, event: BrowserEvent);
}

type PendingResult = Result<BrowserOutcome, BrowserError>;

#[derive(Debug)]
struct Connection {
    id: String,
    writer: Arc<Mutex<TcpStream>>,
    pending: Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    adapter_version: String,
    browser_id: String,
    capabilities: HashMap<String, u16>,
    liveness: Option<Arc<Mutex<ConnectionLiveness>>>,
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
    connection: Arc<Mutex<Option<Connection>>>,
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
            connection: Arc::new(Mutex::new(None)),
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
            connection: Arc::new(Mutex::new(None)),
            event_sink: Mutex::new(None),
            control_state: Mutex::new(RuntimeControlState::Active),
            heartbeat,
        }
    }

    /// Install the direct typed event reaction target.
    pub fn set_event_sink(&self, sink: Arc<dyn BrowserEventSink>) {
        *lock(&self.event_sink) = Some(sink);
    }

    /// Whether a compatible adapter is currently connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        lock(&self.connection).as_ref().is_some_and(|connection| {
            connection.liveness.as_ref().is_none_or(|liveness| {
                lock(liveness).is_available(Instant::now(), self.heartbeat.timeout)
            })
        })
    }

    /// Return the connected adapter version for diagnostics.
    #[must_use]
    pub fn adapter_version(&self) -> Option<String> {
        lock(&self.connection)
            .as_ref()
            .map(|connection| connection.adapter_version.clone())
    }

    /// Return the connected persistent adapter installation id for diagnostics.
    #[must_use]
    pub fn browser_id(&self) -> Option<String> {
        lock(&self.connection)
            .as_ref()
            .map(|connection| connection.browser_id.clone())
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
            browser_id,
            capabilities,
            liveness: liveness.clone(),
        };
        if let Some(previous) = lock(&self.connection).replace(connection) {
            fail_pending(&previous.pending, BrowserError::DisconnectedAfterDispatch);
        }
        let sink = lock(&self.event_sink).clone();
        let reader_connections = Arc::clone(&self.connection);
        let reader_connection_id = connection_id.clone();
        let reader_liveness = liveness.clone();
        let heartbeat_writer = Arc::clone(&writer);
        let heartbeat_pending = Arc::clone(&pending);
        thread::Builder::new()
            .name("ghostlight-browser-reader".into())
            .spawn(move || {
                read_adapter(
                    reader,
                    writer,
                    pending,
                    sink,
                    reader_connections,
                    reader_connection_id,
                    reader_liveness,
                );
            })
            .map_err(|error| BrowserError::Protocol(error.to_string()))?;
        if let Some(liveness) = liveness {
            let heartbeat_connections = Arc::clone(&self.connection);
            let settings = self.heartbeat;
            thread::Builder::new()
                .name("ghostlight-browser-heartbeat".into())
                .spawn(move || {
                    heartbeat_adapter(
                        heartbeat_writer,
                        heartbeat_pending,
                        liveness,
                        heartbeat_connections,
                        connection_id,
                        settings,
                    );
                })
                .map_err(|error| BrowserError::Protocol(error.to_string()))?;
        }
        Ok(())
    }

    fn call_inner(
        &self,
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
            let connection = lock(&self.connection);
            let Some(connection) = connection.as_ref() else {
                return Err(BrowserError::DisconnectedBeforeDispatch);
            };
            if connection.liveness.as_ref().is_some_and(|liveness| {
                !lock(liveness).is_available(Instant::now(), self.heartbeat.timeout)
            }) {
                return Err(BrowserError::DisconnectedBeforeDispatch);
            }
            if connection
                .capabilities
                .get(required_capability)
                .copied()
                .unwrap_or_default()
                < 1
            {
                return Err(BrowserError::Primitive(format!(
                    "adapter does not support physical capability {required_capability} revision 1"
                )));
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
        workspace: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.call_inner(workspace, command, deadline, cancelled)
    }

    fn publish_control_state(&self, state: RuntimeControlState) -> Result<(), BrowserError> {
        *lock(&self.control_state) = state;
        let writer = lock(&self.connection)
            .as_ref()
            .map(|connection| Arc::clone(&connection.writer));
        if let Some(writer) = writer {
            write_native(&mut *lock(&writer), &BrowserFrame::ControlState { state })
                .map_err(|_| BrowserError::DisconnectedAfterDispatch)?;
        }
        Ok(())
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
    connections: Arc<Mutex<Option<Connection>>>,
    connection_id: String,
    liveness: Option<Arc<Mutex<ConnectionLiveness>>>,
) {
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
                let is_current = lock(&connections)
                    .as_ref()
                    .is_some_and(|connection| connection.id == connection_id);
                if is_current {
                    if let Some(sink) = &sink {
                        sink.on_event(event);
                    }
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
                let was_current = {
                    let mut connection = lock(&connections);
                    if connection
                        .as_ref()
                        .is_some_and(|connection| connection.id == connection_id)
                    {
                        connection.take();
                        true
                    } else {
                        false
                    }
                };
                if was_current {
                    if let Some(sink) = &sink {
                        sink.on_event(BrowserEvent::Disconnected);
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
    connections: Arc<Mutex<Option<Connection>>>,
    connection_id: String,
    settings: HeartbeatSettings,
) {
    loop {
        thread::sleep(settings.interval);
        let is_current = lock(&connections)
            .as_ref()
            .is_some_and(|connection| connection.id == connection_id);
        if !is_current {
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
    /// Browser adapter protocol major is incompatible.
    #[error("browser adapter protocol major {offered} is incompatible with required {required}")]
    Incompatible { offered: u16, required: u16 },
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
        BrowserReceipt, ADAPTER_PROTOCOL_MAJOR,
    };
    use ghostlight_bridge::framing::{read_native, write_native};

    use super::{
        adapter_error, lock, BrowserError, BrowserPort, HeartbeatSettings, RelayBrowserPort,
    };

    fn capability(name: &str) -> AdapterCapability {
        AdapterCapability {
            name: name.into(),
            revision: 1,
        }
    }

    fn announce_adapter(stream: &mut TcpStream, capabilities: Vec<AdapterCapability>) {
        write_native(
            stream,
            &BrowserFrame::Hello {
                major: ADAPTER_PROTOCOL_MAJOR,
                adapter_version: "1.0.0".into(),
                browser_id: "browser_test".into(),
                adapter_epoch: "adapter_test".into(),
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
            lock(&port.connection).is_some(),
            "the relay socket is still attached"
        );
        assert_eq!(
            port.call(
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
                "workspace_test",
                BrowserCommand::ListTabs,
                Instant::now() + Duration::from_millis(75),
                &AtomicBool::new(false),
            ),
            Err(BrowserError::DeadlineAfterDispatch)
        );
        assert!(!port.is_connected());
        assert!(
            lock(&port.connection).is_some(),
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
                    .is_some_and(|(_, started)| started.elapsed() >= Duration::from_millis(125))
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
        let port =
            RelayBrowserPort::with_heartbeat_settings("service_test".into(), short_heartbeat());
        port.attach(stream).unwrap();

        assert_eq!(
            port.call(
                "workspace_test",
                BrowserCommand::ListTabs,
                Instant::now() + Duration::from_millis(500),
                &AtomicBool::new(false),
            ),
            Ok(BrowserOutcome::Tabs { tabs: vec![] })
        );
        assert!(port.is_connected());
        release.send(()).unwrap();
        client.join().unwrap();
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Mutex, MutexGuard};
    use std::time::Instant;

    use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome, RuntimeControlState};

    use super::{BrowserError, BrowserPort};

    /// Deterministic browser port for executor contract tests.
    #[derive(Debug, Default)]
    pub struct FakeBrowser {
        calls: Mutex<Vec<BrowserCommand>>,
        outcomes: Mutex<VecDeque<Result<BrowserOutcome, BrowserError>>>,
        control_states: Mutex<Vec<RuntimeControlState>>,
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
    }

    impl BrowserPort for FakeBrowser {
        fn call(
            &self,
            _workspace: &str,
            command: BrowserCommand,
            _deadline: Instant,
            _cancelled: &AtomicBool,
        ) -> Result<BrowserOutcome, BrowserError> {
            lock(&self.calls).push(command);
            lock(&self.outcomes)
                .pop_front()
                .unwrap_or_else(|| Err(BrowserError::Primitive("no fake outcome".into())))
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
