//! The install record and the install-or-refuse decision.
//!
//! `bin/install.json` says what the installer last put in `bin`: the service version, the relay
//! revision the edge speaks, the hashes of the service image and the edge, and the [`Channel`]
//! that installed them. [`decide`] compares that record with the payload being installed and
//! answers whether to install, do nothing, repair or refuse (ADR-0197 Decision 14).
//!
//! [`Replace`] is Ghostlight's one atomic file replace (ADR-0203 Decision 11.2), so a reader sees
//! the old file or the new one, never a partial one. Its options say who may read the new file,
//! whether a backup is kept, and whether a link is written through or refused. [`atomic_replace`]
//! and [`atomic_replace_keeping_access`] name its two common uses.
//!
//! [`create_private`] is the one creator for a file Ghostlight makes that holds a secret or a
//! record about the person (ADR-0203 Decision 11.1). On Windows the file is created with a
//! protected access list that grants the current user and SYSTEM full access and inherits nothing
//! from its directory, so a broad grant on the directory never reaches it. On Unix it is created
//! with mode 0600. A file that already exists keeps the access it has.

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use ghostlight_relay::private;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::language::deployment as words;

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

/// How the name of a replace's temporary file begins; a simple UUID and [`TEMPORARY_SUFFIX`]
/// follow. Every replace names its temporary this way, beside the file it replaces, so the stray
/// sweep knows one name ([`is_temporary`]).
const TEMPORARY_PREFIX: &str = ".ghostlight-";
/// How the name of a replace's temporary file ends.
const TEMPORARY_SUFFIX: &str = ".tmp";
/// How many hexadecimal digits the UUID in a temporary's name has.
const TEMPORARY_ID_DIGITS: usize = 32;
/// What follows the replaced file's extension in the name of the copy a backup keeps.
const BACKUP_SUFFIX: &str = "ghostlight-backup";
/// The extension a backup's name uses for a file that has none.
const BACKUP_BARE_EXTENSION: &str = "config";

/// Who may read the file a [`Replace`] publishes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    /// Only the current user, whatever the old file allowed: on Windows the new file is created
    /// with [`create_private`]'s protected access list, on Unix with mode 0600. Missing parent
    /// directories are created closed to everyone else on Unix.
    Private,
    /// The access the old file had, for a file other accounts or programs read. On Unix the new
    /// file gets the old file's mode; on Windows it inherits from its directory. A file that did
    /// not exist gets its directory's default access, and missing parent directories are created
    /// with the default access too.
    Kept,
}

/// What a [`Replace`] does when its path is a symbolic link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Link {
    /// Replace the file the link names, so the link stays a link to it. A link whose target is
    /// gone fails the replace.
    Follow,
    /// Fail with [`io::ErrorKind::InvalidInput`] and change nothing, so no bytes go where a link
    /// could send them.
    Refuse,
}

/// The stated options of Ghostlight's one atomic file replace (ADR-0203 Decision 11.2).
///
/// [`Replace::write`] puts the bytes in a new file beside the one it replaces, flushes them to
/// disk, and renames that file over the path. A reader sees the old file or the new one, never a
/// partial one. A failure at any step leaves the old file and no temporary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Replace {
    /// Who may read the new file.
    pub access: Access,
    /// Copy the old file to `<name>.<extension>.ghostlight-backup` beside it ([`backup_path`])
    /// before the new one is published. The system's copy makes it, so it keeps the mode on Unix
    /// and takes its directory's access on Windows.
    pub backup: bool,
    /// What a link at the path means.
    pub link: Link,
}

impl Replace {
    /// Ghostlight's own file, which holds a secret or a record about the person: private, no
    /// backup, and a link refused. What [`atomic_replace`] does.
    pub const OWN: Self = Self {
        access: Access::Private,
        backup: false,
        link: Link::Refuse,
    };

    /// A file other accounts or programs also read: its access kept, no backup, and written
    /// through a link. What [`atomic_replace_keeping_access`] does.
    pub const SHARED: Self = Self {
        access: Access::Kept,
        backup: false,
        link: Link::Follow,
    };

