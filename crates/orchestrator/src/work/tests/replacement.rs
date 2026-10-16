//! A stale handle's one trusted replacement (ADR-0198 Decision 28): the lookup the page makes in
//! the failing call, the candidates the service registers as handles under read authority, and
//! the repeat step that appears only when the page proved exactly one control is the same.

use super::execution::{workspace_with_a_target, PICK_STEP, REMOVED_STEP};
use super::*;
use crate::browser::adapter::{StaleCandidate, StaleLookup};

const REPLACEMENT_SUMMARY: &str = "That target is no longer on the page. One control on the page has the same role, name and row.";

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
fn one_same() -> StaleLookup {
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
fn removed(lookup: StaleLookup) -> BrowserError {
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
        None,
        &CancellationToken::default(),
    )
}

fn queried(browser: &FakeBrowser) -> usize {
    browser
        .calls()
        .iter()
        .filter(|call| matches!(call, BrowserCommand::QuerySemantic { .. }))
        .count()
}

#[test]
fn a_same_document_re_render_names_the_one_trusted_replacement_and_nothing_is_sent_twice() {
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    let before = browser.calls().len();
    browser.push(Err(removed(one_same())));

    let refused = click(&executor, &workspace, &tab_handle, &target);

    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.effect, Effect::None);
    assert!(refused.repeat_safe);
    assert_eq!(refused.summary, REPLACEMENT_SUMMARY);
    assert_eq!(refused.facts["reason"], "stale_target");
    assert_eq!(refused.facts["stale"], "removed");
    let candidates = refused.facts["recovery_candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 3);
    let replacement = refused.facts["replacement"].as_str().unwrap();
    assert_eq!(candidates[1]["target"], replacement);
    assert_ne!(replacement, target);
    assert_eq!(candidates[1]["context"], "Beta");
    assert_eq!(candidates[1]["name"], "Delete");
    assert_eq!(candidates[1]["role"], "button");
    assert_eq!(
        refused.next_steps,
        vec![format!("Repeat browser_click with target {replacement}.")]
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

    let repeated = delete_click_with(&executor, &browser, &workspace, &tab_handle, replacement);
    assert_eq!(
        repeated.status,
        Status::Succeeded,
        "the replacement is a working handle"
    );
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
        }),
    }));
    click(executor, workspace, tab_handle, target)
}

#[test]
fn every_single_target_tool_repeats_with_the_replacement() {
    for (tool, input, repeat) in [
        ("browser_hover", json!({}), "browser_hover"),
        (
            "browser_type_text",
            json!({"text":"hello"}),
            "browser_type_text",
        ),
        (
            "browser_press_key",
            json!({"key":"Enter"}),
            "browser_press_key",
        ),
        ("browser_scroll", json!({}), "browser_scroll"),
    ] {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(removed(one_same())));
        let mut arguments = input;
        arguments["tab"] = json!(tab_handle);
        arguments["target"] = json!(target);

        let refused = executor.execute(
            &workspace,
            tool,
            arguments,
            None,
            &CancellationToken::default(),
        );

        let replacement = refused.facts["replacement"]
            .as_str()
            .unwrap_or_else(|| panic!("{tool}: {:?}", refused.facts));
        assert_eq!(
            refused.next_steps,
            vec![format!("Repeat {repeat} with target {replacement}.")],
            "{tool}"
        );
    }
}

#[test]
fn rows_the_page_could_not_prove_give_candidates_and_the_pick_step() {
    let cases = [
        (
            "no control is the same",
            StaleLookup {
                matched: 2,
                same_count: 0,
                truncated: false,
                candidates: vec![
                    candidate("one", "Neon Rain", Some(1), false),
                    candidate("two", "Neon Rain", Some(2), false),
                ],
            },
        ),
        (
            "two controls are the same",
            StaleLookup {
                matched: 2,
                same_count: 2,
                truncated: false,
                candidates: vec![
                    candidate("one", "Neon Rain", Some(1), true),
                    candidate("two", "Neon Rain", Some(2), true),
                ],
            },
        ),
        (
            "the lookup was cut short",
            StaleLookup {
                truncated: true,
                ..one_same()
            },
        ),
    ];
    for (what, lookup) in cases {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(removed(lookup)));

        let refused = click(&executor, &workspace, &tab_handle, &target);

        assert_eq!(
            refused.summary, "That target is no longer on the page.",
            "{what}"
        );
        assert_eq!(refused.next_steps, vec![PICK_STEP], "{what}");
        assert!(refused.facts.get("replacement").is_none(), "{what}");
        let candidates = refused.facts["recovery_candidates"].as_array().unwrap();
        assert!(!candidates.is_empty(), "{what}");
        assert!(candidates
            .iter()
            .all(|entry| entry["target"].as_str().unwrap().starts_with("target_")));
    }
}

