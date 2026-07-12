// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The OS supervisor identifiers + best-effort self-heal start (ADR-0030 Decision 8 amendment;
//! PINS.md SS5.2). The installer (H9) registers a per-user, zero-admin OS supervisor under these
//! SAME names (Windows Task Scheduler; macOS launchd; Linux systemd --user) that keeps
//! `ghostlight service` warm and restarts it on crash. When a thin ADAPTER's first dial to the
//! service fails, it asks this SAME supervisor to start the service (idempotent, out-of-job)
//! before retrying the dial -- never spawning an in-job child itself (that mechanism is deleted;
//! ADR-0030 Decision 8 Provenance, "the always-ready-service amendment").

use crate::role;
use std::time::Duration;

/// Windows Task Scheduler task name for the active instance (ADR-0044). The default instance
/// yields `Ghostlight Service` (the PINNED name H9 registers, PINS.md SS5.2); a named instance
/// yields `Ghostlight Service (<n>)`.
pub fn supervisor_task_name() -> String {
    crate::instance::Instance::resolve().supervisor_task_name()
}

/// macOS launchd label for the active instance (ADR-0044). The default instance yields
/// `org.sylin.ghostlight.service` (the PINNED label, PINS.md SS5.2); a named instance yields
/// `org.sylin.ghostlight.<n>.service`.
pub fn supervisor_label() -> String {
    crate::instance::Instance::resolve().supervisor_label()
}

/// Linux systemd --user unit for the active instance (ADR-0044). The default instance yields
/// `ghostlight.service` (the PINNED unit, PINS.md SS5.2); a named instance yields
/// `ghostlight-<n>.service`.
pub fn supervisor_unit() -> String {
    crate::instance::Instance::resolve().supervisor_unit()
}

/// Self-heal retry window (PINNED, PINS.md SS5.2): after asking the supervisor to start the
/// service, the adapter retries its dial for up to this long before giving up.
pub const SELF_HEAL_RETRY_WINDOW: Duration = Duration::from_secs(3);

/// Self-heal retry interval (PINNED, PINS.md SS5.2): how often the adapter retries its dial
/// within [`SELF_HEAL_RETRY_WINDOW`].
pub const SELF_HEAL_RETRY_INTERVAL: Duration = Duration::from_millis(200);

/// The pinned self-heal failure message (PINS.md SS5.2), logged verbatim when the retry window
/// elapses with the service still unreachable.
pub const SELF_HEAL_FAILURE_MESSAGE: &str = "the Ghostlight service is not running and could not be started automatically; start it with 'ghostlight service' (or reinstall to enable auto-start)";

/// The Windows per-user autostart registry path under HKEY_CURRENT_USER (ADR-0054 Decision 1):
/// the one logon-start mechanism a non-admin user can always write. The value NAME is
/// [`supervisor_task_name`] (unchanged identity); the data is `"<exe>" service`.
pub const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Detached-spawn creation flags for the Windows self-heal (ADR-0054 Decision 2):
/// DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP. Paired with null stdio, this is structurally
/// distinct from the in-job, stdio-inheriting child ADR-0030 D8 deleted.
pub const DETACHED_SPAWN_FLAGS: u32 = 0x0000_0008 | 0x0000_0200;

/// The deploy-quiesce lock file name (ADR-0063), looked up NEXT TO the service executable
/// `start_service` is about to spawn. While it exists (and is fresh), the self-heal holds off, so a
/// deploy can kill and replace the binary without the self-heal racing the swap.
pub const DEPLOY_LOCK_NAME: &str = "deploy.lock";

/// How long a [`DEPLOY_LOCK_NAME`] lock is honored (ADR-0063). Far longer than any real deploy, but
/// finite so a crashed deploy that left the file behind never permanently disables self-heal.
pub const DEPLOY_LOCK_MAX_AGE: Duration = Duration::from_secs(30 * 60);

/// True if a fresh deploy-quiesce lock (ADR-0063) sits next to `service_exe`. Scoped to the exe's
/// DIRECTORY -- a deploy replaces the binaries in ONE directory (a build's `target/release`, an
/// install's `bin/<version>`), and the lock lives there, so it quiesces the self-heal for exactly
/// the binaries being swapped and nothing else. A lock older than [`DEPLOY_LOCK_MAX_AGE`] is treated
/// as stale (ignored). Any filesystem error (no lock, unreadable) reads as "not locked" -- self-heal
/// is best-effort, so it fails OPEN, never wedging on a lock it cannot stat.
fn deploy_lock_present(service_exe: &std::path::Path) -> bool {
    let Some(lock) = service_exe.parent().map(|d| d.join(DEPLOY_LOCK_NAME)) else {
        return false;
    };
    match std::fs::metadata(&lock).and_then(|m| m.modified()) {
        // A future/again-clock-skewed mtime (`elapsed()` errs) is treated as fresh: honor the lock.
        Ok(modified) => modified
            .elapsed()
            .map_or(true, |age| age < DEPLOY_LOCK_MAX_AGE),
        Err(_) => false,
    }
}

