//! The install record and the install-or-refuse decision.
//!
//! `bin/install.json` says what the installer last put in `bin`: the service version, the relay
//! revision the connectors speak, and the hash of the service image. [`decide`] compares that
//! record with the payload being installed and answers whether to install, do nothing, repair or
//! refuse (ADR-0197 Decision 14). Every installer write goes through [`atomic_replace`], so a
//! reader sees the old file or the new one, never a partial one.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use ghostlight_relay::private;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// What the installer last put in `bin`.
///
/// Readers ignore unknown fields, so a later release may add to the record without breaking an
/// earlier one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InstallRecord {
    /// Semantic version of the installed service.
    pub service_version: String,
    /// Relay revision spoken by the installed connectors.
    pub relay_revision: String,
    /// Lowercase hexadecimal SHA-256 of the installed service image.
    pub service_sha256: String,
}

impl InstallRecord {
    /// Read the record at `path`.
    ///
    /// A missing file and a file that does not hold a record both read as `None`: the next
    /// install replaces whatever is there. Only a failure to read the file is an error.
    pub fn read(path: &Path) -> io::Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Write the record to `path` with [`atomic_replace`].
    pub fn write(&self, path: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        atomic_replace(path, &bytes)
    }
}

/// How the installed copy stands against the payload being installed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Standing {
    /// Nothing is recorded as installed.
    Absent,
    /// The installed version is older than the payload: install, whatever state its image is in.
    Older,
    /// Same version and an intact image: nothing to do.
    Same,
    /// The record cannot be trusted, or the version matches and the image is missing or
    /// changed: install again.
    Damaged,
    /// The installed version is newer than the payload, or the payload's version cannot be
    /// ordered: refuse the payload, even when the installed image is damaged.
    Newer,
}

/// Compare the recorded install with the payload.
///
/// `installed_image_sha` is the SHA-256 of the service image now in `bin`, or `None` when there
/// is none. Versions are compared first, because an older payload is refused whatever state the
/// installed image is in: a repair must never be a downgrade. A recorded version that does not
/// parse is damage, and a payload version that does not parse is a refusal, since nothing is
/// installed that cannot be ordered. Only a payload of the same version looks at the image: a
/// missing or different image makes it a repair.
#[must_use]
pub fn decide(
    installed: Option<&InstallRecord>,
    payload: &InstallRecord,
    installed_image_sha: Option<&str>,
) -> Standing {
    let Some(installed) = installed else {
        return Standing::Absent;
    };
    let Some(installed_version) = Version::parse(&installed.service_version) else {
        return Standing::Damaged;
    };
    let Some(payload_version) = Version::parse(&payload.service_version) else {
        return Standing::Newer;
    };
    match payload_version.cmp(&installed_version) {
        Ordering::Greater => Standing::Older,
        Ordering::Less => Standing::Newer,
        Ordering::Equal if installed_image_sha == Some(installed.service_sha256.as_str()) => {
            Standing::Same
        }
        Ordering::Equal => Standing::Damaged,
    }
}

/// Why a payload was not installed.
#[derive(Debug, Error)]
#[error(
    "Ghostlight {installed} is already installed and this package carries the older \
     {payload}, so nothing was changed"
)]
pub struct PayloadRefused {
    /// Version now installed.
    pub installed: String,
    /// Version the refused payload carries.
    pub payload: String,
}

impl PayloadRefused {
    /// The refusal for a payload that [`decide`] answered [`Standing::Newer`] about.
    #[must_use]
    pub fn new(installed: &InstallRecord, payload: &InstallRecord) -> Self {
        Self {
            installed: installed.service_version.clone(),
            payload: payload.service_version.clone(),
        }
    }
}

/// A semantic version (semver.org precedence rules), parsed without a dependency.
///
/// Build metadata is accepted and ignored, as the rules require.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Version {
    core: [u64; 3],
    pre_release: Vec<Identifier>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Identifier {
    Numeric(u64),
    Text(String),
}

