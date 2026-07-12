// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Native-messaging host registration: browser detection, the host manifest, and the per-OS
//! registration target (Windows registry key / macOS+Linux file drop).
//!
//! Path and key computation is pure (takes an injected [`PlanCtx`]) so it is unit-testable on any
//! OS; only the small apply primitives touch the registry / filesystem.

use super::{Hive, PlanCtx, Scope};
use crate::{Error, Result};
use ghostlight_transport::host::{host_name, HOST_DESCRIPTION};
use serde_json::json;
use std::path::{Path, PathBuf};

/// One supported Chromium browser and how to detect + register it per OS (doc 11 A.1-A.3).
pub struct BrowserSpec {
    pub id: &'static str,
    pub display: &'static str,
    /// Windows: subpath under `SOFTWARE\` before `\NativeMessagingHosts\<host>`.
    pub win_vendor: &'static str,
    /// Windows App Paths exe used for detection; `None` => detect only via the user-data dir.
    pub win_app_paths_exe: Option<&'static str>,
    /// Windows user-data subdir under `%LOCALAPPDATA%` (fallback detection).
    pub win_user_data_subdir: &'static str,
    /// macOS `.app` bundle path (detection).
    pub mac_app: &'static str,
    /// macOS host dir under `~/Library/Application Support/`.
    pub mac_host_subdir: &'static str,
    /// Linux host dir under `~/.config/` -- note the CamelCase `NativeMessagingHosts` tail.
    pub linux_user_subdir: &'static str,
    /// Linux config dir under `~/.config/` (fallback detection).
    pub linux_config_subdir: &'static str,
    /// Linux detection binaries (any on PATH => installed).
    pub linux_detect_bins: &'static [&'static str],
}

/// The v1 browser set (Chrome, Edge, Brave, Chromium). Vivaldi/Opera/Arc land in v1.1.
pub const BROWSERS: &[BrowserSpec] = &[
    BrowserSpec {
        id: "chrome",
        display: "Google Chrome",
        win_vendor: r"Google\Chrome",
        win_app_paths_exe: Some("chrome.exe"),
        win_user_data_subdir: r"Google\Chrome\User Data",
        mac_app: "/Applications/Google Chrome.app",
        mac_host_subdir: "Google/Chrome/NativeMessagingHosts",
        linux_user_subdir: "google-chrome/NativeMessagingHosts",
        linux_config_subdir: "google-chrome",
        linux_detect_bins: &["google-chrome", "google-chrome-stable"],
    },
    BrowserSpec {
        id: "edge",
        display: "Microsoft Edge",
        win_vendor: r"Microsoft\Edge",
        win_app_paths_exe: Some("msedge.exe"),
        win_user_data_subdir: r"Microsoft\Edge\User Data",
        mac_app: "/Applications/Microsoft Edge.app",
        mac_host_subdir: "Microsoft Edge/NativeMessagingHosts",
        linux_user_subdir: "microsoft-edge/NativeMessagingHosts",
        linux_config_subdir: "microsoft-edge",
        linux_detect_bins: &["microsoft-edge", "microsoft-edge-stable"],
    },
    BrowserSpec {
        id: "brave",
        display: "Brave",
        win_vendor: r"BraveSoftware\Brave-Browser",
        win_app_paths_exe: Some("brave.exe"),
        win_user_data_subdir: r"BraveSoftware\Brave-Browser\User Data",
        mac_app: "/Applications/Brave Browser.app",
        mac_host_subdir: "BraveSoftware/Brave-Browser/NativeMessagingHosts",
        linux_user_subdir: "BraveSoftware/Brave-Browser/NativeMessagingHosts",
        linux_config_subdir: "BraveSoftware/Brave-Browser",
        linux_detect_bins: &["brave-browser"],
    },
    BrowserSpec {
        id: "chromium",
        display: "Chromium",
        win_vendor: "Chromium",
        win_app_paths_exe: None, // Chromium has no App Paths entry -> detect only via user-data dir
        win_user_data_subdir: r"Chromium\User Data",
        mac_app: "/Applications/Chromium.app",
        mac_host_subdir: "Chromium/NativeMessagingHosts",
        linux_user_subdir: "chromium/NativeMessagingHosts",
        linux_config_subdir: "chromium",
        linux_detect_bins: &["chromium", "chromium-browser"],
    },
];

