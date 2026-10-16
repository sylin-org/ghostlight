//! Ownership-checked per-user Chromium native-messaging registration.
//!
//! The context resolves from the running executable and the user environment. Setting
//! `GHOSTLIGHT_NATIVE_HOST_DIR` roots every manifest and Windows registry key below one
//! directory, so journeys and scratch builds exercise the real install and recovery seams
//! without adopting the machine's persistent browser registration.
//!
//! On Windows the one manifest lives in `home/manifests`, which no app container redirects, and
//! each browser's `HKCU` value names that fixed path. A registry value is written only when it
//! differs from what is read, so an install that runs inside an app container (whose registry
//! writes only that app sees) and finds the right value already registered writes nothing. When
//! it would have to write one, it leaves the value alone and names the browser in
//! [`NativeHostActionResult::private`] (ADR-0197 amendment, 2026-10-14). On Linux the browsers
//! read each manifest file itself from their own configuration directory.

use std::collections::HashSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use super::browser_package::{self, BrowserPackage, BrowserPackageContext, BrowserPackageSpec};
use super::container::{self, AppContainer, Residence};
use super::layout::{Layout, EDGE_COMMAND_NAME, MANIFESTS_DIRECTORY_NAME};

/// Stable native-messaging identity used by every supported Chromium browser.
pub const HOST_NAME: &str = "org.sylin.ghostlight";
/// Public Chrome Web Store extension identity.
pub const STORE_EXTENSION_ID: &str = "lejccfmoeogmhemakeknjjdhkfkgncdl";
/// Unpacked development extension identity pinned by `extension/manifest.json`.
pub const DEVELOPMENT_EXTENSION_ID: &str = "cjcmhepmagomefjggkcohdbfemacojoa";

/// Environment variable that roots every manifest and registry key below one directory.
pub const NATIVE_HOST_DIR_VARIABLE: &str = "GHOSTLIGHT_NATIVE_HOST_DIR";

const HOST_DESCRIPTION: &str = "Ghostlight browser connector";

/// Operating-system registration layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHostPlatform {
    /// Per-user Windows manifest plus HKCU browser keys.
    Windows,
    /// Per-browser files below the XDG user configuration directory.
    Linux,
}

/// State of one browser's Ghostlight native-host registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeHostState {
    /// No registration is present.
    Missing,
    /// The registration exactly names this sibling connector and the fixed origins.
    Current,
    /// A Ghostlight-owned registration of this installation carries stale details.
    Updatable,
    /// A Ghostlight-owned registration names a connector outside this installation.
    ///
    /// Recovery must not adopt it: silent cross-tree repair is how a scratch build rewrites the
    /// machine's real registration (ADR-0149 amendment). Only a deliberate install adopts it.
    OwnedElsewhere,
    /// Malformed or foreign state occupies Ghostlight's registration location.
    NeedsAttention,
}

impl NativeHostState {
    /// The plain words a person reads for this state.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Missing => "not registered",
            Self::Current => "registered",
            Self::Updatable => "registered, needs an update",
            Self::OwnedElsewhere => "registered to another installation",
            Self::NeedsAttention => "needs attention",
        }
    }
}

/// Read-only state for one supported browser.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserRegistration {
    /// Stable browser identifier.
    pub id: String,
    /// Human-readable browser name.
    pub name: String,
    /// Detected package form and native-messaging usability.
    pub package: BrowserPackage,
    /// Content-free package-form explanation.
    pub package_detail: String,
    /// Current ownership and freshness state.
    pub state: NativeHostState,
    /// Content-free explanation of the state.
    pub detail: String,
}

/// Complete native-host inspection result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeHostReport {
    /// Exact connector path the current installation expects.
    pub connector: PathBuf,
    /// One result for every supported browser.
    pub browsers: Vec<BrowserRegistration>,
    /// The app container this inspection ran in, when there is one: the registry it read is that
    /// app's private view, so the states above say nothing about what other apps see.
    pub private_view: Option<AppContainer>,
}

impl NativeHostReport {
    /// Whether any state requires a person before Ghostlight may change it.
    #[must_use]
    pub fn needs_attention(&self) -> bool {
        self.browsers
            .iter()
            .any(|browser| browser.state == NativeHostState::NeedsAttention)
    }
}

/// Definite result of an install or uninstall request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativeHostActionResult {
    /// Whether at least one owned registration changed.
    pub changed: bool,
    /// State observed after the action.
    pub report: NativeHostReport,
    /// Browsers whose registry value had to change but was left alone, because this process runs
    /// inside an app container whose registry writes no other app sees.
    pub private: Vec<String>,
}

/// How a person knows one supported browser family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserIdentity {
    /// Stable browser identifier.
    pub id: &'static str,
    /// Full product name, for example `Google Chrome`.
    pub name: &'static str,
    /// Short name for running text, for example `Chrome`.
    pub short: &'static str,
}

/// Product-owned native-host lifecycle service.
#[derive(Clone, Debug)]
pub struct NativeHostRegistry {
    context: NativeHostContext,
}

impl NativeHostRegistry {
    /// Discover the current per-user and sibling-executable context.
    #[must_use]
    pub fn discover() -> Self {
        Self {
            context: NativeHostContext::system(),
        }
    }

    /// Inspect every supported registration without changing the machine.
    pub fn check(&self) -> Result<NativeHostReport, NativeHostError> {
        inspect(&self.context, &SystemRegistrationIo)
    }

    /// The connector each browser's Ghostlight-owned registration names, by browser id.
    ///
    /// Browsers without an owned registration are left out. Nothing is changed.
    pub fn registered_connectors(&self) -> Result<Vec<(String, PathBuf)>, NativeHostError> {
        let mut registered = Vec::new();
        for browser in BROWSERS {
            let (_, connector) = inspect_browser(&self.context, &SystemRegistrationIo, browser)?;
            if let Some(connector) = connector {
                registered.push((browser.id.to_owned(), connector));
            }
        }
        Ok(registered)
    }

    /// Resolve the ordinary installed executable for one supported browser without changing the
    /// machine.
    #[must_use]
    pub fn browser_executable(&self, browser_id: &str) -> Option<PathBuf> {
        let browser = BROWSERS.iter().find(|browser| browser.id == browser_id)?;
        browser_executable(&self.context, browser)
    }

    /// Where one supported browser keeps its profiles (its user data directory), or `None` for an
    /// id Ghostlight does not support. The directory may not exist. Nothing is read or changed.
    #[must_use]
    pub fn user_data_directory(&self, browser_id: &str) -> Option<PathBuf> {
        let browser = BROWSERS.iter().find(|browser| browser.id == browser_id)?;
        Some(user_data_directory(&self.context, browser))
    }

    /// The current user's home directory, where a browser started for them is given its working
    /// directory. It may be empty when the environment names none.
    #[must_use]
    pub fn user_home(&self) -> &Path {
        &self.context.home
    }

    /// Every supported browser family, in the table's order.
    #[must_use]
    pub fn browser_identities(&self) -> Vec<BrowserIdentity> {
        BROWSERS
            .iter()
            .map(|browser| BrowserIdentity {
                id: browser.id,
                name: browser.name,
                short: browser.short,
            })
            .collect()
    }

    /// Install or update every safe per-user registration.
    pub fn install(&self) -> Result<NativeHostActionResult, NativeHostError> {
        apply_install(&self.context, &SystemRegistrationIo)
    }

    /// Install or update only the named browser registrations.
    pub fn install_selected(
        &self,
        browser_ids: &[String],
    ) -> Result<NativeHostActionResult, NativeHostError> {
        let browsers = select_browsers(browser_ids)?;
        apply_install_for(&self.context, &SystemRegistrationIo, &browsers)
    }

    /// Ensure one registration is current, repairing it only when fresh inspection still proves
    /// Ghostlight owns its stale state.
    pub fn repair_owned_registration(
        &self,
        browser_id: &str,
    ) -> Result<NativeHostActionResult, NativeHostError> {
        let browsers = select_browsers(&[browser_id.to_owned()])?;
        apply_owned_repair_for(&self.context, &SystemRegistrationIo, &browsers)
    }

    /// Remove only registrations whose manifest proves Ghostlight ownership.
    pub fn uninstall(&self) -> Result<NativeHostActionResult, NativeHostError> {
        apply_uninstall(&self.context, &SystemRegistrationIo)
    }

    /// Remove only owned registrations for the named browsers.
    pub fn uninstall_selected(
        &self,
        browser_ids: &[String],
    ) -> Result<NativeHostActionResult, NativeHostError> {
        let browsers = select_browsers(browser_ids)?;
        apply_uninstall_for(&self.context, &SystemRegistrationIo, &browsers)
    }

    /// Register every browser that has no Ghostlight registration and can start the connector.
    ///
    /// Registrations that exist are left as they are, whoever owns them; reclaiming those is the
    /// installer's deliberate act. Returns `None` when nothing was missing.
    pub fn register_missing(&self) -> Result<Option<NativeHostActionResult>, NativeHostError> {
        let missing: Vec<String> = self
            .check()?
            .browsers
            .into_iter()
            .filter(|browser| {
                browser.state == NativeHostState::Missing
                    && browser.package.native_messaging_usable()
            })
            .map(|browser| browser.id)
            .collect();
        if missing.is_empty() {
            return Ok(None);
        }
        self.install_selected(&missing).map(Some)
    }

    /// A registry over Linux-style manifests below `root`, naming `connector`, for tests that
    /// must not touch the machine.
    #[cfg(test)]
    pub(crate) fn isolated(root: &Path, connector: &Path) -> Self {
        Self {
            context: NativeHostContext {
                platform: NativeHostPlatform::Linux,
                home: root.to_path_buf(),
                config: root.join(".config"),
                local: root.join("local"),
                layout: Layout::new(root.join("ghostlight")),
                manifests: root.join(MANIFESTS_DIRECTORY_NAME),
                connector: normalize_path(connector),
                browser_packages: BrowserPackageContext::isolated(&root.join("browser-packages")),
                windows_browser_roots: Vec::new(),
                registry_isolated: true,
                residence: Residence::Regular,
            },
        }
    }

