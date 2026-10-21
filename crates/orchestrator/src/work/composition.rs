//! Shared flow/sequence progress accumulation under the parent's lease and authority snapshot.

use std::time::Instant;

use serde_json::{json, Value};

use crate::governance::Decision;
use crate::language::composition::{CompositionProgress, StepCause, StepCounts, StepIssue};
use crate::language::coverage::{qualify, Coverage, COVERAGE_FACT};
use crate::language::history::OutcomeClass;
use crate::language::outcome::Outcome;

use super::result::{Readiness, UnexecutedStatus};
use super::{Effect, InvocationContext, InvocationResult, Status, Terminal};

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
                facts[COVERAGE_FACT] = json!(coverage);
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

/// The child's coverage when it withheld anything, read from the child's audit projection.
///
/// A child that inspected everything it was asked to contributes nothing.
fn limited_coverage(terminal: &Terminal) -> Option<Coverage> {
    terminal
        .audit
        .coverage()
        .filter(|coverage| coverage.withholds())
        .cloned()
}

/// Keep recovery metadata outside the optional, budgeted child result payload.
///
/// A limited child's coverage rides on its row, so a child cut from the parent's budget still
/// shows what was withheld from it.
pub(super) fn terminal_row(step: usize, terminal: &Terminal, cause: Option<StepCause>) -> Value {
    let mut row = json!({"step":step,"status":terminal.result.status,"effect":terminal.result.effect,
        "repeat_safe":terminal.result.repeat_safe,"cause":cause,"history_storage":terminal.result.history_storage});
    if let Some(coverage) = limited_coverage(terminal) {
        row[COVERAGE_FACT] = json!(coverage);
    }
    row
}

/// Identify an unexecuted step without assigning it an invocation or a terminal status.
pub(super) fn unexecuted_row(step: usize, status: UnexecutedStatus) -> Value {
    json!({"step":step,"status":status,"effect":Effect::None,"repeat_safe":false})
}

/// The cause a flow keeps for a child, derived from the child's one outcome class.
fn child_cause(terminal: &Terminal) -> Option<StepCause> {
    let refusal = terminal.audit.refusal();
    OutcomeClass::of(terminal.result.status, refusal).step_cause(refusal)
}
