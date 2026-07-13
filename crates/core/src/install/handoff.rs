//! One-time handoff from the native installer to the user-visible browser extension.
//!
//! Chromium requires the user to install an extension. After an explicit first install,
//! this module opens Ghostlight's stable extension page once and always leaves a printable URL.
//! It carries no machine state off-device and is not part of the runtime path.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Result;

/// Stable public page that owns the current store link or honest manual-install fallback.
pub const EXTENSION_INSTALL_URL: &str = "https://sylin.org/ghostlight/service/post-install/";

const HANDOFF_MARKER_FILE: &str = "extension-handoff-v1";

/// Result of considering the browser-extension handoff after installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffOutcome {
    /// The extension page was opened and the one-time marker was written.
    Opened,
    /// This installation already offered the extension page.
    AlreadyOffered,
    /// The caller requested a quiet install, or the process is running under CI.
    Suppressed,
}

/// Decide whether an installer run should open the extension handoff.
pub fn should_open(
    dry_run: bool,
    no_open: bool,
    automated: bool,
    install_usable: bool,
    already_offered: bool,
) -> bool {
    !dry_run && !no_open && !automated && install_usable && !already_offered
}

pub(super) fn marker_path(local_data_dir: &Path) -> PathBuf {
    local_data_dir
        .join(ghostlight_transport::instance::Instance::resolve().dir_leaf())
        .join(HANDOFF_MARKER_FILE)
}

#[cfg(windows)]
fn browser_command() -> (&'static str, Vec<&'static str>) {
    (
        "rundll32.exe",
        vec!["url.dll,FileProtocolHandler", EXTENSION_INSTALL_URL],
    )
}

#[cfg(target_os = "macos")]
fn browser_command() -> (&'static str, Vec<&'static str>) {
    ("open", vec![EXTENSION_INSTALL_URL])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn browser_command() -> (&'static str, Vec<&'static str>) {
    ("xdg-open", vec![EXTENSION_INSTALL_URL])
}

/// Open the extension page once when the installation context permits it.
///
/// A launch failure is returned to the caller, which keeps installation successful and prints the
/// same URL for a manual open. The marker is written only after the OS accepts the launch.
pub fn offer(
    local_data_dir: &Path,
    dry_run: bool,
    no_open: bool,
    automated: bool,
    install_usable: bool,
) -> Result<HandoffOutcome> {
    let marker = marker_path(local_data_dir);
    let already_offered = marker.exists();
    if !should_open(dry_run, no_open, automated, install_usable, already_offered) {
        return Ok(if already_offered {
            HandoffOutcome::AlreadyOffered
        } else {
            HandoffOutcome::Suppressed
        });
    }

    let (program, args) = browser_command();
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    super::native_host::write_file_atomic(&marker, EXTENSION_INSTALL_URL)?;
    Ok(HandoffOutcome::Opened)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_requires_a_new_non_automated_usable_install() {
        assert!(should_open(false, false, false, true, false));
        assert!(!should_open(true, false, false, true, false));
        assert!(!should_open(false, true, false, true, false));
        assert!(!should_open(false, false, true, true, false));
        assert!(!should_open(false, false, false, false, false));
        assert!(!should_open(false, false, false, true, true));
    }

    #[test]
    fn platform_launcher_carries_the_exact_stable_url() {
        let (_program, args) = browser_command();
        assert!(args.contains(&EXTENSION_INSTALL_URL));
    }

    #[test]
    fn handoff_marker_lives_inside_the_instance_data_directory() {
        let root = Path::new("/local-data");
        assert_eq!(
            marker_path(root),
            root.join("ghostlight").join(HANDOFF_MARKER_FILE)
        );
    }
}
