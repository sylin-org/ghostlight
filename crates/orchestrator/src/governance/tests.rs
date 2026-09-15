use super::*;
use std::fs;
use std::path::PathBuf;

use ghostlight_bridge::browser::{RuntimeControlIntent, RuntimeControlState};

use super::{
    AuditRecord, AuthoringError, Capability, CapabilitySet, Decision, DenialAttention,
    GovernanceFacade, ReasonCode,
};
use crate::language::outcome::Observed;

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-1.0-{name}-{}.json",
        uuid::Uuid::new_v4()
    ))
}

fn policy(name: &str, grants: &str, config: &str) -> String {
    format!(r#"{{"schema":3,"name":"{name}","version":"1","grants":{grants},"config":{config}}}"#)
}

fn all_open_grant() -> &'static str {
    r#"[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]"#
}

/// The workspace forbids raw memory access everywhere except the one audited FFI crate
/// (ADR-0105 amendment 2026-08-24). The forbid itself cannot see across crates, so this
/// guard does.
#[test]
fn unsafe_stays_confined_to_the_audited_ffi_crate() {
    // The needles are assembled from pieces so this test's own source never contains a
    // matching sequence and can read every crate file, including itself.
    let keyword = ["uns", "afe"].concat();
    let needles = [
        format!("{keyword} {{"),
        format!("{keyword} fn"),
        format!("{keyword} impl"),
        format!("{keyword}_code"),
    ];
    fn walk(directory: &PathBuf, needles: &[String], hits: &mut Vec<String>) {
        for entry in fs::read_dir(directory).expect("crate source directory is readable") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, needles, hits);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = fs::read_to_string(&path).unwrap_or_default();
                for (number, line) in text.lines().enumerate() {
                    // Comment lines may legitimately name the keyword; code lines may not.
                    if line.trim_start().starts_with("// ") {
                        continue;
                    }
                    if needles.iter().any(|needle| line.contains(needle)) {
                        hits.push(format!("{}:{}", path.display(), number + 1));
                    }
                }
            }
        }
    }
    // The orchestrator crate lives at <workspace>/crates/orchestrator; its parent IS the
    // crates directory.
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&crates).expect("crates directory is readable") {
        let path = entry.expect("crate directory").path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name == "win-peer" || name == "target" {
            continue;
        }
        let source = path.join("src");
        if source.is_dir() {
            walk(&source, &needles, &mut offenders);
        }
    }
    assert!(
        offenders.is_empty(),
        "raw memory access must stay inside crates/win-peer; found at:\n{}",
        offenders.join("\n")
    );
}

fn snapshot_for(name: &str, source: impl AsRef<[u8]>) -> super::AuthoritySnapshot {
    let path = temporary(name);
    fs::write(&path, source).unwrap();
    let snapshot = GovernanceFacade::new(Some(path.clone()), None).snapshot();
    let _ = fs::remove_file(path);
    snapshot
}

#[test]
fn no_policy_allows_local_and_remote_http_destinations() {
    let facade = GovernanceFacade::new(None, None);
    let snapshot = facade.snapshot();
    assert!(snapshot.preserves_target_names());
    assert!(snapshot.authorize_tab_close().allowed);
    for url in [
        "https://example.com/",
        "http://localhost:3000/",
        "https://app.localhost:8443/",
        "http://127.0.0.1:5173/",
        "http://127.1.2.3/",
        "http://169.254.169.254/latest/",
        "https://[::1]/",
        "http://[fe80::1]/",
        "https://[::ffff:127.0.0.1]:9200/",
        "https://[::ffff:169.254.169.254]/latest/meta-data/",
        "https://[::127.0.0.1]/",
        "https://[::ffff:8.8.8.8]/",
    ] {
        for capability in Capability::ALL {
            assert!(
                snapshot.authorize_landing(capability, url).allowed,
                "{capability:?} at {url} needs no policy exception"
            );
        }
    }
}

#[test]
fn non_http_schemes_still_refuse_without_policy() {
    let snapshot = GovernanceFacade::new(None, None).snapshot();
    for url in [
        "chrome://extensions/",
        "chrome-extension://example/options.html",
        "file:///tmp/example.html",
        "data:text/html,example",
        "javascript:void(0)",
        "about:blank",
        "ftp://localhost/",
    ] {
        assert_eq!(
            snapshot.authorize_landing(Capability::Read, url).reason,
            ReasonCode::ProtectedHost,
            "{url} is outside the browser HTTP(S) contract"
        );
    }
}

