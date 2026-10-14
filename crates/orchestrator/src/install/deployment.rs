//! `ghostlight deployment install`: put this package's executables into the fixed `bin`.
//!
//! A package verifies its download, then runs its own `ghostlight` with this command. The
//! executables beside it are copied into `home/bin` under the install lock; an executable whose
//! bytes did not change is left alone, so its running streams survive (ADR-0197 Decisions 13
//! and 14). The install owns `bin/install.json`: it compares the recorded install with this
//! package first, refuses an older package, skips an intact install of the same version, and
//! records what it put in `bin` once the copy succeeds.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::layout::{
    Layout, BROWSER_CONNECTOR_EXECUTABLE_NAME, INSTALL_RECORD_NAME, MCP_CONNECTOR_EXECUTABLE_NAME,
};
use super::record::{self, InstallRecord, PayloadRefused, Standing};
use ghostlight_relay::home::SERVICE_EXECUTABLE_NAME;

/// The executables one package carries and one install replaces.
const COMPONENTS: [&str; 3] = [
    SERVICE_EXECUTABLE_NAME,
    MCP_CONNECTOR_EXECUTABLE_NAME,
    BROWSER_CONNECTOR_EXECUTABLE_NAME,
];

/// How long an install waits for another install to finish.
const INSTALL_LOCK_WAIT: Duration = Duration::from_secs(35);

/// Pause between attempts to take the install lock.
const INSTALL_LOCK_POLL: Duration = Duration::from_millis(25);

/// Install the verified siblings of this executable into `destination`.
///
/// Downloaders verify the complete payload before invoking this package seam. The install lock
/// in the Ghostlight home serializes concurrent installs; no updater process persists.
pub fn install_download(destination: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        destination.is_absolute(),
        "The installation destination must be absolute"
    );
    let current = std::env::current_exe()?;
    let source = current
        .parent()
        .ok_or_else(|| io::Error::other("download has no directory"))?;
    validate_siblings(source)?;
    fs::create_dir_all(destination)?;
    let destination = fs::canonicalize(destination)?;
    anyhow::ensure!(
        fs::canonicalize(source)? != destination,
        "Run package installation from its temporary download directory"
    );
    install_locked(
        source,
        &destination,
        &Layout::current()?.install_lock(),
        env!("CARGO_PKG_VERSION"),
    )
}

/// Install the package in `source`, which carries service `version`, into `destination` while
/// holding `lock`.
///
/// The record in `destination` decides first: an older package is refused with
/// [`PayloadRefused`], and an intact install of the same version is left alone. Otherwise the
/// changed executables are copied and the record is rewritten to describe them.
fn install_locked(
    source: &Path,
    destination: &Path,
    lock: &Path,
    version: &str,
) -> anyhow::Result<()> {
    let _guard = hold_install_lock(lock)?;
    let record_path = destination.join(INSTALL_RECORD_NAME);
    let installed = InstallRecord::read(&record_path)?;
    let payload = InstallRecord {
        service_version: version.to_owned(),
        relay_revision: ghostlight_relay::RELAY.to_string(),
        service_sha256: record::sha256_file(&source.join(SERVICE_EXECUTABLE_NAME))?,
    };
    let installed_image = match record::sha256_file(&destination.join(SERVICE_EXECUTABLE_NAME)) {
        Ok(sha) => Some(sha),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let installed = installed.as_ref();
    match (
        record::decide(installed, &payload, installed_image.as_deref()),
        installed,
    ) {
        (Standing::Same, _) => return Ok(()),
        (Standing::Newer, Some(installed)) => {
            return Err(PayloadRefused::new(installed, &payload).into())
        }
        _ => {}
    }
    replace_changed(source, destination)?;
    payload.write(&record_path)?;
    Ok(())
}

/// Copy the executables in `source` whose bytes differ into `destination`, stopping the
/// processes that run the replaced images. A failure puts the previous executables back.
fn replace_changed(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let changed = changed_components(source, destination)?;
    // Copy before stopping anything. The temporary payload never runs.
    let mut replacements: Vec<(PathBuf, PathBuf)> = Vec::new();
    for name in &changed {
        let temporary = destination.join(format!(
            ".{name}.download-{}",
            uuid::Uuid::new_v4().simple()
        ));
        if let Err(error) = fs::copy(source.join(name), &temporary) {
            for (prepared, _) in &replacements {
                let _ = fs::remove_file(prepared);
            }
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        replacements.push((temporary, destination.join(name)));
    }
    let images: Vec<PathBuf> = replacements
        .iter()
        .map(|(_, target)| target.clone())
        .collect();
    let mut backups = Vec::new();
    let replaced = (|| -> anyhow::Result<()> {
        super::migration::stop_legacy_images(&images)?;
        for (temporary, target) in &replacements {
            let backup = target.with_extension(format!("backup-{}", uuid::Uuid::new_v4().simple()));
            if target.exists() {
                fs::rename(target, &backup)?;
            }
            backups.push((target.clone(), backup));
            fs::rename(temporary, target)?;
        }
        Ok(())
    })();
    if replaced.is_err() {
        for (target, backup) in backups.iter().rev() {
            if target.exists() {
                fs::remove_file(target)?;
            }
            if backup.exists() {
                fs::rename(backup, target)?;
            }
        }
    } else {
        for (_, backup) in &backups {
            if backup.exists() {
                fs::remove_file(backup)?;
            }
        }
    }
    for (temporary, _) in &replacements {
        let _ = fs::remove_file(temporary);
    }
    replaced
}

/// Take the install lock at `path`, waiting up to [`INSTALL_LOCK_WAIT`] for another install.
fn hold_install_lock(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let deadline = Instant::now() + INSTALL_LOCK_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(INSTALL_LOCK_POLL);
            }
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Ghostlight installation is still changing; retry shortly",
                ))
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
}

