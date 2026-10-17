//! Each agent's own icon: where the catalog says it may be found, the one resolver that reads it
//! from the installed app, and the cache the window serves it from.
//!
//! The roster names, per host, the places an agent's icon may be found ([`IconSource`]), in
//! order, and the agent whose icon stands for it when none of its own is found
//! ([`AgentIconSources::family`]). The resolver tries the sources in order, off the request path
//! and bounded in time, and keeps the first icon it finds as `state/icons/<agent id>.<png|ico>`
//! with a manifest beside it (`<agent id>.json`: the file the icon came from, that file's size and
//! modification time, and the copy's SHA-256). A later look reads a source again only when its
//! file changed, and drops a copy whose source is gone. A source that cannot be read is skipped,
//! with a diagnostics line when diagnostics are on. An agent with no icon found keeps its
//! monogram, which is the normal case.
//!
//! On Linux an icon comes from the agent's desktop entry and the icon theme, PNG only, because a
//! local SVG could carry script. On Windows this build finds the program a source names, through a
//! launcher script on `PATH` when there is one, but reads no program's icon resource and no Store
//! package's logo, so a Windows source gives no icon.
//!
//! The agent symbols read the cache the service publishes ([`publish`], [`published_version`]),
//! and the window serves its copies ([`IconCache::icon`]). Everything here is a local read of the
//! installed apps' files; nothing is sent anywhere.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use ghostlight_bridge::diagnostics::{event, Level, Sink};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::record::atomic_replace;
use super::{lock, HOME_VARIABLE, USER_PROFILE_VARIABLE};

/// The longest a look at every agent's sources takes before it stops; agents it did not reach
/// keep what was cached for them.
const LOOK_LIMIT: Duration = Duration::from_secs(5);

/// The largest icon file read or kept.
const MAX_ICON_BYTES: u64 = 1024 * 1024;

/// The largest desktop entry or launcher script read.
const MAX_TEXT_BYTES: u64 = 64 * 1024;

/// How many characters of the copy's SHA-256 name its version in the window's address.
pub const VERSION_LENGTH: usize = 12;

/// The suffix of a cached icon's manifest.
const MANIFEST_SUFFIX: &str = ".json";

/// The name of the thread that looks for icons.
const THREAD_NAME: &str = "agent-icons";

/// The first bytes of every PNG file.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// The extension of a PNG file.
const PNG_EXTENSION: &str = "png";

/// The Windows variable that names the per-user application data folder.
const LOCAL_APP_DATA_VARIABLE: &str = "LOCALAPPDATA";

/// The Windows variable that names the machine-wide program folder.
const PROGRAM_FILES_VARIABLE: &str = "ProgramFiles";

/// The variable that lists the directories commands are found in.
const PATH_VARIABLE: &str = "PATH";

/// The Linux variable that names the person's own data directory (XDG base directories).
const XDG_DATA_HOME_VARIABLE: &str = "XDG_DATA_HOME";

/// The Linux variable that lists the system data directories, separated by colons.
const XDG_DATA_DIRS_VARIABLE: &str = "XDG_DATA_DIRS";

/// The system data directories when `XDG_DATA_DIRS` is unset or empty.
const DEFAULT_DATA_DIRS: [&str; 2] = ["/usr/local/share", "/usr/share"];

/// Flatpak's export of the desktop entries and icons of the apps installed for every user.
const FLATPAK_SYSTEM_EXPORTS: &str = "/var/lib/flatpak/exports/share";

/// Flatpak's export below a data directory, of the apps installed for one person.
const FLATPAK_USER_EXPORTS: &str = "flatpak/exports/share";

/// Snap's directory of the desktop entries and icons of installed snaps.
const SNAP_EXPORTS: &str = "/var/lib/snapd/desktop";

/// The directory below a data directory that holds desktop entries.
const APPLICATIONS_DIRECTORY: &str = "applications";

/// The suffix of a desktop entry file.
const DESKTOP_SUFFIX: &str = ".desktop";

/// The group of a desktop entry that describes the app.
const DESKTOP_ENTRY_GROUP: &str = "[Desktop Entry]";

/// The desktop entry key that names the app's icon.
const ICON_KEY: &str = "Icon";

/// The theme every icon lookup falls back to.
const HICOLOR_THEME: &str = "icons/hicolor";

/// The directory of an icon theme size that holds application icons.
const APPS_DIRECTORY: &str = "apps";

/// The directory below a data directory of icons outside any theme.
const PIXMAPS_DIRECTORY: &str = "pixmaps";

/// The theme sizes looked at first, largest first: the sizes the window draws well.
const PREFERRED_SIZES: [u32; 4] = [128, 96, 64, 48];

/// The theme sizes looked at after the unthemed icons, when an app ships no preferred size.
const OTHER_SIZES: [u32; 4] = [256, 512, 32, 24];

/// The suffix of a Windows program.
const EXE_SUFFIX: &str = ".exe";

/// The launcher scripts a command on `PATH` may be, in the order Windows runs them.
const LAUNCHER_SUFFIXES: [&str; 2] = [".cmd", ".ps1"];

/// How a launcher script names its own directory: `%~dp0` in a batch file, `%dp0%` in an npm
/// batch shim, and `$basedir` or `$PSScriptRoot` in a PowerShell one.
const LAUNCHER_DIRECTORY_MARKS: [&str; 4] = ["%~dp0", "%dp0%", "$basedir", "$PSScriptRoot"];

/// Programs a launcher starts to run a script, never the agent's own program: an npm shim names
/// Node, and such a launcher names no program of the agent.
const INTERPRETERS: [&str; 8] = [
    "node.exe",
    "bun.exe",
    "deno.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "wscript.exe",
    "cscript.exe",
];

/// A folder Windows names for the person, below which a program may be installed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnownFolder {
    /// `%LOCALAPPDATA%`, where per-user installers put `Programs\<app>`.
    LocalAppData,
    /// `%ProgramFiles%`, where machine-wide installers put programs.
    ProgramFiles,
}

