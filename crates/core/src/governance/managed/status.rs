// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! The managed:// status sidecar (ADR-0055 Implementation Decision 8): a versioned, no-secrets
//! `managed-status.json` written beside the last-known-good cache on every managed resolve.
//!
//! The sidecar IS the ManagedStatus store: one writer ([`super::activate`], via [`write_sidecar`])
//! and several readers (the `doctor` command, the `explain` tool's Policy Passport, and the Console
//! later). It carries NO secrets (no bearer token, no key material) and NO trust: readers degrade
//! gracefully when it is absent or unparseable ([`read_sidecar`] returns `None`), because the only
//! trust anchor is the SIGNED cache, never this human-facing summary. It exists so an admin can ask
//! "did my policy propagate?" without a running service (ADR-0055 D9 professional register).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::cache::{self, Freshness, Reconciled, StaleReason};
use crate::governance::manifest::bundle::Presentation;

/// The file name of the status sidecar, written beside the managed policy cache.
const SIDECAR_FILE: &str = "managed-status.json";

/// A no-secrets, human-facing snapshot of the last managed resolve (ADR-0055 Impl.8). Serialized as
/// pretty JSON beside the cache; version-stamped (`v`) so a future field addition stays readable by
/// an older reader. This is a REPORT, not a trust artifact -- the signed cache is authoritative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedStatus {
    /// Schema version of this sidecar; always `1` for this build.
    pub v: u32,
    /// The freshness verdict as a stable snake_case string: `"fresh"`, `"last_known_good"`, or
    /// `"no_policy"` (mirrors [`Freshness`]).
    pub freshness: String,
    /// Why the active policy is last-known-good, as snake_case: `"source_unreachable"`,
    /// `"update_rejected"`, or `"rollback_refused"`; `None` when fresh or no-policy.
    pub stale_reason: Option<String>,
    /// The active policy's monotonic publish sequence, if a policy is active.
    pub seq: Option<u64>,
    /// When this sidecar was written (`chrono::Utc::now().to_rfc3339()`).
    pub fetched_at: String,
    /// The bootstrap `source` verbatim (a local path or an https URL). Carries no secret.
    pub source: String,
    /// The org-authored presentation (name, rationale, contacts), cloned from the active policy.
    pub presentation: Option<Presentation>,
    /// A human-facing note about the last error, if any (never key material).
    pub last_error: Option<String>,
}

/// Build a [`ManagedStatus`] from a reconciled outcome (ADR-0055 Impl.8). Maps the freshness verdict
/// to its exact snake_case strings and pulls `seq`/`presentation` from the active policy.
pub fn from_reconciled(r: &Reconciled, source: &str, last_error: Option<String>) -> ManagedStatus {
    let (freshness, stale_reason) = match &r.freshness {
        Freshness::Fresh => ("fresh", None),
        Freshness::LastKnownGood(reason) => (
            "last_known_good",
            Some(match reason {
                StaleReason::SourceUnreachable => "source_unreachable",
                StaleReason::UpdateRejected => "update_rejected",
                StaleReason::RollbackRefused => "rollback_refused",
            }),
        ),
        Freshness::NoPolicy => ("no_policy", None),
    };
    ManagedStatus {
        v: 1,
        freshness: freshness.to_string(),
        stale_reason: stale_reason.map(|s| s.to_string()),
        seq: r.active.as_ref().map(|vm| vm.seq),
        fetched_at: chrono::Utc::now().to_rfc3339(),
        source: source.to_string(),
        presentation: r.active.as_ref().and_then(|vm| vm.presentation.clone()),
        last_error,
    }
}

/// The sidecar path for a given cache path: the cache's parent directory joined with
/// `managed-status.json` (falling back to a same-directory sibling of the cache file).
pub fn sidecar_path(cache_path: &Path) -> PathBuf {
    match cache_path.parent() {
        Some(parent) => parent.join(SIDECAR_FILE),
        None => cache_path.with_file_name(SIDECAR_FILE),
    }
}

/// Atomically write the status sidecar (pretty JSON, temp+rename), creating the parent directory if
/// needed. Reuses [`cache::write_cache`]'s atomic write; a failure here never fails activation.
pub fn write_sidecar(path: &Path, s: &ManagedStatus) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(s)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    cache::write_cache(path, &bytes)
}