/// Require every executable a package carries in `directory`.
fn validate_siblings(directory: &Path) -> io::Result<()> {
    for name in COMPONENTS {
        let path = directory.join(name);
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("The Ghostlight package is missing {}", path.display()),
            ));
        }
    }
    Ok(())
}

/// The executables whose bytes in `source` differ from the copy in `destination`.
fn changed_components(source: &Path, destination: &Path) -> io::Result<Vec<String>> {
    let mut changed = Vec::new();
    for name in COMPONENTS {
        let bytes = fs::read(source.join(name))?;
        let matches = match fs::read(destination.join(name)) {
            Ok(installed) => installed == bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        if !matches {
            changed.push(name.to_owned());
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::super::scratch::Scratch;
    use super::*;

    /// Service version the test packages carry.
    const VERSION: &str = "2.1.0";

    /// A later service version for upgrade tests.
    const NEXT_VERSION: &str = "2.2.0";

    fn payload(scratch: &Scratch) -> (PathBuf, PathBuf, PathBuf) {
        let source = scratch.path().join("download");
        let destination = scratch.path().join("home").join("bin");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        for name in COMPONENTS {
            fs::write(source.join(name), name).unwrap();
        }
        (
            fs::canonicalize(source).unwrap(),
            fs::canonicalize(destination).unwrap(),
            scratch.path().join("home").join("install.lock"),
        )
    }

    #[test]
    fn repeated_installs_replace_only_what_changed() {
        let scratch = Scratch::new();
        let (source, destination, lock) = payload(&scratch);
        install_locked(&source, &destination, &lock, VERSION).unwrap();
        for name in COMPONENTS {
            assert_eq!(fs::read_to_string(destination.join(name)).unwrap(), name);
        }
        fs::write(source.join(SERVICE_EXECUTABLE_NAME), "next service").unwrap();
        assert_eq!(
            changed_components(&source, &destination).unwrap(),
            vec![SERVICE_EXECUTABLE_NAME.to_owned()]
        );
        install_locked(&source, &destination, &lock, NEXT_VERSION).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join(SERVICE_EXECUTABLE_NAME)).unwrap(),
            "next service"
        );
        assert_eq!(
            fs::read_dir(&destination).unwrap().count(),
            COMPONENTS.len() + 1
        );
        assert!(changed_components(&source, &destination)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn an_install_records_what_it_put_in_bin() {
        let scratch = Scratch::new();
        let (source, destination, lock) = payload(&scratch);
        install_locked(&source, &destination, &lock, VERSION).unwrap();
        assert_eq!(
            InstallRecord::read(&destination.join(INSTALL_RECORD_NAME)).unwrap(),
            Some(InstallRecord {
                service_version: VERSION.to_owned(),
                relay_revision: ghostlight_relay::RELAY.to_string(),
                service_sha256: record::sha256_file(&destination.join(SERVICE_EXECUTABLE_NAME))
                    .unwrap(),
            })
        );
    }

    #[test]
    fn an_older_package_is_refused_and_changes_nothing() {
        let scratch = Scratch::new();
        let (source, destination, lock) = payload(&scratch);
        install_locked(&source, &destination, &lock, VERSION).unwrap();
        let record_path = destination.join(INSTALL_RECORD_NAME);
        let recorded = fs::read(&record_path).unwrap();
        fs::write(source.join(SERVICE_EXECUTABLE_NAME), "older service").unwrap();
        let error = install_locked(&source, &destination, &lock, "2.0.0").unwrap_err();
        assert!(error.is::<PayloadRefused>(), "{error}");
        assert_eq!(
            fs::read_to_string(destination.join(SERVICE_EXECUTABLE_NAME)).unwrap(),
            SERVICE_EXECUTABLE_NAME
        );
        assert_eq!(fs::read(&record_path).unwrap(), recorded);
    }

    #[test]
    fn an_intact_install_of_the_same_version_is_left_alone() {
        let scratch = Scratch::new();
        let (source, destination, lock) = payload(&scratch);
        install_locked(&source, &destination, &lock, VERSION).unwrap();
        fs::write(source.join(SERVICE_EXECUTABLE_NAME), "other build").unwrap();
        install_locked(&source, &destination, &lock, VERSION).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join(SERVICE_EXECUTABLE_NAME)).unwrap(),
            SERVICE_EXECUTABLE_NAME
        );
    }

    #[test]
    fn a_damaged_install_of_the_same_version_is_repaired() {
        let scratch = Scratch::new();
        let (source, destination, lock) = payload(&scratch);
        install_locked(&source, &destination, &lock, VERSION).unwrap();
        fs::write(destination.join(SERVICE_EXECUTABLE_NAME), "damaged").unwrap();
        install_locked(&source, &destination, &lock, VERSION).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join(SERVICE_EXECUTABLE_NAME)).unwrap(),
            SERVICE_EXECUTABLE_NAME
        );
    }

    #[test]
    fn a_package_missing_an_executable_is_refused() {
        let scratch = Scratch::new();
        let (source, _, _) = payload(&scratch);
        validate_siblings(&source).unwrap();
        fs::remove_file(source.join(BROWSER_CONNECTOR_EXECUTABLE_NAME)).unwrap();
        assert_eq!(
            validate_siblings(&source).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn a_held_install_lock_is_waited_for() {
        let scratch = Scratch::new();
        let lock = scratch.path().join("install.lock");
        let held = hold_install_lock(&lock).unwrap();
        let waiter = {
            let lock = lock.clone();
            std::thread::spawn(move || hold_install_lock(&lock).map(drop))
        };
        std::thread::sleep(Duration::from_millis(100));
        drop(held);
        waiter.join().unwrap().unwrap();
    }
}
