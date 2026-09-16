//! Invocation lifecycle, cancellation, deadlines, the one executor, and the one completion path.

mod composition;
mod documents;
mod flow;
mod forms;
mod interaction;
mod navigation;
mod pointer;
mod policy;
mod reading;
mod receipt;
mod recording;
pub mod result;
mod workspace_management;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ghostlight_bridge::browser::{
    BrowserCommand, BrowserOutcome, BrowserReadiness, PhysicalActionSubject, PhysicalFile,
    PhysicalPoint, PhysicalRecordingSummary, PhysicalTab, PresentationActivity, RecordingDelivery,
    RecordingState, RecordingStopReason,
};
use serde_json::{json, Value};
use url::Url;
use uuid::Uuid;

use crate::browser::recovery::{
    BrowserRecovery, RecoveryDecision, RecoveryFailure, RecoveryWaitError,
};
use crate::browser::{choose_browser, BrowserError, BrowserPort};
use crate::events::{DenialPresentation, DomainEvent};

use crate::governance::{
    AuditRecord, AuthoritySnapshot, Capability, CapabilitySet, Decision, GovernanceFacade,
    ReasonCode,
};
use crate::language::{
    self,
    audit::AuditProjection,
    outcome::{
        ActionSubject, BlockedReason, BrowserRecoveryReason, Observed, Outcome, Refusal,
        TargetRole, WorkspaceReason,
    },
    Operation, Record, TakeScreenshot,
};
use crate::presentation::PresentationReactor;
use crate::provenance::ConnectionEvidence;
use crate::workbench::WorkbenchProjection;
use crate::workspace::{
    SelectedTab, SelectedTarget, SelectedView, WorkspaceError, WorkspaceId, WorkspaceLease,
    WorkspaceStore,
};
use result::{CompletionGate, Effect, InvocationResult, Readiness, Status};

/// Cloneable cancellation state forwarded from the MCP edge to physical dispatch.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    /// Request cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    /// Whether cancellation is currently requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    fn flag(&self) -> &AtomicBool {
        &self.0
    }
}

/// Decoded once at service intake; waiting consumes the original invocation deadline.
pub(crate) struct PreparedInvocation {
    pub(crate) invocation: String,
    pub(crate) tool: String,
    provenance: Option<Arc<ConnectionEvidence>>,
    started: Instant,
    deadline: Instant,
    decoded: Result<Operation, language::LanguageError>,
    stage: Mutex<InvocationStage>,
}

#[derive(Default)]
struct InvocationStage {
    running_or_finished: bool,
    waiting_reported: bool,
}

