//! The one executor: an invocation's lifecycle, cancellation and deadlines, the frame it runs in,
//! target and tab resolution, and the typed terminals every result is built from. Its parts live
//! beside it: admission (`admission`), browser choice (`routing`), failures (`failure`), path
//! uploads (`upload`), the completion path (`receipt`), flows (`flow`, `composition`), and one
//! module per family of page work.

mod admission;
mod composition;
mod documents;
mod failure;
mod findings;
mod flow;
mod forms;
mod interaction;
mod invocation_id;
mod item;
mod navigation;
mod pending;
mod pointer;
mod policy;
mod reading;
mod receipt;
mod recording;
pub mod result;
mod routing;
mod upload;

use failure::{unchecked, unconfirmed_status, with_denial_facts};
use routing::routing_refusal;
use upload::load_physical_files;

pub use invocation_id::InvocationId;

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, BrowserReadiness, ObservedTarget, PhysicalActionSubject,
    PhysicalFile, PhysicalPoint, PhysicalRecordingSummary, PhysicalTab, PresentationActivity,
    RecordingDelivery, RecordingState, RecordingStopReason, StaleLookup,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::{json, Value};
use url::Url;

use crate::browser::recovery::{BrowserRecovery, RecoveryDecision, RecoveryFailure};
use crate::browser::{
    addressed_tab, choose_browser, Availability, BrowserError, BrowserPort, RETURN_GRACE,
};
use crate::events::{DenialPresentation, DomainEvent};

use crate::audit::AuditRecord;
use crate::browser::adapter::{PhysicalSemantic, PhysicalWait};
use crate::governance::manifest::BrowserStartup;
use crate::governance::{
    AuthoritySnapshot, Capability, CapabilitySet, Decision, GovernanceFacade, Publication,
    ReasonCode,
};
use crate::language::{
    self,
    audit::AuditProjection,
    field_problem::ControlRef,
    outcome::{
        ActionSubject, BlockedReason, BrowserRecoveryReason, BrowserUnavailableReason, CancelCause,
        CheckFailure, ExtensionAdvice, FileProblem, Observed, Outcome, Refusal, StaleHelp,
        TargetRole, WorkspaceReason, CANDIDATE_TABS_FACT, EXCEPTION_FACT, HANDLE_TAB_FACT,
        IMAGE_SIZE_FACT, OWNER_FACT, RECOVERY_CANDIDATES_FACT, REPLACEMENT_FACT, STALE_FACT,
    },
    page_refusal::{PageGesture, PageRefusal},
    Operation, Record, RecordAction, TakeScreenshot,
};
use crate::presentation::PresentationReactor;
use crate::provenance::ConnectionEvidence;
use crate::workbench::WorkbenchProjection;
use crate::workspace::{
    bounded, SelectedTab, SelectedTarget, SelectedView, StaleReason, StaleTargetContext, TabHold,
    WorkspaceError, WorkspaceId, WorkspaceLease, WorkspaceStore,
};
use findings::{Findings, Frame, FrameFindings, StaleOffer};
use item::candidate_item;
use result::{Effect, InvocationResult, Readiness, Status};

/// How many controls a stale-target refusal offers.
const STALE_CANDIDATES_MAX: usize = 3;

/// The most controls one semantic query lists. A selector must match exactly one, and past this
/// many the refusal says at least this many matched. A presence wait and a stale handle's
/// candidates ask with it too, so the page cuts their answer, and the result reports a size
/// limit, only past this many matches.
const SELECTOR_MATCH_MAX: usize = 8;

/// Cloneable cancellation state forwarded from the MCP edge to physical dispatch.
///
/// It keeps who cancelled beside its flag. The cause is set before the flag, so whoever sees the
/// flag can read the cause, and the first cause is kept.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<Cancellation>);

/// The shared state behind a [`CancellationToken`].
#[derive(Debug, Default)]
struct Cancellation {
    flag: AtomicBool,
    cause: OnceLock<CancelCause>,
}

impl CancellationToken {
    /// Request cancellation for `cause`. A later cancel keeps the first cause.
    pub fn cancel(&self, cause: CancelCause) {
        let _ = self.0.cause.set(cause);
        self.0.flag.store(true, Ordering::SeqCst);
    }
    /// Whether cancellation is currently requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.flag.load(Ordering::SeqCst)
    }
    /// Who cancelled, once cancellation is requested.
    #[must_use]
    pub fn cause(&self) -> Option<CancelCause> {
        self.0.cause.get().copied()
    }
    fn flag(&self) -> &AtomicBool {
        &self.0.flag
    }
}

/// One model request, built once at service intake and owned by it until its completion.
///
/// It carries everything the request is: its id, tool, workspace, the evidence of the connection
/// that sent it, its cancellation, when it was received, its deadline and its decoded operation.
/// Waiting behind earlier work consumes the original deadline.
pub(crate) struct PreparedInvocation {
    pub(crate) id: InvocationId,
    pub(crate) tool: String,
    workspace: WorkspaceId,
    provenance: Arc<ConnectionEvidence>,
    cancellation: CancellationToken,
    /// When the service received the request; the deadline and the recorded span count from it.
    received: Instant,
    /// The same moment in wall-clock milliseconds, where a live row starts.
    received_at_ms: u64,
    deadline: Instant,
    /// Whether a longer `timeout_ms` would give this call more time: its value is below the
    /// maximum.
    deadline_adjustable: bool,
    decoded: Result<Operation, language::LanguageError>,
    stage: Mutex<InvocationStage>,
}

#[derive(Default)]
struct InvocationStage {
    running_or_finished: bool,
    waiting_reported: bool,
}

/// How the session queue took a request: queued for its worker, or refused because the queue was
/// full or could admit nothing more, which the executor answers through the completion path
/// without running it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Admission {
    /// The request waited its turn in the session queue.
    Queued,
    /// The session queue was full; the request is refused for capacity.
    OverCapacity,
    /// The session admitted nothing more when the request arrived, because Ghostlight was
    /// quitting; the request is cancelled before it started, for this cause.
    Cancelled(CancelCause),
}

impl PreparedInvocation {
    /// Expiration is measured from intake, including time waiting behind earlier work.
    pub(crate) fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// Decode and start the deadline before any queue wait.
    pub(crate) fn new(
        id: InvocationId,
        tool: &str,
        input: Value,
        workspace: WorkspaceId,
        provenance: Arc<ConnectionEvidence>,
    ) -> Self {
        let received = Instant::now();
        let received_at_ms = crate::audit::unix_ms();
        let decoded = language::decode(tool, input);
        let timeout = decoded
            .as_ref()
            .map_or(language::DEFAULT_TIMEOUT_MS, Operation::timeout_ms)
            .clamp(language::MIN_TIMEOUT_MS, language::MAX_TIMEOUT_MS);
        let deadline_adjustable = decoded.as_ref().is_ok_and(Operation::timeout_can_grow);
        Self {
            id,
            tool: tool.into(),
            workspace,
            provenance,
            cancellation: CancellationToken::default(),
            received,
            received_at_ms,
            deadline: received + Duration::from_millis(timeout),
            deadline_adjustable,
            decoded,
            stage: Mutex::default(),
        }
    }

    /// Operations already independent of the workspace lease retain their own admission lane.
    pub(crate) fn independent(&self) -> bool {
        self.decoded
            .as_ref()
            .is_ok_and(|operation| !operation_requires_workspace_lease(operation))
    }

    /// Cancel this request for `cause`: queued, it completes as cancelled; running, its
    /// dispatch stops.
    pub(crate) fn cancel(&self, cause: CancelCause) {
        self.cancellation.cancel(cause);
    }

    /// Whether this request has been cancelled.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

/// The single application executor for every model-requested operation and sequence step.
pub struct ApplicationExecutor {
    governance: GovernanceFacade,
    workspaces: WorkspaceStore,
    browser: Arc<dyn BrowserPort>,
    recovery: BrowserRecovery,
    presentation: PresentationReactor,
    workbench: WorkbenchProjection,
    audit: Arc<crate::audit::AuditRecorder>,
    diagnostics: Arc<crate::diagnostics::DiagnosticsHub>,
    /// Whether any browser profile on this computer holds the Ghostlight extension. It is asked
    /// only while setup is incomplete and a refusal is about to send the person to a browser.
    extension_probe: ExtensionProbe,
}

/// Reads, when asked, whether any browser profile on this computer holds the extension.
type ExtensionProbe = Arc<dyn Fn() -> bool + Send + Sync>;

/// Whether any installed browser has the Ghostlight extension in some profile (a local,
/// read-only scan that writes and sends nothing).
fn extension_in_a_browser_profile() -> bool {
    crate::install::browser_profiles::scan(
        &crate::install::native_host::NativeHostRegistry::discover(),
    )
    .iter()
    .any(|browser| !browser.scan.with_extension.is_empty())
}

impl ApplicationExecutor {
    /// Construct the orchestrator's only model-requested mutation entry point.
    #[must_use]
    pub fn new(
        governance: GovernanceFacade,
        workspaces: WorkspaceStore,
        browser: Arc<dyn BrowserPort>,
        presentation: PresentationReactor,
        workbench: WorkbenchProjection,
        audit: Arc<crate::audit::AuditRecorder>,
        diagnostics: Arc<crate::diagnostics::DiagnosticsHub>,
    ) -> Self {
        let recovery = BrowserRecovery::discover(governance.clone(), Arc::clone(&browser));
        Self {
            governance,
            workspaces,
            browser,
            recovery,
            presentation,
            workbench,
            audit,
            diagnostics,
            extension_probe: Arc::new(extension_in_a_browser_profile),
        }
    }

