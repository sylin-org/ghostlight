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
//! Detection has two steps, because a packaged ancestor alone proves nothing: Windows Terminal,
//! Store PowerShell and other packaged apps run in the open. The parent chain names the packaged
//! apps the process descends from, meaning images directly below a package directory of
//! `WindowsApps` (`<drive>:\Program Files\WindowsApps\<package>\...`). The process is inside the
//! container of the first such app whose private copy of `%LOCALAPPDATA%` holds a probe file this
//! process just wrote, which is the effect that matters. Nothing here starts a process or reads
//! the registry.

use std::cell::OnceCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;

use super::layout::Layout;
use super::packaged::{private_root, DataRoot};

/// How many ancestors the detection walks before it stops.
pub const MAX_ANCESTOR_DEPTH: usize = 16;

/// The directory below which Windows keeps the files of packaged apps.
const PACKAGE_ROOT_DIRECTORY: &str = "WindowsApps";

/// The directory that holds [`PACKAGE_ROOT_DIRECTORY`] on the system drive.
const PROGRAM_FILES_DIRECTORY: &str = "Program Files";

/// Separates the fields of a package directory such as `Claude_1.2.3_x64__id`: name, version,
/// architecture, resource id and publisher id. A package family name, `Claude_id`, is the first
/// and the last.
pub(super) const PACKAGE_FIELD_SEPARATOR: char = '_';

/// The prefix of the probe file the detection writes below `%LOCALAPPDATA%`.
const PROBE_PREFIX: &str = "ghostlight-residence-probe-";

/// What `doctor` says of a registration whose state only another app can see.
pub const NOT_VISIBLE: &str = "cannot be checked from here";

/// The subcommand of the installed service that registers browsers and clients.
const FINISHING_SUBCOMMAND: &str = "install";

/// The app whose container this process runs in.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AppContainer {
    /// The app's package name, as its package directory spells it.
    pub app: String,
}

impl AppContainer {
    /// Whether this container belongs to the app with package name `package`.
    #[must_use]
    pub fn is_app(&self, package: &str) -> bool {
        self.app.eq_ignore_ascii_case(package)
    }
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
    /// Where this process runs, decided once from its parent chain and a probe write.
    #[must_use]
    pub fn detect() -> Self {
        static DETECTED: OnceLock<Residence> = OnceLock::new();
        DETECTED
            .get_or_init(|| {
                let probe = OnceCell::new();
                Self::from_ancestors(
                    &ghostlight_win_peer::ancestor_images(MAX_ANCESTOR_DEPTH),
                    |family| {
                        probe
                            .get_or_init(Probe::write)
                            .as_ref()
                            .is_some_and(|probe| probe.landed_privately(family))
                    },
                )
            })
            .clone()
    }

