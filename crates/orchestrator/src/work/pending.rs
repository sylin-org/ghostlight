//! Confirming the tab bindings a workspace remembers across a gap in what it knows of a browser.
//!
//! A physical tab id names one tab only within one browser run. A binding kept across a browser
//! disconnect, a replaced connection or a service restart is pending until its browser lists a tab
//! with its id at the URL the workspace last knew (ADR-0203 Decision 1). The first call of a
//! workspace that holds pending bindings asks each connected browser for its tabs once, before it
//! selects a tab or targets a browser, so no call ever reaches a tab the browser has not confirmed.

use serde_json::json;

use crate::browser::adapter::{BrowserCommand, BrowserOutcome};
use crate::browser::BrowserError;
use crate::governance::Decision;
use crate::workspace::{TabHandle, WorkspaceLease};

use super::{ApplicationExecutor, InvocationContext, Terminal};

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
            if let Err(error) = self.confirm_in(context, lease, browser) {
                unconfirmed.push((browser.clone(), error));
            }
        }
        let away = lease.pending_browser_of(context.requested_tab, context.requested_target)?;
        let waited = match unconfirmed
            .into_iter()
            .find(|(browser, _)| *browser == away)
        {
            Some((_, error)) => Err(error),
            None => self
                .admit_dispatch(context)
                .and_then(|()| self.await_pinned_browser(context, &away))
                .and_then(|browser| self.confirm_in(context, lease, &browser)),
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

    /// Ask `browser` for its tabs once and confirm or forget this workspace's pending bindings
    /// there.
    fn confirm_in(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
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
                let _ = lease.confirm(browser, &tabs);
                Ok(())
            }
            Ok(_) => Err(BrowserError::Protocol(
                "list_tabs answered another outcome".into(),
            )),
            Err(error) => Err(listing_failure(error)),
        }
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
