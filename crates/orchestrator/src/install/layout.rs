//! The 2.0 install root: where every installed file lives under the Ghostlight home.
//!
//! Every route installs into `home/bin`. A development build lives in `home/dev` and is selected
//! by the marker file `home/development`. The installer's record, its lock, and the state,
//! policy and manifest directories sit beside them (ADR-0197 Decisions 10, 13 and 14). The
//! layout holds names only and touches no file: callers pass the home that
//! `ghostlight_relay::home::home()` resolves, and tests pass a temporary directory.

use std::io;
use std::path::{Path, PathBuf};

use ghostlight_relay::home::{
    self, BIN_DIRECTORY_NAME, RUN_DIRECTORY_NAME, SERVICE_EXECUTABLE_NAME, SERVICE_LOCK_NAME,
};

/// Directory holding a development build of the service.
pub const DEV_DIRECTORY_NAME: &str = "dev";

/// Marker file selecting the development build. Its presence is the selection.
pub const DEVELOPMENT_MARKER_NAME: &str = "development";

/// Record of what the installer put in `bin`, kept beside those executables.
pub const INSTALL_RECORD_NAME: &str = "install.json";

/// Lock file held by whichever process is installing.
pub const INSTALL_LOCK_NAME: &str = "install.lock";

/// Directory of the service's own state: audit log, harness locations, handoff marker.
pub const STATE_DIRECTORY_NAME: &str = "state";

/// The quit latch under the state directory: a person's Quit writes it, and a browser or recover
/// start that finds it does not bring Ghostlight back.
pub const QUIT_LATCH_NAME: &str = "quit";

/// Directory of the user's policy.
pub const POLICY_DIRECTORY_NAME: &str = "policy";

/// Directory of native-host manifests the installer writes.
pub const MANIFESTS_DIRECTORY_NAME: &str = "manifests";

/// File name of the MCP connector executable on this platform.
#[cfg(windows)]
pub const MCP_CONNECTOR_EXECUTABLE_NAME: &str = "ghostlight-mcp-connector.exe";

/// File name of the MCP connector executable on this platform.
#[cfg(not(windows))]
pub const MCP_CONNECTOR_EXECUTABLE_NAME: &str = "ghostlight-mcp-connector";

/// File name of the browser connector executable on this platform.
#[cfg(windows)]
pub const BROWSER_CONNECTOR_EXECUTABLE_NAME: &str = "ghostlight-browser-connector.exe";

/// File name of the browser connector executable on this platform.
#[cfg(not(windows))]
pub const BROWSER_CONNECTOR_EXECUTABLE_NAME: &str = "ghostlight-browser-connector";

/// Every path the installer and the entry rule use, derived from one home directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Layout {
    home: PathBuf,
}

impl Layout {
    /// The layout under `home`, the Ghostlight home directory itself.
    #[must_use]
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    /// The layout under this user's Ghostlight home, as `ghostlight_relay::home::home()`
    /// resolves it.
    pub fn current() -> io::Result<Self> {
        Ok(Self::new(home::home()?))
    }

    /// The Ghostlight home directory.
    #[must_use]
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Directory of installed executables: `home/bin`.
    #[must_use]
    pub fn bin(&self) -> PathBuf {
        self.home.join(BIN_DIRECTORY_NAME)
    }

    /// Directory of the development build: `home/dev`.
    #[must_use]
    pub fn dev(&self) -> PathBuf {
        self.home.join(DEV_DIRECTORY_NAME)
    }

    /// The installed service executable: `home/bin/ghostlight`.
    #[must_use]
    pub fn service_executable(&self) -> PathBuf {
        self.bin().join(SERVICE_EXECUTABLE_NAME)
    }

    /// The development build's service executable: `home/dev/ghostlight`.
    #[must_use]
    pub fn development_executable(&self) -> PathBuf {
        self.dev().join(SERVICE_EXECUTABLE_NAME)
    }