impl Version {
    /// Parse `major.minor.patch`, optionally followed by `-pre.release` and `+build`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let without_build = text.split_once('+').map_or(text, |(version, _)| version);
        let (core_text, pre_text) = match without_build.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (without_build, None),
        };
        let mut parts = core_text.split('.');
        let mut core = [0u64; 3];
        for slot in &mut core {
            *slot = numeric(parts.next()?)?;
        }
        if parts.next().is_some() {
            return None;
        }
        let pre_release = match pre_text {
            None => Vec::new(),
            Some(pre) => pre.split('.').map(identifier).collect::<Option<Vec<_>>>()?,
        };
        Some(Self { core, pre_release })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core.cmp(&other.core).then_with(|| {
            match (self.pre_release.is_empty(), other.pre_release.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre_release.cmp(&other.pre_release),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Identifier {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Numeric(left), Self::Numeric(right)) => left.cmp(right),
            (Self::Numeric(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Numeric(_)) => Ordering::Greater,
            (Self::Text(left), Self::Text(right)) => left.cmp(right),
        }
    }
}

impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A numeric version field: digits only, no leading zero.
fn numeric(text: &str) -> Option<u64> {
    let digits = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !digits || (text.len() > 1 && text.starts_with('0')) {
        return None;
    }
    text.parse().ok()
}

/// One dot-separated pre-release identifier.
fn identifier(text: &str) -> Option<Identifier> {
    if text.is_empty() {
        return None;
    }
    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        return numeric(text).map(Identifier::Numeric);
    }
    let valid = text
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    valid.then(|| Identifier::Text(text.to_owned()))
}

/// Lowercase hexadecimal SHA-256 of the file at `path`.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Replace the file at `path` with `bytes`, atomically.
///
/// The bytes go to a new file in the same directory, are flushed to disk, and are renamed over
/// `path`. A reader sees the old file or the new one, and a failure leaves the old one. The new
/// file is readable only by the current user: on Windows it is created through the audited
/// private-file call, and on Unix with mode 0600. Missing parent directories are created, closed
/// to everyone else on Unix.
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    replace(path, bytes, &|temporary, _| create_private(temporary))
}

/// Replace the file at `path` with `bytes`, atomically, for a file that is not Ghostlight's own.
///
/// Like [`atomic_replace`], but the new file has the access the old one had instead of being
/// private: a policy file that other accounts must read stays readable by them. On Unix the new
/// file gets the old file's mode; on Windows it inherits from its directory.
pub fn atomic_replace_keeping_access(path: &Path, bytes: &[u8]) -> io::Result<()> {
    replace(path, bytes, &create_like)
}

/// The shared steps of both replacements. `create` makes the new file at its first path argument,
/// given the path being replaced as its second.
fn replace(
    path: &Path,
    bytes: &[u8],
    create: &dyn Fn(&Path, &Path) -> io::Result<File>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    private::create_dir_all(parent)?;
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    let temporary = Path::new(&temporary);
    let published = (|| {
        let mut file = create(temporary, path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temporary, path)
    })();
    if published.is_err() {
        let _ = fs::remove_file(temporary);
    }
    published
}

/// Create a new file with the access of `existing`, which it is about to replace.
fn create_like(path: &Path, existing: &Path) -> io::Result<File> {
    let file = File::options().create_new(true).write(true).open(path)?;
    #[cfg(unix)]
    {
        file.set_permissions(fs::metadata(existing)?.permissions())?;
    }
    #[cfg(not(unix))]
    {
        let _ = existing;
    }
    Ok(file)
}

/// Create a new file only the current user can read.
#[cfg(windows)]
fn create_private(path: &Path) -> io::Result<File> {
    ghostlight_win_peer::create_private_file(path)
}

