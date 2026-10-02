//! Orchestrator-owned installation selection for package setup and the development loop.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ghostlight_bridge::installation::{self, Installation, Selection};

const COMPONENTS: &[&str] = &[
    "ghostlight",
    "ghostlight-mcp-connector",
    "ghostlight-browser-connector",
];

/// Install the verified siblings of this download into one permanent package directory.
///
/// Downloaders verify the complete payload before invoking this package seam. Existing
/// startup custody and exact image paths protect replacement; no updater process persists.
pub fn install_download(destination: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        destination.is_absolute(),
        "The installation destination must be absolute"
    );
    anyhow::ensure!(
        env::var_os("GHOSTLIGHT_RUNTIME_FILE").is_none(),
        "Package installation cannot use an isolated runtime override"
    );
    let current = env::current_exe()?;
    let source = current
        .parent()
        .ok_or_else(|| io::Error::other("download has no directory"))?;
    installation::validate_siblings(source)?;
    fs::create_dir_all(destination)?;
    let destination = fs::canonicalize(destination)?;
    anyhow::ensure!(
        fs::canonicalize(source)? != destination,
        "Run package installation from its temporary download directory"
    );
    let runtime = installation::production_runtime()?;
    let control = installation::control_directory()?;
    fs::create_dir_all(&control)?;
    let marker = control.join(ghostlight_bridge::lifecycle::DEPLOY_LOCK_FILE);
    let marker_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)?;
    let result = install_download_locked(source, &destination, &runtime);
    drop(marker_file);
    fs::remove_file(marker)?;
    result
}