#[test]
fn local_destinations_follow_authored_host_and_capability_rules() {
    let snapshot = snapshot_for(
        "local-destinations",
        policy(
            "local destinations",
            r#"[{"id":"development","hosts":{"allow":["localhost","*.localhost","127.0.0.1","169.254.169.254"],"deny":["admin.localhost","169.254.169.254"]},"allowed":["read","action"]}]"#,
            "[]",
        ),
    );
    for url in [
        "http://localhost:3000/",
        "https://app.localhost/",
        "http://127.0.0.1/",
    ] {
        assert!(snapshot.authorize_landing(Capability::Read, url).allowed);
        assert!(snapshot.authorize_landing(Capability::Action, url).allowed);
        let denied = snapshot.authorize_landing(Capability::Execute, url);
        assert!(!denied.allowed);
        assert_eq!(denied.reason, ReasonCode::CapabilityDenied);
        assert_eq!(
            snapshot.attribution(denied),
            Some(("user", Some("development")))
        );
    }
    for url in [
        "http://admin.localhost/",
        "http://169.254.169.254/latest/",
        "http://127.0.0.2/",
    ] {
        let denied = snapshot.authorize_landing(Capability::Read, url);
        assert!(!denied.allowed);
        assert_eq!(denied.reason, ReasonCode::HostDenied);
        assert!(denied.denial_id().is_some());
    }
}

