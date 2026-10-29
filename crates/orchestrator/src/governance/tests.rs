//! The facade-level proofs of the authority snapshot the executor enforces: how layers and their
//! settings compose, Ghostlight's own ceilings, never-touch lists, grants and host patterns,
//! authoring from the window, the load-time move, the one refresher, runtime control, owner-only
//! files and denial ids. The workspace's unsafe-confinement guard lives here too.

use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::browser::adapter::{RuntimeControlIntent, RuntimeControlState};
use crate::install::record::assert_owner_only;
use crate::language::outcome::OWN_CONTROL_PAGE_CEILING;

use super::effective::LayerKind;
use super::{AuthoringError, Capability, CapabilitySet, GovernanceFacade, ReasonCode};
use crate::scratch::Scratch;

fn temporary(name: &str) -> Scratch {
    Scratch::file(&format!("{name}.json"))
}

fn policy(name: &str, grants: &str, config: &str) -> String {
    format!(r#"{{"schema":4,"name":"{name}","version":"1","grants":{grants},"config":{config}}}"#)
}

fn all_open_grant() -> &'static str {
    r#"[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]"#
}

/// A facade under `organization`, published as a signed bundle, over the user policy at `local`.
///
/// The returned directory holds the bundle and its bootstrap, so it must outlive the facade.
fn under_organization(
    name: &str,
    local: Option<&Path>,
    organization: &str,
) -> (Scratch, GovernanceFacade) {
    let root = Scratch::named(&format!("{name}-organization"));
    let facade = GovernanceFacade::with_managed_paths(
        local.map(Path::to_path_buf),
        managed::ManagedPaths::signed(&root, organization),
    );
    (root, facade)
}

/// Replace a watched file's content and move its modified time forward, so the refresher sees the
/// change even when the new content has the old size and the clock has not ticked.
fn rewrite(path: &Path, content: impl AsRef<[u8]>) {
    let before = fs::metadata(path).and_then(|metadata| metadata.modified());
    fs::write(path, content).unwrap();
    if let Ok(before) = before {
        set_modified(path, before + Duration::from_secs(2));
    }
}

