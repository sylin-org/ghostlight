//! A version check the person starts, and the install of the newer release it finds (ADR-0206).
//!
//! Nothing here runs on its own: the window's About page and `ghostlight update` call it when the
//! person asks (ADR-0028 Decision 9, amended 2026-10-29). A check is one GET of GitHub's
//! latest-release record, with a fixed user agent and nothing that identifies the person, the
//! machine or the installation. An install downloads that release's `SHA256SUMS` and this
//! computer's three images, only through GitHub's release hosts, checks each image, stages them
//! beside each other, and starts the staged service's `deployment install`, which replaces the
//! running set the way every carrier does.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use url::Url;

use super::icons::sha256_hex;
use super::layout::program_file_name;
use super::record::Channel;
use crate::language::update as words;

/// GitHub's latest-release record for Ghostlight.
const LATEST_RELEASE: &str = "https://api.github.com/repos/sylin-org/ghostlight/releases/latest";
/// Where every asset of a release is downloaded from, before the tag.
const RELEASE_DOWNLOADS: &str = "https://github.com/sylin-org/ghostlight/releases/download/";
/// Where a release's own compatibility record is read, before the tag.
const RELEASE_SOURCE: &str = "https://raw.githubusercontent.com/sylin-org/ghostlight/";
/// The compatibility record's file name at a release's tag.
const COMPATIBILITY_FILE: &str = "compatibility.json";
/// The one user agent every request sends. GitHub requires one; this names the product and
/// nothing about the installation, not even its version.
const USER_AGENT: &str = "Ghostlight";
/// What the latest-release record is asked for as.
const GITHUB_JSON: &str = "application/vnd.github+json";
/// The only hosts a request or one of its redirects may reach: GitHub's API, its release download
/// hosts as the npm launcher allows them, and the source host the compatibility record is read
/// from.
const ALLOWED_HOSTS: [&str; 6] = [
    "api.github.com",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "github-releases.githubusercontent.com",
    "raw.githubusercontent.com",
];
/// The most redirects one download follows.
const MAX_REDIRECTS: usize = 5;
/// The largest latest-release record read.
const RECORD_BYTES: u64 = 1024 * 1024;
/// The largest checksum list read.
const SUMS_BYTES: u64 = 64 * 1024;
/// The largest compatibility record read.
const COMPATIBILITY_BYTES: u64 = 256 * 1024;
/// The largest image read.
const IMAGE_BYTES: u64 = 256 * 1024 * 1024;
/// The release notes shown, at most, in characters.
const NOTES_CHARS: usize = 4_000;
/// How long a connection or an answer's first byte may take.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(20);
/// How long one body may take to arrive: an image is tens of megabytes.
const BODY_TIMEOUT: Duration = Duration::from_secs(180);
/// The checksum list every release carries.
const SUMS_ASSET: &str = "SHA256SUMS";
/// The three images of a release, as its assets name them before the target.
const COMPONENTS: [&str; 3] = ["ghostlight", "ghostlight-service", "ghostlight-desktop"];
/// The staged service, among [`COMPONENTS`].
const SERVICE_COMPONENT: &str = "ghostlight-service";
/// The folder of the home a release is staged in, one subfolder per version.
pub const STAGING_DIRECTORY: &str = "updates";