    /// Whether a refusal that sends the person to a browser should also say where the extension
    /// comes from: setup is incomplete and no profile has it. A service with no setup progress
    /// attached tracks no setup, so it adds nothing.
    fn extension_advice(&self) -> ExtensionAdvice {
        let Some(setup) = self.workbench.setup() else {
            return ExtensionAdvice::Plain;
        };
        let complete = setup.is_complete();
        ExtensionAdvice::resolve(complete, !complete && (self.extension_probe)())
    }

    /// Run one call in `workspace` as its one connection's request, cancelled by `cancellation`.
    #[cfg(test)]
    pub(crate) fn execute(
        &self,
        workspace: &WorkspaceId,
        tool: &str,
        input: Value,
        cancellation: &CancellationToken,
    ) -> InvocationResult {
        let mut prepared = PreparedInvocation::new(
            InvocationId::mint(),
            tool,
            input,
            workspace.clone(),
            self.workspaces
                .single_connection(workspace)
                .expect("a test workspace has one connection"),
        );
        prepared.cancellation = cancellation.clone();
        self.execute_prepared(&prepared, Admission::Queued)
    }

    /// Show a sustained admission wait on the existing operation surface, without a popup.
    pub(crate) fn show_waiting(&self, prepared: &PreparedInvocation) {
        let mut stage = prepared
            .stage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if stage.running_or_finished
            || stage.waiting_reported
            || prepared.received.elapsed() < WAITING_SHOWN_AFTER
        {
            return;
        }
        let Ok(operation) = &prepared.decoded else {
            return;
        };
        stage.waiting_reported = true;
        self.workbench.react(&DomainEvent::WorkWaiting {
            invocation: prepared.id.to_string(),
            workspace: prepared.workspace.as_str().into(),
            tool: prepared.tool.clone(),
            activity: operation_activity(operation),
            capabilities: language::capability_map::requirements(operation),
            provenance: prepared.provenance.details(),
            received_at_ms: prepared.received_at_ms,
        });
    }

    /// Execute or refuse one prepared request through the same truthful completion path.
    pub(crate) fn execute_prepared(
        &self,
        prepared: &PreparedInvocation,
        admission: Admission,
    ) -> InvocationResult {
        let invocation = prepared.id.clone();
        let tool = prepared.tool.as_str();
        let workspace = &prepared.workspace;
        let cancellation = &prepared.cancellation;
        let started = prepared.received;
        let capacity_refused = admission == Admission::OverCapacity;
        let refused_at_admission = admission != Admission::Queued;
        let operation = match &prepared.decoded {
            Ok(operation) => operation,
            Err(error) => {
                let snapshot = self.governance.snapshot();
                let decision = Decision::refused(ReasonCode::InvalidRequest);
                let mut terminal = Terminal::refused(
                    &invocation,
                    Shape {
                        status: Status::Failed,
                        effect: Effect::None,
                        readiness: Readiness::NotApplicable,
                        repeat_safe: true,
                    },
                    &Refusal::InvalidRequest,
                    json!({"detail":error.to_string()}),
                    decision,
                    None,
                );
                // The decoder knows how this call missed the catalog, so its guidance is the step.
                terminal.result.next_steps = vec![error.guidance()];
                return self.finish(
                    terminal,
                    // A request that never decoded asked for nothing and learned nothing.
                    Completion {
                        workspace,
                        tool,
                        requirements: CapabilitySet::EMPTY,
                        snapshot: &snapshot,
                        duration_ms: elapsed_ms(started),
                        provenance: &prepared.provenance,
                        findings: Findings::default(),
                    },
                );
            }
        };
        let deadline = prepared.deadline;
        let requires_lease = operation_requires_workspace_lease(operation);
        let lease = if requires_lease && !refused_at_admission {
            if let Ok(lease) = self.workspaces.acquire(workspace) {
                Some(lease)
            } else {
                self.show_waiting(prepared);
                self.workspaces
                    .acquire_until(workspace, deadline, cancellation)
                    .ok()
            }
        } else {
            None
        };
        let publication = self.governance.published();
        let frame = Frame::of(operation);
        let context = InvocationContext::of(prepared, operation, &publication, &frame);
        let mut terminal = if capacity_refused {
            Terminal::refused(
                &invocation,
                Shape {
                    status: Status::Failed,
                    effect: Effect::None,
                    readiness: Readiness::NotApplicable,
                    repeat_safe: true,
                },
                &Refusal::Capacity,
                json!({}),
                Decision::permitted(),
                None,
            )
        } else if let Admission::Cancelled(by) = admission {
            cancelled_before_start(&invocation, by)
        } else if let Some(terminal) = self.channel_preflight(&context) {
            terminal
        } else if (!requires_lease || lease.is_some())
            && !cancellation.is_cancelled()
            && Instant::now() < deadline
        {
            let mut stage = prepared
                .stage
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            stage.running_or_finished = true;
            drop(stage);
            self.emit(DomainEvent::WorkStarted {
                invocation: invocation.to_string(),
                workspace: workspace.as_str().into(),
                tool: tool.into(),
                activity: operation_activity(operation),
                capabilities: context.requirements,
                provenance: prepared.provenance.details(),
                received_at_ms: prepared.received_at_ms,
            });
            if let Some(lease) = lease.as_ref() {
                self.run(&context, lease, operation)
            } else {
                self.run_without_workspace_lease(&context, operation)
            }
        } else if let Some(by) = cancellation.cause() {
            cancelled_before_start(&invocation, by)
        } else if Instant::now() >= deadline {
            Terminal::refused(
                &invocation,
                Shape {
                    status: Status::Failed,
                    effect: Effect::None,
                    readiness: Readiness::NotApplicable,
                    repeat_safe: true,
                },
                &Refusal::DeadlineBeforeStart,
                json!({}),
                Decision::permitted(),
                None,
            )
        } else {
            self.workspace_failure(&context, WorkspaceError::UnknownWorkspace)
        };
        prepared
            .stage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .running_or_finished = true;
        if let Some(lease) = lease.as_ref() {
            pending::report_forgotten(lease, &mut terminal);
        }
        self.finish(terminal, Completion::of(&context, tool, started))
    }

    fn run(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        operation: &Operation,
    ) -> Terminal {
        if !matches!(operation, Operation::ExplainPolicy(_)) {
            if let Some(terminal) = self.audit_preflight(context) {
                return terminal;
            }
            if let Some(terminal) = self.confirm_pending(context, lease, operation) {
                return terminal;
            }
        }
        match operation {
            Operation::ListTabs(_) => self.list_tabs(context, lease),
            Operation::ActivateTab(value) => self.activate_tab(context, lease, &value.tab),
            Operation::OpenPage(value) => self.open_page(context, lease, &value.url, value.reuse),
            Operation::NavigatePage(value) => self.navigate_page(
                context,
                lease,
                value.tab.as_deref(),
                &value.url,
                value.beforeunload_discard,
                value.reuse,
            ),
            Operation::NavigateHistory(value) => {
                self.navigate_history(context, lease, value.tab.as_deref(), value.direction)
            }
            Operation::ReloadPage(value) => {
                self.reload_page(context, lease, value.tab.as_deref(), value.bypass_cache)
            }
            Operation::CloseTab(value) => self.close_tab(context, lease, &value.tab),
            Operation::ReadPage(value) => self.read_page(
                context,
                lease,
                value.tab.as_deref(),
                value.target.as_deref(),
                value.mode,
                value.max_chars,
            ),
            Operation::InspectPage(value) => self.inspect_page(context, lease, value),
            Operation::Find(value) => self.find(
                context,
                lease,
                value.tab.as_deref(),
                &value.text,
                value.scope,
                value.max_results,
            ),
            Operation::TakeScreenshot(value) => self.screenshot(context, lease, value),
            Operation::Click(value) => self.perform_click(context, lease, value),
            Operation::ScrollPage(value) => self.perform_scroll(context, lease, value),
            Operation::SetZoom(value) => {
                self.set_zoom(context, lease, value.tab.as_deref(), value.percent)
            }
            Operation::ResizeWindow(value) => self.resize_window(
                context,
                lease,
                value.tab.as_deref(),
                value.width,
                value.height,
            ),
            Operation::Hover(value) => self.perform_hover(context, lease, value),
            Operation::FillForm(value) => self.perform_fill(context, lease, value),
            Operation::TypeText(value) => self.perform_type_text(context, lease, value),
            Operation::PressKey(value) => self.perform_key(context, lease, value),
            Operation::Drag(value) => self.perform_drag(context, lease, value),
            Operation::UploadFiles(value) => self.upload_files(context, lease, value),
            Operation::RunScript(value) => self.run_script(context, lease, value),
            Operation::Wait(value) => self.perform_wait(context, lease, value),
            Operation::RunFlow(value) => self.flow(context, lease, value),
            Operation::HandleDialog(value) => self.handle_dialog(context, lease, value),
            Operation::Diagnose(value) => self.diagnose(context, lease, value),
            Operation::Record(value) => self.perform_record(context, Some(lease), value),
            Operation::ExplainPolicy(_) => self.explain_policy(context),
        }
    }