fn set_modified(path: &Path, time: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

fn snapshot_for(name: &str, source: impl AsRef<[u8]>) -> super::AuthoritySnapshot {
    let path = temporary(name);
    fs::write(&path, source).unwrap();
    let snapshot = GovernanceFacade::new(Some(path.to_path_buf())).snapshot();
    drop(path);
    snapshot.as_ref().clone()
}

/// A snapshot of an organization layer and a user layer, each with its own never-touch list.
fn never_touch_layers(
    name: &str,
    organization: &[&str],
    user: &[&str],
) -> super::AuthoritySnapshot {
    let never_touch = |hosts: &[&str]| {
        format!(
            r#"[{{"key":"{}","value":{},"level":"mandatory"}}]"#,
            PolicySetting::NeverTouch.key(),
            serde_json::to_string(hosts).unwrap()
        )
    };
    let local = temporary(&format!("{name}-local"));
    fs::write(&local, policy("mine", all_open_grant(), &never_touch(user))).unwrap();
    let (_organization, facade) = under_organization(
        name,
        Some(&local),
        &policy("org", all_open_grant(), &never_touch(organization)),
    );
    facade.snapshot().as_ref().clone()
}

/// The layer a tightened setting is credited to, in each vocabulary that names it.
#[derive(Clone, Copy, Debug)]
struct Credit {
    /// The tier word an attribution and an audit line carry.
    tier: &'static str,
    /// The author a channel refusal names.
    author: PolicyAuthor,
    /// The layer the Policy page names.
    kind: LayerKind,
}

const BY_ORGANIZATION: Credit = Credit {
    tier: MANAGED_TIER,
    author: PolicyAuthor::Organization,
    kind: LayerKind::Organization,
};

const BY_USER: Credit = Credit {
    tier: USER_TIER,
    author: PolicyAuthor::User,
    kind: LayerKind::User,
};

/// `decision` is open when `credit` is none, and otherwise refused for `reason` as the decision of
/// `credit`'s layer, with no grant.
fn assert_credited(
    snapshot: &AuthoritySnapshot,
    decision: Decision,
    reason: ReasonCode,
    credit: Option<Credit>,
) {
    match credit {
        None => assert!(decision.allowed, "{reason:?} is open"),
        Some(credit) => {
            assert!(!decision.allowed, "{reason:?} is tightened");
            assert_eq!(decision.reason, reason);
            assert_eq!(
                snapshot.attribution(decision),
                Some((credit.tier, None)),
                "{reason:?}"
            );
        }
    }
}

/// `channel` is open when `credit` is none, and otherwise refused as `credit`'s decision, while the
/// other channel stays open.
fn assert_channel(
    snapshot: &AuthoritySnapshot,
    channel: IntakeChannel,
    other: IntakeChannel,
    credit: Option<Credit>,
) {
    let refusal = snapshot.channel_refusal(channel);
    match credit {
        None => assert_eq!(refusal, None, "{channel:?}"),
        Some(credit) => {
            let (decision, author) = refusal.expect("the switch is off");
            assert_eq!(author, credit.author, "{channel:?}");
            assert_credited(snapshot, decision, ReasonCode::ChannelDenied, Some(credit));
        }
    }
    assert_eq!(snapshot.channel_refusal(other), None, "{other:?}");
}

/// ACCEPTANCE Governance 4, 14 and 17, Document boundary 4 and Audit health 2: every setting a
/// layer can tighten is on, or open, until a layer tightens it, and a lower layer cannot relax
/// what a higher one tightened. The refusal is credited to the first layer that tightened it, and
/// the Policy page names that same layer. A switch left unset is on, and tightening one setting
/// refuses nothing else.
#[test]
fn a_setting_either_layer_tightens_stays_tight_and_credits_the_layer_that_did() {
    /// One tighten-only setting: the value that tightens it, the value that leaves it open, and
    /// a check of every reader of it, given the organization's authored value and the layer
    /// expected to have tightened it.
    struct Setting {
        key: PolicySetting,
        tight: &'static str,
        open: &'static str,
        holds: fn(&GovernanceFacade, Option<Authored>, Option<Credit>),
    }
    #[derive(Clone, Copy, Debug)]
    enum Authored {
        Tight,
        Open,
    }

    let settings = [
        Setting {
            key: PolicySetting::ScreenshotsEnabled,
            tight: "false",
            open: "true",
            holds: |facade, _, credit| {
                let snapshot = facade.snapshot();
                let decision = snapshot.authorize_screenshot();
                assert_credited(&snapshot, decision, ReasonCode::ScreenshotDenied, credit);
            },
        },
        Setting {
            key: PolicySetting::TabsAllowClose,
            tight: "false",
            open: "true",
            holds: |facade, _, credit| {
                let snapshot = facade.snapshot();
                let decision = snapshot.authorize_tab_close();
                assert_credited(&snapshot, decision, ReasonCode::TabCloseDenied, credit);
            },
        },
        Setting {
            key: PolicySetting::McpChannelEnabled,
            tight: "false",
            open: "true",
            holds: |facade, _, credit| {
                let snapshot = facade.snapshot();
                assert_channel(&snapshot, IntakeChannel::Mcp, IntakeChannel::Cli, credit);
            },
        },
        Setting {
            key: PolicySetting::CliChannelEnabled,
            tight: "false",
            open: "true",
            holds: |facade, _, credit| {
                let snapshot = facade.snapshot();
                assert_channel(&snapshot, IntakeChannel::Cli, IntakeChannel::Mcp, credit);
            },
        },
        Setting {
            key: PolicySetting::PreserveTargetNames,
            tight: "false",
            open: "true",
            holds: |facade, _, credit| {
                assert_eq!(facade.snapshot().preserves_target_names(), credit.is_none());
            },
        },
        Setting {
            key: PolicySetting::UpdatesCheckEnabled,
            tight: "false",
            open: "true",
            holds: |facade, _, credit| {
                assert_eq!(facade.snapshot().allows_update_check(), credit.is_none());
            },
        },
        Setting {
            key: PolicySetting::AuditAvailability,
            tight: r#""require_audit""#,
            open: r#""keep_working""#,
            holds: |facade, _, credit| {
                let snapshot = facade.snapshot();
                let decision = snapshot.authorize_audit(false);
                assert_credited(&snapshot, decision, ReasonCode::AuditUnavailable, credit);
                assert!(snapshot.authorize_audit(true).allowed);
                let view = facade.effective_authority().audit;
                let expected = if credit.is_some() {
                    audit_requirement::AuditMode::RequireAudit
                } else {
                    audit_requirement::AuditMode::KeepWorking
                };
                assert_eq!(view.mode, expected);
                assert_eq!(view.source, credit.map(|credit| credit.kind));
            },
        },
        Setting {
            key: PolicySetting::BrowserStartup,
            tight: r#""manual""#,
            open: r#""on_demand""#,
            holds: |facade, organization, credit| {
                let view = facade.effective_authority().browser_startup;
                // With no layer choosing, the platform default: on demand on Windows only.
                let platform = if cfg!(windows) {
                    manifest::BrowserStartup::OnDemand
                } else {
                    manifest::BrowserStartup::Manual
                };
                let expected = credit.map_or(platform, |_| manifest::BrowserStartup::Manual);
                assert_eq!(view.value, expected);
                assert_eq!(view.decided_by, credit.map(|credit| credit.kind));
                assert_eq!(
                    view.organization_ceiling,
                    organization.map(|authored| match authored {
                        Authored::Tight => manifest::BrowserStartup::Manual,
                        Authored::Open => manifest::BrowserStartup::OnDemand,
                    })
                );
                assert_eq!(facade.browser_startup(), expected);
            },
        },
    ];

    // (user, organization, credited): None for a layer means it leaves the setting unset.
    let rows = [
        (None, None, None),
        (Some(Authored::Tight), Some(Authored::Open), Some(BY_USER)),
        (
            Some(Authored::Open),
            Some(Authored::Tight),
            Some(BY_ORGANIZATION),
        ),
        (
            Some(Authored::Tight),
            Some(Authored::Tight),
            Some(BY_ORGANIZATION),
        ),
    ];
    for setting in &settings {
        let key = setting.key.key();
        // No policy at all.
        (setting.holds)(&GovernanceFacade::new(None), None, None);
        for (user, organization, credit) in rows {
            let config = |authored: Option<Authored>| {
                authored.map_or_else(
                    || "[]".to_owned(),
                    |authored| {
                        let value = match authored {
                            Authored::Tight => setting.tight,
                            Authored::Open => setting.open,
                        };
                        format!(r#"[{{"key":"{key}","value":{value},"level":"mandatory"}}]"#)
                    },
                )
            };
            let local = temporary(&format!("setting-{key}"));
            fs::write(&local, policy("mine", all_open_grant(), &config(user))).unwrap();
            let (_root, facade) = under_organization(
                &format!("setting-{key}"),
                Some(&local),
                &policy("org", all_open_grant(), &config(organization)),
            );
            (setting.holds)(&facade, organization, credit);
            assert!(
                facade
                    .snapshot()
                    .authorize_landing(Capability::Read, "https://example.com/")
                    .allowed,
                "{key} refuses nothing else"
            );
        }
    }
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

const WORKBENCH_PORT: u16 = 41234;

/// ACCEPTANCE Governance 1 and 2 and docs/trust/security-overview.md ("agents can never open or
/// read Ghostlight's own workbench address"): two ceilings are Ghostlight's own, under every
/// policy and credited to no layer. An address that is not HTTP(S) is refused, and so is the
/// window's own origin, which every snapshot refuses from the moment the window binds its port,
/// whatever a grant allows or a never-touch list names. With no policy every other HTTP(S)
/// address, local or remote, is permitted, and the Policy page states the own-origin ceiling
/// (00610714, fd6cd0ca, 0c147d04).
#[test]
fn ghostlights_own_ceilings_refuse_under_every_policy_and_credit_no_layer() {
    let open = GovernanceFacade::new(None);
    let granted_file = temporary("own-origin-grant");
    fs::write(
        &granted_file,
        policy(
            "loopback",
            r#"[{"id":"loopback","hosts":{"allow":["127.0.0.1"]},"allowed":["read","action","write","execute"]}]"#,
            "[]",
        ),
    )
    .unwrap();
    let granted = GovernanceFacade::new(Some(granted_file.to_path_buf()));
    // G-D1 (e0e90fa2): a never-touch list that names the loopback hosts does not take the credit
    // for the workbench address; it stays Ghostlight's own ceiling.
    let listed_file = temporary("own-origin-never-touch");
    fs::write(
        &listed_file,
        policy(
            "listed",
            all_open_grant(),
            &format!(
                r#"[{{"key":"{}","value":["localhost","127.0.0.1"],"level":"mandatory"}}]"#,
                PolicySetting::NeverTouch.key()
            ),
        ),
    )
    .unwrap();
    let listed = GovernanceFacade::new(Some(listed_file.to_path_buf()));

    // A snapshot taken before the window binds its port sees the port once it is set, and the
    // authority id does not change with it.
    let workbench = "http://127.0.0.1:41234/";
    let early = open.snapshot();
    let id = early.id().to_owned();
    assert!(early.authorize_landing(Capability::Read, workbench).allowed);
    for facade in [&open, &granted, &listed] {
        facade.protect_workbench_port(WORKBENCH_PORT);
    }
    assert_eq!(
        early.authorize_landing(Capability::Read, workbench).reason,
        ReasonCode::OwnWorkbench
    );
    assert_eq!(early.id(), id);
    assert_eq!(open.snapshot().id(), id);

    for facade in [&open, &granted, &listed] {
        let snapshot = facade.snapshot();
        for url in [
            "http://127.0.0.1:41234/",
            "http://127.0.0.1:41234/api/snapshot",
            "https://127.0.0.1:41234/",
            "http://localhost:41234/",
            "http://LOCALHOST:41234/x",
            "http://app.localhost:41234/",
            "http://127.8.9.10:41234/",
            "http://[::1]:41234/",
            "http://[::ffff:127.0.0.1]:41234/",
            "http://2130706433:41234/",
            "http://0.0.0.0:41234/",
            // G-D1 (e0e90fa2): a fully qualified name and the unspecified IPv6 address reach it
            // too.
            "http://localhost.:41234/",
            "http://[::]:41234/",
        ] {
            for capability in [Capability::Read, Capability::Action] {
                let refused = snapshot.authorize_landing(capability, url);
                assert_eq!(
                    refused.reason,
                    ReasonCode::OwnWorkbench,
                    "{capability:?} {url}"
                );
                assert_eq!(snapshot.attribution(refused), None, "{url}");
                assert_eq!(refused.denial_id(), None, "{url}");
            }
        }
    }
    // Another loopback port is not the workbench origin, under a grant and with no policy.
    for snapshot in [open.snapshot(), granted.snapshot()] {
        assert!(
            snapshot
                .authorize_landing(Capability::Read, "http://127.0.0.1:41235/")
                .allowed
        );
    }
    let all_open = open.snapshot();
    for url in ["http://localhost:3000/", "https://example.com:41234/"] {
        assert!(
            all_open.authorize_landing(Capability::Read, url).allowed,
            "{url} is not the workbench origin"
        );
    }
    // The never-touch list still protects every other loopback address, as the user's decision.
    let other = listed
        .snapshot()
        .authorize_landing(Capability::Read, "http://localhost:3000/");
    assert_eq!(other.reason, ReasonCode::ProtectedHost);

    // With no policy, every capability is permitted at every other HTTP(S) address.
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
                all_open.authorize_landing(capability, url).allowed,
                "{capability:?} at {url} needs no policy exception"
            );
        }
    }

    // An address that is not HTTP(S) is refused under no policy and under an authored one.
    let authored = never_touch_layers("own-ceiling-scheme", &["bank.example"], &["mail.example"]);
    for snapshot in [all_open.as_ref(), &authored] {
        for url in [
            "chrome://extensions/",
            "chrome-extension://example/options.html",
            "file:///tmp/example.html",
            "file:///tmp/bank.example.html",
            "data:text/html,example",
            "javascript:void(0)",
            "about:blank",
            "ftp://localhost/",
        ] {
            let refused = snapshot.authorize_landing(Capability::Read, url);
            assert!(!refused.allowed, "{url}");
            assert_eq!(refused.reason, ReasonCode::ProtectedHost, "{url}");
            assert_eq!(refused.policy_rule(), None, "{url}");
            assert_eq!(refused.denial_id(), None, "{url}");
            assert_eq!(snapshot.attribution(refused), None, "{url}");
        }
    }

    assert!(open
        .effective_authority()
        .ceilings
        .iter()
        .any(|ceiling| ceiling == OWN_CONTROL_PAGE_CEILING));
}

