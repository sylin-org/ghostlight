//! Which browser profiles hold the Ghostlight extension.
//!
//! The service cannot tell "the extension is added but the browser is closed" from "the extension
//! is missing" by asking the browser, because a closed browser answers nothing. A Chromium
//! browser keeps each profile in a directory of its user data directory and lists them in
//! `Local State`; an installed extension has a directory under the profile's `Extensions`. A
//! turned-off extension keeps that directory, and the profile's preferences mark it disabled, so
//! a profile holds the extension only when it has the directory and no preferences file marks it
//! disabled. A browser does not load a disabled extension, so a profile that only has it turned
//! off cannot connect. This module reads only those names and that one setting. It is local,
//! bounded and read-only, and it runs only when asked: nothing is written, no profile name is
//! logged, stored or sent, and a scan that cannot read what it needs falls back to the one
//! profile every browser starts with.
//!
//! An extension installed from the Chrome Web Store has the same id in Chrome, Edge, Brave and
//! Chromium, so one id finds them all. An unpacked development extension has another id and is
//! not found, by design.

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use serde_json::Value;

use super::native_host::{BrowserIdentity, NativeHostRegistry, STORE_EXTENSION_ID};

/// The profile every browser starts with, and the one a scan assumes when `Local State` cannot be
/// used.
const DEFAULT_PROFILE_DIRECTORY: &str = "Default";
/// The browser's index of profiles, in the user data directory.
const LOCAL_STATE_FILE: &str = "Local State";
/// Where a profile keeps its installed extensions, one directory per extension id.
const EXTENSIONS_DIRECTORY: &str = "Extensions";
/// The profile files that may hold an extension's settings: the protected copy the browser keeps
/// on some systems, and the plain one.
const PREFERENCES_FILES: [&str; 2] = ["Secure Preferences", "Preferences"];
/// The settings key whose value says why an extension is disabled: a number of flags in older
/// browsers, a list of reasons in newer ones. Absent, zero or empty means enabled.
const DISABLE_REASONS_KEY: &str = "disable_reasons";
/// The settings key older browsers used for an extension's state.
const STATE_KEY: &str = "state";
/// The [`STATE_KEY`] value of a disabled extension.
const DISABLED_STATE: i64 = 0;
/// The most of `Local State` that is read. A larger file is treated as unusable.
const LOCAL_STATE_LIMIT: u64 = 8 * 1024 * 1024;
/// The most of a preferences file that is read. A larger file is treated as unusable, and then
/// it marks nothing disabled.
const PREFERENCES_LIMIT: u64 = 32 * 1024 * 1024;
/// The longest profile directory name that is used.
const DIRECTORY_NAME_LIMIT: usize = 64;
/// The longest profile name that is shown.
const SHOWN_NAME_LIMIT: usize = 40;

/// A profile that holds the extension, turned on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionProfile {
    /// The profile's directory name inside the user data directory, such as `Profile 1`.
    pub directory: String,
    /// The name the person gave the profile, cleaned and bounded for display, when there is one.
    pub name: Option<String>,
}

/// What a scan of one browser's user data directory found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileScan {
    /// How many profiles the browser has.
    pub profiles: usize,
    /// The profiles that hold the extension turned on, in the order the browser lists its
    /// profiles. A profile where it is turned off is not one of them.
    pub with_extension: Vec<ExtensionProfile>,
}

/// The scan of one installed browser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserProfiles {
    /// The browser family.
    pub browser: BrowserIdentity,
    /// What its user data directory holds.
    pub scan: ProfileScan,
}

/// Scan every supported browser whose executable is found on this computer.
///
/// Read-only and bounded. Browsers that are not installed are left out.
#[must_use]
pub fn scan(registry: &NativeHostRegistry) -> Vec<BrowserProfiles> {
    registry
        .browser_identities()
        .into_iter()
        .filter(|browser| registry.browser_executable(browser.id).is_some())
        .filter_map(|browser| {
            let user_data = registry.user_data_directory(browser.id)?;
            Some(BrowserProfiles {
                browser,
                scan: scan_user_data(&user_data, STORE_EXTENSION_ID),
            })
        })
        .collect()
}

