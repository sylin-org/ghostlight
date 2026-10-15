//! Shared flow/sequence progress accumulation under the parent's lease and authority snapshot.

use std::time::Instant;

use serde::Serialize;
use serde_json::{json, Value};

use crate::governance::Decision;
use crate::language::audit::AuditRefusal;
use crate::language::composition::{CompositionProgress, StepCause, StepCounts, StepIssue};
use crate::language::coverage::{qualify, Coverage};
use crate::language::outcome::{
    BlockedReason, BrowserUnavailableReason, CheckFailure, Outcome, WorkspaceReason,
};

use super::result::Readiness;
use super::{Effect, InvocationContext, InvocationResult, Status, Terminal};

/// The key inside a result's `facts` that holds its coverage.
const COVERAGE: &str = "coverage";

/// A step without an invocation must not masquerade as a failed child operation.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum UnexecutedStatus {
    NotStarted,
    NotRun,
}

pub(super) struct Composition {
    pub progress: CompositionProgress,
    decision: Decision,
    readiness: Readiness,
    physical_id: Option<u64>,
    all_repeat_safe: bool,
    unconfirmed_history_steps: u32,
    /// What limited any child, kept even when the child's result is cut from the parent.
    coverage: Option<Coverage>,
}

impl Composition {
    /// Begin accounting for the declared steps without fabricating child invocations.
    pub fn new(
        total: usize,
        decision: Decision,
        readiness: Readiness,
        physical_id: Option<u64>,
    ) -> Self {
        Self {
            progress: CompositionProgress {
                counts: StepCounts {
                    total,
                    not_run: total,
                    ..StepCounts::default()
                },
                ..CompositionProgress::default()
            },
            decision,
            readiness,
            physical_id,
            all_repeat_safe: true,
            unconfirmed_history_steps: 0,
            coverage: None,
        }
    }

    /// Record cancellation or timeout between children; the next step remains unexecuted.
    pub fn stop_at_boundary(&mut self, context: &InvocationContext<'_>, step: usize) -> bool {
        let cause = if context.cancellation.is_cancelled() {
            Some(StepCause::Cancelled)
        } else if Instant::now() >= context.deadline {
            Some(StepCause::Deadline)
        } else {
            None
        };
        if let Some(cause) = cause {
            self.note_issue(step, cause);
            self.progress.stopped = true;
        }
        cause.is_some()
    }

    /// Count an unconfirmed preparation receipt without inventing a browser attempt.
    pub fn record_storage(&mut self, storage: crate::language::audit_health::Storage) {
        if storage == crate::language::audit_health::Storage::Unconfirmed {
            self.unconfirmed_history_steps += 1;
        }
    }

    /// Record an input failure that never entered the child executor.
    pub fn not_started(&mut self, step: usize, cause: StepCause) {
        self.progress.counts.not_run -= 1;
        self.progress.counts.not_started += 1;
        self.note_issue(step, cause);
    }

    /// Retain a child's status, effects, and closed recovery cause.
    pub fn record(&mut self, step: usize, terminal: &Terminal) -> Option<StepCause> {
        let result = &terminal.result;
        if result.history_storage == crate::language::audit_health::Storage::Unconfirmed {
            self.unconfirmed_history_steps += 1;
        }
        let counts = &mut self.progress.counts;
        counts.not_run -= 1;
        match result.status {
            Status::Succeeded => counts.succeeded += 1,
            Status::Blocked => counts.blocked += 1,
            Status::Failed => counts.failed += 1,
            Status::Cancelled => counts.cancelled += 1,
            Status::AttentionRequired => counts.attention_required += 1,
            Status::Unknown => counts.unknown += 1,
        }
        match result.effect {
            Effect::None => self.progress.effects.none += 1,
            Effect::Applied => self.progress.effects.applied += 1,
            Effect::Partial => self.progress.effects.partial += 1,
            Effect::Unknown => self.progress.effects.unknown += 1,
        }
        self.all_repeat_safe &= result.repeat_safe;
        if let Some(limited) = limited_coverage(terminal) {
            match &mut self.coverage {
                Some(aggregate) => aggregate.include(&limited),
                None => self.coverage = Some(limited),
            }
        }
        self.readiness = result.readiness;
        self.physical_id = terminal.physical_id.or(self.physical_id);
        // Retain a deciding denial even if Continue later admits an independent step.
        if self.decision.allowed {
            self.decision = terminal.decision;
        }
        let cause = child_cause(terminal);
        if let Some(cause) = cause {
            self.note_issue(step, cause);
        }
        cause
    }

