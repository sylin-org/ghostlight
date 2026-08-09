// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Service-side owner-only bridge for the thin MCP edge.
//!
//! This module terminates the typed transport vocabulary and turns accepted starts into
//! protocol-neutral service work. It owns one bounded writer and one bounded active-work map per
//! admitted stream. Client protocol lifecycle and response envelopes stay at the edge.

use crate::governance::overlay::SessionOverlay;
use crate::governance::ports::ClientInfo;
use crate::hub::peer::PeerCred;
use crate::hub::workspace::{WorkspaceError, WorkspaceLease, WorkspaceRegistry};
use crate::hub::ServiceContext;
use crate::operation::registry;
use crate::tool::outcome::{CallOutcome, DenialSource as CoreDenialSource};
use crate::work::{CancellationToken, WorkContext};
use crate::{Error, ToolError};
use ghostlight_transport::bridge::{
    read_edge_message, write_service_message, BridgeError, BridgeErrorKind, BridgeSequence,
    EdgeMessage, RequestContext, ServiceMessage, TerminalOutcome, WorkId, WorkspaceId,
    WorkspaceUse, BRIDGE_MAJOR,
};
use ghostlight_transport::operation::{
    BrowserResult, BrowserResultStatus, InspectPageArguments, Operation, OperationEffect,
    OperationKind, ResultProblem, ResultProblemCode, SuggestedNextStep,
};
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
                        let projection = crate::operation::registry::project_availability(
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
                        workspace,
                        context,
                    } => {
                        let workspace_was_supplied = workspace.is_some();
                        let canonical_kind = operation.kind();
                        let workspace_use = registry::workspace_use(canonical_kind);
                        if let Err(error) = operation.validate() {
                            if let Err(error) = send_rejection(
                                &writer_tx,
                                sequence,
                                BridgeError {
                                    kind: BridgeErrorKind::InvalidRequest,
                                    message: error.to_string(),
                                    next_step: Some(
                                        "request the current catalog and correct the operation arguments"
                                            .to_string(),
                                    ),
                                },
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
                            workspace_use,
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
                        let _descriptor = registry::descriptor(canonical_kind);
                        if let Err(error) = validate_explicit_tab_handles(
                            &ctx.workspaces,
                            workspace.as_ref(),
                            &operation,
                        ) {
                            if let Err(error) = send_rejection(&writer_tx, sequence, error).await {
                                break Err(error);
                            }
                            continue;
                        }
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
                                context_creating: workspace_use == WorkspaceUse::Creates,
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
    lease: Option<WorkspaceLease>,
) {
    // Increment before spawning so idle shutdown cannot observe a gap between accepting work and
    // the task's first poll. The guard survives edge disconnect and the bounded late-settlement
    // continuation, then decrements on every task exit path.
    let activity = ActiveWorkGuard::new(Arc::clone(&ctx.active_work));
    tokio::spawn(async move {
        let _activity = activity;
        let _lease = lease;
        let operation = work.operation().clone();
        let operation_kind = work.operation_kind();
        let workspace = work.workspace().cloned();
        let work_future = crate::tool::pipeline::run_work(
            &ctx.browser,
            &ctx.store,
            &ctx.authority,
            &ctx.workspaces,
            &work,
            &cancellation,
        );
        report_work_with_deadline(
            &writer,
            work_id,
            operation_kind,
            workspace,
            CALLER_RESPONSE_DEADLINE,
            Some(CompletionContext {
                operation: &operation,
                workspaces: &ctx.workspaces,
            }),
            work_future,
        )
        .await;
        // A settled task still occupies the stream bound while its terminal message is waiting
        // for the bounded writer queue. Removing it earlier would turn blocked senders into an
        // unbounded hidden result backlog under a stalled peer.
        active.lock().await.remove(&work_id);
    });
}

async fn report_work_with_deadline<F>(
    writer: &mpsc::Sender<ServiceMessage>,
    work_id: WorkId,
    operation_kind: ghostlight_transport::operation::OperationKind,
    workspace: Option<WorkspaceId>,
    response_deadline: Duration,
    completion: Option<CompletionContext<'_>>,
    work: F,
) where
    F: Future<Output = CallOutcome>,
{
    tokio::pin!(work);
    tokio::select! {
        biased;
        outcome = &mut work => {
            let outcome = terminal_outcome(
                outcome,
                operation_kind,
                completion.map(|context| context.operation),
                completion.map(|context| context.workspaces),
                workspace.clone(),
            );
            let _ = writer
                .send(ServiceMessage::Completed {
                    work_id,
                    outcome,
                })
                .await;
        }
        _ = tokio::time::sleep(response_deadline) => {
            let response = writer.send(ServiceMessage::Completed {
                work_id,
                outcome: canonical_terminal(
                    operation_kind,
                    workspace.clone(),
                    BrowserResultStatus::OutcomeUnknown,
                    OperationEffect::Unknown,
                    ResultProblemCode::OutcomeUnknown,
                    CALLER_DEADLINE_MESSAGE,
                    uncertain_state_suggestion(),
                ),
            });
            // Poll the bounded response enqueue and the SAME work future concurrently. A stalled
            // edge must not prevent the service from completing its landing check and audit.
            let (_, _) = tokio::join!(response, &mut work);
        }
    }
}

fn validate_explicit_tab_handles(
    workspaces: &WorkspaceRegistry,
    workspace: Option<&WorkspaceId>,
    operation: &Operation,
) -> Result<(), BridgeError> {
    fn check(
        workspaces: &WorkspaceRegistry,
        workspace: Option<&WorkspaceId>,
        tab: Option<&ghostlight_transport::operation::TabHandle>,
    ) -> Result<(), BridgeError> {
        let Some(tab) = tab else { return Ok(()) };
        if workspace.is_some_and(|workspace| workspaces.resolve_tab(workspace, tab).is_some()) {
            return Ok(());
        }
        Err(BridgeError {
            kind: BridgeErrorKind::InvalidRequest,
            message: "unknown tab".to_string(),
            next_step: Some("list controlled tabs and use a current opaque tab handle".to_string()),
        })
    }

    use Operation as C;
    match operation {
        C::BrowserFocusTab(arguments) | C::BrowserCloseTab(arguments) => {
            check(workspaces, workspace, Some(&arguments.tab))
        }
        C::BrowserNavigate(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserGoBack(arguments)
        | C::BrowserGoForward(arguments)
        | C::BrowserReloadPage(arguments)
        | C::BrowserPressEscape(arguments)
        | C::BrowserGetDialog(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserInspectPage(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserReadPage(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserTakeScreenshot(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserClick(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserHover(arguments) | C::BrowserScrollToTarget(arguments) => {
            check(workspaces, workspace, arguments.tab.as_ref())
        }
        C::BrowserScrollPage(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserPressKey(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserDrag(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserFillForm(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserWaitFor(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserHandleDialog(arguments) => check(workspaces, workspace, arguments.tab.as_ref()),
        C::BrowserRunSequence(arguments) => {
            check(workspaces, workspace, arguments.tab.as_ref())?;
            for step in &arguments.steps {
                validate_explicit_tab_handles(workspaces, workspace, step)?;
            }
            Ok(())
        }
        C::BrowserGetStatus(_) | C::BrowserOpenTab(_) | C::BrowserListTabs(_) => Ok(()),
    }
}

#[derive(Clone, Copy)]
struct CompletionContext<'a> {
    operation: &'a Operation,
    workspaces: &'a WorkspaceRegistry,
}

async fn cancel_active(active: &ActiveWork) {
    let cancellations: Vec<CancellationToken> = active.lock().await.values().cloned().collect();
    for cancellation in cancellations {
        cancellation.cancel();
    }
}

fn terminal_outcome(
    outcome: CallOutcome,
    operation_kind: ghostlight_transport::operation::OperationKind,
    _operation: Option<&Operation>,
    _workspaces: Option<&WorkspaceRegistry>,
    workspace: Option<WorkspaceId>,
) -> TerminalOutcome {
    match outcome {
        CallOutcome::Success { result } => TerminalOutcome { result },
        CallOutcome::Failure { error } => tool_failure(operation_kind, workspace, error),
        CallOutcome::NotDispatched { message } => canonical_terminal(
            operation_kind,
            workspace,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
            ResultProblemCode::NotDispatched,
            &message,
            Vec::new(),
        ),
        CallOutcome::OutcomeUnknown { message } => canonical_terminal(
            operation_kind,
            workspace,
            BrowserResultStatus::OutcomeUnknown,
            OperationEffect::Unknown,
            ResultProblemCode::OutcomeUnknown,
            &message,
            uncertain_state_suggestion(),
        ),
        CallOutcome::Denied { message, source } => canonical_terminal(
            operation_kind,
            workspace,
            BrowserResultStatus::Blocked,
            OperationEffect::None,
            match source {
                CoreDenialSource::Policy => ResultProblemCode::PolicyBlocked,
                CoreDenialSource::Sacred => ResultProblemCode::ProtectedHost,
            },
            &message,
            Vec::new(),
        ),
        CallOutcome::Held { prolonged } => canonical_terminal(
            operation_kind,
            workspace,
            BrowserResultStatus::Held,
            OperationEffect::None,
            ResultProblemCode::HeldByUser,
            if prolonged {
                "The user has controlled the browser for more than two minutes."
            } else {
                "The user is controlling the browser."
            },
            vec![SuggestedNextStep::WaitForUser {
                reason: "Wait until the user returns browser control.".into(),
            }],
        ),
        CallOutcome::AttentionRequired { message } => canonical_terminal(
            operation_kind,
            workspace,
            BrowserResultStatus::AttentionRequired,
            OperationEffect::None,
            ResultProblemCode::AttentionRequired,
            &message,
            vec![SuggestedNextStep::AskUser {
                reason: "The user must review Ghostlight before work continues.".into(),
                question: "Please review Ghostlight and tell me when I should continue.".into(),
            }],
        ),
        CallOutcome::Cancelled { message, effect } => {
            let (status, effect, suggestions) = match effect {
                OperationEffect::None => (
                    BrowserResultStatus::Cancelled,
                    OperationEffect::None,
                    Vec::new(),
                ),
                OperationEffect::Committed => (
                    BrowserResultStatus::Cancelled,
                    OperationEffect::Committed,
                    Vec::new(),
                ),
                OperationEffect::Dispatched | OperationEffect::Unknown => (
                    BrowserResultStatus::OutcomeUnknown,
                    OperationEffect::Unknown,
                    uncertain_state_suggestion(),
                ),
            };
            canonical_terminal(
                operation_kind,
                workspace,
                status,
                effect,
                if status == BrowserResultStatus::Cancelled {
                    ResultProblemCode::Cancelled
                } else {
                    ResultProblemCode::OutcomeUnknown
                },
                &message,
                suggestions,
            )
        }
    }
}

fn tool_failure(
    operation: OperationKind,
    workspace: Option<WorkspaceId>,
    error: ToolError,
) -> TerminalOutcome {
    let effectful = registry::descriptor(operation)
        .requires
        .iter()
        .any(|capability| {
            matches!(
                capability,
                crate::governance::ports::Capability::Interact
                    | crate::governance::ports::Capability::Write
                    | crate::governance::ports::Capability::Execute
            )
        });
    let effect_unknown = effectful
        && matches!(
            &error,
            ToolError::Ipc { .. } | ToolError::Extension { .. } | ToolError::Cdp { .. }
        );
    let (code, message, suggestion) = match error {
        ToolError::InvalidRequest { .. } => (
            ResultProblemCode::InvalidArguments,
            "Ghostlight rejected invalid operation arguments.",
            None,
        ),
        ToolError::Page { .. } => (
            ResultProblemCode::TargetStale,
            "The page target or document changed before Ghostlight could finish.",
            Some(SuggestedNextStep::Call {
                reason: "Inspect the current page to get fresh targets.".into(),
                operation: Operation::BrowserInspectPage(
                    InspectPageArguments::default(),
                ),
            }),
        ),
        ToolError::Ipc { .. } | ToolError::Binary { .. } => (
            ResultProblemCode::CapabilityUnavailable,
            "Ghostlight could not complete the operation because its local service path failed.",
            Some(SuggestedNextStep::ReconnectClient {
                reason: "Reconnect the MCP client before continuing.".into(),
            }),
        ),
        ToolError::Extension { .. } => (
            ResultProblemCode::BrowserDisconnected,
            "Ghostlight could not complete the operation because the browser is unavailable.",
            Some(SuggestedNextStep::ReconnectBrowser {
                reason: "Reconnect the Ghostlight browser extension before continuing.".into(),
            }),
        ),
        ToolError::CapabilityNotReady { capability, .. } if capability == "browser" => (
            ResultProblemCode::BrowserDisconnected,
            "Ghostlight could not complete the operation because the browser is unavailable.",
            Some(SuggestedNextStep::ReconnectBrowser {
                reason: "Reconnect the Ghostlight browser extension before continuing.".into(),
            }),
        ),
        ToolError::CapabilityNotReady { .. } => (
            ResultProblemCode::CapabilityUnavailable,
            "The connected browser adapter does not provide a capability required by this operation.",
            Some(SuggestedNextStep::ReconnectBrowser {
                reason: "Reload or update the Ghostlight extension before trying again.".into(),
            }),
        ),
        ToolError::Cdp { .. } => (
            ResultProblemCode::CapabilityUnavailable,
            "The browser rejected the requested operation.",
            None,
        ),
        ToolError::Held { prolonged } => {
            return terminal_outcome(
                CallOutcome::Held { prolonged },
                operation,
                None,
                None,
                workspace,
            )
        }
        ToolError::AttentionRequired { message } => {
            return terminal_outcome(
                CallOutcome::AttentionRequired { message },
                operation,
                None,
                None,
                workspace,
            )
        }
    };
    canonical_terminal(
        operation,
        workspace,
        if effect_unknown {
            BrowserResultStatus::OutcomeUnknown
        } else if matches!(
            code,
            ResultProblemCode::InvalidArguments | ResultProblemCode::TargetStale
        ) {
            BrowserResultStatus::Blocked
        } else {
            BrowserResultStatus::Unavailable
        },
        if effect_unknown {
            OperationEffect::Unknown
        } else {
            OperationEffect::None
        },
        if effect_unknown {
            ResultProblemCode::OutcomeUnknown
        } else {
            code
        },
        message,
        if effect_unknown {
            uncertain_state_suggestion()
        } else {
            suggestion.into_iter().collect()
        },
    )
}

fn canonical_terminal(
    operation: OperationKind,
    workspace: Option<WorkspaceId>,
    status: BrowserResultStatus,
    effect: OperationEffect,
    code: ResultProblemCode,
    message: &str,
    suggested_next_steps: Vec<SuggestedNextStep>,
) -> TerminalOutcome {
    let mut result = BrowserResult::new(operation, status, effect);
    result.workspace = workspace;
    result.problem = Some(ResultProblem {
        code,
        message: bounded_guidance(message),
    });
    result.suggested_next_steps = suggested_next_steps;
    debug_assert!(result.validate_semantics().is_ok());
    TerminalOutcome {
        result: Box::new(result),
    }
}

fn uncertain_state_suggestion() -> Vec<SuggestedNextStep> {
    vec![SuggestedNextStep::Call {
        reason: "Inspect the current page before deciding what to do next.".into(),
        operation: Operation::BrowserInspectPage(InspectPageArguments::default()),
    }]
}

fn bounded_guidance(message: &str) -> String {
    let mut bounded = String::new();
    for character in message.chars().filter(|character| !character.is_control()) {
        if bounded.len() + character.len_utf8()
            > ghostlight_transport::operation::MAX_RESULT_GUIDANCE_BYTES
        {
            break;
        }
        bounded.push(character);
    }
    if bounded.is_empty() {
        "Ghostlight could not complete the operation.".into()
    } else {
        bounded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::bridge::ClientPresentation;
    use ghostlight_transport::operation::{
        BrowserConnectionStatus, BrowserResult, BrowserResultStatus, EmptyArguments,
        GovernanceModeStatus, OperationKind, OperationResult, PolicySourceStatus, RetryDisposition,
        StatusAuthority, StatusLimits,
    };

    const TEST_OPERATION: OperationKind = OperationKind::BrowserGetStatus;

    fn test_call() -> Operation {
        Operation::BrowserGetStatus(EmptyArguments {})
    }

    fn completed_status_result() -> Box<BrowserResult> {
        let mut result = BrowserResult::new(
            OperationKind::BrowserGetStatus,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.result = Some(OperationResult::BrowserGetStatus {
            browser: BrowserConnectionStatus::Disconnected,
            authority: StatusAuthority {
                policy_source: PolicySourceStatus::None,
                mode: GovernanceModeStatus::Open,
            },
            operations: crate::operation::registry::descriptors()
                .iter()
                .map(|descriptor| descriptor.operation)
                .collect(),
            packs: Vec::new(),
            limits: StatusLimits {
                max_sequence_steps: 10,
                max_tabs: 64,
                max_read_chars: 50_000,
            },
        });
        Box::new(result)
    }

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
        let call = test_call();
        let result = terminal_outcome(
            CallOutcome::Success {
                result: completed_status_result(),
            },
            TEST_OPERATION,
            Some(&call),
            None,
            None,
        )
        .result;
        assert_eq!(result.operation, TEST_OPERATION);
        assert_eq!(result.status, BrowserResultStatus::Ok);
        assert_eq!(result.effect, OperationEffect::None);
        let ghostlight_transport::operation::OperationResult::BrowserGetStatus {
            browser,
            operations,
            ..
        } = result.result.as_ref().expect("typed status result")
        else {
            panic!("expected browser_get_status result")
        };
        assert_eq!(
            *browser,
            ghostlight_transport::operation::BrowserConnectionStatus::Disconnected
        );
        assert_eq!(operations.len(), 24);
        let denied = terminal_outcome(
            CallOutcome::Denied {
                message: "no".to_string(),
                source: CoreDenialSource::Sacred,
            },
            TEST_OPERATION,
            Some(&call),
            None,
            None,
        );
        assert_eq!(denied.result.status, BrowserResultStatus::Blocked);
        assert_eq!(denied.result.effect, OperationEffect::None);
        assert_eq!(
            denied.result.problem.as_ref().map(|problem| problem.code),
            Some(ResultProblemCode::ProtectedHost)
        );
        let cancelled = terminal_outcome(
            CallOutcome::Cancelled {
                message: "stopped between steps".to_string(),
                effect: OperationEffect::None,
            },
            TEST_OPERATION,
            Some(&call),
            None,
            None,
        );
        assert_eq!(cancelled.result.status, BrowserResultStatus::Cancelled);
        assert_eq!(cancelled.result.effect, OperationEffect::None);
    }

    #[test]
    fn tool_failure_is_a_canonical_problem_not_an_edge_payload() {
        let outcome = tool_failure(
            TEST_OPERATION,
            None,
            ToolError::invalid_request("bad arguments"),
        );
        assert_eq!(outcome.result.status, BrowserResultStatus::Blocked);
        assert_eq!(outcome.result.effect, OperationEffect::None);
        assert_eq!(
            outcome.result.problem.as_ref().map(|problem| problem.code),
            Some(ResultProblemCode::InvalidArguments)
        );
    }

    #[test]
    fn adapter_capability_mismatch_has_one_actionable_recovery() {
        let outcome = tool_failure(
            OperationKind::BrowserOpenTab,
            None,
            ToolError::CapabilityNotReady {
                capability: "atomic tab opening".into(),
                message: "the adapter is older than this operation".into(),
                next_step: "reload the extension".into(),
            },
        );
        assert_eq!(outcome.result.status, BrowserResultStatus::Unavailable);
        assert_eq!(outcome.result.effect, OperationEffect::None);
        assert_eq!(outcome.result.repeat, RetryDisposition::Safe);
        assert_eq!(
            outcome.result.suggested_next_steps,
            vec![SuggestedNextStep::ReconnectBrowser {
                reason: "Reload or update the Ghostlight extension before trying again.".into(),
            }]
        );
    }

    #[tokio::test]
    async fn completed_work_wins_the_response_deadline_exactly_once() {
        let (writer, mut messages) = mpsc::channel(2);
        report_work_with_deadline(
            &writer,
            WorkId(7),
            TEST_OPERATION,
            None,
            Duration::ZERO,
            None,
            std::future::ready(CallOutcome::Success {
                result: completed_status_result(),
            }),
        )
        .await;

        assert!(matches!(
            messages.recv().await,
            Some(ServiceMessage::Completed {
                work_id: WorkId(7),
                outcome: TerminalOutcome { result },
            })
            if result.status == BrowserResultStatus::Ok
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
            report_work_with_deadline(
                &writer,
                WorkId(8),
                TEST_OPERATION,
                None,
                Duration::from_millis(5),
                None,
                async move {
                    let _ = settled.await;
                    CallOutcome::Success {
                        result: completed_status_result(),
                    }
                },
            )
            .await;
        });

        assert!(matches!(
            messages.recv().await,
            Some(ServiceMessage::Completed {
                work_id: WorkId(8),
                outcome: TerminalOutcome { result },
            })
            if result.status == BrowserResultStatus::OutcomeUnknown
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
            report_work_with_deadline(
                &writer,
                WorkId(9),
                TEST_OPERATION,
                None,
                Duration::ZERO,
                None,
                async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    work_polled.send(()).ok();
                    CallOutcome::Success {
                        result: completed_status_result(),
                    }
                },
            )
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
                outcome: TerminalOutcome { result },
            })
            if result.status == BrowserResultStatus::OutcomeUnknown
        ));
        reporter.await.expect("reporter completes after enqueue");
    }
}
