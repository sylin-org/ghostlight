//! The one 2.0 break migration: from any 1.x layout to the fixed home.
//!
//! Before 2.0 an installation record chose where the executables, the state and the policy
//! lived, and each of those could sit in a different place on each platform. 2.0 keeps
//! everything under the Ghostlight home (ADR-0197 Decisions 10, 16 and 22). This module reads
//! the old layout, says in a [`MigrationPlan`] what will move, be archived, be deleted, be
//! stopped and be re-registered, and carries that plan out in steps the installer interleaves
//! with its own:
//!
//! 1. [`prepare`] makes every destination and refuses a destination that holds other content, so
//!    a conflict is found before anything has changed;
//! 2. [`copy_state`] copies the policy, the managed-policy files and the client locations to where
//!    2.0 keeps them, writes the setup record for a home that was in use, before the new executables are swapped in and
//!    registered, and leaves every original in place: a 1.x service is still running and reads
//!    the user's policy from its original on every decision, and a 2.0 service that a browser or
//!    client starts the moment the registrations change must find the same files at their new
//!    place. Beside each copy it records the content that was copied;
//! 3. [`archive_audit`] moves the old audit log aside once the programs that write it have
//!    stopped;
//! 4. [`retire_state`] removes those originals once the last 1.x program has stopped and cannot
//!    write them again. A 1.x program may have rewritten an original in the meantime (its policy
//!    workbench saves the user policy, its managed-policy refresh rewrites the status and cache),
//!    and a 2.0 service may have rewritten a copy, so each original is compared with the content
//!    that was copied: an original that changed is published over a copy that did not, and one
//!    that diverged from a changed copy is set aside under a name no plan reads, so the next
//!    install is never blocked by it;
//! 5. [`finish`] deletes the old lifecycle files, the installation record last, once the last
//!    old program has stopped and cannot write them again;
//! 6. [`retire_flatpak_route`] removes what the 1.x Flatpak Chromium route left in the user's
//!    home: the Flatpak browser's host manifest, the D-Bus activation registration, and the
//!    permission grant the route added to the browser's override file. Each is touched only
//!    when its content is still the route's own, and every step is reported; the install never
//!    fails on a leftover.
//!
//! The plan is data: `deployment install --plan` prints it. Stopping the old programs is the
//! installer's step, not this module's: it must happen after the registrations name the new
//! executables, or a browser relaunches an old connector from an old registration. A failure
//! leaves the source file where it was. A run that fails after the copies and before the
//! originals are retired leaves both in place, and either may change before the next run; the
//! record beside each copy tells that run what was copied, so it retires the originals by the
//! same rule instead of refusing them. A run after success has nothing to do. A destination that
//! holds content this migration did not copy is never overwritten, and an original is deleted
//! only when its content is at the destination.

use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use ghostlight_relay::home::EDGE_EXECUTABLE_NAME;
use ghostlight_relay::private;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use thiserror::Error;

use super::entry;
use super::layout::{retired_connector_names, Layout, INSTALL_RECORD_NAME};
use super::native_host::{self, NativeHostRegistry};
use super::record::{self, InstallRecord, Version};
use super::setup_state::{self, SetupRecord};
use super::{HarnessRegistry, HarnessState};

/// Largest installation record the migration reads.
const LEGACY_RECORD_LIMIT: u64 = 64 * 1024;

/// How long a stopped process may take to exit.
const STOP_WAIT: Duration = Duration::from_secs(10);

/// File name of the 1.x installation record, beside the runtime file in the home.
pub(super) const LEGACY_RECORD_NAME: &str = "ghostlight-runtime.installation.json";

/// File name of the 1.x startup admission file in the home.
pub(super) const LEGACY_STARTUP_NAME: &str = "ghostlight-runtime.startup";

/// File name of the 1.x installation lock in the home.
const LEGACY_INSTALLATION_LOCK_NAME: &str = "ghostlight-runtime.installation.lock";

/// File name of the 1.x deployment lock, kept in the home and in the serving directory.
pub(super) const LEGACY_DEPLOY_LOCK_NAME: &str = "deploy.lock";

/// The audit log, in the 1.x state directory and again in `home/state`.
pub(super) const AUDIT_FILE_NAME: &str = "audit.jsonl";

/// The user policy file, in the 1.x policy directory and again in `home/policy`.
pub(super) const USER_POLICY_FILE_NAME: &str = "user-policy.json";

/// The verified managed-policy cache, in the 1.x policy directory and again in `home/policy`.
const MANAGED_CACHE_FILE_NAME: &str = "managed-policy.bundle";

/// The managed-policy status file, in the 1.x policy directory and again in `home/policy`.
const MANAGED_STATUS_FILE_NAME: &str = "managed-status.json";

/// The 1.x extension walkthrough marker, in the 1.x policy directory. 2.0 records setup in
/// `state/setup.json` instead, so the migration deletes the marker.
const HANDOFF_MARKER_NAME: &str = "extension-handoff-v1";

/// The MCP client location file, in the 1.x configuration directory and again in `home/state`.
pub(super) const HARNESS_LOCATIONS_NAME: &str = "harness-locations.json";

/// Directory below the 1.x configuration root that held the client location file.
const LEGACY_CONFIG_DIRECTORY_NAME: &str = "ghostlight";

/// Directory below the Windows local-data root that held the 1.x policy.
const LEGACY_WINDOWS_POLICY_DIRECTORY_NAME: &str = "Ghostlight";

/// Directory below the Linux state root that held the 1.x policy.
const LEGACY_LINUX_POLICY_DIRECTORY_NAME: &str = "ghostlight";

/// Directory below the Linux cache root that held state beside a `/usr/bin` install.
const LEGACY_CACHE_DIRECTORY_NAME: &str = "ghostlight";

/// Suffix of the name an original takes when it and its copy both changed.
const SET_ASIDE_SUFFIX: &str = "pre-2.0";

/// Suffix of the file beside a copy that records the content that was copied.
const COPY_BASE_SUFFIX: &str = "copied-sha256";

/// Label used in the archived audit file name when the old version cannot be read.
const UNKNOWN_VERSION_LABEL: &str = "pre-2.0";

/// The Flatpak Chromium application the 1.x Flatpak route registered with.
const FLATPAK_CHROMIUM_APP: &str = "org.chromium.Chromium";

/// The host manifest of the Flatpak Chromium route, below the user's home.
const FLATPAK_MANIFEST_RELATIVE: &str =
    ".var/app/org.chromium.Chromium/config/chromium/NativeMessagingHosts/org.sylin.ghostlight.json";

/// The D-Bus session activation registration of the route, below the user's home.
const ACTIVATION_REGISTRATION_RELATIVE: &str =
    ".local/share/dbus-1/services/org.sylin.ghostlight.service";

/// The directory of per-application Flatpak overrides, below the user's home.
const FLATPAK_OVERRIDES_RELATIVE: &str = ".local/share/flatpak/overrides";

/// The record the route kept of the permission it added, below the user's home.
const FLATPAK_CUSTODY_RELATIVE: &str = ".local/share/ghostlight/flatpak-activation.json";

/// The only record version the 1.x Flatpak route wrote.
const FLATPAK_CUSTODY_VERSION: u8 = 1;

/// The file name of the executable the activation registration started.
const ACTIVATION_EXECUTABLE_NAME: &str = "ghostlight";

/// Largest Flatpak route file the migration reads.
const FLATPAK_FILE_LIMIT: u64 = 64 * 1024;

/// Why the migration could not be planned or carried out. Each message names the path involved.
#[derive(Debug, Error)]
pub enum MigrationError {
    /// The old installation record exists but cannot be understood.
    #[error(
        "The old Ghostlight installation record {path} cannot be read ({reason}). Nothing was \
         changed; remove that file only if no Ghostlight 1.x state should be kept"
    )]
    Record {
        /// The record file.
        path: PathBuf,
        /// Why it could not be read.
        reason: String,
    },
    /// The Ghostlight home is a packaged app's private copy rather than the real profile.
    #[error(
        "The Ghostlight home {0} lies inside a packaged app's private storage, not the real \
         profile directory. Nothing was changed"
    )]
    VirtualizedHome(PathBuf),
    /// A destination directory could not be created.
    #[error("Could not prepare {path}: {source}. Nothing was changed")]
    Prepare {
        /// The directory.
        path: PathBuf,
        /// The failure.
        #[source]
        source: io::Error,
    },
    /// A destination holds something other than the file being moved.
    #[error(
        "{destination} holds different content than {source_path}. {disposition}; keep the one \
         you want and remove the other"
    )]
    Conflict {
        /// The file that would have moved.
        source_path: PathBuf,
        /// The destination that is occupied.
        destination: PathBuf,
        /// What became of the original.
        disposition: Disposition,
    },
    /// The setup record of a home that was in use could not be written.
    #[error("Could not write {path}: {source}. Nothing else was changed")]
    SetupRecord {
        /// The record file.
        path: PathBuf,
        /// The failure.
        #[source]
        source: io::Error,
    },
    /// A file could not be moved. The source is intact.
    #[error("Could not move {from} to {to}: {source}. The original is still in place")]
    Move {
        /// The source file.
        from: PathBuf,
        /// The destination file.
        to: PathBuf,
        /// The failure.
        #[source]
        source: io::Error,
    },
    /// A file whose content is already safe at its destination could not be removed.
    #[error("Could not remove {path}: {source}. Its content is already at its new location")]
    Remove {
        /// The file.
        path: PathBuf,
        /// The failure.
        #[source]
        source: io::Error,
    },
}

/// What became of the original when a destination held other content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Disposition {
    /// The original stays where it was and nothing was changed.
    Untouched,
    /// The original was renamed, so no later plan finds it and no later install is blocked.
    SetAside(PathBuf),
}

impl std::fmt::Display for Disposition {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Untouched => formatter.write_str("Nothing was changed"),
            Self::SetAside(kept) => write!(
                formatter,
                "The original was set aside as {}, so the next install is not blocked",
                kept.display()
            ),
        }
    }
}

/// The 1.x installation record, read without the crate that wrote it.
///
/// Every field is optional and unknown fields are ignored, so a record from any 1.x release
/// reads. Nothing here is validated beyond being a path: the migration only ever deletes files
/// it names explicitly.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct LegacyInstallation {
    /// Record format version.
    #[serde(default)]
    pub schema: u32,
    /// Directory the old connectors started the service from.
    #[serde(default)]
    pub serving_directory: Option<PathBuf>,
    /// Directory of the packaged release, kept while a development build served.
    #[serde(default)]
    pub release_directory: Option<PathBuf>,
    /// Directory of the development build, when one was selected.
    #[serde(default)]
    pub development_directory: Option<PathBuf>,
    /// Directory holding the audit log.
    #[serde(default)]
    pub state_directory: Option<PathBuf>,
    /// Directory holding the user policy and the managed-policy files.
    #[serde(default)]
    pub policy_directory: Option<PathBuf>,
}

impl LegacyInstallation {
    /// Read the record at `path`; a missing file reads as `None`.
    pub fn read(path: &Path) -> Result<Option<Self>, MigrationError> {
        let failed = |reason: String| MigrationError::Record {
            path: path.to_owned(),
            reason,
        };
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(failed(error.to_string())),
        };
        let mut bytes = Vec::new();
        file.take(LEGACY_RECORD_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| failed(error.to_string()))?;
        if bytes.len() as u64 > LEGACY_RECORD_LIMIT {
            return Err(failed(
                "it is larger than any record Ghostlight wrote".into(),
            ));
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| failed(error.to_string()))
    }

    /// The absolute executable directories the record names: serving, release, development.
    fn directories(&self) -> Vec<PathBuf> {
        let mut directories = Vec::new();
        for directory in [
            &self.serving_directory,
            &self.release_directory,
            &self.development_directory,
        ]
        .into_iter()
        .flatten()
        .filter(|directory| directory.is_absolute())
        {
            push_unique(&mut directories, directory.clone());
        }
        directories
    }
}

