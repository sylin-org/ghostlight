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
//! The container is found by its effect alone, never by the processes above this one, because a
//! wrapper such as `timeout` or a pipeline breaks a parent chain while the container holds. The
//! detection writes a probe file below `%LOCALAPPDATA%` and lists its `Packages` folder once: the
//! process is inside the container of the package whose private copy of `%LOCALAPPDATA%` also
//! holds the probe. Windows shows a process inside a container only the merged view of `HKCU`,
//! `HKEY_USERS` included, and keeps the app's private registry hive locked, so the registry view
//! is private exactly when the probe lands privately (ADR-0197 amendment, 2026-10-18). Nothing
//! here starts a process or reads the registry.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Serialize;
use uuid::Uuid;

use super::layout::Layout;
use super::packaged::{families, private_root, DataRoot};

/// Separates the fields of a package directory such as `Claude_1.2.3_x64__id`: name, version,
/// architecture, resource id and publisher id. A package family name, `Claude_id`, is the first
/// and the last.
pub(super) const PACKAGE_FIELD_SEPARATOR: char = '_';

/// The prefix of the probe file the detection writes below `%LOCALAPPDATA%`.
const PROBE_PREFIX: &str = "ghostlight-residence-probe-";

/// The variable that names this person's `%LOCALAPPDATA%`.
const LOCAL_APP_DATA_VARIABLE: &str = "LOCALAPPDATA";

/// The state `doctor --json` gives a registration whose state only another app can see.
pub const NOT_VISIBLE_STATE: &str = "not_visible";

/// The subcommand of the installed service that registers browsers and clients.
const FINISHING_SUBCOMMAND: &str = "install";

/// The subcommand of the installed service that checks every registration.
const CHECKING_SUBCOMMAND: &str = "doctor";

/// The subcommand of the installed service that removes every registration.
const REMOVING_SUBCOMMAND: &str = "uninstall";

/// The app whose container this process runs in.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AppContainer {
    /// The app's package name, as its package family name spells it.
    pub app: String,
}

impl AppContainer {
    /// The container of the package family `family`, `<name>_<publisher id>`.
    fn of_family(family: &str) -> Self {
        let app = family
            .rsplit_once(PACKAGE_FIELD_SEPARATOR)
            .map_or(family, |(name, _publisher)| name);
        Self {
            app: app.to_owned(),
        }
    }

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
    /// Where this process runs, decided once from a probe write. Only Windows has app containers.
    #[must_use]
    pub fn detect() -> Self {
        static DETECTED: OnceLock<Residence> = OnceLock::new();
        DETECTED
            .get_or_init(|| {
                if !cfg!(target_os = "windows") {
                    return Self::Regular;
                }
                std::env::var_os(LOCAL_APP_DATA_VARIABLE).map_or(Self::Regular, |local| {
                    Probe::write_in(PathBuf::from(local))
                        .as_ref()
                        .map_or(Self::Regular, Self::of_probe)
                })
            })
            .clone()
    }

