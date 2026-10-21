//! The person's directories as the environment names them, resolved once under one rule.
//!
//! The installer finds every client configuration, native-host manifest, desktop entry, icon
//! source and Store container below directories that environment variables name. Each resolver
//! used to read them on its own, under rules that drifted apart. [`UserDirs`] reads them once,
//! under the rule the Ghostlight home already follows (`ghostlight_relay::home`): the home is
//! `USERPROFILE` on Windows and `HOME` elsewhere, and a variable names a directory only when it is
//! set, not empty and absolute. An empty value taken as a path is the working directory, and a
//! relative one depends on it, so an install would write a client's file there; the XDG
//! base-directory rules ask for both to be ignored. A variable that names nothing leaves the
//! profile-relative default in place.
//!
//! The getters are platform-neutral, and each resolver picks the ones its platform reads. The
//! variables a single client defines for its own file (such as `CODEX_HOME`) stay with that
//! client's resolver and are read through [`directory_in`] under the same rule.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The Windows variable that names the person's home.
pub(super) const USER_PROFILE_VARIABLE: &str = "USERPROFILE";
/// The variable that names the person's home everywhere but Windows.
pub(super) const HOME_VARIABLE: &str = "HOME";
/// The Windows variable that names the roaming application data directory.
pub(super) const APP_DATA_VARIABLE: &str = "APPDATA";
/// The Windows variable that names the local application data directory.
pub(super) const LOCAL_APP_DATA_VARIABLE: &str = "LOCALAPPDATA";
/// The variable that names the configuration directory (XDG base directories).
pub(super) const XDG_CONFIG_HOME_VARIABLE: &str = "XDG_CONFIG_HOME";
/// The variable that names the data directory (XDG base directories).
pub(super) const XDG_DATA_HOME_VARIABLE: &str = "XDG_DATA_HOME";
/// The Windows variables that name the program folders, the native one first.
const PROGRAM_FILES_VARIABLES: [&str; 2] = ["ProgramFiles", "ProgramFiles(x86)"];
/// The variable that names the program search path.
pub(super) const PATH_VARIABLE: &str = "PATH";

/// The roaming application data directory below the home, where `APPDATA` names none.
const ROAMING_DEFAULT: [&str; 2] = ["AppData", "Roaming"];
/// The local application data directory below the home, where `LOCALAPPDATA` names none.
const LOCAL_DEFAULT: [&str; 2] = ["AppData", "Local"];
/// The configuration directory below the home, where `XDG_CONFIG_HOME` names none.
const XDG_CONFIG_DEFAULT: [&str; 1] = [".config"];
/// The data directory below the home, where `XDG_DATA_HOME` names none.
const XDG_DATA_DEFAULT: [&str; 2] = [".local", "share"];

