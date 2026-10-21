//! How a call that did not complete reads: a refusal, a stale target, a page's own refusal, a
//! protocol fault, a browser failure with its true effect, or a workspace failure.

use super::*;

impl ApplicationExecutor {
    /// Report a denial the caller can act on, naming the host when the work named an address.
    ///
    /// `blocked_url` is the address the refused work asked for or landed on. Only its host is
    /// kept for the audit; its origin is kept for the one refusal whose sentence names it, and
    /// that sentence reaches the caller only: the audit names the refusal without the address.
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per field the denial result states, plus the address the refusal names"
    )]
    pub(super) fn blocked_at(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        effect: Effect,
        repeat_safe: bool,
        facts: Value,
        blocked_url: Option<&str>,
    ) -> Terminal {
        let refusal = Refusal::AuthorityBlocked {
            code: decision.reason,
            host: blocked_url.and_then(observed_host),
            origin: blocked_url
                .filter(|_| decision.reason == ReasonCode::OwnWorkbench)
                .and_then(observed_origin),
        };
        Terminal::refused(
            context.invocation,
            Shape {
                status: Status::Blocked,
                effect,
                readiness: Readiness::Unknown,
                repeat_safe,
            },
            &refusal,
            with_denial_facts(decision, facts),
            decision,
            physical_id,
        )
    }

    /// Report a denial with no host in play.
    pub(super) fn blocked(
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

    pub(super) fn failed(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        refusal: Refusal,
        facts: Value,
    ) -> Terminal {
        Terminal::refused(
            context.invocation,
            Shape {
                status: Status::Failed,
                effect: Effect::None,
                readiness: Readiness::Unknown,
                repeat_safe: true,
            },
            &refusal,
            facts,
            decision,
            physical_id,
        )
    }

    pub(super) fn unknown(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        refusal: Refusal,
        facts: Value,
    ) -> Terminal {
        Terminal::refused(
            context.invocation,
            Shape {
                status: Status::Unknown,
                effect: Effect::Unknown,
                readiness: Readiness::Unknown,
                repeat_safe: false,
            },
            &refusal,
            facts,
            decision,
            physical_id,
        )
    }

    /// The page found a handle's control gone or unknown, which a stale handle says the same way
    /// whoever noticed. A control the page reported removed is remembered, so the handle refuses
    /// at once from now on.
    pub(super) fn stale_locator_failure(
        &self,
        context: &InvocationContext<'_>,
        reason: StaleReason,
        lookup: Option<&StaleLookup>,
    ) -> Terminal {
        if let Some(handle) = context.requested_target {
            if reason == StaleReason::Removed {
                self.workspaces
                    .mark_target_removed(context.workspace, handle);
            }
            self.recover_stale(context, handle, reason, lookup);
        }
        self.workspace_failure(context, WorkspaceError::StaleTarget { reason })
    }

    /// The page refused the call's control before acting on it. Nothing happened, so the call
    /// is safe to repeat, and the refusal names its gesture and one step that would work.
    pub(super) fn page_refused(
        &self,
        context: &InvocationContext<'_>,
        decision: Decision,
        physical_id: Option<u64>,
        refusal: PageRefusal,
    ) -> Terminal {
        Terminal::refused(
            context.invocation,
            Shape {
                status: Status::Failed,
                effect: Effect::None,
                readiness: Readiness::NotApplicable,
                repeat_safe: true,
            },
            &Refusal::PageRefused {
                refusal,
                gesture: context.gesture,
            },
            json!({}),
            decision,
            physical_id,
        )
    }

    pub(super) fn protocol_failure(
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
            json!({}),
        )
    }

    pub(super) fn browser_failure(
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
                json!({}),
            );
        }
        if error == BrowserError::StaleView {
            return self.workspace_failure(context, WorkspaceError::StaleView);
        }
        if error == BrowserError::StaleTab {
            return self.workspace_failure(context, WorkspaceError::StaleTab);
        }
        if error == BrowserError::DialogOpen {
            return self.failed(
                context,
                decision,
                physical_id,
                Refusal::DialogOpen,
                json!({}),
            );
        }
        if error == BrowserError::TabNotShowing {
            return self.failed(
                context,
                decision,
                physical_id,
                Refusal::TabNotShowing,
                json!({}),
            );
        }
        if error == BrowserError::DocumentUnavailable {
            return self.failed(
                context,
                decision,
                physical_id,
                Refusal::DocumentUnavailable,
                json!({}),
            );
        }
        if let BrowserError::RuntimeControl(reason) = error {
            return self.blocked(
                context,
                Decision::refused(reason),
                physical_id,
                Effect::None,
                false,
                json!({}),
            );
        }
        if matches!(&error, BrowserError::LocalInterlock(_)) {
            return Terminal::refused(
                context.invocation,
                Shape {
                    status: Status::Blocked,
                    effect: Effect::None,
                    readiness: Readiness::NotApplicable,
                    repeat_safe: true,
                },
                &Refusal::LocalInterlock,
                json!({}),
                decision,
                physical_id,
            );
        }
        // Routing refusals are decisive and physical-effect-free: nothing was dispatched, because
        // nothing could be dispatched anywhere in particular. They name the browsers the caller
        // can choose between rather than picking one on the caller's behalf.
        if let BrowserError::StaleLocator { reason, lookup } = &error {
            return self.stale_locator_failure(context, *reason, lookup.as_ref());
        }
        if let BrowserError::PageRefused(refusal) = error {
            return self.page_refused(context, decision, physical_id, refusal);
        }
        if let Some((refusal, facts)) = routing_refusal(&error, || self.extension_advice()) {
            let mut terminal = Terminal::refused(
                context.invocation,
                Shape {
                    status: Status::Failed,
                    effect: Effect::None,
                    readiness: Readiness::NotApplicable,
                    repeat_safe: true,
                },
                &refusal,
                facts,
                decision,
                physical_id,
            );
            terminal.audit = terminal.audit.for_gesture(context.gesture);
            return terminal;
        }
        if error.effect_unknown() {
            // Every after-dispatch class is an honest unknown: name its phase so the caller
            // can tell a silent adapter from a spent deadline, and give a spent deadline its own
            // sentence, distinct from a disconnection.
            let (refusal, facts) = match &error {
                BrowserError::EffectUnknown(detail) => (
                    Refusal::EffectUnknown,
                    json!({"detail":detail,"phase":"adapter_reported"}),
                ),
                BrowserError::ScriptThrew(exception) => {
                    let mut facts = json!({"phase":"adapter_reported"});
                    facts[EXCEPTION_FACT] = Value::String(exception.clone());
                    (
                        Refusal::ScriptThrew {
                            exception: exception.clone(),
                        },
                        facts,
                    )
                }
                BrowserError::DeadlineAfterDispatch => (
                    Refusal::DeadlineExpired {
                        before_dispatch: false,
                        timeout_adjustable: context.deadline_adjustable,
                    },
                    json!({"phase":"after_dispatch"}),
                ),
                BrowserError::DisconnectedAfterDispatch => {
                    (Refusal::ConnectionLost, json!({"phase":"after_dispatch"}))
                }
                BrowserError::CancelledAfterDispatch => (
                    Refusal::CancelledAfterDispatch {
                        by: context.cancellation.cause(),
                    },
                    json!({"phase":"after_dispatch"}),
                ),
                _ => (Refusal::EffectUnknown, json!({"phase":"unknown"})),
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
                    timeout_adjustable: context.deadline_adjustable,
                },
                json!({"phase":"before_dispatch"}),
            );
        }
        if matches!(error, BrowserError::Unsupported { .. }) {
            return self.failed(
                context,
                decision,
                physical_id,
                Refusal::BrowserAdapterOutdated,
                json!({}),
            );
        }
        // Each case says which fact stopped the call. Every availability reason and the
        // cancellation stop before dispatch, so they say nothing was sent. A cancellation the
        // caller asked for is not a browser problem and never says it was. A contract failure is
        // not a disconnection, and it makes no dispatch claim: a receipt in the wrong shape
        // arrives after the browser already received the request.
        let unavailable = |reason| (Status::Failed, Refusal::BrowserUnavailable { reason });
        let (status, refusal) = match &error {
            BrowserError::CancelledBeforeDispatch => (
                Status::Cancelled,
                Refusal::CancelledBeforeDispatch {
                    by: context.cancellation.cause(),
                },
            ),
            BrowserError::DisconnectedBeforeDispatch => {
                unavailable(BrowserUnavailableReason::NotConnected)
            }
            BrowserError::NotAnsweringBeforeDispatch => {
                unavailable(BrowserUnavailableReason::NotAnswering)
            }
            BrowserError::WrongProfile => unavailable(BrowserUnavailableReason::WrongProfile),
            BrowserError::BusyBeforeDispatch => unavailable(BrowserUnavailableReason::Busy),
            // Protocol, authentication, and incompatible-major failures, including a receipt
            // that arrived in the wrong shape after dispatch. Every other class returned above.
            _ => (Status::Failed, Refusal::BrowserContract),
        };
        Terminal::refused(
            context.invocation,
            Shape {
                status,
                effect: Effect::None,
                readiness: Readiness::Unknown,
                repeat_safe: true,
            },
            &refusal,
            json!({}),
            decision,
            physical_id,
        )
    }

    pub(super) fn workspace_failure(
        &self,
        context: &InvocationContext<'_>,
        error: WorkspaceError,
    ) -> Terminal {
        self.workspace_refusal(context, error, None)
    }

    /// A workspace failure discovered after the browser already acted on `physical_id`.
    ///
    /// The tab was opened, navigated, focused, or closed before the aggregate refused to record
    /// it, so the result says the effect was applied and is not safe to repeat.
    pub(super) fn workspace_failure_after_effect(
        &self,
        context: &InvocationContext<'_>,
        error: WorkspaceError,
        physical_id: u64,
    ) -> Terminal {
        self.workspace_refusal(context, error, Some(physical_id))
    }

    pub(super) fn workspace_refusal(
        &self,
        context: &InvocationContext<'_>,
        error: WorkspaceError,
        acted_on: Option<u64>,
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
        let offer = context.findings.take_offer();
        // A held tab repeats the refusal of the page that held it, never a pause.
        let held = match &error {
            WorkspaceError::Held(hold) => Some(self.held_decision(context, hold)),
            _ => None,
        };
        let mut refusal = language::outcome::workspace_refusal(error);
        let mut facts = json!({});
        if let Some(denial) = held {
            facts = with_denial_facts(denial, facts);
        }
        if let Refusal::WorkspaceUnusable { reason } = &mut refusal {
            match reason {
                WorkspaceReason::OwnershipMismatch { owner: named } => {
                    if let Some(owner) = &owner {
                        facts[OWNER_FACT] = Value::String(owner.clone());
                    }
                    *named = owner;
                }
                WorkspaceReason::StaleTarget { reason, help } => {
                    *help = stale_help(context, *reason, offer.as_ref(), acted_on);
                    facts[STALE_FACT] = Value::String(reason.word().into());
                }
                WorkspaceReason::AmbiguousTab => {
                    facts[CANDIDATE_TABS_FACT] = self.candidate_tabs(context);
                }
                WorkspaceReason::HandleOnOtherTab(Some(tab)) => {
                    facts[HANDLE_TAB_FACT] = Value::String(tab.clone());
                }
                WorkspaceReason::OutsideImage(Some(size)) => {
                    facts[IMAGE_SIZE_FACT] = json!({"width":size.width,"height":size.height});
                }
                _ => {}
            }
        }
        if let Some(offer) = offer {
            facts[RECOVERY_CANDIDATES_FACT] = Value::Array(offer.candidates);
            if let Refusal::WorkspaceUnusable {
                reason:
                    WorkspaceReason::StaleTarget {
                        help: StaleHelp::Replacement { target, .. },
                        ..
                    },
            } = &refusal
            {
                facts[REPLACEMENT_FACT] = Value::String(target.clone());
            }
        }
        let status = if held.is_some() {
            Status::Blocked
        } else {
            Status::Failed
        };
        let effect = if acted_on.is_some() {
            Effect::Applied
        } else {
            Effect::None
        };
        Terminal::refused(
            context.invocation,
            Shape {
                status,
                effect,
                readiness: Readiness::Unknown,
                repeat_safe: status == Status::Failed && effect == Effect::None,
            },
            &refusal,
            facts,
            held.unwrap_or_else(Decision::permitted),
            acted_on,
        )
    }
}

