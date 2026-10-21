//! The per-user `ghostlight` command entry (ADR-0126 Decision 8).
//!
//! The installation puts a fixed sibling set under a product-owned directory, which no shell looks
//! in, so `ghostlight doctor` did not work in a fresh terminal unless the person had
//! installed the npm package globally.
//!
//! On Unix this owns one symlink at `~/.local/bin/ghostlight`, the de facto user executable
//! directory that modern distributions already place on PATH. Shell startup files are never
//! edited: rewriting a person's `.bashrc` to fix PATH is exactly the kind of uninvited change this
//! product does not make. When the directory is not on the PATH the install runs with, the install
//! result carries the link's full path ([`CommandPathActionResult::off_path`]) and the summary names
//! it, so the terminal route works either way.
//!
//! Ownership is conservative. A symlink whose target is named `ghostlight` is ours to update or
//! remove. Anything else -- a regular file, a directory, a symlink pointing somewhere else -- is
//! someone else's and is left exactly as it is.
//!
//! On Windows the command is the installed `bin` directory on the per-user `Path` (see
//! [`super::user_path`]). Install appends that one entry, uninstall removes only it, and each
//! change is announced so terminals opened afterwards see it. Inside an app container a registry
//! write would land in that app's private view, so the entry is read and never written: an entry
//! already there is [`CommandPathState::Current`], and a missing one is
//! [`CommandPathState::PrivateView`], to be added from a regular terminal.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use thiserror::Error;

use super::container::{AppContainer, Residence};
use super::user_path::{self, PathValue, UserPathStore};
use crate::language::deployment as words;

/// The command name a person types.
const COMMAND_NAME: &str = "ghostlight";

/// What an install calls the command's `Path` entry when an app container kept it private.
pub const PATH_ENTRY_REGISTRATION: &str = "PATH entry";

/// The variable whose value the home-relative form of a `Path` entry names.
const PROFILE_VARIABLE: &str = super::user_dirs::USER_PROFILE_VARIABLE;

/// The subcommand of the installed edge that puts the command on the `Path`.
const FIXING_SUBCOMMAND: &str = "install";

/// Current state of the per-user command entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandPathState {
    /// This platform or installation does not use a per-user command entry.
    NotApplicable,
    /// No Ghostlight-owned entry exists.
    Missing,
    /// The entry points at this exact installed executable.
    Current,
    /// A Ghostlight-owned entry points at an older installed executable.
    Updatable,
    /// Something not owned by Ghostlight occupies the location.
    NeedsAttention,
    /// The entry is missing and this process runs in an app container, which keeps a change made
    /// here private to that app.
    PrivateView,
}

/// Read-only result for the per-user command entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandPathReport {
    /// Current ownership and freshness state.
    pub state: CommandPathState,
    /// Product-authored explanation of the state.
    pub detail: String,
    /// Exact entry location: the symlink on Unix, the directory on the `Path` on Windows.
    pub link: PathBuf,
    /// The executable a person runs, whether or not the entry exists.
    pub executable: PathBuf,
}

/// Result of a command-entry install or uninstall request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandPathActionResult {
    /// Whether an owned link or value changed.
    pub changed: bool,
    /// Whether a terminal that is already open still lacks the change. A new terminal has it.
    pub needs_new_terminal: bool,
    /// The link's full path after an install, when its directory is not on the `PATH` this process
    /// runs with: typing `ghostlight` in that terminal finds nothing, and the full path works.
    /// Only the Unix link is judged; the Windows `Path` entry says so with
    /// [`Self::needs_new_terminal`].
    pub off_path: Option<PathBuf>,
    /// State observed after the action.
    pub report: CommandPathReport,
}

/// The command entry an uninstall removes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnedEntry {
    /// The installed `bin` directory on the per-user `Path`.
    UserPath(PathBuf),
    /// The `ghostlight` symlink in the user executable directory.
    Link(PathBuf),
}

/// Product-owned per-user command entry.
#[derive(Clone, Debug)]
pub struct CommandPath {
    context: CommandPathContext,
}

impl CommandPath {
    /// Discover the current executable and where this platform keeps the command entry.
    #[must_use]
    pub fn discover() -> Self {
        Self::in_residence(&Residence::detect())
    }

