// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Same-user Unix session discovery for processes launched with a scrubbed environment.
//!
//! Chromium may clear the environment inherited by a native-messaging host. On Linux that removes
//! both `XDG_RUNTIME_DIR`, which [`dirs::runtime_dir`] needs, and the D-Bus address used by
//! `systemctl --user`. The service still owns its socket under `/run/user/<uid>`, so a relay that
//! falls through to the user cache silently resolves a different endpoint. This module provides
//! one security-checked fallback for both endpoint resolution and supervisor self-heal.

#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
const XDG_RUNTIME_DIR_ENV: &str = "XDG_RUNTIME_DIR";
#[cfg(target_os = "linux")]
const DBUS_SESSION_BUS_ADDRESS_ENV: &str = "DBUS_SESSION_BUS_ADDRESS";

#[cfg(target_os = "linux")]
const LINUX_RUN_USER_ROOT: &str = "/run/user";

/// Resolve the current user's runtime directory.
///
/// The standard environment-backed resolver stays authoritative. Linux adds a fallback to
/// `/run/user/<effective-uid>` only when that directory is owned by the effective user and has no
/// group or other permission bits. Other Unix platforms retain their existing cache fallback in
/// the caller.
pub(crate) fn runtime_dir() -> Option<PathBuf> {
    dirs::runtime_dir().or_else(platform_runtime_dir)
}

#[cfg(target_os = "linux")]
fn platform_runtime_dir() -> Option<PathBuf> {
    // SAFETY: `geteuid` has no arguments, pointers, or caller preconditions. It reads the kernel's
    // effective user id for this process, which is the identity the filesystem permission check
    // below must match.
    let effective_uid = unsafe { libc::geteuid() };
    secure_linux_runtime_dir(Path::new(LINUX_RUN_USER_ROOT), effective_uid)
}

#[cfg(not(target_os = "linux"))]
fn platform_runtime_dir() -> Option<PathBuf> {
    None
}

/// Complete the missing Linux user-session variables for a child command.
///
/// Existing values always win. A discovered runtime directory supplies `XDG_RUNTIME_DIR` and the
/// conventional systemd user-bus address only when each value is absent. Non-Linux Unix platforms
/// intentionally remain unchanged.
pub(crate) fn complete_command_environment(command: &mut std::process::Command) {
    #[cfg(target_os = "linux")]
    {
        let existing_runtime = std::env::var_os(XDG_RUNTIME_DIR_ENV);
        let existing_bus = std::env::var_os(DBUS_SESSION_BUS_ADDRESS_ENV);
        let discovered = runtime_dir();
        complete_command_environment_from(
            command,
            existing_runtime.as_deref(),
            existing_bus.as_deref(),
            discovered.as_deref(),
        );
    }

    #[cfg(not(target_os = "linux"))]
    let _ = command;
}

#[cfg(target_os = "linux")]
fn secure_linux_runtime_dir(root: &Path, effective_uid: u32) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;

    let candidate = root.join(effective_uid.to_string());
    let metadata = std::fs::symlink_metadata(&candidate).ok()?;
    let private_mode = metadata.mode() & 0o077 == 0;
    (metadata.file_type().is_dir() && metadata.uid() == effective_uid && private_mode)
        .then_some(candidate)
}

#[cfg(target_os = "linux")]
fn complete_command_environment_from(
    command: &mut std::process::Command,
    existing_runtime: Option<&std::ffi::OsStr>,
    existing_bus: Option<&std::ffi::OsStr>,
    runtime: Option<&Path>,
) {
    let Some(runtime) = runtime else {
        return;
    };
    if existing_runtime.is_none() {
        command.env(XDG_RUNTIME_DIR_ENV, runtime);
    }
    if existing_bus.is_none() {
        command.env(
            DBUS_SESSION_BUS_ADDRESS_ENV,
            format!("unix:path={}", runtime.join("bus").display()),
        );
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-user-session-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn linux_runtime_fallback_requires_private_user_owned_directory() {
        let root = temp_root("secure");
        std::fs::create_dir_all(&root).unwrap();
        let uid = std::fs::metadata(&root).unwrap().uid();
        let candidate = root.join(uid.to_string());
        std::fs::create_dir(&candidate).unwrap();
        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(
            secure_linux_runtime_dir(&root, uid),
            Some(candidate.clone())
        );

        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o750)).unwrap();
        assert_eq!(secure_linux_runtime_dir(&root, uid), None);

        std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mismatched_uid = uid.wrapping_add(1);
        let ownership_mismatch = root.join(mismatched_uid.to_string());
        std::fs::create_dir(&ownership_mismatch).unwrap();
        std::fs::set_permissions(&ownership_mismatch, std::fs::Permissions::from_mode(0o700))
            .unwrap();
        assert_eq!(std::fs::metadata(&ownership_mismatch).unwrap().uid(), uid);
        assert_eq!(secure_linux_runtime_dir(&root, mismatched_uid), None);

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn linux_runtime_fallback_rejects_a_symlink() {
        let root = temp_root("symlink");
        let target = temp_root("symlink-target");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        let uid = std::fs::metadata(&root).unwrap().uid();
        symlink(&target, root.join(uid.to_string())).unwrap();

        assert_eq!(secure_linux_runtime_dir(&root, uid), None);

        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&target).unwrap();
    }

    #[test]
    fn command_environment_fills_only_missing_session_values() {
        let runtime = Path::new("/run/user/4242");
        let mut missing = std::process::Command::new("systemctl");
        complete_command_environment_from(&mut missing, None, None, Some(runtime));
        let missing_env: std::collections::HashMap<_, _> = missing
            .get_envs()
            .map(|(key, value)| (key.to_os_string(), value.map(std::ffi::OsStr::to_os_string)))
            .collect();
        assert_eq!(
            missing_env.get(std::ffi::OsStr::new(XDG_RUNTIME_DIR_ENV)),
            Some(&Some(runtime.as_os_str().to_os_string()))
        );
        assert_eq!(
            missing_env.get(std::ffi::OsStr::new(DBUS_SESSION_BUS_ADDRESS_ENV)),
            Some(&Some(std::ffi::OsString::from(
                "unix:path=/run/user/4242/bus"
            )))
        );

        let mut existing = std::process::Command::new("systemctl");
        complete_command_environment_from(
            &mut existing,
            Some(std::ffi::OsStr::new("/custom/runtime")),
            Some(std::ffi::OsStr::new("unix:path=/custom/bus")),
            Some(runtime),
        );
        assert_eq!(existing.get_envs().count(), 0);
    }
}
