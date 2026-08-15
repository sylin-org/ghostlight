//! Invocation lifecycle, cancellation, deadlines, the one executor, and the one completion path.

pub mod result;

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
    BrowserCommand, BrowserOutcome, BrowserReadiness, ClickShape, DiagnosticDetail,
    DiagnosticEntry, DiagnosticSource, EncodedRecording, PhysicalActionSubject, PhysicalField,
    PhysicalFile, PhysicalPoint, PhysicalRecordingSummary, PhysicalTab, PresentationActivity,
    RecordingDelivery, RecordingDestination, RecordingState, RecordingStopReason,
    RECORDING_LOCAL_MAX_BYTES, RECORDING_TRANSFER_MAX_BYTES,
};
use ghostlight_bridge::service::ServiceContent;
use serde_json::{json, Value};
use url::Url;
use uuid::Uuid;

use crate::browser::{choose_browser, BrowserError, BrowserPort};
use crate::events::{DenialPresentation, DomainEvent};
use ghostlight_bridge::service::IntakeChannel;

use crate::governance::{
    AuditRecord, AuditSink, AuthoritySnapshot, Capability, CapabilitySet, Decision,
    GovernanceFacade, ReasonCode,
};
use crate::language::{
    self,
    outcome::{
        ActionSubject, BlockedReason, Observed, Outcome, Refusal, SavedTo, TargetNoun, TargetRole,
        WorkspaceReason,
    },
    Click, Diagnose, Drag, FillForm, FormField, HandleDialog, Hover, Operation, PressKey, Record,
    RunScript, RunSequence, ScrollPage, SequenceStep, TypeText, UploadFiles, Wait,
};
use crate::presentation::PresentationReactor;
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

/// The single application executor for every model-requested operation and sequence step.
pub struct ApplicationExecutor {
    governance: GovernanceFacade,
    workspaces: WorkspaceStore,
    browser: Arc<dyn BrowserPort>,
    presentation: PresentationReactor,
    workbench: WorkbenchProjection,
    audit: Arc<dyn AuditSink>,
    active_authority: ActiveAuthorityRegistry,
    observations: ObservationRegistry,
}

/// Current immutable invocation snapshots used only to govern asynchronous browser events.
///
/// Keyed by workspace, but the value is every invocation currently governing that workspace, not
/// just one: operations that skip the workspace lease (recording status/stop/discard) can run
/// fully concurrently with a lease-holding operation on the same workspace, on separate threads.
/// A single `HashMap<String, AuthoritySnapshot>` here let one invocation's completion silently
/// clear -- or its start silently overwrite -- another invocation's still-active entry, and the
/// reader's fallback on a missing entry is the *widest* policy available, which made this a
/// fail-open race rather than a merely confusing one. Removal here is scoped to the exact
/// invocation that inserted it, never to "whatever is currently there for this workspace".
pub type ActiveAuthorityRegistry = Arc<Mutex<HashMap<String, Vec<(String, AuthoritySnapshot)>>>>;

/// Add one invocation's snapshot to its workspace's active set.
fn register_active_authority(
    registry: &ActiveAuthorityRegistry,
    workspace: &str,
    invocation: &str,
    snapshot: &AuthoritySnapshot,
) {
    registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry(workspace.to_owned())
        .or_default()
        .push((invocation.to_owned(), snapshot.clone()));
}

/// Remove exactly this invocation's snapshot, leaving any other invocation still governing the
/// same workspace untouched.
fn deregister_active_authority(
    registry: &ActiveAuthorityRegistry,
    workspace: &str,
    invocation: &str,
) {
    let mut registry = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(entries) = registry.get_mut(workspace) {
        entries.retain(|(id, _)| id != invocation);
        if entries.is_empty() {
            registry.remove(workspace);
        }
    }
}

/// What each in-flight invocation has been observed doing at the browser boundary.
///
/// An entry lives from the invocation's first browser crossing until the completion path reads it,
/// which happens exactly once per invocation.
type ObservationRegistry = Arc<Mutex<HashMap<String, Observed>>>;

