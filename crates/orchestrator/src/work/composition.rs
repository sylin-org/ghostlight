//! Shared flow/sequence progress accumulation under the parent's lease and authority snapshot.

use std::time::Instant;

use serde::Serialize;
use serde_json::{json, Value};

use crate::governance::Decision;
use crate::language::composition::{CompositionProgress, StepCause, StepCounts, StepIssue};
use crate::language::outcome::{BlockedReason, Outcome, Refusal, WorkspaceReason};

use super::result::Readiness;
use super::{CompletedWork, Conclusion, Effect, InvocationContext, Status, WorkEvidence};

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
    pub fn record(&mut self, step: usize, terminal: &CompletedWork) -> Option<StepCause> {
        let result = &terminal.resolution;
        if terminal.storage == crate::language::audit_health::Storage::Unconfirmed {
            self.unconfirmed_history_steps += 1;
        }
        let counts = &mut self.progress.counts;
        counts.not_run -= 1;
        match result.status() {
            Status::Succeeded => counts.succeeded += 1,
            Status::Blocked => counts.blocked += 1,
            Status::Failed => counts.failed += 1,
            Status::Cancelled => counts.cancelled += 1,
            Status::AttentionRequired => counts.attention_required += 1,
            Status::Unknown => counts.unknown += 1,
        }
        match result.effect() {
            Effect::None => self.progress.effects.none += 1,
            Effect::Applied => self.progress.effects.applied += 1,
            Effect::Partial => self.progress.effects.partial += 1,
            Effect::Unknown => self.progress.effects.unknown += 1,
        }
        self.all_repeat_safe &= result.repeat_safe();
        self.readiness = result.readiness();
        self.physical_id = terminal.resolution.physical_id().or(self.physical_id);
        // Retain a deciding denial even if Continue later admits an independent step.
        if self.decision.allowed {
            self.decision = terminal.resolution.decision();
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
    pub fn finish(self, context: &InvocationContext<'_>, mut facts: Value) -> WorkEvidence {
        let status = self.status();
        let effect = self.effect();
        facts["progress"] = json!(self.progress);
        facts["unconfirmed_history_steps"] = json!(self.unconfirmed_history_steps);
        let outcome = Outcome::CompositionRan(self.progress);
        let mut evidence = WorkEvidence::new(
            context.invocation,
            status,
            effect,
            self.readiness,
            status == Status::Succeeded && effect == Effect::None && self.all_repeat_safe,
            Conclusion::Outcome(outcome),
            facts,
            self.decision,
            self.physical_id,
        );
        evidence.unconfirmed_history_steps = self.unconfirmed_history_steps;
        evidence
    }
}

/// Keep recovery metadata outside the optional, budgeted child result payload.
pub(super) fn terminal_row(
    step: usize,
    terminal: &CompletedWork,
    cause: Option<StepCause>,
) -> Value {
    json!({"step":step,"status":terminal.resolution.status(),"effect":terminal.resolution.effect(),
        "repeat_safe":terminal.resolution.repeat_safe(),"cause":cause,"history_storage":terminal.storage})
}

/// Identify an unexecuted step without assigning it an invocation or a terminal status.
pub(super) fn unexecuted_row(step: usize, status: UnexecutedStatus) -> Value {
    json!({"step":step,"status":status,"effect":Effect::None,"repeat_safe":false})
}

fn child_cause(terminal: &CompletedWork) -> Option<StepCause> {
    use Refusal as R;
    Some(match terminal.resolution.cause() {
        Some(R::AuthorityBlocked {
            reason: BlockedReason::AuditUnavailable,
            ..
        }) => StepCause::AuditUnavailable,
        Some(
            R::AuthorityBlocked {
                reason: BlockedReason::Hold,
                ..
            }
            | R::WorkspaceUnusable {
                reason: WorkspaceReason::TabHeld,
            },
        ) => StepCause::Paused,
        Some(R::AuthorityBlocked {
            reason: BlockedReason::SessionEnded,
            ..
        }) => StepCause::SessionEnded,
        Some(R::CredentialAuthorization) => StepCause::Failed,
        Some(R::AttentionRequired | R::LocalInterlock) => StepCause::AttentionRequired,
        Some(R::DeadlineBeforeStart | R::DeadlineExpired { .. }) => StepCause::Deadline,
        Some(R::CancelledBeforeStart | R::CancelledAfterDispatch) => StepCause::Cancelled,
        Some(R::ConnectionLost | R::BrowserStopped { reconnect: true }) => {
            StepCause::ConnectionLost
        }
        Some(R::ScriptException) => StepCause::ScriptException,
        _ => match terminal.resolution.status() {
            Status::Succeeded => return None,
            Status::Blocked => StepCause::PolicyBlocked,
            Status::Failed => StepCause::Failed,
            Status::Cancelled => StepCause::Cancelled,
            Status::AttentionRequired => StepCause::AttentionRequired,
            Status::Unknown => StepCause::Unconfirmed,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::GovernanceFacade;
    use crate::language::outcome::{Refusal, HUMAN_PAUSE_DIRECTIVE, HUMAN_STOP_DIRECTIVE};

    use crate::workspace::WorkspaceStore;
    use ghostlight_bridge::service::IntakeChannel;

    fn child(status: Status, effect: Effect, refusal: Option<Refusal>) -> CompletedWork {
        let conclusion = refusal.map_or_else(
            || Conclusion::Outcome(Outcome::TabsListed { count: 0 }),
            Conclusion::Refusal,
        );
        let (resolution, payload) = super::super::resolution::Resolution::fixture(
            conclusion,
            status,
            effect,
            super::super::Verification::NotRequested,
            super::super::Phase::RequestedEffect,
            Default::default(),
            json!({}),
        );
        let projection = crate::language::resolution::project(&resolution, payload);
        CompletedWork {
            resolution,
            caller: projection.caller,
            storage: crate::language::audit_health::Storage::Saved,
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
                Refusal::ScriptException,
                StepCause::ScriptException,
                "The script in step 2 threw an exception.",
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
        let workspace = store.admit("test".into(), IntakeChannel::Mcp, None);
        let snapshot = GovernanceFacade::new(None, None).snapshot();
        for cancelled in [true, false] {
            let token = super::super::CancellationToken::default();
            if cancelled {
                token.cancel();
            }
            let execution = std::sync::Mutex::new(super::super::ExecutionEvidence::default());
            let context = InvocationContext {
                provenance: None,
                requirements: crate::governance::CapabilitySet::READ,
                invocation: "parent",
                workspace: &workspace,
                requested_browser: None,
                requested_tab: None,
                requested_target: None,
                snapshot: &snapshot,
                deadline: Instant::now(),
                cancellation: &token,
                execution: &execution,
                phase: super::super::Phase::RequestedEffect,
            };
            let mut composition =
                Composition::new(3, Decision::permitted(), Readiness::NotApplicable, None);
            composition.record(1, &child(Status::Succeeded, Effect::Applied, None));
            assert!(composition.stop_at_boundary(&context, 2));
            let (resolution, payload) = composition.finish(&context, json!({})).resolve(
                super::super::ExecutionEvidence::default(),
                None,
                Default::default(),
            );
            let result = crate::language::resolution::project(&resolution, payload).caller;
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
}