/// Where a Windows program may be.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramPlace {
    /// A command on `PATH`: its `.exe`, or the program its `.cmd` or `.ps1` launcher starts when
    /// the launcher names one. An npm shim, which starts a script under Node, names none.
    OnPath(&'static str),
    /// A program file below a known folder, its path written with forward slashes.
    Below(KnownFolder, &'static str),
}

/// One place the catalog says an agent's own icon may be found on a host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IconSource {
    /// The icon resource of a Windows program, at the first of these places that holds one.
    WindowsExe(&'static [ProgramPlace]),
    /// The logo of a Microsoft Store (MSIX) app, by its package name. Its package family name is
    /// that name, an underscore and the publisher's id.
    WindowsPackage(&'static str),
    /// A Linux desktop entry, by its desktop file ids in order, and the icon it names.
    LinuxDesktop(&'static [&'static str]),
}

/// One agent's icon facts from the catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentIconSources {
    /// The agent's catalog id.
    pub agent_id: &'static str,
    /// Where its own icon may be found, in order: its hosts' sources in roster order.
    pub sources: Vec<IconSource>,
    /// The agent whose icon stands for this one when none of its own is found.
    pub family: Option<&'static str>,
}

/// The kinds of icon the cache keeps and the window serves.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IconKind {
    /// A PNG image.
    Png,
    /// A Windows icon file.
    Ico,
}

impl IconKind {
    /// Every kind, for removing a copy whatever its kind.
    const ALL: [Self; 2] = [Self::Png, Self::Ico];

    /// The extension of a cached copy of this kind.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => PNG_EXTENSION,
            Self::Ico => "ico",
        }
    }

    /// The media type the window serves a copy of this kind with.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Ico => "image/x-icon",
        }
    }
}

/// The computer an agent's icon is looked for on.
#[derive(Clone, Debug, Default)]
pub struct IconPlaces {
    /// Whether the computer runs Windows: Windows sources are looked at there, Linux ones
    /// elsewhere.
    windows: bool,
    /// The folder [`KnownFolder::LocalAppData`] names, when known.
    local_app_data: Option<PathBuf>,
    /// The folder [`KnownFolder::ProgramFiles`] names, when known.
    program_files: Option<PathBuf>,
    /// The directories commands are found in, in order.
    path_entries: Vec<PathBuf>,
    /// The data directories desktop entries and icons are found in, in order.
    data_dirs: Vec<PathBuf>,
}

impl IconPlaces {
    /// This computer, read from the environment of this process.
    #[must_use]
    pub fn system() -> Self {
        let windows = cfg!(target_os = "windows");
        let home = if windows {
            env::var_os(USER_PROFILE_VARIABLE).or_else(|| env::var_os(HOME_VARIABLE))
        } else {
            env::var_os(HOME_VARIABLE)
        }
        .map(PathBuf::from)
        .unwrap_or_default();
        let path_entries = env::var_os(PATH_VARIABLE)
            .map(|path| env::split_paths(&path).collect())
            .unwrap_or_default();
        if windows {
            Self {
                windows,
                local_app_data: Some(
                    env::var_os(LOCAL_APP_DATA_VARIABLE)
                        .map_or_else(|| home.join("AppData").join("Local"), PathBuf::from),
                ),
                program_files: env::var_os(PROGRAM_FILES_VARIABLE).map(PathBuf::from),
                path_entries,
                data_dirs: Vec::new(),
            }
        } else {
            Self {
                windows,
                local_app_data: None,
                program_files: None,
                path_entries,
                data_dirs: linux_data_dirs(
                    &home,
                    env::var_os(XDG_DATA_HOME_VARIABLE),
                    env::var_os(XDG_DATA_DIRS_VARIABLE),
                ),
            }
        }
    }

    /// A Linux computer whose data directories are `data_dirs`, for tests that must not read this
    /// one.
    #[cfg(test)]
    pub(crate) fn linux(data_dirs: Vec<PathBuf>) -> Self {
        Self {
            data_dirs,
            ..Self::default()
        }
    }

    /// A Windows computer with these folders and `PATH`, for tests that must not read this one.
    #[cfg(test)]
    pub(crate) fn windows(
        local_app_data: PathBuf,
        program_files: PathBuf,
        path_entries: Vec<PathBuf>,
    ) -> Self {
        Self {
            windows: true,
            local_app_data: Some(local_app_data),
            program_files: Some(program_files),
            path_entries,
            data_dirs: Vec::new(),
        }
    }

    fn folder(&self, folder: KnownFolder) -> Option<&Path> {
        match folder {
            KnownFolder::LocalAppData => self.local_app_data.as_deref(),
            KnownFolder::ProgramFiles => self.program_files.as_deref(),
        }
    }
}

/// The data directories of a Linux computer, in the order an icon is looked for: the person's own
/// (`XDG_DATA_HOME`, else `~/.local/share`), the system's (`XDG_DATA_DIRS`, else
/// `/usr/local/share` and `/usr/share`), then the Flatpak exports (the person's, then every
/// user's) and Snap's. Each directory is listed once.
fn linux_data_dirs(
    home: &Path,
    data_home: Option<OsString>,
    data_dirs: Option<OsString>,
) -> Vec<PathBuf> {
    let data_home = data_home
        .filter(|value| !value.is_empty())
        .map_or_else(|| home.join(".local").join("share"), PathBuf::from);
    let system: Vec<PathBuf> = data_dirs
        .map(|value| {
            value
                .to_string_lossy()
                .split(':')
                .filter(|entry| !entry.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .filter(|entries| !entries.is_empty())
        .unwrap_or_else(|| DEFAULT_DATA_DIRS.iter().map(PathBuf::from).collect());
    let mut dirs = vec![data_home.clone()];
    dirs.extend(system);
    dirs.push(data_home.join(FLATPAK_USER_EXPORTS));
    dirs.push(PathBuf::from(FLATPAK_SYSTEM_EXPORTS));
    dirs.push(PathBuf::from(SNAP_EXPORTS));
    let mut listed = Vec::new();
    for dir in dirs {
        if !listed.contains(&dir) {
            listed.push(dir);
        }
    }
    listed
}

/// What a cached icon's manifest says: where the icon came from and what the copy holds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Manifest {
    /// The file the icon was read from.
    source: PathBuf,
    /// That file's size in bytes when it was read.
    size: u64,
    /// That file's modification time when it was read, in nanoseconds since the Unix epoch.
    modified_ns: u64,
    /// The kind of the copy.
    kind: IconKind,
    /// The SHA-256 of the copy, in lowercase hex.
    sha256: String,
}

impl Manifest {
    /// Whether the icon came from `stamp`'s file as it is now.
    fn read_from(&self, stamp: &Stamp) -> bool {
        self.source == stamp.source
            && self.size == stamp.size
            && self.modified_ns == stamp.modified_ns
    }
}

/// A source file as it is now.
struct Stamp {
    source: PathBuf,
    size: u64,
    modified_ns: u64,
}

impl Stamp {
    fn of(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let modified_ns = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_nanos()).unwrap_or(u64::MAX)
            });
        Ok(Self {
            source: path.to_path_buf(),
            size: metadata.len(),
            modified_ns,
        })
    }
}

