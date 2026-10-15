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

/// Directory under the home that holds the service's run-time objects.
pub const RUN_DIRECTORY_NAME: &str = "run";

/// The relay socket under the run directory, on Linux.
pub const RELAY_SOCKET_NAME: &str = "relay.sock";

/// The service's lock file under the run directory, held for as long as one service runs.
pub const SERVICE_LOCK_NAME: &str = "service.lock";

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
mod tests;
