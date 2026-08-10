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
    BrowserCommand, BrowserOutcome, BrowserReadiness, PhysicalField, PhysicalFile, PhysicalPoint,
    PhysicalTab, PresentationActivity,
};
use ghostlight_bridge::service::ServiceContent;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::browser::{BrowserError, BrowserPort};
use crate::events::{DenialPresentation, DomainEvent};
use crate::governance::{
    AuditRecord, AuditSink, AuthoritySnapshot, Capability, Decision, GovernanceFacade, ReasonCode,
};
use crate::language::{
    self, Click, Drag, FillForm, FormField, Hover, Operation, PressKey, RunScript, RunSequence,
    ScrollPage, SequenceStep, TypeText, UploadFiles, Wait,
};
use crate::presentation::PresentationReactor;
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
    audit: Arc<dyn AuditSink>,
    active_authority: ActiveAuthorityRegistry,
}

/// Current immutable invocation snapshots used only to govern asynchronous browser events.
pub type ActiveAuthorityRegistry = Arc<Mutex<HashMap<String, AuthoritySnapshot>>>;

impl ApplicationExecutor {
    /// Construct the orchestrator's only model-requested mutation entry point.
    #[must_use]
    pub fn new(
        governance: GovernanceFacade,
        workspaces: WorkspaceStore,
        browser: Arc<dyn BrowserPort>,
        presentation: PresentationReactor,
        audit: Arc<dyn AuditSink>,
    ) -> Self {
        Self {
            governance,
            workspaces,
            browser,
            presentation,
            audit,
            active_authority: Arc::new(Mutex::new(HashMap::new())),
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
        let gate = CompletionGate::default();
        let decoded = language::decode(tool, input);
        let (operation, capability) = match decoded {
            Ok(operation) => {
                let capability = operation_capability(&operation);
                (operation, capability)
            }
            Err(error) => {
                let snapshot = self
                    .governance
                    .snapshot(&language::RequestRestrictions::default());
                let decision = Decision {
                    allowed: false,
                    reason: ReasonCode::InvalidRequest,
                };
                let result = InvocationResult::new(
                    &invocation,
                    Status::Failed,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    "The call does not match the Ghostlight catalog.",
                    json!({"reason":"invalid_input","detail":error.to_string()}),
                    vec!["Correct the call using the advertised tool schema.".into()],
                );
                let terminal = Terminal {
                    result,
                    decision,
                    physical_id: None,
                };
                return self.finish(
                    &gate,
                    terminal,
                    workspace,
                    tool,
                    Capability::Read,
                    &snapshot,
                );
            }
        };
        let deadline_ms = caller_deadline_ms
            .unwrap_or_else(|| operation_timeout(&operation))
            .clamp(100, 30_000);
        let deadline = Instant::now() + Duration::from_millis(deadline_ms);
        let lease = loop {
            match self.workspaces.acquire(workspace) {
                Ok(lease) => break Some(lease),
                Err(WorkspaceError::Busy)
                    if !cancellation.is_cancelled() && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(_) => break None,
            }
        };
        let snapshot = self.governance.snapshot(operation.restrictions());
        let context = InvocationContext {
            invocation: &invocation,
            workspace,
            snapshot: &snapshot,
            deadline,
            cancellation,
        };
        let terminal = if let Some(lease) = lease {
            self.emit(DomainEvent::WorkStarted {
                invocation: invocation.clone(),
                workspace: workspace.as_str().into(),
                activity: operation_activity(&operation),
            });
            self.active_authority
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(workspace.as_str().into(), snapshot.clone());
            let terminal = self.run(&context, &lease, &operation);
            self.active_authority
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(workspace.as_str());
            terminal
        } else if cancellation.is_cancelled() {
            Terminal {
                result: InvocationResult::new(
                    &invocation,
                    Status::Cancelled,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    "The browser job was cancelled before it started.",
                    json!({"reason":"cancelled"}),
                    vec![],
                ),
                decision: Decision {
                    allowed: true,
                    reason: ReasonCode::Permitted,
                },
                physical_id: None,
            }
        } else if Instant::now() >= deadline {
            Terminal {
                result: InvocationResult::new(
                    &invocation,
                    Status::Failed,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    "The browser job deadline expired while waiting for the workspace.",
                    json!({"reason":"deadline"}),
                    vec![],
                ),
                decision: Decision {
                    allowed: true,
                    reason: ReasonCode::Permitted,
                },
                physical_id: None,
            }
        } else {
            self.workspace_failure(&context, WorkspaceError::UnknownWorkspace)
        };
        self.finish(&gate, terminal, workspace, tool, capability, &snapshot)
    }

    fn finish(
        &self,
        gate: &CompletionGate,
        terminal: Terminal,
        workspace: &WorkspaceId,
        tool: &str,
        capability: Capability,
        snapshot: &AuthoritySnapshot,
    ) -> InvocationResult {
        let event = match terminal.result.status {
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
        let record = AuditRecord::now(
            &terminal.result.invocation,
            workspace.as_str(),
            tool,
            capability,
            snapshot.id(),
            terminal.decision,
            &status,
            &effect,
        );
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
                &value.kind,
                value.max_items,
            ),
            Operation::Find(value) => self.find(
                context,
                lease,
                value.tab.as_deref(),
                &value.text,
                &value.kind,
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
            Operation::Hover(value) => self.perform_hover(context, lease, value),
            Operation::FillForm(value) => self.perform_fill(context, lease, value),
            Operation::TypeText(value) => self.perform_type_text(context, lease, value),
            Operation::PressKey(value) => self.perform_key(context, lease, value),
            Operation::Drag(value) => self.perform_drag(context, lease, value),
            Operation::UploadFiles(value) => self.upload_files(context, lease, value),
            Operation::RunScript(value) => self.run_script(context, lease, value),
            Operation::Wait(value) => self.perform_wait(context, lease, value),
            Operation::RunSequence(value) => self.sequence(context, lease, value),
            Operation::HandleDialog(value) => self.handle_dialog(
                context,
                lease,
                value.tab.as_deref(),
                value.accept,
                value.text.as_deref(),
            ),
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
                self.succeeded(
                    context,
                    decision,
                    None,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    "Controlled tabs listed.",
                    json!({"tabs":facts}),
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
        let decision = self.authorize(context, Capability::Action, current_url(&selected));
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
                    "Controlled tab brought into view.",
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
        let decision = self.authorize(context, Capability::Action, Some(url));
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
        let landing = self.authorize_commits(context, Capability::Action, &tab, &commits);
        if !landing.allowed {
            return match self.compensate_close(context, lease, &controlled) {
                CloseCompensation::Closed => self.blocked(
                    context,
                    landing,
                    Some(tab.tab_id),
                    Effect::None,
                    true,
                    json!({"reason":landing.reason.as_str(),"compensated":true}),
                ),
                CloseCompensation::Retained => self.blocked(
                    context,
                    landing,
                    Some(tab.tab_id),
                    Effect::Applied,
                    false,
                    json!({"reason":landing.reason.as_str(),"compensated":false,"retained":true}),
                ),
                CloseCompensation::Unknown => self.unknown(
                    context,
                    landing,
                    Some(tab.tab_id),
                    "The landing was denied, but the new tab's final state cannot be determined.",
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
        self.succeeded(context, landing, Some(governed.physical_id), Effect::Applied, readiness(governed.readiness), false, "Page opened and its landing was governed.", json!({"tab":governed.handle.as_str(),"url":governed.url,"title":governed.title,"created":true,"document_generation":governed.generation}))
    }

    fn navigate_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        url: &str,
    ) -> Terminal {
        let decision = self.authorize(context, Capability::Action, Some(url));
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
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
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
                    self.authorize_commits(context, Capability::Action, &tab, &committed_urls);
                if !landing.allowed {
                    let _ = lease.hold_tab(&selected.handle);
                    self.emit(DomainEvent::HoldEntered {
                        invocation: context.invocation.into(),
                        workspace: context.workspace.as_str().into(),
                        physical_id: selected.physical_id,
                    });
                    return self.blocked(context, landing, Some(selected.physical_id), Effect::Applied, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}));
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
                self.succeeded(context, landing, Some(governed.physical_id), Effect::Applied, readiness(governed.readiness), false, "Page navigation completed and its landing was governed.", json!({"tab":governed.handle.as_str(),"url":governed.url,"title":governed.title,"document_generation":governed.generation}))
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
        let decision = self.authorize(context, Capability::Action, current_url(&selected));
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
            "Browser history navigation completed and its landing was governed.",
            json!({"direction":direction}),
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
        let decision = self.authorize(context, Capability::Action, current_url(&selected));
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
            "Page reloaded and its landing was governed.",
            json!({"bypass_cache":bypass_cache}),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_navigation(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        selected: &SelectedTab,
        decision: Decision,
        outcome: Result<BrowserOutcome, BrowserError>,
        summary: &str,
        mut facts: Value,
    ) -> Terminal {
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
                    return self.blocked(
                        context,
                        landing,
                        Some(selected.physical_id),
                        Effect::Applied,
                        false,
                        json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}),
                    );
                }
                let governed = match lease.apply_landing(&selected.handle, &tab) {
                    Ok(tab) => tab,
                    Err(error) => return self.workspace_failure(context, error),
                };
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
                    summary,
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
                    "Controlled tab closed.",
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
        let (selected, locator) = match self.resolve_optional_target(lease, requested_tab, target) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, current_url(&selected));
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
                    return self.blocked(context, landing, Some(tab_id), Effect::None, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}));
                }
                self.succeeded(context, landing, Some(tab_id), Effect::None, readiness(selected.readiness), true, "Page text read.", json!({"tab":selected.handle.as_str(),"url":url,"title":bounded(&title,500),"text":bounded(&text,max_chars),"truncated":truncated || text.chars().count() > max_chars,"document_generation":selected.generation}))
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
            "Page inspected.",
            "items",
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
            "Targets found.",
            "matches",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn targets_operation(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        capability: Capability,
        command: BrowserCommand,
        summary: &str,
        fact_key: &str,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, capability, current_url(&selected));
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
                    summary,
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
        let (selected, locator) = match self.resolve_optional_target(lease, requested_tab, target) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, current_url(&selected));
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
                        "Screenshot exceeded the product result bound.",
                        json!({"reason":"screenshot_too_large"}),
                        vec![],
                    );
                }
                let view = match lease.register_view(&selected, viewport, width, height) {
                    Ok(view) => view,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let mut terminal = self.succeeded(context, decision, Some(tab_id), Effect::None, readiness(selected.readiness), true, "Screenshot captured.", json!({"tab":selected.handle.as_str(),"view":view.as_str(),"mime_type":mime_type,"width":width,"height":height}));
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
        let decision = self.authorize(context, Capability::Action, current_url(&selected));
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
        let (command, facts, summary) = match location {
            ResolvedLocation::Target { tab, target } => {
                self.emit(DomainEvent::TargetIndicated {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    physical_id: tab.physical_id,
                    locator: target.locator.clone(),
                });
                (
                    BrowserCommand::Activate {
                        tab_id: tab.physical_id,
                        locator: target.locator,
                        button: value.button.clone(),
                        click_count: value.click_count,
                    },
                    json!({"tab":tab.handle.as_str(),"target":target.handle.as_str(),"activated":true}),
                    "Target activated.",
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
                "Screenshot point activated.",
            ),
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Activated {
                tab,
                committed_urls,
            }) => self.action_success(
                context,
                lease,
                decision,
                Capability::Action,
                &selected,
                &tab,
                &committed_urls,
                summary,
                facts,
            ),
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
        let (selected, locator) = match self.resolve_optional_target(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, current_url(&selected));
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
            Ok(BrowserOutcome::Scrolled { tab_id, x, y }) if tab_id == selected.physical_id => {
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
                        "Target revealed."
                    } else {
                        "Page scrolled."
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
        let decision = self.authorize(context, Capability::Read, current_url(&selected));
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
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    "Tab zoom set.",
                    json!({"tab":selected.handle.as_str(),"percent":(zoom * 100.0).round() as u16,"zoomed":true}),
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
        let decision = self.authorize(context, Capability::Read, current_url(&selected));
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
        let (command, facts) = match location {
            ResolvedLocation::Target { tab, target } => {
                self.emit(DomainEvent::TargetIndicated {
                    invocation: context.invocation.into(),
                    workspace: context.workspace.as_str().into(),
                    physical_id: tab.physical_id,
                    locator: target.locator.clone(),
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
            Ok(BrowserOutcome::Hovered { tab_id }) if tab_id == selected.physical_id => self
                .succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::Applied,
                    readiness(selected.readiness),
                    true,
                    "Pointer hover applied.",
                    facts,
                ),
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
        let capability = if submit.is_some() {
            Capability::Execute
        } else {
            Capability::Write
        };
        let decision = self.authorize(context, capability, current_url(&selected));
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
        match self.dispatch(context, BrowserCommand::Fill { tab_id: selected.physical_id, fields, submit_locator: submit.map(|target| target.locator) }) {
            Ok(BrowserOutcome::Filled { tab, filled_count, submitted, committed_urls }) => self.action_success(context, lease, decision, capability, &selected, &tab, &committed_urls, "Form fields filled.", json!({"tab":selected.handle.as_str(),"filled_count":filled_count,"submitted":submitted})),
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => self.browser_failure(context, decision, error, Some(selected.physical_id)),
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
        let decision = self.authorize(context, Capability::Write, current_url(&selected));
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
                committed_urls,
            }) => self.action_success(
                context,
                lease,
                decision,
                Capability::Write,
                &selected,
                &tab,
                &committed_urls,
                "Text typed through browser input events.",
                json!({"tab":selected.handle.as_str(),"target":target.handle.as_str(),"typed":true,"character_count":character_count}),
            ),
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
            });
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
        let decision = self.authorize(context, Capability::Action, current_url(&selected));
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
                committed_urls,
            }) => self.action_success(
                context,
                lease,
                decision,
                Capability::Action,
                &selected,
                &tab,
                &committed_urls,
                "Drag completed.",
                facts,
            ),
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
        let decision = self.authorize(context, Capability::Write, current_url(&selected));
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
                    "The selected local files could not be prepared safely.",
                    json!({"reason":reason}),
                    vec![],
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
                    "Files uploaded to the selected control.",
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
        let decision = self.authorize(context, Capability::Execute, current_url(&selected));
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
                self.action_success(
                    context,
                    lease,
                    decision,
                    Capability::Execute,
                    &selected,
                    &tab,
                    &committed_urls,
                    "Page script evaluated.",
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
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::AttentionRequired,
                Effect::None,
                readiness(selected.readiness),
                false,
                "A credential-class field requires user handoff in the visible browser.",
                json!({"tab":selected.handle.as_str(),"credential_handoff":true,"values_sent":false}),
                vec!["Complete the credential field in the visible browser, then inspect the page again.".into()],
            ),
            decision,
            physical_id: Some(selected.physical_id),
        }
    }

    fn perform_key(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &PressKey,
    ) -> Terminal {
        let (selected, locator) = match self.resolve_optional_target(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Action, current_url(&selected));
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
                committed_urls,
            }) => self.action_success(
                context,
                lease,
                decision,
                Capability::Action,
                &selected,
                &tab,
                &committed_urls,
                "Keyboard action sent.",
                json!({"tab":selected.handle.as_str(),"key":key,"pressed":true}),
            ),
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
        let (selected, locator) = match self.resolve_optional_target(
            lease,
            value.tab.as_deref(),
            value.target.as_deref(),
        ) {
            Ok(value) => value,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, Capability::Read, current_url(&selected));
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
                readiness: observed,
            }) if tab_id == selected.physical_id => {
                let _ = lease.update_readiness(&selected.handle, observed);
                let status = if satisfied {
                    Status::Succeeded
                } else {
                    Status::Failed
                };
                Terminal {
                    result: InvocationResult::new(
                        context.invocation,
                        status,
                        Effect::None,
                        readiness(observed),
                        true,
                        if satisfied {
                            "Wait condition satisfied."
                        } else {
                            "Wait condition was not satisfied before the timeout."
                        },
                        json!({"tab":selected.handle.as_str(),"condition":value.condition,"satisfied":satisfied,"elapsed_ms":elapsed_ms}),
                        if satisfied {
                            vec![]
                        } else {
                            vec!["Inspect the current page before choosing another action.".into()]
                        },
                    ),
                    decision,
                    physical_id: Some(tab_id),
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
        let mut last_decision = self.authorize(context, Capability::Read, current_url(&selected));
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
            return Terminal {
                result: InvocationResult::new(
                    context.invocation,
                    status,
                    effect,
                    terminal.result.readiness,
                    effect == Effect::None,
                    "Sequence stopped at the first non-successful step.",
                    json!({"tab":selected.handle.as_str(),"completed_steps":completed,"total_steps":value.steps.len(),"steps":statuses}),
                    vec![],
                ),
                decision: last_decision,
                physical_id: terminal.physical_id,
            };
        }
        self.succeeded(context, last_decision, Some(selected.physical_id), if applied_any { Effect::Applied } else { Effect::None }, readiness(selected.readiness), !applied_any, "Sequence completed.", json!({"tab":selected.handle.as_str(),"completed_steps":completed,"total_steps":value.steps.len(),"steps":statuses}))
    }

    fn handle_dialog(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        accept: bool,
        text: Option<&str>,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let capability = if text.is_some() {
            Capability::Write
        } else {
            Capability::Action
        };
        let decision = self.authorize(context, capability, current_url(&selected));
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
        let dialog_type = match self.dispatch(
            context,
            BrowserCommand::InspectDialog {
                tab_id: selected.physical_id,
            },
        ) {
            Ok(BrowserOutcome::Dialog {
                tab_id,
                present: true,
                dialog_type,
            }) if tab_id == selected.physical_id => dialog_type,
            Ok(BrowserOutcome::Dialog { present: false, .. }) => {
                return self.failed(
                    context,
                    decision,
                    Some(selected.physical_id),
                    "No JavaScript dialog is currently visible.",
                    json!({"tab":selected.handle.as_str(),"handled":false}),
                    vec![],
                )
            }
            Ok(_) => return self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                return self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        };
        match self.dispatch(context, BrowserCommand::HandleDialog { tab_id: selected.physical_id, accept, text: text.map(str::to_owned) }) {
            Ok(BrowserOutcome::DialogHandled { tab_id, dialog_type: handled_type, accepted }) if tab_id == selected.physical_id => self.succeeded(context, decision, Some(tab_id), Effect::Applied, readiness(selected.readiness), false, "Browser dialog handled.", json!({"tab":selected.handle.as_str(),"dialog_type":if handled_type.is_empty(){dialog_type}else{handled_type},"accepted":accepted,"handled":true})),
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => self.browser_failure(context, decision, error, Some(selected.physical_id)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn action_success(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        decision: Decision,
        landing_capability: Capability,
        selected: &SelectedTab,
        physical: &PhysicalTab,
        commits: &[String],
        summary: &str,
        mut facts: Value,
    ) -> Terminal {
        let landing = self.authorize_commits(context, landing_capability, physical, commits);
        if !landing.allowed {
            let _ = lease.hold_tab(&selected.handle);
            self.emit(DomainEvent::HoldEntered {
                invocation: context.invocation.into(),
                workspace: context.workspace.as_str().into(),
                physical_id: selected.physical_id,
            });
            return self.blocked(context, landing, Some(selected.physical_id), Effect::Applied, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}));
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
            summary,
            facts,
        )
    }

    fn resolve_optional_target(
        &self,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
    ) -> Result<(SelectedTab, Option<String>), WorkspaceError> {
        match target {
            Some(target) => {
                let (tab, target) = self.resolve_target(lease, requested_tab, target)?;
                Ok((tab, Some(target.locator)))
            }
            None => Ok((lease.select_tab(requested_tab)?, None)),
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

    fn authorize(
        &self,
        context: &InvocationContext<'_>,
        capability: Capability,
        url: Option<&str>,
    ) -> Decision {
        let runtime = self.governance.runtime_decision();
        let _ = self
            .browser
            .publish_control_state(self.governance.runtime_state());
        if !runtime.allowed {
            return runtime;
        }
        url.map_or_else(
            || context.snapshot.authorize_capability(capability),
            |url| context.snapshot.authorize_landing(capability, url),
        )
    }

    fn authorize_commits(
        &self,
        context: &InvocationContext<'_>,
        capability: Capability,
        tab: &PhysicalTab,
        commits: &[String],
    ) -> Decision {
        let runtime = self.governance.runtime_decision();
        let _ = self
            .browser
            .publish_control_state(self.governance.runtime_state());
        if !runtime.allowed {
            return runtime;
        }
        for url in commits.iter().chain(std::iter::once(&tab.url)) {
            let decision = context.snapshot.authorize_landing(capability, url);
            if !decision.allowed {
                return decision;
            }
        }
        Decision {
            allowed: true,
            reason: ReasonCode::Permitted,
        }
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
        context.snapshot.authorize_tab_close()
    }

    fn dispatch(
        &self,
        context: &InvocationContext<'_>,
        command: BrowserCommand,
    ) -> Result<BrowserOutcome, BrowserError> {
        self.browser.call(
            context.workspace.as_str(),
            command,
            context.deadline,
            context.cancellation.flag(),
        )
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
        match self.browser.call(
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
        summary: &str,
        facts: Value,
    ) -> Terminal {
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::Succeeded,
                effect,
                readiness,
                repeat_safe,
                summary,
                facts,
                vec![],
            ),
            decision,
            physical_id,
        }
    }

    fn blocked(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        effect: Effect,
        repeat_safe: bool,
        facts: Value,
    ) -> Terminal {
        let attention = decision.reason == ReasonCode::RuntimeAttention;
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
                if attention {
                    "The browser job requires user attention."
                } else {
                    "Authority blocked the browser job."
                },
                facts,
                vec![],
            ),
            decision,
            physical_id,
        }
    }

    fn failed(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        summary: &str,
        facts: Value,
        next_steps: Vec<String>,
    ) -> Terminal {
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::Failed,
                Effect::None,
                Readiness::Unknown,
                true,
                summary,
                facts,
                next_steps,
            ),
            decision,
            physical_id,
        }
    }

    fn unknown(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        summary: &str,
        facts: Value,
    ) -> Terminal {
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                Status::Unknown,
                Effect::Unknown,
                Readiness::Unknown,
                false,
                summary,
                facts,
                vec![],
            ),
            decision,
            physical_id,
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
            "The browser adapter returned an incompatible primitive receipt.",
            json!({"reason":"incompatible_browser_receipt"}),
            vec![],
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
            return Terminal {
                result: InvocationResult::new(
                    context.invocation,
                    Status::Blocked,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    "A local browser safety setting blocked this action.",
                    json!({"reason":"browser_local_interlock"}),
                    vec![
                        "The user can change the relevant Ghostlight extension setting or perform the action directly."
                            .into(),
                    ],
                ),
                decision,
                physical_id,
            };
        }
        if error.effect_unknown() {
            return self.unknown(
                context,
                decision,
                physical_id,
                "A browser effect was dispatched, but its final state cannot be determined.",
                json!({"reason":"browser_effect_unknown"}),
            );
        }
        let status = if matches!(error, BrowserError::CancelledBeforeDispatch) {
            Status::Cancelled
        } else {
            Status::Failed
        };
        let next_steps = if matches!(error, BrowserError::DisconnectedBeforeDispatch) {
            vec!["Reconnect the Ghostlight browser adapter.".into()]
        } else {
            vec![]
        };
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                status,
                Effect::None,
                Readiness::Unknown,
                true,
                "The browser job stopped before a physical effect.",
                json!({"reason":browser_reason(&error)}),
                next_steps,
            ),
            decision,
            physical_id,
        }
    }

    fn workspace_failure(
        &self,
        context: &InvocationContext<'_>,
        error: WorkspaceError,
    ) -> Terminal {
        let (reason, next_steps) = match error {
            WorkspaceError::StaleTab | WorkspaceError::NoTab | WorkspaceError::AmbiguousTab => (
                "tab_unavailable",
                vec!["Call browser_list_tabs to obtain current controlled tab handles.".into()],
            ),
            WorkspaceError::StaleTarget => (
                "stale_target",
                vec![
                    "Call browser_inspect_page or browser_find to obtain current target handles."
                        .into(),
                ],
            ),
            WorkspaceError::StaleView | WorkspaceError::ViewPointOutOfBounds => (
                "stale_view",
                vec!["Call browser_take_screenshot to obtain a current view handle.".into()],
            ),
            WorkspaceError::Held => ("tab_held", vec![]),
            WorkspaceError::Busy => (
                "workspace_busy",
                vec!["Wait for the active Ghostlight invocation to finish.".into()],
            ),
            WorkspaceError::NotOwnedTab
            | WorkspaceError::NotOwnedTarget
            | WorkspaceError::NotOwnedView
            | WorkspaceError::TargetTabMismatch
            | WorkspaceError::ViewTabMismatch
            | WorkspaceError::PhysicalTabOwned => ("ownership_mismatch", vec![]),
            WorkspaceError::UnknownWorkspace => ("workspace_closed", vec![]),
        };
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
                "The requested workspace target is not currently usable.",
                json!({"reason":reason}),
                next_steps,
            ),
            decision: Decision {
                allowed: status != Status::Blocked,
                reason: if status == Status::Blocked {
                    ReasonCode::RuntimeHold
                } else {
                    ReasonCode::Permitted
                },
            },
            physical_id: None,
        }
    }

    fn emit(&self, event: DomainEvent) {
        self.presentation.react(&event);
    }
}

