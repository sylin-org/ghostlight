// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The MCP edge's typed service client and request-to-work correlation.
//!
//! The service wire is protocol-neutral. This module remembers only enough MCP request identity to
//! route a later semantic outcome back to the selected date-named handler. It does not retain
//! results, replay work, or create a second work platform.

use crate::jsonrpc::RequestId;
use ghostlight_transport::bridge::{
    self as wire, BridgeError, BridgeSequence, CatalogProjection, EdgeMessage, ServiceMessage,
    TerminalOutcome, WorkId, WorkspaceId, BRIDGE_MAJOR,
};
use ghostlight_transport::operation::{BrowserResult, OperationKind};
use ghostlight_transport::{ipc, supervisor};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::time::{sleep, Duration};

/// Outputs and service messages produced by one state-machine transition.
#[derive(Default)]
pub struct Effects {
    /// JSON-RPC envelopes to write in order.
    pub output: Vec<Value>,
    /// Protocol-neutral messages to send to the service in order.
    pub service: Vec<EdgeMessage>,
}

impl Effects {
    /// Produce one JSON-RPC envelope.
    pub fn output(value: Value) -> Self {
        Self {
            output: vec![value],
            service: Vec::new(),
        }
    }

    /// Produce one service message.
    pub fn service(message: EdgeMessage) -> Self {
        Self {
            output: Vec::new(),
            service: vec![message],
        }
    }

    /// Append another transition's outputs while preserving order.
    pub fn extend(&mut self, mut other: Self) {
        self.output.append(&mut other.output);
        self.service.append(&mut other.service);
    }
}

/// Why one sequence is pending. These are routing tokens, not protocol implementations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingKind {
    /// Open the implicit 2025 workspace during initialization.
    OpenWorkspace2025,
    /// Reattach the retained 2025 workspace or replace it after a service restart.
    ReopenWorkspace2025,
    /// Fetch the projection needed to complete 2025 initialization.
    InitializeCatalog2025,
    /// Refresh the 2025 projection after the implicit workspace reconnects.
    ReconnectCatalog2025,
    /// Fetch a 2025 tools list after cache invalidation.
    ListTools2025,
    /// Execute a 2025 tool call.
    CallTool2025,
    /// Fetch one request-scoped 2026 tools list.
    ListTools2026,
    /// Execute a 2026 tool call.
    CallTool2026 {
        /// Whether the service identified this as a context-creating operation.
        context_creating: bool,
    },
    /// Cleanly release the implicit 2025 workspace.
    ReleaseWorkspace2025,
}

/// Correlation retained while one bridge operation is unresolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRequest {
    /// MCP request id, absent for background reconnect work and clean workspace release.
    pub request_id: Option<RequestId>,
    /// Handler transition to resume.
    pub kind: PendingKind,
    /// Workspace reported when the service accepted a tool call.
    pub service_workspace: Option<WorkspaceId>,
    /// Workspace placed on the immutable matching Start request.
    pub requested_workspace: Option<WorkspaceId>,
    /// Canonical identity sent in the matching start request.
    ///
    /// A terminal success must carry this same identity before any profile renderer runs.
    pub expected_operation: Option<OperationKind>,
    /// Whether the client cancelled and no response may be written.
    pub suppressed: bool,
    /// Whether the complete request frame was flushed to the service.
    pub delivered: bool,
}

impl PendingRequest {
    /// Construct a response-bearing pending operation.
    pub fn request(request_id: RequestId, kind: PendingKind) -> Self {
        Self {
            request_id: Some(request_id),
            kind,
            service_workspace: None,
            requested_workspace: None,
            expected_operation: None,
            suppressed: false,
            delivered: false,
        }
    }

    /// Construct a response-bearing tool call with its immutable surface invocation.
    pub fn tool_request(
        request_id: RequestId,
        kind: PendingKind,
        expected_operation: OperationKind,
        requested_workspace: Option<WorkspaceId>,
    ) -> Self {
        Self {
            request_id: Some(request_id),
            kind,
            service_workspace: None,
            requested_workspace,
            expected_operation: Some(expected_operation),
            suppressed: false,
            delivered: false,
        }
    }

