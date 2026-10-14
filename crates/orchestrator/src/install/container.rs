//! Where an install runs: in the open, or inside a Windows app container.
//!
//! A process started under an MSIX-packaged desktop app runs in that app's container even when it
//! reports no package identity. Its writes to `HKCU`, `%APPDATA%` and `%LOCALAPPDATA%` land in a
//! private per-app view that no other program, a browser included, ever reads. The user profile
//! root is not redirected, so `~/.ghostlight` is shared with everything. The installer therefore
//! writes native-host manifests below the Ghostlight home, writes a registry value or a client
//! configuration under `AppData` only when it differs from what is already there, and, inside a
//! container, leaves those redirected registrations unwritten and says which ones, with the one
//! command that finishes them from a regular terminal (ADR-0197 amendment, 2026-10-14).
//!
//! Detection looks at the parent chain: the process is inside a container when an ancestor's
//! executable lies under a `WindowsApps` directory. Nothing here starts a process or reads the
//! registry.

use std::path::Path;
use std::sync::OnceLock;

use serde::Serialize;

use super::layout::Layout;

/// How many ancestors the detection walks before it stops.
pub const MAX_ANCESTOR_DEPTH: usize = 16;

/// The directory below which Windows keeps the files of packaged apps.
const PACKAGE_ROOT_DIRECTORY: &str = "WindowsApps";

/// Separates the name from the version in a package directory such as `Claude_1.2.3_x64__id`.
const PACKAGE_NAME_END: char = '_';

/// What the report calls an app whose package directory has no readable name.
const UNNAMED_APP: &str = "an app";

/// What `doctor` says of a registration whose state only another app can see.
pub const NOT_VISIBLE: &str = "cannot be checked from here";

/// The subcommand of the installed service that registers browsers and clients.
const FINISHING_SUBCOMMAND: &str = "install";

/// The app whose container this process runs in.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AppContainer {
    /// The app's name as its package directory spells it, or a plain stand-in.
    pub app: String,
}

/// Whether this process shares the machine's registry and profile with other programs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Residence {
    /// An ordinary process: what it writes, every program sees.
    Regular,
    /// Inside an app container: registry and `AppData` writes stay private to that app.
    AppContainer(AppContainer),
}

impl Residence {
    /// Where this process runs, decided once from its parent chain.
    #[must_use]
    pub fn detect() -> Self {
        static DETECTED: OnceLock<Residence> = OnceLock::new();
        DETECTED
            .get_or_init(|| {
                Self::from_ancestors(&ghostlight_win_peer::ancestor_images(MAX_ANCESTOR_DEPTH))
            })
            .clone()
    }

    /// Where a process runs whose ancestors' executables are `images`, nearest first.
    #[must_use]
    pub fn from_ancestors<P: AsRef<Path>>(images: &[P]) -> Self {
        images
            .iter()
            .take(MAX_ANCESTOR_DEPTH)
            .find_map(|image| packaged_app(image.as_ref()))
            .map_or(Self::Regular, |app| {
                Self::AppContainer(AppContainer { app })
            })
    }

    /// The container, when this process is inside one.
    #[must_use]
    pub fn container(&self) -> Option<&AppContainer> {
        match self {
            Self::Regular => None,
            Self::AppContainer(container) => Some(container),
        }
    }
}

/// The app name when `image` lies below a `WindowsApps` directory.
fn packaged_app(image: &Path) -> Option<String> {
    // Windows paths are split by hand so the answer does not depend on the host operating system.
    let text = image.to_string_lossy();
    let mut components = text.split(['\\', '/']).filter(|part| !part.is_empty());
    components.find(|name| name.eq_ignore_ascii_case(PACKAGE_ROOT_DIRECTORY))?;
    let package = components.next().unwrap_or_default();
    let name = package.split(PACKAGE_NAME_END).next().unwrap_or_default();
    Some(if name.is_empty() { UNNAMED_APP } else { name }.to_owned())
}

/// The command that finishes the private registrations from a regular terminal.
#[must_use]
pub fn finishing_command(layout: &Layout) -> String {
    format!(
        "{} {FINISHING_SUBCOMMAND}",
        layout.service_executable().display()
    )
}

