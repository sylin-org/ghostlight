//! Per-user manual pages for the three installed executables.
//!
//! The Debian package installs into `/usr/share/man/man1`. The per-user route has no packaging
//! step, so it installs the same pages under the XDG user data directory. `man` finds them without
//! configuration: for every directory on PATH it also searches the sibling `../share/man`, and the
//! per-user route owns `~/.local/bin` (see [`super::command_path`]).
//!
//! Ownership follows the same rule the Applications entry uses. A symlink in a product-owned
//! location is someone else's and is never touched. A regular file is replaced on install, and
//! removed on uninstall only when its bytes are exactly what Ghostlight would have written.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

/// The manual pages this installation owns, as `(file name, contents)`.
const PAGES: &[(&str, &str)] = &[
    (
        "ghostlight.1",
        include_str!("../../../../packaging/linux/man/ghostlight.1"),
    ),
    (
        "ghostlight-mcp-connector.1",
        include_str!("../../../../packaging/linux/man/ghostlight-mcp-connector.1"),
    ),
    (
        "ghostlight-browser-connector.1",
        include_str!("../../../../packaging/linux/man/ghostlight-browser-connector.1"),
    ),
];

/// Current state of the per-user manual pages.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManPageState {
    /// This platform or installation does not use per-user manual pages.
    NotApplicable,
    /// One or more owned pages are absent or out of date.
    Missing,
    /// Every owned page is present and current.
    Current,
    /// A foreign file or symlink occupies a product-owned location.
    NeedsAttention,
}

/// Read-only result for the per-user manual pages.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManPageReport {
    /// Current ownership and freshness state.
    pub state: ManPageState,
    /// Product-authored explanation of the state.
    pub detail: String,
    /// Directory the pages live in.
    pub directory: PathBuf,
}

/// Result of a manual-page install or uninstall request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManPageActionResult {
    /// Whether an owned file changed.
    pub changed: bool,
    /// State observed after the action.
    pub report: ManPageReport,
}

/// Product-owned per-user manual pages.
#[derive(Clone, Debug)]
pub struct ManPages {
    context: ManPageContext,
}

impl ManPages {
    /// Discover the current executable and XDG user-data root.
    #[must_use]
    pub fn discover() -> Self {
        Self {
            context: ManPageContext::system(),
        }
    }

    /// Inspect the pages without changing them.
    pub fn check(&self) -> Result<ManPageReport, ManPageError> {
        inspect(&self.context)
    }

    /// Write or refresh the owned pages.
    pub fn install(&self) -> Result<ManPageActionResult, ManPageError> {
        apply_install(&self.context)
    }

    /// Remove only byte-identical owned pages.
    pub fn uninstall(&self) -> Result<ManPageActionResult, ManPageError> {
        apply_uninstall(&self.context)
    }
}

#[derive(Clone, Debug)]
struct ManPageContext {
    linux: bool,
    executable: PathBuf,
    data_home: PathBuf,
}

impl ManPageContext {
    fn system() -> Self {
        let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        Self {
            linux: cfg!(target_os = "linux"),
            executable: env::current_exe().unwrap_or_default(),
            data_home: env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share")),
        }
    }

    fn directory(&self) -> PathBuf {
        self.data_home.join("man/man1")
    }

    /// The system package installs into `/usr/share/man` instead.
    fn system_package(&self) -> bool {
        self.executable.parent() == Some(Path::new("/usr/bin"))
    }

    fn applicable(&self) -> bool {
        self.linux && !self.system_package()
    }
}

fn inspect(context: &ManPageContext) -> Result<ManPageReport, ManPageError> {
    let directory = context.directory();
    if !context.applicable() {
        return Ok(ManPageReport {
            state: ManPageState::NotApplicable,
            detail: String::new(),
            directory,
        });
    }
    let mut current = true;
    for (name, contents) in PAGES {
        let path = directory.join(name);
        if is_symlink(&path)? {
            return Ok(ManPageReport {
                state: ManPageState::NeedsAttention,
                detail: format!(
                    "A symlink occupies Ghostlight's manual-page location at {}; it was left untouched.",
                    path.display()
                ),
                directory,
            });
        }
        if read_optional(&path)?.as_deref() != Some(*contents) {
            current = false;
        }
    }
    let state = if current {
        ManPageState::Current
    } else {
        ManPageState::Missing
    };
    Ok(ManPageReport {
        detail: detail_for(state, &directory),
        state,
        directory,
    })
}

fn apply_install(context: &ManPageContext) -> Result<ManPageActionResult, ManPageError> {
    let before = inspect(context)?;
    if matches!(
        before.state,
        ManPageState::NotApplicable | ManPageState::NeedsAttention | ManPageState::Current
    ) {
        return Ok(ManPageActionResult {
            changed: false,
            report: before,
        });
    }
    let directory = context.directory();
    fs::create_dir_all(&directory).map_err(|source| ManPageError::Write {
        path: directory.clone(),
        source,
    })?;
    for (name, contents) in PAGES {
        let path = directory.join(name);
        fs::write(&path, contents).map_err(|source| ManPageError::Write { path, source })?;
    }
    Ok(ManPageActionResult {
        changed: true,
        report: inspect(context)?,
    })
}

