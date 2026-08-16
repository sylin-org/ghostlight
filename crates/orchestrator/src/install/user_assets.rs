//! Per-user manual pages and shell completions.
//!
//! The Debian package installs these under `/usr/share`. The per-user route has no packaging step,
//! so it writes the same bytes under the XDG user data directory, in the locations each consumer
//! already searches:
//!
//! - `man` looks in `../share/man` for every directory on PATH, and the per-user route owns
//!   `~/.local/bin` (see [`super::command_path`]).
//! - bash-completion reads `$XDG_DATA_HOME/bash-completion/completions`.
//! - fish reads `$XDG_DATA_HOME/fish/vendor_completions.d`.
//! - zsh reads no user data directory by default. The file is still written to the conventional
//!   `site-functions` location so a person who adds it to `fpath` gets completions, and
//!   `ghostlight.1` says so rather than implying it works untouched.
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

/// Every file this installation owns under the user data directory, as
/// `(path relative to that directory, contents)`.
const ASSETS: &[(&str, &str)] = &[
    (
        "man/man1/ghostlight.1",
        include_str!("../../../../packaging/linux/man/ghostlight.1"),
    ),
    (
        "man/man1/ghostlight-mcp-connector.1",
        include_str!("../../../../packaging/linux/man/ghostlight-mcp-connector.1"),
    ),
    (
        "man/man1/ghostlight-browser-connector.1",
        include_str!("../../../../packaging/linux/man/ghostlight-browser-connector.1"),
    ),
    (
        "bash-completion/completions/ghostlight",
        include_str!("../../../../packaging/linux/completions/ghostlight.bash"),
    ),
    (
        "zsh/site-functions/_ghostlight",
        include_str!("../../../../packaging/linux/completions/_ghostlight"),
    ),
    (
        "fish/vendor_completions.d/ghostlight.fish",
        include_str!("../../../../packaging/linux/completions/ghostlight.fish"),
    ),
];

/// Current state of the per-user documentation and completion files.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserAssetState {
    /// This platform or installation does not use per-user data files.
    NotApplicable,
    /// One or more owned files are absent or out of date.
    Missing,
    /// Every owned file is present and current.
    Current,
    /// A foreign file or symlink occupies a product-owned location.
    NeedsAttention,
}

/// Read-only result for the per-user documentation and completion files.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UserAssetReport {
    /// Current ownership and freshness state.
    pub state: UserAssetState,
    /// Product-authored explanation of the state.
    pub detail: String,
    /// Root the files live under.
    pub directory: PathBuf,
}

/// Result of a user-asset install or uninstall request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UserAssetActionResult {
    /// Whether an owned file changed.
    pub changed: bool,
    /// State observed after the action.
    pub report: UserAssetReport,
}

/// Product-owned per-user documentation and completion files.
#[derive(Clone, Debug)]
pub struct UserAssets {
    context: UserAssetContext,
}

impl UserAssets {
    /// Discover the current executable and XDG user-data root.
    #[must_use]
    pub fn discover() -> Self {
        Self {
            context: UserAssetContext::system(),
        }
    }

    /// Inspect the files without changing them.
    pub fn check(&self) -> Result<UserAssetReport, UserAssetError> {
        inspect(&self.context)
    }

    /// Write or refresh the owned files.
    pub fn install(&self) -> Result<UserAssetActionResult, UserAssetError> {
        apply_install(&self.context)
    }

    /// Remove only byte-identical owned files.
    pub fn uninstall(&self) -> Result<UserAssetActionResult, UserAssetError> {
        apply_uninstall(&self.context)
    }
}

#[derive(Clone, Debug)]
struct UserAssetContext {
    linux: bool,
    executable: PathBuf,
    data_home: PathBuf,
}

impl UserAssetContext {
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

    /// The system package installs the same files under `/usr/share` instead.
    fn system_package(&self) -> bool {
        self.executable.parent() == Some(Path::new("/usr/bin"))
    }

    fn applicable(&self) -> bool {
        self.linux && !self.system_package()
    }
}