/// A file an icon may be read from.
enum Located {
    /// A PNG image.
    Png(PathBuf),
    /// A Windows program, whose icon resource this build does not read.
    Program(PathBuf),
}

impl Located {
    fn path(&self) -> &Path {
        match self {
            Self::Png(path) | Self::Program(path) => path,
        }
    }
}

/// What one look decided for one agent.
enum Outcome {
    /// The cached copy still stands for the source it came from.
    Kept(Manifest),
    /// A new copy, with its manifest.
    Fresh(Manifest, Vec<u8>),
}

impl Outcome {
    fn manifest(&self) -> &Manifest {
        match self {
            Self::Kept(manifest) | Self::Fresh(manifest, _) => manifest,
        }
    }
}

/// Why a source was skipped, for the diagnostics line.
enum Skip {
    /// The file could not be read.
    Unreadable(io::ErrorKind),
    /// The file is larger than [`MAX_ICON_BYTES`].
    TooLarge,
    /// The file named as a PNG is not one.
    NotPng,
    /// The copy could not be written to the cache.
    NotKept(io::ErrorKind),
    /// The look ran out of time before it reached this agent.
    OutOfTime,
}

impl Skip {
    fn detail(&self, agent_id: &str) -> String {
        let reason = match self {
            Self::Unreadable(kind) => format!("source unreadable ({kind:?})"),
            Self::TooLarge => "source larger than an icon may be".to_owned(),
            Self::NotPng => "source is not a PNG".to_owned(),
            Self::NotKept(kind) => format!("copy not kept ({kind:?})"),
            Self::OutOfTime => "the look ran out of time".to_owned(),
        };
        format!("{agent_id}: {reason}")
    }
}

/// A cached icon as the window serves it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedIcon {
    /// Its kind, which says the media type.
    pub kind: IconKind,
    /// Its bytes.
    pub bytes: Vec<u8>,
    /// Its SHA-256, in lowercase hex.
    pub sha256: String,
}

/// The cached icons of the catalog's agents, and the one resolver that keeps them.
///
/// Cheap to clone: every clone is the same cache.
#[derive(Clone)]
pub struct IconCache {
    inner: Arc<CacheInner>,
}

struct CacheInner {
    directory: PathBuf,
    places: IconPlaces,
    catalog: Vec<AgentIconSources>,
    sink: Option<Arc<Sink>>,
    /// The manifest of each agent with a cached copy.
    shelf: RwLock<BTreeMap<&'static str, Manifest>>,
    /// Held while a look runs, so two never write at once.
    looking: Mutex<()>,
    /// Whether a background look runs, and whether another was asked for meanwhile.
    runs: Mutex<Runs>,
}

#[derive(Default)]
struct Runs {
    running: bool,
    again: bool,
}

impl IconCache {
    /// The cache in `directory` for the catalog's agents on the computer `places` describes,
    /// reporting skipped sources to `sink`. The copies a manifest still vouches for are offered at
    /// once; nothing is looked for until [`Self::refresh`].
    #[must_use]
    pub fn open(directory: PathBuf, places: IconPlaces, sink: Option<Arc<Sink>>) -> Self {
        Self::over(directory, places, super::icon_catalog(), sink)
    }

    /// Like [`Self::open`], for the agents of `catalog`, for tests that name their own sources.
    #[cfg(test)]
    pub(crate) fn with_catalog(
        directory: PathBuf,
        places: IconPlaces,
        catalog: Vec<AgentIconSources>,
    ) -> Self {
        Self::over(directory, places, catalog, None)
    }

    fn over(
        directory: PathBuf,
        places: IconPlaces,
        catalog: Vec<AgentIconSources>,
        sink: Option<Arc<Sink>>,
    ) -> Self {
        let shelf = catalog
            .iter()
            .filter_map(|agent| {
                kept_manifest(&directory, agent.agent_id).map(|manifest| (agent.agent_id, manifest))
            })
            .collect();
        Self {
            inner: Arc::new(CacheInner {
                directory,
                places,
                catalog,
                sink,
                shelf: RwLock::new(shelf),
                looking: Mutex::new(()),
                runs: Mutex::new(Runs::default()),
            }),
        }
    }

    /// The version of `agent_id`'s cached icon, the first [`VERSION_LENGTH`] characters of its
    /// SHA-256; none when no icon is cached for it.
    #[must_use]
    pub fn version(&self, agent_id: &str) -> Option<String> {
        self.shelf()
            .get(agent_id)
            .map(|manifest| manifest.sha256.chars().take(VERSION_LENGTH).collect())
    }

    /// The cached icon of `agent_id`, read from the cache now; none for an id outside the catalog
    /// or an agent with no icon cached.
    #[must_use]
    pub fn icon(&self, agent_id: &str) -> Option<CachedIcon> {
        let manifest = self.shelf().get(agent_id)?.clone();
        let bytes = fs::read(copy_path(&self.inner.directory, agent_id, manifest.kind)).ok()?;
        Some(CachedIcon {
            kind: manifest.kind,
            bytes,
            sha256: manifest.sha256,
        })
    }

