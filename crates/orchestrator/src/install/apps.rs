//! Which catalog agent's app a running program belongs to, found by where the program is
//! installed, and which programs only start another.
//!
//! An agent's app is where the catalog says its program or Store app is (the [`IconSource`]s its
//! icon is read from) and where its commands are found on `PATH`. [`AppLocations::discover`] looks
//! for each of them on this computer on every call, with the same lookups the icon resolver uses:
//! a Store app is the install folders Windows names for its package families, a program below a
//! known folder is that program's own folder, and a command on `PATH` or the program a Linux
//! desktop entry starts is that one file. So a Store Claude Desktop is recognized by where Windows
//! installed it, never by a list of executable names. [`AppLocations::agent_of`] then says which
//! agent, if any, a program's path belongs to.
//!
//! [`is_wrapper`] names, in one place, the programs that start another and are never the app a
//! person knows: shells, consoles, script interpreters, runners and Ghostlight's own executables.
//! The icon resolver skips them in a launcher script, and the observed source of a connection
//! skips them among its ancestors ([`crate::provenance::Source`]).
//!
//! Everything here is a local read of where apps are installed; nothing is sent anywhere.

use std::fs;
use std::path::{Path, PathBuf};

use ghostlight_relay::home::{EDGE_EXECUTABLE_NAME, SERVICE_EXECUTABLE_NAME};

use super::entry::{lies_within, same_path};
use super::icons::{desktop_entry_value, program, read_text, IconPlaces, IconSource, ProgramPlace};

/// The programs that only start another, by file stem in lowercase: shells, consoles, script
/// interpreters, package runners and process wrappers. A stem compares without the `.exe` suffix
/// and without a trailing version (`python3.12` is `python`).
const WRAPPERS: &[&str] = &[
    "sh",
    "bash",
    "dash",
    "zsh",
    "fish",
    "ksh",
    "tcsh",
    "csh",
    "nu",
    "cmd",
    "powershell",
    "pwsh",
    "conhost",
    "openconsole",
    "wsl",
    "node",
    "npm",
    "npx",
    "pnpm",
    "yarn",
    "bun",
    "bunx",
    "deno",
    "python",
    "pythonw",
    "py",
    "uv",
    "uvx",
    "wscript",
    "cscript",
    "timeout",
    "env",
    "nohup",
    "setsid",
    "sudo",
    "doas",
    "script",
    "flatpak",
    "flatpak-spawn",
    "bwrap",
    "snap",
];

/// The separators a path may carry on either system.
pub const PATH_SEPARATORS: [char; 2] = ['/', '\\'];

/// The suffix of a Windows program, which a stem leaves out.
const EXE_SUFFIX: &str = ".exe";

/// The `[Desktop Entry]` key that names the program to check for before showing an app.
const TRY_EXEC_KEY: &str = "TryExec";

/// The `[Desktop Entry]` key whose first word is the program an app runs.
const EXEC_KEY: &str = "Exec";

/// A program's file stem as [`WRAPPERS`] compares it: what follows the last path separator of
/// either system, lowercase, without `.exe` and without a trailing version.
fn stem(file_name: &str) -> String {
    let last = file_name.rsplit(PATH_SEPARATORS).next().unwrap_or_default();
    let lower = last.to_ascii_lowercase();
    let bare = lower.strip_suffix(EXE_SUFFIX).unwrap_or(&lower);
    bare.trim_end_matches(|character: char| character.is_ascii_digit() || character == '.')
        .to_owned()
}

/// Whether the program at `path` only starts another: one of [`WRAPPERS`], or one of Ghostlight's
/// own executables.
#[must_use]
pub fn is_wrapper(path: &Path) -> bool {
    let stem = stem(&path.to_string_lossy());
    if stem.is_empty() {
        return false;
    }
    WRAPPERS.contains(&stem.as_str())
        || [EDGE_EXECUTABLE_NAME, SERVICE_EXECUTABLE_NAME]
            .iter()
            .any(|own| self::stem(own) == stem)
}

/// One catalog agent's app facts: where its program or Store app may be installed, and the
/// commands its client is started by.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentAppSources {
    /// The agent's catalog id.
    pub agent_id: &'static str,
    /// Where its program or Store app may be, in order: its hosts' icon sources in roster order.
    pub sources: Vec<IconSource>,
    /// The commands on `PATH` its hosts are started by, in roster order.
    pub commands: Vec<&'static str>,
}

/// Where one app is installed.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Location {
    /// A folder the app owns, which holds every program of the app.
    Folder(PathBuf),
    /// One program file, in a folder other programs may share.
    File(PathBuf),
}

impl Location {
    /// Whether the program at `image` is this location's.
    fn holds(&self, image: &Path) -> bool {
        match self {
            Self::Folder(folder) => lies_within(image, folder, cfg!(windows)),
            Self::File(file) => same_path(image, file),
        }
    }
}