/// Find a browser spec by CLI id.
pub fn browser_by_id(id: &str) -> Option<&'static BrowserSpec> {
    BROWSERS.iter().find(|b| b.id == id)
}

// --- Extension id + origin ---

/// Validate a Chrome unpacked-dev extension id: exactly 32 chars, each in `a..=p` (doc 11 A.6).
pub fn validate_extension_id(id: &str) -> Result<()> {
    let ok = id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b));
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidExtensionId(id.to_string()))
    }
}

/// The Chrome Web Store extension id (the published "Ghostlight in Browser" listing).
pub const STORE_EXTENSION_ID: &str = "lejccfmoeogmhemakeknjjdhkfkgncdl";

/// The unpacked-dev extension id, pinned by the committed manifest `key` (ADR-0016).
pub const DEV_EXTENSION_ID: &str = "cjcmhepmagomefjggkcohdbfemacojoa";

/// The exact allowed origin for an extension id (trailing slash required; no wildcards, doc 11 A.5).
pub fn origin_for(id: &str) -> String {
    format!("chrome-extension://{id}/")
}

/// Normalize an exe path for Chrome: strip Windows verbatim prefixes and prefer an absolute path.
/// On canonicalize failure, fall back to the input (Chrome only needs *an* absolute path).
pub fn normalize_exe_path(p: &Path) -> PathBuf {
    let canon = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let s = canon.to_string_lossy();
    let stripped = s
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| s.strip_prefix(r"\\?\").map(str::to_string));
    match stripped {
        Some(v) => PathBuf::from(v),
        None => canon,
    }
}

/// The path of a sibling role executable next to the running one (ADR-0046): same directory,
/// platform suffix appended on Windows.
pub fn sibling_bin(current_exe: &Path, name: &str) -> PathBuf {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    normalize_exe_path(current_exe)
        .parent()
        .map(|d| d.join(&file))
        .unwrap_or_else(|| PathBuf::from(file))
}

// --- Host manifest ---

/// The native-messaging host manifest the installer generates (doc 11 A.0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostManifest {
    pub path: PathBuf,
    pub allowed_origins: Vec<String>,
}

impl HostManifest {
    /// Build from the binary path plus an OPTIONAL extra extension id (ADR-0065: one stack). The
    /// host serves BOTH known builds of the extension -- the Web Store [`STORE_EXTENSION_ID`] and
    /// the unpacked [`DEV_EXTENSION_ID`] (pinned by the committed manifest `key`, ADR-0016) -- so
    /// either connects to whatever engine currently holds the endpoint. `--extension-id` appends
    /// one more origin (validated, deduplicated) for a fork or an enterprise-packaged extension.
    pub fn resolve(current_exe: &Path, extension_id: Option<&str>) -> Result<Self> {
        let mut allowed_origins =
            vec![origin_for(STORE_EXTENSION_ID), origin_for(DEV_EXTENSION_ID)];
        if let Some(id) = extension_id {
            validate_extension_id(id)?;
            let origin = origin_for(id);
            if !allowed_origins.contains(&origin) {
                allowed_origins.push(origin);
            }
        }
        Ok(Self {
            path: normalize_exe_path(current_exe),
            allowed_origins,
        })
    }

    /// Emit the manifest JSON (name, description, path, `type: "stdio"`, allowed_origins) + newline.
    pub fn to_json(&self) -> String {
        let value = json!({
            "name": host_name(),
            "description": HOST_DESCRIPTION,
            "path": self.path.to_string_lossy(),
            "type": "stdio",
            "allowed_origins": self.allowed_origins,
        });
        serde_json::to_string_pretty(&value).expect("host manifest serializes") + "\n"
    }
}

