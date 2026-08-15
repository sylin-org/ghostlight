//! Shared local-process lifecycle for the one Ghostlight engine.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use fs2::FileExt;

use crate::runtime::runtime_file;

const DEPLOY_LOCK_FILE: &str = "deploy.lock";
const DEPLOY_LOCK_MAX_AGE: Duration = Duration::from_secs(30 * 60);
const SERVICE_LOCK_EXTENSION: &str = "lock";

#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg(windows)]
const ERROR_LOCK_VIOLATION: i32 = 33;

/// Exclusive lifetime lease for the one orchestrator that may publish a runtime endpoint.
#[derive(Debug)]
pub struct ServiceLease {
    _file: File,
}

impl ServiceLease {
    /// Try to acquire the service lease associated with one runtime-discovery file.
    ///
    /// `Ok(None)` means another process currently owns the engine. The operating system releases
    /// the lock if that process exits, including after an unclean termination.
    pub fn try_acquire(runtime_path: &Path) -> io::Result<Option<Self>> {
        let lock_path = service_lock_file(runtime_path);
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock_path)?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(error) if lock_is_contended(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn lock_is_contended(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::WouldBlock {
        return true;
    }
    #[cfg(windows)]
    {
        error.raw_os_error() == Some(ERROR_LOCK_VIOLATION)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Result of asking the local lifecycle seam to make the orchestrator available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartDisposition {
    /// A detached sibling orchestrator was started.
    Spawned {
        /// Operating-system process id returned by the spawn.
        process_id: u32,
    },
    /// A service lease is already held, so the caller should keep reconnecting.
    AlreadyRunning,
    /// A fresh deployment lock is deliberately quiescing automatic startup.
    DeploymentInProgress,
}

/// Ask the trusted sibling `ghostlight` executable to start its desktop authority.
///
/// Callers invoke this only after a connection attempt fails. The service lease makes concurrent
/// requests harmless, and a fresh deploy lock suppresses self-heal while binaries are swapped.
pub fn request_orchestrator_start() -> io::Result<StartDisposition> {
    let current_executable = env::current_exe()?;
    request_orchestrator_start_from(&current_executable, &runtime_file(), SystemTime::now())
}

fn request_orchestrator_start_from(
    current_executable: &Path,
    runtime_path: &Path,
    now: SystemTime,
) -> io::Result<StartDisposition> {
    let service_executable = orchestrator_executable(current_executable)?;
    let service_directory = service_executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "orchestrator executable has no parent directory",
        )
    })?;
    if deploy_lock_present(service_directory, now)? {
        return Ok(StartDisposition::DeploymentInProgress);
    }

    let Some(lease) = ServiceLease::try_acquire(runtime_path)? else {
        return Ok(StartDisposition::AlreadyRunning);
    };
    // The child must acquire the lifetime lease itself. Releasing immediately before spawn leaves
    // a narrow benign race: concurrent children may start, but only one can pass ServiceHost::start.
    drop(lease);

    if !service_executable.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "trusted sibling orchestrator is missing: {}",
                service_executable.display()
            ),
        ));
    }
    let mut command = orchestrator_command(&service_executable, service_directory);
    let child = command.spawn()?;
    let process_id = child.id();
    drop(child);
    Ok(StartDisposition::Spawned { process_id })
}

fn orchestrator_command(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_detached(&mut command);
    command
}

fn orchestrator_executable(current_executable: &Path) -> io::Result<PathBuf> {
    let directory = current_executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "connector executable has no parent directory",
        )
    })?;
    Ok(directory.join(if cfg!(windows) {
        "ghostlight.exe"
    } else {
        "ghostlight"
    }))
}

fn service_lock_file(runtime_path: &Path) -> PathBuf {
    runtime_path.with_extension(SERVICE_LOCK_EXTENSION)
}

