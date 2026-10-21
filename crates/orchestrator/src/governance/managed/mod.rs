//! Opt-in organization-managed policy from signed local or HTTPS bundles.

mod bundle;
mod crypto;
mod http;

pub mod cli;

#[cfg(target_os = "windows")]
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use url::Url;

use super::effective::{ContactView, OrganizationIdentity};
use super::file_state::FileState;
use super::{
    manifest, ManagedPolicyContact, ManagedPolicyFreshness, ManagedPolicyPassport,
    ManagedPolicySource,
};
use crate::install::layout::Layout;
use crate::install::record::atomic_replace;

const DEFAULT_POLL_SECONDS: u64 = 900;
const MAX_POLL_SECONDS: u64 = 86_400;

#[derive(Clone, Debug)]
pub(crate) struct ManagedPaths {
    bootstrap: PathBuf,
    cache: Option<PathBuf>,
    status: Option<PathBuf>,
}

impl ManagedPaths {
    pub(crate) fn production() -> Self {
        #[cfg(target_os = "windows")]
        let bootstrap = env::var_os("PROGRAMDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("Ghostlight")
            .join("managed.json");
        #[cfg(target_os = "linux")]
        let bootstrap = PathBuf::from("/etc/ghostlight/managed.json");

        let layout = Layout::current().ok();

        Self {
            bootstrap,
            cache: layout.as_ref().map(Layout::managed_policy_bundle),
            status: layout.as_ref().map(Layout::managed_status),
        }
    }

    #[cfg(test)]
    pub(crate) fn under(path: &Path) -> Self {
        use crate::install::layout::{MANAGED_POLICY_BUNDLE_NAME, MANAGED_STATUS_NAME};

        Self {
            bootstrap: path.join("managed.json"),
            cache: Some(path.join(MANAGED_POLICY_BUNDLE_NAME)),
            status: Some(path.join(MANAGED_STATUS_NAME)),
        }
    }

    /// Paths under `root` whose bootstrap names one local bundle signed over `manifest`, so a test
    /// layers an organization policy the one way an administrator publishes it.
    #[cfg(test)]
    pub(crate) fn signed(root: &Path, manifest: &str) -> Self {
        Self::signed_at(root, manifest, 1)
    }

    /// [`Self::signed`] at publish `sequence`. Calling it again under the same `root` re-signs
    /// the one local bundle in place, the way an administrator publishes the next sequence.
    #[cfg(test)]
    pub(crate) fn signed_at(root: &Path, manifest: &str, sequence: u64) -> Self {
        const SEED: [u8; 32] = [43; 32];
        const SOURCE: &str = "org.bundle";
        let paths = Self::under(root);
        let manifest = serde_json::from_str(manifest).expect("a test manifest is JSON");
        fs::write(
            root.join(SOURCE),
            bundle::sign(&SEED, None, sequence, manifest, None),
        )
        .expect("the test bundle is written");
        let bootstrap = serde_json::json!({
            "source": SOURCE,
            "pubkey_ed25519": encode_hex(&crypto::signing::ed25519_public(&SEED)),
        });
        fs::write(
            &paths.bootstrap,
            serde_json::to_vec_pretty(&bootstrap).expect("the bootstrap serializes"),
        )
        .expect("the test bootstrap is written");
        paths
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bootstrap {
    source: String,
    pubkey_ed25519: String,
    #[serde(default)]
    pubkey_mldsa: Option<String>,
    #[serde(default)]
    bearer_token: Option<String>,
    #[serde(default)]
    ca_cert_pem: Option<String>,
    #[serde(default)]
    poll_seconds: Option<u64>,
}

#[derive(Clone, Debug)]
struct ActivePolicy {
    manifest: manifest::Manifest,
    sequence: u64,
    /// The organization this publication names, resolved once when it was verified.
    organization: Option<OrganizationIdentity>,
    envelope_hash: [u8; 32],
}

impl ActivePolicy {
    /// A verified publication, its organization resolved from the signed presentation over the
    /// manifest's own block.
    fn verified(
        manifest: manifest::Manifest,
        verified: bundle::VerifiedBundle,
        envelope_hash: [u8; 32],
    ) -> Self {
        let signed = verified
            .presentation
            .map(|presentation| OrganizationIdentity {
                name: presentation.org_name,
                statement: presentation.rationale,
                url: None,
                contacts: presentation
                    .contacts
                    .into_iter()
                    .map(|contact| ContactView {
                        kind: contact.kind,
                        value: contact.value,
                        label: contact.label,
                    })
                    .collect(),
            });
        let organization = OrganizationIdentity::resolve(signed, manifest.organization.as_ref());
        Self {
            manifest,
            sequence: verified.sequence,
            organization,
            envelope_hash,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct ManagedStatus {
    v: u32,
    configured: bool,
    verified: bool,
    freshness: ManagedPolicyFreshness,
    #[serde(skip_serializing_if = "Option::is_none")]
    stale_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contacts: Option<Vec<ManagedPolicyContact>>,
    source_class: ManagedPolicySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_success_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_attempt_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ManagedAuthority {
    paths: ManagedPaths,
    configured: bool,
    bootstrap: Option<Bootstrap>,
    bootstrap_hash: Option<[u8; 32]>,
    /// The bootstrap file as last read. `None` until the first read.
    bootstrap_state: Option<FileState>,
    active: Option<ActivePolicy>,
    etag: Option<String>,
    /// The local bundle source as last looked at. `None` means not looked at yet.
    source_fingerprint: Option<FileState>,
    next_attempt: Option<Instant>,
    consecutive_failures: u32,
    last_success_ms: Option<u64>,
    last_attempt_ms: Option<u64>,
    last_error: Option<String>,
    stale_reason: Option<&'static str>,
}

impl ManagedAuthority {
    pub(crate) fn production() -> Self {
        Self::new(ManagedPaths::production())
    }

    #[cfg(test)]
    pub(crate) fn from_paths(paths: ManagedPaths) -> Self {
        Self::new(paths)
    }

    fn new(paths: ManagedPaths) -> Self {
        Self {
            paths,
            configured: false,
            bootstrap: None,
            bootstrap_hash: None,
            bootstrap_state: None,
            active: None,
            etag: None,
            source_fingerprint: None,
            next_attempt: None,
            consecutive_failures: 0,
            last_success_ms: None,
            last_attempt_ms: None,
            last_error: None,
            stale_reason: None,
        }
    }

    /// Read the bootstrap and its source, fetching over HTTPS when the poll is due.
    pub(crate) fn refresh(&mut self) {
        self.refresh_with(true);
    }

    /// Read the bootstrap, the verified cache and any local bundle source, and touch no network.
    ///
    /// A service starts from this, so the last verified policy is enforced before the first poll
    /// and a slow endpoint never delays start.
    pub(crate) fn load_offline(&mut self) {
        self.refresh_with(false);
    }

    /// Whether `refresh` has anything to do: the bootstrap file changed, the HTTPS poll is due,
    /// or the local bundle source changed. It examines file metadata only.
    pub(crate) fn needs_refresh(&self) -> bool {
        if self.bootstrap_state != Some(FileState::of(&self.paths.bootstrap)) {
            return true;
        }
        let Some(bootstrap) = &self.bootstrap else {
            return false;
        };
        if is_https(&bootstrap.source) {
            self.next_attempt
                .is_none_or(|deadline| Instant::now() >= deadline)
        } else {
            let path = local_source_path(&self.paths.bootstrap, &bootstrap.source);
            self.source_fingerprint != Some(FileState::of(&path))
        }
    }

    fn refresh_with(&mut self, network: bool) {
        self.bootstrap_state = Some(FileState::of(&self.paths.bootstrap));
        let bytes = match fs::read(&self.paths.bootstrap) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if self.configured {
                    // Like every other failure path, a missing descriptor on a configured
                    // machine calls reject(): `configured` stays true and the last verified
                    // policy stays active -- fail closed, not open. A deployment tool that
                    // updates the bootstrap descriptor by delete-then-recreate, rather than an
                    // atomic rename, hits exactly this window: an already-managed machine must
                    // not become indistinguishable from one that was never managed at all just
                    // because the file was briefly absent.
                    self.reject(
                        format!("managed bootstrap could not be read: {error}"),
                        "bootstrap_error",
                    );
                }
                return;
            }
            Err(error) => {
                self.configured = true;
                self.reject(
                    format!("managed bootstrap could not be read: {error}"),
                    "bootstrap_error",
                );
                return;
            }
        };
        self.configured = true;
        let bootstrap: Bootstrap = match serde_json::from_slice(&bytes) {
            Ok(bootstrap) => bootstrap,
            Err(error) => {
                self.reject(
                    format!("managed bootstrap is invalid: {error}"),
                    "bootstrap_error",
                );
                return;
            }
        };
        if let Err(error) = validate_bootstrap(&bootstrap) {
            self.reject(error, "bootstrap_error");
            return;
        }
        let hash = digest(&bytes);
        if self.bootstrap_hash != Some(hash) {
            self.accept_bootstrap(bootstrap, hash);
        }
        let Some(bootstrap) = self.bootstrap.clone() else {
            return;
        };
        if is_https(&bootstrap.source) {
            if !network
                || self
                    .next_attempt
                    .is_some_and(|deadline| Instant::now() < deadline)
            {
                return;
            }
        } else if !self.local_source_changed(&bootstrap.source) {
            return;
        }
        self.last_attempt_ms = Some(unix_ms());
        match self.fetch(&bootstrap) {
            Ok(Fresh::NotModified) if self.active.is_some() => self.accept_unchanged(&bootstrap),
            Ok(Fresh::NotModified) => self.reject(
                "managed source returned no policy on cold start".into(),
                "no_policy",
            ),
            Ok(Fresh::Modified { bytes, etag }) => self.accept_bundle(&bootstrap, &bytes, etag),
            Err(error) => self.reject(error, "source_unreachable"),
        }
    }

    pub(crate) fn configured(&self) -> bool {
        self.configured
    }

    pub(crate) fn valid(&self) -> bool {
        !self.configured || self.active.is_some()
    }

    pub(crate) fn manifest(&self) -> Option<&manifest::Manifest> {
        self.active.as_ref().map(|active| &active.manifest)
    }

    pub(crate) fn sequence(&self) -> Option<u64> {
        self.active.as_ref().map(|active| active.sequence)
    }

    /// The organization the active publication names.
    pub(crate) fn organization(&self) -> Option<&OrganizationIdentity> {
        self.active
            .as_ref()
            .and_then(|active| active.organization.as_ref())
    }

    /// Provenance of the active publication, naming the organization it resolved.
    pub(crate) fn passport(&self) -> ManagedPolicyPassport {
        ManagedPolicyPassport {
            configured: self.configured,
            verified: self.active.is_some(),
            freshness: if !self.configured {
                ManagedPolicyFreshness::NotConfigured
            } else if self.active.is_none() {
                ManagedPolicyFreshness::NoPolicy
            } else if self.stale_reason.is_some() {
                ManagedPolicyFreshness::LastKnownGood
            } else {
                ManagedPolicyFreshness::Fresh
            },
            sequence: self.active.as_ref().map(|active| active.sequence),
            organization: None,
            rationale: None,
            contacts: Vec::new(),
            source_class: self
                .bootstrap
                .as_ref()
                .map_or(ManagedPolicySource::None, |bootstrap| {
                    if is_https(&bootstrap.source) {
                        ManagedPolicySource::Https
                    } else {
                        ManagedPolicySource::File
                    }
                }),
            last_success_ms: self.last_success_ms,
            last_attempt_ms: self.last_attempt_ms,
        }
        .naming(self.organization())
    }

    fn accept_bootstrap(&mut self, bootstrap: Bootstrap, hash: [u8; 32]) {
        self.bootstrap = Some(bootstrap.clone());
        self.bootstrap_hash = Some(hash);
        self.active = self.read_cache(&bootstrap);
        self.etag = None;
        self.source_fingerprint = None;
        self.next_attempt = None;
        self.consecutive_failures = 0;
        self.last_error = None;
        self.stale_reason = self.active.as_ref().map(|_| "awaiting_refresh");
    }

    fn fetch(&mut self, bootstrap: &Bootstrap) -> Result<Fresh, String> {
        if is_https(&bootstrap.source) {
            return http::fetch(bootstrap, self.etag.as_deref())
                .map(|outcome| match outcome {
                    http::FetchOutcome::Modified { bytes, etag } => Fresh::Modified { bytes, etag },
                    http::FetchOutcome::NotModified => Fresh::NotModified,
                })
                .map_err(|error| error.to_string());
        }
        let path = local_source_path(&self.paths.bootstrap, &bootstrap.source);
        fs::read(&path)
            .map(|bytes| Fresh::Modified { bytes, etag: None })
            .map_err(|error| format!("managed source could not be read: {error}"))
    }

    fn accept_bundle(&mut self, bootstrap: &Bootstrap, bytes: &[u8], etag: Option<String>) {
        let key = match key_from_bootstrap(bootstrap) {
            Ok(key) => key,
            Err(error) => {
                self.reject(error, "bootstrap_error");
                return;
            }
        };
        let verified = match bundle::verify(bytes, &key) {
            Ok(bundle) => bundle,
            Err(error) => {
                self.reject(error.to_string(), "update_rejected");
                return;
            }
        };
        let text = match serde_json::to_string(&verified.manifest) {
            Ok(text) => text,
            Err(error) => {
                self.reject(
                    format!("managed manifest could not be decoded: {error}"),
                    "update_rejected",
                );
                return;
            }
        };
        let manifest = match manifest::parse(&text, "signed managed policy") {
            Ok(manifest) => manifest,
            Err(error) => {
                self.reject(error.to_string(), "update_rejected");
                return;
            }
        };
        let envelope_hash = digest(bytes);
        if let Some(active) = &self.active {
            if verified.sequence < active.sequence {
                self.reject(
                    format!(
                        "managed policy rollback refused: sequence {} is older than {}",
                        verified.sequence, active.sequence
                    ),
                    "rollback_refused",
                );
                return;
            }
            if verified.sequence == active.sequence && envelope_hash != active.envelope_hash {
                self.reject(
                    format!(
                        "managed policy sequence {} was reused for different signed content",
                        verified.sequence
                    ),
                    "sequence_reused",
                );
                return;
            }
        }
        self.active = Some(ActivePolicy::verified(manifest, verified, envelope_hash));
        if let Some(path) = &self.paths.cache {
            if let Err(error) = atomic_replace(path, bytes) {
                self.last_error = Some(format!(
                    "verified managed policy could not be cached: {error}"
                ));
            } else {
                self.last_error = None;
            }
        } else {
            self.last_error =
                Some("no user state directory is available for the managed policy cache".into());
        }
        self.etag = etag;
        self.last_success_ms = Some(unix_ms());
        self.stale_reason = None;
        self.consecutive_failures = 0;
        self.schedule(bootstrap, false);
        self.write_status();
    }

    fn accept_unchanged(&mut self, bootstrap: &Bootstrap) {
        self.last_success_ms = Some(unix_ms());
        self.last_error = None;
        self.stale_reason = None;
        self.consecutive_failures = 0;
        self.schedule(bootstrap, false);
        self.write_status();
    }

    fn reject(&mut self, error: String, reason: &'static str) {
        if self.last_error.as_deref() != Some(error.as_str()) {
            if self.active.is_some() {
                eprintln!("Ghostlight kept the last verified managed policy: {error}");
            } else {
                eprintln!("Ghostlight managed policy is not available: {error}");
            }
        }
        self.last_error = Some(error);
        self.stale_reason = Some(reason);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if let Some(bootstrap) = self.bootstrap.clone() {
            self.schedule(&bootstrap, true);
        }
        self.write_status();
    }

    fn read_cache(&self, bootstrap: &Bootstrap) -> Option<ActivePolicy> {
        let key = key_from_bootstrap(bootstrap).ok()?;
        let bytes = fs::read(self.paths.cache.as_ref()?).ok()?;
        let verified = bundle::verify(&bytes, &key).ok()?;
        let text = serde_json::to_string(&verified.manifest).ok()?;
        Some(ActivePolicy::verified(
            manifest::parse(&text, "verified managed policy cache").ok()?,
            verified,
            digest(&bytes),
        ))
    }

    fn local_source_changed(&mut self, source: &str) -> bool {
        let path = local_source_path(&self.paths.bootstrap, source);
        let current = Some(FileState::of(&path));
        if self.source_fingerprint == current {
            return false;
        }
        self.source_fingerprint = current;
        true
    }

    fn schedule(&mut self, bootstrap: &Bootstrap, failed: bool) {
        if !is_https(&bootstrap.source) {
            self.next_attempt = None;
            return;
        }
        let base = bootstrap.poll_seconds.unwrap_or(DEFAULT_POLL_SECONDS);
        let seconds = if failed {
            let exponent = self.consecutive_failures.saturating_sub(1).min(8);
            15_u64.saturating_mul(1_u64 << exponent).min(base)
        } else {
            base
        };
        let jitter_ceiling = (seconds / 10).max(1);
        let mut hasher = Sha256::new();
        hasher.update(bootstrap.source.as_bytes());
        hasher.update(std::process::id().to_le_bytes());
        hasher.update(self.last_attempt_ms.unwrap_or_default().to_le_bytes());
        let jitter_bytes: [u8; 8] = hasher.finalize()[..8].try_into().expect("eight bytes");
        let jitter = u64::from_le_bytes(jitter_bytes) % jitter_ceiling;
        self.next_attempt = Some(Instant::now() + Duration::from_secs(seconds + jitter));
    }

    fn write_status(&self) {
        let Some(path) = &self.paths.status else {
            return;
        };
        let passport = self.passport();
        let status = ManagedStatus {
            v: 1,
            configured: passport.configured,
            verified: passport.verified,
            freshness: passport.freshness,
            stale_reason: self.stale_reason.map(str::to_owned),
            sequence: passport.sequence,
            organization: passport.organization,
            contacts: (!passport.contacts.is_empty()).then_some(passport.contacts),
            source_class: passport.source_class,
            last_success_ms: passport.last_success_ms,
            last_attempt_ms: passport.last_attempt_ms,
            last_error: self.last_error.clone(),
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&status) {
            let _ = atomic_replace(path, &bytes);
        }
    }
}

enum Fresh {
    Modified {
        bytes: Vec<u8>,
        etag: Option<String>,
    },
    NotModified,
}

fn validate_bootstrap(bootstrap: &Bootstrap) -> Result<(), String> {
    if bootstrap.source.trim().is_empty() || bootstrap.source.len() > 2_048 {
        return Err("managed bootstrap source must be 1 to 2048 bytes".into());
    }
    if bootstrap.source.contains("://") && !is_https(&bootstrap.source) {
        return Err("managed bootstrap source must be a local path or HTTPS URL".into());
    }
    if is_https(&bootstrap.source) {
        let url = Url::parse(&bootstrap.source).map_err(|error| {
            format!("managed bootstrap source is not a valid HTTPS URL: {error}")
        })?;
        if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
            return Err("managed HTTPS source must have a host and no embedded credentials".into());
        }
    }
    if bootstrap
        .poll_seconds
        .is_some_and(|seconds| seconds == 0 || seconds > MAX_POLL_SECONDS)
    {
        return Err("managed poll_seconds must be between 1 and 86400".into());
    }
    if bootstrap
        .bearer_token
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.len() > 4_096)
    {
        return Err("managed bearer_token must be 1 to 4096 bytes".into());
    }
    if bootstrap
        .ca_cert_pem
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.len() > 131_072)
    {
        return Err("managed ca_cert_pem must be 1 to 131072 bytes".into());
    }
    key_from_bootstrap(bootstrap).map(|_| ())
}

fn key_from_bootstrap(bootstrap: &Bootstrap) -> Result<crypto::VerificationKey, String> {
    let ed25519 = decode_hex::<32>(&bootstrap.pubkey_ed25519)
        .ok_or_else(|| "managed pubkey_ed25519 must be exactly 32 bytes of hex".to_string())?;
    let mldsa = match bootstrap.pubkey_mldsa.as_deref() {
        Some(value) => Some(
            decode_hex::<{ crypto::MLDSA_PUBLIC_BYTES }>(value).ok_or_else(|| {
                "managed pubkey_mldsa has the wrong hex length or content".to_string()
            })?,
        ),
        None => None,
    };
    crypto::verification_key(&ed25519, mldsa.as_ref())
        .ok_or_else(|| "managed public key bytes are not valid".into())
}

fn decode_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(value.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(bytes)
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(value, "{byte:02x}").expect("writing to a string succeeds");
    }
    value
}

fn is_https(source: &str) -> bool {
    source.starts_with("https://")
}

fn local_source_path(bootstrap: &Path, source: &str) -> PathBuf {
    let path = PathBuf::from(source);
    if path.is_absolute() {
        path
    } else {
        bootstrap
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn unix_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::ErrorKind;
    use std::net::TcpListener;
    use std::path::Path;
    use std::time::Duration;

    use serde_json::{json, Value};

    use super::super::{
        Capability, GovernanceFacade, ManagedPolicyFreshness, ManagedPolicySource, ReasonCode,
    };
    use super::{bundle, crypto, encode_hex, ManagedAuthority, ManagedPaths};
    use crate::install::record::assert_owner_only;
    use crate::scratch::Scratch;

    /// The key the bootstrap names and every publication is signed with, unless a row says not.
    const SEED: [u8; 32] = [31; 32];
    /// A key no bootstrap names.
    const OTHER_SEED: [u8; 32] = [32; 32];
    /// The local bundle the bootstrap names, beside it.
    const SOURCE: &str = "org.bundle";
    /// A local source no file holds.
    const MISSING_SOURCE: &str = "missing.bundle";

    fn directory(name: &str) -> Scratch {
        Scratch::named(&format!("managed-{name}"))
    }

    fn manifest(name: &str, capability: &str) -> Value {
        json!({
            "schema": 4,
            "name": name,
            "version": "1",
            "grants": [{
                "id": "org",
                "hosts": {"allow": ["*"]},
                "allowed": [capability]
            }]
        })
    }

    /// A bootstrap naming `source` under [`SEED`]'s public key.
    fn bootstrap(source: &str) -> Value {
        json!({
            "source": source,
            "pubkey_ed25519": encode_hex(&crypto::signing::ed25519_public(&SEED))
        })
    }

    fn write_json(path: &Path, value: &Value) {
        fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }

    /// `manifest` signed under [`SEED`] at `sequence`.
    fn signed(sequence: u64, manifest: Value) -> Vec<u8> {
        bundle::sign(&SEED, None, sequence, manifest, None)
    }

    /// Replace a watched file and move its modified time past the last read, so the refresher
    /// sees the change whatever the new size and however coarse the clock.
    fn replace(path: &Path, content: impl AsRef<[u8]>) {
        let before = fs::metadata(path).and_then(|metadata| metadata.modified());
        fs::write(path, content).unwrap();
        if let Ok(before) = before {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(before + Duration::from_secs(2))
                .unwrap();
        }
    }

    fn presentation(name: &str, rationale: &str, contact: &str) -> bundle::Presentation {
        bundle::Presentation {
            org_name: Some(name.into()),
            rationale: Some(rationale.into()),
            contacts: vec![bundle::Contact {
                kind: "email".into(),
                value: contact.into(),
                label: None,
            }],
        }
    }

    /// ACCEPTANCE Governance 18, ADR-0028 and docs/trust/continuity.md: without an
    /// administrator's managed.json, the managed authority fetches nothing, writes nothing and
    /// leaves policy all-open.
    #[test]
    fn no_bootstrap_means_no_policy_network_work_and_nothing_written() {
        let root = directory("absent");
        let mut authority = ManagedAuthority::new(ManagedPaths::under(&root));

        authority.refresh();
        authority.refresh();

        assert!(!authority.configured());
        assert!(authority.valid());
        assert!(authority.manifest().is_none());
        let passport = authority.passport();
        assert_eq!(passport.freshness, ManagedPolicyFreshness::NotConfigured);
        assert_eq!(passport.source_class, ManagedPolicySource::None);
        assert_eq!(
            passport.last_attempt_ms, None,
            "an attempt is stamped before any fetch"
        );
        assert!(!authority.needs_refresh());
        assert_eq!(
            fs::read_dir(&root).unwrap().count(),
            0,
            "neither a cache nor a status file is written"
        );
    }

    /// docs/trust/security-overview.md (Signed managed policy, "a customer-owned file or HTTPS
    /// source"): a bootstrap whose source is a URL but not a plain HTTPS one is refused before any
    /// fetch, and the machine fails closed. Plain HTTP, other schemes and embedded credentials
    /// never reach the network.
    #[test]
    fn a_managed_source_is_a_local_path_or_an_https_url_and_nothing_else_is_fetched() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let plain_http = format!(
            "http://127.0.0.1:{}/policy",
            listener.local_addr().unwrap().port()
        );
        for source in [
            plain_http.as_str(),
            "ftp://policy.example/bundle",
            "file:///etc/ghostlight/org.bundle",
            "https://user:secret@policy.example/bundle",
        ] {
            let root = directory("scheme");
            let paths = ManagedPaths::under(&root);
            write_json(&paths.bootstrap, &bootstrap(source));
            let mut authority = ManagedAuthority::new(paths);

            authority.refresh();

            assert!(authority.configured(), "{source}");
            assert!(!authority.valid(), "{source}");
            let passport = authority.passport();
            assert!(!passport.verified, "{source}");
            assert_eq!(passport.last_attempt_ms, None, "{source} was fetched");
        }
        assert_eq!(
            listener.accept().err().map(|error| error.kind()),
            Some(ErrorKind::WouldBlock),
            "the plain HTTP source was connected to"
        );
    }

    /// ADR-0055 Decision 8 (the version-1 status sidecar administrators script against),
    /// docs/trust/data-flows.md and 97ddf868: the sidecar carries verification, sequence,
    /// freshness, source class, timing, organization and contacts, and never the source address,
    /// the bearer token, a verification key or a rule. The cache and the sidecar are readable only
    /// by the user.
    #[test]
    fn the_status_sidecar_and_cache_hold_no_source_credential_key_or_rule_and_only_the_user_reads_them(
    ) {
        const SOURCE_NAME: &str = "org-source-name.bundle";
        const BEARER: &str = "bearer-secret-value";
        const GRANT: &str = "org-grant";
        const RULES_KEY: &str = "grants";
        const SEQUENCE: u64 = 4;
        const ORGANIZATION: &str = "Example Org";
        const CONTACT: &str = "security@example.com";
        let root = directory("sidecar");
        let paths = ManagedPaths::under(&root);
        let mut policy = manifest("org", "read");
        policy[RULES_KEY][0]["id"] = json!(GRANT);
        fs::write(
            root.join(SOURCE_NAME),
            bundle::sign(
                &SEED,
                None,
                SEQUENCE,
                policy,
                Some(presentation(
                    ORGANIZATION,
                    "Keeps browser work inside approved sites.",
                    CONTACT,
                )),
            ),
        )
        .unwrap();
        let mut descriptor = bootstrap(SOURCE_NAME);
        descriptor["bearer_token"] = json!(BEARER);
        write_json(&paths.bootstrap, &descriptor);

        ManagedAuthority::new(paths.clone()).refresh();

        let cache = paths.cache.unwrap();
        let status_path = paths.status.unwrap();
        let text = fs::read_to_string(&status_path).unwrap();
        let status: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(status["v"], 1);
        assert_eq!(status["verified"], true);
        assert_eq!(status["freshness"], "fresh");
        assert_eq!(status["sequence"], SEQUENCE);
        assert_eq!(status["organization"], ORGANIZATION);
        assert_eq!(status["contacts"][0]["value"], CONTACT);
        assert_eq!(status["source_class"], "file");
        assert!(status["last_success_ms"].is_u64(), "{text}");
        let public_key = encode_hex(&crypto::signing::ed25519_public(&SEED));
        for withheld in [SOURCE_NAME, BEARER, public_key.as_str(), RULES_KEY, GRANT] {
            assert!(
                !text.contains(withheld),
                "the sidecar holds {withheld}: {text}"
            );
        }
        assert_owner_only(&cache);
        assert_owner_only(&status_path);
    }

    /// A facade enforcing one signed bundle, and the status file the managed authority wrote.
    fn published(
        name: &str,
        manifest: Value,
        presentation: Option<bundle::Presentation>,
    ) -> (Scratch, GovernanceFacade, String) {
        let root = directory(name);
        let paths = ManagedPaths::under(&root);
        fs::write(
            root.join(SOURCE),
            bundle::sign(&SEED, None, 3, manifest, presentation),
        )
        .unwrap();
        write_json(&paths.bootstrap, &bootstrap(SOURCE));
        let status = paths.status.clone().unwrap();
        let facade = GovernanceFacade::with_managed_paths(None, paths);
        let status = fs::read_to_string(status).unwrap();
        (root, facade, status)
    }

    /// An organization manifest that turns MCP agents off, so the switch sentence names it too.
    fn switched(organization: Option<Value>) -> Value {
        let mut manifest = manifest("org", "read");
        manifest["config"] = json!([{
            "key": crate::governance::manifest::PolicySetting::McpChannelEnabled.key(),
            "value": false,
            "level": "mandatory"
        }]);
        if let Some(organization) = organization {
            manifest["organization"] = organization;
        }
        manifest
    }

    /// One way a publication names its organization, and what every surface then shows.
    struct Naming {
        row: &'static str,
        /// The manifest's own `organization` block.
        block: Option<Value>,
        /// The signed presentation.
        presentation: Option<bundle::Presentation>,
        name: &'static str,
        statement: &'static str,
        url: Option<&'static str>,
        /// The one contact shown.
        contact: &'static str,
        /// What the status file must not hold.
        withheld: &'static [&'static str],
    }

