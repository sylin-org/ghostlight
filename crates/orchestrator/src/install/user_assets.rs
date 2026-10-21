//! Per-user manual pages and shell completions.
//!
//! Ghostlight ships no system package, so the installation writes these under the XDG user data
//! directory, in the locations each consumer already searches:
//!
//! - `man` looks in `../share/man` for every directory on PATH, and the per-user route owns
//!   `~/.local/bin` (see [`super::command_path`]).
//! - bash-completion reads `$XDG_DATA_HOME/bash-completion/completions`.
//! - fish reads `$XDG_DATA_HOME/fish/vendor_completions.d`.
//! - zsh reads no user data directory by default. The file is still written to the conventional
//!   `site-functions` location so a person who adds it to `fpath` gets completions, and
//!   `ghostlight.1` gives the one line to add rather than implying it works untouched.
//!
//! Ownership follows the same rule the Applications entry uses. A symlink in a product-owned
//! location is someone else's and is never touched. A regular file is replaced on install, and
//! removed on uninstall only when its bytes are exactly what Ghostlight would have written.
//!
//! 1.x also installed manual pages for its two connectors, which 2.0 replaced with the edge
//! ([`RETIRED_PAGES`]). Install and uninstall remove each one 1.x wrote, so `man` never describes a
//! program that is gone, and inspection counts one that lingers as out of date.

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

/// The manual pages 1.x installed for its connectors, as `(path relative to the user data
/// directory, the page's title)`. Their date and version changed with every 1.x release, so a
/// page is known as 1.x's by its `.TH` line rather than by its bytes.
const RETIRED_PAGES: &[(&str, &str)] = &[
    (
        "man/man1/ghostlight-mcp-connector.1",
        "GHOSTLIGHT-MCP-CONNECTOR",
    ),
    (
        "man/man1/ghostlight-browser-connector.1",
        "GHOSTLIGHT-BROWSER-CONNECTOR",
    ),
];

/// Whether `text` is the page 1.x wrote for `title`: its `.TH` line names `title` in section 1 of
/// a Ghostlight 1.x manual.
fn written_by_1x(text: &str, title: &str) -> bool {
    text.lines()
        .find(|line| line.starts_with(".TH "))
        .is_some_and(|line| {
            line.starts_with(&format!(".TH {title} 1 "))
                && line.contains("\"Ghostlight 1.")
                && line.ends_with("\"Ghostlight Manual\"")
        })
}

/// The retired 1.x pages under `directory` that 1.x wrote and that are still there. A symlink is
/// someone else's and is never one of them.
fn lingering_pages(directory: &Path) -> Result<Vec<PathBuf>, UserAssetError> {
    let mut lingering = Vec::new();
    for (name, title) in RETIRED_PAGES {
        let path = directory.join(name);
        if is_symlink(&path)? {
            continue;
        }
        if read_optional(&path)?.is_some_and(|text| written_by_1x(&text, title)) {
            lingering.push(path);
        }
    }
    Ok(lingering)
}