/// bc00a84b (ADR-0203 Decision 7.2, G-05), ACCEPTANCE Governance 2 and
/// docs/trust/security-overview.md: a host a layer marks never-touch is refused for every agent,
/// local hosts too, as the decision of the first layer whose list matches: rule sacred_host, that
/// layer's tier and no grant. Every host one layer lists shares one denial id, which names none
/// of them. The fully qualified name of a listed host is the same host (G-D1, e0e90fa2).
#[test]
fn a_never_touch_host_is_the_listing_layers_refusal_and_its_id_names_no_host() {
    let snapshot = never_touch_layers(
        "never-touch",
        &["bank.example", "*.vault.example", "*.corp.example"],
        &["mail.example", "localhost", "www.corp.example"],
    );
    let bank = snapshot.authorize_landing(Capability::Read, "https://bank.example/accounts");
    assert!(!bank.allowed);
    assert_eq!(bank.reason, ReasonCode::ProtectedHost);
    assert_eq!(bank.policy_rule(), Some(PolicyRule::NeverTouch.as_str()));
    assert_eq!(snapshot.attribution(bank), Some((MANAGED_TIER, None)));
    let id = bank.denial_id().expect("a policy refusal has a denial id");

    // G-D1 (e0e90fa2): the fully qualified name a browser loads as the same site is the same
    // host.
    for url in ["https://bank.example./", "https://BANK.Example../x"] {
        let refused = snapshot.authorize_landing(Capability::Read, url);
        assert_eq!(refused.reason, ReasonCode::ProtectedHost, "{url}");
        assert_eq!(snapshot.attribution(refused), Some((MANAGED_TIER, None)));
        assert_eq!(refused.denial_id().as_deref(), Some(id.as_str()), "{url}");
    }

    // Every host one layer lists gets the same id, so the id cannot tell which entry matched.
    let vault = snapshot.authorize_landing(Capability::Read, "https://a.vault.example/");
    assert_eq!(vault.denial_id().as_deref(), Some(id.as_str()));
    assert!(!id.contains("bank") && !id.contains("vault"), "{id}");

    // The person's own list is the person's decision, local hosts included.
    for url in ["https://mail.example/", "http://localhost:3000/"] {
        let own = snapshot.authorize_landing(Capability::Read, url);
        assert_eq!(own.reason, ReasonCode::ProtectedHost, "{url}");
        assert_eq!(snapshot.attribution(own), Some((USER_TIER, None)), "{url}");
        assert_ne!(own.denial_id().as_deref(), Some(id.as_str()), "{url}");
    }

    // A host both lists hold is credited to the first layer whose list matches.
    let both = snapshot.authorize_landing(Capability::Read, "https://www.corp.example/");
    assert_eq!(snapshot.attribution(both), Some((MANAGED_TIER, None)));
}

