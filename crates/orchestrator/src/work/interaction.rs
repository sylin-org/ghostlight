//! Browser dialog and diagnostic execution.

use serde_json::json;

use ghostlight_bridge::browser::{
    BrowserCommand, BrowserOutcome, DiagnosticDetail, DiagnosticEntry, DiagnosticSource,
};

use crate::governance::Capability;
use crate::language::outcome::{Outcome, Refusal};
use crate::language::{Diagnose, HandleDialog};
use crate::workspace::WorkspaceLease;

use super::result::Effect;
use super::{observed_host, readiness, ApplicationExecutor, InvocationContext, Terminal};

impl ApplicationExecutor {
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