/// Resolve the SIBLING service executable for a running relay (ADR-0054 Decision 2): the role
/// executables ship side by side (ADR-0046/ADR-0051), so `<dir>/ghostlight-relay*[.exe]` maps to
/// `<dir>/ghostlight[.exe]`. Pure: unit-tested against paths, never touches the filesystem.
pub fn sibling_service_exe(relay_exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let dir = relay_exe.parent()?;
    let name = match relay_exe.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("ghostlight.{ext}"),
        None => "ghostlight".to_string(),
    };
    Some(dir.join(name))
}

/// Spawn `<exe> [--instance <n>] service` fully detached (ADR-0054 Decision 2): null stdio and
/// [`DETACHED_SPAWN_FLAGS`], so the child inherits neither the caller's MCP pipes nor its console
/// or process group -- the hazards that killed the old in-job spawn are structurally absent. The
/// service's own singleton endpoint claim makes concurrent spawns harmless (losers exit). No role
/// assertion here: the installer's start-once step (CLI role) shares this helper; the self-heal
/// wrapper [`start_service`] carries the adapter-role assertion.
#[cfg(windows)]
pub fn spawn_service_detached(exe: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new(exe);
    if let Some(n) = crate::instance::Instance::resolve().name() {
        cmd.args(["--instance", n]);
    }
    cmd.arg("service")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(DETACHED_SPAWN_FLAGS);
    cmd.spawn().map(|_| ())
}

/// The pure program+args to idempotently (re)start the registered supervisor unit (PINS.md SS5.2;
/// Unix only since ADR-0054 -- the Windows self-heal spawns the service directly, see
/// [`spawn_service_detached`]). NEVER executed by this function -- see [`start_service`] -- so it
/// stays unit-testable as a pure string. macOS variant:
#[cfg(target_os = "macos")]
pub fn supervisor_start_command() -> Option<(String, Vec<String>)> {
    Some((
        "launchctl".to_string(),
        vec![
            "kickstart".to_string(),
            "-k".to_string(),
            format!("gui/{}/{}", unsafe { libc::getuid() }, supervisor_label()),
        ],
    ))
}

/// See the Windows doc above; Linux (non-macOS Unix) variant (PINS.md SS5.2).
#[cfg(all(unix, not(target_os = "macos")))]
pub fn supervisor_start_command() -> Option<(String, Vec<String>)> {
    Some((
        "systemctl".to_string(),
        vec!["--user".to_string(), "start".to_string(), supervisor_unit()],
    ))
}