    fn note_issue(&mut self, step: usize, cause: StepCause) {
        if self
            .progress
            .issue
            .is_none_or(|issue| cause.priority() > issue.cause.priority())
        {
            self.progress.issue = Some(StepIssue { step, cause });
        }
    }

    fn effect(&self) -> Effect {
        let p = &self.progress;
        if p.effects.unknown > 0 {
            Effect::Unknown
        } else if p.effects.partial > 0
            || (p.effects.applied > 0 && p.counts.succeeded < p.counts.total)
        {
            Effect::Partial
        } else if p.effects.applied > 0 {
            Effect::Applied
        } else {
            Effect::None
        }
    }

    fn status(&self) -> Status {
        let p = &self.progress;
        let cause = p.issue.map(|issue| issue.cause);
        if p.effects.unknown > 0 || p.counts.unknown > 0 {
            Status::Unknown
        } else if p.counts.attention_required > 0 {
            Status::AttentionRequired
        } else if p.counts.cancelled > 0 || cause == Some(StepCause::Cancelled) {
            Status::Cancelled
        } else if p.counts.blocked > 0
            || matches!(cause, Some(StepCause::Paused | StepCause::SessionEnded))
        {
            Status::Blocked
        } else if p.counts.succeeded == p.counts.total {
            Status::Succeeded
        } else {
            Status::Failed
        }
    }

    /// Complete the parent with language and audit projected from the same progress account.
    pub fn finish(self, context: &InvocationContext<'_>, mut facts: Value) -> Terminal {
        let status = self.status();
        let effect = self.effect();
        facts["progress"] = json!(self.progress);
        facts["unconfirmed_history_steps"] = json!(self.unconfirmed_history_steps);
        let outcome = Outcome::CompositionRan(self.progress);
        let summary = match &self.coverage {
            Some(coverage) => {
                facts["coverage"] = json!(coverage);
                qualify(&outcome.summary(), coverage)
            }
            None => outcome.summary(),
        };
        Terminal {
            result: InvocationResult::new(
                context.invocation,
                status,
                effect,
                self.readiness,
                status == Status::Succeeded && effect == Effect::None && self.all_repeat_safe,
                &summary,
                facts,
                outcome.next_steps(),
            ),
            decision: self.decision,
            physical_id: self.physical_id,
            observed: outcome.observed(),
            audit: outcome
                .audit()
                .with_unconfirmed_history(self.unconfirmed_history_steps),
        }
    }
}

/// The child's coverage when something limited it, read from the canonical child envelope.
///
/// A limitation is any exclusion, unavailable document, size ceiling or masked region. A child
/// that inspected everything it was asked to contributes nothing.
fn limited_coverage(terminal: &Terminal) -> Option<Coverage> {
    let coverage: Coverage =
        serde_json::from_value(terminal.result.facts.get(COVERAGE)?.clone()).ok()?;
    let limited =
        coverage.limited() || coverage.page_excluded_documents > 0 || coverage.masked_regions > 0;
    limited.then_some(coverage)
}

/// Keep recovery metadata outside the optional, budgeted child result payload.
///
/// A limited child's coverage rides on its row, so a child cut from the parent's budget still
/// shows what was withheld from it.
pub(super) fn terminal_row(step: usize, terminal: &Terminal, cause: Option<StepCause>) -> Value {
    let mut row = json!({"step":step,"status":terminal.result.status,"effect":terminal.result.effect,
        "repeat_safe":terminal.result.repeat_safe,"cause":cause,"history_storage":terminal.result.history_storage});
    if let Some(coverage) = limited_coverage(terminal) {
        row[COVERAGE] = json!(coverage);
    }
    row
}