    fn run_without_workspace_lease(
        &self,
        context: &InvocationContext<'_>,
        operation: &Operation,
    ) -> Terminal {
        if !matches!(operation, Operation::ExplainPolicy(_)) {
            if let Some(terminal) = self.audit_preflight(context) {
                return terminal;
            }
        }
        match operation {
            Operation::Record(value) => self.perform_record(context, None, value),
            Operation::ExplainPolicy(_) => self.explain_policy(context),
            _ => {
                unreachable!("only recording cleanup, client export, and policy explain bypass the workspace lease")
            }
        }
    }

    fn credential_guidance(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        selected: &SelectedTab,
        control: ControlRef,
    ) -> Terminal {
        Terminal::refused(
            context.invocation,
            Shape {
                status: Status::Blocked,
                effect: Effect::None,
                readiness: readiness(selected.readiness),
                repeat_safe: false,
            },
            &Refusal::CredentialAuthorization { control },
            json!({"tab":selected.handle.as_str(),"user_authorization_required":true,"values_sent":false}),
            decision,
            Some(selected.physical_id),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the landing check needs the lease, the tab, its commits and their requirements, and the success needs the outcome and facts"
    )]
    fn action_success(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        decision: Decision,
        landing_requirements: impl Into<CapabilitySet>,
        selected: &SelectedTab,
        physical: &PhysicalTab,
        commits: &[String],
        outcome: Outcome,
        mut facts: Value,
    ) -> Terminal {
        let landing_requirements = landing_requirements.into();
        let landing = self.authorize_commits(context, landing_requirements, physical, commits);
        if !landing.allowed {
            let hold = context.hold_of(landing, landing_requirements, &physical.url);
            return self.hold(context, lease, selected, hold, Effect::Applied);
        }
        let change = DocumentChange::between(&selected.url, physical, commits);
        let resulting = match change {
            DocumentChange::NewDocument => match lease.apply_landing(&selected.handle, physical) {
                Ok(tab) => {
                    self.emit(DomainEvent::DocumentCommitted {
                        invocation: context.invocation.into(),
                        workspace: context.workspace.as_str().into(),
                        tab: tab.handle.clone(),
                        physical_id: tab.physical_id,
                    });
                    tab
                }
                Err(error) => {
                    return self.workspace_failure_after_effect(
                        context,
                        error,
                        selected.physical_id,
                    )
                }
            },
            // The destination was governed above like any landing; only the handles survive.
            DocumentChange::SameDocument => {
                match lease.apply_same_document(&selected.handle, physical) {
                    Ok(tab) => tab,
                    Err(error) => {
                        return self.workspace_failure_after_effect(
                            context,
                            error,
                            selected.physical_id,
                        )
                    }
                }
            }
            DocumentChange::Unchanged => {
                let _ = lease.update_readiness(&selected.handle, physical.readiness);
                selected.clone()
            }
        };
        if let Some(object) = facts.as_object_mut() {
            if change != DocumentChange::Unchanged {
                object.insert("landing".into(), json!({"url":resulting.url,"title":resulting.title,"document_generation":resulting.generation}));
            }
        }
        self.succeeded(
            context,
            decision,
            Some(selected.physical_id),
            Effect::Applied,
            readiness(physical.readiness),
            false,
            outcome,
            facts,
        )
    }

    /// Hold a tab whose page governance just refused, and announce the hold.
    ///
    /// Every path that leaves its tab on a refused page comes here, a read, an action, and an
    /// open whose refused landing could not be undone, so the hold always shows. The tab then
    /// refuses reading and acting until a navigation lands it on an allowed page or it is closed
    /// under the tab-close setting alone (ADR-0203 Decisions 2.4 and 7.6). The hold keeps the
    /// refusal, so every later refusal on the tab names the refused host and records its reason
    /// (Decision 5.5).
    fn enter_hold(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        tab: &SelectedTab,
        hold: TabHold,
    ) {
        let _ = lease.hold_tab(&tab.handle, hold);
        self.emit(DomainEvent::HoldEntered {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            physical_id: tab.physical_id,
        });
    }

    /// Hold a tab whose page governance just refused, through [`Self::enter_hold`], and refuse
    /// the call that found it there. `effect` is what the call already did.
    fn hold(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        tab: &SelectedTab,
        hold: TabHold,
        effect: Effect,
    ) -> Terminal {
        let landing = hold.denial;
        let url = hold.url.clone();
        self.enter_hold(context, lease, tab, hold);
        self.blocked_at(
            context,
            landing,
            Some(tab.physical_id),
            effect,
            false,
            json!({"tab":tab.handle.as_str(),"held":true}),
            Some(&url),
        )
    }

