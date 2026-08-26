// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Where Ghostlight keeps the policy files it owns on this machine.
//!
//! One home for the per-user state directory, so the verified managed cache, the managed status
//! sidecar, and the user policy the workbench authors cannot drift onto different roots.

use std::env;
use std::path::PathBuf;

/// File name of the user policy the workbench owns.
const USER_POLICY_FILE: &str = "user-policy.json";

/// The per-user state directory, when this environment names one.
#[must_use]
pub(super) fn state_directory() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Ghostlight"))
    }
    #[cfg(target_os = "linux")]
    {
        env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("state"))
            })
            .map(|path| path.join("ghostlight"))
    }
}

/// The user policy path Ghostlight owns and the workbench may write.
///
/// This is the only file the window writes. A policy named by `GHOSTLIGHT_POLICY_FILE` belongs to
/// whoever set that variable, and Ghostlight reads it without ever writing back (ADR-0122 D4).
#[must_use]
pub fn user_policy_path() -> Option<PathBuf> {
    state_directory().map(|path| path.join(USER_POLICY_FILE))
}