#[test]
fn the_nth_of_a_repeated_row_reaches_the_model() {
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Err(removed(StaleLookup {
        matched: 2,
        same_count: 0,
        truncated: false,
        candidates: vec![
            candidate("one", "Neon Rain", Some(1), false),
            candidate("two", "Neon Rain", Some(2), false),
        ],
    })));

    let refused = click(&executor, &workspace, &tab_handle, &target);

    let candidates = refused.facts["recovery_candidates"].as_array().unwrap();
    assert_eq!(candidates[0]["context"], "Neon Rain");
    assert_eq!(candidates[0]["nth"], 1);
    assert_eq!(candidates[1]["nth"], 2);
}

#[test]
fn a_lookup_with_nothing_found_leaves_the_plain_refusal() {
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Err(removed(StaleLookup::default())));

    let refused = click(&executor, &workspace, &tab_handle, &target);

    assert_eq!(refused.summary, "That target is no longer on the page.");
    assert_eq!(refused.next_steps, vec![REMOVED_STEP]);
    assert!(refused.facts.get("replacement").is_none());
    assert!(refused.facts.get("recovery_candidates").is_none());
}

#[test]
fn a_fill_names_no_replacement_because_it_names_several_targets() {
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Err(removed(one_same())));

    let refused = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"tab":tab_handle,"fields":[{"target":target,"value":"x"}]}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(refused.facts["reason"], "stale_target");
    assert!(
        refused.facts.get("replacement").is_none(),
        "{:?}",
        refused.facts
    );
    assert!(
        refused
            .next_steps
            .iter()
            .all(|step| !step.starts_with("Repeat")),
        "{:?}",
        refused.next_steps
    );
}

#[test]
fn a_drag_names_no_replacement_because_it_names_two_targets() {
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Err(removed(one_same())));

    let refused = executor.execute(
        &workspace,
        "browser_drag",
        json!({"tab":tab_handle,"source_target":target,"destination_target":target}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(refused.facts["reason"], "stale_target");
    assert!(
        refused.facts.get("replacement").is_none(),
        "{:?}",
        refused.facts
    );
    assert!(
        refused
            .next_steps
            .iter()
            .all(|step| !step.starts_with("Repeat")),
        "{:?}",
        refused.next_steps
    );
}

#[test]
fn the_tombstoned_second_use_lists_candidates_but_never_a_replacement() {
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
    assert_eq!(again.next_steps, vec![PICK_STEP]);
    assert_eq!(
        again.facts["recovery_candidates"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn after_a_navigation_the_step_is_browser_inspect_with_candidates_as_handles() {
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.com/next"),
        committed_urls: vec![],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/next","tab":tab_handle}),
        None,
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
        vec!["Call browser_inspect to obtain current target handles."]
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
}

#[test]
fn without_target_names_the_replacement_is_named_and_candidates_carry_no_names_or_contexts() {
    let policy = temporary_policy("replacement-no-names");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let (executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
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
        None,
        &CancellationToken::default(),
    );
    let target = inspected.facts["items"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut lookup = one_same();
    for entry in &mut lookup.candidates {
        entry.target.name = "Delete patient".into();
    }
    lookup.candidates[0].target.nth = Some(1);
    browser.push(Err(removed(lookup)));

    let refused = click(&executor, &workspace, &tab_handle, &target);

    let replacement = refused.facts["replacement"].as_str().unwrap();
    assert_eq!(
        refused.next_steps,
        vec![format!("Repeat browser_click with target {replacement}.")]
    );
    let candidates = refused.facts["recovery_candidates"].as_array().unwrap();
    for entry in candidates {
        assert!(entry.get("name").is_none(), "{entry}");
        assert!(entry.get("context").is_none(), "{entry}");
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
    let _ = fs::remove_file(policy);
}

#[test]
fn without_read_authorization_there_is_no_replacement_and_no_candidates() {
    let policy = temporary_policy("replacement-no-read");
    fs::write(
        &policy,
        r#"{"schema":4,"name":"work test","version":"1","grants":[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read","action"]},{"id":"act","hosts":{"allow":["*"]},"allowed":["action","write","execute"]}],"config":[]}"#,
    )
    .unwrap();
    let (executor, browser, workspaces, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"]
        .as_str()
        .unwrap_or_else(|| panic!("{:?} {:?}", opened.summary, opened.facts))
        .to_owned();
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
    assert_eq!(refused.next_steps, vec![REMOVED_STEP]);
    assert_eq!(refused.summary, "That target is no longer on the page.");
    let _ = fs::remove_file(policy);
}

#[test]
fn the_audit_and_the_history_hold_no_handle_context_or_candidate() {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
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
        None,
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

#[test]
fn the_replacement_is_not_offered_after_an_effect_began() {
    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    // The link restates any failure after a started effect as an unknown effect, so a stale
    // lookup can only arrive here before one. The help still refuses to name a replacement for
    // a failure the call reports after acting.
    browser.push(Err(BrowserError::EffectUnknown("x".into())));

    let refused = click(&executor, &workspace, &tab_handle, &target);

    assert!(refused.facts.get("replacement").is_none());
    assert!(refused
        .next_steps
        .iter()
        .all(|step| !step.starts_with("Repeat browser_click with target")));
}
