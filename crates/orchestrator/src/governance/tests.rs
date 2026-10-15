use super::*;
use std::fs;
use std::path::PathBuf;

use crate::browser::adapter::{RuntimeControlIntent, RuntimeControlState};

use super::{AuthoringError, Capability, CapabilitySet, GovernanceFacade, ReasonCode};
use crate::audit::AuditRecord;

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-1.0-{name}-{}.json",
        uuid::Uuid::new_v4()
    ))
}

fn policy(name: &str, grants: &str, config: &str) -> String {
    format!(r#"{{"schema":4,"name":"{name}","version":"1","grants":{grants},"config":{config}}}"#)
}

fn all_open_grant() -> &'static str {
    r#"[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]"#
}

#[test]
fn screenshot_permission_is_binary_default_on_and_tighten_only() {
    assert!(
        GovernanceFacade::new(None, None)
            .snapshot()
            .authorize_screenshot()
            .allowed
    );
    let local = temporary("screenshot-local");
    let managed = temporary("screenshot-managed");
    for (local_value, managed_value, author) in [(true, false, "managed"), (false, true, "user")] {
        for (path, value) in [(&local, local_value), (&managed, managed_value)] {
            fs::write(path, policy("capture", all_open_grant(), &format!(
                r#"[{{"key":"{SCREENSHOTS_ENABLED_KEY}","value":{value},"level":"mandatory"}}]"#,
            ))).unwrap();
        }
        let snapshot = GovernanceFacade::new(Some(local.clone()), Some(managed.clone())).snapshot();
        let decision = snapshot.authorize_screenshot();
        assert!(!decision.allowed);
        assert_eq!(decision.reason, ReasonCode::ScreenshotDenied);
        assert_eq!(snapshot.attribution(decision), Some((author, None)));
        assert!(snapshot.authorize_capability(Capability::Read).allowed);
    }
    assert!(manifest::parse(
        &policy(
            "invalid",
            all_open_grant(),
            &format!(
                r#"[{{"key":"{SCREENSHOTS_ENABLED_KEY}","value":"masked","level":"mandatory"}}]"#,
            )
        ),
        "test"
    )
    .is_err());
    fs::remove_file(local).unwrap();
    fs::remove_file(managed).unwrap();
}

/// The workspace forbids raw memory access everywhere except the two audited crates (ADR-0105
/// amendment 2026-08-24, ADR-0199 Decision 3). Every workspace member but `crates/win-peer`
/// (which allows it) and `crates/relay` (which denies it, so only the `os` module's expectation
/// lifts it) inherits that forbid, and this test proves it from the manifests. The compiler then
/// rejects any such code in an inheriting crate, and no in-source allow can override an
/// inherited forbid.
#[test]
fn unsafe_stays_confined_to_the_audited_ffi_crate() {
    use toml_edit::DocumentMut;

    /// Each audited crate and the level its own lint table sets for `unsafe_code`.
    const AUDITED: [(&str, &str); 2] = [("crates/relay", "deny"), ("crates/win-peer", "allow")];
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |path: PathBuf| -> DocumentMut {
        fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
            .parse()
            .unwrap_or_else(|error| panic!("{} parses: {error}", path.display()))
    };

    let workspace = read(root.join("Cargo.toml"));
    assert_eq!(
        workspace["workspace"]["lints"]["rust"]["unsafe_code"].as_str(),
        Some("forbid"),
        "the workspace lint table must forbid unsafe_code"
    );
    let members: Vec<&str> = workspace["workspace"]["members"]
        .as_array()
        .expect("workspace members are listed")
        .iter()
        .map(|member| member.as_str().expect("member is a path"))
        .collect();
    for (audited, _) in AUDITED {
        assert!(
            members.contains(&audited),
            "{audited} is a workspace member"
        );
    }

    let mut without_inheritance = Vec::new();
    for member in members {
        let manifest = read(root.join(member).join("Cargo.toml"));
        let inherits = manifest
            .get("lints")
            .and_then(|lints| lints.get("workspace"))
            .and_then(|value| value.as_bool())
            == Some(true);
        if let Some((_, level)) = AUDITED.iter().find(|(audited, _)| *audited == member) {
            assert!(!inherits, "{member} carries its own lint table");
            assert_eq!(
                manifest["lints"]["rust"]["unsafe_code"].as_str(),
                Some(*level),
                "{member} sets unsafe_code to {level}"
            );
            without_inheritance.push(member);
            continue;
        }
        assert!(inherits, "{member} must set [lints] workspace = true");
        assert!(
            manifest["lints"].get("rust").is_none(),
            "{member} must not override the workspace rust lints"
        );
    }
    assert_eq!(without_inheritance, AUDITED.map(|(audited, _)| audited));
}

