//! Shared local-process lifecycle for the one Ghostlight engine.
//!
//! Demand-start launches the sibling of runtime discovery: beside this executable by default,
//! or in the directory an explicit `GHOSTLIGHT_RUNTIME_FILE` elects (ADR-0150).

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fs2::FileExt;

use crate::diagnostics::{event, Level, Sink};
use crate::parent_process::ParentProcess;
use crate::runtime::{read_runtime, runtime_discovery, RuntimeDiscovery, RuntimeEndpoint};

/// Presence marker used to quiesce demand-start during a sibling replacement.
pub const DEPLOY_LOCK_FILE: &str = "deploy.lock";
const DEPLOY_LOCK_MAX_AGE: Duration = Duration::from_secs(30 * 60);
const SERVICE_LOCK_EXTENSION: &str = "lock";
const STARTUP_LOCK_EXTENSION: &str = "startup";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const STARTUP_POLL: Duration = Duration::from_millis(25);
const FAILED_START_COOLDOWN: Duration = Duration::from_secs(5);
const PARENT_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg(windows)]
const ERROR_LOCK_VIOLATION: i32 = 33;

enum ConnectorExit {
    InputClosed(&'static str),
    ParentExited,
}

/// One ordered exit path for a connector's normal input closure and parent-death detector.
pub struct ConnectorShutdown {
    exit: mpsc::SyncSender<ConnectorExit>,
}

impl ConnectorShutdown {
    /// Start the connector exit coordinator and, where available, its parent-death detector.
    pub fn start(diagnostics: Arc<Sink>) -> io::Result<Self> {
        let (exit, requests) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("ghostlight-connector-shutdown".into())
            .spawn(move || {
                let Ok(reason) = requests.recv() else {
                    return;
                };
                let detail = match reason {
                    ConnectorExit::InputClosed(detail) => detail,
                    ConnectorExit::ParentExited => "spawning client process exited",
                };
                diagnostics.emit(event::PROCESS_EXITED, Level::Info, None, detail);
                std::process::exit(0);
            })?;

        if let Some(parent) = ParentProcess::capture() {
            let parent_exit = exit.clone();
            thread::Builder::new()
                .name("ghostlight-parent-watchdog".into())
                .spawn(move || loop {
                    thread::sleep(PARENT_POLL_INTERVAL);
                    if !parent.is_alive() {
                        let _ = parent_exit.send(ConnectorExit::ParentExited);
                        return;
                    }
                })?;
        }
        Ok(Self { exit })
    }

    /// Complete a normal connector input closure through the shared ordered exit path.
    pub fn finish(self, detail: &'static str) -> ! {
        self.exit
            .send(ConnectorExit::InputClosed(detail))
            .expect("connector exit coordinator remains available");
        loop {
            thread::park();
        }
    }
}

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
    /// A detached sibling was spawned and desktop-ready discovery became available.
    Spawned {
        /// Operating-system process id returned by the spawn.
        process_id: u32,
    },
    /// A service lease is already held, so the caller should keep reconnecting.
    AlreadyRunning,
    /// Another caller owns the bounded startup exchange; keep reconnecting.
    Starting,
    /// A recent failed startup is cooling down; keep reconnecting without spawning.
    RetryDeferred,
    /// A named OS activation request completed and desktop-ready discovery became available.
    ActivationRequested,
    /// A fresh deployment lock is deliberately quiescing automatic startup.
    DeploymentInProgress,
}

impl StartDisposition {
    /// Content-free operational event and detail shared by every lifecycle caller.
    pub fn diagnostic(&self) -> (&'static str, String) {
        use crate::diagnostics::event;
        match self {
            Self::Spawned { process_id } => (
                event::DEMAND_START_SPAWNED,
                format!("desktop ready after spawning pid {process_id}"),
            ),
            Self::AlreadyRunning => (
                event::DEMAND_START_ALREADY_RUNNING,
                "authority lease held; retrying connection".into(),
            ),
            Self::Starting => (
                event::DEMAND_START_PENDING,
                "startup exchange in progress; retrying connection".into(),
            ),
            Self::RetryDeferred => (
                event::DEMAND_START_RETRY_DEFERRED,
                "recent startup failed; five-second cooldown".into(),
            ),
            Self::ActivationRequested => (
                event::DEMAND_START_ACTIVATED,
                "desktop ready after OS activation".into(),
            ),
            Self::DeploymentInProgress => (
                event::DEMAND_START_DEPLOYMENT_IN_PROGRESS,
                "deploy lock present; startup quiesced".into(),
            ),
        }
    }
}

