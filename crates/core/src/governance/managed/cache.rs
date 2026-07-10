// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! The managed:// last-known-good cache and the never-fail-open reconcile (ADR-0055 D5 / Phase 2).
//!
//! Continuity Promise, mechanized: every VERIFIED bundle is written through to disk, and at boot the
//! endpoint loads and RE-VERIFIES the cached bundle before the network is even consulted. Two
//! failure modes both retain last-known-good and never fall back to unrestricted. UNREACHABLE
//! (source down or absent) keeps the cached bundle (OPA `persist` behavior); REACHABLE-BUT-BAD (a
//! fetched bundle fails signature or schema) rejects the bad update and keeps the last VALID bundle
//! (the Envoy-xDS NACK-and-keep-last-valid behavior). A third, security-critical case: a fetched,
//! validly-signed bundle whose publish sequence is BELOW what we hold is a rollback and is refused
//! (ADR-0055 D6 anti-rollback), keeping the cache -- the guardian moment surfaced in Phase 5.
//!
//! The cache is SIGNED (it is the exact bundle bytes) and verified on load, so a tampered cache file
//! is ignored, never trusted -- closing the fail-open-via-cache hole every plain-JSON flag SDK
//! leaves open. Machine-bound at-rest ENCRYPTION is deferred (ADR-0055 Implementation Decision 5):
//! integrity, not confidentiality, is the security-critical half, and the policy is not secret.

use std::path::Path;

use crate::governance::crypto::GenKey;

use super::{fetch_bytes, org_key, verify_and_parse, ManagedBootstrap, ManagedError, VerifiedManaged};

/// The two failure modes of a fresh-load attempt (ADR-0055 D5); both retain last-known-good.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreshError {
    /// The source could not be reached or read (network down, file absent). Keep the cache.
    Unreachable,
    /// The source was reached but returned a malformed / bad-signature / bad-schema bundle. Keep the
    /// last VALID cache; the bad update is never applied.
    Bad(String),
}

impl FreshError {
    /// Classify a raw-fetch error: an I/O failure or an un-fetchable network source is UNREACHABLE;
    /// anything else is treated as a bad source.
    fn from_fetch(e: ManagedError) -> Self {
        match e {
            ManagedError::Io { .. } | ManagedError::NetworkNotYet | ManagedError::Fetch(_) => {
                FreshError::Unreachable
            }
            other => FreshError::Bad(other.to_string()),
        }
    }
}

/// Why the active policy is the cached last-known-good rather than a fresh one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleReason {
    /// The source was unreachable; the cache keeps enforcing.
    SourceUnreachable,
    /// A fetched bundle failed verification; the last valid bundle keeps enforcing.
    UpdateRejected,
    /// A fetched, validly-signed bundle was older than the cache (a downgrade); it was refused.
    RollbackRefused,
}

/// The freshness verdict surfaced to the operator (the Phase 5 FRESH / STALE state; OpenFeature's
/// STALE analog).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    /// Enforcing a freshly fetched-and-verified bundle.
    Fresh,
    /// Enforcing the cached last-known-good, for the given reason.
    LastKnownGood(StaleReason),
    /// No fresh bundle AND no usable cache: the caller MUST fail closed (refuse to run), never open.
    NoPolicy,
}

/// The reconciled outcome: which policy to enforce (never fail open), its freshness, and whether the
/// caller should persist the fresh bundle to the cache.
#[derive(Debug, Clone, PartialEq)]
pub struct Reconciled {
    pub active: Option<VerifiedManaged>,
    pub freshness: Freshness,
    pub persist_fresh: bool,
}

