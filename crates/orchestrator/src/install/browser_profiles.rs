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

    #[test]
    fn the_extension_is_found_only_in_the_profile_that_holds_it() {
        let scratch = Scratch::new();
        local_state(scratch.path(), TWO_PROFILES);
        install_extension(scratch.path(), "Profile 1");
        fs::create_dir_all(scratch.path().join("Default")).unwrap();

        let scan = scan_user_data(scratch.path(), ID);

        assert_eq!(scan.profiles, 2);
        assert_eq!(scan.with_extension, vec![found("Profile 1", Some("Work"))]);
    }

    #[test]
    fn profiles_with_the_extension_come_in_local_state_order() {
        let scratch = Scratch::new();
        local_state(
            scratch.path(),
            &listing(
                r#""Profile 3":{"name":"Zed"},"Default":{"name":"Ann"},"Profile 1":{"name":"Bo"}"#,
            ),
        );
        for profile in ["Default", "Profile 1", "Profile 3"] {
            install_extension(scratch.path(), profile);
        }

        let scan = scan_user_data(scratch.path(), ID);

        assert_eq!(
            scan.with_extension,
            vec![
                found("Profile 3", Some("Zed")),
                found("Default", Some("Ann")),
                found("Profile 1", Some("Bo")),
            ]
        );
    }

    #[test]
    fn one_profile_is_one_profile() {
        let scratch = Scratch::new();
        local_state(scratch.path(), &listing(r#""Default":{"name":"Person 1"}"#));
        install_extension(scratch.path(), "Default");

        let scan = scan_user_data(scratch.path(), ID);

        assert_eq!(scan.profiles, 1);
        assert_eq!(
            scan.with_extension,
            vec![found("Default", Some("Person 1"))]
        );
    }

    #[test]
    fn without_local_state_the_default_profile_stands_alone_and_unnamed() {
        let scratch = Scratch::new();
        install_extension(scratch.path(), "Default");

        let scan = scan_user_data(scratch.path(), ID);

        assert_eq!(scan.profiles, 1);
        assert_eq!(scan.with_extension, vec![found("Default", None)]);
    }

    #[test]
    fn a_browser_with_no_data_has_no_profiles() {
        let scratch = Scratch::new();

        let scan = scan_user_data(&scratch.path().join("absent"), ID);

        assert_eq!(scan.profiles, 0);
        assert!(scan.with_extension.is_empty());
    }

    #[test]
    fn unusable_local_state_falls_back_to_the_default_profile() {
        for content in [
            "{ not json".to_owned(),
            r#"{"profile":{"info_cache":[1,2]}}"#.to_owned(),
            r#"{"profile":{}}"#.to_owned(),
            listing(""),
            // Larger than the read limit, and otherwise valid.
            format!(
                r#"{{"padding":"{}","profile":{{"info_cache":{{"Profile 1":{{"name":"Work"}}}}}}}}"#,
                "x".repeat(LOCAL_STATE_LIMIT as usize)
            ),
        ] {
            let scratch = Scratch::new();
            local_state(scratch.path(), &content);
            install_extension(scratch.path(), "Default");
            install_extension(scratch.path(), "Profile 1");

            let scan = scan_user_data(scratch.path(), ID);

            assert_eq!(scan.profiles, 1);
            assert_eq!(scan.with_extension, vec![found("Default", None)]);
        }
    }

    #[test]
    fn hostile_profile_keys_are_never_joined_to_a_path() {
        let scratch = Scratch::new();
        let user_data = scratch.path().join("User Data");
        // A directory beside the user data directory that holds the extension.
        install_extension(scratch.path(), "outside");
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

        let scan = scan_user_data(&user_data, ID);

        assert_eq!(scan.profiles, 1);
        assert_eq!(scan.with_extension, vec![found("Profile 1", Some("Work"))]);
    }

    #[test]
    fn a_key_of_exactly_64_characters_is_used() {
        let key = "p".repeat(64);
        let scratch = Scratch::new();
        local_state(
            scratch.path(),
            &listing(&format!(r#""{key}":{{"name":"Long"}}"#)),
        );
        install_extension(scratch.path(), &key);

        let scan = scan_user_data(scratch.path(), ID);

        assert_eq!(scan.with_extension, vec![found(&key, Some("Long"))]);
    }

    #[test]
    fn shown_names_are_cleaned_and_bounded() {
        let scratch = Scratch::new();
        let long = "n".repeat(60);
        let entries = format!(
            r#""Default":{{"name":"  Per\u0007son\n1  "}},
            "Profile 1":{{"name":"{long}"}},
            "Profile 2":{{"name":" \u0001 \t "}},
            "Profile 3":{{"name":7}},
            "Profile 4":{{}}"#
        );
        local_state(scratch.path(), &listing(&entries));
        for profile in [
            "Default",
            "Profile 1",
            "Profile 2",
            "Profile 3",
            "Profile 4",
        ] {
            install_extension(scratch.path(), profile);
        }

        let scan = scan_user_data(scratch.path(), ID);

        assert_eq!(
            scan.with_extension,
            vec![
                found("Default", Some("Person1")),
                found("Profile 1", Some(&"n".repeat(40))),
                found("Profile 2", None),
                found("Profile 3", None),
                found("Profile 4", None),
            ]
        );
    }

    #[test]
    fn a_name_cut_at_the_limit_does_not_end_in_a_space() {
        let scratch = Scratch::new();
        let name = format!("{} {}", "a".repeat(39), "b".repeat(10));
        local_state(
            scratch.path(),
            &listing(&format!(r#""Default":{{"name":"{name}"}}"#)),
        );
        install_extension(scratch.path(), "Default");

        let scan = scan_user_data(scratch.path(), ID);

        assert_eq!(
            scan.with_extension,
            vec![found("Default", Some(&"a".repeat(39)))]
        );
    }

    /// A profile file holding `settings` as the settings of the extension `id`.
    fn extension_settings(id: &str, settings: &str) -> String {
        format!(r#"{{"extensions":{{"settings":{{"{id}":{settings}}}}}}}"#)
    }

    #[test]
    fn a_turned_off_copy_does_not_hold_the_extension() {
        let scratch = Scratch::new();
        local_state(
            scratch.path(),
            &listing(
                r#""Default":{"name":"Reasons"},"Profile 1":{"name":"Flags"},
                "Profile 2":{"name":"Old"},"Profile 3":{"name":"On"},"Profile 4":{"name":"Clear"}"#,
            ),
        );
        // Turned off, as newer browsers record it, in the protected file.
        install_extension(scratch.path(), "Default");
        write(
            &scratch.path().join("Default").join("Secure Preferences"),
            &extension_settings(ID, r#"{"disable_reasons":[1],"path":"x"}"#),
        );
        // Turned off, as older browsers record it, in the plain file.
        install_extension(scratch.path(), "Profile 1");
        write(
            &scratch.path().join("Profile 1").join("Preferences"),
            &extension_settings(ID, r#"{"disable_reasons":1}"#),
        );
        install_extension(scratch.path(), "Profile 2");
        write(
            &scratch.path().join("Profile 2").join("Preferences"),
            &extension_settings(ID, r#"{"state":0}"#),
        );
        // On: only another extension is turned off.
        install_extension(scratch.path(), "Profile 3");
        write(
            &scratch.path().join("Profile 3").join("Secure Preferences"),
            &extension_settings(
                "otherotherotherotherotherotherot",
                r#"{"disable_reasons":[1]}"#,
            ),
        );
        // On: the reasons are cleared.
        install_extension(scratch.path(), "Profile 4");
        write(
            &scratch.path().join("Profile 4").join("Preferences"),
            &extension_settings(ID, r#"{"state":1,"disable_reasons":0}"#),
        );
        write(
            &scratch.path().join("Profile 4").join("Secure Preferences"),
            &extension_settings(ID, r#"{"disable_reasons":[]}"#),
        );

        let scan = scan_user_data(scratch.path(), ID);

        assert_eq!(scan.profiles, 5);
        assert_eq!(
            scan.with_extension,
            vec![
                found("Profile 3", Some("On")),
                found("Profile 4", Some("Clear"))
            ]
        );
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

    #[test]
    fn a_browser_with_only_a_turned_off_copy_sends_the_person_to_the_store() {
        let scratch = Scratch::new();
        let registry = registry_with_chrome(&scratch);
        let chrome = registry.user_data_directory("chrome").unwrap();
        local_state(&chrome, &listing(r#""Default":{"name":"Person 1"}"#));
        fs::create_dir_all(
            chrome
                .join("Default")
                .join("Extensions")
                .join(STORE_EXTENSION_ID),
        )
        .unwrap();
        write(
            &chrome.join("Default").join("Preferences"),
            &extension_settings(STORE_EXTENSION_ID, r#"{"disable_reasons":[1]}"#),
        );

        // What the executor asks before it words a refusal while setup is incomplete.
        let held = scan(&registry)
            .iter()
            .any(|browser| !browser.scan.with_extension.is_empty());

        assert!(!held);
        assert_eq!(
            crate::language::outcome::ExtensionAdvice::resolve(false, held),
            crate::language::outcome::ExtensionAdvice::AddFromStore
        );
    }

    #[test]
    fn scanning_finds_installed_browsers_and_leaves_the_others_out() {
        let scratch = Scratch::new();
        // Chrome is installed; Edge has data but no executable.
        let registry = registry_with_chrome(&scratch);
        let chrome = registry.user_data_directory("chrome").unwrap();
        local_state(&chrome, TWO_PROFILES);
        fs::create_dir_all(
            chrome
                .join("Profile 1")
                .join("Extensions")
                .join(STORE_EXTENSION_ID),
        )
        .unwrap();
        let edge = registry.user_data_directory("edge").unwrap();
        fs::create_dir_all(
            edge.join("Default")
                .join("Extensions")
                .join(STORE_EXTENSION_ID),
        )
        .unwrap();

        let scans = scan(&registry);

        assert_eq!(scans.len(), 1);
        assert_eq!(scans[0].browser.short, "Chrome");
        assert_eq!(scans[0].scan.profiles, 2);
        assert_eq!(
            scans[0].scan.with_extension,
            vec![found("Profile 1", Some("Work"))]
        );
    }
}
