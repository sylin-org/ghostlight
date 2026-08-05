// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Service-side owner-only bridge for the thin MCP edge.
//!
//! This module terminates the typed transport vocabulary and turns accepted starts into
//! protocol-neutral service work. It owns one bounded writer and one bounded active-work map per
//! admitted stream. Client protocol lifecycle and response envelopes stay at the edge.

use crate::browser::directory::{self, WorkspaceUse};
use crate::governance::overlay::SessionOverlay;
use crate::governance::ports::ClientInfo;
use crate::hub::peer::PeerCred;
use crate::hub::workspace::{WorkspaceError, WorkspaceLease};
use crate::hub::ServiceContext;
use crate::tool::outcome::{CallOutcome, DenialSource as CoreDenialSource};
use crate::work::{CancellationToken, WorkContext};
use crate::{Error, ToolError};
use ghostlight_transport::bridge::{
    read_edge_message, write_service_message, BridgeError, BridgeErrorKind, BridgeSequence,
    DenialSource, EdgeMessage, RequestContext, ServiceMessage, TerminalOutcome, WorkId,
    WorkspaceId, BRIDGE_MAJOR,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, Mutex};

/// Maximum active work accepted from one admitted edge stream.
pub const MAX_ACTIVE_WORK: usize = 128;

/// Bounded terminal and catalog queue feeding the stream's sole writer.
const WRITER_QUEUE_CAPACITY: usize = 64;

/// How long the service waits before giving the protocol shore an outcome-unknown response.
///
/// The same work future keeps settling privately after this boundary. Browser delivery has its
/// own longer bound, so landing checks, post-processing, audit completion, and workspace lease
/// release still happen without making the MCP client wait indefinitely.
const CALLER_RESPONSE_DEADLINE: Duration = Duration::from_secs(60);

const CALLER_DEADLINE_MESSAGE: &str = "The operation is still settling inside Ghostlight. Its effect may have completed; do not retry automatically. Inspect the browser before deciding what to do next.";

type ActiveWork = Arc<Mutex<HashMap<WorkId, CancellationToken>>>;

struct ActiveWorkGuard {
    count: Arc<AtomicUsize>,
}

impl ActiveWorkGuard {
    fn new(count: Arc<AtomicUsize>) -> Self {
        count.fetch_add(1, Ordering::AcqRel);
        Self { count }
    }
}

impl Drop for ActiveWorkGuard {
    fn drop(&mut self) {
        let previous = self.count.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "active work counter underflow");
    }
}

struct ValidatedContext {
    client: Option<ClientInfo>,
    restriction: Option<Arc<SessionOverlay>>,
}

