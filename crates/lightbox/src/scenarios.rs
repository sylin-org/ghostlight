// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Lightbox scenarios (ADR-0056): named, runnable proofs of ADR-0055 invariants, executed through
//! the REAL `governance::managed` code with injected [`GovernancePaths`] and a real localhost
//! endpoint. Each returns `Ok(())` on pass. This is the executable spec that closes the ADR-0055
//! Phase-4a owed live-e2e as one command.

use anyhow::{anyhow, ensure};

use ghostlight_core::governance::managed;
use ghostlight_core::governance::managed::cache::{Freshness, StaleReason};
use ghostlight_core::governance::paths::GovernancePaths;

use crate::support::{self, BundleServer, TempRoot};

/// A permissive host-pattern validator: the scenario manifests carry no host patterns, so this is
/// never exercised; the real validator lives in the browser plugin.
fn any_pattern(_: &str) -> bool {
    true
}

/// A scenario: a stable name paired with its runnable proof.
pub type Scenario = (&'static str, fn() -> anyhow::Result<()>);

/// The scenario registry: stable name -> function.
pub fn registry() -> Vec<Scenario> {
    vec![
        ("managed-activation-local", managed_activation_local),
        ("managed-activation-network", managed_activation_network),
        ("fail-closed-cold-boot", fail_closed_cold_boot),
        ("continuity-source-unreachable", continuity_source_unreachable),
        ("rollback-guardian", rollback_guardian),
        ("update-on-reresolve", update_on_reresolve),
        ("no-clobber-on-reresolve", no_clobber_on_reresolve),
    ]
}

/// An org-signed bundle at a LOCAL path (the air-gap path) activates as the org's policy.
fn managed_activation_local() -> anyhow::Result<()> {
    let tmp = TempRoot::new("activate-local")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [7u8; 32];
    let bundle_path = tmp.path().join("policy.bundle");
    std::fs::write(&bundle_path, support::sign(&seed, 3, support::manifest("acme-corp")))?;
    support::write_bootstrap(&paths.managed_bootstrap, &bundle_path.display().to_string(), &seed)?;

    let reconciled = managed::activate(&paths, any_pattern)?
        .ok_or_else(|| anyhow!("bootstrap present but activate returned None"))?;
    let active = reconciled
        .active
        .ok_or_else(|| anyhow!("no active policy after activation"))?;
    ensure!(active.manifest.name == "acme-corp", "wrong manifest: {}", active.manifest.name);
    ensure!(active.seq == 3, "wrong seq: {}", active.seq);
    ensure!(matches!(reconciled.freshness, Freshness::Fresh), "expected Fresh");
    Ok(())
}

/// An org-signed bundle fetched over a REAL localhost endpoint (the ureq/rustls path) activates.
fn managed_activation_network() -> anyhow::Result<()> {
    let tmp = TempRoot::new("activate-net")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [8u8; 32];
    let server = BundleServer::start(support::sign(&seed, 5, support::manifest("acme-net")))?;
    support::write_bootstrap(&paths.managed_bootstrap, &server.url(), &seed)?;

    let reconciled = managed::activate(&paths, any_pattern)?
        .ok_or_else(|| anyhow!("bootstrap present but activate returned None"))?;
    let active = reconciled
        .active
        .ok_or_else(|| anyhow!("network fetch did not activate a policy"))?;
    ensure!(active.manifest.name == "acme-net", "wrong manifest: {}", active.manifest.name);
    ensure!(active.seq == 5, "wrong seq: {}", active.seq);
    ensure!(matches!(reconciled.freshness, Freshness::Fresh), "expected Fresh");
    Ok(())
}

/// First boot, source unreachable, no cache: refuse to run unrestricted (fail closed).
fn fail_closed_cold_boot() -> anyhow::Result<()> {
    let tmp = TempRoot::new("fail-closed")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [9u8; 32];
    // Port 1 is not listening: an immediate connection refusal, no cache to fall back on.
    support::write_bootstrap(&paths.managed_bootstrap, "http://127.0.0.1:1/policy.bundle", &seed)?;

    let reconciled = managed::activate(&paths, any_pattern)?
        .ok_or_else(|| anyhow!("bootstrap present but activate returned None"))?;
    ensure!(
        matches!(reconciled.freshness, Freshness::NoPolicy),
        "expected NoPolicy, got {:?}",
        reconciled.freshness
    );
    ensure!(
        reconciled.active.is_none(),
        "a cold boot with no policy must have no active policy (fail closed)"
    );
    Ok(())
}

/// Source unreachable after a prior success: the cached last-known-good keeps enforcing.
fn continuity_source_unreachable() -> anyhow::Result<()> {
    let tmp = TempRoot::new("continuity")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [10u8; 32];
    {
        let server = BundleServer::start(support::sign(&seed, 6, support::manifest("acme-cont")))?;
        support::write_bootstrap(&paths.managed_bootstrap, &server.url(), &seed)?;
        let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
        ensure!(matches!(r.freshness, Freshness::Fresh), "first activation should be Fresh");
        ensure!(r.active.is_some(), "first activation should have a policy");
    } // server dropped -> the source is now unreachable

    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(
        matches!(r.freshness, Freshness::LastKnownGood(StaleReason::SourceUnreachable)),
        "expected cached last-known-good, got {:?}",
        r.freshness
    );
    ensure!(
        r.active.map(|v| v.seq) == Some(6),
        "the cache must still enforce the last policy"
    );
    Ok(())
}

/// A downgrade (an older, validly-signed bundle from a stale mirror) is refused; the cache stands.
fn rollback_guardian() -> anyhow::Result<()> {
    let tmp = TempRoot::new("rollback")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [11u8; 32];
    let bundle_path = tmp.path().join("policy.bundle");
    support::write_bootstrap(&paths.managed_bootstrap, &bundle_path.display().to_string(), &seed)?;

    std::fs::write(&bundle_path, support::sign(&seed, 9, support::manifest("acme-v9")))?;
    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(r.active.as_ref().map(|v| v.seq) == Some(9), "seq 9 should activate");

    // The source now serves an OLDER seq (a rollback attempt): refused, cache stands.
    std::fs::write(&bundle_path, support::sign(&seed, 3, support::manifest("acme-v3")))?;
    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(
        matches!(r.freshness, Freshness::LastKnownGood(StaleReason::RollbackRefused)),
        "expected RollbackRefused, got {:?}",
        r.freshness
    );
    ensure!(
        r.active.as_ref().map(|v| v.seq) == Some(9),
        "the seq-9 cache must stand against the downgrade"
    );
    Ok(())
}

/// A newer org policy is picked up on re-resolve (what each Phase-4b poll tick performs), via a real
/// conditional-fetch endpoint that swaps the served bundle mid-run.
fn update_on_reresolve() -> anyhow::Result<()> {
    let tmp = TempRoot::new("update")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [12u8; 32];
    let server = BundleServer::start(support::sign(&seed, 5, support::manifest("acme-v5")))?;
    support::write_bootstrap(&paths.managed_bootstrap, &server.url(), &seed)?;

    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(r.active.as_ref().map(|v| v.seq) == Some(5), "seq 5 should activate first");

    // The org publishes a newer policy; a re-resolve picks it up (bumped ETag => a fresh 200).
    server.set_bundle(support::sign(&seed, 6, support::manifest("acme-v6")));
    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(matches!(r.freshness, Freshness::Fresh), "the update should be Fresh");
    ensure!(
        r.active.as_ref().map(|v| v.seq) == Some(6),
        "the newer policy should be picked up"
    );
    Ok(())
}

/// The fail-open fix (ADR-0056): a re-resolve under managed governance -- what a routine user
/// `config set` triggers via the file watcher -- must NOT clobber the managed policy with all-open.
/// The old code re-ran the source-string loader here and published unrestricted.
fn no_clobber_on_reresolve() -> anyhow::Result<()> {
    use ghostlight_core::governance::config::reload::{ConfigStore, PolicySource};
    use ghostlight_core::governance::manifest::source::{LoadedPolicy, ManifestOrigin};

    let tmp = TempRoot::new("no-clobber")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [13u8; 32];
    let bundle_path = tmp.path().join("policy.bundle");
    std::fs::write(&bundle_path, support::sign(&seed, 4, support::manifest("acme-live")))?;
    support::write_bootstrap(&paths.managed_bootstrap, &bundle_path.display().to_string(), &seed)?;

    // Build the live store with the MANAGED policy source, exactly as the service does.
    let initial = managed::activate(&paths, any_pattern)?
        .and_then(|r| r.active)
        .ok_or_else(|| anyhow!("initial managed activation failed"))?;
    let loaded = LoadedPolicy {
        manifest: Some(initial.manifest),
        origin: Some(ManifestOrigin::Managed),
        user_manifest_ignored: false,
    };
    let store = ConfigStore::load_initial_with_policy(
        any_pattern,
        &loaded,
        PolicySource::Managed { paths },
    )
    .map_err(|e| anyhow!("build store: {e}"))?;

    // A file-watch tick (what a `config set` triggers). The managed policy MUST stand.
    store.reresolve();
    let published = store.policy().borrow().clone();
    let name = published.manifest.as_ref().map(|m| m.name.clone());
    ensure!(
        name.as_deref() == Some("acme-live"),
        "reresolve clobbered the managed policy (got {name:?}) -- this is the fail-open"
    );
    ensure!(
        matches!(published.origin, Some(ManifestOrigin::Managed)),
        "the origin must stay Managed after reresolve"
    );
    Ok(())
}