    /// Construct the response-free clean-release operation.
    pub fn release() -> Self {
        Self {
            request_id: None,
            kind: PendingKind::ReleaseWorkspace2025,
            service_workspace: None,
            requested_workspace: None,
            expected_operation: None,
            suppressed: false,
            delivered: false,
        }
    }

    /// Construct response-free background reconnect work.
    pub fn background(kind: PendingKind) -> Self {
        Self {
            request_id: None,
            kind,
            service_workspace: None,
            requested_workspace: None,
            expected_operation: None,
            suppressed: false,
            delivered: false,
        }
    }

    /// Return whether a successful service result matches the operation sent for this request.
    pub fn result_matches_expected_operation(&self, result: &BrowserResult) -> bool {
        self.expected_operation == Some(result.operation)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Location {
    Sequence(BridgeSequence),
    Work(WorkId),
}

/// Resolution of a correlated service message.
#[derive(Debug)]
pub enum Resolution {
    /// An implicit workspace was opened.
    WorkspaceOpened {
        /// Original pending operation.
        pending: PendingRequest,
        /// Service-minted handle.
        workspace: WorkspaceId,
    },
    /// A clean workspace release completed.
    WorkspaceReleased,
    /// A catalog projection arrived.
    Catalog {
        /// Original pending operation.
        pending: PendingRequest,
        /// Service-owned projection.
        projection: CatalogProjection,
    },
    /// The service rejected an operation before it started.
    Rejected {
        /// Original pending operation.
        pending: PendingRequest,
        /// Semantic rejection.
        error: BridgeError,
    },
    /// Accepted work reached a terminal semantic outcome.
    Completed {
        /// Original pending operation.
        pending: PendingRequest,
        /// Service-owned terminal outcome.
        outcome: TerminalOutcome,
    },
}

/// Meaningful observation after consuming one service message.
pub enum Observation {
    /// No edge-visible transition is needed.
    None,
    /// One correlated operation resolved.
    Resolved(Resolution),
    /// A cancelled pre-start request was accepted and must now receive bridge cancellation.
    Cancel(EdgeMessage),
    /// The service catalog generation changed.
    CatalogChanged(u64),
}

/// One pending operation retired because its service stream disappeared.
pub struct DisconnectedPending {
    /// Correlation token.
    pub pending: PendingRequest,
    /// Whether the service had already returned a work id.
    pub may_have_started: bool,
}

/// The one per-edge correlation table required for cancellation and response routing.
#[derive(Default)]
pub struct Correlation {
    next_sequence: u64,
    by_sequence: HashMap<BridgeSequence, PendingRequest>,
    by_work: HashMap<WorkId, PendingRequest>,
    by_request: HashMap<RequestId, Location>,
}

impl Correlation {
    /// Whether a response-bearing bridge operation already owns this request id.
    pub fn contains_request(&self, request_id: &RequestId) -> bool {
        self.by_request.contains_key(request_id)
    }

    /// Register a new bridge operation and return its stream-local sequence.
    pub fn track(&mut self, pending: PendingRequest) -> Result<BridgeSequence, &'static str> {
        if pending
            .request_id
            .as_ref()
            .is_some_and(|id| self.by_request.contains_key(id))
        {
            return Err("a request with this id is already active");
        }
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
        let sequence = BridgeSequence(self.next_sequence);
        if let Some(id) = pending.request_id.clone() {
            self.by_request.insert(id, Location::Sequence(sequence));
        }
        self.by_sequence.insert(sequence, pending);
        Ok(sequence)
    }

    /// Retire a sequence whose service message could not be written.
    pub fn take_unsent(&mut self, sequence: BridgeSequence) -> Option<PendingRequest> {
        let pending = self.by_sequence.remove(&sequence)?;
        if let Some(id) = &pending.request_id {
            self.by_request.remove(id);
        }
        Some(pending)
    }

    /// Record that a complete pre-start bridge frame was flushed to the service.
    pub fn mark_sent(&mut self, sequence: BridgeSequence) {
        if let Some(pending) = self.by_sequence.get_mut(&sequence) {
            pending.delivered = true;
        }
    }

    /// Cooperatively cancel an active request id.
    ///
    /// A pre-start sequence is marked suppressed; if the service later accepts it, `observe`
    /// immediately emits `Cancel`. An already-started operation can be cancelled now.
    pub fn cancel(&mut self, request_id: &RequestId) -> Option<EdgeMessage> {
        match self.by_request.get(request_id).copied()? {
            Location::Sequence(sequence) => {
                if let Some(pending) = self.by_sequence.get_mut(&sequence) {
                    pending.suppressed = true;
                }
                None
            }
            Location::Work(work_id) => {
                if let Some(pending) = self.by_work.get_mut(&work_id) {
                    pending.suppressed = true;
                }
                Some(EdgeMessage::Cancel { work_id })
            }
        }
    }

    /// Consume one service message and advance correlation.
    pub fn observe(&mut self, message: ServiceMessage) -> Observation {
        match message {
            ServiceMessage::Hello { .. } => Observation::None,
            ServiceMessage::CatalogChanged { generation } => {
                Observation::CatalogChanged(generation)
            }
            ServiceMessage::WorkspaceOpened {
                sequence,
                workspace,
            } => self
                .finish_sequence(sequence)
                .map(|pending| {
                    Observation::Resolved(Resolution::WorkspaceOpened { pending, workspace })
                })
                .unwrap_or(Observation::None),
            ServiceMessage::WorkspaceReleased { sequence } => {
                if self.finish_sequence(sequence).is_some() {
                    Observation::Resolved(Resolution::WorkspaceReleased)
                } else {
                    Observation::None
                }
            }
            ServiceMessage::Catalog {
                sequence,
                projection,
            } => self
                .finish_sequence(sequence)
                .map(|pending| {
                    Observation::Resolved(Resolution::Catalog {
                        pending,
                        projection,
                    })
                })
                .unwrap_or(Observation::None),
            ServiceMessage::Rejected { sequence, error } => self
                .finish_sequence(sequence)
                .map(|pending| Observation::Resolved(Resolution::Rejected { pending, error }))
                .unwrap_or(Observation::None),
            ServiceMessage::Started {
                sequence,
                work_id,
                workspace,
                context_creating,
            } => {
                let Some(mut pending) = self.by_sequence.remove(&sequence) else {
                    return Observation::None;
                };
                pending.service_workspace = workspace;
                if let PendingKind::CallTool2026 {
                    context_creating: pending_context_creating,
                } = &mut pending.kind
                {
                    *pending_context_creating = context_creating;
                }
                if let Some(id) = pending.request_id.clone() {
                    self.by_request.insert(id, Location::Work(work_id));
                }
                let cancelled = pending.suppressed;
                self.by_work.insert(work_id, pending);
                if cancelled {
                    Observation::Cancel(EdgeMessage::Cancel { work_id })
                } else {
                    Observation::None
                }
            }
            ServiceMessage::Completed { work_id, outcome } => {
                let Some(pending) = self.by_work.remove(&work_id) else {
                    return Observation::None;
                };
                if let Some(id) = &pending.request_id {
                    self.by_request.remove(id);
                }
                Observation::Resolved(Resolution::Completed { pending, outcome })
            }
        }
    }

    /// Retire every correlation after a bridge disconnect without replaying any operation.
    pub fn disconnect(&mut self) -> Vec<DisconnectedPending> {
        self.by_request.clear();
        let mut pending = Vec::with_capacity(self.by_sequence.len() + self.by_work.len());
        pending.extend(self.by_sequence.drain().map(|(_, pending)| {
            let may_have_started = pending.delivered
                && matches!(
                    pending.kind,
                    PendingKind::CallTool2025 | PendingKind::CallTool2026 { .. }
                );
            DisconnectedPending {
                pending,
                may_have_started,
            }
        }));
        pending.extend(
            self.by_work
                .drain()
                .map(|(_, pending)| DisconnectedPending {
                    pending,
                    may_have_started: true,
                }),
        );
        pending
    }

    fn finish_sequence(&mut self, sequence: BridgeSequence) -> Option<PendingRequest> {
        let pending = self.by_sequence.remove(&sequence)?;
        if let Some(id) = &pending.request_id {
            self.by_request.remove(id);
        }
        Some(pending)
    }
}

/// Extract a pre-start sequence from an outbound bridge message.
pub fn sequence_of(message: &EdgeMessage) -> Option<BridgeSequence> {
    match message {
        EdgeMessage::OpenWorkspace { sequence, .. }
        | EdgeMessage::ReleaseWorkspace { sequence, .. }
        | EdgeMessage::Catalog { sequence, .. }
        | EdgeMessage::Start { sequence, .. } => Some(*sequence),
        EdgeMessage::Hello { .. } | EdgeMessage::Cancel { .. } => None,
    }
}

struct Outbound {
    generation: u64,
    message: EdgeMessage,
    written: oneshot::Sender<Result<(), SendFailure>>,
}

const WRITE_SETTLEMENT_TIMEOUT: Duration = Duration::from_secs(35);

/// Truthful disposition when an edge-to-service bridge message does not settle successfully.
#[derive(Debug, PartialEq, Eq)]
pub enum SendFailure {
    /// The bridge proved that no byte of this message was written on a compatible stream.
    NotWritten(String),
    /// Some or all of the frame may have reached the service, so an effect cannot be retried.
    PossiblyWritten(String),
}

impl SendFailure {
    /// Return the human-readable transport detail.
    pub fn reason(&self) -> &str {
        match self {
            Self::NotWritten(reason) | Self::PossiblyWritten(reason) => reason,
        }
    }

