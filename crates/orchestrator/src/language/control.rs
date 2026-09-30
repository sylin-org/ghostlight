//! Human guidance following a completed action and an unsuccessful observation.

/// A separate observation failure never makes repeating the completed action safe.
pub const APPLIED_BEFORE_CHECK_FAILURE: &str =
    "Keep the confirmed change. Inspect the page before preparing unfinished work.";