/// The user-level roots the 1.x layout was derived from.
///
/// These are the answers to environment questions, taken once, so the plan itself reads no
/// environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyRoots {
    /// Whether the Windows layout applies.
    pub windows: bool,
    /// The real user profile directory.
    pub user_home: PathBuf,
    /// `LOCALAPPDATA` (Windows).
    pub local_app_data: Option<PathBuf>,
    /// `APPDATA` (Windows).
    pub app_data: Option<PathBuf>,
    /// `XDG_STATE_HOME` (Linux).
    pub xdg_state_home: Option<PathBuf>,
    /// `XDG_CONFIG_HOME` (Linux).
    pub xdg_config_home: Option<PathBuf>,
}

impl LegacyRoots {
    /// The roots this process sees.
    pub fn from_environment() -> io::Result<Self> {
        let windows = cfg!(windows);
        let user_home = env::var_os(if windows { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "the user home directory is unknown",
                )
            })?;
        let absolute = |name: &str| {
            env::var_os(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        Ok(Self {
            windows,
            user_home,
            local_app_data: absolute("LOCALAPPDATA"),
            app_data: absolute("APPDATA"),
            xdg_state_home: absolute("XDG_STATE_HOME"),
            xdg_config_home: absolute("XDG_CONFIG_HOME"),
        })
    }

    /// Roots that all lie inside `home`, so a test home never reads or moves this user's real
    /// 1.x files.
    #[must_use]
    pub fn within(home: &Path) -> Self {
        Self {
            windows: cfg!(windows),
            user_home: home.to_owned(),
            local_app_data: None,
            app_data: None,
            xdg_state_home: None,
            xdg_config_home: None,
        }
    }

    /// A root taken from the environment, or the profile-relative default when the environment
    /// names none or names a packaged app's private copy.
    fn real_root(&self, named: Option<&Path>, default: &[&str]) -> PathBuf {
        match named {
            Some(path) if !is_virtualized(path) => path.to_owned(),
            _ => default
                .iter()
                .fold(self.user_home.clone(), |path, part| path.join(part)),
        }
    }

    /// Where the default 1.x policy directory was.
    fn policy_directory(&self) -> PathBuf {
        if self.windows {
            self.real_root(self.local_app_data.as_deref(), &["AppData", "Local"])
                .join(LEGACY_WINDOWS_POLICY_DIRECTORY_NAME)
        } else {
            self.real_root(self.xdg_state_home.as_deref(), &[".local", "state"])
                .join(LEGACY_LINUX_POLICY_DIRECTORY_NAME)
        }
    }

    /// Where the 1.x client location file was.
    pub(super) fn harness_locations_directory(&self) -> PathBuf {
        let root = if self.windows {
            self.real_root(self.app_data.as_deref(), &["AppData", "Roaming"])
        } else {
            self.real_root(self.xdg_config_home.as_deref(), &[".config"])
        };
        root.join(LEGACY_CONFIG_DIRECTORY_NAME)
    }

    /// Where state was kept beside a Linux system-package install.
    fn cache_directory(&self) -> Option<PathBuf> {
        (!self.windows).then(|| {
            self.user_home
                .join(".cache")
                .join(LEGACY_CACHE_DIRECTORY_NAME)
        })
    }
}

/// What kind of registration the migration found.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationKind {
    /// A browser's native-host manifest.
    NativeHost,
    /// An MCP client's configuration entry.
    McpClient,
}

/// A registration that names an old installation directory and must be rewritten.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Registration {
    /// What kind of registration this is.
    pub kind: RegistrationKind,
    /// The browser or client identifier.
    pub id: String,
    /// The path found: the connector a native-host manifest names, or the client configuration
    /// file that names an old directory.
    pub names: PathBuf,
}

/// Everything the migration needs to know about the 1.x layout.
#[derive(Clone, Debug)]
pub struct Legacy {
    /// The installation record, when one exists.
    pub installation: Option<LegacyInstallation>,
    /// The roots the default 1.x locations derive from.
    pub roots: LegacyRoots,
    /// Registrations found that name an old installation directory.
    pub registrations: Vec<Registration>,
}

impl Legacy {
    /// Read the 1.x layout below `roots`, with the Ghostlight home at `home`.
    ///
    /// Nothing is changed. Registrations come from the browsers' native-host manifests and the
    /// MCP clients' configurations as this user has them.
    pub fn discover(home: &Path, roots: LegacyRoots) -> Result<Self, MigrationError> {
        let installation = LegacyInstallation::read(&home.join(LEGACY_RECORD_NAME))?;
        let layout = Layout::new(home);
        let old_directories: Vec<PathBuf> = installation
            .iter()
            .flat_map(LegacyInstallation::directories)
            .filter(|directory| {
                !same_path(directory, &layout.bin())
                    && !same_path(directory, &layout.retired_development_directory())
            })
            .collect();
        let registrations = find_registrations(&old_directories);
        Ok(Self {
            installation,
            roots,
            registrations,
        })
    }

    /// Directories that may hold the 1.x policy files, most authoritative first.
    fn policy_sources(&self) -> Vec<PathBuf> {
        let mut sources = Vec::new();
        if let Some(recorded) = self
            .installation
            .as_ref()
            .and_then(|record| record.policy_directory.clone())
            .filter(|path| path.is_absolute())
        {
            push_unique(&mut sources, recorded);
        }
        push_unique(&mut sources, self.roots.policy_directory());
        sources
    }

    /// Directories that may hold the 1.x audit log, most authoritative first.
    fn state_sources(&self, home: &Path) -> Vec<PathBuf> {
        if let Some(recorded) = self
            .installation
            .as_ref()
            .and_then(|record| record.state_directory.clone())
            .filter(|path| path.is_absolute())
        {
            return vec![recorded];
        }
        let mut sources = vec![home.to_owned()];
        sources.extend(self.roots.cache_directory());
        sources
    }

    /// The version the old installation reports, when it can be read without running it.
    fn old_version(&self) -> Option<String> {
        let serving = self
            .installation
            .as_ref()?
            .serving_directory
            .as_ref()
            .filter(|path| path.is_absolute())?;
        let recorded = InstallRecord::read(&serving.join(INSTALL_RECORD_NAME))
            .ok()
            .flatten()
            .map(|record| record.service_version);
        let named = serving
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .map(|name| name.strip_prefix(['v', 'V']).unwrap_or(&name).to_owned());
        [recorded, named]
            .into_iter()
            .flatten()
            .find(|version| Version::parse(version).is_some())
    }
}

/// One file to relocate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Move {
    /// Where the file is now.
    pub from: PathBuf,
    /// Where it belongs in 2.0.
    pub to: PathBuf,
}

/// What the break migration will do, in the order it does it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MigrationPlan {
    /// Files moved into `home/state` and `home/policy`.
    pub moves: Vec<Move>,
    /// The old audit log, moved to a file of its own in `home/state` so 2.0 starts a new log.
    pub audit_archive: Option<Move>,
    /// Old lifecycle files deleted, the installation record last. The 1.x walkthrough marker is
    /// among them.
    pub deletions: Vec<PathBuf>,
    /// Where to write the setup record: a home that was in use has already finished setup. It is
    /// set only when a 1.x installation is being migrated and no record exists yet.
    pub setup_record: Option<PathBuf>,
    /// Old executables stopped before their files move.
    pub images_to_stop: Vec<PathBuf>,
    /// Registrations to rewrite to the fixed paths.
    pub registrations_to_reclaim: Vec<Registration>,
    /// Directories read that lie inside a packaged app's private storage.
    pub virtualized_sources: Vec<PathBuf>,
    /// What the 1.x Flatpak route left behind, when Ghostlight's own files are still there.
    pub flatpak: Option<FlatpakLeftovers>,
}

/// The places the 1.x Flatpak Chromium route wrote below the user's home.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FlatpakLeftovers {
    /// The Flatpak browser's native-messaging manifest.
    pub manifest: PathBuf,
    /// The D-Bus session activation registration.
    pub registration: PathBuf,
    /// The browser's Flatpak override file, where the route added a permission.
    pub permission_override: PathBuf,
    /// The record of the permission the route added.
    pub custody: PathBuf,
}

impl FlatpakLeftovers {
    /// The route's files below `user_home`.
    pub(super) fn below(user_home: &Path) -> Self {
        Self {
            manifest: user_home.join(FLATPAK_MANIFEST_RELATIVE),
            registration: user_home.join(ACTIVATION_REGISTRATION_RELATIVE),
            permission_override: user_home
                .join(FLATPAK_OVERRIDES_RELATIVE)
                .join(FLATPAK_CHROMIUM_APP),
            custody: user_home.join(FLATPAK_CUSTODY_RELATIVE),
        }
    }

    /// Whether any of the files is still Ghostlight's own.
    fn present(&self) -> bool {
        let owned = |path: &Path, is_owned: fn(&[u8]) -> bool| matches!(find(path), Ok(Found::Regular(bytes)) if is_owned(&bytes));
        owned(&self.manifest, native_host::names_ghostlight_host)
            || owned(&self.registration, names_ghostlight_activation)
            || owned(&self.custody, |bytes| {
                FlatpakCustody::parse(bytes).is_some()
            })
    }
}

/// The record the 1.x Flatpak route kept of the permission it added to the browser's override.
#[derive(Debug, Deserialize)]
struct FlatpakCustody {
    version: u8,
    permission_before: Option<String>,
    permission_after: Option<String>,
    permission_added: bool,
}

impl FlatpakCustody {
    /// The record in `bytes`, when it is one this route wrote.
    fn parse(bytes: &[u8]) -> Option<Self> {
        let record: Self = serde_json::from_slice(bytes).ok()?;
        (record.version == FLATPAK_CUSTODY_VERSION).then_some(record)
    }
}

/// Which of the route's files a step concerns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlatpakPart {
    /// The Flatpak browser's native-messaging manifest.
    Manifest,
    /// The D-Bus session activation registration.
    Registration,
    /// The browser's Flatpak override file.
    PermissionOverride,
    /// The record of the permission the route added.
    Custody,
}

/// What became of one of the route's files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlatpakResult {
    /// The file was Ghostlight's own and is deleted.
    Removed,
    /// The override file holds what it held before the route added its permission.
    Restored,
    /// The file is not Ghostlight's own and was left as it is.
    LeftForeign,
    /// The override file changed after the route wrote it and was left as it is.
    LeftChanged,
    /// The file could not be read or changed; the next install tries again.
    Failed(String),
}

/// One reported step of [`retire_flatpak_route`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatpakStep {
    /// Which file.
    pub part: FlatpakPart,
    /// Where it is.
    pub path: PathBuf,
    /// What became of it.
    pub result: FlatpakResult,
}

impl MigrationPlan {
    /// Whether there is nothing to do.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.has_earlier_installation() && self.flatpak.is_none()
    }

    /// Whether a 1.x installation left files, programs or registrations to move or retire.
    #[must_use]
    pub fn has_earlier_installation(&self) -> bool {
        !(self.moves.is_empty()
            && self.audit_archive.is_none()
            && self.deletions.is_empty()
            && self.setup_record.is_none()
            && self.images_to_stop.is_empty()
            && self.registrations_to_reclaim.is_empty())
    }

    /// Every file transfer, the archive last.
    fn transfers(&self) -> impl Iterator<Item = &Move> {
        self.moves.iter().chain(self.audit_archive.iter())
    }
}