impl PreparedInvocation {
    /// Expiration is measured from intake, including time waiting behind earlier work.
    pub(crate) fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// Decode and start the deadline before any queue wait.
    pub(crate) fn new(
        tool: &str,
        input: Value,
        caller_deadline_ms: Option<u64>,
        provenance: Option<Arc<ConnectionEvidence>>,
    ) -> Self {
        let started = Instant::now();
        let decoded = language::decode(tool, input);
        let timeout = caller_deadline_ms
            .unwrap_or_else(|| decoded.as_ref().map(operation_timeout).unwrap_or(8_000))
            .clamp(100, 30_000);
        Self {
            invocation: format!("invocation_{}", Uuid::new_v4().simple()),
            tool: tool.into(),
            provenance,
            started,
            deadline: started + Duration::from_millis(timeout),
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
    observations: ObservationRegistry,
    stale_candidates: StaleCandidateRegistry,
    permissions: Mutex<HashMap<String, crate::governance::evidence::PermissionTrace>>,
    coverage: Mutex<HashMap<String, language::coverage::Coverage>>,
}

/// What each in-flight invocation has been observed doing at the browser boundary.
///
/// An entry lives from the invocation's first browser crossing until the completion path reads it,
/// which happens exactly once per invocation.
type ObservationRegistry = Arc<Mutex<HashMap<String, Observed>>>;

/// Fresh candidate controls attached to one invocation's stale-target refusal, consumed by the
/// completion path exactly once. Values are language-authored {role,name} objects.
type StaleCandidateRegistry = Arc<Mutex<HashMap<String, Vec<Value>>>>;

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
            observations: Arc::new(Mutex::new(HashMap::new())),
            stale_candidates: Arc::new(Mutex::new(HashMap::new())),
            permissions: Mutex::new(HashMap::new()),
            coverage: Mutex::new(HashMap::new()),
        }
    }

    /// Decode, govern, execute, react, audit, and complete one invocation.
    pub fn execute(
        &self,
        workspace: &WorkspaceId,
        tool: &str,
        input: Value,
        caller_deadline_ms: Option<u64>,
        cancellation: &CancellationToken,
    ) -> InvocationResult {
        let prepared = PreparedInvocation::new(
            tool,
            input,
            caller_deadline_ms,
            self.workspaces.single_connection(workspace),
        );
        self.execute_prepared(workspace, &prepared, cancellation, false)
    }

    /// Show a sustained admission wait on the existing operation surface, without a popup.
    pub(crate) fn show_waiting(&self, workspace: &WorkspaceId, prepared: &PreparedInvocation) {
        let mut stage = prepared
            .stage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if stage.running_or_finished
            || stage.waiting_reported
            || prepared.started.elapsed() < Duration::from_millis(500)
        {
            return;
        }
        let Ok(operation) = &prepared.decoded else {
            return;
        };
        stage.waiting_reported = true;
        self.workbench.react(&DomainEvent::WorkWaiting {
            invocation: prepared.invocation.clone(),
            workspace: workspace.as_str().into(),
            tool: prepared.tool.clone(),
            activity: operation_activity(operation),
            capabilities: language::capability_map::requirements(operation),
            provenance: prepared
                .provenance
                .as_ref()
                .map(|connection| connection.details()),
        });
    }

    /// Execute or refuse one prepared request through the same truthful completion path.
    pub(crate) fn execute_prepared(
        &self,
        workspace: &WorkspaceId,
        prepared: &PreparedInvocation,
        cancellation: &CancellationToken,
        capacity_refused: bool,
    ) -> InvocationResult {
        let invocation = prepared.invocation.clone();
        let tool = prepared.tool.as_str();
        let started = prepared.started;
        let gate = CompletionGate::default();
        let (operation, requirements) = match &prepared.decoded {
            Ok(operation) => {
                let requirements = language::capability_map::requirements(operation);
                (operation, requirements)
            }
            Err(error) => {
                let snapshot = self.governance.snapshot();
                let decision = Decision::refused(ReasonCode::InvalidRequest);
                let refusal = Refusal::InvalidRequest;
                let summary = refusal.summary();
                let result = InvocationResult::new(
                    &invocation,
                    Status::Failed,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    &summary,
                    json!({"reason":"invalid_input","detail":error.to_string()}),
                    vec![error.guidance()],
                );
                let terminal = Terminal {
                    result,
                    decision,
                    physical_id: None,
                    observed: Observed::default(),
                    audit: refusal.audit(),
                };
                return self.finish(
                    &gate,
                    terminal,
                    Completion {
                        workspace,
                        tool,
                        requirements: CapabilitySet::READ,
                        snapshot: &snapshot,
                        duration_ms: elapsed_ms(started),
                        provenance: prepared.provenance.as_deref(),
                    },
                );
            }
        };
        let deadline = prepared.deadline;
        let requires_lease = operation_requires_workspace_lease(operation);
        let lease = if requires_lease && !capacity_refused {
            if let Ok(lease) = self.workspaces.acquire(workspace) {
                Some(lease)
            } else {
                self.show_waiting(workspace, prepared);
                self.workspaces
                    .acquire_until(workspace, deadline, cancellation)
                    .ok()
            }
        } else {
            None
        };
        let snapshot = self.governance.snapshot();
        let context = InvocationContext {
            requirements,
            provenance: prepared.provenance.as_deref(),
            invocation: &invocation,
            workspace,
            requested_browser: operation_browser(operation),
            requested_tab: operation_tab(operation),
            requested_target: operation_target(operation),
            snapshot: &snapshot,
            deadline,
            cancellation,
        };
        let terminal = if capacity_refused {
            let refusal = Refusal::Capacity;
            Terminal {
                result: InvocationResult::new(
                    &invocation,
                    Status::Failed,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    &refusal.summary(),
                    json!({"reason":"capacity"}),
                    refusal.next_steps(),
                ),
                decision: Decision::permitted(),
                physical_id: None,
                observed: Observed::default(),
                audit: refusal.audit(),
            }
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
                invocation: invocation.clone(),
                workspace: workspace.as_str().into(),
                tool: tool.into(),
                activity: operation_activity(operation),
                capabilities: requirements,
                provenance: prepared
                    .provenance
                    .as_ref()
                    .map(|connection| connection.details()),
            });
            if let Some(lease) = lease.as_ref() {
                self.run(&context, lease, operation)
            } else {
                self.run_without_workspace_lease(&context, operation)
            }
        } else if cancellation.is_cancelled() {
            let refusal = Refusal::CancelledBeforeStart;
            let summary = refusal.summary();
            Terminal {
                result: InvocationResult::new(
                    &invocation,
                    Status::Cancelled,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    &summary,
                    json!({"reason":"cancelled"}),
                    refusal.next_steps(),
                ),
                decision: Decision::permitted(),
                physical_id: None,
                observed: Observed::default(),
                audit: refusal.audit(),
            }
        } else if Instant::now() >= deadline {
            let refusal = Refusal::DeadlineBeforeStart;
            let summary = refusal.summary();
            Terminal {
                result: InvocationResult::new(
                    &invocation,
                    Status::Failed,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    &summary,
                    json!({"reason":"deadline"}),
                    refusal.next_steps(),
                ),
                decision: Decision::permitted(),
                physical_id: None,
                observed: Observed::default(),
                audit: refusal.audit(),
            }
        } else {
            self.workspace_failure(&context, WorkspaceError::UnknownWorkspace)
        };
        prepared
            .stage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .running_or_finished = true;
        self.finish(
            &gate,
            terminal,
            Completion {
                workspace,
                tool,
                requirements,
                snapshot: &snapshot,
                duration_ms: elapsed_ms(started),
                provenance: prepared.provenance.as_deref(),
            },
        )
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
                self.navigate_history(context, lease, value.tab.as_deref(), &value.direction)
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
            Operation::InspectPage(value) => self.inspect_page(
                context,
                lease,
                value.tab.as_deref(),
                &value.scope,
                value.root.as_deref(),
                value.max_depth,
                value.max_items,
            ),
            Operation::Find(value) => self.find(
                context,
                lease,
                value.tab.as_deref(),
                &value.text,
                &value.scope,
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
            Operation::ManageWorkspace(value) => self.manage_workspace(context, value),
            Operation::ExplainPolicy(_) => self.explain_policy(context),
        }
    }

    fn run_without_workspace_lease(
        &self,
        context: &InvocationContext<'_>,
        operation: &Operation,
    ) -> Terminal {
        if !matches!(
            operation,
            Operation::ExplainPolicy(_) | Operation::ManageWorkspace(_)
        ) {
            if let Some(terminal) = self.audit_preflight(context) {
                return terminal;
            }
        }
        match operation {
            Operation::Record(value) => self.perform_record(context, None, value),
            Operation::ExplainPolicy(_) => self.explain_policy(context),
            Operation::ManageWorkspace(value) => self.manage_workspace(context, value),
            _ => {
                unreachable!("only recording cleanup, client export, policy explain, and workspace manage bypass the workspace lease")
            }
        }
    }

    fn credential_handoff(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        selected: &SelectedTab,
    ) -> Terminal {
        self.require_session_attention(
            context.workspace,
            context.invocation,
            crate::workspace::AttentionReason::CredentialHandoff,
        );
        self.emit(DomainEvent::AttentionRequired {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            physical_id: Some(selected.physical_id),
        });
        let refusal = Refusal::CredentialHandoff;
        let summary = refusal.summary();
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::AttentionRequired,
                Effect::None,
                readiness(selected.readiness),
                false,
                &summary,
                json!({"tab":selected.handle.as_str(),"credential_handoff":true,"values_sent":false}),
                refusal.next_steps(),
            ),
            decision,
            physical_id: Some(selected.physical_id),
            observed: Observed::default(),
            audit: refusal.audit(),
        }
    }

    #[allow(clippy::too_many_arguments)]
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
        let landing = self.authorize_commits(context, landing_requirements, physical, commits);
        if !landing.allowed {
            let _ = lease.hold_tab(&selected.handle);
            self.emit(DomainEvent::HoldEntered {
                invocation: context.invocation.into(),
                workspace: context.workspace.as_str().into(),
                physical_id: selected.physical_id,
            });
            return self.blocked_at(context, landing, Some(selected.physical_id), Effect::Applied, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}), observed_host(&physical.url));
        }
        let navigated =
            !commits.is_empty() || (!physical.url.is_empty() && physical.url != selected.url);
        let resulting = if navigated {
            match lease.apply_landing(&selected.handle, physical) {
                Ok(tab) => {
                    self.emit(DomainEvent::DocumentCommitted {
                        invocation: context.invocation.into(),
                        workspace: context.workspace.as_str().into(),
                        tab: tab.handle.clone(),
                        physical_id: tab.physical_id,
                    });
                    tab
                }
                Err(error) => return self.workspace_failure(context, error),
            }
        } else {
            let _ = lease.update_readiness(&selected.handle, physical.readiness);
            selected.clone()
        };
        if let Some(object) = facts.as_object_mut() {
            if navigated {
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

    /// Authorize and resolve one controlled tab, executing the closure only when allowed (ADR-0176).
    fn with_authorized_tab<F>(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        capability: impl Into<CapabilitySet>,
        f: F,
    ) -> Terminal
    where
        F: FnOnce(&SelectedTab, Decision) -> Terminal,
    {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, capability, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason": decision.reason.as_str()}),
            );
        }
        f(&selected, decision)
    }

    /// Authorize and resolve one controlled tab and target, executing the closure only when allowed (ADR-0176).
    #[allow(dead_code)]
    fn with_authorized_target<F>(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: &str,
        capability: impl Into<CapabilitySet>,
        f: F,
    ) -> Terminal
    where
        F: FnOnce(&SelectedTab, &SelectedTarget, Decision) -> Terminal,
    {
        let (selected, target) = match self.resolve_target(context, lease, requested_tab, target) {
            Ok(pair) => pair,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, capability, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason": decision.reason.as_str()}),
            );
        }
        f(&selected, &target, decision)
    }

    /// Authorize and resolve one controlled tab and optional target, executing the closure only when allowed (ADR-0177).
    fn with_authorized_optional_target<F>(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        capability: impl Into<CapabilitySet>,
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
        let decision = self.authorize(context, capability, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason": decision.reason.as_str()}),
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
        if matches!(&result, Err(WorkspaceError::StaleTarget)) {
            self.record_stale_candidates(context, lease, target);
        }
        result
    }

    /// Best-effort fresh candidates for a stale target handle. Silence on any failure or
    /// governance denial is deliberate -- the refusal stays truthful without them.
    fn record_stale_candidates(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        target_handle: &str,
    ) {
        let Some(info) = lease.stale_target_context(target_handle) else {
            return;
        };
        if info.name.trim().is_empty() {
            return;
        }
        let decision = self.authorize(context, Capability::Read, Some(info.url.as_str()));
        if !decision.allowed {
            return;
        }
        if let Ok(BrowserOutcome::Targets { targets, .. }) = self.dispatch(
            context,
            BrowserCommand::QuerySemantic {
                tab_id: info.physical_id,
                name: info.name.clone(),
                role: Some(info.role_page.clone()),
                exact: false,
                form_scope: false,
            },
        ) {
            let candidates: Vec<Value> = targets
                .iter()
                .take(3)
                .map(|target| json!({"role":target.role,"name":target.name}))
                .collect();
            if !candidates.is_empty() {
                self.stale_candidates
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(context.invocation.to_owned(), candidates);
            }
        }
    }

    /// Check one optional postcondition after an applied effect. A failed
    /// expectation leaves the effect applied, failed, and never repeat-safe.
    #[allow(clippy::too_many_arguments)]
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
        // Govern and retain the landed action before attempting a separate observation.
        let applied = self.action_success(
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
        if applied.result.status != Status::Succeeded {
            return applied;
        }
        if let Some(expectation) = expect {
            let remaining = context.deadline.saturating_duration_since(Instant::now());
            let budget = remaining
                .min(std::time::Duration::from_millis(2_000))
                .as_millis() as u64;
            match self.dispatch(
                context,
                BrowserCommand::Observe {
                    tab_id: selected.physical_id,
                    condition: expectation.condition.clone(),
                    value: expectation.value.clone(),
                    locator: None,
                    timeout_ms: budget,
                },
            ) {
                Ok(BrowserOutcome::Observed { satisfied, .. }) => {
                    if !satisfied {
                        let mut steps = applied.result.next_steps.clone();
                        steps.push(
                            "The effect was applied, but the expected condition did not hold. Inspect the page before repeating.".into(),
                        );
                        return Terminal {
                            result: InvocationResult::new(
                                context.invocation,
                                Status::Failed,
                                Effect::Applied,
                                readiness(selected.readiness),
                                false,
                                &applied.result.summary,
                                applied.result.facts,
                                steps,
                            ),
                            decision,
                            physical_id: Some(selected.physical_id),
                            observed: applied.observed,
                            audit: applied.audit,
                        };
                    }
                }
                Ok(_) => {
                    let mut failed =
                        self.protocol_failure(context, decision, Some(selected.physical_id));
                    failed.result.effect = Effect::Applied;
                    failed.result.repeat_safe = false;
                    failed.result.next_steps =
                        vec![language::control::APPLIED_BEFORE_CHECK_FAILURE.into()];
                    return failed;
                }
                Err(error) => {
                    let human_control = matches!(error, BrowserError::RuntimeControl(_));
                    let mut failed =
                        self.browser_failure(context, decision, error, Some(selected.physical_id));
                    // Observation failure cannot erase the acknowledged action.
                    failed.result.effect = Effect::Applied;
                    failed.result.repeat_safe = false;
                    if !human_control {
                        failed.result.next_steps =
                            vec![language::control::APPLIED_BEFORE_CHECK_FAILURE.into()];
                    }
                    return failed;
                }
            }
        }
        applied
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
                json!({"reason":decision.reason.as_str()}),
            )));
        }
        match self.dispatch(
            context,
            BrowserCommand::QuerySemantic {
                tab_id: selected.physical_id,
                name: selector.name.clone(),
                role: selector.role.clone(),
                exact: selector.exact,
                form_scope: false,
            },
        ) {
            Ok(BrowserOutcome::Targets { tab_id, targets }) if tab_id == selected.physical_id => {
                if targets.len() != 1 {
                    let matched = targets.len();
                    let outcome = Outcome::SelectorUnresolved { matched };
                    return Err(Box::new(Terminal {
                        result: InvocationResult::new(
                            context.invocation,
                            Status::Failed,
                            Effect::None,
                            readiness(selected.readiness),
                            true,
                            outcome.summary().as_str(),
                            json!({"tab":selected.handle.as_str(),"selector_matched":matched}),
                            outcome.next_steps(),
                        ),
                        decision,
                        physical_id: Some(selected.physical_id),
                        observed: outcome.observed(),
                        audit: outcome.audit(),
                    }));
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

    #[allow(clippy::too_many_arguments)]
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

    /// Resolve independent global human control and this session's review requirement.
    fn runtime_decision(&self, context: &InvocationContext<'_>) -> Decision {
        self.governance
            .session_decision(self.workspaces.attention(context.workspace).is_some())
    }

    fn audit_decision(&self, context: &InvocationContext<'_>) -> Decision {
        if !context.snapshot.requires_audit() {
            return Decision::permitted();
        }
        context
            .snapshot
            .authorize_audit(!self.audit.health().unavailable())
    }

    fn audit_preflight(&self, context: &InvocationContext<'_>) -> Option<Terminal> {
        // Existing human-control paths keep their precedence and language.
        if !self.runtime_decision(context).allowed {
            return None;
        }
        let decision = self.audit_decision(context);
        if decision.allowed {
            return None;
        }
        self.retain_permission(
            context,
            context
                .snapshot
                .decision_evidence(context.requirements, None, decision),
        );
        Some(self.blocked(
            context,
            decision,
            None,
            Effect::None,
            true,
            json!({"reason": decision.reason.as_str()}),
        ))
    }

    fn admit_dispatch(&self, context: &InvocationContext<'_>) -> Result<(), BrowserError> {
        let mut decision = self.runtime_decision(context);
        if decision.allowed {
            decision = self.audit_decision(context);
        }
        if decision.allowed {
            Ok(())
        } else {
            self.retain_permission(
                context,
                context
                    .snapshot
                    .decision_evidence(CapabilitySet::EMPTY, None, decision),
            );
            Err(BrowserError::RuntimeControl(decision.reason))
        }
    }

    fn require_session_attention(
        &self,
        workspace: &WorkspaceId,
        invocation: &str,
        reason: crate::workspace::AttentionReason,
    ) {
        let attention = crate::workspace::SessionAttention {
            id: format!("attention_{}", Uuid::new_v4().simple()),
            invocation: invocation.into(),
            reason,
        };
        if self
            .workspaces
            .require_attention(workspace, attention.clone())
        {
            let label = self
                .workspaces
                .client_label(workspace)
                .unwrap_or_else(|_| "Session".into());
            self.workbench
                .session_attention_changed(workspace.as_str(), &label, Some(attention));
        }
    }

    /// Authorize one operation against its real destination whenever it names one.
    /// `url: None` means this operation has no tab in play at all -- `list_tabs` is the only
    /// caller, since listing needs no destination to check. Every operation that names a tab
    /// must pass `Some(&tab.url)`, the tab's raw string as tracked right now, **even when that
    /// string is empty** because the tab's first landing has not been governed yet. An empty
    /// or otherwise unparseable
    /// string falls straight through to `authorize_landing`, which denies it as `HostDenied` --
    /// there is no third option here that falls back to a host-blind capability check. That
    /// fallback used to exist and was the bug: a tab whose destination genuinely is not yet
    /// known must be treated as though its destination is denied, never as though no destination
    /// applies, or the operator's host allowlist is bypassed for exactly the tabs it exists to
    /// cover.
    fn authorize(
        &self,
        context: &InvocationContext<'_>,
        requirements: impl Into<CapabilitySet>,
        url: Option<&str>,
    ) -> Decision {
        let requirements = requirements.into();
        let runtime = self.runtime_decision(context);
        let _ = self
            .browser
            .publish_control_state(self.governance.runtime_state());
        let (decision, evidence) = if runtime.allowed {
            context.snapshot.authorize_with_evidence(requirements, url)
        } else {
            (
                runtime,
                context
                    .snapshot
                    .decision_evidence(requirements, url, runtime),
            )
        };
        self.retain_permission(context, evidence);
        decision
    }

    fn authorize_commits(
        &self,
        context: &InvocationContext<'_>,
        requirements: impl Into<CapabilitySet>,
        tab: &PhysicalTab,
        commits: &[String],
    ) -> Decision {
        let requirements = requirements.into();
        let mut observed = None;
        for url in commits.iter().chain(std::iter::once(&tab.url)) {
            let decision = self.authorize_landing(context, requirements, url);
            if !decision.allowed {
                return decision;
            }
            if decision.observed && observed.is_none() {
                observed = Some(decision);
            }
        }
        observed.unwrap_or_else(Decision::permitted)
    }

    /// Govern a completed observation's destination. Runtime controls guard the next dispatch;
    /// they cannot turn an already admitted receipt into a permanent policy hold on the tab.
    fn authorize_landing(
        &self,
        context: &InvocationContext<'_>,
        requirements: impl Into<CapabilitySet>,
        url: &str,
    ) -> Decision {
        let (decision, evidence) = context
            .snapshot
            .authorize_with_evidence(requirements.into(), Some(url));
        self.retain_permission(context, evidence);
        decision
    }

    fn authorize_tab_close(&self, context: &InvocationContext<'_>) -> Decision {
        let action = self.authorize(context, Capability::Action, None);
        if !action.allowed {
            return action;
        }
        let close = context.snapshot.authorize_tab_close();
        if !close.allowed || close.observed {
            self.retain_permission(
                context,
                context
                    .snapshot
                    .decision_evidence(CapabilitySet::ACTION, None, close),
            );
        }
        if !close.allowed || close.observed {
            close
        } else {
            action
        }
    }

    /// The one browser seam.
    ///
    /// Every model-requested browser command crosses here, which is why the invocation's
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
            self.observe(context.invocation, observed_from(outcome));
        }
        outcome
    }

    fn dispatch_physical(
        &self,
        context: &InvocationContext<'_>,
        command: BrowserCommand,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.admit_dispatch(context)?;
        let browser = self.target_browser(context)?;
        self.presentation
            .bind_command(context.workspace.as_str(), context.invocation, &command);
        let admit = || self.admit_dispatch(context);
        let outcome = self.browser.call_guarded(
            &browser,
            context.workspace.as_str(),
            command,
            crate::browser::BrowserDispatch {
                deadline: context.deadline,
                cancelled: context.cancellation.flag(),
                admit: &admit,
            },
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

    /// Decide which browser this invocation belongs to, and bind the workspace to it.
    ///
    /// Resolution happens at the seam rather than at admission because a call that never reaches
    /// a browser must not need one: listing this workspace's tabs answers truthfully with no
    /// browser connected at all.
    ///
    /// The binding is what makes the choice stable. It is taken once, on the first crossing, and
    /// every later crossing in this workspace reads it back instead of choosing again.
    fn target_browser(&self, context: &InvocationContext<'_>) -> Result<String, BrowserError> {
        let pinned = self.workspaces.browser_of(context.workspace.as_str());
        let chosen = match choose_browser(
            context.requested_browser,
            pinned.as_deref(),
            &self.browser.browsers(),
        ) {
            Ok(chosen) => chosen,
            Err(BrowserError::DisconnectedBeforeDispatch) => {
                match self.recovery.request(
                    context.requested_browser,
                    pinned.as_deref(),
                    context.deadline,
                    context.cancellation.flag(),
                ) {
                    Ok(RecoveryDecision::Manual { browsers }) => {
                        return Err(BrowserError::RecoveryManual {
                            browsers: browsers.into_iter().map(|browser| browser.name).collect(),
                        });
                    }
                    Ok(RecoveryDecision::Failed { reason, details }) => {
                        return Err(BrowserError::RecoveryFailed { reason, details });
                    }
                    Ok(RecoveryDecision::Ready { browser }) => browser,
                    Ok(RecoveryDecision::Launch { .. }) => {
                        unreachable!("the recovery service consumes its internal launch plan")
                    }
                    Err(RecoveryWaitError::Cancelled) => {
                        return Err(BrowserError::CancelledBeforeDispatch);
                    }
                    Err(RecoveryWaitError::Deadline) => {
                        return Err(BrowserError::DeadlineBeforeDispatch);
                    }
                }
            }
            Err(error) => return Err(error),
        };
        match self
            .workspaces
            .pin_browser(context.workspace.as_str(), &chosen)
        {
            Ok(()) => Ok(chosen),
            Err(WorkspaceError::BrowserPinned) => Err(BrowserError::BrowserPinned),
            // The workspace vanished between admission and dispatch. Nothing physical should
            // happen for a workspace that no longer exists.
            Err(_) => Err(BrowserError::CancelledBeforeDispatch),
        }
    }

    /// Record what one crossing saw, on top of what the invocation already observed.
    fn observe(&self, invocation: &str, observed: Observed) {
        let mut registry = self.observations();
        let merged = registry
            .remove(invocation)
            .unwrap_or_default()
            .merged(observed);
        registry.insert(invocation.into(), merged);
    }

    /// Take the invocation's observation and leave nothing behind.
    ///
    /// The completion path runs exactly once per invocation, so reading here is what keeps the
    /// registry bounded by work in flight rather than by work ever done.
    fn take_observation(&self, invocation: &str) -> Observed {
        self.observations().remove(invocation).unwrap_or_default()
    }

    /// Consume any candidates recorded for this invocation's stale-target refusal.
    fn take_stale_candidates(&self, invocation: &str) -> Option<Vec<Value>> {
        self.stale_candidates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(invocation)
    }

    fn observations(&self) -> std::sync::MutexGuard<'_, HashMap<String, Observed>> {
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn compensate_close(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        tab: &SelectedTab,
    ) -> CloseCompensation {
        if !self.authorize_tab_close(context).allowed {
            return CloseCompensation::Retained;
        }
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(2);
        // The tab exists, so the workspace is already bound to the browser holding it. Undoing an
        // effect is never the moment to choose a browser.
        let Some(browser) = self.workspaces.browser_of(context.workspace.as_str()) else {
            return CloseCompensation::Unknown;
        };
        match self.browser.call_guarded(
            &browser,
            context.workspace.as_str(),
            BrowserCommand::CloseTab {
                tab_id: tab.physical_id,
                released: false,
            },
            crate::browser::BrowserDispatch {
                deadline,
                cancelled: &cancelled,
                admit: &|| self.admit_dispatch(context),
            },
        ) {
            Ok(BrowserOutcome::TabClosed { tab_id }) if tab_id == tab.physical_id => {
                if lease.confirm_tab_closed(&tab.handle).is_ok() {
                    CloseCompensation::Closed
                } else {
                    CloseCompensation::Unknown
                }
            }
            Err(error) if !error.effect_unknown() => CloseCompensation::Retained,
            _ => CloseCompensation::Unknown,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn succeeded(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        effect: Effect,
        readiness: Readiness,
        repeat_safe: bool,
        outcome: Outcome,
        facts: Value,
    ) -> Terminal {
        let summary = outcome.summary();
        let next_steps = outcome.next_steps();
        let observed = outcome.observed();
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::Succeeded,
                effect,
                readiness,
                repeat_safe,
                &summary,
                facts,
                next_steps,
            ),
            decision,
            physical_id,
            observed,
            audit: outcome.audit(),
        }
    }

    /// Report a denial the caller can act on, naming the host when the work named one.
    #[allow(clippy::too_many_arguments)]
    fn blocked_at(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        effect: Effect,
        repeat_safe: bool,
        mut facts: Value,
        blocked_host: Option<String>,
    ) -> Terminal {
        if let Value::Object(object) = &mut facts {
            if let Some(denial_id) = decision.denial_id() {
                object.insert("denial_id".into(), Value::String(denial_id));
            }
            if let Some(rule) = decision.policy_rule() {
                object.insert("policy_rule".into(), Value::String(rule.into()));
            }
            if let Some(mode) = decision.policy_mode() {
                object.insert("policy_mode".into(), Value::String(mode.into()));
            }
        }
        let attention = decision.reason == ReasonCode::RuntimeAttention;
        let refusal = if attention {
            Refusal::AttentionRequired
        } else {
            Refusal::AuthorityBlocked {
                reason: blocked_reason(decision.reason),
                host: blocked_host,
            }
        };
        let observed = refusal.observed();
        let summary = refusal.summary();
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                if attention {
                    Status::AttentionRequired
                } else {
                    Status::Blocked
                },
                effect,
                Readiness::Unknown,
                repeat_safe,
                &summary,
                facts,
                refusal.next_steps(),
            ),
            decision,
            physical_id,
            observed,
            audit: refusal.audit(),
        }
    }

    /// Report a denial with no host in play.
    fn blocked(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        effect: Effect,
        repeat_safe: bool,
        facts: Value,
    ) -> Terminal {
        self.blocked_at(
            context,
            decision,
            physical_id,
            effect,
            repeat_safe,
            facts,
            None,
        )
    }

    fn failed(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        refusal: Refusal,
        facts: Value,
    ) -> Terminal {
        let summary = refusal.summary();
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::Failed,
                Effect::None,
                Readiness::Unknown,
                true,
                &summary,
                facts,
                refusal.next_steps(),
            ),
            decision,
            physical_id,
            observed: Observed::default(),
            audit: refusal.audit(),
        }
    }

    fn unknown(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        refusal: Refusal,
        facts: Value,
    ) -> Terminal {
        let summary = refusal.summary();
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::Unknown,
                Effect::Unknown,
                Readiness::Unknown,
                false,
                &summary,
                facts,
                refusal.next_steps(),
            ),
            decision,
            physical_id,
            observed: Observed::default(),
            audit: refusal.audit(),
        }
    }

    fn protocol_failure(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
    ) -> Terminal {
        self.failed(
            context,
            decision,
            physical_id,
            Refusal::IncompatibleReceipt,
            json!({"reason":"incompatible_browser_receipt"}),
        )
    }

    fn browser_failure(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        error: BrowserError,
        physical_id: Option<u64>,
    ) -> Terminal {
        if let BrowserError::DocumentAccess(decision) = error {
            return self.blocked(
                context,
                decision,
                physical_id,
                Effect::None,
                false,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        if error == BrowserError::DocumentUnavailable {
            return self.failed(
                context,
                decision,
                physical_id,
                Refusal::DocumentUnavailable,
                json!({"reason":"document_unavailable"}),
            );
        }
        if let BrowserError::RuntimeControl(reason) = error {
            return self.blocked(
                context,
                Decision::refused(reason),
                physical_id,
                Effect::None,
                false,
                json!({"reason":reason.as_str()}),
            );
        }
        if matches!(&error, BrowserError::LocalInterlock(_)) {
            let refusal = Refusal::LocalInterlock;
            let summary = refusal.summary();
            return Terminal {
                result: InvocationResult::new(
                    context.invocation,
                    Status::Blocked,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    &summary,
                    json!({"reason":"browser_local_interlock"}),
                    refusal.next_steps(),
                ),
                decision,
                physical_id,
                observed: Observed::default(),
                audit: refusal.audit(),
            };
        }
        // Routing refusals are decisive and physical-effect-free: nothing was dispatched, because
        // nothing could be dispatched anywhere in particular. They name the browsers the caller
        // can choose between rather than picking one on the caller's behalf.
        if let Some((refusal, facts)) = routing_refusal(&error) {
            let summary = refusal.summary();
            return Terminal {
                result: InvocationResult::new(
                    context.invocation,
                    Status::Failed,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    &summary,
                    facts,
                    refusal.next_steps(),
                ),
                decision,
                physical_id,
                observed: Observed::default(),
                audit: refusal.audit(),
            };
        }
        if error.effect_unknown() {
            // Every after-dispatch class is an honest unknown: name its phase so the caller
            // can tell a silent adapter from a spent deadline, and give deadlines their own
            // sentence instead of the disconnection costume they used to wear.
            let (refusal, facts) = match &error {
                BrowserError::EffectUnknown(detail) => (
                    Refusal::EffectUnknown,
                    json!({"reason":"browser_effect_unknown","detail":detail,"phase":"adapter_reported"}),
                ),
                BrowserError::DeadlineAfterDispatch => (
                    Refusal::DeadlineExpired {
                        before_dispatch: false,
                    },
                    json!({"reason":"deadline","phase":"after_dispatch"}),
                ),
                BrowserError::DisconnectedAfterDispatch => (
                    Refusal::ConnectionLost,
                    json!({"reason":"browser_effect_unknown","phase":"after_dispatch"}),
                ),
                BrowserError::CancelledAfterDispatch => (
                    Refusal::CancelledAfterDispatch,
                    json!({"reason":"browser_effect_unknown","phase":"after_dispatch"}),
                ),
                _ => (
                    Refusal::EffectUnknown,
                    json!({"reason":"browser_effect_unknown","phase":"unknown"}),
                ),
            };
            return self.unknown(context, decision, physical_id, refusal, facts);
        }
        if matches!(error, BrowserError::DeadlineBeforeDispatch) {
            return self.failed(
                context,
                decision,
                physical_id,
                Refusal::DeadlineExpired {
                    before_dispatch: true,
                },
                json!({"reason":"deadline","phase":"before_dispatch"}),
            );
        }
        if matches!(error, BrowserError::CapabilityVersion { .. }) {
            return self.failed(
                context,
                decision,
                physical_id,
                Refusal::BrowserAdapterOutdated,
                json!({"reason":browser_reason(&error)}),
            );
        }
        let status = if matches!(error, BrowserError::CancelledBeforeDispatch) {
            Status::Cancelled
        } else {
            Status::Failed
        };
        let refusal = Refusal::BrowserStopped {
            reconnect: matches!(error, BrowserError::DisconnectedBeforeDispatch),
        };
        let summary = refusal.summary();
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                status,
                Effect::None,
                Readiness::Unknown,
                true,
                &summary,
                json!({"reason":browser_reason(&error)}),
                refusal.next_steps(),
            ),
            decision,
            physical_id,
            observed: Observed::default(),
            audit: refusal.audit(),
        }
    }

    fn workspace_failure(
        &self,
        context: &InvocationContext<'_>,
        error: WorkspaceError,
    ) -> Terminal {
        let owner = match error {
            WorkspaceError::NotOwnedTab => context
                .requested_tab
                .and_then(|tab| self.workspaces.tab_owner(context.workspace, tab)),
            WorkspaceError::NotOwnedTarget => context
                .requested_target
                .and_then(|target| self.workspaces.target_owner(context.workspace, target)),
            _ => None,
        };
        let reason = match error {
            WorkspaceError::NotOwnedTab
            | WorkspaceError::NotOwnedTarget
            | WorkspaceError::NotOwnedView => WorkspaceReason::OwnershipMismatch {
                owner: owner.clone(),
            },
            _ => WorkspaceReason::from(error),
        };
        let refusal = Refusal::WorkspaceUnusable {
            reason: reason.clone(),
        };
        let summary = refusal.summary();
        let status = if error == WorkspaceError::Held {
            Status::Blocked
        } else {
            Status::Failed
        };
        let mut facts = json!({"reason":reason.as_fact()});
        if let Some(owner) = owner {
            facts["owner_workspace"] = Value::String(owner);
        }
        if let Some(candidates) = self.take_stale_candidates(context.invocation) {
            facts["recovery_candidates"] = Value::Array(candidates);
        }
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                status,
                Effect::None,
                Readiness::Unknown,
                status == Status::Failed,
                &summary,
                facts,
                refusal.next_steps(),
            ),
            decision: if status == Status::Blocked {
                Decision::refused(ReasonCode::RuntimeHold)
            } else {
                Decision::permitted()
            },
            physical_id: None,
            observed: Observed::default(),
            audit: refusal.audit(),
        }
    }

    /// One bounded window for a waking adapter to reattach, used by reads that must touch
    /// real state. Deliberately local: no launch, no installed-browser discovery, hard
    /// ceiling.
    fn wait_for_any_browser(&self, context: &InvocationContext<'_>) -> bool {
        // One bounded window for a waking adapter to reattach. This is deliberately local:
        // no launch, no installed-browser discovery, and a hard ceiling well under any
        // caller's patience.
        const WAKE_BUDGET: Duration = Duration::from_secs(2);
        let started = Instant::now();
        loop {
            if !self.browser.browsers().is_empty() {
                return true;
            }
            if context
                .cancellation
                .flag()
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return false;
            }
            if started.elapsed() >= WAKE_BUDGET || Instant::now() >= context.deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    fn emit(&self, event: DomainEvent) {
        self.presentation.react(&event);
        self.workbench.react(&event);
    }
}

