//! Tool-call **dispatch chokepoint** -- the single seam for the v1.5 governance overlay.
//!
//! Every tool call flows through here. In v1.0 (the all-open engine) the policy and audit hooks
//! are **no-ops**: [`policy_check`] always returns [`PolicyDecision::Allow`] and [`audit`] does
//! nothing. The v1.5 overlay replaces these *in place* -- manifest-driven enforcement and the
//! audit subsystem attach here WITHOUT touching any tool code (Principle 5: separation of
//! concerns; see `docs/research/10-forks-decided.md`).

/// The outcome of the per-call policy check.
///
/// In v1.0 this is always [`PolicyDecision::Allow`]. The `Deny` variant (with a structured
/// denial) arrives with the v1.5 governance overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    /// The call is permitted. (The only outcome in the all-open v1.0 engine.)
    Allow,
}

/// v1.0 no-op policy hook -- the all-open engine permits everything.
///
/// The v1.5 overlay replaces this with manifest-driven enforcement (sec 5.3 STEP 0 short-circuits to
/// `Allow` when no manifest is present, so all-open behavior is preserved by construction).
pub fn policy_check(_tool: &str) -> PolicyDecision {
    PolicyDecision::Allow
}

/// v1.0 no-op audit hook.
///
/// The v1.5 audit subsystem subscribes here to record every call, without touching tool code.
pub fn audit(_tool: &str) {}
