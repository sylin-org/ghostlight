// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Lightbox scenarios (ADR-0056): named, runnable proofs of ADR-0055 invariants, executed through
//! the REAL `governance::managed` code with injected [`GovernancePaths`] and a real localhost
//! endpoint. Each returns `Ok(())` on pass. This is the executable spec that closes the ADR-0055
//! Phase-4a owed live-e2e as one command.

use std::sync::Arc;

use anyhow::{anyhow, ensure};

use ghostlight_core::browser::polarity;
use ghostlight_core::governance::audit::Recorder;
use ghostlight_core::governance::dispatch::{Gate, Governance};
use ghostlight_core::governance::enforcement::LocalPdp;
use ghostlight_core::governance::license::{self, Claims, LicenseState};
use ghostlight_core::governance::managed;
use ghostlight_core::governance::managed::cache::{Freshness, StaleReason};
use ghostlight_core::governance::managed::status;
use ghostlight_core::governance::manifest::document::{Grant, HostRules};
use ghostlight_core::governance::paths::GovernancePaths;
use ghostlight_core::governance::ports::{AuditSink, Capability, EffectiveMode, GoverningResource};

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
    let mut scenarios: Vec<Scenario> = vec![
        ("managed-activation-local", managed_activation_local),
        ("managed-activation-network", managed_activation_network),
        ("fail-closed-cold-boot", fail_closed_cold_boot),
        (
            "continuity-source-unreachable",
            continuity_source_unreachable,
        ),
        ("rollback-guardian", rollback_guardian),
        ("update-on-reresolve", update_on_reresolve),
        ("no-clobber-on-reresolve", no_clobber_on_reresolve),
        ("sidecar-propagation", sidecar_propagation),
        ("passport-freshness", passport_freshness),
        ("license-expiry-continuity", license_expiry_continuity),
    ];
    scenarios.extend(crate::legacy::registry());
    scenarios.extend(crate::mechanism_wire::registry());
    scenarios
}

/// License expiry changes only the audit marker: the same governed call remains allowed under the
/// same grant, with every deterministic audit field unchanged (ADR-0028 Decisions 1 and 6).
fn license_expiry_continuity() -> anyhow::Result<()> {
    let claims = Claims {
        id: "00000000-0000-4000-8000-000000000001".into(),
        licensee: "Acme Security".into(),
        org: "acme".into(),
        tier: "community".into(),
        seats: 5,
        products: vec!["browser".into()],
        issued: "2026-01-01".into(),
        expires: "2026-12-31".into(),
    };
    let valid = LicenseState::Valid {
        claims: claims.clone(),
        keygen: 1,
    };
    let expired = LicenseState::Expired { claims, keygen: 1 };

    let tmp = TempRoot::new("license-expiry")?;
    let valid_record = governed_read_record(&tmp.path().join("valid.jsonl"), &valid)?;
    let expired_record = governed_read_record(&tmp.path().join("expired.jsonl"), &expired)?;

    ensure!(
        valid_record.get("license").is_none(),
        "an in-date production license must not add an audit marker: {valid_record}"
    );
    ensure!(
        expired_record
            .get("license")
            .and_then(serde_json::Value::as_str)
            == Some("expired"),
        "an expired license must add exactly the expired marker: {expired_record}"
    );

    let stable = |mut record: serde_json::Value| -> anyhow::Result<serde_json::Value> {
        let object = record
            .as_object_mut()
            .ok_or_else(|| anyhow!("audit record is not an object"))?;
        for field in ["event_id", "ts", "duration_ms", "license"] {
            object.remove(field);
        }
        Ok(record)
    };
    ensure!(
        stable(valid_record)? == stable(expired_record)?,
        "license expiry changed a deterministic authorization or audit field"
    );
    Ok(())
}

