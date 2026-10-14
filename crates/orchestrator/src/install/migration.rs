//! The one 2.0 break migration: from any 1.x layout to the fixed home.
//!
//! Before 2.0 an installation record chose where the executables, the state and the policy
//! lived, and each of those could sit in a different place on each platform. 2.0 keeps
//! everything under the Ghostlight home (ADR-0197 Decisions 10, 16 and 22). This module reads
//! the old layout, says in a [`MigrationPlan`] what will move, be archived, be deleted, be
//! stopped and be re-registered, and [`execute`]s that plan.
//!
//! The plan is data: `deployment install --plan` prints it. Executing it stops the old images
//! first, then moves each file by rename (or copy, verify and delete across volumes), and deletes
//! the old record last. Any failure stops before the new executables are swapped in and leaves
//! the source file where it was. A second run continues from whatever a failed run completed,
//! and a run after success has nothing to do.

use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use ghostlight_relay::home::SERVICE_EXECUTABLE_NAME;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::entry;
use super::layout::{
    Layout, BROWSER_CONNECTOR_EXECUTABLE_NAME, INSTALL_RECORD_NAME, MCP_CONNECTOR_EXECUTABLE_NAME,
};
use super::native_host::NativeHostRegistry;
use super::record::{self, InstallRecord, Version};
use super::{HarnessRegistry, HarnessState};

/// Largest installation record the migration reads.
const LEGACY_RECORD_LIMIT: u64 = 64 * 1024;

/// How long a stopped process may take to exit.
const STOP_WAIT: Duration = Duration::from_secs(10);

/// File name of the 1.x installation record, beside the runtime file in the home.
const LEGACY_RECORD_NAME: &str = "ghostlight-runtime.installation.json";

/// File name of the 1.x startup admission file in the home.
const LEGACY_STARTUP_NAME: &str = "ghostlight-runtime.startup";

/// File name of the 1.x installation lock in the home.
const LEGACY_INSTALLATION_LOCK_NAME: &str = "ghostlight-runtime.installation.lock";

/// File name of the 1.x deployment lock, kept in the home and in the serving directory.
const LEGACY_DEPLOY_LOCK_NAME: &str = "deploy.lock";

/// The audit log, in the 1.x state directory and again in `home/state`.
const AUDIT_FILE_NAME: &str = "audit.jsonl";

/// The user policy file, in the 1.x policy directory and again in `home/policy`.
pub(super) const USER_POLICY_FILE_NAME: &str = "user-policy.json";

/// The verified managed-policy cache, in the 1.x policy directory and again in `home/policy`.
const MANAGED_CACHE_FILE_NAME: &str = "managed-policy.bundle";

/// The managed-policy status file, in the 1.x policy directory and again in `home/policy`.
const MANAGED_STATUS_FILE_NAME: &str = "managed-status.json";

/// The extension walkthrough marker, in the 1.x policy directory and again in `home/state`.
const HANDOFF_MARKER_NAME: &str = "extension-handoff-v1";

/// The MCP client location file, in the 1.x configuration directory and again in `home/state`.
const HARNESS_LOCATIONS_NAME: &str = "harness-locations.json";

/// Directory below the 1.x configuration root that held the client location file.
const LEGACY_CONFIG_DIRECTORY_NAME: &str = "ghostlight";

/// Directory below the Windows local-data root that held the 1.x policy.
const LEGACY_WINDOWS_POLICY_DIRECTORY_NAME: &str = "Ghostlight";

/// Directory below the Linux state root that held the 1.x policy.
const LEGACY_LINUX_POLICY_DIRECTORY_NAME: &str = "ghostlight";

/// Directory below the Linux cache root that held state beside a `/usr/bin` install.
const LEGACY_CACHE_DIRECTORY_NAME: &str = "ghostlight";