impl ApplicationExecutor {
    /// Construct the orchestrator's only model-requested mutation entry point.
    #[must_use]
    pub fn new(
        governance: GovernanceFacade,
        workspaces: WorkspaceStore,
        browser: Arc<dyn BrowserPort>,
        presentation: PresentationReactor,
        workbench: WorkbenchProjection,
        audit: Arc<dyn AuditSink>,
    ) -> Self {
        Self {
            governance,
            workspaces,
            browser,
            presentation,
            workbench,
            audit,
            active_authority: Arc::new(Mutex::new(HashMap::new())),
            observations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Shared read-only source of active immutable snapshots for browser-event governance.
    #[must_use]
    pub fn active_authority(&self) -> ActiveAuthorityRegistry {
        Arc::clone(&self.active_authority)
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
        let invocation = format!("invocation_{}", Uuid::new_v4().simple());
        let started = std::time::Instant::now();
        let gate = CompletionGate::default();
        let decoded = language::decode(tool, input);
        let (operation, requirements) = match decoded {
            Ok(operation) => {
                let requirements = language::capability_map::requirements(&operation);
                (operation, requirements)
            }
            Err(error) => {
                let snapshot = self
                    .governance
                    .snapshot(&language::RequestRestrictions::default());
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
                    refusal.next_steps(),
                );
                let terminal = Terminal {
                    result,
                    decision,
                    physical_id: None,
                    observed: Observed::default(),
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
                        channel: self.workspaces.channel(workspace).ok(),
                    },
                );
            }
        };
        let deadline_ms = caller_deadline_ms
            .unwrap_or_else(|| operation_timeout(&operation))
            .clamp(100, 30_000);
        let deadline = Instant::now() + Duration::from_millis(deadline_ms);
        let requires_lease = operation_requires_workspace_lease(&operation);
        let lease = if requires_lease {
            loop {
                match self.workspaces.acquire(workspace) {
                    Ok(lease) => break Some(lease),
                    Err(WorkspaceError::Busy)
                        if !cancellation.is_cancelled() && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => break None,
                }
            }
        } else {
            None
        };
        let snapshot = self.governance.snapshot(operation.restrictions());
        let context = InvocationContext {
            invocation: &invocation,
            workspace,
            requested_browser: operation_browser(&operation),
            snapshot: &snapshot,
            deadline,
            cancellation,
        };
        let terminal = if !requires_lease || lease.is_some() {
            self.emit(DomainEvent::WorkStarted {
                invocation: invocation.clone(),
                workspace: workspace.as_str().into(),
                tool: tool.into(),
                activity: operation_activity(&operation),
                capabilities: requirements,
            });
            register_active_authority(
                &self.active_authority,
                workspace.as_str(),
                &invocation,
                &snapshot,
            );
            let terminal = if let Some(lease) = lease.as_ref() {
                self.run(&context, lease, &operation)
            } else {
                self.run_without_workspace_lease(&context, &operation)
            };
            deregister_active_authority(&self.active_authority, workspace.as_str(), &invocation);
            terminal
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
            }
        } else {
            self.workspace_failure(&context, WorkspaceError::UnknownWorkspace)
        };
        self.finish(
            &gate,
            terminal,
            Completion {
                workspace,
                tool,
                requirements,
                snapshot: &snapshot,
                duration_ms: elapsed_ms(started),
                channel: self.workspaces.channel(workspace).ok(),
            },
        )
    }

    fn finish(
        &self,
        gate: &CompletionGate,
        terminal: Terminal,
        completion: Completion<'_>,
    ) -> InvocationResult {
        let Completion {
            workspace,
            tool,
            requirements,
            snapshot,
            duration_ms,
            channel,
        } = completion;
        let denial_attention = terminal.result.status == Status::Blocked
            && self
                .governance
                .record_denial_attention(workspace.as_str(), terminal.decision);
        if denial_attention {
            self.governance.controls().require_attention();
            let _ = self
                .browser
                .publish_control_state(self.governance.runtime_state());
        }
        let event = if denial_attention {
            DomainEvent::AttentionRequired {
                invocation: terminal.result.invocation.clone(),
                workspace: workspace.as_str().into(),
                physical_id: terminal.physical_id,
            }
        } else {
            match terminal.result.status {
                Status::Blocked => DomainEvent::WorkBlocked {
                    invocation: terminal.result.invocation.clone(),
                    workspace: workspace.as_str().into(),
                    physical_id: terminal.physical_id,
                    presentation: denial_presentation(tool, &terminal.result),
                },
                Status::AttentionRequired => DomainEvent::AttentionRequired {
                    invocation: terminal.result.invocation.clone(),
                    workspace: workspace.as_str().into(),
                    physical_id: terminal.physical_id,
                },
                _ => DomainEvent::WorkCompleted {
                    invocation: terminal.result.invocation.clone(),
                    workspace: workspace.as_str().into(),
                    physical_id: terminal.physical_id,
                },
            }
        };
        self.emit(event);
        let status = serde_json::to_value(terminal.result.status)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".into());
        let effect = serde_json::to_value(terminal.result.effect)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".into());
        let observed = self
            .take_observation(&terminal.result.invocation)
            .merged(terminal.observed.clone());
        let record = AuditRecord::now(
            &terminal.result.invocation,
            workspace.as_str(),
            tool,
            requirements,
            snapshot.id(),
            terminal.decision,
            &status,
            &effect,
            &terminal.result.summary,
            duration_ms,
        )
        .from_channel(channel)
        .with_policy(snapshot, terminal.decision)
        .with_observation(observed);
        let _ = self.audit.record(&record);
        gate.complete(terminal.result)
            .expect("single executor completion path");
        gate.take().expect("completion committed")
    }

    fn run(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        operation: &Operation,
    ) -> Terminal {
        match operation {
            Operation::ListTabs(_) => self.list_tabs(context, lease),
            Operation::ActivateTab(value) => self.activate_tab(context, lease, &value.tab),
            Operation::OpenPage(value) => self.open_page(context, lease, &value.url),
            Operation::NavigatePage(value) => {
                self.navigate_page(context, lease, value.tab.as_deref(), &value.url)
            }
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
                value.max_chars,
            ),
            Operation::InspectPage(value) => self.inspect_page(
                context,
                lease,
                value.tab.as_deref(),
                &value.scope,
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
            Operation::TakeScreenshot(value) => self.screenshot(
                context,
                lease,
                value.tab.as_deref(),
                value.target.as_deref(),
                value.full_page,
            ),
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
            Operation::RunSequence(value) => self.sequence(context, lease, value),
            Operation::HandleDialog(value) => self.handle_dialog(context, lease, value),
            Operation::Diagnose(value) => self.diagnose(context, lease, value),
            Operation::Record(value) => self.perform_record(context, Some(lease), value),
        }
    }

    fn run_without_workspace_lease(
        &self,
        context: &InvocationContext<'_>,
        operation: &Operation,
    ) -> Terminal {
        match operation {
            Operation::Record(value) => self.perform_record(context, None, value),
            _ => {
                unreachable!("only recording cleanup and client export bypass the workspace lease")
            }
        }
    }

    fn list_tabs(&self, context: &InvocationContext<'_>, lease: &WorkspaceLease) -> Terminal {
        let decision = self.authorize(context, Capability::Read, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match lease.tabs() {
            Ok(tabs) => {
                let facts: Vec<_> = tabs.into_iter().map(|tab| json!({"tab":tab.handle.as_str(),"title":tab.title,"url":tab.url,"active":tab.active,"readiness":readiness(tab.readiness)})).collect();
                let outcome = Outcome::TabsListed { count: facts.len() };
                // Listing tabs is also how a caller discovers where tabs can be opened. A model
                // asked to choose a browser needs the choices in front of it, and this is the one
                // read that already answers "what is there".
                let browsers: Vec<_> = self
                    .browser
                    .browsers()
                    .into_iter()
                    .map(|browser| {
                        json!({"browser":browser.id,"name":browser.name,"attended":browser.attended})
                    })
                    .collect();
                self.succeeded(
                    context,
                    decision,
                    None,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    outcome,
                    json!({"tabs":facts,"browsers":browsers}),
                )
            }
            Err(error) => self.workspace_failure(context, error),
        }
    }

    fn activate_tab(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: &str,
    ) -> Terminal {
        let selected = match lease.select_tab(Some(requested_tab)) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, CapabilitySet::EMPTY, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::FocusTab {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::TabFocused {
                tab_id,
                active,
                window_focused,
            }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.mark_active(&selected.handle) {
                    return self.workspace_failure(context, error);
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    Outcome::TabActivated {
                        host: observed_host(&selected.url),
                    },
                    json!({"tab":selected.handle.as_str(),"active":active,"window_focused":window_focused}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn open_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        url: &str,
    ) -> Terminal {
        let decision = self.authorize(context, Capability::Read, Some(url));
        if !decision.allowed {
            return self.blocked_at(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
                observed_host(url),
            );
        }
        let client_label = match self.workspaces.client_label(context.workspace) {
            Ok(label) => label,
            Err(error) => return self.workspace_failure(context, error),
        };
        let group_title = format!("Ghostlight - {}", bounded(&client_label, 80));
        let (tab, commits) = match self.dispatch(
            context,
            BrowserCommand::OpenTab {
                url: url.into(),
                group_title,
            },
        ) {
            Ok(BrowserOutcome::TabOpened {
                tab,
                committed_urls,
            }) => (tab, committed_urls),
            Ok(_) => return self.protocol_failure(context, decision, None),
            Err(error) => return self.browser_failure(context, decision, error, None),
        };
        let controlled = match lease.add_tab(&tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        self.emit(DomainEvent::TabCreated {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            tab: controlled.handle.clone(),
            physical_id: controlled.physical_id,
        });
        let landing = self.authorize_commits(context, Capability::Read, &tab, &commits);
        if !landing.allowed {
            return match self.compensate_close(context, lease, &controlled) {
                CloseCompensation::Closed => self.blocked_at(
                    context,
                    landing,
                    Some(tab.tab_id),
                    Effect::None,
                    true,
                    json!({"reason":landing.reason.as_str(),"compensated":true}),
                    observed_host(&tab.url),
                ),
                CloseCompensation::Retained => self.blocked_at(
                    context,
                    landing,
                    Some(tab.tab_id),
                    Effect::Applied,
                    false,
                    json!({"reason":landing.reason.as_str(),"compensated":false,"retained":true}),
                    observed_host(&tab.url),
                ),
                CloseCompensation::Unknown => self.unknown(
                    context,
                    landing,
                    Some(tab.tab_id),
                    Refusal::LandingDeniedUnknown,
                    json!({"reason":landing.reason.as_str(),"compensated":false}),
                ),
            };
        }
        let governed = match lease.apply_landing(&controlled.handle, &tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        self.emit(DomainEvent::DocumentCommitted {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            tab: governed.handle.clone(),
            physical_id: governed.physical_id,
        });
        self.succeeded(context, landing, Some(governed.physical_id), Effect::Applied, readiness(governed.readiness), false, Outcome::PageOpened { host: observed_host(&governed.url) }, json!({"tab":governed.handle.as_str(),"url":governed.url,"title":governed.title,"created":true,"document_generation":governed.generation}))
    }

    fn navigate_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        url: &str,
    ) -> Terminal {
        let decision = self.authorize(context, Capability::Read, Some(url));
        if !decision.allowed {
            return self.blocked_at(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
                observed_host(url),
            );
        }
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(WorkspaceError::NoTab) if requested_tab.is_none() => {
                return self.open_page(context, lease, url)
            }
            Err(error) => return self.workspace_failure(context, error),
        };
        match self.dispatch(
            context,
            BrowserCommand::Navigate {
                tab_id: selected.physical_id,
                url: url.into(),
            },
        ) {
            Ok(BrowserOutcome::Navigated {
                tab,
                committed_urls,
            }) => {
                let landing =
                    self.authorize_commits(context, Capability::Read, &tab, &committed_urls);
                if !landing.allowed {
                    let _ = lease.hold_tab(&selected.handle);
                    self.emit(DomainEvent::HoldEntered {
                        invocation: context.invocation.into(),
                        workspace: context.workspace.as_str().into(),
                        physical_id: selected.physical_id,
                    });
                    return self.blocked_at(context, landing, Some(selected.physical_id), Effect::Applied, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}), observed_host(&tab.url));
                }
                let governed = match lease.apply_landing(&selected.handle, &tab) {
                    Ok(tab) => tab,
                    Err(error) => return self.workspace_failure(context, error),
                };
                self.emit(DomainEvent::DocumentCommitted {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    tab: governed.handle.clone(),
                    physical_id: governed.physical_id,
                });
                self.succeeded(context, landing, Some(governed.physical_id), Effect::Applied, readiness(governed.readiness), false, Outcome::PageNavigated { host: observed_host(&governed.url) }, json!({"tab":governed.handle.as_str(),"url":governed.url,"title":governed.title,"created":false,"document_generation":governed.generation}))
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn navigate_history(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        direction: &str,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Action, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let outcome = self.dispatch(
            context,
            BrowserCommand::TraverseHistory {
                tab_id: selected.physical_id,
                direction: direction.into(),
            },
        );
        self.complete_navigation(
            context,
            lease,
            &selected,
            decision,
            outcome,
            |host| Outcome::HistoryTraversed {
                direction: direction.into(),
                host,
            },
            json!({"action":direction}),
        )
    }

    fn reload_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        bypass_cache: bool,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Action, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let outcome = self.dispatch(
            context,
            BrowserCommand::Reload {
                tab_id: selected.physical_id,
                bypass_cache,
            },
        );
        self.complete_navigation(
            context,
            lease,
            &selected,
            decision,
            outcome,
            |host| Outcome::PageReloaded { host },
            json!({"action":"reload","bypass_cache":bypass_cache}),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_navigation<F>(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        selected: &SelectedTab,
        decision: Decision,
        outcome: Result<BrowserOutcome, BrowserError>,
        make_outcome: F,
        mut facts: Value,
    ) -> Terminal
    where
        F: FnOnce(Option<String>) -> Outcome,
    {
        match outcome {
            Ok(BrowserOutcome::Navigated {
                tab,
                committed_urls,
            }) => {
                let landing =
                    self.authorize_commits(context, Capability::Action, &tab, &committed_urls);
                if !landing.allowed {
                    let _ = lease.hold_tab(&selected.handle);
                    self.emit(DomainEvent::HoldEntered {
                        invocation: context.invocation.into(),
                        workspace: context.workspace.as_str().into(),
                        physical_id: selected.physical_id,
                    });
                    return self.blocked_at(
                        context,
                        landing,
                        Some(selected.physical_id),
                        Effect::Applied,
                        false,
                        json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}),
                        observed_host(&tab.url),
                    );
                }
                let governed = match lease.apply_landing(&selected.handle, &tab) {
                    Ok(tab) => tab,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let outcome = make_outcome(observed_host(&governed.url));
                if let Some(object) = facts.as_object_mut() {
                    object.insert("tab".into(), json!(governed.handle.as_str()));
                    object.insert("url".into(), json!(governed.url));
                    object.insert("title".into(), json!(governed.title));
                    object.insert("document_generation".into(), json!(governed.generation));
                }
                self.emit(DomainEvent::DocumentCommitted {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    tab: governed.handle.clone(),
                    physical_id: governed.physical_id,
                });
                self.succeeded(
                    context,
                    landing,
                    Some(governed.physical_id),
                    Effect::Applied,
                    readiness(governed.readiness),
                    false,
                    outcome,
                    facts,
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn close_tab(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested: &str,
    ) -> Terminal {
        let selected = match lease.select_tab(Some(requested)) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize_tab_close(context);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"tab":selected.handle.as_str(),"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::CloseTab {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::TabClosed { tab_id }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.confirm_tab_closed(&selected.handle) {
                    return self.workspace_failure(context, error);
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    Readiness::NotApplicable,
                    false,
                    Outcome::TabClosed,
                    json!({"tab":selected.handle.as_str(),"closed":true}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn read_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        max_chars: usize,
    ) -> Terminal {
        let (selected, locator, _) =
            match self.resolve_optional_target(lease, requested_tab, target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::ReadText {
                tab_id: selected.physical_id,
                locator,
                max_chars,
            },
        ) {
            Ok(BrowserOutcome::Text {
                tab_id,
                text,
                truncated,
                title,
                url,
            }) if tab_id == selected.physical_id => {
                let landing = self.authorize(context, Capability::Read, Some(&url));
                if !landing.allowed {
                    let _ = lease.hold_tab(&selected.handle);
                    return self.blocked_at(context, landing, Some(tab_id), Effect::None, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}), observed_host(&url));
                }
                let words = word_count(&text);
                self.succeeded(context, landing, Some(tab_id), Effect::None, readiness(selected.readiness), true, Outcome::TextRead { words, host: observed_host(&url) }, json!({"tab":selected.handle.as_str(),"url":url,"title":bounded(&title,500),"text":bounded(&text,max_chars),"truncated":truncated || text.chars().count() > max_chars,"document_generation":selected.generation}))
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn inspect_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        kind: &str,
        max_items: usize,
    ) -> Terminal {
        self.targets_operation(
            context,
            lease,
            requested_tab,
            Capability::Read,
            BrowserCommand::Inspect {
                tab_id: 0,
                kind: kind.into(),
                max_items,
            },
            if kind == "controls" {
                TargetNoun::Control
            } else {
                TargetNoun::Item
            },
        )
    }

    fn find(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        text: &str,
        kind: &str,
        max_results: usize,
    ) -> Terminal {
        self.targets_operation(
            context,
            lease,
            requested_tab,
            Capability::Read,
            BrowserCommand::Find {
                tab_id: 0,
                text: text.into(),
                kind: kind.into(),
                max_results,
            },
            TargetNoun::Match,
        )
    }

    /// One governed target retrieval.
    ///
    /// The closed noun chooses both the product sentence and the structured fact key.
    #[allow(clippy::too_many_arguments)]
    fn targets_operation(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        capability: Capability,
        command: BrowserCommand,
        noun: TargetNoun,
    ) -> Terminal {
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
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let command = match command {
            BrowserCommand::Inspect {
                kind, max_items, ..
            } => BrowserCommand::Inspect {
                tab_id: selected.physical_id,
                kind,
                max_items,
            },
            BrowserCommand::Find {
                text,
                kind,
                max_results,
                ..
            } => BrowserCommand::Find {
                tab_id: selected.physical_id,
                text,
                kind,
                max_results,
            },
            _ => unreachable!("target operations are closed"),
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Targets { tab_id, targets }) if tab_id == selected.physical_id => {
                let mapped = match lease.register_targets(&selected, &targets) {
                    Ok(mapped) => mapped,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let items: Vec<_> = mapped.into_iter().map(|(handle, target)| json!({"target":handle.as_str(),"role":bounded(&target.role,100),"name":bounded(&target.name,500),"state":target.state,"credential_class":target.credential_class})).collect();
                let outcome = Outcome::TargetsListed {
                    noun,
                    count: items.len(),
                    host: observed_host(&selected.url),
                };
                let fact_key = match noun {
                    TargetNoun::Match => "matches",
                    TargetNoun::Control | TargetNoun::Item => "items",
                };
                let mut facts = serde_json::Map::new();
                facts.insert("tab".into(), json!(selected.handle.as_str()));
                facts.insert("document_generation".into(), json!(selected.generation));
                facts.insert(fact_key.into(), json!(items));
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::None,
                    readiness(selected.readiness),
                    true,
                    outcome,
                    Value::Object(facts),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn screenshot(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        full_page: bool,
    ) -> Terminal {
        let (selected, locator, _) =
            match self.resolve_optional_target(lease, requested_tab, target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::Screenshot {
                tab_id: selected.physical_id,
                locator,
                full_page,
            },
        ) {
            Ok(BrowserOutcome::Screenshot {
                tab_id,
                mime_type,
                data,
                width,
                height,
                viewport,
            }) if tab_id == selected.physical_id => {
                if data.len() > 7_000_000 {
                    return self.failed(
                        context,
                        decision,
                        Some(tab_id),
                        Refusal::CaptureTooLarge,
                        json!({"reason":"screenshot_too_large"}),
                    );
                }
                let view = match lease.register_view(&selected, viewport, width, height) {
                    Ok(view) => view,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let outcome = Outcome::Captured {
                    full_page,
                    width,
                    height,
                };
                let mut terminal = self.succeeded(context, decision, Some(tab_id), Effect::None, readiness(selected.readiness), true, outcome, json!({"tab":selected.handle.as_str(),"view":view.as_str(),"mime_type":mime_type,"width":width,"height":height}));
                terminal.result = terminal
                    .result
                    .with_content(ServiceContent::Image { mime_type, data });
                terminal
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn perform_click(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Click,
    ) -> Terminal {
        let location = match self.resolve_location(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
            value.view.as_deref(),
            value.x,
            value.y,
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let selected = location.tab();
        let decision = self.authorize(context, Capability::Action, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let (command, facts, clicked) = match location {
            ResolvedLocation::Target { tab, target } => {
                let clicked = Clicked::Target(target.role);
                self.emit(DomainEvent::TargetIndicated {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    physical_id: tab.physical_id,
                    locator: target.locator.clone(),
                    click: Some(ClickShape {
                        clicks: value.click_count,
                        button: value.button.clone(),
                    }),
                });
                (
                    BrowserCommand::Activate {
                        tab_id: tab.physical_id,
                        locator: target.locator,
                        button: value.button.clone(),
                        click_count: value.click_count,
                    },
                    json!({"tab":tab.handle.as_str(),"target":target.handle.as_str(),"activated":true}),
                    clicked,
                )
            }
            ResolvedLocation::Point { tab, view, point } => (
                BrowserCommand::ActivatePoint {
                    tab_id: tab.physical_id,
                    point,
                    expected_viewport: view.viewport,
                    button: value.button.clone(),
                    click_count: value.click_count,
                },
                json!({"tab":tab.handle.as_str(),"view":view.handle.as_str(),"activated":true}),
                Clicked::Point(point),
            ),
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Activated {
                tab,
                subject,
                committed_urls,
            }) => {
                let host = observed_host(&tab.url);
                let outcome = match clicked {
                    Clicked::Target(role) => Outcome::TargetClicked {
                        host,
                        subject: action_subject(context, subject, Some(role))
                            .expect("a semantic click has a fallback subject"),
                    },
                    Clicked::Point(point) => Outcome::PointClicked {
                        host,
                        x: point.x.round().max(0.0) as u32,
                        y: point.y.round().max(0.0) as u32,
                        subject: action_subject(context, subject, None),
                    },
                };
                self.action_success(
                    context,
                    lease,
                    decision,
                    Capability::Action,
                    &selected,
                    &tab,
                    &committed_urls,
                    outcome,
                    facts,
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn perform_scroll(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &ScrollPage,
    ) -> Terminal {
        let (selected, locator, revealed_role) = match self.resolve_optional_target(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::Scroll {
                tab_id: selected.physical_id,
                locator,
                direction: value
                    .target
                    .is_none()
                    .then(|| value.direction.clone().unwrap_or_else(|| "down".into())),
                amount: value
                    .target
                    .is_none()
                    .then(|| value.amount.clone().unwrap_or_else(|| "medium".into())),
            },
        ) {
            Ok(BrowserOutcome::Scrolled {
                tab_id,
                x,
                y,
                subject,
            }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.invalidate_views(&selected.handle) {
                    return self.workspace_failure(context, error);
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    value.target.is_some(),
                    if value.target.is_some() {
                        Outcome::TargetRevealed {
                            host: observed_host(&selected.url),
                            subject: action_subject(
                                context,
                                subject,
                                Some(revealed_role.unwrap_or(TargetRole::Control)),
                            )
                            .expect("a semantic reveal has a fallback subject"),
                        }
                    } else {
                        Outcome::PageScrolled {
                            host: observed_host(&selected.url),
                            direction: value
                                .direction
                                .clone()
                                .unwrap_or_else(|| "down".into()),
                        }
                    },
                    json!({"tab":selected.handle.as_str(),"target":value.target,"scrolled":true,"x":x,"y":y}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn set_zoom(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        percent: u16,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::SetZoom {
                tab_id: selected.physical_id,
                zoom: f64::from(percent) / 100.0,
            },
        ) {
            Ok(BrowserOutcome::Zoomed { tab_id, zoom }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.invalidate_views(&selected.handle) {
                    return self.workspace_failure(context, error);
                }
                let actual_percent = (zoom * 100.0).round() as u16;
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    Outcome::ZoomSet {
                        percent: actual_percent,
                        host: observed_host(&selected.url),
                    },
                    json!({"tab":selected.handle.as_str(),"action":"zoom","requested_percent":percent,"percent":actual_percent,"zoomed":true}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn resize_window(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        width: u32,
        height: u32,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, CapabilitySet::EMPTY, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::ResizeWindow {
                tab_id: selected.physical_id,
                width,
                height,
            },
        ) {
            Ok(BrowserOutcome::WindowResized {
                tab_id,
                width: observed_width,
                height: observed_height,
                affected_tab_ids,
            }) if tab_id == selected.physical_id => {
                if let Err(error) = lease.invalidate_views_for_physical(&affected_tab_ids) {
                    return self.workspace_failure(context, error);
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    Outcome::WindowResized {
                        width: observed_width,
                        height: observed_height,
                    },
                    json!({"tab":selected.handle.as_str(),"action":"resize","requested_width":width,"requested_height":height,"width":observed_width,"height":observed_height,"resized":true}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn perform_hover(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Hover,
    ) -> Terminal {
        let location = match self.resolve_location(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
            value.view.as_deref(),
            value.x,
            value.y,
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let selected = location.tab();
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let hovered_role = match &location {
            ResolvedLocation::Target { target, .. } => Some(target.role),
            ResolvedLocation::Point { .. } => None,
        };
        let (command, facts) = match location {
            ResolvedLocation::Target { tab, target } => {
                self.emit(DomainEvent::TargetIndicated {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    physical_id: tab.physical_id,
                    locator: target.locator.clone(),
                    click: None,
                });
                (
                    BrowserCommand::Hover {
                        tab_id: tab.physical_id,
                        locator: target.locator,
                    },
                    json!({"tab":tab.handle.as_str(),"target":target.handle.as_str(),"hovered":true}),
                )
            }
            ResolvedLocation::Point { tab, view, point } => (
                BrowserCommand::HoverPoint {
                    tab_id: tab.physical_id,
                    point,
                    expected_viewport: view.viewport,
                },
                json!({"tab":tab.handle.as_str(),"view":view.handle.as_str(),"hovered":true}),
            ),
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Hovered { tab_id, subject }) if tab_id == selected.physical_id => {
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    Outcome::Hovered {
                        host: observed_host(&selected.url),
                        subject: action_subject(context, subject, hovered_role),
                    },
                    facts,
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn perform_fill(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &FillForm,
    ) -> Terminal {
        let mut resolved = Vec::with_capacity(value.fields.len());
        let mut selected: Option<SelectedTab> = None;
        for field in &value.fields {
            let (tab, target) = match self.resolve_target(
                lease,
                value
                    .tab
                    .as_deref()
                    .or_else(|| selected.as_ref().map(|tab| tab.handle.as_str())),
                &field.target,
            ) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            if let Some(current) = &selected {
                if current.handle != tab.handle {
                    return self.workspace_failure(context, WorkspaceError::TargetTabMismatch);
                }
            } else {
                selected = Some(tab);
            }
            resolved.push((target, field.value.clone()));
        }
        let selected = selected.expect("validated non-empty fields");
        let submit = match value.submit_target.as_deref() {
            Some(handle) => {
                match self.resolve_target(lease, Some(selected.handle.as_str()), handle) {
                    Ok((_, target)) => Some(target),
                    Err(error) => return self.workspace_failure(context, error),
                }
            }
            None => None,
        };
        let requirements = if submit.is_some() {
            CapabilitySet::READ
                .union(CapabilitySet::WRITE)
                .union(CapabilitySet::ACTION)
        } else {
            CapabilitySet::READ.union(CapabilitySet::WRITE)
        };
        let decision = self.authorize(context, requirements, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let mut locators: Vec<_> = resolved
            .iter()
            .map(|(target, _)| target.locator.clone())
            .collect();
        if let Some(target) = &submit {
            locators.push(target.locator.clone());
        }
        match self.dispatch(
            context,
            BrowserCommand::DescribeTargets {
                tab_id: selected.physical_id,
                locators: locators.clone(),
            },
        ) {
            Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                if tab_id == selected.physical_id =>
            {
                if targets.len() != locators.len() {
                    return self.protocol_failure(context, decision, Some(tab_id));
                }
                if targets.iter().any(|target| target.credential_class) {
                    return self.credential_handoff(context, decision, &selected);
                }
            }
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
        let fields = resolved
            .into_iter()
            .map(|(target, value)| PhysicalField {
                locator: target.locator,
                value,
            })
            .collect();
        match self.dispatch(
            context,
            BrowserCommand::Fill {
                tab_id: selected.physical_id,
                fields,
                submit_locator: submit.map(|target| target.locator),
            },
        ) {
            Ok(BrowserOutcome::Filled {
                tab,
                filled_count,
                submitted,
                committed_urls,
            }) => self.action_success(context, lease, decision, requirements, &selected, &tab, &committed_urls, Outcome::FormFilled { fields: filled_count, submitted, host: observed_host(&tab.url) }, json!({"tab":selected.handle.as_str(),"filled_count":filled_count,"submitted":submitted})),
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn perform_type_text(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &TypeText,
    ) -> Terminal {
        let (selected, target) =
            match self.resolve_target(lease, value.tab.as_deref(), &value.target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
        let typed_role = target.role;
        let decision = self.authorize(context, Capability::Action, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::DescribeTargets {
                tab_id: selected.physical_id,
                locators: vec![target.locator.clone()],
            },
        ) {
            Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                if tab_id == selected.physical_id && targets.len() == 1 =>
            {
                if targets[0].credential_class {
                    return self.credential_handoff(context, decision, &selected);
                }
            }
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
        self.emit(DomainEvent::TargetIndicated {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            physical_id: selected.physical_id,
            locator: target.locator.clone(),
            click: None,
        });
        match self.dispatch(
            context,
            BrowserCommand::TypeText {
                tab_id: selected.physical_id,
                locator: target.locator,
                text: value.text.clone(),
                clear_first: value.clear_first,
            },
        ) {
            Ok(BrowserOutcome::Typed {
                tab,
                character_count,
                subject,
                committed_urls,
            }) => {
                let outcome = Outcome::TextTyped {
                    host: observed_host(&tab.url),
                    subject: action_subject(context, subject, Some(typed_role))
                        .expect("typing has a fallback subject"),
                    characters: character_count,
                };
                self.action_success(
                    context,
                    lease,
                    decision,
                    Capability::Write,
                    &selected,
                    &tab,
                    &committed_urls,
                    outcome,
                    json!({"tab":selected.handle.as_str(),"target":target.handle.as_str(),"typed":true,"character_count":character_count}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn perform_drag(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Drag,
    ) -> Terminal {
        let mut dragged_from = None;
        let mut dragged_onto = None;
        let (selected, command, facts) = if let (Some(source), Some(destination)) = (
            value.source_target.as_deref(),
            value.destination_target.as_deref(),
        ) {
            let (selected, source) = match self.resolve_target(lease, value.tab.as_deref(), source)
            {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            let (_, destination) =
                match self.resolve_target(lease, Some(selected.handle.as_str()), destination) {
                    Ok(value) => value,
                    Err(error) => return self.workspace_failure(context, error),
                };
            self.emit(DomainEvent::TargetIndicated {
                invocation: context.invocation.into(),
                workspace: context.workspace.as_str().into(),
                physical_id: selected.physical_id,
                locator: source.locator.clone(),
                click: None,
            });
            dragged_from = Some(source.role);
            dragged_onto = Some(destination.role);
            let facts = json!({"tab":selected.handle.as_str(),"source_target":source.handle.as_str(),"destination_target":destination.handle.as_str(),"dragged":true});
            let command = BrowserCommand::Drag {
                tab_id: selected.physical_id,
                source_locator: source.locator,
                destination_locator: destination.locator,
            };
            (selected, command, facts)
        } else {
            let view_handle = value.view.as_deref().expect("language validated view");
            let start_location = match self.resolve_location(
                lease,
                value.tab.as_deref(),
                None,
                Some(view_handle),
                value.start_x,
                value.start_y,
            ) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            let ResolvedLocation::Point {
                tab: selected,
                view,
                point: start,
            } = start_location
            else {
                unreachable!("view input resolves to a point")
            };
            let (_, end) = match lease.resolve_view_point(
                view_handle,
                Some(&selected),
                value.end_x.expect("language validated end_x"),
                value.end_y.expect("language validated end_y"),
            ) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            let facts =
                json!({"tab":selected.handle.as_str(),"view":view.handle.as_str(),"dragged":true});
            let command = BrowserCommand::DragPoints {
                tab_id: selected.physical_id,
                start,
                end,
                expected_viewport: view.viewport,
            };
            (selected, command, facts)
        };
        let decision = self.authorize(context, Capability::Action, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Dragged {
                tab,
                source_subject,
                destination_subject,
                committed_urls,
            }) => {
                let outcome = Outcome::Dragged {
                    host: observed_host(&tab.url),
                    source: action_subject(context, source_subject, dragged_from),
                    destination: action_subject(context, destination_subject, dragged_onto),
                };
                self.action_success(
                    context,
                    lease,
                    decision,
                    Capability::Action,
                    &selected,
                    &tab,
                    &committed_urls,
                    outcome,
                    facts,
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn upload_files(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &UploadFiles,
    ) -> Terminal {
        let (selected, target) =
            match self.resolve_target(lease, value.tab.as_deref(), &value.target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
        let decision = self.authorize(context, Capability::Write, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::DescribeTargets {
                tab_id: selected.physical_id,
                locators: vec![target.locator.clone()],
            },
        ) {
            Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                if tab_id == selected.physical_id && targets.len() == 1 =>
            {
                if targets[0].credential_class {
                    return self.credential_handoff(context, decision, &selected);
                }
            }
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
        let (files, total) = match load_physical_files(&value.paths) {
            Ok(value) => value,
            Err(reason) => {
                return self.failed(
                    context,
                    decision,
                    Some(selected.physical_id),
                    Refusal::FilesUnreadable,
                    json!({"reason":reason}),
                )
            }
        };
        match self.dispatch(
            context,
            BrowserCommand::UploadFiles {
                tab_id: selected.physical_id,
                locator: target.locator,
                files,
            },
        ) {
            Ok(BrowserOutcome::FilesUploaded {
                tab_id,
                uploaded_count,
                uploaded_bytes,
                subject,
            }) if tab_id == selected.physical_id
                && uploaded_count == value.paths.len()
                && uploaded_bytes == total =>
            {
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    false,
                    Outcome::FilesUploaded {
                        count: uploaded_count,
                        host: observed_host(&selected.url),
                        subject: action_subject(context, subject, Some(target.role)),
                    },
                    json!({"tab":selected.handle.as_str(),"target":target.handle.as_str(),"uploaded_count":uploaded_count,"uploaded_bytes":uploaded_bytes}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn run_script(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &RunScript,
    ) -> Terminal {
        let selected = match lease.select_tab(value.tab.as_deref()) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Execute, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::EvaluateScript {
                tab_id: selected.physical_id,
                script: value.script.clone(),
                max_result_chars: value.max_result_chars,
            },
        ) {
            Ok(BrowserOutcome::ScriptEvaluated {
                tab,
                value,
                truncated,
                committed_urls,
            }) => {
                let rendered = serde_json::from_str(&value).unwrap_or(Value::String(value));
                let outcome = Outcome::ScriptEvaluated {
                    host: observed_host(&tab.url),
                };
                self.action_success(
                    context,
                    lease,
                    decision,
                    Capability::Execute,
                    &selected,
                    &tab,
                    &committed_urls,
                    outcome,
                    json!({"tab":selected.handle.as_str(),"value":rendered,"truncated":truncated}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn credential_handoff(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        selected: &SelectedTab,
    ) -> Terminal {
        self.governance.controls().require_attention();
        let _ = self
            .browser
            .publish_control_state(self.governance.runtime_state());
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
        }
    }

    fn perform_key(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &PressKey,
    ) -> Terminal {
        let (selected, locator, focused_role) = match self.resolve_optional_target(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Action, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::PressKey {
                tab_id: selected.physical_id,
                locator,
                key: value.key.clone(),
                modifiers: value.modifiers.clone(),
            },
        ) {
            Ok(BrowserOutcome::KeyPressed {
                tab,
                key,
                subject,
                committed_urls,
            }) => {
                let outcome = Outcome::KeyboardSent {
                    host: observed_host(&tab.url),
                    key: named_key(&key),
                    subject: action_subject(context, subject, focused_role),
                };
                self.action_success(
                    context,
                    lease,
                    decision,
                    Capability::Action,
                    &selected,
                    &tab,
                    &committed_urls,
                    outcome,
                    json!({"tab":selected.handle.as_str(),"key":key,"pressed":true}),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn perform_wait(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Wait,
    ) -> Terminal {
        let (selected, locator, _target_role) = match self.resolve_optional_target(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::Observe {
                tab_id: selected.physical_id,
                condition: value.condition.clone(),
                value: value.value.clone(),
                locator,
                timeout_ms: observation_budget_ms(
                    value.timeout_ms,
                    context.deadline.saturating_duration_since(Instant::now()),
                ),
            },
        ) {
            Ok(BrowserOutcome::Observed {
                tab_id,
                satisfied,
                elapsed_ms,
                readiness: browser_readiness,
            }) if tab_id == selected.physical_id => {
                let _ = lease.update_readiness(&selected.handle, browser_readiness);
                let status = if satisfied {
                    Status::Succeeded
                } else {
                    Status::Failed
                };
                // The condition is a closed vocabulary and its value is not: only the name of the
                // condition joins the sentence that reaches audit.
                let outcome = Outcome::Waited {
                    condition: value.condition.clone(),
                    elapsed_ms,
                    satisfied,
                    host: observed_host(&selected.url),
                };
                let summary = outcome.summary();
                let next_steps = outcome.next_steps();
                let outcome_observed = outcome.observed();
                Terminal {
                    result: InvocationResult::new(
                        context.invocation,
                        status,
                        Effect::None,
                        readiness(browser_readiness),
                        true,
                        &summary,
                        json!({"tab":selected.handle.as_str(),"condition":value.condition,"satisfied":satisfied,"elapsed_ms":elapsed_ms,"readiness":readiness(browser_readiness)}),
                        next_steps,
                    ),
                    decision,
                    physical_id: Some(tab_id),
                    observed: outcome_observed,
                }
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn sequence(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &RunSequence,
    ) -> Terminal {
        let selected = match lease.select_tab(value.tab.as_deref()) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let mut completed = 0_usize;
        let mut applied_any = false;
        let mut statuses = Vec::with_capacity(value.steps.len());
        let mut last_decision = permitted();
        for step in &value.steps {
            self.emit(DomainEvent::WorkPhaseStarted {
                invocation: context.invocation.into(),
                workspace: context.workspace.as_str().into(),
                physical_id: Some(selected.physical_id),
                activity: step_activity(step),
            });
            let terminal = match step {
                SequenceStep::Click {
                    target,
                    button,
                    click_count,
                } => self.perform_click(
                    context,
                    lease,
                    &Click {
                        target: Some(target.clone()),
                        view: None,
                        x: None,
                        y: None,
                        tab: Some(selected.handle.as_str().into()),
                        button: button.clone(),
                        click_count: *click_count,
                        timeout_ms: value.timeout_ms,
                        restrictions: value.restrictions.clone(),
                    },
                ),
                SequenceStep::TypeText {
                    target,
                    text,
                    clear_first,
                } => self.perform_type_text(
                    context,
                    lease,
                    &TypeText {
                        target: target.clone(),
                        text: text.clone(),
                        tab: Some(selected.handle.as_str().into()),
                        clear_first: *clear_first,
                        timeout_ms: value.timeout_ms,
                        restrictions: value.restrictions.clone(),
                    },
                ),
                SequenceStep::Fill {
                    target,
                    value: field_value,
                } => self.perform_fill(
                    context,
                    lease,
                    &FillForm {
                        fields: vec![FormField {
                            target: target.clone(),
                            value: field_value.clone(),
                        }],
                        tab: Some(selected.handle.as_str().into()),
                        submit_target: None,
                        timeout_ms: value.timeout_ms,
                        restrictions: value.restrictions.clone(),
                    },
                ),
                SequenceStep::PressKey {
                    key,
                    target,
                    modifiers,
                } => self.perform_key(
                    context,
                    lease,
                    &PressKey {
                        key: key.clone(),
                        tab: Some(selected.handle.as_str().into()),
                        target: target.clone(),
                        modifiers: modifiers.clone(),
                        restrictions: value.restrictions.clone(),
                    },
                ),
                SequenceStep::Scroll {
                    target,
                    direction,
                    amount,
                } => self.perform_scroll(
                    context,
                    lease,
                    &ScrollPage {
                        tab: Some(selected.handle.as_str().into()),
                        target: target.clone(),
                        direction: direction.clone(),
                        amount: amount.clone(),
                        timeout_ms: value.timeout_ms,
                        restrictions: value.restrictions.clone(),
                    },
                ),
                SequenceStep::Hover { target } => self.perform_hover(
                    context,
                    lease,
                    &Hover {
                        target: Some(target.clone()),
                        view: None,
                        x: None,
                        y: None,
                        tab: Some(selected.handle.as_str().into()),
                        timeout_ms: value.timeout_ms,
                        restrictions: value.restrictions.clone(),
                    },
                ),
                SequenceStep::Wait {
                    condition,
                    value: condition_value,
                    target,
                } => self.perform_wait(
                    context,
                    lease,
                    &Wait {
                        condition: condition.clone(),
                        tab: Some(selected.handle.as_str().into()),
                        value: condition_value.clone(),
                        target: target.clone(),
                        timeout_ms: value.timeout_ms,
                        restrictions: value.restrictions.clone(),
                    },
                ),
            };
            last_decision = terminal.decision;
            statuses.push(
                json!({"step":statuses.len() + 1,"status":status_name(terminal.result.status)}),
            );
            if terminal.result.status == Status::Succeeded {
                completed += 1;
                applied_any |= terminal.result.effect == Effect::Applied;
                continue;
            }
            let effect = if terminal.result.effect == Effect::Unknown {
                Effect::Unknown
            } else if applied_any || terminal.result.effect == Effect::Applied {
                Effect::Partial
            } else {
                Effect::None
            };
            let status = if effect == Effect::Unknown {
                Status::Unknown
            } else {
                terminal.result.status
            };
            let outcome = Outcome::SequenceRan {
                completed,
                total: value.steps.len(),
            };
            let summary = outcome.summary();
            let observed = outcome.observed();
            return Terminal {
                result: InvocationResult::new(
                    context.invocation,
                    status,
                    effect,
                    terminal.result.readiness,
                    effect == Effect::None,
                    &summary,
                    json!({"tab":selected.handle.as_str(),"completed_steps":completed,"total_steps":value.steps.len(),"steps":statuses}),
                    outcome.next_steps(),
                ),
                decision: last_decision,
                physical_id: terminal.physical_id,
                observed,
            };
        }
        self.succeeded(context, last_decision, Some(selected.physical_id), if applied_any { Effect::Applied } else { Effect::None }, readiness(selected.readiness), !applied_any, Outcome::SequenceRan { completed, total: value.steps.len() }, json!({"tab":selected.handle.as_str(),"completed_steps":completed,"total_steps":value.steps.len(),"steps":statuses}))
    }

    fn handle_dialog(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &HandleDialog,
    ) -> Terminal {
        let selected = match lease.select_tab(value.tab.as_deref()) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let capability = if value.action == "status" {
            Capability::Read
        } else {
            Capability::Action
        };
        let decision = self.authorize(context, capability, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let observed = match self.dispatch(
            context,
            BrowserCommand::InspectDialog {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::Dialog {
                tab_id,
                present,
                dialog_type,
            }) if tab_id == selected.physical_id => (present, dialog_type),
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        };
        if value.action == "status" {
            return self.succeeded(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                readiness(selected.readiness),
                true,
                Outcome::DialogObserved {
                    present: observed.0,
                },
                json!({"tab":selected.handle.as_str(),"present":observed.0,"dialog_type":observed.1}),
            );
        }
        if !observed.0 {
            return self.failed(
                context,
                decision,
                Some(selected.physical_id),
                Refusal::NoDialogVisible,
                json!({"tab":selected.handle.as_str(),"handled":false}),
            );
        }
        let accept = value.action != "dismiss";
        let text = (value.action == "respond")
            .then(|| value.text.clone())
            .flatten();
        match self.dispatch(context, BrowserCommand::HandleDialog { tab_id: selected.physical_id, accept, text }) {
            Ok(BrowserOutcome::DialogHandled { tab_id, dialog_type: handled_type, accepted }) if tab_id == selected.physical_id => self.succeeded(context, decision, Some(tab_id), Effect::Applied, readiness(selected.readiness), false, Outcome::DialogHandled { accepted }, json!({"tab":selected.handle.as_str(),"dialog_type":if handled_type.is_empty(){observed.1}else{handled_type},"accepted":accepted,"handled":true})),
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => self.browser_failure(context, decision, error, Some(selected.physical_id)),
        }
    }

    fn diagnose(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Diagnose,
    ) -> Terminal {
        let selected = match lease.select_tab(value.tab.as_deref()) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let source = match value.source.as_str() {
            "console" => DiagnosticSource::Console,
            "network" => DiagnosticSource::Network,
            _ => DiagnosticSource::Both,
        };
        let detail = if value.detail == "all" {
            DiagnosticDetail::All
        } else {
            DiagnosticDetail::Problems
        };
        match self.dispatch(
            context,
            BrowserCommand::ReadDiagnostics {
                tab_id: selected.physical_id,
                source,
                detail,
                match_text: value.r#match.clone(),
                after: value.after.clone(),
                limit: u16::try_from(value.limit).expect("validated diagnostic limit"),
            },
        ) {
            Ok(BrowserOutcome::DiagnosticsRead {
                tab_id,
                entries,
                cursor,
                truncated,
                evicted,
                capture_started,
                omitted_count,
            }) if tab_id == selected.physical_id => {
                let mut authority_omitted = 0_usize;
                let entries: Vec<_> = entries
                    .into_iter()
                    .filter(|entry| match entry {
                        DiagnosticEntry::Console { url, .. }
                        | DiagnosticEntry::Network { url, .. } => {
                            let allowed = context
                                .snapshot
                                .authorize_landing(Capability::Read, url)
                                .allowed;
                            if !allowed {
                                authority_omitted += 1;
                            }
                            allowed
                        }
                    })
                    .collect();
                let count = entries.len();
                let omitted_count = omitted_count.saturating_add(authority_omitted);
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::None,
                    readiness(selected.readiness),
                    true,
                    Outcome::DiagnosticsRead {
                        count,
                        capture_started,
                        problems_only: value.detail == "problems",
                        host: observed_host(&selected.url),
                    },
                    json!({
                        "tab":selected.handle.as_str(),
                        "source":value.source,
                        "detail":value.detail,
                        "entries":entries,
                        "cursor":cursor,
                        "truncated":truncated,
                        "evicted":evicted,
                        "capture_started":capture_started,
                        "omitted_count":omitted_count
                    }),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn perform_record(
        &self,
        context: &InvocationContext<'_>,
        lease: Option<&WorkspaceLease>,
        value: &Record,
    ) -> Terminal {
        match value.action.as_str() {
            "start" => self.start_recording(
                context,
                lease.expect("recording start holds the workspace lease"),
                value,
            ),
            "status" => {
                // Needs no capability, but every path to the browser still crosses the runtime
                // gate -- status/stop/discard used to dispatch straight through, the one family
                // of operations in this executor that ignored a pause. See stop_recording and
                // discard_recording for the same fix and the same reasoning.
                let decision = self.authorize(context, CapabilitySet::EMPTY, None);
                if !decision.allowed {
                    return self.blocked(
                        context,
                        decision,
                        None,
                        Effect::None,
                        true,
                        json!({"reason":decision.reason.as_str()}),
                    );
                }
                match self.dispatch(
                    context,
                    BrowserCommand::StatusRecording {
                        recording_id: value.recording.clone(),
                    },
                ) {
                    Ok(BrowserOutcome::RecordingStatus { summary }) => {
                        self.recording_observed(context, decision, &summary)
                    }
                    Ok(outcome) => self.recording_selection_failure(context, outcome),
                    Err(error) => self.browser_failure(context, decision, error, None),
                }
            }
            "stop" => self.stop_recording(context, value.recording.as_deref()),
            "save" => self.save_recording(context, lease, value),
            "discard" => self.discard_recording(context, value.recording.as_deref()),
            _ => unreachable!("recording action was validated"),
        }
    }

    fn start_recording(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Record,
    ) -> Terminal {
        let selected = match lease.select_tab(value.tab.as_deref()) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::StartRecording {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::RecordingStarted { summary, existing })
                if summary.tab_id == selected.physical_id =>
            {
                let mut facts = recording_facts(&summary);
                if let Some(object) = facts.as_object_mut() {
                    object.insert("tab".into(), json!(selected.handle.as_str()));
                }
                self.succeeded(
                    context,
                    decision,
                    Some(selected.physical_id),
                    if existing {
                        Effect::None
                    } else {
                        Effect::Applied
                    },
                    readiness(selected.readiness),
                    existing,
                    Outcome::RecordingStarted {
                        host: observed_host(&selected.url),
                    },
                    facts,
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    fn stop_recording(&self, context: &InvocationContext<'_>, requested: Option<&str>) -> Terminal {
        // Needs no capability, but every operation that reaches the browser still crosses the
        // runtime pause/attention gate -- this one used to dispatch straight through it, so a
        // paused session could still have its recording stopped from underneath it.
        let decision = self.authorize(context, CapabilitySet::EMPTY, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::StopRecording {
                recording_id: requested.map(str::to_owned),
            },
        ) {
            Ok(BrowserOutcome::RecordingStopped { summary, changed }) => self.succeeded(
                context,
                decision,
                Some(summary.tab_id),
                if changed {
                    Effect::Applied
                } else {
                    Effect::None
                },
                Readiness::NotApplicable,
                true,
                Outcome::RecordingStopped {
                    duration_ms: summary.duration_ms,
                },
                recording_facts(&summary),
            ),
            Ok(outcome) => self.recording_selection_failure(context, outcome),
            Err(error) => self.browser_failure(context, decision, error, None),
        }
    }

    fn ensure_recording_stopped(
        &self,
        context: &InvocationContext<'_>,
        requested: Option<&str>,
    ) -> Result<PhysicalRecordingSummary, Box<Terminal>> {
        match self.dispatch(
            context,
            BrowserCommand::StopRecording {
                recording_id: requested.map(str::to_owned),
            },
        ) {
            Ok(BrowserOutcome::RecordingStopped { summary, .. }) => Ok(summary),
            Ok(outcome) => Err(Box::new(self.recording_selection_failure(context, outcome))),
            Err(error) => Err(Box::new(self.browser_failure(
                context,
                permitted(),
                error,
                None,
            ))),
        }
    }

    /// Govern a save, then let the browser encode and deliver it.
    ///
    /// Ghostlight decides whether the replay may be made and where it may go. The browser does
    /// the rest: it holds the frames, so it encodes them, and for a page or a file it delivers
    /// them without anything crossing (ADR-0109). Only a client return crosses, and then once.
    fn save_recording(
        &self,
        context: &InvocationContext<'_>,
        lease: Option<&WorkspaceLease>,
        value: &Record,
    ) -> Terminal {
        let stopped = match self.ensure_recording_stopped(context, value.recording.as_deref()) {
            Ok(summary) => summary,
            Err(terminal) => return *terminal,
        };

        let (destination, decision, tab_id, budget) =
            match self.recording_destination(context, lease, value, &stopped) {
                Ok(resolved) => resolved,
                Err(terminal) => return *terminal,
            };

        match self.dispatch(
            context,
            BrowserCommand::ExportRecording {
                recording_id: Some(stopped.recording_id.clone()),
                destination,
                max_output_bytes: budget,
            },
        ) {
            Ok(BrowserOutcome::RecordingExported {
                summary,
                encoded,
                delivery,
            }) if summary.recording_id == stopped.recording_id
                && summary.state != RecordingState::Recording =>
            {
                self.recording_delivered(context, decision, &summary, encoded, delivery)
            }
            Ok(BrowserOutcome::RecordingExportFailed { reason }) => {
                self.recording_export_failure(context, &reason)
            }
            Ok(
                outcome @ (BrowserOutcome::RecordingAmbiguous { .. }
                | BrowserOutcome::RecordingNotFound),
            ) => self.recording_selection_failure(context, outcome),
            Ok(_) => self.protocol_failure(context, decision, tab_id),
            Err(error) => self.browser_failure(context, decision, error, tab_id),
        }
    }

    /// Authorize one save and name where the browser should put the result.
    #[allow(clippy::type_complexity)]
    fn recording_destination(
        &self,
        context: &InvocationContext<'_>,
        lease: Option<&WorkspaceLease>,
        value: &Record,
        stopped: &PhysicalRecordingSummary,
    ) -> Result<(RecordingDestination, Decision, Option<u64>, usize), Box<Terminal>> {
        if let Some(requested_target) = value.target.as_deref() {
            let lease = lease.expect("recording target save holds the workspace lease");
            let (selected, target) = match self.resolve_target(lease, None, requested_target) {
                Ok(value) => value,
                Err(error) => return Err(Box::new(self.workspace_failure(context, error))),
            };
            let decision = self.authorize(context, Capability::Write, Some(selected.url.as_str()));
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
                BrowserCommand::DescribeTargets {
                    tab_id: selected.physical_id,
                    locators: vec![target.locator.clone()],
                },
            ) {
                Ok(BrowserOutcome::TargetsDescribed { tab_id, targets })
                    if tab_id == selected.physical_id && targets.len() == 1 =>
                {
                    if targets[0].credential_class {
                        return Err(Box::new(
                            self.credential_handoff(context, decision, &selected),
                        ));
                    }
                }
                Ok(_) => {
                    return Err(Box::new(self.protocol_failure(
                        context,
                        decision,
                        Some(selected.physical_id),
                    )))
                }
                Err(error) => {
                    return Err(Box::new(self.browser_failure(
                        context,
                        decision,
                        error,
                        Some(selected.physical_id),
                    )))
                }
            }
            return Ok((
                RecordingDestination::Target {
                    tab_id: selected.physical_id,
                    locator: target.locator,
                    file_name: RECORDING_FILE_NAME.into(),
                },
                decision,
                Some(selected.physical_id),
                RECORDING_LOCAL_MAX_BYTES,
            ));
        }

        // A download stays in the browser, but the recording still pictures pages the caller
        // must be allowed to read, so both remaining destinations are authorized the same way.
        let denied = stopped.source_urls.iter().find_map(|url| {
            let decision = context.snapshot.authorize_landing(Capability::Read, url);
            (!decision.allowed).then_some(decision)
        });
        let decision = denied.unwrap_or_else(permitted);
        if !decision.allowed {
            return Err(Box::new(self.blocked(
                context,
                decision,
                Some(stopped.tab_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            )));
        }
        if value.download {
            return Ok((
                RecordingDestination::Download {
                    file_name: RECORDING_FILE_NAME.into(),
                },
                decision,
                Some(stopped.tab_id),
                RECORDING_LOCAL_MAX_BYTES,
            ));
        }
        Ok((
            RecordingDestination::Client,
            decision,
            Some(stopped.tab_id),
            RECORDING_TRANSFER_MAX_BYTES,
        ))
    }

    /// Report a delivered replay in the terms a reader cares about.
    fn recording_delivered(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        summary: &PhysicalRecordingSummary,
        encoded: EncodedRecording,
        delivery: RecordingDelivery,
    ) -> Terminal {
        let landing = match &delivery {
            RecordingDelivery::Attached { tab_id } => Some(*tab_id),
            _ => Some(summary.tab_id),
        };
        let facts = json!({
            "recording":summary.recording_id,
            "state":recording_state_name(summary.state),
            "duration_ms":encoded.duration_ms,
            "frame_count":encoded.frame_count,
            "captured_frame_count":encoded.captured_frame_count,
            "gif_bytes":encoded.byte_count,
            "width":encoded.width,
            "height":encoded.height,
            "delivery":recording_delivery_name(&delivery)
        });
        let outcome = Outcome::RecordingSaved {
            duration_ms: encoded.duration_ms,
            delivery: match &delivery {
                RecordingDelivery::Attached { .. } => SavedTo::PageTarget,
                RecordingDelivery::Downloaded => SavedTo::Download,
                RecordingDelivery::Returned { .. } => SavedTo::Client,
            },
        };
        // Encoding the same recording twice produces the same replay, but putting it on a page or
        // on disk again is a fresh effect on the world, so only a client return is repeat-safe.
        let landed = !matches!(delivery, RecordingDelivery::Returned { .. });
        let mut terminal = self.succeeded(
            context,
            decision,
            landing,
            if landed {
                Effect::Applied
            } else {
                Effect::None
            },
            Readiness::NotApplicable,
            !landed,
            outcome,
            facts,
        );
        if let RecordingDelivery::Returned { mime_type, data } = delivery {
            terminal.result = terminal
                .result
                .with_content(ServiceContent::Image { mime_type, data });
        }
        terminal
    }

    fn discard_recording(
        &self,
        context: &InvocationContext<'_>,
        requested: Option<&str>,
    ) -> Terminal {
        // Needs no capability, but every operation that reaches the browser still crosses the
        // runtime pause/attention gate -- this one used to dispatch straight through it.
        let decision = self.authorize(context, CapabilitySet::EMPTY, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::DiscardRecording {
                recording_id: requested.map(str::to_owned),
            },
        ) {
            Ok(BrowserOutcome::RecordingDiscarded {
                recording_id,
                released_bytes,
            }) => self.succeeded(
                context,
                decision,
                None,
                Effect::Applied,
                Readiness::NotApplicable,
                true,
                Outcome::RecordingDiscarded,
                json!({
                    "recording":recording_id,
                    "discarded":true,
                    "released_bytes":released_bytes
                }),
            ),
            Ok(outcome) => self.recording_selection_failure(context, outcome),
            Err(error) => self.browser_failure(context, decision, error, None),
        }
    }

    fn recording_observed(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        summary: &PhysicalRecordingSummary,
    ) -> Terminal {
        self.succeeded(
            context,
            decision,
            None,
            Effect::None,
            Readiness::NotApplicable,
            true,
            Outcome::RecordingObserved {
                frames: summary.frame_count,
                duration_ms: summary.duration_ms,
            },
            recording_facts(summary),
        )
    }

    fn recording_selection_failure(
        &self,
        context: &InvocationContext<'_>,
        outcome: BrowserOutcome,
    ) -> Terminal {
        let facts = match outcome {
            BrowserOutcome::RecordingAmbiguous { recording_ids } => {
                json!({"reason":"ambiguous","recordings":recording_ids})
            }
            BrowserOutcome::RecordingNotFound => json!({"reason":"not_found"}),
            _ => return self.protocol_failure(context, permitted(), None),
        };
        self.failed(
            context,
            permitted(),
            None,
            Refusal::RecordingUnavailable,
            facts,
        )
    }

    fn recording_export_failure(&self, context: &InvocationContext<'_>, reason: &str) -> Terminal {
        self.failed(
            context,
            permitted(),
            None,
            Refusal::RecordingExportFailed,
            json!({"reason":bounded(reason, 160)}),
        )
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

    fn resolve_optional_target(
        &self,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
    ) -> Result<(SelectedTab, Option<String>, Option<TargetRole>), WorkspaceError> {
        match target {
            Some(target) => {
                let (tab, target) = self.resolve_target(lease, requested_tab, target)?;
                Ok((tab, Some(target.locator), Some(target.role)))
            }
            None => Ok((lease.select_tab(requested_tab)?, None, None)),
        }
    }

    fn resolve_target(
        &self,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: &str,
    ) -> Result<(SelectedTab, SelectedTarget), WorkspaceError> {
        if let Some(requested) = requested_tab {
            let tab = lease.select_tab(Some(requested))?;
            let target = lease.resolve_target(target, Some(&tab))?;
            Ok((tab, target))
        } else {
            let target = lease.resolve_target(target, None)?;
            let tab = lease.select_tab(Some(target.tab.as_str()))?;
            Ok((tab, target))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_location(
        &self,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        view: Option<&str>,
        x: Option<f64>,
        y: Option<f64>,
    ) -> Result<ResolvedLocation, WorkspaceError> {
        if let Some(target) = target {
            let (tab, target) = self.resolve_target(lease, requested_tab, target)?;
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

    /// Authorize one operation, checked against a real destination whenever it names one.
    ///
    /// `url: None` means this operation has no tab in play at all -- `list_tabs` is the only
    /// caller, since listing needs no destination to check. Every operation that names a tab
    /// must pass `Some(&tab.url)`, the tab's raw string as tracked right now, **even when that
    /// string is empty** because the tab's first landing has not been governed yet (a page
    /// calling `window.open()` is adopted immediately, before the async navigation-committed
    /// event that would establish its real host arrives). An empty or otherwise unparseable
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
        let runtime = self.governance.runtime_decision();
        let _ = self
            .browser
            .publish_control_state(self.governance.runtime_state());
        if !runtime.allowed {
            return runtime;
        }
        url.map_or_else(
            || context.snapshot.authorize_requirements(requirements),
            |url| context.snapshot.authorize_landing(requirements, url),
        )
    }

    fn authorize_commits(
        &self,
        context: &InvocationContext<'_>,
        requirements: impl Into<CapabilitySet>,
        tab: &PhysicalTab,
        commits: &[String],
    ) -> Decision {
        let requirements = requirements.into();
        let runtime = self.governance.runtime_decision();
        let _ = self
            .browser
            .publish_control_state(self.governance.runtime_state());
        if !runtime.allowed {
            return runtime;
        }
        let mut observed = None;
        for url in commits.iter().chain(std::iter::once(&tab.url)) {
            let decision = context.snapshot.authorize_landing(requirements, url);
            if !decision.allowed {
                return decision;
            }
            if decision.observed && observed.is_none() {
                observed = Some(decision);
            }
        }
        observed.unwrap_or_else(Decision::permitted)
    }

    fn authorize_tab_close(&self, context: &InvocationContext<'_>) -> Decision {
        let runtime = self.governance.runtime_decision();
        let _ = self
            .browser
            .publish_control_state(self.governance.runtime_state());
        if !runtime.allowed {
            return runtime;
        }
        let action = context.snapshot.authorize_capability(Capability::Action);
        if !action.allowed {
            return action;
        }
        let close = context.snapshot.authorize_tab_close();
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
        let browser = self.target_browser(context)?;
        let outcome = self.browser.call(
            &browser,
            context.workspace.as_str(),
            command,
            context.deadline,
            context.cancellation.flag(),
        );
        if let Ok(outcome) = &outcome {
            self.observe(context.invocation, observed_from(outcome));
        }
        outcome
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
        let chosen = choose_browser(
            context.requested_browser,
            pinned.as_deref(),
            &self.browser.browsers(),
        )?;
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
        match self.browser.call(
            &browser,
            context.workspace.as_str(),
            BrowserCommand::CloseTab {
                tab_id: tab.physical_id,
            },
            deadline,
            &cancelled,
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
            };
        }
        if error.effect_unknown() {
            return self.unknown(
                context,
                decision,
                physical_id,
                Refusal::EffectUnknown,
                json!({"reason":"browser_effect_unknown"}),
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
        }
    }

    fn workspace_failure(
        &self,
        context: &InvocationContext<'_>,
        error: WorkspaceError,
    ) -> Terminal {
        let reason = WorkspaceReason::from(error);
        let refusal = Refusal::WorkspaceUnusable { reason };
        let summary = refusal.summary();
        let status = if error == WorkspaceError::Held {
            Status::Blocked
        } else {
            Status::Failed
        };
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                status,
                Effect::None,
                Readiness::Unknown,
                status == Status::Failed,
                &summary,
                json!({"reason":reason.as_fact()}),
                refusal.next_steps(),
            ),
            decision: if status == Status::Blocked {
                Decision::refused(ReasonCode::RuntimeHold)
            } else {
                Decision::permitted()
            },
            physical_id: None,
            observed: Observed::default(),
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
    /// Which intake admitted the workspace this work arrived on.
    channel: Option<IntakeChannel>,
}

/// Milliseconds elapsed since an invocation began, saturating rather than wrapping.
fn elapsed_ms(started: std::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

struct InvocationContext<'a> {
    invocation: &'a str,
    workspace: &'a WorkspaceId,
    /// The browser this call named, when it named one.
    ///
    /// Only a call that can open the first tab of a workspace can carry one. Every other call
    /// reaches the browser through a handle that already belongs to it.
    requested_browser: Option<&'a str>,
    snapshot: &'a AuthoritySnapshot,
    deadline: Instant,
    cancellation: &'a CancellationToken,
}

struct Terminal {
    result: InvocationResult,
    decision: Decision,
    physical_id: Option<u64>,
    observed: Observed,
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
        Operation::RunSequence(_) => PresentationActivity::Quiet,
        Operation::HandleDialog(_) => PresentationActivity::Dialog,
        Operation::Diagnose(_) => PresentationActivity::Quiet,
        Operation::Record(value) if value.action == "start" => PresentationActivity::Screenshot,
        Operation::Record(_) => PresentationActivity::Quiet,
    }
}

fn step_activity(step: &SequenceStep) -> PresentationActivity {
    match step {
        SequenceStep::Click { .. } => PresentationActivity::Click,
        SequenceStep::TypeText { .. } => PresentationActivity::Type,
        SequenceStep::Fill { .. } => PresentationActivity::Fill,
        SequenceStep::PressKey { .. } => PresentationActivity::Key,
        SequenceStep::Scroll { .. } => PresentationActivity::Scroll,
        SequenceStep::Hover { .. } => PresentationActivity::Hover,
        SequenceStep::Wait { .. } => PresentationActivity::Wait,
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
        _ => None,
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
        Operation::RunSequence(value) => value.timeout_ms,
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
        ReasonCode::Permitted | ReasonCode::InvalidRequest | ReasonCode::RuntimeAttention => {
            BlockedReason::Unspecified
        }
    }
}

const WAIT_RECEIPT_RESERVE_MS: u64 = 250;

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

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Succeeded => "succeeded",
        Status::Blocked => "blocked",
        Status::Failed => "failed",
        Status::Cancelled => "cancelled",
        Status::AttentionRequired => "attention_required",
        Status::Unknown => "unknown",
    }
}

fn browser_reason(error: &BrowserError) -> &'static str {
    match error {
        BrowserError::DisconnectedBeforeDispatch => "browser_disconnected",
        BrowserError::CancelledBeforeDispatch => "cancelled",
        BrowserError::DeadlineBeforeDispatch => "deadline",
        BrowserError::Primitive(_) => "browser_primitive_failed",
        BrowserError::LocalInterlock(_) => "browser_local_interlock",
        BrowserError::Protocol(_)
        | BrowserError::Authentication
        | BrowserError::Incompatible { .. } => "browser_contract_failed",
        _ => "browser_effect_unknown",
    }
}

#[cfg(test)]
mod tests {
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

    use crate::browser::testing::{summary, FakeBrowser, FAKE_BROWSER};
    use ghostlight_bridge::service::IntakeChannel;

    use crate::governance::{AuditRecord, AuditSink, GovernanceFacade};
    use crate::language::outcome::Observed;
    use crate::presentation::{PresentationError, PresentationPort, PresentationReactor};
    use crate::workbench::WorkbenchProjection;
    use crate::workspace::WorkspaceStore;

    use super::{
        deregister_active_authority, observation_budget_ms, observed_from, readiness_name,
        register_active_authority, ApplicationExecutor, CancellationToken, Effect, Readiness,
        Status,
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
        let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp);
        let audit = Arc::new(MemoryAudit::default());
        let executor = ApplicationExecutor::new(
            governance,
            workspaces.clone(),
            browser.clone(),
            PresentationReactor::new(Arc::new(NoPresentation)),
            WorkbenchProjection::default(),
            audit.clone(),
        );
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

    fn all_open_policy_with(config: &str) -> String {
        format!(
            r#"{{"schema":3,"name":"work test","version":"1","grants":[{{"id":"all","hosts":{{"allow":["*"]}},"allowed":["read","action","write","execute"]}}],"config":{config}}}"#
        )
    }

    #[test]
    fn work_follows_the_attended_browser_and_then_stays_where_it_started() {
        let (executor, browser, workspaces, workspace, _) = fixture();
        browser.connect(vec![
            summary("browser_chrome", false),
            summary("browser_edge", true),
        ]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));

        // Nothing named a browser, so the work goes where the person last was.
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        assert_eq!(browser.routed(), vec!["browser_edge"]);
        assert_eq!(
            workspaces.browser_of(workspace.as_str()).as_deref(),
            Some("browser_edge")
        );

        // The person turns to Chrome. Established work does not follow them there.
        browser.connect(vec![
            summary("browser_chrome", true),
            summary("browser_edge", false),
        ]);
        browser.push(Ok(BrowserOutcome::Navigated {
            tab: tab(7, "https://example.com/next"),
            committed_urls: vec!["https://example.com/next".into()],
        }));
        let followed = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/next"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(followed.status, Status::Succeeded);
        assert_eq!(browser.routed(), vec!["browser_edge", "browser_edge"]);
    }

    #[test]
    fn an_ambiguous_bootstrap_names_the_choices_and_touches_no_browser() {
        let (executor, browser, workspaces, workspace, _) = fixture();
        browser.connect(vec![
            summary("browser_chrome", false),
            summary("browser_edge", false),
        ]);

        let refused = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
            None,
            &CancellationToken::default(),
        );

        assert_eq!(refused.status, Status::Failed);
        assert_eq!(refused.effect, Effect::None);
        assert_eq!(refused.facts["reason"], json!("browser_ambiguous"));
        assert_eq!(
            refused.facts["browsers"],
            json!(["browser_chrome", "browser_edge"])
        );
        assert!(refused.repeat_safe);
        // Nothing was dispatched and nothing was bound, so naming a browser next still works.
        assert!(browser.routed().is_empty());
        assert_eq!(workspaces.browser_of(workspace.as_str()), None);
    }

    #[test]
    fn a_named_browser_opens_there_and_a_named_stranger_is_refused() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![
            summary("browser_chrome", false),
            summary("browser_edge", true),
        ]);
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));

        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true,"browser":"browser_chrome"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        assert_eq!(browser.routed(), vec!["browser_chrome"]);

        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![summary("browser_edge", true)]);
        let refused = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true,"browser":"browser_absent"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(refused.status, Status::Failed);
        assert_eq!(refused.facts["reason"], json!("browser_unknown"));
        assert!(browser.routed().is_empty());
    }

    #[test]
    fn listing_tabs_answers_with_no_browser_connected_and_shows_the_ones_there_are() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.connect(vec![]);

        let listed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            None,
            &CancellationToken::default(),
        );

        // A read about this workspace's own tabs never needs a browser to answer truthfully.
        assert_eq!(listed.status, Status::Succeeded);
        assert_eq!(listed.facts["tabs"], json!([]));
        assert_eq!(listed.facts["browsers"], json!([]));

        browser.connect(vec![summary(FAKE_BROWSER, true)]);
        let listed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(
            listed.facts["browsers"],
            json!([{"browser":FAKE_BROWSER,"name":null,"attended":true}])
        );
    }

    #[test]
    fn repeated_policy_denials_pause_browser_work_until_the_user_resumes() {
        let policy = temporary_policy("denial-attention");
        fs::write(
            &policy,
            r#"{"schema":3,"name":"deny reads","version":"1","grants":[],"config":[]}"#,
        )
        .unwrap();
        let governance = GovernanceFacade::new(Some(policy.clone()), None);
        let (executor, browser, _, workspace, _) = fixture_with_governance(governance.clone());

        for _ in 0..3 {
            let denied = executor.execute(
                &workspace,
                "browser_tabs",
                json!({"action":"list"}),
                None,
                &CancellationToken::default(),
            );
            assert_eq!(denied.status, Status::Blocked);
        }
        assert_eq!(governance.runtime_state(), RuntimeControlState::Attention);
        assert_eq!(
            browser.control_states().last(),
            Some(&RuntimeControlState::Attention)
        );

        let paused = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(paused.status, Status::AttentionRequired);

        assert_eq!(
            governance.apply_runtime_intent(RuntimeControlIntent::Resume),
            RuntimeControlState::Active
        );
        let denied_again = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(denied_again.status, Status::Blocked);
        assert_eq!(governance.runtime_state(), RuntimeControlState::Active);
        let _ = fs::remove_file(policy);
    }

    #[test]
    fn observation_budget_preserves_time_for_the_physical_receipt() {
        assert_eq!(
            observation_budget_ms(3_000, Duration::from_millis(3_000)),
            2_750
        );
        assert_eq!(observation_budget_ms(100, Duration::from_millis(100)), 0);
        assert_eq!(
            observation_budget_ms(500, Duration::from_millis(5_000)),
            500
        );
    }

    #[test]
    fn record_actions_cross_only_the_extension_owned_request_receipt_seam() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::RecordingStarted {
            summary: recording_summary(RecordingState::Recording, "https://example.com/"),
            existing: false,
        }));
        assert_eq!(
            executor
                .execute(
                    &workspace,
                    "browser_record",
                    json!({"action":"start","tab":handle}),
                    None,
                    &CancellationToken::default(),
                )
                .status,
            Status::Succeeded
        );

        browser.push(Ok(BrowserOutcome::RecordingStatus {
            summary: recording_summary(RecordingState::Recording, "https://example.com/"),
        }));
        executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"status"}),
            None,
            &CancellationToken::default(),
        );

        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            changed: true,
        }));
        executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"stop"}),
            None,
            &CancellationToken::default(),
        );

        browser.push(Ok(BrowserOutcome::RecordingDiscarded {
            recording_id: "recording_one".into(),
            released_bytes: 1,
        }));
        executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"discard"}),
            None,
            &CancellationToken::default(),
        );

        let calls = browser.calls();
        assert!(matches!(
            calls[1],
            BrowserCommand::StartRecording { tab_id: 7 }
        ));
        assert!(matches!(calls[2], BrowserCommand::StatusRecording { .. }));
        assert!(matches!(calls[3], BrowserCommand::StopRecording { .. }));
        assert!(matches!(calls[4], BrowserCommand::DiscardRecording { .. }));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, BrowserCommand::ExportRecording { .. })));
    }

    #[test]
    fn one_invocations_completion_never_clears_a_still_active_sibling() {
        // Recording status/stop/discard skip the workspace lease and can run fully concurrently
        // with a lease-holding operation on the same workspace, on separate threads. A single
        // snapshot-per-workspace registry let one invocation's insert overwrite another's entry,
        // and one invocation's finish clear an entry a still-running sibling depended on -- the
        // reader's fallback on a miss is the widest policy available, so this was a fail-open
        // race, not just a confusing one. Two distinct policies stand in for two distinct
        // invocations' snapshots, so a clobber would be visible as the wrong one surviving.
        let (executor, _, _, workspace, _) = fixture();
        let registry = executor.active_authority();

        let narrow = GovernanceFacade::new(
            Some({
                let path = temporary_policy("sibling-narrow");
                fs::write(
                    &path,
                    r#"{"schema":3,"name":"narrow","version":"1","grants":[]}"#,
                )
                .unwrap();
                path
            }),
            None,
        )
        .snapshot(&crate::language::RequestRestrictions::default());
        let wide = GovernanceFacade::new(None, None)
            .snapshot(&crate::language::RequestRestrictions::default());
        assert_ne!(
            narrow.id(),
            wide.id(),
            "the two snapshots must be distinguishable"
        );

        register_active_authority(&registry, workspace.as_str(), "invocation_a", &narrow);
        register_active_authority(&registry, workspace.as_str(), "invocation_b", &wide);

        // invocation_a finishes first. Its own entry must go; invocation_b's must not.
        deregister_active_authority(&registry, workspace.as_str(), "invocation_a");
        {
            let locked = registry.lock().unwrap();
            let entries = locked
                .get(workspace.as_str())
                .expect("invocation_b is still active");
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].0, "invocation_b");
            assert_eq!(entries[0].1.id(), wide.id());
        }

        // invocation_b finishes. The workspace now has no active invocation at all.
        deregister_active_authority(&registry, workspace.as_str(), "invocation_b");
        assert!(registry.lock().unwrap().get(workspace.as_str()).is_none());
    }

    #[test]
    fn a_paused_runtime_refuses_recording_status_stop_and_discard() {
        // Every other operation in this executor -- even ones needing no capability at all, like
        // activating a tab -- crosses the runtime gate before it can reach the browser. Recording
        // status, stop, and discard used to be the one family that dispatched straight through,
        // so pausing Ghostlight did not actually stop a recording from being stopped or discarded
        // out from under the person who paused it.
        let governance = GovernanceFacade::new(None, None);
        assert_eq!(
            governance.apply_runtime_intent(RuntimeControlIntent::Hold),
            RuntimeControlState::Held
        );
        let (executor, browser, _, workspace, _) = fixture_with_governance(governance);

        for action in ["status", "stop", "discard"] {
            let result = executor.execute(
                &workspace,
                "browser_record",
                json!({"action":action}),
                None,
                &CancellationToken::default(),
            );
            assert_ne!(
                result.status,
                Status::Succeeded,
                "recording {action} must not succeed while paused: {result:?}"
            );
        }
        assert!(
            browser.calls().is_empty(),
            "a paused runtime must never reach the browser at all: {:?}",
            browser.calls()
        );
    }

    #[test]
    fn a_save_asks_the_browser_for_one_finished_replay() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            changed: true,
        }));
        browser.push(Ok(BrowserOutcome::RecordingExported {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            encoded: EncodedRecording {
                frame_count: 17,
                captured_frame_count: 65,
                duration_ms: 30_400,
                width: 1_280,
                height: 800,
                byte_count: 3_804_453,
            },
            delivery: RecordingDelivery::Returned {
                mime_type: "image/gif".into(),
                data: "R0lGODlh".into(),
            },
        }));

        let result = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save","recording":"recording_one"}),
            None,
            &CancellationToken::default(),
        );

        // Stop, then export. Nothing in between: the frames never come here to be encoded.
        let calls = browser.calls();
        assert!(matches!(calls[0], BrowserCommand::StopRecording { .. }));
        assert!(matches!(
            calls[1],
            BrowserCommand::ExportRecording {
                destination: RecordingDestination::Client,
                max_output_bytes: RECORDING_TRANSFER_MAX_BYTES,
                ..
            }
        ));
        assert_eq!(calls.len(), 2);
        // The sentence is what a person would say about a replay. The mechanism it was made from
        // is real, and belongs in the facts.
        assert_eq!(
            result.summary,
            "Saved a replay of 30 seconds of page changes."
        );
        assert_eq!(result.facts["frame_count"], json!(17));
        assert_eq!(result.facts["captured_frame_count"], json!(65));
        assert_eq!(result.facts["gif_bytes"], json!(3_804_453));
        assert_eq!(result.facts["delivery"], json!("returned_to_client"));
    }

    #[test]
    fn a_download_save_never_returns_the_replay_bytes() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            changed: true,
        }));
        browser.push(Ok(BrowserOutcome::RecordingExported {
            summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
            encoded: EncodedRecording {
                frame_count: 40,
                captured_frame_count: 40,
                duration_ms: 1_500,
                width: 1_280,
                height: 800,
                byte_count: 9_000_000,
            },
            delivery: RecordingDelivery::Downloaded,
        }));

        let result = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save","recording":"recording_one","download":true}),
            None,
            &CancellationToken::default(),
        );

        assert!(matches!(
            browser.calls()[1],
            BrowserCommand::ExportRecording {
                destination: RecordingDestination::Download { .. },
                // A replay that stays in the browser is not bounded by what can cross out of it.
                max_output_bytes: RECORDING_LOCAL_MAX_BYTES,
                ..
            }
        ));
        assert_eq!(
            result.summary,
            "Downloaded a replay of 2 seconds of page changes."
        );
        assert!(
            result.content.is_empty(),
            "a browser-local save must return no bytes: {:?}",
            result.content
        );
    }

    #[test]
    fn client_save_authorizes_source_before_recording_bytes_cross() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, "http://127.0.0.1/private"),
            changed: true,
        }));

        let result = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":"save","recording":"recording_one"}),
            None,
            &CancellationToken::default(),
        );

        assert_eq!(result.status, Status::Blocked);
        assert!(matches!(
            browser.calls().as_slice(),
            [BrowserCommand::StopRecording { .. }]
        ));
    }

    #[test]
    fn unsatisfied_wait_is_decisive_before_the_invocation_deadline() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: false,
            elapsed_ms: 750,
            readiness: BrowserReadiness::Complete,
        }));
        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({
                "tab":tab_handle,
                "condition":"text_present",
                "value":"never present",
                "timeout_ms":1_000
            }),
            None,
            &CancellationToken::default(),
        );

        assert_eq!(waited.status, Status::Failed);
        assert_eq!(waited.effect, Effect::None);
        assert!(waited.repeat_safe);
        assert_eq!(waited.facts["satisfied"], false);
        let calls = browser.calls();
        let timeout_ms = calls
            .iter()
            .find_map(|call| match call {
                BrowserCommand::Observe { timeout_ms, .. } => Some(*timeout_ms),
                _ => None,
            })
            .unwrap();
        assert!(timeout_ms <= 750);
        assert!(timeout_ms > 0);
    }

    #[test]
    fn open_read_close_is_one_truthful_result_per_call() {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        let handle = opened.facts["tab"].as_str().unwrap();
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "Example Domain".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/".into(),
        }));
        let read = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":handle}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(read.status, Status::Succeeded);
        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Succeeded);
        assert_eq!(audit.0.lock().unwrap().len(), 3);
        let calls = browser.calls();
        assert_eq!(calls.len(), 3);
        assert!(matches!(
            &calls[0],
            BrowserCommand::OpenTab { url, group_title }
                if url == "https://example.com" && group_title == "Ghostlight - test"
        ));
    }

    #[test]
    fn readiness_names_stay_the_vocabulary_a_result_uses() {
        for value in [
            Readiness::NotApplicable,
            Readiness::Loading,
            Readiness::Interactive,
            Readiness::Complete,
            Readiness::Unknown,
        ] {
            let encoded = serde_json::to_value(value).unwrap();
            assert_eq!(
                encoded.as_str().unwrap(),
                readiness_name(value),
                "the observation and the result would disagree about readiness"
            );
        }
    }

    #[test]
    fn browser_seam_observes_landing_facts_but_not_outcome_measurements() {
        let tabs = observed_from(&BrowserOutcome::Tabs {
            tabs: vec![tab(7, "https://example.com/")],
        });
        assert_eq!(tabs, Observed::default());

        let text = observed_from(&BrowserOutcome::Text {
            tab_id: 7,
            text: "three private words".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/private?id=3".into(),
        });
        assert_eq!(text.host.as_deref(), Some("example.com"));
        assert_eq!(text.count, None);

        let wait = observed_from(&BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 1_830,
            readiness: BrowserReadiness::Complete,
        });
        assert_eq!(wait.readiness.as_deref(), Some("complete"));
        assert_eq!(wait.count, None);
    }

    #[test]
    fn outcome_language_and_the_seam_observe_without_carrying_page_detail() {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://Example.com/patients/48219?ssn=1#note"),
            committed_urls: vec!["https://example.com/patients/48219?ssn=1#note".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/patients/48219?ssn=1#note"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        let handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "Patient 48219 has an appointment".into(),
            truncated: false,
            title: "Example".into(),
            url: "https://example.com/patients/48219?ssn=1#note".into(),
        }));
        let read = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab":handle}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(read.status, Status::Succeeded);
        assert_eq!(read.summary, "Read 5 words from example.com.");

        let records = audit.0.lock().unwrap();
        let landing = &records[0].observed;
        assert_eq!(landing.host.as_deref(), Some("example.com"));
        assert_eq!(landing.readiness.as_deref(), Some("complete"));
        let text = &records[1].observed;
        assert_eq!(text.host.as_deref(), Some("example.com"));
        assert_eq!(text.count, Some(5));

        // The model-facing facts legitimately carry the URL and the text. The audit carries the
        // same action, and none of it.
        assert!(read.facts["url"].as_str().unwrap().contains("48219"));
        let encoded = serde_json::to_string(&*records).unwrap();
        for detail in ["patients", "48219", "ssn", "note", "appointment"] {
            assert!(
                !encoded.contains(detail),
                "the audit leaked {detail} from a page"
            );
        }
    }

    #[test]
    fn an_observation_never_outlives_the_invocation_it_describes() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        // A failure before any browser crossing must not leave a key behind either.
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"nonsense":true}),
            None,
            &CancellationToken::default(),
        );
        assert!(
            executor.observations().is_empty(),
            "the registry grows with every invocation instead of with work in flight"
        );
    }

    #[test]
    fn a_capture_reports_its_size_and_a_wait_reports_how_long_it_waited() {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::Screenshot {
            tab_id: 7,
            mime_type: "image/jpeg".into(),
            data: "image".into(),
            width: 1280,
            height: 720,
            viewport: ViewportGeometry {
                scope: CaptureScope::Viewport,
                page_x: 0.0,
                page_y: 0.0,
                css_width: 1280.0,
                css_height: 720.0,
                visual_page_x: 0.0,
                visual_page_y: 0.0,
                visual_css_width: 1280.0,
                visual_css_height: 720.0,
                device_scale: 1.0,
                zoom: 1.0,
                output_scale: 1.0,
            },
        }));
        let captured = executor.execute(
            &workspace,
            "browser_screenshot",
            json!({"tab":handle}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(captured.summary, "Captured the viewport at 1280x720.");

        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 1_830,
            readiness: BrowserReadiness::Complete,
        }));
        let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({"tab":handle,"condition":"load_ready","timeout_ms":5_000}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(waited.summary, "example.com finished loading in 2 seconds.");

        let records = audit.0.lock().unwrap();
        let capture = &records[1].observed;
        assert_eq!((capture.width, capture.height), (Some(1280), Some(720)));
        // A capture is its own invocation. It reports the size it took and leaves the landing to
        // the invocation that navigated, because an observation never outlives its invocation.
        assert_eq!(capture.host.as_deref(), None);
        let wait = &records[2].observed;
        assert_eq!(wait.count, Some(2));
        assert_eq!(wait.readiness.as_deref(), Some("complete"));
    }

    #[test]
    fn tab_close_policy_blocks_before_browser_dispatch() {
        let policy = temporary_policy("tab-close");
        fs::write(
            &policy,
            all_open_policy_with(
                r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let (executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap();
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Blocked);
        assert_eq!(closed.effect, Effect::None);
        assert_eq!(closed.facts["reason"], "tab_close_denied");
        assert_eq!(browser.calls().len(), 1);
        let _ = fs::remove_file(policy);
    }

    #[test]
    fn refused_navigation_audits_only_the_attempted_host() {
        let (executor, browser, _, workspace, audit) = fixture();
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"http://127.0.0.1/private/record-42?token=secret#detail"}),
            None,
            &CancellationToken::default(),
        );

        assert_eq!(result.status, Status::Blocked);
        assert_eq!(
            result.summary,
            "Blocked: 127.0.0.1 is protected and is never automated."
        );
        assert!(browser.calls().is_empty());

        let records = audit.0.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].observed.host.as_deref(), Some("127.0.0.1"));
        let encoded = serde_json::to_string(&records[0]).unwrap();
        assert!(!encoded.contains("record-42"));
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("secret"));
    }

    #[test]
    fn a_tab_whose_landing_is_not_yet_known_is_refused_rather_than_checked_by_capability_alone() {
        // A page under the model's control can open a child tab; the workspace adopts it
        // immediately, before the async navigation-committed event that would establish its real
        // host arrives, so the tab's own url is briefly empty. A policy that grants Read only on
        // a specific host, never "*", proves the point: if authorize() fell back to a host-blind
        // capability check for this tab (the bug), the read would wrongly succeed, because that
        // fallback unions grants across every host the policy names, ignoring which host the tab
        // is actually on. It must be refused instead, exactly as an unparseable committed URL
        // already is on the click/type/fill path.
        let policy = temporary_policy("unknown-landing");
        fs::write(
            &policy,
            r#"{"schema":3,"name":"work test","version":"1","grants":[{"id":"approved","hosts":{"allow":["approved.example"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
        let (executor, browser, workspaces, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));

        // An ordinary, fully governed opener tab, admitted under the narrow policy.
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://approved.example/"),
            committed_urls: vec!["https://approved.example/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://approved.example","new_tab":true}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded, "{opened:?}");
        let bound_browser = workspaces.browser_of(workspace.as_str()).unwrap();

        // Adopt a child the way a page's own `window.open()` does: through the real production
        // path (`WorkspaceStore::apply_browser_child`), which stores the new tab's url as empty
        // regardless of what the physical tab record otherwise says, exactly as it does when the
        // extension reports a page-opened tab before that tab's first navigation has committed.
        let (child_workspace, handle) = workspaces
            .apply_browser_child(&bound_browser, 7, &tab(8, "https://attacker.example/"))
            .expect("the opener tab is owned by this workspace");
        assert_eq!(child_workspace, workspace);

        let read = executor.execute(
            &workspace,
            "browser_read",
            json!({"tab": handle.as_str()}),
            None,
            &CancellationToken::default(),
        );

        assert_eq!(
            read.status,
            Status::Blocked,
            "a tab with no known landing must be refused, not checked by capability alone: {read:?}"
        );
        assert!(
            browser
                .calls()
                .iter()
                .all(|call| !matches!(call, BrowserCommand::Observe { .. })),
            "the browser must never be asked to read a tab whose destination was never checked"
        );
        let _ = fs::remove_file(policy);
    }

    #[test]
    fn physical_action_receipt_names_the_target_without_trusting_its_role() {
        let (executor, browser, _, workspace, audit) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

        browser.push(Ok(BrowserOutcome::Targets {
            tab_id: 7,
            targets: vec![ObservedTarget {
                locator: "hostile-role".into(),
                role: "Save my document".into(),
                name: "private patient action".into(),
                state: vec![],
                credential_class: false,
            }],
        }));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            None,
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();

        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: Some(PhysicalActionSubject {
                role: "Save my document".into(),
                name: "Save patient record".into(),
            }),
        }));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            None,
            &CancellationToken::default(),
        );

        assert_eq!(
            clicked.summary,
            "Clicked the \"Save patient record\" control on example.com."
        );
        let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(!encoded.contains("Save my document"));
        assert!(encoded.contains("Save patient record"));
        assert!(!encoded.contains("private patient action"));
    }

    #[test]
    fn governance_can_remove_target_names_without_losing_the_safe_role() {
        let policy = temporary_policy("hide-target-names");
        fs::write(
            &policy,
            all_open_policy_with(
                r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let (executor, browser, _, workspace, audit) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::Targets {
            tab_id: 7,
            targets: vec![ObservedTarget {
                locator: "save".into(),
                role: "button".into(),
                name: "Save patient record".into(),
                state: vec![],
                credential_class: false,
            }],
        }));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            None,
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();
        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: Some(PhysicalActionSubject {
                role: "button".into(),
                name: "Save patient record".into(),
            }),
        }));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            None,
            &CancellationToken::default(),
        );

        assert_eq!(clicked.summary, "Clicked a button on example.com.");
        let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(!encoded.contains("Save patient record"));
        let _ = fs::remove_file(policy);
    }

    #[test]
    fn local_preserve_tabs_refusal_is_blocked_without_an_effect() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap();
        browser.push(Err(crate::browser::BrowserError::LocalInterlock(
            "preserved".into(),
        )));
        let closed = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"close","tab":handle}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(closed.status, Status::Blocked);
        assert_eq!(closed.effect, Effect::None);
        assert!(closed.repeat_safe);
        assert_eq!(closed.facts["reason"], "browser_local_interlock");
        assert_eq!(closed.next_steps.len(), 1);
    }

    #[test]
    fn uncertain_effect_has_no_replay_guidance() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Err(crate::browser::BrowserError::DisconnectedAfterDispatch));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Unknown);
        assert_eq!(result.effect, Effect::Unknown);
        assert!(!result.repeat_safe);
        assert!(result.next_steps.is_empty());
    }

    #[test]
    fn direct_and_sequence_actions_use_the_same_physical_executor_path() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::Targets {
            tab_id: 7,
            targets: vec![ObservedTarget {
                locator: "button-1".into(),
                role: "button".into(),
                name: "Go".into(),
                state: vec![],
                credential_class: false,
            }],
        }));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            None,
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();

        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: None,
        }));
        let direct = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(direct.status, Status::Succeeded);

        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: None,
        }));
        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 5,
            readiness: BrowserReadiness::Complete,
        }));
        let sequence = executor.execute(&workspace, "browser_sequence", json!({"tab":tab_handle,"steps":[{"action":"click","target":target},{"action":"wait","condition":"load_ready"}]}), None, &CancellationToken::default());
        assert_eq!(sequence.status, Status::Succeeded);
        let calls = browser.calls();
        assert_eq!(
            calls
                .iter()
                .filter(|call| matches!(call, BrowserCommand::Activate { .. }))
                .count(),
            2
        );
        assert_eq!(
            calls
                .iter()
                .filter(|call| matches!(call, BrowserCommand::Observe { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn credential_target_requests_handoff_before_any_value_dispatch() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        let credential = ObservedTarget {
            locator: "password-1".into(),
            role: "textbox".into(),
            name: "Password".into(),
            state: vec![],
            credential_class: true,
        };
        browser.push(Ok(BrowserOutcome::Targets {
            tab_id: 7,
            targets: vec![credential.clone()],
        }));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            None,
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();
        browser.push(Ok(BrowserOutcome::TargetsDescribed {
            tab_id: 7,
            targets: vec![credential],
        }));
        let result = executor.execute(
            &workspace,
            "browser_fill_form",
            json!({"tab":tab_handle,"fields":[{"target":target,"value":"not-sent"}]}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::AttentionRequired);
        assert_eq!(result.facts["values_sent"], false);
        assert_eq!(
            browser.control_states().last(),
            Some(&ghostlight_bridge::browser::RuntimeControlState::Attention)
        );
        assert!(!browser
            .calls()
            .iter()
            .any(|call| matches!(call, BrowserCommand::Fill { .. })));
    }

    #[test]
    fn denied_redirect_is_compensated_without_replay_risk() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "http://127.0.0.1/private"),
            committed_urls: vec![
                "https://example.com/".into(),
                "http://127.0.0.1/private".into(),
            ],
        }));
        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::None);
        assert_eq!(result.facts["compensated"], true);
        assert!(result.repeat_safe);
    }

    #[test]
    fn denied_redirect_remains_visibly_open_when_close_policy_refuses_compensation() {
        let policy = temporary_policy("retained-denied-landing");
        fs::write(
            &policy,
            all_open_policy_with(
                r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let (executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "http://127.0.0.1/private"),
            committed_urls: vec![
                "https://example.com/".into(),
                "http://127.0.0.1/private".into(),
            ],
        }));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::Applied);
        assert!(!result.repeat_safe);
        assert_eq!(result.facts["compensated"], false);
        assert_eq!(result.facts["retained"], true);
        assert_eq!(browser.calls().len(), 1);
        let _ = fs::remove_file(policy);
    }

    #[test]
    fn denied_redirect_remains_visibly_open_when_local_preservation_refuses_compensation() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "http://127.0.0.1/private"),
            committed_urls: vec![
                "https://example.com/".into(),
                "http://127.0.0.1/private".into(),
            ],
        }));
        browser.push(Err(crate::browser::BrowserError::LocalInterlock(
            "preserved".into(),
        )));
        let result = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked);
        assert_eq!(result.effect, Effect::Applied);
        assert!(!result.repeat_safe);
        assert_eq!(result.facts["compensated"], false);
        assert_eq!(result.facts["retained"], true);
        assert_eq!(browser.calls().len(), 2);
    }

    #[test]
    fn stale_target_fails_before_browser_dispatch() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::Targets {
            tab_id: 7,
            targets: vec![ObservedTarget {
                locator: "old".into(),
                role: "button".into(),
                name: "Old".into(),
                state: vec![],
                credential_class: false,
            }],
        }));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":tab_handle}),
            None,
            &CancellationToken::default(),
        );
        let target = inspected.facts["items"][0]["target"]
            .as_str()
            .unwrap()
            .to_owned();
        browser.push(Ok(BrowserOutcome::Navigated {
            tab: tab(7, "https://example.org/"),
            committed_urls: vec!["https://example.org/".into()],
        }));
        let navigated = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"tab":tab_handle,"url":"https://example.org"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(navigated.status, Status::Succeeded);
        let before = browser.calls().len();
        let stale = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"target":target}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(stale.status, Status::Failed);
        assert_eq!(stale.facts["reason"], "stale_target");
        assert_eq!(browser.calls().len(), before);
    }

    #[test]
    fn screenshot_coordinates_are_resolved_once_and_expire_on_navigation() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
        let viewport = ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 10.0,
            page_y: 20.0,
            css_width: 800.0,
            css_height: 600.0,
            visual_page_x: 10.0,
            visual_page_y: 20.0,
            visual_css_width: 800.0,
            visual_css_height: 600.0,
            device_scale: 1.0,
            zoom: 1.0,
            output_scale: 0.5,
        };
        browser.push(Ok(BrowserOutcome::Screenshot {
            tab_id: 7,
            mime_type: "image/jpeg".into(),
            data: "image".into(),
            width: 400,
            height: 300,
            viewport,
        }));
        let screenshot = executor.execute(
            &workspace,
            "browser_screenshot",
            json!({"tab":tab_handle}),
            None,
            &CancellationToken::default(),
        );
        assert!(screenshot.facts.get("data").is_none());
        assert!(matches!(
            screenshot.content.as_slice(),
            [ServiceContent::Image { mime_type, data }]
                if mime_type == "image/jpeg" && data == "image"
        ));
        let view = screenshot.facts["view"].as_str().unwrap().to_owned();
        browser.push(Ok(BrowserOutcome::Activated {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
            subject: None,
        }));
        let clicked = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"view":view,"x":100,"y":50}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(clicked.status, Status::Succeeded);
        assert!(browser.calls().iter().any(|call| matches!(
            call,
            BrowserCommand::ActivatePoint { point, expected_viewport, .. }
                if point.x == 210.0 && point.y == 120.0 && *expected_viewport == viewport
        )));

        browser.push(Ok(BrowserOutcome::Navigated {
            tab: tab(7, "https://example.org/"),
            committed_urls: vec!["https://example.org/".into()],
        }));
        let _ = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"tab":tab_handle,"url":"https://example.org"}),
            None,
            &CancellationToken::default(),
        );
        let before = browser.calls().len();
        let stale = executor.execute(
            &workspace,
            "browser_click",
            json!({"tab":tab_handle,"view":view,"x":1,"y":1}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(stale.facts["reason"], "stale_view");
        assert_eq!(browser.calls().len(), before);
    }
}