// --- Registration targets (pure path/key computation) ---

/// Which registry view(s) to write (doc 11 A.0: 32-bit browser builds read WOW6432Node).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WowView {
    /// HKCU: unaffected by WOW redirection -- a single write suffices.
    Native,
    /// HKLM system writes: write both the 64-bit and 32-bit (WOW6432Node) views.
    Both,
}

/// Windows registry key (under the hive) for a browser's native host entry.
pub fn win_reg_key(spec: &BrowserSpec) -> String {
    format!(
        r"SOFTWARE\{}\NativeMessagingHosts\{}",
        spec.win_vendor,
        host_name()
    )
}

/// The (Windows) shared host-manifest file path -- one file per instance, referenced by every
/// browser's key. The dir leaf and the manifest file name both carry the active instance
/// (`ghostlight` / `org.sylin.ghostlight.json` for the default; `ghostlight-<n>` /
/// `org.sylin.ghostlight.<n>.json` for a named one), so instances never share a manifest.
pub fn win_manifest_path(ctx: &PlanCtx) -> PathBuf {
    ctx.local
        .join(ghostlight_transport::instance::Instance::resolve().dir_leaf())
        .join("NativeMessagingHosts")
        .join(format!("{}.json", host_name()))
}

/// macOS per-browser host-manifest file path.
pub fn mac_host_path(spec: &BrowserSpec, ctx: &PlanCtx) -> PathBuf {
    ctx.home
        .join("Library/Application Support")
        .join(spec.mac_host_subdir)
        .join(format!("{}.json", host_name()))
}

/// Linux per-browser host-manifest file path (user scope; CamelCase `NativeMessagingHosts` tail).
pub fn linux_host_path(spec: &BrowserSpec, ctx: &PlanCtx) -> PathBuf {
    ctx.config
        .join(spec.linux_user_subdir)
        .join(format!("{}.json", host_name()))
}

/// The native-host launcher for the active instance (ADR-0044 Decision 4 / ADR-0046): the path the
/// host manifest `path` field points at, plus whether the installer must place a per-instance copy.
///
/// The DEFAULT instance points the manifest straight at the `ghostlight-relay` sibling beside the
/// running binary -- no copy, byte-identical. A NON-DEFAULT instance points it at a per-instance copy
/// named `ghostlight-relay-<n>[.exe]` under that instance's data dir, because Chrome launches the
/// native host with a bare path and no argument room; the copied binary reads its own `argv[0]`
/// basename to know which instance it is (SPEC 7), and detects the browser role from the
/// `chrome-extension://` origin Chrome passes (ADR-0051 Phase 3). Only the tiny relay is ever copied,
/// never the multi-MB `ghostlight` brain. A stale copy is harmless (the native host is a dumb pipe;
/// only the service, which the installer launches via `--instance`, carries code).
pub fn instance_launcher(ctx: &PlanCtx) -> (PathBuf, bool) {
    let instance = ghostlight_transport::instance::Instance::resolve();
    if instance.is_default() {
        (sibling_bin(&ctx.current_exe, "ghostlight-relay"), false)
    } else {
        let name = instance.name().expect("a non-default instance has a name");
        let file_name = if cfg!(windows) {
            format!("ghostlight-relay-{name}.exe")
        } else {
            format!("ghostlight-relay-{name}")
        };
        let path = ctx.local.join(instance.dir_leaf()).join(file_name);
        (path, true)
    }
}

/// The hive to use for a scope.
pub fn hive_for(scope: Scope) -> Hive {
    match scope {
        Scope::User => Hive::Hkcu,
        Scope::System => Hive::Hklm,
    }
}

/// The WOW view(s) to write for a scope.
pub fn wow_for(scope: Scope) -> WowView {
    match scope {
        Scope::User => WowView::Native,
        Scope::System => WowView::Both,
    }
}