/// How a resolver reads one environment variable by name: the process environment, or a fixed
/// set in a test.
pub(super) type Lookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The directory `lookup`'s variable `name` names: none when it is unset, empty or relative.
#[must_use]
pub(super) fn directory_in(lookup: Lookup<'_>, name: &str) -> Option<PathBuf> {
    lookup(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// The directory the process environment's variable `name` names, under [`directory_in`]'s rule.
#[must_use]
pub(super) fn directory(name: &str) -> Option<PathBuf> {
    directory_in(&|name| env::var_os(name), name)
}

/// The person's directories, read from the environment once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct UserDirs {
    home: PathBuf,
    app_data: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    xdg_data_home: Option<PathBuf>,
    program_files: Vec<PathBuf>,
    path_entries: Vec<PathBuf>,
}

impl UserDirs {
    /// This process's directories.
    #[must_use]
    pub(super) fn system() -> Self {
        Self::from_lookup(&|name| env::var_os(name), cfg!(windows))
    }

    /// The directories `lookup` names, on Windows when `windows`. A home that no variable names
    /// is empty, as each resolver took it before.
    #[must_use]
    pub(super) fn from_lookup(lookup: Lookup<'_>, windows: bool) -> Self {
        let home_variable = if windows {
            USER_PROFILE_VARIABLE
        } else {
            HOME_VARIABLE
        };
        let mut program_files = Vec::new();
        for variable in PROGRAM_FILES_VARIABLES {
            if let Some(folder) = directory_in(lookup, variable) {
                if !program_files.contains(&folder) {
                    program_files.push(folder);
                }
            }
        }
        Self {
            home: directory_in(lookup, home_variable).unwrap_or_default(),
            app_data: directory_in(lookup, APP_DATA_VARIABLE),
            local_app_data: directory_in(lookup, LOCAL_APP_DATA_VARIABLE),
            xdg_config_home: directory_in(lookup, XDG_CONFIG_HOME_VARIABLE),
            xdg_data_home: directory_in(lookup, XDG_DATA_HOME_VARIABLE),
            program_files,
            path_entries: lookup(PATH_VARIABLE)
                .map(|path| env::split_paths(&path).collect())
                .unwrap_or_default(),
        }
    }

    /// The person's home.
    #[must_use]
    pub(super) fn home(&self) -> &Path {
        &self.home
    }

    /// The roaming application data directory: `APPDATA`, else `home/AppData/Roaming`.
    #[must_use]
    pub(super) fn roaming(&self) -> PathBuf {
        self.or_below_home(self.app_data.as_deref(), &ROAMING_DEFAULT)
    }

    /// The roaming application data directory only when `APPDATA` names it.
    #[must_use]
    pub(super) fn named_roaming(&self) -> Option<&Path> {
        self.app_data.as_deref()
    }

    /// The local application data directory: `LOCALAPPDATA`, else `home/AppData/Local`.
    #[must_use]
    pub(super) fn local(&self) -> PathBuf {
        self.or_below_home(self.local_app_data.as_deref(), &LOCAL_DEFAULT)
    }

    /// The local application data directory only when `LOCALAPPDATA` names it.
    #[must_use]
    pub(super) fn named_local(&self) -> Option<&Path> {
        self.local_app_data.as_deref()
    }

    /// The configuration directory: `XDG_CONFIG_HOME`, else `home/.config`.
    #[must_use]
    pub(super) fn xdg_config(&self) -> PathBuf {
        self.or_below_home(self.xdg_config_home.as_deref(), &XDG_CONFIG_DEFAULT)
    }

    /// The data directory: `XDG_DATA_HOME`, else `home/.local/share`.
    #[must_use]
    pub(super) fn xdg_data(&self) -> PathBuf {
        self.or_below_home(self.xdg_data_home.as_deref(), &XDG_DATA_DEFAULT)
    }

    /// The data directory only when `XDG_DATA_HOME` names it.
    #[must_use]
    pub(super) fn named_xdg_data(&self) -> Option<&Path> {
        self.xdg_data_home.as_deref()
    }

    /// The program folders the environment names, the native one first, each once.
    #[must_use]
    pub(super) fn program_files(&self) -> &[PathBuf] {
        &self.program_files
    }

    /// The program search path.
    #[must_use]
    pub(super) fn path_entries(&self) -> &[PathBuf] {
        &self.path_entries
    }

    fn or_below_home(&self, named: Option<&Path>, default: &[&str]) -> PathBuf {
        named.map_or_else(
            || {
                default
                    .iter()
                    .fold(self.home.clone(), |path, part| path.join(part))
            },
            Path::to_path_buf,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lookup over fixed `variables`.
    fn set(variables: Vec<(&'static str, String)>) -> impl Fn(&str) -> Option<OsString> {
        move |name| {
            variables
                .iter()
                .find(|(set, _)| *set == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    /// An absolute path on this system.
    fn absolute(path: &str) -> String {
        if cfg!(windows) {
            format!("C:{path}")
        } else {
            path.to_owned()
        }
    }

    #[test]
    fn a_set_absolute_variable_names_its_directory() {
        let codex = absolute("/work/codex");
        assert_eq!(
            directory_in(&set(vec![("CODEX_HOME", codex.clone())]), "CODEX_HOME"),
            Some(PathBuf::from(codex))
        );
    }

    #[test]
    fn an_empty_variable_names_no_directory() {
        let lookup = set(vec![("CODEX_HOME", String::new())]);
        assert_eq!(directory_in(&lookup, "CODEX_HOME"), None);
    }

    #[test]
    fn a_relative_variable_names_no_directory() {
        let lookup = set(vec![("CODEX_HOME", "codex".to_owned())]);
        assert_eq!(directory_in(&lookup, "CODEX_HOME"), None);
    }

    #[test]
    fn an_unset_variable_names_no_directory() {
        assert_eq!(directory_in(&set(Vec::new()), "CODEX_HOME"), None);
    }

    #[test]
    fn empty_and_relative_roots_fall_back_below_the_home() {
        let home = absolute("/home/test");
        let lookup = set(vec![
            (HOME_VARIABLE, home.clone()),
            (USER_PROFILE_VARIABLE, home.clone()),
            (APP_DATA_VARIABLE, String::new()),
            (LOCAL_APP_DATA_VARIABLE, "Local".to_owned()),
            (XDG_CONFIG_HOME_VARIABLE, String::new()),
            (XDG_DATA_HOME_VARIABLE, "share".to_owned()),
        ]);
        let home = PathBuf::from(home);
        for windows in [false, true] {
            let dirs = UserDirs::from_lookup(&lookup, windows);
            assert_eq!(dirs.home(), home);
            assert_eq!(dirs.roaming(), home.join("AppData").join("Roaming"));
            assert_eq!(dirs.local(), home.join("AppData").join("Local"));
            assert_eq!(dirs.xdg_config(), home.join(".config"));
            assert_eq!(dirs.xdg_data(), home.join(".local").join("share"));
            assert_eq!(dirs.named_roaming(), None);
            assert_eq!(dirs.named_local(), None);
            assert_eq!(dirs.named_xdg_data(), None);
        }
    }

    #[test]
    fn the_home_follows_the_platform_variable_only() {
        let profile = absolute("/Users/profile");
        let home = absolute("/home/other");
        let both = set(vec![
            (USER_PROFILE_VARIABLE, profile.clone()),
            (HOME_VARIABLE, home.clone()),
        ]);
        assert_eq!(
            UserDirs::from_lookup(&both, true).home(),
            Path::new(&profile)
        );
        assert_eq!(UserDirs::from_lookup(&both, false).home(), Path::new(&home));
        let only_home = set(vec![(HOME_VARIABLE, home)]);
        assert_eq!(
            UserDirs::from_lookup(&only_home, true).home(),
            Path::new("")
        );
    }

    #[test]
    fn named_roots_are_taken_as_they_are() {
        let roaming = absolute("/Profiles/Roaming");
        let program = absolute("/Program Files");
        let lookup = set(vec![
            (APP_DATA_VARIABLE, roaming.clone()),
            ("ProgramFiles", program.clone()),
            ("ProgramFiles(x86)", program.clone()),
        ]);
        let dirs = UserDirs::from_lookup(&lookup, true);
        assert_eq!(dirs.roaming(), PathBuf::from(&roaming));
        assert_eq!(dirs.named_roaming(), Some(Path::new(&roaming)));
        assert_eq!(dirs.program_files(), [PathBuf::from(program)]);
    }
}