    /// The same, for a process that runs in `residence`.
    #[must_use]
    pub fn in_residence(residence: &Residence) -> Self {
        Self {
            context: CommandPathContext::system(residence),
        }
    }

    /// Inspect the entry without changing it.
    pub fn check(&self) -> Result<CommandPathReport, CommandPathError> {
        inspect(&self.context)
    }

    /// Create or update the owned entry.
    pub fn install(&self) -> Result<CommandPathActionResult, CommandPathError> {
        apply_install(&self.context)
    }

    /// Remove the owned entry, and only that.
    pub fn uninstall(&self) -> Result<CommandPathActionResult, CommandPathError> {
        apply_uninstall(&self.context)
    }

    /// The entry [`Self::uninstall`] would remove, or `None` when it would remove nothing.
    /// Nothing is changed.
    pub fn removable(&self) -> Result<Option<OwnedEntry>, CommandPathError> {
        removable(&self.context)
    }
}

#[derive(Clone, Debug)]
struct CommandPathContext {
    unix: bool,
    executable: PathBuf,
    bin_home: PathBuf,
    /// Where the per-user `Path` is stored, on a platform that keeps the command there.
    user_path: Option<Arc<dyn UserPathStore>>,
    /// The user profile directory the home-relative `Path` entry names.
    profile: Option<PathBuf>,
    /// The app container this process runs in, when there is one.
    container: Option<AppContainer>,
    /// The directories this process finds commands in, from its `PATH`.
    search_path: Vec<PathBuf>,
}

impl CommandPathContext {
    fn system(residence: &Residence) -> Self {
        let dirs = super::user_dirs::UserDirs::system();
        Self {
            unix: cfg!(unix),
            executable: super::layout::Layout::current()
                .map(|layout| layout.edge())
                .unwrap_or_default(),
            bin_home: dirs.home().join(".local/bin"),
            user_path: system_user_path(),
            profile: super::user_dirs::directory(PROFILE_VARIABLE),
            container: residence.container().cloned(),
            search_path: dirs.path_entries().to_vec(),
        }
    }

    fn link(&self) -> PathBuf {
        self.bin_home.join(COMMAND_NAME)
    }

    /// A system package already provides the command; the per-user entry would shadow it.
    fn system_package(&self) -> bool {
        self.executable.parent() == Some(Path::new("/usr/bin"))
    }

    fn applicable(&self) -> bool {
        self.unix && !self.system_package()
    }

    /// The directory the `Path` entry names: the one holding the edge.
    fn bin_directory(&self) -> Option<&Path> {
        self.executable
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
    }

    /// The value of environment variable `name`, with the profile directory this context was built
    /// with answering for `USERPROFILE`.
    fn variable(&self, name: &str) -> Option<String> {
        if name.eq_ignore_ascii_case(PROFILE_VARIABLE) {
            return self
                .profile
                .as_ref()
                .map(|profile| profile.to_string_lossy().into_owned());
        }
        env::var(name).ok()
    }
}