    /// Whether the service may have received enough of the message to act on it.
    pub fn possibly_written(&self) -> bool {
        matches!(self, Self::PossiblyWritten(_))
    }
}

/// Events delivered by the reconnecting bridge task.
pub enum BridgeEvent {
    /// A bridge-major-compatible stream is ready.
    Connected,
    /// One complete service message arrived.
    Message(ServiceMessage),
    /// The current stream ended. Pending work must be failed without replay.
    Disconnected(String),
    /// A permanent local trust failure for which reconnecting would be unsafe.
    Fatal(String),
}

/// A reconnecting client for future calls. It never replays a message from an old stream.
pub struct BridgeClient {
    outbound: mpsc::Sender<Outbound>,
    events: mpsc::Receiver<BridgeEvent>,
    generation: Arc<AtomicU64>,
    ready: Arc<Notify>,
}

impl BridgeClient {
    /// Start a bridge task targeting one owner-only service endpoint.
    pub fn spawn(endpoint: String) -> Self {
        let (outbound, outbound_rx) = mpsc::channel(64);
        let (event_tx, events) = mpsc::channel(128);
        let generation = Arc::new(AtomicU64::new(0));
        let ready = Arc::new(Notify::new());
        tokio::spawn(bridge_loop(
            endpoint,
            outbound_rx,
            event_tx,
            Arc::clone(&generation),
            Arc::clone(&ready),
        ));
        Self {
            outbound,
            events,
            generation,
            ready,
        }
    }

