//! Explicit Flatpak Chromium registration and narrowly owned activation-permission custody.
//!
//! This opt-in package seam never restarts a browser or participates in aggregate detection.
//! It preserves prior permissions and refuses malformed or foreign installation artifacts.

use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use ghostlight_bridge::desktop_activation::{
    registration_bytes, registration_path, BUS_NAME, FLATPAK_APP,
};
use serde::{Deserialize, Serialize};

use super::native_host;

const LIMIT: u64 = 64 * 1024;
const PERMISSION_GROUP: &str = "Session Bus Policy";

/// Read-only state of the explicit Flatpak route; configured does not imply a hot sandbox grant.
#[derive(Debug, Serialize)]
pub struct FlatpakReport {
    /// Exact installed authority selected by this check.
    pub executable: PathBuf,
    /// Both native and activation registrations name this installation.
    pub registration_current: bool,
    /// The persisted per-user app override explicitly grants the one required talk name.
    pub permission_configured: bool,
    /// Human-readable state including the first-sandbox restart qualification.
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Custody {
    version: u8,
    executable: PathBuf,
    permission_before: Option<String>,
    permission_after: Option<String>,
    permission_added: bool,
}

/// The one host installation selected for explicit Flatpak setup.
pub struct FlatpakRegistry {
    home: PathBuf,
    executable: PathBuf,
    runtime: Option<PathBuf>,
    refresh: fn() -> io::Result<()>,
}

impl FlatpakRegistry {
    /// Resolve only the current home-resident installation and default host XDG data root.
    pub fn discover() -> io::Result<Self> {
        let home =
            PathBuf::from(std::env::var_os("HOME").ok_or_else(|| invalid("HOME is unavailable"))?);
        let executable = std::env::current_exe()?;
        if std::env::var_os("FLATPAK_ID").is_some()
            || !executable.starts_with(&home)
            || std::env::var_os("XDG_DATA_HOME")
                .is_some_and(|root| Path::new(&root) != home.join(".local/share"))
            || std::env::var_os("GHOSTLIGHT_NATIVE_HOST_DIR").is_some()
            || std::env::var_os("GHOSTLIGHT_RUNTIME_FILE").is_some()
        {
            return Err(invalid("Flatpak setup requires the host's home-resident installation and default data/runtime roots"));
        }
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| {
                invalid(
                    "the host session runtime directory is required to check activation precedence",
                )
            })?;
        Ok(Self {
            home,
            executable,
            runtime: Some(runtime),
            refresh: ghostlight_bridge::desktop_activation::refresh_registration_cache,
        })
    }

    fn connector(&self) -> PathBuf {
        self.executable
            .with_file_name("ghostlight-browser-connector")
    }

    fn manifest(&self) -> PathBuf {
        self.home
            .join(".var/app")
            .join(FLATPAK_APP)
            .join("config/chromium/NativeMessagingHosts")
            .join(format!("{}.json", native_host::HOST_NAME))
    }

    fn permission(&self) -> PathBuf {
        self.home
            .join(".local/share/flatpak/overrides")
            .join(FLATPAK_APP)
    }

    fn custody(&self) -> PathBuf {
        self.home
            .join(".local/share/ghostlight/flatpak-activation.json")
    }

    fn lock(&self) -> io::Result<fs::File> {
        let path = self.custody().with_extension("lock");
        fs::create_dir_all(path.parent().unwrap())?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        FileExt::try_lock_exclusive(&file)?;
        Ok(file)
    }

    /// Inspect persisted registration and the explicit per-user grant without starting anything.
    pub fn check(&self) -> io::Result<FlatpakReport> {
        self.verify_precedence()?;
        let activation = read(&registration_path(&self.home))?;
        let manifest = read(&self.manifest())?;
        let permission = read(&self.permission())?;
        let registration_current = activation.as_deref()
            == Some(registration_bytes(&self.executable)?.as_slice())
            && manifest.as_deref()
                == Some(
                    native_host::manifest_bytes(&self.connector())
                        .map_err(other)?
                        .as_slice(),
                );
        let permission_configured = has_permission(permission.as_deref())?;
        Ok(FlatpakReport {
            executable: self.executable.clone(), registration_current, permission_configured,
            detail: if registration_current && permission_configured {
                "Configured. A browser sandbox opened before the grant needs a restart for browser-led cold activation; warm connections remain usable."
            } else { "Not fully configured. Explicit Flatpak registration and an app-wide named activation grant are required." }.into(),
        })
    }