    /// The installed MCP connector: `home/bin/ghostlight-mcp-connector`.
    #[must_use]
    pub fn mcp_connector(&self) -> PathBuf {
        self.bin().join(MCP_CONNECTOR_EXECUTABLE_NAME)
    }

    /// The installed browser connector: `home/bin/ghostlight-browser-connector`.
    #[must_use]
    pub fn browser_connector(&self) -> PathBuf {
        self.bin().join(BROWSER_CONNECTOR_EXECUTABLE_NAME)
    }

    /// The development marker: `home/development`.
    #[must_use]
    pub fn development_marker(&self) -> PathBuf {
        self.home.join(DEVELOPMENT_MARKER_NAME)
    }

    /// The install record: `home/bin/install.json`.
    #[must_use]
    pub fn install_record(&self) -> PathBuf {
        self.bin().join(INSTALL_RECORD_NAME)
    }

    /// The install lock: `home/install.lock`.
    #[must_use]
    pub fn install_lock(&self) -> PathBuf {
        self.home.join(INSTALL_LOCK_NAME)
    }

    /// The directory of the service's run-time objects: `home/run`.
    #[must_use]
    pub fn run(&self) -> PathBuf {
        self.home.join(RUN_DIRECTORY_NAME)
    }

    /// The lock one running service holds: `home/run/service.lock`.
    #[must_use]
    pub fn service_lock(&self) -> PathBuf {
        self.run().join(SERVICE_LOCK_NAME)
    }

    /// The state directory: `home/state`.
    #[must_use]
    pub fn state(&self) -> PathBuf {
        self.home.join(STATE_DIRECTORY_NAME)
    }

    /// The quit latch: `home/state/quit`.
    #[must_use]
    pub fn quit_latch(&self) -> PathBuf {
        self.state().join(QUIT_LATCH_NAME)
    }

    /// The user policy directory: `home/policy`.
    #[must_use]
    pub fn policy(&self) -> PathBuf {
        self.home.join(POLICY_DIRECTORY_NAME)
    }

    /// The native-host manifest directory: `home/manifests`.
    #[must_use]
    pub fn manifests(&self) -> PathBuf {
        self.home.join(MANIFESTS_DIRECTORY_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    const HOME: &str = "C:\\Users\\person\\.ghostlight";
    #[cfg(not(windows))]
    const HOME: &str = "/home/person/.ghostlight";

    #[cfg(windows)]
    const EXE: &str = ".exe";
    #[cfg(not(windows))]
    const EXE: &str = "";

    #[test]
    fn every_path_derives_from_the_home() {
        let layout = Layout::new(HOME);
        let home = Path::new(HOME);
        assert_eq!(layout.home(), home);
        assert_eq!(layout.bin(), home.join("bin"));
        assert_eq!(layout.dev(), home.join("dev"));
        assert_eq!(layout.development_marker(), home.join("development"));
        assert_eq!(
            layout.install_record(),
            home.join("bin").join("install.json")
        );
        assert_eq!(layout.install_lock(), home.join("install.lock"));
        assert_eq!(layout.run(), home.join("run"));
        assert_eq!(layout.service_lock(), home.join("run").join("service.lock"));
        assert_eq!(layout.state(), home.join("state"));
        assert_eq!(layout.quit_latch(), home.join("state").join("quit"));
        assert_eq!(layout.policy(), home.join("policy"));
        assert_eq!(layout.manifests(), home.join("manifests"));
    }

    #[test]
    fn executables_carry_the_platform_suffix() {
        let layout = Layout::new(HOME);
        let bin = Path::new(HOME).join("bin");
        assert_eq!(
            layout.service_executable(),
            bin.join(format!("ghostlight{EXE}"))
        );
        assert_eq!(
            layout.mcp_connector(),
            bin.join(format!("ghostlight-mcp-connector{EXE}"))
        );
        assert_eq!(
            layout.browser_connector(),
            bin.join(format!("ghostlight-browser-connector{EXE}"))
        );
        assert_eq!(
            layout.development_executable(),
            Path::new(HOME).join("dev").join(format!("ghostlight{EXE}"))
        );
    }
}
