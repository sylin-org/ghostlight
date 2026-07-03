//! Manifest identity (ADR-0020 commitment 5; shared format doc sections 4.1, 4.2). Every
//! policy manifest carries a required `name`, a required `version`, and a SHA-256 content
//! hash computed over its canonical bytes, so every logged decision is attributable to the
//! exact policy version that made it. The hash is attribution, NOT authentication: manifest
//! signing is excluded (SPEC section 10); file ACLs plus the deployment channel are the
//! usage-surface guard (shared format doc section 1.2).
//!
//! Standalone today (the manifest engine, task G12, has not landed): this module reads the
//! org policy file directly and computes its identity. When G12 lands, it computes identity
//! from the exact source bytes it already parsed (never a second read) and this module's
//! standalone reader retires in favor of the engine's loader; [`canonical_hash`] and
//! [`identity_from_source`] stay as the shared primitives.
//!
//! `std`-only plus `serde`/`serde_json`/`sha2`/`thiserror`/`tracing`. No async, no tokio.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Identity of the active policy manifest (shared format doc 4.1, 4.2). Serializes as the
/// audit record's `manifest` object (shared format doc 6.1) and the `get_status`
/// `governance.manifest` object (shared format doc 9.2): keys `name`, `version`, `hash`, in
/// that order.
///
/// INTEGRATION POINT (audit subsystem): embed as `manifest: Option<ManifestIdentity>` on the
/// audit record; `None` must serialize as JSON `null` and the field must always be present
/// (shared format doc 6.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ManifestIdentity {
    /// Required top-level manifest `name` field.
    pub name: String,
    /// Required top-level manifest `version` field (free-form label).
    pub version: String,
    /// SHA-256 over the canonical bytes, 64 lowercase hex characters.
    pub hash: String,
}

/// Why a manifest source could not yield an identity.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// The source is not valid JSON.
    #[error("manifest is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// The top-level JSON value is not an object.
    #[error("manifest is not a JSON object")]
    NotAnObject,
    /// A required top-level string field is missing or not a string.
    #[error("manifest is missing required string field '{0}'")]
    MissingField(&'static str),
}

/// Strip a leading UTF-8 BOM (`EF BB BF`) if present, exactly once. Nothing else is trimmed.
fn strip_bom(source: &[u8]) -> &[u8] {
    source.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(source)
}

/// Render a SHA-256 digest as 64 lowercase hex characters.
fn hex_lower(digest: &[u8]) -> String {
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(out, "{byte:02x}").expect("writing to a String cannot fail");
    }
    out
}

/// SHA-256 content hash over the canonical bytes of a manifest source (shared format doc
/// 4.2). Strips a UTF-8 BOM, parses, re-serializes compactly with authored key order
/// preserved, hashes, and renders 64 lowercase hex characters.
pub fn canonical_hash(source: &[u8]) -> Result<String, IdentityError> {
    let parsed: serde_json::Value = serde_json::from_slice(strip_bom(source))?;
    let canonical = serde_json::to_vec(&parsed).expect("a parsed Value always re-serializes");
    Ok(hex_lower(&Sha256::digest(&canonical)))
}

/// Parse a manifest source and extract its identity: required top-level `name` and `version`
/// strings (shared format doc 4.1) plus the canonical content hash. Parses exactly once; the
/// hash and the field extraction share the one parsed value.
///
/// No other validation happens here: no `schema` version check, no `grants` parsing, no
/// `config` entries, no `mode`, and no rejection of an unrecognized `hash` field inside the
/// document (the binary computes the hash; an authored `hash` key is ordinary content and
/// participates in the hash like any other field). Identity is deliberately computable even
/// for a manifest that would fail full validation, so the flight recorder can attribute
/// records from day one; the manifest engine (G12) owns rejecting invalid manifests.
pub fn identity_from_source(source: &[u8]) -> Result<ManifestIdentity, IdentityError> {
    let parsed: serde_json::Value = serde_json::from_slice(strip_bom(source))?;
    let canonical = serde_json::to_vec(&parsed).expect("a parsed Value always re-serializes");
    let hash = hex_lower(&Sha256::digest(&canonical));

    let obj = parsed.as_object().ok_or(IdentityError::NotAnObject)?;
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(IdentityError::MissingField("name"))?;
    let version = obj
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or(IdentityError::MissingField("version"))?;

    Ok(ManifestIdentity {
        name: name.to_string(),
        version: version.to_string(),
        hash,
    })
}