fn deploy_lock_present(directory: &Path, now: SystemTime) -> io::Result<bool> {
    let path = directory.join(DEPLOY_LOCK_FILE);
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(lock_is_fresh(metadata.modified()?, now))
}

fn lock_is_fresh(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .map_or(true, |age| age <= DEPLOY_LOCK_MAX_AGE)
}

#[cfg(windows)]
fn configure_detached(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(target_os = "linux")]
fn configure_detached(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(any(windows, target_os = "linux")))]
fn configure_detached(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use super::{
        deploy_lock_present, lock_is_fresh, orchestrator_command, orchestrator_executable,
        service_lock_file, ServiceLease, DEPLOY_LOCK_FILE, DEPLOY_LOCK_MAX_AGE,
    };

    fn temporary_directory(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-lifecycle-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("clock follows epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("temporary directory is created");
        path
    }

    #[test]
    fn service_lease_is_exclusive_and_released_on_drop() {
        let directory = temporary_directory("lease");
        let runtime = directory.join("ghostlight-runtime.json");
        let first = ServiceLease::try_acquire(&runtime)
            .expect("first lease attempt succeeds")
            .expect("first lease is acquired");
        assert!(ServiceLease::try_acquire(&runtime)
            .expect("second lease attempt is decisive")
            .is_none());
        drop(first);
        assert!(ServiceLease::try_acquire(&runtime)
            .expect("released lease can be acquired")
            .is_some());
        fs::remove_dir_all(directory).expect("temporary directory is removed");
    }

    #[test]
    fn service_lock_sits_beside_runtime_discovery() {
        assert_eq!(
            service_lock_file(Path::new("engine/ghostlight-runtime.json")),
            Path::new("engine/ghostlight-runtime.lock")
        );
    }

    #[test]
    fn sibling_orchestrator_name_is_platform_exact() {
        let connector = Path::new("engine").join(if cfg!(windows) {
            "ghostlight-mcp-connector.exe"
        } else {
            "ghostlight-mcp-connector"
        });
        let expected = Path::new("engine").join(if cfg!(windows) {
            "ghostlight.exe"
        } else {
            "ghostlight"
        });
        assert_eq!(
            orchestrator_executable(&connector).expect("connector has a parent"),
            expected
        );
    }

    #[test]
    fn demand_start_uses_the_one_normal_desktop_launch() {
        let executable = Path::new("engine").join(if cfg!(windows) {
            "ghostlight.exe"
        } else {
            "ghostlight"
        });
        let command = orchestrator_command(&executable, Path::new("engine"));
        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(command.get_args().count(), 0);
    }

    #[test]
    fn deployment_lock_is_fresh_only_inside_the_quiesce_window() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
        assert!(lock_is_fresh(now, now));
        assert!(lock_is_fresh(
            now - DEPLOY_LOCK_MAX_AGE + Duration::from_secs(1),
            now
        ));
        assert!(!lock_is_fresh(
            now - DEPLOY_LOCK_MAX_AGE - Duration::from_secs(1),
            now
        ));
        assert!(lock_is_fresh(now + Duration::from_secs(1), now));
    }

    #[test]
    fn a_fresh_deploy_lock_suppresses_startup() {
        let directory = temporary_directory("deploy");
        fs::write(directory.join(DEPLOY_LOCK_FILE), b"deploy").expect("deploy lock is written");
        assert!(
            deploy_lock_present(&directory, SystemTime::now()).expect("deploy lock check succeeds")
        );
        fs::remove_dir_all(directory).expect("temporary directory is removed");
    }

    #[test]
    fn both_connectors_recover_through_the_shared_lifecycle_seam() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("bridge has a workspace crates directory");
        for relative in [
            "mcp-connector/src/service_session.rs",
            "browser-connector/src/main.rs",
        ] {
            let source = fs::read_to_string(workspace.join(relative))
                .expect("connector source is available");
            assert!(source.contains("request_orchestrator_start()"));
            assert!(!source.contains("Command::new"));
        }
    }
}
