// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Typed owner-only messages between the MCP edge and the persistent Ghostlight service.
//!
//! This vocabulary is deliberately protocol-neutral. It carries product operations, immutable
//! request context, workspace handles, catalog projections, and semantic outcomes. JSON-RPC ids,
//! MCP revisions, stdio lifecycle, and client capability bags belong at the edge and must never
//! appear here.

use crate::host;
use crate::operation::{
    BrowserOperation, BrowserResult, IntentId, InvocationPresentation, OperationEffect, OperationId,
};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

pub use crate::workspace_id::WorkspaceId;

/// The only bridge major understood by this build.
pub const BRIDGE_MAJOR: u32 = 2;

/// Payload size above which bridge writes yield between bounded chunks.
pub const BRIDGE_WRITE_YIELD_THRESHOLD: usize = 64 * 1024;

/// Maximum payload bytes written before yielding to other asynchronous work.
pub const BRIDGE_WRITE_CHUNK_SIZE: usize = 64 * 1024;

/// Correlates one edge request until the service accepts or rejects it.
///
/// This value has meaning only on the bridge stream that carried it. Once a [`WorkId`] is
/// returned, the sequence no longer participates in work routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BridgeSequence(pub u64);

/// Identifies one active unit of work on one admitted bridge stream.
///
/// A work id is stream-local and exists only until the corresponding terminal outcome. It is not
/// a durable task id and must not be persisted or rebound to another stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkId(pub u64);

/// Presentation-only client information attached to one immutable request context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientPresentation {
    /// Human-readable client name used for audit presentation only.
    pub name: String,
    /// Human-readable client version used for audit presentation only.
    pub version: String,
}

/// Product facts that may differ between concurrent requests on the same edge stream.
///
/// Neither field grants authority. A restriction can only tighten current service authority and
/// must be validated by the service before use.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestContext {
    /// Optional client name and version for this call's audit presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientPresentation>,
    /// Optional serialized tighten-only restriction supplied for this call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restriction: Option<String>,
}

/// How one canonical tool relates to a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceUse {
    /// The operation does not read or create workspace state.
    Independent,
    /// The operation creates workspace state when no handle is supplied.
    Creates,
    /// The operation requires an existing workspace handle.
    Uses,
}

/// One concrete canonical operation advertised as available by the service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationAvailability {
    /// Canonical operation family.
    pub id: OperationId,
    /// Concrete semantic intent available within the family.
    pub intent: IntentId,
    /// The operation's relationship to workspace state.
    pub workspace_use: WorkspaceUse,
}

/// The canonical service-owned tool catalog projected for one request context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProjection {
    /// Monotonic service-local generation for catalog change detection.
    pub generation: u64,
    /// Ordered concrete canonical operation availability.
    pub operations: Vec<OperationAvailability>,
    /// Whether a tighten-only restriction affected this projection.
    pub restricted: bool,
}

/// Semantic reason a bridge request was rejected before work started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeErrorKind {
    /// The operation or its arguments were invalid.
    InvalidRequest,
    /// A required workspace was absent, unknown, expired, or incompatible.
    InvalidWorkspace,
    /// The supplied tighten-only restriction was invalid or could not be applied.
    Restriction,
    /// A bounded service resource could not admit more work.
    Busy,
    /// The owner-only local transport failed.
    Transport,
    /// The peer requested a bridge major this build does not support.
    UnsupportedBridge,
}

/// A protocol-neutral rejection returned before a [`WorkId`] exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeError {
    /// Machine-readable semantic category.
    pub kind: BridgeErrorKind,
    /// Human-readable explanation that does not disclose bearer material.
    pub message: String,
    /// Optional corrective action for the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_step: Option<String>,
}

/// Source of a pre-dispatch denial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenialSource {
    /// Current governance policy denied the operation.
    Policy,
    /// The always-on sacred-domain boundary denied the operation.
    Sacred,
}

