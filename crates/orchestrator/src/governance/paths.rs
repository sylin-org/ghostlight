// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Where Ghostlight keeps the policy files it owns on this machine.
//!
//! One policy directory, `home/policy` in the Ghostlight home, so the verified managed cache, the
//! managed status sidecar, and the user policy the workbench authors cannot drift onto different
//! roots (ADR-0197 Decision 10).

use std::path::PathBuf;

use crate::install::layout::Layout;

/// File name of the user policy the workbench owns.
const USER_POLICY_FILE: &str = "user-policy.json";

/// The policy directory in the Ghostlight home, when the home can be resolved.
#[must_use]
pub(super) fn policy_directory() -> Option<PathBuf> {
    Layout::current().ok().map(|layout| layout.policy())
}

/// The user policy path Ghostlight owns and the workbench may write.
///
/// This is the only file the window writes. A policy named by `GHOSTLIGHT_POLICY_FILE` belongs to
/// whoever set that variable, and Ghostlight reads it without ever writing back (ADR-0122 D4).
#[must_use]
pub fn user_policy_path() -> Option<PathBuf> {
    policy_directory().map(|path| path.join(USER_POLICY_FILE))
}