/// The lines an install prints when it left `registrations` unwritten because `container` keeps
/// them private.
#[must_use]
pub fn kept_private_lines(
    container: &AppContainer,
    registrations: &[String],
    layout: &Layout,
) -> Vec<String> {
    vec![
        format!(
            "Windows keeps these registrations private to {}, so no other app sees them: {}.",
            container.app,
            registrations.join(", ")
        ),
        format!(
            "To finish them, run this from a regular terminal: {}",
            finishing_command(layout)
        ),
    ]
}

/// The line `doctor` prints in place of a registration state it cannot see.
#[must_use]
pub fn cannot_see_line(container: &AppContainer, layout: &Layout) -> String {
    format!(
        "This terminal runs inside {}, and Windows gives that app its own view of the registry \
         and AppData, so Ghostlight cannot see what other apps see there. To check the real \
         registrations, run this from a regular terminal: {} doctor",
        container.app,
        layout.service_executable().display()
    )
}

/// The refusal when an uninstall would only change the private view.
#[must_use]
pub fn uninstall_refusal(container: &AppContainer, layout: &Layout) -> String {
    format!(
        "This terminal runs inside {}, and Windows gives that app its own view of the registry \
         and AppData, so removing registrations here would not remove them for other apps. Run \
         this from a regular terminal: {} uninstall",
        container.app,
        layout.service_executable().display()
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn chain(images: &[&str]) -> Residence {
        Residence::from_ancestors(&images.iter().map(PathBuf::from).collect::<Vec<_>>())
    }

    #[test]
    fn an_ancestor_under_windows_apps_puts_the_process_in_that_apps_container() {
        let residence = chain(&[
            r"C:\Windows\System32\cmd.exe",
            r"C:\Program Files\WindowsApps\Claude_1.4.2.0_x64__pzs8sxrjxfjjc\app\Claude.exe",
            r"C:\Windows\explorer.exe",
        ]);
        assert_eq!(
            residence.container(),
            Some(&AppContainer {
                app: "Claude".into()
            })
        );
    }

    #[test]
    fn the_directory_name_is_matched_without_regard_to_case() {
        let residence =
            chain(&[r"c:\program files\windowsapps\OpenAI.Codex_26.1_x64__abc\codex.exe"]);
        assert_eq!(
            residence.container().map(|c| c.app.as_str()),
            Some("OpenAI.Codex")
        );
    }

    #[test]
    fn ordinary_ancestors_leave_the_process_regular() {
        assert_eq!(
            chain(&[
                r"C:\Windows\System32\cmd.exe",
                r"C:\Program Files\WindowsAppsTools\tool.exe",
                r"C:\Windows\explorer.exe",
            ]),
            Residence::Regular
        );
        assert_eq!(chain(&[]), Residence::Regular);
    }

    #[test]
    fn a_package_directory_without_a_name_is_called_an_app() {
        let bare = chain(&[r"C:\Program Files\WindowsApps\_x64\app.exe"]);
        assert_eq!(bare.container().map(|c| c.app.as_str()), Some(UNNAMED_APP));
        let truncated = chain(&[r"C:\Program Files\WindowsApps"]);
        assert_eq!(
            truncated.container().map(|c| c.app.as_str()),
            Some(UNNAMED_APP)
        );
    }

    #[test]
    fn only_the_first_sixteen_ancestors_are_considered() {
        let mut images = vec![r"C:\Windows\System32\cmd.exe"; MAX_ANCESTOR_DEPTH];
        images.push(r"C:\Program Files\WindowsApps\Claude_1_x64__id\Claude.exe");
        assert_eq!(chain(&images), Residence::Regular);
        images.remove(0);
        assert!(chain(&images).container().is_some());
    }

    #[test]
    fn the_report_names_the_registrations_and_the_command_with_the_full_path() {
        let layout = Layout::new(r"C:\Users\person\.ghostlight");
        let container = AppContainer {
            app: "Claude".into(),
        };
        let lines = kept_private_lines(
            &container,
            &["Google Chrome".to_owned(), "Codex".to_owned()],
            &layout,
        );
        assert_eq!(
            lines[0],
            "Windows keeps these registrations private to Claude, so no other app sees them: \
             Google Chrome, Codex."
        );
        assert!(lines[1].starts_with("To finish them, run this from a regular terminal: "));
        assert!(lines[1].ends_with(&format!(
            "{} install",
            layout.service_executable().display()
        )));
        assert!(lines[1].contains("ghostlight"));
        assert!(cannot_see_line(&container, &layout).contains("cannot see what other apps see"));
        assert!(uninstall_refusal(&container, &layout).ends_with("uninstall"));
    }
}
