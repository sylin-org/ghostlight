//! Ownership-checked XDG application entry for Linux per-user installations.
//!
//! The entry runs the shell, `bin/ghostlight-desktop` (ADR-0199 Decision 4), which starts the
//! service when none runs, and names the window class the shell sets so the desktop groups the
//! window under the entry. An entry that ran the service image is Ghostlight's own and is
//! rewritten.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use super::record::{Link, Replace};
use crate::language::deployment as words;

const APPLICATION_ID: &str = "org.sylin.ghostlight";
const DESKTOP_TEMPLATE: &str = include_str!("../../../../packaging/linux/ghostlight.desktop.hbs");
const ICON: &[u8] = include_bytes!("../../../../extension/icons/icon128.png");
const OWNERSHIP_MARKER: &str = "X-Ghostlight-Owned=true";

/// Current state of Ghostlight's per-user application-menu integration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopIntegrationState {
    /// The operating system or system package owns desktop integration instead.
    NotApplicable,
    /// No complete Ghostlight-owned entry exists.
    Missing,
    /// The entry and icon name this exact installed executable.
    Current,
    /// Ghostlight-owned files need an ordinary version-path update.
    Updatable,
    /// A foreign file or symlink occupies a product-owned location.
    NeedsAttention,
}

/// Read-only result for the per-user Applications entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopIntegrationReport {
    /// Current ownership and freshness state.
    pub state: DesktopIntegrationState,
    /// Product-authored explanation of the state.
    pub detail: String,
    /// Exact desktop-entry location; absent where the platform has no XDG entry.
    pub desktop_entry: Option<PathBuf>,
    /// Exact icon location; absent where the platform has no XDG entry.
    pub icon: Option<PathBuf>,
}

/// Result of an application-entry install or uninstall request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DesktopIntegrationActionResult {
    /// Whether an owned file changed.
    pub changed: bool,
    /// State observed after the action.
    pub report: DesktopIntegrationReport,
}

/// Product-owned XDG desktop integration service.
#[derive(Clone, Debug)]
pub struct DesktopIntegration {
    context: DesktopIntegrationContext,
}

impl DesktopIntegration {
    /// Discover the current executable and XDG user-data root.
    #[must_use]
    pub fn discover() -> Self {
        Self {
            context: DesktopIntegrationContext::system(),
        }
    }

    /// Inspect desktop integration without changing it.
    pub fn check(&self) -> Result<DesktopIntegrationReport, DesktopIntegrationError> {
        inspect(&self.context)
    }

    /// Install or update the exact owned desktop entry and icon.
    pub fn install(&self) -> Result<DesktopIntegrationActionResult, DesktopIntegrationError> {
        apply_install(&self.context)
    }

    /// Remove only the owned desktop entry and byte-identical icon.
    pub fn uninstall(&self) -> Result<DesktopIntegrationActionResult, DesktopIntegrationError> {
        apply_uninstall(&self.context)
    }
}

#[derive(Clone, Debug)]
struct DesktopIntegrationContext {
    linux: bool,
    executable: PathBuf,
    data_home: PathBuf,
}

impl DesktopIntegrationContext {
    fn system() -> Self {
        let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        Self {
            linux: cfg!(target_os = "linux"),
            executable: super::layout::Layout::current()
                .map(|layout| layout.desktop_executable())
                .unwrap_or_default(),
            data_home: env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share")),
        }
    }

    fn desktop_entry(&self) -> PathBuf {
        self.data_home
            .join("applications")
            .join(format!("{APPLICATION_ID}.desktop"))
    }

    fn icon(&self) -> PathBuf {
        self.data_home
            .join("icons/hicolor/128x128/apps")
            .join(format!("{APPLICATION_ID}.png"))
    }

    fn system_package(&self) -> bool {
        self.executable.parent() == Some(Path::new("/usr/bin"))
    }
}