#[test]
fn tab_close_policy_is_monotonic_across_authority_layers() {
    let local = temporary("local-tab-close");
    let managed = temporary("managed-tab-close");
    fs::write(
        &local,
        policy(
            "local",
            all_open_grant(),
            r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    fs::write(
        &managed,
        policy(
            "managed",
            all_open_grant(),
            r#"[{"key":"browser.tabs.allow_close","value":true,"level":"recommended"}]"#,
        ),
    )
    .unwrap();
    let snapshot = GovernanceFacade::new(Some(local.clone()), Some(managed.clone())).snapshot();
    assert_eq!(
        snapshot.authorize_tab_close().reason,
        ReasonCode::TabCloseDenied
    );
    assert!(snapshot.authorize_capability(Capability::Action).allowed);
    let _ = fs::remove_file(local);
    let _ = fs::remove_file(managed);
}

#[test]
fn target_name_preservation_is_default_on_and_monotonic_across_layers() {
    let local = temporary("local-target-names");
    let managed = temporary("managed-target-names");
    fs::write(
        &local,
        policy(
            "local",
            all_open_grant(),
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    fs::write(
        &managed,
        policy(
            "managed",
            all_open_grant(),
            r#"[{"key":"privacy.preserve_target_names","value":true,"level":"recommended"}]"#,
        ),
    )
    .unwrap();

    let snapshot = GovernanceFacade::new(Some(local.clone()), Some(managed.clone())).snapshot();
    assert!(!snapshot.preserves_target_names());

    fs::write(
        &local,
        policy(
            "local",
            all_open_grant(),
            r#"[{"key":"privacy.preserve_target_names","value":true,"level":"recommended"}]"#,
        ),
    )
    .unwrap();
    fs::write(
        &managed,
        policy(
            "managed",
            all_open_grant(),
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let snapshot = GovernanceFacade::new(Some(local.clone()), Some(managed.clone())).snapshot();
    assert!(!snapshot.preserves_target_names());

    let _ = fs::remove_file(local);
    let _ = fs::remove_file(managed);
}

#[test]
fn configured_policy_bounds_hosts_and_capabilities() {
    let path = temporary("configured-boundary");
    fs::write(
        &path,
        policy(
            "test",
            r#"[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read"]}]"#,
            "[]",
        ),
    )
    .unwrap();
    let snapshot = GovernanceFacade::new(Some(path.clone()), None).snapshot();
    fs::remove_file(path).unwrap();
    assert!(
        snapshot
            .authorize_landing(Capability::Read, "https://example.com")
            .allowed
    );
    assert_eq!(
        snapshot
            .authorize_landing(Capability::Action, "https://example.com")
            .reason,
        ReasonCode::CapabilityDenied
    );
    assert_eq!(
        snapshot
            .authorize_landing(Capability::Read, "https://example.org")
            .reason,
        ReasonCode::HostDenied
    );
    for url in [
        "http://localhost:3000/",
        "http://127.0.0.1/",
        "http://[::1]/",
        "http://169.254.169.254/",
    ] {
        assert_eq!(
            snapshot.authorize_landing(Capability::Read, url).reason,
            ReasonCode::HostDenied,
            "configured policy still bounds {url}"
        );
    }
}

#[test]
fn authority_requires_every_independent_capability_in_a_compound_set() {
    let path = temporary("compound-capabilities");
    fs::write(
            &path,
            policy(
                "compound",
                r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]},{"id":"write","hosts":{"allow":["*"]},"allowed":["write"]}]"#,
                "[]",
            ),
        )
        .unwrap();
    let snapshot = GovernanceFacade::new(Some(path.clone()), None).snapshot();
    assert!(
        snapshot
            .authorize_requirements(CapabilitySet::READ.union(CapabilitySet::WRITE))
            .allowed
    );
    assert_eq!(
        snapshot
            .authorize_requirements(
                CapabilitySet::READ
                    .union(CapabilitySet::WRITE)
                    .union(CapabilitySet::ACTION),
            )
            .reason,
        ReasonCode::CapabilityDenied
    );
    assert!(
        snapshot
            .authorize_requirements(CapabilitySet::EMPTY)
            .allowed
    );
    let _ = fs::remove_file(path);
}

#[test]
fn host_specificity_denies_ties_but_more_specific_allows_win() {
    let exact_allow = snapshot_for(
        "exact-allow",
        policy(
            "exact allow",
            r#"[{"id":"g","hosts":{"allow":["admin.example.com"],"deny":["*.example.com"]},"allowed":["read"]}]"#,
            "[]",
        ),
    );
    assert!(
        exact_allow
            .authorize_landing(Capability::Read, "https://admin.example.com")
            .allowed
    );

    let exact_tie = snapshot_for(
        "exact-tie",
        policy(
            "exact tie",
            r#"[{"id":"g","hosts":{"allow":["admin.example.com"],"deny":["admin.example.com"]},"allowed":["read"]}]"#,
            "[]",
        ),
    );
    let denied = exact_tie.authorize_landing(Capability::Read, "https://admin.example.com");
    assert!(!denied.allowed);
    assert_eq!(denied.policy_rule(), Some("denied_host"));

    let longer_deny = snapshot_for(
        "longer-deny",
        policy(
            "longer deny",
            r#"[{"id":"g","hosts":{"allow":["*.example.com"],"deny":["*.secure.example.com"]},"allowed":["read"]}]"#,
            "[]",
        ),
    );
    assert!(
        !longer_deny
            .authorize_landing(Capability::Read, "https://a.secure.example.com")
            .allowed
    );
}

#[test]
fn a_grant_deny_only_shrinks_that_grant_and_search_continues_for_an_admission() {
    let snapshot = snapshot_for(
        "grant-order",
        policy(
            "grant order",
            r#"[{"id":"carved","hosts":{"allow":["*.example.com"],"deny":["admin.example.com"]},"allowed":["read"]},{"id":"admin","hosts":{"allow":["admin.example.com"]},"allowed":["read"]}]"#,
            "[]",
        ),
    );
    assert!(
        snapshot
            .authorize_landing(Capability::Read, "https://admin.example.com")
            .allowed
    );

    let capabilities = snapshot_for(
        "grant-capability-order",
        policy(
            "grant capability order",
            r#"[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"action","hosts":{"allow":["example.com"]},"allowed":["action"]}]"#,
            "[]",
        ),
    );
    assert!(
        capabilities
            .authorize_landing(Capability::Action, "https://example.com")
            .allowed,
        "the first host match is not a refusal when a later grant admits the full set"
    );
}

#[test]
fn first_applicable_denial_is_stably_attributed() {
    let snapshot = snapshot_for(
        "first-denial",
        policy(
            "first denial",
            r#"[{"id":"first","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"second","hosts":{"allow":["other.example"]},"allowed":["action"]}]"#,
            "[]",
        ),
    );
    let first = snapshot.authorize_landing(Capability::Action, "https://example.com");
    let second = snapshot.authorize_landing(Capability::Action, "https://example.com");
    assert!(!first.allowed);
    assert_eq!(first.policy_rule(), Some("capability"));
    assert_eq!(snapshot.attribution(first), Some(("user", Some("first"))));
    assert_eq!(first.denial_id(), second.denial_id());

    let record = AuditRecord::now(
        "invocation_policy",
        "workspace_policy",
        "browser_click",
        Capability::Action,
        snapshot.id(),
        first,
        "blocked",
        "none",
        &crate::language::outcome::Refusal::AuthorityBlocked {
            reason: crate::language::outcome::BlockedReason::Capability,
            host: None,
        }
        .audit(),
        0,
    )
    .with_policy(&snapshot, first);
    assert_eq!(record.policy_tier.as_deref(), Some("user"));
    assert_eq!(record.grant_id.as_deref(), Some("first"));
    assert_eq!(record.policy_rule.as_deref(), Some("capability"));
    assert_eq!(record.policy_mode.as_deref(), Some("enforce"));
    assert_eq!(record.denial_id, first.denial_id());

    let changed = snapshot_for(
        "changed-denial",
        policy(
            "changed denial",
            r#"[{"id":"renamed","hosts":{"allow":["example.com"]},"allowed":["read"]}]"#,
            "[]",
        ),
    )
    .authorize_landing(Capability::Action, "https://example.com");
    assert_ne!(first.denial_id(), changed.denial_id());
}

#[test]
fn observe_shadows_ordinary_denials_but_never_protected_resources() {
    let snapshot = snapshot_for(
        "observe",
        r#"{"schema":3,"name":"observe","version":"1","mode":"observe","grants":[]}"#,
    );
    for url in [
        "https://example.com",
        "http://localhost:3000",
        "http://127.0.0.1",
        "http://169.254.169.254",
    ] {
        let ordinary = snapshot.authorize_landing(Capability::Read, url);
        assert!(ordinary.allowed);
        assert!(ordinary.observed);
        assert_eq!(ordinary.reason, ReasonCode::HostDenied);
        assert!(ordinary.denial_id().is_some());
    }

    let protected = snapshot.authorize_landing(Capability::Read, "chrome://extensions");
    assert!(!protected.allowed);
    assert!(!protected.observed);
    assert_eq!(protected.reason, ReasonCode::ProtectedHost);

    let sacred = snapshot_for(
        "observe-sacred",
        r#"{"schema":3,"name":"observe sacred","version":"1","mode":"observe","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[{"key":"content.security.sacred_domains","value":["localhost"],"level":"mandatory"}]}"#,
    );
    assert!(
        !sacred
            .authorize_landing(Capability::Read, "http://localhost:3000")
            .allowed
    );
}

#[test]
fn strictest_layer_mode_wins_and_snapshots_have_deterministic_identity() {
    let managed = temporary("mode-managed");
    let local = temporary("mode-local");
    fs::write(
        &managed,
        r#"{"schema":3,"name":"managed","version":"1","mode":"observe","grants":[]}"#,
    )
    .unwrap();
    fs::write(
            &local,
            r#"{"schema":3,"name":"local","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
    let facade = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()));
    let first = facade.snapshot();
    let same = facade.snapshot();
    let denied = first.authorize_landing(Capability::Read, "https://example.com");
    assert!(
        !denied.allowed,
        "the enforcing local tier prevents shadow admission"
    );
    assert_eq!(first.id(), same.id());

    fs::write(
            &local,
            r#"{"schema":3,"name":"local","version":"2","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
    let changed = facade.snapshot();
    assert_ne!(first.id(), changed.id());
    let _ = fs::remove_file(managed);
    let _ = fs::remove_file(local);
}

#[test]
fn maintained_policy_examples_match_the_schema_three_decoder() {
    for (name, source) in [
        (
            "research-read-only",
            include_str!("../../../../examples/research-read-only.json"),
        ),
        (
            "qa-staging",
            include_str!("../../../../examples/qa-staging.json"),
        ),
        (
            "enterprise-healthcare",
            include_str!("../../../../examples/enterprise-healthcare.json"),
        ),
        (
            "developer-unrestricted",
            include_str!("../../../../examples/developer-unrestricted.json"),
        ),
        (
            "developer-observe",
            include_str!("../../../../examples/developer-observe.json"),
        ),
        (
            "dev-live-test",
            include_str!("../../../../examples/dev-live-test.json"),
        ),
        (
            "demo-policy",
            include_str!("../../../../examples/demo-policy.json"),
        ),
        (
            "scripting-disabled",
            include_str!("../../../../examples/scripting-disabled.json"),
        ),
        (
            "personal-starter",
            include_str!("../../../../examples/personal-starter.json"),
        ),
        (
            "personal-everywhere-except",
            include_str!("../../../../examples/personal-everywhere-except.json"),
        ),
        (
            "no-page-code",
            include_str!("../../../../examples/no-page-code.json"),
        ),
        (
            "organization-support",
            include_str!("../../../../examples/organization-support.json"),
        ),
        (
            "organization-locked-fleet",
            include_str!("../../../../examples/organization-locked-fleet.json"),
        ),
    ] {
        let path = temporary(name);
        fs::write(&path, source).unwrap();
        assert!(
            super::read_policy(&path).is_ok(),
            "{name} must remain a valid 1.0 policy"
        );
        let _ = fs::remove_file(path);
    }
}

#[test]
fn authoring_validates_before_it_replaces_and_never_leaves_the_product_failing_closed() {
    let path = temporary("authored-policy");
    let facade = GovernanceFacade::owning_user_policy(path.clone(), None);
    // A machine that has never authored one is open, not failing closed.
    assert!(
        facade
            .snapshot()
            .authorize_capability(Capability::Execute)
            .allowed
    );

    let good = r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#;
    facade.apply_user_policy(good).unwrap();
    let applied = facade.snapshot();
    assert!(
        applied
            .authorize_landing(CapabilitySet::READ, "https://example.com")
            .allowed
    );
    assert!(
        !applied
            .authorize_landing(CapabilitySet::EXECUTE, "https://example.com")
            .allowed
    );

    // A rejected document leaves both the file and the authority exactly as they were.
    let error = facade.apply_user_policy("{\"schema\":3,").unwrap_err();
    assert!(matches!(error, AuthoringError::Invalid(_)));
    assert_eq!(fs::read_to_string(&path).unwrap(), good);
    assert!(
        facade
            .snapshot()
            .authorize_landing(CapabilitySet::READ, "https://example.com")
            .allowed
    );

    // The organization switch is not something a user layer may author for itself.
    let overreach = r#"{"schema":3,"name":"mine","version":"2","grants":[],"config":[{"key":"policy.user.enabled","value":true,"level":"mandatory"}]}"#;
    assert!(matches!(
        facade.apply_user_policy(overreach).unwrap_err(),
        AuthoringError::OrganizationOnlySetting(_)
    ));

    facade.remove_user_policy().unwrap();
    assert!(!path.exists());
    assert!(
        facade
            .snapshot()
            .authorize_capability(Capability::Execute)
            .allowed
    );
    // Removing what is already gone is not a failure.
    facade.remove_user_policy().unwrap();
}

#[test]
fn a_policy_file_ghostlight_does_not_own_is_never_written_back() {
    let path = temporary("foreign-policy");
    fs::write(
        &path,
        br#"{"schema":3,"name":"theirs","version":"1","grants":[]}"#,
    )
    .unwrap();
    let facade = GovernanceFacade::new(Some(path.clone()), None);
    assert!(matches!(
        facade
            .apply_user_policy(r#"{"schema":3,"name":"mine","version":"1","grants":[]}"#)
            .unwrap_err(),
        AuthoringError::NotOwned
    ));
    assert!(matches!(
        facade.remove_user_policy().unwrap_err(),
        AuthoringError::NotOwned
    ));
    assert!(path.exists());
    let _ = fs::remove_file(path);
}

#[test]
fn a_candidate_policy_is_decided_under_the_organization_ceiling_without_being_applied() {
    let managed = temporary("preview-managed");
    let owned = temporary("preview-user");
    fs::write(
            &managed,
            br#"{"schema":3,"name":"org","version":"1","grants":[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}]}"#,
        )
        .unwrap();
    let facade = GovernanceFacade::owning_user_policy(owned.clone(), Some(managed.clone()));

    let decided = facade
            .candidate_snapshot(
                r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"wide","hosts":{"allow":["*"]},"allowed":["read","action","write"]}]}"#,
            )
            .unwrap();
    assert_eq!(decided.rules, 1);
    let candidate = decided.snapshot;
    // The candidate asks for more than the organization allows, so the ceiling still wins.
    assert!(
        candidate
            .authorize_landing(CapabilitySet::READ, "https://example.com")
            .allowed
    );
    assert!(
        !candidate
            .authorize_landing(CapabilitySet::WRITE, "https://example.com")
            .allowed
    );
    assert!(
        !candidate
            .authorize_landing(CapabilitySet::READ, "https://elsewhere.test")
            .allowed
    );
    // Previewing writes nothing.
    assert!(!owned.exists());
    assert!(facade.candidate_snapshot("not json").is_err());

    let _ = fs::remove_file(managed);
}

#[test]
fn an_organization_may_switch_off_user_authoring_without_widening_authority() {
    let managed = temporary("authoring-managed");
    let local = temporary("authoring-local");
    fs::write(
            &managed,
            br#"{"schema":3,"name":"org","version":"1","grants":[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}],"config":[{"key":"policy.user.enabled","value":false,"level":"mandatory"}]}"#,
        )
        .unwrap();
    fs::write(
            &local,
            br#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"narrow","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        )
        .unwrap();

    let facade = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()));
    assert!(!facade.user_authoring_allowed());
    // The switch gates authoring only. An existing user layer keeps subtracting, because
    // ignoring it would restore authority the organization never granted back.
    let snapshot = facade.snapshot();
    assert!(
        snapshot
            .authorize_landing(CapabilitySet::READ, "https://example.com")
            .allowed
    );
    assert!(
        !snapshot
            .authorize_landing(CapabilitySet::ACTION, "https://example.com")
            .allowed
    );

    assert!(GovernanceFacade::new(Some(local.clone()), None).user_authoring_allowed());
    let _ = fs::remove_file(managed);
    let _ = fs::remove_file(local);
}

