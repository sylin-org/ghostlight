//! Sequence, dialog, and diagnostic execution.

use serde_json::json;

use ghostlight_bridge::browser::{
    BrowserCommand, BrowserOutcome, DiagnosticDetail, DiagnosticEntry, DiagnosticSource,
};

use crate::events::DomainEvent;
use crate::governance::Capability;
use crate::language::history::{CompositionKind, StepReceipt};
use crate::language::outcome::{Outcome, Refusal};
use crate::language::{
    Click, Diagnose, FillForm, FormField, HandleDialog, Hover, Operation, PressKey, RunSequence,
    ScrollPage, SequenceStep, TypeText, Wait,
};
use crate::workspace::WorkspaceLease;

use super::composition::{terminal_row, unexecuted_row, Composition, UnexecutedStatus};
use super::result::{Effect, Status};
use super::{
    observed_host, permitted, readiness, step_activity, ApplicationExecutor, InvocationContext,
    Terminal,
};

impl ApplicationExecutor {
    pub(super) fn sequence(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &RunSequence,
    ) -> Terminal {
        let selected = match lease.select_tab(value.tab.as_deref()) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let total = value.steps.len();
        let mut progress = Composition::new(
            total,
            permitted(),
            readiness(selected.readiness),
            Some(selected.physical_id),
        );
        let mut statuses = Vec::with_capacity(total);
        let tools: Vec<_> = value.steps.iter().map(sequence_tool).collect();
        for (index, step) in value.steps.iter().enumerate() {
            if progress.stop_at_boundary(context, index + 1) {
                break;
            }
            self.emit(DomainEvent::WorkPhaseStarted {
                invocation: context.invocation.into(),
                workspace: context.workspace.as_str().into(),
                physical_id: Some(selected.physical_id),
                activity: step_activity(step),
            });
            let operation = match step {
                SequenceStep::Click {
                    target,
                    button,
                    click_count,
                } => Operation::Click(Click {
                    target: Some(target.clone()),
                    selector: None,
                    view: None,
                    x: None,
                    y: None,
                    tab: Some(selected.handle.as_str().into()),
                    button: button.clone(),
                    click_count: *click_count,
                    expect: None,
                    modifiers: Vec::new(),
                    timeout_ms: value.timeout_ms,
                    restrictions: value.restrictions.clone(),
                }),
                SequenceStep::TypeText {
                    target,
                    text,
                    clear_first,
                } => Operation::TypeText(TypeText {
                    target: target.clone(),
                    focused: false,
                    selector: None,
                    text: text.clone(),
                    tab: Some(selected.handle.as_str().into()),
                    clear_first: *clear_first,
                    expect: None,
                    timeout_ms: value.timeout_ms,
                    restrictions: value.restrictions.clone(),
                }),
                SequenceStep::Fill {
                    target,
                    value: field_value,
                } => Operation::FillForm(FillForm {
                    fields: vec![FormField {
                        target: Some(target.clone()),
                        selector: None,
                        value: crate::language::FormFieldValue::Text(field_value.clone()),
                    }],
                    tab: Some(selected.handle.as_str().into()),
                    submit_target: None,
                    expect: None,
                    timeout_ms: value.timeout_ms,
                    restrictions: value.restrictions.clone(),
                }),
                SequenceStep::PressKey {
                    key,
                    target,
                    modifiers,
                } => Operation::PressKey(PressKey {
                    key: key.clone(),
                    strokes: Vec::new(),
                    repeat: 1,
                    tab: Some(selected.handle.as_str().into()),
                    target: target.clone(),
                    modifiers: modifiers.clone(),
                    expect: None,
                    restrictions: value.restrictions.clone(),
                }),
                SequenceStep::Scroll {
                    target,
                    direction,
                    amount,
                } => Operation::ScrollPage(ScrollPage {
                    tab: Some(selected.handle.as_str().into()),
                    target: target.clone(),
                    direction: direction.clone(),
                    amount: amount.clone(),
                    view: None,
                    x: None,
                    y: None,
                    ticks: None,
                    timeout_ms: value.timeout_ms,
                    restrictions: value.restrictions.clone(),
                }),
                SequenceStep::Hover { target } => Operation::Hover(Hover {
                    target: Some(target.clone()),
                    view: None,
                    x: None,
                    y: None,
                    tab: Some(selected.handle.as_str().into()),
                    timeout_ms: value.timeout_ms,
                    restrictions: value.restrictions.clone(),
                }),
                SequenceStep::Wait {
                    condition,
                    value: condition_value,
                    target,
                } => Operation::Wait(Wait {
                    condition: condition.clone(),
                    tab: Some(selected.handle.as_str().into()),
                    value: condition_value.clone(),
                    target: target.clone(),
                    selector: None,
                    timeout_ms: value.timeout_ms,
                    restrictions: value.restrictions.clone(),
                }),
            };
            let terminal = self.run_child(
                context,
                lease,
                &operation,
                StepReceipt {
                    parent: CompositionKind::Sequence,
                    position: index + 1,
                    total,
                    preparation_failed: false,
                },
            );
            let cause = progress.record(index + 1, &terminal);
            statuses.push(terminal_row(index + 1, &terminal, cause));
            if terminal.result.status != Status::Succeeded {
                progress.progress.stopped = true;
                break;
            }
        }
        for index in statuses.len()..total {
            statuses.push(unexecuted_row(index + 1, UnexecutedStatus::NotRun));
        }
        let facts = json!({"tab":selected.handle.as_str(),"completed_steps":progress.progress.counts.succeeded,
            "total_steps":total,"steps":statuses});
        let mut terminal = progress.finish(context, facts);
        terminal.audit = terminal.audit.with_tools(tools);
        terminal
    }

    pub(super) fn handle_dialog(
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
        let accept = value.action != "dismiss";
        let text = (value.action == "respond")
            .then(|| value.text.clone())
            .flatten();
        match self.dispatch(context, BrowserCommand::HandleDialog { tab_id: selected.physical_id, accept, text }) {
            Ok(BrowserOutcome::DialogHandled { tab_id, dialog_type: handled_type, accepted }) if tab_id == selected.physical_id => self.succeeded(context, decision, Some(tab_id), Effect::Applied, readiness(selected.readiness), false, Outcome::DialogHandled { accepted }, json!({"tab":selected.handle.as_str(),"dialog_type":if handled_type.is_empty(){observed.1}else{handled_type},"accepted":accepted,"handled":true})),
            Ok(BrowserOutcome::DialogAbsent { tab_id }) if tab_id == selected.physical_id => self.failed(context, decision, Some(tab_id), Refusal::NoDialogVisible, json!({"tab":selected.handle.as_str(),"handled":false})),
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => self.browser_failure(context, decision, error, Some(selected.physical_id)),
        }
    }

    pub(super) fn diagnose(
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
}

fn sequence_tool(step: &SequenceStep) -> &'static str {
    match step {
        SequenceStep::Click { .. } => "browser_click",
        SequenceStep::TypeText { .. } => "browser_type_text",
        SequenceStep::Fill { .. } => "browser_fill_form",
        SequenceStep::PressKey { .. } => "browser_press_key",
        SequenceStep::Scroll { .. } => "browser_scroll",
        SequenceStep::Hover { .. } => "browser_hover",
        SequenceStep::Wait { .. } => "browser_wait",
    }
}