/// Create a new file only the current user can read.
#[cfg(not(windows))]
fn create_private(path: &Path) -> io::Result<File> {
    private::file_options()
        .create_new(true)
        .write(true)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::super::scratch::Scratch;
    use super::*;

    fn record(version: &str, sha: &str) -> InstallRecord {
        InstallRecord {
            service_version: version.to_owned(),
            relay_revision: "3".to_owned(),
            service_sha256: sha.to_owned(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_replaced_file_and_the_directories_made_for_it_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new();
        let path = scratch.path().join("state").join("settings.json");
        atomic_replace(&path, b"{}").unwrap();
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }

    #[test]
    fn a_record_round_trips() {
        let scratch = Scratch::new();
        let path = scratch.path().join("bin").join("install.json");
        let written = record("2.0.0", "abc");
        written.write(&path).unwrap();
        assert_eq!(InstallRecord::read(&path).unwrap(), Some(written));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let scratch = Scratch::new();
        let path = scratch.path().join("install.json");
        fs::write(
            &path,
            br#"{"service_version":"2.1.0","relay_revision":"3","service_sha256":"f0","later":{"a":1}}"#,
        )
        .unwrap();
        assert_eq!(
            InstallRecord::read(&path).unwrap(),
            Some(record("2.1.0", "f0"))
        );
    }

    #[test]
    fn a_missing_or_unreadable_record_reads_as_none() {
        let scratch = Scratch::new();
        let path = scratch.path().join("install.json");
        assert_eq!(InstallRecord::read(&path).unwrap(), None);
        fs::write(&path, b"not json").unwrap();
        assert_eq!(InstallRecord::read(&path).unwrap(), None);
        fs::write(&path, br#"{"service_version":"2.0.0"}"#).unwrap();
        assert_eq!(InstallRecord::read(&path).unwrap(), None);
    }

    #[test]
    fn decisions_follow_version_order_and_image_health() {
        let installed = record("2.0.0", "aa");
        let unordered = record("not a version", "aa");
        let cases = [
            (None, "2.0.0", None, Standing::Absent),
            (Some(&installed), "2.0.1", Some("aa"), Standing::Older),
            (Some(&installed), "2.1.0", Some("aa"), Standing::Older),
            (Some(&installed), "2.0.0", Some("aa"), Standing::Same),
            (Some(&installed), "1.9.9", Some("aa"), Standing::Newer),
            (Some(&installed), "2.0.0", Some("bb"), Standing::Damaged),
            (Some(&installed), "2.0.0", None, Standing::Damaged),
            (Some(&installed), "1.0.0", Some("bb"), Standing::Newer),
            (Some(&installed), "1.0.0", None, Standing::Newer),
            (Some(&installed), "2.0.1", Some("bb"), Standing::Older),
            (Some(&installed), "2.0.1", None, Standing::Older),
            (Some(&installed), "nonsense", None, Standing::Newer),
            (Some(&unordered), "2.0.0", None, Standing::Damaged),
            (Some(&installed), "nonsense", Some("aa"), Standing::Newer),
            (Some(&unordered), "2.0.0", Some("aa"), Standing::Damaged),
        ];
        for (installed, payload, sha, expected) in cases {
            assert_eq!(
                decide(installed, &record(payload, "cc"), sha),
                expected,
                "payload {payload}, image {sha:?}"
            );
        }
    }

    #[test]
    fn a_refused_payload_names_both_versions() {
        let message = PayloadRefused::new(&record("2.1.0", "a"), &record("2.0.0", "b")).to_string();
        assert!(
            message.contains("2.1.0") && message.contains("2.0.0"),
            "{message}"
        );
    }

    #[test]
    fn versions_order_by_semver_precedence() {
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.2.0",
            "1.10.0",
            "2.0.0",
        ];
        let versions: Vec<Version> = ordered
            .iter()
            .map(|text| Version::parse(text).unwrap())
            .collect();
        for pair in versions.windows(2) {
            assert!(pair[0] < pair[1], "{pair:?}");
        }
        assert_eq!(Version::parse("1.0.0+a"), Version::parse("1.0.0+b"));
    }

    #[test]
    fn malformed_versions_do_not_parse() {
        for text in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.x",
            "1.2.3-",
            "1.2.3-a..b",
            "v1.2.3",
        ] {
            assert_eq!(Version::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn atomic_replace_publishes_whole_files_and_leaves_no_temporary() {
        let scratch = Scratch::new();
        let path = scratch.path().join("nested").join("file.json");
        atomic_replace(&path, b"first").unwrap();
        atomic_replace(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        let names: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["file.json"]);
    }

    #[test]
    fn a_failed_replace_keeps_the_old_file() {
        let scratch = Scratch::new();
        let path = scratch.path().join("directory");
        fs::create_dir(&path).unwrap();
        assert!(atomic_replace(&path, b"bytes").is_err());
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 1);
    }

    #[test]
    fn a_file_hashes_to_its_sha256() {
        let scratch = Scratch::new();
        let path = scratch.path().join("image");
        fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
