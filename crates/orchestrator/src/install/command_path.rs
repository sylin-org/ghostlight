//! The per-user `ghostlight` command entry (ADR-0126 Decision 8).
//!
//! The Debian package installs `/usr/bin/ghostlight`, so a package user already has the command.
//! The per-user route installs a versioned sibling set under a product-owned directory, which no
//! shell looks in, so `ghostlight doctor` did not work in a fresh terminal unless the person had
//! installed the npm package globally.
//!
//! This owns one symlink at `~/.local/bin/ghostlight`, the de facto user executable directory that
//! modern distributions already place on PATH. Shell startup files are never edited: rewriting a
//! person's `.bashrc` to fix PATH is exactly the kind of uninvited change this product does not
//! make. When the directory is not on PATH, the install output still names the absolute path, so
//! the terminal route works either way.
//!
//! Ownership is conservative. A symlink whose target is named `ghostlight` is ours to update or
//! remove. Anything else -- a regular file, a directory, a symlink pointing somewhere else -- is
//! someone else's and is left exactly as it is.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

/// The command name a person types.
const COMMAND_NAME: &str = "ghostlight";

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
}

impl CommandPathState {
    /// The plain words a person reads for this state.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotApplicable => "not used on this installation",
            Self::Missing => "not installed",
            Self::Current => "installed",
            Self::Updatable => "installed, needs an update",
            Self::NeedsAttention => "needs attention",
        }
    }
}

/// Read-only result for the per-user command entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandPathReport {
    /// Current ownership and freshness state.
    pub state: CommandPathState,
    /// Product-authored explanation of the state.
    pub detail: String,
    /// Exact entry location.
    pub link: PathBuf,
    /// The executable a person runs, whether or not the entry exists.
    pub executable: PathBuf,
}

/// Result of a command-entry install or uninstall request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandPathActionResult {
    /// Whether an owned file changed.
    pub changed: bool,
    /// State observed after the action.
    pub report: CommandPathReport,
}

/// Product-owned per-user command entry.
#[derive(Clone, Debug)]
pub struct CommandPath {
    context: CommandPathContext,
}