/// Ask the trusted sibling `ghostlight` executable to start its desktop authority.
///
/// Callers invoke this only after a connection attempt fails. The authority is the sibling of
/// the runtime discovery: beside this executable by default, or in the directory elected by an
/// explicit `GHOSTLIGHT_RUNTIME_FILE` (ADR-0150). The service lease makes concurrent requests
/// harmless, and a fresh deploy lock suppresses self-heal while binaries are swapped.
pub fn request_orchestrator_start() -> io::Result<StartDisposition> {
    let current_executable = env::current_exe()?;
    #[cfg(target_os = "linux")]
    if use_flatpak_activation(env::var_os("FLATPAK_ID").as_deref())? {
        let home = env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "host HOME is unavailable"))?;
        let executable = resolved_orchestrator(&current_executable, &runtime_discovery())?;
        return request_orchestrator_activation(|| {
            crate::desktop_activation::request_registered_start(Path::new(&home), &executable)
        });
    }
    request_orchestrator_start_from(&current_executable, &runtime_discovery(), SystemTime::now())
}

#[cfg(target_os = "linux")]
fn use_flatpak_activation(app: Option<&std::ffi::OsStr>) -> io::Result<bool> {
    match app {
        None => Ok(false),
        Some(app) if app == crate::desktop_activation::FLATPAK_APP => Ok(true),
        Some(_) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this Flatpak app has no authorized host desktop activation route",
        )),
    }
}

/// Ask a narrowly authorized OS activation route to start the elected desktop authority.
///
/// The callback runs once after shared startup admission and must itself be bounded. The bridge
/// then waits for new desktop-ready discovery. It neither invents a child PID nor executes a
/// fallback command. Deployment quiescence, authority custody and failed-start cooldown are the
/// same as native sibling startup. The activation provider owns any platform-specific consent.
pub fn request_orchestrator_activation(
    activate: impl FnOnce() -> io::Result<()>,
) -> io::Result<StartDisposition> {
    let discovery = runtime_discovery();
    let current = env::current_exe()?;
    let executable = resolved_orchestrator(&current, &discovery)?;
    request_start(
        &executable,
        &discovery,
        SystemTime::now(),
        STARTUP_TIMEOUT,
        || {
            activate()?;
            Ok(Launch::Activation)
        },
    )
}

fn request_orchestrator_start_from(
    current_executable: &Path,
    discovery: &RuntimeDiscovery,
    now: SystemTime,
) -> io::Result<StartDisposition> {
    let service_executable = resolved_orchestrator(current_executable, discovery)?;
    request_start(&service_executable, discovery, now, STARTUP_TIMEOUT, || {
        if !service_executable.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "trusted sibling orchestrator is missing: {}",
                    service_executable.display()
                ),
            ));
        }
        let directory = service_executable
            .parent()
            .expect("validated service directory");
        orchestrator_command(&service_executable, directory)
            .spawn()
            .map(Launch::Child)
    })
}

enum Launch {
    Child(Child),
    Activation,
}