/// Deny-by-default (ACCEPTANCE Governance 2, ADR-0155): under an authored policy a destination,
/// local or remote, is admitted only by a grant that names its host and holds the capability.
/// Localhost, loopback and link-local addresses get no exemption, and a policy with no grants
/// refuses every host.
#[test]
fn an_authored_policy_admits_a_destination_only_through_a_grant_naming_it() {
    let development = snapshot_for(
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
        assert!(development.authorize_landing(Capability::Read, url).allowed);
        assert!(
            development
                .authorize_landing(Capability::Action, url)
                .allowed
        );
        let denied = development.authorize_landing(Capability::Execute, url);
        assert!(!denied.allowed);
        assert_eq!(denied.reason, ReasonCode::CapabilityDenied);
        assert_eq!(
            development.attribution(denied),
            Some((USER_TIER, Some("development")))
        );
    }

    let reading = snapshot_for(
        "configured-boundary",
        policy(
            "reading",
            r#"[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read"]}]"#,
            "[]",
        ),
    );
    assert!(
        reading
            .authorize_landing(Capability::Read, "https://example.com/")
            .allowed
    );
    assert_eq!(
        reading
            .authorize_landing(Capability::Action, "https://example.com/")
            .reason,
        ReasonCode::CapabilityDenied
    );

    let empty = snapshot_for("empty", policy("empty", "[]", "[]"));
    let refused: [(&AuthoritySnapshot, &[&str]); 3] = [
        (
            &development,
            &[
                "http://admin.localhost/",
                "http://169.254.169.254/latest/",
                "http://127.0.0.2/",
                "http://[::1]/",
                "https://example.org/",
            ],
        ),
        (
            &reading,
            &[
                "https://example.org/",
                "http://localhost:3000/",
                "http://127.0.0.1/",
                "http://[::1]/",
                "http://169.254.169.254/",
            ],
        ),
        (
            &empty,
            &[
                "https://example.com/",
                "http://localhost:3000/",
                "http://127.0.0.1/",
                "http://169.254.169.254/",
            ],
        ),
    ];
    for (snapshot, urls) in refused {
        for url in urls {
            let denied = snapshot.authorize_landing(Capability::Read, url);
            assert!(!denied.allowed, "{url}");
            assert_eq!(denied.reason, ReasonCode::HostDenied, "{url}");
            assert!(denied.denial_id().is_some(), "{url}");
        }
    }
}