    /// c2c958ff, ADR-0203 P4 and ADR-0122 Decision 3: a publication resolves its organization
    /// once, the signed presentation winning and the manifest's block filling what it lacks, and
    /// every surface names that one organization: the snapshot, the compiled view, the channel
    /// sentence, the chip, the passports and the status file.
    #[test]
    fn a_publication_names_one_organization_on_every_surface_the_signed_presentation_first() {
        const SIGNED_NAME: &str = "Example Org";
        const SIGNED_STATEMENT: &str = "The signed presentation's words.";
        const SIGNED_CONTACT: &str = "security@example.com";
        const BLOCK_NAME: &str = "Northwind";
        const BLOCK_STATEMENT: &str = "The manifest's own words.";
        const BLOCK_URL: &str = "https://northwind.example/policy";
        const BLOCK_CONTACT: &str = "it@northwind.example";
        let block = json!({
            "name": BLOCK_NAME,
            "statement": BLOCK_STATEMENT,
            "url": BLOCK_URL,
            "contacts": [{"kind": "email", "value": BLOCK_CONTACT}]
        });
        let signed = presentation(SIGNED_NAME, SIGNED_STATEMENT, SIGNED_CONTACT);
        let rows = [
            Naming {
                row: "presentation only",
                block: None,
                presentation: Some(signed.clone()),
                name: SIGNED_NAME,
                statement: SIGNED_STATEMENT,
                url: None,
                contact: SIGNED_CONTACT,
                withheld: &[],
            },
            Naming {
                row: "manifest block only",
                block: Some(block.clone()),
                presentation: None,
                name: BLOCK_NAME,
                statement: BLOCK_STATEMENT,
                url: Some(BLOCK_URL),
                contact: BLOCK_CONTACT,
                withheld: &[],
            },
            Naming {
                row: "both",
                block: Some(block),
                presentation: Some(signed),
                name: SIGNED_NAME,
                statement: SIGNED_STATEMENT,
                url: Some(BLOCK_URL),
                contact: SIGNED_CONTACT,
                withheld: &[BLOCK_NAME, BLOCK_CONTACT],
            },
        ];
        for Naming {
            row,
            block,
            presentation,
            name,
            statement,
            url,
            contact,
            withheld,
        } in rows
        {
            let (_root, facade, status_text) =
                published("organization", switched(block), presentation);
            let snapshot = facade.snapshot();
            let authority = facade.effective_authority();

            let organization = snapshot.organization().expect(row);
            assert_eq!(organization.name.as_deref(), Some(name), "{row}");
            assert_eq!(organization.statement.as_deref(), Some(statement), "{row}");
            assert_eq!(organization.url.as_deref(), url, "{row}");
            let contacts: Vec<_> = organization
                .contacts
                .iter()
                .map(|contact| contact.value.as_str())
                .collect();
            assert_eq!(contacts, [contact], "{row}");
            assert_eq!(authority.organization.as_ref(), Some(organization), "{row}");
            assert_eq!(snapshot.channels_off().organization, Some(name), "{row}");
            assert_eq!(facade.policy_chip().detail, authority.headline, "{row}");
            assert!(
                authority.headline.contains(name),
                "{row}: {}",
                authority.headline
            );
            assert_eq!(authority.layers[0].title, name, "{row}");
            for passport in [&authority.passport, &facade.diagnostics().managed_policy] {
                assert_eq!(passport.organization.as_deref(), Some(name), "{row}");
                assert_eq!(passport.rationale.as_deref(), Some(statement), "{row}");
                assert_eq!(passport.contacts[0].value, contact, "{row}");
            }
            let status: Value = serde_json::from_str(&status_text).unwrap();
            assert_eq!(status["organization"], name, "{row}");
            assert_eq!(status["contacts"][0]["value"], contact, "{row}");
            for withheld in withheld {
                assert!(!status_text.contains(withheld), "{row}: {status_text}");
            }
        }

        // G-D3: both delivery paths bound a name in bytes, so a name one refuses the other refuses.
        let bound = crate::governance::manifest::organization_bounds::NAME_BYTES;
        let wide = "\u{e9}".repeat(bound / 2 + 10);
        for (name, fits) in [("a".repeat(bound), true), (wide, false)] {
            let manifest = json!({
                "schema": 4, "name": "org", "version": "1", "grants": [],
                "organization": {"name": name}
            });
            let block = crate::governance::manifest::parse(&manifest.to_string(), "test").is_ok();
            let signed = bundle::validate_presentation(Some(&presentation(
                &name,
                SIGNED_STATEMENT,
                SIGNED_CONTACT,
            )))
            .is_ok();
            assert_eq!((block, signed), (fits, fits), "{} bytes", name.len());
        }
    }