    /// Wait until the first compatible service stream is ready.
    pub async fn wait_ready(&self) {
        loop {
            if self.generation.load(Ordering::Acquire) != 0 {
                return;
            }
            self.ready.notified().await;
        }
    }

    /// Send one message on the current stream and wait until its full frame is flushed.
    pub async fn send(&self, message: EdgeMessage) -> Result<(), SendFailure> {
        self.send_with_settlement_timeout(message, WRITE_SETTLEMENT_TIMEOUT)
            .await
    }

    async fn send_with_settlement_timeout(
        &self,
        message: EdgeMessage,
        settlement_timeout: Duration,
    ) -> Result<(), SendFailure> {
        if self.generation.load(Ordering::Acquire) == 0 {
            tokio::time::timeout(supervisor::SELF_HEAL_RETRY_WINDOW, self.wait_ready())
                .await
                .map_err(|_| {
                    SendFailure::NotWritten(
                        "timed out connecting to the Ghostlight service bridge".to_string(),
                    )
                })?;
        }
        let generation = self.generation.load(Ordering::Acquire);
        let (written, result) = oneshot::channel();
        self.outbound
            .send(Outbound {
                generation,
                message,
                written,
            })
            .await
            .map_err(|_| {
                SendFailure::NotWritten("Ghostlight service bridge has stopped".to_string())
            })?;
        match tokio::time::timeout(settlement_timeout, result).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(SendFailure::PossiblyWritten(
                "Ghostlight service bridge ended before the write settled".to_string(),
            )),
            Err(_) => Err(SendFailure::PossiblyWritten(
                "timed out waiting for the Ghostlight service bridge write to settle".to_string(),
            )),
        }
    }