#[test]
fn invalid_managed_authority_fails_closed() {
    let path = temporary("invalid-managed");
    fs::write(&path, br#"{"version":1,"managed":false}"#).unwrap();
    let facade = GovernanceFacade::new(None, Some(path.clone()));
    let snapshot = facade.snapshot();
    assert_eq!(
        snapshot.authorize_capability(Capability::Read).reason,
        ReasonCode::InvalidAuthority
    );
    let _ = fs::remove_file(path);
}

#[test]
fn an_unreachable_managed_authority_fails_closed_from_cold_start() {
    let path = temporary("missing-managed");
    let _ = fs::remove_file(&path);
    let facade = GovernanceFacade::new(None, Some(path));
    let snapshot = facade.snapshot();
    assert_eq!(
        snapshot.authorize_capability(Capability::Read).reason,
        ReasonCode::InvalidAuthority
    );
}

#[test]
fn snapshot_does_not_change_when_policy_file_changes() {
    let path = temporary("immutable");
    fs::write(
        &path,
        policy(
            "read",
            r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]"#,
            "[]",
        ),
    )
    .unwrap();
    let facade = GovernanceFacade::new(Some(path.clone()), None);
    let first = facade.snapshot();
    fs::write(
        &path,
        policy(
            "action",
            r#"[{"id":"action","hosts":{"allow":["*"]},"allowed":["action"]}]"#,
            "[]",
        ),
    )
    .unwrap();
    assert!(first.authorize_capability(Capability::Read).allowed);
    assert!(!first.authorize_capability(Capability::Action).allowed);
    let second = facade.snapshot();
    assert!(!second.authorize_capability(Capability::Read).allowed);
    assert!(second.authorize_capability(Capability::Action).allowed);
    let _ = fs::remove_file(path);
}