fn denial_presentation(tool: &str, result: &InvocationResult) -> DenialPresentation {
    if tool == "browser_tabs" {
        return match result.facts.get("reason").and_then(Value::as_str) {
            Some("tab_close_denied") => DenialPresentation::TabKeptOpenByPolicy,
            Some("browser_local_interlock") => DenialPresentation::TabKeptOpenBySetting,
            _ => DenialPresentation::Guardrail,
        };
    }
    DenialPresentation::Guardrail
}

/// What the single completion path needs to record one terminal outcome.
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
    provenance: Option<&'a ConnectionEvidence>,
}

/// Milliseconds elapsed since an invocation began, saturating rather than wrapping.
fn elapsed_ms(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[derive(Clone, Copy)]
struct InvocationContext<'a> {
    requirements: CapabilitySet,
    provenance: Option<&'a ConnectionEvidence>,
    invocation: &'a str,
    workspace: &'a WorkspaceId,
    /// The browser this call named, when it named one.
    ///
    /// Only a call that can open the first tab of a workspace can carry one. Every other call
    /// reaches the browser through a handle that already belongs to it.
    requested_browser: Option<&'a str>,
    requested_tab: Option<&'a str>,
    requested_target: Option<&'a str>,
    snapshot: &'a AuthoritySnapshot,
    deadline: Instant,
    cancellation: &'a CancellationToken,
}