    /// Install exact owned registrations; adding the app-wide grant requires separate explicit consent.
    pub fn install(&self, allow_activation: bool) -> io::Result<FlatpakReport> {
        let _lock = self.lock()?;
        self.verify_precedence()?;
        if !self.executable.is_file() || !self.connector().is_file() {
            return Err(invalid(
                "the installed authority and sibling browser connector must exist",
            ));
        }
        let activation_path = registration_path(&self.home);
        let activation = read(&activation_path)?;
        let manifest = read(&self.manifest())?;
        let permission = read(&self.permission())?;
        let custody_before = read(&self.custody())?;
        if activation
            .as_deref()
            .is_some_and(|bytes| registered_executable(bytes).is_none())
            || manifest
                .as_deref()
                .is_some_and(|bytes| native_host::owned_manifest_connector(bytes).is_none())
        {
            return Err(invalid(
                "foreign or malformed Flatpak registration was preserved",
            ));
        }
        let existing_custody = custody_before.as_deref().map(parse_custody).transpose()?;
        if let Some(custody) = &existing_custody {
            if activation
                .as_deref()
                .and_then(registered_executable)
                .as_ref()
                != Some(&custody.executable)
                || manifest.as_deref().is_some_and(|bytes| {
                    native_host::owned_manifest_connector(bytes)
                        != Some(
                            custody
                                .executable
                                .with_file_name("ghostlight-browser-connector"),
                        )
                })
            {
                return Err(invalid(
                    "activation custody does not match its registrations; preserved",
                ));
            }
        }
        let already_allowed = has_permission(permission.as_deref())?;
        if !already_allowed && !allow_activation {
            return Err(invalid("explicit --allow-flatpak-activation is required: this grants the entire Chromium app permission to start Ghostlight, not general host execution"));
        }
        let permission_after = if already_allowed {
            permission.clone()
        } else {
            Some(set_permission(permission.as_deref(), Some("talk"))?)
        };
        let custody = if let Some(mut previous) = existing_custody {
            previous.executable = self.executable.clone();
            if !already_allowed {
                previous.permission_before = as_text(permission.clone())?;
                previous.permission_after = as_text(permission_after.clone())?;
                previous.permission_added = true;
            }
            previous
        } else {
            Custody {
                version: 1,
                executable: self.executable.clone(),
                permission_before: as_text(permission.clone())?,
                permission_after: as_text(permission_after.clone())?,
                permission_added: !already_allowed,
            }
        };
        let changes = vec![
            Change::new(self.permission(), permission, permission_after),
            Change::new(
                activation_path,
                activation,
                Some(registration_bytes(&self.executable)?),
            ),
            Change::new(
                self.manifest(),
                manifest,
                Some(native_host::manifest_bytes(&self.connector()).map_err(other)?),
            ),
            Change::new(
                self.custody(),
                custody_before,
                Some(serde_json::to_vec_pretty(&custody).map_err(other)?),
            ),
        ];
        apply(&changes)?;
        self.refresh_bus()?;
        self.check()
    }

    /// Remove only this installation's registrations and still-identifiable added permission.
    pub fn uninstall(&self) -> io::Result<FlatpakReport> {
        let _lock = self.lock()?;
        let custody_before = read(&self.custody())?;
        let Some(custody_bytes) = custody_before.as_deref() else {
            self.refresh_bus()?;
            return self.check();
        };
        let custody = parse_custody(custody_bytes)?;
        if custody.executable != self.executable {
            return Err(invalid(
                "another installation owns Flatpak activation; preserved",
            ));
        }
        let activation_path = registration_path(&self.home);
        let activation = read(&activation_path)?;
        let manifest = read(&self.manifest())?;
        if activation
            .as_deref()
            .is_some_and(|bytes| bytes != registration_bytes(&self.executable).unwrap_or_default())
            || manifest.as_deref().is_some_and(|bytes| {
                native_host::owned_manifest_connector(bytes).as_ref() != Some(&self.connector())
            })
        {
            return Err(invalid("changed or foreign registration was preserved"));
        }
        let permission = read(&self.permission())?;
        let permission_after = if !custody.permission_added {
            permission.clone()
        } else if permission.as_deref() == custody.permission_after.as_deref().map(str::as_bytes) {
            custody.permission_before.map(String::into_bytes)
        } else if permission_value(permission.as_deref())?.as_deref() == Some("talk") {
            let prior = permission_value(custody.permission_before.as_deref().map(str::as_bytes))?;
            Some(set_permission(permission.as_deref(), prior.as_deref())?)
        } else {
            permission.clone()
        };
        apply(&[
            Change::new(self.manifest(), manifest, None),
            Change::new(activation_path, activation, None),
            Change::new(self.permission(), permission, permission_after),
            Change::new(self.custody(), custody_before, None),
        ])?;
        self.refresh_bus()?;
        self.check()
    }