/// Pure: given a fresh-load result and the cached last-known-good, decide the active policy without
/// ever failing open. A fresh bundle wins only if it is valid AND its sequence is not a rollback;
/// otherwise the cache stands; and with neither available the verdict is `NoPolicy` (fail closed).
pub fn reconcile(fresh: Result<VerifiedManaged, FreshError>, cached: Option<VerifiedManaged>) -> Reconciled {
    match (fresh, cached) {
        (Ok(f), None) => Reconciled {
            active: Some(f),
            freshness: Freshness::Fresh,
            persist_fresh: true,
        },
        (Ok(f), Some(c)) => {
            if f.seq >= c.seq {
                Reconciled {
                    active: Some(f),
                    freshness: Freshness::Fresh,
                    persist_fresh: true,
                }
            } else {
                // The guardian moment: a fetched, validly-signed bundle OLDER than what we hold.
                Reconciled {
                    active: Some(c),
                    freshness: Freshness::LastKnownGood(StaleReason::RollbackRefused),
                    persist_fresh: false,
                }
            }
        }
        (Err(FreshError::Unreachable), Some(c)) => Reconciled {
            active: Some(c),
            freshness: Freshness::LastKnownGood(StaleReason::SourceUnreachable),
            persist_fresh: false,
        },
        (Err(FreshError::Bad(_)), Some(c)) => Reconciled {
            active: Some(c),
            freshness: Freshness::LastKnownGood(StaleReason::UpdateRejected),
            persist_fresh: false,
        },
        (Err(_), None) => Reconciled {
            active: None,
            freshness: Freshness::NoPolicy,
            persist_fresh: false,
        },
    }
}

/// Read and RE-VERIFY the cached bundle. `None` when absent, unreadable, OR unverifiable: a tampered
/// or corrupt cache is ignored (never trusted), which is safe because the caller still has the fresh
/// attempt and the fail-closed backstop.
pub fn read_cache(path: &Path, key: &GenKey, domain_pattern_valid: fn(&str) -> bool) -> Option<VerifiedManaged> {
    let bytes = std::fs::read(path).ok()?;
    match verify_and_parse(&bytes, key, domain_pattern_valid) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "ignoring an unverifiable managed policy cache");
            None
        }
    }
}

/// Atomically write the verified bundle bytes to the cache (temp file + rename), creating the parent
/// directory if needed.
pub fn write_cache(path: &Path, bundle_bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bundle_bytes)?;
    std::fs::rename(&tmp, path)
}