    /// One change to a published organization's files: its paths and its local source.
    type Change = &'static dyn Fn(&ManagedPaths, &Path);

    /// ACCEPTANCE Governance 6 and 19, ADR-0121 Decision 2, docs/trust/continuity.md and
    /// security-overview.md: once a policy is in force, a replacement that does not validate never
    /// displaces it. An edit to the user file that does not parse keeps the last valid user policy
    /// enforced. A malformed, wrongly signed, older, sequence-reusing or unreachable organization
    /// update keeps the verified publication, shown as last-known-good, and a removed bootstrap
    /// keeps the machine managed (6c138338). The next valid replacement still takes effect.
    #[test]
    fn once_a_policy_is_in_force_only_a_valid_replacement_takes_its_place() {
        const CURRENT: u64 = 8;
        const NEWER: u64 = 9;
        const OLDER: u64 = 3;
        let rows: [(&str, Change); 6] = [
            ("malformed", &|_, source| replace(source, b"half written")),
            ("another key", &|_, source| {
                replace(
                    source,
                    bundle::sign(&OTHER_SEED, None, NEWER, manifest("newer", "action"), None),
                );
            }),
            ("rollback", &|_, source| {
                replace(source, signed(OLDER, manifest("older", "action")));
            }),
            ("sequence reused", &|_, source| {
                replace(source, signed(CURRENT, manifest("other", "action")));
            }),
            ("unreachable", &|_, source| fs::remove_file(source).unwrap()),
            ("bootstrap removed", &|paths, _| {
                fs::remove_file(&paths.bootstrap).unwrap();
            }),
        ];
        for (row, change) in rows {
            let root = directory("last-known-good");
            let paths = ManagedPaths::under(&root);
            let source = root.join(SOURCE);
            fs::write(&source, signed(CURRENT, manifest("current", "read"))).unwrap();
            write_json(&paths.bootstrap, &bootstrap(SOURCE));
            let facade = GovernanceFacade::with_managed_paths(None, paths.clone());
            assert_eq!(facade.snapshot().managed_sequence, Some(CURRENT), "{row}");

            change(&paths, &source);
            facade.refresh_tick();

            let kept = facade.snapshot();
            assert_eq!(kept.managed_sequence, Some(CURRENT), "{row}");
            assert!(kept.authorize_capability(Capability::Read).allowed, "{row}");
            assert!(
                !kept.authorize_capability(Capability::Action).allowed,
                "{row}"
            );
            let diagnostics = facade.diagnostics();
            assert!(diagnostics.managed_authority_configured, "{row}");
            assert!(diagnostics.authority_valid, "{row}");
            assert!(diagnostics.managed_policy.verified, "{row}");
            assert_eq!(
                diagnostics.managed_policy.freshness,
                ManagedPolicyFreshness::LastKnownGood,
                "{row}"
            );

            if !paths.bootstrap.exists() {
                write_json(&paths.bootstrap, &bootstrap(SOURCE));
            }
            replace(&source, signed(NEWER, manifest("newer", "action")));
            facade.refresh_tick();

            let replaced = facade.snapshot();
            assert_eq!(replaced.managed_sequence, Some(NEWER), "{row}");
            assert!(
                replaced.authorize_capability(Capability::Action).allowed,
                "{row}"
            );
            assert_eq!(
                facade.diagnostics().managed_policy.freshness,
                ManagedPolicyFreshness::Fresh,
                "{row}"
            );
        }

        let user = Scratch::file("user-last-known-good.json");
        fs::write(&user, manifest("read", "read").to_string()).unwrap();
        let unmanaged = directory("unmanaged");
        let facade = GovernanceFacade::with_managed_paths(
            Some(user.to_path_buf()),
            ManagedPaths::under(&unmanaged),
        );
        replace(&user, "{half-written");
        facade.refresh_tick();
        let kept = facade.snapshot();
        assert!(kept.authorize_capability(Capability::Read).allowed);
        assert!(!kept.authorize_capability(Capability::Action).allowed);
        let diagnostics = facade.diagnostics();
        assert!(diagnostics.local_policy_active);
        assert!(!diagnostics.local_policy_valid);
        assert!(
            diagnostics.authority_valid,
            "the kept user policy is enforced, so authority is not failing closed"
        );

        replace(&user, manifest("action", "action").to_string());
        facade.refresh_tick();
        let replaced = facade.snapshot();
        assert!(!replaced.authorize_capability(Capability::Read).allowed);
        assert!(replaced.authorize_capability(Capability::Action).allowed);
        let diagnostics = facade.diagnostics();
        assert!(diagnostics.local_policy_valid);
        assert!(diagnostics.authority_valid);
    }