    /// Resolve one controlled tab and authorize the frame's classified requirements on its page,
    /// executing the closure only when allowed (ADR-0176, ADR-0203 Decision 7.7).
    fn with_authorized_tab<F>(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        f: F,
    ) -> Terminal
    where
        F: FnOnce(&SelectedTab, Decision) -> Terminal,
    {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, context.requirements, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({}),
            );
        }
        f(&selected, decision)
    }

    /// Resolve one controlled tab and optional target and authorize the frame's classified
    /// requirements on its page, executing the closure only when allowed (ADR-0177, ADR-0203
    /// Decision 7.7).
    fn with_authorized_optional_target<F>(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        f: F,
    ) -> Terminal
    where
        F: FnOnce(&SelectedTab, Option<String>, Option<TargetRole>, Decision) -> Terminal,
    {
        let (selected, locator, role) =
            match self.resolve_optional_target(context, lease, requested_tab, target) {
                Ok(tuple) => tuple,
                Err(error) => return self.workspace_failure(context, error),
            };
        let decision = self.authorize(context, context.requirements, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({}),
            );
        }
        f(&selected, locator, role, decision)
    }

    fn resolve_optional_target(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
    ) -> Result<(SelectedTab, Option<String>, Option<TargetRole>), WorkspaceError> {
        match target {
            Some(target) => {
                let (tab, target) = self.resolve_target(context, lease, requested_tab, target)?;
                Ok((tab, Some(target.locator), Some(target.role)))
            }
            None => Ok((lease.select_tab(requested_tab)?, None, None)),
        }
    }

    fn resolve_target(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: &str,
    ) -> Result<(SelectedTab, SelectedTarget), WorkspaceError> {
        let result = match requested_tab {
            Some(requested) => {
                let tab = lease.select_tab(Some(requested))?;
                lease
                    .resolve_target(target, Some(&tab))
                    .map(|resolved| (tab, resolved))
            }
            None => {
                let target = lease.resolve_target(target, None)?;
                let tab = lease.select_tab(Some(target.tab.as_str()))?;
                Ok((tab, target))
            }
        };
        // A stale handle is where recovery is cheapest: the page still has controls matching
        // what that handle used to be called. Offer up to three of them so the refusal
        // arrives pre-recovered instead of sending the driver back to inspect.
        if let Err(WorkspaceError::StaleTarget { reason }) = &result {
            self.recover_stale(context, target, *reason, None);
        }
        result
    }

    /// Best-effort recovery for a stale target handle (ADR-0198 Decision 28). Silence on any
    /// failure or governance denial is deliberate: the refusal stays truthful without it.
    ///
    /// Controls are offered only after a read authorization on the tab's URL, and each is
    /// registered as a handle. A control the page reported removed brings the lookup the page
    /// made in the failing call, which proves at most one candidate is the same control. Any
    /// other stale handle is looked up by what the page called it, in a separate unpainted read.
    /// The proven replacement is named only for a reason `removed`, for a call that takes the
    /// stale handle as its one target, and from a lookup no cap cut short. A stale-locator
    /// failure is raised before any effect, so nothing was sent.
    fn recover_stale(
        &self,
        context: &InvocationContext<'_>,
        target_handle: &str,
        reason: StaleReason,
        lookup: Option<&StaleLookup>,
    ) {
        let Some(info) = self
            .workspaces
            .stale_target_context(context.workspace, target_handle)
        else {
            return;
        };
        if !self
            .authorize(context, Capability::Read, Some(info.url.as_str()))
            .allowed
        {
            return;
        }
        let lookup = lookup.filter(|_| reason == StaleReason::Removed);
        let (found, proven) = match lookup {
            Some(lookup) => (
                lookup
                    .candidates
                    .iter()
                    .map(|candidate| candidate.target.clone())
                    .collect(),
                proven_candidate(context, target_handle, lookup),
            ),
            None => (self.query_stale_candidates(context, &info), None),
        };
        let registered = self.workspaces.register_recovered(
            context.workspace,
            target_handle,
            lookup.is_some(),
            &found,
        );
        if registered.is_empty() {
            return;
        }
        let preserve = context.snapshot.preserves_target_names();
        let mut candidates: Vec<Value> = registered
            .iter()
            .map(|(handle, target)| candidate_item(handle.as_str(), target, preserve))
            .collect();
        // Repeats are listed in the order the page has them. The replacement is found by its
        // position in `registered`, which stays as it was, so the order shown does not move it.
        candidates.sort_by_key(|item| item["nth"].as_u64().unwrap_or(u64::MAX));
        let replacement = proven
            .and_then(|index| registered.get(index))
            .map(|(handle, _)| handle.as_str().to_owned());
        context.findings.offer(StaleOffer {
            candidates,
            replacement,
        });
    }

    /// Up to three controls the page names like a stale handle's control, from a separate
    /// unpainted semantic query. Empty on any failure.
    fn query_stale_candidates(
        &self,
        context: &InvocationContext<'_>,
        info: &StaleTargetContext,
    ) -> Vec<ObservedTarget> {
        if info.name.trim().is_empty() {
            return Vec::new();
        }
        match self.dispatch(
            context,
            BrowserCommand::QuerySemantic(PhysicalSemantic {
                tab_id: info.physical_id,
                name: info.name.clone(),
                role: Some(info.role_page.clone()),
                exact: false,
                form_scope: false,
                max_results: SELECTOR_MATCH_MAX,
            }),
        ) {
            Ok(BrowserOutcome::Targets { targets, .. }) => {
                targets.into_iter().take(STALE_CANDIDATES_MAX).collect()
            }
            _ => Vec::new(),
        }
    }

    /// Check one optional postcondition after an applied effect.
    ///
    /// A check that does not confirm the expectation never erases the acknowledged action. The
    /// result stays applied and never repeat-safe, and one typed refusal names both the action
    /// and why the check fell short, for the caller and the audit alike.
    #[expect(
        clippy::too_many_arguments,
        reason = "the landing check needs the lease, the tab, its commits and their requirements, the success needs the outcome and facts, and the check needs the postcondition"
    )]
    fn finish_with_expectation(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        decision: Decision,
        landing_requirements: impl Into<CapabilitySet>,
        selected: &SelectedTab,
        physical: &PhysicalTab,
        commits: &[String],
        outcome: Outcome,
        facts: Value,
        expect: Option<&crate::language::Postcondition>,
    ) -> Terminal {
        let Some(expectation) = expect else {
            return self.action_success(
                context,
                lease,
                decision,
                landing_requirements,
                selected,
                physical,
                commits,
                outcome,
                facts,
            );
        };
        // Govern and retain the landed action before attempting a separate observation.
        let applied = self.action_success(
            context,
            lease,
            decision,
            landing_requirements,
            selected,
            physical,
            commits,
            outcome.clone(),
            facts,
        );
        if applied.result.status != Status::Succeeded {
            return applied;
        }
        let remaining = context.deadline.saturating_duration_since(Instant::now());
        let budget = remaining.min(EXPECTATION_OBSERVE_BUDGET).as_millis() as u64;
        let (cause, decision) = match self.dispatch(
            context,
            BrowserCommand::Observe(PhysicalWait {
                tab_id: selected.physical_id,
                condition: expectation.condition,
                value: expectation.value.clone(),
                locator: None,
                timeout_ms: budget,
            }),
        ) {
            Ok(BrowserOutcome::Observed {
                satisfied: true, ..
            }) => return applied,
            Ok(BrowserOutcome::Observed {
                satisfied: false, ..
            }) => (CheckFailure::Unmet, decision),
            Ok(_) => (CheckFailure::BrowserRefused, decision),
            Err(error) => unchecked(&error, decision),
        };
        self.unconfirmed(context, decision, applied, outcome, cause)
    }

    /// Report an applied action whose requested check did not confirm the expected page state.
    ///
    /// Everything is built once from the typed refusal: the summary keeps the action's own
    /// sentence, the facts keep the action's facts, and the audit keeps both.
    fn unconfirmed(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        applied: Terminal,
        action: Outcome,
        cause: CheckFailure,
    ) -> Terminal {
        let refusal = Refusal::ActionUnconfirmed {
            action: Box::new(action),
            cause,
            code: decision.reason,
        };
        let facts = applied.result.facts;
        Terminal::refused(
            context.invocation,
            Shape {
                status: unconfirmed_status(cause),
                effect: Effect::Applied,
                readiness: applied.result.readiness,
                repeat_safe: false,
            },
            &refusal,
            with_denial_facts(decision, facts),
            decision,
            applied.physical_id,
        )
    }

    /// Resolve one typed semantic selector through a single adapter query.
    /// Zero or several matches fail without any effect; exactly one match is
    /// registered as an ordinary generation-bound target. Named controls use the
    /// same physical scope as handles; no implicit form ancestry is required.
    fn resolve_semantic(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        selector: &crate::language::SemanticSelector,
    ) -> Result<(SelectedTab, SelectedTarget), Box<Terminal>> {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return Err(Box::new(self.workspace_failure(context, error))),
        };
        let decision = self.authorize(context, context.requirements, Some(selected.url.as_str()));
        if !decision.allowed {
            return Err(Box::new(self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({}),
            )));
        }
        match self.dispatch(
            context,
            BrowserCommand::QuerySemantic(PhysicalSemantic {
                tab_id: selected.physical_id,
                name: selector.name.clone(),
                role: selector.role.clone(),
                exact: selector.exact,
                form_scope: false,
                max_results: SELECTOR_MATCH_MAX,
            }),
        ) {
            Ok(BrowserOutcome::Targets {
                tab_id,
                targets,
                listing,
            }) if tab_id == selected.physical_id => {
                if targets.len() != 1 {
                    let matched = targets.len();
                    let outcome = Outcome::SelectorUnresolved {
                        matched,
                        more: listing.truncated,
                    };
                    return Err(Box::new(Terminal::completed(
                        context.invocation,
                        Shape {
                            status: Status::Failed,
                            effect: Effect::None,
                            readiness: readiness(selected.readiness),
                            repeat_safe: true,
                        },
                        &outcome,
                        json!({"tab":selected.handle.as_str(),"selector_matched":matched}),
                        decision,
                        Some(selected.physical_id),
                    )));
                }
                let observed = &targets[0];
                let registered = lease
                    .register_targets(&selected, std::slice::from_ref(observed))
                    .map_err(|error| Box::new(self.workspace_failure(context, error)))?;
                let (handle, _) = registered
                    .into_iter()
                    .next()
                    .expect("one observed target registers one handle");
                match self.resolve_target(
                    context,
                    lease,
                    Some(selected.handle.as_str()),
                    handle.as_str(),
                ) {
                    Ok(value) => Ok(value),
                    Err(error) => Err(Box::new(self.workspace_failure(context, error))),
                }
            }
            Ok(_) => Err(Box::new(self.protocol_failure(
                context,
                decision,
                Some(selected.physical_id),
            ))),
            Err(error) => Err(Box::new(self.browser_failure(
                context,
                decision,
                error,
                Some(selected.physical_id),
            ))),
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a location is a target handle or a view point, and each form keeps its own optional arguments"
    )]
    fn resolve_location(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        view: Option<&str>,
        x: Option<f64>,
        y: Option<f64>,
    ) -> Result<ResolvedLocation, WorkspaceError> {
        if let Some(target) = target {
            let (tab, target) = self.resolve_target(context, lease, requested_tab, target)?;
            return Ok(ResolvedLocation::Target { tab, target });
        }
        let view = view.expect("language validated view location");
        let x = x.expect("language validated x coordinate");
        let y = y.expect("language validated y coordinate");
        if let Some(requested) = requested_tab {
            let tab = lease.select_tab(Some(requested))?;
            let (view, point) = lease.resolve_view_point(view, Some(&tab), x, y)?;
            Ok(ResolvedLocation::Point { tab, view, point })
        } else {
            let (view, point) = lease.resolve_view_point(view, None, x, y)?;
            let tab = lease.select_tab(Some(view.tab.as_str()))?;
            Ok(ResolvedLocation::Point { tab, view, point })
        }
    }

    /// The one browser seam.
    ///
    /// Every model-requested browser command crosses here, which is why the frame's
    /// observation is gathered here rather than at each of the call sites that funnel into it. A
    /// tool written tomorrow is observed for free; a tool that had to remember would not be.
    fn dispatch(
        &self,
        context: &InvocationContext<'_>,
        command: BrowserCommand,
    ) -> Result<BrowserOutcome, BrowserError> {
        let outcome = match self.dispatch_documents(context, command) {
            Ok(BrowserOutcome::EffectUnknown { reason }) => {
                Err(BrowserError::EffectUnknown(reason))
            }
            outcome => outcome,
        };
        if let Ok(outcome) = &outcome {
            context.findings.observe(observed_from(outcome));
        }
        outcome
    }

    fn dispatch_physical(
        &self,
        context: &InvocationContext<'_>,
        command: BrowserCommand,
    ) -> Result<BrowserOutcome, BrowserError> {
        let browser = self.admitted_browser(context)?;
        self.confirm_crossing(context, &browser, addressed_tab(&command))?;
        self.presentation.bind_command(
            context.workspace.as_str(),
            context.invocation.as_str(),
            &command,
        );
        let outcome = self.send(
            context,
            &browser,
            command,
            context.deadline,
            context.cancellation.flag(),
        );
        // An adapter that reports effect-unknown has answered honestly. Route it through the
        // truthful unknown rendering instead of letting per-family receipt matching mistake it
        // for an incompatible receipt.
        match outcome {
            Ok(BrowserOutcome::EffectUnknown { reason }) => {
                Err(BrowserError::EffectUnknown(reason))
            }
            outcome => outcome,
        }
    }

    /// Send one command to `browser` under runtime control, and remember in the frame's findings
    /// that it reached the browser when the port wrote any of the command, whatever the outcome.
    ///
    /// The name is read before sending, so a browser that disconnects after taking the command is
    /// still named in the record of the work it took part in.
    ///
    /// A command addressed to one tab is admitted only while that tab is one this workspace
    /// controls in `browser` ([`Self::admit_crossing`]). A port asks admission for a command that may
    /// change the page, once it holds the connection it writes to. A read it never asked about may
    /// have crossed to a connection that replaced the one its tab was confirmed on, so its tab is
    /// confirmed again before its answer is used, and an answer from a tab the browser no longer
    /// confirms is dropped as [`BrowserError::StaleTab`].
    fn send(
        &self,
        context: &InvocationContext<'_>,
        browser: &str,
        command: BrowserCommand,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<BrowserOutcome, BrowserError> {
        let name = self
            .browser
            .browsers()
            .into_iter()
            .find(|summary| summary.id == browser)
            .map(|summary| summary.display_name());
        let tab = addressed_tab(&command);
        let written = AtomicBool::new(false);
        let asked = AtomicBool::new(false);
        let outcome = self.browser.call_guarded(
            browser,
            command,
            crate::browser::BrowserDispatch {
                deadline,
                cancelled,
                admit: &|| {
                    asked.store(true, Ordering::SeqCst);
                    self.admit_dispatch(context)?;
                    tab.map_or(Ok(()), |tab| self.admit_crossing(context, browser, tab))
                },
                written: &written,
            },
        );
        if written.load(Ordering::SeqCst) {
            context.findings.reach(findings::Reached {
                id: browser.into(),
                name,
            });
        }
        if outcome.is_ok() && !asked.load(Ordering::SeqCst) {
            self.confirm_crossing(context, browser, tab)?;
        }
        outcome
    }

    fn compensate_close(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        tab: &SelectedTab,
    ) -> CloseCompensation {
        if !self.authorize_close_setting(context, None).allowed {
            return CloseCompensation::Retained;
        }
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + CLOSE_COMPENSATION_BUDGET;
        // The tab exists, so the workspace is already bound to the browser holding it. Undoing an
        // effect is never the moment to choose a browser.
        let Some(browser) = self.workspaces.browser_of(context.workspace.as_str()) else {
            return CloseCompensation::Unknown;
        };
        match self.send(
            context,
            &browser,
            BrowserCommand::CloseTab {
                tab_id: tab.physical_id,
                released: false,
            },
            deadline,
            &cancelled,
        ) {
            Ok(BrowserOutcome::TabClosed { tab_id }) if tab_id == tab.physical_id => {
                if lease.forget_closed_tab(&tab.handle).is_ok() {
                    CloseCompensation::Closed
                } else {
                    CloseCompensation::Unknown
                }
            }
            Err(error) if !error.effect_unknown() => CloseCompensation::Retained,
            _ => CloseCompensation::Unknown,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per field the success result states, plus the tab whose dialog state it checks"
    )]
    fn succeeded(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        effect: Effect,
        readiness: Readiness,
        repeat_safe: bool,
        outcome: Outcome,
        mut facts: Value,
    ) -> Terminal {
        let outcome = match physical_id
            .filter(|_| outcome.can_open_dialog())
            .and_then(|physical_id| self.workspaces.dialog_open(context.workspace, physical_id))
        {
            Some(dialog) => {
                facts["dialog_opened"] = json!(dialog.name());
                outcome.with_open_dialog(dialog)
            }
            None => outcome,
        };
        // One structural bool: the sentence is the action's usual one, and no handle, row or
        // link goes into the facts (ADR-0198 Decision 17).
        if outcome.rebound() {
            facts["rebound"] = json!(true);
        }
        Terminal::completed(
            context.invocation,
            Shape {
                status: Status::Succeeded,
                effect,
                readiness,
                repeat_safe,
            },
            &outcome,
            facts,
            decision,
            physical_id,
        )
    }

    /// The decision a refusal on a tab under `hold` records (ADR-0203 Decision 5.5).
    ///
    /// The hold's own decision names its policy layer and grant by index into the authority that
    /// made it, so it is recorded only under that authority. Under another, the held page is
    /// decided again. When that authority admits the page, the tab is still held until it leaves
    /// it (Decision 2.4), and the record keeps the hold's reason with no policy attribution.
    fn held_decision(&self, context: &InvocationContext<'_>, hold: &TabHold) -> Decision {
        if hold.authority == context.snapshot.id() {
            return hold.denial;
        }
        let current = self.check_landing(context, hold.requirements, &hold.url);
        if current.allowed {
            Decision::refused(hold.denial.reason)
        } else {
            current
        }
    }

    /// The session's tabs an `ambiguous_tab` refusal lists, with the facts the session already
    /// saw for each: its handle, last governed title and URL.
    fn candidate_tabs(&self, context: &InvocationContext<'_>) -> Value {
        let tabs = self.workspaces.tabs(context.workspace).unwrap_or_default();
        Value::Array(
            tabs.into_iter()
                .map(|tab| json!({"tab":tab.handle.as_str(),"title":tab.title,"url":tab.url}))
                .collect(),
        )
    }

    fn emit(&self, event: DomainEvent) {
        self.presentation.react(&event);
        self.workbench.react(&event);
    }

    /// Say which control on tab `physical_id` a governed physical action's page feedback belongs
    /// to, so the feedback lands on it. Each caller picks the moment (see
    /// `DomainEvent::TargetIndicated`). Only a click describes its `click` shape.
    fn indicate_target(
        &self,
        context: &InvocationContext<'_>,
        physical_id: u64,
        locator: &str,
        click: Option<crate::browser::adapter::ClickShape>,
    ) {
        self.emit(DomainEvent::TargetIndicated {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            physical_id,
            locator: locator.into(),
            click,
        });
    }
}