/// ACCEPTANCE Governance 4 and ADR-0121 Decision 2: inside one grant the most specific matching
/// pattern decides a host: an exact host beats any wildcard, a longer suffix beats a shorter one,
/// `*` is the weakest, and an allow and a deny of equal specificity deny. A suffix wildcard names
/// the hosts below it and never the bare host, whatever the address's case.
#[test]
fn the_most_specific_pattern_in_a_grant_decides_a_host_and_a_tie_denies() {
    // (allow, deny, address) -> None when admitted, or the rule that refused it.
    let rows: [(&str, Option<&str>, &str, Option<PolicyRule>); 13] = [
        (
            "admin.example.com",
            Some("*.example.com"),
            "https://admin.example.com/",
            None,
        ),
        (
            "admin.example.com",
            Some("*.example.com"),
            "https://b.example.com/",
            Some(PolicyRule::DeniedHost),
        ),
        (
            "admin.example.com",
            Some("admin.example.com"),
            "https://admin.example.com/",
            Some(PolicyRule::DeniedHost),
        ),
        (
            "*.example.com",
            Some("*.example.com"),
            "https://a.example.com/",
            Some(PolicyRule::DeniedHost),
        ),
        (
            "*.example.com",
            Some("*.secure.example.com"),
            "https://a.secure.example.com/",
            Some(PolicyRule::DeniedHost),
        ),
        (
            "*.example.com",
            Some("*.docs.example.com"),
            "https://a.docs.example.com/",
            Some(PolicyRule::DeniedHost),
        ),
        (
            "*.docs.example.com",
            Some("*.example.com"),
            "https://a.docs.example.com/",
            None,
        ),
        (
            "*.a.example.com",
            Some("*.example.com"),
            "https://x.a.example.com/",
            None,
        ),
        ("*.example.com", Some("*"), "https://a.example.com/", None),
        (
            "*",
            Some("intranet.example"),
            "https://intranet.example/",
            Some(PolicyRule::DeniedHost),
        ),
        (
            "*",
            Some("intranet.example"),
            "https://other.example/",
            None,
        ),
        (
            "*.example.com",
            None,
            "https://example.com/",
            Some(PolicyRule::UnmatchedHost),
        ),
        ("*.example.com", None, "https://A.B.Example.COM/", None),
    ];
    for (index, (allow, deny, url, refused)) in rows.into_iter().enumerate() {
        let hosts = match deny {
            Some(deny) => format!(r#"{{"allow":["{allow}"],"deny":["{deny}"]}}"#),
            None => format!(r#"{{"allow":["{allow}"]}}"#),
        };
        let snapshot = snapshot_for(
            &format!("specificity-{index}"),
            policy(
                "specificity",
                &format!(r#"[{{"id":"g","hosts":{hosts},"allowed":["read"]}}]"#),
                "[]",
            ),
        );
        let decision = snapshot.authorize_landing(Capability::Read, url);
        assert_eq!(
            decision.allowed,
            refused.is_none(),
            "{allow} - {deny:?} at {url}"
        );
        assert_eq!(
            decision.policy_rule(),
            refused.map(PolicyRule::as_str),
            "{allow} - {deny:?} at {url}"
        );
    }
}

/// ACCEPTANCE Governance 3, 4 and 11, ADR-0121 Decisions 1 and 2: a layer admits an operation on a
/// host through the first grant, in order, whose hosts admit the host and whose capabilities hold
/// the whole set; two grants that split the set do not admit it; a grant's own deny shrinks only
/// that grant; when no grant admits, the first grant that applied is credited. An operation with
/// no host takes the union of the layer's grants, and an empty set needs no grant.
#[test]
fn a_layer_admits_an_operation_through_the_first_grant_that_holds_all_of_it() {
    let carved = snapshot_for(
        "grant-order",
        policy(
            "grant order",
            r#"[{"id":"carved","hosts":{"allow":["*.example.com"],"deny":["admin.example.com"]},"allowed":["read"]},{"id":"admin","hosts":{"allow":["admin.example.com"]},"allowed":["read"]}]"#,
            "[]",
        ),
    );
    assert!(
        carved
            .authorize_landing(Capability::Read, "https://admin.example.com/")
            .allowed
    );

    // G-D1 (e0e90fa2): a grant's deny names the fully qualified name too, so under a universal
    // allow the trailing dot does not step around it.
    let universal = snapshot_for(
        "grant-deny-fqdn",
        policy(
            "universal with a deny",
            r#"[{"id":"all","hosts":{"allow":["*"],"deny":["bank.example","*.bank.example"]},"allowed":["read"]}]"#,
            "[]",
        ),
    );
    for url in ["https://bank.example./", "https://www.bank.example./"] {
        assert!(
            !universal.authorize_landing(Capability::Read, url).allowed,
            "{url}"
        );
    }
    assert!(
        universal
            .authorize_landing(Capability::Read, "https://example.com./")
            .allowed
    );

    let split = snapshot_for(
        "grant-capability-order",
        policy(
            "grant capability order",
            r#"[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"action","hosts":{"allow":["example.com"]},"allowed":["action"]}]"#,
            "[]",
        ),
    );
    assert!(
        split
            .authorize_landing(Capability::Action, "https://example.com/")
            .allowed,
        "the first host match is not a refusal when a later grant admits the full set"
    );
    let whole = split.authorize_landing(
        CapabilitySet::READ.union(CapabilitySet::ACTION),
        "https://example.com/",
    );
    assert_eq!(whole.reason, ReasonCode::CapabilityDenied);
    assert_eq!(split.attribution(whole), Some((USER_TIER, Some("read"))));

    let first = snapshot_for(
        "first-denial",
        policy(
            "first denial",
            r#"[{"id":"first","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"second","hosts":{"allow":["other.example"]},"allowed":["action"]}]"#,
            "[]",
        ),
    );
    let refused = first.authorize_landing(Capability::Action, "https://example.com/");
    assert!(!refused.allowed);
    assert_eq!(refused.policy_rule(), Some(PolicyRule::Capability.as_str()));
    assert_eq!(first.attribution(refused), Some((USER_TIER, Some("first"))));

    let union = snapshot_for(
        "compound-capabilities",
        policy(
            "compound",
            r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]},{"id":"write","hosts":{"allow":["*"]},"allowed":["write"]}]"#,
            "[]",
        ),
    );
    let read_write = CapabilitySet::READ.union(CapabilitySet::WRITE);
    assert!(union.authorize_requirements(read_write).allowed);
    assert_eq!(
        union
            .authorize_requirements(read_write.union(CapabilitySet::ACTION))
            .reason,
        ReasonCode::CapabilityDenied
    );
    assert!(union.authorize_requirements(CapabilitySet::EMPTY).allowed);
}

