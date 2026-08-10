//! The physical browser port and authenticated relay-backed adapter implementation.

use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_bridge::browser::{
    AdapterCapability, BrowserCommand, BrowserEvent, BrowserFrame, BrowserOutcome, BrowserRequest,
    RuntimeControlState, ADAPTER_PROTOCOL_MAJOR,
};
use ghostlight_bridge::framing::{read_native, write_native};
use thiserror::Error;
use uuid::Uuid;

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
}

/// Authenticated loopback implementation of the physical browser port.
pub struct RelayBrowserPort {
    service_epoch: String,
    connection: Arc<Mutex<Option<Connection>>>,
    event_sink: Mutex<Option<Arc<dyn BrowserEventSink>>>,
    control_state: Mutex<RuntimeControlState>,
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
        }
    }

    /// Install the direct typed event reaction target.
    pub fn set_event_sink(&self, sink: Arc<dyn BrowserEventSink>) {
        *lock(&self.event_sink) = Some(sink);
    }

    /// Whether a compatible adapter is currently connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        lock(&self.connection).is_some()
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
        };
        if let Some(previous) = lock(&self.connection).replace(connection) {
            fail_pending(&previous.pending, BrowserError::DisconnectedAfterDispatch);
        }
        let sink = lock(&self.event_sink).clone();
        let connections = Arc::clone(&self.connection);
        thread::Builder::new()
            .name("ghostlight-browser-reader".into())
            .spawn(move || read_adapter(reader, writer, pending, sink, connections, connection_id))
            .map_err(|error| BrowserError::Protocol(error.to_string()))?;
        Ok(())
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
        if cancelled.load(Ordering::SeqCst) {
            return Err(BrowserError::CancelledBeforeDispatch);
        }
        if Instant::now() >= deadline {
            return Err(BrowserError::DeadlineBeforeDispatch);
        }
        let correlation = format!("physical_{}", Uuid::new_v4().simple());
        let (sender, receiver) = mpsc::channel();
        let required_capability = command.required_capability();
        let (writer, pending) = {
            let connection = lock(&self.connection);
            let Some(connection) = connection.as_ref() else {
                return Err(BrowserError::DisconnectedBeforeDispatch);
            };
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
            )
        };
        lock(&pending).insert(correlation.clone(), sender);
        let frame = BrowserFrame::Request {
            request: BrowserRequest {
                correlation: correlation.clone(),
                workspace: workspace.into(),
                command,
            },
        };
        if write_native(&mut *lock(&writer), &frame).is_err() {
            lock(&pending).remove(&correlation);
            return Err(BrowserError::DisconnectedAfterDispatch);
        }
        await_receipt(
            receiver,
            &correlation,
            &writer,
            &pending,
            deadline,
            cancelled,
        )
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

fn await_receipt(
    receiver: Receiver<PendingResult>,
    correlation: &str,
    writer: &Arc<Mutex<TcpStream>>,
    pending: &Arc<Mutex<HashMap<String, Sender<PendingResult>>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
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
    use std::thread;

    use ghostlight_bridge::browser::{BrowserFrame, ADAPTER_PROTOCOL_MAJOR};
    use ghostlight_bridge::framing::write_native;

    use super::{adapter_error, BrowserError, RelayBrowserPort};

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