/// The service's semantic account of an accepted unit of work.
///
/// The edge converts this outcome into the selected protocol revision's result envelope. No
/// variant is itself a JSON-RPC response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalOutcome {
    /// The operation completed and produced its canonical product result.
    Success {
        /// Canonical result produced by the operation pipeline.
        result: Box<BrowserResult>,
    },
    /// Tool execution failed conclusively.
    ToolFailure {
        /// Structured failure value when one is available.
        result: Value,
        /// Human-readable failure explanation.
        message: String,
    },
    /// Queue admission failed before browser dispatch, so retry may be safe.
    NotDispatched {
        /// Human-readable admission failure.
        message: String,
    },
    /// Dispatch may have occurred but no conclusive terminal acknowledgement exists.
    OutcomeUnknown {
        /// Human-readable uncertainty explanation.
        message: String,
    },
    /// Governance or the sacred-domain boundary denied the operation before dispatch.
    Denied {
        /// Human-readable denial explanation.
        message: String,
        /// Boundary that produced the denial.
        source: DenialSource,
    },
    /// The take-the-wheel hold prevented dispatch.
    Held {
        /// Human-readable hold explanation.
        message: String,
    },
    /// The workspace denial circuit requires user attention before more work can run.
    AttentionRequired {
        /// Human-readable attention request.
        message: String,
    },
    /// Cooperative cancellation retired the work without a normal result.
    Cancelled {
        /// Truthful cancellation disposition, including uncertainty when an effect may have run.
        message: String,
        /// Proven physical-effect disposition at the cancellation boundary.
        effect: OperationEffect,
    },
}

/// Messages sent from the MCP edge to the persistent service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EdgeMessage {
    /// Establish fail-loud bridge-major compatibility.
    Hello {
        /// Major bridge vocabulary requested by the edge.
        bridge_major: u32,
    },
    /// Mint an implicit workspace for an edge that needs one.
    OpenWorkspace {
        /// Pre-start request correlation sequence.
        sequence: BridgeSequence,
        /// Preferred still-live workspace when a connection-bound shore reconnects.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<WorkspaceId>,
        /// Immutable presentation and tighten-only restriction for the request.
        context: RequestContext,
    },
    /// Cleanly release a workspace owned by this admitted local user.
    ReleaseWorkspace {
        /// Pre-start request correlation sequence.
        sequence: BridgeSequence,
        /// Workspace to release.
        workspace: WorkspaceId,
    },
    /// Request the canonical catalog projection for a context.
    Catalog {
        /// Pre-start request correlation sequence.
        sequence: BridgeSequence,
        /// Optional workspace whose stored state participates in the projection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<WorkspaceId>,
        /// Immutable presentation and tighten-only restriction for the request.
        context: RequestContext,
    },
    /// Ask the service to accept one normalized product operation.
    Start {
        /// Correlates the subsequent `started` or `rejected` response.
        sequence: BridgeSequence,
        /// Canonical operation and arguments without protocol lifecycle metadata.
        operation: BrowserOperation,
        /// Bounded external call facts used only for corrective copy and audit presentation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        presentation: Option<InvocationPresentation>,
        /// Existing workspace, when the operation uses one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<WorkspaceId>,
        /// Immutable presentation and tighten-only restriction for this call.
        context: RequestContext,
    },
    /// Cooperatively cancel one active unit of work on this stream.
    Cancel {
        /// Active stream-local work id to cancel.
        work_id: WorkId,
    },
}

/// Messages sent from the persistent service to the MCP edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServiceMessage {
    /// Confirm fail-loud bridge-major compatibility.
    Hello {
        /// Major bridge vocabulary selected by the service.
        bridge_major: u32,
    },
    /// Return a newly minted implicit workspace.
    WorkspaceOpened {
        /// Sequence from the corresponding open request.
        sequence: BridgeSequence,
        /// Service-minted workspace handle.
        workspace: WorkspaceId,
    },
    /// Confirm clean workspace release.
    WorkspaceReleased {
        /// Sequence from the corresponding release request.
        sequence: BridgeSequence,
    },
    /// Return the service-owned catalog projection.
    Catalog {
        /// Sequence from the corresponding catalog request.
        sequence: BridgeSequence,
        /// Canonical projected catalog.
        projection: CatalogProjection,
    },
    /// Signal that clients with a cached catalog should request a newer generation.
    CatalogChanged {
        /// Current service-local catalog generation.
        generation: u64,
    },
    /// Confirm that one start request now owns an active work id.
    Started {
        /// Sequence from the corresponding start request.
        sequence: BridgeSequence,
        /// Active stream-local work id.
        work_id: WorkId,
        /// Existing or newly minted workspace used by the operation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<WorkspaceId>,
        /// Whether this operation establishes or refreshes a usable workspace context.
        context_creating: bool,
    },
    /// Reject a request before a work id exists.
    Rejected {
        /// Sequence from the rejected request.
        sequence: BridgeSequence,
        /// Semantic rejection detail.
        error: BridgeError,
    },
    /// Complete one previously started unit of work.
    Completed {
        /// Active work id returned by `started`.
        work_id: WorkId,
        /// Protocol-neutral terminal outcome.
        outcome: TerminalOutcome,
    },
}

