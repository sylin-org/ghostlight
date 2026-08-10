//! One bounded terminal product envelope and its single-completion gate.

use std::sync::{Mutex, MutexGuard};

use ghostlight_bridge::service::ServiceContent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Terminal product status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The requested job completed truthfully.
    Succeeded,
    /// Authority prevented completion.
    Blocked,
    /// A decisive failure occurred without uncertain effects.
    Failed,
    /// Cancellation reached a safe boundary.
    Cancelled,
    /// The user must act in the visible browser.
    AttentionRequired,
    /// A dispatched effect cannot be determined.
    Unknown,
}

/// Terminal physical effect classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    /// No lasting requested effect occurred.
    None,
    /// The requested effect was decisively applied.
    Applied,
    /// Only some requested effects were applied.
    Partial,
    /// Dispatch occurred but the effect cannot be determined.
    Unknown,
}

/// Product-facing readiness after governed completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// Readiness does not apply to this job.
    NotApplicable,
    /// A governed document is still loading.
    Loading,
    /// A governed document is useful and interactive.
    Interactive,
    /// A governed document reported complete.
    Complete,
    /// Readiness cannot be determined truthfully.
    Unknown,
}

/// The only model-facing terminal result shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationResult {
    /// Opaque invocation correlation handle.
    pub invocation: String,
    /// Terminal product status.
    pub status: Status,
    /// Truthful physical effect class.
    pub effect: Effect,
    /// Governed page readiness.
    pub readiness: Readiness,
    /// Whether repeating this call is known safe.
    pub repeat_safe: bool,
    /// Bounded Ghostlight-authored explanation.
    pub summary: String,
    /// Tool-specific canonical facts.
    pub facts: Value,
    /// Zero to two Ghostlight-authored safe recovery suggestions.
    pub next_steps: Vec<String>,
    /// Protocol-neutral rich content carried separately from structured facts.
    #[serde(skip)]
    pub content: Vec<ServiceContent>,
}

impl InvocationResult {
    /// Construct a bounded result and enforce the contextual next-step limit.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        invocation: &str,
        status: Status,
        effect: Effect,
        readiness: Readiness,
        repeat_safe: bool,
        summary: &str,
        facts: Value,
        mut next_steps: Vec<String>,
    ) -> Self {
        next_steps.truncate(2);
        Self {
            invocation: invocation.into(),
            status,
            effect,
            readiness,
            repeat_safe,
            summary: summary.chars().take(500).collect(),
            facts,
            next_steps,
            content: Vec::new(),
        }
    }

    /// Attach one protocol-neutral content item for generic edge rendering.
    #[must_use]
    pub fn with_content(mut self, content: ServiceContent) -> Self {
        self.content.push(content);
        self
    }
}

/// Enforces exactly one terminal construction path for an invocation.
#[derive(Debug, Default)]
pub struct CompletionGate {
    result: Mutex<Option<InvocationResult>>,
}

impl CompletionGate {
    /// Commit the invocation's only terminal outcome.
    pub fn complete(&self, result: InvocationResult) -> Result<(), CompletionError> {
        let mut slot = lock(&self.result);
        if slot.is_some() {
            return Err(CompletionError::AlreadyCompleted);
        }
        *slot = Some(result);
        Ok(())
    }

    /// Consume the terminal result after all synchronous reactions finish.
    pub fn take(&self) -> Result<InvocationResult, CompletionError> {
        lock(&self.result)
            .take()
            .ok_or(CompletionError::NotCompleted)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Completion invariant failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CompletionError {
    /// A second terminal outcome was attempted.
    #[error("invocation already completed")]
    AlreadyCompleted,
    /// The executor exited without a terminal outcome.
    #[error("invocation did not complete")]
    NotCompleted,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CompletionError, CompletionGate, Effect, InvocationResult, Readiness, Status};

    #[test]
    fn gate_accepts_exactly_one_terminal_outcome() {
        let gate = CompletionGate::default();
        let result = InvocationResult::new(
            "invocation_x",
            Status::Succeeded,
            Effect::None,
            Readiness::NotApplicable,
            true,
            "done",
            json!({}),
            vec![],
        );
        gate.complete(result.clone()).unwrap();
        assert_eq!(
            gate.complete(result),
            Err(CompletionError::AlreadyCompleted)
        );
        assert_eq!(gate.take().unwrap().status, Status::Succeeded);
    }
}
