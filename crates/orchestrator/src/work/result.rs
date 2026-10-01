//! The bounded caller projection of frozen Work resolution and its closed wire vocabulary.

use ghostlight_bridge::service::ServiceContent;
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
    /// Storage confirmation for this receipt, independent of browser effects.
    #[serde(default)]
    pub history_storage: crate::language::audit_health::Storage,
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
            history_storage: crate::language::audit_health::Storage::Saved,
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

    /// Render the concise authored text that precedes structured MCP facts.
    #[must_use]
    pub fn model_text(&self) -> String {
        if self.next_steps.is_empty() {
            self.summary.clone()
        } else {
            format!("{} Next: {}", self.summary, self.next_steps.join(" "))
        }
    }

    /// Whether the terminal result should carry MCP's tool-execution error signal.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        !matches!(self.status, Status::Succeeded)
    }
}