/// The notice a refused frame shows, read from the decision and refusal it carries rather than
/// from its tool, so a flow that a refused tab close stopped shows the kept-tab notice too.
fn denial_presentation(terminal: &Terminal) -> DenialPresentation {
    if terminal.decision.reason == ReasonCode::TabCloseDenied {
        DenialPresentation::TabKeptOpenByPolicy
    } else if terminal.audit.refusal()
        == Some(&crate::language::audit::AuditRefusal::LocalInterlock)
    {
        DenialPresentation::TabKeptOpenBySetting
    } else {
        DenialPresentation::Guardrail
    }
}

/// What the single completion path needs to record one frame's terminal outcome.
///
/// These travel together and only together, so they arrive as one value rather than as a
/// growing parameter list on `finish`.
struct Completion<'a> {
    workspace: &'a WorkspaceId,
    tool: &'a str,
    requirements: CapabilitySet,
    snapshot: &'a AuthoritySnapshot,
    /// Measured span from decode to terminal outcome. For a navigation this is time to settle.
    duration_ms: u64,
    /// Immutable evidence from this operation's original connection.
    provenance: &'a ConnectionEvidence,
    /// Everything the frame learned, taken once for its record.
    findings: Findings,
}

impl<'a> Completion<'a> {
    /// What the frame `context` records as `tool`, timed from `started`, with the findings it
    /// gathered. Taking the findings leaves the frame with none.
    fn of(context: &InvocationContext<'a>, tool: &'a str, started: Instant) -> Self {
        Self {
            workspace: context.workspace,
            tool,
            requirements: context.requirements,
            snapshot: context.snapshot,
            duration_ms: elapsed_ms(started),
            provenance: context.provenance,
            findings: context.findings.take(),
        }
    }
}