    /// Where the Linux-style manifest of `browser_id` lives for a registry made by
    /// [`Self::isolated`].
    #[cfg(test)]
    pub(crate) fn isolated_manifest(&self, browser_id: &str) -> PathBuf {
        let browser = BROWSERS
            .iter()
            .find(|browser| browser.id == browser_id)
            .expect("a supported browser id");
        browser_manifest_path(&self.context, browser)
    }
}

#[derive(Clone, Debug)]
struct NativeHostContext {
    platform: NativeHostPlatform,
    home: PathBuf,
    config: PathBuf,
    local: PathBuf,
    /// The Ghostlight home, for the commands a report names.
    layout: Layout,
    /// Where the Windows manifest lives: `home/manifests`, or below the isolated root.
    manifests: PathBuf,
    connector: PathBuf,
    browser_packages: BrowserPackageContext,
    windows_browser_roots: Vec<PathBuf>,
    /// Registry writes stay below the Ghostlight-owned isolated subkey (env-scoped journeys).
    registry_isolated: bool,
    /// Where this process runs. Registry writes inside an app container are that app's alone.
    residence: Residence,
}

impl NativeHostContext {
    fn system() -> Self {
        let home = env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let platform = if cfg!(target_os = "windows") {
            NativeHostPlatform::Windows
        } else {
            NativeHostPlatform::Linux
        };
        let (home, local, config, registry_isolated) = native_host_roots(
            env::var_os(NATIVE_HOST_DIR_VARIABLE).map(PathBuf::from),
            env::var_os("LOCALAPPDATA").map(PathBuf::from),
            env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            &home,
        );
        let layout = Layout::current().unwrap_or_else(|_| Layout::new(PathBuf::new()));
        let connector = layout
            .bin()
            .join(executable_name(EDGE_COMMAND_NAME, platform));
        let manifests = if registry_isolated {
            home.join(MANIFESTS_DIRECTORY_NAME)
        } else {
            layout.manifests()
        };
        // An isolated registry key is read by this process's own descendants only, so an app
        // container's private view is exactly right for it.
        let residence = if registry_isolated || platform != NativeHostPlatform::Windows {
            Residence::Regular
        } else {
            Residence::detect()
        };
        let browser_packages =
            BrowserPackageContext::system(&home, platform == NativeHostPlatform::Linux);
        let windows_browser_roots = if platform == NativeHostPlatform::Windows {
            windows_browser_roots(&local)
        } else {
            Vec::new()
        };
        Self {
            platform,
            home,
            config,
            local,
            layout,
            manifests,
            connector: normalize_path(&connector),
            browser_packages,
            windows_browser_roots,
            registry_isolated,
            residence,
        }
    }

    /// The app container whose private registry view this process writes to, on Windows.
    fn private_registry(&self) -> Option<&AppContainer> {
        if self.platform == NativeHostPlatform::Windows {
            self.residence.container()
        } else {
            None
        }
    }

    /// Registry location this context reads and writes for one browser.
    fn registry_key(&self, browser: &BrowserSpec) -> String {
        if self.registry_isolated {
            format!(
                r"Software\Ghostlight\Isolated\{}\NativeMessagingHosts\{HOST_NAME}",
                browser.windows_vendor.join(r"\")
            )
        } else {
            windows_registry_key(browser)
        }
    }
}

/// Resolve the manifest roots and the registry scope for this process.
///
/// An explicit root (from `GHOSTLIGHT_NATIVE_HOST_DIR`) carries both platform roots and marks
/// the registry isolated, so no real browser key is read or written. Without it, the ordinary
/// per-user locations apply.
fn native_host_roots(
    explicit: Option<PathBuf>,
    local_appdata: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    home: &Path,
) -> (PathBuf, PathBuf, PathBuf, bool) {
    match explicit {
        Some(root) => (root.clone(), root.clone(), root, true),
        None => (
            home.to_path_buf(),
            local_appdata.unwrap_or_else(|| home.join("AppData/Local")),
            xdg_config_home.unwrap_or_else(|| home.join(".config")),
            false,
        ),
    }
}

#[derive(Clone, Copy)]
struct BrowserSpec {
    id: &'static str,
    name: &'static str,
    /// The name a sentence uses when the family is clear from context: `Chrome`, not
    /// `Google Chrome`.
    short: &'static str,
    windows_vendor: &'static [&'static str],
    windows_directory: &'static [&'static str],
    windows_executable: &'static str,
    linux_directory: &'static str,
    package: BrowserPackageSpec,
}

const BROWSERS: &[BrowserSpec] = &[
    BrowserSpec {
        id: "chrome",
        name: "Google Chrome",
        short: "Chrome",
        windows_vendor: &["Google", "Chrome"],
        windows_directory: &["Google", "Chrome", "Application"],
        windows_executable: "chrome.exe",
        linux_directory: "google-chrome/NativeMessagingHosts",
        package: BrowserPackageSpec {
            executables: &["google-chrome", "google-chrome-stable"],
            snap_executable: "google-chrome",
            flatpak_ids: &["com.google.Chrome"],
        },
    },
    BrowserSpec {
        id: "edge",
        name: "Microsoft Edge",
        short: "Edge",
        windows_vendor: &["Microsoft", "Edge"],
        windows_directory: &["Microsoft", "Edge", "Application"],
        windows_executable: "msedge.exe",
        linux_directory: "microsoft-edge/NativeMessagingHosts",
        package: BrowserPackageSpec {
            executables: &["microsoft-edge", "microsoft-edge-stable"],
            snap_executable: "microsoft-edge",
            flatpak_ids: &["com.microsoft.Edge"],
        },
    },
    BrowserSpec {
        id: "brave",
        name: "Brave",
        short: "Brave",
        windows_vendor: &["BraveSoftware", "Brave-Browser"],
        windows_directory: &["BraveSoftware", "Brave-Browser", "Application"],
        windows_executable: "brave.exe",
        linux_directory: "BraveSoftware/Brave-Browser/NativeMessagingHosts",
        package: BrowserPackageSpec {
            executables: &["brave-browser", "brave"],
            snap_executable: "brave",
            flatpak_ids: &["com.brave.Browser"],
        },
    },
    BrowserSpec {
        id: "chromium",
        name: "Chromium",
        short: "Chromium",
        windows_vendor: &["Chromium"],
        windows_directory: &["Chromium", "Application"],
        windows_executable: "chrome.exe",
        linux_directory: "chromium/NativeMessagingHosts",
        package: BrowserPackageSpec {
            executables: &["chromium", "chromium-browser"],
            snap_executable: "chromium",
            flatpak_ids: &["org.chromium.Chromium"],
        },
    },
];

/// Name of the directory under a Windows vendor path that holds the profiles.
const WINDOWS_USER_DATA_DIRECTORY: &str = "User Data";
/// Last component of a Linux native-messaging directory; its parent holds the profiles.
const LINUX_NATIVE_MESSAGING_COMPONENT: &str = "NativeMessagingHosts";

/// The directory that holds one browser's profiles, derived from the same table as its
/// registration: the vendor path under `LOCALAPPDATA` on Windows, and the native-messaging
/// directory's parent under the XDG configuration home on Linux.
fn user_data_directory(context: &NativeHostContext, browser: &BrowserSpec) -> PathBuf {
    match context.platform {
        NativeHostPlatform::Windows => browser
            .windows_vendor
            .iter()
            .fold(context.local.clone(), |path, component| {
                path.join(component)
            })
            .join(WINDOWS_USER_DATA_DIRECTORY),
        NativeHostPlatform::Linux => {
            let directory = browser
                .linux_directory
                .strip_suffix(LINUX_NATIVE_MESSAGING_COMPONENT)
                .unwrap_or(browser.linux_directory)
                .trim_end_matches('/');
            context.config.join(directory)
        }
    }
}