/// A stable release version, `major.minor.patch`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    /// The version `text` names, or `None` for anything that is not three whole numbers.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.').map(|part| {
            (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse::<u64>().ok())
                .flatten()
        });
        let version = Self {
            major: parts.next()??,
            minor: parts.next()??,
            patch: parts.next()??,
        };
        parts.next().is_none().then_some(version)
    }

    /// Whether `other` is in this version's major.minor block, the contract an extension covers
    /// (ADR-0093).
    fn same_block(self, other: Self) -> bool {
        self.major == other.major && self.minor == other.minor
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Why a check or an install did not finish. Each says what the person reads.
#[derive(Debug, Error)]
pub enum UpdateError {
    /// No answer arrived, or the connection failed.
    #[error("{}", words::UNREACHABLE)]
    Unreachable(String),
    /// GitHub answered with something other than a stable Ghostlight release.
    #[error("{}", words::UNEXPECTED)]
    Unexpected(String),
    /// A request or a redirect led outside the allowed hosts.
    #[error("{}", words::WRONG_HOST)]
    Host(String),
    /// A downloaded image did not match its checksum.
    #[error("{}", words::CHECKSUM)]
    Checksum(String),
    /// The release has no images for this computer.
    #[error("{}", words::NO_IMAGES)]
    NoImages,
    /// The staged images could not be written, or the installer could not start.
    #[error("{}", words::NOT_STAGED)]
    Staging(#[from] io::Error),
}

/// One GET of a URL, read up to `limit` bytes. The real transport is [`Https`]; tests answer
/// from a table.
pub trait Fetch: Send + Sync {
    /// The body `url` answers with.
    ///
    /// # Errors
    ///
    /// Fails when the URL or a redirect leaves the allowed hosts, when no answer arrives, on any
    /// status but success, and when the body is larger than `limit`.
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, UpdateError>;
}

/// The latest stable release, pinned to its tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    /// The release's version.
    pub version: Version,
    /// Its tag, `v` and the version.
    pub tag: String,
    /// Its notes as plain text, bounded.
    pub notes: String,
    /// Every asset's download URL, by asset name, each inside this release's download folder.
    assets: BTreeMap<String, String>,
}

impl Release {
    fn asset(&self, name: &str) -> Result<&str, UpdateError> {
        self.assets
            .get(name)
            .map(String::as_str)
            .ok_or(UpdateError::NoImages)
    }
}

/// What a check found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Finding {
    /// The installed version is the latest, or newer.
    Current {
        /// The installed version.
        installed: Version,
    },
    /// A newer release can be installed.
    Newer {
        /// The installed version.
        installed: Version,
        /// The newer release.
        release: Release,
    },
    /// A newer release needs a newer extension than the connected browsers run (ADR-0093).
    NeedsExtension {
        /// The installed version.
        installed: Version,
        /// The newer release.
        release: Release,
    },
    /// A development build: the latest release is named and nothing is offered.
    Development {
        /// The latest release's version.
        latest: Version,
    },
}

/// The latest-release record, as far as the check reads it.
#[derive(Deserialize)]
struct ReleaseRecord {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<AssetRecord>,
}

#[derive(Deserialize)]
struct AssetRecord {
    name: String,
    browser_download_url: String,
}

/// Read the latest release: a stable `vX.Y.Z` tag, not a draft or a prerelease, whose every asset
/// downloads from that tag's own folder (docs/MEMORY.md, "Release identity precedes asset
/// transport").
///
/// # Errors
///
/// [`UpdateError::Unexpected`] for any other record, and the transport's failure.
pub fn latest(fetch: &dyn Fetch) -> Result<Release, UpdateError> {
    let bytes = fetch.get(LATEST_RELEASE, RECORD_BYTES)?;
    let record: ReleaseRecord = serde_json::from_slice(&bytes)
        .map_err(|error| UpdateError::Unexpected(error.to_string()))?;
    let version = record
        .tag_name
        .strip_prefix('v')
        .and_then(Version::parse)
        .filter(|_| !record.draft && !record.prerelease)
        .ok_or_else(|| UpdateError::Unexpected(record.tag_name.clone()))?;
    let folder = format!("{RELEASE_DOWNLOADS}{}/", record.tag_name);
    let mut assets = BTreeMap::new();
    for asset in record.assets {
        if !asset.browser_download_url.starts_with(&folder) {
            return Err(UpdateError::Unexpected(asset.browser_download_url));
        }
        assets.insert(asset.name, asset.browser_download_url);
    }
    Ok(Release {
        version,
        tag: record.tag_name,
        notes: record
            .body
            .unwrap_or_default()
            .chars()
            .take(NOTES_CHARS)
            .collect(),
        assets,
    })
}

