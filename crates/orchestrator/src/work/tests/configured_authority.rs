//! Configured authority: one explanation per frame, and a typed effect kept under a landing that
//! denies its check.

use super::*;
use crate::work::InvocationContext;
use std::time::Instant;

/// `policy_explain` reports the authority its frame enforces, even when the policy changes between
/// the frame's start and the explanation, so the explanation and the frame's audit line, which
/// records the snapshot's id, name one authority (ADR-0203 Decision 7.5).
#[test]
fn policy_explain_and_its_audit_line_name_one_authority() {
    let policy = TestPolicy::new();
    let (executor, _, _, workspace, _) = fixture_with_governance(policy.facade());
    let publication = executor.governance.published();
    let enforced = publication.authority().clone();
    assert_eq!(enforced.id, publication.snapshot().id());
    policy.set(json!(["read"]));
    let newer = executor.governance.published();
    assert_ne!(newer.authority().id, enforced.id, "the edit was published");
    assert_ne!(newer.authority().capabilities, enforced.capabilities);

    let cancellation = CancellationToken::default();
    let context = InvocationContext {
        settlement_pending: &std::sync::atomic::AtomicBool::new(false),
        settlement_requested: false,
        provenance: &crate::provenance::ConnectionEvidence::claimed("test", IntakeChannel::Mcp),
        invocation: &crate::work::InvocationId::mint(),
        workspace: &workspace,
        requested_browser: None,
        requested_tab: None,
        requested_target: None,
        repeat_tool: None,
        gesture: crate::language::page_refusal::PageGesture::Other,
        snapshot: publication.snapshot(),
        publication: &publication,
        requirements: crate::governance::CapabilitySet::EMPTY,
        deadline: Instant::now() + Duration::from_secs(1),
        deadline_adjustable: false,
        cancellation: &cancellation,
        findings: &crate::work::findings::FrameFindings::default(),
    };
    let explained = executor.explain_policy(&context);
    assert_eq!(explained.result.status, Status::Succeeded);
    assert_eq!(
        explained.result.facts["capabilities"],
        json!(enforced.capabilities)
    );
    assert_eq!(context.snapshot.id(), enforced.id);
}

/// b9c2da60: typing that landed stays applied when the landing denies the check that would
/// follow it, and the tab is not held for it. Folds into `tests/tabs.rs` when that file is cut.
#[test]
fn a_read_denied_typing_postcondition_preserves_the_applied_landing_without_holding_it() {
    for location in ["target", "focused", "selector"] {
        for composed in [false, true] {
            let policy = temporary_policy("typing-postcondition-landing");
            fs::write(&policy, serde_json::to_vec(&json!({
                "schema":4,"name":"Observe source, act at destination","version":"1","grants":[
                    {"id":"source","hosts":{"allow":["example.com"]},"allowed":["read","action"]},
                    {"id":"landing","hosts":{"allow":["landing.example"]},"allowed":["action"]}
                ]
            })).unwrap()).unwrap();
            let (executor, browser, _, workspace, audit) =
                fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));
            browser.push(Ok(BrowserOutcome::TabOpened {
                tab: tab(7, "https://example.com/"),
                committed_urls: vec!["https://example.com/".into()],
            }));
            let opened = executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":"https://example.com/"}),
                &CancellationToken::default(),
            );
            assert_eq!(opened.status, Status::Succeeded);
            let field = ObservedTarget {
                locator: "draft".into(),
                role: "textbox".into(),
                name: "Draft".into(),
                state: vec![],
                credential_class: false,
                ..Default::default()
            };
            let mut arguments = json!({"tab":opened.facts["tab"],"text":"PRIVATE_DRAFT","expect":{"condition":"load_ready"}});
            if location == "target" {
                browser.push(Ok(BrowserOutcome::targets(7, vec![field.clone()])));
                let inspected = executor.execute(
                    &workspace,
                    "browser_inspect",
                    json!({}),
                    &CancellationToken::default(),
                );
                assert_eq!(inspected.status, Status::Succeeded);
                arguments["target"] = inspected.facts["items"][0]["target"].clone();
            } else if location == "focused" {
                arguments["focused"] = json!(true);
            } else {
                arguments["selector"] = json!({"name":"Draft","role":"textbox","exact":true});
                browser.push(Ok(BrowserOutcome::targets(7, vec![field.clone()])));
            }
            browser.push(Ok(BrowserOutcome::TargetsDescribed {
                rebound: false,
                tab_id: 7,
                targets: vec![field],
            }));
            browser.push(Ok(BrowserOutcome::Typed {
                tab: tab(7, "https://landing.example/"),
                character_count: 13,
                subject: None,
                committed_urls: vec!["https://landing.example/".into()],
            }));
            let before = browser.calls().len();
            let invocation = if composed {
                json!({"steps":[{"id":"draft","tool":"browser_type_text","arguments":arguments}]})
            } else {
                arguments
            };

            let result = executor.execute(
                &workspace,
                if composed {
                    "browser_flow"
                } else {
                    "browser_type_text"
                },
                invocation,
                &CancellationToken::default(),
            );
            let label = format!("{location}, composed={composed}");
            assert_eq!(result.status, Status::Blocked, "{label}: {result:?}");
            assert_eq!(
                result.effect,
                if composed {
                    Effect::Partial
                } else {
                    Effect::Applied
                },
                "{label}"
            );
            if composed {
                assert_eq!(
                    result.facts["steps"][0]["result"]["effect"], "applied",
                    "{label}: composition retains the child's confirmed effect"
                );
            }
            assert!(!result.repeat_safe, "{label}");
            let facts = if composed {
                &result.facts["steps"][0]["result"]["facts"]
            } else {
                &result.facts
            };
            assert!(
                facts.get("held").is_none(),
                "{label}: failed observation must not masquerade as a landing hold"
            );
            assert_eq!(
                browser.calls().len(),
                before + if location == "selector" { 3 } else { 2 },
                "{label}: no observation of the read-denied landing"
            );
            let records = audit.0.lock().unwrap();
            let checks: Vec<_> = records
                .iter()
                .filter(|record| record.invocation == result.invocation)
                .flat_map(|record| &record.permissions.checks)
                .collect();
            assert!(
                checks.iter().any(|check| check.requirements
                    == crate::governance::CapabilitySet::ACTION
                    && check.allowed),
                "{label}: acknowledged action landing is admitted"
            );
            assert!(
                checks.iter().any(|check| !check.allowed),
                "{label}: separate observation denial remains visible"
            );
            assert!(!serde_json::to_string(&*records)
                .unwrap()
                .contains("PRIVATE_DRAFT"));
            drop(records);
            browser.push(Ok(BrowserOutcome::KeyPressed {
                tab: tab(7, "https://landing.example/"),
                key: "Tab".into(),
                subject: None,
                committed_urls: vec![],
            }));
            let follow_up = executor.execute(
                &workspace,
                "browser_press_key",
                json!({"key":"Tab"}),
                &CancellationToken::default(),
            );
            assert_eq!(
                follow_up.status,
                Status::Succeeded,
                "{label}: the admitted landing stays usable: {follow_up:?}"
            );
        }
    }
}