struct Terminal {
    result: InvocationResult,
    decision: Decision,
    physical_id: Option<u64>,
    observed: Observed,
    audit: AuditProjection,
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
        }) if matches!(action.as_str(), "status" | "stop" | "save" | "discard")
    ) && !matches!(
        operation,
        Operation::ExplainPolicy(_) | Operation::ManageWorkspace(_)
    )
}

fn permitted() -> Decision {
    Decision::permitted()
}

fn recording_facts(summary: &PhysicalRecordingSummary) -> Value {
    json!({
        "recording":summary.recording_id,
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
        Operation::ManageWorkspace(_) => PresentationActivity::Quiet,
        Operation::ExplainPolicy(_) => PresentationActivity::Quiet,
        Operation::Record(value) if value.action == "start" => PresentationActivity::Screenshot,
        Operation::Record(_) => PresentationActivity::Quiet,
    }
}

/// The refusal for a browser choice that could not be made, and the facts that explain it.
///
/// Returning candidates rather than a choice is the point: two connected browsers are two
/// different signed-in contexts, and guessing between them would put the person's work somewhere
/// they did not ask for.
fn routing_refusal(error: &BrowserError) -> Option<(Refusal, Value)> {
    match error {
        BrowserError::AmbiguousBrowser(candidates) => Some((
            Refusal::BrowserAmbiguous,
            json!({"reason":"browser_ambiguous","browsers":candidates}),
        )),
        BrowserError::UnknownBrowser(_) => {
            Some((Refusal::BrowserUnknown, json!({"reason":"browser_unknown"})))
        }
        BrowserError::BrowserPinned => {
            Some((Refusal::BrowserPinned, json!({"reason":"browser_pinned"})))
        }
        BrowserError::Primitive(detail) => Some((
            Refusal::BrowserPrimitive {
                detail: detail.clone(),
            },
            json!({"reason":"browser_primitive_failed","detail":detail}),
        )),
        BrowserError::RecoveryManual { browsers } => Some((
            Refusal::BrowserStartupManual {
                browsers: browsers.clone(),
            },
            manual_browser_facts(browsers),
        )),
        BrowserError::RecoveryFailed { reason, details } => Some((
            Refusal::BrowserRecoveryFailed {
                reason: recovery_reason(*reason),
            },
            json!({"reason":reason.as_str(),"details":details}),
        )),
        _ => None,
    }
}

fn manual_browser_facts(browsers: &[String]) -> Value {
    let mut facts = json!({
        "reason": "browser_startup_manual",
        "browsers": browsers,
    });
    if let [browser] = browsers {
        facts["browser"] = json!(browser);
    }
    facts
}

const fn recovery_reason(reason: RecoveryFailure) -> BrowserRecoveryReason {
    match reason {
        RecoveryFailure::BrowserAbsent => BrowserRecoveryReason::BrowserAbsent,
        RecoveryFailure::LaunchFailed => BrowserRecoveryReason::LaunchFailed,
        RecoveryFailure::SandboxedPackage => BrowserRecoveryReason::SandboxedPackage,
        RecoveryFailure::ExtensionAbsent => BrowserRecoveryReason::ExtensionAbsent,
        RecoveryFailure::NativeHostUnavailable => BrowserRecoveryReason::NativeHostUnavailable,
        RecoveryFailure::OwnedElsewhere => BrowserRecoveryReason::OwnedElsewhere,
        RecoveryFailure::WrongProfile => BrowserRecoveryReason::WrongProfile,
        RecoveryFailure::HandshakeTimeout => BrowserRecoveryReason::HandshakeTimeout,
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

/// The target handle named by this operation, if its parameters name one.
fn operation_target(operation: &Operation) -> Option<&str> {
    match operation {
        Operation::ReadPage(value) => value.target.as_deref(),
        Operation::Click(value) => value.target.as_deref(),
        Operation::ScrollPage(value) => value.target.as_deref(),
        Operation::Hover(value) => value.target.as_deref(),
        Operation::PressKey(value) => value.target.as_deref(),
        Operation::Wait(value) => value.target.as_deref(),
        _ => None,
    }
}

fn operation_timeout(operation: &Operation) -> u64 {
    match operation {
        Operation::OpenPage(value) => value.timeout_ms,
        Operation::NavigatePage(value) => value.timeout_ms,
        Operation::NavigateHistory(value) => value.timeout_ms,
        Operation::ReloadPage(value) => value.timeout_ms,
        Operation::TakeScreenshot(value) => value.timeout_ms,
        Operation::Click(value) => value.timeout_ms,
        Operation::ScrollPage(value) => value.timeout_ms,
        Operation::Hover(value) => value.timeout_ms,
        Operation::FillForm(value) => value.timeout_ms,
        Operation::TypeText(value) => value.timeout_ms,
        Operation::Drag(value) => value.timeout_ms,
        Operation::UploadFiles(value) => value.timeout_ms,
        Operation::RunScript(value) => value.timeout_ms,
        Operation::Wait(value) => value.timeout_ms,
        Operation::RunFlow(value) => value.timeout_ms,
        Operation::Record(_) => 30_000,
        _ => 8_000,
    }
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
        })
        .or_else(|| fallback_role.map(ActionSubject::unnamed))
}

/// Name a key only when it is one of the catalog's named keys.
///
/// A single literal character is the caller's own text. The audit keeps the caller's intent, not
/// the caller's payload, so "Pressed a key" is as much as a one-character press gets to say.
fn named_key(key: &str) -> Option<String> {
    (key.chars().count() > 1).then(|| key.to_owned())
}

/// Translate a governance reason into the language's own denial vocabulary.
const fn blocked_reason(reason: ReasonCode) -> BlockedReason {
    match reason {
        ReasonCode::HostDenied => BlockedReason::Host,
        ReasonCode::ProtectedHost => BlockedReason::ProtectedHost,
        ReasonCode::CapabilityDenied => BlockedReason::Capability,
        ReasonCode::TabCloseDenied => BlockedReason::TabClose,
        ReasonCode::InvalidAuthority => BlockedReason::InvalidAuthority,
        ReasonCode::RuntimeHold => BlockedReason::Hold,
        ReasonCode::SessionEnded => BlockedReason::SessionEnded,
        ReasonCode::ChannelDenied => BlockedReason::Channel,
        ReasonCode::AuditUnavailable => BlockedReason::AuditUnavailable,
        ReasonCode::Permitted | ReasonCode::InvalidRequest | ReasonCode::RuntimeAttention => {
            BlockedReason::Unspecified
        }
    }
}

// A wait's physical timer must finish early enough for the extension, native relay, browser
// port, executor, and MCP edge to return one decisive unsatisfied receipt. A quarter second was
// too narrow on a live cache-bypassing reload and turned a normal loading timeout into an
// uncertain after-dispatch deadline.
const WAIT_RECEIPT_RESERVE_MS: u64 = 750;

fn observation_budget_ms(requested_ms: u64, remaining: Duration) -> u64 {
    let available = remaining.saturating_sub(Duration::from_millis(WAIT_RECEIPT_RESERVE_MS));
    let available_ms = u64::try_from(available.as_millis()).unwrap_or(u64::MAX);
    requested_ms.min(available_ms)
}

fn load_physical_files(paths: &[String]) -> Result<(Vec<PhysicalFile>, u64), &'static str> {
    const MAX_UPLOAD_BYTES: u64 = 5_000_000;
    let mut files = Vec::with_capacity(paths.len());
    let mut total = 0_u64;
    for requested in paths {
        let path = Path::new(requested);
        let mut file = File::open(path).map_err(|_| "file_unavailable")?;
        let before = file.metadata().map_err(|_| "file_unavailable")?;
        if !before.is_file() {
            return Err("not_a_file");
        }
        if before.len() > MAX_UPLOAD_BYTES {
            return Err("file_too_large");
        }
        total = total.checked_add(before.len()).ok_or("upload_too_large")?;
        if total > MAX_UPLOAD_BYTES {
            return Err("upload_too_large");
        }
        let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
        (&mut file)
            .take(MAX_UPLOAD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "file_read_failed")?;
        let after = file.metadata().map_err(|_| "file_changed")?;
        let modified_changed = before
            .modified()
            .ok()
            .zip(after.modified().ok())
            .is_some_and(|(left, right)| left != right);
        if u64::try_from(bytes.len()).ok() != Some(before.len())
            || after.len() != before.len()
            || modified_changed
        {
            return Err("file_changed");
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .ok_or("invalid_file_name")?;
        files.push(PhysicalFile {
            name: bounded(name, 255),
            media_type: media_type(path).into(),
            data: BASE64.encode(bytes),
            size: before.len(),
        });
    }
    Ok((files, total))
}

fn media_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "html" | "htm" => "text/html",
        _ => "application/octet-stream",
    }
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
        | BrowserOutcome::RecordingAmbiguous { .. }
        | BrowserOutcome::RecordingNotFound
        | BrowserOutcome::Presented { .. }
        | BrowserOutcome::Cancelled
        | BrowserOutcome::BiDi { .. }
        | BrowserOutcome::SetPreloadScript
        | BrowserOutcome::Cdp { .. }
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