/// Check for a newer release than `installed`, for an installation of `channel` whose connected
/// browsers run the extension versions `adapters`.
///
/// A release in another major.minor block is offered only when that release's own compatibility
/// record covers every connected extension, and when at least one is connected to say so.
///
/// # Errors
///
/// The failure of reading the latest release or its compatibility record.
pub fn check(
    fetch: &dyn Fetch,
    installed: Version,
    channel: Channel,
    adapters: &[String],
) -> Result<Finding, UpdateError> {
    let release = latest(fetch)?;
    if channel == Channel::Development {
        return Ok(Finding::Development {
            latest: release.version,
        });
    }
    if release.version <= installed {
        return Ok(Finding::Current { installed });
    }
    if !release.version.same_block(installed) {
        let url = format!("{RELEASE_SOURCE}{}/{COMPATIBILITY_FILE}", release.tag);
        let record: Value = serde_json::from_slice(&fetch.get(&url, COMPATIBILITY_BYTES)?)
            .map_err(|error| UpdateError::Unexpected(error.to_string()))?;
        let covered = !adapters.is_empty()
            && adapters
                .iter()
                .all(|adapter| covers(&record, adapter, release.version));
        if !covered {
            return Ok(Finding::NeedsExtension { installed, release });
        }
    }
    Ok(Finding::Newer { installed, release })
}

/// Whether the compatibility record names `adapter` with a block or range that holds `service`.
fn covers(record: &Value, adapter: &str, service: Version) -> bool {
    record["chromeAdapters"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["adapterVersion"].as_str() == Some(adapter))
        .any(|row| match row["serviceVersionBlock"].as_str() {
            Some(block) => block == format!("{}.{}", service.major, service.minor),
            None => {
                let bound = |field: &str| row[field].as_str().and_then(Version::parse);
                matches!(
                    (bound("minimumServiceVersion"), bound("maximumServiceVersion")),
                    (Some(minimum), Some(maximum)) if minimum <= service && service <= maximum
                )
            }
        })
}

/// The target this service was built for, as release assets name it, or `None` on a computer
/// the releases carry no images for.
#[must_use]
pub const fn this_target() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("x86_64-pc-windows-msvc")
    } else if cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "musl"
    )) {
        Some("x86_64-unknown-linux-musl")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("x86_64-unknown-linux-gnu")
    } else {
        None
    }
}

/// A release staged in a folder of the home, ready for its own `deployment install`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Staged {
    /// The folder holding the three images.
    pub directory: PathBuf,
    /// The staged service.
    pub service: PathBuf,
}

/// Download `release`'s checksum list and its three images for `target`, check each against the
/// list, and write them into a fresh `staging/<version>` folder under the names the installer
/// reads. Nothing is written until every image matched.
///
/// # Errors
///
/// [`UpdateError::NoImages`] when the release lacks one, [`UpdateError::Checksum`] for an image
/// that does not match, the transport's failure, and [`UpdateError::Staging`] when the folder
/// cannot be written.
pub fn stage(
    fetch: &dyn Fetch,
    release: &Release,
    target: &str,
    windows: bool,
    staging: &Path,
) -> Result<Staged, UpdateError> {
    let sums = String::from_utf8(fetch.get(release.asset(SUMS_ASSET)?, SUMS_BYTES)?)
        .map_err(|error| UpdateError::Unexpected(error.to_string()))?;
    let mut images = Vec::with_capacity(COMPONENTS.len());
    for component in COMPONENTS {
        let name = format!("{component}-{target}{}", if windows { ".exe" } else { "" });
        let expected = bound_checksum(&sums, &name).ok_or(UpdateError::NoImages)?;
        let bytes = fetch.get(release.asset(&name)?, IMAGE_BYTES)?;
        if sha256_hex(&bytes) != expected {
            return Err(UpdateError::Checksum(name));
        }
        images.push((component, bytes));
    }
    let directory = staging.join(release.version.to_string());
    match fs::remove_dir_all(&directory) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    fs::create_dir_all(&directory)?;
    for (component, bytes) in &images {
        let path = directory.join(program_file_name(component, windows));
        fs::write(&path, bytes)?;
        executable(&path)?;
    }
    Ok(Staged {
        service: directory.join(program_file_name(SERVICE_COMPONENT, windows)),
        directory,
    })
}

