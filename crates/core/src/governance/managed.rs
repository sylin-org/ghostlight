// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! managed:// central policy distribution (ADR-0055): the org-authoritative bootstrap and the
//! load-and-verify path for an org-signed policy bundle.
//!
//! Trust model (ADR-0055 D1 / Implementation Decision 1): the org signs its policy bundle with its
//! OWN composite keypair and provisions the endpoint with the PUBLIC key. Because that key is the
//! trust anchor, it must be org-authoritative -- so it lives in the admin-only [`bootstrap_path`] (a
//! `managed.json` sibling of the org policy file), NOT in any user-writable config layer. A user
//! cannot self-activate managed governance through `--manifest` / `GHOSTLIGHT_MANIFEST` (that path
//! rejects `managed://`, see [`super::manifest::source::parse_source_string`]); only the admin
//! bootstrap activates it. When active, the fetched signed policy is org-authoritative
//! ([`super::manifest::source::ManifestOrigin::Managed`], wired in ADR-0055 Phase 4).
//!
//! Transport-agnostic (ADR-0055 D7): [`verify_and_parse`] verifies a bundle's bytes regardless of
//! origin, so a local file, a USB stick, and a network fetch share ONE trust model. Phase 1c loads
//! from a local path (the air-gap / sneakernet path); Phase 3 adds the network fetch; Phase 2 adds
//! the last-known-good cache.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::governance::crypto::{self, GenKey};
use crate::governance::manifest::bundle::{self, Presentation};
use crate::governance::manifest::document::{parse_manifest, Manifest, ManifestError};

/// The admin-provisioned managed:// bootstrap (a `managed.json` sibling of the org policy file). It
/// carries the org's public verifying key(s) and the policy source. Org-authoritative: MDM drops it
/// in the admin location exactly as it drops `policy.json`. Deliberately `deny_unknown_fields`: a
/// typo in a governance trust anchor is a hard error, never silently ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedBootstrap {
    /// Where the signed policy bundle comes from: a local filesystem path (Phase 1c) or an
    /// `https://` URL (Phase 3).
    pub source: String,
    /// The org's Ed25519 verifying key as lowercase hex (64 chars = 32 bytes).
    pub pubkey_ed25519: String,
    /// The org's ML-DSA-65 verifying key as hex (optional: absent = an evaluation-grade Ed25519-only
    /// key; present = a production composite key, which then requires both signature legs).
    #[serde(default)]
    pub pubkey_mldsa: Option<String>,
}

/// Why a managed bootstrap or bundle could not be loaded or verified. Owned by this module (rather
/// than reusing the manifest loader's `LoadError`) so `governance::managed` has no dependency back
/// into `manifest::source`; the Phase 4 composition maps this into the loader's flow.
#[derive(Debug, thiserror::Error)]
pub enum ManagedError {
    #[error("managed bootstrap {path}: {reason}")]
    Bootstrap { path: String, reason: String },
    #[error("managed org public key: {0}")]
    Key(String),
    #[error("managed policy bundle: {0}")]
    Bundle(String),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("network policy fetch is not yet supported (ADR-0055 Phase 3); managed source must be a local path")]
    NetworkNotYet,
}

/// A verified managed policy: the parsed manifest, its monotonic publish sequence (ADR-0055 D6
/// anti-rollback), and the org-authored presentation (ADR-0055 D9).
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedManaged {
    pub manifest: Manifest,
    pub seq: u64,
    pub presentation: Option<Presentation>,
}

/// The admin-location bootstrap path: `managed.json` beside the org policy file.
pub fn bootstrap_path() -> PathBuf {
    let org = crate::governance::config::load::org_policy_path();
    match org.parent() {
        Some(dir) => dir.join("managed.json"),
        None => org.with_file_name("managed.json"),
    }
}

/// Read and parse the bootstrap at `path`, if present. `Ok(None)` = absent (managed:// not
/// configured). `Err` = present but unreadable or invalid, always fatal -- matching the
/// org-policy-file fail-closed discipline (a broken governance trust anchor is worse than a crash).
pub fn load_bootstrap_at(path: &Path) -> Result<Option<ManagedBootstrap>, ManagedError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let b: ManagedBootstrap =
                serde_json::from_slice(&bytes).map_err(|e| ManagedError::Bootstrap {
                    path: path.display().to_string(),
                    reason: e.to_string(),
                })?;
            Ok(Some(b))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ManagedError::Io {
            path: path.display().to_string(),
            source: e,
        }),
    }
}