/// Work out what migrating the `legacy` layout into the Ghostlight `home` involves.
///
/// Only files that exist are listed, so a plan for a home that holds no 1.x leftovers is empty.
/// The plan reads the disk and changes nothing.
pub fn plan(home: &Path, legacy: &Legacy) -> Result<MigrationPlan, MigrationError> {
    if is_virtualized(home) {
        return Err(MigrationError::VirtualizedHome(home.to_owned()));
    }
    let layout = Layout::new(home);
    let mut plan = MigrationPlan::default();
    let mut virtualized = Vec::new();

    let policy_sources = legacy.policy_sources();
    let state_sources = legacy.state_sources(home);
    let location_sources = [legacy.roots.harness_locations_directory()];
    let placements: [(&str, &[PathBuf], PathBuf); 4] = [
        (
            USER_POLICY_FILE_NAME,
            policy_sources.as_slice(),
            layout.policy(),
        ),
        (
            MANAGED_CACHE_FILE_NAME,
            policy_sources.as_slice(),
            layout.policy(),
        ),
        (
            MANAGED_STATUS_FILE_NAME,
            policy_sources.as_slice(),
            layout.policy(),
        ),
        (
            HARNESS_LOCATIONS_NAME,
            location_sources.as_slice(),
            layout.state(),
        ),
    ];
    for (name, sources, destination) in placements {
        if let Some(directory) = holding(sources, name) {
            if same_location(directory, &destination) {
                // The file already sits where 2.0 keeps it: 1.x recorded the 2.0 directory.
                continue;
            }
            note_virtualized(&mut virtualized, directory);
            plan.moves.push(Move {
                from: directory.join(name),
                to: destination.join(name),
            });
        }
    }
    if let Some(directory) = holding(&state_sources, AUDIT_FILE_NAME) {
        note_virtualized(&mut virtualized, directory);
        let label = legacy
            .old_version()
            .map_or_else(|| UNKNOWN_VERSION_LABEL.to_owned(), |v| format!("pre-{v}"));
        plan.audit_archive = Some(Move {
            from: directory.join(AUDIT_FILE_NAME),
            to: layout.state().join(format!("audit-{label}.jsonl")),
        });
    }
    let marker = holding(&policy_sources, HANDOFF_MARKER_NAME).map(|directory| {
        note_virtualized(&mut virtualized, directory);
        directory.join(HANDOFF_MARKER_NAME)
    });
    plan.virtualized_sources = virtualized;

    let serving = legacy
        .installation
        .as_ref()
        .and_then(|record| record.serving_directory.clone())
        .filter(|directory| directory.is_absolute());
    let mut deletions = vec![
        home.join(LEGACY_STARTUP_NAME),
        home.join(LEGACY_DEPLOY_LOCK_NAME),
    ];
    deletions.extend(
        serving
            .iter()
            .filter(|directory| !same_path(directory, home))
            .map(|directory| directory.join(LEGACY_DEPLOY_LOCK_NAME)),
    );
    deletions.push(home.join(LEGACY_INSTALLATION_LOCK_NAME));
    deletions.extend(marker.iter().cloned());
    deletions.extend(retired_connectors(&layout));
    // The record decides the next run's plan, so it goes last.
    deletions.push(home.join(LEGACY_RECORD_NAME));
    plan.deletions = deletions
        .into_iter()
        .filter(|path| path.is_file())
        .collect();

    for directory in legacy
        .installation
        .iter()
        .flat_map(LegacyInstallation::directories)
    {
        // 1.x named its service image after the product, which is now the edge's name.
        let service = EDGE_EXECUTABLE_NAME.to_owned();
        for name in std::iter::once(service).chain(retired_connector_names()) {
            let image = directory.join(name);
            if image.is_file() {
                push_unique(&mut plan.images_to_stop, image);
            }
        }
    }
    for image in pre_edge_service_images(&layout)
        .into_iter()
        .chain(retired_connectors(&layout))
    {
        push_unique(&mut plan.images_to_stop, image);
    }
    let was_in_use = legacy.installation.is_some()
        || marker.is_some()
        || !plan.moves.is_empty()
        || plan.audit_archive.is_some();
    plan.setup_record = (was_in_use && !setup_state::is_complete(&layout.setup_record()))
        .then(|| layout.setup_record());
    plan.registrations_to_reclaim = legacy.registrations.clone();
    plan.flatpak =
        Some(FlatpakLeftovers::below(&legacy.roots.user_home)).filter(FlatpakLeftovers::present);
    Ok(plan)
}

/// The retired connector images that exist in `layout`'s `bin`.
fn retired_connectors(layout: &Layout) -> Vec<PathBuf> {
    retired_connector_names()
        .into_iter()
        .map(|name| layout.bin().join(name))
        .filter(|path| path.is_file())
        .collect()
}

/// Whether `bin` still holds the layout of an earlier 2.0 development build, where the service
/// image had the edge's name: connector images beside it, or an install record that names no
/// edge.
pub fn pre_edge_layout(layout: &Layout) -> bool {
    !retired_connectors(layout).is_empty()
        || matches!(
            InstallRecord::read(&layout.install_record()),
            Ok(Some(record)) if record.edge_sha256.is_none()
        )
}

/// The service image of the earlier layout that sits under the edge's name in `bin`, as a file
/// that exists, while `bin` holds that layout. The earlier development directory is cleaned by the
/// installer's own plan, which stops every image in it.
fn pre_edge_service_images(layout: &Layout) -> Vec<PathBuf> {
    Some(layout.edge())
        .filter(|image| pre_edge_layout(layout) && image.is_file())
        .into_iter()
        .collect()
}

/// Make every destination directory and refuse a destination that holds something else.
///
/// Everything that can fail before a file is touched is checked here, so the installer calls it
/// before it changes anything.
pub fn prepare(plan: &MigrationPlan) -> Result<(), MigrationError> {
    plan.transfers().try_for_each(prepare_transfer)
}

/// A file copied to where 2.0 keeps it, and the content that was copied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Copied {
    /// The original and its new place.
    transfer: Move,
    /// SHA-256 of the content at the new place when the copy was made.
    sha256: String,
}

/// Copy the policy, managed-policy and client-location files of `plan` to where 2.0 keeps them,
/// and leave every original in place. A home that was in use also gets its setup record, dated
/// now: a person who used 1.x has finished setup.
///
/// A 1.x service is still running and reads the user policy from its original on every decision,
/// so the originals stay until [`retire_state`] runs after the last 1.x program has stopped. A
/// failure returns an error naming the path, and running the same plan again continues from
/// there. A copy an earlier run made, and a program has changed since, is adopted: the record
/// beside it names the content that was copied.
pub fn copy_state(plan: &MigrationPlan) -> Result<Vec<Copied>, MigrationError> {
    if let Some(path) = &plan.setup_record {
        setup_state::record_if_absent(path, &SetupRecord::now()).map_err(|source| {
            MigrationError::SetupRecord {
                path: path.clone(),
                source,
            }
        })?;
    }
    let mut copied = Vec::new();
    for transfer in &plan.moves {
        if let Some(done) = copy_transfer(transfer)? {
            copied.push(done);
        }
    }
    Ok(copied)
}

/// Remove the originals of the files [`copy_state`] copied.
///
/// Called once the last 1.x program has stopped. An original that still holds the copied content
/// is deleted. One that a 1.x program rewrote since is published over the copy and then deleted,
/// unless the copy changed too: then the original is set aside beside where it was and a
/// [`MigrationError::Conflict`] names both. Every file is handled before the first conflict is
/// returned.
pub fn retire_state(copied: &[Copied]) -> Result<(), MigrationError> {
    let mut first_conflict = None;
    for done in copied {
        match retire_transfer(done) {
            Ok(()) => {}
            Err(error @ MigrationError::Conflict { .. }) => {
                first_conflict.get_or_insert(error);
            }
            Err(error) => return Err(error),
        }
    }
    first_conflict.map_or(Ok(()), Err)
}

/// Move the old audit log of `plan` aside, to the archive file that gives 2.0 a new log.
///
/// The old programs that write the log must already be stopped. A failure leaves the log where
/// it was, and running the same plan again continues from there.
pub fn archive_audit(plan: &MigrationPlan) -> Result<(), MigrationError> {
    plan.audit_archive.iter().try_for_each(archive_transfer)
}

/// Delete the old lifecycle files of `plan`, the installation record last.
///
/// Called once the last old program has stopped: a program that is still running, or one a
/// browser relaunches from an old registration, writes the record again.
pub fn finish(plan: &MigrationPlan) -> Result<(), MigrationError> {
    for path in &plan.deletions {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(MigrationError::Remove {
                    path: path.clone(),
                    source,
                })
            }
        }
    }
    Ok(())
}

/// Remove what the 1.x Flatpak route left in the user's home, and say what became of each file.
///
/// A manifest or an activation registration is deleted only when its content still names
/// Ghostlight's host or executable. The override file is restored to what it held before the
/// route added its permission only when it still holds what the route wrote; a person's later
/// edit is never undone. Nothing is ever granted. The record of the route goes last, and only
/// when every step before it succeeded, so a step that failed is tried again by the next
/// install. A leftover never fails the install: the result is the steps.
pub fn retire_flatpak_route(plan: &MigrationPlan) -> Vec<FlatpakStep> {
    let Some(route) = &plan.flatpak else {
        return Vec::new();
    };
    let mut steps = Vec::new();
    let mut step = |part, path: &Path, result| {
        steps.push(FlatpakStep {
            part,
            path: path.to_owned(),
            result,
        });
    };
    let custody = match find(&route.custody) {
        Ok(Found::Regular(bytes)) => FlatpakCustody::parse(&bytes),
        _ => None,
    };
    for (part, path, is_owned) in [
        (
            FlatpakPart::Manifest,
            &route.manifest,
            native_host::names_ghostlight_host as fn(&[u8]) -> bool,
        ),
        (
            FlatpakPart::Registration,
            &route.registration,
            names_ghostlight_activation,
        ),
    ] {
        match find(path) {
            Ok(Found::Absent) => {}
            Ok(Found::Regular(bytes)) if is_owned(&bytes) => step(part, path, remove_file(path)),
            Ok(_) => step(part, path, FlatpakResult::LeftForeign),
            Err(error) => step(part, path, FlatpakResult::Failed(error.to_string())),
        }
    }
    if let Some(custody) = custody.filter(|record| record.permission_added) {
        let part = FlatpakPart::PermissionOverride;
        let path = &route.permission_override;
        match find(path) {
            Ok(Found::Absent) => {}
            Ok(Found::Regular(bytes))
                if custody.permission_after.as_deref().map(str::as_bytes) == Some(&bytes) =>
            {
                step(
                    part,
                    path,
                    restore_override(path, custody.permission_before.as_deref()),
                );
            }
            Ok(Found::Regular(bytes))
                if custody.permission_before.as_deref().map(str::as_bytes) == Some(&bytes) => {}
            Ok(_) => step(part, path, FlatpakResult::LeftChanged),
            Err(error) => step(part, path, FlatpakResult::Failed(error.to_string())),
        }
    }
    let settled = steps
        .iter()
        .all(|done| !matches!(done.result, FlatpakResult::Failed(_)));
    if settled && matches!(find(&route.custody), Ok(Found::Regular(_))) {
        let result = remove_file(&route.custody);
        steps.push(FlatpakStep {
            part: FlatpakPart::Custody,
            path: route.custody.clone(),
            result,
        });
    }
    steps
}

/// What a Flatpak route path holds.
enum Found {
    /// Nothing is there.
    Absent,
    /// A regular file of a size the route could have written.
    Regular(Vec<u8>),
    /// A directory, a link or an oversized file, which the route never wrote.
    Other,
}

