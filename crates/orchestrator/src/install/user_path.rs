//! The per-user `Path` on Windows: its entries as strings, and where the value is stored.
//!
//! The typed `ghostlight` command works in a new terminal because the installed `bin` directory is
//! one entry of the per-user `Path` (`HKEY_CURRENT_USER\Environment`). The machine `Path` is never
//! touched. This module holds the edits as pure functions over the stored text, so every rule is
//! tested without a registry, and the [`UserPathStore`] seam the registry implements.
//!
//! Ownership is the directory. An entry is Ghostlight's when it names the same directory as the
//! one being installed, compared without case, without a trailing backslash, without surrounding
//! quotes and after `%NAME%` references are expanded. Every other entry is kept byte for byte and
//! in order, empty segments included.

use std::fmt::Debug;
use std::io;
use std::path::Path;

/// Separates the entries of a `Path` value.
const SEPARATOR: char = ';';

/// Delimits an environment variable reference such as `%USERPROFILE%`.
const REFERENCE_DELIMITER: char = '%';

/// The variable the home-relative form of an entry names.
const PROFILE_VARIABLE: &str = "USERPROFILE";

/// Where the per-user `Path` lives, for messages.
pub const LOCATION: &str = r"HKEY_CURRENT_USER\Environment\Path";

/// A stored per-user `Path`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathValue {
    /// The entries, separated by semicolons.
    pub text: String,
    /// Whether the value holds `%NAME%` references to expand (`REG_EXPAND_SZ`) or is taken
    /// literally (`REG_SZ`).
    pub expandable: bool,
}

impl PathValue {
    /// The value a user with no per-user `Path` gets: empty, and expandable as Windows makes it.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            expandable: true,
        }
    }
}

/// Where the per-user `Path` is stored. The registry implements it on Windows; tests use a fake.
pub trait UserPathStore: Debug + Send + Sync {
    /// The stored value, or `None` when the user has none.
    fn read(&self) -> io::Result<Option<PathValue>>;
    /// Store `value`, keeping its type.
    fn write(&self, value: &PathValue) -> io::Result<()>;
    /// Tell running programs that the value changed. A failure is ignored.
    fn announce(&self);
}

/// Replace each `%NAME%` in `text` with what `lookup` answers for NAME. A reference `lookup` does
/// not know, and a lone `%`, stay as written.
#[must_use]
pub fn expand(text: &str, lookup: &impl Fn(&str) -> Option<String>) -> String {
    let mut expanded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(REFERENCE_DELIMITER) {
        expanded.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(length) = after.find(REFERENCE_DELIMITER) else {
            expanded.push_str(&rest[start..]);
            return expanded;
        };
        let name = &after[..length];
        match lookup(name).filter(|_| !name.is_empty()) {
            Some(value) => {
                expanded.push_str(&value);
                rest = &after[length + 1..];
            }
            None => {
                expanded.push(REFERENCE_DELIMITER);
                rest = after;
            }
        }
    }
    expanded.push_str(rest);
    expanded
}