/// Execute one real governed read and return its single audit record under `state`.
fn governed_read_record(
    audit_path: &std::path::Path,
    state: &LicenseState,
) -> anyhow::Result<serde_json::Value> {
    static READ: &[Capability] = &[Capability::Read];

    let recorder = Arc::new(Recorder::to_file(audit_path.to_path_buf()));
    recorder.set_license_stamp(license::stamp_for(state));
    let audit: Arc<dyn AuditSink> = recorder;
    let grant = Grant {
        id: "acme-read".into(),
        hosts: HostRules {
            allow: vec!["app.acme.example".into()],
            deny: Vec::new(),
        },
        allowed: vec![Capability::Read],
        description: None,
        mode: None,
    };
    let governance = Governance::governed(
        Box::new(LocalPdp::new(polarity::evaluate_host)),
        audit,
        vec![grant],
        "license-expiry-continuity".into(),
        None,
    );
    let mut call = governance.begin("read_page", None, Some(READ));
    call.set_domain(Some("app.acme.example".into()));
    let gate = governance.authorize(
        &mut call,
        Some(GoverningResource::Resource("app.acme.example".into())),
        EffectiveMode::Enforce,
    );
    ensure!(
        matches!(gate, Gate::Proceed),
        "the governed read must remain authorized, got {gate:?}"
    );
    call.complete();

    let line = std::fs::read_to_string(audit_path)?;
    ensure!(
        line.lines().count() == 1,
        "expected exactly one audit record, got {line:?}"
    );
    Ok(serde_json::from_str(line.trim())?)
}

/// An org-signed bundle at a LOCAL path (the air-gap path) activates as the org's policy.
fn managed_activation_local() -> anyhow::Result<()> {
    let tmp = TempRoot::new("activate-local")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [7u8; 32];
    let bundle_path = tmp.path().join("policy.bundle");
    std::fs::write(
        &bundle_path,
        support::sign(&seed, 3, support::manifest("acme-corp")),
    )?;
    support::write_bootstrap(
        &paths.managed_bootstrap,
        &bundle_path.display().to_string(),
        &seed,
    )?;

    let reconciled = managed::activate(&paths, any_pattern)?
        .ok_or_else(|| anyhow!("bootstrap present but activate returned None"))?;
    let active = reconciled
        .active
        .ok_or_else(|| anyhow!("no active policy after activation"))?;
    ensure!(
        active.manifest.name == "acme-corp",
        "wrong manifest: {}",
        active.manifest.name
    );
    ensure!(active.seq == 3, "wrong seq: {}", active.seq);
    ensure!(
        matches!(reconciled.freshness, Freshness::Fresh),
        "expected Fresh"
    );
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
    ensure!(
        active.manifest.name == "acme-net",
        "wrong manifest: {}",
        active.manifest.name
    );
    ensure!(active.seq == 5, "wrong seq: {}", active.seq);
    ensure!(
        matches!(reconciled.freshness, Freshness::Fresh),
        "expected Fresh"
    );
    Ok(())
}