    /// Replace the file at `path` with `bytes`, atomically, under these options.
    pub fn write(self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let path = &self.target(path)?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
        match self.access {
            Access::Private => private::create_dir_all(parent)?,
            Access::Kept => fs::create_dir_all(parent)?,
        }
        let temporary = parent.join(format!(
            "{TEMPORARY_PREFIX}{}{TEMPORARY_SUFFIX}",
            uuid::Uuid::new_v4().simple()
        ));
        let published = (|| {
            let mut file = match self.access {
                Access::Private => create_private(&temporary)?,
                Access::Kept => create_like(&temporary, path)?,
            };
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            #[cfg(test)]
            steps::take(steps::Step::Synced(temporary.clone()));
            if self.backup && path.exists() {
                fs::copy(path, backup_path(path))?;
            }
            fs::rename(&temporary, path)?;
            #[cfg(test)]
            steps::take(steps::Step::Renamed(temporary.clone(), path.clone()));
            Ok(())
        })();
        if published.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        published
    }

    /// The file this replace writes for `path`: the path itself, or the file a link there names.
    /// A link refused, or one whose target is gone, is an error.
    pub fn target(self, path: &Path) -> io::Result<PathBuf> {
        let linked = fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink());
        match (linked, self.link) {
            (false, _) => Ok(path.to_path_buf()),
            (true, Link::Follow) => fs::canonicalize(path),
            (true, Link::Refuse) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                words::link_refused(&path.display().to_string()),
            )),
        }
    }
}

/// Replace the file at `path` with `bytes`, atomically, as Ghostlight's own: [`Replace::OWN`].
///
/// The new file is readable only by the current user, missing parent directories are created
/// closed to everyone else on Unix, and a link at `path` is refused.
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    Replace::OWN.write(path, bytes)
}

/// Replace the file at `path` with `bytes`, atomically, for a file that is not Ghostlight's own:
/// [`Replace::SHARED`].
///
/// The new file has the access the old one had instead of being private, so a policy file that
/// other accounts must read stays readable by them, and a new file gets its directory's default
/// access. A link at `path` is written through.
pub fn atomic_replace_keeping_access(path: &Path, bytes: &[u8]) -> io::Result<()> {
    Replace::SHARED.write(path, bytes)
}