/// The host of a landed URL, and never anything after it.
fn observed_host(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()?
        .host_str()
        .map(str::to_ascii_lowercase)
}

/// How many words of text a page returned. The words themselves do not enter outcome language.
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn bounded(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn browser_reason(error: &BrowserError) -> &'static str {
    match error {
        BrowserError::DocumentAccess(_) => "document_access_denied",
        BrowserError::DocumentUnavailable => "document_unavailable",
        BrowserError::RuntimeControl(reason) => reason.as_str(),
        BrowserError::DisconnectedBeforeDispatch => "browser_disconnected",
        BrowserError::CancelledBeforeDispatch => "cancelled",
        BrowserError::DeadlineBeforeDispatch => "deadline",
        BrowserError::Primitive(_) => "browser_primitive_failed",
        BrowserError::LocalInterlock(_) => "browser_local_interlock",
        BrowserError::Protocol(_)
        | BrowserError::Authentication
        | BrowserError::Incompatible { .. }
        | BrowserError::CapabilityVersion { .. } => "browser_contract_failed",
        _ => "browser_effect_unknown",
    }
}

#[cfg(test)]
mod tests {
    mod audit_health;
    mod catalog_authority;
    mod configured_authority;
    mod control;
    mod documents;
    mod execution;
    mod presentation;
    mod provenance;
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use ghostlight_bridge::browser::{
        BrowserCommand, BrowserOutcome, BrowserReadiness, CaptureScope, EncodedRecording,
        ObservedTarget, PhysicalActionSubject, PhysicalRecordingSummary, PhysicalTab,
        RecordingDelivery, RecordingDestination, RecordingState, RecordingStopReason,
        RuntimeControlIntent, RuntimeControlState, ViewportGeometry, RECORDING_LOCAL_MAX_BYTES,
        RECORDING_TRANSFER_MAX_BYTES,
    };
    use ghostlight_bridge::service::ServiceContent;
    use serde_json::json;