/// The checksum `SHA256SUMS` binds to `name`: a line of 64 hexadecimal characters, white space,
/// an optional `*`, and exactly that name.
fn bound_checksum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        let file = file.trim_start().trim_start_matches('*').trim_end();
        (file == name && hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

#[cfg(unix)]
fn executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the Unix twin sets permissions and can fail"
)]
fn executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// The channel of the set installed in `layout`'s `bin`. A home with no install record holds no
/// release this check may replace, so it reads as the development channel and nothing is
/// offered.
#[must_use]
pub fn installed_channel(layout: &super::layout::Layout) -> Channel {
    match super::record::InstallRecord::read(&layout.install_record()) {
        Ok(Some(record)) => record.channel,
        _ => Channel::Development,
    }
}

/// Start the staged service's `deployment install`, detached, so it outlives the service it
/// replaces. Returns its process id.
///
/// # Errors
///
/// The failure to start it.
pub fn start(staged: &Staged) -> io::Result<u32> {
    ghostlight_relay::os::spawn_detached(
        &staged.service,
        &[OsStr::new("deployment"), OsStr::new("install")],
        &staged.directory,
    )
}

/// Refuse `url` unless it is HTTPS on one of the allowed hosts.
fn allowed(url: &Url) -> Result<(), UpdateError> {
    let host = url.host_str().unwrap_or_default();
    if url.scheme() == "https" && ALLOWED_HOSTS.contains(&host) {
        Ok(())
    } else {
        Err(UpdateError::Host(url.to_string()))
    }
}

/// The real transport: HTTPS against Mozilla's roots, through a proxy the environment names,
/// following each redirect only while it stays on the allowed hosts.
pub struct Https {
    agent: ureq::Agent,
}

impl Https {
    /// A transport for one check or one install.
    #[must_use]
    pub fn new() -> Self {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::WebPki)
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        let agent = ureq::Agent::config_builder()
            // Each redirect comes back as its own status, so its host is checked first.
            .max_redirects(0)
            .http_status_as_error(false)
            .proxy(ureq::Proxy::try_from_env())
            .timeout_connect(Some(ANSWER_TIMEOUT))
            .timeout_recv_response(Some(ANSWER_TIMEOUT))
            .timeout_recv_body(Some(BODY_TIMEOUT))
            .tls_config(tls)
            .build()
            .new_agent();
        Self { agent }
    }
}

