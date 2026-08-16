//! Ownership-checked per-user Chromium native-messaging registration.

use std::collections::HashSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use super::browser_package::{self, BrowserPackage, BrowserPackageContext, BrowserPackageSpec};

/// Stable native-messaging identity used by every supported Chromium browser.
pub const HOST_NAME: &str = "org.sylin.ghostlight";
/// Public Chrome Web Store extension identity.
pub const STORE_EXTENSION_ID: &str = "lejccfmoeogmhemakeknjjdhkfkgncdl";
/// Unpacked development extension identity pinned by `extension/manifest.json`.
pub const DEVELOPMENT_EXTENSION_ID: &str = "cjcmhepmagomefjggkcohdbfemacojoa";

const HOST_DESCRIPTION: &str = "Ghostlight browser connector";
const CONNECTOR_NAME: &str = "ghostlight-browser-connector";

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
    /// A Ghostlight-owned registration is present but carries stale details.
    Updatable,
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

    /// Reconcile a package whose ordinary desktop launch runs in an identifiable final location.
    ///
    /// Windows NSIS performs this before launch. Linux Debian installs use the first normal user
    /// launch so a stale per-user 0.8 manifest cannot shadow the package-owned registration.
    pub fn reconcile_packaged_launch(
        &self,
    ) -> Result<Option<NativeHostActionResult>, NativeHostError> {
        if packaged_desktop_executable(&self.context.connector, self.context.platform) {
            self.install().map(Some)
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Debug)]
struct NativeHostContext {
    platform: NativeHostPlatform,
    #[cfg(test)]
    home: PathBuf,
    config: PathBuf,
    local: PathBuf,
    connector: PathBuf,
    browser_packages: BrowserPackageContext,
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
        let config = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let local = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"));
        let connector = env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_default()
            .join(executable_name(CONNECTOR_NAME, platform));
        let browser_packages =
            BrowserPackageContext::system(&home, platform == NativeHostPlatform::Linux);
        Self {
            platform,
            #[cfg(test)]
            home,
            config,
            local,
            connector: normalize_path(&connector),
            browser_packages,
        }
    }
}

#[derive(Clone, Copy)]
struct BrowserSpec {
    id: &'static str,
    name: &'static str,
    windows_vendor: &'static str,
    linux_directory: &'static str,
    package: BrowserPackageSpec,
}

const BROWSERS: &[BrowserSpec] = &[
    BrowserSpec {
        id: "chrome",
        name: "Google Chrome",
        windows_vendor: r"Google\Chrome",
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
        windows_vendor: r"Microsoft\Edge",
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
        windows_vendor: r"BraveSoftware\Brave-Browser",
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
        windows_vendor: "Chromium",
        linux_directory: "chromium/NativeMessagingHosts",
        package: BrowserPackageSpec {
            executables: &["chromium", "chromium-browser"],
            snap_executable: "chromium",
            flatpak_ids: &["org.chromium.Chromium"],
        },
    },
];

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

trait RegistrationIo {
    fn read_file(&self, path: &Path) -> Result<Option<String>, NativeHostError>;
    fn write_file(&self, path: &Path, contents: &str) -> Result<(), NativeHostError>;
    fn remove_file(&self, path: &Path) -> Result<(), NativeHostError>;
    fn read_registry(&self, key: &str) -> Result<Option<String>, NativeHostError>;
    fn write_registry(&self, key: &str, value: &str) -> Result<(), NativeHostError>;
    fn remove_registry(&self, key: &str) -> Result<(), NativeHostError>;
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

    fn read_registry(&self, key: &str) -> Result<Option<String>, NativeHostError> {
        read_registry_value(key)
    }

    fn write_registry(&self, key: &str, value: &str) -> Result<(), NativeHostError> {
        write_registry_value(key, value)
    }

    fn remove_registry(&self, key: &str) -> Result<(), NativeHostError> {
        remove_registry_key(key)
    }
}

fn inspect(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
) -> Result<NativeHostReport, NativeHostError> {
    let expected = HostManifest::expected(&context.connector);
    let browsers = BROWSERS
        .iter()
        .map(|browser| {
            let state = inspect_browser(context, registration_io, browser, &expected)?;
            let package = browser_package::inspect(&context.browser_packages, browser.package);
            Ok(BrowserRegistration {
                id: browser.id.into(),
                name: browser.name.into(),
                package,
                package_detail: browser_package::detail(browser.name, package),
                state,
                detail: state_detail(state).into(),
            })
        })
        .collect::<Result<Vec<_>, NativeHostError>>()?;
    Ok(NativeHostReport {
        connector: context.connector.clone(),
        browsers,
    })
}