/// Status of the org policy file for identity purposes.
#[derive(Debug)]
pub enum ManifestStatus {
    /// No org policy file exists. Normal; means all-open unless a later task says otherwise.
    Absent,
    /// The org policy file exists and yielded an identity.
    Active(ManifestIdentity),
    /// The org policy file exists but could not yield an identity.
    Invalid { path: PathBuf, error: String },
}

/// Read the org policy file (if any) and compute its identity status.
///
/// INTEGRATION POINT (G12 manifest engine): when the manifest engine lands, the active
/// manifest is selected by shared-format 1.3 (org policy file, else --manifest/env, else
/// none) and identity must be computed from the exact bytes of THAT source at load time.
/// This standalone org-policy-file reader then retires in favor of the engine's loader;
/// identity_from_source and canonical_hash stay as the shared primitives.
///
/// Reuses [`crate::governance::config::load::org_policy_path`] (G02) for the platform path
/// rather than re-deriving it a second time; that function already implements the exact
/// shared-format 1.2 per-platform rule this task would otherwise duplicate.
pub fn manifest_status() -> ManifestStatus {
    status_at(&crate::governance::config::load::org_policy_path())
}

/// Test seam: compute the status for an arbitrary path, so tests exercise the real
/// read+parse logic without touching the real platform path.
fn status_at(path: &Path) -> ManifestStatus {
    match std::fs::read(path) {
        Ok(bytes) => match identity_from_source(&bytes) {
            Ok(id) => ManifestStatus::Active(id),
            Err(e) => ManifestStatus::Invalid {
                path: path.to_path_buf(),
                error: e.to_string(),
            },
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => ManifestStatus::Absent,
        Err(e) => ManifestStatus::Invalid {
            path: path.to_path_buf(),
            error: e.to_string(),
        },
    }
}

/// The identity to stamp into audit records: `Some` for `Active`, `None` otherwise. When
/// `None` because of `Invalid`, emits exactly one `tracing::warn!` naming the path and the
/// error (the engine is truthful: a present but broken policy file must not be silently
/// ignored). `Absent` warns nothing.
pub fn active_manifest_identity() -> Option<ManifestIdentity> {
    match manifest_status() {
        ManifestStatus::Active(id) => Some(id),
        ManifestStatus::Invalid { path, error } => {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "org policy file present but invalid; identity unavailable"
            );
            None
        }
        ManifestStatus::Absent => None,
    }
}

