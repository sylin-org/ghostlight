//! Opt-in organization-managed policy from signed local or HTTPS bundles.

mod bundle;
mod crypto;
mod http;

pub mod cli;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use url::Url;

use super::{
    manifest, ManagedPolicyContact, ManagedPolicyFreshness, ManagedPolicyPassport,
    ManagedPolicySource,
};

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

        #[cfg(target_os = "windows")]
        let state = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Ghostlight"));
        #[cfg(target_os = "linux")]
        let state = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("state"))
            })
            .map(|path| path.join("ghostlight"));

        Self {
            bootstrap,
            cache: state
                .as_ref()
                .map(|path| path.join("managed-policy.bundle")),
            status: state.map(|path| path.join("managed-status.json")),
        }
    }

    #[cfg(test)]
    pub(crate) fn under(path: &Path) -> Self {
        Self {
            bootstrap: path.join("managed.json"),
            cache: Some(path.join("managed-policy.bundle")),
            status: Some(path.join("managed-status.json")),
        }
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
    presentation: Option<bundle::Presentation>,
    envelope_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalFingerprint {
    length: u64,
    modified: Option<SystemTime>,
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
    active: Option<ActivePolicy>,
    etag: Option<String>,
    source_fingerprint: Option<LocalFingerprint>,
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

    pub(crate) fn refresh(&mut self) {
        let bytes = match fs::read(&self.paths.bootstrap) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if self.configured {
                    *self = Self::new(self.paths.clone());
                    self.write_status();
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
            if self
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

    pub(crate) fn passport(&self) -> ManagedPolicyPassport {
        let presentation = self
            .active
            .as_ref()
            .and_then(|active| active.presentation.as_ref());
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
            organization: presentation.and_then(|value| value.org_name.clone()),
            rationale: presentation.and_then(|value| value.rationale.clone()),
            contacts: presentation
                .map(|value| {
                    value
                        .contacts
                        .iter()
                        .map(|contact| ManagedPolicyContact {
                            kind: contact.kind.clone(),
                            value: contact.value.clone(),
                            label: contact.label.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
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
        self.active = Some(ActivePolicy {
            manifest,
            sequence: verified.sequence,
            presentation: verified.presentation,
            envelope_hash,
        });
        if let Some(path) = &self.paths.cache {
            if let Err(error) = atomic_write(path, bytes) {
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
        Some(ActivePolicy {
            manifest: manifest::parse(&text, "verified managed policy cache").ok()?,
            sequence: verified.sequence,
            presentation: verified.presentation,
            envelope_hash: digest(&bytes),
        })
    }

    fn local_source_changed(&mut self, source: &str) -> bool {
        let path = local_source_path(&self.paths.bootstrap, source);
        let current = fs::metadata(path).ok().map(|metadata| LocalFingerprint {
            length: metadata.len(),
            modified: metadata.modified().ok(),
        });
        if self.source_fingerprint == current && self.active.is_some() {
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
            let _ = atomic_write(path, &bytes);
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

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)
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

    use super::{bundle, crypto, encode_hex, Bootstrap, ManagedAuthority, ManagedPaths};

    fn directory(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-managed-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn manifest(name: &str, capability: &str) -> serde_json::Value {
        serde_json::json!({
            "schema": 3,
            "name": name,
            "version": "1",
            "grants": [{
                "id": "org",
                "hosts": {"allow": ["*"]},
                "allowed": [capability]
            }]
        })
    }

    fn write_bootstrap(path: &std::path::Path, source: &str, seed: &[u8; 32]) {
        let bootstrap = serde_json::json!({
            "source": source,
            "pubkey_ed25519": encode_hex(&crypto::signing::ed25519_public(seed))
        });
        fs::write(path, serde_json::to_vec_pretty(&bootstrap).unwrap()).unwrap();
    }

    #[test]
    fn no_bootstrap_has_no_network_or_policy_effect() {
        let root = directory("absent");
        let mut authority = ManagedAuthority::new(ManagedPaths::under(&root));
        authority.refresh();
        assert!(!authority.configured());
        assert!(authority.valid());
        assert!(authority.manifest().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_signed_local_bundle_activates_and_populates_verified_cache() {
        let root = directory("local");
        let paths = ManagedPaths::under(&root);
        let seed = [31_u8; 32];
        fs::write(
            root.join("org.bundle"),
            bundle::sign(
                &seed,
                None,
                4,
                manifest("org", "read"),
                Some(bundle::Presentation {
                    org_name: Some("Example Org".into()),
                    rationale: Some("Keeps browser work inside approved sites.".into()),
                    contacts: vec![bundle::Contact {
                        kind: "email".into(),
                        value: "security@example.com".into(),
                        label: Some("Security team".into()),
                    }],
                }),
            ),
        )
        .unwrap();
        write_bootstrap(&paths.bootstrap, "org.bundle", &seed);
        let mut authority = ManagedAuthority::new(paths.clone());

        authority.refresh();

        assert!(authority.configured());
        assert!(authority.valid());
        assert_eq!(authority.sequence(), Some(4));
        assert_eq!(authority.manifest().unwrap().name, "org");
        let passport = authority.passport();
        assert!(passport.verified);
        assert_eq!(passport.organization.as_deref(), Some("Example Org"));
        assert_eq!(
            passport.rationale.as_deref(),
            Some("Keeps browser work inside approved sites.")
        );
        assert_eq!(passport.contacts[0].value, "security@example.com");
        assert!(paths.cache.unwrap().exists());
        let status_path = paths.status.unwrap();
        assert!(status_path.exists());
        let status = fs::read_to_string(status_path).unwrap();
        assert!(status.contains("\"sequence\": 4"));
        assert!(status.contains("Example Org"));
        assert!(status.contains("security@example.com"));
        assert!(!status.contains("pubkey"));
        assert!(!status.contains("grants"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_governance_facade_enforces_the_signed_managed_manifest_and_stamps_sequence() {
        let root = directory("facade");
        let paths = ManagedPaths::under(&root);
        let seed = [34_u8; 32];
        fs::write(
            root.join("org.bundle"),
            bundle::sign(&seed, None, 12, manifest("read-only", "read"), None),
        )
        .unwrap();
        write_bootstrap(&paths.bootstrap, "org.bundle", &seed);
        let facade = super::super::GovernanceFacade::with_managed_paths(paths);

        let snapshot = facade.snapshot(&crate::language::RequestRestrictions::default());

        assert!(
            snapshot
                .authorize_capability(super::super::Capability::Read)
                .allowed
        );
        assert!(
            !snapshot
                .authorize_capability(super::super::Capability::Action)
                .allowed
        );
        assert_eq!(snapshot.managed_sequence, Some(12));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bad_updates_and_rollbacks_keep_the_verified_last_known_good() {
        let root = directory("lkg");
        let paths = ManagedPaths::under(&root);
        let seed = [32_u8; 32];
        let source = root.join("org.bundle");
        fs::write(
            &source,
            bundle::sign(&seed, None, 8, manifest("current", "read"), None),
        )
        .unwrap();
        write_bootstrap(&paths.bootstrap, "org.bundle", &seed);
        let mut authority = ManagedAuthority::new(paths);
        authority.refresh();

        fs::write(&source, b"half written").unwrap();
        authority.source_fingerprint = None;
        authority.refresh();
        assert_eq!(authority.sequence(), Some(8));
        assert_eq!(authority.manifest().unwrap().name, "current");

        fs::write(
            &source,
            bundle::sign(&seed, None, 3, manifest("older", "action"), None),
        )
        .unwrap();
        authority.source_fingerprint = None;
        authority.refresh();
        assert_eq!(authority.sequence(), Some(8));
        assert_eq!(authority.manifest().unwrap().name, "current");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_cold_invalid_source_fails_closed_but_a_verified_cache_recovers_offline() {
        let root = directory("cold");
        let paths = ManagedPaths::under(&root);
        let seed = [33_u8; 32];
        write_bootstrap(&paths.bootstrap, "missing.bundle", &seed);
        let mut cold = ManagedAuthority::new(paths.clone());
        cold.refresh();
        assert!(cold.configured());
        assert!(!cold.valid());

        fs::write(
            paths.cache.as_ref().unwrap(),
            bundle::sign(&seed, None, 9, manifest("cached", "read"), None),
        )
        .unwrap();
        let mut recovered = ManagedAuthority::new(paths);
        recovered.refresh();
        assert!(recovered.valid());
        assert_eq!(recovered.sequence(), Some(9));
        assert_eq!(recovered.manifest().unwrap().name, "cached");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bootstrap_is_strict_and_plain_http_is_not_a_production_source() {
        assert!(serde_json::from_str::<Bootstrap>(
            r#"{"source":"x","pubkey_ed25519":"00","typo":true}"#
        )
        .is_err());
        let bootstrap = Bootstrap {
            source: "http://policy.example/bundle".into(),
            pubkey_ed25519: "00".repeat(32),
            ..Bootstrap::default()
        };
        assert!(super::validate_bootstrap(&bootstrap).is_err());
    }
}