    use crate::browser::recovery::RecoveryCandidate;
    use crate::browser::testing::{summary, FakeBrowser, FAKE_BROWSER};
    use ghostlight_bridge::service::IntakeChannel;

    use crate::governance::{AuditRecord, AuditSink, GovernanceFacade};
    use crate::install::browser_package::BrowserPackage;
    use crate::install::native_host::NativeHostState;
    use crate::language::outcome::Observed;
    use crate::presentation::{PresentationError, PresentationPort, PresentationReactor};
    use crate::workbench::WorkbenchProjection;
    use crate::workspace::WorkspaceStore;

    use super::{
        browser_reason, observation_budget_ms, observed_from, readiness_name, routing_refusal,
        ApplicationExecutor, BrowserError, CancellationToken, Effect, Readiness, Status,
    };

    #[derive(Default)]
    struct MemoryAudit(Mutex<Vec<AuditRecord>>);
    impl AuditSink for MemoryAudit {
        fn record(&self, record: &AuditRecord) -> io::Result<()> {
            self.0.lock().unwrap().push(record.clone());
            Ok(())
        }
    }

    struct NoPresentation;
    impl PresentationPort for NoPresentation {
        fn present(
            &self,
            _workspace: &str,
            _signal: ghostlight_bridge::browser::PresentationSignal,
        ) -> Result<(), PresentationError> {
            Ok(())
        }
    }