fn apply_uninstall(context: &ManPageContext) -> Result<ManPageActionResult, ManPageError> {
    if !context.applicable() {
        return Ok(ManPageActionResult {
            changed: false,
            report: inspect(context)?,
        });
    }
    let directory = context.directory();
    let mut changed = false;
    for (name, contents) in PAGES {
        let path = directory.join(name);
        if is_symlink(&path)? {
            continue;
        }
        // Remove only what this version would have written. An edited page is someone's work.
        if read_optional(&path)?.as_deref() == Some(*contents) {
            fs::remove_file(&path).map_err(|source| ManPageError::Write { path, source })?;
            changed = true;
        }
    }
    Ok(ManPageActionResult {
        changed,
        report: inspect(context)?,
    })
}

fn detail_for(state: ManPageState, directory: &Path) -> String {
    match state {
        ManPageState::Current => format!("Manual pages are installed in {}.", directory.display()),
        ManPageState::Missing => format!(
            "Manual pages are absent or out of date in {}.",
            directory.display()
        ),
        _ => String::new(),
    }
}

fn is_symlink(path: &Path) -> Result<bool, ManPageError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_symlink()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ManPageError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn read_optional(path: &Path) -> Result<Option<String>, ManPageError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ManPageError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Failures while inspecting or changing per-user manual pages.
#[derive(Debug, Error)]
pub enum ManPageError {
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

    fn context(name: &str) -> ManPageContext {
        let root = std::env::temp_dir().join(format!(
            "ghostlight-man-{name}-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        ManPageContext {
            linux: true,
            executable: root.join(".ghostlight/bin/v1.0.0/ghostlight"),
            data_home: root.join("share"),
        }
    }

    #[test]
    fn every_owned_page_has_content_and_a_title() {
        for (name, contents) in PAGES {
            assert!(!contents.is_empty(), "{name} is empty");
            assert!(contents.contains(".TH "), "{name} has no man title line");
            assert!(contents.contains(".SH NAME"), "{name} has no NAME section");
        }
        assert_eq!(PAGES.len(), 3, "one page per installed executable");
    }

    #[test]
    fn pages_are_written_idempotently_and_removed() {
        let context = context("lifecycle");
        let first = apply_install(&context).unwrap();
        assert!(first.changed);
        assert_eq!(first.report.state, ManPageState::Current);
        for (name, contents) in PAGES {
            let written = fs::read_to_string(context.directory().join(name)).unwrap();
            assert_eq!(&written, contents);
        }

        assert!(!apply_install(&context).unwrap().changed);

        let removed = apply_uninstall(&context).unwrap();
        assert!(removed.changed);
        assert_eq!(removed.report.state, ManPageState::Missing);
        assert!(!apply_uninstall(&context).unwrap().changed);

        fs::remove_dir_all(context.data_home.parent().unwrap()).unwrap();
    }

    #[test]
    fn an_edited_page_is_refreshed_on_install_and_kept_on_uninstall() {
        let context = context("edited");
        apply_install(&context).unwrap();
        let edited = context.directory().join(PAGES[0].0);
        fs::write(&edited, "someone's own notes").unwrap();

        assert_eq!(inspect(&context).unwrap().state, ManPageState::Missing);
        assert!(apply_install(&context).unwrap().changed);
        assert_eq!(fs::read_to_string(&edited).unwrap(), PAGES[0].1);

        // Now edit again and uninstall: the edited file survives, the untouched ones go.
        fs::write(&edited, "someone's own notes").unwrap();
        apply_uninstall(&context).unwrap();
        assert_eq!(fs::read_to_string(&edited).unwrap(), "someone's own notes");
        assert!(!context.directory().join(PAGES[1].0).exists());

        fs::remove_dir_all(context.data_home.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_system_package_installation_needs_no_user_pages() {
        let context = ManPageContext {
            linux: true,
            executable: PathBuf::from("/usr/bin/ghostlight"),
            data_home: PathBuf::from("/nonexistent/share"),
        };
        assert_eq!(
            inspect(&context).unwrap().state,
            ManPageState::NotApplicable
        );
        assert!(!apply_install(&context).unwrap().changed);
    }

    #[test]
    fn a_non_linux_platform_installs_nothing() {
        let mut context = context("windows");
        context.linux = false;
        assert_eq!(
            inspect(&context).unwrap().state,
            ManPageState::NotApplicable
        );
        assert!(!apply_install(&context).unwrap().changed);
        assert!(!context.directory().exists());
    }
}