/// The terminal of `invocation` when it was cancelled by `by` before it started: nothing ran,
/// so it is safe to repeat.
fn cancelled_before_start(invocation: &InvocationId, by: CancelCause) -> Terminal {
    Terminal::refused(
        invocation,
        Shape {
            status: Status::Cancelled,
            effect: Effect::None,
            readiness: Readiness::NotApplicable,
            repeat_safe: true,
        },
        &Refusal::CancelledBeforeStart { by },
        json!({}),
        Decision::permitted(),
        None,
    )
}

/// Milliseconds elapsed since an invocation began, saturating rather than wrapping.
fn elapsed_ms(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// What one frame runs with: its request's shared facts and its own operation's facts.
///
/// A frame is a request's top-level operation ([`InvocationContext::of`]) or one flow step
/// ([`InvocationContext::step`]). An override inside one frame keeps `..*context`, and with it
/// that frame's findings.
#[derive(Clone, Copy)]
struct InvocationContext<'a> {
    /// Whether this frame's preparation budget is still unspent. The first dispatch that settles
    /// takes it.
    settlement_pending: &'a AtomicBool,
    /// The agent's intent: false only when the call said `visual_settle: false`. It never
    /// changes once the budget is spent, so a spent budget is not read as an opt-out.
    settlement_requested: bool,
    requirements: CapabilitySet,
    /// Immutable evidence from the connection that sent this call.
    provenance: &'a ConnectionEvidence,
    invocation: &'a InvocationId,
    workspace: &'a WorkspaceId,
    /// The browser this call named, when it named one.
    ///
    /// Only a call that can open the first tab of a workspace can carry one. Every other call
    /// reaches the browser through a handle that already belongs to it.
    requested_browser: Option<&'a str>,
    requested_tab: Option<&'a str>,
    requested_target: Option<&'a str>,
    /// The catalog tool of this call when it takes `requested_target` as its one target and can
    /// be repeated with another handle in its place.
    repeat_tool: Option<&'static str>,
    /// What this call does to a control, which a page refusal says did not happen. A flow child
    /// carries its own.
    gesture: PageGesture,
    snapshot: &'a AuthoritySnapshot,
    /// The publication `snapshot` came from, so what this frame reports of the authority in
    /// force describes the snapshot it enforces (ADR-0203 Decision 7.5).
    publication: &'a Publication,
    deadline: Instant,
    /// Whether the top-level call that set `deadline` could be repeated with a longer
    /// `timeout_ms`.
    ///
    /// Flow children inherit it with the deadline, because the flow's own `timeout_ms` is the
    /// field that would give them more time.
    deadline_adjustable: bool,
    cancellation: &'a CancellationToken,
    /// What this frame learns, read once by its own completion.
    findings: &'a FrameFindings,
}

impl<'a> InvocationContext<'a> {
    /// The top-level frame of `prepared`, running its decoded `operation` under the snapshot of
    /// `publication`.
    fn of(
        prepared: &'a PreparedInvocation,
        operation: &'a Operation,
        publication: &'a Publication,
        frame: &'a Frame,
    ) -> Self {
        Self {
            settlement_pending: &frame.settlement_pending,
            settlement_requested: language::settlement::enabled(operation),
            requirements: language::capability_map::requirements(operation),
            provenance: &prepared.provenance,
            invocation: &prepared.id,
            workspace: &prepared.workspace,
            requested_browser: operation_browser(operation),
            requested_tab: operation_tab(operation),
            requested_target: operation_target(operation),
            repeat_tool: repeatable_tool(operation),
            gesture: PageGesture::of(operation),
            snapshot: publication.snapshot(),
            publication,
            deadline: prepared.deadline,
            deadline_adjustable: prepared.deadline_adjustable,
            cancellation: &prepared.cancellation,
            findings: &frame.findings,
        }
    }

    /// The frame of one flow step running `operation` inside this flow frame.
    ///
    /// The step shares its flow's id, workspace, caller, deadline, cancellation and authority
    /// snapshot. It owns its operation's requirements, requested browser, tab and target, repeat
    /// tool, gesture and settlement, and its own findings, so it answers as the same call outside
    /// a flow would (ADR-0202 Decision 13).
    fn step<'b>(&self, operation: &'b Operation, frame: &'b Frame) -> InvocationContext<'b>
    where
        'a: 'b,
    {
        InvocationContext {
            settlement_pending: &frame.settlement_pending,
            settlement_requested: language::settlement::enabled(operation),
            requirements: language::capability_map::requirements(operation),
            requested_browser: operation_browser(operation),
            requested_tab: operation_tab(operation),
            requested_target: operation_target(operation),
            repeat_tool: repeatable_tool(operation),
            gesture: PageGesture::of(operation),
            findings: &frame.findings,
            ..*self
        }
    }

    /// The hold a refusal of the page at `url` puts on its tab: `landing`, decided by this
    /// frame's authority snapshot for `requirements`.
    fn hold_of(
        &self,
        landing: Decision,
        requirements: impl Into<CapabilitySet>,
        url: &str,
    ) -> TabHold {
        TabHold {
            denial: landing,
            authority: self.snapshot.id().into(),
            requirements: requirements.into(),
            url: url.into(),
        }
    }
}

struct Terminal {
    result: InvocationResult,
    decision: Decision,
    physical_id: Option<u64>,
    observed: Observed,
    audit: AuditProjection,
}

/// How a result is shaped apart from its words: its status, its effect, the page's readiness,
/// and whether repeating the call is safe.
#[derive(Clone, Copy)]
struct Shape {
    status: Status,
    effect: Effect,
    readiness: Readiness,
    repeat_safe: bool,
}

impl Terminal {
    /// The terminal of `refusal` (ADR-0203 Decision 5.1). Its sentence, next steps, observation
    /// and audit come from the refusal, and the refusal's model code is the `reason` fact beside
    /// `facts`, so no call site writes that code.
    fn refused(
        invocation: &InvocationId,
        shape: Shape,
        refusal: &Refusal,
        mut facts: Value,
        decision: Decision,
        physical_id: Option<u64>,
    ) -> Self {
        facts["reason"] = Value::from(refusal.reason());
        Self {
            result: InvocationResult::new(
                invocation,
                shape.status,
                shape.effect,
                shape.readiness,
                shape.repeat_safe,
                &refusal.summary(),
                facts,
                refusal.next_steps(),
            ),
            decision,
            physical_id,
            observed: refusal.observed(),
            audit: refusal.audit(),
        }
    }

    /// The terminal of `outcome`: its sentence, next steps, observation and audit come from the
    /// one typed outcome (AGENTS.md), so no call site pairs them by hand.
    fn completed(
        invocation: &InvocationId,
        shape: Shape,
        outcome: &Outcome,
        facts: Value,
        decision: Decision,
        physical_id: Option<u64>,
    ) -> Self {
        Self {
            result: InvocationResult::new(
                invocation,
                shape.status,
                shape.effect,
                shape.readiness,
                shape.repeat_safe,
                &outcome.summary(),
                facts,
                outcome.next_steps(),
            ),
            decision,
            physical_id,
            observed: outcome.observed(),
            audit: outcome.audit(),
        }
    }
}