fn inspect(
    context: &DesktopIntegrationContext,
) -> Result<DesktopIntegrationReport, DesktopIntegrationError> {
    let desktop_entry = context.desktop_entry();
    let icon = context.icon();
    let mut icon_only = false;
    let state = if !context.linux || context.system_package() {
        DesktopIntegrationState::NotApplicable
    } else if symlink(&desktop_entry)? || symlink(&icon)? {
        DesktopIntegrationState::NeedsAttention
    } else {
        let existing_entry = read_optional(&desktop_entry)?;
        let existing_icon = read_optional(&icon)?;
        let expected_entry = render_desktop_entry(&context.executable);
        let entry_owned = existing_entry.as_deref().is_some_and(desktop_entry_owned);
        let icon_owned = existing_icon.as_deref().is_none_or(|bytes| bytes == ICON);
        if existing_entry.as_deref() == Some(expected_entry.as_bytes())
            && existing_icon.as_deref() == Some(ICON)
        {
            DesktopIntegrationState::Current
        } else if existing_entry.is_some() && !entry_owned || !icon_owned {
            DesktopIntegrationState::NeedsAttention
        } else if entry_owned {
            icon_only = existing_entry.as_deref() == Some(expected_entry.as_bytes());
            DesktopIntegrationState::Updatable
        } else {
            DesktopIntegrationState::Missing
        }
    };
    Ok(DesktopIntegrationReport {
        state,
        detail: if context.linux {
            words::applications_detail(state, icon_only).into()
        } else {
            words::APPLICATIONS_ELSEWHERE.into()
        },
        desktop_entry: context.linux.then_some(desktop_entry),
        icon: context.linux.then_some(icon),
    })
}

fn apply_install(
    context: &DesktopIntegrationContext,
) -> Result<DesktopIntegrationActionResult, DesktopIntegrationError> {
    let before = inspect(context)?;
    if before.state == DesktopIntegrationState::NotApplicable
        || before.state == DesktopIntegrationState::Current
    {
        return Ok(DesktopIntegrationActionResult {
            changed: false,
            report: before,
        });
    }
    if before.state == DesktopIntegrationState::NeedsAttention {
        return Err(DesktopIntegrationError::ForeignState {
            desktop_entry: context.desktop_entry(),
            icon: context.icon(),
        });
    }
    write_atomic(&context.icon(), ICON)?;
    write_atomic(
        &context.desktop_entry(),
        render_desktop_entry(&context.executable).as_bytes(),
    )?;
    let report = inspect(context)?;
    if report.state != DesktopIntegrationState::Current {
        return Err(DesktopIntegrationError::VerificationFailed);
    }
    Ok(DesktopIntegrationActionResult {
        changed: true,
        report,
    })
}

fn apply_uninstall(
    context: &DesktopIntegrationContext,
) -> Result<DesktopIntegrationActionResult, DesktopIntegrationError> {
    let before = inspect(context)?;
    if before.state == DesktopIntegrationState::NotApplicable {
        return Ok(DesktopIntegrationActionResult {
            changed: false,
            report: before,
        });
    }
    let mut changed = false;
    if read_optional(&context.desktop_entry())?
        .as_deref()
        .is_some_and(desktop_entry_owned)
        && !symlink(&context.desktop_entry())?
    {
        remove_file(&context.desktop_entry())?;
        changed = true;
    }
    if read_optional(&context.icon())?.as_deref() == Some(ICON) && !symlink(&context.icon())? {
        remove_file(&context.icon())?;
        changed = true;
    }
    Ok(DesktopIntegrationActionResult {
        changed,
        report: inspect(context)?,
    })
}

fn render_desktop_entry(executable: &Path) -> String {
    DESKTOP_TEMPLATE
        .replace("{{exec}}", &quote_exec(executable))
        .replace("{{icon}}", APPLICATION_ID)
        .replace("{{wm_class}}", APPLICATION_ID)
        .replace("{{name}}", "Ghostlight")
}

fn quote_exec(executable: &Path) -> String {
    let value = executable.to_string_lossy();
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        if character == '%' {
            escaped.push_str("%%");
            continue;
        }
        if matches!(character, '"' | '\\' | '$' | '`') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped.push('"');
    escaped
}