/// Where each catalog agent's app is installed on one computer, as one look found it.
#[derive(Clone, Debug, Default)]
pub struct AppLocations {
    /// Each location with the catalog id of the agent whose app it is, in catalog order.
    apps: Vec<(&'static str, Location)>,
}

impl AppLocations {
    /// Look now for every catalog agent's app on the computer `places` describes.
    #[must_use]
    pub fn discover(places: &IconPlaces) -> Self {
        Self::of(places, &super::app_catalog())
    }

    /// Look now for the apps of `catalog` on the computer `places` describes.
    fn of(places: &IconPlaces, catalog: &[AgentAppSources]) -> Self {
        let mut apps = Vec::new();
        for agent in catalog {
            for source in &agent.sources {
                for location in locations(places, *source) {
                    apps.push((agent.agent_id, location));
                }
            }
            for command in &agent.commands {
                if let Some(file) = command_file(places, command) {
                    apps.push((agent.agent_id, Location::File(file)));
                }
            }
        }
        Self { apps }
    }

    /// The catalog id of the agent whose installed app holds the program at `image`, the first in
    /// catalog order; none for a program outside every agent's app.
    #[must_use]
    pub fn agent_of(&self, image: &Path) -> Option<&'static str> {
        self.apps
            .iter()
            .find(|(_, location)| location.holds(image))
            .map(|(agent_id, _)| *agent_id)
    }
}

/// Where `source` says an app is, as it is installed on the computer `places` describes.
fn locations(places: &IconPlaces, source: IconSource) -> Vec<Location> {
    match source {
        IconSource::WindowsExe(programs) if places.is_windows() => programs
            .iter()
            .filter_map(|place| {
                let found = program(places, *place)?;
                Some(match place {
                    ProgramPlace::Below(..) => Location::Folder(found.parent()?.to_path_buf()),
                    ProgramPlace::OnPath(_) => Location::File(found),
                })
            })
            .collect(),
        IconSource::WindowsPackage(name) if places.is_windows() => places
            .package_install_folders(name)
            .into_iter()
            .map(Location::Folder)
            .collect(),
        IconSource::LinuxDesktop(ids) if !places.is_windows() => ids
            .iter()
            .filter_map(|id| desktop_program(places, id))
            .map(Location::File)
            .collect(),
        // A source for the other system names nothing here.
        IconSource::WindowsPackage(_) | IconSource::WindowsExe(_) | IconSource::LinuxDesktop(_) => {
            Vec::new()
        }
    }
}

/// The program file the command `name` runs on the computer `places` describes: on Windows its
/// `.exe` or the program its launcher starts, elsewhere the file of that name on `PATH` with its
/// links resolved. None when it is not found or is a wrapper.
fn command_file(places: &IconPlaces, name: &'static str) -> Option<PathBuf> {
    let found = if places.is_windows() {
        program(places, ProgramPlace::OnPath(name))?
    } else {
        resolved(
            places
                .path_entries()
                .iter()
                .map(|directory| directory.join(name))
                .find(|candidate| candidate.is_file())?,
        )?
    };
    (!is_wrapper(&found)).then_some(found)
}

/// The program the Linux desktop entry `id` starts: its `TryExec`, else the first word of its
/// `Exec`, found on `PATH` when it is a bare name, with its links resolved. None when the entry
/// starts it through a wrapper (`flatpak run`, `env`, a shell), whose file every such app shares.
fn desktop_program(places: &IconPlaces, id: &str) -> Option<PathBuf> {
    let text = read_text(&places.desktop_entry(id)?).ok()??;
    let named = desktop_entry_value(&text, TRY_EXEC_KEY).or_else(|| {
        desktop_entry_value(&text, EXEC_KEY)?
            .split_whitespace()
            .next()
    })?;
    let named = named.trim_matches('"');
    let path = Path::new(named);
    let found = if path.is_absolute() {
        path.to_path_buf()
    } else {
        places
            .path_entries()
            .iter()
            .map(|directory| directory.join(named))
            .find(|candidate| candidate.is_file())?
    };
    let found = resolved(found)?;
    (!is_wrapper(&found)).then_some(found)
}