/// Read and deserialize one edge-to-service bridge message.
///
/// Returns `Ok(None)` only when the peer cleanly closes before another frame begins.
pub async fn read_edge_message<R>(reader: &mut R) -> Result<Option<EdgeMessage>>
where
    R: AsyncRead + Unpin,
{
    read_typed_message(reader).await
}

/// Write and flush one edge-to-service bridge message with bounded cooperative chunking.
pub async fn write_edge_message<W>(writer: &mut W, message: &EdgeMessage) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    write_typed_message(writer, message).await
}

/// Read and deserialize one service-to-edge bridge message.
///
/// Returns `Ok(None)` only when the peer cleanly closes before another frame begins.
pub async fn read_service_message<R>(reader: &mut R) -> Result<Option<ServiceMessage>>
where
    R: AsyncRead + Unpin,
{
    read_typed_message(reader).await
}

/// Write and flush one service-to-edge bridge message with bounded cooperative chunking.
pub async fn write_service_message<W>(writer: &mut W, message: &ServiceMessage) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    write_typed_message(writer, message).await
}

async fn read_typed_message<R, T>(reader: &mut R) -> Result<Option<T>>
where
    R: AsyncRead + Unpin,
    T: for<'de> Deserialize<'de>,
{
    let Some(payload) = host::read_message(reader).await? else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_slice(&payload)?))
}