impl Default for Https {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetch for Https {
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, UpdateError> {
        let mut current = Url::parse(url).map_err(|_| UpdateError::Host(url.into()))?;
        for _ in 0..=MAX_REDIRECTS {
            allowed(&current)?;
            let mut request = self
                .agent
                .get(current.as_str())
                .header("User-Agent", USER_AGENT);
            if current.host_str() == Some(ALLOWED_HOSTS[0]) {
                request = request.header("Accept", GITHUB_JSON);
            }
            let response = request
                .call()
                .map_err(|error| UpdateError::Unreachable(error.to_string()))?;
            match response.status().as_u16() {
                200 => {
                    return response
                        .into_body()
                        .with_config()
                        .limit(limit)
                        .read_to_vec()
                        .map_err(|error| UpdateError::Unreachable(error.to_string()));
                }
                301 | 302 | 303 | 307 | 308 => {
                    let location = response
                        .headers()
                        .get("Location")
                        .and_then(|value| value.to_str().ok())
                        .ok_or_else(|| {
                            UpdateError::Unexpected("a redirect named no place".into())
                        })?;
                    current = current
                        .join(location)
                        .map_err(|_| UpdateError::Host(location.into()))?;
                }
                status => return Err(UpdateError::Unexpected(format!("HTTP {status}"))),
            }
        }
        Err(UpdateError::Unexpected("too many redirects".into()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{json, Value};

    use super::*;

    /// A transport that answers from a table, so no test reaches the network.
    struct Table(HashMap<String, Vec<u8>>);

    impl Fetch for Table {
        fn get(&self, url: &str, _limit: u64) -> Result<Vec<u8>, UpdateError> {
            self.0
                .get(url)
                .cloned()
                .ok_or_else(|| UpdateError::Unreachable(url.into()))
        }
    }

    const TARGET: &str = "x86_64-unknown-linux-gnu";

    fn asset_url(tag: &str, name: &str) -> String {
        format!("{RELEASE_DOWNLOADS}{tag}/{name}")
    }

    /// The latest-release record for `tag`, with `changes` merged over a stable release whose
    /// assets are the checksum list and this target's three images.
    fn record(tag: &str, changes: &Value) -> Vec<u8> {
        let names = COMPONENTS
            .iter()
            .map(|component| format!("{component}-{TARGET}"))
            .chain([SUMS_ASSET.to_owned()]);
        let mut record = json!({
            "tag_name": tag,
            "draft": false,
            "prerelease": false,
            "body": "What changed.",
            "assets": names
                .map(|name| json!({"name": name, "browser_download_url": asset_url(tag, &name)}))
                .collect::<Vec<_>>(),
        });
        for (key, value) in changes.as_object().into_iter().flatten() {
            record[key] = value.clone();
        }
        serde_json::to_vec(&record).unwrap()
    }

    fn latest_is(tag: &str, changes: &Value) -> Table {
        Table(HashMap::from([(
            LATEST_RELEASE.to_owned(),
            record(tag, changes),
        )]))
    }

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    /// ADR-0206 Decisions 3 and 6: a check offers only a stable release newer than the installed
    /// one, pinned to its own tag; a development build is told the latest and offered nothing;
    /// and a release in another major.minor block is offered only when its own compatibility
    /// record covers every connected extension (ADR-0093).
    #[test]
    fn a_check_offers_only_a_stable_newer_release_the_browsers_can_run() {
        let installed = version("2.0.2");
        let adapters = ["2.0.1".to_owned()];
        let found = |table: &Table, channel: Channel, adapters: &[String]| {
            check(table, installed, channel, adapters)
        };

        assert!(matches!(
            found(
                &latest_is("v2.0.2", &json!({})),
                Channel::Release,
                &adapters
            ),
            Ok(Finding::Current { .. })
        ));
        match found(
            &latest_is("v2.0.3", &json!({})),
            Channel::Release,
            &adapters,
        ) {
            Ok(Finding::Newer { release, .. }) => {
                assert_eq!(release.version, version("2.0.3"));
                assert_eq!(release.notes, "What changed.");
            }
            other => panic!("a newer patch is offered: {other:?}"),
        }
        assert!(matches!(
            found(&latest_is("v2.0.3", &json!({})), Channel::Development, &adapters),
            Ok(Finding::Development { latest }) if latest == version("2.0.3")
        ));
        for refused in [
            json!({"draft": true}),
            json!({"prerelease": true}),
            json!({"tag_name": "v2.0.3-rc.1"}),
            json!({"tag_name": "2.0.3"}),
            json!({"assets": [{"name": "SHA256SUMS", "browser_download_url": "https://example.com/SHA256SUMS"}]}),
            json!({"assets": [{"name": "SHA256SUMS", "browser_download_url": asset_url("v2.0.2", "SHA256SUMS")}]}),
        ] {
            assert!(
                matches!(
                    found(&latest_is("v2.0.3", &refused), Channel::Release, &adapters),
                    Err(UpdateError::Unexpected(_))
                ),
                "{refused}"
            );
        }

        // A new minor reads that release's compatibility record.
        let compatibility = |row: Value| {
            let mut table = latest_is("v2.1.0", &json!({}));
            table.0.insert(
                format!("{RELEASE_SOURCE}v2.1.0/{COMPATIBILITY_FILE}"),
                serde_json::to_vec(&json!({"schemaVersion": 2, "chromeAdapters": [row]})).unwrap(),
            );
            table
        };
        let covering =
            compatibility(json!({"adapterVersion": "2.0.1", "serviceVersionBlock": "2.1"}));
        assert!(matches!(
            found(&covering, Channel::Release, &adapters),
            Ok(Finding::Newer { .. })
        ));
        assert!(
            matches!(
                found(&covering, Channel::Release, &[]),
                Ok(Finding::NeedsExtension { .. })
            ),
            "no connected browser can say it is covered"
        );
        let older_block =
            compatibility(json!({"adapterVersion": "2.0.1", "serviceVersionBlock": "2.0"}));
        assert!(matches!(
            found(&older_block, Channel::Release, &adapters),
            Ok(Finding::NeedsExtension { .. })
        ));
        let ranged = compatibility(json!({
            "adapterVersion": "2.0.1",
            "minimumServiceVersion": "2.0.0",
            "maximumServiceVersion": "2.1.5"
        }));
        assert!(matches!(
            found(&ranged, Channel::Release, &adapters),
            Ok(Finding::Newer { .. })
        ));
    }

    /// ADR-0206 Decision 5: an install stages the three images only when each matches the
    /// release's checksum list, under the names the installer reads; one that does not match
    /// stages nothing. A download is refused before it starts unless it is HTTPS on GitHub's
    /// release hosts.
    #[test]
    fn an_install_stages_only_images_that_match_their_checksums() {
        let tag = "v2.0.3";
        let release = latest(&latest_is(tag, &json!({}))).unwrap();
        let images: Vec<(String, Vec<u8>)> = COMPONENTS
            .iter()
            .map(|component| {
                (
                    format!("{component}-{TARGET}"),
                    component.as_bytes().to_vec(),
                )
            })
            .collect();
        let downloads = |tampered: Option<&str>| {
            let sums: String = images
                .iter()
                .map(|(name, bytes)| format!("{}  {name}\n", sha256_hex(bytes)))
                .collect();
            let mut table = HashMap::from([(asset_url(tag, SUMS_ASSET), sums.into_bytes())]);
            for (name, bytes) in &images {
                let served = if tampered == Some(name.as_str()) {
                    b"something else".to_vec()
                } else {
                    bytes.clone()
                };
                table.insert(asset_url(tag, name), served);
            }
            Table(table)
        };
        let root = crate::scratch::Scratch::named("update-stage");
        let staging = root.path().join(STAGING_DIRECTORY);

        let staged = stage(&downloads(None), &release, TARGET, false, &staging).unwrap();
        assert_eq!(staged.directory, staging.join("2.0.3"));
        assert_eq!(staged.service, staged.directory.join("ghostlight-service"));
        for component in COMPONENTS {
            assert_eq!(
                fs::read(staged.directory.join(component)).unwrap(),
                component.as_bytes()
            );
        }

        fs::remove_dir_all(&staging).unwrap();
        let tampered = format!("ghostlight-desktop-{TARGET}");
        assert!(matches!(
            stage(&downloads(Some(&tampered)), &release, TARGET, false, &staging),
            Err(UpdateError::Checksum(name)) if name == tampered
        ));
        assert!(
            !staging.exists(),
            "nothing is staged when one image does not match"
        );
        assert!(matches!(
            stage(
                &downloads(None),
                &release,
                "x86_64-apple-darwin",
                false,
                &staging
            ),
            Err(UpdateError::NoImages)
        ));

        for url in [
            "http://github.com/sylin-org/ghostlight/releases/download/v2.0.3/SHA256SUMS",
            "https://example.com/ghostlight-service",
            "https://github.com.example.com/x",
        ] {
            assert!(
                matches!(
                    allowed(&Url::parse(url).unwrap()),
                    Err(UpdateError::Host(_))
                ),
                "{url}"
            );
        }
        assert!(allowed(&Url::parse(LATEST_RELEASE).unwrap()).is_ok());
    }
}