    /// One cold start under an administrator's bootstrap.
    struct ColdStart {
        row: &'static str,
        bootstrap: Value,
        /// What the local [`SOURCE`] holds, if anything.
        source: Option<Vec<u8>>,
        /// What the verified cache holds, if anything.
        cache: Option<Vec<u8>>,
        /// The sequence enforced from the cache, or `None` when the start fails closed.
        enforced: Option<u64>,
    }

    /// ACCEPTANCE Governance 6, 19 and 22, ADR-0121 Decision 2, ADR-0197, ADR-0055 Decision 5
    /// (the cache is verified on read) and docs/trust/continuity.md: a configured service starts
    /// from what is on the machine, without touching the network. An organization's verified cache
    /// is enforced before the first poll and through a source outage. A user policy file, or an
    /// organization source, that holds no valid policy and has no verified cache refuses every
    /// read for invalid authority, reported exactly as it is enforced, and the passport says
    /// nothing is verified.
    #[test]
    fn a_cold_start_enforces_only_a_valid_source_or_verified_cache_and_otherwise_fails_closed() {
        const CACHED: u64 = 9;
        const HTTPS_SOURCE: &str = "https://policy.invalid/bundle";
        let cache = signed(CACHED, manifest("cached", "read"));
        let mut misspelled = bootstrap(SOURCE);
        misspelled["pubkey_mldsa65"] = misspelled["pubkey_ed25519"].clone();
        let rows = [
            ColdStart {
                row: "an https source with a verified cache",
                bootstrap: bootstrap(HTTPS_SOURCE),
                source: None,
                cache: Some(cache.clone()),
                enforced: Some(CACHED),
            },
            ColdStart {
                row: "an unreachable source with a verified cache",
                bootstrap: bootstrap(MISSING_SOURCE),
                source: None,
                cache: Some(cache),
                enforced: Some(CACHED),
            },
            ColdStart {
                row: "an unreachable source and no cache",
                bootstrap: bootstrap(MISSING_SOURCE),
                source: None,
                cache: None,
                enforced: None,
            },
            ColdStart {
                row: "an unsigned manifest as the source",
                bootstrap: bootstrap(SOURCE),
                source: Some(serde_json::to_vec(&manifest("org", "read")).unwrap()),
                cache: None,
                enforced: None,
            },
            ColdStart {
                row: "a cache signed by another key",
                bootstrap: bootstrap(MISSING_SOURCE),
                source: None,
                cache: Some(bundle::sign(
                    &OTHER_SEED,
                    None,
                    CACHED,
                    manifest("cached", "read"),
                    None,
                )),
                enforced: None,
            },
            ColdStart {
                row: "a signed schema-3 manifest",
                bootstrap: bootstrap(SOURCE),
                source: Some(signed(
                    1,
                    json!({"schema": 3, "name": "theirs", "version": "1", "grants": []}),
                )),
                cache: None,
                enforced: None,
            },
            ColdStart {
                row: "a misspelled key name beside a valid source",
                bootstrap: misspelled,
                source: Some(signed(1, manifest("org", "read"))),
                cache: None,
                enforced: None,
            },
        ];
        for ColdStart {
            row,
            bootstrap,
            source,
            cache,
            enforced,
        } in rows
        {
            let root = directory("cold-start");
            let paths = ManagedPaths::under(&root);
            write_json(&paths.bootstrap, &bootstrap);
            if let Some(source) = &source {
                fs::write(root.join(SOURCE), source).unwrap();
            }
            if let Some(cache) = &cache {
                fs::write(paths.cache.as_ref().unwrap(), cache).unwrap();
            }

            let facade = GovernanceFacade::with_managed_paths(None, paths.clone());

            let snapshot = facade.snapshot();
            let diagnostics = facade.diagnostics();
            let passports = [
                diagnostics.managed_policy.clone(),
                facade.effective_authority().passport,
            ];
            assert!(diagnostics.managed_authority_configured, "{row}");
            match enforced {
                Some(sequence) => {
                    assert_eq!(snapshot.managed_sequence, Some(sequence), "{row}");
                    assert!(
                        snapshot.authorize_capability(Capability::Read).allowed,
                        "{row}"
                    );
                    assert!(diagnostics.authority_valid, "{row}");
                    for passport in &passports {
                        assert!(passport.verified, "{row}");
                        assert_eq!(passport.sequence, Some(sequence), "{row}");
                        assert_eq!(
                            passport.freshness,
                            ManagedPolicyFreshness::LastKnownGood,
                            "{row}"
                        );
                    }
                }
                None => {
                    assert_eq!(
                        snapshot.authorize_capability(Capability::Read).reason,
                        ReasonCode::InvalidAuthority,
                        "{row}"
                    );
                    assert!(!diagnostics.authority_valid, "{row}");
                    assert!(!diagnostics.managed_authority_active, "{row}");
                    for passport in &passports {
                        assert!(passport.configured, "{row}");
                        assert!(!passport.verified, "{row}");
                        assert_eq!(
                            passport.freshness,
                            ManagedPolicyFreshness::NoPolicy,
                            "{row}"
                        );
                        assert_eq!(passport.sequence, None, "{row}");
                    }
                }
            }
            if bootstrap["source"] == HTTPS_SOURCE {
                for passport in &passports {
                    assert_eq!(passport.last_attempt_ms, None, "{row}: a start fetched");
                }
                let mut authority = ManagedAuthority::new(paths);
                authority.load_offline();
                assert!(
                    authority.needs_refresh(),
                    "{row}: the first poll is owed to the refresher"
                );
            }
            if let Some(source) = source {
                assert_eq!(
                    fs::read(root.join(SOURCE)).unwrap(),
                    source,
                    "{row}: an administrator's file is never rewritten"
                );
            }
        }

        let user = Scratch::file("never-valid.json");
        fs::write(&user, "{half-written").unwrap();
        let unmanaged = directory("unmanaged");
        let facade = GovernanceFacade::with_managed_paths(
            Some(user.to_path_buf()),
            ManagedPaths::under(&unmanaged),
        );
        assert_eq!(
            facade
                .snapshot()
                .authorize_capability(Capability::Read)
                .reason,
            ReasonCode::InvalidAuthority
        );
        let diagnostics = facade.diagnostics();
        assert!(!diagnostics.authority_valid);
        assert!(diagnostics.local_policy_configured);
        assert!(!diagnostics.local_policy_active);
    }
}