fn install_download_locked(
    source: &Path,
    destination: &Path,
    runtime: &Path,
) -> anyhow::Result<()> {
    let guard = installation::lock(runtime)?;
    let previous = installation::read(runtime)?;
    let changed = changed_components(source, destination)?;
    let legacy = previous.as_ref().filter(|record| {
        record.development_directory.is_none() && record.directory() != destination
    });
    anyhow::ensure!(
        legacy.is_none() || runtime == installation::production_runtime()?,
        "Registration migration cannot use a private installation record"
    );
    let mut images: Vec<PathBuf> = changed.iter().map(|name| destination.join(name)).collect();
    // A service change retires the serving image. Unchanged shores retain their streams.
    if let Some(record) = legacy {
        images.extend(
            COMPONENTS
                .iter()
                .map(|name| record.directory().join(installation::executable_name(name))),
        );
    }
    // Copy before stopping anything. The temporary payload is never an elected installation.
    let mut replacements = Vec::new();
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
    let mut backups = Vec::new();
    let replaced = (|| -> anyhow::Result<()> {
        stop_exact_images(&images)?;
        for (temporary, target) in &replacements {
            let backup = target.with_extension(format!("backup-{}", uuid::Uuid::new_v4().simple()));
            if target.exists() {
                fs::rename(target, &backup)?;
            }
            backups.push((target.clone(), backup));
            fs::rename(temporary, target)?;
        }
        installation::select_locked(runtime, destination, Selection::Release)?;
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
    replaced?;
    drop(guard);
    if let Some(record) = legacy {
        // Migrate the existing owned routes once; later updates leave them fixed.
        let migrated = (|| -> anyhow::Result<()> {
            super::native_host::NativeHostRegistry::discover().install()?;
            let harnesses = super::HarnessRegistry::discover().setup_detected()?;
            anyhow::ensure!(
                harnesses.failures.is_empty(),
                "Could not migrate every owned harness registration; old binaries were retained"
            );
            super::command_path::CommandPath::discover().install()?;
            super::desktop_entry::DesktopIntegration::discover().install()?;
            Ok(())
        })();
        if migrated.is_err() {
            // Keep the previous selection eligible for a migration retry. Both routes
            // still have their binaries if an independent registration already moved.
            let _guard = installation::lock(runtime)?;
            installation::select_locked(runtime, record.directory(), Selection::Release)?;
        }
        migrated?;
        retire_versioned_siblings(record.directory(), destination)?;
    }
    Ok(())
}

fn changed_components(source: &Path, destination: &Path) -> io::Result<Vec<String>> {
    let mut changed = Vec::new();
    for name in COMPONENTS {
        let name = installation::executable_name(name);
        let bytes = fs::read(source.join(&name))?;
        let matches = match fs::read(destination.join(&name)) {
            Ok(installed) => installed == bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        if !matches {
            changed.push(name);
        }
    }
    Ok(changed)
}

fn stop_exact_images(images: &[PathBuf]) -> anyhow::Result<()> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    let mut stopped = Vec::new();
    for process in system.processes().values() {
        let Some(executable) = process.exe().and_then(|path| fs::canonicalize(path).ok()) else {
            continue;
        };
        if images
            .iter()
            .any(|image| fs::canonicalize(image).is_ok_and(|path| path == executable))
        {
            anyhow::ensure!(
                process.pid().as_u32() != std::process::id(),
                "Cannot replace the running installer"
            );
            anyhow::ensure!(
                process.kill(),
                "Could not stop Ghostlight process {}",
                process.pid()
            );
            stopped.push(process.pid());
        }
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while !stopped.is_empty() {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&stopped),
            true,
            ProcessRefreshKind::nothing(),
        );
        stopped.retain(|pid| {
            system
                .process(*pid)
                .is_some_and(|process| process.status() != sysinfo::ProcessStatus::Zombie)
        });
        anyhow::ensure!(
            stopped.is_empty() || Instant::now() < deadline,
            "Ghostlight processes did not finish their replacement"
        );
        if !stopped.is_empty() {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    Ok(())
}

fn retire_versioned_siblings(previous: &Path, destination: &Path) -> io::Result<()> {
    // Only the former package layout under this exact bin root is eligible. History,
    // diagnostics, unknown files and arbitrary portable installations stay untouched.
    if previous.parent() != Some(destination)
        || !previous
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.strip_prefix('v').is_some_and(|version| {
                    version.split('.').count() == 3
                        && version.split('.').all(|part| {
                            !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())
                        })
                })
            })
    {
        return Ok(());
    }
    for name in COMPONENTS {
        match fs::remove_file(previous.join(installation::executable_name(name))) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    // Remove only an empty directory; retained state is deliberately preserved.
    let _ = fs::remove_dir(previous);
    Ok(())
}

/// Select this executable set while preserving development custody during package installation.
pub fn select(selection: Selection) -> io::Result<Option<Installation>> {
    if env::var_os("GHOSTLIGHT_RUNTIME_FILE").is_some() {
        // Explicit fixtures retain their private election; never edit the production record.
        return Ok(None);
    }
    let runtime = installation::production_runtime()?;
    let guard = installation::lock(&runtime)?;
    let current = env::current_exe()?;
    let directory = current
        .parent()
        .ok_or_else(|| io::Error::other("executable has no directory"))?;
    let record = installation::select_locked(&runtime, directory, selection)?;
    drop(guard);
    Ok(Some(record))
}

/// An inactive package may remove its artifacts, but cannot unregister the user's active route.
pub fn owns_registration() -> io::Result<bool> {
    if env::var_os("GHOSTLIGHT_RUNTIME_FILE").is_some() {
        return Ok(true);
    }
    let runtime = installation::production_runtime()?;
    let Some(record) = installation::read(&runtime)? else {
        return Ok(true);
    };
    let current = env::current_exe()?;
    let directory = current
        .parent()
        .ok_or_else(|| io::Error::other("executable has no directory"))?;
    Ok(std::fs::canonicalize(directory)? == std::fs::canonicalize(record.directory())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = env::temp_dir().join(format!(
            "ghostlight-fixed-install-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let source = root.join("download");
        let destination = root.join("bin");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        for name in COMPONENTS {
            fs::write(source.join(installation::executable_name(name)), name).unwrap();
        }
        (
            root,
            fs::canonicalize(source).unwrap(),
            fs::canonicalize(destination).unwrap(),
        )
    }

    #[test]
    fn repeated_package_updates_keep_one_directory_and_preserve_state() {
        let (root, source, destination) = fixture();
        let runtime = root.join("ghostlight-runtime.json");
        install_download_locked(&source, &destination, &runtime).unwrap();
        let first = installation::read(&runtime).unwrap().unwrap();
        fs::write(
            first.state_directory.join("audit.jsonl"),
            "retained history",
        )
        .unwrap();
        fs::write(
            source.join(installation::executable_name("ghostlight")),
            "next service",
        )
        .unwrap();
        assert_eq!(
            changed_components(&source, &destination).unwrap(),
            vec![installation::executable_name("ghostlight")]
        );
        install_download_locked(&source, &destination, &runtime).unwrap();
        assert_eq!(installation::read(&runtime).unwrap().unwrap(), first);
        assert_eq!(
            fs::read_to_string(first.state_directory.join("audit.jsonl")).unwrap(),
            "retained history"
        );
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 3);
        assert!(changed_components(&source, &destination)
            .unwrap()
            .is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fixed_package_updates_preserve_development_custody() {
        let (root, source, destination) = fixture();
        let runtime = root.join("ghostlight-runtime.json");
        let before =
            installation::select_locked(&runtime, &source, Selection::Development).unwrap();
        install_download_locked(&source, &destination, &runtime).unwrap();
        let after = installation::read(&runtime).unwrap().unwrap();
        assert_eq!(after.directory(), before.directory());
        assert_eq!(after.state_directory, before.state_directory);
        assert_eq!(
            after.release_directory,
            Some(fs::canonicalize(&destination).unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_installation_record_cannot_migrate_user_registrations() {
        let (root, source, destination) = fixture();
        let runtime = root.join("ghostlight-runtime.json");
        let before = installation::select_locked(&runtime, &source, Selection::Release).unwrap();
        let error = install_download_locked(&source, &destination, &runtime).unwrap_err();
        assert!(error.to_string().contains("private installation record"));
        assert_eq!(installation::read(&runtime).unwrap().unwrap(), before);
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migration_removes_only_old_package_binaries_and_keeps_history() {
        let (root, source, destination) = fixture();
        let old = destination.join("v1.3.11");
        fs::create_dir(&old).unwrap();
        for name in COMPONENTS {
            fs::copy(
                source.join(installation::executable_name(name)),
                old.join(installation::executable_name(name)),
            )
            .unwrap();
        }
        fs::write(old.join("audit.jsonl"), "old history").unwrap();
        retire_versioned_siblings(&source, &destination).unwrap();
        installation::validate_siblings(&source).unwrap();
        retire_versioned_siblings(&old, &destination).unwrap();
        assert_eq!(
            fs::read_to_string(old.join("audit.jsonl")).unwrap(),
            "old history"
        );
        assert_eq!(fs::read_dir(&old).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