enum ResolvedLocation {
    Target {
        tab: SelectedTab,
        target: SelectedTarget,
    },
    Point {
        tab: SelectedTab,
        view: SelectedView,
        point: PhysicalPoint,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CloseCompensation {
    Closed,
    Retained,
    Unknown,
}

impl ResolvedLocation {
    fn tab(&self) -> SelectedTab {
        match self {
            Self::Target { tab, .. } | Self::Point { tab, .. } => tab.clone(),
        }
    }
}

fn operation_requires_workspace_lease(operation: &Operation) -> bool {
    !matches!(
        operation,
        Operation::Record(Record {
            action,
            target: None,
            ..
        }) if matches!(
            action,
            RecordAction::Status | RecordAction::Stop | RecordAction::Save | RecordAction::Discard
        )
    ) && !matches!(operation, Operation::ExplainPolicy(_))
}

fn recording_facts(summary: &PhysicalRecordingSummary) -> Value {
    json!({
        "state":recording_state_name(summary.state),
        "frame_count":summary.frame_count,
        "bytes_held":summary.bytes_held,
        "duration_ms":summary.duration_ms,
        "hard_expires_unix_ms":summary.hard_expires_unix_ms,
        "retention_expires_unix_ms":summary.retention_expires_unix_ms,
        "stop_reason":summary.stop_reason.map(recording_stop_reason_name)
    })
}

/// Base name for every saved replay. A downloaded file lands wherever the browser puts
/// downloads; Ghostlight names the artifact and never a path.
const RECORDING_FILE_NAME: &str = "ghostlight-recording.gif";

const fn recording_delivery_name(delivery: &RecordingDelivery) -> &'static str {
    match delivery {
        RecordingDelivery::Attached { .. } => "attached_to_page",
        RecordingDelivery::Downloaded => "downloaded_by_browser",
        RecordingDelivery::Returned { .. } => "returned_to_client",
    }
}

const fn recording_state_name(state: RecordingState) -> &'static str {
    match state {
        RecordingState::Recording => "recording",
        RecordingState::Frozen => "frozen",
        RecordingState::Interrupted => "interrupted",
    }
}

const fn recording_stop_reason_name(reason: RecordingStopReason) -> &'static str {
    match reason {
        RecordingStopReason::DocumentBoundary => "document_boundary",
        RecordingStopReason::Explicit => "explicit",
        RecordingStopReason::HardTimeout => "hard_timeout",
        RecordingStopReason::MemoryLimit => "memory_limit",
        RecordingStopReason::BrowserDetached => "browser_detached",
        RecordingStopReason::RuntimeHeld => "runtime_held",
        RecordingStopReason::ServiceDisconnected => "service_disconnected",
        RecordingStopReason::FrameTooLarge => "frame_too_large",
    }
}

fn operation_activity(operation: &Operation) -> PresentationActivity {
    match operation {
        Operation::ListTabs(_) | Operation::ActivateTab(_) | Operation::CloseTab(_) => {
            PresentationActivity::Quiet
        }
        Operation::OpenPage(_)
        | Operation::NavigatePage(_)
        | Operation::NavigateHistory(_)
        | Operation::ReloadPage(_) => PresentationActivity::Navigate,
        Operation::ReadPage(_) | Operation::InspectPage(_) => PresentationActivity::Read,
        Operation::Find(_) => PresentationActivity::Find,
        Operation::TakeScreenshot(_) => PresentationActivity::Screenshot,
        Operation::Click(_) => PresentationActivity::Click,
        Operation::ScrollPage(_) => PresentationActivity::Scroll,
        Operation::SetZoom(_) | Operation::ResizeWindow(_) => PresentationActivity::Zoom,
        Operation::Hover(_) => PresentationActivity::Hover,
        Operation::FillForm(_) => PresentationActivity::Fill,
        Operation::TypeText(_) => PresentationActivity::Type,
        Operation::PressKey(_) => PresentationActivity::Key,
        Operation::Drag(_) => PresentationActivity::Drag,
        Operation::UploadFiles(_) => PresentationActivity::Upload,
        Operation::RunScript(_) => PresentationActivity::Script,
        Operation::Wait(_) => PresentationActivity::Wait,
        Operation::RunFlow(_) => PresentationActivity::Quiet,
        Operation::HandleDialog(_) => PresentationActivity::Dialog,
        Operation::Diagnose(_) => PresentationActivity::Quiet,
        Operation::ExplainPolicy(_) => PresentationActivity::Quiet,
        Operation::Record(value) if value.action == RecordAction::Start => {
            PresentationActivity::Screenshot
        }
        Operation::Record(_) => PresentationActivity::Quiet,
    }
}

/// The browser one operation named, if its shape can name one.
///
/// Only opening a page can carry a selection, because only opening a page can be the first work a
/// workspace ever does. Everything else arrives holding a handle that already names its browser.
fn operation_browser(operation: &Operation) -> Option<&str> {
    match operation {
        Operation::OpenPage(value) => value.browser.as_deref(),
        _ => None,
    }
}

/// The tab handle named by this operation, if its parameters name one.
fn operation_tab(operation: &Operation) -> Option<&str> {
    match operation {
        Operation::ActivateTab(value) => Some(value.tab.as_str()),
        Operation::CloseTab(value) => Some(value.tab.as_str()),
        Operation::NavigatePage(value) => value.tab.as_deref(),
        Operation::NavigateHistory(value) => value.tab.as_deref(),
        Operation::ReloadPage(value) => value.tab.as_deref(),
        Operation::ReadPage(value) => value.tab.as_deref(),
        Operation::InspectPage(value) => value.tab.as_deref(),
        Operation::Find(value) => value.tab.as_deref(),
        Operation::TakeScreenshot(value) => value.tab.as_deref(),
        Operation::Click(value) => value.tab.as_deref(),
        Operation::ScrollPage(value) => value.tab.as_deref(),
        Operation::SetZoom(value) => value.tab.as_deref(),
        Operation::ResizeWindow(value) => value.tab.as_deref(),
        Operation::Hover(value) => value.tab.as_deref(),
        Operation::FillForm(value) => value.tab.as_deref(),
        Operation::TypeText(value) => value.tab.as_deref(),
        Operation::PressKey(value) => value.tab.as_deref(),
        Operation::Drag(value) => value.tab.as_deref(),
        Operation::UploadFiles(value) => value.tab.as_deref(),
        Operation::RunScript(value) => value.tab.as_deref(),
        Operation::Wait(value) => value.tab.as_deref(),
        Operation::HandleDialog(value) => value.tab.as_deref(),
        Operation::Record(value) => value.tab.as_deref(),
        Operation::Diagnose(value) => value.tab.as_deref(),
        _ => None,
    }
}

/// The target handle named by this operation, if its parameters name exactly one. A fill_form
/// field list and a drag name several, so they have none.
fn operation_target(operation: &Operation) -> Option<&str> {
    match operation {
        Operation::ReadPage(value) => value.target.as_deref(),
        Operation::Click(value) => value.target.as_deref(),
        Operation::ScrollPage(value) => value.target.as_deref(),
        Operation::Hover(value) => value.target.as_deref(),
        Operation::PressKey(value) => value.target.as_deref(),
        Operation::Wait(value) => value.target.as_deref(),
        Operation::TypeText(value) => {
            Some(value.target.as_str()).filter(|target| !target.is_empty())
        }
        Operation::UploadFiles(value) => value.target.as_deref(),
        Operation::TakeScreenshot(value) => value.target.as_deref(),
        Operation::Record(value) => value.target.as_deref(),
        _ => None,
    }
}

/// The catalog tool of a call that takes exactly one target and can be repeated with another
/// handle in its place. A fill's fields and a drag's ends name several targets, so they have none.
const fn repeatable_tool(operation: &Operation) -> Option<&'static str> {
    match operation {
        Operation::Click(_)
        | Operation::Hover(_)
        | Operation::TypeText(_)
        | Operation::PressKey(_)
        | Operation::ScrollPage(_)
        | Operation::UploadFiles(_)
        | Operation::Wait(_) => Some(operation.name()),
        _ => None,
    }
}

/// What a stale-target refusal tells the model about the controls it found. A replacement is named
/// only for a removed control, in a call that sent nothing and that can be repeated with it.
fn stale_help(
    context: &InvocationContext<'_>,
    reason: StaleReason,
    offer: Option<&StaleOffer>,
    acted_on: Option<u64>,
) -> StaleHelp {
    let Some(offer) = offer else {
        return StaleHelp::None;
    };
    match (reason, &offer.replacement, context.repeat_tool) {
        (StaleReason::Removed, Some(target), Some(tool)) if acted_on.is_none() => {
            StaleHelp::Replacement {
                tool,
                target: target.clone(),
            }
        }
        _ => StaleHelp::Candidates {
            named: context.snapshot.preserves_target_names(),
        },
    }
}

/// The position among a lookup's candidates of the one control the page proved is the control
/// the stale handle named, when the call can be repeated with it: the tool takes the handle as
/// its one target, no cap cut the lookup short, and exactly one control passed every test.
fn proven_candidate(
    context: &InvocationContext<'_>,
    stale_handle: &str,
    lookup: &StaleLookup,
) -> Option<usize> {
    if context.repeat_tool.is_none()
        || context.requested_target != Some(stale_handle)
        || lookup.truncated
        || lookup.same_count != 1
    {
        return None;
    }
    lookup
        .candidates
        .iter()
        .position(|candidate| candidate.same)
}