    /// Look for every agent's icon now, bounded by [`LOOK_LIMIT`], and keep what is found.
    /// Returns whether any agent's icon changed.
    pub fn refresh(&self) -> bool {
        let _looking = lock(&self.inner.looking);
        let before = self.shelf().clone();
        let deadline = Instant::now() + LOOK_LIMIT;
        let mut outcomes: BTreeMap<&'static str, Option<Outcome>> = BTreeMap::new();
        for agent in &self.inner.catalog {
            if Instant::now() >= deadline {
                self.report(agent.agent_id, &Skip::OutOfTime);
                break;
            }
            let kept = before.get(agent.agent_id);
            outcomes.insert(agent.agent_id, self.own_icon(agent, kept));
        }
        for agent in &self.inner.catalog {
            let Some(family) = agent.family else {
                continue;
            };
            if !matches!(outcomes.get(agent.agent_id), Some(None)) {
                continue;
            }
            let stood_in = match outcomes.get(family) {
                Some(Some(outcome)) => {
                    self.stand_in(agent.agent_id, family, outcome, before.get(agent.agent_id))
                }
                _ => None,
            };
            outcomes.insert(agent.agent_id, stood_in);
        }
        let mut after = before.clone();
        for (agent_id, outcome) in outcomes {
            match outcome {
                Some(Outcome::Kept(manifest)) => {
                    after.insert(agent_id, manifest);
                }
                Some(Outcome::Fresh(manifest, bytes)) => {
                    match self.keep(agent_id, &manifest, &bytes) {
                        Ok(()) => {
                            after.insert(agent_id, manifest);
                        }
                        Err(error) => {
                            self.report(agent_id, &Skip::NotKept(error.kind()));
                            self.drop_copy(agent_id);
                            after.remove(agent_id);
                        }
                    }
                }
                None => {
                    if after.remove(agent_id).is_some() {
                        self.drop_copy(agent_id);
                    }
                }
            }
        }
        let changed = after != before;
        *self
            .inner
            .shelf
            .write()
            .unwrap_or_else(PoisonError::into_inner) = after;
        changed
    }

    /// Run [`Self::refresh`] on its own thread, then call `changed` when an icon changed. A look
    /// asked for while one runs runs once more after it, never at the same time.
    pub fn refresh_in_background(&self, changed: impl Fn() + Send + 'static) {
        {
            let mut runs = lock(&self.inner.runs);
            if runs.running {
                runs.again = true;
                return;
            }
            runs.running = true;
        }
        let cache = self.clone();
        let spawned = thread::Builder::new()
            .name(THREAD_NAME.to_owned())
            .spawn(move || loop {
                if cache.refresh() {
                    changed();
                }
                let mut runs = lock(&cache.inner.runs);
                if runs.again {
                    runs.again = false;
                } else {
                    runs.running = false;
                    break;
                }
            });
        if spawned.is_err() {
            lock(&self.inner.runs).running = false;
        }
    }

