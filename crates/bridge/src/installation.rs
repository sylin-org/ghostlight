//! Durable executable selection for the user's single production installation (ADR-0167).
//!
//! This is local lifecycle mechanism. Installers choose release/development custody;
//! connectors only read it and bootstrap an empty installation with their trusted siblings.

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

const RECORD_SCHEMA: u32 = 1;
const RECORD_LIMIT: u64 = 16 * 1024;

/// Persistent selection, independent of the running process and its authentication token.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Installation {
    /// Version of this small local record.
    pub schema: u32,
    /// Selected serving directory; a warm release upgrade becomes active on the next start.
    pub serving_directory: PathBuf,
    /// Available packaged release, retained while development is selected.
    pub release_directory: Option<PathBuf>,
    /// Explicit development deployment; package invocation never changes it.
    pub development_directory: Option<PathBuf>,
    /// Existing history and diagnostics location, preserved through binary replacement.
    pub state_directory: PathBuf,
    /// Existing policy custody, independent of the environment of later launchers.
    pub policy_directory: PathBuf,
}

impl Installation {
    /// The sole executable directory to start, including while it is temporarily unavailable.
    pub fn directory(&self) -> &Path {
        &self.serving_directory
    }

    /// Human-readable source of the selected authority.
    pub fn source(&self) -> &'static str {
        if self.development_directory.is_some() {
            "development"
        } else {
            "release"
        }
    }
}

/// Explicit installation lifecycle changes. Ordinary launches only bootstrap an absent record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selection {
    /// Select only when no installation exists.
    Bootstrap,
    /// Remember a package update, preserving development custody.
    Release,
    /// Select the development loop's deployed binary set.
    Development,
    /// Return development custody to the remembered packaged release.
    Restore,
}

/// Canonical user control root, outside package-specific AppData and sandbox XDG remapping.
pub fn control_directory() -> io::Result<PathBuf> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = env::var_os(variable)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    home.map(|p| p.join(".ghostlight")).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Ghostlight cannot locate this user's home directory",
        )
    })
}

/// Production runtime location. Only explicit runtime overrides create isolated test contexts.
pub fn production_runtime() -> io::Result<PathBuf> {
    Ok(control_directory()?.join("ghostlight-runtime.json"))
}

/// Read selection without registering, downloading or starting anything.
pub fn read(runtime: &Path) -> io::Result<Option<Installation>> {
    let path = record_path(runtime);
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(RECORD_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > RECORD_LIMIT {
        return Err(invalid(
            "Ghostlight installation record exceeds its size limit",
        ));
    }
    let record: Installation = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("Ghostlight installation record is invalid; it was preserved"))?;
    if record.schema != RECORD_SCHEMA
        || record.development_directory.is_none() && record.release_directory.is_none()
        || !record.state_directory.is_absolute()
        || !record.serving_directory.is_absolute()
        || !record.policy_directory.is_absolute()
        || record
            .development_directory
            .as_ref()
            .is_some_and(|path| path != &record.serving_directory)
        || record
            .release_directory
            .iter()
            .chain(record.development_directory.iter())
            .any(|p| !p.is_absolute())
    {
        return Err(invalid(
            "Ghostlight installation record is unsupported; it was preserved",
        ));
    }
    Ok(Some(record))
}

/// Serialize selection and startup so a competing installer cannot elect another child.
pub fn lock(runtime: &Path) -> io::Result<File> {
    let parent = runtime
        .parent()
        .ok_or_else(|| invalid("runtime has no directory"))?;
    fs::create_dir_all(parent)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(runtime.with_extension("installation.lock"))?;
    // Bounded admission: installation must not hang indefinitely behind an abandoned operation.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(35);
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(file),
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(33) =>
            {
                if std::time::Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Ghostlight installation is still changing; retry shortly",
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(error) => return Err(error),
        }
    }
}