#[test]
fn invalid_reload_keeps_last_valid_authority_until_a_valid_replacement_arrives() {
    let path = temporary("last-known-good");
    fs::write(
        &path,
        policy(
            "read",
            r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]"#,
            "[]",
        ),
    )
    .unwrap();
    let facade = GovernanceFacade::new(Some(path.clone()), None);
    assert!(
        facade
            .snapshot()
            .authorize_capability(Capability::Read)
            .allowed
    );

    fs::write(&path, "{half-written").unwrap();
    let retained = facade.snapshot();
    assert!(retained.authorize_capability(Capability::Read).allowed);
    assert!(!retained.authorize_capability(Capability::Action).allowed);
    let retained_diagnostics = facade.diagnostics();
    assert!(retained_diagnostics.local_policy_active);
    assert!(!retained_diagnostics.local_policy_valid);

    fs::write(
        &path,
        policy(
            "action",
            r#"[{"id":"action","hosts":{"allow":["*"]},"allowed":["action"]}]"#,
            "[]",
        ),
    )
    .unwrap();
    let replaced = facade.snapshot();
    assert!(!replaced.authorize_capability(Capability::Read).allowed);
    assert!(replaced.authorize_capability(Capability::Action).allowed);
    let replaced_diagnostics = facade.diagnostics();
    assert!(replaced_diagnostics.local_policy_active);
    assert!(replaced_diagnostics.local_policy_valid);
    let _ = fs::remove_file(path);
}