    fn refresh_bus(&self) -> io::Result<()> {
        (self.refresh)().map_err(|error| io::Error::other(format!(
            "registration files were updated but the session bus could not refresh them: {error}; repeat the explicit setup command"
        )))
    }

    fn verify_precedence(&self) -> io::Result<()> {
        let primary = registration_path(&self.home);
        let mut roots = vec![primary.parent().unwrap().to_path_buf()];
        if let Some(runtime) = &self.runtime {
            roots.push(runtime.join("dbus-1/services"));
        }
        for root in roots {
            let entries = match fs::read_dir(root) {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            for entry in entries {
                let path = entry?.path();
                if path == primary
                    || path
                        .extension()
                        .is_none_or(|extension| extension != "service")
                {
                    continue;
                }
                let bytes = read(&path)?;
                let Some(bytes) = bytes else {
                    continue;
                };
                if key_file(Some(&bytes))
                    .ok()
                    .and_then(|file| file.string("D-BUS Service", "Name").ok())
                    .is_some_and(|name| name == BUS_NAME)
                {
                    return Err(invalid(
                        "another session activation file can shadow Ghostlight; it was preserved",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn parse_custody(bytes: &[u8]) -> io::Result<Custody> {
    let custody: Custody = serde_json::from_slice(bytes).map_err(other)?;
    if custody.version != 1 || !custody.executable.is_absolute() {
        return Err(invalid("unsupported activation custody was preserved"));
    }
    Ok(custody)
}

fn registered_executable(bytes: &[u8]) -> Option<PathBuf> {
    let text = std::str::from_utf8(bytes).ok()?;
    let command = text
        .strip_prefix(&format!("[D-BUS Service]\nName={BUS_NAME}\nExec="))?
        .strip_suffix('\n')?;
    let arguments = glib::shell_parse_argv(command).ok()?;
    if arguments.len() != 1 {
        return None;
    }
    let executable = PathBuf::from(&arguments[0]);
    (registration_bytes(&executable).ok()?.as_slice() == bytes).then_some(executable)
}

fn key_file(bytes: Option<&[u8]>) -> io::Result<glib::KeyFile> {
    let file = glib::KeyFile::new();
    if let Some(bytes) = bytes {
        file.load_from_data(
            std::str::from_utf8(bytes).map_err(other)?,
            glib::KeyFileFlags::KEEP_COMMENTS,
        )
        .map_err(other)?;
    }
    Ok(file)
}

fn permission_value(bytes: Option<&[u8]>) -> io::Result<Option<String>> {
    let file = key_file(bytes)?;
    if !file.has_group(PERMISSION_GROUP)
        || !file.has_key(PERMISSION_GROUP, BUS_NAME).map_err(other)?
    {
        return Ok(None);
    }
    file.string(PERMISSION_GROUP, BUS_NAME)
        .map(|value| Some(value.to_string()))
        .map_err(other)
}

fn has_permission(bytes: Option<&[u8]>) -> io::Result<bool> {
    Ok(matches!(
        permission_value(bytes)?.as_deref(),
        Some("talk" | "own")
    ))
}

fn set_permission(bytes: Option<&[u8]>, value: Option<&str>) -> io::Result<Vec<u8>> {
    let file = key_file(bytes)?;
    if let Some(value) = value {
        file.set_string(PERMISSION_GROUP, BUS_NAME, value);
    } else if file.has_group(PERMISSION_GROUP)
        && file.has_key(PERMISSION_GROUP, BUS_NAME).map_err(other)?
    {
        file.remove_key(PERMISSION_GROUP, BUS_NAME).map_err(other)?;
    }
    Ok(file.to_data().as_bytes().to_vec())
}

fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err(invalid(
            "non-regular or oversized installation file was preserved",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(invalid("installation file exceeded its bound"));
    }
    Ok(Some(bytes))
}

struct Change {
    path: PathBuf,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
}
impl Change {
    fn new(path: PathBuf, before: Option<Vec<u8>>, after: Option<Vec<u8>>) -> Self {
        Self {
            path,
            before,
            after,
        }
    }
}

fn apply(changes: &[Change]) -> io::Result<()> {
    for change in changes {
        if change
            .after
            .as_ref()
            .is_some_and(|bytes| bytes.len() as u64 > LIMIT)
        {
            return Err(invalid(
                "installation custody exceeds its bounded file size; nothing changed",
            ));
        }
        if read(&change.path)? != change.before {
            return Err(invalid("installation changed during inspection; preserved"));
        }
    }
    for (index, change) in changes.iter().enumerate() {
        if let Err(error) = replace(&change.path, &change.before, &change.after) {
            for previous in changes[..index].iter().rev() {
                let _ = replace(&previous.path, &previous.after, &previous.before);
            }
            return Err(error);
        }
    }
    Ok(())
}

fn replace(path: &Path, before: &Option<Vec<u8>>, after: &Option<Vec<u8>>) -> io::Result<()> {
    if before == after {
        return Ok(());
    }
    if read(path)? != *before {
        return Err(invalid("installation changed before write; preserved"));
    }
    if let Some(bytes) = after {
        fs::create_dir_all(path.parent().unwrap())?;
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            if read(path)? != *before {
                return Err(invalid(
                    "installation changed before publication; preserved",
                ));
            }
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    } else {
        fs::remove_file(path)
    }
}

fn as_text(bytes: Option<Vec<u8>>) -> io::Result<Option<String>> {
    bytes.map(String::from_utf8).transpose().map_err(other)
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn other(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> FlatpakRegistry {
        let home = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!("flatpak-install-{}", uuid::Uuid::new_v4()));
        let executable = home.join("selected/ghostlight");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"test fixture, never executed").unwrap();
        fs::write(
            executable.with_file_name("ghostlight-browser-connector"),
            b"fixture",
        )
        .unwrap();
        let home = home.canonicalize().unwrap();
        let executable = home.join("selected/ghostlight");
        FlatpakRegistry {
            home,
            executable,
            runtime: None,
            refresh: || Ok(()),
        }
    }

    fn write_permission(registry: &FlatpakRegistry, bytes: &[u8]) {
        fs::create_dir_all(registry.permission().parent().unwrap()).unwrap();
        fs::write(registry.permission(), bytes).unwrap();
    }

    #[test]
    fn adding_permission_requires_explicit_selection() {
        let registry = fixture();
        assert!(registry.install(false).is_err());
        assert!(!registry.permission().exists());
        assert!(!registry.manifest().exists());
        assert!(!registration_path(&registry.home).exists());
        fs::remove_dir_all(registry.home).unwrap();
    }

    #[test]
    fn install_repeat_remove_reinstall_preserves_original_permissions_exactly() {
        let registry = fixture();
        let original = b"# owned by the person\n[Session Bus Policy]\norg.example.Other=talk\n[Context]\nfilesystems=xdg-download;\n";
        write_permission(&registry, original);
        let report = registry.install(true).unwrap();
        assert!(report.registration_current && report.permission_configured);
        let installed = read(&registry.permission()).unwrap();
        let custody = read(&registry.custody()).unwrap();
        registry.install(false).unwrap();
        assert_eq!(read(&registry.permission()).unwrap(), installed);
        assert_eq!(read(&registry.custody()).unwrap(), custody);
        assert!(std::str::from_utf8(installed.as_ref().unwrap())
            .unwrap()
            .contains("# owned by the person"));
        registry.uninstall().unwrap();
        assert_eq!(fs::read(registry.permission()).unwrap(), original);
        assert!(!registry.manifest().exists());
        assert!(!registration_path(&registry.home).exists());
        registry.uninstall().unwrap();
        registry.install(true).unwrap();
        registry.uninstall().unwrap();
        assert_eq!(fs::read(registry.permission()).unwrap(), original);
        fs::remove_dir_all(registry.home).unwrap();
    }

    #[test]
    fn preexisting_grant_and_later_unrelated_permission_edits_survive_removal() {
        let registry = fixture();
        let original = format!("[Session Bus Policy]\n{BUS_NAME}=talk\n");
        write_permission(&registry, original.as_bytes());
        registry.install(false).unwrap();
        registry.uninstall().unwrap();
        assert_eq!(
            fs::read(registry.permission()).unwrap(),
            original.as_bytes()
        );
        fs::remove_file(registry.permission()).unwrap();
        registry.install(true).unwrap();
        let mut changed = fs::read_to_string(registry.permission()).unwrap();
        changed.push_str("org.example.New=talk\n");
        write_permission(&registry, changed.as_bytes());
        registry.uninstall().unwrap();
        let remaining = fs::read(registry.permission()).unwrap();
        assert!(!has_permission(Some(&remaining)).unwrap());
        assert_eq!(
            key_file(Some(&remaining))
                .unwrap()
                .string(PERMISSION_GROUP, "org.example.New")
                .unwrap(),
            "talk"
        );
        fs::remove_dir_all(registry.home).unwrap();
    }

    #[test]
    fn foreign_malformed_and_symlinked_artifacts_are_never_overwritten() {
        for bytes in [
            b"foreign".as_slice(),
            b"[D-BUS Service]\nName=org.sylin.ghostlight\nExec=/usr/bin/other\n".as_slice(),
        ] {
            let registry = fixture();
            let path = registration_path(&registry.home);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
            assert!(registry.install(true).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert!(!registry.permission().exists());
            fs::remove_dir_all(registry.home).unwrap();
        }
        let registry = fixture();
        let target = registry.home.join("foreign");
        fs::write(&target, b"foreign").unwrap();
        fs::create_dir_all(registry.manifest().parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&target, registry.manifest()).unwrap();
        assert!(registry.install(true).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"foreign");
        fs::remove_dir_all(registry.home).unwrap();
    }

    #[test]
    fn upgrade_is_explicit_and_old_installation_cannot_remove_new_ownership() {
        let old = fixture();
        old.install(true).unwrap();
        let new = FlatpakRegistry {
            home: old.home.clone(),
            executable: old.home.join("upgrade/ghostlight"),
            runtime: None,
            refresh: old.refresh,
        };
        fs::create_dir_all(new.executable.parent().unwrap()).unwrap();
        fs::write(&new.executable, b"fixture").unwrap();
        fs::write(new.connector(), b"fixture").unwrap();
        assert!(new.uninstall().is_err());
        new.install(false).unwrap();
        assert!(old.uninstall().is_err());
        new.uninstall().unwrap();
        assert!(!new.permission().exists());
        fs::remove_dir_all(old.home).unwrap();
    }

    #[test]
    fn malformed_permission_and_changed_custody_are_preserved() {
        let registry = fixture();
        write_permission(&registry, b"not an ini document");
        assert!(registry.install(true).is_err());
        assert_eq!(
            fs::read(registry.permission()).unwrap(),
            b"not an ini document"
        );
        fs::remove_file(registry.permission()).unwrap();
        registry.install(true).unwrap();
        fs::write(registry.custody(), b"foreign custody").unwrap();
        assert!(registry.uninstall().is_err());
        assert!(registry.install(true).is_err());
        assert!(registry.manifest().exists());
        fs::remove_dir_all(registry.home).unwrap();
    }

    #[test]
    fn duplicate_and_runtime_precedence_registrations_block_setup_without_changes() {
        for runtime_shadow in [false, true] {
            let mut registry = fixture();
            let path = if runtime_shadow {
                let runtime = registry.home.join("runtime");
                registry.runtime = Some(runtime.clone());
                runtime.join("dbus-1/services/org.sylin.ghostlight.service")
            } else {
                registration_path(&registry.home).with_file_name("foreign.service")
            };
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let bytes = registration_bytes(Path::new("/foreign/ghostlight")).unwrap();
            fs::write(&path, &bytes).unwrap();
            assert!(registry.install(true).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert!(!registry.permission().exists());
            assert!(!registry.manifest().exists());
            fs::remove_dir_all(registry.home).unwrap();
        }
    }

    #[test]
    fn changed_native_owner_blocks_upgrade_without_overwriting_it() {
        let registry = fixture();
        registry.install(true).unwrap();
        let other =
            native_host::manifest_bytes(&registry.home.join("other/ghostlight-browser-connector"))
                .unwrap();
        fs::write(registry.manifest(), &other).unwrap();
        let custody = fs::read(registry.custody()).unwrap();
        assert!(registry.install(false).is_err());
        assert_eq!(fs::read(registry.manifest()).unwrap(), other);
        assert_eq!(fs::read(registry.custody()).unwrap(), custody);
        fs::remove_dir_all(registry.home).unwrap();
    }

    #[test]
    fn failed_bus_refresh_reports_persisted_state_and_repeat_repairs_it() {
        let mut registry = fixture();
        registry.refresh = || Err(io::Error::other("fixture bus unavailable"));
        let error = registry.install(true).unwrap_err();
        assert!(error
            .to_string()
            .contains("registration files were updated"));
        assert!(registry.check().unwrap().registration_current);
        registry.refresh = || Ok(());
        assert!(registry.install(false).unwrap().registration_current);
        registry.refresh = || Err(io::Error::other("fixture bus unavailable"));
        assert!(registry.uninstall().is_err());
        assert!(!registry.check().unwrap().registration_current);
        registry.refresh = || Ok(());
        registry.uninstall().unwrap();
        fs::remove_dir_all(registry.home).unwrap();
    }
}