/// Apply an explicit selection transaction; callers hold the installation lock.
pub fn select_locked(
    runtime: &Path,
    directory: &Path,
    selection: Selection,
) -> io::Result<Installation> {
    let previous = read(runtime)?;
    if selection == Selection::Bootstrap {
        if let Some(record) = previous {
            if record.development_directory.is_none() {
                if let Some(release) = &record.release_directory {
                    if release != record.directory() {
                        return select_locked(runtime, release, Selection::Release);
                    }
                }
            }
            return Ok(record);
        }
    }
    let directory = fs::canonicalize(directory)?;
    let initial_policy_directory = if previous.is_none() {
        let canonical = legacy_policy_directory().map(fs::canonicalize).transpose();
        match canonical {
            Ok(Some(path)) => path,
            Ok(None) => runtime
                .parent()
                .expect("locked runtime has a parent")
                .join("policy"),
            Err(error) if error.kind() == io::ErrorKind::NotFound => runtime
                .parent()
                .expect("locked runtime has a parent")
                .join("policy"),
            Err(error) => return Err(error),
        }
    } else {
        PathBuf::new() // Unused: the existing record retains its policy custody.
    };
    let mut record = previous.unwrap_or_else(|| Installation {
        schema: RECORD_SCHEMA,
        serving_directory: directory.clone(),
        release_directory: None,
        development_directory: None,
        // Adoption retains the old journal and diagnostic marker without rewriting either.
        state_directory: if directory.join("audit.jsonl").exists()
            || directory.join("diagnostics.on").exists()
        {
            directory.clone()
        } else {
            runtime
                .parent()
                .expect("locked runtime has a parent")
                .to_path_buf()
        },
        policy_directory: initial_policy_directory,
    });
    let lease = crate::lifecycle::ServiceLease::try_acquire(runtime)?;
    match selection {
        Selection::Bootstrap | Selection::Release => {
            validate_siblings(&directory)?;
            if record.development_directory.is_none() && lease.is_some() {
                record.serving_directory = directory.clone();
            }
            if record.development_directory.as_ref() != Some(&directory) {
                record.release_directory = Some(directory);
            }
        }
        Selection::Development => {
            if record.directory() != directory && lease.is_none() {
                return Err(invalid("The serving authority must finish its deployment before selecting another build"));
            }
            record.serving_directory = directory.clone();
            record.development_directory = Some(directory);
        }
        Selection::Restore => {
            if record.release_directory.is_none() {
                return Err(invalid(
                    "No packaged Ghostlight release has been installed to restore",
                ));
            }
            if lease.is_none() {
                return Err(invalid(
                    "Stop the development authority before restoring the release",
                ));
            }
            record.development_directory = None;
            record.serving_directory = record.release_directory.clone().expect("release exists");
        }
    }
    validate_siblings(record.directory())?;
    let bytes = serde_json::to_vec_pretty(&record).map_err(io::Error::other)?;
    crate::runtime::write_private_document(&record_path(runtime), &bytes)?;
    Ok(record)
}

/// Resolve and, only if absent, establish this user's installation from trusted siblings.
/// The caller retains the lock through its startup exchange; the selected child only reads it.
pub fn startup_selection() -> io::Result<(Option<File>, crate::runtime::RuntimeDiscovery)> {
    let mut discovery = crate::runtime::runtime_discovery()?;
    if env::var_os("GHOSTLIGHT_RUNTIME_FILE").is_some() {
        return Ok((None, discovery));
    }
    let guard = lock(&discovery.path)?;
    let current = env::current_exe()?;
    let directory = current
        .parent()
        .ok_or_else(|| invalid("executable has no directory"))?;
    let selected = select_locked(&discovery.path, directory, Selection::Bootstrap)?;
    discovery.service_directory = Some(selected.directory().to_path_buf());
    Ok((Some(guard), discovery))
}

/// Verify the selected local executable set before accepting deployment custody.
pub fn validate_siblings(directory: &Path) -> io::Result<()> {
    for name in [
        "ghostlight",
        "ghostlight-mcp-connector",
        "ghostlight-browser-connector",
    ] {
        let path = directory.join(executable_name(name));
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "The selected Ghostlight installation is missing {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

/// Resolve the selected executable for setup and diagnosis, without creating a selection.
pub fn selected_executable() -> io::Result<PathBuf> {
    let discovery = crate::runtime::runtime_discovery()?;
    Ok(discovery
        .service_directory
        .or_else(|| {
            env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf))
        })
        .ok_or_else(|| invalid("Ghostlight executable has no directory"))?
        .join(executable_name("ghostlight")))
}

/// Stable state location for production; explicitly isolated fixtures retain their own state.
pub fn state_directory(runtime: &Path) -> io::Result<PathBuf> {
    if production_runtime().is_ok_and(|path| path == runtime) {
        if let Some(record) = read(runtime)? {
            return Ok(record.state_directory);
        }
    }
    runtime
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| invalid("runtime has no directory"))
}

/// Refuse publication from a superseded executable after acquiring the lifetime lease.
pub fn verify_serving_executable(runtime: &Path) -> io::Result<()> {
    if production_runtime().is_ok_and(|path| path == runtime) {
        let record = read(runtime)?
            .ok_or_else(|| invalid("Ghostlight installation has not been selected"))?;
        let current = env::current_exe()?;
        if fs::canonicalize(record.directory())?
            != fs::canonicalize(
                current
                    .parent()
                    .ok_or_else(|| invalid("executable has no directory"))?,
            )?
        {
            return Err(invalid(
                "Ghostlight selection changed during startup; retry the selected installation",
            ));
        }
    }
    Ok(())
}

fn legacy_policy_directory() -> Option<PathBuf> {
    if cfg!(windows) {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Ghostlight"))
    } else {
        env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .map(|path| path.join("ghostlight"))
    }
}