async fn write_typed_message<W, T>(writer: &mut W, message: &T) -> Result<()>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let payload = serde_json::to_vec(message)?;
    if payload.len() > host::MAX_MESSAGE_LEN as usize {
        return Err(Error::NativeMessaging(format!(
            "bridge payload length {} exceeds MAX_MESSAGE_LEN {}",
            payload.len(),
            host::MAX_MESSAGE_LEN
        )));
    }
    let length = u32::try_from(payload.len())
        .map_err(|_| Error::NativeMessaging("bridge payload exceeds u32 length".into()))?;

    writer
        .write_all(&length.to_le_bytes())
        .await
        .map_err(Error::Io)?;

    let should_yield = payload.len() > BRIDGE_WRITE_YIELD_THRESHOLD;
    let mut chunks = payload.chunks(BRIDGE_WRITE_CHUNK_SIZE).peekable();
    while let Some(chunk) = chunks.next() {
        writer.write_all(chunk).await.map_err(Error::Io)?;
        if should_yield && chunks.peek().is_some() {
            tokio::task::yield_now().await;
        }
    }
    writer.flush().await.map_err(Error::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::{BrowserResultStatus, OperationEffect, ResultPart};
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::AsyncWrite;

    fn request_context() -> RequestContext {
        RequestContext {
            client: Some(ClientPresentation {
                name: "edge-test".into(),
                version: "1.2.3".into(),
            }),
            restriction: Some("read-only".into()),
        }
    }

    fn click_operation(arguments: Value) -> BrowserOperation {
        BrowserOperation::new(OperationId::BrowserAct, IntentId::ActClick, arguments)
    }

    fn successful_click_result() -> BrowserResult {
        let mut result = BrowserResult::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::Ok,
            OperationEffect::Committed,
        );
        result.parts.push(ResultPart::Text {
            text: "clicked".into(),
        });
        result
    }

    #[tokio::test]
    async fn edge_message_round_trips_through_framing() {
        let message = EdgeMessage::Start {
            sequence: BridgeSequence(7),
            operation: click_operation(serde_json::json!({"coordinate": [10, 20]})),
            presentation: Some(
                InvocationPresentation::new(
                    "ghostlight-legacy",
                    1,
                    "computer",
                    Some("left_click".into()),
                )
                .expect("valid presentation"),
            ),
            workspace: Some(WorkspaceId::mint()),
            context: request_context(),
        };
        let mut framed = Vec::new();
        write_edge_message(&mut framed, &message)
            .await
            .expect("write edge message");

        let mut reader: &[u8] = &framed;
        let decoded = read_edge_message(&mut reader)
            .await
            .expect("read edge message")
            .expect("framed message");
        assert_eq!(decoded, message);
        assert!(read_edge_message(&mut reader)
            .await
            .expect("read eof")
            .is_none());
    }

    #[tokio::test]
    async fn service_message_round_trips_through_framing() {
        let message = ServiceMessage::Completed {
            work_id: WorkId(9),
            outcome: TerminalOutcome::Success {
                result: Box::new(successful_click_result()),
            },
        };
        let mut framed = Vec::new();
        write_service_message(&mut framed, &message)
            .await
            .expect("write service message");

        let mut reader: &[u8] = &framed;
        assert_eq!(
            read_service_message(&mut reader)
                .await
                .expect("read service message"),
            Some(message)
        );
    }

    #[test]
    fn tagged_wire_shape_is_exact_and_protocol_neutral() {
        let value = serde_json::to_value(EdgeMessage::Start {
            sequence: BridgeSequence(12),
            operation: click_operation(serde_json::json!({"x": 1})),
            presentation: None,
            workspace: None,
            context: RequestContext::default(),
        })
        .expect("serialize start");

        assert_eq!(
            value,
            serde_json::json!({
                "type": "start",
                "sequence": 12,
                "operation": {
                    "id": "browser.act",
                    "intent": "act.click",
                    "arguments": {"x": 1}
                },
                "context": {}
            })
        );
        let rendered = value.to_string();
        assert!(!rendered.contains("jsonrpc"));
        assert!(!rendered.contains("protocolVersion"));
    }

    #[test]
    fn catalog_projection_contains_availability_not_model_declarations() {
        let projection = CatalogProjection {
            generation: 4,
            operations: vec![
                OperationAvailability {
                    id: OperationId::BrowserTabs,
                    intent: IntentId::TabsList,
                    workspace_use: WorkspaceUse::Creates,
                },
                OperationAvailability {
                    id: OperationId::BrowserAct,
                    intent: IntentId::ActClick,
                    workspace_use: WorkspaceUse::Uses,
                },
            ],
            restricted: true,
        };
        let value = serde_json::to_value(&projection).expect("serialize catalog projection");

        assert_eq!(
            value,
            serde_json::json!({
                "generation": 4,
                "operations": [
                    {
                        "id": "browser.tabs",
                        "intent": "tabs.list",
                        "workspaceUse": "creates"
                    },
                    {
                        "id": "browser.act",
                        "intent": "act.click",
                        "workspaceUse": "uses"
                    }
                ],
                "restricted": true
            })
        );
        let rendered = value.to_string();
        assert!(!rendered.contains("description"));
        assert!(!rendered.contains("inputSchema"));
        assert!(!rendered.contains("instructions"));
    }

    #[tokio::test]
    async fn oversized_frame_is_rejected_before_allocation() {
        let mut framed = (host::MAX_MESSAGE_LEN + 1).to_le_bytes().to_vec();
        let mut reader: &[u8] = &framed;
        let error = read_edge_message(&mut reader)
            .await
            .expect_err("oversized frame must fail");
        assert!(error.to_string().contains("exceeds MAX_MESSAGE_LEN"));
        framed.clear();
    }

    #[tokio::test]
    async fn large_message_writes_one_prefix_then_bounded_payload_chunks() {
        let text = "x".repeat(BRIDGE_WRITE_CHUNK_SIZE * 2 + 17);
        let mut result = successful_click_result();
        result.parts = vec![ResultPart::Text { text }];
        let message = ServiceMessage::Completed {
            work_id: WorkId(3),
            outcome: TerminalOutcome::Success {
                result: Box::new(result),
            },
        };
        let expected_payload = serde_json::to_vec(&message).expect("serialize message");
        assert!(expected_payload.len() > BRIDGE_WRITE_YIELD_THRESHOLD);

        let mut writer = RecordingWriter::default();
        write_service_message(&mut writer, &message)
            .await
            .expect("write large message");

        assert_eq!(writer.writes.first().map(Vec::len), Some(4));
        assert!(
            writer.writes.len() >= 4,
            "prefix plus at least three chunks"
        );
        assert!(writer.writes[1..]
            .iter()
            .all(|chunk| chunk.len() <= BRIDGE_WRITE_CHUNK_SIZE));
        assert_eq!(writer.flushes, 1);

        let length = u32::from_le_bytes(writer.writes[0].clone().try_into().unwrap()) as usize;
        assert_eq!(length, expected_payload.len());
        let joined: Vec<u8> = writer.writes[1..]
            .iter()
            .flat_map(|chunk| chunk.iter().copied())
            .collect();
        assert_eq!(joined, expected_payload);
    }

    #[derive(Default)]
    struct RecordingWriter {
        writes: Vec<Vec<u8>>,
        flushes: usize,
    }

    impl AsyncWrite for RecordingWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.writes.push(buffer.to_vec());
            Poll::Ready(Ok(buffer.len()))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
        ) -> Poll<io::Result<()>> {
            self.flushes += 1;
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }
}