/// The comparison form of one entry: references expanded, quotes and a trailing backslash gone,
/// forward slashes as backslashes, and lower case.
fn comparable(entry: &str, expandable: bool, lookup: &impl Fn(&str) -> Option<String>) -> String {
    let expanded = if expandable {
        expand(entry, lookup)
    } else {
        entry.to_owned()
    };
    expanded
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// Whether `entry` of `value` names the directory whose comparison form is `wanted`.
fn names(
    entry: &str,
    value: &PathValue,
    wanted: &str,
    lookup: &impl Fn(&str) -> Option<String>,
) -> bool {
    !entry.is_empty() && comparable(entry, value.expandable, lookup) == wanted
}

/// Whether some entry of `value` names `directory`.
#[must_use]
pub fn names_directory(
    value: &PathValue,
    directory: &Path,
    lookup: &impl Fn(&str) -> Option<String>,
) -> bool {
    let wanted = comparable(&directory.to_string_lossy(), false, lookup);
    value
        .text
        .split(SEPARATOR)
        .any(|entry| names(entry, value, &wanted, lookup))
}

/// How `directory` is written into a value: under `%USERPROFILE%` when the value expands
/// references and `profile` holds the directory, otherwise as the absolute path.
#[must_use]
pub fn entry_for(directory: &Path, expandable: bool, profile: Option<&Path>) -> String {
    let absolute = directory.to_string_lossy().into_owned();
    let relative = profile.filter(|_| expandable).and_then(|profile| {
        let profile = profile.to_string_lossy();
        let prefix = absolute.get(..profile.len())?;
        let remainder = absolute.get(profile.len()..)?;
        let boundary = remainder.is_empty() || remainder.starts_with(['\\', '/']);
        (prefix.eq_ignore_ascii_case(&profile) && boundary).then(|| {
            format!("{REFERENCE_DELIMITER}{PROFILE_VARIABLE}{REFERENCE_DELIMITER}{remainder}")
        })
    });
    relative.unwrap_or(absolute)
}

/// `text` with `entry` after its last entry. Nothing already in `text` changes. A trailing
/// semicolon stays the last character, so removing the entry restores the text exactly.
#[must_use]
pub fn append(text: &str, entry: &str) -> String {
    match text.strip_suffix(SEPARATOR) {
        Some(entries) => format!("{entries}{SEPARATOR}{entry}{SEPARATOR}"),
        None if text.is_empty() => entry.to_owned(),
        None => format!("{text}{SEPARATOR}{entry}"),
    }
}

/// `value` without the entries that name `directory`, or `None` when none does. The other entries
/// stay exactly as they were, in order.
#[must_use]
pub fn remove(
    value: &PathValue,
    directory: &Path,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Option<PathValue> {
    let wanted = comparable(&directory.to_string_lossy(), false, lookup);
    let entries: Vec<&str> = value.text.split(SEPARATOR).collect();
    let kept: Vec<&str> = entries
        .iter()
        .copied()
        .filter(|entry| !names(entry, value, &wanted, lookup))
        .collect();
    (kept.len() != entries.len()).then(|| PathValue {
        text: kept.join(&SEPARATOR.to_string()),
        expandable: value.expandable,
    })
}

/// The per-user `Path` in the registry.
#[cfg(windows)]
#[derive(Debug)]
pub struct Registry;

#[cfg(windows)]
mod registry {
    use std::io;

    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_SZ};
    use winreg::{RegKey, RegValue};

    use super::{PathValue, Registry, UserPathStore};

    /// The key below `HKEY_CURRENT_USER` that holds the user's environment variables.
    const ENVIRONMENT_KEY: &str = "Environment";

    /// The value that holds the user's `Path`.
    const PATH_VALUE: &str = "Path";

    impl UserPathStore for Registry {
        fn read(&self) -> io::Result<Option<PathValue>> {
            let key = match RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(ENVIRONMENT_KEY, KEY_READ)
            {
                Ok(key) => key,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error),
            };
            match key.get_raw_value(PATH_VALUE) {
                Ok(raw) => decode(&raw).map(Some),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error),
            }
        }

        fn write(&self, value: &PathValue) -> io::Result<()> {
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey_with_flags(ENVIRONMENT_KEY, KEY_READ | KEY_WRITE)?;
            key.set_raw_value(PATH_VALUE, &encode(value))
        }

        fn announce(&self) {
            ghostlight_win_peer::broadcast_environment_change();
        }
    }

    /// The value as the registry stores it: UTF-16 with a terminating NUL.
    fn encode(value: &PathValue) -> RegValue {
        let bytes = value
            .text
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        RegValue {
            bytes,
            vtype: if value.expandable {
                REG_EXPAND_SZ
            } else {
                REG_SZ
            },
        }
    }

    /// The text of a stored value. A value that is not a string, or not valid UTF-16, is refused
    /// rather than rewritten.
    fn decode(raw: &RegValue) -> io::Result<PathValue> {
        let expandable = match &raw.vtype {
            REG_EXPAND_SZ => true,
            REG_SZ => false,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("the value has type {other:?}, not a string"),
                ))
            }
        };
        let units: Vec<u16> = raw
            .bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect();
        let text = String::from_utf16(&units)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        Ok(PathValue { text, expandable })
    }

    #[cfg(test)]
    mod tests {
        use winreg::enums::REG_DWORD;

        use super::*;

        #[test]
        fn a_value_survives_the_registry_encoding() {
            for expandable in [true, false] {
                let value = PathValue {
                    text: r"C:\one;%USERPROFILE%\two;".to_owned(),
                    expandable,
                };
                assert_eq!(decode(&encode(&value)).unwrap(), value);
            }
        }

        #[test]
        fn a_value_that_is_not_a_string_is_refused() {
            let raw = RegValue {
                bytes: vec![1, 0, 0, 0],
                vtype: REG_DWORD,
            };
            assert!(decode(&raw).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const BIN: &str = r"C:\Users\person\.ghostlight\bin";

    fn lookup(name: &str) -> Option<String> {
        name.eq_ignore_ascii_case("USERPROFILE")
            .then(|| r"C:\Users\person".to_owned())
    }

    fn value(text: &str) -> PathValue {
        PathValue {
            text: text.to_owned(),
            expandable: true,
        }
    }

    fn bin() -> PathBuf {
        PathBuf::from(BIN)
    }

    fn with_ours(text: &str) -> String {
        append(
            text,
            &entry_for(&bin(), true, Some(Path::new(r"C:\Users\person"))),
        )
    }

    #[test]
    fn a_missing_directory_is_appended_after_the_others() {
        let before = value(r"C:\Windows;C:\Tools");
        assert!(!names_directory(&before, &bin(), &lookup));
        assert_eq!(
            with_ours(&before.text),
            r"C:\Windows;C:\Tools;%USERPROFILE%\.ghostlight\bin"
        );
    }

    #[test]
    fn an_empty_value_becomes_just_the_entry() {
        assert_eq!(append("", "C:\\one"), "C:\\one");
    }

    #[test]
    fn a_trailing_semicolon_stays_last() {
        assert_eq!(append(r"C:\a;C:\b;", r"C:\c"), r"C:\a;C:\b;C:\c;");
    }

    #[test]
    fn a_present_directory_is_found_however_it_is_written() {
        for text in [
            BIN,
            r"C:\Users\person\.ghostlight\bin\",
            r"c:\users\PERSON\.GhostLight\BIN",
            r"%USERPROFILE%\.ghostlight\bin",
            r"%userprofile%\.ghostlight\bin\",
            r#""C:\Users\person\.ghostlight\bin""#,
            r"C:\Users\person/.ghostlight/bin",
        ] {
            let before = value(&format!(r"C:\Windows;{text};C:\Tools"));
            assert!(names_directory(&before, &bin(), &lookup), "{text}");
        }
    }

    #[test]
    fn a_literal_value_does_not_expand_references() {
        let literal = PathValue {
            text: r"%USERPROFILE%\.ghostlight\bin".to_owned(),
            expandable: false,
        };
        assert!(!names_directory(&literal, &bin(), &lookup));
    }

    #[test]
    fn a_longer_path_is_not_ours() {
        for text in [
            r"C:\Users\person\.ghostlight\bin\old",
            r"C:\Users\person\.ghostlight\binary",
            r"C:\Users\person\.ghostlight",
            r"%USERPROFILE%\.ghostlight\bin-old",
        ] {
            let before = value(text);
            assert!(!names_directory(&before, &bin(), &lookup), "{text}");
            assert_eq!(remove(&before, &bin(), &lookup), None, "{text}");
        }
    }

    #[test]
    fn the_entry_uses_the_profile_form_only_in_an_expandable_value() {
        let profile = Path::new(r"C:\Users\person");
        assert_eq!(
            entry_for(&bin(), true, Some(profile)),
            r"%USERPROFILE%\.ghostlight\bin"
        );
        assert_eq!(entry_for(&bin(), false, Some(profile)), BIN);
        assert_eq!(entry_for(&bin(), true, None), BIN);
        assert_eq!(
            entry_for(&bin(), true, Some(Path::new(r"C:\Users\per"))),
            BIN,
            "a prefix that ends inside a directory name is not the profile"
        );
        assert_eq!(
            entry_for(Path::new(r"D:\elsewhere\bin"), true, Some(profile)),
            r"D:\elsewhere\bin"
        );
    }

    #[test]
    fn appending_twice_changes_nothing_the_second_time() {
        let once = with_ours(r"C:\Windows");
        assert!(names_directory(&value(&once), &bin(), &lookup));
    }

    #[test]
    fn other_entries_keep_their_bytes_order_and_empty_segments() {
        for original in [
            r"C:\A;;c:\b\;%SystemRoot%\x;",
            r"C:\A;;c:\b\;%SystemRoot%\x",
        ] {
            let added = value(&with_ours(original));
            assert!(added.text.starts_with(original.trim_end_matches(';')));
            assert!(names_directory(&added, &bin(), &lookup));
            let removed = remove(&added, &bin(), &lookup).unwrap();
            assert_eq!(removed.text, original);
            assert!(removed.expandable);
        }
    }

    #[test]
    fn removal_takes_only_the_entries_that_name_the_directory() {
        let before =
            value(r"C:\a;%USERPROFILE%\.ghostlight\bin;C:\b;C:\Users\person\.ghostlight\bin\old");
        let after = remove(&before, &bin(), &lookup).unwrap();
        assert_eq!(after.text, r"C:\a;C:\b;C:\Users\person\.ghostlight\bin\old");
    }

    #[test]
    fn removing_the_only_entry_leaves_an_empty_value() {
        let after = remove(&value(BIN), &bin(), &lookup).unwrap();
        assert_eq!(after.text, "");
    }

    #[test]
    fn expansion_leaves_unknown_references_and_lone_percent_signs() {
        assert_eq!(expand("%NOPE%x", &lookup), "%NOPE%x");
        assert_eq!(expand("100%", &lookup), "100%");
        assert_eq!(expand("%%", &lookup), "%%");
        assert_eq!(
            expand("%NOPE%%USERPROFILE%", &lookup),
            r"%NOPE%C:\Users\person"
        );
        assert_eq!(expand("a%USERPROFILE%b", &lookup), r"aC:\Users\personb");
    }
}