fn request_start(
    service_executable: &Path,
    discovery: &RuntimeDiscovery,
    now: SystemTime,
    timeout: Duration,
    launch: impl FnOnce() -> io::Result<Launch>,
) -> io::Result<StartDisposition> {
    let service_directory = service_executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "orchestrator executable has no parent directory",
        )
    })?;
    if deploy_lock_present(service_directory, now)? {
        return Ok(StartDisposition::DeploymentInProgress);
    }

    let Some(lease) = ServiceLease::try_acquire(&discovery.path)? else {
        return Ok(StartDisposition::AlreadyRunning);
    };
    drop(lease);
    let Some(mut startup) = StartupAdmission::try_acquire(&discovery.path)? else {
        return Ok(StartDisposition::Starting);
    };
    // Recheck custody under startup admission: another launcher may have won since our probe.
    let Some(lease) = ServiceLease::try_acquire(&discovery.path)? else {
        return Ok(StartDisposition::AlreadyRunning);
    };
    drop(lease);
    if deploy_lock_present(service_directory, SystemTime::now())? {
        return Ok(StartDisposition::DeploymentInProgress);
    }
    if startup.cooling_down(now)? {
        return Ok(StartDisposition::RetryDeferred);
    }
    // A stale document is not a completed launch. Only publication of a new token under an
    // authority lease can complete this exchange. Its contents never enter startup diagnostics.
    let previous = read_runtime(&discovery.path).ok();
    let result = launch().and_then(|mut launched| {
        if let Err(error) = wait_for_startup(
            &mut launched,
            &discovery.path,
            service_directory,
            previous.as_ref(),
            timeout,
        ) {
            if let Launch::Child(child) = &mut launched {
                if child.try_wait()?.is_none() {
                    child.kill()?;
                }
                child.wait()?;
            }
            return if error.kind() == io::ErrorKind::Interrupted {
                Ok(StartDisposition::DeploymentInProgress)
            } else {
                Err(error)
            };
        }
        Ok(match launched {
            Launch::Activation => StartDisposition::ActivationRequested,
            Launch::Child(mut child) => {
                let process_id = child.id();
                // Failed launches were already reaped synchronously. A healthy Unix child still
                // needs wait() when it eventually quits. This waiter makes no restart decisions.
                thread::spawn(move || {
                    let _ = child.wait();
                });
                StartDisposition::Spawned { process_id }
            }
        })
    });
    match result {
        Ok(disposition) => {
            startup.file.set_len(0)?;
            Ok(disposition)
        }
        Err(error) => {
            startup.record_failure(SystemTime::now())?;
            Err(error)
        }
    }
}

struct StartupAdmission {
    file: File,
}

impl StartupAdmission {
    fn try_acquire(runtime: &Path) -> io::Result<Option<Self>> {
        let path = runtime.with_extension(STARTUP_LOCK_EXTENSION);
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(Self { file })),
            Err(error) if lock_is_contended(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn cooling_down(&mut self, now: SystemTime) -> io::Result<bool> {
        self.file.seek(SeekFrom::Start(0))?;
        let mut bytes = [0; 9];
        if self.file.read(&mut bytes)? != 8 {
            return Ok(false);
        }
        let millis = u64::from_le_bytes(bytes[..8].try_into().expect("eight timestamp bytes"));
        let Some(recorded) = UNIX_EPOCH.checked_add(Duration::from_millis(millis)) else {
            return Ok(false);
        };
        // A backwards clock or stale/corrupt timestamp must not suppress recovery indefinitely.
        Ok(now
            .duration_since(recorded)
            .is_ok_and(|age| age < FAILED_START_COOLDOWN))
    }

    fn record_failure(&mut self, now: SystemTime) -> io::Result<()> {
        let millis = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let millis = u64::try_from(millis).unwrap_or(u64::MAX);
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&millis.to_le_bytes())?;
        self.file.set_len(8)
    }
}

fn wait_for_startup(
    launch: &mut Launch,
    runtime: &Path,
    service_directory: &Path,
    previous: Option<&RuntimeEndpoint>,
    timeout: Duration,
) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if deploy_lock_present(service_directory, SystemTime::now())? {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "deployment quiesced startup",
            ));
        }
        let exited = match launch {
            Launch::Child(child) => child.try_wait()?,
            Launch::Activation => None,
        };
        if read_runtime(runtime)
            .is_ok_and(|current| previous.is_none_or(|old| old.token != current.token))
            && ServiceLease::try_acquire(runtime)?.is_none()
        {
            return Ok(());
        }
        if let Some(status) = exited {
            return Err(io::Error::other(format!(
                "orchestrator exited {status} before desktop readiness; retry is delayed briefly"
            )));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "orchestrator did not publish desktop readiness in time; retry is delayed briefly",
            ));
        }
        thread::sleep(STARTUP_POLL.min(deadline.saturating_duration_since(Instant::now())));
    }
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

/// The trusted sibling authority this process demand-starts: the directory elected by an
/// explicit runtime override when present (ADR-0150), otherwise the directory of this
/// executable. There is deliberately no fallback between the two: a foreign authority must
/// never be spawned into an elected slot.
fn resolved_orchestrator(
    current_executable: &Path,
    discovery: &RuntimeDiscovery,
) -> io::Result<PathBuf> {
    match &discovery.service_directory {
        Some(directory) => Ok(directory.join(orchestrator_file_name())),
        None => orchestrator_executable(current_executable),
    }
}