/// Look at `path` without following a link.
fn find(path: &Path) -> io::Result<Found> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Found::Absent),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() > FLATPAK_FILE_LIMIT {
        return Ok(Found::Other);
    }
    fs::read(path).map(Found::Regular)
}

/// Whether `bytes` is the D-Bus activation registration that started Ghostlight's executable.
fn names_ghostlight_activation(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let head = format!("[D-BUS Service]\nName={}\nExec=\"", native_host::HOST_NAME);
    text.strip_prefix(&head)
        .and_then(|rest| rest.strip_suffix("\"\n"))
        .is_some_and(|command| {
            Path::new(command)
                .file_name()
                .is_some_and(|name| name == ACTIVATION_EXECUTABLE_NAME)
        })
}

/// Delete `path`; a file that is already gone counts as removed.
fn remove_file(path: &Path) -> FlatpakResult {
    match fs::remove_file(path) {
        Ok(()) => FlatpakResult::Removed,
        Err(error) if error.kind() == io::ErrorKind::NotFound => FlatpakResult::Removed,
        Err(error) => FlatpakResult::Failed(error.to_string()),
    }
}

/// Put the override file back to `before`: its text, or no file at all.
fn restore_override(path: &Path, before: Option<&str>) -> FlatpakResult {
    let Some(text) = before else {
        return remove_file(path);
    };
    let mut staged = path.as_os_str().to_owned();
    staged.push(".restore");
    let staged = PathBuf::from(staged);
    match fs::write(&staged, text).and_then(|()| fs::rename(&staged, path)) {
        Ok(()) => FlatpakResult::Restored,
        Err(error) => {
            let _ = fs::remove_file(&staged);
            FlatpakResult::Failed(error.to_string())
        }
    }
}

/// Make the destination directory and refuse a destination that holds something else.
fn prepare_transfer(transfer: &Move) -> Result<(), MigrationError> {
    if !transfer.from.is_file() || same_location(&transfer.from, &transfer.to) {
        return Ok(());
    }
    if let Some(parent) = transfer.to.parent() {
        private::create_dir_all(parent).map_err(|source| MigrationError::Prepare {
            path: parent.to_owned(),
            source,
        })?;
    }
    if transfer.to.exists()
        && !same_content(&transfer.from, &transfer.to)
        && recorded_copy(&transfer.to).is_none()
    {
        return Err(MigrationError::Conflict {
            source_path: transfer.from.clone(),
            destination: transfer.to.clone(),
            disposition: Disposition::Untouched,
        });
    }
    Ok(())
}

/// Copy one file to its destination and report what was copied.
///
/// A file that is gone, or that already sits at its destination, has nothing to copy. Before a
/// copy is made, a file beside the destination records what is being copied, and it stays until
/// the original is retired. A destination that holds other content than the original is one an
/// earlier run copied, and a 1.x or 2.0 program changed since, when that record is there: the
/// record is the content to compare against when the original is retired. Without the record the
/// destination was never ours, and the original stays.
fn copy_transfer(transfer: &Move) -> Result<Option<Copied>, MigrationError> {
    let Move { from, to } = transfer;
    if !from.is_file() || same_location(from, to) {
        return Ok(None);
    }
    let failed = |source| MigrationError::Move {
        from: from.clone(),
        to: to.clone(),
        source,
    };
    let sha256 = if !to.exists() {
        let intended = record::sha256_file(from).map_err(failed)?;
        record_copy(to, &intended).map_err(failed)?;
        copy_verified(from, to).map_err(failed)?;
        let copied = record::sha256_file(to).map_err(failed)?;
        if copied != intended {
            record_copy(to, &copied).map_err(failed)?;
        }
        copied
    } else if same_content(from, to) {
        let copied = record::sha256_file(to).map_err(failed)?;
        record_copy(to, &copied).map_err(failed)?;
        copied
    } else {
        recorded_copy(to).ok_or_else(|| MigrationError::Conflict {
            source_path: from.clone(),
            destination: to.clone(),
            disposition: Disposition::Untouched,
        })?
    };
    Ok(Some(Copied {
        transfer: transfer.clone(),
        sha256,
    }))
}

/// The file beside a copy that records the content that was copied.
fn record_path(copy: &Path) -> PathBuf {
    let mut name = copy.as_os_str().to_owned();
    name.push(format!(".{COPY_BASE_SUFFIX}"));
    PathBuf::from(name)
}

/// Record that `sha256` is the content being copied to `copy`.
fn record_copy(copy: &Path, sha256: &str) -> io::Result<()> {
    record::atomic_replace(&record_path(copy), sha256.as_bytes())
}

/// The content an earlier run recorded as copied to `copy`, if it did.
fn recorded_copy(copy: &Path) -> Option<String> {
    let text = fs::read_to_string(record_path(copy)).ok()?;
    let sha256 = text.trim();
    (!sha256.is_empty()).then(|| sha256.to_owned())
}

/// Retire one original after its copy was made, and drop the record of what was copied once
/// nothing is left to retire: a file that cannot be removed keeps its record, so the next run
/// still knows what the copy was.
fn retire_transfer(done: &Copied) -> Result<(), MigrationError> {
    let outcome = settle_transfer(done);
    if matches!(outcome, Ok(()) | Err(MigrationError::Conflict { .. })) {
        let _ = fs::remove_file(record_path(&done.transfer.to));
    }
    outcome
}

/// Delete, publish or set aside one original, by comparing it and its copy with what was copied.
fn settle_transfer(done: &Copied) -> Result<(), MigrationError> {
    let Move { from, to } = &done.transfer;
    if !from.is_file() {
        return Ok(());
    }
    let failed = |source| MigrationError::Move {
        from: from.clone(),
        to: to.clone(),
        source,
    };
    let original = record::sha256_file(from).map_err(failed)?;
    if original == done.sha256 {
        // Nothing wrote to the original since the copy. Whatever 2.0 has done to the copy since
        // is newer than this file.
        return remove_original(from);
    }
    match record::sha256_file(to) {
        Ok(current) if current == original => remove_original(from),
        Ok(current) if current != done.sha256 => {
            let kept = set_aside(from).map_err(failed)?;
            Err(MigrationError::Conflict {
                source_path: from.clone(),
                destination: to.clone(),
                disposition: Disposition::SetAside(kept),
            })
        }
        Ok(_) => publish_original(from, to),
        Err(error) if error.kind() == io::ErrorKind::NotFound => publish_original(from, to),
        Err(error) => Err(failed(error)),
    }
}

/// Publish an original that a 1.x program rewrote after its copy over the copy, then delete it.
fn publish_original(from: &Path, to: &Path) -> Result<(), MigrationError> {
    copy_verified(from, to).map_err(|source| MigrationError::Move {
        from: from.to_owned(),
        to: to.to_owned(),
        source,
    })?;
    remove_original(from)
}

/// Move an old audit log to its archive file, or finish a move an earlier run began.
pub(super) fn archive_transfer(transfer: &Move) -> Result<(), MigrationError> {
    let Move { from, to } = transfer;
    if !from.is_file() || same_location(from, to) {
        return Ok(());
    }
    if to.exists() {
        // A previous run copied the log and stopped before deleting the original. Any other
        // content was written there since `prepare` looked, and the original stays.
        if !same_content(from, to) {
            return Err(MigrationError::Conflict {
                source_path: from.clone(),
                destination: to.clone(),
                disposition: Disposition::Untouched,
            });
        }
        return remove_original(from);
    }
    if fs::rename(from, to).is_ok() {
        return private::restrict_file(to).map_err(|source| MigrationError::Move {
            from: from.clone(),
            to: to.clone(),
            source,
        });
    }
    copy_verified(from, to).map_err(|source| MigrationError::Move {
        from: from.clone(),
        to: to.clone(),
        source,
    })?;
    remove_original(from)
}

/// Rename `path` beside itself to a name no plan reads, and return the new name.
fn set_aside(path: &Path) -> io::Result<PathBuf> {
    for number in 1_u32.. {
        let mut name = path.as_os_str().to_owned();
        name.push(match number {
            1 => format!(".{SET_ASIDE_SUFFIX}"),
            _ => format!(".{SET_ASIDE_SUFFIX}-{number}"),
        });
        let kept = PathBuf::from(name);
        if !kept.exists() {
            fs::rename(path, &kept)?;
            return Ok(kept);
        }
    }
    unreachable!("an unbounded range of names ends only by returning")
}

/// Delete an original whose content is already at its destination.
fn remove_original(path: &Path) -> Result<(), MigrationError> {
    fs::remove_file(path).map_err(|source| MigrationError::Remove {
        path: path.to_owned(),
        source,
    })
}

/// Copy `from` to `to` through a temporary file and publish it only if its hash matches.
///
/// The copy is a new file in the home, so it is private to the user whatever mode the original had.
fn copy_verified(from: &Path, to: &Path) -> io::Result<()> {
    let mut temporary = to.as_os_str().to_owned();
    temporary.push(format!(".{}.migrating", uuid::Uuid::new_v4().simple()));
    let temporary = PathBuf::from(temporary);
    let published = (|| {
        let mut target = private::file_options()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        io::copy(&mut File::open(from)?, &mut target)?;
        target.sync_all()?;
        drop(target);
        if record::sha256_file(from)? != record::sha256_file(&temporary)? {
            return Err(io::Error::other("the copy does not match the original"));
        }
        fs::rename(&temporary, to)
    })();
    if published.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    published
}

/// Whether two files hold the same bytes. A file that cannot be read matches nothing.
fn same_content(left: &Path, right: &Path) -> bool {
    matches!(
        (record::sha256_file(left), record::sha256_file(right)),
        (Ok(left), Ok(right)) if left == right
    )
}

/// Find, among `directories`, the first that holds a file called `name`.
fn holding<'a>(directories: &'a [PathBuf], name: &str) -> Option<&'a Path> {
    directories
        .iter()
        .map(PathBuf::as_path)
        .find(|directory| directory.join(name).is_file())
}

/// Record a source directory that lies inside a packaged app's private storage.
fn note_virtualized(list: &mut Vec<PathBuf>, directory: &Path) {
    if is_virtualized(directory) {
        push_unique(list, directory.to_owned());
    }
}

/// Add `path` to `list` unless it is already there.
fn push_unique(list: &mut Vec<PathBuf>, path: PathBuf) {
    if !list.iter().any(|existing| same_path(existing, &path)) {
        list.push(path);
    }
}

/// Whether `path` lies inside a packaged app's private copy of the user profile.
///
/// A packaged client writes under `%LOCALAPPDATA%\Packages\<family>\LocalCache` while believing
/// it wrote under the real `%LOCALAPPDATA%`. The path is real and readable from outside the
/// package, but nothing Ghostlight records may point into it (the installer's view of the file
/// system is not the browser's).
#[must_use]
pub fn is_virtualized(path: &Path) -> bool {
    let names: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect();
    names
        .windows(3)
        .any(|window| window[0] == "packages" && window[2] == "localcache")
}

/// The text of `path` in a form that compares equal across separator and case spellings.
fn comparable(path: &Path) -> String {
    comparable_text(&path.to_string_lossy())
}

/// Normalize separators, the verbatim prefix and, on Windows, case.
fn comparable_text(text: &str) -> String {
    let text = if cfg!(windows) {
        text.replace('\\', "/").to_lowercase()
    } else {
        text.to_owned()
    };
    let text = text.strip_prefix("//?/").unwrap_or(&text);
    text.trim_end_matches('/').to_owned()
}

/// Whether two paths name the same location, by spelling.
fn same_path(left: &Path, right: &Path) -> bool {
    comparable(left) == comparable(right)
}

/// Whether two paths name the same location: the same spelling, or the same place once links, case
/// and the Windows verbatim prefix are resolved.
fn same_location(left: &Path, right: &Path) -> bool {
    same_path(left, right)
        || entry::same_path(&entry::canonical_path(left), &entry::canonical_path(right))
}