/// Whether `name` is the name of a [`Replace`]'s temporary file, which an interrupted write may
/// leave beside the file it was replacing.
#[must_use]
pub fn is_temporary(name: &OsStr) -> bool {
    name.to_str()
        .and_then(|name| name.strip_prefix(TEMPORARY_PREFIX))
        .and_then(|name| name.strip_suffix(TEMPORARY_SUFFIX))
        .is_some_and(|id| {
            id.len() == TEMPORARY_ID_DIGITS && id.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

/// Where a [`Replace`] with a backup keeps the copy of the file at `path`.
#[must_use]
pub fn backup_path(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or(BACKUP_BARE_EXTENSION);
    path.with_extension(format!("{extension}.{BACKUP_SUFFIX}"))
}

/// Create a new file with the access of `existing`, which it is about to replace, or with the
/// directory's default access when there is none.
fn create_like(path: &Path, existing: &Path) -> io::Result<File> {
    let file = File::options().create_new(true).write(true).open(path)?;
    #[cfg(unix)]
    match fs::metadata(existing) {
        Ok(metadata) => file.set_permissions(metadata.permissions())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    #[cfg(not(unix))]
    let _ = existing;
    Ok(file)
}

/// The steps a [`Replace`] took on this thread, for the tests that prove their order.
#[cfg(test)]
pub(crate) mod steps {
    use std::cell::RefCell;
    use std::path::PathBuf;

    /// One step of a replace.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) enum Step {
        /// The temporary's bytes reached the disk.
        Synced(PathBuf),
        /// The temporary was renamed over the file it replaces.
        Renamed(PathBuf, PathBuf),
    }

    thread_local! {
        static TAKEN: RefCell<Vec<Step>> = const { RefCell::new(Vec::new()) };
    }

    /// Note that this thread's replace took `step`.
    pub(super) fn take(step: Step) {
        TAKEN.with(|taken| taken.borrow_mut().push(step));
    }

    /// The steps this thread's replaces took since the last call, in order.
    pub(crate) fn taken() -> Vec<Step> {
        TAKEN.with(RefCell::take)
    }
}

/// Create a new file at `path`, open for writing, that only the current user can read.
///
/// On Windows the file gets a protected access list for the current user and SYSTEM, applied at
/// creation and inheriting nothing from the directory; on Unix it gets mode 0600. An existing
/// path, a link included, is refused with [`io::ErrorKind::AlreadyExists`], so this never
/// overwrites. The parent directory must exist. The caller writes the bytes and syncs them.
pub fn create_private(path: &Path) -> io::Result<File> {
    #[cfg(windows)]
    return ghostlight_win_peer::create_private_file(path);
    #[cfg(not(windows))]
    private::file_options()
        .create_new(true)
        .write(true)
        .open(path)
}

/// Assert that only the current user can read the file at `path`: on Windows its access list is
/// protected and grants the current user and SYSTEM alone, on Unix its mode is 0600.
#[cfg(test)]
pub(crate) fn assert_owner_only(path: &Path) {
    #[cfg(windows)]
    {
        assert_eq!(
            windows_access_list(path),
            format!("D:P(A;;FA;;;{})(A;;FA;;;SY)", windows_user_sid()),
            "{}",
            path.display()
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{}", path.display());
    }
}

/// The access list of the file at `path` in the system's descriptor language, as `icacls`
/// saves it.
#[cfg(all(test, windows))]
fn windows_access_list(path: &Path) -> String {
    let saved = crate::scratch::Scratch::file("saved.acl");
    let status = std::process::Command::new("icacls")
        .arg(path)
        .arg("/save")
        .arg(saved.path())
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "icacls could not read {}", path.display());
    let units: Vec<u16> = fs::read(saved.path())
        .unwrap()
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let text = String::from_utf16(&units).unwrap();
    text.lines().nth(1).unwrap_or_default().trim().to_owned()
}

/// The security identifier of the user running the tests, as `whoami` prints it.
#[cfg(all(test, windows))]
fn windows_user_sid() -> String {
    let output = std::process::Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .unwrap();
    assert!(output.status.success(), "whoami could not name the user");
    let text = String::from_utf8_lossy(&output.stdout);
    let sid = text.trim().rsplit(',').next().unwrap_or_default();
    sid.trim_matches('"').to_owned()
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

    #[test]
    fn a_record_without_a_channel_reads_as_release_and_the_channel_round_trips() {
        let scratch = Scratch::new();
        let path = scratch.path().join("install.json");
        fs::write(
            &path,
            r#"{"service_version":"2.0.0","relay_revision":"4","service_sha256":"ab"}"#,
        )
        .unwrap();
        let read = InstallRecord::read(&path).unwrap().unwrap();
        assert_eq!(read.channel, Channel::Release);
        let development = InstallRecord {
            channel: Channel::Development,
            ..read
        };
        development.write(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(r#""channel": "development""#), "{text}");
        assert_eq!(InstallRecord::read(&path).unwrap(), Some(development));
    }

    #[test]
    fn a_record_written_before_the_edge_existed_reads_with_no_edge() {
        let scratch = Scratch::new();
        let path = scratch.path().join("install.json");
        fs::write(
            &path,
            r#"{"service_version":"2.0.0","relay_revision":"4","service_sha256":"ab"}"#,
        )
        .unwrap();
        let read = InstallRecord::read(&path).unwrap().unwrap();
        assert_eq!(read.edge_sha256, None);
        let with_edge = InstallRecord {
            edge_sha256: Some("cd".to_owned()),
            ..read
        };
        with_edge.write(&path).unwrap();
        assert_eq!(InstallRecord::read(&path).unwrap(), Some(with_edge));
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
    fn only_a_replace_temporary_is_recognised() {
        assert!(is_temporary(OsStr::new(
            ".ghostlight-0123456789abcdef0123456789abcdef.tmp"
        )));
        for other in [
            ".ghostlight-desktop-0123456789abcdef0123456789abcdef.tmp",
            ".ghostlight-0123456789abcdef.tmp",
            ".ghostlight-0123456789abcdef0123456789abcdef.json",
            "ghostlight-0123456789abcdef0123456789abcdef.tmp",
            "settings.json",
        ] {
            assert!(!is_temporary(OsStr::new(other)), "{other}");
        }
    }

    #[test]
    fn a_replace_names_its_temporary_with_the_one_pattern() {
        let scratch = Scratch::new();
        for (replace, name) in [(Replace::OWN, "own.json"), (Replace::SHARED, "shared.json")] {
            let path = scratch.path().join(name);
            steps::taken();
            replace.write(&path, b"{}").unwrap();
            let taken = steps::taken();
            let [steps::Step::Synced(temporary), steps::Step::Renamed(_, published)] =
                taken.as_slice()
            else {
                panic!("one synced write then one rename, not {taken:?}");
            };
            assert_eq!(temporary.parent(), path.parent());
            assert!(
                is_temporary(temporary.file_name().unwrap()),
                "{temporary:?}"
            );
            assert_eq!(published, &path);
        }
    }

    #[test]
    fn a_kept_access_replace_creates_a_new_file_and_its_directories() {
        let scratch = Scratch::new();
        let path = scratch.path().join("shared").join("policy.json");
        atomic_replace_keeping_access(&path, b"first").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let plain = scratch.path().join("plain");
            fs::create_dir(&plain).unwrap();
            fs::write(plain.join("file"), b"").unwrap();
            let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(path.parent().unwrap()), mode(&plain));
            assert_eq!(mode(&path), mode(&plain.join("file")));

            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            atomic_replace_keeping_access(&path, b"second").unwrap();
            assert_eq!(mode(&path), 0o644);
        }
    }

    #[test]
    fn a_backup_keeps_the_old_file_beside_the_new_one() {
        let scratch = Scratch::new();
        let path = scratch.path().join("settings.json");
        let replace = Replace {
            backup: true,
            ..Replace::SHARED
        };
        replace.write(&path, b"first").unwrap();
        assert!(!backup_path(&path).exists());
        replace.write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(
            backup_path(&path),
            scratch.path().join("settings.json.ghostlight-backup")
        );
        assert_eq!(fs::read(backup_path(&path)).unwrap(), b"first");
    }

    #[test]
    fn a_replace_target_without_a_link_is_the_path_itself() {
        let scratch = Scratch::new();
        let path = scratch.path().join("absent.json");
        assert_eq!(Replace::OWN.target(&path).unwrap(), path);
        assert_eq!(Replace::SHARED.target(&path).unwrap(), path);
    }

    #[cfg(unix)]
    #[test]
    fn an_own_replace_refuses_a_link_and_a_shared_one_writes_through_it() {
        use std::os::unix::fs::symlink;

        let scratch = Scratch::new();
        let real = scratch.path().join("real.json");
        fs::write(&real, b"old").unwrap();
        let link = scratch.path().join("link.json");
        symlink(&real, &link).unwrap();

        let refused = atomic_replace(&link, b"secret").unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read(&real).unwrap(), b"old");
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());

        atomic_replace_keeping_access(&link, b"new").unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(fs::read(&real).unwrap(), b"new");
    }

    #[cfg(windows)]
    #[test]
    fn an_own_replace_refuses_a_link_and_a_shared_one_writes_through_it() {
        use std::os::windows::fs::symlink_file;

        let scratch = Scratch::new();
        let real = scratch.path().join("real.json");
        fs::write(&real, b"old").unwrap();
        let link = scratch.path().join("link.json");
        if symlink_file(&real, &link).is_err() {
            // No symlink privilege (Developer Mode or an elevated shell) here; the Unix test
            // exercises the same replace.
            return;
        }

        let refused = atomic_replace(&link, b"secret").unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read(&real).unwrap(), b"old");
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());

        atomic_replace_keeping_access(&link, b"new").unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(fs::read(&real).unwrap(), b"new");
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