/// What a click landed on, so the completed sentence can say which it was.
enum Clicked {
    Target(TargetRole),
    Point(PhysicalPoint),
}

/// Turn one physical action receipt into the single governed language subject.
///
/// The browser reports what it actually acted upon. A semantic handle supplies only the fallback
/// role for an older or unobservable receipt; no second browser description is requested for log
/// wording.
fn action_subject(
    context: &InvocationContext<'_>,
    physical: Option<PhysicalActionSubject>,
    fallback_role: Option<TargetRole>,
) -> Option<ActionSubject> {
    physical
        .map(|subject| {
            ActionSubject::from_page(
                &subject.role,
                &subject.name,
                context.snapshot.preserves_target_names(),
            )
            .rebound(subject.rebound)
        })
        .or_else(|| fallback_role.map(ActionSubject::unnamed))
}

/// Say the action reached its subject by a handle that followed its control, when the call that
/// described the target before the action did.
///
/// Describing comes first, so the action's own call finds the control live and reports nothing;
/// the description's fact is the one that survives (ADR-0198 Decision 17).
fn followed(subject: Option<ActionSubject>, described_rebound: bool) -> Option<ActionSubject> {
    subject.map(|subject| subject.also_rebound(described_rebound))
}

/// Name a key only when it is one of the catalog's named keys.
///
/// A single literal character is the caller's own text. The audit keeps the caller's intent, not
/// the caller's payload, so "Pressed a key" is as much as a one-character press gets to say.
fn named_key(key: &str) -> Option<String> {
    (key.chars().count() > 1).then(|| key.to_owned())
}

/// The structured `reason` fact of an applied action whose check did not confirm it.
///
/// A governed refusal keeps the governance reason every other refusal reports.
// Physical work must finish early enough for the extension, native relay, browser port, executor,
// and MCP edge to return one decisive receipt. A quarter second was too narrow on a live
// cache-bypassing reload and turned a normal timeout into an uncertain after-dispatch deadline.
const ADAPTER_RECEIPT_RESERVE_MS: u64 = 750;

fn adapter_budget_ms(requested_ms: u64, remaining: Duration) -> u64 {
    let available = remaining.saturating_sub(Duration::from_millis(ADAPTER_RECEIPT_RESERVE_MS));
    let available_ms = u64::try_from(available.as_millis()).unwrap_or(u64::MAX);
    requested_ms.min(available_ms)
}

fn readiness(value: BrowserReadiness) -> Readiness {
    match value {
        BrowserReadiness::Loading => Readiness::Loading,
        BrowserReadiness::Interactive => Readiness::Interactive,
        BrowserReadiness::Complete => Readiness::Complete,
        BrowserReadiness::Unknown => Readiness::Unknown,
    }
}

/// The product readiness vocabulary as a name, so audit, surface, and result all say one thing.
fn readiness_name(value: Readiness) -> &'static str {
    match value {
        Readiness::NotApplicable => "not_applicable",
        Readiness::Loading => "loading",
        Readiness::Interactive => "interactive",
        Readiness::Complete => "complete",
        Readiness::Unknown => "unknown",
    }
}

/// What one crossing of the browser boundary can honestly say about its landing.
///
/// This match is exhaustive on purpose: a new browser outcome must not compile until someone
/// decides what it observes. That is the whole point of observing at the seam instead of asking
/// each tool to remember.
///
/// Counts and sizes belong to `Outcome`, where the sentence gives them meaning. This seam owns the
/// host and readiness that every browser-crossing result should receive without per-tool memory.
fn observed_from(outcome: &BrowserOutcome) -> Observed {
    match outcome {
        BrowserOutcome::Documents { .. } => Observed::default(),
        BrowserOutcome::InDocuments { result, .. } => observed_from(result),
        BrowserOutcome::TabOpened { tab, .. }
        | BrowserOutcome::Navigated { tab, .. }
        | BrowserOutcome::Activated { tab, .. }
        | BrowserOutcome::Dragged { tab, .. }
        | BrowserOutcome::KeyPressed { tab, .. }
        | BrowserOutcome::Typed { tab, .. }
        | BrowserOutcome::ScriptEvaluated { tab, .. } => landed(tab),
        BrowserOutcome::Filled { tab, .. } => landed(tab),
        BrowserOutcome::Text { url, .. } => Observed {
            host: observed_host(url),
            ..Observed::default()
        },
        BrowserOutcome::DocumentTree { .. } => Observed::default(),
        BrowserOutcome::Observed {
            readiness: observed,
            ..
        } => Observed {
            readiness: Some(readiness_name(readiness(*observed)).into()),
            ..Observed::default()
        },
        // Receipts without landing metadata leave what the invocation already observed standing.
        BrowserOutcome::Tabs { .. }
        | BrowserOutcome::Targets { .. }
        | BrowserOutcome::Screenshot { .. }
        | BrowserOutcome::FilesUploaded { .. }
        | BrowserOutcome::TabFocused { .. }
        | BrowserOutcome::TabClosed { .. }
        | BrowserOutcome::TargetsDescribed { .. }
        | BrowserOutcome::Scrolled { .. }
        | BrowserOutcome::Zoomed { .. }
        | BrowserOutcome::WindowResized { .. }
        | BrowserOutcome::Hovered { .. }
        | BrowserOutcome::Dialog { .. }
        | BrowserOutcome::DialogHandled { .. }
        | BrowserOutcome::DialogAbsent { .. }
        | BrowserOutcome::DiagnosticsRead { .. }
        | BrowserOutcome::DiagnosticsCleared { .. }
        | BrowserOutcome::RecordingStarted { .. }
        | BrowserOutcome::RecordingStatus { .. }
        | BrowserOutcome::RecordingStopped { .. }
        | BrowserOutcome::RecordingExported { .. }
        | BrowserOutcome::RecordingExportFailed { .. }
        | BrowserOutcome::RecordingDiscarded { .. }
        | BrowserOutcome::RecordingHeld
        | BrowserOutcome::RecordingNotFound
        | BrowserOutcome::Presented { .. }
        | BrowserOutcome::EffectUnknown { .. } => Observed::default(),
    }
}

/// Where a committed landing put the browser, and how far that document had come.
fn landed(tab: &PhysicalTab) -> Observed {
    Observed {
        host: observed_host(&tab.url),
        readiness: Some(readiness_name(readiness(tab.readiness)).into()),
        ..Observed::default()
    }
}

/// The host of a landed URL, as policy matches it, and never anything after it.
fn observed_host(url: &str) -> Option<String> {
    crate::governance::host_pattern::url_host(&Url::parse(url).ok()?)
}

/// The scheme, host and port of a URL as the caller wrote them, and never anything after.
fn observed_origin(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let origin = url.origin();
    origin.is_tuple().then(|| origin.ascii_serialization())
}

/// What an action's receipt says happened to its tab's document.
///
/// Only a new document starts a new generation. A same-document change, an anchor link or a
/// `history.pushState` route inside the same origin, keeps every handle valid; single-page apps
/// make one on most clicks (ADR-0195 Decision 3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DocumentChange {
    /// The tab still shows the same document at the same address.
    Unchanged,
    /// The address moved within the same origin and no document committed.
    SameDocument,
    /// A document committed, or the address moved to another origin.
    NewDocument,
}

impl DocumentChange {
    /// Classify a receipt against the tab's last governed address.
    ///
    /// The browser reports commits for new documents only, so a changed address with no commit
    /// is a same-document change. A change into another origin cannot be one, and an address
    /// that does not parse is never assumed to be.
    fn between(previous: &str, physical: &PhysicalTab, commits: &[String]) -> Self {
        if !commits.is_empty() {
            return Self::NewDocument;
        }
        if physical.url.is_empty() || physical.url == previous {
            return Self::Unchanged;
        }
        let same_origin = match (Url::parse(previous), Url::parse(&physical.url)) {
            (Ok(previous), Ok(current)) => {
                let origin = previous.origin();
                origin.is_tuple() && origin == current.origin()
            }
            _ => false,
        };
        if same_origin {
            Self::SameDocument
        } else {
            Self::NewDocument
        }
    }
}

/// How many words of text a page returned. The words themselves do not enter outcome language.
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// How long a call waits for its lease before the window shows it as waiting.
const WAITING_SHOWN_AFTER: Duration = Duration::from_millis(500);

/// The most an applied action spends observing whether its expectation came true.
const EXPECTATION_OBSERVE_BUDGET: Duration = Duration::from_secs(2);

/// The most a compensating close of a refused landing may take.
const CLOSE_COMPENSATION_BUDGET: Duration = Duration::from_secs(2);

#[cfg(test)]
mod tests;
