//! The 2.0 install root: where every installed file lives under the Ghostlight home.
//!
//! Every route installs into `home/bin`: the edge `ghostlight` and the service image
//! `ghostlight-service` (ADR-0199 Decision 1), and the shell `ghostlight-desktop` beside them
//! when the payload carries it. A development build installs there too, as a channel recorded in
//! the installer's record (ADR-0199 amendment, the development channel lands in 2.0). The record,
//! its lock, and the state, policy and manifest directories sit beside the executables
//! (ADR-0197 Decisions 10 and 14). The layout holds names only and touches no file: callers pass
//! the home that `ghostlight_relay::home::home()` resolves, and tests pass a temporary directory.

use std::env::consts::EXE_SUFFIX;
use std::io;
use std::path::{Path, PathBuf};

use ghostlight_relay::home::{
    self, BIN_DIRECTORY_NAME, EDGE_EXECUTABLE_NAME, RUN_DIRECTORY_NAME, SERVICE_EXECUTABLE_NAME,
    SERVICE_LOCK_NAME,
};

/// File name of the Ghostlight shell on this platform. The relay owns the names of the service and
/// the edge; the shell is not a relay peer's executable, so its name lives here.
#[cfg(windows)]
pub const DESKTOP_EXECUTABLE_NAME: &str = "ghostlight-desktop.exe";

/// File name of the Ghostlight shell on this platform. The relay owns the names of the service and
/// the edge; the shell is not a relay peer's executable, so its name lives here.
#[cfg(not(windows))]
pub const DESKTOP_EXECUTABLE_NAME: &str = "ghostlight-desktop";

/// Directory in which earlier 2.0 development builds ran beside the installed release. An install
/// stops what runs there and removes it.
pub const RETIRED_DEVELOPMENT_DIRECTORY_NAME: &str = "dev";

/// Marker file with which earlier 2.0 development builds selected the build in the retired
/// development directory. An install removes it.
pub const RETIRED_DEVELOPMENT_MARKER_NAME: &str = "development";

/// Record of what the installer put in `bin`, kept beside those executables.
pub const INSTALL_RECORD_NAME: &str = "install.json";

/// Lock file held by whichever process is installing.
pub const INSTALL_LOCK_NAME: &str = "install.lock";

/// Directory of the service's own state: audit log, harness locations, setup record.
pub const STATE_DIRECTORY_NAME: &str = "state";

/// The start-failure record under the run directory: one line saying why the last service start
/// failed, written by that start and removed once a service publishes its endpoint. A started
/// service's output goes nowhere, so this is how its starter learns the reason.
pub const START_FAILURE_NAME: &str = "start-failure";

/// The quit latch under the state directory: a person's Quit writes it, and a browser or recover
/// start that finds it does not bring Ghostlight back.
pub const QUIT_LATCH_NAME: &str = "quit";

/// The marker under the state directory that exists exactly while "Others can watch" is off.
pub const WATCH_OFF_MARKER_NAME: &str = "watch.off";

/// The marker under the state directory that exists once the first close of the window has been
/// explained. It is service-private.
pub const TRAY_HINT_NAME: &str = "tray-hint";

/// The record under the state directory that exists once setup is complete: the first agent
/// request that reached a browser.
pub const SETUP_RECORD_NAME: &str = "setup.json";

/// The record under the state directory that names the installed browser whose extension last
/// connected, by its native-host browser id. It holds that id and nothing else.
pub const LAST_BROWSER_RECORD_NAME: &str = "last-browser.json";

/// The directory under the state directory that keeps a copy of each installed agent's own icon,
/// with a manifest beside each copy.
pub const ICONS_DIRECTORY_NAME: &str = "icons";

/// The record under the state directory of where each MCP client's configuration was found.
pub const HARNESS_LOCATIONS_NAME: &str = "harness-locations.json";

/// Directory of the user's policy.
pub const POLICY_DIRECTORY_NAME: &str = "policy";

/// The user policy the workbench authors, under the policy directory.
pub const USER_POLICY_NAME: &str = "user-policy.json";

/// The verified managed-policy cache, under the policy directory.
pub const MANAGED_POLICY_BUNDLE_NAME: &str = "managed-policy.bundle";

/// The managed-policy status sidecar, under the policy directory.
pub const MANAGED_STATUS_NAME: &str = "managed-status.json";

/// Directory of native-host manifests the installer writes.
pub const MANIFESTS_DIRECTORY_NAME: &str = "manifests";

/// The edge's command name without a platform suffix, which is how a client entry names it.
pub const EDGE_COMMAND_NAME: &str = "ghostlight";

// The relay names the edge's executable; a client entry that starts `EDGE_COMMAND_NAME` must reach
// that file, so the relay's name is this name and the platform suffix. Checked when this compiles.
const _: () = assert!(is_command_of(EDGE_COMMAND_NAME, EDGE_EXECUTABLE_NAME));