/// Platform-specific spelling of a native Ghostlight executable.
pub fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn record_path(runtime: &Path) -> PathBuf {
    runtime.with_extension("installation.json")
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_launch_and_update_preserve_development_and_history() {
        let root = env::temp_dir().join(format!("ghostlight-selection-{}", uuid::Uuid::new_v4()));
        let runtime = root.join("control/runtime.json");
        for directory in ["dev", "release", "new-release"] {
            let directory = root.join(directory);
            fs::create_dir_all(&directory).unwrap();
            for name in [
                "ghostlight",
                "ghostlight-mcp-connector",
                "ghostlight-browser-connector",
            ] {
                fs::write(directory.join(executable_name(name)), b"fixture").unwrap();
            }
        }
        fs::write(root.join("dev/audit.jsonl"), b"preserved history").unwrap();
        let _lock = lock(&runtime).unwrap();
        let dev = select_locked(&runtime, &root.join("dev"), Selection::Development).unwrap();
        assert_eq!(
            select_locked(&runtime, &root.join("release"), Selection::Bootstrap).unwrap(),
            dev
        );
        let updated = select_locked(&runtime, &root.join("release"), Selection::Release).unwrap();
        assert_eq!(updated.directory(), dev.directory());
        let dev_setup = select_locked(&runtime, &root.join("dev"), Selection::Release).unwrap();
        assert_eq!(dev_setup.release_directory, updated.release_directory);
        let updated =
            select_locked(&runtime, &root.join("new-release"), Selection::Release).unwrap();
        assert_eq!(updated.state_directory, dev.state_directory);
        let restored = select_locked(&runtime, &root.join("dev"), Selection::Restore).unwrap();
        assert_eq!(
            restored.directory(),
            fs::canonicalize(root.join("new-release")).unwrap()
        );
        assert_eq!(restored.state_directory, dev.state_directory);
        assert_eq!(
            fs::read(root.join("dev/audit.jsonl")).unwrap(),
            b"preserved history"
        );
        drop(_lock);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_record_never_falls_back_or_gets_overwritten() {
        let root = env::temp_dir().join(format!(
            "ghostlight-invalid-selection-{}",
            uuid::Uuid::new_v4()
        ));
        let runtime = root.join("runtime.json");
        let guard = lock(&runtime).unwrap();
        fs::write(record_path(&runtime), b"broken").unwrap();
        assert!(select_locked(&runtime, &root, Selection::Bootstrap).is_err());
        assert_eq!(fs::read(record_path(&runtime)).unwrap(), b"broken");
        drop(guard);
        fs::remove_dir_all(root).unwrap();
    }

    fn fixture(root: &Path, name: &str) -> PathBuf {
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        for name in [
            "ghostlight",
            "ghostlight-mcp-connector",
            "ghostlight-browser-connector",
        ] {
            fs::write(directory.join(executable_name(name)), b"fixture").unwrap();
        }
        directory
    }

    #[test]
    fn warm_release_upgrade_stages_until_the_authority_exits() {
        let root = env::temp_dir().join(format!(
            "ghostlight-warm-selection-{}",
            uuid::Uuid::new_v4()
        ));
        let runtime = root.join("runtime.json");
        let old = fixture(&root, "old");
        let new = fixture(&root, "new");
        let guard = lock(&runtime).unwrap();
        let first = select_locked(&runtime, &old, Selection::Release).unwrap();
        let lease = crate::lifecycle::ServiceLease::try_acquire(&runtime)
            .unwrap()
            .unwrap();
        let staged = select_locked(&runtime, &new, Selection::Release).unwrap();
        assert_eq!(staged.directory(), first.directory());
        assert_eq!(
            select_locked(&runtime, &old, Selection::Bootstrap).unwrap(),
            staged
        );
        assert!(select_locked(&runtime, &new, Selection::Development).is_err());
        assert!(select_locked(&runtime, &old, Selection::Restore).is_err());
        drop(lease);
        let selected = select_locked(&runtime, &old, Selection::Bootstrap).unwrap();
        assert_eq!(selected.directory(), fs::canonicalize(new).unwrap());
        assert_eq!(selected.state_directory, first.state_directory);
        drop(guard);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_bootstraps_elect_once_and_missing_development_never_falls_back() {
        let root = env::temp_dir().join(format!(
            "ghostlight-race-selection-{}",
            uuid::Uuid::new_v4()
        ));
        let runtime = root.join("runtime.json");
        let candidates = [fixture(&root, "first"), fixture(&root, "second")];
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = candidates
            .iter()
            .map(|directory| {
                let directory = directory.clone();
                let runtime = runtime.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    let _guard = lock(&runtime).unwrap();
                    select_locked(&runtime, &directory, Selection::Bootstrap).unwrap()
                })
            })
            .collect();
        let records: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(records[0], records[1]);
        let guard = lock(&runtime).unwrap();
        let dev = select_locked(&runtime, &candidates[0], Selection::Development).unwrap();
        fs::remove_file(candidates[0].join(executable_name("ghostlight"))).unwrap();
        assert_eq!(
            select_locked(&runtime, &candidates[1], Selection::Bootstrap).unwrap(),
            dev
        );
        assert!(validate_siblings(dev.directory()).is_err());
        assert!(select_locked(&runtime, &candidates[0], Selection::Development).is_err());
        assert_eq!(read(&runtime).unwrap().unwrap(), dev);
        drop(guard);
        fs::remove_dir_all(root).unwrap();
    }
}