    fn shelf(&self) -> std::sync::RwLockReadGuard<'_, BTreeMap<&'static str, Manifest>> {
        self.inner
            .shelf
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The first icon `agent`'s own sources give, in order. `kept` is the manifest of its cached
    /// copy, which still stands while its source file is unchanged.
    fn own_icon(&self, agent: &AgentIconSources, kept: Option<&Manifest>) -> Option<Outcome> {
        for source in &agent.sources {
            for located in self.locate(agent.agent_id, *source) {
                let stamp = match Stamp::of(located.path()) {
                    Ok(stamp) => stamp,
                    Err(error) => {
                        self.report(agent.agent_id, &Skip::Unreadable(error.kind()));
                        continue;
                    }
                };
                if let Some(kept) = kept.filter(|kept| {
                    kept.read_from(&stamp)
                        && copy_path(&self.inner.directory, agent.agent_id, kept.kind).is_file()
                }) {
                    return Some(Outcome::Kept(kept.clone()));
                }
                match read_icon(&located, &stamp) {
                    Ok(Some((kind, bytes))) => {
                        let manifest = Manifest {
                            source: stamp.source,
                            size: stamp.size,
                            modified_ns: stamp.modified_ns,
                            kind,
                            sha256: sha256_hex(&bytes),
                        };
                        return Some(Outcome::Fresh(manifest, bytes));
                    }
                    Ok(None) => {}
                    Err(skip) => self.report(agent.agent_id, &skip),
                }
            }
        }
        None
    }

    /// The icon of `family`, as this look found it, standing for `agent_id`, whose cached copy has
    /// the manifest `kept`.
    fn stand_in(
        &self,
        agent_id: &str,
        family: &str,
        found: &Outcome,
        kept: Option<&Manifest>,
    ) -> Option<Outcome> {
        let manifest = found.manifest();
        if let Some(kept) = kept.filter(|kept| {
            *kept == manifest && copy_path(&self.inner.directory, agent_id, kept.kind).is_file()
        }) {
            return Some(Outcome::Kept(kept.clone()));
        }
        let bytes = match found {
            Outcome::Fresh(_, bytes) => bytes.clone(),
            Outcome::Kept(_) => {
                fs::read(copy_path(&self.inner.directory, family, manifest.kind)).ok()?
            }
        };
        Some(Outcome::Fresh(manifest.clone(), bytes))
    }

    /// The files `source` names that exist on this computer, in order.
    fn locate(&self, agent_id: &str, source: IconSource) -> Vec<Located> {
        let places = &self.inner.places;
        match source {
            IconSource::WindowsExe(programs) if places.windows => programs
                .iter()
                .filter_map(|place| program(places, *place))
                .map(Located::Program)
                .collect(),
            IconSource::LinuxDesktop(ids) if !places.windows => ids
                .iter()
                .filter_map(|id| self.desktop_icon(agent_id, id))
                .map(Located::Png)
                .collect(),
            // This build does not find a Store package's install folder, and a source for the
            // other system names nothing here.
            IconSource::WindowsPackage(_)
            | IconSource::WindowsExe(_)
            | IconSource::LinuxDesktop(_) => Vec::new(),
        }
    }

    /// The PNG icon the desktop entry `id` names, from the first data directory that holds the
    /// entry.
    fn desktop_icon(&self, agent_id: &str, id: &str) -> Option<PathBuf> {
        let places = &self.inner.places;
        let entry = places
            .data_dirs
            .iter()
            .map(|dir| {
                dir.join(APPLICATIONS_DIRECTORY)
                    .join(format!("{id}{DESKTOP_SUFFIX}"))
            })
            .find(|path| path.is_file())?;
        let text = match read_text(&entry) {
            Ok(text) => text?,
            Err(error) => {
                self.report(agent_id, &Skip::Unreadable(error.kind()));
                return None;
            }
        };
        theme_icon(desktop_icon_name(&text)?, &places.data_dirs)
    }

    /// Write `bytes` as `agent_id`'s copy with its manifest, and remove a copy of another kind.
    fn keep(&self, agent_id: &str, manifest: &Manifest, bytes: &[u8]) -> io::Result<()> {
        let directory = &self.inner.directory;
        atomic_replace(&copy_path(directory, agent_id, manifest.kind), bytes)?;
        for kind in IconKind::ALL
            .into_iter()
            .filter(|kind| *kind != manifest.kind)
        {
            remove_if_present(&copy_path(directory, agent_id, kind))?;
        }
        let text = serde_json::to_vec_pretty(manifest).map_err(io::Error::other)?;
        atomic_replace(&manifest_path(directory, agent_id), &text)
    }

    /// Remove `agent_id`'s copy and manifest, whatever is there.
    fn drop_copy(&self, agent_id: &str) {
        let directory = &self.inner.directory;
        for kind in IconKind::ALL {
            let _ = remove_if_present(&copy_path(directory, agent_id, kind));
        }
        let _ = remove_if_present(&manifest_path(directory, agent_id));
    }

    fn report(&self, agent_id: &str, skip: &Skip) {
        if let Some(sink) = &self.inner.sink {
            sink.emit(
                event::AGENT_ICON_SKIPPED,
                Level::Info,
                None,
                &skip.detail(agent_id),
            );
        }
    }
}

/// The cache the agent symbols read, once the service publishes one.
static PUBLISHED: OnceLock<IconCache> = OnceLock::new();

/// Make `cache` the one every agent's symbol reads. The first call wins; the service makes it once,
/// when it starts.
pub fn publish(cache: &IconCache) {
    let _ = PUBLISHED.set(cache.clone());
}

/// The version of `agent_id`'s icon in the published cache; none before a cache is published or
/// when no icon is cached for the agent.
#[must_use]
pub fn published_version(agent_id: &str) -> Option<String> {
    PUBLISHED.get()?.version(agent_id)
}

/// The manifest of `agent_id`'s cached copy in `directory`, when it reads and the copy's bytes are
/// the ones it vouches for.
fn kept_manifest(directory: &Path, agent_id: &str) -> Option<Manifest> {
    let text = fs::read(manifest_path(directory, agent_id)).ok()?;
    let manifest: Manifest = serde_json::from_slice(&text).ok()?;
    let bytes = fs::read(copy_path(directory, agent_id, manifest.kind)).ok()?;
    (sha256_hex(&bytes) == manifest.sha256).then_some(manifest)
}

fn copy_path(directory: &Path, agent_id: &str, kind: IconKind) -> PathBuf {
    directory.join(format!("{agent_id}.{}", kind.extension()))
}

fn manifest_path(directory: &Path, agent_id: &str) -> PathBuf {
    directory.join(format!("{agent_id}{MANIFEST_SUFFIX}"))
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The icon in `located`, as `stamp` saw it: a PNG is read whole when it is small enough and
/// starts like one. A program gives none, because this build reads no icon resource.
fn read_icon(located: &Located, stamp: &Stamp) -> Result<Option<(IconKind, Vec<u8>)>, Skip> {
    match located {
        Located::Png(path) => {
            if stamp.size > MAX_ICON_BYTES {
                return Err(Skip::TooLarge);
            }
            let bytes = read_bounded(path, MAX_ICON_BYTES)
                .map_err(|error| Skip::Unreadable(error.kind()))?
                .ok_or(Skip::TooLarge)?;
            if bytes.starts_with(PNG_SIGNATURE) {
                Ok(Some((IconKind::Png, bytes)))
            } else {
                Err(Skip::NotPng)
            }
        }
        Located::Program(_) => Ok(None),
    }
}

/// The bytes of `path`, or none when it holds more than `limit`.
fn read_bounded(path: &Path, limit: u64) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() as u64 <= limit).then_some(bytes))
}

/// The text of a desktop entry or launcher script, or none when it is larger than one should be.
fn read_text(path: &Path) -> io::Result<Option<String>> {
    Ok(read_bounded(path, MAX_TEXT_BYTES)?
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
}

/// The value of the `Icon` key in the `[Desktop Entry]` group of a desktop entry, when it has one.
/// Localized variants (`Icon[de]`) are not read.
fn desktop_icon_name(text: &str) -> Option<&str> {
    let mut in_entry = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == DESKTOP_ENTRY_GROUP;
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == ICON_KEY {
                let value = value.trim();
                return (!value.is_empty()).then_some(value);
            }
        }
    }
    None
}

/// The PNG file an `Icon` value names: an absolute path to a PNG, or a theme name looked for in
/// the hicolor theme of every data directory (the preferred sizes first, then the unthemed
/// `pixmaps`, then the other sizes). An SVG or any other kind is never taken.
fn theme_icon(name: &str, data_dirs: &[PathBuf]) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() {
        let png = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(PNG_EXTENSION));
        return (png && path.is_file()).then(|| path.to_path_buf());
    }
    if name.contains(['/', '\\']) {
        return None;
    }
    let file = format!("{name}.{PNG_EXTENSION}");
    let sized = |size: u32| {
        data_dirs
            .iter()
            .map(|dir| {
                dir.join(HICOLOR_THEME)
                    .join(format!("{size}x{size}"))
                    .join(APPS_DIRECTORY)
                    .join(&file)
            })
            .find(|candidate| candidate.is_file())
    };
    PREFERRED_SIZES
        .into_iter()
        .find_map(sized)
        .or_else(|| {
            data_dirs
                .iter()
                .map(|dir| dir.join(PIXMAPS_DIRECTORY).join(&file))
                .find(|candidate| candidate.is_file())
        })
        .or_else(|| OTHER_SIZES.into_iter().find_map(sized))
}

/// The Windows program `place` names on the computer `places` describes, when it exists.
fn program(places: &IconPlaces, place: ProgramPlace) -> Option<PathBuf> {
    match place {
        ProgramPlace::Below(folder, relative) => {
            let path = relative
                .split('/')
                .fold(places.folder(folder)?.to_path_buf(), |path, part| {
                    path.join(part)
                });
            path.is_file().then_some(path)
        }
        ProgramPlace::OnPath(name) => places
            .path_entries
            .iter()
            .find_map(|directory| command_program(directory, name)),
    }
}