/// Best-effort start the service when an adapter's dial fails (ADR-0030 Decision 8; amended by
/// ADR-0054): a hint, not a guarantee; the adapter's own bounded dial retry
/// (`ipc::relay_adapter`), not this call, decides whether the service ever came up. On Windows it
/// spawns the sibling service executable detached (a Run key cannot be "run" on demand, and the
/// old schtasks handle required an elevation-only registration -- issue #17); on Unix it asks the
/// registered launchd/systemd supervisor as before. Asserts the ADAPTER role first (PINS.md SS8):
/// a SERVICE must never trigger a service start (that would mean the SoC boundary already failed
/// elsewhere).
pub fn start_service() {
    role::assert_adapter_role("start_service");

    #[cfg(windows)]
    {
        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| sibling_service_exe(&p));
        match exe {
            // ADR-0063: hold off self-healing while a deploy owns this binary, so the swap is not
            // raced by relaunching the OLD image.
            Some(exe) if deploy_lock_present(&exe) => tracing::info!(
                exe = %exe.display(),
                "deploy in progress (deploy.lock present); not self-healing the service"
            ),
            Some(exe) => match spawn_service_detached(&exe) {
                Ok(()) => tracing::info!(
                    exe = %exe.display(),
                    "spawned the Ghostlight service detached (self-heal)"
                ),
                Err(e) => tracing::debug!(
                    exe = %exe.display(),
                    error = %e,
                    "could not spawn the service detached (best-effort; ignored)"
                ),
            },
            None => tracing::debug!(
                "could not resolve the sibling service executable (best-effort; ignored)"
            ),
        }
    }

    #[cfg(not(windows))]
    {
        // ADR-0063: the deploy-quiesce lock holds off self-heal on EVERY platform. The Unix
        // self-heal goes through the OS supervisor rather than a direct spawn, but the lock's
        // meaning is identical -- a deploy owns the sibling binaries; do not relaunch the old
        // image mid-swap. (Until v0.5.6 only the Windows branch checked it.)
        let deploying = std::env::current_exe()
            .ok()
            .and_then(|p| sibling_service_exe(&p))
            .is_some_and(|exe| deploy_lock_present(&exe));
        if deploying {
            tracing::info!(
                "deploy in progress (deploy.lock present); not self-healing the service"
            );
            return;
        }
        let Some((program, args)) = supervisor_start_command() else {
            tracing::debug!("no OS supervisor mechanism on this platform; nothing to start");
            return;
        };
        match std::process::Command::new(&program).args(&args).status() {
            Ok(status) if status.success() => {
                tracing::info!(
                    program,
                    "asked the OS supervisor to start the Ghostlight service"
                );
            }
            Ok(status) => tracing::debug!(
                program,
                code = ?status.code(),
                "OS supervisor start command exited non-zero (best-effort; ignored)"
            ),
            Err(e) => tracing::debug!(
                program,
                error = %e,
                "could not run the OS supervisor start command (best-effort; ignored)"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The self-heal mechanism pins for the current platform (PINS.md SS5.2 as amended by
    /// ADR-0054): Unix pins the exact supervisor command (pure, never executed); Windows pins the
    /// sibling-exe resolution, the Run-key path, and the detached-spawn flags.
    #[test]
    fn self_heal_mechanism_is_pinned_for_this_platform() {
        #[cfg(windows)]
        {
            use std::path::Path;
            assert_eq!(
                sibling_service_exe(Path::new(r"C:\bin\ghostlight-relay.exe")),
                Some(Path::new(r"C:\bin\ghostlight.exe").to_path_buf()),
                "the relay's sibling is the bare service exe"
            );
            assert_eq!(
                sibling_service_exe(Path::new(r"C:\bin\ghostlight-relay-dev.exe")),
                Some(Path::new(r"C:\bin\ghostlight.exe").to_path_buf()),
                "a per-instance relay copy still maps to the ONE service exe (instance rides the flag)"
            );
            assert_eq!(
                RUN_KEY_PATH,
                r"Software\Microsoft\Windows\CurrentVersion\Run"
            );
            // DETACHED_PROCESS (0x8) | CREATE_NEW_PROCESS_GROUP (0x200): no console, no inherited
            // process group -- pinned so the hazard analysis in ADR-0054 D2 stays true.
            assert_eq!(DETACHED_SPAWN_FLAGS, 0x208);
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let (program, args) =
                supervisor_start_command().expect("systemd path exists on Linux CI");
            assert_eq!(program, "systemctl");
            assert_eq!(
                args,
                vec!["--user".to_string(), "start".to_string(), supervisor_unit()]
            );
        }
        #[cfg(target_os = "macos")]
        {
            let (program, args) =
                supervisor_start_command().expect("launchd path exists on macOS CI");
            assert_eq!(program, "launchctl");
            assert_eq!(args[0], "kickstart");
            assert_eq!(args[1], "-k");
            assert!(args[2].starts_with("gui/"));
            assert!(args[2].ends_with(supervisor_label().as_str()));
        }
    }

    #[test]
    fn sibling_resolution_handles_unix_style_names() {
        // Extension-less relays (Unix) map to the extension-less service binary.
        assert_eq!(
            sibling_service_exe(std::path::Path::new("/opt/gl/ghostlight-relay")),
            Some(std::path::PathBuf::from("/opt/gl/ghostlight"))
        );
    }

    #[test]
    fn self_heal_window_is_wider_than_its_own_retry_interval() {
        assert!(SELF_HEAL_RETRY_WINDOW > SELF_HEAL_RETRY_INTERVAL);
    }

    /// ADR-0063: the deploy-quiesce lock is honored ONLY when it exists next to the service exe and
    /// is fresh -- absent reads as "not locked" (self-heal proceeds), and a stale lock (a crashed
    /// deploy) is ignored so self-heal is never permanently disabled.
    #[test]
    fn deploy_lock_present_honors_fresh_ignores_stale_and_absent() {
        let dir = std::env::temp_dir().join(format!(
            "ghostlight-deploy-lock-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("ghostlight.exe");
        let lock = dir.join(DEPLOY_LOCK_NAME);

        // Absent -> not present (self-heal proceeds).
        let _ = std::fs::remove_file(&lock);
        assert!(!deploy_lock_present(&exe));

        // Fresh -> present (self-heal holds off).
        std::fs::write(&lock, b"").unwrap();
        assert!(deploy_lock_present(&exe));

        // Stale (older than the max age) -> ignored, so a crashed deploy self-recovers.
        let stale = std::time::SystemTime::now() - DEPLOY_LOCK_MAX_AGE - Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_modified(stale)
            .unwrap();
        assert!(!deploy_lock_present(&exe));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