/// ACCEPTANCE Governance 4 (layer intersection), docs/trust/security-overview.md ("Managed and
/// local tiers intersect") and ADR-0122's preview: a user grant, applied or previewed as a draft,
/// never admits what the organization's layer refuses, the refusal is the organization's, and
/// previewing writes nothing.
#[test]
fn the_organization_bounds_the_users_layer_applied_or_previewed() {
    let organization = Scratch::named("bounded-organization");
    let owned = temporary("bounded-user");
    let facade = GovernanceFacade::publishing(
        PolicySources::owning(owned.to_path_buf()),
        Some(managed::ManagedAuthority::from_paths(
            managed::ManagedPaths::signed(
                &organization,
                &policy(
                    "org",
                    r#"[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}]"#,
                    "[]",
                ),
            ),
        )),
    );
    let wide = policy(
        "mine",
        r#"[{"id":"wide","hosts":{"allow":["*"]},"allowed":["read","action","write"]}]"#,
        "[]",
    );

    let previewed = facade.candidate_snapshot(&wide).unwrap().snapshot;
    assert!(!owned.exists(), "previewing writes nothing");
    facade.apply_user_policy(&wide).unwrap();
    let applied = facade.snapshot();

    for (what, snapshot) in [("previewed", &previewed), ("applied", applied.as_ref())] {
        assert!(
            snapshot
                .authorize_landing(Capability::Read, "https://example.com/")
                .allowed,
            "{what}"
        );
        for (capability, url) in [
            (Capability::Write, "https://example.com/"),
            (Capability::Read, "https://elsewhere.test/"),
        ] {
            let refused = snapshot.authorize_landing(capability, url);
            assert!(!refused.allowed, "{what}: {capability:?} at {url}");
            assert_eq!(
                snapshot.attribution(refused).map(|(tier, _)| tier),
                Some(MANAGED_TIER),
                "{what}: {capability:?} at {url}"
            );
        }
    }
}

/// Contract with administrators: every policy in examples/, which examples/README.md and the
/// governance guide tell them to copy, stays a valid schema-4 policy.
#[test]
fn every_maintained_policy_example_is_a_valid_schema_four_policy() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut read = 0;
    for entry in fs::read_dir(&examples).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            if let Err(error) = super::read_policy(&path, false) {
                panic!("{} must remain a valid policy: {error}", path.display());
            }
            read += 1;
        }
    }
    assert!(read > 0, "examples/ holds the policies");
}

/// ADR-0122 Decision 4, ACCEPTANCE Governance 22 ("a workbench edit at once") and arch-1 P8: with
/// a file Ghostlight owns, a never-authored machine is open, a valid document applies at once, an
/// invalid one changes neither the file nor the authority, and removing it returns to all-open,
/// twice without failing. A file an environment variable names, present or absent, and a machine
/// with no per-user state directory refuse every write and removal in the one sentence the window
/// shows, and the file stays as it was.
#[test]
fn the_window_writes_only_the_policy_ghostlight_owns_and_says_one_sentence_when_it_cannot() {
    use crate::language::policy::{NOT_OWNED, NO_HOME};

    let path = temporary("authored-policy");
    let facade = GovernanceFacade::owning_user_policy(path.to_path_buf());
    let execute = |facade: &GovernanceFacade| {
        facade
            .snapshot()
            .authorize_landing(Capability::Execute, "https://example.com/")
            .allowed
    };
    let read = |facade: &GovernanceFacade| {
        facade
            .snapshot()
            .authorize_landing(Capability::Read, "https://example.com/")
            .allowed
    };
    assert!(execute(&facade), "a never-authored machine is open");

    let good = policy(
        "mine",
        r#"[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]"#,
        "[]",
    );
    facade.apply_user_policy(&good).unwrap();
    assert!(read(&facade));
    assert!(!execute(&facade));

    let error = facade.apply_user_policy("{\"schema\":4,").unwrap_err();
    assert!(matches!(error, AuthoringError::Invalid(_)));
    assert_eq!(fs::read_to_string(&path).unwrap(), good);
    assert!(read(&facade));
    assert!(!execute(&facade));

    facade.remove_user_policy().unwrap();
    assert!(!path.exists());
    assert!(execute(&facade), "a removal returns to all-open");
    facade.remove_user_policy().unwrap();

    let theirs = policy("theirs", "[]", "[]");
    let named = temporary("named-elsewhere");
    fs::write(&named, &theirs).unwrap();
    let absent = temporary("named-elsewhere-absent");
    let homeless = PolicySources {
        remote: None,
        user: PolicySource::with_options(None, true),
        user_origin: effective::UserLayerSource::Workbench,
        owned_user_path: None,
    };
    for (facade, said) in [
        (GovernanceFacade::new(Some(named.to_path_buf())), NOT_OWNED),
        (GovernanceFacade::new(Some(absent.to_path_buf())), NOT_OWNED),
        (GovernanceFacade::publishing(homeless, None), NO_HOME),
    ] {
        let window = facade.effective_authority().user_layer;
        assert!(!window.editable, "{said}");
        assert_eq!(window.path, None, "{said}");
        assert_eq!(window.blocked_reason.as_deref(), Some(said));
        let refused = facade
            .apply_user_policy(&policy("mine", "[]", "[]"))
            .unwrap_err();
        assert_eq!(refused.to_string(), said);
        assert_eq!(facade.remove_user_policy().unwrap_err().to_string(), said);
    }
    assert_eq!(fs::read_to_string(&named).unwrap(), theirs);
    assert!(!absent.exists());
}

