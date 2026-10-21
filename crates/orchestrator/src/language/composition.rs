//! Payload-free composition progress and contextual outcome language.

use serde::{Deserialize, Serialize};

use super::outcome::{HUMAN_PAUSE_DIRECTIVE, HUMAN_STOP_DIRECTIVE};

/// Disjoint step counts; unable-to-start and unreached steps are not terminal failures.
///
/// Unknown fields are ignored, so retained history lines with other counts still read.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StepCounts {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub blocked: usize,
    pub cancelled: usize,
    pub unknown: usize,
    pub not_started: usize,
    pub not_run: usize,
}

/// Child effect counts retain known progress even when another child is uncertain.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EffectCounts {
    pub none: usize,
    pub applied: usize,
    pub partial: usize,
    pub unknown: usize,
}

/// Closed causes safe to retain without child inputs, results, ids, or error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepCause {
    AuditUnavailable,
    InvalidArguments,
    MissingReference,
    PolicyBlocked,
    Paused,
    SessionEnded,
    AttentionRequired,
    Failed,
    ConnectionLost,
    Deadline,
    Cancelled,
    Unconfirmed,
}

impl StepCause {
    /// Human controls and invocation limits take precedence over ordinary error continuation.
    #[must_use]
    pub fn stops_execution(self) -> bool {
        matches!(
            self,
            Self::AuditUnavailable
                | Self::Paused
                | Self::SessionEnded
                | Self::AttentionRequired
                | Self::Deadline
                | Self::Cancelled
        )
    }

    /// Prefer human instructions and execution limits over ordinary recovery problems.
    pub(crate) fn priority(self) -> u8 {
        match self {
            Self::SessionEnded | Self::Paused => 4,
            Self::AuditUnavailable | Self::AttentionRequired | Self::Cancelled | Self::Deadline => {
                3
            }
            Self::ConnectionLost | Self::Unconfirmed => 2,
            _ => 1,
        }
    }
}

/// One-based position and reason for the problem most relevant to recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StepIssue {
    pub step: usize,
    pub cause: StepCause,
}

/// Maximum child count supported by a composition, including restored history.
pub const COMPOSITION_STEP_LIMIT: usize = 20;

/// What live history says of a flow still running: how many of its steps succeeded so far.
#[must_use]
pub fn running_summary(completed: usize, total: usize) -> String {
    format!("Completed {completed} of {total} steps.")
}

/// What live history says of a restored flow whose steps were recorded but whose completion
/// was not.
pub const UNRECORDED_SUMMARY: &str = "Completion was not recorded.";

/// One composition's bounded, payload-free account, shared by results and retained history.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompositionProgress {
    pub counts: StepCounts,
    pub effects: EffectCounts,
    pub stopped: bool,
    pub issue: Option<StepIssue>,
}

impl CompositionProgress {
    /// Say what succeeded and what needs attention without inventing effects.
    #[must_use]
    pub fn summary(&self) -> String {
        let c = self.counts;
        if c.succeeded == c.total {
            if c.total == 1 {
                return "Completed 1 step.".into();
            }
            return format!("Completed all {} steps.", c.total);
        }
        let noun = if c.total == 1 { "step" } else { "steps" };
        let progress = format!("Completed {} of {} {noun}.", c.succeeded, c.total);
        if let Some(StepIssue { step, cause }) = self.issue {
            match cause {
                StepCause::Paused => return format!("{HUMAN_PAUSE_DIRECTIVE} {progress}"),
                StepCause::SessionEnded => return format!("{HUMAN_STOP_DIRECTIVE} {progress}"),
                _ => {}
            }
            if self.stopped || cause.priority() > 1 {
                let problem = match cause {
                    StepCause::InvalidArguments | StepCause::MissingReference => {
                        format!("Step {step} could not start.")
                    }
                    StepCause::AuditUnavailable => {
                        format!("Step {step} stopped because policy requires saved history.")
                    }
                    StepCause::PolicyBlocked => format!("Step {step} blocked by policy."),
                    StepCause::AttentionRequired => format!("Step {step} needs your attention."),
                    StepCause::Failed => format!("Step {step} failed."),
                    StepCause::ConnectionLost => format!("Connection lost during step {step}."),
                    StepCause::Deadline => format!("Time limit reached at step {step}."),
                    StepCause::Cancelled => format!("Cancelled at step {step}."),
                    StepCause::Unconfirmed => format!("Step {step} did not confirm completion."),
                    StepCause::Paused | StepCause::SessionEnded => unreachable!(),
                };
                return format!("{progress} {problem}");
            }
        }
        let problems: Vec<String> = [
            (c.failed, "failed"),
            (c.blocked, "blocked"),
            (c.cancelled, "cancelled"),
            (c.unknown, "unconfirmed"),
            (c.not_started, "could not start"),
            (c.not_run, "not run"),
        ]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect();
        format!("{progress} {}.", problems.join(", "))
    }

    /// Offer recovery for unfinished work while respecting effects and human control.
    #[must_use]
    pub fn next_steps(&self) -> Vec<String> {
        if self.counts.succeeded == self.counts.total {
            return vec![];
        }
        let cause = self.issue.map(|issue| issue.cause);
        if matches!(
            cause,
            Some(StepCause::Paused | StepCause::SessionEnded | StepCause::Cancelled)
        ) {
            return vec![];
        }
        if cause == Some(StepCause::AttentionRequired) {
            return vec![
                "Wait for the user to resolve the attention request before continuing.".into(),
            ];
        }
        if cause == Some(StepCause::AuditUnavailable) {
            return vec!["Wait for history saving to recover, then prepare only unfinished work. Keep confirmed changes.".into()];
        }
        let mut steps = Vec::new();
        if self.effects.unknown > 0 || self.counts.unknown > 0 {
            steps.push(
                "Inspect the current page to establish what happened before continuing.".into(),
            );
        } else if self.effects.applied + self.effects.partial > 0 {
            steps.push("Keep the confirmed changes; inspect the current page before preparing unfinished work.".into());
        }
        steps.push(match cause {
            Some(StepCause::PolicyBlocked) => "Use the per-step results to choose only unfinished work allowed by policy.",
            Some(StepCause::InvalidArguments | StepCause::MissingReference) => "Correct the unresolved inputs and prepare only the unfinished steps with current page references.",
            _ => "Use the per-step results to prepare only unfinished work with current page references.",
        }.into());
        steps
    }
}
