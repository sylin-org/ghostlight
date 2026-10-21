//! The install record and the install-or-refuse decision.
//!
//! `bin/install.json` says what the installer last put in `bin`: the service version, the relay
//! revision the edge speaks, the hashes of the service image and the edge, and the [`Channel`]
//! that installed them. [`decide`] compares that record with the payload being installed and
//! answers whether to install, do nothing, repair or refuse (ADR-0197 Decision 14).
//!
//! The one atomic replace and the one private-file creator (ADR-0203 Decisions 11.1 and 11.2)
//! live in [`super::files`]. They are named here too, where those decisions name them, so every
//! caller outside the installer keeps the path it has.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use super::files::{atomic_replace, atomic_replace_keeping_access, create_private};
use crate::language::deployment as words;

#[cfg(test)]
pub(crate) use super::files::assert_owner_only;

/// Which kind of build the installation in `bin` is.
///
/// The development loop installs its build into `bin` as the development channel. A release
/// package never installs over it; `deployment install --restore` is the way back (ADR-0199
/// amendment, the development channel lands in 2.0).
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// A packaged release.
    #[default]
    Release,
    /// A build the development loop installed.
    Development,
}

/// What the installer last put in `bin`.
///
/// Readers ignore unknown fields, so a later release may add to the record without breaking an
/// earlier one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InstallRecord {
    /// Semantic version of the installed service.
    pub service_version: String,
    /// Relay revision spoken by the installed edge.
    pub relay_revision: String,
    /// Lowercase hexadecimal SHA-256 of the installed service image.
    pub service_sha256: String,
    /// Lowercase hexadecimal SHA-256 of the installed edge. A record written before the edge
    /// existed has none, which is how the installer knows `bin` still holds the earlier layout.
    #[serde(default)]
    pub edge_sha256: Option<String>,
    /// Which channel installed this set. A record written before channels existed reads as
    /// [`Channel::Release`].
    #[serde(default)]
    pub channel: Channel,
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
#[error("{}", words::payload_refused(.installed, .payload))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn record(version: &str, sha: &str) -> InstallRecord {
        InstallRecord {
            service_version: version.to_owned(),
            relay_revision: "3".to_owned(),
            service_sha256: sha.to_owned(),
            edge_sha256: None,
            channel: Channel::Release,
        }
    }

    /// The version vectors both launchers test against too (`packaging/version-order.json`).
    #[derive(Deserialize)]
    struct VersionVectors {
        ascending: Vec<String>,
        equal: Vec<[String; 2]>,
        malformed: Vec<String>,
    }

    /// The record the service writes as the npm and mcpb launchers read it, which their own tests
    /// can read too.
    const INSTALL_GOLDEN: &str = include_str!("../../../../tests/golden/install/install.json");

    /// Protects the coordinator's reserve contract between the service and its launchers:
    /// `bin/install.json` is written byte for byte as `tests/golden/install/install.json` records,
    /// so `service_version` and `channel`, which the npm and mcpb launchers read, keep their names,
    /// their spelling and the development channel's value, and the golden reads back as the record.
    #[test]
    fn install_json_is_written_as_the_golden_the_launchers_read() {
        let record = InstallRecord {
            service_version: "2.0.0".to_owned(),
            relay_revision: "5".to_owned(),
            service_sha256: "9df6b026a8c6c26e3c3acd2370a16e93fffdc0015ff5bd879218788025db0280"
                .to_owned(),
            edge_sha256: Some(
                "a1cb100f57e971cacf269e7c26e4630a25a8e9d4bdd35e32df1a80b66b896254".to_owned(),
            ),
            channel: Channel::Development,
        };
        let scratch = Scratch::new();
        let path = scratch.path().join("install.json");
        record.write(&path).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            INSTALL_GOLDEN,
            "the launchers read tests/golden/install/install.json: change it and them together"
        );
        assert_eq!(InstallRecord::read(&path).unwrap(), Some(record));
    }

    /// Protects promise 1 (ADR-0197 Decision 14): `bin/install.json` reads whatever an earlier or
    /// later release wrote. A missing, broken or incomplete record reads as none, so the next
    /// install replaces it; a record from before channels and the edge reads as a release with no
    /// edge; fields a later release adds are ignored; and a written record reads back whole.
    #[test]
    fn an_install_record_reads_whatever_any_release_wrote() {
        let scratch = Scratch::new();
        let path = scratch.path().join("bin").join("install.json");
        assert_eq!(InstallRecord::read(&path).unwrap(), None);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        for unusable in [&b"not json"[..], br#"{"service_version":"2.0.0"}"#] {
            fs::write(&path, unusable).unwrap();
            assert_eq!(InstallRecord::read(&path).unwrap(), None);
        }

        fs::write(
            &path,
            r#"{"service_version":"2.0.0","relay_revision":"3","service_sha256":"ab"}"#,
        )
        .unwrap();
        assert_eq!(
            InstallRecord::read(&path).unwrap(),
            Some(record("2.0.0", "ab"))
        );
        fs::write(
            &path,
            br#"{"service_version":"2.1.0","relay_revision":"3","service_sha256":"f0","later":{"a":1}}"#,
        )
        .unwrap();
        assert_eq!(
            InstallRecord::read(&path).unwrap(),
            Some(record("2.1.0", "f0"))
        );

        let development = InstallRecord {
            edge_sha256: Some("cd".to_owned()),
            channel: Channel::Development,
            ..record("2.0.0", "ab")
        };
        development.write(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#""channel": "development""#), "{text}");
        assert_eq!(InstallRecord::read(&path).unwrap(), Some(development));
    }

    /// Protects promise 1 (ADR-0197 Decision 14): a payload installs only forward. Versions order
    /// by semver precedence on the vectors the npm and mcpb launchers share, a malformed one does
    /// not parse, an older payload is refused whatever state the image is in, and a payload of
    /// the same version repairs an image whose SHA-256 is missing or not the recorded one.
    #[test]
    fn a_payload_installs_only_forward_by_the_versions_the_launchers_share() {
        let vectors: VersionVectors =
            serde_json::from_str(include_str!("../../../../packaging/version-order.json")).unwrap();
        let versions: Vec<Version> = vectors
            .ascending
            .iter()
            .map(|text| Version::parse(text).unwrap_or_else(|| panic!("{text:?}")))
            .collect();
        for pair in versions.windows(2) {
            assert!(pair[0] < pair[1], "{pair:?}");
        }
        for [left, right] in &vectors.equal {
            assert!(Version::parse(left).is_some(), "{left}");
            assert_eq!(
                Version::parse(left),
                Version::parse(right),
                "{left} {right}"
            );
        }
        for text in &vectors.malformed {
            assert_eq!(Version::parse(text), None, "{text:?}");
        }

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

        let scratch = Scratch::new();
        let image = scratch.path().join("image");
        fs::write(&image, b"abc").unwrap();
        let hashed = record(
            "2.0.0",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
        let sha = sha256_file(&image).unwrap();
        assert_eq!(
            decide(Some(&hashed), &record("2.0.0", "cc"), Some(&sha)),
            Standing::Same
        );
    }
}
