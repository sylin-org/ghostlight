//! The context a harness is read on: the computer's roots, the person's environment, the
//! edge Ghostlight registers and the app container this process may run in.

use super::*;

/// The variable that names Codex's own directory, which holds its `config.toml`.
pub(in crate::install) const CODEX_HOME_VARIABLE: &str = "CODEX_HOME";
/// The variable that names Claude Code's own directory: Claude Code reads its [`CLAUDE_CODE_FILE`]
/// there in place of the person's home.
pub(in crate::install) const CLAUDE_CONFIG_VARIABLE: &str = "CLAUDE_CONFIG_DIR";
/// The file Claude Code keeps its user-scope MCP servers in, in its own directory or the home.
pub(super) const CLAUDE_CODE_FILE: &str = ".claude.json";
/// The variable that names Crush's whole configuration file, in place of its default.
pub(in crate::install) const CRUSH_CONFIG_VARIABLE: &str = "CRUSH_GLOBAL_CONFIG";

#[derive(Clone)]
pub(in crate::install) struct HarnessContext {
    pub(in crate::install) home: PathBuf,
    /// The directory agents keep their per-user settings in: `%APPDATA%` on Windows and
    /// `$XDG_CONFIG_HOME`, or `~/.config`, elsewhere.
    pub(in crate::install) config_root: PathBuf,
    pub(in crate::install) codex_config: PathBuf,
    /// The directory [`CLAUDE_CONFIG_VARIABLE`] names, when this process or the person's stored
    /// environment sets it ([`claude_config_dir`]): Claude Code reads its [`CLAUDE_CODE_FILE`]
    /// there in place of the home.
    pub(in crate::install) claude_config_dir: Option<PathBuf>,
    /// The file [`CRUSH_CONFIG_VARIABLE`] names, when it is set: Crush reads it in place of its
    /// default file.
    pub(in crate::install) crush_config: Option<PathBuf>,
    pub(in crate::install) edge: PathBuf,
    pub(in crate::install) path_entries: Vec<PathBuf>,
    pub(in crate::install) locations: PathBuf,
    /// Whether the computer runs Windows: paths compare ignoring case and programs end in `.exe`.
    pub(in crate::install) windows: bool,
    /// Whether the computer runs Linux, where Codex's entry forwards the desktop session
    /// ([`CODEX_DESKTOP_ENVIRONMENT`]).
    pub(in crate::install) linux: bool,
    /// The person's `%LOCALAPPDATA%` on Windows, below which each Store app keeps its private
    /// copy of the data roots ([`packaged`]); none elsewhere, where no copy is looked for.
    pub(in crate::install) local_app_data: Option<PathBuf>,
    /// Set when this process runs inside an app container: configuration below its `AppData`
    /// roots is that app's private view, which no other app reads.
    pub(in crate::install) private_view: Option<PrivateView>,
}

/// The app container this process runs in and the directories it redirects.
#[derive(Clone)]
pub(in crate::install) struct PrivateView {
    pub(in crate::install) container: AppContainer,
    pub(in crate::install) roots: Vec<PathBuf>,
}

impl HarnessContext {
    /// The app whose private view holds the configuration of `definition`, when this process
    /// writes there and the client is not that app itself: an app reads the very view it
    /// writes, so its own configuration is written like any other.
    pub(super) fn private_to(&self, definition: &ResolvedHarness) -> Option<&AppContainer> {
        let view = self.private_view.as_ref()?;
        if self.inside_own_app(definition.own_package) {
            return None;
        }
        view.roots
            .iter()
            .any(|root| paths::lies_within(&definition.path, root, self.windows))
            .then_some(&view.container)
    }

    /// Whether this process runs inside the container of `own_package`, the packaged app of a
    /// client, whose private view is the one that client reads.
    pub(super) fn inside_own_app(&self, own_package: Option<&str>) -> bool {
        self.private_view
            .as_ref()
            .is_some_and(|view| own_package.is_some_and(|package| view.container.is_app(package)))
    }

