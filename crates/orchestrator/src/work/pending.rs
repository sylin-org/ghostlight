//! Confirming the tab bindings a workspace remembers across a gap in what it knows of a browser.
//!
//! A physical tab id names one tab only within one browser run. A binding kept across a browser
//! disconnect, a replaced connection or a service restart is pending until its browser lists a tab
//! with its id at the URL the workspace last knew (ADR-0203 Decision 1). The first call of a
//! workspace that holds pending bindings asks each connected browser for its tabs once, before it
//! selects a tab or targets a browser. A gap can also open while a call runs, between two of its
//! crossings, so every command addressed to one tab checks that tab's binding again before it
//! crosses. No command reaches a tab the browser has not confirmed.

use serde_json::json;

use crate::browser::adapter::{BrowserCommand, BrowserOutcome};
use crate::browser::BrowserError;
use crate::governance::Decision;
use crate::language::Operation;
use crate::workspace::{BindingState, TabHandle, TabRequest, WorkspaceLease};

use super::{operation_tab, operation_target, ApplicationExecutor, InvocationContext, Terminal};

/// The result fact that names the tab handles a call forgot because its workspace bound another
/// browser.
const FORGOTTEN_TABS_FACT: &str = "forgotten_tabs";

impl ApplicationExecutor {
    /// Confirm or forget this workspace's pending bindings before the call selects a tab.
    ///
    /// Every connected browser that holds one is asked for its tabs once. A call that selects a
    /// pending tab of a browser not connected then waits for that browser as a pinned session
    /// waits, and forgets the binding when the wait ends without it, so the handle reads as no
    /// longer open. Returns the call's answer only when it cannot go on: Ghostlight is paused, the
    /// tab's browser is silent, or the call ran out of time or was cancelled while it waited.
    pub(super) fn confirm_pending(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        operation: &Operation,
    ) -> Option<Terminal> {
        let pending = lease.pending_browsers();
        if pending.is_empty() {
            return None;
        }
        let connected = self.browser.browsers();
        let mut unconfirmed = Vec::new();
        for browser in pending
            .iter()
            .filter(|browser| connected.iter().any(|summary| &summary.id == *browser))
        {
            if let Err(error) = self.confirm_in(context, browser) {
                unconfirmed.push((browser.clone(), error));
            }
        }
        let away = lease.pending_browser_of(&tab_request(operation)?)?;
        let waited = match unconfirmed
            .into_iter()
            .find(|(browser, _)| *browser == away)
        {
            Some((_, error)) => Err(error),
            None => self
                .admit_dispatch(context)
                .and_then(|()| self.await_pinned_browser(context, &away))
                .and_then(|browser| self.confirm_in(context, &browser)),
        };
        match waited {
            Ok(()) => None,
            Err(error) if stayed_away(&error) => {
                let _ = lease.forget_pending(&away);
                None
            }
            Err(error) => Some(self.browser_failure(context, Decision::permitted(), error, None)),
        }
    }

    /// Check, before a command addressed to `tab` crosses to `browser`, that the tab is still one
    /// this workspace controls there.
    ///
    /// A binding that went pending while the call ran is confirmed first, with one listing of
    /// `browser`'s tabs, as a call's first crossing confirms it. A binding the browser does not
    /// confirm, or a tab the workspace no longer holds, answers [`BrowserError::StaleTab`], and
    /// nothing is sent.
    pub(super) fn confirm_crossing(
        &self,
        context: &InvocationContext<'_>,
        browser: &str,
        tab: Option<u64>,
    ) -> Result<(), BrowserError> {
        let Some(tab) = tab else {
            return Ok(());
        };
        if self.binding_state(context, browser, tab) == BindingState::Pending {
            self.confirm_in(context, browser)?;
        }
        self.admit_tab(context, browser, tab)
    }

    /// Admit a command addressed to `tab` in `browser` at the moment it is written: only a tab
    /// this workspace controls there is admitted.
    ///
    /// A binding that went pending since the call checked it means a new connection replaced the
    /// one the call meant to use, so the command is refused as never sent and the binding waits
    /// for the next call to confirm it.
    pub(super) fn admit_tab(
        &self,
        context: &InvocationContext<'_>,
        browser: &str,
        tab: u64,
    ) -> Result<(), BrowserError> {
        match self.binding_state(context, browser, tab) {
            BindingState::Confirmed => Ok(()),
            BindingState::Pending => Err(BrowserError::DisconnectedBeforeDispatch),
            BindingState::Unbound => Err(BrowserError::StaleTab),
        }
    }

    /// How this call's workspace holds `browser`'s tab `tab` now.
    fn binding_state(
        &self,
        context: &InvocationContext<'_>,
        browser: &str,
        tab: u64,
    ) -> BindingState {
        self.workspaces
            .binding_state(context.workspace.as_str(), browser, tab)
    }

