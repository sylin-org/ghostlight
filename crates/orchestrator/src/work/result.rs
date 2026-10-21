//! One bounded terminal product envelope. Each invocation completes once: the executor hands its
//! single `Terminal` to the completion seam by value, so a second completion cannot be written.

use crate::intake::session::ServiceContent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// A dispatched effect cannot be determined.
    Unknown,
}

impl Status {
    /// The name an audit record stores for this status.
    #[must_use]
    pub fn stored_name(self) -> String {
        stored_name(self)
    }

    /// The status an audit record stored as `name`, or `None` for a word no settled call ends
    /// with, such as [`UnexecutedStatus::NotStarted`]'s.
    #[must_use]
    pub fn from_stored_name(name: &str) -> Option<Self> {
        use serde::de::value::{Error, StrDeserializer};
        Self::deserialize(StrDeserializer::<Error>::new(name)).ok()
    }
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

impl Effect {
    /// The name an audit record stores for this effect.
    #[must_use]
    pub fn stored_name(self) -> String {
        stored_name(self)
    }
}

/// The status of a composition step that has no invocation of its own.
///
/// A flow row carries it, and the audit record of a step that never started stores
/// [`Self::NotStarted`]'s name where a settled call stores its [`Status`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnexecutedStatus {
    /// Its input could not be prepared, so the step never started.
    NotStarted,
    /// The composition stopped before reaching it.
    NotRun,
}

impl UnexecutedStatus {
    /// The name a flow row and an audit record store for this status.
    #[must_use]
    pub fn stored_name(self) -> String {
        stored_name(self)
    }
}

/// The serialized name of a closed status or effect word.
fn stored_name(word: impl Serialize) -> String {
    serde_json::to_value(word)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
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
    /// Storage confirmation for this receipt, independent of browser effects.
    #[serde(default)]
    pub history_storage: crate::language::audit_health::Storage,
    /// The request's id, the one its audit line names.
    pub invocation: super::InvocationId,
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
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per field the result contract requires; rich content is attached after construction"
    )]
    #[must_use]
    pub fn new(
        invocation: &super::InvocationId,
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
            history_storage: crate::language::audit_health::Storage::Saved,
            invocation: invocation.clone(),
            status,
            effect,
            readiness,
            repeat_safe,
            summary: summary
                .chars()
                .take(crate::language::outcome::SUMMARY_MAX_CHARS)
                .collect(),
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

    /// Render the concise authored text that precedes structured MCP facts.
    #[must_use]
    pub fn model_text(&self) -> String {
        crate::language::outcome::model_text(&self.summary, &self.next_steps)
    }

    /// Whether the terminal result should carry MCP's tool-execution error signal.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        !matches!(self.status, Status::Succeeded)
    }
}