impl CommandPath {
    /// Discover the current executable and the user executable directory.
    #[must_use]
    pub fn discover() -> Self {
        Self {
            context: CommandPathContext::system(),
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
}

#[derive(Clone, Debug)]
struct CommandPathContext {
    unix: bool,
    executable: PathBuf,
    bin_home: PathBuf,
}

impl CommandPathContext {
    fn system() -> Self {
        let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        Self {
            unix: cfg!(unix),
            executable: env::current_exe().unwrap_or_default(),
            bin_home: home.join(".local/bin"),
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
}

fn inspect(context: &CommandPathContext) -> Result<CommandPathReport, CommandPathError> {
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

fn apply_install(
    context: &CommandPathContext,
) -> Result<CommandPathActionResult, CommandPathError> {
    let before = inspect(context)?;
    if matches!(
        before.state,
        CommandPathState::NotApplicable
            | CommandPathState::NeedsAttention
            | CommandPathState::Current
    ) {
        return Ok(CommandPathActionResult {
            changed: false,
            report: before,
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
        report: inspect(context)?,
    })
}

fn apply_uninstall(
    context: &CommandPathContext,
) -> Result<CommandPathActionResult, CommandPathError> {
    let before = inspect(context)?;
    if !matches!(
        before.state,
        CommandPathState::Current | CommandPathState::Updatable
    ) {
        return Ok(CommandPathActionResult {
            changed: false,
            report: before,
        });
    }
    remove(&context.link())?;
    Ok(CommandPathActionResult {
        changed: true,
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
    if context.system_package() {
        "The system package already provides the ghostlight command.".to_owned()
    } else {
        "This platform does not use a per-user command entry.".to_owned()
    }
}

fn detail_for(state: CommandPathState, link: &Path, executable: &Path) -> String {
    match state {
        CommandPathState::NotApplicable => String::new(),
        CommandPathState::Missing => format!(
            "No ghostlight command entry exists. Run it directly at {}.",
            executable.display()
        ),
        CommandPathState::Current => {
            format!("The ghostlight command runs from {}.", link.display())
        }
        CommandPathState::Updatable => {
            "The ghostlight command entry points at an older installed version.".to_owned()
        }
        CommandPathState::NeedsAttention => format!(
            "Something not owned by Ghostlight occupies {}; it was left untouched.",
            link.display()
        ),
    }
}

/// Failures while inspecting or changing the per-user command entry.
#[derive(Debug, Error)]
pub enum CommandPathError {
    /// A product-owned location could not be read.
    #[error("could not read {path}: {source}")]
    Read {
        /// The location involved.
        path: PathBuf,
        /// The underlying failure.
        source: io::Error,
    },
    /// A product-owned location could not be written.
    #[error("could not write {path}: {source}")]
    Write {
        /// The location involved.
        path: PathBuf,
        /// The underlying failure.
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    use uuid::Uuid;

    fn context(name: &str) -> CommandPathContext {
        let root = std::env::temp_dir().join(format!(
            "ghostlight-command-{name}-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        CommandPathContext {
            unix: true,
            executable: root.join(".ghostlight/bin/v1.0.0/ghostlight"),
            bin_home: root.join(".local/bin"),
        }
    }

    #[test]
    fn a_system_package_installation_needs_no_user_entry() {
        let context = CommandPathContext {
            unix: true,
            executable: PathBuf::from("/usr/bin/ghostlight"),
            bin_home: PathBuf::from("/nonexistent/.local/bin"),
        };
        let report = inspect(&context).unwrap();
        assert_eq!(report.state, CommandPathState::NotApplicable);
        assert!(report.detail.contains("system package"));
    }

    #[test]
    fn a_non_unix_platform_reports_not_applicable() {
        let mut context = context("windows");
        context.unix = false;
        let report = inspect(&context).unwrap();
        assert_eq!(report.state, CommandPathState::NotApplicable);
        // No install may be attempted, so the unsupported symlink path is never reached.
        assert!(!apply_install(&context).unwrap().changed);
    }

    #[test]
    fn a_missing_entry_names_the_absolute_executable() {
        let context = context("missing");
        let report = inspect(&context).unwrap();
        assert_eq!(report.state, CommandPathState::Missing);
        assert!(report
            .detail
            .contains(&context.executable.display().to_string()));
    }

    #[test]
    fn user_entry_is_created_idempotent_and_removed() {
        if !cfg!(unix) {
            return;
        }
        let context = context("lifecycle");
        fs::create_dir_all(context.executable.parent().unwrap()).unwrap();
        fs::write(&context.executable, b"ghostlight").unwrap();

        let first = apply_install(&context).unwrap();
        assert!(first.changed);
        assert_eq!(first.report.state, CommandPathState::Current);
        assert_eq!(fs::read_link(context.link()).unwrap(), context.executable);

        assert!(!apply_install(&context).unwrap().changed);

        let removed = apply_uninstall(&context).unwrap();
        assert!(removed.changed);
        assert_eq!(removed.report.state, CommandPathState::Missing);
        assert!(!apply_uninstall(&context).unwrap().changed);

        fs::remove_dir_all(context.bin_home.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn an_older_owned_entry_is_updated_in_place() {
        if !cfg!(unix) {
            return;
        }
        let context = context("update");
        fs::create_dir_all(context.executable.parent().unwrap()).unwrap();
        fs::write(&context.executable, b"ghostlight").unwrap();
        let older = context
            .executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("v0.9.0/ghostlight");
        fs::create_dir_all(older.parent().unwrap()).unwrap();
        fs::write(&older, b"ghostlight").unwrap();
        fs::create_dir_all(&context.bin_home).unwrap();
        create_symlink(&older, &context.link()).unwrap();

        assert_eq!(
            inspect(&context).unwrap().state,
            CommandPathState::Updatable
        );
        let updated = apply_install(&context).unwrap();
        assert!(updated.changed);
        assert_eq!(updated.report.state, CommandPathState::Current);
        assert_eq!(fs::read_link(context.link()).unwrap(), context.executable);

        fs::remove_dir_all(context.bin_home.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn a_foreign_file_is_never_replaced_or_removed() {
        if !cfg!(unix) {
            return;
        }
        let context = context("foreign");
        fs::create_dir_all(context.executable.parent().unwrap()).unwrap();
        fs::write(&context.executable, b"ghostlight").unwrap();
        fs::create_dir_all(&context.bin_home).unwrap();
        fs::write(context.link(), b"someone else's script").unwrap();

        assert_eq!(
            inspect(&context).unwrap().state,
            CommandPathState::NeedsAttention
        );
        assert!(!apply_install(&context).unwrap().changed);
        assert!(!apply_uninstall(&context).unwrap().changed);
        assert_eq!(
            fs::read(context.link()).unwrap(),
            b"someone else's script".to_vec()
        );

        fs::remove_dir_all(context.bin_home.parent().unwrap().parent().unwrap()).unwrap();
    }

    #[test]
    fn a_symlink_to_something_else_is_foreign() {
        if !cfg!(unix) {
            return;
        }
        let context = context("foreign-link");
        fs::create_dir_all(context.executable.parent().unwrap()).unwrap();
        fs::write(&context.executable, b"ghostlight").unwrap();
        let stranger = context.executable.parent().unwrap().join("other-tool");
        fs::write(&stranger, b"other").unwrap();
        fs::create_dir_all(&context.bin_home).unwrap();
        create_symlink(&stranger, &context.link()).unwrap();

        assert_eq!(
            inspect(&context).unwrap().state,
            CommandPathState::NeedsAttention
        );
        assert!(!apply_install(&context).unwrap().changed);
        assert_eq!(fs::read_link(context.link()).unwrap(), stranger);

        fs::remove_dir_all(context.bin_home.parent().unwrap().parent().unwrap()).unwrap();
    }
}