    /// Ask `browser` for its tabs once and confirm or forget this workspace's pending bindings
    /// there.
    fn confirm_in(
        &self,
        context: &InvocationContext<'_>,
        browser: &str,
    ) -> Result<(), BrowserError> {
        match self.send(
            context,
            browser,
            BrowserCommand::ListTabs,
            context.deadline,
            context.cancellation.flag(),
        ) {
            Ok(BrowserOutcome::Tabs { tabs }) => {
                // A workspace that went away meanwhile has nothing left to confirm.
                let _ = self.workspaces.confirm(context.workspace, browser, &tabs);
                Ok(())
            }
            Ok(_) => Err(BrowserError::Protocol(
                "list_tabs answered another outcome".into(),
            )),
            Err(error) => Err(listing_failure(error)),
        }
    }
}

/// The handles `operation` names that choose its tab, or none when the operation selects no tab
/// at all.
///
/// A listing never waits for a browser (ADR-0203 Decision 2), an open makes a fresh tab, a flow's
/// steps each select their own, and an explanation reaches no browser.
fn tab_request(operation: &Operation) -> Option<TabRequest<'_>> {
    let targets: Vec<Option<&str>> = match operation {
        Operation::ListTabs(_)
        | Operation::OpenPage(_)
        | Operation::RunFlow(_)
        | Operation::ExplainPolicy(_) => return None,
        Operation::FillForm(value) => value
            .fields
            .iter()
            .map(|field| field.target.as_deref())
            .chain([value.submit_target.as_deref()])
            .collect(),
        Operation::Drag(value) => vec![
            value.source_target.as_deref(),
            value.destination_target.as_deref(),
        ],
        _ => vec![operation_target(operation)],
    };
    Some(TabRequest {
        tab: operation_tab(operation),
        targets: targets.into_iter().flatten().collect(),
        view: operation_view(operation),
    })
}

/// The view handle `operation` names, whose points it acts on.
fn operation_view(operation: &Operation) -> Option<&str> {
    match operation {
        Operation::Click(value) => value.view.as_deref(),
        Operation::Hover(value) => value.view.as_deref(),
        Operation::ScrollPage(value) => value.view.as_deref(),
        Operation::TakeScreenshot(value) => value.view.as_deref(),
        Operation::Drag(value) => value.view.as_deref(),
        Operation::UploadFiles(value) => value.view.as_deref(),
        _ => None,
    }
}

/// Say in the call's result which pending tabs of another browser the workspace forgot when this
/// call bound its browser, so the agent knows those handles are gone.
pub(super) fn report_forgotten(lease: &WorkspaceLease, terminal: &mut Terminal) {
    let forgotten = lease.take_forgotten();
    if !forgotten.is_empty() {
        terminal.result.facts[FORGOTTEN_TABS_FACT] =
            json!(forgotten.iter().map(TabHandle::as_str).collect::<Vec<_>>());
    }
}

/// Whether the wait for a pending tab's browser ended without that browser, so the binding is
/// forgotten rather than kept for a later call.
const fn stayed_away(error: &BrowserError) -> bool {
    matches!(
        error,
        BrowserError::DisconnectedBeforeDispatch
            | BrowserError::WrongProfile
            | BrowserError::RecoveryManual { .. }
            | BrowserError::RecoveryFailed { .. }
    )
}

/// What a failed tab listing means for the call that asked. A listing changes nothing in the
/// browser, so a failure after its dispatch is still a failure before any effect of the call.
fn listing_failure(error: BrowserError) -> BrowserError {
    match error {
        BrowserError::DisconnectedAfterDispatch => BrowserError::DisconnectedBeforeDispatch,
        BrowserError::CancelledAfterDispatch => BrowserError::CancelledBeforeDispatch,
        BrowserError::DeadlineAfterDispatch => BrowserError::DeadlineBeforeDispatch,
        error if error.effect_unknown() => BrowserError::NotAnsweringBeforeDispatch,
        error => error,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::tab_request;
    use crate::language::decode;

    #[test]
    fn a_fill_and_a_drag_select_their_tab_by_every_target_they_name() {
        let fill = decode(
            "browser_fill_form",
            json!({"fields":[
                {"target":"target_name","value":"Ada"},
                {"selector":{"name":"Email"},"value":"ada@example.com"}
            ],"submit_target":"target_send"}),
        )
        .unwrap();
        let request = tab_request(&fill).unwrap();
        assert_eq!(request.tab, None);
        assert_eq!(request.targets, vec!["target_name", "target_send"]);

        let drag = decode(
            "browser_drag",
            json!({"source_target":"target_card","destination_target":"target_lane"}),
        )
        .unwrap();
        assert_eq!(
            tab_request(&drag).unwrap().targets,
            vec!["target_card", "target_lane"]
        );
    }

    #[test]
    fn a_listing_and_an_open_select_no_tab() {
        for (tool, input) in [
            ("browser_tabs", json!({"action":"list"})),
            (
                "browser_navigate",
                json!({"url":"https://example.com/","new_tab":true}),
            ),
        ] {
            let operation = decode(tool, input).unwrap();
            assert!(tab_request(&operation).is_none(), "{tool}");
        }
        let read = decode("browser_read", json!({})).unwrap();
        let request = tab_request(&read).unwrap();
        assert!(request.tab.is_none() && request.targets.is_empty() && request.view.is_none());
    }
}