    /// Where a process runs that wrote `probe`: inside the container of the package whose private
    /// copy of `%LOCALAPPDATA%` holds it too, or in the open when none does.
    fn of_probe(probe: &Probe) -> Self {
        probe.private_family().map_or(Self::Regular, |family| {
            Self::AppContainer(AppContainer::of_family(&family))
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

/// A file written below `%LOCALAPPDATA%` to see whether Windows redirects the write. Its name is
/// new on every write, so a probe an earlier process left behind is never mistaken for this one.
/// It is removed when dropped.
struct Probe {
    local_data: PathBuf,
    name: String,
}

impl Probe {
    /// Write a probe below `local_data`.
    fn write_in(local_data: PathBuf) -> Option<Self> {
        let name = format!("{PROBE_PREFIX}{}", Uuid::new_v4().simple());
        fs::write(local_data.join(&name), []).ok()?;
        Some(Self { local_data, name })
    }

    /// The package family whose private copy of `%LOCALAPPDATA%` also holds the probe, which is
    /// where a redirected write goes, from one listing of the installed packages.
    fn private_family(&self) -> Option<String> {
        families(&self.local_data)
            .into_iter()
            .find(|family| self.landed_in(family))
    }

    /// Whether the probe is also in the private copy of `%LOCALAPPDATA%` of package family
    /// `family`.
    fn landed_in(&self, family: &str) -> bool {
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

/// The command that checks the real registrations from a regular terminal.
#[must_use]
pub fn checking_command(layout: &Layout) -> String {
    format!("{} {CHECKING_SUBCOMMAND}", layout.edge().display())
}

/// The command that removes the real registrations from a regular terminal.
#[must_use]
pub fn removing_command(layout: &Layout) -> String {
    format!("{} {REMOVING_SUBCOMMAND}", layout.edge().display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    const CLAUDE_FAMILY: &str = "Claude_pzs8sxrjxfjjc";

    fn temporary(name: &str) -> Scratch {
        Scratch::named(&format!("container-{name}"))
    }

    /// Give `family` a private copy of `%LOCALAPPDATA%` below `local`, and return it.
    fn private_copy(local: &std::path::Path, family: &str) -> PathBuf {
        let private = private_root(local, family, DataRoot::Local);
        fs::create_dir_all(&private).unwrap();
        private
    }

    #[test]
    fn a_probe_in_any_packages_private_copy_decides_the_container_without_a_parent_chain() {
        // Nothing names Claude but the redirected write: an agent's `timeout` or pipeline between
        // Claude and this process changes nothing.
        let local = temporary("any-package");
        private_copy(&local, "Microsoft.WindowsTerminal_8wekyb3d8bbwe");
        let claude = private_copy(&local, CLAUDE_FAMILY);
        private_copy(&local, "OpenAI.Codex_2p2nqsd0c76g0");
        let probe = Probe::write_in(local.to_path_buf()).unwrap();
        assert_eq!(Residence::of_probe(&probe), Residence::Regular);

        fs::write(claude.join(&probe.name), []).unwrap();
        assert_eq!(
            Residence::of_probe(&probe),
            Residence::AppContainer(AppContainer {
                app: "Claude".into()
            })
        );
    }

    #[test]
    fn a_folder_that_is_no_package_family_never_counts() {
        let local = temporary("not-a-family");
        let probe = Probe::write_in(local.to_path_buf()).unwrap();
        for folder in ["NotAFamily", "_pzs8sxrjxfjjc", "Claude_pzs-8"] {
            fs::write(private_copy(&local, folder).join(&probe.name), []).unwrap();
        }
        assert_eq!(Residence::of_probe(&probe), Residence::Regular);
    }

    #[test]
    fn without_a_packages_folder_the_process_is_regular() {
        let local = temporary("no-packages");
        let probe = Probe::write_in(local.to_path_buf()).unwrap();
        assert_eq!(Residence::of_probe(&probe), Residence::Regular);
    }

    #[test]
    fn every_probe_has_a_new_name_and_is_removed_when_dropped() {
        let local = temporary("probe");
        let first = Probe::write_in(local.to_path_buf()).unwrap();
        let second = Probe::write_in(local.to_path_buf()).unwrap();
        assert_ne!(first.name, second.name);
        let file = local.join(&first.name);
        assert!(file.is_file());
        drop(first);
        assert!(!file.exists());
    }

    #[test]
    fn the_app_is_the_family_name_without_its_publisher_id() {
        assert_eq!(AppContainer::of_family(CLAUDE_FAMILY).app, "Claude");
        let codex = AppContainer::of_family("OpenAI.Codex_2p2nqsd0c76g0");
        assert_eq!(codex.app, "OpenAI.Codex");
        assert!(codex.is_app("openai.codex"));
    }
}