/// Read the status sidecar, or `None` when it is absent or unparseable. Readers degrade gracefully:
/// the sidecar carries no trust, so a missing or corrupt file is simply "no status".
pub fn read_sidecar(path: &Path) -> Option<ManagedStatus> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::crypto;
    use crate::governance::manifest::bundle;

    fn ok_pattern(_: &str) -> bool {
        true
    }

    fn verified(seq: u64, seed: &[u8; 32]) -> crate::governance::managed::VerifiedManaged {
        let bytes = bundle::sign_bundle(
            seed,
            None,
            seq,
            serde_json::json!({ "schema": 3, "name": "acme", "version": "1", "grants": [] }),
            None,
        );
        let key = bundle::org_key(&crypto::admin::ed_public(seed), None).unwrap();
        crate::governance::managed::verify_and_parse(&bytes, &key, ok_pattern).unwrap()
    }

    fn reconciled(freshness: Freshness, seq: u64, seed: &[u8; 32]) -> Reconciled {
        Reconciled {
            active: Some(verified(seq, seed)),
            freshness,
            persist_fresh: false,
        }
    }

    #[test]
    fn snake_case_mapping_is_exact() {
        let seed = [71u8; 32];
        let fresh = from_reconciled(&reconciled(Freshness::Fresh, 4, &seed), "src", None);
        assert_eq!(
            (fresh.freshness.as_str(), fresh.stale_reason),
            ("fresh", None)
        );

        let unreachable = from_reconciled(
            &reconciled(
                Freshness::LastKnownGood(StaleReason::SourceUnreachable),
                4,
                &seed,
            ),
            "src",
            None,
        );
        assert_eq!(
            (unreachable.freshness.as_str(), unreachable.stale_reason),
            ("last_known_good", Some("source_unreachable".to_string()))
        );

        let rejected = from_reconciled(
            &reconciled(
                Freshness::LastKnownGood(StaleReason::UpdateRejected),
                4,
                &seed,
            ),
            "src",
            None,
        );
        assert_eq!(
            (rejected.freshness.as_str(), rejected.stale_reason),
            ("last_known_good", Some("update_rejected".to_string()))
        );

        let rollback = from_reconciled(
            &reconciled(
                Freshness::LastKnownGood(StaleReason::RollbackRefused),
                4,
                &seed,
            ),
            "src",
            None,
        );
        assert_eq!(
            (rollback.freshness.as_str(), rollback.stale_reason),
            ("last_known_good", Some("rollback_refused".to_string()))
        );

        let no_policy = from_reconciled(
            &Reconciled {
                active: None,
                freshness: Freshness::NoPolicy,
                persist_fresh: false,
            },
            "src",
            None,
        );
        assert_eq!(
            (no_policy.freshness.as_str(), no_policy.stale_reason),
            ("no_policy", None)
        );
    }

    #[test]
    fn sidecar_round_trips() {
        let seed = [72u8; 32];
        let status = from_reconciled(
            &reconciled(Freshness::Fresh, 6, &seed),
            "https://policy.example/x",
            None,
        );
        let path = std::env::temp_dir().join(format!("gl-sidecar-rt-{}.json", std::process::id()));
        write_sidecar(&path, &status).unwrap();
        let read = read_sidecar(&path);
        std::fs::remove_file(&path).ok();
        let read = read.expect("round-trips");
        assert_eq!(read.v, 1);
        assert_eq!(read.seq, Some(6));
        assert_eq!(read.source, "https://policy.example/x");
    }

    #[test]
    fn read_sidecar_absent_or_garbage_is_none() {
        let missing =
            std::env::temp_dir().join(format!("gl-sidecar-absent-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&missing);
        assert!(read_sidecar(&missing).is_none());

        let garbage =
            std::env::temp_dir().join(format!("gl-sidecar-garbage-{}.json", std::process::id()));
        std::fs::write(&garbage, b"not json").unwrap();
        let read = read_sidecar(&garbage);
        std::fs::remove_file(&garbage).ok();
        assert!(read.is_none());
    }
}