/// Replace a watched file's content and move its modified time forward, so the refresher sees the
/// change even when the new content has the old size and the clock has not ticked.
fn rewrite(path: &Path, content: impl AsRef<[u8]>) {
    let before = fs::metadata(path).and_then(|metadata| metadata.modified());
    fs::write(path, content).unwrap();
    if let Ok(before) = before {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(before + std::time::Duration::from_secs(2))
            .unwrap();
    }
}

fn snapshot_for(name: &str, source: impl AsRef<[u8]>) -> super::AuthoritySnapshot {
    let path = temporary(name);
    fs::write(&path, source).unwrap();
    let snapshot = GovernanceFacade::new(Some(path.clone()), None).snapshot();
    let _ = fs::remove_file(path);
    snapshot.as_ref().clone()
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
fn an_empty_policy_refuses_ordinary_hosts_and_protected_resources_stay_protected() {
    let snapshot = snapshot_for(
        "empty",
        r#"{"schema":4,"name":"empty","version":"1","grants":[]}"#,
    );
    for url in [
        "https://example.com",
        "http://localhost:3000",
        "http://127.0.0.1",
        "http://169.254.169.254",
    ] {
        let ordinary = snapshot.authorize_landing(Capability::Read, url);
        assert!(!ordinary.allowed);
        assert_eq!(ordinary.reason, ReasonCode::HostDenied);
        assert!(ordinary.denial_id().is_some());
    }

    let protected = snapshot.authorize_landing(Capability::Read, "chrome://extensions");
    assert!(!protected.allowed);
    assert_eq!(protected.reason, ReasonCode::ProtectedHost);

    let sacred = snapshot_for(
        "sacred",
        r#"{"schema":4,"name":"sacred","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[{"key":"content.security.sacred_domains","value":["localhost"],"level":"mandatory"}]}"#,
    );
    assert!(
        !sacred
            .authorize_landing(Capability::Read, "http://localhost:3000")
            .allowed
    );
}

#[test]
fn layers_compose_by_intersection_and_snapshots_have_deterministic_identity() {
    let managed = temporary("compose-managed");
    let local = temporary("compose-local");
    fs::write(
        &managed,
        r#"{"schema":4,"name":"managed","version":"1","grants":[]}"#,
    )
    .unwrap();
    fs::write(
            &local,
            r#"{"schema":4,"name":"local","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
    let facade = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()));
    let first = facade.snapshot();
    let same = facade.snapshot();
    let denied = first.authorize_landing(Capability::Read, "https://example.com");
    assert!(
        !denied.allowed,
        "the local tier cannot readmit what the managed tier refuses"
    );
    assert_eq!(first.id(), same.id());

    rewrite(
        &local,
        r#"{"schema":4,"name":"local","version":"2","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
    );
    facade.refresh_tick();
    let changed = facade.snapshot();
    assert_ne!(first.id(), changed.id());
    let _ = fs::remove_file(managed);
    let _ = fs::remove_file(local);
}