fn sample_record() -> AuditRecord {
    AuditRecord::now(
        "invocation_x",
        "workspace_x",
        "browser_fill_form",
        CapabilitySet::READ.union(CapabilitySet::WRITE),
        "authority_x",
        Decision::permitted(),
        "succeeded",
        "applied",
        &crate::language::outcome::Outcome::TextRead {
            words: 3,
            host: None,
        }
        .audit(),
        1200,
    )
}

/// Every key anywhere in the record, so a payload cannot hide one level down.
fn keys(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, nested) in object {
                found.push(key.clone());
                keys(nested, found);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                keys(item, found);
            }
        }
        _ => {}
    }
}

#[test]
fn audit_record_has_no_payload_fields() {
    let record = sample_record().with_observation(Observed {
        host: Some("example.com".into()),
        readiness: Some("complete".into()),
        count: Some(3),
        width: Some(1280),
        height: Some(720),
    });
    let value = serde_json::to_value(record).unwrap();
    let mut found = Vec::new();
    keys(&value, &mut found);
    // The walk must reach nested keys, or it would pass any payload hidden one level down.
    assert!(found.contains(&"observed".to_owned()));
    assert!(
        found.contains(&"host".to_owned()),
        "the walk stops at the top"
    );
    for forbidden in [
        "url",
        "text",
        "content",
        "selector",
        "value",
        "screenshot",
        "dialog",
        "path",
        "query",
    ] {
        assert!(
            !found.contains(&forbidden.to_owned()),
            "the record grew a {forbidden} field"
        );
    }
}

#[test]
fn new_audit_records_serialize_the_complete_set_without_a_highest_capability() {
    let value = serde_json::to_value(sample_record()).unwrap();
    assert_eq!(value["capabilities"], serde_json::json!(["read", "write"]));
    assert!(value.get("capability").is_none());

    let historical: AuditRecord = serde_json::from_value(serde_json::json!({
        "timestamp_ms": 1,
        "invocation": "invocation_old",
        "workspace": "workspace_old",
        "tool": "browser_read",
        "capability": "read",
        "authority": "authority_old",
        "allowed": true,
        "reason": "permitted",
        "status": "succeeded",
        "effect": "none"
    }))
    .expect("historical audit record remains readable");
    assert_eq!(historical.requirements(), CapabilitySet::READ);
}

#[test]
fn audit_reads_legacy_failure_payloads_without_reemitting_them() {
    for legacy in [
        serde_json::json!({"reason":"browser_primitive_failed","detail":"PRIVATE_EXCEPTION"}),
        serde_json::json!({"steps":[{"result":{"text":"PRIVATE_READ"}}]}),
        serde_json::json!({"reason":"PRIVATE_REASON"}),
        serde_json::json!(["PRIVATE_ARRAY"]),
    ] {
        let mut encoded = serde_json::to_value(sample_record()).unwrap();
        encoded["refusal_facts"] = legacy;
        let record: AuditRecord =
            serde_json::from_value(encoded).expect("old history stays readable");
        assert!(!serde_json::to_string(&record).unwrap().contains("PRIVATE_"));
        assert_eq!(record.invocation, "invocation_x");
        assert_eq!(
            record.requirements(),
            CapabilitySet::READ.union(CapabilitySet::WRITE)
        );
    }
}

