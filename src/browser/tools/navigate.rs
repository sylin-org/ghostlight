//! `navigate` -- navigate to a URL, or `back`/`forward` in history. **Observe** tier.
//!
//! In the v1.5 overlay this is the primary domain-enforcement point (pre- and post-commit checks
//! against the committed origin from [`crate::origin`]). In v1.0 it navigates unconditionally.
//! Implemented in Phase 2.