/// Body lines of the doctor "Policy manifest:" section, each pre-indented two spaces.
pub fn manifest_section_lines(status: &ManifestStatus) -> Vec<String> {
    match status {
        ManifestStatus::Absent => vec!["  none (all-open)".to_string()],
        ManifestStatus::Active(id) => vec![
            format!("  {:<8} {}", "name", id.name),
            format!("  {:<8} {}", "version", id.version),
            format!("  {:<8} {}", "hash", id.hash),
        ],
        ManifestStatus::Invalid { path, error } => vec![format!(
            "  {}: invalid ({}); identity unavailable",
            path.display(),
            error
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST1_HASH: &str = "8f834113c263e4430f86580b7be7b14248fa65686eaf81ff60965c91a809ba90";

    #[test]
    fn canonical_hash_is_whitespace_and_bom_insensitive() {
        let compact = br#"{"name":"a","version":"1","grants":[]}"#;
        let reformatted = b"{\r\n  \"name\": \"a\",\n  \"version\": \"1\",\r\n  \"grants\": []\n}";
        let bom_prefixed: Vec<u8> = [0xEF, 0xBB, 0xBF]
            .iter()
            .copied()
            .chain(compact.iter().copied())
            .collect();

        assert_eq!(canonical_hash(compact).unwrap(), TEST1_HASH);
        assert_eq!(canonical_hash(reformatted).unwrap(), TEST1_HASH);
        assert_eq!(canonical_hash(&bom_prefixed).unwrap(), TEST1_HASH);
    }

    #[test]
    fn canonical_hash_is_sensitive_to_key_order_and_content() {
        let reordered = br#"{"version":"1","name":"a","grants":[]}"#;
        let changed = br#"{"name":"b","version":"1","grants":[]}"#;
        assert_ne!(canonical_hash(reordered).unwrap(), TEST1_HASH);
        assert_ne!(canonical_hash(changed).unwrap(), TEST1_HASH);
    }

    #[test]
    fn canonical_hash_of_the_empty_object() {
        assert_eq!(
            canonical_hash(b"{}").unwrap(),
            "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
    }

    #[test]
    fn hash_is_64_lowercase_hex() {
        assert_eq!(TEST1_HASH.len(), 64);
        assert!(TEST1_HASH
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn identity_extraction_requires_name_and_version() {
        let source = br#"{"name":"a","version":"1","grants":[]}"#;
        let id = identity_from_source(source).unwrap();
        assert_eq!(id.name, "a");
        assert_eq!(id.version, "1");
        assert_eq!(id.hash, TEST1_HASH);

        let err = identity_from_source(br#"{"version":"1"}"#).unwrap_err();
        assert!(err.to_string().contains("name"));

        let err = identity_from_source(br#"{"name":"a","version":2}"#).unwrap_err();
        assert!(err.to_string().contains("version"));

        assert!(matches!(
            identity_from_source(b"[]").unwrap_err(),
            IdentityError::NotAnObject
        ));
        assert!(matches!(
            identity_from_source(b"not json").unwrap_err(),
            IdentityError::InvalidJson(_)
        ));
    }

    #[test]
    fn identity_serializes_as_the_audit_manifest_object() {
        let id = ManifestIdentity {
            name: "acme-clinical-pilot".to_string(),
            version: "2026.07.1".to_string(),
            hash: TEST1_HASH.to_string(),
        };
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            format!(
                "{{\"name\":\"acme-clinical-pilot\",\"version\":\"2026.07.1\",\"hash\":\"{TEST1_HASH}\"}}"
            )
        );
    }

    #[test]
    fn manifest_section_lines_render_each_status() {
        assert_eq!(
            manifest_section_lines(&ManifestStatus::Absent),
            vec!["  none (all-open)".to_string()]
        );

        let id = ManifestIdentity {
            name: "acme".to_string(),
            version: "1.0".to_string(),
            hash: TEST1_HASH.to_string(),
        };
        assert_eq!(
            manifest_section_lines(&ManifestStatus::Active(id)),
            vec![
                format!("  {:<8} acme", "name"),
                format!("  {:<8} 1.0", "version"),
                format!("  {:<8} {TEST1_HASH}", "hash"),
            ]
        );

        let invalid = ManifestStatus::Invalid {
            path: PathBuf::from("/etc/browser-mcp/policy.json"),
            error: "manifest is not valid JSON: EOF".to_string(),
        };
        let lines = manifest_section_lines(&invalid);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("/etc/browser-mcp/policy.json"));
        assert!(lines[0].contains("invalid ("));
        assert!(lines[0].ends_with("identity unavailable"));
    }

    #[test]
    fn manifest_status_reads_the_org_policy_file() {
        let dir =
            std::env::temp_dir().join(format!("browser-mcp-identity-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("policy.json");

        assert!(matches!(status_at(&path), ManifestStatus::Absent));

        std::fs::write(&path, br#"{"name":"acme","version":"1","grants":[]}"#).unwrap();
        match status_at(&path) {
            ManifestStatus::Active(id) => {
                assert_eq!(id.name, "acme");
                assert_eq!(id.version, "1");
            }
            other => panic!("expected Active, got {other:?}"),
        }

        std::fs::write(&path, b"not json").unwrap();
        match status_at(&path) {
            ManifestStatus::Invalid { error, .. } => assert!(error.contains("JSON")),
            other => panic!("expected Invalid, got {other:?}"),
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