fn orchestrator_executable(current_executable: &Path) -> io::Result<PathBuf> {
    let directory = current_executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "connector executable has no parent directory",
        )
    })?;
    Ok(directory.join(orchestrator_file_name()))
}

fn orchestrator_file_name() -> &'static str {
    if cfg!(windows) {
        "ghostlight.exe"
    } else {
        "ghostlight"
    }
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
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    use super::{
        deploy_lock_present, lock_is_fresh, orchestrator_command, orchestrator_executable,
        orchestrator_file_name, request_orchestrator_start_from, resolved_orchestrator,
        service_lock_file, ServiceLease, DEPLOY_LOCK_FILE, DEPLOY_LOCK_MAX_AGE,
    };
    use crate::runtime::RuntimeDiscovery;

    #[cfg(target_os = "linux")]
    #[test]
    fn only_the_explicit_flatpak_app_uses_host_activation_without_native_fallback() {
        use std::ffi::OsStr;
        assert!(!super::use_flatpak_activation(None).unwrap());
        assert!(super::use_flatpak_activation(Some(OsStr::new(
            crate::desktop_activation::FLATPAK_APP
        )))
        .unwrap());
        for app in ["", "org.chromium.Other", "org.chromium.Chromium.beta"] {
            assert_eq!(
                super::use_flatpak_activation(Some(OsStr::new(app)))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::Unsupported
            );
        }
    }

    fn discovery(directory: &Path) -> RuntimeDiscovery {
        RuntimeDiscovery {
            path: directory.join("runtime.json"),
            service_directory: Some(directory.into()),
        }
    }

    fn endpoint(token: &str) -> crate::runtime::RuntimeEndpoint {
        crate::runtime::RuntimeEndpoint {
            service_port: 1,
            browser_port: 2,
            token: token.into(),
            service_bridge_major: 2,
            browser_relay_major: 2,
            service_version: "test".into(),
        }
    }

    #[test]
    fn concurrent_callers_share_failed_start_cooldown_and_recover_after_expiry() {
        use super::{request_start, StartDisposition, FAILED_START_COOLDOWN};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Barrier,
        };
        let directory = temporary_directory("startup-concurrency");
        let barrier = Arc::new(Barrier::new(12));
        let launches = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..12)
            .map(|_| {
                let directory = directory.clone();
                let barrier = barrier.clone();
                let launches = launches.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    request_start(
                        &directory.join(orchestrator_file_name()),
                        &discovery(&directory),
                        SystemTime::now(),
                        Duration::from_millis(100),
                        || {
                            launches.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(50));
                            Err(std::io::Error::other("launch failed"))
                        },
                    )
                })
            })
            .collect();
        for thread in threads {
            let _ = thread.join().unwrap();
        }
        assert_eq!(launches.load(Ordering::SeqCst), 1);
        let result = request_start(
            &directory.join(orchestrator_file_name()),
            &discovery(&directory),
            SystemTime::now(),
            Duration::from_millis(100),
            || panic!("cooldown must suppress launch"),
        );
        assert_eq!(result.unwrap(), StartDisposition::RetryDeferred);
        let mut lease = None;
        let result = request_start(
            &directory.join(orchestrator_file_name()),
            &discovery(&directory),
            SystemTime::now() + FAILED_START_COOLDOWN,
            Duration::from_millis(100),
            || {
                lease = ServiceLease::try_acquire(&discovery(&directory).path)?;
                crate::runtime::write_runtime(&discovery(&directory).path, &endpoint("ready"))?;
                Ok(super::Launch::Activation)
            },
        );
        assert_eq!(result.unwrap(), StartDisposition::ActivationRequested);
        drop(lease);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stale_discovery_does_not_complete_an_activation_and_deploy_suppresses_it() {
        use super::{request_start, Launch, StartDisposition};
        let directory = temporary_directory("activation-stale");
        let runtime = discovery(&directory);
        crate::runtime::write_runtime(&runtime.path, &endpoint("stale")).unwrap();
        let result = request_start(
            &directory.join(orchestrator_file_name()),
            &runtime,
            SystemTime::now(),
            Duration::from_millis(30),
            || Ok(Launch::Activation),
        );
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        fs::write(directory.join(DEPLOY_LOCK_FILE), "deploy").unwrap();
        assert_eq!(
            request_start(
                &directory.join(orchestrator_file_name()),
                &runtime,
                SystemTime::now(),
                Duration::from_millis(30),
                || panic!("deployment forbids activation")
            )
            .unwrap(),
            StartDisposition::DeploymentInProgress
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn malformed_or_future_cooldown_cannot_disable_startup_indefinitely() {
        let directory = temporary_directory("startup-clock");
        let runtime = discovery(&directory);
        fs::create_dir_all(&directory).unwrap();
        let mut guard = super::StartupAdmission::try_acquire(&runtime.path)
            .unwrap()
            .unwrap();
        guard
            .record_failure(SystemTime::now() + Duration::from_secs(60))
            .unwrap();
        assert!(!guard.cooling_down(SystemTime::now()).unwrap());
        guard.file.set_len(1).unwrap();
        assert!(!guard.cooling_down(SystemTime::now()).unwrap());
        drop(guard);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn failed_and_stalled_children_are_reaped_before_admission_reopens() {
        use super::{request_start, Launch};
        for (name, command, args, kind) in [
            (
                "early-exit",
                "/bin/sh",
                vec!["-c", "exit 7"],
                std::io::ErrorKind::Other,
            ),
            (
                "stalled",
                "/bin/sleep",
                vec!["20"],
                std::io::ErrorKind::TimedOut,
            ),
        ] {
            let directory = temporary_directory(name);
            let mut pid = 0;
            let result = request_start(
                &directory.join(orchestrator_file_name()),
                &discovery(&directory),
                SystemTime::now(),
                Duration::from_millis(50),
                || {
                    let child = std::process::Command::new(command).args(&args).spawn()?;
                    pid = child.id();
                    Ok(Launch::Child(child))
                },
            );
            assert_eq!(result.unwrap_err().kind(), kind);
            assert!(
                !Path::new(&format!("/proc/{pid}")).exists(),
                "owned child must be reaped"
            );
            assert!(
                super::StartupAdmission::try_acquire(&discovery(&directory).path)
                    .unwrap()
                    .is_some()
            );
            fs::remove_dir_all(directory).unwrap();
        }
    }

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
    fn the_runtime_override_elects_its_directory_for_demand_start() {
        let connector = Path::new("/installed").join(if cfg!(windows) {
            "ghostlight-mcp-connector.exe"
        } else {
            "ghostlight-mcp-connector"
        });
        let discovery = RuntimeDiscovery {
            path: PathBuf::from("/dev/tree/ghostlight-runtime.json"),
            service_directory: Some(PathBuf::from("/dev/tree")),
        };
        let expected = Path::new("/dev/tree").join(if cfg!(windows) {
            "ghostlight.exe"
        } else {
            "ghostlight"
        });
        assert_eq!(
            resolved_orchestrator(&connector, &discovery).expect("override has a directory"),
            expected
        );
    }

    #[test]
    fn the_default_resolution_never_elects_a_foreign_directory() {
        let connector = Path::new("engine").join(if cfg!(windows) {
            "ghostlight-mcp-connector.exe"
        } else {
            "ghostlight-mcp-connector"
        });
        let discovery = RuntimeDiscovery {
            path: PathBuf::from("elsewhere/ghostlight-runtime.json"),
            service_directory: None,
        };
        assert_eq!(
            resolved_orchestrator(&connector, &discovery).expect("connector has a parent"),
            orchestrator_executable(&connector).expect("connector has a parent")
        );
    }

    #[test]
    fn demand_start_refuses_an_elected_directory_without_an_authority() {
        let directory = temporary_directory("override-missing");
        let discovery = RuntimeDiscovery {
            path: directory.join("ghostlight-runtime.json"),
            service_directory: Some(directory.clone()),
        };
        let connector = Path::new("engine").join(if cfg!(windows) {
            "ghostlight-mcp-connector.exe"
        } else {
            "ghostlight-mcp-connector"
        });
        let error = request_orchestrator_start_from(&connector, &discovery, SystemTime::now())
            .expect_err("an elected directory without an authority is a loud failure");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(
            !directory.join(orchestrator_file_name()).exists(),
            "the foreign sibling must never be spawned into the elected slot"
        );
        fs::remove_dir_all(&directory).expect("temporary directory is removed");
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
