//! A stale handle's one trusted replacement (ADR-0198 Decision 28): the lookup the page makes in
//! the failing call, the candidates the service registers as handles under read authority, and
//! the repeat step that appears only when the page proved exactly one control is the same.

use super::execution::workspace_with_a_target;
use super::*;
use crate::browser::adapter::PhysicalSemantic;
use crate::browser::adapter::{StaleCandidate, StaleLookup};
use crate::language::outcome::{StaleHelp, WorkspaceReason};

/// One control a page found again, with its row context.
fn candidate(locator: &str, context: &str, nth: Option<u32>, same: bool) -> StaleCandidate {
    StaleCandidate {
        target: ObservedTarget {
            locator: locator.into(),
            role: "button".into(),
            name: "Delete".into(),
            context: Some(context.into()),
            nth,
            ..Default::default()
        },
        same,
    }
}

/// The lookup of a page that found three buttons and proved the second is the same control.
pub(super) fn one_same() -> StaleLookup {
    StaleLookup {
        matched: 3,
        same_count: 1,
        truncated: false,
        candidates: vec![
            candidate("alpha", "Alpha", None, false),
            candidate("beta", "Beta", None, true),
            candidate("gamma", "Gamma", None, false),
        ],
    }
}

/// The failure of a call whose handle the page found removed, with what it found again.
pub(super) fn removed(lookup: StaleLookup) -> BrowserError {
    BrowserError::StaleLocator {
        reason: StaleReason::Removed,
        lookup: Some(lookup),
    }
}

fn click(
    executor: &ApplicationExecutor,
    workspace: &crate::workspace::WorkspaceId,
    tab_handle: &str,
    target: &str,
) -> crate::work::result::InvocationResult {
    executor.execute(
        workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target}),
        &CancellationToken::default(),
    )
}

fn queried(browser: &FakeBrowser) -> usize {
    browser
        .calls()
        .iter()
        .filter(|call| matches!(call, BrowserCommand::QuerySemantic(PhysicalSemantic { .. })))
        .count()
}

/// A click that succeeds, for a handle of the test's choosing.
fn delete_click_with(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
    tab_handle: &str,
    target: &str,
) -> crate::work::result::InvocationResult {
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: Some(PhysicalActionSubject {
            role: "button".into(),
            name: "Delete".into(),
            rebound: false,
        }),
    }));
    click(executor, workspace, tab_handle, target)
}

/// A workspace under a policy that withholds target names, holding one inspected button, with
/// the policy file, which goes when its guard drops.
fn workspace_without_target_names(
    name: &str,
) -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    crate::workspace::WorkspaceId,
    String,
    String,
    Scratch,
) {
    let policy = temporary_policy(name);
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let (executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![ObservedTarget {
            locator: "delete".into(),
            role: "button".into(),
            name: "Delete patient".into(),
            ..Default::default()
        }],
    )));
    let inspected = executor.execute(
        &workspace,
        "browser_inspect",
        json!({"tab":tab_handle}),
        &CancellationToken::default(),
    );
    let target = inspected.facts["items"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();
    (executor, browser, workspace, tab_handle, target, policy)
}

/// What the model is told of a stale handle: its sentence and its next steps.
fn told(reason: StaleReason, help: StaleHelp) -> (String, Vec<String>) {
    let refusal = WorkspaceReason::StaleTarget { reason, help };
    (refusal.summary(), refusal.next_steps())
}

/// The sentence and next steps of `result`.
fn heard(result: &crate::work::result::InvocationResult) -> (String, Vec<String>) {
    (result.summary.clone(), result.next_steps.clone())
}

/// A page that found two rows like the removed one, proving neither the same, listed out of
/// order and with a third control that has no place among repeats.
fn none_same() -> StaleLookup {
    StaleLookup {
        matched: 3,
        same_count: 0,
        truncated: false,
        candidates: vec![
            candidate("two", "Neon Rain", Some(2), false),
            candidate("none", "Other", None, false),
            candidate("one", "Neon Rain", Some(1), false),
        ],
    }
}