#[test]
fn maintained_policy_examples_match_the_schema_four_decoder() {
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
            "dev-live-test",
            include_str!("../../../../examples/dev-live-test.json"),
        ),
        (
            "demo-policy",
            include_str!("../../../../examples/demo-policy.json"),
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
            super::read_policy(&path, false).is_ok(),
            "{name} must remain a valid policy"
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

    let good = r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#;
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
    let error = facade.apply_user_policy("{\"schema\":4,").unwrap_err();
    assert!(matches!(error, AuthoringError::Invalid(_)));
    assert_eq!(fs::read_to_string(&path).unwrap(), good);
    assert!(
        facade
            .snapshot()
            .authorize_landing(CapabilitySet::READ, "https://example.com")
            .allowed
    );

    // A switch schema 4 no longer has is an unregistered setting, not something to ignore.
    let retired = r#"{"schema":4,"name":"mine","version":"2","grants":[],"config":[{"key":"policy.user.enabled","value":true,"level":"mandatory"}]}"#;
    assert!(matches!(
        facade.apply_user_policy(retired).unwrap_err(),
        AuthoringError::Invalid(_)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), good);

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
        br#"{"schema":4,"name":"theirs","version":"1","grants":[]}"#,
    )
    .unwrap();
    let facade = GovernanceFacade::new(Some(path.clone()), None);
    assert!(matches!(
        facade
            .apply_user_policy(r#"{"schema":4,"name":"mine","version":"1","grants":[]}"#)
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
            br#"{"schema":4,"name":"org","version":"1","grants":[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}]}"#,
        )
        .unwrap();
    let facade = GovernanceFacade::owning_user_policy(owned.clone(), Some(managed.clone()));

    let decided = facade
            .candidate_snapshot(
                r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"wide","hosts":{"allow":["*"]},"allowed":["read","action","write"]}]}"#,
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
fn an_owned_schema_three_user_policy_moves_at_load_and_keeps_a_backup() {
    let path = temporary("owned-schema-three");
    let original = r#"{"schema":3,"name":"mine","version":"1","mode":"enforce","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}],"config":[{"key":"channels.cli.enabled","value":true,"level":"mandatory"},{"key":"policy.user.enabled","value":false,"level":"mandatory"},{"key":"content.frames.handling","value":"permitted_content","level":"mandatory"}]}"#;
    fs::write(&path, original).unwrap();
    let facade = GovernanceFacade::owning_user_policy(path.clone(), None);
    let snapshot = facade.snapshot();
    assert!(facade.diagnostics().authority_valid);
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
    let moved = fs::read_to_string(&path).unwrap();
    assert!(manifest::parse(&moved, "moved").is_ok());
    let backup = migrate::backup_path(&path);
    assert_eq!(fs::read_to_string(&backup).unwrap(), original);
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(backup);
}

#[test]
fn an_owned_schema_three_user_policy_that_would_change_meaning_fails_closed_untouched() {
    let path = temporary("owned-observe");
    let original = r#"{"schema":3,"name":"mine","version":"1","mode":"observe","grants":[]}"#;
    fs::write(&path, original).unwrap();
    let facade = GovernanceFacade::owning_user_policy(path.clone(), None);
    assert_eq!(
        facade
            .snapshot()
            .authorize_capability(Capability::Read)
            .reason,
        ReasonCode::InvalidAuthority
    );
    assert!(!facade.diagnostics().authority_valid);
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    assert!(!migrate::backup_path(&path).exists());
    let _ = fs::remove_file(path);
}

#[test]
fn administrator_schema_three_files_fail_closed_and_are_never_rewritten() {
    let managed = temporary("admin-managed");
    let named = temporary("admin-named");
    let document = r#"{"schema":3,"name":"theirs","version":"1","grants":[]}"#;
    for (user, organization) in [(Some(named.clone()), None), (None, Some(managed.clone()))] {
        fs::write(&managed, document).unwrap();
        fs::write(&named, document).unwrap();
        let facade = GovernanceFacade::new(user, organization);
        assert_eq!(
            facade
                .snapshot()
                .authorize_capability(Capability::Read)
                .reason,
            ReasonCode::InvalidAuthority
        );
        for path in [&managed, &named] {
            assert_eq!(fs::read_to_string(path).unwrap(), document);
            assert!(!migrate::backup_path(path).exists());
        }
    }
    let _ = fs::remove_file(managed);
    let _ = fs::remove_file(named);
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
    assert!(!facade.diagnostics().authority_valid);
    let _ = fs::remove_file(path);
}