/// Identify an unexecuted step without assigning it an invocation or a terminal status.
pub(super) fn unexecuted_row(step: usize, status: UnexecutedStatus) -> Value {
    json!({"step":step,"status":status,"effect":Effect::None,"repeat_safe":false})
}

fn child_cause(terminal: &Terminal) -> Option<StepCause> {
    use AuditRefusal as A;
    Some(match terminal.audit.refusal() {
        Some(A::AuthorityBlocked {
            cause: BlockedReason::AuditUnavailable,
        }) => StepCause::AuditUnavailable,
        Some(
            A::AuthorityBlocked {
                cause: BlockedReason::Hold,
            }
            | A::WorkspaceUnusable {
                cause: WorkspaceReason::TabHeld,
            },
        ) => StepCause::Paused,
        Some(A::AuthorityBlocked {
            cause: BlockedReason::SessionEnded,
        }) => StepCause::SessionEnded,
        Some(A::CredentialHandoff | A::CredentialAuthorization) => StepCause::Failed,
        Some(A::AttentionRequired | A::LocalInterlock) => StepCause::AttentionRequired,
        Some(A::DeadlineBeforeStart | A::DeadlineExpired { .. }) => StepCause::Deadline,
        Some(A::CancelledBeforeStart | A::CancelledBeforeDispatch | A::CancelledAfterDispatch) => {
            StepCause::Cancelled
        }
        Some(
            A::ConnectionLost
            | A::BrowserUnavailable {
                cause:
                    BrowserUnavailableReason::NotConnected | BrowserUnavailableReason::NotAnswering,
            },
        ) => StepCause::ConnectionLost,
        Some(A::ActionUnconfirmed { cause }) => unconfirmed_cause(*cause),
        _ => match terminal.result.status {
            Status::Succeeded => return None,
            Status::Blocked => StepCause::PolicyBlocked,
            Status::Failed => StepCause::Failed,
            Status::Cancelled => StepCause::Cancelled,
            Status::AttentionRequired => StepCause::AttentionRequired,
            Status::Unknown => StepCause::Unconfirmed,
        },
    })
}