/// The program the command `name` in `directory` is: its `.exe`, or the program its launcher
/// script starts.
fn command_program(directory: &Path, name: &str) -> Option<PathBuf> {
    let exe = directory.join(format!("{name}{EXE_SUFFIX}"));
    if exe.is_file() {
        return Some(exe);
    }
    LAUNCHER_SUFFIXES
        .iter()
        .map(|suffix| directory.join(format!("{name}{suffix}")))
        .filter(|launcher| launcher.is_file())
        .find_map(|launcher| launched_program(&launcher))
}

/// The first program a launcher script names that exists and is not an interpreter, with the
/// script's own directory marks (`%~dp0`, `$basedir`) read as the script's directory.
fn launched_program(launcher: &Path) -> Option<PathBuf> {
    let text = read_text(launcher).ok()??;
    let directory = launcher.parent()?;
    text.split(|character: char| {
        character.is_whitespace() || matches!(character, '"' | '\'' | '(' | ')' | '&' | '|' | '@')
    })
    .filter(|token| token.to_ascii_lowercase().ends_with(EXE_SUFFIX))
    .filter_map(|token| launcher_path(directory, token))
    .filter(|path| !is_interpreter(path))
    .find(|path| path.is_file())
}

/// The path a launcher's `token` names, relative to the launcher's `directory` when the token
/// starts with a directory mark. A token that names another variable names nothing here.
fn launcher_path(directory: &Path, token: &str) -> Option<PathBuf> {
    let relative = LAUNCHER_DIRECTORY_MARKS
        .iter()
        .find_map(|mark| token.strip_prefix(mark));
    let rest = relative.unwrap_or(token);
    if rest.contains(['%', '$']) {
        return None;
    }
    let Some(rest) = relative else {
        let path = PathBuf::from(rest);
        return path.is_absolute().then_some(path);
    };
    let mut path = directory.to_path_buf();
    for part in rest.split(['\\', '/']) {
        match part {
            "" | "." => {}
            ".." => {
                path.pop();
            }
            part => path.push(part),
        }
    }
    Some(path)
}