fn inspect_browser(
    context: &NativeHostContext,
    registration_io: &dyn RegistrationIo,
    browser: &BrowserSpec,
    expected: &HostManifest,
) -> Result<NativeHostState, NativeHostError> {
    match context.platform {
        NativeHostPlatform::Windows => {
            let Some(registered_path) =
                registration_io.read_registry(&windows_registry_key(browser))?
            else {
                return Ok(NativeHostState::Missing);
            };
            let registered_path = PathBuf::from(registered_path);
            let contents = registration_io.read_file(&registered_path)?;
            classify_manifest(
                contents.as_deref(),
                expected,
                context.platform,
                same_path(
                    &registered_path,
                    &windows_manifest_path(context),
                    context.platform,
                ),
            )
        }
        NativeHostPlatform::Linux => {
            let path = browser_manifest_path(context, browser);
            classify_manifest(
                registration_io.read_file(&path)?.as_deref(),
                expected,
                context.platform,
                true,
            )
        }
    }
}

fn classify_manifest(
    contents: Option<&str>,
    expected: &HostManifest,
    platform: NativeHostPlatform,
    current_location: bool,
) -> Result<NativeHostState, NativeHostError> {
    let Some(contents) = contents else {
        return Ok(if current_location {
            NativeHostState::Missing
        } else {
            NativeHostState::NeedsAttention
        });
    };
    let manifest: HostManifest = match serde_json::from_str(contents) {
        Ok(manifest) => manifest,
        Err(_) => return Ok(NativeHostState::NeedsAttention),
    };
    if !manifest.owned() {
        return Ok(NativeHostState::NeedsAttention);
    }
    Ok(
        if current_location && manifest.current(expected, platform) {
            NativeHostState::Current
        } else {
            NativeHostState::Updatable
        },
    )
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
    if !context.connector.is_file() {
        return Err(NativeHostError::ConnectorMissing(context.connector.clone()));
    }
    let before = inspect(context, registration_io)?;
    if browsers.iter().all(|browser| {
        let observed = before
            .browsers
            .iter()
            .find(|observed| observed.id == browser.id)
            .expect("every browser specification has an inspection result");
        matches!(
            observed.state,
            NativeHostState::Current | NativeHostState::NeedsAttention
        )
    }) {
        return Ok(NativeHostActionResult {
            changed: false,
            report: before,
        });
    }

    let expected = HostManifest::expected(&context.connector);
    let contents = expected.to_json()?;
    let changed = browsers.iter().any(|browser| {
        let observed = before
            .browsers
            .iter()
            .find(|observed| observed.id == browser.id)
            .expect("every browser specification has an inspection result");
        matches!(
            observed.state,
            NativeHostState::Missing | NativeHostState::Updatable
        )
    });
    match context.platform {
        NativeHostPlatform::Windows => {
            let manifest_path = windows_manifest_path(context);
            if let Some(existing) = registration_io.read_file(&manifest_path)? {
                let parsed = serde_json::from_str::<HostManifest>(&existing).ok();
                if !parsed.as_ref().is_some_and(HostManifest::owned) {
                    return Err(NativeHostError::ForeignManifest(manifest_path));
                }
            }
            registration_io.write_file(&manifest_path, &contents)?;
            let manifest_value = manifest_path.to_string_lossy();
            for browser in browsers {
                let observed = before
                    .browsers
                    .iter()
                    .find(|observed| observed.id == browser.id)
                    .expect("every browser specification has an inspection result");
                if observed.state != NativeHostState::NeedsAttention {
                    registration_io
                        .write_registry(&windows_registry_key(browser), manifest_value.as_ref())?;
                }
            }
        }
        NativeHostPlatform::Linux => {
            for browser in browsers {
                let observed = before
                    .browsers
                    .iter()
                    .find(|observed| observed.id == browser.id)
                    .expect("every browser specification has an inspection result");
                if observed.state != NativeHostState::NeedsAttention {
                    registration_io
                        .write_file(&browser_manifest_path(context, browser), &contents)?;
                }
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
        matches!(
            observed.state,
            NativeHostState::Missing | NativeHostState::Updatable
        )
    }) {
        return Err(NativeHostError::VerificationFailed);
    }
    Ok(NativeHostActionResult { changed, report })
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
    let mut changed = false;
    let mut owned_manifests = HashSet::new();
    match context.platform {
        NativeHostPlatform::Windows => {
            for browser in browsers {
                let key = windows_registry_key(browser);
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
                .read_registry(&windows_registry_key(browser))?
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
    Ok(NativeHostActionResult {
        changed,
        report: inspect(context, registration_io)?,
    })
}

fn state_detail(state: NativeHostState) -> &'static str {
    match state {
        NativeHostState::Missing => "Ghostlight is not registered with this browser.",
        NativeHostState::Current => "The browser points at this installation's connector.",
        NativeHostState::Updatable => "A Ghostlight-owned registration points at older details.",
        NativeHostState::NeedsAttention => {
            "Malformed or foreign state was found and will not be changed."
        }
    }
}

fn windows_manifest_path(context: &NativeHostContext) -> PathBuf {
    context
        .local
        .join("Ghostlight")
        .join("NativeMessagingHosts")
        .join(format!("{HOST_NAME}.json"))
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
        browser.windows_vendor
    )
}

fn executable_name(name: &str, platform: NativeHostPlatform) -> String {
    if platform == NativeHostPlatform::Windows {
        format!("{name}.exe")
    } else {
        name.into()
    }
}

fn packaged_desktop_executable(connector: &Path, platform: NativeHostPlatform) -> bool {
    let Some(directory) = connector.parent() else {
        return false;
    };
    match platform {
        NativeHostPlatform::Windows => false,
        NativeHostPlatform::Linux => directory == Path::new("/usr/bin"),
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
    /// An action completed without reaching the required exact state.
    #[error("native-host registration did not reach the expected current state")]
    VerificationFailed,
    /// A target has no parent directory.
    #[error("native-host path has no parent: {0}")]
    InvalidPath(PathBuf),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    #[derive(Default)]
    struct MemoryIo {
        files: Mutex<HashMap<PathBuf, String>>,
        registry: Mutex<HashMap<String, String>>,
    }

    impl MemoryIo {
        fn files(&self) -> MutexGuard<'_, HashMap<PathBuf, String>> {
            self.files.lock().unwrap()
        }

        fn registry(&self) -> MutexGuard<'_, HashMap<String, String>> {
            self.registry.lock().unwrap()
        }
    }

    impl RegistrationIo for MemoryIo {
        fn read_file(&self, path: &Path) -> Result<Option<String>, NativeHostError> {
            Ok(self.files().get(path).cloned())
        }

        fn write_file(&self, path: &Path, contents: &str) -> Result<(), NativeHostError> {
            self.files().insert(path.to_path_buf(), contents.into());
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> Result<(), NativeHostError> {
            self.files().remove(path);
            Ok(())
        }

        fn read_registry(&self, key: &str) -> Result<Option<String>, NativeHostError> {
            Ok(self.registry().get(key).cloned())
        }

        fn write_registry(&self, key: &str, value: &str) -> Result<(), NativeHostError> {
            self.registry().insert(key.into(), value.into());
            Ok(())
        }

        fn remove_registry(&self, key: &str) -> Result<(), NativeHostError> {
            self.registry().remove(key);
            Ok(())
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
        NativeHostContext {
            platform,
            home: root.clone(),
            config: root.join(".config"),
            local: root.join("AppData/Local"),
            connector: root
                .join("Ghostlight/bin")
                .join(executable_name(CONNECTOR_NAME, platform)),
            browser_packages: BrowserPackageContext::isolated(&root.join("browser-packages")),
        }
    }

    #[test]
    fn manifest_keeps_both_established_extension_identities() {
        let expected = HostManifest::expected(Path::new("/opt/ghostlight/browser"));
        assert_eq!(expected.name, HOST_NAME);
        assert_eq!(expected.connection_type, "stdio");
        assert_eq!(
            expected.allowed_origins,
            vec![
                format!("chrome-extension://{STORE_EXTENSION_ID}/"),
                format!("chrome-extension://{DEVELOPMENT_EXTENSION_ID}/"),
            ]
        );
    }

    #[test]
    fn layouts_cover_the_four_browser_families_exactly() {
        let linux = NativeHostContext {
            platform: NativeHostPlatform::Linux,
            home: PathBuf::from("/home/test"),
            config: PathBuf::from("/home/test/.config"),
            local: PathBuf::from(r"C:\Users\test\AppData\Local"),
            connector: PathBuf::from("/opt/ghostlight/ghostlight-browser-connector"),
            browser_packages: BrowserPackageContext::isolated(Path::new("/browser-packages")),
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
    }

    #[test]
    fn only_final_package_locations_trigger_first_launch_reconciliation() {
        assert!(packaged_desktop_executable(
            Path::new("/usr/bin/ghostlight-browser-connector"),
            NativeHostPlatform::Linux
        ));
        assert!(!packaged_desktop_executable(
            Path::new("/repo/target/debug/ghostlight-browser-connector"),
            NativeHostPlatform::Linux
        ));
        assert!(!packaged_desktop_executable(
            Path::new(r"C:\Program Files\Ghostlight\ghostlight-browser-connector.exe"),
            NativeHostPlatform::Windows
        ));
    }

    #[test]
    fn owned_stale_state_is_updatable_but_foreign_state_needs_attention() {
        let expected = HostManifest::expected(Path::new("/current/browser"));
        let stale = HostManifest::expected(Path::new("/old/browser"))
            .to_json()
            .unwrap();
        assert_eq!(
            classify_manifest(Some(&stale), &expected, NativeHostPlatform::Linux, true).unwrap(),
            NativeHostState::Updatable
        );
        assert_eq!(
            classify_manifest(
                Some(r#"{"name":"org.example.foreign"}"#),
                &expected,
                NativeHostPlatform::Linux,
                true
            )
            .unwrap(),
            NativeHostState::NeedsAttention
        );
        assert_eq!(
            classify_manifest(Some("not json"), &expected, NativeHostPlatform::Linux, true)
                .unwrap(),
            NativeHostState::NeedsAttention
        );
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
            HostManifest::expected(Path::new(r"C:\old\ghostlight-browser-connector.exe"))
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
}