/// Resolve the org verifying key from a bootstrap's hex public-key fields.
pub fn org_key(b: &ManagedBootstrap) -> Result<GenKey, ManagedError> {
    let ed = hex_array::<32>(&b.pubkey_ed25519)
        .ok_or_else(|| ManagedError::Key("pubkey_ed25519 is not 32 bytes of hex".into()))?;
    let mldsa = match &b.pubkey_mldsa {
        Some(h) => Some(hex_array::<{ crypto::MLDSA_PK_LEN }>(h).ok_or_else(|| {
            ManagedError::Key("pubkey_mldsa is not the right length in hex".into())
        })?),
        None => None,
    };
    bundle::org_key(&ed, mldsa.as_ref())
        .ok_or_else(|| ManagedError::Key("org public key bytes are not a valid key".into()))
}

/// Verify a policy bundle's bytes (raw envelope or armored) against `key` and parse the manifest
/// inside. Transport-agnostic: the caller supplies the bytes from any source.
pub fn verify_and_parse(
    bundle_bytes: &[u8],
    key: &GenKey,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<VerifiedManaged, ManagedError> {
    let text = String::from_utf8_lossy(bundle_bytes);
    let envelope = if bundle::is_armored(&text) {
        bundle::dearmor(&text)
            .ok_or_else(|| ManagedError::Bundle("the armored policy block is malformed".into()))?
    } else {
        bundle_bytes.to_vec()
    };
    let verified =
        bundle::verify_bundle(&envelope, key).map_err(|e| ManagedError::Bundle(e.to_string()))?;
    let manifest = parse_manifest(&verified.manifest_json, "managed://policy", domain_pattern_valid)?;
    Ok(VerifiedManaged {
        manifest,
        seq: verified.seq,
        presentation: verified.presentation,
    })
}

/// Load and verify a signed policy bundle from a local filesystem path (the ADR-0055 D7 air-gap /
/// sneakernet path). Accepts a raw envelope or an armored block.
pub fn load_from_local_path(
    path: &Path,
    key: &GenKey,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<VerifiedManaged, ManagedError> {
    let bytes = std::fs::read(path).map_err(|e| ManagedError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    verify_and_parse(&bytes, key, domain_pattern_valid)
}

/// Load the managed policy named by a bootstrap: from a local path today, or (Phase 3) an
/// `https://` source. The org key is derived from the same bootstrap.
pub fn load_bundle(
    b: &ManagedBootstrap,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<VerifiedManaged, ManagedError> {
    let key = org_key(b)?;
    if b.source.starts_with("http://") || b.source.starts_with("https://") {
        return Err(ManagedError::NetworkNotYet);
    }
    load_from_local_path(Path::new(&b.source), &key, domain_pattern_valid)
}

/// Decode a lowercase/uppercase hex string into a fixed-size byte array, or `None` on any malformed
/// input or length mismatch.
fn hex_array<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != 2 * N {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_pattern(_: &str) -> bool {
        true
    }

    fn hex_encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    fn manifest_value(name: &str) -> serde_json::Value {
        serde_json::json!({ "schema": 3, "name": name, "version": "1", "grants": [] })
    }

    #[test]
    fn hex_array_round_trips_and_rejects_bad_input() {
        let bytes = [0xde, 0xad, 0xbe, 0xef];
        assert_eq!(hex_array::<4>(&hex_encode(&bytes)), Some(bytes));
        assert_eq!(hex_array::<4>("deadbe"), None, "wrong length");
        assert_eq!(hex_array::<4>("deadbeeg"), None, "non-hex digit");
    }

    #[test]
    fn bootstrap_parses_and_rejects_unknown_fields() {
        let json = r#"{"source":"/etc/ghostlight/policy.bundle","pubkey_ed25519":"ab"}"#;
        let b: ManagedBootstrap = serde_json::from_str(json).unwrap();
        assert_eq!(b.source, "/etc/ghostlight/policy.bundle");
        assert!(b.pubkey_mldsa.is_none());
        let bad = r#"{"source":"x","pubkey_ed25519":"ab","surprise":1}"#;
        assert!(serde_json::from_str::<ManagedBootstrap>(bad).is_err());
    }

    #[test]
    fn org_key_resolves_ed_only_and_composite() {
        let ed_seed = [31u8; 32];
        let mldsa_seed = [32u8; 32];
        let ed_only = ManagedBootstrap {
            source: "x".into(),
            pubkey_ed25519: hex_encode(&crypto::admin::ed_public(&ed_seed)),
            pubkey_mldsa: None,
        };
        assert!(matches!(org_key(&ed_only), Ok(GenKey::Ed25519(_))));
        let composite = ManagedBootstrap {
            source: "x".into(),
            pubkey_ed25519: hex_encode(&crypto::admin::ed_public(&ed_seed)),
            pubkey_mldsa: Some(hex_encode(&crypto::admin::mldsa_public(&mldsa_seed))),
        };
        assert!(matches!(org_key(&composite), Ok(GenKey::Composite { .. })));
        let bad = ManagedBootstrap {
            source: "x".into(),
            pubkey_ed25519: "notlongenough".into(),
            pubkey_mldsa: None,
        };
        assert!(matches!(org_key(&bad), Err(ManagedError::Key(_))));
    }

    #[test]
    fn verify_and_parse_round_trips_a_signed_bundle() {
        let ed_seed = [41u8; 32];
        let bytes = bundle::sign_bundle(&ed_seed, None, 9, manifest_value("acme"), None);
        let key = bundle::org_key(&crypto::admin::ed_public(&ed_seed), None).unwrap();
        let v = verify_and_parse(&bytes, &key, ok_pattern).expect("verifies");
        assert_eq!(v.manifest.name, "acme");
        assert_eq!(v.seq, 9);
    }

    #[test]
    fn verify_and_parse_accepts_the_armored_form() {
        let ed_seed = [42u8; 32];
        let bytes = bundle::sign_bundle(&ed_seed, None, 1, manifest_value("acme"), None);
        let block = bundle::armor(&bytes);
        let key = bundle::org_key(&crypto::admin::ed_public(&ed_seed), None).unwrap();
        assert!(verify_and_parse(block.as_bytes(), &key, ok_pattern).is_ok());
    }

    #[test]
    fn verify_and_parse_rejects_a_wrong_key() {
        let bytes = bundle::sign_bundle(&[1u8; 32], None, 1, manifest_value("acme"), None);
        let other = bundle::org_key(&crypto::admin::ed_public(&[2u8; 32]), None).unwrap();
        assert!(matches!(
            verify_and_parse(&bytes, &other, ok_pattern),
            Err(ManagedError::Bundle(_))
        ));
    }

    #[test]
    fn verify_and_parse_rejects_an_invalid_inner_manifest() {
        // A validly-signed bundle whose manifest is schema-2 must fail at manifest validation.
        let ed_seed = [43u8; 32];
        let bad_manifest = serde_json::json!({ "schema": 2, "name": "x", "version": "1", "grants": [] });
        let bytes = bundle::sign_bundle(&ed_seed, None, 1, bad_manifest, None);
        let key = bundle::org_key(&crypto::admin::ed_public(&ed_seed), None).unwrap();
        assert!(matches!(
            verify_and_parse(&bytes, &key, ok_pattern),
            Err(ManagedError::Manifest(_))
        ));
    }

    #[test]
    fn load_from_local_path_reads_verifies_and_parses() {
        let ed_seed = [44u8; 32];
        let bytes = bundle::sign_bundle(&ed_seed, None, 5, manifest_value("acme-local"), None);
        let path = std::env::temp_dir().join(format!(
            "ghostlight-managed-test-{}-bundle.json",
            std::process::id()
        ));
        std::fs::write(&path, &bytes).unwrap();
        let key = bundle::org_key(&crypto::admin::ed_public(&ed_seed), None).unwrap();
        let result = load_from_local_path(&path, &key, ok_pattern);
        std::fs::remove_file(&path).ok();
        let v = result.expect("verifies");
        assert_eq!(v.manifest.name, "acme-local");
        assert_eq!(v.seq, 5);
    }

    #[test]
    fn load_bootstrap_at_absent_is_ok_none() {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-managed-test-{}-absent.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        assert!(matches!(load_bootstrap_at(&path), Ok(None)));
    }

    #[test]
    fn load_bundle_rejects_a_network_source_for_now() {
        let b = ManagedBootstrap {
            source: "https://policy.acme.example/ghostlight.bundle".into(),
            pubkey_ed25519: hex_encode(&crypto::admin::ed_public(&[7u8; 32])),
            pubkey_mldsa: None,
        };
        assert!(matches!(
            load_bundle(&b, ok_pattern),
            Err(ManagedError::NetworkNotYet)
        ));
    }
}