#[test]
fn a_local_policy_that_never_validated_fails_closed_everywhere_it_is_reported() {
    let path = temporary("invalid-local");
    fs::write(&path, "{half-written").unwrap();
    let facade = GovernanceFacade::new(Some(path.clone()), None);
    assert_eq!(
        facade
            .snapshot()
            .authorize_capability(Capability::Read)
            .reason,
        ReasonCode::InvalidAuthority
    );
    let diagnostics = facade.diagnostics();
    assert!(!diagnostics.local_policy_valid);
    assert!(!diagnostics.authority_valid);
    assert_eq!(
        facade.policy_chip().situation,
        effective::Situation::FailingClosed
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
    rewrite(
        &path,
        policy(
            "action",
            r#"[{"id":"action","hosts":{"allow":["*"]},"allowed":["action"]}]"#,
            "[]",
        ),
    );
    assert!(first.authorize_capability(Capability::Read).allowed);
    assert!(!first.authorize_capability(Capability::Action).allowed);
    assert_eq!(
        facade.snapshot().id(),
        first.id(),
        "an edit is invisible to every reader until the refresher publishes it"
    );
    facade.refresh_tick();
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

    rewrite(&path, "{half-written");
    facade.refresh_tick();
    let retained = facade.snapshot();
    assert!(retained.authorize_capability(Capability::Read).allowed);
    assert!(!retained.authorize_capability(Capability::Action).allowed);
    let retained_diagnostics = facade.diagnostics();
    assert!(retained_diagnostics.local_policy_active);
    assert!(!retained_diagnostics.local_policy_valid);
    assert!(
        retained_diagnostics.authority_valid,
        "the retained policy is enforced, so authority is not failing closed"
    );
    assert_ne!(
        facade.policy_chip().situation,
        effective::Situation::FailingClosed
    );

    rewrite(
        &path,
        policy(
            "action",
            r#"[{"id":"action","hosts":{"allow":["*"]},"allowed":["action"]}]"#,
            "[]",
        ),
    );
    facade.refresh_tick();
    let replaced = facade.snapshot();
    assert!(!replaced.authorize_capability(Capability::Read).allowed);
    assert!(replaced.authorize_capability(Capability::Action).allowed);
    let replaced_diagnostics = facade.diagnostics();
    assert!(replaced_diagnostics.local_policy_active);
    assert!(replaced_diagnostics.local_policy_valid);
    assert!(replaced_diagnostics.authority_valid);
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

/// The runtime state is the in-memory controls and nothing else: a file named by the old control
/// variable is never read.
#[test]
fn the_runtime_control_variable_is_ignored() {
    let home = crate::service::test_home::TestHome::new("runtime-control");
    let path = home.path().join("runtime-control.txt");
    fs::write(&path, "attention").unwrap();
    let inherited_policy = std::env::var_os("GHOSTLIGHT_POLICY_FILE");
    std::env::remove_var("GHOSTLIGHT_POLICY_FILE");
    std::env::set_var("GHOSTLIGHT_RUNTIME_CONTROL_FILE", &path);
    let facade = GovernanceFacade::from_environment();
    std::env::remove_var("GHOSTLIGHT_RUNTIME_CONTROL_FILE");
    if let Some(value) = inherited_policy {
        std::env::set_var("GHOSTLIGHT_POLICY_FILE", value);
    }
    assert_eq!(facade.runtime_state(), RuntimeControlState::Active);
    assert!(facade.runtime_decision().allowed);
    facade.apply_runtime_intent(RuntimeControlIntent::Hold);
    assert_eq!(facade.runtime_decision().reason, ReasonCode::RuntimeHold);
    facade.apply_runtime_intent(RuntimeControlIntent::EndSession);
    assert_eq!(facade.runtime_decision().reason, ReasonCode::SessionEnded);
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
fn a_reader_is_never_held_up_by_the_sources_lock() {
    let path = temporary("lock-free-readers");
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
    let sources = facade.hold_sources();

    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = facade.clone();
    let thread = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let _ = reader.snapshot();
        let _ = reader.diagnostics();
        let _ = reader.policy_chip();
        let _ = reader.effective_authority();
        let _ = reader.browser_startup();
        let _ = sender.send(started.elapsed());
    });
    let elapsed = receiver.recv_timeout(std::time::Duration::from_secs(10));
    drop(sources);
    thread.join().unwrap();
    let _ = fs::remove_file(path);

    let elapsed = elapsed.expect("a reader waited on the sources lock");
    assert!(
        elapsed < std::time::Duration::from_millis(50),
        "readers took {elapsed:?}"
    );
}

#[test]
fn a_workbench_edit_is_visible_at_once_and_a_removal_returns_to_all_open() {
    let path = temporary("workbench-edit");
    let facade = GovernanceFacade::owning_user_policy(path.clone(), None);
    assert!(
        facade
            .snapshot()
            .authorize_capability(Capability::Action)
            .allowed
    );

    facade
        .apply_user_policy(&policy(
            "read",
            r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]"#,
            "[]",
        ))
        .unwrap();
    let applied = facade.snapshot();
    assert!(applied.authorize_capability(Capability::Read).allowed);
    assert!(!applied.authorize_capability(Capability::Action).allowed);
    assert_ne!(
        facade.policy_chip().situation,
        effective::Situation::AllOpen
    );

    facade.remove_user_policy().unwrap();
    assert!(
        facade
            .snapshot()
            .authorize_capability(Capability::Action)
            .allowed
    );
    assert_eq!(
        facade.policy_chip().situation,
        effective::Situation::AllOpen
    );
    let _ = fs::remove_file(path);
}