#[test]
fn typed_audit_failures_round_trip_with_policy_and_measurements() {
    use crate::language::outcome::Refusal;
    let record = AuditRecord::now(
        "invocation_failure",
        "workspace_failure",
        "browser_execute",
        Capability::Execute,
        "authority_failure",
        Decision::permitted(),
        "unknown",
        "unknown",
        &Refusal::DeadlineExpired {
            before_dispatch: false,
        }
        .audit(),
        123,
    )
    .with_observation(Observed {
        host: Some("example.com".into()),
        ..Observed::default()
    });
    let encoded = serde_json::to_string(&record).unwrap();
    assert_eq!(
        serde_json::from_str::<AuditRecord>(&encoded).unwrap(),
        record
    );
    assert_eq!(
        record.refusal_facts,
        Some(crate::language::audit::AuditRefusal::DeadlineExpired {
            before_dispatch: false
        })
    );
}

/// The record has exactly one URL-shaped field and it is a host.
///
/// What the executor puts in it is guarded where it is extracted, in `work`; this pins that
/// there is nowhere else for the rest of a URL to travel.
#[test]
fn an_observation_has_one_place_for_a_host_and_none_for_the_rest_of_a_url() {
    let record = sample_record().with_observation(Observed {
        host: Some("example.com".into()),
        readiness: Some("complete".into()),
        count: Some(3),
        width: Some(1280),
        height: Some(720),
    });
    let encoded = serde_json::to_value(&record).unwrap();
    let observed = encoded["observed"].as_object().unwrap();
    assert_eq!(observed["host"], "example.com");
    let mut text: Vec<&str> = observed
        .iter()
        .filter(|(_, value)| value.is_string())
        .map(|(key, _)| key.as_str())
        .collect();
    text.sort_unstable();
    assert_eq!(
        text,
        ["host", "readiness"],
        "an observation grew another field a URL could travel in"
    );
}

#[test]
fn naming_a_channel_is_how_a_layer_takes_control_of_it() {
    use ghostlight_bridge::service::IntakeChannel;

    let unconfigured = GovernanceFacade::new(None, None);
    for channel in [IntakeChannel::Mcp, IntakeChannel::Cli] {
        assert!(
            unconfigured.admits_channel(channel).allowed,
            "an unconfigured Ghostlight must admit every intake"
        );
    }

    let path = temporary("channel-empty");
    fs::write(
        &path,
        policy(
            "channel-off",
            "[]",
            r#"[{"key":"channels.cli.enabled","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let empty = GovernanceFacade::new(Some(path.clone()), None);
    assert_eq!(
        empty.admits_channel(IntakeChannel::Cli).reason,
        ReasonCode::ChannelDenied
    );
    assert!(
        empty.admits_channel(IntakeChannel::Mcp).allowed,
        "a layer restricts only the channels it names"
    );
    let _ = fs::remove_file(path);

    let path = temporary("channel-false");
    fs::write(
            &path,
            policy(
                "channels",
                "[]",
                r#"[{"key":"channels.cli.enabled","value":false,"level":"mandatory"},{"key":"channels.mcp.enabled","value":true,"level":"recommended"}]"#,
            ),
        )
        .unwrap();
    let explicit = GovernanceFacade::new(Some(path.clone()), None);
    assert!(!explicit.admits_channel(IntakeChannel::Cli).allowed);
    assert!(explicit.admits_channel(IntakeChannel::Mcp).allowed);
    let _ = fs::remove_file(path);
}

#[test]
fn a_local_layer_cannot_readmit_a_channel_managed_authority_refused() {
    use ghostlight_bridge::service::IntakeChannel;

    let managed = temporary("channel-managed");
    fs::write(
        &managed,
        policy(
            "managed",
            "[]",
            r#"[{"key":"channels.cli.enabled","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let local = temporary("channel-local");
    fs::write(
        &local,
        policy(
            "local",
            "[]",
            r#"[{"key":"channels.cli.enabled","value":true,"level":"recommended"}]"#,
        ),
    )
    .unwrap();

    let facade = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()));
    assert_eq!(
        facade.admits_channel(IntakeChannel::Cli).reason,
        ReasonCode::ChannelDenied,
        "layers compose by intersection; local cannot hand access back"
    );
    // The negative control: the same local layer alone does admit it, so the refusal above
    // is the managed layer's doing and not an accident of parsing.
    let alone = GovernanceFacade::new(Some(local.clone()), None);
    assert!(alone.admits_channel(IntakeChannel::Cli).allowed);
    let _ = fs::remove_file(managed);
    let _ = fs::remove_file(local);
}

#[test]
fn an_unknown_channel_name_is_a_typo_not_a_silent_pass() {
    use ghostlight_bridge::service::IntakeChannel;

    let path = temporary("channel-typo");
    fs::write(
            &path,
            r#"{"schema":3,"name":"typo","version":"1","grants":[],"config":[{"key":"channels.cli-tool.enabled","value":false,"level":"mandatory"}]}"#,
        )
        .unwrap();
    let facade = GovernanceFacade::new(Some(path.clone()), None);
    assert_eq!(
        facade.admits_channel(IntakeChannel::Cli).reason,
        ReasonCode::InvalidAuthority,
        "a misspelled channel must fail closed rather than restrict nothing"
    );
    let _ = fs::remove_file(path);
}

/// A pause denies before any effect and a resume restores work, without anything waiting.
///
/// ADR-0126 Decision 4 chose refusal over a held caller, so this is the whole mechanism: the
/// final boundary answers deny while held and allow once resumed. There is no queue to drain
/// and no deadline to reconcile.
#[test]
fn pause_prevents_the_next_browser_effect_and_resume_restores_it() {
    let facade = GovernanceFacade::new(None, None);
    assert!(facade.runtime_decision().allowed);

    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::Hold),
        RuntimeControlState::Held
    );
    assert_eq!(facade.runtime_decision().reason, ReasonCode::RuntimeHold);
    assert!(!facade.runtime_decision().allowed);
    // Idempotent: pausing twice is still one paused state.
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::Hold),
        RuntimeControlState::Held
    );

    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::Resume),
        RuntimeControlState::Active
    );
    assert!(facade.runtime_decision().allowed);
}