/// Resolve the active managed policy: read the last-known-good cache, attempt a fresh load
/// (local today, network in Phase 3), reconcile the two never-failing-open, and write through a
/// newly accepted fresh bundle. `Err` is a FATAL bootstrap error (a malformed org key -- nothing can
/// be verified, not even the cache); `Ok(Reconciled)` is the reconciled outcome, whose `NoPolicy`
/// freshness tells a first-boot-offline caller to fail closed.
pub fn resolve_managed(
    bootstrap: &ManagedBootstrap,
    cache_path: &Path,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<Reconciled, ManagedError> {
    let key = org_key(bootstrap)?;
    let cached = read_cache(cache_path, &key, domain_pattern_valid);
    let (fresh, fresh_bytes): (Result<VerifiedManaged, FreshError>, Option<Vec<u8>>) =
        match fetch_bytes(bootstrap) {
            Ok(bytes) => match verify_and_parse(&bytes, &key, domain_pattern_valid) {
                Ok(v) => (Ok(v), Some(bytes)),
                Err(e) => (Err(FreshError::Bad(e.to_string())), None),
            },
            Err(e) => (Err(FreshError::from_fetch(e)), None),
        };
    let reconciled = reconcile(fresh, cached);
    if reconciled.persist_fresh {
        if let Some(bytes) = &fresh_bytes {
            if let Err(e) = write_cache(cache_path, bytes) {
                tracing::warn!(error = %e, "failed to persist the managed policy cache; enforcing anyway");
            }
        }
    }
    Ok(reconciled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::crypto;
    use crate::governance::manifest::bundle;

    fn ok_pattern(_: &str) -> bool {
        true
    }

    fn hex(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    fn signed(seq: u64, name: &str, seed: &[u8; 32]) -> Vec<u8> {
        bundle::sign_bundle(
            seed,
            None,
            seq,
            serde_json::json!({ "schema": 3, "name": name, "version": "1", "grants": [] }),
            None,
        )
    }

    fn verified(seq: u64, name: &str, seed: &[u8; 32]) -> VerifiedManaged {
        let bytes = signed(seq, name, seed);
        let key = bundle::org_key(&crypto::admin::ed_public(seed), None).unwrap();
        verify_and_parse(&bytes, &key, ok_pattern).unwrap()
    }

    // --- reconcile (pure) ---

    #[test]
    fn fresh_with_no_cache_is_accepted_and_persisted() {
        let r = reconcile(Ok(verified(1, "a", &[1u8; 32])), None);
        assert_eq!(r.freshness, Freshness::Fresh);
        assert!(r.persist_fresh);
        assert_eq!(r.active.unwrap().seq, 1);
    }

    #[test]
    fn fresh_with_a_higher_sequence_replaces_the_cache() {
        let seed = [2u8; 32];
        let r = reconcile(Ok(verified(5, "a", &seed)), Some(verified(3, "a", &seed)));
        assert_eq!(r.freshness, Freshness::Fresh);
        assert!(r.persist_fresh);
        assert_eq!(r.active.unwrap().seq, 5);
    }

    #[test]
    fn fresh_with_a_lower_sequence_is_a_refused_rollback() {
        let seed = [3u8; 32];
        let r = reconcile(Ok(verified(2, "a", &seed)), Some(verified(9, "a", &seed)));
        assert_eq!(r.freshness, Freshness::LastKnownGood(StaleReason::RollbackRefused));
        assert!(!r.persist_fresh);
        assert_eq!(r.active.unwrap().seq, 9, "the cache stands");
    }

    #[test]
    fn unreachable_keeps_the_cache() {
        let r = reconcile(Err(FreshError::Unreachable), Some(verified(4, "a", &[4u8; 32])));
        assert_eq!(r.freshness, Freshness::LastKnownGood(StaleReason::SourceUnreachable));
        assert_eq!(r.active.unwrap().seq, 4);
    }

    #[test]
    fn a_bad_update_keeps_the_cache() {
        let r = reconcile(
            Err(FreshError::Bad("bad signature".into())),
            Some(verified(4, "a", &[5u8; 32])),
        );
        assert_eq!(r.freshness, Freshness::LastKnownGood(StaleReason::UpdateRejected));
        assert_eq!(r.active.unwrap().seq, 4);
    }

    #[test]
    fn nothing_available_is_no_policy_never_open() {
        let r = reconcile(Err(FreshError::Unreachable), None);
        assert_eq!(r.freshness, Freshness::NoPolicy);
        assert!(r.active.is_none(), "no policy -> caller fails closed, never all-open");
    }

    // --- cache I/O ---

    #[test]
    fn write_then_read_round_trips() {
        let seed = [11u8; 32];
        let bytes = signed(7, "cache", &seed);
        let path = std::env::temp_dir().join(format!("gl-cache-rt-{}.bundle", std::process::id()));
        write_cache(&path, &bytes).unwrap();
        let key = bundle::org_key(&crypto::admin::ed_public(&seed), None).unwrap();
        let read = read_cache(&path, &key, ok_pattern);
        std::fs::remove_file(&path).ok();
        assert_eq!(read.unwrap().seq, 7);
    }

    #[test]
    fn read_cache_ignores_a_tampered_file() {
        let seed = [12u8; 32];
        let bytes = signed(1, "cache", &seed);
        let path = std::env::temp_dir().join(format!("gl-cache-bad-{}.bundle", std::process::id()));
        write_cache(&path, &bytes).unwrap();
        // Verify against a DIFFERENT key -> the on-disk cache does not authenticate -> ignored.
        let wrong = bundle::org_key(&crypto::admin::ed_public(&[99u8; 32]), None).unwrap();
        let read = read_cache(&path, &wrong, ok_pattern);
        std::fs::remove_file(&path).ok();
        assert!(read.is_none());
    }

    // --- resolve_managed (orchestration) ---

    fn bootstrap_for(seed: &[u8; 32], source: &Path) -> ManagedBootstrap {
        ManagedBootstrap {
            source: source.display().to_string(),
            pubkey_ed25519: hex(&crypto::admin::ed_public(seed)),
            ..Default::default()
        }
    }

    #[test]
    fn first_load_populates_the_cache() {
        let seed = [21u8; 32];
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let src = dir.join(format!("gl-resolve-first-src-{pid}.bundle"));
        let cache = dir.join(format!("gl-resolve-first-cache-{pid}.bundle"));
        std::fs::write(&src, signed(3, "first", &seed)).unwrap();
        let _ = std::fs::remove_file(&cache);

        let r = resolve_managed(&bootstrap_for(&seed, &src), &cache, ok_pattern).unwrap();
        let cache_existed = cache.exists();
        for p in [&src, &cache] {
            std::fs::remove_file(p).ok();
        }
        assert_eq!(r.freshness, Freshness::Fresh);
        assert_eq!(r.active.unwrap().seq, 3);
        assert!(cache_existed, "the accepted fresh bundle was written through");
    }

    #[test]
    fn unreachable_source_enforces_the_cache() {
        let seed = [22u8; 32];
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let cache = dir.join(format!("gl-resolve-unreach-cache-{pid}.bundle"));
        write_cache(&cache, &signed(4, "cached", &seed)).unwrap();
        // A source path that does not exist -> Unreachable.
        let missing = dir.join(format!("gl-resolve-unreach-missing-{pid}.bundle"));
        let _ = std::fs::remove_file(&missing);

        let r = resolve_managed(&bootstrap_for(&seed, &missing), &cache, ok_pattern).unwrap();
        std::fs::remove_file(&cache).ok();
        assert_eq!(r.freshness, Freshness::LastKnownGood(StaleReason::SourceUnreachable));
        assert_eq!(r.active.unwrap().seq, 4);
    }

    #[test]
    fn a_downgrade_source_is_refused_and_the_cache_stands() {
        let seed = [23u8; 32];
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let src = dir.join(format!("gl-resolve-down-src-{pid}.bundle"));
        let cache = dir.join(format!("gl-resolve-down-cache-{pid}.bundle"));
        write_cache(&cache, &signed(9, "cached", &seed)).unwrap();
        std::fs::write(&src, signed(3, "older", &seed)).unwrap(); // older, validly signed

        let r = resolve_managed(&bootstrap_for(&seed, &src), &cache, ok_pattern).unwrap();
        for p in [&src, &cache] {
            std::fs::remove_file(p).ok();
        }
        assert_eq!(r.freshness, Freshness::LastKnownGood(StaleReason::RollbackRefused));
        assert_eq!(r.active.unwrap().seq, 9);
    }

    #[test]
    fn first_boot_offline_is_no_policy() {
        let seed = [24u8; 32];
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let missing_src = dir.join(format!("gl-resolve-cold-src-{pid}.bundle"));
        let missing_cache = dir.join(format!("gl-resolve-cold-cache-{pid}.bundle"));
        for p in [&missing_src, &missing_cache] {
            let _ = std::fs::remove_file(p);
        }
        let r = resolve_managed(&bootstrap_for(&seed, &missing_src), &missing_cache, ok_pattern).unwrap();
        assert_eq!(r.freshness, Freshness::NoPolicy);
        assert!(r.active.is_none());
    }

    #[test]
    fn a_malformed_org_key_is_a_fatal_bootstrap_error() {
        let bootstrap = ManagedBootstrap {
            source: "irrelevant".into(),
            pubkey_ed25519: "tooshort".into(),
            ..Default::default()
        };
        let cache = std::env::temp_dir().join("gl-resolve-badkey.bundle");
        assert!(matches!(
            resolve_managed(&bootstrap, &cache, ok_pattern),
            Err(ManagedError::Key(_))
        ));
    }
}
