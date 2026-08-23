//! Sequence, dialog, and diagnostic execution.

use serde_json::json;

use ghostlight_bridge::browser::{
    BrowserCommand, BrowserOutcome, DiagnosticDetail, DiagnosticEntry, DiagnosticSource,
};

use crate::events::DomainEvent;
use crate::governance::Capability;
use crate::language::outcome::{Outcome, Refusal};
use crate::language::{
    Click, Diagnose, FillForm, FormField, HandleDialog, Hover, PressKey, RunSequence, ScrollPage,
    SequenceStep, TypeText, Wait,
};
use crate::workspace::WorkspaceLease;

use super::result::{Effect, InvocationResult, Status};
use super::{
    observed_host, permitted, readiness, status_name, step_activity, ApplicationExecutor,
    InvocationContext, Terminal,
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
                        modifiers: Vec::new(),
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
                        focused: false,
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
                        strokes: Vec::new(),
                        repeat: 1,
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
                        view: None,
                        x: None,
                        y: None,
                        ticks: None,
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
