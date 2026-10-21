//! The person's directories as the environment names them, read under one rule.
//!
//! The installer finds every client configuration, native-host manifest, desktop entry and Store
//! container below directories that environment variables name: the home, `APPDATA`,
//! `LOCALAPPDATA`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and the clients' own variables such as
//! `CODEX_HOME`. A variable that is set but empty names no directory: taken as a path it is the
//! working directory, and an install would write a client's file there. So an empty value counts
//! as unset, and the caller's default applies, as the XDG base-directory rules ask and as Claude
//! Code already reads `CLAUDE_CONFIG_DIR`.

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

/// How a resolver reads one environment variable by name: the process environment, or a fixed
/// set in a test.
pub(super) type Lookup<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The directory the process environment's variable `name` names, or none when it is unset or
/// empty.
#[must_use]
pub(super) fn directory(name: &str) -> Option<PathBuf> {
    directory_in(&|name| env::var_os(name), name)
}

/// The directory `lookup`'s variable `name` names, or none when it is unset or empty.
#[must_use]
pub(super) fn directory_in(lookup: Lookup<'_>, name: &str) -> Option<PathBuf> {
    lookup(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(set: &'static str, value: &'static str) -> impl Fn(&str) -> Option<OsString> {
        move |name| (name == set).then(|| OsString::from(value))
    }

    #[test]
    fn a_set_variable_names_its_directory() {
        assert_eq!(
            directory_in(&only("CODEX_HOME", "/work/codex"), "CODEX_HOME"),
            Some(PathBuf::from("/work/codex"))
        );
    }

    #[test]
    fn an_empty_variable_names_no_directory() {
        assert_eq!(directory_in(&only("CODEX_HOME", ""), "CODEX_HOME"), None);
    }

    #[test]
    fn an_unset_variable_names_no_directory() {
        assert_eq!(directory_in(&only("OTHER", "/x"), "CODEX_HOME"), None);
    }
}