/// First boot, source unreachable, no cache: refuse to run unrestricted (fail closed).
fn fail_closed_cold_boot() -> anyhow::Result<()> {
    let tmp = TempRoot::new("fail-closed")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [9u8; 32];
    // Port 1 is not listening: an immediate connection refusal, no cache to fall back on.
    support::write_bootstrap(
        &paths.managed_bootstrap,
        "http://127.0.0.1:1/policy.bundle",
        &seed,
    )?;

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
        ensure!(
            matches!(r.freshness, Freshness::Fresh),
            "first activation should be Fresh"
        );
        ensure!(r.active.is_some(), "first activation should have a policy");
    } // server dropped -> the source is now unreachable

    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(
        matches!(
            r.freshness,
            Freshness::LastKnownGood(StaleReason::SourceUnreachable)
        ),
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
    support::write_bootstrap(
        &paths.managed_bootstrap,
        &bundle_path.display().to_string(),
        &seed,
    )?;

    std::fs::write(
        &bundle_path,
        support::sign(&seed, 9, support::manifest("acme-v9")),
    )?;
    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(
        r.active.as_ref().map(|v| v.seq) == Some(9),
        "seq 9 should activate"
    );

    // The source now serves an OLDER seq (a rollback attempt): refused, cache stands.
    std::fs::write(
        &bundle_path,
        support::sign(&seed, 3, support::manifest("acme-v3")),
    )?;
    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(
        matches!(
            r.freshness,
            Freshness::LastKnownGood(StaleReason::RollbackRefused)
        ),
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
    ensure!(
        r.active.as_ref().map(|v| v.seq) == Some(5),
        "seq 5 should activate first"
    );

    // The org publishes a newer policy; a re-resolve picks it up (bumped ETag => a fresh 200).
    server.set_bundle(support::sign(&seed, 6, support::manifest("acme-v6")));
    let r = managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    ensure!(
        matches!(r.freshness, Freshness::Fresh),
        "the update should be Fresh"
    );
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
    std::fs::write(
        &bundle_path,
        support::sign(&seed, 4, support::manifest("acme-live")),
    )?;
    support::write_bootstrap(
        &paths.managed_bootstrap,
        &bundle_path.display().to_string(),
        &seed,
    )?;

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

/// The admin's "did my policy propagate?" artifact, end to end (ADR-0055 Impl.8 / ADR-0056 D5): each
/// managed resolve writes the T2 status sidecar; the sidecar tracks a fresh publish, a live update,
/// and a guardian fallback when the source goes away.
fn sidecar_propagation() -> anyhow::Result<()> {
    let tmp = TempRoot::new("sidecar-prop")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [14u8; 32];
    let cache_path = paths
        .managed_cache
        .as_ref()
        .ok_or_else(|| anyhow!("no managed cache path under the temp root"))?
        .clone();
    let sidecar = status::sidecar_path(&cache_path);

    {
        let server = BundleServer::start(support::sign(&seed, 5, support::manifest("acme-prop")))?;
        support::write_bootstrap(&paths.managed_bootstrap, &server.url(), &seed)?;

        managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
        let s = status::read_sidecar(&sidecar)
            .ok_or_else(|| anyhow!("no sidecar after the first activate"))?;
        ensure!(
            s.freshness == "fresh",
            "expected fresh, got {}",
            s.freshness
        );
        ensure!(s.seq == Some(5), "expected seq 5, got {:?}", s.seq);

        // The org publishes a newer policy; a re-resolve propagates it into the sidecar.
        server.set_bundle(support::sign(&seed, 6, support::manifest("acme-prop-v6")));
        managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
        let s =
            status::read_sidecar(&sidecar).ok_or_else(|| anyhow!("no sidecar after the update"))?;
        ensure!(
            s.seq == Some(6),
            "expected seq 6 after the update, got {:?}",
            s.seq
        );
    } // server dropped -> the source is now unreachable

    managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    let s = status::read_sidecar(&sidecar)
        .ok_or_else(|| anyhow!("no sidecar after the source went away"))?;
    ensure!(
        s.freshness == "last_known_good",
        "expected last_known_good, got {}",
        s.freshness
    );
    ensure!(
        s.stale_reason.as_deref() == Some("source_unreachable"),
        "expected source_unreachable, got {:?}",
        s.stale_reason
    );
    ensure!(s.seq == Some(6), "seq should stay 6, got {:?}", s.seq);
    Ok(())
}

/// The Policy Passport rendered from a real activation (ADR-0055 D9 / ADR-0056 D5): an org-signed
/// bundle carrying presentation activates, the sidecar captures it, and the `explain`-tool renderer
/// speaks for the governed session (who governs, the policy version, the sacred line, and a contact).
fn passport_freshness() -> anyhow::Result<()> {
    use ghostlight_core::governance::manifest::bundle::{self, Contact, Presentation};

    let tmp = TempRoot::new("passport")?;
    let paths = GovernancePaths::under(tmp.path());
    let seed = [15u8; 32];
    let presentation = Presentation {
        org_name: Some("Acme Security".into()),
        rationale: None,
        contacts: vec![Contact {
            kind: "email".into(),
            value: "security@acme.example".into(),
            label: None,
        }],
    };
    let bundle_path = tmp.path().join("policy.bundle");
    std::fs::write(
        &bundle_path,
        bundle::sign_bundle(
            &seed,
            None,
            3,
            support::manifest("acme-passport"),
            Some(presentation),
        ),
    )?;
    support::write_bootstrap(
        &paths.managed_bootstrap,
        &bundle_path.display().to_string(),
        &seed,
    )?;

    managed::activate(&paths, any_pattern)?.ok_or_else(|| anyhow!("bootstrap"))?;
    let cache_path = paths
        .managed_cache
        .as_ref()
        .ok_or_else(|| anyhow!("no managed cache path under the temp root"))?;
    let status = status::read_sidecar(&status::sidecar_path(cache_path))
        .ok_or_else(|| anyhow!("no sidecar after activation"))?;

    let passport = ghostlight_core::governance::explain::managed_passport(&status);
    ensure!(
        passport.contains("Governed by: Acme Security."),
        "passport missing the org name: {passport}"
    );
    ensure!(
        passport.contains("Policy version 3,"),
        "passport missing the policy version: {passport}"
    );
    ensure!(
        passport.contains(
            "Sacred domains remain off-limits to automation under any policy, including this one."
        ),
        "passport missing the sacred-domains line: {passport}"
    );
    ensure!(
        passport.contains("security@acme.example"),
        "passport missing the contact: {passport}"
    );
    Ok(())
}