#[test]
fn control_changes_reach_the_observer_once_each() {
    use std::sync::Mutex;

    let facade = GovernanceFacade::new(None, None);
    let seen: Arc<Mutex<Vec<RuntimeControlState>>> = Arc::default();
    let recorder = Arc::clone(&seen);
    facade
        .controls()
        .observe(Arc::new(move |state| recorder.lock().unwrap().push(state)));

    facade.apply_runtime_intent(RuntimeControlIntent::Resume);
    facade.apply_runtime_intent(RuntimeControlIntent::Hold);
    facade.apply_runtime_intent(RuntimeControlIntent::Hold);
    facade.apply_runtime_intent(RuntimeControlIntent::ToggleHold);
    facade.apply_runtime_intent(RuntimeControlIntent::EndSession);
    facade.apply_runtime_intent(RuntimeControlIntent::EndSession);
    facade.apply_runtime_intent(RuntimeControlIntent::StartSession);

    assert_eq!(
        *seen.lock().unwrap(),
        [
            RuntimeControlState::Held,
            RuntimeControlState::Active,
            RuntimeControlState::Ended,
            RuntimeControlState::Active,
        ]
    );
}

#[cfg(unix)]
#[test]
fn the_authored_policy_and_its_directory_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let directory = std::env::temp_dir().join(format!("ghostlight-1.0-{}", uuid::Uuid::new_v4()));
    let path = directory.join("policy").join("user.json");
    let facade = GovernanceFacade::owning_user_policy(path.clone(), None);
    facade
        .apply_user_policy(r#"{"schema":4,"name":"mine","version":"1","grants":[]}"#)
        .unwrap();
    let mode = |path: &std::path::Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(path.parent().unwrap()), 0o700);
    let _ = fs::remove_dir_all(directory);
}