/// The per-user `Path` in the registry on Windows. Under a test home there is none, so a journey
/// never changes the `Path` of the person running it.
fn system_user_path() -> Option<Arc<dyn UserPathStore>> {
    #[cfg(windows)]
    {
        use ghostlight_relay::home::TEST_HOME_VARIABLE;

        env::var_os(TEST_HOME_VARIABLE)
            .is_none()
            .then(|| Arc::new(user_path::Registry) as Arc<dyn UserPathStore>)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn inspect(context: &CommandPathContext) -> Result<CommandPathReport, CommandPathError> {
    if let Some(store) = &context.user_path {
        return inspect_user_path(context, store.as_ref());
    }
    let link = context.link();
    let executable = context.executable.clone();
    if !context.applicable() {
        return Ok(CommandPathReport {
            state: CommandPathState::NotApplicable,
            detail: not_applicable_detail(context),
            link,
            executable,
        });
    }
    let state = match ownership(&link)? {
        Ownership::Absent => CommandPathState::Missing,
        Ownership::Foreign => CommandPathState::NeedsAttention,
        Ownership::Ours(target) if target == executable => CommandPathState::Current,
        Ownership::Ours(_) => CommandPathState::Updatable,
    };
    Ok(CommandPathReport {
        state,
        detail: detail_for(state, &link, &executable),
        link,
        executable,
    })
}

/// The state of the `Path` entry on a platform that keeps the command there.
fn inspect_user_path(
    context: &CommandPathContext,
    store: &dyn UserPathStore,
) -> Result<CommandPathReport, CommandPathError> {
    let executable = context.executable.clone();
    let Some(directory) = context.bin_directory() else {
        return Ok(CommandPathReport {
            state: CommandPathState::NotApplicable,
            detail: not_applicable_detail(context),
            link: PathBuf::new(),
            executable,
        });
    };
    let (state, detail) = if on_user_path(context, store, directory)? {
        (
            CommandPathState::Current,
            format!("{} is on your PATH.", directory.display()),
        )
    } else if let Some(container) = &context.container {
        (
            CommandPathState::PrivateView,
            format!(
                "{} is not on your PATH. Windows keeps a change made from {} private to that app. To add it, run this from a regular terminal: {} {FIXING_SUBCOMMAND}",
                directory.display(),
                container.app,
                executable.display()
            ),
        )
    } else {
        (
            CommandPathState::Missing,
            format!(
                "{} is not on your PATH. To add it, run: {} {FIXING_SUBCOMMAND}",
                directory.display(),
                executable.display()
            ),
        )
    };
    Ok(CommandPathReport {
        state,
        detail,
        link: directory.to_path_buf(),
        executable,
    })
}

/// Whether some entry of the stored `Path` names `directory`.
fn on_user_path(
    context: &CommandPathContext,
    store: &dyn UserPathStore,
    directory: &Path,
) -> Result<bool, CommandPathError> {
    Ok(read_user_path(store)?.is_some_and(|value| {
        user_path::names_directory(&value, directory, &|name| context.variable(name))
    }))
}

fn read_user_path(store: &dyn UserPathStore) -> Result<Option<PathValue>, CommandPathError> {
    store.read().map_err(|source| CommandPathError::Read {
        path: PathBuf::from(user_path::LOCATION),
        source,
    })
}

/// Store `value` and tell running programs. The announcement cannot fail the install: the value is
/// already written.
fn write_user_path(store: &dyn UserPathStore, value: &PathValue) -> Result<(), CommandPathError> {
    store
        .write(value)
        .map_err(|source| CommandPathError::Write {
            path: PathBuf::from(user_path::LOCATION),
            source,
        })?;
    store.announce();
    Ok(())
}

/// Append `directory` to the per-user `Path`.
fn install_user_path(
    context: &CommandPathContext,
    store: &dyn UserPathStore,
    directory: &Path,
) -> Result<(), CommandPathError> {
    let value = read_user_path(store)?.unwrap_or_else(PathValue::empty);
    let entry = user_path::entry_for(directory, value.expandable, context.profile.as_deref());
    write_user_path(
        store,
        &PathValue {
            text: user_path::append(&value.text, &entry),
            expandable: value.expandable,
        },
    )
}

/// Remove the entries that name `directory` from the per-user `Path`.
fn uninstall_user_path(
    context: &CommandPathContext,
    store: &dyn UserPathStore,
    directory: &Path,
) -> Result<(), CommandPathError> {
    let remaining = read_user_path(store)?
        .and_then(|value| user_path::remove(&value, directory, &|name| context.variable(name)));
    match remaining {
        Some(remaining) => write_user_path(store, &remaining),
        None => Ok(()),
    }
}

fn unchanged(report: CommandPathReport) -> CommandPathActionResult {
    CommandPathActionResult {
        changed: false,
        needs_new_terminal: false,
        off_path: None,
        report,
    }
}

/// The full path of the link `report` describes, when it is in place and its directory is not on
/// the `PATH` this process runs with.
fn off_path(context: &CommandPathContext, report: &CommandPathReport) -> Option<PathBuf> {
    if context.user_path.is_some() || report.state != CommandPathState::Current {
        return None;
    }
    let directory = report.link.parent()?;
    (!on_search_path(&context.search_path, directory)).then(|| report.link.clone())
}

/// Whether some entry of `search_path` names `directory`, as written or once both are resolved.
fn on_search_path(search_path: &[PathBuf], directory: &Path) -> bool {
    let resolved = fs::canonicalize(directory).ok();
    search_path.iter().any(|entry| {
        entry == directory
            || resolved
                .as_ref()
                .is_some_and(|resolved| fs::canonicalize(entry).ok().as_ref() == Some(resolved))
    })
}

fn apply_install(
    context: &CommandPathContext,
) -> Result<CommandPathActionResult, CommandPathError> {
    let mut result = install_entry(context)?;
    result.off_path = off_path(context, &result.report);
    Ok(result)
}

/// Create or update the entry, without judging the `PATH` it is found by.
fn install_entry(
    context: &CommandPathContext,
) -> Result<CommandPathActionResult, CommandPathError> {
    let before = inspect(context)?;
    if matches!(
        before.state,
        CommandPathState::NotApplicable
            | CommandPathState::NeedsAttention
            | CommandPathState::Current
            | CommandPathState::PrivateView
    ) {
        return Ok(unchanged(before));
    }
    if let (Some(store), Some(directory)) = (&context.user_path, context.bin_directory()) {
        install_user_path(context, store.as_ref(), directory)?;
        return Ok(CommandPathActionResult {
            changed: true,
            needs_new_terminal: true,
            off_path: None,
            report: inspect(context)?,
        });
    }
    let link = context.link();
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent).map_err(|source| CommandPathError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    // Only reached for Missing or an owned Updatable entry, so this removes nothing foreign.
    if matches!(before.state, CommandPathState::Updatable) {
        remove(&link)?;
    }
    create_symlink(&context.executable, &link)?;
    Ok(CommandPathActionResult {
        changed: true,
        needs_new_terminal: false,
        off_path: None,
        report: inspect(context)?,
    })
}

/// Whether an uninstall removes the entry `report` describes: one Ghostlight owns, and never from
/// inside an app container, whose write no other app would see.
fn removes(context: &CommandPathContext, report: &CommandPathReport) -> bool {
    matches!(
        report.state,
        CommandPathState::Current | CommandPathState::Updatable
    ) && context.container.is_none()
}

fn removable(context: &CommandPathContext) -> Result<Option<OwnedEntry>, CommandPathError> {
    let report = inspect(context)?;
    if !removes(context, &report) {
        return Ok(None);
    }
    Ok(Some(if context.user_path.is_some() {
        OwnedEntry::UserPath(report.link)
    } else {
        OwnedEntry::Link(report.link)
    }))
}

fn apply_uninstall(
    context: &CommandPathContext,
) -> Result<CommandPathActionResult, CommandPathError> {
    let before = inspect(context)?;
    if !removes(context, &before) {
        return Ok(unchanged(before));
    }
    if let (Some(store), Some(directory)) = (&context.user_path, context.bin_directory()) {
        uninstall_user_path(context, store.as_ref(), directory)?;
    } else {
        remove(&context.link())?;
    }
    Ok(CommandPathActionResult {
        changed: true,
        needs_new_terminal: false,
        off_path: None,
        report: inspect(context)?,
    })
}

enum Ownership {
    Absent,
    Foreign,
    Ours(PathBuf),
}

/// Decide whether Ghostlight may touch what is at `link`.
///
/// Only a symlink whose target is named `ghostlight` counts as ours. A regular file at that path
/// belongs to whoever put it there.
fn ownership(link: &Path) -> Result<Ownership, CommandPathError> {
    let metadata = match fs::symlink_metadata(link) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Ownership::Absent),
        Err(source) => {
            return Err(CommandPathError::Read {
                path: link.to_path_buf(),
                source,
            })
        }
    };
    if !metadata.file_type().is_symlink() {
        return Ok(Ownership::Foreign);
    }
    let target = fs::read_link(link).map_err(|source| CommandPathError::Read {
        path: link.to_path_buf(),
        source,
    })?;
    let named_ghostlight = target
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == COMMAND_NAME);
    if named_ghostlight {
        Ok(Ownership::Ours(target))
    } else {
        Ok(Ownership::Foreign)
    }
}