    /// Receive the next connection or service event.
    pub async fn next_event(&mut self) -> Option<BridgeEvent> {
        self.events.recv().await
    }
}

async fn bridge_loop(
    endpoint: String,
    mut outbound: mpsc::Receiver<Outbound>,
    events: mpsc::Sender<BridgeEvent>,
    generation: Arc<AtomicU64>,
    ready: Arc<Notify>,
) {
    let mut next_generation = 0_u64;
    let mut start_requested = false;
    loop {
        let stream = match ipc::connect_mcp_edge(&endpoint).await {
            Ok(stream) => {
                start_requested = false;
                stream
            }
            Err(error) => {
                generation.store(0, Ordering::Release);
                let rendered = error.to_string();
                if rendered.contains(ghostlight_transport::antisquat::REFUSAL_MESSAGE) {
                    let _ = events.send(BridgeEvent::Fatal(rendered)).await;
                    return;
                }
                if !start_requested {
                    supervisor::start_service();
                    start_requested = true;
                }
                let _ = events.send(BridgeEvent::Disconnected(rendered)).await;
                sleep(supervisor::SELF_HEAL_RETRY_INTERVAL).await;
                continue;
            }
        };
        next_generation = next_generation.wrapping_add(1).max(1);
        let result = run_connected(
            stream,
            next_generation,
            &mut outbound,
            &events,
            &generation,
            &ready,
            WRITE_SETTLEMENT_TIMEOUT,
        )
        .await;
        generation.store(0, Ordering::Release);
        let _ = events.send(BridgeEvent::Disconnected(result)).await;
        sleep(Duration::from_millis(250)).await;
    }
}