/// Translate a governance reason into the language's own denial vocabulary.
/// Add the policy explanation a refusing decision carries to a result's facts.
pub(super) fn with_denial_facts(decision: Decision, mut facts: Value) -> Value {
    if let Value::Object(object) = &mut facts {
        if let Some(denial_id) = decision.denial_id() {
            object.insert("denial_id".into(), Value::String(denial_id));
        }
        if let Some(rule) = decision.policy_rule() {
            object.insert("policy_rule".into(), Value::String(rule.into()));
        }
    }
    facts
}

/// Why a postcondition check that failed in the browser did not confirm its applied action,
/// and the decision the result reports.
///
/// A check refused by a human control or by authority reports that refusing decision; every
/// other failure keeps the decision that admitted the action.
pub(super) fn unchecked(error: &BrowserError, admitted: Decision) -> (CheckFailure, Decision) {
    match error {
        BrowserError::DocumentAccess(refused) => (governed_check(refused.reason), *refused),
        BrowserError::RuntimeControl(reason) => {
            (governed_check(*reason), Decision::refused(*reason))
        }
        BrowserError::DocumentUnavailable => (CheckFailure::DocumentUnavailable, admitted),
        BrowserError::DialogOpen | BrowserError::TabNotShowing | BrowserError::NoHistoryEntry => {
            (CheckFailure::BrowserRefused, admitted)
        }
        BrowserError::CancelledBeforeDispatch | BrowserError::CancelledAfterDispatch => {
            (CheckFailure::Cancelled, admitted)
        }
        BrowserError::DeadlineBeforeDispatch | BrowserError::DeadlineAfterDispatch => {
            (CheckFailure::DeadlineExpired, admitted)
        }
        BrowserError::DisconnectedBeforeDispatch
        | BrowserError::DisconnectedAfterDispatch
        | BrowserError::StaleTab => (CheckFailure::Disconnected, admitted),
        BrowserError::NotAnsweringBeforeDispatch
        | BrowserError::BusyBeforeDispatch
        | BrowserError::WrongProfile
        | BrowserError::UnknownBrowser(_)
        | BrowserError::BrowserPinned
        | BrowserError::AmbiguousBrowser(_)
        | BrowserError::RecoveryManual { .. }
        | BrowserError::RecoveryFailed { .. } => (CheckFailure::BrowserUnavailable, admitted),
        BrowserError::Primitive { .. }
        | BrowserError::CaptureTooLarge { .. }
        | BrowserError::PageRefused(_)
        | BrowserError::StaleLocator { .. }
        | BrowserError::FieldsRejected { .. }
        | BrowserError::FieldsReverted { .. }
        | BrowserError::StaleView
        | BrowserError::LocalInterlock(_)
        | BrowserError::EffectUnknown(_)
        | BrowserError::ScriptThrew(_)
        | BrowserError::Protocol(_)
        | BrowserError::Authentication
        | BrowserError::Incompatible { .. }
        | BrowserError::Unsupported { .. } => (CheckFailure::BrowserRefused, admitted),
    }
}

/// The check failure for a check that governance refused.
const fn governed_check(reason: ReasonCode) -> CheckFailure {
    CheckFailure::Blocked(BlockedReason::of(reason))
}

/// The terminal status of an applied action whose check did not confirm it.
pub(super) const fn unconfirmed_status(cause: CheckFailure) -> Status {
    match cause {
        CheckFailure::Blocked(_) => Status::Blocked,
        CheckFailure::Cancelled => Status::Cancelled,
        CheckFailure::Unmet
        | CheckFailure::DeadlineExpired
        | CheckFailure::Disconnected
        | CheckFailure::BrowserUnavailable
        | CheckFailure::DocumentUnavailable
        | CheckFailure::BrowserRefused => Status::Failed,
    }
}