/// Whether `path` is `directory` or lies below it, by spelling.
fn is_inside(path: &Path, directory: &Path) -> bool {
    let path = comparable(path);
    let directory = comparable(directory);
    path == directory
        || path
            .strip_prefix(&directory)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether `text` (a configuration file) mentions any of `directories`.
///
/// Paths in JSON and TOML configuration carry doubled backslashes on Windows, so both spellings
/// are looked for.
fn mentions_directory(text: &str, directories: &[PathBuf]) -> bool {
    let text = if cfg!(windows) {
        text.replace("\\\\", "\\").to_lowercase()
    } else {
        text.to_owned()
    };
    directories.iter().any(|directory| {
        let directory = directory.to_string_lossy();
        let directory = directory.strip_prefix(r"\\?\").unwrap_or(&directory);
        let directory = if cfg!(windows) {
            directory.to_lowercase()
        } else {
            directory.to_owned()
        };
        text.contains(&directory)
    })
}

/// The native-host manifests and client entries of this user that name `old_directories`.
fn find_registrations(old_directories: &[PathBuf]) -> Vec<Registration> {
    if old_directories.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    if let Ok(connectors) = NativeHostRegistry::discover().registered_connectors() {
        for (id, connector) in connectors {
            if old_directories
                .iter()
                .any(|directory| is_inside(&connector, directory))
            {
                found.push(Registration {
                    kind: RegistrationKind::NativeHost,
                    id,
                    names: connector,
                });
            }
        }
    }
    for summary in HarnessRegistry::discover().summaries() {
        if summary.state != HarnessState::Updatable {
            continue;
        }
        let names = PathBuf::from(&summary.config_path);
        let owned =
            fs::read_to_string(&names).is_ok_and(|text| mentions_directory(&text, old_directories));
        if owned {
            found.push(Registration {
                kind: RegistrationKind::McpClient,
                id: summary.id,
                names,
            });
        }
    }
    found
}

/// Stop every process running one of `images`, and wait for them to exit.
///
/// This is the bounded fallback for services that predate `quit`, and the way the break
/// migration quiesces the old installation: an old connector must not start an old service while
/// the files move. It refuses to stop the running installer.
pub fn stop_legacy_images(images: &[PathBuf]) -> anyhow::Result<()> {
    if images.is_empty() {
        return Ok(());
    }
    let mut system = process_table();
    let mut stopped = Vec::new();
    for pid in running_from(&system, images) {
        anyhow::ensure!(
            pid.as_u32() != std::process::id(),
            "Cannot replace the running installer"
        );
        let killed = system.process(pid).is_some_and(|process| process.kill());
        let gone = !killed && !still_running(&mut system, pid);
        match stop_outcome(killed, gone) {
            StopOutcome::Stopping => stopped.push(pid),
            StopOutcome::AlreadyGone => {}
            StopOutcome::Failed => anyhow::bail!("Could not stop Ghostlight process {pid}"),
        }
    }
    let deadline = Instant::now() + STOP_WAIT;
    while !stopped.is_empty() {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&stopped),
            true,
            ProcessRefreshKind::nothing().without_tasks(),
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

/// What a stop attempt on one process came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopOutcome {
    /// The kill was delivered; wait for the process to exit.
    Stopping,
    /// The kill failed because the process had already exited, which is what stopping wants.
    AlreadyGone,
    /// The kill failed and the process is still running.
    Failed,
}

/// Classify one stop attempt. A process can exit on its own between the process snapshot and
/// the kill (a connector the browser is cycling does), and that must not fail the install.
fn stop_outcome(killed: bool, gone: bool) -> StopOutcome {
    if killed {
        StopOutcome::Stopping
    } else if gone {
        StopOutcome::AlreadyGone
    } else {
        StopOutcome::Failed
    }
}

/// Whether `pid` still runs: present in a fresh look and not a zombie.
fn still_running(system: &mut System, pid: Pid) -> bool {
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    system
        .process(pid)
        .is_some_and(|process| process.status() != sysinfo::ProcessStatus::Zombie)
}

/// Whether any process other than the running installer is running one of `images`.
#[must_use]
pub fn any_running_from(images: &[PathBuf]) -> bool {
    !images.is_empty()
        && running_from(&process_table(), images)
            .iter()
            .any(|pid| pid.as_u32() != std::process::id())
}

/// The processes of this machine, with their executable paths.
fn process_table() -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_exe(UpdateKind::Always),
    );
    system
}