/// `path` with every link resolved, when it is a file.
fn resolved(path: PathBuf) -> Option<PathBuf> {
    let file = fs::canonicalize(path).ok()?;
    file.is_file().then_some(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::icons::KnownFolder;
    use crate::scratch::Scratch;

    /// An empty file at `path`, with its folders.
    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn wrappers_are_named_by_their_stem_whatever_the_suffix_case_or_version() {
        for wrapper in [
            "/usr/bin/bash",
            "/usr/bin/python3.12",
            r"C:\Windows\System32\cmd.exe",
            r"C:\Program Files\PowerShell\7\pwsh.EXE",
            r"C:\Windows\System32\conhost.exe",
            r"C:\Windows\System32\timeout.exe",
            r"C:\Program Files\nodejs\node.exe",
            "/home/ann/.ghostlight/bin/ghostlight",
            r"C:\Users\ann\.ghostlight\bin\ghostlight-service.exe",
        ] {
            assert!(is_wrapper(Path::new(wrapper)), "{wrapper}");
        }
        for app in [
            r"C:\Program Files\WindowsApps\Claude_1.0.0.0_x64__pzs8sxrjxfjjc\app\claude.exe",
            "/usr/share/code/code",
            r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.0_x64__8wekyb3d8bbwe\WindowsTerminal.exe",
            "/usr/bin/gnome-terminal-server",
        ] {
            assert!(!is_wrapper(Path::new(app)), "{app}");
        }
    }

    #[test]
    fn a_program_below_a_known_folder_names_its_whole_folder_and_a_command_names_one_file() {
        let scratch = Scratch::named("app-locations");
        let local = scratch.path().join("local");
        let bin = scratch.path().join("bin");
        touch(&local.join("Programs/cursor/Cursor.exe"));
        touch(&bin.join("claude.exe"));
        touch(&bin.join("other.exe"));
        let places =
            IconPlaces::windows(local.clone(), scratch.path().join("pf"), vec![bin.clone()]);
        let catalog = [
            AgentAppSources {
                agent_id: "cursor",
                sources: vec![IconSource::WindowsExe(&[ProgramPlace::Below(
                    KnownFolder::LocalAppData,
                    "Programs/cursor/Cursor.exe",
                )])],
                commands: Vec::new(),
            },
            AgentAppSources {
                agent_id: "claude-code",
                sources: Vec::new(),
                commands: vec!["claude", "absent"],
            },
        ];
        let apps = AppLocations::of(&places, &catalog);
        assert_eq!(
            apps.agent_of(&local.join("Programs/cursor/resources/helper/Cursor Helper.exe")),
            Some("cursor"),
            "every program in the app's own folder is the app's"
        );
        assert_eq!(apps.agent_of(&bin.join("claude.exe")), Some("claude-code"));
        assert_eq!(
            apps.agent_of(&bin.join("other.exe")),
            None,
            "a command names its file, not the folder it shares"
        );
        assert_eq!(apps.agent_of(&local.join("Programs/zed/Zed.exe")), None);
    }

    #[test]
    fn a_store_app_is_recognized_by_where_its_package_is_installed_not_by_its_file_name() {
        let scratch = Scratch::named("app-locations-store");
        let local = scratch.path().join("local");
        fs::create_dir_all(local.join("Packages/Claude_pzs8sxrjxfjjc")).unwrap();
        let mut places = IconPlaces::windows(local, scratch.path().join("pf"), Vec::new());
        fn installed(family: &str) -> Vec<PathBuf> {
            (family == "Claude_pzs8sxrjxfjjc")
                .then(|| {
                    PathBuf::from(r"C:\Program Files\WindowsApps\Claude_1.0.0.0_x64__pzs8sxrjxfjjc")
                })
                .into_iter()
                .collect()
        }
        places.set_package_folders(installed);
        let catalog = [AgentAppSources {
            agent_id: "claude-desktop",
            sources: vec![IconSource::WindowsPackage("Claude")],
            commands: Vec::new(),
        }];
        let apps = AppLocations::of(&places, &catalog);
        let packaged =
            r"C:\Program Files\WindowsApps\Claude_1.0.0.0_x64__pzs8sxrjxfjjc\app\claude.exe";
        if cfg!(windows) {
            assert_eq!(apps.agent_of(Path::new(packaged)), Some("claude-desktop"));
            assert_eq!(
                apps.agent_of(Path::new(&format!(r"\\?\{}", packaged.to_uppercase()))),
                Some("claude-desktop"),
                "the verbatim form and the case do not matter on Windows"
            );
        }
        assert_eq!(
            apps.agent_of(Path::new(r"C:\Users\ann\Downloads\claude.exe")),
            None,
            "the same file name elsewhere is not the app"
        );
    }

    #[test]
    fn a_linux_desktop_entry_names_the_program_it_starts_and_never_a_shared_launcher() {
        let scratch = Scratch::named("app-locations-linux");
        let share = scratch.path().join("share");
        let code = scratch.path().join("opt/code/code");
        touch(&code);
        let flatpak = scratch.path().join("bin/flatpak");
        touch(&flatpak);
        fs::create_dir_all(share.join("applications")).unwrap();
        fs::write(
            share.join("applications/code.desktop"),
            format!(
                "[Desktop Entry]\nName=Code\nExec={} --unity-launch %F\n",
                code.display()
            ),
        )
        .unwrap();
        fs::write(
            share.join("applications/zed.desktop"),
            format!(
                "[Desktop Entry]\nExec={} run dev.zed.Zed\n",
                flatpak.display()
            ),
        )
        .unwrap();
        let places = IconPlaces::linux(vec![share]);
        let catalog = [
            AgentAppSources {
                agent_id: "vscode",
                sources: vec![IconSource::LinuxDesktop(&["code"])],
                commands: Vec::new(),
            },
            AgentAppSources {
                agent_id: "zed",
                sources: vec![IconSource::LinuxDesktop(&["zed"])],
                commands: Vec::new(),
            },
        ];
        let apps = AppLocations::of(&places, &catalog);
        assert_eq!(
            apps.agent_of(&fs::canonicalize(&code).unwrap()),
            Some("vscode")
        );
        assert_eq!(apps.agent_of(&fs::canonicalize(&flatpak).unwrap()), None);
    }
}