/// Find the profiles of the browser whose user data directory is `user_data` that hold the
/// extension `extension_id` turned on.
///
/// Never fails: a missing, oversize or unreadable `Local State` falls back to the `Default`
/// profile alone, counted when its directory exists and carrying no name. A preferences file
/// that cannot be read marks nothing disabled.
#[must_use]
pub fn scan_user_data(user_data: &Path, extension_id: &str) -> ProfileScan {
    let listed = read_listed_profiles(user_data).unwrap_or_else(|| fallback_profiles(user_data));
    let with_extension = listed
        .iter()
        .filter(|(directory, _)| {
            let profile = user_data.join(directory);
            profile
                .join(EXTENSIONS_DIRECTORY)
                .join(extension_id)
                .is_dir()
                && !marked_disabled(&profile, extension_id)
        })
        .map(|(directory, name)| ExtensionProfile {
            directory: directory.clone(),
            name: name.clone(),
        })
        .collect();
    ProfileScan {
        profiles: listed.len(),
        with_extension,
    }
}

/// The profiles `Local State` lists, in its order, each with its cleaned name.
///
/// `None` when the file is missing, too large, unreadable, not the expected shape or lists no
/// usable profile.
fn read_listed_profiles(user_data: &Path) -> Option<Vec<(String, Option<String>)>> {
    let bytes = read_bounded(&user_data.join(LOCAL_STATE_FILE), LOCAL_STATE_LIMIT)?;
    let state: LocalState = serde_json::from_slice(&bytes).ok()?;
    let listed: Vec<(String, Option<String>)> = state
        .profile?
        .info_cache?
        .0
        .into_iter()
        .filter(|(directory, _)| usable_directory(directory))
        .map(|(directory, entry)| {
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .and_then(clean_name);
            (directory, name)
        })
        .collect();
    (!listed.is_empty()).then_some(listed)
}

/// Whether a preferences file of `profile` marks the extension `extension_id` disabled.
fn marked_disabled(profile: &Path, extension_id: &str) -> bool {
    PREFERENCES_FILES.iter().any(|file| {
        read_bounded(&profile.join(file), PREFERENCES_LIMIT)
            .and_then(|bytes| serde_json::from_slice::<Preferences>(&bytes).ok())
            .and_then(|preferences| preferences.extensions?.settings?.remove(extension_id))
            .is_some_and(|settings| settings_say_disabled(&settings))
    })
}

/// Whether one extension's settings say it is disabled: a disable reason is recorded, or the
/// older state says disabled.
fn settings_say_disabled(settings: &Value) -> bool {
    let has_reason = match settings.get(DISABLE_REASONS_KEY) {
        Some(Value::Number(flags)) => flags.as_u64().is_none_or(|flags| flags != 0),
        Some(Value::Array(reasons)) => !reasons.is_empty(),
        _ => false,
    };
    has_reason || settings.get(STATE_KEY).and_then(Value::as_i64) == Some(DISABLED_STATE)
}

