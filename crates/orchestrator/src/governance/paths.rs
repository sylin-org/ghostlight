// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Where Ghostlight keeps the policy files it owns on this machine.
//!
//! One policy directory, `home/policy` in the Ghostlight home, so the verified managed cache, the
//! managed status sidecar, and the user policy the workbench authors cannot drift onto different
//! roots (ADR-0197 Decision 10). The home's layout names every one of those files, and the 2.0
//! migration moves a 1.x file to the same name.

use std::path::PathBuf;

use crate::install::layout::Layout;

/// The user policy path Ghostlight owns and the workbench may write.
///
/// This is the only file the window writes. A policy named by `GHOSTLIGHT_POLICY_FILE` belongs to
/// whoever set that variable, and Ghostlight reads it without ever writing back (ADR-0122 D4).
#[must_use]
pub fn user_policy_path() -> Option<PathBuf> {
    Layout::current().ok().map(|layout| layout.user_policy())
}