async fn run_connected<S>(
    mut stream: S,
    stream_generation: u64,
    outbound: &mut mpsc::Receiver<Outbound>,
    events: &mpsc::Sender<BridgeEvent>,
    generation: &AtomicU64,
    ready: &Notify,
    write_timeout: Duration,
) -> String
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    if let Err(error) = wire::write_edge_message(
        &mut stream,
        &EdgeMessage::Hello {
            bridge_major: BRIDGE_MAJOR,
        },
    )
    .await
    {
        return error.to_string();
    }
    match wire::read_service_message(&mut stream).await {
        Ok(Some(ServiceMessage::Hello { bridge_major })) if bridge_major == BRIDGE_MAJOR => {}
        Ok(Some(ServiceMessage::Hello { bridge_major })) => {
            return format!("Ghostlight service selected unsupported bridge major {bridge_major}");
        }
        Ok(Some(_)) => return "Ghostlight service did not begin with a bridge hello".into(),
        Ok(None) => return "Ghostlight service closed during the bridge hello".into(),
        Err(error) => return error.to_string(),
    }

    generation.store(stream_generation, Ordering::Release);
    // `notify_one` stores a permit if the runtime has not reached `wait_ready` yet. That closes
    // the check/register race that `notify_waiters` would leave during a very fast local connect.
    ready.notify_one();
    if events.send(BridgeEvent::Connected).await.is_err() {
        return "MCP runtime closed".into();
    }

    let (mut reader, mut writer) = tokio::io::split(stream);
    let (closed_tx, mut closed_rx) = mpsc::channel(1);
    let reader_events = events.clone();
    let reader_task = tokio::spawn(async move {
        let reason = loop {
            match wire::read_service_message(&mut reader).await {
                Ok(Some(message)) => {
                    if reader_events
                        .send(BridgeEvent::Message(message))
                        .await
                        .is_err()
                    {
                        break "MCP runtime closed".to_string();
                    }
                }
                Ok(None) => break "Ghostlight service closed the bridge".to_string(),
                Err(error) => break error.to_string(),
            }
        };
        let _ = closed_tx.send(reason).await;
    });

    let reason = loop {
        tokio::select! {
            closed = closed_rx.recv() => {
                break closed.unwrap_or_else(|| "bridge reader stopped".into());
            }
            command = outbound.recv() => {
                let Some(command) = command else {
                    break "MCP runtime closed".into();
                };
                if command.written.is_closed() {
                    continue;
                }
                if command.generation != stream_generation {
                    let _ = command.written.send(Err(SendFailure::NotWritten(
                        "bridge reconnected before this request was written".into(),
                    )));
                    continue;
                }
                let write = wire::write_edge_message(&mut writer, &command.message);
                tokio::pin!(write);
                let write_result = tokio::select! {
                    result = &mut write => result.map_err(|error| error.to_string()),
                    closed = closed_rx.recv() => {
                        let reason = closed.unwrap_or_else(|| "bridge reader stopped".into());
                        let _ = command.written.send(Err(SendFailure::PossiblyWritten(
                            reason.clone(),
                        )));
                        break reason;
                    }
                    _ = sleep(write_timeout) => {
                        let reason =
                            "timed out writing to the Ghostlight service bridge".to_string();
                        let _ = command.written.send(Err(SendFailure::PossiblyWritten(
                            reason.clone(),
                        )));
                        break reason;
                    }
                };
                match write_result {
                    Ok(()) => {
                        let _ = command.written.send(Ok(()));
                    }
                    Err(rendered) => {
                        let _ = command
                            .written
                            .send(Err(SendFailure::PossiblyWritten(rendered.clone())));
                        break rendered;
                    }
                }
            }
        }
    };
    reader_task.abort();
    reason
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::bridge::RequestContext;
    use ghostlight_transport::operation::{
        BrowserResult, BrowserResultStatus, NavigateArguments, Operation, OperationEffect,
        OperationKind,
    };

    fn request_id(value: i64) -> RequestId {
        RequestId::Number(value.into())
    }

    fn test_message() -> EdgeMessage {
        EdgeMessage::Catalog {
            sequence: BridgeSequence(1),
            workspace: None,
            context: RequestContext::default(),
        }
    }

    fn navigate_operation(arguments: Value) -> Operation {
        Operation::BrowserNavigate(NavigateArguments {
            url: arguments["url"].as_str().expect("test URL").to_string(),
            tab: None,
        })
    }

    fn empty_result() -> BrowserResult {
        BrowserResult::new(
            OperationKind::BrowserGetStatus,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        )
    }

    #[tokio::test]
    async fn new_edge_rejects_an_old_service_before_marking_the_stream_ready() {
        let (edge_stream, mut service_stream) = tokio::io::duplex(1024);
        let service = tokio::spawn(async move {
            let hello = wire::read_edge_message(&mut service_stream)
                .await
                .expect("read edge hello")
                .expect("edge hello frame");
            assert!(matches!(
                hello,
                EdgeMessage::Hello {
                    bridge_major: BRIDGE_MAJOR
                }
            ));
            wire::write_service_message(
                &mut service_stream,
                &ServiceMessage::Hello { bridge_major: 1 },
            )
            .await
            .expect("write old service hello");
        });

        let (_outbound_tx, mut outbound_rx) = mpsc::channel(1);
        let (event_tx, mut events) = mpsc::channel(1);
        let generation = AtomicU64::new(0);
        let ready = Notify::new();
        let reason = run_connected(
            edge_stream,
            1,
            &mut outbound_rx,
            &event_tx,
            &generation,
            &ready,
            Duration::from_millis(20),
        )
        .await;

        assert_eq!(
            reason,
            "Ghostlight service selected unsupported bridge major 1"
        );
        assert_eq!(generation.load(Ordering::Acquire), 0);
        assert!(events.try_recv().is_err());
        service.await.expect("old service task");
    }

    #[tokio::test]
    async fn queued_write_settlement_is_bounded_without_a_reconnect() {
        let (outbound, mut queued) = mpsc::channel(1);
        let (_event_tx, events) = mpsc::channel(1);
        let client = BridgeClient {
            outbound,
            events,
            generation: Arc::new(AtomicU64::new(7)),
            ready: Arc::new(Notify::new()),
        };

        let failure = client
            .send_with_settlement_timeout(test_message(), Duration::from_millis(10))
            .await
            .expect_err("an unconsumed queued write must time out");
        assert!(matches!(failure, SendFailure::PossiblyWritten(_)));

        let abandoned = queued.recv().await.expect("queued outbound command");
        assert!(
            abandoned.written.is_closed(),
            "a timed-out queued command must be skippable and never execute later"
        );
    }

    #[tokio::test]
    async fn in_flight_write_is_bounded_and_ambiguous() {
        let (edge_stream, mut service_stream) = tokio::io::duplex(64);
        let service = tokio::spawn(async move {
            let hello = wire::read_edge_message(&mut service_stream)
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(hello, EdgeMessage::Hello { .. }));
            wire::write_service_message(
                &mut service_stream,
                &ServiceMessage::Hello {
                    bridge_major: BRIDGE_MAJOR,
                },
            )
            .await
            .unwrap();
            sleep(Duration::from_secs(1)).await;
        });

        let (outbound_tx, mut outbound_rx) = mpsc::channel(1);
        let (written, written_result) = oneshot::channel();
        outbound_tx
            .send(Outbound {
                generation: 1,
                message: EdgeMessage::Start {
                    sequence: BridgeSequence(1),
                    operation: navigate_operation(serde_json::json!({ "url": "x".repeat(4096) })),
                    workspace: None,
                    context: RequestContext::default(),
                },
                written,
            })
            .await
            .unwrap();
        let (event_tx, _events) = mpsc::channel(4);
        let generation = AtomicU64::new(0);
        let ready = Notify::new();

        let reason = run_connected(
            edge_stream,
            1,
            &mut outbound_rx,
            &event_tx,
            &generation,
            &ready,
            Duration::from_millis(20),
        )
        .await;
        assert!(reason.contains("timed out writing"));
        assert!(matches!(
            written_result.await.unwrap(),
            Err(SendFailure::PossiblyWritten(_))
        ));
        service.abort();
    }

    #[test]
    fn started_rebinds_request_id_to_work_id_and_completion_retires_it() {
        let mut correlation = Correlation::default();
        let id = request_id(7);
        let sequence = correlation
            .track(PendingRequest::request(
                id.clone(),
                PendingKind::CallTool2025,
            ))
            .unwrap();
        assert!(matches!(
            correlation.observe(ServiceMessage::Started {
                sequence,
                work_id: WorkId(9),
                workspace: None,
                context_creating: false,
            }),
            Observation::None
        ));
        assert!(matches!(
            correlation.observe(ServiceMessage::Completed {
                work_id: WorkId(9),
                outcome: TerminalOutcome {
                    result: Box::new(empty_result())
                },
            }),
            Observation::Resolved(Resolution::Completed { .. })
        ));
        assert!(correlation.cancel(&id).is_none());
    }

    #[test]
    fn started_carries_context_creation_without_teaching_the_edge_the_registry() {
        let mut correlation = Correlation::default();
        let workspace = WorkspaceId::mint();
        let sequence = correlation
            .track(PendingRequest::request(
                request_id(11),
                PendingKind::CallTool2026 {
                    context_creating: false,
                },
            ))
            .unwrap();
        correlation.observe(ServiceMessage::Started {
            sequence,
            work_id: WorkId(12),
            workspace: Some(workspace.clone()),
            context_creating: true,
        });
        let Observation::Resolved(Resolution::Completed { pending, .. }) =
            correlation.observe(ServiceMessage::Completed {
                work_id: WorkId(12),
                outcome: TerminalOutcome {
                    result: Box::new(empty_result()),
                },
            })
        else {
            panic!("completed resolution expected");
        };
        assert_eq!(pending.service_workspace, Some(workspace));
        assert_eq!(
            pending.kind,
            PendingKind::CallTool2026 {
                context_creating: true
            }
        );
    }

    #[test]
    fn pre_start_cancellation_is_forwarded_immediately_after_started() {
        let mut correlation = Correlation::default();
        let id = request_id(8);
        let sequence = correlation
            .track(PendingRequest::request(
                id.clone(),
                PendingKind::CallTool2025,
            ))
            .unwrap();
        assert!(correlation.cancel(&id).is_none());
        assert!(matches!(
            correlation.observe(ServiceMessage::Started {
                sequence,
                work_id: WorkId(10),
                workspace: None,
                context_creating: false,
            }),
            Observation::Cancel(EdgeMessage::Cancel {
                work_id: WorkId(10)
            })
        ));
    }

    #[test]
    fn disconnect_distinguishes_unstarted_from_outcome_unknown_work() {
        let mut correlation = Correlation::default();
        let sequence = correlation
            .track(PendingRequest::request(
                request_id(1),
                PendingKind::CallTool2025,
            ))
            .unwrap();
        let second = correlation
            .track(PendingRequest::request(
                request_id(2),
                PendingKind::CallTool2025,
            ))
            .unwrap();
        correlation.observe(ServiceMessage::Started {
            sequence: second,
            work_id: WorkId(2),
            workspace: None,
            context_creating: false,
        });
        let drained = correlation.disconnect();
        assert_eq!(drained.len(), 2);
        assert_eq!(
            drained
                .iter()
                .filter(|pending| pending.may_have_started)
                .count(),
            1
        );
        assert!(correlation.take_unsent(sequence).is_none());
    }

    #[test]
    fn flushed_start_is_outcome_unknown_even_before_started_arrives() {
        let mut correlation = Correlation::default();
        let sequence = correlation
            .track(PendingRequest::request(
                request_id(3),
                PendingKind::CallTool2026 {
                    context_creating: false,
                },
            ))
            .unwrap();
        correlation.mark_sent(sequence);
        let drained = correlation.disconnect();
        assert_eq!(drained.len(), 1);
        assert!(drained[0].may_have_started);
    }

    #[test]
    fn outbound_bridge_vocabulary_remains_protocol_neutral() {
        let message = EdgeMessage::Catalog {
            sequence: BridgeSequence(3),
            workspace: None,
            context: RequestContext::default(),
        };
        let rendered = serde_json::to_string(&message).unwrap();
        assert!(!rendered.contains("jsonrpc"));
        assert!(!rendered.contains("2025"));
        assert!(!rendered.contains("2026"));
    }
}