/// Whether `file` is the command `name` with this platform's executable suffix.
const fn is_command_of(name: &str, file: &str) -> bool {
    let (name, file, suffix) = (name.as_bytes(), file.as_bytes(), EXE_SUFFIX.as_bytes());
    if file.len() != name.len() + suffix.len() {
        return false;
    }
    let mut index = 0;
    while index < file.len() {
        let expected = if index < name.len() {
            name[index]
        } else {
            suffix[index - name.len()]
        };
        if file[index] != expected {
            return false;
        }
        index += 1;
    }
    true
}

/// The retired MCP connector's name without a platform suffix. Earlier builds installed it beside
/// the service, and client entries written for them still name it.
pub const RETIRED_MCP_CONNECTOR_NAME: &str = "ghostlight-mcp-connector";

/// The retired browser connector's name without a platform suffix.
pub const RETIRED_BROWSER_CONNECTOR_NAME: &str = "ghostlight-browser-connector";

/// File names of the connector images 1.x and earlier 2.0 development builds installed, with this
/// platform's executable suffix.
#[must_use]
pub fn retired_connector_names() -> [String; 2] {
    [RETIRED_MCP_CONNECTOR_NAME, RETIRED_BROWSER_CONNECTOR_NAME]
        .map(|name| format!("{name}{EXE_SUFFIX}"))
}

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

    /// The retired directory of earlier development builds: `home/dev`.
    #[must_use]
    pub fn retired_development_directory(&self) -> PathBuf {
        self.home.join(RETIRED_DEVELOPMENT_DIRECTORY_NAME)
    }

    /// The retired development marker: `home/development`.
    #[must_use]
    pub fn retired_development_marker(&self) -> PathBuf {
        self.home.join(RETIRED_DEVELOPMENT_MARKER_NAME)
    }

    /// The installed service executable: `home/bin/ghostlight-service`.
    #[must_use]
    pub fn service_executable(&self) -> PathBuf {
        self.bin().join(SERVICE_EXECUTABLE_NAME)
    }

    /// The installed edge: `home/bin/ghostlight`. Clients, browsers and shells start it.
    #[must_use]
    pub fn edge(&self) -> PathBuf {
        self.bin().join(EDGE_EXECUTABLE_NAME)
    }

    /// The installed shell: `home/bin/ghostlight-desktop`.
    #[must_use]
    pub fn desktop_executable(&self) -> PathBuf {
        self.bin().join(DESKTOP_EXECUTABLE_NAME)
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

    /// The start-failure record: `home/run/start-failure`. See [`START_FAILURE_NAME`].
    #[must_use]
    pub fn start_failure(&self) -> PathBuf {
        self.run().join(START_FAILURE_NAME)
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

    /// The watching-off marker: `home/state/watch.off`. It exists only while watching is off.
    #[must_use]
    pub fn watch_off_marker(&self) -> PathBuf {
        self.state().join(WATCH_OFF_MARKER_NAME)
    }

    /// The marker that records the first close of the window was explained:
    /// `home/state/tray-hint`.
    #[must_use]
    pub fn tray_hint(&self) -> PathBuf {
        self.state().join(TRAY_HINT_NAME)
    }

    /// The setup record: `home/state/setup.json`. It exists once setup is complete.
    #[must_use]
    pub fn setup_record(&self) -> PathBuf {
        self.state().join(SETUP_RECORD_NAME)
    }

    /// The last-browser record: `home/state/last-browser.json`. It exists once a browser
    /// Ghostlight can name has connected.
    #[must_use]
    pub fn last_browser_record(&self) -> PathBuf {
        self.state().join(LAST_BROWSER_RECORD_NAME)
    }

    /// The agent icon cache: `home/state/icons`.
    #[must_use]
    pub fn icons(&self) -> PathBuf {
        self.state().join(ICONS_DIRECTORY_NAME)
    }

    /// The record of where each MCP client's configuration was found:
    /// `home/state/harness-locations.json`.
    #[must_use]
    pub fn harness_locations(&self) -> PathBuf {
        self.state().join(HARNESS_LOCATIONS_NAME)
    }

    /// The user policy directory: `home/policy`.
    #[must_use]
    pub fn policy(&self) -> PathBuf {
        self.home.join(POLICY_DIRECTORY_NAME)
    }

    /// The user policy the workbench authors: `home/policy/user-policy.json`.
    #[must_use]
    pub fn user_policy(&self) -> PathBuf {
        self.policy().join(USER_POLICY_NAME)
    }

    /// The verified managed-policy cache: `home/policy/managed-policy.bundle`.
    #[must_use]
    pub fn managed_policy_bundle(&self) -> PathBuf {
        self.policy().join(MANAGED_POLICY_BUNDLE_NAME)
    }

    /// The managed-policy status sidecar: `home/policy/managed-status.json`.
    #[must_use]
    pub fn managed_status(&self) -> PathBuf {
        self.policy().join(MANAGED_STATUS_NAME)
    }

    /// The native-host manifest directory: `home/manifests`.
    #[must_use]
    pub fn manifests(&self) -> PathBuf {
        self.home.join(MANIFESTS_DIRECTORY_NAME)
    }
}
