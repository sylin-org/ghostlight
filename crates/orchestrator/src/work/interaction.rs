//! Browser dialog and diagnostic execution.

use serde_json::json;

use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, DiagnosticDetail, DiagnosticEntry, DiagnosticSource,
};

use crate::governance::Capability;
use crate::language::outcome::{DialogKind, Outcome, PromptField, Refusal};
use crate::language::{Diagnose, HandleDialog};
use crate::workspace::WorkspaceLease;

use super::result::Effect;
use super::{observed_host, readiness, ApplicationExecutor, InvocationContext, Terminal};
use crate::browser::adapter::PhysicalDiagnosticsRead;

impl ApplicationExecutor {
    /// Keep the workspace's record of a tab's dialog in line with what the browser just said,
    /// whether or not the matching event arrived.
    fn note_dialog(
        &self,
        context: &InvocationContext<'_>,
        physical_id: u64,
        dialog: Option<DialogKind>,
    ) {
        if let Some(browser) = self.workspaces.browser_of(context.workspace.as_str()) {
            self.workspaces
                .apply_browser_dialog(&browser, physical_id, dialog);
        }
    }

    /// Report or answer the dialog showing on a tab.
    ///
    /// Status asks the browser what is showing. Every other action sends one handling command,
    /// whose own answer says whether a dialog was there; the browser step looks first, so no
    /// separate look is needed here.
    pub(super) fn handle_dialog(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &HandleDialog,
    ) -> Terminal {
        self.with_authorized_tab(
            context,
            lease,
            value.tab.as_deref(),
            |selected, decision| {
                if value.action == "status" {
                    return match self.dispatch(
                        context,
                        BrowserCommand::InspectDialog {
                            tab_id: selected.physical_id,
                        },
                    ) {
                        Ok(BrowserOutcome::Dialog {
                            tab_id,
                            present,
                            dialog_type,
                            message,
                            default_prompt,
                            ..
                        }) if tab_id == selected.physical_id => {
                            let kind = present.then(|| DialogKind::classify(&dialog_type));
                            self.note_dialog(context, tab_id, kind);
                            let prompt = (kind == Some(DialogKind::Prompt)).then(|| {
                                default_prompt.map_or(PromptField::Unreported, PromptField::Holds)
                            });
                            self.succeeded(
                                context,
                                decision,
                                Some(tab_id),
                                Effect::None,
                                readiness(selected.readiness),
                                true,
                                Outcome::DialogObserved {
                                    present,
                                    message,
                                    prompt,
                                },
                                json!({"tab":selected.handle.as_str(),"present":present,"dialog_type":dialog_type}),
                            )
                        }
                        Ok(_) => {
                            self.protocol_failure(context, decision, Some(selected.physical_id))
                        }
                        Err(error) => self.browser_failure(
                            context,
                            decision,
                            error,
                            Some(selected.physical_id),
                        ),
                    };
                }
                let accept = value.action != "dismiss";
                let text = (value.action == "respond")
                    .then(|| value.text.clone())
                    .flatten();
                match self.dispatch(
                    context,
                    BrowserCommand::HandleDialog {
                        tab_id: selected.physical_id,
                        accept,
                        text,
                    },
                ) {
                    Ok(BrowserOutcome::DialogHandled {
                        tab_id,
                        dialog_type,
                        accepted,
                        ..
                    }) if tab_id == selected.physical_id => {
                        self.note_dialog(context, tab_id, None);
                        self.succeeded(
                            context,
                            decision,
                            Some(tab_id),
                            Effect::Applied,
                            readiness(selected.readiness),
                            false,
                            Outcome::DialogHandled { accepted },
                            json!({"tab":selected.handle.as_str(),"dialog_type":dialog_type,"accepted":accepted,"handled":true}),
                        )
                    }
                    Ok(BrowserOutcome::DialogAbsent { tab_id })
                        if tab_id == selected.physical_id =>
                    {
                        self.note_dialog(context, tab_id, None);
                        self.failed(
                            context,
                            decision,
                            Some(tab_id),
                            Refusal::NoDialogVisible,
                            json!({"tab":selected.handle.as_str(),"handled":false}),
                        )
                    }
                    Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
                    Err(error) => {
                        self.browser_failure(context, decision, error, Some(selected.physical_id))
                    }
                }
            },
        )
    }

    pub(super) fn diagnose(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &Diagnose,
    ) -> Terminal {
        self.with_authorized_tab(
            context,
            lease,
            value.tab.as_deref(),
            |selected, decision| {
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
                    BrowserCommand::ReadDiagnostics(PhysicalDiagnosticsRead {
                        tab_id: selected.physical_id,
                        source,
                        detail,
                        match_text: value.r#match.clone(),
                        after: value.after.clone(),
                        limit: u16::try_from(value.limit).expect("validated diagnostic limit"),
                    }),
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
            },
        )
    }
}