/// The flow cause of an applied child whose requested check did not confirm it.
///
/// It is the cause the check would have had on its own, so a Pause, Stop, cancellation, or
/// deadline during a check still stops the flow the way it stops any other step.
const fn unconfirmed_cause(cause: CheckFailure) -> StepCause {
    match cause {
        CheckFailure::Blocked(BlockedReason::AuditUnavailable) => StepCause::AuditUnavailable,
        CheckFailure::Blocked(BlockedReason::Hold) => StepCause::Paused,
        CheckFailure::Blocked(BlockedReason::SessionEnded) => StepCause::SessionEnded,
        CheckFailure::Blocked(_) => StepCause::PolicyBlocked,
        CheckFailure::AttentionRequired => StepCause::AttentionRequired,
        CheckFailure::Cancelled => StepCause::Cancelled,
        CheckFailure::DeadlineExpired => StepCause::Deadline,
        CheckFailure::Disconnected => StepCause::ConnectionLost,
        CheckFailure::Unmet
        | CheckFailure::BrowserUnavailable
        | CheckFailure::DocumentUnavailable
        | CheckFailure::BrowserRefused => StepCause::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::GovernanceFacade;
    use crate::language::outcome::{Refusal, HUMAN_PAUSE_DIRECTIVE, HUMAN_STOP_DIRECTIVE};

    use crate::intake::session::IntakeChannel;
    use crate::workspace::WorkspaceStore;

    fn child(status: Status, effect: Effect, refusal: Option<Refusal>) -> Terminal {
        let audit = refusal.map_or_else(|| Outcome::TabsListed { count: 0 }.audit(), |r| r.audit());
        Terminal {
            result: InvocationResult::new(
                "child",
                status,
                effect,
                Readiness::Complete,
                effect == Effect::None,
                audit.summary(),
                json!({}),
                vec![],
            ),
            decision: Decision::permitted(),
            physical_id: Some(7),
            observed: Default::default(),
            audit,
        }
    }

    #[test]
    fn mixed_progress_preserves_known_effects_and_never_promotes_failure() {
        let cases = [
            (
                Effect::None,
                Status::Failed,
                Effect::None,
                Status::Failed,
                Effect::None,
            ),
            (
                Effect::Applied,
                Status::Failed,
                Effect::None,
                Status::Failed,
                Effect::Partial,
            ),
            (
                Effect::Partial,
                Status::Blocked,
                Effect::None,
                Status::Blocked,
                Effect::Partial,
            ),
            (
                Effect::Applied,
                Status::Unknown,
                Effect::Unknown,
                Status::Unknown,
                Effect::Unknown,
            ),
            (
                Effect::Partial,
                Status::Unknown,
                Effect::Unknown,
                Status::Unknown,
                Effect::Unknown,
            ),
            (
                Effect::None,
                Status::AttentionRequired,
                Effect::None,
                Status::AttentionRequired,
                Effect::None,
            ),
            (
                Effect::Applied,
                Status::Cancelled,
                Effect::None,
                Status::Cancelled,
                Effect::Partial,
            ),
        ];
        for (earlier, child_status, child_effect, status, effect) in cases {
            let mut composition =
                Composition::new(3, Decision::permitted(), Readiness::NotApplicable, None);
            composition.record(1, &child(Status::Succeeded, earlier, None));
            composition.record(2, &child(child_status, child_effect, None));
            // Continue succeeds at later work; the first failure and its effects still count.
            composition.record(3, &child(Status::Succeeded, Effect::None, None));
            assert_eq!(composition.status(), status);
            assert_eq!(composition.effect(), effect);
            assert_eq!(composition.progress.counts.succeeded, 2);
            assert_eq!(composition.progress.counts.not_run, 0);
            assert_eq!(
                composition.progress.effects.applied,
                usize::from(earlier == Effect::Applied)
            );
            assert_eq!(
                composition.progress.effects.partial,
                usize::from(earlier == Effect::Partial)
            );
        }
    }

    #[test]
    fn actual_unknown_causes_and_human_directives_survive_composition() {
        for (refusal, cause, sentence) in [
            (
                Refusal::ConnectionLost,
                StepCause::ConnectionLost,
                "Connection lost during step 2.",
            ),
            (
                Refusal::EffectUnknown,
                StepCause::Unconfirmed,
                "Step 2 did not confirm completion.",
            ),
            (
                Refusal::AuthorityBlocked {
                    reason: BlockedReason::Hold,
                    host: None,
                },
                StepCause::Paused,
                HUMAN_PAUSE_DIRECTIVE,
            ),
            (
                Refusal::AuthorityBlocked {
                    reason: BlockedReason::SessionEnded,
                    host: None,
                },
                StepCause::SessionEnded,
                HUMAN_STOP_DIRECTIVE,
            ),
        ] {
            let mut composition =
                Composition::new(3, Decision::permitted(), Readiness::NotApplicable, None);
            composition.record(1, &child(Status::Succeeded, Effect::Applied, None));
            let status = if cause.stops_execution() {
                Status::Blocked
            } else {
                Status::Unknown
            };
            let effect = if status == Status::Unknown {
                Effect::Unknown
            } else {
                Effect::None
            };
            assert_eq!(
                composition.record(2, &child(status, effect, Some(refusal))),
                Some(cause)
            );
            composition.progress.stopped = true;
            assert!(composition.progress.summary().contains(sentence));
            if cause.stops_execution() {
                assert!(composition.progress.next_steps().is_empty());
            } else {
                assert!(
                    composition.progress.next_steps()[0].starts_with("Inspect the current page")
                );
            }
        }
    }

    #[test]
    fn between_step_limits_do_not_fabricate_attempts_or_lose_prior_effects() {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let snapshot = GovernanceFacade::new(None, None).snapshot();
        for cancelled in [true, false] {
            let token = super::super::CancellationToken::default();
            if cancelled {
                token.cancel();
            }
            let context = InvocationContext {
                settlement_pending: &std::sync::atomic::AtomicBool::new(false),
                settlement_requested: false,
                provenance: None,
                requirements: crate::governance::CapabilitySet::READ,
                invocation: "parent",
                workspace: &workspace,
                requested_browser: None,
                requested_tab: None,
                requested_target: None,
                snapshot: &snapshot,
                deadline: Instant::now(),
                deadline_adjustable: false,
                cancellation: &token,
            };
            let mut composition =
                Composition::new(3, Decision::permitted(), Readiness::NotApplicable, None);
            composition.record(1, &child(Status::Succeeded, Effect::Applied, None));
            assert!(composition.stop_at_boundary(&context, 2));
            let result = composition.finish(&context, json!({})).result;
            assert_eq!(
                result.status,
                if cancelled {
                    Status::Cancelled
                } else {
                    Status::Failed
                }
            );
            assert_eq!(result.effect, Effect::Partial);
            assert!(!result.repeat_safe);
            assert_eq!(result.facts["progress"]["counts"]["not_run"], 2);
            assert_eq!(result.facts["progress"]["counts"]["cancelled"], 0);
            assert_eq!(result.facts["progress"]["counts"]["failed"], 0);
        }
    }

    fn read_with(coverage: Coverage) -> Terminal {
        let mut terminal = child(Status::Succeeded, Effect::None, None);
        terminal.result.facts["coverage"] = json!(coverage);
        terminal
    }

    fn excluded() -> Coverage {
        Coverage {
            inspected_documents: 1,
            excluded_documents: 1,
            ..Coverage::default()
        }
    }

    #[test]
    fn a_limited_child_row_carries_its_coverage_and_an_unlimited_one_adds_nothing() {
        let limited = terminal_row(1, &read_with(excluded()), None);
        assert_eq!(limited["coverage"]["excluded_documents"], 1);
        let complete = Coverage {
            inspected_documents: 2,
            ..Coverage::default()
        };
        assert!(terminal_row(2, &read_with(complete), None)
            .get("coverage")
            .is_none());
        let none = child(Status::Succeeded, Effect::None, None);
        assert!(terminal_row(3, &none, None).get("coverage").is_none());
    }

    #[test]
    fn the_parent_aggregates_limited_children_and_qualifies_its_summary() {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let snapshot = GovernanceFacade::new(None, None).snapshot();
        let token = super::super::CancellationToken::default();
        let context = InvocationContext {
            settlement_pending: &std::sync::atomic::AtomicBool::new(false),
            settlement_requested: false,
            provenance: None,
            requirements: crate::governance::CapabilitySet::READ,
            invocation: "parent",
            workspace: &workspace,
            requested_browser: None,
            requested_tab: None,
            requested_target: None,
            snapshot: &snapshot,
            deadline: Instant::now(),
            deadline_adjustable: false,
            cancellation: &token,
        };
        let finish = |children: Vec<Terminal>| {
            let mut composition = Composition::new(
                children.len(),
                Decision::permitted(),
                Readiness::NotApplicable,
                None,
            );
            for (index, terminal) in children.iter().enumerate() {
                composition.record(index + 1, terminal);
            }
            composition.finish(&context, json!({})).result
        };
        let unlimited = finish(vec![
            read_with(Coverage::default()),
            child(Status::Succeeded, Effect::None, None),
        ]);
        assert!(unlimited.facts.get("coverage").is_none());
        assert!(!unlimited.summary.contains("excluded by policy"));
        let size = Coverage {
            limited_by_size: true,
            ..Coverage::default()
        };
        let limited = finish(vec![read_with(excluded()), read_with(size)]);
        assert_eq!(limited.facts["coverage"]["excluded_documents"], 1);
        assert_eq!(limited.facts["coverage"]["limited_by_size"], true);
        assert!(limited
            .summary
            .ends_with("Content excluded by policy. The result reached its size limit."));
    }
}