/// ACCEPTANCE Governance 4: at load Ghostlight moves a schema-3 user policy only when it owns the
/// file and the move keeps its meaning. The moved file enforces the same grants and its backup
/// holds the original. A file whose meaning would change, or one Ghostlight does not own, fails
/// closed and stays byte for byte, with no backup.
#[test]
fn a_schema_three_user_policy_moves_at_load_only_when_ghostlight_owns_it_and_its_meaning_holds() {
    let path = temporary("owned-schema-three");
    let original = r#"{"schema":3,"name":"mine","version":"1","mode":"enforce","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}],"config":[{"key":"channels.cli.enabled","value":true,"level":"mandatory"},{"key":"policy.user.enabled","value":false,"level":"mandatory"},{"key":"content.frames.handling","value":"permitted_content","level":"mandatory"}]}"#;
    fs::write(&path, original).unwrap();
    let facade = GovernanceFacade::owning_user_policy(path.to_path_buf());
    let snapshot = facade.snapshot();
    assert!(facade.diagnostics().authority_valid);
    assert!(
        snapshot
            .authorize_landing(Capability::Read, "https://example.com/")
            .allowed
    );
    assert!(
        !snapshot
            .authorize_landing(Capability::Action, "https://example.com/")
            .allowed
    );
    assert!(manifest::parse(&fs::read_to_string(&path).unwrap(), "moved").is_ok());
    assert_eq!(
        fs::read_to_string(migrate::backup_path(&path)).unwrap(),
        original
    );

    for (name, document, owned) in [
        (
            "owned-observe",
            r#"{"schema":3,"name":"mine","version":"1","mode":"observe","grants":[]}"#,
            true,
        ),
        (
            "admin-named",
            r#"{"schema":3,"name":"theirs","version":"1","grants":[]}"#,
            false,
        ),
    ] {
        let path = temporary(name);
        fs::write(&path, document).unwrap();
        let facade = if owned {
            GovernanceFacade::owning_user_policy(path.to_path_buf())
        } else {
            GovernanceFacade::new(Some(path.to_path_buf()))
        };
        assert_eq!(
            facade
                .snapshot()
                .authorize_capability(Capability::Read)
                .reason,
            ReasonCode::InvalidAuthority,
            "{name}"
        );
        assert!(!facade.diagnostics().authority_valid, "{name}");
        assert_eq!(fs::read_to_string(&path).unwrap(), document, "{name}");
        assert!(!migrate::backup_path(&path).exists(), "{name}");
    }
}

/// ACCEPTANCE Governance 12, desktop gate 19 and ADR-0126 Decision 4; arch-1 P10 (A-01, 570528ad,
/// 4476fda8) and 76e74030: a pause refuses the next effect and a resume admits it; an end refuses
/// until a new session, and nothing else undoes it; a new session never resumes a pause; a toggle
/// asks for a pause while active and a resume otherwise. Each state offers exactly its acts, each
/// change reaches the observer once, and a request that changes nothing never does.
#[test]
fn the_runtime_state_moves_only_through_the_acts_it_offers() {
    use std::sync::Mutex;
    use RuntimeControlIntent::{EndSession, Hold, Resume, StartSession, ToggleHold};
    use RuntimeControlState::{Active, Ended, Held};

    let offered = [
        (Active, &[Hold, EndSession][..]),
        (Held, &[Resume, EndSession][..]),
        (Ended, &[StartSession][..]),
    ];
    for (state, acts) in offered {
        assert_eq!(super::offers(state), acts, "{state:?}");
    }

    // (from, intent) -> the next state.
    let table = [
        (Active, ToggleHold, Held),
        (Active, Hold, Held),
        (Active, Resume, Active),
        (Active, EndSession, Ended),
        (Active, StartSession, Active),
        (Held, ToggleHold, Active),
        (Held, Hold, Held),
        (Held, Resume, Active),
        (Held, EndSession, Ended),
        (Held, StartSession, Held),
        (Ended, ToggleHold, Ended),
        (Ended, Hold, Ended),
        (Ended, Resume, Ended),
        (Ended, EndSession, Ended),
        (Ended, StartSession, Active),
    ];
    for (from, intent, next) in table {
        let facade = GovernanceFacade::new(None);
        match from {
            Active => {}
            Held => assert_eq!(facade.apply_runtime_intent(Hold), Held),
            Ended => assert_eq!(facade.apply_runtime_intent(EndSession), Ended),
        }
        let seen: Arc<Mutex<Vec<RuntimeControlState>>> = Arc::default();
        let recorder = Arc::clone(&seen);
        facade
            .controls()
            .observe(Arc::new(move |state| recorder.lock().unwrap().push(state)));

        assert_eq!(
            facade.apply_runtime_intent(intent),
            next,
            "{intent:?} from {from:?}"
        );
        assert_eq!(facade.runtime_state(), next);
        let decision = facade.runtime_decision();
        match next {
            Active => assert!(decision.allowed, "{intent:?} from {from:?}"),
            Held => assert_eq!(decision.reason, ReasonCode::RuntimeHold),
            Ended => assert_eq!(decision.reason, ReasonCode::SessionEnded),
        }
        let expected: &[RuntimeControlState] = if next == from { &[] } else { &[next] };
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            expected,
            "{intent:?} from {from:?}"
        );
    }
}

/// The organization bundle's name in the directory `ManagedPaths::signed_at` signs into.
const ORGANIZATION_BUNDLE: &str = "org.bundle";