// --- Detection ---

/// Multi-signal detection: is this browser installed? (doc 11 A.1-A.3; Chromium relies on the
/// user-data-dir fallback since it has no App Paths entry.)
pub fn detect_browser(spec: &BrowserSpec, ctx: &PlanCtx) -> bool {
    if cfg!(windows) {
        // App Paths (when present) OR the user-data dir.
        let app_paths = spec
            .win_app_paths_exe
            .map(win_app_path_registered)
            .unwrap_or(false);
        app_paths || ctx.local.join(spec.win_user_data_subdir).is_dir()
    } else if cfg!(target_os = "macos") {
        Path::new(spec.mac_app).is_dir()
    } else {
        spec.linux_detect_bins.iter().any(|b| super::on_path(b))
            || ctx.config.join(spec.linux_config_subdir).is_dir()
    }
}

#[cfg(windows)]
fn win_app_path_registered(exe: &str) -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let key = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .iter()
        .any(|&root| RegKey::predef(root).open_subkey(&key).is_ok())
}

#[cfg(not(windows))]
fn win_app_path_registered(_exe: &str) -> bool {
    false
}

// --- Apply primitives (the only I/O in this module) ---

/// Write a file atomically (create parents, write a temp sibling, rename over the target). The temp
/// sibling appends `.tmp` to the full file name (`foo.json` -> `foo.json.tmp`) so it never collides
/// with a differently-extensioned file that shares the same stem.
pub fn write_file_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = super::append_extension(path, "tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Ownership of a host-manifest file: `None` = absent, `Some(true)` = ours (its `name` is our
/// HOST_NAME), `Some(false)` = present but owned by something else. Used to classify a removal at
/// plan time so a foreign manifest is reported as skipped rather than falsely as removed.
pub fn host_file_owner(path: &Path) -> Result<Option<bool>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let ours = serde_json::from_str::<serde_json::Value>(&contents)
                .ok()
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .is_some_and(|name| name == host_name());
            Ok(Some(ours))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Io(e)),
    }
}