fn windows_browser_roots(local: &Path) -> Vec<PathBuf> {
    let mut roots = vec![local.to_path_buf()];
    for variable in ["PROGRAMFILES", "PROGRAMFILES(X86)"] {
        let Some(root) = env::var_os(variable)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
        else {
            continue;
        };
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

fn browser_executable(context: &NativeHostContext, browser: &BrowserSpec) -> Option<PathBuf> {
    match context.platform {
        NativeHostPlatform::Windows => context
            .windows_browser_roots
            .iter()
            .map(|root| windows_browser_executable_path(root, browser))
            .find(|candidate| candidate.is_file())
            .or_else(|| {
                let app_alias = context
                    .local
                    .join("Microsoft")
                    .join("WindowsApps")
                    .join(browser.windows_executable);
                if app_alias.is_file() {
                    Some(app_alias)
                } else {
                    None
                }
            }),
        NativeHostPlatform::Linux => browser.package.executables.iter().find_map(|name| {
            browser_package::native_executable_path(&context.browser_packages, name)
        }),
    }
}

fn windows_browser_executable_path(root: &Path, browser: &BrowserSpec) -> PathBuf {
    browser
        .windows_directory
        .iter()
        .fold(root.to_path_buf(), |path, component| path.join(component))
        .join(browser.windows_executable)
}

fn select_browsers(ids: &[String]) -> Result<Vec<BrowserSpec>, NativeHostError> {
    let mut selected = Vec::new();
    for id in ids {
        let browser = BROWSERS
            .iter()
            .find(|browser| browser.id == id)
            .copied()
            .ok_or_else(|| NativeHostError::UnknownBrowser(id.clone()))?;
        if !selected
            .iter()
            .any(|selected: &BrowserSpec| selected.id == browser.id)
        {
            selected.push(browser);
        }
    }
    Ok(selected)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct HostManifest {
    name: String,
    description: String,
    path: PathBuf,
    #[serde(rename = "type")]
    connection_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    allowed_origins: Vec<String>,
}

impl HostManifest {
    fn expected(connector: &Path) -> Self {
        Self {
            name: HOST_NAME.into(),
            description: HOST_DESCRIPTION.into(),
            path: normalize_path(connector),
            connection_type: "stdio".into(),
            allowed_origins: expected_origins(),
        }
    }

    fn owned(&self) -> bool {
        self.name == HOST_NAME
    }

    fn current(&self, expected: &Self, platform: NativeHostPlatform) -> bool {
        self.name == expected.name
            && self.description == expected.description
            && same_path(&self.path, &expected.path, platform)
            && self.connection_type == expected.connection_type
            && self.allowed_origins == expected.allowed_origins
    }

    fn to_json(&self) -> Result<String, NativeHostError> {
        Ok(serde_json::to_string_pretty(self).map_err(NativeHostError::Serialize)? + "\n")
    }
}

fn expected_origins() -> Vec<String> {
    [STORE_EXTENSION_ID, DEVELOPMENT_EXTENSION_ID]
        .into_iter()
        .map(|id| format!("chrome-extension://{id}/"))
        .collect()
}

/// Whether `bytes` is a native-messaging manifest that names Ghostlight's host.
pub(crate) fn names_ghostlight_host(bytes: &[u8]) -> bool {
    serde_json::from_slice::<HostManifest>(bytes).is_ok_and(|manifest| manifest.owned())
}

trait RegistrationIo {
    fn read_file(&self, path: &Path) -> Result<Option<String>, NativeHostError>;
    fn write_file(&self, path: &Path, contents: &str) -> Result<(), NativeHostError>;
    fn remove_file(&self, path: &Path) -> Result<(), NativeHostError>;
    /// Remove `path` when it is an empty directory; whether it was removed.
    fn remove_empty_directory(&self, path: &Path) -> Result<bool, NativeHostError>;
    fn read_registry(&self, key: &str) -> Result<Option<String>, NativeHostError>;
    fn write_registry(&self, key: &str, value: &str) -> Result<(), NativeHostError>;
    fn remove_registry(&self, key: &str) -> Result<(), NativeHostError>;
    /// Whether one path names an existing file, for dead-connector diagnosis.
    fn path_exists(&self, path: &Path) -> bool;
}

struct SystemRegistrationIo;

impl RegistrationIo for SystemRegistrationIo {
    fn read_file(&self, path: &Path) -> Result<Option<String>, NativeHostError> {
        match fs::read_to_string(path) {
            Ok(contents) => Ok(Some(contents)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(NativeHostError::Read {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    fn write_file(&self, path: &Path, contents: &str) -> Result<(), NativeHostError> {
        write_file_atomic(path, contents)
    }

    fn remove_file(&self, path: &Path) -> Result<(), NativeHostError> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(NativeHostError::Write {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    fn remove_empty_directory(&self, path: &Path) -> Result<bool, NativeHostError> {
        let empty = match fs::read_dir(path) {
            Ok(mut entries) => entries.next().is_none(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(source) => {
                return Err(NativeHostError::Read {
                    path: path.to_path_buf(),
                    source,
                })
            }
        };
        if !empty {
            return Ok(false);
        }
        match fs::remove_dir(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(NativeHostError::Write {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    fn read_registry(&self, key: &str) -> Result<Option<String>, NativeHostError> {
        read_registry_value(key)
    }

    fn write_registry(&self, key: &str, value: &str) -> Result<(), NativeHostError> {
        write_registry_value(key, value)
    }

    fn remove_registry(&self, key: &str) -> Result<(), NativeHostError> {
        remove_registry_key(key)
    }

    fn path_exists(&self, path: &Path) -> bool {
        path.is_file()
    }
}

fn inspect(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
) -> Result<NativeHostReport, NativeHostError> {
    let browsers = BROWSERS
        .iter()
        .map(|browser| {
            let (state, registered_connector) = inspect_browser(context, registration_io, browser)?;
            let package = browser_package::inspect(&context.browser_packages, browser.package);
            let detail = match state {
                NativeHostState::OwnedElsewhere => {
                    owned_elsewhere_detail(registration_io, registered_connector.as_deref())
                }
                _ => state_detail(state).into(),
            };
            Ok(BrowserRegistration {
                id: browser.id.into(),
                name: browser.name.into(),
                package,
                package_detail: browser_package::detail(browser.name, package),
                state,
                detail,
            })
        })
        .collect::<Result<Vec<_>, NativeHostError>>()?;
    Ok(NativeHostReport {
        connector: context.connector.clone(),
        browsers,
        private_view: context.private_registry().cloned(),
    })
}

fn inspect_browser(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
    browser: &BrowserSpec,
) -> Result<(NativeHostState, Option<PathBuf>), NativeHostError> {
    let expected = HostManifest::expected(&context.connector);
    match context.platform {
        NativeHostPlatform::Windows => {
            let Some(registered_path) =
                registration_io.read_registry(&context.registry_key(browser))?
            else {
                return Ok((NativeHostState::Missing, None));
            };
            let registered_path = PathBuf::from(registered_path);
            let contents = registration_io.read_file(&registered_path)?;
            classify_manifest(
                contents.as_deref(),
                &expected,
                context.platform,
                same_path(
                    &registered_path,
                    &browser_manifest_path(context, browser),
                    context.platform,
                ),
            )
        }
        NativeHostPlatform::Linux => {
            let path = browser_manifest_path(context, browser);
            classify_manifest(
                registration_io.read_file(&path)?.as_deref(),
                &expected,
                context.platform,
                true,
            )
        }
    }
}

/// Classify one registration and, when a Ghostlight-owned manifest named a connector, return
/// that connector path so the owning installation can be named in the diagnosis.
fn classify_manifest(
    contents: Option<&str>,
    expected: &HostManifest,
    platform: NativeHostPlatform,
    current_location: bool,
) -> Result<(NativeHostState, Option<PathBuf>), NativeHostError> {
    let absent = if current_location {
        NativeHostState::Missing
    } else {
        NativeHostState::NeedsAttention
    };
    let Some(contents) = contents else {
        return Ok((absent, None));
    };
    let manifest: HostManifest = match serde_json::from_str(contents) {
        Ok(manifest) => manifest,
        Err(_) => return Ok((NativeHostState::NeedsAttention, None)),
    };
    if !manifest.owned() {
        return Ok((NativeHostState::NeedsAttention, None));
    }
    let registered_connector = manifest.path.clone();
    let same_installation = manifest
        .path
        .parent()
        .zip(expected.path.parent())
        .is_some_and(|(registered, expected)| same_path(registered, expected, platform));
    Ok((
        if current_location && manifest.current(expected, platform) {
            NativeHostState::Current
        } else if !same_installation {
            NativeHostState::OwnedElsewhere
        } else {
            NativeHostState::Updatable
        },
        Some(registered_connector),
    ))
}

fn apply_install(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
) -> Result<NativeHostActionResult, NativeHostError> {
    apply_install_for(context, registration_io, BROWSERS)
}

fn apply_install_for(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
    browsers: &[BrowserSpec],
) -> Result<NativeHostActionResult, NativeHostError> {
    apply_install_for_mode(
        context,
        registration_io,
        browsers,
        InstallMode::InstallOrUpdate,
    )
}

fn apply_owned_repair_for(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
    browsers: &[BrowserSpec],
) -> Result<NativeHostActionResult, NativeHostError> {
    apply_install_for_mode(context, registration_io, browsers, InstallMode::OwnedRepair)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstallMode {
    InstallOrUpdate,
    OwnedRepair,
}

fn apply_install_for_mode(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
    browsers: &[BrowserSpec],
    mode: InstallMode,
) -> Result<NativeHostActionResult, NativeHostError> {
    if !context.connector.is_file() {
        return Err(NativeHostError::ConnectorMissing(context.connector.clone()));
    }
    let before = inspect(context, registration_io)?;
    if mode == InstallMode::OwnedRepair {
        for browser in browsers {
            let observed = before
                .browsers
                .iter()
                .find(|observed| observed.id == browser.id)
                .expect("every browser specification has an inspection result");
            if !matches!(
                observed.state,
                NativeHostState::Current | NativeHostState::Updatable
            ) {
                return Err(NativeHostError::OwnedRepairUnavailable(browser.id.into()));
            }
        }
    }
    if browsers.iter().all(|browser| {
        let observed = before
            .browsers
            .iter()
            .find(|observed| observed.id == browser.id)
            .expect("every browser specification has an inspection result");
        observed.state == NativeHostState::Current
            || (mode == InstallMode::InstallOrUpdate
                && observed.state == NativeHostState::NeedsAttention)
    }) {
        let superseded_changed = remove_superseded(context, registration_io)?;
        return Ok(NativeHostActionResult {
            changed: superseded_changed,
            report: before,
            private: Vec::new(),
        });
    }

    let observed_state = |browser: &BrowserSpec| {
        before
            .browsers
            .iter()
            .find(|observed| observed.id == browser.id)
            .expect("every browser specification has an inspection result")
            .state
    };
    let mut changed = false;
    let mut private = Vec::new();
    match context.platform {
        NativeHostPlatform::Windows => {
            let mut written_manifests = HashSet::new();
            for browser in browsers {
                if observed_state(browser) == NativeHostState::NeedsAttention {
                    continue;
                }
                let manifest_path = browser_manifest_path(context, browser);
                if written_manifests.insert(manifest_path.clone()) {
                    changed |= write_windows_manifest(context, registration_io, &manifest_path)?;
                }
                match register_manifest(context, registration_io, browser, &manifest_path)? {
                    Registration::Unchanged => {}
                    Registration::Written => changed = true,
                    Registration::KeptPrivate => private.push(browser.name.to_owned()),
                }
            }
        }
        NativeHostPlatform::Linux => {
            for browser in browsers {
                let state = observed_state(browser);
                if state != NativeHostState::NeedsAttention {
                    let expected = HostManifest::expected(&context.connector);
                    let contents = expected.to_json()?;
                    registration_io
                        .write_file(&browser_manifest_path(context, browser), &contents)?;
                }
                changed |= matches!(
                    state,
                    NativeHostState::Missing
                        | NativeHostState::Updatable
                        | NativeHostState::OwnedElsewhere
                );
            }
        }
    }
    let report = inspect(context, registration_io)?;
    if browsers.iter().any(|browser| {
        let observed = report
            .browsers
            .iter()
            .find(|observed| observed.id == browser.id)
            .expect("every browser specification has an inspection result");
        if private.iter().any(|name| name == browser.name) {
            return false;
        }
        match mode {
            InstallMode::InstallOrUpdate => matches!(
                observed.state,
                NativeHostState::Missing
                    | NativeHostState::Updatable
                    | NativeHostState::OwnedElsewhere
            ),
            InstallMode::OwnedRepair => observed.state != NativeHostState::Current,
        }
    }) {
        return Err(NativeHostError::VerificationFailed);
    }
    changed |= remove_superseded(context, registration_io)?;
    Ok(NativeHostActionResult {
        changed,
        report,
        private,
    })
}

/// What registering one browser's manifest path did.
enum Registration {
    /// The registry already names the manifest.
    Unchanged,
    /// The registry value was written.
    Written,
    /// The value differs, but this process runs inside an app container: a write would reach
    /// only that app's private view, so none was made.
    KeptPrivate,
}

/// Write the Windows manifest unless it already holds exactly the expected contents. A manifest
/// that is not Ghostlight's is never replaced.
fn write_windows_manifest(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
    manifest_path: &Path,
) -> Result<bool, NativeHostError> {
    let contents = HostManifest::expected(&context.connector).to_json()?;
    if let Some(existing) = registration_io.read_file(manifest_path)? {
        if existing == contents {
            return Ok(false);
        }
        let parsed = serde_json::from_str::<HostManifest>(&existing).ok();
        if !parsed.as_ref().is_some_and(HostManifest::owned) {
            return Err(NativeHostError::ForeignManifest(
                manifest_path.to_path_buf(),
            ));
        }
    }
    registration_io.write_file(manifest_path, &contents)?;
    Ok(true)
}

/// Point one browser's registry value at the manifest, reading it first and writing only when it
/// differs.
fn register_manifest(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
    browser: &BrowserSpec,
    manifest_path: &Path,
) -> Result<Registration, NativeHostError> {
    let key = context.registry_key(browser);
    let value = normalize_path(manifest_path).to_string_lossy().into_owned();
    if registration_io
        .read_registry(&key)?
        .is_some_and(|current| current.eq_ignore_ascii_case(&value))
    {
        return Ok(Registration::Unchanged);
    }
    if context.private_registry().is_some() {
        return Ok(Registration::KeptPrivate);
    }
    registration_io.write_registry(&key, &value)?;
    Ok(Registration::Written)
}

fn apply_uninstall(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
) -> Result<NativeHostActionResult, NativeHostError> {
    apply_uninstall_for(context, registration_io, BROWSERS)
}

fn apply_uninstall_for(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
    browsers: &[BrowserSpec],
) -> Result<NativeHostActionResult, NativeHostError> {
    if let Some(container) = context.private_registry() {
        return Err(NativeHostError::PrivateView(container::uninstall_refusal(
            container,
            &context.layout,
        )));
    }
    let mut changed = false;
    let mut owned_manifests = HashSet::new();
    match context.platform {
        NativeHostPlatform::Windows => {
            for browser in browsers {
                let key = context.registry_key(browser);
                let Some(value) = registration_io.read_registry(&key)? else {
                    continue;
                };
                let path = PathBuf::from(value);
                let owned = registration_io
                    .read_file(&path)?
                    .and_then(|contents| serde_json::from_str::<HostManifest>(&contents).ok())
                    .is_some_and(|manifest| manifest.owned());
                if owned {
                    registration_io.remove_registry(&key)?;
                    owned_manifests.insert(path);
                    changed = true;
                }
            }
            owned_manifests.insert(windows_manifest_path(context));
        }
        NativeHostPlatform::Linux => {
            for browser in browsers {
                let path = browser_manifest_path(context, browser);
                let owned = registration_io
                    .read_file(&path)?
                    .and_then(|contents| serde_json::from_str::<HostManifest>(&contents).ok())
                    .is_some_and(|manifest| manifest.owned());
                if owned {
                    registration_io.remove_file(&path)?;
                    changed = true;
                }
            }
        }
    }
    for path in owned_manifests {
        let mut still_referenced = false;
        for browser in BROWSERS {
            if registration_io
                .read_registry(&context.registry_key(browser))?
                .is_some_and(|value| {
                    same_path(&PathBuf::from(value), &path, NativeHostPlatform::Windows)
                })
            {
                still_referenced = true;
                break;
            }
        }
        let owned = registration_io
            .read_file(&path)?
            .and_then(|contents| serde_json::from_str::<HostManifest>(&contents).ok())
            .is_some_and(|manifest| manifest.owned());
        if !still_referenced && owned {
            registration_io.remove_file(&path)?;
            changed = true;
        }
    }
    changed |= remove_superseded(context, registration_io)?;
    if context.platform == NativeHostPlatform::Windows {
        registration_io.remove_empty_directory(&context.manifests)?;
    }
    Ok(NativeHostActionResult {
        changed,
        report: inspect(context, registration_io)?,
        private: Vec::new(),
    })
}

/// Remove what earlier versions registered and this version no longer uses: the retired Firefox
/// registration and, on Windows, the manifest in the old `AppData` directory once no browser
/// names it, then that directory when it is empty.
///
/// Nothing is removed from inside an app container: its view of `AppData` and the registry is
/// that app's alone, so what it sees there is not what other apps have.
fn remove_superseded(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
) -> Result<bool, NativeHostError> {
    if context.private_registry().is_some() {
        return Ok(false);
    }
    let mut changed = remove_retired_firefox_registration(context, registration_io)?;
    if context.platform == NativeHostPlatform::Windows {
        changed |= remove_legacy_windows_manifest(context, registration_io)?;
    }
    Ok(changed)
}

/// Remove the owned manifest in the old per-user `AppData` directory when no browser's registry
/// value still names it.
fn remove_legacy_windows_manifest(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
) -> Result<bool, NativeHostError> {
    let directory = legacy_windows_manifest_directory(context);
    let manifest = directory.join(format!("{HOST_NAME}.json"));
    let owned = registration_io
        .read_file(&manifest)?
        .and_then(|contents| serde_json::from_str::<HostManifest>(&contents).ok())
        .is_some_and(|manifest| manifest.owned());
    let mut changed = false;
    if owned {
        let mut referenced = false;
        for browser in BROWSERS {
            referenced |= registration_io
                .read_registry(&context.registry_key(browser))?
                .is_some_and(|value| {
                    same_path(
                        &PathBuf::from(value),
                        &manifest,
                        NativeHostPlatform::Windows,
                    )
                });
        }
        if !referenced {
            registration_io.remove_file(&manifest)?;
            changed = true;
        }
    }
    changed |= registration_io.remove_empty_directory(&directory)?;
    Ok(changed)
}

/// Remove only the retired Firefox registration when its manifest still proves Ghostlight
/// ownership. This bounded upgrade cleanup is not an active Firefox install path (ADR-0183).
fn remove_retired_firefox_registration(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
) -> Result<bool, NativeHostError> {
    let manifest_path = match context.platform {
        NativeHostPlatform::Windows => {
            legacy_windows_manifest_directory(context).join(format!("{HOST_NAME}.firefox.json"))
        }
        NativeHostPlatform::Linux => context
            .home
            .join(".mozilla")
            .join("native-messaging-hosts")
            .join(format!("{HOST_NAME}.json")),
    };
    let owned = registration_io
        .read_file(&manifest_path)?
        .and_then(|contents| serde_json::from_str::<HostManifest>(&contents).ok())
        .is_some_and(|manifest| manifest.owned());
    if !owned {
        return Ok(false);
    }

    if context.platform == NativeHostPlatform::Windows {
        let registry_key = if context.registry_isolated {
            format!(r"Software\Ghostlight\Isolated\Mozilla\NativeMessagingHosts\{HOST_NAME}")
        } else {
            format!(r"Software\Mozilla\NativeMessagingHosts\{HOST_NAME}")
        };
        if let Some(value) = registration_io.read_registry(&registry_key)? {
            let registered_path = PathBuf::from(value);
            let registered_owned = registration_io
                .read_file(&registered_path)?
                .and_then(|contents| serde_json::from_str::<HostManifest>(&contents).ok())
                .is_some_and(|manifest| manifest.owned());
            if registered_owned {
                registration_io.remove_registry(&registry_key)?;
            }
        }
    }
    registration_io.remove_file(&manifest_path)?;
    Ok(true)
}

fn state_detail(state: NativeHostState) -> &'static str {
    match state {
        NativeHostState::Missing => "Ghostlight is not registered with this browser.",
        NativeHostState::Current => "The browser points at this installation's connector.",
        NativeHostState::Updatable => "A Ghostlight-owned registration points at older details.",
        NativeHostState::OwnedElsewhere => {
            "A Ghostlight-owned registration belongs to another installation."
        }
        NativeHostState::NeedsAttention => {
            "Malformed or foreign state was found and will not be changed."
        }
    }
}

/// Name the installation that owns a cross-tree registration, and say when that installation
/// no longer exists -- the state a deleted scratch tree leaves behind.
fn owned_elsewhere_detail(
    registration_io: &dyn RegistrationIo,
    registered_connector: Option<&Path>,
) -> String {
    let Some(connector) = registered_connector else {
        return state_detail(NativeHostState::OwnedElsewhere).into();
    };
    let location = connector.parent().map_or_else(
        || connector.display().to_string(),
        |dir| dir.display().to_string(),
    );
    if registration_io.path_exists(connector) {
        format!("Another Ghostlight installation at {location} owns this registration.")
    } else {
        format!("A removed Ghostlight installation at {location} still owns this registration.")
    }
}

/// The one Windows manifest every browser's registry value names: `home/manifests`.
fn windows_manifest_path(context: &NativeHostContext) -> PathBuf {
    context.manifests.join(format!("{HOST_NAME}.json"))
}

/// The per-user `AppData` directory earlier versions kept the Windows manifest in. Packaged apps
/// see a private copy of it, which is why the manifest moved below the Ghostlight home.
fn legacy_windows_manifest_directory(context: &NativeHostContext) -> PathBuf {
    context
        .local
        .join("Ghostlight")
        .join("NativeMessagingHosts")
}

fn browser_manifest_path(context: &NativeHostContext, browser: &BrowserSpec) -> PathBuf {
    if context.platform == NativeHostPlatform::Windows {
        return windows_manifest_path(context);
    }
    context
        .config
        .join(browser.linux_directory)
        .join(format!("{HOST_NAME}.json"))
}

fn windows_registry_key(browser: &BrowserSpec) -> String {
    format!(
        r"Software\{}\NativeMessagingHosts\{HOST_NAME}",
        browser.windows_vendor.join(r"\")
    )
}

fn executable_name(name: &str, platform: NativeHostPlatform) -> String {
    if platform == NativeHostPlatform::Windows {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let rendered = canonical.to_string_lossy();
    let stripped = rendered
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| rendered.strip_prefix(r"\\?\").map(str::to_string));
    stripped.map_or(canonical, PathBuf::from)
}

fn same_path(left: &Path, right: &Path, platform: NativeHostPlatform) -> bool {
    let left = normalize_path(left).to_string_lossy().into_owned();
    let right = normalize_path(right).to_string_lossy().into_owned();
    if platform == NativeHostPlatform::Windows {
        left.eq_ignore_ascii_case(&right)
    } else {
        left == right
    }
}

fn write_file_atomic(path: &Path, contents: &str) -> Result<(), NativeHostError> {
    let path = &super::resolve_through_symlink(path).map_err(|source| NativeHostError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    let parent = path
        .parent()
        .ok_or_else(|| NativeHostError::InvalidPath(path.to_path_buf()))?;
    fs::create_dir_all(parent).map_err(|source| NativeHostError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let temporary = parent.join(format!(
        ".ghostlight-native-host-{}.tmp",
        Uuid::new_v4().simple()
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| NativeHostError::Write {
            path: temporary.clone(),
            source,
        })?;
    file.write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|source| NativeHostError::Write {
            path: temporary.clone(),
            source,
        })?;
    if let Err(source) = fs::rename(&temporary, path) {
        if path.exists() {
            fs::remove_file(path).map_err(|source| NativeHostError::Write {
                path: path.to_path_buf(),
                source,
            })?;
            fs::rename(&temporary, path).map_err(|source| NativeHostError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        } else {
            let _ = fs::remove_file(&temporary);
            return Err(NativeHostError::Write {
                path: path.to_path_buf(),
                source,
            });
        }
    }
    Ok(())
}

#[cfg(windows)]
fn read_registry_value(key: &str) -> Result<Option<String>, NativeHostError> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    match RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(key, KEY_READ) {
        Ok(key) => key
            .get_value("")
            .map(Some)
            .map_err(|error| NativeHostError::Registry(error.to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(NativeHostError::Registry(error.to_string())),
    }
}

#[cfg(not(windows))]
fn read_registry_value(_key: &str) -> Result<Option<String>, NativeHostError> {
    Err(NativeHostError::UnsupportedRegistry)
}

#[cfg(windows)]
fn write_registry_value(key: &str, value: &str) -> Result<(), NativeHostError> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_WRITE};
    use winreg::RegKey;

    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey_with_flags(key, KEY_WRITE)
        .map_err(|error| NativeHostError::Registry(error.to_string()))?;
    key.set_value("", &value)
        .map_err(|error| NativeHostError::Registry(error.to_string()))
}

#[cfg(not(windows))]
fn write_registry_value(_key: &str, _value: &str) -> Result<(), NativeHostError> {
    Err(NativeHostError::UnsupportedRegistry)
}

#[cfg(windows)]
fn remove_registry_key(key: &str) -> Result<(), NativeHostError> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(key) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(NativeHostError::Registry(error.to_string())),
    }
}

#[cfg(not(windows))]
fn remove_registry_key(_key: &str) -> Result<(), NativeHostError> {
    Err(NativeHostError::UnsupportedRegistry)
}

/// Safe native-host lifecycle failure.
#[derive(Debug, Error)]
pub enum NativeHostError {
    /// A command named a browser outside the closed supported set.
    #[error("unknown browser '{0}'; expected chrome, edge, brave, or chromium")]
    UnknownBrowser(String),
    /// The packaged sibling browser connector is unavailable.
    #[error("the sibling Ghostlight browser connector is missing: {0}")]
    ConnectorMissing(PathBuf),
    /// A registration file could not be read.
    #[error("could not read native-host state at {path}: {source}")]
    Read {
        /// File that could not be read.
        path: PathBuf,
        /// Operating-system error.
        source: io::Error,
    },
    /// A registration file could not be written or removed.
    #[error("could not update native-host state at {path}: {source}")]
    Write {
        /// File that could not be changed.
        path: PathBuf,
        /// Operating-system error.
        source: io::Error,
    },
    /// The manifest could not be encoded.
    #[error("could not encode the native-host manifest: {0}")]
    Serialize(serde_json::Error),
    /// Windows registry access failed.
    #[error("could not update the per-user browser registration: {0}")]
    Registry(String),
    /// A non-Windows build was asked to touch the Windows registry.
    #[error("Windows registry access is unavailable on this operating system")]
    UnsupportedRegistry,
    /// A foreign manifest occupies the current product location.
    #[error("a foreign manifest was left untouched at {0}")]
    ForeignManifest(PathBuf),
    /// Recovery revalidation no longer proves that the named state is owned and stale.
    #[error(
        "browser '{0}' is not an owned stale registration; automatic recovery left it unchanged"
    )]
    OwnedRepairUnavailable(String),
    /// An action completed without reaching the required exact state.
    #[error("native-host registration did not reach the expected current state")]
    VerificationFailed,
    /// A target has no parent directory.
    #[error("native-host path has no parent: {0}")]
    InvalidPath(PathBuf),
    /// The change would reach only the private view of the app container this process runs in.
    #[error("{0}")]
    PrivateView(String),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    #[derive(Default)]
    struct MemoryIo {
        files: Mutex<HashMap<PathBuf, String>>,
        directories: Mutex<HashSet<PathBuf>>,
        registry: Mutex<HashMap<String, String>>,
        registry_writes: Mutex<Vec<String>>,
    }

    impl MemoryIo {
        fn files(&self) -> MutexGuard<'_, HashMap<PathBuf, String>> {
            self.files.lock().unwrap()
        }

        fn registry(&self) -> MutexGuard<'_, HashMap<String, String>> {
            self.registry.lock().unwrap()
        }

        /// The registry keys written so far, in order.
        fn registry_writes(&self) -> Vec<String> {
            self.registry_writes.lock().unwrap().clone()
        }

        fn has_directory(&self, path: &Path) -> bool {
            self.directories.lock().unwrap().contains(path)
        }
    }

    impl RegistrationIo for MemoryIo {
        fn read_file(&self, path: &Path) -> Result<Option<String>, NativeHostError> {
            Ok(self.files().get(path).cloned())
        }

        fn write_file(&self, path: &Path, contents: &str) -> Result<(), NativeHostError> {
            self.files().insert(path.to_path_buf(), contents.into());
            if let Some(parent) = path.parent() {
                self.directories
                    .lock()
                    .unwrap()
                    .insert(parent.to_path_buf());
            }
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> Result<(), NativeHostError> {
            self.files().remove(path);
            Ok(())
        }

        fn remove_empty_directory(&self, path: &Path) -> Result<bool, NativeHostError> {
            if self.files().keys().any(|file| file.starts_with(path)) {
                return Ok(false);
            }
            Ok(self.directories.lock().unwrap().remove(path))
        }

        fn read_registry(&self, key: &str) -> Result<Option<String>, NativeHostError> {
            Ok(self.registry().get(key).cloned())
        }

        fn write_registry(&self, key: &str, value: &str) -> Result<(), NativeHostError> {
            self.registry_writes.lock().unwrap().push(key.into());
            self.registry().insert(key.into(), value.into());
            Ok(())
        }

        fn remove_registry(&self, key: &str) -> Result<(), NativeHostError> {
            self.registry().remove(key);
            Ok(())
        }

        fn path_exists(&self, path: &Path) -> bool {
            self.files().contains_key(path)
        }
    }

    fn symlink_probe_directory(name: &str) -> PathBuf {
        let directory = env::temp_dir().join(format!(
            "ghostlight-native-host-{name}-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[cfg(unix)]
    #[test]
    fn write_file_atomic_writes_through_a_symlinked_registration_file() {
        use std::os::unix::fs::symlink;

        let directory = symlink_probe_directory("symlink-unix");
        let real = directory.join("real-registration.json");
        fs::write(&real, "old").unwrap();
        let link = directory.join("registration.json");
        symlink(&real, &link).unwrap();

        write_file_atomic(&link, "new").unwrap();

        // The link is untouched -- still a symlink, still pointing at the same real file.
        // Before write_file_atomic resolved through the link, the rename inside it unlinked the
        // link and left a plain file in its place, orphaning whatever the link pointed to.
        let metadata = fs::symlink_metadata(&link).unwrap();
        assert!(metadata.file_type().is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        assert_eq!(fs::read_to_string(&link).unwrap(), "new");
    }

    #[cfg(windows)]
    #[test]
    fn write_file_atomic_writes_through_a_symlinked_registration_file() {
        use std::os::windows::fs::symlink_file;

        let directory = symlink_probe_directory("symlink-windows");
        let real = directory.join("real-registration.json");
        fs::write(&real, "old").unwrap();
        let link = directory.join("registration.json");
        if symlink_file(&real, &link).is_err() {
            // No symlink privilege (Developer Mode / an elevated shell) in this environment.
            // The equivalent Unix test above exercises the same write_file_atomic logic.
            return;
        }

        write_file_atomic(&link, "new").unwrap();

        let metadata = fs::symlink_metadata(&link).unwrap();
        assert!(metadata.file_type().is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        assert_eq!(fs::read_to_string(&link).unwrap(), "new");
    }

    fn context(platform: NativeHostPlatform) -> NativeHostContext {
        let root = env::temp_dir().join(format!(
            "ghostlight-native-host-{}-{}",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        let local = root.join("AppData/Local");
        let layout = Layout::new(root.join("Ghostlight"));
        NativeHostContext {
            platform,
            home: root.clone(),
            config: root.join(".config"),
            local: local.clone(),
            manifests: layout.manifests(),
            layout,
            connector: root
                .join("Ghostlight/bin")
                .join(executable_name(EDGE_COMMAND_NAME, platform)),
            browser_packages: BrowserPackageContext::isolated(&root.join("browser-packages")),
            windows_browser_roots: vec![
                local,
                root.join("Program Files"),
                root.join("Program Files (x86)"),
            ],
            registry_isolated: false,
            residence: Residence::Regular,
        }
    }

    #[test]
    fn an_explicit_native_host_root_isolates_both_platform_roots() {
        let (home, local, config, isolated) = native_host_roots(
            Some(PathBuf::from("/isolated")),
            Some(PathBuf::from("/real/Local")),
            Some(PathBuf::from("/real/config")),
            Path::new("/home/test"),
        );
        assert_eq!(home, PathBuf::from("/isolated"));
        assert_eq!(local, PathBuf::from("/isolated"));
        assert_eq!(config, PathBuf::from("/isolated"));
        assert!(isolated);

        let (home, local, config, isolated) = native_host_roots(
            None,
            Some(PathBuf::from("/real/Local")),
            Some(PathBuf::from("/real/config")),
            Path::new("/home/test"),
        );
        assert_eq!(home, PathBuf::from("/home/test"));
        assert_eq!(local, PathBuf::from("/real/Local"));
        assert_eq!(config, PathBuf::from("/real/config"));
        assert!(!isolated);

        let (home, local, config, isolated) =
            native_host_roots(None, None, None, Path::new("/home/test"));
        assert_eq!(home, PathBuf::from("/home/test"));
        assert_eq!(local, PathBuf::from("/home/test/AppData/Local"));
        assert_eq!(config, PathBuf::from("/home/test/.config"));
        assert!(!isolated);
    }

    #[test]
    fn an_isolated_context_writes_registry_keys_only_ghostlight_owns() {
        let mut isolated = context(NativeHostPlatform::Windows);
        isolated.registry_isolated = true;
        assert_eq!(
            isolated.registry_key(&BROWSERS[0]),
            r"Software\Ghostlight\Isolated\Google\Chrome\NativeMessagingHosts\org.sylin.ghostlight"
        );
        let real = context(NativeHostPlatform::Windows);
        assert_eq!(
            real.registry_key(&BROWSERS[0]),
            r"Software\Google\Chrome\NativeMessagingHosts\org.sylin.ghostlight"
        );
    }

    #[test]
    fn manifest_keeps_both_established_extension_identities() {
        let chrome = HostManifest::expected(Path::new("/opt/ghostlight/browser"));
        assert_eq!(chrome.name, HOST_NAME);
        assert_eq!(chrome.connection_type, "stdio");
        assert_eq!(
            chrome.allowed_origins,
            vec![
                format!("chrome-extension://{STORE_EXTENSION_ID}/"),
                format!("chrome-extension://{DEVELOPMENT_EXTENSION_ID}/"),
            ]
        );
    }

    #[test]
    fn install_removes_only_owned_retired_firefox_registration() {
        for platform in [NativeHostPlatform::Windows, NativeHostPlatform::Linux] {
            let context = context(platform);
            let registration_io = MemoryIo::default();
            fs::create_dir_all(context.connector.parent().unwrap()).unwrap();
            fs::write(&context.connector, b"connector").unwrap();

            let retired_manifest = match platform {
                NativeHostPlatform::Windows => context
                    .local
                    .join("Ghostlight/NativeMessagingHosts/org.sylin.ghostlight.firefox.json"),
                NativeHostPlatform::Linux => context
                    .home
                    .join(".mozilla/native-messaging-hosts/org.sylin.ghostlight.json"),
            };
            registration_io.files().insert(
                retired_manifest.clone(),
                HostManifest::expected(&context.connector)
                    .to_json()
                    .unwrap(),
            );
            if platform == NativeHostPlatform::Windows {
                registration_io.registry().insert(
                    r"Software\Mozilla\NativeMessagingHosts\org.sylin.ghostlight".into(),
                    retired_manifest.to_string_lossy().into_owned(),
                );
            }

            let result = apply_install(&context, &registration_io).unwrap();
            assert!(result.changed);
            assert!(!registration_io.files().contains_key(&retired_manifest));
            assert!(!registration_io
                .registry()
                .contains_key(r"Software\Mozilla\NativeMessagingHosts\org.sylin.ghostlight"));

            let _ = fs::remove_dir_all(context.home);
        }
    }

    #[test]
    fn layouts_cover_the_four_browser_families_exactly() {
        let linux = NativeHostContext {
            platform: NativeHostPlatform::Linux,
            home: PathBuf::from("/home/test"),
            config: PathBuf::from("/home/test/.config"),
            local: PathBuf::from(r"C:\Users\test\AppData\Local"),
            layout: Layout::new("/home/test/.ghostlight"),
            manifests: PathBuf::from("/home/test/.ghostlight/manifests"),
            connector: PathBuf::from("/opt/ghostlight/ghostlight"),
            browser_packages: BrowserPackageContext::isolated(Path::new("/browser-packages")),
            windows_browser_roots: Vec::new(),
            registry_isolated: false,
            residence: Residence::Regular,
        };
        let windows = NativeHostContext {
            platform: NativeHostPlatform::Windows,
            home: PathBuf::from("/home/test"),
            config: PathBuf::from("/home/test/.config"),
            local: PathBuf::from(r"C:\Users\test\AppData\Local"),
            layout: Layout::new("/home/test/.ghostlight"),
            manifests: PathBuf::from("/home/test/.ghostlight/manifests"),
            connector: PathBuf::from(r"C:\Users\test\.ghostlight\bin\ghostlight.exe"),
            browser_packages: BrowserPackageContext::isolated(Path::new("/browser-packages")),
            windows_browser_roots: Vec::new(),
            registry_isolated: false,
            residence: Residence::Regular,
        };
        assert_eq!(BROWSERS.len(), 4);
        assert_eq!(
            browser_manifest_path(&linux, &BROWSERS[0]),
            PathBuf::from(
                "/home/test/.config/google-chrome/NativeMessagingHosts/org.sylin.ghostlight.json"
            )
        );
        assert_eq!(
            windows_registry_key(&BROWSERS[2]),
            r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\org.sylin.ghostlight"
        );
        assert_eq!(
            browser_manifest_path(&windows, &BROWSERS[0]),
            windows_manifest_path(&windows)
        );
    }

    #[test]
    fn user_data_directories_follow_the_table_on_both_platforms() {
        let windows = context(NativeHostPlatform::Windows);
        let linux = context(NativeHostPlatform::Linux);
        let windows_tails = [
            "Google/Chrome/User Data",
            "Microsoft/Edge/User Data",
            "BraveSoftware/Brave-Browser/User Data",
            "Chromium/User Data",
        ];
        let linux_tails = [
            "google-chrome",
            "microsoft-edge",
            "BraveSoftware/Brave-Browser",
            "chromium",
        ];
        for (index, browser) in BROWSERS.iter().enumerate() {
            let on_windows = user_data_directory(&windows, browser);
            assert!(on_windows.starts_with(&windows.local), "{}", browser.id);
            assert!(on_windows.ends_with(windows_tails[index]), "{}", browser.id);
            let on_linux = user_data_directory(&linux, browser);
            assert!(on_linux.starts_with(&linux.config), "{}", browser.id);
            assert!(on_linux.ends_with(linux_tails[index]), "{}", browser.id);
        }
    }

    #[test]
    fn the_registry_answers_user_data_and_identities_in_table_order() {
        let root = env::temp_dir().join("ghostlight-user-data-registry");
        let registry = NativeHostRegistry::isolated(&root, &root.join("ghostlight"));

        assert_eq!(
            registry.user_data_directory("brave"),
            Some(root.join(".config/BraveSoftware/Brave-Browser"))
        );
        assert_eq!(registry.user_data_directory("firefox"), None);
        let identities: Vec<(&str, &str)> = registry
            .browser_identities()
            .iter()
            .map(|browser| (browser.id, browser.short))
            .collect();
        assert_eq!(
            identities,
            [
                ("chrome", "Chrome"),
                ("edge", "Edge"),
                ("brave", "Brave"),
                ("chromium", "Chromium")
            ]
        );
        assert_eq!(registry.browser_identities()[0].name, "Google Chrome");
    }

    #[test]
    fn windows_executable_discovery_uses_only_fixed_roots_for_all_families() {
        let context = context(NativeHostPlatform::Windows);
        let outside = context.home.join("Portable");
        let decoy = windows_browser_executable_path(&outside, &BROWSERS[0]);
        fs::create_dir_all(decoy.parent().unwrap()).unwrap();
        fs::write(&decoy, b"portable browser").unwrap();
        assert_eq!(browser_executable(&context, &BROWSERS[0]), None);

        let placements = [(0, 0), (1, 1), (2, 2), (3, 0)];
        for (browser_index, root_index) in placements {
            let browser = &BROWSERS[browser_index];
            let expected = windows_browser_executable_path(
                &context.windows_browser_roots[root_index],
                browser,
            );
            fs::create_dir_all(expected.parent().unwrap()).unwrap();
            fs::write(&expected, b"ordinary browser").unwrap();
            assert_eq!(browser_executable(&context, browser), Some(expected));
        }

        fs::remove_dir_all(context.home).unwrap();
    }

    #[test]
    fn same_tree_stale_is_updatable_and_another_installation_is_owned_elsewhere() {
        let expected = HostManifest::expected(Path::new("/opt/ghost/browser-connector"));
        let mut stale = expected.clone();
        stale.description = "Ghostlight connector (old)".into();
        assert_eq!(
            classify_manifest(
                Some(&stale.to_json().unwrap()),
                &expected,
                NativeHostPlatform::Linux,
                true
            )
            .unwrap()
            .0,
            NativeHostState::Updatable
        );
        let elsewhere = HostManifest::expected(Path::new("/other/tree/browser-connector"))
            .to_json()
            .unwrap();
        let (state, registered) =
            classify_manifest(Some(&elsewhere), &expected, NativeHostPlatform::Linux, true)
                .unwrap();
        assert_eq!(state, NativeHostState::OwnedElsewhere);
        assert_eq!(
            registered.as_deref(),
            Some(Path::new("/other/tree/browser-connector"))
        );
        assert_eq!(
            classify_manifest(
                Some(r#"{"name":"org.example.foreign"}"#),
                &expected,
                NativeHostPlatform::Linux,
                true
            )
            .unwrap()
            .0,
            NativeHostState::NeedsAttention
        );
        assert_eq!(
            classify_manifest(Some("not json"), &expected, NativeHostPlatform::Linux, true)
                .unwrap()
                .0,
            NativeHostState::NeedsAttention
        );
    }

    #[test]
    fn a_registration_owned_elsewhere_is_diagnosed_then_adopted_only_by_install() {
        let context = context(NativeHostPlatform::Windows);
        let registration_io = MemoryIo::default();
        fs::create_dir_all(context.connector.parent().unwrap()).unwrap();
        fs::write(&context.connector, b"connector").unwrap();
        // Joined components, not a backslash literal: Path::parent must split this path on
        // every host operating system, not only on Windows.
        let other_connector = context.local.join("other-tree").join("ghostlight.exe");
        let manifest_path = context.local.join("Ghostlight-old/host.json");
        registration_io.files().insert(
            manifest_path.clone(),
            HostManifest::expected(&other_connector).to_json().unwrap(),
        );
        // The other installation still exists, so the diagnosis names it as alive.
        registration_io
            .files()
            .insert(other_connector.clone(), "connector".into());
        for browser in BROWSERS {
            registration_io.registry().insert(
                windows_registry_key(browser),
                manifest_path.to_string_lossy().into_owned(),
            );
        }

        let observed = inspect(&context, &registration_io).unwrap();
        assert!(observed
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::OwnedElsewhere));
        assert!(
            observed.browsers.iter().all(|browser| browser
                .detail
                .contains("Another Ghostlight installation at"))
                && observed
                    .browsers
                    .iter()
                    .all(|browser| browser.detail.contains("other-tree"))
        );

        // The owning installation is deleted: the dead-path diagnosis replaces the alive one.
        registration_io.files().remove(&other_connector);
        let removed = inspect(&context, &registration_io).unwrap();
        assert!(removed.browsers.iter().all(|browser| browser
            .detail
            .contains("A removed Ghostlight installation at")));

        // Recovery's ownership-checked repair refuses; only a deliberate install adopts.
        let refused = apply_owned_repair_for(&context, &registration_io, &[BROWSERS[0]]);
        assert!(matches!(
            refused,
            Err(NativeHostError::OwnedRepairUnavailable(_))
        ));
        assert!(apply_install(&context, &registration_io).unwrap().changed);
        assert!(inspect(&context, &registration_io)
            .unwrap()
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Current));

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn unix_install_is_idempotent_and_uninstall_removes_only_owned_files() {
        let context = context(NativeHostPlatform::Linux);
        let registration_io = MemoryIo::default();
        fs::create_dir_all(context.connector.parent().unwrap()).unwrap();
        fs::write(&context.connector, b"connector").unwrap();

        let first = apply_install(&context, &registration_io).unwrap();
        assert!(first.changed);
        assert!(first
            .report
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Current));
        assert!(!apply_install(&context, &registration_io).unwrap().changed);

        let foreign_path = browser_manifest_path(&context, &BROWSERS[0]);
        registration_io.files().insert(
            foreign_path.clone(),
            r#"{"name":"org.example.foreign"}"#.into(),
        );
        let removed = apply_uninstall(&context, &registration_io).unwrap();
        assert!(removed.changed);
        assert!(registration_io.files().contains_key(&foreign_path));
        assert_eq!(
            removed.report.browsers[0].state,
            NativeHostState::NeedsAttention
        );

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn windows_install_repairs_owned_old_paths_and_refuses_unowned_ones() {
        let context = context(NativeHostPlatform::Windows);
        let registration_io = MemoryIo::default();
        fs::create_dir_all(context.connector.parent().unwrap()).unwrap();
        fs::write(&context.connector, b"connector").unwrap();
        let old_path = context.local.join("Ghostlight-old/host.json");
        registration_io.files().insert(
            old_path.clone(),
            HostManifest::expected(Path::new(r"C:\old\ghostlight.exe"))
                .to_json()
                .unwrap(),
        );
        for browser in BROWSERS {
            registration_io.registry().insert(
                windows_registry_key(browser),
                old_path.to_string_lossy().into_owned(),
            );
        }

        assert!(apply_install(&context, &registration_io).unwrap().changed);
        assert!(inspect(&context, &registration_io)
            .unwrap()
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Current));

        let foreign_path = context.local.join("foreign.json");
        registration_io.files().insert(
            foreign_path.clone(),
            r#"{"name":"org.example.foreign"}"#.into(),
        );
        registration_io.registry().insert(
            windows_registry_key(&BROWSERS[0]),
            foreign_path.to_string_lossy().into_owned(),
        );
        registration_io
            .registry()
            .remove(&windows_registry_key(&BROWSERS[1]));
        let partial = apply_install(&context, &registration_io).unwrap();
        assert!(partial.changed);
        assert_eq!(
            partial.report.browsers[0].state,
            NativeHostState::NeedsAttention
        );
        assert_eq!(partial.report.browsers[1].state, NativeHostState::Current);

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn windows_owned_repair_revalidates_before_writing() {
        let context = context(NativeHostPlatform::Windows);
        let registration_io = MemoryIo::default();
        fs::create_dir_all(context.connector.parent().unwrap()).unwrap();
        fs::write(&context.connector, b"connector").unwrap();
        let browser = BROWSERS[0];
        let key = windows_registry_key(&browser);
        let current_manifest = windows_manifest_path(&context);

        assert!(matches!(
            apply_owned_repair_for(&context, &registration_io, &[browser]),
            Err(NativeHostError::OwnedRepairUnavailable(id)) if id == browser.id
        ));
        assert!(registration_io.registry().is_empty());
        assert!(!registration_io.files().contains_key(&current_manifest));

        let occupied = context.local.join("foreign.json");
        registration_io
            .registry()
            .insert(key.clone(), occupied.to_string_lossy().into_owned());
        registration_io
            .files()
            .insert(occupied.clone(), r#"{"name":"org.example.foreign"}"#.into());
        assert!(matches!(
            apply_owned_repair_for(&context, &registration_io, &[browser]),
            Err(NativeHostError::OwnedRepairUnavailable(id)) if id == browser.id
        ));
        assert_eq!(
            registration_io.registry().get(&key),
            Some(&occupied.to_string_lossy().into_owned())
        );
        assert_eq!(
            registration_io.files().get(&occupied).map(String::as_str),
            Some(r#"{"name":"org.example.foreign"}"#)
        );
        assert!(!registration_io.files().contains_key(&current_manifest));

        // Same-tree stale details are the state recovery may silently repair; a connector in
        // another installation is OwnedElsewhere and is pinned by its own test.
        let stale_connector = context.connector.with_file_name("ghostlight-old.exe");
        registration_io.files().insert(
            occupied,
            HostManifest::expected(&stale_connector).to_json().unwrap(),
        );
        let repaired = apply_owned_repair_for(&context, &registration_io, &[browser]).unwrap();
        assert!(repaired.changed);
        assert_eq!(repaired.report.browsers[0].state, NativeHostState::Current);
        assert_eq!(
            registration_io.registry().get(&key),
            Some(&current_manifest.to_string_lossy().into_owned())
        );
        assert!(
            !apply_owned_repair_for(&context, &registration_io, &[browser])
                .unwrap()
                .changed
        );

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn windows_uninstall_removes_an_unreferenced_owned_shared_manifest() {
        let context = context(NativeHostPlatform::Windows);
        let registration_io = MemoryIo::default();
        let manifest_path = windows_manifest_path(&context);
        registration_io.files().insert(
            manifest_path.clone(),
            HostManifest::expected(&context.connector)
                .to_json()
                .unwrap(),
        );

        let result = apply_uninstall(&context, &registration_io).unwrap();

        assert!(result.changed);
        assert!(!registration_io.files().contains_key(&manifest_path));
        assert!(result
            .report
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Missing));

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn selected_unix_install_and_uninstall_touch_only_named_browsers() {
        let context = context(NativeHostPlatform::Linux);
        let registration_io = MemoryIo::default();
        fs::create_dir_all(context.connector.parent().unwrap()).unwrap();
        fs::write(&context.connector, b"connector").unwrap();

        let installed = apply_install_for(&context, &registration_io, &[BROWSERS[0]]).unwrap();
        assert_eq!(installed.report.browsers[0].state, NativeHostState::Current);
        assert!(installed.report.browsers[1..]
            .iter()
            .all(|browser| browser.state == NativeHostState::Missing));

        apply_install_for(&context, &registration_io, &[BROWSERS[1]]).unwrap();
        let removed = apply_uninstall_for(&context, &registration_io, &[BROWSERS[0]]).unwrap();
        assert_eq!(removed.report.browsers[0].state, NativeHostState::Missing);
        assert_eq!(removed.report.browsers[1].state, NativeHostState::Current);
        assert!(select_browsers(&["firefox".into()]).is_err());
        assert!(select_browsers(&["safari".into()]).is_err());

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn selected_windows_uninstall_preserves_a_shared_manifest_still_in_use() {
        let context = context(NativeHostPlatform::Windows);
        let registration_io = MemoryIo::default();
        fs::create_dir_all(context.connector.parent().unwrap()).unwrap();
        fs::write(&context.connector, b"connector").unwrap();
        apply_install_for(&context, &registration_io, &BROWSERS[..2]).unwrap();

        let removed = apply_uninstall_for(&context, &registration_io, &[BROWSERS[0]]).unwrap();
        assert_eq!(removed.report.browsers[0].state, NativeHostState::Missing);
        assert_eq!(removed.report.browsers[1].state, NativeHostState::Current);
        assert!(registration_io
            .files()
            .contains_key(&windows_manifest_path(&context)));

        let _ = fs::remove_dir_all(context.home);
    }

    fn windows_context_with_connector() -> NativeHostContext {
        let context = context(NativeHostPlatform::Windows);
        fs::create_dir_all(context.connector.parent().unwrap()).unwrap();
        fs::write(&context.connector, b"connector").unwrap();
        context
    }

    fn inside_container(mut context: NativeHostContext, app: &str) -> NativeHostContext {
        context.residence = Residence::AppContainer(AppContainer { app: app.into() });
        context
    }

    fn browser_names() -> Vec<String> {
        BROWSERS
            .iter()
            .map(|browser| browser.name.to_owned())
            .collect()
    }

    #[test]
    fn the_windows_manifest_lives_in_the_home_and_every_browser_names_it() {
        let context = windows_context_with_connector();
        let registration_io = MemoryIo::default();

        let result = apply_install(&context, &registration_io).unwrap();

        let manifest = windows_manifest_path(&context);
        assert!(manifest.starts_with(context.layout.home()));
        assert_eq!(
            manifest,
            context.layout.manifests().join("org.sylin.ghostlight.json")
        );
        assert!(registration_io.files().contains_key(&manifest));
        for browser in BROWSERS {
            assert_eq!(
                registration_io
                    .registry()
                    .get(&context.registry_key(browser)),
                Some(&manifest.to_string_lossy().into_owned()),
                "{}",
                browser.id
            );
        }
        assert!(result.changed);
        assert!(result.private.is_empty());
        assert_eq!(result.report.private_view, None);

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn a_registry_value_is_written_only_when_it_differs() {
        let context = windows_context_with_connector();
        let registration_io = MemoryIo::default();
        apply_install(&context, &registration_io).unwrap();
        assert_eq!(registration_io.registry_writes().len(), BROWSERS.len());

        // A stale manifest is rewritten; the registry already names it, so it is not touched.
        let manifest = windows_manifest_path(&context);
        let mut stale = HostManifest::expected(&context.connector);
        stale.description = "Ghostlight connector (old)".into();
        registration_io
            .files()
            .insert(manifest.clone(), stale.to_json().unwrap());
        let repaired = apply_install(&context, &registration_io).unwrap();
        assert!(repaired.changed);
        assert_eq!(registration_io.registry_writes().len(), BROWSERS.len());
        assert!(repaired
            .report
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Current));

        assert!(!apply_install(&context, &registration_io).unwrap().changed);
        assert_eq!(registration_io.registry_writes().len(), BROWSERS.len());

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn inside_a_container_the_manifest_is_written_and_no_registry_value_is() {
        let context = inside_container(windows_context_with_connector(), "Claude");
        let registration_io = MemoryIo::default();

        let result = apply_install(&context, &registration_io).unwrap();

        assert!(registration_io
            .files()
            .contains_key(&windows_manifest_path(&context)));
        assert!(registration_io.registry_writes().is_empty());
        assert!(registration_io.registry().is_empty());
        assert!(result.changed);
        assert_eq!(result.private, browser_names());
        assert_eq!(
            result.report.private_view,
            Some(AppContainer {
                app: "Claude".into()
            })
        );
        assert!(result
            .report
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Missing));

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn inside_a_container_an_install_that_finds_the_registration_writes_nothing_private() {
        let regular = windows_context_with_connector();
        let registration_io = MemoryIo::default();
        apply_install(&regular, &registration_io).unwrap();
        let writes = registration_io.registry_writes().len();

        // The manifest is stale but the registry value is right: only the manifest changes.
        let manifest = windows_manifest_path(&regular);
        let mut stale = HostManifest::expected(&regular.connector);
        stale.description = "Ghostlight connector (old)".into();
        registration_io
            .files()
            .insert(manifest.clone(), stale.to_json().unwrap());
        let context = inside_container(regular, "Codex");
        let result = apply_install(&context, &registration_io).unwrap();

        assert!(result.changed);
        assert!(result.private.is_empty());
        assert_eq!(registration_io.registry_writes().len(), writes);
        assert!(result
            .report
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Current));
        assert!(!apply_install(&context, &registration_io).unwrap().changed);

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn inside_a_container_an_uninstall_is_refused_before_anything_changes() {
        let context = inside_container(windows_context_with_connector(), "Claude");
        let registration_io = MemoryIo::default();
        let manifest = windows_manifest_path(&context);
        registration_io.files().insert(
            manifest.clone(),
            HostManifest::expected(&context.connector)
                .to_json()
                .unwrap(),
        );

        let refused = apply_uninstall(&context, &registration_io).unwrap_err();

        assert!(matches!(refused, NativeHostError::PrivateView(_)));
        assert!(refused.to_string().contains("Claude"));
        assert!(refused.to_string().ends_with("uninstall"));
        assert!(registration_io.files().contains_key(&manifest));

        let _ = fs::remove_dir_all(context.home);
    }

    /// Plant an owned manifest in the old per-user `AppData` directory and point `browsers` at it.
    fn plant_legacy_manifest(
        context: &NativeHostContext,
        registration_io: &MemoryIo,
        browsers: &[BrowserSpec],
    ) -> PathBuf {
        let legacy = legacy_windows_manifest_directory(context).join("org.sylin.ghostlight.json");
        registration_io.files().insert(
            legacy.clone(),
            HostManifest::expected(&context.connector)
                .to_json()
                .unwrap(),
        );
        registration_io
            .directories
            .lock()
            .unwrap()
            .insert(legacy.parent().unwrap().to_path_buf());
        for browser in browsers {
            registration_io.registry().insert(
                context.registry_key(browser),
                legacy.to_string_lossy().into_owned(),
            );
        }
        legacy
    }

    #[test]
    fn the_old_appdata_manifest_is_repointed_removed_and_its_directory_goes() {
        let context = windows_context_with_connector();
        let registration_io = MemoryIo::default();
        let legacy = plant_legacy_manifest(&context, &registration_io, BROWSERS);
        let before = inspect(&context, &registration_io).unwrap();
        assert!(before
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Updatable));

        let result = apply_install(&context, &registration_io).unwrap();

        assert!(result.changed);
        let manifest = windows_manifest_path(&context);
        for browser in BROWSERS {
            assert_eq!(
                registration_io
                    .registry()
                    .get(&context.registry_key(browser)),
                Some(&manifest.to_string_lossy().into_owned())
            );
        }
        assert!(!registration_io.files().contains_key(&legacy));
        assert!(!registration_io.has_directory(legacy.parent().unwrap()));
        assert!(result
            .report
            .browsers
            .iter()
            .all(|browser| browser.state == NativeHostState::Current));

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn the_old_appdata_directory_stays_while_it_holds_anything_else() {
        let context = windows_context_with_connector();
        let registration_io = MemoryIo::default();
        let legacy = plant_legacy_manifest(&context, &registration_io, BROWSERS);
        let other = legacy.with_file_name("notes.txt");
        registration_io.files().insert(other.clone(), "mine".into());

        apply_install(&context, &registration_io).unwrap();

        assert!(!registration_io.files().contains_key(&legacy));
        assert!(registration_io.files().contains_key(&other));
        assert!(registration_io.has_directory(legacy.parent().unwrap()));

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn the_old_manifest_stays_while_a_browser_outside_the_install_still_names_it() {
        let context = windows_context_with_connector();
        let registration_io = MemoryIo::default();
        let legacy = plant_legacy_manifest(&context, &registration_io, BROWSERS);

        apply_install_for(&context, &registration_io, &[BROWSERS[0]]).unwrap();

        assert!(registration_io.files().contains_key(&legacy));
        assert_eq!(
            registration_io
                .registry()
                .get(&context.registry_key(&BROWSERS[1])),
            Some(&legacy.to_string_lossy().into_owned())
        );

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn inside_a_container_the_old_manifest_and_its_registry_values_are_left_alone() {
        let context = inside_container(windows_context_with_connector(), "Claude");
        let registration_io = MemoryIo::default();
        let legacy = plant_legacy_manifest(&context, &registration_io, BROWSERS);

        let result = apply_install(&context, &registration_io).unwrap();

        assert!(registration_io.files().contains_key(&legacy));
        assert!(registration_io.has_directory(legacy.parent().unwrap()));
        assert!(registration_io.registry_writes().is_empty());
        assert_eq!(result.private, browser_names());
        // The manifest in the home is ready for the install that runs outside the container.
        assert!(registration_io
            .files()
            .contains_key(&windows_manifest_path(&context)));

        let _ = fs::remove_dir_all(context.home);
    }

    #[test]
    fn uninstall_removes_the_empty_manifests_directory() {
        let context = windows_context_with_connector();
        let registration_io = MemoryIo::default();
        apply_install(&context, &registration_io).unwrap();

        apply_uninstall(&context, &registration_io).unwrap();

        assert!(!registration_io.has_directory(&context.manifests));

        let _ = fs::remove_dir_all(context.home);
    }
}