/// 76e74030, ADR-0197, ACCEPTANCE Governance 22 and docs/1.0/ARCHITECTURE.md (one refresher):
/// readers take the published snapshot and never read or wait on a policy source. While the
/// refresher holds the sources lock, the snapshot, the diagnostics, the chip, the compiled view
/// and the browser startup still answer. An edit to the user file or to the organization's local
/// bundle is invisible to every reader until the next tick publishes it, a file whose existence,
/// modified time and size are unchanged is not read again, and a tick that finds nothing changed
/// publishes nothing.
#[test]
fn a_policy_change_reaches_readers_only_through_the_refresher() {
    let organization = |name: &str| policy(name, all_open_grant(), "[]");
    let root = Scratch::named("refresher-organization");
    let paths = managed::ManagedPaths::signed_at(&root, &organization("org-a"), 12);
    let bundle = root.join(ORGANIZATION_BUNDLE);
    let local = temporary("refresher-user");
    fs::write(
        &local,
        policy(
            "read",
            r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]"#,
            "[]",
        ),
    )
    .unwrap();
    let facade = GovernanceFacade::with_managed_paths(Some(local.to_path_buf()), paths);
    let first = facade.snapshot();
    assert_eq!(first.managed_sequence(), Some(12));

    let sources = facade.hold_sources();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = facade.clone();
    let thread = std::thread::spawn(move || {
        let _ = reader.snapshot();
        let _ = reader.diagnostics();
        let _ = reader.policy_chip();
        let _ = reader.effective_authority();
        let _ = reader.browser_startup();
        let _ = sender.send(());
    });
    // The deadline only bounds a hang: a reader that waits on the lock never answers while the
    // test holds it.
    let answered = receiver.recv_timeout(Duration::from_secs(10));
    drop(sources);
    thread.join().unwrap();
    answered.expect("a reader waited on the sources lock");

    facade.refresh_tick();
    assert!(
        Arc::ptr_eq(&first, &facade.snapshot()),
        "a tick that finds nothing changed publishes nothing"
    );

    rewrite(
        &local,
        policy(
            "action",
            r#"[{"id":"action","hosts":{"allow":["*"]},"allowed":["action"]}]"#,
            "[]",
        ),
    );
    assert_eq!(
        facade.snapshot().id(),
        first.id(),
        "an edit is invisible to every reader until the refresher publishes it"
    );
    facade.refresh_tick();
    let edited = facade.snapshot();
    assert_ne!(edited.id(), first.id());
    assert!(!edited.authorize_capability(Capability::Read).allowed);
    assert!(edited.authorize_capability(Capability::Action).allowed);

    let state = fs::metadata(&bundle).unwrap();
    // Re-signed at the next sequence with the same byte length.
    managed::ManagedPaths::signed_at(&root, &organization("org-b"), 13);
    assert_eq!(fs::metadata(&bundle).unwrap().len(), state.len());
    set_modified(&bundle, state.modified().unwrap());
    facade.refresh_tick();
    assert_eq!(
        facade.snapshot().managed_sequence(),
        Some(12),
        "a file whose existence, modified time and size are unchanged is not read again"
    );

    set_modified(&bundle, state.modified().unwrap() + Duration::from_secs(2));
    assert_eq!(facade.snapshot().managed_sequence(), Some(12));
    facade.refresh_tick();
    let republished = facade.snapshot();
    assert_eq!(republished.managed_sequence(), Some(13));
    assert_ne!(republished.id(), first.id());
    assert_ne!(republished.id(), edited.id());
}

/// 97ddf868 and 994a6ffa (arch-1 P9, G2-02): every user-policy file Ghostlight writes is readable
/// only by the user: an authored policy and its directory, and a schema-3 file moved at load and
/// its backup, even when the old file was open to others.
#[test]
fn every_user_policy_file_ghostlight_writes_is_owner_only() {
    let directory = Scratch::named("authored-policy");
    let path = directory.join("policy").join("user.json");
    let facade = GovernanceFacade::owning_user_policy(path.clone());
    facade
        .apply_user_policy(&policy("mine", "[]", "[]"))
        .unwrap();
    assert_owner_only(&path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let parent = fs::metadata(path.parent().unwrap()).unwrap();
        assert_eq!(parent.permissions().mode() & 0o777, 0o700);
    }

    let moved = temporary("owned-schema-three-access");
    fs::write(
        &moved,
        r#"{"schema":3,"name":"mine","version":"1","mode":"enforce","grants":[]}"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&moved, fs::Permissions::from_mode(0o644)).unwrap();
    }
    let facade = GovernanceFacade::owning_user_policy(moved.to_path_buf());
    assert!(facade.diagnostics().authority_valid);
    assert!(manifest::parse(&fs::read_to_string(&moved).unwrap(), "moved").is_ok());
    assert_owner_only(&moved);
    assert_owner_only(&migrate::backup_path(&moved));
}

/// Contract with every audit log already written and the SIEM rules built on it (cacb1062): the
/// denial id of each kind of policy refusal -- a grant's, screenshots, a channel, never-touch, tab
/// close and audit availability -- is derived from its layer, grant and rule and must not move.
#[test]
fn every_policy_refusal_keeps_its_denial_id() {
    let snapshot = snapshot_for(
        "denial-ids",
        policy(
            "denials",
            r#"[{"id":"reader","hosts":{"allow":["example.com"]},"allowed":["read"]}]"#,
            r#"[{"key":"browser.screenshots.enabled","value":false,"level":"mandatory"},
               {"key":"browser.tabs.allow_close","value":false,"level":"mandatory"},
               {"key":"channels.mcp.enabled","value":false,"level":"mandatory"},
               {"key":"content.security.sacred_domains","value":["bank.example"],"level":"mandatory"},
               {"key":"audit.availability","value":"require_audit","level":"mandatory"}]"#,
        ),
    );
    let grant = snapshot.authorize_landing(Capability::Write, "https://example.com/");
    assert_eq!(
        snapshot.attribution(grant),
        Some((USER_TIER, Some("reader")))
    );
    let refusals = [
        grant,
        snapshot.authorize_screenshot(),
        snapshot.channel_refusal(IntakeChannel::Mcp).unwrap().0,
        snapshot.authorize_landing(Capability::Read, "https://bank.example/"),
        snapshot.authorize_tab_close(),
        snapshot.authorize_audit(false),
    ];
    let ids: Vec<String> = refusals
        .iter()
        .map(|refusal| {
            refusal
                .denial_id()
                .expect("a policy refusal has a denial id")
        })
        .collect();
    // The ids an audit log already holds: a refactor of how they are built must not move them.
    assert_eq!(
        ids,
        [
            "D-bf3492e7",
            "D-4b81180e",
            "D-debaf9fd",
            "D-0b9ad60a",
            "D-e2dfd3a3",
            "D-899efd47"
        ]
    );
}