/// The bytes of the file at `path`, or `None` when it is missing, unreadable or larger than
/// `limit`.
fn read_bounded(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

/// The one profile assumed when `Local State` cannot be used.
fn fallback_profiles(user_data: &Path) -> Vec<(String, Option<String>)> {
    if user_data.join(DEFAULT_PROFILE_DIRECTORY).is_dir() {
        vec![(DEFAULT_PROFILE_DIRECTORY.to_owned(), None)]
    } else {
        Vec::new()
    }
}

/// Whether a key from `Local State` may be joined to the user data directory: one plain path
/// component of 1 to 64 characters, with no separator, parent reference or control character.
fn usable_directory(directory: &str) -> bool {
    let length = directory.chars().count();
    if !(1..=DIRECTORY_NAME_LIMIT).contains(&length)
        || directory.contains("..")
        || directory.contains(['/', '\\'])
        || directory.chars().any(char::is_control)
    {
        return false;
    }
    let mut components = Path::new(directory).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

/// A profile name made safe to show: control characters removed, trimmed, at most 40
/// characters. `None` when nothing is left.
fn clean_name(name: &str) -> Option<String> {
    let visible: String = name.chars().filter(|c| !c.is_control()).collect();
    let bounded: String = visible.trim().chars().take(SHOWN_NAME_LIMIT).collect();
    let bounded = bounded.trim();
    (!bounded.is_empty()).then(|| bounded.to_owned())
}

/// The parts of `Local State` a scan reads.
#[derive(Deserialize)]
struct LocalState {
    profile: Option<ProfileSection>,
}

#[derive(Deserialize)]
struct ProfileSection {
    info_cache: Option<InfoCache>,
}

/// The parts of a profile's preferences file a scan reads.
#[derive(Deserialize)]
struct Preferences {
    extensions: Option<ExtensionsSection>,
}

#[derive(Deserialize)]
struct ExtensionsSection {
    /// Each extension's settings, by extension id.
    settings: Option<serde_json::Map<String, Value>>,
}

/// The `info_cache` object as key and value pairs in file order. A JSON map type would sort or
/// hash the keys; the browser's order is the person's order.
struct InfoCache(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for InfoCache {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;

        impl<'de> Visitor<'de> for Entries {
            type Value = InfoCache;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object of profiles")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, Value>()? {
                    entries.push(entry);
                }
                Ok(InfoCache(entries))
            }
        }

        deserializer.deserialize_map(Entries)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::scratch::Scratch;

    const ID: &str = "abcdefghijklmnopabcdefghijklmnop";

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn install_extension(user_data: &Path, profile: &str) {
        fs::create_dir_all(user_data.join(profile).join("Extensions").join(ID)).unwrap();
    }

    fn local_state(user_data: &Path, content: &str) {
        write(&user_data.join("Local State"), content);
    }

    fn found(directory: &str, name: Option<&str>) -> ExtensionProfile {
        ExtensionProfile {
            directory: directory.to_owned(),
            name: name.map(str::to_owned),
        }
    }

    /// A `Local State` listing `entries`, given as JSON `"key":{...}` fragments.
    fn listing(entries: &str) -> String {
        format!(r#"{{"profile":{{"info_cache":{{{entries}}}}}}}"#)
    }

    const TWO_PROFILES: &str = r#"{"profile":{"info_cache":{
        "Default":{"name":"Person 1"},"Profile 1":{"name":"Work"}}}}"#;

    /// A profile file holding `settings` as the settings of the extension `id`.
    fn extension_settings(id: &str, settings: &str) -> String {
        format!(r#"{{"extensions":{{"settings":{{"{id}":{settings}}}}}}}"#)
    }

    /// A registry for `scratch` in which Chrome is installed.
    fn registry_with_chrome(scratch: &Scratch) -> NativeHostRegistry {
        let connector = scratch.path().join("ghostlight");
        let registry = NativeHostRegistry::isolated(scratch.path(), &connector);
        let executable = scratch
            .path()
            .join("browser-packages")
            .join("native-bin")
            .join("google-chrome");
        write(&executable, "binary");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        registry
    }

    /// ACCEPTANCE Desktop 23 and 24: the scan finds, in each installed browser, the profiles that
    /// hold the extension installed and turned on, in the order `Local State` lists them, so Open
    /// goes to a profile that can connect and a person with only a turned-off copy is sent to the
    /// store (defect 68aa9656). A browser with no data has no profile; without a usable
    /// `Local State` the Default profile stands alone and unnamed; and a profile key that could
    /// leave the user data directory is never joined to a path.
    #[test]
    fn a_profile_holds_the_extension_only_when_installed_and_turned_on() {
        let scratch = Scratch::new();
        local_state(
            scratch.path(),
            &listing(
                r#""Profile 3":{"name":"Zed"},"Default":{"name":"Ann"},"Profile 1":{"name":"Bo"},"Profile 2":{"name":"None"}"#,
            ),
        );
        for profile in ["Default", "Profile 1", "Profile 3"] {
            install_extension(scratch.path(), profile);
        }
        fs::create_dir_all(scratch.path().join("Profile 2")).unwrap();
        let seen = scan_user_data(scratch.path(), ID);
        assert_eq!(seen.profiles, 4);
        assert_eq!(
            seen.with_extension,
            vec![
                found("Profile 3", Some("Zed")),
                found("Default", Some("Ann")),
                found("Profile 1", Some("Bo")),
            ]
        );

        let absent = scan_user_data(&scratch.path().join("absent"), ID);
        assert_eq!((absent.profiles, absent.with_extension.len()), (0, 0));

        for content in [
            None,
            Some("{ not json".to_owned()),
            Some(r#"{"profile":{"info_cache":[1,2]}}"#.to_owned()),
            Some(listing("")),
            Some(format!(
                r#"{{"padding":"{}","profile":{{"info_cache":{{"Profile 1":{{"name":"Work"}}}}}}}}"#,
                "x".repeat(LOCAL_STATE_LIMIT as usize)
            )),
        ] {
            let scratch = Scratch::new();
            if let Some(content) = &content {
                local_state(scratch.path(), content);
            }
            install_extension(scratch.path(), "Default");
            install_extension(scratch.path(), "Profile 1");
            let seen = scan_user_data(scratch.path(), ID);
            assert_eq!(seen.profiles, 1, "{content:?}");
            assert_eq!(seen.with_extension, vec![found("Default", None)]);
        }

        let hostile = Scratch::new();
        let user_data = hostile.path().join("User Data");
        install_extension(hostile.path(), "outside");
        let long = "p".repeat(65);
        let keys = [
            "../outside",
            "..",
            ".",
            "a/b",
            "a\\b",
            "Pro\u{7}file",
            "",
            long.as_str(),
        ];
        let mut entries: Vec<String> = keys
            .iter()
            .map(|key| format!(r#"{}:{{"name":"x"}}"#, serde_json::to_string(key).unwrap()))
            .collect();
        entries.push(r#""Profile 1":{"name":"Work"}"#.to_owned());
        local_state(&user_data, &listing(&entries.join(",")));
        install_extension(&user_data, "Profile 1");
        let seen = scan_user_data(&user_data, ID);
        assert_eq!(seen.profiles, 1);
        assert_eq!(seen.with_extension, vec![found("Profile 1", Some("Work"))]);

        let turned = Scratch::new();
        local_state(
            turned.path(),
            &listing(
                r#""Default":{"name":"Reasons"},"Profile 1":{"name":"Flags"},
                "Profile 2":{"name":"Old"},"Profile 3":{"name":"On"},"Profile 4":{"name":"Clear"}"#,
            ),
        );
        for (profile, file, settings) in [
            (
                "Default",
                "Secure Preferences",
                extension_settings(ID, r#"{"disable_reasons":[1],"path":"x"}"#),
            ),
            (
                "Profile 1",
                "Preferences",
                extension_settings(ID, r#"{"disable_reasons":1}"#),
            ),
            (
                "Profile 2",
                "Preferences",
                extension_settings(ID, r#"{"state":0}"#),
            ),
            (
                "Profile 3",
                "Secure Preferences",
                extension_settings(
                    "otherotherotherotherotherotherot",
                    r#"{"disable_reasons":[1]}"#,
                ),
            ),
            (
                "Profile 4",
                "Preferences",
                extension_settings(ID, r#"{"state":1,"disable_reasons":0}"#),
            ),
            (
                "Profile 4",
                "Secure Preferences",
                extension_settings(ID, r#"{"disable_reasons":[]}"#),
            ),
        ] {
            install_extension(turned.path(), profile);
            write(&turned.path().join(profile).join(file), &settings);
        }
        let seen = scan_user_data(turned.path(), ID);
        assert_eq!(seen.profiles, 5);
        assert_eq!(
            seen.with_extension,
            vec![
                found("Profile 3", Some("On")),
                found("Profile 4", Some("Clear"))
            ]
        );

        let browsers = Scratch::new();
        let registry = registry_with_chrome(&browsers);
        let chrome = registry.user_data_directory("chrome").unwrap();
        local_state(&chrome, TWO_PROFILES);
        let store = |profile: &str| {
            chrome
                .join(profile)
                .join("Extensions")
                .join(STORE_EXTENSION_ID)
        };
        fs::create_dir_all(store("Default")).unwrap();
        write(
            &chrome.join("Default").join("Preferences"),
            &extension_settings(STORE_EXTENSION_ID, r#"{"disable_reasons":[1]}"#),
        );
        let edge = registry.user_data_directory("edge").unwrap();
        fs::create_dir_all(
            edge.join("Default")
                .join("Extensions")
                .join(STORE_EXTENSION_ID),
        )
        .unwrap();
        let scans = scan(&registry);
        assert_eq!(scans.len(), 1, "an uninstalled browser's data is left out");
        assert_eq!(scans[0].browser.short, "Chrome");
        assert_eq!(scans[0].scan.profiles, 2);
        let held = scans
            .iter()
            .any(|browser| !browser.scan.with_extension.is_empty());
        assert!(!held);
        assert_eq!(
            crate::language::outcome::ExtensionAdvice::resolve(false, held),
            crate::language::outcome::ExtensionAdvice::AddFromStore
        );
        fs::create_dir_all(store("Profile 1")).unwrap();
        assert_eq!(
            scan(&registry)[0].scan.with_extension,
            vec![found("Profile 1", Some("Work"))]
        );
    }
}
