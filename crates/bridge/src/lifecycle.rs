//! The connector exit path and the deployment quiesce check.
//!
//! A deploy lock in the Ghostlight home quiesces every entry point while executables are swapped.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::diagnostics::{event, Level, Sink};
use crate::parent_process::ParentProcess;

/// Presence marker used to quiesce startup during a sibling replacement.
pub const DEPLOY_LOCK_FILE: &str = "deploy.lock";
const DEPLOY_LOCK_MAX_AGE: Duration = Duration::from_secs(30 * 60);
const PARENT_POLL_INTERVAL: Duration = Duration::from_secs(1);

enum ConnectorExit {
    InputClosed(&'static str),
    ParentExited,
    #[cfg(windows)]
    SystemShutdown,
}

/// One ordered exit path for a connector's normal input closure and parent-death detector.
pub struct ConnectorShutdown {
    exit: mpsc::SyncSender<ConnectorExit>,
    #[cfg(windows)]
    _system_shutdown: ghostlight_win_peer::SystemShutdownListener,
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
                    #[cfg(windows)]
                    ConnectorExit::SystemShutdown => "system shutting down",
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

        #[cfg(windows)]
        let system_shutdown = {
            let shutdown_exit = exit.clone();
            ghostlight_win_peer::listen_for_system_shutdown(Box::new(move |event| {
                if event == ghostlight_win_peer::ShutdownEvent::Terminating {
                    // A window procedure must return promptly. If another exit reason already
                    // filled the coordinator slot, that path is already terminating the process.
                    let _ = shutdown_exit.try_send(ConnectorExit::SystemShutdown);
                }
            }))?
        };

        Ok(Self {
            exit,
            #[cfg(windows)]
            _system_shutdown: system_shutdown,
        })
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

/// Whether the production deployment is deliberately quiescing all entry points.
pub fn production_deployment_in_progress() -> io::Result<bool> {
    deploy_lock_present(
        &crate::installation::control_directory()?,
        SystemTime::now(),
    )
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime};

    use super::{deploy_lock_present, lock_is_fresh, DEPLOY_LOCK_FILE, DEPLOY_LOCK_MAX_AGE};

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
}