    fn tab(id: u64, url: &str) -> PhysicalTab {
        PhysicalTab {
            tab_id: id,
            title: "Example".into(),
            url: url.into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        }
    }

    fn recording_summary(state: RecordingState, source_url: &str) -> PhysicalRecordingSummary {
        PhysicalRecordingSummary {
            recording_id: "recording_one".into(),
            tab_id: 7,
            state,
            frame_count: usize::from(state != RecordingState::Recording),
            bytes_held: usize::from(state != RecordingState::Recording),
            duration_ms: 500,
            hard_expires_unix_ms: (state == RecordingState::Recording).then_some(121_000),
            retention_expires_unix_ms: (state != RecordingState::Recording).then_some(301_000),
            stop_reason: (state != RecordingState::Recording)
                .then_some(RecordingStopReason::Explicit),
            source_urls: vec![source_url.into()],
            source_urls_complete: true,
        }
    }

    fn fixture_with_governance(
        governance: GovernanceFacade,
    ) -> (
        ApplicationExecutor,
        Arc<FakeBrowser>,
        WorkspaceStore,
        crate::workspace::WorkspaceId,
        Arc<MemoryAudit>,
    ) {
        let browser = Arc::new(FakeBrowser::default());
        let workspaces = WorkspaceStore::default();
        let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp, None);
        let audit = Arc::new(MemoryAudit::default());
        let mut executor = ApplicationExecutor::new(
            governance,
            workspaces.clone(),
            browser.clone(),
            PresentationReactor::new(Arc::new(NoPresentation)),
            WorkbenchProjection::default(),
            Arc::new(crate::audit::AuditRecorder::new(
                audit.clone(),
                WorkbenchProjection::default(),
            )),
            crate::diagnostics::DiagnosticsHub::for_tests(),
        );
        executor
            .recovery
            .set_test_candidates(vec![RecoveryCandidate {
                id: "chromium".into(),
                name: "Chromium".into(),
                package: BrowserPackage::Native,
                package_detail: "Chromium native package".into(),
                registration: NativeHostState::Current,
                ordinary_executable: Some(PathBuf::from("chromium")),
            }]);
        (executor, browser, workspaces, workspace, audit)
    }

    fn fixture() -> (
        ApplicationExecutor,
        Arc<FakeBrowser>,
        WorkspaceStore,
        crate::workspace::WorkspaceId,
        Arc<MemoryAudit>,
    ) {
        fixture_with_governance(GovernanceFacade::new(None, None))
    }

    fn temporary_policy(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-1.0-work-{name}-{}.json",
            uuid::Uuid::new_v4()
        ))
    }

    /// A real, isolated policy source that tests may edit between invocations.
    struct TestPolicy(PathBuf);

    impl TestPolicy {
        fn new() -> Self {
            let policy = Self(temporary_policy("configured-authority"));
            policy.set(json!(["read", "action", "write", "execute"]));
            policy
        }

        fn facade(&self) -> GovernanceFacade {
            GovernanceFacade::new(Some(self.0.clone()), None)
        }

        fn set(&self, capabilities: serde_json::Value) {
            fs::write(
                &self.0,
                serde_json::to_vec(&json!({
                    "schema":3,"name":"test authority","version":"1",
                    "grants":[{"id":"test","hosts":{"allow":["*"]},"allowed":capabilities}]
                }))
                .unwrap(),
            )
            .unwrap();
        }
    }

    impl Drop for TestPolicy {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn all_open_policy_with(config: &str) -> String {
        format!(
            r#"{{"schema":3,"name":"work test","version":"1","grants":[{{"id":"all","hosts":{{"allow":["*"]}},"allowed":["read","action","write","execute"]}}],"config":{config}}}"#
        )
    }
}