/// Remove a host-manifest file, but only if it is ours (its `name` is our HOST_NAME). Returns
/// whether a file was actually deleted; a foreign or missing file is left in place (`Ok(false)`).
pub fn remove_host_file_if_ours(path: &Path) -> Result<bool> {
    if host_file_owner(path)? == Some(true) {
        std::fs::remove_file(path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
        KEY_WRITE,
    };
    use winreg::RegKey;

    fn root(hive: Hive) -> RegKey {
        RegKey::predef(match hive {
            Hive::Hkcu => HKEY_CURRENT_USER,
            Hive::Hklm => HKEY_LOCAL_MACHINE,
        })
    }
    fn views(wow: WowView) -> &'static [u32] {
        match wow {
            WowView::Native => &[0],
            WowView::Both => &[KEY_WOW64_64KEY, KEY_WOW64_32KEY],
        }
    }

    /// Read the current `(Default)` value of the key (across the relevant views), if any.
    pub fn read_default(hive: Hive, key: &str, wow: WowView) -> Option<String> {
        for &view in views(wow) {
            if let Ok(k) = root(hive).open_subkey_with_flags(key, KEY_READ | view) {
                if let Ok(v) = k.get_value::<String, _>("") {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Set the `(Default)` value of the key in every relevant view (create-or-open).
    pub fn set_default(hive: Hive, key: &str, wow: WowView, value: &str) -> Result<()> {
        for &view in views(wow) {
            let (k, _) = root(hive)
                .create_subkey_with_flags(key, KEY_WRITE | view)
                .map_err(|e| Error::HostRegistration(e.to_string()))?;
            k.set_value("", &value)
                .map_err(|e| Error::HostRegistration(e.to_string()))?;
        }
        Ok(())
    }

    /// Delete our host key (every view). Missing = ok. Never touches the vendor parent.
    pub fn delete_key(hive: Hive, key: &str, wow: WowView) -> Result<()> {
        for &view in views(wow) {
            match root(hive).open_subkey_with_flags(key, KEY_WRITE | view) {
                Ok(_) => root(hive)
                    .delete_subkey_all(key)
                    .map_err(|e| Error::HostRegistration(e.to_string()))?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Error::HostRegistration(e.to_string())),
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use win::{delete_key, read_default, set_default};

#[cfg(not(windows))]
pub fn read_default(_hive: Hive, _key: &str, _wow: WowView) -> Option<String> {
    None
}
#[cfg(not(windows))]
pub fn set_default(_hive: Hive, _key: &str, _wow: WowView, _value: &str) -> Result<()> {
    Err(Error::Unsupported(
        "windows registry on a non-windows OS".into(),
    ))
}
#[cfg(not(windows))]
pub fn delete_key(_hive: Hive, _key: &str, _wow: WowView) -> Result<()> {
    Err(Error::Unsupported(
        "windows registry on a non-windows OS".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> PlanCtx {
        PlanCtx {
            current_exe: PathBuf::from("/abs/ghostlight"),
            home: PathBuf::from("/home/u"),
            config: PathBuf::from("/home/u/.config"),
            local: PathBuf::from(r"C:\Users\u\AppData\Local"),
        }
    }

    #[test]
    fn instance_launcher_default_is_the_relay_sibling() {
        // The default instance never copies: the manifest points straight at the relay sibling
        // beside the running binary (ADR-0046 + ADR-0051 Phase 3). No GHOSTLIGHT_INSTANCE is set
        // here -- mutating it would race the parallel tests that call Instance::resolve.
        let (path, needs_copy) = instance_launcher(&ctx());
        assert!(
            !needs_copy,
            "the default instance places no per-instance copy"
        );
        let s = path.to_string_lossy();
        let suffix = if cfg!(windows) {
            "ghostlight-relay.exe"
        } else {
            "ghostlight-relay"
        };
        assert!(
            s.ends_with(suffix),
            "the default launcher is the relay sibling: {s}"
        );
    }

    #[test]
    fn host_manifest_json_has_type_stdio_and_exact_origin() {
        // ADR-0065: the host serves both known extension builds (store + unpacked dev);
        // `--extension-id` appends one more.
        let m = HostManifest::resolve(Path::new("/abs/ghostlight"), Some(&"a".repeat(32))).unwrap();
        let v: serde_json::Value = serde_json::from_str(&m.to_json()).unwrap();
        assert_eq!(v["name"], host_name());
        assert_eq!(v["type"], "stdio");
        let origins = v["allowed_origins"].as_array().unwrap();
        assert_eq!(origins.len(), 3);
        assert_eq!(
            origins[0],
            format!("chrome-extension://{STORE_EXTENSION_ID}/")
        );
        assert_eq!(
            origins[1],
            format!("chrome-extension://{DEV_EXTENSION_ID}/")
        );
        assert_eq!(
            origins[2],
            format!("chrome-extension://{}/", "a".repeat(32))
        );
    }

    #[test]
    fn extension_id_validation() {
        assert!(validate_extension_id(&"a".repeat(32)).is_ok());
        assert!(validate_extension_id(&"p".repeat(32)).is_ok());
        assert!(validate_extension_id(&"a".repeat(31)).is_err()); // too short
        assert!(validate_extension_id(&"q".repeat(32)).is_err()); // q > p
        assert!(validate_extension_id(&"A".repeat(32)).is_err()); // uppercase
        assert!(validate_extension_id(&"1".repeat(32)).is_err()); // digits
        assert!(validate_extension_id("").is_err());
    }

    /// ADR-0065: the host always serves both known extension builds (store + unpacked dev);
    /// `--extension-id` appends one more origin (validated, deduplicated).
    #[test]
    fn resolve_without_an_id_allows_both_known_extensions() {
        let m = HostManifest::resolve(Path::new("/x"), None).unwrap();
        assert_eq!(
            m.allowed_origins,
            vec![
                format!("chrome-extension://{STORE_EXTENSION_ID}/"),
                format!("chrome-extension://{DEV_EXTENSION_ID}/"),
            ]
        );
        // Re-passing an id the manifest already carries never duplicates.
        let dup = HostManifest::resolve(Path::new("/x"), Some(STORE_EXTENSION_ID)).unwrap();
        assert_eq!(dup.allowed_origins.len(), 2);
        let dup2 = HostManifest::resolve(Path::new("/x"), Some(DEV_EXTENSION_ID)).unwrap();
        assert_eq!(dup2.allowed_origins.len(), 2);
    }

    #[test]
    fn normalize_strips_windows_verbatim_prefixes() {
        assert_eq!(
            normalize_exe_path(Path::new(r"\\?\C:\x\ghostlight.exe")),
            PathBuf::from(r"C:\x\ghostlight.exe")
        );
        assert_eq!(
            normalize_exe_path(Path::new(r"\\?\UNC\srv\share\ghostlight.exe")),
            PathBuf::from(r"\\srv\share\ghostlight.exe")
        );
    }

    #[test]
    fn windows_reg_key_per_browser() {
        assert_eq!(
            win_reg_key(browser_by_id("chrome").unwrap()),
            format!(
                r"SOFTWARE\Google\Chrome\NativeMessagingHosts\{}",
                host_name()
            )
        );
        assert_eq!(
            win_reg_key(browser_by_id("brave").unwrap()),
            format!(
                r"SOFTWARE\BraveSoftware\Brave-Browser\NativeMessagingHosts\{}",
                host_name()
            )
        );
    }

    #[test]
    fn unix_paths_keep_the_casing_distinction() {
        let c = ctx();
        // Linux user tail is CamelCase NativeMessagingHosts.
        let linux = linux_host_path(browser_by_id("chrome").unwrap(), &c);
        assert!(linux.to_string_lossy().contains("NativeMessagingHosts"));
        assert!(linux.to_string_lossy().contains("google-chrome"));
        // macOS lives under Library/Application Support.
        let mac = mac_host_path(browser_by_id("chrome").unwrap(), &c);
        assert!(mac
            .to_string_lossy()
            .contains("Library/Application Support"));
    }

    #[test]
    fn wow_view_by_scope() {
        assert_eq!(wow_for(Scope::User), WowView::Native);
        assert_eq!(wow_for(Scope::System), WowView::Both);
    }

    #[test]
    fn write_atomic_creates_parents_overwrites_and_removes_only_ours() {
        let dir = std::env::temp_dir().join(format!("ghostlight-it-{}", std::process::id()));
        let path = dir.join("nested").join(format!("{}.json", host_name()));
        let ours = HostManifest::resolve(Path::new("/abs/ghostlight"), Some(&"a".repeat(32)))
            .unwrap()
            .to_json();

        // Absent -> owner is None.
        assert_eq!(host_file_owner(&path).unwrap(), None);
        // Writes through non-existent parent dirs; temp sibling appends .tmp (never left behind).
        write_file_atomic(&path, &ours).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ours);
        assert!(!crate::install::append_extension(&path, "tmp").exists());
        // Ours -> owner Some(true).
        assert_eq!(host_file_owner(&path).unwrap(), Some(true));
        // Overwrites in place (re-install).
        write_file_atomic(&path, &ours).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ours);
        // Ours (name == HOST_NAME) -> removed.
        assert!(remove_host_file_if_ours(&path).unwrap());
        assert!(!path.exists());
        // Missing -> ok, reports "not removed".
        assert!(!remove_host_file_if_ours(&path).unwrap());
        // A foreign host file is classified Some(false) and never deleted.
        std::fs::write(&path, r#"{"name":"com.example.other"}"#).unwrap();
        assert_eq!(host_file_owner(&path).unwrap(), Some(false));
        assert!(!remove_host_file_if_ours(&path).unwrap());
        assert!(path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