fn desktop_entry_owned(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok_and(|source| {
        source.lines().any(|line| line == OWNERSHIP_MARKER)
            && source.lines().any(|line| line == "Type=Application")
    })
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, DesktopIntegrationError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(DesktopIntegrationError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn symlink(path: &Path) -> Result<bool, DesktopIntegrationError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_symlink()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(DesktopIntegrationError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// How the entry and its icon are written: the access a file there had kept, and a link refused,
/// since inspection already calls a link at either path foreign and one may appear after it
/// looked.
const ENTRY_WRITE: Replace = Replace {
    link: Link::Refuse,
    ..Replace::SHARED
};

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), DesktopIntegrationError> {
    ENTRY_WRITE
        .write(path, bytes)
        .map_err(|source| DesktopIntegrationError::Write {
            path: path.to_path_buf(),
            source,
        })
}

fn remove_file(path: &Path) -> Result<(), DesktopIntegrationError> {
    fs::remove_file(path).map_err(|source| DesktopIntegrationError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Safe desktop-integration failure.
#[derive(Debug, Error)]
pub enum DesktopIntegrationError {
    /// An owned location could not be read.
    #[error("could not read desktop integration at {path}: {source}")]
    Read {
        /// File that could not be read.
        path: PathBuf,
        /// Operating-system error.
        source: io::Error,
    },
    /// An owned location could not be changed.
    #[error("could not update desktop integration at {path}: {source}")]
    Write {
        /// File that could not be changed.
        path: PathBuf,
        /// Operating-system error.
        source: io::Error,
    },
    /// Foreign state occupies one of the two exact product paths.
    #[error("foreign desktop integration was left untouched at {desktop_entry} or {icon}")]
    ForeignState {
        /// Desktop entry location.
        desktop_entry: PathBuf,
        /// Icon location.
        icon: PathBuf,
    },
    /// A completed write did not produce the expected state.
    #[error("desktop integration did not reach its exact current state")]
    VerificationFailed,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::scratch::Scratch;

    use super::{
        apply_install, apply_uninstall, inspect, render_desktop_entry, DesktopIntegrationContext,
        DesktopIntegrationState, ICON,
    };

    /// A context rooted in a fresh scratch directory, which lives as long as the returned guard.
    fn context(name: &str) -> (Scratch, DesktopIntegrationContext) {
        let root = Scratch::named(&format!("desktop-{name}"));
        let context = DesktopIntegrationContext {
            linux: true,
            executable: root.join(".ghostlight/bin/v1.0.0/ghostlight"),
            data_home: root.join("share"),
        };
        (root, context)
    }

    #[test]
    fn other_platforms_name_no_linux_paths() {
        let (_root, mut context) = context("elsewhere");
        context.linux = false;
        let report = inspect(&context).unwrap();
        assert_eq!(report.state, DesktopIntegrationState::NotApplicable);
        assert!(report.desktop_entry.is_none() && report.icon.is_none());
        assert!(!report.detail.contains("operating system package"));
    }

    #[test]
    fn user_install_is_exact_idempotent_and_ownership_safe() {
        let (_root, context) = context("lifecycle");
        fs::create_dir_all(context.executable.parent().unwrap()).unwrap();
        fs::write(&context.executable, b"ghostlight").unwrap();

        let first = apply_install(&context).unwrap();
        assert!(first.changed);
        assert_eq!(first.report.state, DesktopIntegrationState::Current);
        let source = fs::read_to_string(context.desktop_entry()).unwrap();
        assert_eq!(source, render_desktop_entry(&context.executable));
        assert_eq!(fs::read(context.icon()).unwrap(), ICON);
        assert!(!apply_install(&context).unwrap().changed);

        let removed = apply_uninstall(&context).unwrap();
        assert!(removed.changed);
        assert_eq!(removed.report.state, DesktopIntegrationState::Missing);
        assert!(!apply_uninstall(&context).unwrap().changed);
    }

    #[test]
    fn upgrade_rewrites_only_an_owned_entry() {
        let (_root, context) = context("upgrade");
        let entry = context.desktop_entry();
        let icon = context.icon();
        fs::create_dir_all(entry.parent().unwrap()).unwrap();
        fs::create_dir_all(icon.parent().unwrap()).unwrap();
        fs::write(
            &entry,
            render_desktop_entry(Path::new("/home/test/.ghostlight/bin/v0.8.0/ghostlight")),
        )
        .unwrap();
        fs::write(&icon, ICON).unwrap();

        let report = inspect(&context).unwrap();
        assert_eq!(report.state, DesktopIntegrationState::Updatable);
        assert_eq!(
            report.detail,
            "The owned Applications entry points at an older installation."
        );
        assert!(apply_install(&context).unwrap().changed);
        assert_eq!(
            fs::read_to_string(entry).unwrap(),
            render_desktop_entry(&context.executable)
        );
    }

    #[test]
    fn an_entry_that_lost_only_its_icon_is_not_said_to_be_older() {
        let (_root, context) = context("icon-only");
        let entry = context.desktop_entry();
        fs::create_dir_all(entry.parent().unwrap()).unwrap();
        fs::write(&entry, render_desktop_entry(&context.executable)).unwrap();

        let report = inspect(&context).unwrap();
        assert_eq!(report.state, DesktopIntegrationState::Updatable);
        assert_eq!(
            report.detail,
            "The owned Applications entry starts this installation, but its icon is missing."
        );
        assert!(!report.detail.contains("older"));
        assert!(apply_install(&context).unwrap().changed);
        assert_eq!(fs::read(context.icon()).unwrap(), ICON);
    }

    #[test]
    fn foreign_files_and_symlinks_are_preserved() {
        let (_root, context) = context("foreign");
        let entry = context.desktop_entry();
        fs::create_dir_all(entry.parent().unwrap()).unwrap();
        fs::write(&entry, b"[Desktop Entry]\nType=Application\nName=Foreign\n").unwrap();

        assert_eq!(
            inspect(&context).unwrap().state,
            DesktopIntegrationState::NeedsAttention
        );
        assert!(apply_install(&context).is_err());
        assert!(!apply_uninstall(&context).unwrap().changed);
        assert!(entry.exists());
    }

    #[test]
    fn system_package_does_not_create_a_user_shadow() {
        let (_root, mut context) = context("system");
        context.executable = PathBuf::from("/usr/bin/ghostlight");
        assert_eq!(
            apply_install(&context).unwrap().report.state,
            DesktopIntegrationState::NotApplicable
        );
        assert!(!context.desktop_entry().exists());
    }

    #[test]
    fn desktop_exec_quotes_shell_characters_and_escapes_field_codes() {
        let rendered = render_desktop_entry(Path::new("/home/Ghost %Light/$bin/ghostlight"));
        assert!(rendered.contains("Exec=\"/home/Ghost %%Light/\\$bin/ghostlight\"\n"));
    }

    #[test]
    fn the_entry_runs_the_shell_and_names_its_window_class() {
        let rendered =
            render_desktop_entry(Path::new("/home/p/.ghostlight/bin/ghostlight-desktop"));
        assert!(rendered.contains("Exec=\"/home/p/.ghostlight/bin/ghostlight-desktop\"\n"));
        assert!(rendered.contains("StartupWMClass=org.sylin.ghostlight\n"));
        assert!(!rendered.contains(" open"));
    }

    #[test]
    fn an_entry_that_ran_the_service_image_is_owned_and_rewritten() {
        let (_root, context) = context("older-entry");
        let entry = context.desktop_entry();
        let icon = context.icon();
        fs::create_dir_all(entry.parent().unwrap()).unwrap();
        fs::create_dir_all(icon.parent().unwrap()).unwrap();
        let older = "[Desktop Entry]\nExec=\"/h/bin/ghostlight-service\" open\nName=Ghostlight\n\
                     Type=Application\nX-Ghostlight-Owned=true\n";
        fs::write(&entry, older).unwrap();
        fs::write(&icon, ICON).unwrap();
        assert_eq!(
            inspect(&context).unwrap().state,
            DesktopIntegrationState::Updatable
        );
        assert!(apply_install(&context).unwrap().changed);
        let rewritten = fs::read_to_string(&entry).unwrap();
        assert!(rewritten.contains("StartupWMClass=org.sylin.ghostlight"));
        assert!(!rewritten.contains("ghostlight-service"));
    }
}