/// Stop is terminal and idempotent, and a resume cannot undo it.
///
/// Only an explicit new session leaves the ended state, so a model that retries after the stop
/// directive gets the same refusal rather than quietly resuming the work a person interrupted.
#[test]
fn stop_is_terminal_and_idempotent() {
    let facade = GovernanceFacade::new(None, None);
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::EndSession),
        RuntimeControlState::Ended
    );
    assert_eq!(facade.runtime_decision().reason, ReasonCode::SessionEnded);
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::EndSession),
        RuntimeControlState::Ended
    );
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::Resume),
        RuntimeControlState::Ended,
        "a resume must not undo a stop"
    );
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::StartSession),
        RuntimeControlState::Active
    );
}

/// A policy attention hold is its own state, not the person's pause.
#[test]
fn attention_stays_distinct_from_a_human_pause() {
    let facade = GovernanceFacade::new(None, None);
    facade.apply_runtime_intent(RuntimeControlIntent::Hold);
    assert_eq!(facade.runtime_state(), RuntimeControlState::Held);
    assert_eq!(facade.runtime_decision().reason, ReasonCode::RuntimeHold);
    assert_ne!(
        facade.runtime_decision().reason,
        ReasonCode::RuntimeAttention
    );
}

#[test]
fn runtime_control_file_is_checked_at_each_final_boundary() {
    let path = temporary("runtime-control");
    fs::write(&path, "hold").unwrap();
    let facade = GovernanceFacade::new(None, None).with_runtime_control_file(Some(path.clone()));
    assert_eq!(facade.runtime_decision().reason, ReasonCode::RuntimeHold);
    fs::write(&path, "attention").unwrap();
    assert_eq!(
        facade.runtime_decision().reason,
        ReasonCode::RuntimeAttention
    );
    fs::write(&path, "end_session").unwrap();
    assert_eq!(facade.runtime_decision().reason, ReasonCode::SessionEnded);
    fs::write(&path, "active").unwrap();
    assert!(facade.runtime_decision().allowed);
    let _ = fs::remove_file(path);
}

#[test]
fn human_control_intents_are_authoritative_and_end_is_terminal() {
    let facade = GovernanceFacade::new(None, None);
    assert_eq!(facade.runtime_state(), RuntimeControlState::Active);
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::ToggleHold),
        RuntimeControlState::Held
    );
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::ToggleHold),
        RuntimeControlState::Active
    );
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::EndSession),
        RuntimeControlState::Ended
    );
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::Resume),
        RuntimeControlState::Ended
    );
    assert_eq!(
        facade.apply_runtime_intent(RuntimeControlIntent::StartSession),
        RuntimeControlState::Active
    );
}

#[test]
fn repeated_matching_denials_require_attention_per_workspace() {
    let mut attention = DenialAttention::default();
    let denied = Decision::deny(ReasonCode::CapabilityDenied);

    assert!(!attention.record("workspace_one", denied, 1_000));
    assert!(!attention.record("workspace_two", denied, 10_000));
    assert!(!attention.record("workspace_one", denied, 30_000));
    assert!(attention.record("workspace_one", denied, 60_000));
    assert!(!attention.record("workspace_one", denied, 61_000));
    assert!(!attention.record("workspace_two", denied, 61_000));
}

#[test]
fn five_distinct_enforced_denials_require_attention_and_old_attempts_expire() {
    let mut attention = DenialAttention::default();
    for (index, reason) in [
        ReasonCode::CapabilityDenied,
        ReasonCode::TabCloseDenied,
        ReasonCode::HostDenied,
        ReasonCode::ProtectedHost,
    ]
    .into_iter()
    .enumerate()
    {
        assert!(!attention.record("workspace", Decision::deny(reason), index as u64 * 1_000));
    }
    assert!(attention.record(
        "workspace",
        Decision::deny(ReasonCode::InvalidAuthority),
        4_000
    ));

    assert!(!attention.record(
        "expired",
        Decision::deny(ReasonCode::CapabilityDenied),
        1_000
    ));
    assert!(!attention.record(
        "expired",
        Decision::deny(ReasonCode::CapabilityDenied),
        122_000
    ));
}