/// Serve one already admitted local edge stream until it closes or its transport fails.
///
/// OS peer admission happens before this function. Only the peer's user principal participates in
/// workspace ownership and quotas; its process id is diagnostic and never becomes an authority or
/// routing key.
pub async fn serve_bridge<S>(
    mut stream: S,
    ctx: ServiceContext,
    peer: PeerCred,
) -> crate::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    tracing::debug!(peer_pid = peer.pid, "owner bridge connected");

    match read_edge_message(&mut stream).await? {
        Some(EdgeMessage::Hello { bridge_major }) if bridge_major == BRIDGE_MAJOR => {}
        Some(EdgeMessage::Hello { bridge_major }) => {
            return Err(Error::Ipc(format!(
                "unsupported owner bridge major {bridge_major}; this service requires {BRIDGE_MAJOR}"
            )));
        }
        Some(_) => {
            return Err(Error::Ipc(
                "owner bridge must begin with an exact hello".to_string(),
            ));
        }
        None => return Ok(()),
    }
    write_service_message(
        &mut stream,
        &ServiceMessage::Hello {
            bridge_major: BRIDGE_MAJOR,
        },
    )
    .await?;

    let (mut reader, writer) = tokio::io::split(stream);
    let (writer_tx, writer_rx) = mpsc::channel(WRITER_QUEUE_CAPACITY);
    let mut writer_task = tokio::spawn(write_loop(writer, writer_rx));
    let active: ActiveWork = Arc::new(Mutex::new(HashMap::new()));
    let mut attached_workspaces = HashSet::new();
    let mut next_work_id = 1u64;
    let mut catalog_changes = ctx.catalog_generation.subscribe();
    let catalog_writer = writer_tx.clone();
    let catalog_task = tokio::spawn(async move {
        while catalog_changes.changed().await.is_ok() {
            let generation = *catalog_changes.borrow_and_update();
            if catalog_writer
                .send(ServiceMessage::CatalogChanged { generation })
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let loop_result = loop {
        tokio::select! {
            writer_result = &mut writer_task => {
                break match writer_result {
                    Ok(Ok(())) => Err(Error::Ipc("owner bridge writer stopped".to_string())),
                    Ok(Err(error)) => Err(error),
                    Err(error) => Err(Error::Ipc(format!("owner bridge writer task failed: {error}"))),
                };
            }
            incoming = read_edge_message(&mut reader) => {
                let message = match incoming {
                    Ok(Some(message)) => message,
                    Ok(None) => break Ok(()),
                    Err(error) => break Err(error),
                };

                match message {
                    EdgeMessage::Hello { .. } => {
                        break Err(Error::Ipc(
                            "owner bridge hello may appear only as the first message".to_string(),
                        ));
                    }
                    EdgeMessage::OpenWorkspace {
                        sequence,
                        workspace: preferred,
                        context,
                    } => {
                        let validated = match validate_context(context) {
                            Ok(validated) => validated,
                            Err(error) => {
                                if let Err(error) = send_rejection(&writer_tx, sequence, error).await {
                                    break Err(error);
                                }
                                continue;
                            }
                        };
                        let workspace = match open_workspace(
                            &ctx,
                            &peer,
                            preferred,
                            &attached_workspaces,
                        ) {
                            Ok(workspace) => workspace,
                            Err(error) => {
                                if let Err(error) = send_rejection(
                                    &writer_tx,
                                    sequence,
                                    workspace_bridge_error(error),
                                ).await {
                                    break Err(error);
                                }
                                continue;
                            }
                        };
                        apply_presentation(&ctx, &workspace, validated.client.as_ref());
                        attached_workspaces.insert(workspace.clone());
                        if let Err(error) = send_message(
                            &writer_tx,
                            ServiceMessage::WorkspaceOpened { sequence, workspace },
                        ).await {
                            break Err(error);
                        }
                    }
                    EdgeMessage::ReleaseWorkspace { sequence, workspace } => {
                        match ctx.workspaces.release(&workspace, &peer.user) {
                            Ok(()) => {
                                attached_workspaces.remove(&workspace);
                                if let Err(error) = send_message(
                                    &writer_tx,
                                    ServiceMessage::WorkspaceReleased { sequence },
                                ).await {
                                    break Err(error);
                                }
                            }
                            Err(error) => {
                                if let Err(error) = send_rejection(
                                    &writer_tx,
                                    sequence,
                                    workspace_bridge_error(error),
                                ).await {
                                    break Err(error);
                                }
                            }
                        }
                    }
                    EdgeMessage::Catalog { sequence, workspace, context } => {
                        if workspace.as_ref().is_some_and(|workspace| {
                            !ctx.workspaces.contains(workspace, &peer.user)
                        }) {
                            if let Err(error) = send_rejection(
                                &writer_tx,
                                sequence,
                                invalid_workspace_error(),
                            ).await {
                                break Err(error);
                            }
                            continue;
                        }
                        let validated = match validate_context(context) {
                            Ok(validated) => validated,
                            Err(error) => {
                                if let Err(error) = send_rejection(&writer_tx, sequence, error).await {
                                    break Err(error);
                                }
                                continue;
                            }
                        };
                        let generation = *ctx.catalog_generation.borrow();
                        let authority = ctx.authority.current();
                        let projection = crate::tool::catalog::project_catalog(
                            &authority.governance,
                            validated.restriction.as_deref(),
                            generation,
                        );
                        if let Err(error) = send_message(
                            &writer_tx,
                            ServiceMessage::Catalog { sequence, projection },
                        ).await {
                            break Err(error);
                        }
                    }
                    EdgeMessage::Start {
                        sequence,
                        operation,
                        arguments,
                        workspace,
                        context,
                    } => {
                        let workspace_was_supplied = workspace.is_some();
                        let Some(descriptor) = directory::descriptor(&operation) else {
                            if let Err(error) = send_rejection(
                                &writer_tx,
                                sequence,
                                BridgeError {
                                    kind: BridgeErrorKind::InvalidRequest,
                                    message: format!("unknown operation: {operation}"),
                                    next_step: Some(
                                        "request the current catalog and use an advertised operation"
                                            .to_string(),
                                    ),
                                },
                            ).await {
                                break Err(error);
                            }
                            continue;
                        };
                        let validated = match validate_context(context) {
                            Ok(validated) => validated,
                            Err(error) => {
                                if let Err(error) = send_rejection(&writer_tx, sequence, error).await {
                                    break Err(error);
                                }
                                continue;
                            }
                        };
                        if active.lock().await.len() >= MAX_ACTIVE_WORK {
                            if let Err(error) = send_rejection(
                                &writer_tx,
                                sequence,
                                BridgeError {
                                    kind: BridgeErrorKind::Busy,
                                    message: "this edge stream has reached its active-work limit"
                                        .to_string(),
                                    next_step: Some(
                                        "wait for an active operation to settle and retry"
                                            .to_string(),
                                    ),
                                },
                            ).await {
                                break Err(error);
                            }
                            continue;
                        }

                        let workspace = match resolve_start_workspace(
                            &ctx,
                            &peer,
                            descriptor.workspace_use,
                            workspace,
                        ) {
                            Ok(resolved) => resolved,
                            Err(error) => {
                                if let Err(error) = send_rejection(&writer_tx, sequence, error).await {
                                    break Err(error);
                                }
                                continue;
                            }
                        };
                        if !workspace_was_supplied {
                            if let Some(workspace) = workspace.as_ref() {
                                apply_presentation(&ctx, workspace, validated.client.as_ref());
                            }
                        }
                        let lease = match workspace.as_ref() {
                            Some(workspace) => match ctx.workspaces.lease(workspace, &peer.user) {
                                Ok(lease) => Some(lease),
                                Err(error) => {
                                    if let Err(error) = send_rejection(
                                        &writer_tx,
                                        sequence,
                                        workspace_bridge_error(error),
                                    ).await {
                                        break Err(error);
                                    }
                                    continue;
                                }
                            },
                            None => None,
                        };

                        let work_id = match next_work_id.checked_add(1) {
                            Some(next) => {
                                let work_id = WorkId(next_work_id);
                                next_work_id = next;
                                work_id
                            }
                            None => {
                                if let Err(error) = send_rejection(
                                    &writer_tx,
                                    sequence,
                                    BridgeError {
                                        kind: BridgeErrorKind::Busy,
                                        message: "this edge stream exhausted its work-id space"
                                            .to_string(),
                                        next_step: Some("reconnect the edge and retry".to_string()),
                                    },
                                ).await {
                                    break Err(error);
                                }
                                continue;
                            }
                        };
                        let cancellation = CancellationToken::new();
                        active.lock().await.insert(work_id, cancellation.clone());
                        if let Err(error) = send_message(
                            &writer_tx,
                            ServiceMessage::Started {
                                sequence,
                                work_id,
                                workspace: workspace.clone(),
                                context_creating: descriptor.workspace_use == WorkspaceUse::Creates,
                            },
                        ).await {
                            active.lock().await.remove(&work_id);
                            cancellation.cancel();
                            break Err(error);
                        }

                        spawn_work(
                            ctx.clone(),
                            Arc::clone(&active),
                            writer_tx.clone(),
                            work_id,
                            WorkContext::new(
                                workspace,
                                operation,
                                validated.client,
                                validated.restriction,
                            ),
                            cancellation,
                            arguments,
                            lease,
                        );

                    }
                    EdgeMessage::Cancel { work_id } => {
                        if let Some(cancellation) = active.lock().await.get(&work_id).cloned() {
                            cancellation.cancel();
                        }
                    }
                }
            }
        }
    };

    cancel_active(&active).await;
    for workspace in attached_workspaces {
        ctx.workspaces.detach(&workspace, &peer.user);
    }
    catalog_task.abort();
    writer_task.abort();
    loop_result
}

async fn write_loop<W>(
    mut writer: W,
    mut messages: mpsc::Receiver<ServiceMessage>,
) -> crate::Result<()>
where
    W: AsyncWrite + Unpin,
{
    while let Some(message) = messages.recv().await {
        write_service_message(&mut writer, &message).await?;
    }
    Ok(())
}

async fn send_message(
    writer: &mpsc::Sender<ServiceMessage>,
    message: ServiceMessage,
) -> crate::Result<()> {
    writer
        .send(message)
        .await
        .map_err(|_| Error::Ipc("owner bridge writer is unavailable".to_string()))
}

async fn send_rejection(
    writer: &mpsc::Sender<ServiceMessage>,
    sequence: BridgeSequence,
    error: BridgeError,
) -> crate::Result<()> {
    send_message(writer, ServiceMessage::Rejected { sequence, error }).await
}

fn validate_context(context: RequestContext) -> Result<ValidatedContext, BridgeError> {
    let client = context.client.map(|client| ClientInfo {
        name: client.name,
        version: client.version,
    });
    let restriction = match context.restriction {
        Some(restriction) => Some(Arc::new(
            SessionOverlay::parse(
                &restriction,
                crate::browser::pattern::is_valid_pattern,
                crate::browser::polarity::evaluate_host,
            )
            .map_err(|error| BridgeError {
                kind: BridgeErrorKind::Restriction,
                message: format!("invalid tighten-only restriction: {error}"),
                next_step: Some(
                    "supply a valid schema-3 restriction or omit the restriction".to_string(),
                ),
            })?,
        )),
        None => None,
    };
    Ok(ValidatedContext {
        client,
        restriction,
    })
}

fn mint_workspace(
    ctx: &ServiceContext,
    peer: &PeerCred,
    attached: bool,
) -> Result<WorkspaceId, WorkspaceError> {
    let workspace = ctx.workspaces.mint(&peer.user, attached)?;
    let authority = Arc::clone(&ctx.authority);
    ctx.browser
        .register_attention_session(workspace.as_str(), move |event| {
            authority
                .current()
                .governance
                .record_attention_event_with_client(event, None);
        });
    Ok(workspace)
}

fn open_workspace(
    ctx: &ServiceContext,
    peer: &PeerCred,
    preferred: Option<WorkspaceId>,
    attached: &HashSet<WorkspaceId>,
) -> Result<WorkspaceId, WorkspaceError> {
    let Some(preferred) = preferred else {
        return mint_workspace(ctx, peer, true);
    };
    if attached.contains(&preferred) {
        return Ok(preferred);
    }
    match ctx.workspaces.attach(&preferred, &peer.user) {
        Ok(()) => Ok(preferred),
        Err(WorkspaceError::Unknown) => mint_workspace(ctx, peer, true),
        Err(error) => Err(error),
    }
}

fn apply_presentation(ctx: &ServiceContext, workspace: &WorkspaceId, client: Option<&ClientInfo>) {
    let label = client
        .map(|client| client.name.as_str())
        .unwrap_or("Ghostlight");
    ctx.browser.set_workspace_label(workspace.as_str(), label);
    ctx.browser.set_attention_label(workspace.as_str(), label);
}

fn resolve_start_workspace(
    ctx: &ServiceContext,
    peer: &PeerCred,
    workspace_use: WorkspaceUse,
    workspace: Option<WorkspaceId>,
) -> Result<Option<WorkspaceId>, BridgeError> {
    if let Some(workspace) = workspace {
        if !ctx.workspaces.contains(&workspace, &peer.user) {
            return Err(invalid_workspace_error());
        }
        return Ok(Some(workspace));
    }

    match workspace_use {
        WorkspaceUse::Independent => Ok(None),
        WorkspaceUse::Creates => mint_workspace(ctx, peer, false)
            .map(Some)
            .map_err(workspace_bridge_error),
        WorkspaceUse::Uses => Err(BridgeError {
            kind: BridgeErrorKind::InvalidWorkspace,
            message: "this operation requires a live workspace".to_string(),
            next_step: Some(
                "call a context-creating tab operation and retry with its workspace handle"
                    .to_string(),
            ),
        }),
    }
}

fn invalid_workspace_error() -> BridgeError {
    BridgeError {
        kind: BridgeErrorKind::InvalidWorkspace,
        message: "the workspace is unknown, expired, or unavailable to this local user".to_string(),
        next_step: Some(
            "create a new workspace context and retry with the returned handle".to_string(),
        ),
    }
}

fn workspace_bridge_error(error: WorkspaceError) -> BridgeError {
    match error {
        WorkspaceError::Unknown => invalid_workspace_error(),
        WorkspaceError::Quota => BridgeError {
            kind: BridgeErrorKind::Busy,
            message: "the local user has reached the live-workspace limit".to_string(),
            next_step: Some("release an unused workspace and retry".to_string()),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_work(
    ctx: ServiceContext,
    active: ActiveWork,
    writer: mpsc::Sender<ServiceMessage>,
    work_id: WorkId,
    work: WorkContext,
    cancellation: CancellationToken,
    arguments: Value,
    lease: Option<WorkspaceLease>,
) {
    // Increment before spawning so idle shutdown cannot observe a gap between accepting work and
    // the task's first poll. The guard survives edge disconnect and the bounded late-settlement
    // continuation, then decrements on every task exit path.
    let activity = ActiveWorkGuard::new(Arc::clone(&ctx.active_work));
    tokio::spawn(async move {
        let _activity = activity;
        let _lease = lease;
        let work_future = crate::tool::pipeline::run_work(
            &ctx.browser,
            &ctx.store,
            &ctx.authority,
            &ctx.workspaces,
            &work,
            &cancellation,
            &arguments,
        );
        report_work_with_deadline(&writer, work_id, CALLER_RESPONSE_DEADLINE, work_future).await;
        // A settled task still occupies the stream bound while its terminal message is waiting
        // for the bounded writer queue. Removing it earlier would turn blocked senders into an
        // unbounded hidden result backlog under a stalled peer.
        active.lock().await.remove(&work_id);
    });
}

async fn report_work_with_deadline<F>(
    writer: &mpsc::Sender<ServiceMessage>,
    work_id: WorkId,
    response_deadline: Duration,
    work: F,
) where
    F: Future<Output = CallOutcome>,
{
    tokio::pin!(work);
    tokio::select! {
        biased;
        outcome = &mut work => {
            let _ = writer
                .send(ServiceMessage::Completed {
                    work_id,
                    outcome: terminal_outcome(outcome),
                })
                .await;
        }
        _ = tokio::time::sleep(response_deadline) => {
            let response = writer.send(ServiceMessage::Completed {
                work_id,
                outcome: TerminalOutcome::OutcomeUnknown {
                    message: CALLER_DEADLINE_MESSAGE.to_string(),
                },
            });
            // Poll the bounded response enqueue and the SAME work future concurrently. A stalled
            // edge must not prevent the service from completing its landing check and audit.
            let (_, _) = tokio::join!(response, &mut work);
        }
    }
}

async fn cancel_active(active: &ActiveWork) {
    let cancellations: Vec<CancellationToken> = active.lock().await.values().cloned().collect();
    for cancellation in cancellations {
        cancellation.cancel();
    }
}

fn terminal_outcome(outcome: CallOutcome) -> TerminalOutcome {
    match outcome {
        CallOutcome::Success { result } => TerminalOutcome::Success { result },
        CallOutcome::Failure { error } => tool_failure(error),
        CallOutcome::NotDispatched { message } => TerminalOutcome::NotDispatched { message },
        CallOutcome::OutcomeUnknown { message } => TerminalOutcome::OutcomeUnknown { message },
        CallOutcome::Denied { message, source } => TerminalOutcome::Denied {
            message,
            source: match source {
                CoreDenialSource::Policy => DenialSource::Policy,
                CoreDenialSource::Sacred => DenialSource::Sacred,
            },
        },
        CallOutcome::Held { message } => TerminalOutcome::Held { message },
        CallOutcome::AttentionRequired { message } => {
            TerminalOutcome::AttentionRequired { message }
        }
        CallOutcome::Cancelled { message } => TerminalOutcome::Cancelled { message },
    }
}

fn tool_failure(error: ToolError) -> TerminalOutcome {
    let message = error.to_string();
    let result = json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    });
    TerminalOutcome::ToolFailure { result, message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::bridge::ClientPresentation;

    #[test]
    fn active_work_guard_covers_the_entire_spawned_future_lifetime() {
        let count = Arc::new(AtomicUsize::new(0));
        let guard = ActiveWorkGuard::new(Arc::clone(&count));
        assert_eq!(count.load(Ordering::Acquire), 1);
        drop(guard);
        assert_eq!(count.load(Ordering::Acquire), 0);
    }

    #[test]
    fn request_context_is_immutable_client_presentation_and_validated_restriction() {
        let context = RequestContext {
            client: Some(ClientPresentation {
                name: "test-client".to_string(),
                version: "1.2.3".to_string(),
            }),
            restriction: None,
        };
        let validated = validate_context(context).expect("context validates");
        assert_eq!(
            validated.client,
            Some(ClientInfo {
                name: "test-client".to_string(),
                version: "1.2.3".to_string(),
            })
        );
        assert!(validated.restriction.is_none());

        let error = validate_context(RequestContext {
            client: None,
            restriction: Some("not-json".to_string()),
        })
        .err()
        .expect("malformed restriction is rejected");
        assert_eq!(error.kind, BridgeErrorKind::Restriction);
    }

    #[test]
    fn semantic_outcomes_remain_distinct_at_the_edge_boundary() {
        assert_eq!(
            terminal_outcome(CallOutcome::Success {
                result: json!({"ok": true}),
            }),
            TerminalOutcome::Success {
                result: json!({"ok": true}),
            }
        );
        assert_eq!(
            terminal_outcome(CallOutcome::Denied {
                message: "no".to_string(),
                source: CoreDenialSource::Sacred,
            }),
            TerminalOutcome::Denied {
                message: "no".to_string(),
                source: DenialSource::Sacred,
            }
        );
        assert_eq!(
            terminal_outcome(CallOutcome::Cancelled {
                message: "stopped between steps".to_string(),
            }),
            TerminalOutcome::Cancelled {
                message: "stopped between steps".to_string(),
            }
        );
    }

    #[test]
    fn tool_failure_has_text_and_explicit_error_marker() {
        let TerminalOutcome::ToolFailure { result, message } =
            tool_failure(ToolError::invalid_request("bad arguments"))
        else {
            panic!("expected tool failure");
        };
        assert_eq!(result["isError"], true);
        assert_eq!(result["content"][0]["type"], "text");
        assert_eq!(result["content"][0]["text"], message);
    }

    #[tokio::test]
    async fn completed_work_wins_the_response_deadline_exactly_once() {
        let (writer, mut messages) = mpsc::channel(2);
        report_work_with_deadline(
            &writer,
            WorkId(7),
            Duration::ZERO,
            std::future::ready(CallOutcome::Success {
                result: json!({"settled": true}),
            }),
        )
        .await;

        assert!(matches!(
            messages.recv().await,
            Some(ServiceMessage::Completed {
                work_id: WorkId(7),
                outcome: TerminalOutcome::Success { .. },
            })
        ));
        assert!(matches!(
            messages.try_recv(),
            Err(mpsc::error::TryRecvError::Empty | mpsc::error::TryRecvError::Disconnected)
        ));
    }

    #[tokio::test]
    async fn late_work_sends_one_unknown_then_settles_privately() {
        let (writer, mut messages) = mpsc::channel(2);
        let (settle, settled) = tokio::sync::oneshot::channel::<()>();
        let reporter = tokio::spawn(async move {
            report_work_with_deadline(&writer, WorkId(8), Duration::from_millis(5), async move {
                let _ = settled.await;
                CallOutcome::Success {
                    result: json!({"late": true}),
                }
            })
            .await;
        });

        assert!(matches!(
            messages.recv().await,
            Some(ServiceMessage::Completed {
                work_id: WorkId(8),
                outcome: TerminalOutcome::OutcomeUnknown { .. },
            })
        ));
        assert!(!reporter.is_finished());
        settle.send(()).expect("late work receiver remains live");
        reporter.await.expect("reporter completes after settlement");
        assert!(matches!(
            messages.try_recv(),
            Err(mpsc::error::TryRecvError::Empty | mpsc::error::TryRecvError::Disconnected)
        ));
    }

    #[tokio::test]
    async fn full_writer_queue_does_not_block_private_settlement() {
        let (writer, mut messages) = mpsc::channel(1);
        writer
            .send(ServiceMessage::CatalogChanged { generation: 1 })
            .await
            .unwrap();
        let (work_polled, work_observed) = tokio::sync::oneshot::channel::<()>();
        let reporter = tokio::spawn(async move {
            report_work_with_deadline(&writer, WorkId(9), Duration::ZERO, async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                work_polled.send(()).ok();
                CallOutcome::Success {
                    result: json!({"late": true}),
                }
            })
            .await;
        });

        tokio::time::timeout(Duration::from_millis(500), work_observed)
            .await
            .expect("work is polled while response enqueue is blocked")
            .expect("work settlement signal remains live");
        assert!(!reporter.is_finished());
        assert!(matches!(
            messages.recv().await,
            Some(ServiceMessage::CatalogChanged { generation: 1 })
        ));
        assert!(matches!(
            messages.recv().await,
            Some(ServiceMessage::Completed {
                work_id: WorkId(9),
                outcome: TerminalOutcome::OutcomeUnknown { .. },
            })
        ));
        reporter.await.expect("reporter completes after enqueue");
    }
}
