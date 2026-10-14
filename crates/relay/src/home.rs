//! The fixed Ghostlight home and the names of the files under it.
//!
//! There is one home per OS user: `~/.ghostlight`, derived from the user's home directory. Every
//! process finds the same files without a search, which is what makes one service per user true
//! by construction (ADR-0197 Decision 10).

use std::env;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

/// Test-only override of the home directory, for journeys that must not touch the real one. It
/// names the Ghostlight home itself, not its parent. Production code never sets it.
pub const TEST_HOME_VARIABLE: &str = "GHOSTLIGHT_TEST_HOME";

/// Name of the Ghostlight home directory under the user's home directory.
pub const HOME_DIRECTORY_NAME: &str = ".ghostlight";

/// Runtime file: where the running service published its port and token.
pub const RUNTIME_FILE_NAME: &str = "ghostlight-runtime.json";

/// Lease file: held by the one running service. The same file earlier services take, so old and
/// new services exclude each other.
pub const LEASE_FILE_NAME: &str = "ghostlight-runtime.lock";

/// Failure-template file: the sentences the service handed the MCP connector in advance.
pub const TEMPLATES_FILE_NAME: &str = "relay-templates.json";

/// Directory holding the installed executables.
pub const BIN_DIRECTORY_NAME: &str = "bin";

/// Marker file: its presence turns diagnostics on.
pub const DIAGNOSTICS_MARKER_NAME: &str = "diagnostics.on";

/// Directory receiving diagnostics logs.
pub const DIAGNOSTICS_DIRECTORY_NAME: &str = "diagnostics";

/// File name of the service executable on this platform.
#[cfg(windows)]
pub const SERVICE_EXECUTABLE_NAME: &str = "ghostlight.exe";

/// File name of the service executable on this platform.
#[cfg(not(windows))]
pub const SERVICE_EXECUTABLE_NAME: &str = "ghostlight";

/// Environment variable naming the user's home directory on this platform.
#[cfg(windows)]
const USER_HOME_VARIABLE: &str = "USERPROFILE";

/// Environment variable naming the user's home directory on this platform.
#[cfg(not(windows))]
const USER_HOME_VARIABLE: &str = "HOME";

/// The Ghostlight home directory.
///
/// This is the value of `GHOSTLIGHT_TEST_HOME` when it is set (a test-only override), otherwise
/// the platform's user home directory joined with `.ghostlight`. The path must be absolute;
/// anything else is an error rather than a guess.
pub fn home() -> io::Result<PathBuf> {
    resolve(
        env::var_os(TEST_HOME_VARIABLE),
        env::var_os(USER_HOME_VARIABLE),
    )
}

/// Runtime file path: `home/ghostlight-runtime.json`.
pub fn runtime_file() -> io::Result<PathBuf> {
    Ok(home()?.join(RUNTIME_FILE_NAME))
}

/// Lease file path: `home/ghostlight-runtime.lock`.
pub fn lease_file() -> io::Result<PathBuf> {
    Ok(home()?.join(LEASE_FILE_NAME))
}

/// Failure-template file path: `home/relay-templates.json`.
pub fn templates_file() -> io::Result<PathBuf> {
    Ok(home()?.join(TEMPLATES_FILE_NAME))
}

/// Directory of installed executables: `home/bin`.
pub fn bin() -> io::Result<PathBuf> {
    Ok(home()?.join(BIN_DIRECTORY_NAME))
}

/// Installed service executable: `home/bin/ghostlight` (`ghostlight.exe` on Windows).
pub fn service_executable() -> io::Result<PathBuf> {
    Ok(bin()?.join(SERVICE_EXECUTABLE_NAME))
}

/// Diagnostics marker path: `home/diagnostics.on`.
pub fn diagnostics_marker() -> io::Result<PathBuf> {
    Ok(home()?.join(DIAGNOSTICS_MARKER_NAME))
}

/// Diagnostics directory: `home/diagnostics`.
pub fn diagnostics_dir() -> io::Result<PathBuf> {
    Ok(home()?.join(DIAGNOSTICS_DIRECTORY_NAME))
}

/// Pick the home from the two environment values.
fn resolve(test_home: Option<OsString>, user_home: Option<OsString>) -> io::Result<PathBuf> {
    if let Some(test_home) = test_home {
        return absolute(PathBuf::from(test_home), TEST_HOME_VARIABLE);
    }
    match user_home {
        Some(user_home) => absolute(PathBuf::from(user_home), USER_HOME_VARIABLE)
            .map(|path| path.join(HOME_DIRECTORY_NAME)),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{USER_HOME_VARIABLE} is not set"),
        )),
    }
}

/// Require an absolute path, naming the variable it came from when it is not.
fn absolute(path: PathBuf, variable: &str) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{variable} must be an absolute path"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[cfg(windows)]
    const ROOT: &str = "C:\\Users\\person";
    #[cfg(not(windows))]
    const ROOT: &str = "/home/person";

    fn os(value: &str) -> Option<OsString> {
        Some(OsString::from(value))
    }

    #[test]
    fn the_home_is_the_user_home_joined_with_the_ghostlight_directory() {
        let home = resolve(None, os(ROOT)).unwrap();
        assert_eq!(home, Path::new(ROOT).join(".ghostlight"));
    }

    #[test]
    fn the_test_override_names_the_home_itself() {
        let home = resolve(os(ROOT), os("ignored")).unwrap();
        assert_eq!(home, Path::new(ROOT));
    }

    #[test]
    fn a_relative_home_is_refused() {
        assert_eq!(
            resolve(None, os("relative")).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            resolve(os("relative"), os(ROOT)).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn a_missing_user_home_is_not_found() {
        assert_eq!(
            resolve(None, None).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn file_names_are_the_ones_older_services_already_use() {
        assert_eq!(RUNTIME_FILE_NAME, "ghostlight-runtime.json");
        assert_eq!(LEASE_FILE_NAME, "ghostlight-runtime.lock");
        assert_eq!(TEMPLATES_FILE_NAME, "relay-templates.json");
    }
}