    pub(super) fn system() -> Self {
        let windows = cfg!(target_os = "windows");
        let dirs = UserDirs::system();
        let home = dirs.home().to_path_buf();
        let roots = environment_roots(&dirs, &|name| env::var_os(name), windows);
        let layout = layout::Layout::current();
        let edge = layout
            .as_ref()
            .map(layout::Layout::bin)
            .unwrap_or_default()
            .join(program_file_name(EDGE_COMMAND_NAME, windows));
        let path_entries = dirs.path_entries().to_vec();
        let locations = layout.map_or_else(
            |_| PathBuf::from(layout::HARNESS_LOCATIONS_NAME),
            |layout| layout.harness_locations(),
        );
        let local_app_data = windows.then(|| dirs.local());
        let private_view = local_app_data.as_ref().and_then(|local| {
            Residence::detect()
                .container()
                .map(|container| PrivateView {
                    container: container.clone(),
                    roots: vec![roots.config_root.clone(), local.clone()],
                })
        });
        Self {
            home,
            config_root: roots.config_root,
            codex_config: roots.codex_config,
            claude_config_dir: claude_config_dir(
                env::var_os(CLAUDE_CONFIG_VARIABLE),
                stored_variable(CLAUDE_CONFIG_VARIABLE),
                &|name| env::var(name).ok(),
            ),
            crush_config: user_dirs::directory(CRUSH_CONFIG_VARIABLE),
            edge,
            path_entries,
            locations,
            windows,
            linux: cfg!(target_os = "linux"),
            local_app_data,
            private_view,
        }
    }

    /// The Store packages of this look that keep a private copy of a data root: one listing on
    /// Windows, none elsewhere.
    pub(super) fn packages(&self) -> Packages {
        match &self.local_app_data {
            Some(local) if self.windows => Packages::listed(DataRoots {
                roaming: self.config_root.clone(),
                local: local.clone(),
            }),
            _ => Packages::none(),
        }
    }
}

/// Claude Code's own directory: the value of [`CLAUDE_CONFIG_VARIABLE`] in this process's
/// environment, else the one stored in the person's environment, expanded when it holds
/// references. A value that is empty counts as none, as it does for Claude Code.
///
/// The stored value is what every program the person starts receives. It matters to the
/// long-lived service, whose own environment is whatever its first starter passed on, which may
/// be a client that filters its child's environment.
pub(super) fn claude_config_dir(
    process: Option<OsString>,
    stored: Option<user_path::PathValue>,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    if let Some(directory) = process.filter(|directory| !directory.is_empty()) {
        return Some(PathBuf::from(directory));
    }
    let stored = stored?;
    let text = if stored.expandable {
        user_path::expand(&stored.text, lookup)
    } else {
        stored.text
    };
    (!text.is_empty()).then(|| PathBuf::from(text))
}

/// The variable `name` as the person's environment stores it, in the per-user registry. A value
/// that cannot be read counts as none.
#[cfg(windows)]
pub(super) fn stored_variable(name: &str) -> Option<user_path::PathValue> {
    user_path::Registry.variable(name).ok().flatten()
}

/// The variable `name` as the person's environment stores it: nowhere a process can read on
/// this system, where it lives in the person's shell startup files.
#[cfg(not(windows))]
pub(super) fn stored_variable(_name: &str) -> Option<user_path::PathValue> {
    None
}

/// The configuration roots of the person's directories `dirs`, with Codex's own directory where
/// `lookup` names one. A variable that is empty or relative names none ([`user_dirs`]), so no
/// client's file lands in the working directory.
pub(super) fn environment_roots(
    dirs: &UserDirs,
    lookup: user_dirs::Lookup<'_>,
    windows: bool,
) -> HarnessRoots {
    harness_roots(
        dirs.home(),
        Some(dirs.xdg_config()),
        Some(dirs.roaming()),
        user_dirs::directory_in(lookup, CODEX_HOME_VARIABLE),
        windows,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::install) struct HarnessRoots {
    /// The directory agents keep their per-user settings in: `%APPDATA%` on Windows and
    /// `$XDG_CONFIG_HOME`, or `~/.config`, elsewhere.
    pub(in crate::install) config_root: PathBuf,
    pub(in crate::install) codex_config: PathBuf,
}

pub(in crate::install) fn harness_roots(
    home: &Path,
    xdg_config_home: Option<PathBuf>,
    app_data: Option<PathBuf>,
    codex_home: Option<PathBuf>,
    windows: bool,
) -> HarnessRoots {
    let config_root = if windows {
        app_data.unwrap_or_else(|| home.join("AppData/Roaming"))
    } else {
        xdg_config_home.unwrap_or_else(|| home.join(".config"))
    };
    let codex_config = codex_home
        .unwrap_or_else(|| home.join(".codex"))
        .join("config.toml");
    HarnessRoots {
        config_root,
        codex_config,
    }
}
