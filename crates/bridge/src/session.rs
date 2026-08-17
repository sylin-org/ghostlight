//! Security-checked local graphical-session discovery for child processes.
//!
//! Linux relays and the service may start with a scrubbed environment. This seam applies the
//! ADR-0082 same-user runtime-directory rule and returns the verified values a child needs.
//! It never guesses a graphical display: a launch is available only when the current service
//! already carries a Wayland or X11 display value.

use std::ffi::{OsStr, OsString};
use std::io;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

/// Environment values required to place a child in the current graphical user session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphicalSessionEnvironment {
    values: Vec<(OsString, OsString)>,
}

impl GraphicalSessionEnvironment {
    /// Values to overlay on a child command without clearing its inherited environment.
    pub fn values(&self) -> impl Iterator<Item = (&OsStr, &OsStr)> {
        self.values
            .iter()
            .map(|(key, value)| (key.as_os_str(), value.as_os_str()))
    }
}

/// Resolve the current same-user graphical session where the platform can prove one.
///
/// Linux requires a display value plus a real, owner-private runtime directory owned by the
/// effective process user. A missing XDG runtime directory may use `/run/user/<uid>` under the
/// same checks. Other platforms need no environment overlay.
///
/// # Errors
///
/// Returns an I/O error when Linux cannot inspect its process identity or candidate runtime path.
pub fn graphical_session_environment() -> io::Result<Option<GraphicalSessionEnvironment>> {
    #[cfg(target_os = "linux")]
    {
        linux_graphical_session_environment()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(Some(GraphicalSessionEnvironment { values: Vec::new() }))
    }
}

#[cfg(target_os = "linux")]
fn linux_graphical_session_environment() -> io::Result<Option<GraphicalSessionEnvironment>> {
    use std::os::unix::fs::MetadataExt as _;

    let display = std::env::var_os("WAYLAND_DISPLAY")
        .map(|value| (OsString::from("WAYLAND_DISPLAY"), value))
        .or_else(|| std::env::var_os("DISPLAY").map(|value| (OsString::from("DISPLAY"), value)));
    let Some(display) = display else {
        return Ok(None);
    };

    let effective_uid = std::fs::metadata("/proc/self")?.uid();
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{effective_uid}")));
    if !secure_runtime_directory(&runtime, effective_uid)? {
        return Ok(None);
    }

    let mut values = vec![(OsString::from("XDG_RUNTIME_DIR"), runtime.clone().into())];
    values.push((
        OsString::from("DBUS_SESSION_BUS_ADDRESS"),
        std::env::var_os("DBUS_SESSION_BUS_ADDRESS").unwrap_or_else(|| {
            OsString::from(format!("unix:path={}", runtime.join("bus").display()))
        }),
    ));
    values.push(display);
    Ok(Some(GraphicalSessionEnvironment { values }))
}

#[cfg(target_os = "linux")]
fn secure_runtime_directory(path: &Path, effective_uid: u32) -> io::Result<bool> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    let metadata = std::fs::symlink_metadata(path)?;
    Ok(metadata.file_type().is_dir()
        && !metadata.file_type().is_symlink()
        && metadata.uid() == effective_uid
        && metadata.permissions().mode() & 0o077 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_launch_environment_is_an_overlay_not_a_scrub() {
        let environment = GraphicalSessionEnvironment {
            values: vec![(OsString::from("DISPLAY"), OsString::from(":1"))],
        };
        assert_eq!(
            environment.values().collect::<Vec<_>>(),
            vec![(OsStr::new("DISPLAY"), OsStr::new(":1"))]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_runtime_directory_must_be_real_private_and_same_user() {
        use std::os::unix::fs::{symlink, MetadataExt as _, PermissionsExt as _};

        let root = std::env::temp_dir().join(format!(
            "ghostlight-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let uid = std::fs::metadata(&root).unwrap().uid();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(secure_runtime_directory(&root, uid).unwrap());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o750)).unwrap();
        assert!(!secure_runtime_directory(&root, uid).unwrap());
        assert!(!secure_runtime_directory(&root, uid.saturating_add(1)).unwrap());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = root.with_extension("link");
        symlink(&root, &link).unwrap();
        assert!(!secure_runtime_directory(&link, uid).unwrap());
        std::fs::remove_file(link).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