/// Label used in the archived audit file name when the old version cannot be read.
const UNKNOWN_VERSION_LABEL: &str = "pre-2.0";

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
    /// A destination already holds something other than the file being moved.
    #[error(
        "{destination} already holds different content than {source_path}. Nothing was changed; \
         keep the one you want and remove the other"
    )]
    Conflict {
        /// The file that would have moved.
        source_path: PathBuf,
        /// The destination that is already occupied.
        destination: PathBuf,
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
    /// An old Ghostlight program would not stop.
    #[error("Could not stop the old Ghostlight programs: {0}. Nothing was changed")]
    Stop(String),
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
    fn harness_locations_directory(&self) -> PathBuf {
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
                !same_path(directory, &layout.bin()) && !same_path(directory, &layout.dev())
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
    /// Old lifecycle files deleted, the installation record last.
    pub deletions: Vec<PathBuf>,
    /// Old executables stopped before anything moves.
    pub images_to_stop: Vec<PathBuf>,
    /// Registrations to rewrite to the fixed paths.
    pub registrations_to_reclaim: Vec<Registration>,
    /// Directories read that lie inside a packaged app's private storage.
    pub virtualized_sources: Vec<PathBuf>,
}

impl MigrationPlan {
    /// Whether there is nothing to do.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
            && self.audit_archive.is_none()
            && self.deletions.is_empty()
            && self.images_to_stop.is_empty()
            && self.registrations_to_reclaim.is_empty()
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
    let placements: [(&str, &[PathBuf], PathBuf); 5] = [
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
            HANDOFF_MARKER_NAME,
            policy_sources.as_slice(),
            layout.state(),
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
        for name in [
            SERVICE_EXECUTABLE_NAME,
            MCP_CONNECTOR_EXECUTABLE_NAME,
            BROWSER_CONNECTOR_EXECUTABLE_NAME,
        ] {
            let image = directory.join(name);
            if image.is_file() {
                push_unique(&mut plan.images_to_stop, image);
            }
        }
    }
    plan.registrations_to_reclaim = legacy.registrations.clone();
    Ok(plan)
}

/// Carry out `plan`.
///
/// Everything that can fail before a file is touched is checked first: destinations are made
/// and none may hold different content. Then the old programs are stopped, each file is moved,
/// and the deletions run. A failure returns an error naming the path and leaves the file it was
/// moving in place, and running the same plan again continues from there.
pub fn execute(plan: &MigrationPlan) -> Result<(), MigrationError> {
    for transfer in plan.transfers() {
        prepare(transfer)?;
    }
    stop_legacy_images(&plan.images_to_stop)
        .map_err(|error| MigrationError::Stop(error.to_string()))?;
    for transfer in plan.transfers() {
        relocate(transfer)?;
    }
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

/// Make the destination directory and refuse a destination that holds something else.
fn prepare(transfer: &Move) -> Result<(), MigrationError> {
    if !transfer.from.is_file() || same_location(&transfer.from, &transfer.to) {
        return Ok(());
    }
    if let Some(parent) = transfer.to.parent() {
        fs::create_dir_all(parent).map_err(|source| MigrationError::Prepare {
            path: parent.to_owned(),
            source,
        })?;
    }
    if transfer.to.exists() && !same_content(&transfer.from, &transfer.to) {
        return Err(MigrationError::Conflict {
            source_path: transfer.from.clone(),
            destination: transfer.to.clone(),
        });
    }
    Ok(())
}

/// Move one file: rename it, or copy it, verify the copy and delete the original.
fn relocate(transfer: &Move) -> Result<(), MigrationError> {
    let Move { from, to } = transfer;
    if !from.is_file() || same_location(from, to) {
        return Ok(());
    }
    if to.exists() {
        // A previous run copied the file and stopped before deleting the original.
        return remove_original(from);
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_verified(from, to).map_err(|source| MigrationError::Move {
        from: from.clone(),
        to: to.clone(),
        source,
    })?;
    remove_original(from)
}

/// Delete an original whose content is already at its destination.
fn remove_original(path: &Path) -> Result<(), MigrationError> {
    fs::remove_file(path).map_err(|source| MigrationError::Remove {
        path: path.to_owned(),
        source,
    })
}

/// Copy `from` to `to` through a temporary file and publish it only if its hash matches.
fn copy_verified(from: &Path, to: &Path) -> io::Result<()> {
    let mut temporary = to.as_os_str().to_owned();
    temporary.push(format!(".{}.migrating", uuid::Uuid::new_v4().simple()));
    let temporary = PathBuf::from(temporary);
    let published = (|| {
        let mut target = File::create_new(&temporary)?;
        io::copy(&mut File::open(from)?, &mut target)?;
        target.sync_all()?;
        drop(target);
        fs::set_permissions(&temporary, fs::metadata(from)?.permissions())?;
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
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    if images.is_empty() {
        return Ok(());
    }
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_exe(UpdateKind::Always),
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::scratch::Scratch;
    use super::*;

    /// Bytes of the audit log the fake 1.x installation holds.
    const AUDIT: &str = "{\"event\":\"old\"}\n";

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
            [
                SERVICE_EXECUTABLE_NAME,
                MCP_CONNECTOR_EXECUTABLE_NAME,
                BROWSER_CONNECTOR_EXECUTABLE_NAME,
            ]
            .into_iter()
            .map(|name| {
                let path = directory.join(name);
                self.write(&path, name);
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
                moves(&policy, HANDOFF_MARKER_NAME, &state),
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
        assert_eq!(
            plan.deletions,
            vec![
                fixture.home.join(LEGACY_STARTUP_NAME),
                fixture.home.join(LEGACY_DEPLOY_LOCK_NAME),
                fixture
                    .path("repo/target/release")
                    .join(LEGACY_DEPLOY_LOCK_NAME),
                fixture.home.join(LEGACY_INSTALLATION_LOCK_NAME),
                fixture.home.join(LEGACY_RECORD_NAME),
            ]
        );
        let repo_release = fixture.path("repo/target/release");
        let bin = fixture.home.join("bin");
        let expected: Vec<PathBuf> = [repo_release, bin]
            .iter()
            .flat_map(|directory| {
                [
                    SERVICE_EXECUTABLE_NAME,
                    MCP_CONNECTOR_EXECUTABLE_NAME,
                    BROWSER_CONNECTOR_EXECUTABLE_NAME,
                ]
                .map(|name| directory.join(name))
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
        assert!(state.join(HANDOFF_MARKER_NAME).is_file());
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
                "images_to_stop",
                "moves",
                "registrations_to_reclaim",
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
}
