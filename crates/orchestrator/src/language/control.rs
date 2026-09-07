//! Human session-review and recovery language, independent from policy authority.

use crate::workspace::AttentionReason;
use ghostlight_bridge::browser::RuntimeControlState;

/// Explain the review requirement in the session's own surface.
pub fn attention(reason: AttentionReason) -> &'static str {
    match reason {
        AttentionReason::RepeatedDenials => {
            "This session needs your attention after repeated policy refusals."
        }
        AttentionReason::CredentialHandoff => {
            "This session needs you to handle credentials in the browser."
        }
    }
}

/// State what an explicit recovery permits, retaining any independent global control.
pub fn resumed(state: RuntimeControlState) -> &'static str {
    match state {
        RuntimeControlState::Active => {
            "Session resumed. New requests can proceed under its policy."
        }
        RuntimeControlState::Held => "Session resumed. Ghostlight is still paused.",
        RuntimeControlState::Ended => "Session resumed. Ghostlight is still stopped.",
        RuntimeControlState::Attention => "Session resumed. Ghostlight still needs your attention.",
    }
}

/// A reviewed incident has already cleared or been replaced.
pub const STALE_REVIEW: &str = "This attention request is no longer current.";
/// The session that needed review has ended.
pub const SESSION_GONE: &str = "This session has ended.";
/// A separate observation failure never makes repeating the completed action safe.
pub const APPLIED_BEFORE_CHECK_FAILURE: &str =
    "Keep the confirmed change. Inspect the page before preparing unfinished work.";

/// Explain scoped attention without claiming every session has stopped.
pub fn sessions_needing_review(count: usize) -> String {
    if count == 1 {
        "One session needs your review. Other sessions can continue.".into()
    } else {
        format!("{count} sessions need your review. Other sessions can continue.")
    }
}