/// ACCEPTANCE Executor 6, LANGUAGE stale handles and ADR-0198 Decision 28 (43eff032): a removed
/// handle names a replacement only when the page proved one, the tool takes one, and governance
/// reveals it.
///
/// - Proven: the failing call's own lookup names exactly one control the same; the refusal names
///   it among the candidates, each a working handle, keeps the verdict in the service, sends
///   nothing twice, and the repeat step names the call's own tool, for every single-target tool.
///   Listing candidates by their place among repeats never changes which one was proven.
/// - Not proven (no control the same, two the same, or a lookup cut short): candidates and the
///   pick step, in the order of their place. Nothing found: the plain refusal.
/// - A fill or a drag names several targets and is offered no replacement; nor is a handle's
///   second use after it was marked removed, nor a call that began an effect.
/// - After a navigation the step is browser_inspect, with candidates as working handles.
/// - Without target names the replacement is still named, the candidates carry only role and
///   place, and the pick step asks for nothing nobody sent; without Read there are no
///   candidates at all.
/// - The audit holds no handle, row or candidate.
#[test]
fn a_removed_handle_names_a_replacement_only_when_the_page_proved_one() {
    // Proven: one trusted replacement, and nothing sent twice.
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    let before = browser.calls().len();
    browser.push(Err(removed(one_same())));
    let refused = click(&executor, &workspace, &tab_handle, &target);
    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.effect, Effect::None);
    assert!(refused.repeat_safe);
    assert_eq!(refused.facts["reason"], "stale_target");
    assert_eq!(refused.facts["stale"], "removed");
    let replacement = refused.facts["replacement"].as_str().unwrap().to_owned();
    assert_ne!(replacement, target);
    assert_eq!(
        heard(&refused),
        told(
            StaleReason::Removed,
            StaleHelp::Replacement {
                tool: "browser_click",
                target: replacement.clone()
            }
        )
    );
    let candidates = refused.facts["recovery_candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 3);
    assert_eq!(candidates[1]["target"], replacement.as_str());
    assert_eq!(
        (
            &candidates[1]["context"],
            &candidates[1]["name"],
            &candidates[1]["role"]
        ),
        (&json!("Beta"), &json!("Delete"), &json!("button"))
    );
    assert!(
        !refused.facts.to_string().contains("same"),
        "the verdict stays in the service"
    );
    assert_eq!(
        browser.calls().len() - before,
        1,
        "the click was dispatched once"
    );
    assert_eq!(queried(&browser), 0, "the page's own answer is the lookup");
    let repeated = delete_click_with(&executor, &browser, &workspace, &tab_handle, &replacement);
    assert_eq!(repeated.status, Status::Succeeded, "the replacement works");

    // Every single-target tool repeats with the replacement.
    for (tool, input) in [
        ("browser_hover", json!({})),
        ("browser_type_text", json!({"text":"hello"})),
        ("browser_press_key", json!({"key":"Enter"})),
        ("browser_scroll", json!({})),
    ] {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(removed(one_same())));
        let mut arguments = input;
        arguments["tab"] = json!(tab_handle);
        arguments["target"] = json!(target);
        let refused = executor.execute(&workspace, tool, arguments, &CancellationToken::default());
        let replacement = refused.facts["replacement"]
            .as_str()
            .unwrap_or_else(|| panic!("{tool}: {:?}", refused.facts))
            .to_owned();
        assert_eq!(
            heard(&refused),
            told(
                StaleReason::Removed,
                StaleHelp::Replacement {
                    tool,
                    target: replacement
                }
            ),
            "{tool}"
        );
    }

    // Listed by place, the proven control stays the replacement.
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Err(removed(StaleLookup {
        matched: 2,
        same_count: 1,
        truncated: false,
        candidates: vec![
            candidate("two", "Neon Rain", Some(2), true),
            candidate("one", "Neon Rain", Some(1), false),
        ],
    })));
    let refused = click(&executor, &workspace, &tab_handle, &target);
    let replacement = refused.facts["replacement"].as_str().unwrap();
    let named = refused.facts["recovery_candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["target"] == replacement)
        .expect("the replacement is among the candidates");
    assert_eq!(named["nth"], 2, "the proven control is the one with nth 2");

    // Not proven: candidates in the order of their place, and the pick step.
    let mut two_same = none_same();
    two_same.same_count = 2;
    for entry in &mut two_same.candidates {
        entry.same = entry.target.context.as_deref() == Some("Neon Rain");
    }
    for (what, lookup) in [
        ("no control is the same", none_same()),
        ("two controls are the same", two_same),
        (
            "the lookup was cut short",
            StaleLookup {
                truncated: true,
                ..one_same()
            },
        ),
    ] {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(removed(lookup)));
        let refused = click(&executor, &workspace, &tab_handle, &target);
        assert_eq!(
            heard(&refused),
            told(StaleReason::Removed, StaleHelp::Candidates { named: true }),
            "{what}"
        );
        assert!(refused.facts.get("replacement").is_none(), "{what}");
        let candidates = refused.facts["recovery_candidates"].as_array().unwrap();
        assert!(candidates
            .iter()
            .all(|entry| entry["target"].as_str().unwrap().starts_with("target_")));
        if what == "no control is the same" {
            let places: Vec<_> = candidates.iter().map(|entry| entry.get("nth")).collect();
            assert_eq!(places, [Some(&json!(1)), Some(&json!(2)), None], "{what}");
            assert_eq!(candidates[0]["context"], "Neon Rain");
        }
    }

    // Nothing found: the plain refusal.
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Err(removed(StaleLookup::default())));
    let refused = click(&executor, &workspace, &tab_handle, &target);
    assert_eq!(heard(&refused), told(StaleReason::Removed, StaleHelp::None));
    assert!(refused.facts.get("replacement").is_none());
    assert!(refused.facts.get("recovery_candidates").is_none());

    // Several targets, a second use, or a begun effect: no replacement.
    for (tool, arguments) in [
        (
            "browser_fill_form",
            json!({"fields":[{"target":"TARGET","value":"x"}]}),
        ),
        (
            "browser_drag",
            json!({"source_target":"TARGET","destination_target":"TARGET"}),
        ),
    ] {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(removed(one_same())));
        let mut arguments: serde_json::Value =
            serde_json::from_str(&arguments.to_string().replace("TARGET", &target)).unwrap();
        arguments["tab"] = json!(tab_handle);
        let refused = executor.execute(&workspace, tool, arguments, &CancellationToken::default());
        assert_eq!(refused.facts["reason"], "stale_target", "{tool}");
        assert!(refused.facts.get("replacement").is_none(), "{tool}");
        assert_ne!(
            refused.next_steps,
            told(
                StaleReason::Removed,
                StaleHelp::Replacement {
                    tool,
                    target: target.clone()
                }
            )
            .1
        );
    }
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Err(removed(one_same())));
    click(&executor, &workspace, &tab_handle, &target);
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![ObservedTarget {
            locator: "delete-2".into(),
            role: "button".into(),
            name: "Delete".into(),
            ..Default::default()
        }],
    )));
    let again = click(&executor, &workspace, &tab_handle, &target);
    assert_eq!(again.facts["stale"], "removed");
    assert!(again.facts.get("replacement").is_none());
    assert_eq!(
        again.next_steps,
        told(StaleReason::Removed, StaleHelp::Candidates { named: true }).1
    );
    assert_eq!(
        again.facts["recovery_candidates"].as_array().unwrap().len(),
        1
    );
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    // The link restates any failure after a started effect as an unknown effect, so a stale
    // lookup can only arrive here before one. The help still refuses to name a replacement for
    // a failure the call reports after acting.
    browser.push(Err(BrowserError::EffectUnknown("x".into())));
    let refused = click(&executor, &workspace, &tab_handle, &target);
    assert!(refused.facts.get("replacement").is_none());

    // After a navigation: browser_inspect, with candidates as working handles.
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.com/next"),
        committed_urls: vec![],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/next","tab":tab_handle}),
        &CancellationToken::default(),
    );
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![ObservedTarget {
            locator: "delete-next".into(),
            role: "button".into(),
            name: "Delete".into(),
            context: Some("Next row".into()),
            ..Default::default()
        }],
    )));
    let before = browser.calls().len();
    let refused = click(&executor, &workspace, &tab_handle, &target);
    assert_eq!(refused.facts["stale"], "navigated");
    assert_eq!(
        refused.next_steps,
        told(
            StaleReason::Navigated,
            StaleHelp::Candidates { named: true }
        )
        .1
    );
    assert!(refused.facts.get("replacement").is_none());
    let candidates = refused.facts["recovery_candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["context"], "Next row");
    let handle = candidates[0]["target"].as_str().unwrap().to_owned();
    assert_eq!(
        browser.calls().len() - before,
        1,
        "only the candidate search crossed"
    );
    let clicked = delete_click_with(&executor, &browser, &workspace, &tab_handle, &handle);
    assert_eq!(
        clicked.status,
        Status::Succeeded,
        "a candidate is a working handle"
    );

    // Without target names: the replacement is named, the candidates carry role and place only.
    let (executor, browser, workspace, tab_handle, target, _policy) =
        workspace_without_target_names("replacement-no-names");
    let mut lookup = one_same();
    for entry in &mut lookup.candidates {
        entry.target.name = "Delete patient".into();
    }
    lookup.candidates[0].target.nth = Some(1);
    browser.push(Err(removed(lookup)));
    let refused = click(&executor, &workspace, &tab_handle, &target);
    let replacement = refused.facts["replacement"].as_str().unwrap().to_owned();
    assert_eq!(
        heard(&refused),
        told(
            StaleReason::Removed,
            StaleHelp::Replacement {
                tool: "browser_click",
                target: replacement
            }
        )
    );
    let candidates = refused.facts["recovery_candidates"].as_array().unwrap();
    for entry in candidates {
        assert!(
            entry.get("name").is_none() && entry.get("context").is_none(),
            "{entry}"
        );
        assert_eq!(entry["role"], "button");
    }
    assert_eq!(
        candidates[0]["nth"], 1,
        "the place among repeats is structure"
    );
    let shown = format!(
        "{} {:?} {}",
        refused.summary, refused.next_steps, refused.facts
    );
    assert!(
        !shown.contains("Delete patient") && !shown.contains("Beta"),
        "{shown}"
    );
    let (executor, browser, workspace, tab_handle, target, _policy) =
        workspace_without_target_names("pick-no-names");
    browser.push(Err(removed(none_same())));
    let refused = click(&executor, &workspace, &tab_handle, &target);
    assert!(refused.facts.get("replacement").is_none());
    assert_eq!(
        heard(&refused),
        told(StaleReason::Removed, StaleHelp::Candidates { named: false })
    );
    for entry in refused.facts["recovery_candidates"].as_array().unwrap() {
        assert!(
            entry.get("name").is_none() && entry.get("context").is_none(),
            "{entry}"
        );
    }

    // Without Read: no replacement and no candidates.
    let policy = temporary_policy("replacement-no-read");
    fs::write(
        &policy,
        r#"{"schema":4,"name":"work test","version":"1","grants":[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read","action"]},{"id":"act","hosts":{"allow":["*"]},"allowed":["action","write","execute"]}],"config":[]}"#,
    )
    .unwrap();
    let (executor, browser, workspaces, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.to_path_buf())));
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
    let lease = workspaces.acquire(&workspace).unwrap();
    let selected = lease.select_tab(Some(&tab_handle)).unwrap();
    let registered = lease
        .register_targets(
            &selected,
            &[ObservedTarget {
                locator: "delete".into(),
                role: "button".into(),
                name: "Delete".into(),
                ..Default::default()
            }],
        )
        .unwrap();
    let target = registered[0].0.as_str().to_owned();
    // The page moves to a host where the grants allow acting but not reading, without leaving
    // the document, so the handle is still the page's to report removed.
    lease
        .apply_same_document(&selected.handle, &tab(7, "https://elsewhere.test/"))
        .unwrap();
    drop(lease);
    browser.push(Err(removed(one_same())));
    let refused = click(&executor, &workspace, &tab_handle, &target);
    assert_eq!(refused.facts["stale"], "removed", "{:?}", refused.facts);
    assert!(refused.facts.get("replacement").is_none());
    assert!(refused.facts.get("recovery_candidates").is_none());
    assert_eq!(heard(&refused), told(StaleReason::Removed, StaleHelp::None));

    // The audit holds no handle, row or candidate.
    let (executor, browser, _, workspace, audit) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![ObservedTarget {
            locator: "private".into(),
            role: "button".into(),
            name: "Delete".into(),
            ..Default::default()
        }],
    )));
    let inspected = executor.execute(
        &workspace,
        "browser_inspect",
        json!({"tab":tab_handle}),
        &CancellationToken::default(),
    );
    let target = inspected.facts["items"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();
    browser.push(Err(removed(one_same())));
    let refused = click(&executor, &workspace, &tab_handle, &target);
    let replacement = refused.facts["replacement"].as_str().unwrap().to_owned();
    let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(encoded.contains("removed"), "{encoded}");
    for secret in [
        replacement.as_str(),
        target.as_str(),
        "Beta",
        "Alpha",
        "Gamma",
        "replacement",
        "recovery_candidates",
    ] {
        assert!(!encoded.contains(secret), "{secret} in {encoded}");
    }
}