fn inspect(context: &UserAssetContext) -> Result<UserAssetReport, UserAssetError> {
    let directory = context.data_home.clone();
    if !context.applicable() {
        return Ok(UserAssetReport {
            state: UserAssetState::NotApplicable,
            detail: String::new(),
            directory,
        });
    }
    let mut current = true;
    for (name, contents) in ASSETS {
        let path = directory.join(name);
        if is_symlink(&path)? {
            return Ok(UserAssetReport {
                state: UserAssetState::NeedsAttention,
                detail: format!(
                    "A symlink occupies a Ghostlight-owned location at {}; it was left untouched.",
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
        UserAssetState::Current
    } else {
        UserAssetState::Missing
    };
    Ok(UserAssetReport {
        detail: detail_for(state, &directory),
        state,
        directory,
    })
}

fn apply_install(context: &UserAssetContext) -> Result<UserAssetActionResult, UserAssetError> {
    let before = inspect(context)?;
    if matches!(
        before.state,
        UserAssetState::NotApplicable | UserAssetState::NeedsAttention | UserAssetState::Current
    ) {
        return Ok(UserAssetActionResult {
            changed: false,
            report: before,
        });
    }
    let directory = context.data_home.clone();
    for (name, contents) in ASSETS {
        let path = directory.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| UserAssetError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&path, contents).map_err(|source| UserAssetError::Write { path, source })?;
    }
    Ok(UserAssetActionResult {
        changed: true,
        report: inspect(context)?,
    })
}

fn apply_uninstall(context: &UserAssetContext) -> Result<UserAssetActionResult, UserAssetError> {
    if !context.applicable() {
        return Ok(UserAssetActionResult {
            changed: false,
            report: inspect(context)?,
        });
    }
    let directory = context.data_home.clone();
    let mut changed = false;
    for (name, contents) in ASSETS {
        let path = directory.join(name);
        if is_symlink(&path)? {
            continue;
        }
        // Remove only what this version would have written. An edited file is someone's work.
        if read_optional(&path)?.as_deref() == Some(*contents) {
            fs::remove_file(&path).map_err(|source| UserAssetError::Write { path, source })?;
            changed = true;
        }
    }
    Ok(UserAssetActionResult {
        changed,
        report: inspect(context)?,
    })
}

fn detail_for(state: UserAssetState, directory: &Path) -> String {
    match state {
        UserAssetState::Current => format!(
            "Manual pages and shell completions are installed under {}.",
            directory.display()
        ),
        UserAssetState::Missing => format!(
            "Manual pages or shell completions are absent or out of date under {}.",
            directory.display()
        ),
        _ => String::new(),
    }
}

fn is_symlink(path: &Path) -> Result<bool, UserAssetError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_symlink()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(UserAssetError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn read_optional(path: &Path) -> Result<Option<String>, UserAssetError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(UserAssetError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Failures while inspecting or changing per-user data files.
#[derive(Debug, Error)]
pub enum UserAssetError {
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

    fn context(name: &str) -> UserAssetContext {
        let root = std::env::temp_dir().join(format!(
            "ghostlight-assets-{name}-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        UserAssetContext {
            linux: true,
            executable: root.join(".ghostlight/bin/v1.0.0/ghostlight"),
            data_home: root.join("share"),
        }
    }

    #[test]
    fn every_owned_asset_has_content() {
        for (name, contents) in ASSETS {
            assert!(!contents.is_empty(), "{name} is empty");
            if name.starts_with("man/") {
                assert!(contents.contains(".TH "), "{name} has no man title line");
                assert!(contents.contains(".SH NAME"), "{name} has no NAME section");
            }
        }
        // Three manual pages, one per installed executable, plus one completion per shell.
        assert_eq!(ASSETS.len(), 6);
    }

    #[test]
    fn assets_are_written_idempotently_and_removed() {
        let context = context("lifecycle");
        let first = apply_install(&context).unwrap();
        assert!(first.changed);
        assert_eq!(first.report.state, UserAssetState::Current);
        for (name, contents) in ASSETS {
            let written = fs::read_to_string(context.data_home.join(name)).unwrap();
            assert_eq!(&written, contents, "{name} was not written verbatim");
        }

        assert!(!apply_install(&context).unwrap().changed);

        let removed = apply_uninstall(&context).unwrap();
        assert!(removed.changed);
        assert_eq!(removed.report.state, UserAssetState::Missing);
        assert!(!apply_uninstall(&context).unwrap().changed);

        fs::remove_dir_all(context.data_home.parent().unwrap()).unwrap();
    }

    #[test]
    fn an_edited_file_is_refreshed_on_install_and_kept_on_uninstall() {
        let context = context("edited");
        apply_install(&context).unwrap();
        let edited = context.data_home.join(ASSETS[0].0);
        fs::write(&edited, "someone's own notes").unwrap();

        assert_eq!(inspect(&context).unwrap().state, UserAssetState::Missing);
        assert!(apply_install(&context).unwrap().changed);
        assert_eq!(fs::read_to_string(&edited).unwrap(), ASSETS[0].1);

        // Edit again, then uninstall: the edited file survives and the untouched ones go.
        fs::write(&edited, "someone's own notes").unwrap();
        apply_uninstall(&context).unwrap();
        assert_eq!(fs::read_to_string(&edited).unwrap(), "someone's own notes");
        assert!(!context.data_home.join(ASSETS[1].0).exists());

        fs::remove_dir_all(context.data_home.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_system_package_installation_needs_no_user_assets() {
        let context = UserAssetContext {
            linux: true,
            executable: PathBuf::from("/usr/bin/ghostlight"),
            data_home: PathBuf::from("/nonexistent/share"),
        };
        assert_eq!(
            inspect(&context).unwrap().state,
            UserAssetState::NotApplicable
        );
        assert!(!apply_install(&context).unwrap().changed);
    }

    #[test]
    fn a_non_linux_platform_installs_nothing() {
        let mut context = context("windows");
        context.linux = false;
        assert_eq!(
            inspect(&context).unwrap().state,
            UserAssetState::NotApplicable
        );
        assert!(!apply_install(&context).unwrap().changed);
        assert!(!context.data_home.join(ASSETS[0].0).exists());
    }
}