/// The processes of `system` whose executable is one of `images`.
fn running_from(system: &System, images: &[PathBuf]) -> Vec<Pid> {
    let images: Vec<PathBuf> = images
        .iter()
        .filter_map(|image| fs::canonicalize(image).ok())
        .collect();
    system
        .processes()
        .values()
        .filter(|process| {
            process
                .exe()
                .and_then(|path| fs::canonicalize(path).ok())
                .is_some_and(|executable| images.contains(&executable))
        })
        .map(sysinfo::Process::pid)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::scratch::Scratch;

    /// Bytes of the audit log the fake 1.x installation holds.
    const AUDIT: &str = "{\"event\":\"old\"}\n";

    /// The image names a 1.x installation held: the service under the product's name, which is
    /// now the edge's, and both connectors.
    fn legacy_image_names() -> Vec<String> {
        std::iter::once(EDGE_EXECUTABLE_NAME.to_owned())
            .chain(retired_connector_names())
            .collect()
    }

    /// Carry out every step of `plan` with nothing to stop.
    fn execute(plan: &MigrationPlan) -> Result<(), MigrationError> {
        prepare(plan)?;
        let copied = copy_state(plan)?;
        retire_state(&copied)?;
        archive_audit(plan)?;
        finish(plan)
    }

    /// A fake 1.x user: a home, a repo build, a policy directory and the profile roots.
    struct Fixture {
        scratch: Scratch,
        home: PathBuf,
        roots: LegacyRoots,
    }

    impl Fixture {
        fn new(windows: bool) -> Self {
            let scratch = Scratch::new();
            let user_home = scratch.path().join("user");
            let home = user_home.join(".ghostlight");
            fs::create_dir_all(&home).unwrap();
            let roots = LegacyRoots {
                windows,
                user_home: user_home.clone(),
                local_app_data: Some(user_home.join("AppData").join("Local")),
                app_data: Some(user_home.join("AppData").join("Roaming")),
                xdg_state_home: None,
                xdg_config_home: None,
            };
            Self {
                scratch,
                home,
                roots,
            }
        }

        fn path(&self, relative: &str) -> PathBuf {
            relative
                .split('/')
                .fold(self.scratch.path().to_owned(), |path, part| path.join(part))
        }

        fn write(&self, path: &Path, text: &str) {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        /// The three executables of a 1.x installation in `directory`.
        fn executables(&self, directory: &Path) -> Vec<PathBuf> {
            legacy_image_names()
                .into_iter()
                .map(|name| {
                    let path = directory.join(&name);
                    self.write(&path, &name);
                    path
                })
                .collect()
        }

        fn record(&self, record: &serde_json::Value) {
            self.write(&self.home.join(LEGACY_RECORD_NAME), &record.to_string());
        }

        fn legacy(&self) -> Legacy {
            Legacy {
                installation: LegacyInstallation::read(&self.home.join(LEGACY_RECORD_NAME))
                    .unwrap(),
                roots: self.roots.clone(),
                registrations: Vec::new(),
            }
        }

        fn plan(&self) -> MigrationPlan {
            plan(&self.home, &self.legacy()).unwrap()
        }
    }

    /// A user whose development build served from a repo's `target/release`.
    fn development_custody() -> Fixture {
        let fixture = Fixture::new(true);
        let repo_release = fixture.path("repo/target/release");
        let bin = fixture.home.join("bin");
        fixture.executables(&repo_release);
        fixture.executables(&bin);
        let policy = fixture.path("user/AppData/Local/Ghostlight");
        fixture.record(&json!({
            "schema": 1,
            "serving_directory": repo_release,
            "release_directory": bin,
            "development_directory": repo_release,
            "state_directory": fixture.home,
            "policy_directory": policy,
            "added_later": true,
        }));
        fixture.write(&fixture.home.join(AUDIT_FILE_NAME), AUDIT);
        fixture.write(&policy.join(USER_POLICY_FILE_NAME), "user policy");
        fixture.write(&policy.join(MANAGED_CACHE_FILE_NAME), "cache");
        fixture.write(&policy.join(MANAGED_STATUS_FILE_NAME), "status");
        fixture.write(&policy.join(HANDOFF_MARKER_NAME), "");
        fixture.write(
            &fixture.path("user/AppData/Roaming/ghostlight/harness-locations.json"),
            "{}",
        );
        fixture.write(&fixture.home.join(LEGACY_STARTUP_NAME), "");
        fixture.write(&fixture.home.join(LEGACY_DEPLOY_LOCK_NAME), "");
        fixture.write(&repo_release.join(LEGACY_DEPLOY_LOCK_NAME), "");
        fixture.write(&fixture.home.join(LEGACY_INSTALLATION_LOCK_NAME), "");
        fixture
    }

    #[test]
    fn a_development_custody_record_plans_every_move_deletion_and_stop() {
        let fixture = development_custody();
        let plan = fixture.plan();
        let policy = fixture.path("user/AppData/Local/Ghostlight");
        let state = fixture.home.join("state");
        let moves = |from: &Path, name: &str, to: &Path| Move {
            from: from.join(name),
            to: to.join(name),
        };
        assert_eq!(
            plan.moves,
            vec![
                moves(&policy, USER_POLICY_FILE_NAME, &fixture.home.join("policy")),
                moves(
                    &policy,
                    MANAGED_CACHE_FILE_NAME,
                    &fixture.home.join("policy")
                ),
                moves(
                    &policy,
                    MANAGED_STATUS_FILE_NAME,
                    &fixture.home.join("policy")
                ),
                moves(
                    &fixture.path("user/AppData/Roaming/ghostlight"),
                    HARNESS_LOCATIONS_NAME,
                    &state
                ),
            ]
        );
        assert_eq!(
            plan.audit_archive,
            Some(Move {
                from: fixture.home.join(AUDIT_FILE_NAME),
                to: state.join("audit-pre-2.0.jsonl"),
            })
        );
        assert_eq!(plan.setup_record, Some(state.join("setup.json")));
        assert_eq!(
            plan.deletions,
            vec![
                fixture.home.join(LEGACY_STARTUP_NAME),
                fixture.home.join(LEGACY_DEPLOY_LOCK_NAME),
                fixture
                    .path("repo/target/release")
                    .join(LEGACY_DEPLOY_LOCK_NAME),
                fixture.home.join(LEGACY_INSTALLATION_LOCK_NAME),
                fixture
                    .path("user/AppData/Local/Ghostlight")
                    .join(HANDOFF_MARKER_NAME),
                fixture.home.join("bin").join(&retired_connector_names()[0]),
                fixture.home.join("bin").join(&retired_connector_names()[1]),
                fixture.home.join(LEGACY_RECORD_NAME),
            ]
        );
        let repo_release = fixture.path("repo/target/release");
        let bin = fixture.home.join("bin");
        let expected: Vec<PathBuf> = [repo_release, bin]
            .iter()
            .flat_map(|directory| {
                legacy_image_names()
                    .into_iter()
                    .map(|name| directory.join(name))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(plan.images_to_stop, expected);
        assert!(plan.virtualized_sources.is_empty());
    }

    #[test]
    fn no_record_reads_the_default_roots_on_windows() {
        let fixture = Fixture::new(true);
        let policy = fixture.path("user/AppData/Local/Ghostlight");
        fixture.write(&policy.join(USER_POLICY_FILE_NAME), "policy");
        fixture.write(&fixture.home.join(AUDIT_FILE_NAME), AUDIT);
        let plan = fixture.plan();
        assert_eq!(
            plan.moves,
            vec![Move {
                from: policy.join(USER_POLICY_FILE_NAME),
                to: fixture.home.join("policy").join(USER_POLICY_FILE_NAME),
            }]
        );
        assert_eq!(
            plan.audit_archive.map(|archive| archive.from),
            Some(fixture.home.join(AUDIT_FILE_NAME))
        );
        assert!(plan.deletions.is_empty());
        assert!(plan.images_to_stop.is_empty());
    }

    #[test]
    fn no_record_reads_the_default_roots_on_linux() {
        let mut fixture = Fixture::new(false);
        fixture.roots.xdg_config_home = Some(fixture.path("user/xdg-config"));
        let policy = fixture.path("user/.local/state/ghostlight");
        fixture.write(&policy.join(USER_POLICY_FILE_NAME), "policy");
        fixture.write(
            &fixture.path("user/xdg-config/ghostlight/harness-locations.json"),
            "{}",
        );
        let cache = fixture.path("user/.cache/ghostlight");
        fixture.write(&cache.join(AUDIT_FILE_NAME), AUDIT);
        let plan = fixture.plan();
        assert_eq!(plan.moves.len(), 2);
        assert_eq!(
            plan.audit_archive.map(|archive| archive.from),
            Some(cache.join(AUDIT_FILE_NAME))
        );
    }

    #[test]
    fn a_state_directory_beside_the_old_serving_binary_is_adopted() {
        let fixture = Fixture::new(true);
        let serving = fixture.path("old/v0.9.2");
        fixture.executables(&serving);
        fixture.write(&serving.join(AUDIT_FILE_NAME), AUDIT);
        fixture.record(&json!({
            "schema": 1,
            "serving_directory": serving,
            "release_directory": serving,
            "state_directory": serving,
            "policy_directory": fixture.path("user/AppData/Local/Ghostlight"),
        }));
        let plan = fixture.plan();
        assert_eq!(
            plan.audit_archive,
            Some(Move {
                from: serving.join(AUDIT_FILE_NAME),
                to: fixture.home.join("state").join("audit-pre-0.9.2.jsonl"),
            })
        );
    }

    #[test]
    fn a_policy_directory_inside_a_packaged_apps_storage_is_read_where_it_is() {
        let fixture = Fixture::new(true);
        let virtualized =
            fixture.path("user/AppData/Local/Packages/OpenAI.Codex_x/LocalCache/Local/Ghostlight");
        fixture.write(&virtualized.join(USER_POLICY_FILE_NAME), "policy");
        fixture.record(&json!({
            "schema": 1,
            "serving_directory": fixture.home.join("bin"),
            "state_directory": fixture.home,
            "policy_directory": virtualized,
        }));
        let plan = fixture.plan();
        assert_eq!(
            plan.moves,
            vec![Move {
                from: virtualized.join(USER_POLICY_FILE_NAME),
                to: fixture.home.join("policy").join(USER_POLICY_FILE_NAME),
            }]
        );
        assert_eq!(plan.virtualized_sources, vec![virtualized]);
        execute(&plan).unwrap();
        let policy = fixture.home.join("policy").join(USER_POLICY_FILE_NAME);
        assert_eq!(fs::read_to_string(&policy).unwrap(), "policy");
        assert!(!is_virtualized(&policy));
    }

    /// Put the policy files in `home/policy`, record `spell(home/policy)` as the 1.x policy
    /// directory, and check that nothing moves and nothing is lost.
    fn assert_policy_already_home(spell: impl Fn(&Path) -> PathBuf) {
        let fixture = Fixture::new(true);
        let policy = fixture.home.join("policy");
        let names = [
            USER_POLICY_FILE_NAME,
            MANAGED_CACHE_FILE_NAME,
            MANAGED_STATUS_FILE_NAME,
        ];
        for name in names {
            fixture.write(&policy.join(name), name);
        }
        fixture.record(&json!({
            "schema": 1,
            "serving_directory": fixture.home.join("bin"),
            "state_directory": fixture.home,
            "policy_directory": spell(&policy),
        }));
        let plan = fixture.plan();
        assert!(plan.moves.is_empty(), "{:?}", plan.moves);
        execute(&plan).unwrap();
        for name in names {
            assert_eq!(fs::read_to_string(policy.join(name)).unwrap(), name);
        }
    }

    #[test]
    fn a_policy_directory_that_is_already_the_new_home_keeps_its_files() {
        assert_policy_already_home(Path::to_path_buf);
    }

    #[cfg(windows)]
    #[test]
    fn a_verbatim_or_recased_spelling_of_the_new_policy_directory_keeps_its_files() {
        assert_policy_already_home(|policy| PathBuf::from(format!(r"\?\{}", policy.display())));
        assert_policy_already_home(|policy| PathBuf::from(policy.to_string_lossy().to_uppercase()));
    }

    #[test]
    fn a_move_from_a_file_to_itself_leaves_the_file() {
        let fixture = Fixture::new(true);
        let file = fixture.home.join("policy").join(USER_POLICY_FILE_NAME);
        fixture.write(&file, "policy");
        let plan = MigrationPlan {
            moves: vec![Move {
                from: file.clone(),
                to: file.clone(),
            }],
            ..MigrationPlan::default()
        };
        execute(&plan).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "policy");
    }

    #[test]
    fn a_packaged_environment_root_falls_back_to_the_real_profile() {
        let mut fixture = Fixture::new(true);
        fixture.roots.local_app_data =
            Some(fixture.path("user/AppData/Local/Packages/OpenAI.Codex_x/LocalCache/Local"));
        assert_eq!(
            fixture.roots.policy_directory(),
            fixture.path("user/AppData/Local/Ghostlight")
        );
    }

    #[test]
    fn a_home_inside_packaged_storage_is_refused() {
        let fixture = Fixture::new(true);
        let home = fixture.path("user/AppData/Local/Packages/X_y/LocalCache/Local/.ghostlight");
        assert!(matches!(
            plan(&home, &fixture.legacy()),
            Err(MigrationError::VirtualizedHome(_))
        ));
    }

    #[test]
    fn executing_moves_archives_and_deletes_then_nothing_is_left_to_do() {
        let fixture = development_custody();
        let plan = fixture.plan();
        execute(&plan).unwrap();
        let policy = fixture.home.join("policy");
        let state = fixture.home.join("state");
        assert_eq!(
            fs::read_to_string(policy.join(USER_POLICY_FILE_NAME)).unwrap(),
            "user policy"
        );
        assert_eq!(
            fs::read_to_string(policy.join(MANAGED_CACHE_FILE_NAME)).unwrap(),
            "cache"
        );
        assert!(!state.join(HANDOFF_MARKER_NAME).exists());
        let record = state.join("setup.json");
        let recorded = setup_state::read(&record).expect("the migration wrote the setup record");
        assert!(recorded.first_request_ms > 0);
        assert!(state.join(HARNESS_LOCATIONS_NAME).is_file());
        assert_eq!(
            fs::read_to_string(state.join("audit-pre-2.0.jsonl")).unwrap(),
            AUDIT
        );
        for gone in [
            fixture.home.join(AUDIT_FILE_NAME),
            fixture.home.join(LEGACY_RECORD_NAME),
            fixture.home.join(LEGACY_STARTUP_NAME),
            fixture.home.join(LEGACY_DEPLOY_LOCK_NAME),
            fixture.home.join(LEGACY_INSTALLATION_LOCK_NAME),
            fixture
                .path("repo/target/release")
                .join(LEGACY_DEPLOY_LOCK_NAME),
            fixture
                .path("user/AppData/Local/Ghostlight")
                .join(USER_POLICY_FILE_NAME),
            fixture
                .path("user/AppData/Local/Ghostlight")
                .join(HANDOFF_MARKER_NAME),
        ] {
            assert!(!gone.exists(), "{}", gone.display());
        }

        let again = fixture.plan();
        assert!(again.is_empty(), "{again:?}");
        execute(&again).unwrap();
        assert_eq!(
            fs::read_to_string(policy.join(USER_POLICY_FILE_NAME)).unwrap(),
            "user policy"
        );
    }

    #[test]
    fn an_existing_setup_record_is_left_untouched() {
        let fixture = development_custody();
        let record = fixture.home.join("state").join("setup.json");
        fixture.write(&record, r#"{"first_request_ms":5}"#);
        let plan = fixture.plan();
        assert_eq!(plan.setup_record, None);
        execute(&plan).unwrap();
        assert_eq!(
            setup_state::read(&record),
            Some(SetupRecord {
                first_request_ms: 5
            })
        );
    }

    #[test]
    fn a_home_with_no_earlier_installation_gets_no_setup_record() {
        let fixture = Fixture::new(true);
        let plan = fixture.plan();
        assert!(plan.is_empty(), "{plan:?}");
        assert_eq!(plan.setup_record, None);
    }

    #[test]
    fn a_failing_move_names_the_path_and_leaves_everything_in_place() {
        let fixture = development_custody();
        let plan = fixture.plan();
        // The last file in the plan cannot be placed: its destination holds other content.
        let blocked = fixture.home.join("state").join(HARNESS_LOCATIONS_NAME);
        fixture.write(&blocked, "something else");
        let error = execute(&plan).unwrap_err();
        assert!(matches!(error, MigrationError::Conflict { .. }), "{error}");
        assert!(error.to_string().contains(&blocked.display().to_string()));
        assert!(fixture.home.join(LEGACY_RECORD_NAME).is_file());
        assert!(fixture.home.join(AUDIT_FILE_NAME).is_file());
        assert!(fixture
            .path("user/AppData/Local/Ghostlight")
            .join(USER_POLICY_FILE_NAME)
            .is_file());
        assert!(!fixture
            .home
            .join("policy")
            .join(USER_POLICY_FILE_NAME)
            .exists());
        assert_eq!(fs::read_to_string(&blocked).unwrap(), "something else");
    }

    #[test]
    fn a_destination_that_cannot_be_made_is_named() {
        let fixture = development_custody();
        let plan = fixture.plan();
        fixture.write(&fixture.home.join("policy"), "a file, not a directory");
        let error = execute(&plan).unwrap_err();
        assert!(matches!(error, MigrationError::Prepare { .. }), "{error}");
        assert!(error
            .to_string()
            .contains(&fixture.home.join("policy").display().to_string()));
        assert!(fixture.home.join(LEGACY_RECORD_NAME).is_file());
        assert!(fixture.home.join(AUDIT_FILE_NAME).is_file());
    }

    #[test]
    fn a_rerun_after_a_partial_success_continues() {
        let fixture = development_custody();
        let plan = fixture.plan();
        // The first file was moved, and the second was copied but its original not yet removed.
        let policy = fixture.path("user/AppData/Local/Ghostlight");
        let destination = fixture.home.join("policy");
        fs::create_dir_all(&destination).unwrap();
        fs::rename(
            policy.join(USER_POLICY_FILE_NAME),
            destination.join(USER_POLICY_FILE_NAME),
        )
        .unwrap();
        fs::copy(
            policy.join(MANAGED_CACHE_FILE_NAME),
            destination.join(MANAGED_CACHE_FILE_NAME),
        )
        .unwrap();
        execute(&plan).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join(USER_POLICY_FILE_NAME)).unwrap(),
            "user policy"
        );
        assert!(!policy.join(MANAGED_CACHE_FILE_NAME).exists());
        assert_eq!(
            fs::read_to_string(destination.join(MANAGED_CACHE_FILE_NAME)).unwrap(),
            "cache"
        );
        assert!(!fixture.home.join(LEGACY_RECORD_NAME).exists());
    }

    #[test]
    fn a_destination_that_appears_after_the_check_keeps_the_original() {
        let fixture = development_custody();
        let plan = fixture.plan();
        prepare(&plan).unwrap();
        let policy = fixture.path("user/AppData/Local/Ghostlight");
        let cache = fixture.home.join("policy").join(MANAGED_CACHE_FILE_NAME);
        fixture.write(&cache, "a bundle written by a 2.0 service");
        let error = copy_state(&plan).unwrap_err();
        assert!(matches!(error, MigrationError::Conflict { .. }), "{error}");
        assert!(error.to_string().contains(&cache.display().to_string()));
        assert_eq!(
            fs::read_to_string(policy.join(MANAGED_CACHE_FILE_NAME)).unwrap(),
            "cache"
        );
        assert_eq!(
            fs::read_to_string(&cache).unwrap(),
            "a bundle written by a 2.0 service"
        );
    }

    #[test]
    fn the_audit_log_is_archived_apart_from_the_early_copies() {
        let fixture = development_custody();
        let plan = fixture.plan();
        let archive = fixture.home.join("state").join("audit-pre-2.0.jsonl");
        prepare(&plan).unwrap();
        copy_state(&plan).unwrap();
        assert!(fixture.home.join(AUDIT_FILE_NAME).is_file());
        assert!(!archive.exists());
        archive_audit(&plan).unwrap();
        assert!(!fixture.home.join(AUDIT_FILE_NAME).exists());
        assert_eq!(fs::read_to_string(archive).unwrap(), AUDIT);
    }

    /// Every path under `directory`, directories included.
    #[cfg(unix)]
    fn everything_under(directory: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            found.push(path.clone());
            if path.is_dir() {
                found.extend(everything_under(&path));
            }
        }
        found
    }

    #[cfg(unix)]
    #[test]
    fn what_the_migration_creates_in_the_home_is_private_whatever_the_originals_were() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = development_custody();
        let plan = fixture.plan();
        for transfer in plan.transfers() {
            fs::set_permissions(&transfer.from, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let archive = fixture.home.join("state").join("audit-pre-2.0.jsonl");
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        let assert_private = || {
            for directory in ["policy", "state"] {
                let directory = fixture.home.join(directory);
                assert_eq!(mode(&directory), 0o700, "{}", directory.display());
                for path in everything_under(&directory) {
                    let expected = if path.is_dir() { 0o700 } else { 0o600 };
                    assert_eq!(mode(&path), expected, "{}", path.display());
                }
            }
        };

        prepare(&plan).unwrap();
        copy_state(&plan).unwrap();
        assert!(everything_under(&fixture.home.join("state"))
            .iter()
            .any(|path| path.to_string_lossy().contains(COPY_BASE_SUFFIX)));
        assert_private();
        archive_audit(&plan).unwrap();
        assert!(archive.is_file());
        assert_private();
    }

    /// The 1.x user policy of `development_custody`.
    fn legacy_user_policy(fixture: &Fixture) -> PathBuf {
        fixture
            .path("user/AppData/Local/Ghostlight")
            .join(USER_POLICY_FILE_NAME)
    }

    #[test]
    fn copying_leaves_every_original_for_the_service_that_still_runs() {
        let fixture = development_custody();
        let plan = fixture.plan();
        prepare(&plan).unwrap();
        let copied = copy_state(&plan).unwrap();
        assert_eq!(copied.len(), plan.moves.len());
        for transfer in &plan.moves {
            assert!(transfer.from.is_file(), "{}", transfer.from.display());
            assert_eq!(
                fs::read(&transfer.from).unwrap(),
                fs::read(&transfer.to).unwrap()
            );
        }
        retire_state(&copied).unwrap();
        for transfer in &plan.moves {
            assert!(!transfer.from.exists(), "{}", transfer.from.display());
            assert!(transfer.to.is_file());
        }
    }

    #[test]
    fn an_original_rewritten_between_the_copy_and_the_retire_wins() {
        let fixture = development_custody();
        let plan = fixture.plan();
        prepare(&plan).unwrap();
        let copied = copy_state(&plan).unwrap();
        let original = legacy_user_policy(&fixture);
        fs::write(&original, "saved by the 1.x workbench").unwrap();
        retire_state(&copied).unwrap();
        assert!(!original.exists());
        assert_eq!(
            fs::read_to_string(fixture.home.join("policy").join(USER_POLICY_FILE_NAME)).unwrap(),
            "saved by the 1.x workbench"
        );
        assert!(fixture.plan().moves.is_empty());
    }

    #[test]
    fn a_copy_that_a_2_0_service_changed_is_kept_when_the_original_did_not_change() {
        let fixture = development_custody();
        let plan = fixture.plan();
        prepare(&plan).unwrap();
        let copied = copy_state(&plan).unwrap();
        let status = fixture.home.join("policy").join(MANAGED_STATUS_FILE_NAME);
        fs::write(&status, "written by a 2.0 service").unwrap();
        retire_state(&copied).unwrap();
        assert_eq!(
            fs::read_to_string(&status).unwrap(),
            "written by a 2.0 service"
        );
        assert!(!fixture
            .path("user/AppData/Local/Ghostlight")
            .join(MANAGED_STATUS_FILE_NAME)
            .exists());
    }

    #[test]
    fn an_original_and_a_copy_that_both_changed_keep_both_and_never_block_the_next_run() {
        let fixture = development_custody();
        let plan = fixture.plan();
        prepare(&plan).unwrap();
        let copied = copy_state(&plan).unwrap();
        let original = legacy_user_policy(&fixture);
        let copy = fixture.home.join("policy").join(USER_POLICY_FILE_NAME);
        fs::write(&original, "edited in the 1.x workbench").unwrap();
        fs::write(&copy, "edited by a 2.0 service").unwrap();
        let error = retire_state(&copied).unwrap_err();
        let aside = original.with_file_name(format!("{USER_POLICY_FILE_NAME}.pre-2.0"));
        assert!(
            matches!(
                &error,
                MigrationError::Conflict {
                    disposition: Disposition::SetAside(kept),
                    ..
                } if *kept == aside
            ),
            "{error}"
        );
        assert!(error.to_string().contains(&aside.display().to_string()));
        assert!(!original.exists());
        assert_eq!(
            fs::read_to_string(&aside).unwrap(),
            "edited in the 1.x workbench"
        );
        assert_eq!(
            fs::read_to_string(&copy).unwrap(),
            "edited by a 2.0 service"
        );
        // Every other original was still retired, and the next install has nothing to refuse.
        let again = fixture.plan();
        assert!(again.moves.is_empty(), "{:?}", again.moves);
        execute(&again).unwrap();
        assert_eq!(
            fs::read_to_string(&copy).unwrap(),
            "edited by a 2.0 service"
        );
    }

    #[test]
    fn the_record_beside_a_copy_goes_when_its_original_is_retired() {
        let fixture = development_custody();
        let plan = fixture.plan();
        prepare(&plan).unwrap();
        let copied = copy_state(&plan).unwrap();
        let copy = fixture.home.join("policy").join(USER_POLICY_FILE_NAME);
        assert!(record_path(&copy).is_file());
        retire_state(&copied).unwrap();
        for transfer in &plan.moves {
            assert!(!record_path(&transfer.to).exists());
        }
    }

    #[test]
    fn a_copy_that_a_failed_run_left_is_adopted_whichever_side_changed_since() {
        let fixture = development_custody();
        let plan = fixture.plan();
        prepare(&plan).unwrap();
        copy_state(&plan).unwrap();
        let original = legacy_user_policy(&fixture);
        let copy = fixture.home.join("policy").join(USER_POLICY_FILE_NAME);
        let status = fixture.home.join("policy").join(MANAGED_STATUS_FILE_NAME);
        fs::write(&original, "saved by a 1.x workbench").unwrap();
        fs::write(&status, "written by a 2.0 service").unwrap();

        // The failed run never retired anything, and the next plan lists the same files.
        let again = fixture.plan();
        assert_eq!(again.moves, plan.moves);
        execute(&again).unwrap();
        assert!(!original.exists());
        assert_eq!(
            fs::read_to_string(&copy).unwrap(),
            "saved by a 1.x workbench"
        );
        assert_eq!(
            fs::read_to_string(&status).unwrap(),
            "written by a 2.0 service"
        );
        for transfer in &plan.moves {
            assert!(!transfer.from.exists(), "{}", transfer.from.display());
            assert!(!record_path(&transfer.to).exists());
        }
    }

    #[test]
    fn a_different_destination_without_a_record_is_refused_and_nothing_is_copied() {
        let fixture = development_custody();
        let plan = fixture.plan();
        let copy = fixture.home.join("policy").join(USER_POLICY_FILE_NAME);
        fs::create_dir_all(copy.parent().unwrap()).unwrap();
        fs::write(&copy, "somebody else's file").unwrap();
        let error = prepare(&plan).unwrap_err();
        assert!(matches!(error, MigrationError::Conflict { .. }), "{error}");
        assert!(!record_path(&copy).exists());
    }

    #[test]
    fn set_aside_names_never_overwrite_an_earlier_one() {
        let scratch = Scratch::new();
        let file = scratch.path().join("user-policy.json");
        fs::write(&file, "first").unwrap();
        let first = set_aside(&file).unwrap();
        fs::write(&file, "second").unwrap();
        let second = set_aside(&file).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read_to_string(first).unwrap(), "first");
        assert_eq!(fs::read_to_string(second).unwrap(), "second");
    }

    #[test]
    fn a_copy_is_published_only_when_it_matches() {
        let scratch = Scratch::new();
        let from = scratch.path().join("from.json");
        let to = scratch.path().join("to").join("copy.json");
        fs::write(&from, "content").unwrap();
        assert!(copy_verified(&from, &to).is_err());
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        copy_verified(&from, &to).unwrap();
        assert_eq!(fs::read_to_string(&to).unwrap(), "content");
        assert!(from.is_file());
        assert_eq!(fs::read_dir(to.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn the_plan_serializes_the_same_way_every_time() {
        let fixture = development_custody();
        let first = serde_json::to_string(&fixture.plan()).unwrap();
        let second = serde_json::to_string(&fixture.plan()).unwrap();
        assert_eq!(first, second);
        let value: serde_json::Value = serde_json::from_str(&first).unwrap();
        let object = value.as_object().unwrap();
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "audit_archive",
                "deletions",
                "flatpak",
                "images_to_stop",
                "moves",
                "registrations_to_reclaim",
                "setup_record",
                "virtualized_sources"
            ]
        );
        assert_eq!(
            value["moves"][0],
            json!({
                "from": fixture
                    .path("user/AppData/Local/Ghostlight")
                    .join(USER_POLICY_FILE_NAME),
                "to": fixture.home.join("policy").join(USER_POLICY_FILE_NAME),
            })
        );
    }

    #[test]
    fn a_plan_carries_the_registrations_that_were_found() {
        let fixture = Fixture::new(true);
        let mut legacy = fixture.legacy();
        legacy.registrations = vec![Registration {
            kind: RegistrationKind::NativeHost,
            id: "chrome".into(),
            names: fixture.path("old/ghostlight-browser-connector"),
        }];
        let plan = plan(&fixture.home, &legacy).unwrap();
        assert!(!plan.is_empty());
        assert_eq!(
            serde_json::to_value(&plan.registrations_to_reclaim).unwrap(),
            json!([{
                "kind": "native_host",
                "id": "chrome",
                "names": fixture.path("old/ghostlight-browser-connector"),
            }])
        );
    }

    #[test]
    fn registrations_are_matched_by_directory() {
        let directory = PathBuf::from(if cfg!(windows) {
            r"C:\Repo\target\release"
        } else {
            "/repo/target/release"
        });
        let inside = directory.join("ghostlight-browser-connector");
        assert!(is_inside(&inside, &directory));
        assert!(is_inside(&directory, &directory));
        assert!(!is_inside(
            &directory.with_file_name("release-old").join("connector"),
            &directory
        ));
        assert!(mentions_directory(
            &format!("{{\"command\": \"{}\"}}", inside.display()).replace('\\', "\\\\"),
            std::slice::from_ref(&directory)
        ));
        assert!(!mentions_directory(
            "{\"command\": \"/elsewhere/connector\"}",
            std::slice::from_ref(&directory)
        ));
        if cfg!(windows) {
            assert!(is_inside(
                Path::new(r"\\?\c:\repo\TARGET\release\x.exe"),
                &directory
            ));
        }
    }

    #[test]
    fn the_record_reads_tolerantly_and_a_broken_one_stops_the_plan() {
        let fixture = Fixture::new(true);
        fixture.record(&json!({ "serving_directory": "relative/path", "other": 7 }));
        let record = LegacyInstallation::read(&fixture.home.join(LEGACY_RECORD_NAME))
            .unwrap()
            .unwrap();
        assert_eq!(record.schema, 0);
        assert!(record.directories().is_empty());
        assert!(fixture.plan().images_to_stop.is_empty());

        fixture.write(&fixture.home.join(LEGACY_RECORD_NAME), "not json");
        let error = LegacyInstallation::read(&fixture.home.join(LEGACY_RECORD_NAME)).unwrap_err();
        assert!(error.to_string().contains(LEGACY_RECORD_NAME), "{error}");
    }

    #[test]
    fn the_old_version_comes_from_the_serving_install_record_or_directory_name() {
        let fixture = Fixture::new(true);
        let serving = fixture.path("old/bin");
        InstallRecord {
            service_version: "1.3.4".into(),
            relay_revision: "2".into(),
            service_sha256: "00".into(),
            edge_sha256: None,
            channel: record::Channel::Release,
        }
        .write(&serving.join(INSTALL_RECORD_NAME))
        .unwrap();
        let mut legacy = fixture.legacy();
        legacy.installation = Some(LegacyInstallation {
            serving_directory: Some(serving),
            ..LegacyInstallation::default()
        });
        assert_eq!(legacy.old_version().as_deref(), Some("1.3.4"));
        legacy.installation = Some(LegacyInstallation {
            serving_directory: Some(fixture.path("old/v0.8.1")),
            ..LegacyInstallation::default()
        });
        assert_eq!(legacy.old_version().as_deref(), Some("0.8.1"));
        legacy.installation = Some(LegacyInstallation {
            serving_directory: Some(fixture.path("old/plain")),
            ..LegacyInstallation::default()
        });
        assert_eq!(legacy.old_version(), None);
    }

    /// The override file of a person who had their own settings before the 1.x route.
    const OVERRIDE_BEFORE: &str = "[Context]\nfilesystems=xdg-download;\n";

    /// The same file after the route added its permission.
    const OVERRIDE_AFTER: &str =
        "[Context]\nfilesystems=xdg-download;\n[Session Bus Policy]\norg.sylin.ghostlight=talk\n";

    /// Custody text naming `before` as what the override held.
    fn custody(before: Option<&str>, added: bool) -> String {
        json!({
            "version": 1,
            "executable": "/home/user/.ghostlight/bin/ghostlight",
            "permission_before": before,
            "permission_after": OVERRIDE_AFTER,
            "permission_added": added,
        })
        .to_string()
    }

    /// The files a 1.x Flatpak route install left below the user's home.
    fn flatpak_route(fixture: &Fixture, before: Option<&str>) -> FlatpakLeftovers {
        let route = FlatpakLeftovers::below(&fixture.roots.user_home);
        fixture.write(
            &route.manifest,
            &json!({
                "name": native_host::HOST_NAME,
                "description": "Ghostlight browser connector",
                "path": "/home/user/.ghostlight/bin/ghostlight-browser-connector",
                "type": "stdio",
                "allowed_origins": ["chrome-extension://abc/"],
            })
            .to_string(),
        );
        fixture.write(
            &route.registration,
            &format!(
                "[D-BUS Service]\nName={}\nExec=\"/home/user/.ghostlight/bin/ghostlight\"\n",
                native_host::HOST_NAME
            ),
        );
        fixture.write(&route.permission_override, OVERRIDE_AFTER);
        fixture.write(&route.custody, &custody(before, true));
        route
    }

    fn results(steps: &[FlatpakStep]) -> Vec<(FlatpakPart, FlatpakResult)> {
        steps
            .iter()
            .map(|step| (step.part, step.result.clone()))
            .collect()
    }

    #[test]
    fn the_flatpak_route_is_removed_and_the_override_restored_then_a_second_run_does_nothing() {
        let fixture = Fixture::new(false);
        let route = flatpak_route(&fixture, Some(OVERRIDE_BEFORE));
        let plan = fixture.plan();
        assert!(!plan.is_empty());
        assert!(!plan.has_earlier_installation());
        assert_eq!(plan.flatpak.as_ref(), Some(&route));

        let steps = retire_flatpak_route(&plan);
        assert_eq!(
            results(&steps),
            [
                (FlatpakPart::Manifest, FlatpakResult::Removed),
                (FlatpakPart::Registration, FlatpakResult::Removed),
                (FlatpakPart::PermissionOverride, FlatpakResult::Restored),
                (FlatpakPart::Custody, FlatpakResult::Removed),
            ]
        );
        assert!(!route.manifest.exists());
        assert!(!route.registration.exists());
        assert!(!route.custody.exists());
        assert_eq!(
            fs::read_to_string(&route.permission_override).unwrap(),
            OVERRIDE_BEFORE
        );

        assert!(fixture.plan().is_empty());
        assert!(retire_flatpak_route(&fixture.plan()).is_empty());
        assert!(retire_flatpak_route(&plan).is_empty());
    }

    #[test]
    fn an_override_the_route_created_is_deleted() {
        let fixture = Fixture::new(false);
        let route = flatpak_route(&fixture, None);
        let steps = retire_flatpak_route(&fixture.plan());
        assert!(
            results(&steps).contains(&(FlatpakPart::PermissionOverride, FlatpakResult::Removed))
        );
        assert!(!route.permission_override.exists());
    }

    #[test]
    fn an_override_a_person_changed_since_is_left_and_reported() {
        let fixture = Fixture::new(false);
        let route = flatpak_route(&fixture, Some(OVERRIDE_BEFORE));
        let changed = format!("{OVERRIDE_AFTER}[Environment]\nKEY=value\n");
        fixture.write(&route.permission_override, &changed);
        let steps = retire_flatpak_route(&fixture.plan());
        assert_eq!(
            results(&steps),
            [
                (FlatpakPart::Manifest, FlatpakResult::Removed),
                (FlatpakPart::Registration, FlatpakResult::Removed),
                (FlatpakPart::PermissionOverride, FlatpakResult::LeftChanged),
                (FlatpakPart::Custody, FlatpakResult::Removed),
            ]
        );
        assert_eq!(
            fs::read_to_string(&route.permission_override).unwrap(),
            changed
        );
        assert!(fixture.plan().is_empty());
    }

    #[test]
    fn a_manifest_or_registration_that_is_not_ghostlights_is_left_and_reported() {
        let fixture = Fixture::new(false);
        let route = flatpak_route(&fixture, Some(OVERRIDE_BEFORE));
        let plan = fixture.plan();
        fixture.write(&route.manifest, "{\"name\":\"org.example.other\"}");
        fixture.write(
            &route.registration,
            "[D-BUS Service]\nName=org.sylin.ghostlight\nExec=\"/usr/bin/other\"\n",
        );
        let steps = retire_flatpak_route(&plan);
        assert_eq!(
            results(&steps),
            [
                (FlatpakPart::Manifest, FlatpakResult::LeftForeign),
                (FlatpakPart::Registration, FlatpakResult::LeftForeign),
                (FlatpakPart::PermissionOverride, FlatpakResult::Restored),
                (FlatpakPart::Custody, FlatpakResult::Removed),
            ]
        );
        assert!(route.manifest.is_file() && route.registration.is_file());
        assert!(fixture.plan().is_empty());
    }

    #[test]
    fn files_that_were_never_ghostlights_do_not_plan_anything() {
        let fixture = Fixture::new(false);
        let route = FlatpakLeftovers::below(&fixture.roots.user_home);
        fixture.write(&route.manifest, "{\"name\":\"org.example.other\"}");
        fixture.write(
            &route.registration,
            "[D-BUS Service]\nName=org.example.other\n",
        );
        fixture.write(&route.permission_override, OVERRIDE_AFTER);
        fixture.write(&route.custody, "not a record");
        let plan = fixture.plan();
        assert!(plan.is_empty());
        assert!(retire_flatpak_route(&plan).is_empty());
        assert!(route.manifest.is_file() && route.permission_override.is_file());
    }

    #[test]
    fn a_permission_that_was_already_there_is_left_alone() {
        let fixture = Fixture::new(false);
        let route = flatpak_route(&fixture, Some(OVERRIDE_AFTER));
        fixture.write(&route.custody, &custody(Some(OVERRIDE_AFTER), false));
        let steps = retire_flatpak_route(&fixture.plan());
        assert!(!results(&steps)
            .iter()
            .any(|(part, _)| *part == FlatpakPart::PermissionOverride));
        assert_eq!(
            fs::read_to_string(&route.permission_override).unwrap(),
            OVERRIDE_AFTER
        );
        assert!(!route.custody.exists());
    }

    #[test]
    fn an_override_an_earlier_run_already_restored_is_not_reported_as_changed() {
        let fixture = Fixture::new(false);
        let route = flatpak_route(&fixture, Some(OVERRIDE_BEFORE));
        // An earlier run restored the override and then failed a later step, so the record stays.
        fixture.write(&route.permission_override, OVERRIDE_BEFORE);
        let steps = retire_flatpak_route(&fixture.plan());
        assert_eq!(
            results(&steps),
            [
                (FlatpakPart::Manifest, FlatpakResult::Removed),
                (FlatpakPart::Registration, FlatpakResult::Removed),
                (FlatpakPart::Custody, FlatpakResult::Removed),
            ]
        );
        assert_eq!(
            fs::read_to_string(&route.permission_override).unwrap(),
            OVERRIDE_BEFORE
        );
        assert!(fixture.plan().is_empty());
    }

    #[test]
    fn nothing_is_granted_when_the_override_is_gone() {
        let fixture = Fixture::new(false);
        let route = flatpak_route(&fixture, Some(OVERRIDE_BEFORE));
        fs::remove_file(&route.permission_override).unwrap();
        retire_flatpak_route(&fixture.plan());
        assert!(!route.permission_override.exists());
    }

    #[test]
    fn a_directory_at_a_route_path_is_foreign() {
        let fixture = Fixture::new(false);
        let route = flatpak_route(&fixture, Some(OVERRIDE_BEFORE));
        let plan = fixture.plan();
        fs::remove_file(&route.manifest).unwrap();
        fs::create_dir(&route.manifest).unwrap();
        let steps = retire_flatpak_route(&plan);
        assert_eq!(
            results(&steps)[0],
            (FlatpakPart::Manifest, FlatpakResult::LeftForeign)
        );
        assert!(route.manifest.is_dir());
    }

    #[test]
    fn a_process_that_exits_before_its_kill_counts_as_stopped() {
        assert_eq!(stop_outcome(true, false), StopOutcome::Stopping);
        assert_eq!(stop_outcome(false, true), StopOutcome::AlreadyGone);
        assert_eq!(stop_outcome(false, false), StopOutcome::Failed);
    }
}