fn remove(link: &Path) -> Result<(), CommandPathError> {
    fs::remove_file(link).map_err(|source| CommandPathError::Write {
        path: link.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> Result<(), CommandPathError> {
    std::os::unix::fs::symlink(target, link).map_err(|source| CommandPathError::Write {
        path: link.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn create_symlink(_target: &Path, link: &Path) -> Result<(), CommandPathError> {
    Err(CommandPathError::Write {
        path: link.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::Unsupported,
            "the per-user command entry is a Unix symlink",
        ),
    })
}

fn not_applicable_detail(context: &CommandPathContext) -> String {
    words::command_path_not_applicable(context.system_package()).to_owned()
}

fn detail_for(state: CommandPathState, link: &Path, executable: &Path) -> String {
    words::command_path_detail(state, link, executable)
}

/// Failures while inspecting or changing the per-user command entry.
#[derive(Debug, Error)]
pub enum CommandPathError {
    /// A product-owned location could not be read.
    #[error("{}", words::location_unreadable(.path, .source))]
    Read {
        /// The location involved.
        path: PathBuf,
        /// The underlying failure.
        source: io::Error,
    },
    /// A product-owned location could not be written.
    #[error("{}", words::location_unwritable(.path, .source))]
    Write {
        /// The location involved.
        path: PathBuf,
        /// The underlying failure.
        source: io::Error,
    },
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    mod user_path_route {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Mutex;

        use super::*;

        /// The per-user `Path` held in memory, counting how often it is written and announced.
        #[derive(Debug, Default)]
        struct FakeUserPath {
            value: Mutex<Option<PathValue>>,
            writes: AtomicUsize,
            announcements: AtomicUsize,
        }

        impl FakeUserPath {
            fn holding(text: &str, expandable: bool) -> Arc<Self> {
                Arc::new(Self {
                    value: Mutex::new(Some(PathValue {
                        text: text.to_owned(),
                        expandable,
                    })),
                    ..Self::default()
                })
            }

            fn text(&self) -> Option<String> {
                self.value
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|value| value.text.clone())
            }

            fn expandable(&self) -> Option<bool> {
                self.value
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|value| value.expandable)
            }
        }

        impl UserPathStore for FakeUserPath {
            fn read(&self) -> io::Result<Option<PathValue>> {
                Ok(self.value.lock().unwrap().clone())
            }

            fn write(&self, value: &PathValue) -> io::Result<()> {
                *self.value.lock().unwrap() = Some(value.clone());
                self.writes.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn announce(&self) {
                self.announcements.fetch_add(1, Ordering::SeqCst);
            }
        }

        const PROFILE: &str = r"C:\Users\person";

        fn windows(store: &Arc<FakeUserPath>) -> CommandPathContext {
            CommandPathContext {
                unix: false,
                executable: PathBuf::from(r"C:\Users\person\.ghostlight\bin\ghostlight.exe"),
                bin_home: PathBuf::new(),
                user_path: Some(store.clone()),
                profile: Some(PathBuf::from(PROFILE)),
                container: None,
                search_path: Vec::new(),
            }
        }

        fn in_container(store: &Arc<FakeUserPath>) -> CommandPathContext {
            let mut context = windows(store);
            context.container = Some(AppContainer {
                app: "Claude".to_owned(),
            });
            context
        }

        /// Ghostlight puts its `bin` on the per-user `Path` on Windows: once, announced to running programs,
        /// keeping a literal value literal and making an expandable one when there is none, recognizing an
        /// entry written another way, and on uninstall removing only that entry, so an install and an
        /// uninstall leave the value byte for byte as it was. Inside an app container nothing is written,
        /// since the change would reach only the app's private view.
        #[test]
        fn the_path_entry_round_trips_and_a_container_writes_nothing() {
            {
                let store = FakeUserPath::holding(r"C:\Windows;C:\Tools;", true);
                let context = windows(&store);
                assert_eq!(inspect(&context).unwrap().state, CommandPathState::Missing);

                let first = apply_install(&context).unwrap();
                assert!(first.changed);
                assert!(first.needs_new_terminal);
                assert_eq!(first.report.state, CommandPathState::Current);
                assert_eq!(
                    store.text().unwrap(),
                    r"C:\Windows;C:\Tools;%USERPROFILE%\.ghostlight\bin;"
                );
                assert_eq!(store.expandable(), Some(true));
                assert_eq!(store.announcements.load(Ordering::SeqCst), 1);

                let second = apply_install(&context).unwrap();
                assert!(!second.changed);
                assert!(!second.needs_new_terminal);
                assert_eq!(store.writes.load(Ordering::SeqCst), 1);
                assert_eq!(store.announcements.load(Ordering::SeqCst), 1);
            }
            {
                let store = FakeUserPath::holding(r"C:\Windows", false);
                apply_install(&windows(&store)).unwrap();
                assert_eq!(
                    store.text().unwrap(),
                    r"C:\Windows;C:\Users\person\.ghostlight\bin"
                );
                assert_eq!(store.expandable(), Some(false));
            }
            {
                let store = Arc::new(FakeUserPath::default());
                let context = windows(&store);
                assert_eq!(inspect(&context).unwrap().state, CommandPathState::Missing);
                apply_install(&context).unwrap();
                assert_eq!(store.text().unwrap(), r"%USERPROFILE%\.ghostlight\bin");
                assert_eq!(store.expandable(), Some(true));
            }
            {
                for entry in [
                    r"%USERPROFILE%\.ghostlight\bin",
                    r"c:\users\PERSON\.ghostlight\bin\",
                ] {
                    let store = FakeUserPath::holding(&format!(r"C:\Windows;{entry}"), true);
                    let context = windows(&store);
                    assert_eq!(inspect(&context).unwrap().state, CommandPathState::Current);
                    assert!(!apply_install(&context).unwrap().changed);
                    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
                }
            }
            {
                let store = FakeUserPath::holding(
                    r"C:\Windows;%USERPROFILE%\.ghostlight\bin;C:\Users\person\.ghostlight\bin\old;;C:\Tools",
                    true,
                );
                let context = windows(&store);
                assert_eq!(
                    removable(&context).unwrap(),
                    Some(OwnedEntry::UserPath(PathBuf::from(
                        r"C:\Users\person\.ghostlight\bin"
                    )))
                );
                let removed = apply_uninstall(&context).unwrap();
                assert!(removed.changed);
                assert!(!removed.needs_new_terminal);
                assert_eq!(removed.report.state, CommandPathState::Missing);
                assert_eq!(
                    store.text().unwrap(),
                    r"C:\Windows;C:\Users\person\.ghostlight\bin\old;;C:\Tools"
                );
                assert_eq!(store.announcements.load(Ordering::SeqCst), 1);

                assert!(!apply_uninstall(&context).unwrap().changed);
                assert_eq!(store.writes.load(Ordering::SeqCst), 1);
            }
            {
                let original = r"C:\Windows;;C:\Tools\;%SystemRoot%\x;";
                let store = FakeUserPath::holding(original, true);
                let context = windows(&store);
                apply_install(&context).unwrap();
                apply_uninstall(&context).unwrap();
                assert_eq!(store.text().as_deref(), Some(original));
            }
            {
                let store =
                    FakeUserPath::holding(r"C:\Windows;%USERPROFILE%\.ghostlight\bin", true);
                let context = in_container(&store);

                assert_eq!(inspect(&context).unwrap().state, CommandPathState::Current);
                let installed = apply_install(&context).unwrap();
                assert!(!installed.changed);
                assert_eq!(installed.report.state, CommandPathState::Current);
                assert_eq!(removable(&context).unwrap(), None);
                let removed = apply_uninstall(&context).unwrap();
                assert!(!removed.changed);
                assert_eq!(store.writes.load(Ordering::SeqCst), 0);
                assert_eq!(store.announcements.load(Ordering::SeqCst), 0);
                assert_eq!(
                    store.text().as_deref(),
                    Some(r"C:\Windows;%USERPROFILE%\.ghostlight\bin")
                );
            }
            {
                let store = FakeUserPath::holding(r"C:\Windows", true);
                let context = in_container(&store);

                let report = inspect(&context).unwrap();
                assert_eq!(report.state, CommandPathState::PrivateView);
                assert!(report.detail.contains("Claude"));
                assert!(!report.detail.contains("  "));
                let installed = apply_install(&context).unwrap();
                assert!(!installed.changed);
                assert_eq!(installed.report.state, CommandPathState::PrivateView);
                assert!(!apply_uninstall(&context).unwrap().changed);
                assert_eq!(store.writes.load(Ordering::SeqCst), 0);
                assert_eq!(store.announcements.load(Ordering::SeqCst), 0);
                assert_eq!(store.text().as_deref(), Some(r"C:\Windows"));
            }
        }
    }
}