fn is_interpreter(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|name| INTERPRETERS.contains(&name.as_str()))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use serde_json::Value;

    use super::super::scratch::Scratch;
    use super::*;

    /// A tiny file that starts like a PNG, told apart from others by `mark`.
    fn png(mark: &str) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend_from_slice(mark.as_bytes());
        bytes
    }

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    /// A desktop entry `id` in `data` that names the icon `icon`.
    fn desktop_entry(data: &Path, id: &str, icon: &str) {
        write(
            &data.join("applications").join(format!("{id}.desktop")),
            format!(
                "[Desktop Entry]\nType=Application\nName={id}\nIcon[de]=other\nIcon={icon}\n\n\
                 [Desktop Action new]\nIcon=action\n"
            )
            .as_bytes(),
        );
    }

    /// The hicolor PNG `name` at `size` in `data`.
    fn theme_png(data: &Path, size: u32, name: &str, bytes: &[u8]) -> PathBuf {
        let path = data
            .join("icons/hicolor")
            .join(format!("{size}x{size}"))
            .join("apps")
            .join(format!("{name}.png"));
        write(&path, bytes);
        path
    }

    fn agent(
        agent_id: &'static str,
        sources: Vec<IconSource>,
        family: Option<&'static str>,
    ) -> AgentIconSources {
        AgentIconSources {
            agent_id,
            sources,
            family,
        }
    }

    struct Tree {
        scratch: Scratch,
    }

    impl Tree {
        fn new() -> Self {
            Self {
                scratch: Scratch::new(),
            }
        }

        fn data(&self) -> PathBuf {
            self.scratch.path().join("share")
        }

        fn cache_dir(&self) -> PathBuf {
            self.scratch.path().join("state").join("icons")
        }

        fn cache(&self, catalog: Vec<AgentIconSources>) -> IconCache {
            IconCache::with_catalog(
                self.cache_dir(),
                IconPlaces::linux(vec![self.data()]),
                catalog,
            )
        }
    }

    #[test]
    fn the_first_source_that_gives_an_icon_wins() {
        let tree = Tree::new();
        desktop_entry(&tree.data(), "second", "second-icon");
        theme_png(&tree.data(), 64, "second-icon", &png("second"));
        desktop_entry(&tree.data(), "third", "third-icon");
        theme_png(&tree.data(), 64, "third-icon", &png("third"));
        let cache = tree.cache(vec![agent(
            "editor",
            vec![
                IconSource::WindowsPackage("Editor"),
                IconSource::LinuxDesktop(&["missing"]),
                IconSource::LinuxDesktop(&["second"]),
                IconSource::LinuxDesktop(&["third"]),
            ],
            None,
        )]);
        assert!(cache.refresh());
        let icon = cache.icon("editor").unwrap();
        assert_eq!(icon.kind, IconKind::Png);
        assert_eq!(icon.bytes, png("second"));
        assert_eq!(
            cache.version("editor").unwrap(),
            sha256_hex(&png("second"))[..VERSION_LENGTH]
        );
        assert!(tree.cache_dir().join("editor.png").is_file());
    }

    #[test]
    fn an_agent_without_its_own_icon_wears_its_familys_and_nothing_found_leaves_none() {
        let tree = Tree::new();
        desktop_entry(&tree.data(), "desktop-app", "desktop-app");
        theme_png(&tree.data(), 128, "desktop-app", &png("family"));
        let cache = tree.cache(vec![
            agent(
                "terminal-tool",
                vec![IconSource::LinuxDesktop(&["absent"])],
                Some("desktop-app"),
            ),
            agent(
                "desktop-app",
                vec![IconSource::LinuxDesktop(&["desktop-app"])],
                None,
            ),
            agent("lonely", vec![IconSource::LinuxDesktop(&["absent"])], None),
            agent("orphan", Vec::new(), Some("lonely")),
        ]);
        assert!(cache.refresh());
        assert_eq!(cache.icon("terminal-tool").unwrap().bytes, png("family"));
        assert_eq!(cache.version("terminal-tool"), cache.version("desktop-app"));
        assert_eq!(cache.icon("lonely"), None);
        assert_eq!(cache.version("lonely"), None);
        assert_eq!(
            cache.version("orphan"),
            None,
            "a family with no icon gives none"
        );
        assert_eq!(cache.icon("not-in-the-catalog"), None);
        assert!(!cache.refresh(), "nothing changed, so nothing is new");
    }

    #[test]
    fn a_changed_source_refreshes_its_copy_and_a_gone_one_drops_it() {
        let tree = Tree::new();
        desktop_entry(&tree.data(), "zed", "zed");
        let source = theme_png(&tree.data(), 48, "zed", &png("old"));
        let cache = tree.cache(vec![agent(
            "zed",
            vec![IconSource::LinuxDesktop(&["zed"])],
            None,
        )]);
        assert!(cache.refresh());
        let old = cache.version("zed").unwrap();
        assert!(!cache.refresh(), "an unchanged source keeps its copy");

        write(&source, &png("a newer and longer icon"));
        assert!(cache.refresh());
        assert_ne!(cache.version("zed").unwrap(), old);
        assert_eq!(
            cache.icon("zed").unwrap().bytes,
            png("a newer and longer icon")
        );

        fs::remove_file(&source).unwrap();
        assert!(cache.refresh());
        assert_eq!(cache.version("zed"), None);
        assert!(!tree.cache_dir().join("zed.png").exists());
        assert!(!tree.cache_dir().join("zed.json").exists());
    }

    #[test]
    fn the_manifest_says_where_the_icon_came_from_and_vouches_for_the_copy() {
        let tree = Tree::new();
        desktop_entry(&tree.data(), "kiro", "kiro");
        let source = theme_png(&tree.data(), 96, "kiro", &png("kiro"));
        let catalog = vec![agent(
            "kiro",
            vec![IconSource::LinuxDesktop(&["kiro"])],
            None,
        )];
        let cache = tree.cache(catalog.clone());
        assert!(cache.refresh());

        let manifest: Value =
            serde_json::from_slice(&fs::read(tree.cache_dir().join("kiro.json")).unwrap()).unwrap();
        assert_eq!(manifest["source"], source.to_string_lossy().as_ref());
        assert_eq!(manifest["size"], png("kiro").len());
        assert!(manifest["modified_ns"].as_u64().unwrap() > 0);
        assert_eq!(manifest["kind"], "png");
        assert_eq!(manifest["sha256"], sha256_hex(&png("kiro")));

        let reopened = tree.cache(catalog.clone());
        assert_eq!(
            reopened.version("kiro"),
            cache.version("kiro"),
            "a copy the manifest vouches for is offered before any look"
        );

        write(&tree.cache_dir().join("kiro.png"), &png("tampered"));
        assert_eq!(
            tree.cache(catalog).version("kiro"),
            None,
            "a copy whose bytes the manifest does not vouch for is not offered"
        );
    }

    #[test]
    fn a_cached_windows_icon_is_served_as_an_icon_file() {
        let tree = Tree::new();
        let bytes = b"\0\0\x01\0icon".to_vec();
        write(&tree.cache_dir().join("antigravity.ico"), &bytes);
        let manifest = Manifest {
            source: tree.scratch.path().join("Antigravity.exe"),
            size: 1,
            modified_ns: 1,
            kind: IconKind::Ico,
            sha256: sha256_hex(&bytes),
        };
        write(
            &tree.cache_dir().join("antigravity.json"),
            &serde_json::to_vec(&manifest).unwrap(),
        );
        let cache = tree.cache(vec![agent("antigravity", Vec::new(), None)]);
        let icon = cache.icon("antigravity").unwrap();
        assert_eq!(icon.kind.media_type(), "image/x-icon");
        assert_eq!(icon.bytes, bytes);
        assert_eq!(IconKind::Png.media_type(), "image/png");
    }

    #[test]
    fn linux_finds_the_desktop_entrys_png_in_the_theme_and_never_an_svg() {
        let tree = Tree::new();
        let own = tree.scratch.path().join("home-share");
        let system = tree.data();
        let dirs = vec![own.clone(), system.clone()];

        desktop_entry(&own, "dev.zed.Zed", "zed");
        theme_png(&system, 512, "zed", &png("512"));
        theme_png(&system, 64, "zed", &png("64"));
        assert_eq!(
            fs::read(theme_icon("zed", &dirs).unwrap()).unwrap(),
            png("64"),
            "a preferred size before a larger one"
        );

        write(&system.join("pixmaps/code.png"), &png("pixmap"));
        theme_png(&system, 256, "code", &png("256"));
        assert_eq!(
            fs::read(theme_icon("code", &dirs).unwrap()).unwrap(),
            png("pixmap"),
            "the unthemed icon before a size the window does not prefer"
        );
        theme_png(&own, 256, "only-large", &png("large"));
        assert!(theme_icon("only-large", &dirs).is_some());

        let absolute = tree.scratch.path().join("snap/meta/gui/icon.png");
        write(&absolute, &png("absolute"));
        assert_eq!(
            theme_icon(&absolute.to_string_lossy(), &dirs),
            Some(absolute)
        );
        let svg = tree.scratch.path().join("icon.svg");
        write(&svg, b"<svg><script/></svg>");
        assert_eq!(theme_icon(&svg.to_string_lossy(), &dirs), None);
        write(
            &system.join("icons/hicolor/scalable/apps/vector.svg"),
            b"<svg/>",
        );
        assert_eq!(theme_icon("vector", &dirs), None);
        assert_eq!(theme_icon("relative/name", &dirs), None);

        assert_eq!(
            desktop_icon_name("[Desktop Entry]\nName=X\nIcon[fr]=localized\nIcon = zed \n"),
            Some("zed")
        );
        assert_eq!(
            desktop_icon_name("[Desktop Action new]\nIcon=action\n[Desktop Entry]\nName=X\n"),
            None,
            "only the Desktop Entry group names the app's icon"
        );

        let cache = IconCache::with_catalog(
            tree.cache_dir(),
            IconPlaces::linux(dirs),
            vec![agent(
                "zed",
                vec![IconSource::LinuxDesktop(&["zed", "dev.zed.Zed"])],
                None,
            )],
        );
        assert!(cache.refresh());
        assert_eq!(cache.icon("zed").unwrap().bytes, png("64"));
    }

    #[test]
    fn a_file_that_is_not_a_png_is_skipped_for_the_next_source() {
        let tree = Tree::new();
        desktop_entry(&tree.data(), "fake", "fake");
        theme_png(&tree.data(), 64, "fake", b"<svg onload='x'/>");
        desktop_entry(&tree.data(), "real", "real");
        theme_png(&tree.data(), 64, "real", &png("real"));
        let cache = tree.cache(vec![agent(
            "agent",
            vec![
                IconSource::LinuxDesktop(&["fake"]),
                IconSource::LinuxDesktop(&["real"]),
            ],
            None,
        )]);
        assert!(cache.refresh());
        assert_eq!(cache.icon("agent").unwrap().bytes, png("real"));
    }

    #[test]
    fn linux_data_directories_follow_xdg_then_flatpak_and_snap() {
        let spelled = |dirs: Vec<PathBuf>| -> Vec<String> {
            dirs.iter()
                .map(|dir| dir.to_string_lossy().replace('\\', "/"))
                .collect()
        };
        let home = Path::new("/home/pat");
        assert_eq!(
            spelled(linux_data_dirs(home, None, None)),
            [
                "/home/pat/.local/share",
                "/usr/local/share",
                "/usr/share",
                "/home/pat/.local/share/flatpak/exports/share",
                "/var/lib/flatpak/exports/share",
                "/var/lib/snapd/desktop",
            ]
        );
        assert_eq!(
            spelled(linux_data_dirs(
                home,
                Some("/data/me".into()),
                Some("/opt/share::/usr/share:/data/me".into()),
            )),
            [
                "/data/me",
                "/opt/share",
                "/usr/share",
                "/data/me/flatpak/exports/share",
                "/var/lib/flatpak/exports/share",
                "/var/lib/snapd/desktop",
            ]
        );
    }

    #[test]
    fn a_launcher_on_path_names_the_program_it_starts_and_an_npm_shim_names_none() {
        let scratch = Scratch::new();
        let root = scratch.path();
        let local = root.join("Local");
        let program_files = root.join("Program Files");
        let vscode_bin = root.join("VSCode").join("bin");
        let npm = root.join("npm");
        let tools = root.join("tools");
        write(&root.join("VSCode/Code.exe"), b"MZ");
        write(
            &vscode_bin.join("code.cmd"),
            b"@echo off\r\nsetlocal\r\nset VSCODE_DEV=\r\n\"%~dp0..\\Code.exe\" \"%~dp0..\\resources\\app\\out\\cli.js\" %*\r\n",
        );
        write(&npm.join("node.exe"), b"MZ");
        write(
            &npm.join("claude.cmd"),
            b"@ECHO off\r\nSET dp0=%~dp0\r\nIF EXIST \"%dp0%\\node.exe\" (\r\n  SET \"_prog=%dp0%\\node.exe\"\r\n)\r\n\"%_prog%\" \"%dp0%\\node_modules\\@anthropic-ai\\claude-code\\cli.js\" %*\r\n",
        );
        write(
            &npm.join("claude.ps1"),
            b"$basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent\n& \"$basedir/node$exe\" \"$basedir/node_modules/@anthropic-ai/claude-code/cli.js\" $args\n",
        );
        write(&tools.join("zed.exe"), b"MZ");
        write(
            &tools.join("helper.ps1"),
            b"& \"$PSScriptRoot\\app\\Helper.exe\" $args\n",
        );
        write(&tools.join("app/Helper.exe"), b"MZ");
        write(&local.join("Programs/Antigravity/Antigravity.exe"), b"MZ");
        let places = IconPlaces::windows(
            local.clone(),
            program_files,
            vec![npm.clone(), vscode_bin, tools.clone()],
        );

        assert_eq!(
            program(&places, ProgramPlace::OnPath("code")),
            Some(root.join("VSCode").join("Code.exe"))
        );
        assert_eq!(program(&places, ProgramPlace::OnPath("claude")), None);
        assert_eq!(
            program(&places, ProgramPlace::OnPath("zed")),
            Some(tools.join("zed.exe"))
        );
        assert_eq!(
            program(&places, ProgramPlace::OnPath("helper")),
            Some(tools.join("app").join("Helper.exe"))
        );
        assert_eq!(program(&places, ProgramPlace::OnPath("absent")), None);
        assert_eq!(
            program(
                &places,
                ProgramPlace::Below(
                    KnownFolder::LocalAppData,
                    "Programs/Antigravity/Antigravity.exe"
                )
            ),
            Some(
                local
                    .join("Programs")
                    .join("Antigravity")
                    .join("Antigravity.exe")
            )
        );
        assert_eq!(
            program(
                &places,
                ProgramPlace::Below(KnownFolder::ProgramFiles, "Microsoft VS Code/Code.exe")
            ),
            None
        );
    }

    #[test]
    fn sources_for_the_other_system_are_not_looked_at() {
        let tree = Tree::new();
        desktop_entry(&tree.data(), "code", "code");
        theme_png(&tree.data(), 64, "code", &png("code"));
        let mut places = IconPlaces::windows(
            tree.scratch.path().into(),
            tree.scratch.path().into(),
            Vec::new(),
        );
        places.data_dirs = vec![tree.data()];
        let cache = IconCache::with_catalog(
            tree.cache_dir(),
            places,
            vec![agent(
                "vscode",
                vec![IconSource::LinuxDesktop(&["code"])],
                None,
            )],
        );
        assert!(!cache.refresh());
        assert_eq!(cache.version("vscode"), None);
    }

    #[test]
    fn a_background_look_says_when_an_icon_changed() {
        let tree = Tree::new();
        desktop_entry(&tree.data(), "cursor", "cursor");
        theme_png(&tree.data(), 64, "cursor", &png("cursor"));
        let cache = tree.cache(vec![agent(
            "cursor",
            vec![IconSource::LinuxDesktop(&["cursor"])],
            None,
        )]);
        let (sender, received) = mpsc::channel();
        cache.refresh_in_background(move || sender.send(()).unwrap());
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("the look says the icon is new");
        assert!(cache.version("cursor").is_some());
    }
}