/// Remove the retired 1.x pages under `directory`. Returns whether one was removed.
fn retire_pages(directory: &Path) -> Result<bool, UserAssetError> {
    let lingering = lingering_pages(directory)?;
    for path in &lingering {
        fs::remove_file(path).map_err(|source| UserAssetError::Write {
            path: path.clone(),
            source,
        })?;
    }
    Ok(!lingering.is_empty())
}

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
        let home = super::user_dirs::directory("HOME").unwrap_or_default();
        Self {
            linux: cfg!(target_os = "linux"),
            executable: super::layout::Layout::current()
                .map(|layout| layout.service_executable())
                .unwrap_or_default(),
            data_home: super::user_dirs::directory("XDG_DATA_HOME")
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
    if !lingering_pages(&directory)?.is_empty() {
        current = false;
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
    let retired = context.applicable() && retire_pages(&context.data_home)?;
    let before = inspect(context)?;
    if matches!(
        before.state,
        UserAssetState::NotApplicable | UserAssetState::NeedsAttention | UserAssetState::Current
    ) {
        return Ok(UserAssetActionResult {
            changed: retired,
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
    let mut changed = retire_pages(&directory)?;
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

    use crate::scratch::Scratch;

    /// A context rooted in a fresh scratch directory, which lives as long as the returned guard.
    fn context(name: &str) -> (Scratch, UserAssetContext) {
        let root = Scratch::named(&format!("assets-{name}"));
        let context = UserAssetContext {
            linux: true,
            executable: root.join(".ghostlight/bin/v1.0.0/ghostlight"),
            data_home: root.join("share"),
        };
        (root, context)
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
        // One manual page plus one completion per shell.
        assert_eq!(ASSETS.len(), 4);
    }

    #[test]
    fn assets_are_written_idempotently_and_removed() {
        let (_root, context) = context("lifecycle");
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
    }

    #[test]
    fn an_edited_file_is_refreshed_on_install_and_kept_on_uninstall() {
        let (_root, context) = context("edited");
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
    }

    /// The `.TH` line a 1.x release wrote for `title`.
    fn page_of_1x(title: &str, version: &str) -> String {
        format!(
            ".\\\" SPDX-License-Identifier: Apache-2.0 OR MIT\n.TH {title} 1 \"2026-09-08\" \"Ghostlight {version}\" \"Ghostlight Manual\"\n.SH NAME\n"
        )
    }

    #[test]
    fn the_connector_pages_1x_left_are_retired_and_anything_else_there_is_kept() {
        let (_root, context) = context("retired");
        let man = context.data_home.join("man/man1");
        fs::create_dir_all(&man).unwrap();
        let mcp = man.join("ghostlight-mcp-connector.1");
        let browser = man.join("ghostlight-browser-connector.1");
        fs::write(&mcp, page_of_1x("GHOSTLIGHT-MCP-CONNECTOR", "1.3.5")).unwrap();
        fs::write(
            &browser,
            page_of_1x("GHOSTLIGHT-BROWSER-CONNECTOR", "1.0.0"),
        )
        .unwrap();
        apply_install(&context).unwrap();
        assert!(
            !mcp.exists() && !browser.exists(),
            "1.x's pages are retired"
        );
        assert_eq!(inspect(&context).unwrap().state, UserAssetState::Current);

        // A page that is current already still loses a lingering 1.x page, and says it changed.
        fs::write(&mcp, page_of_1x("GHOSTLIGHT-MCP-CONNECTOR", "1.3.15")).unwrap();
        assert_eq!(inspect(&context).unwrap().state, UserAssetState::Missing);
        assert!(apply_install(&context).unwrap().changed);
        assert!(!mcp.exists());

        // Someone's own file under that name, and a symlink, are kept.
        fs::write(&mcp, "someone's own notes").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&mcp, &browser).unwrap();
        apply_install(&context).unwrap();
        apply_uninstall(&context).unwrap();
        assert_eq!(fs::read_to_string(&mcp).unwrap(), "someone's own notes");
        #[cfg(unix)]
        assert!(is_symlink(&browser).unwrap());
    }

    #[test]
    fn an_uninstall_retires_the_connector_pages_1x_left() {
        let (_root, context) = context("retired-uninstall");
        let man = context.data_home.join("man/man1");
        fs::create_dir_all(&man).unwrap();
        let mcp = man.join("ghostlight-mcp-connector.1");
        fs::write(&mcp, page_of_1x("GHOSTLIGHT-MCP-CONNECTOR", "1.3.14")).unwrap();
        assert!(apply_uninstall(&context).unwrap().changed);
        assert!(!mcp.exists());
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
        let (_root, mut context) = context("windows");
        context.linux = false;
        assert_eq!(
            inspect(&context).unwrap().state,
            UserAssetState::NotApplicable
        );
        assert!(!apply_install(&context).unwrap().changed);
        assert!(!context.data_home.join(ASSETS[0].0).exists());
    }
}