    /// Where a process runs whose ancestors' executables are `images`, nearest first.
    ///
    /// `redirected` answers, for a package family name, whether this process's `%LOCALAPPDATA%`
    /// writes land in that app's private copy. It is asked only about packaged ancestors.
    #[must_use]
    pub fn from_ancestors<P: AsRef<Path>>(images: &[P], redirected: impl Fn(&str) -> bool) -> Self {
        images
            .iter()
            .take(MAX_ANCESTOR_DEPTH)
            .filter_map(|image| Package::of(image.as_ref()))
            .find(|package| redirected(&package.family))
            .map_or(Self::Regular, |package| {
                Self::AppContainer(AppContainer { app: package.name })
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

/// A packaged app named by the package directory its image lies in.
struct Package {
    /// The package name, such as `Claude`.
    name: String,
    /// The package family name, `<name>_<publisher id>`, which names the app's private data.
    family: String,
}

impl Package {
    /// The package whose directory holds `image`, when that directory is a package root.
    ///
    /// Only `<drive>:\Program Files\WindowsApps\<package>\...` and `<drive>:\WindowsApps\<package>\...`
    /// qualify. Any other directory called `WindowsApps`, such as the per-user
    /// `%LOCALAPPDATA%\Microsoft\WindowsApps` of app execution aliases, does not.
    fn of(image: &Path) -> Option<Self> {
        // Windows paths are split by hand so the answer does not depend on the host operating system.
        let text = image.to_string_lossy();
        let mut components = text.split(['\\', '/']).filter(|part| !part.is_empty());
        let drive = components.next()?;
        if !(drive.len() == 2 && drive.ends_with(':')) {
            return None;
        }
        let mut directory = components.next()?;
        if directory.eq_ignore_ascii_case(PROGRAM_FILES_DIRECTORY) {
            directory = components.next()?;
        }
        if !directory.eq_ignore_ascii_case(PACKAGE_ROOT_DIRECTORY) {
            return None;
        }
        let package: Vec<&str> = components.next()?.split(PACKAGE_FIELD_SEPARATOR).collect();
        let [name, _version, _architecture, _resource, publisher] = package[..] else {
            return None;
        };
        (!name.is_empty() && !publisher.is_empty()).then(|| Self {
            name: name.to_owned(),
            family: format!("{name}{PACKAGE_FIELD_SEPARATOR}{publisher}"),
        })
    }
}

/// A file written below `%LOCALAPPDATA%` to see whether Windows redirects the write. It is
/// removed when dropped.
struct Probe {
    local_data: PathBuf,
    name: String,
}

impl Probe {
    /// Write a probe below this user's `%LOCALAPPDATA%`.
    fn write() -> Option<Self> {
        Self::write_in(PathBuf::from(std::env::var_os("LOCALAPPDATA")?))
    }

    /// Write a probe below `local_data`.
    fn write_in(local_data: PathBuf) -> Option<Self> {
        let name = format!("{PROBE_PREFIX}{}", std::process::id());
        fs::write(local_data.join(&name), []).ok()?;
        Some(Self { local_data, name })
    }

    /// Whether the probe is also in the private copy of `%LOCALAPPDATA%` of package family
    /// `family`, which is where a redirected write goes.
    fn landed_privately(&self, family: &str) -> bool {
        private_root(&self.local_data, family, DataRoot::Local)
            .join(&self.name)
            .is_file()
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.local_data.join(&self.name));
    }
}

/// The command that finishes the private registrations from a regular terminal.
#[must_use]
pub fn finishing_command(layout: &Layout) -> String {
    format!("{} {FINISHING_SUBCOMMAND}", layout.edge().display())
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
        layout.edge().display()
    )
}

/// The line `doctor` prints for an MCP client whose configuration only another app can see.
#[must_use]
pub fn client_cannot_see_line(client: &str) -> String {
    format!(
        "MCP client: {client} -- {NOT_VISIBLE} -- Windows gives the app this terminal runs in \
         its own view of this configuration."
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
        layout.edge().display()
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::scratch::Scratch;

    const CLAUDE_IMAGE: &str =
        r"C:\Program Files\WindowsApps\Claude_1.4.2.0_x64__pzs8sxrjxfjjc\app\Claude.exe";
    const TERMINAL_IMAGE: &str = r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.24.12741.0_x64__8wekyb3d8bbwe\WindowsTerminal.exe";

    fn chain(images: &[&str], redirected: &[&str]) -> Residence {
        Residence::from_ancestors(
            &images.iter().map(PathBuf::from).collect::<Vec<_>>(),
            |family| redirected.contains(&family),
        )
    }

    fn temporary(name: &str) -> Scratch {
        Scratch::named(&format!("container-{name}"))
    }

    #[test]
    fn an_ancestor_whose_package_redirects_puts_the_process_in_its_container() {
        let residence = chain(
            &[
                r"C:\Windows\System32\cmd.exe",
                CLAUDE_IMAGE,
                r"C:\Windows\explorer.exe",
            ],
            &["Claude_pzs8sxrjxfjjc"],
        );
        assert_eq!(
            residence.container(),
            Some(&AppContainer {
                app: "Claude".into()
            })
        );
    }

    #[test]
    fn the_directory_names_are_matched_without_regard_to_case() {
        let residence = chain(
            &[r"c:\program files\windowsapps\OpenAI.Codex_26.1_x64__abc\codex.exe"],
            &["OpenAI.Codex_abc"],
        );
        assert_eq!(
            residence.container().map(|c| c.app.as_str()),
            Some("OpenAI.Codex")
        );
    }

    #[test]
    fn a_packaged_terminal_whose_writes_are_not_redirected_is_regular() {
        let store_powershell = r"C:\Program Files\WindowsApps\Microsoft.PowerShell_7.5.0.0_x64__8wekyb3d8bbwe\pwsh.exe";
        assert_eq!(
            chain(
                &[store_powershell, TERMINAL_IMAGE, r"C:\Windows\explorer.exe"],
                &[]
            ),
            Residence::Regular
        );
    }

    #[test]
    fn the_first_ancestor_that_redirects_decides_the_app() {
        let residence = chain(&[TERMINAL_IMAGE, CLAUDE_IMAGE], &["Claude_pzs8sxrjxfjjc"]);
        assert!(residence.container().is_some_and(|c| c.is_app("claude")));
    }

    #[test]
    fn only_a_package_root_below_windows_apps_counts() {
        // The redirection answer would say yes to anything, so only the path can keep these out.
        let always = |images: &[&str]| {
            Residence::from_ancestors(
                &images.iter().map(PathBuf::from).collect::<Vec<_>>(),
                |_| true,
            )
        };
        assert_eq!(
            always(&[
                r"C:\Users\person\AppData\Local\Microsoft\WindowsApps\pwsh.exe",
                r"C:\Program Files\WindowsAppsTools\tool.exe",
                r"C:\Program Files\Tool\WindowsApps\Claude_1_x64__id\Claude.exe",
                r"C:\Program Files\WindowsApps\_1_x64__id\app.exe",
                r"C:\Program Files\WindowsApps\Claude\app.exe",
                r"C:\Windows\System32\cmd.exe",
            ]),
            Residence::Regular
        );
        assert_eq!(always(&[]), Residence::Regular);
        assert!(always(&[r"D:\WindowsApps\Claude_1_x64__id\Claude.exe"])
            .container()
            .is_some());
    }

    #[test]
    fn only_the_first_sixteen_ancestors_are_considered() {
        let mut images = vec![r"C:\Windows\System32\cmd.exe"; MAX_ANCESTOR_DEPTH];
        images.push(CLAUDE_IMAGE);
        let redirected = ["Claude_pzs8sxrjxfjjc"];
        assert_eq!(chain(&images, &redirected), Residence::Regular);
        images.remove(0);
        assert!(chain(&images, &redirected).container().is_some());
    }

    #[test]
    fn a_probe_counts_only_when_it_also_lands_in_the_packages_private_copy() {
        let local = temporary("probe");
        let probe = Probe::write_in(local.to_path_buf()).unwrap();
        let file = local.join(&probe.name);
        assert!(file.is_file());
        assert!(!probe.landed_privately("Claude_pzs8sxrjxfjjc"));

        let private = local
            .join("Packages")
            .join("Claude_pzs8sxrjxfjjc")
            .join("LocalCache")
            .join("Local");
        fs::create_dir_all(&private).unwrap();
        fs::write(private.join(&probe.name), []).unwrap();
        assert!(probe.landed_privately("Claude_pzs8sxrjxfjjc"));
        assert!(!probe.landed_privately("Microsoft.WindowsTerminal_8wekyb3d8bbwe"));

        drop(probe);
        assert!(!file.exists());
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
        assert!(lines[1].ends_with(&format!("{} install", layout.edge().display())));
        assert!(lines[1].contains("ghostlight"));
        assert!(cannot_see_line(&container, &layout).contains("cannot see what other apps see"));
        assert!(uninstall_refusal(&container, &layout).ends_with("uninstall"));
    }

    #[test]
    fn the_client_line_reads_as_one_sentence() {
        let line = client_cannot_see_line("Claude Desktop");
        assert_eq!(
            line,
            "MCP client: Claude Desktop -- cannot be checked from here -- Windows gives the app \
             this terminal runs in its own view of this configuration."
        );
        assert!(!line.contains("  "));
    }
}
