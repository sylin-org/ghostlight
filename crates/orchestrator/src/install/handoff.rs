//! One-time handoff between the installed native service and the browser extension.
//!
//! Chromium deliberately requires a person to approve extension installation. A successful
//! native install therefore opens one stable Ghostlight page once, while scripts, CI, dry runs,
//! and repeated installs remain non-interactive. The marker is local, version-family state and
//! carries no installation identity or machine data off-device.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Stable service-first page that points at the current store extension.
pub const EXTENSION_INSTALL_URL: &str = "https://sylin.org/ghostlight/service/post-install/";

const HANDOFF_MARKER: &str = "extension-handoff-v1";

/// Result of considering the browser-extension handoff after installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoffOutcome {
    /// The operating system accepted a request to open the walkthrough.
    Opened,
    /// This user has already been offered the walkthrough for Ghostlight 1.x.
    AlreadyOffered,
    /// The caller deliberately requested a non-interactive run.
    Suppressed,
}

/// Decide whether this install run should offer the browser-extension walkthrough.
#[must_use]
pub fn should_offer(
    dry_run: bool,
    no_open: bool,
    automated: bool,
    install_usable: bool,
    already_offered: bool,
) -> bool {
    !dry_run && !no_open && !automated && install_usable && !already_offered
}

/// Offer the extension walkthrough once for the current user.
pub fn offer(
    dry_run: bool,
    no_open: bool,
    automated: bool,
    install_usable: bool,
) -> io::Result<HandoffOutcome> {
    let marker = marker_path(&state_root()?);
    let already_offered = marker.exists();
    if !should_offer(dry_run, no_open, automated, install_usable, already_offered) {
        return Ok(if already_offered {
            HandoffOutcome::AlreadyOffered
        } else {
            HandoffOutcome::Suppressed
        });
    }

    if !reserve_marker(&marker)? {
        return Ok(HandoffOutcome::AlreadyOffered);
    }
    if let Err(error) = open_walkthrough() {
        let _ = fs::remove_file(&marker);
        return Err(error);
    }
    Ok(HandoffOutcome::Opened)
}

fn state_root() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("USERPROFILE")
                    .map(PathBuf::from)
                    .map(|home| home.join("AppData/Local"))
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no local data directory"));
    }
    #[cfg(target_os = "linux")]
    {
        env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".local/state"))
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no user state directory"))
    }
}

fn marker_path(state_root: &Path) -> PathBuf {
    state_root.join("ghostlight").join(HANDOFF_MARKER)
}

fn reserve_marker(path: &Path) -> io::Result<bool> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "handoff marker has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut marker) => {
            let written = marker
                .write_all(EXTENSION_INSTALL_URL.as_bytes())
                .and_then(|()| marker.write_all(b"\n"))
                .and_then(|()| marker.sync_all());
            if let Err(error) = written {
                drop(marker);
                let _ = fs::remove_file(path);
                return Err(error);
            }
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

fn browser_command() -> (&'static str, Vec<&'static str>) {
    #[cfg(target_os = "windows")]
    {
        return (
            "rundll32.exe",
            vec!["url.dll,FileProtocolHandler", EXTENSION_INSTALL_URL],
        );
    }
    #[cfg(target_os = "linux")]
    {
        ("xdg-open", vec![EXTENSION_INSTALL_URL])
    }
}

fn open_walkthrough() -> io::Result<()> {
    let (program, arguments) = browser_command();
    Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{browser_command, marker_path, should_offer, EXTENSION_INSTALL_URL};

    #[test]
    fn handoff_requires_a_first_usable_interactive_install() {
        assert!(should_offer(false, false, false, true, false));
        assert!(!should_offer(true, false, false, true, false));
        assert!(!should_offer(false, true, false, true, false));
        assert!(!should_offer(false, false, true, true, false));
        assert!(!should_offer(false, false, false, false, false));
        assert!(!should_offer(false, false, false, true, true));
    }

    #[test]
    fn marker_is_private_to_ghostlight_user_state() {
        assert_eq!(
            marker_path(Path::new("/state")),
            Path::new("/state/ghostlight/extension-handoff-v1")
        );
    }

    #[test]
    fn platform_browser_command_uses_the_service_first_page() {
        let (_, arguments) = browser_command();
        assert!(arguments.contains(&EXTENSION_INSTALL_URL));
        assert!(EXTENSION_INSTALL_URL.contains("/service/post-install/"));
    }
}