fn denial_presentation(tool: &str, result: &InvocationResult) -> DenialPresentation {
    if tool == "browser_close_tab" {
        return match result.facts.get("reason").and_then(Value::as_str) {
            Some("tab_close_denied") => DenialPresentation::TabKeptOpenByPolicy,
            Some("browser_local_interlock") => DenialPresentation::TabKeptOpenBySetting,
            _ => DenialPresentation::Guardrail,
        };
    }
    DenialPresentation::Guardrail
}

struct InvocationContext<'a> {
    invocation: &'a str,
    workspace: &'a WorkspaceId,
    snapshot: &'a AuthoritySnapshot,
    deadline: Instant,
    cancellation: &'a CancellationToken,
}

struct Terminal {
    result: InvocationResult,
    decision: Decision,
    physical_id: Option<u64>,
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

fn operation_capability(operation: &Operation) -> Capability {
    match operation {
        Operation::ListTabs(_)
        | Operation::ReadPage(_)
        | Operation::InspectPage(_)
        | Operation::Find(_)
        | Operation::TakeScreenshot(_)
        | Operation::ScrollPage(_)
        | Operation::SetZoom(_)
        | Operation::Hover(_)
        | Operation::Wait(_) => Capability::Read,
        Operation::ActivateTab(_)
        | Operation::OpenPage(_)
        | Operation::NavigatePage(_)
        | Operation::NavigateHistory(_)
        | Operation::ReloadPage(_)
        | Operation::CloseTab(_)
        | Operation::Click(_)
        | Operation::Drag(_)
        | Operation::PressKey(_) => Capability::Action,
        Operation::TypeText(_) | Operation::UploadFiles(_) => Capability::Write,
        Operation::RunScript(_) => Capability::Execute,
        Operation::FillForm(value) => {
            if value.submit_target.is_some() {
                Capability::Execute
            } else {
                Capability::Write
            }
        }
        Operation::RunSequence(value) => value
            .steps
            .iter()
            .map(step_capability)
            .max()
            .unwrap_or(Capability::Read),
        Operation::HandleDialog(value) => {
            if value.text.is_some() {
                Capability::Write
            } else {
                Capability::Action
            }
        }
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
        Operation::SetZoom(_) => PresentationActivity::Zoom,
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

fn step_capability(step: &SequenceStep) -> Capability {
    match step {
        SequenceStep::Wait { .. } | SequenceStep::Scroll { .. } | SequenceStep::Hover { .. } => {
            Capability::Read
        }
        SequenceStep::Fill { .. } | SequenceStep::TypeText { .. } => Capability::Write,
        SequenceStep::Click { .. } | SequenceStep::PressKey { .. } => Capability::Action,
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
        _ => 8_000,
    }
}

const WAIT_RECEIPT_RESERVE_MS: u64 = 250;

fn observation_budget_ms(requested_ms: u64, remaining: Duration) -> u64 {
    let available = remaining.saturating_sub(Duration::from_millis(WAIT_RECEIPT_RESERVE_MS));
    let available_ms = u64::try_from(available.as_millis()).unwrap_or(u64::MAX);
    requested_ms.min(available_ms)
}

fn current_url(tab: &SelectedTab) -> Option<&str> {
    if tab.url.is_empty() {
        None
    } else {
        Some(&tab.url)
    }
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
        BrowserCommand, BrowserOutcome, BrowserReadiness, CaptureScope, ObservedTarget,
        PhysicalTab, ViewportGeometry,
    };
    use ghostlight_bridge::service::ServiceContent;
    use serde_json::json;

    use crate::browser::testing::FakeBrowser;
    use crate::governance::{AuditRecord, AuditSink, GovernanceFacade};
    use crate::presentation::{PresentationError, PresentationPort, PresentationReactor};
    use crate::workspace::WorkspaceStore;

    use super::{observation_budget_ms, ApplicationExecutor, CancellationToken, Effect, Status};

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
        let workspace = workspaces.admit("test".into());
        let audit = Arc::new(MemoryAudit::default());
        let executor = ApplicationExecutor::new(
            governance,
            workspaces.clone(),
            browser.clone(),
            PresentationReactor::new(Arc::new(NoPresentation)),
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
    fn unsatisfied_wait_is_decisive_before_the_invocation_deadline() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_open_page",
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
            "browser_open_page",
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
            "browser_read_page",
            json!({"tab":handle}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(read.status, Status::Succeeded);
        browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
        let closed = executor.execute(
            &workspace,
            "browser_close_tab",
            json!({"tab":handle}),
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
    fn tab_close_policy_blocks_before_browser_dispatch() {
        let policy = temporary_policy("tab-close");
        fs::write(&policy, br#"{"version":1,"allow_tab_close":false}"#).unwrap();
        let (executor, browser, _, workspace, _) =
            fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_open_page",
            json!({"url":"https://example.com"}),
            None,
            &CancellationToken::default(),
        );
        let handle = opened.facts["tab"].as_str().unwrap();
        let closed = executor.execute(
            &workspace,
            "browser_close_tab",
            json!({"tab":handle}),
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
    fn local_preserve_tabs_refusal_is_blocked_without_an_effect() {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_open_page",
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
            "browser_close_tab",
            json!({"tab":handle}),
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
            "browser_open_page",
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
            "browser_open_page",
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
            "browser_inspect_page",
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
        }));
        browser.push(Ok(BrowserOutcome::Observed {
            tab_id: 7,
            satisfied: true,
            elapsed_ms: 5,
            readiness: BrowserReadiness::Complete,
        }));
        let sequence = executor.execute(&workspace, "browser_run_sequence", json!({"tab":tab_handle,"steps":[{"action":"click","target":target},{"action":"wait","condition":"load_ready"}]}), None, &CancellationToken::default());
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
            "browser_open_page",
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
            "browser_inspect_page",
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
            "browser_open_page",
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
        fs::write(&policy, br#"{"version":1,"allow_tab_close":false}"#).unwrap();
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
            "browser_open_page",
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
            "browser_open_page",
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
            "browser_open_page",
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
            "browser_inspect_page",
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
            "browser_navigate_page",
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
            "browser_open_page",
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
            "browser_take_screenshot",
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
            "browser_navigate_page",
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
