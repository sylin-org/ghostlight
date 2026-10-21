//! The `tools/call` results a model receives for one success and one refusal, pinned.
//!
//! Both calls run through the real executor with the fake browser, and the returned envelope is
//! rendered by a real MCP session, so the goldens hold what the product sends and not a rebuilt
//! copy of it.

use super::execution::workspace_with_a_target;
use super::*;
use crate::language::self_check::golden::{
    check_golden, golden_text, rendered_result, REFUSAL_GOLDEN, REPEATED_FIND_GOLDEN,
    STALE_REFUSALS_GOLDEN, SUCCESS_GOLDEN,
};
use crate::workspace::StaleReason;

/// A listed tab, and a click on a handle that followed its control through a re-render, rendered
/// the way a client receives them.
#[test]
fn successes_match_their_golden() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        &CancellationToken::default(),
    );
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(7, "https://example.com/")],
    }));
    let arguments = json!({"action":"list"});
    let result = executor.execute(
        &workspace,
        "browser_tabs",
        arguments.clone(),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    let mut rendered = serde_json::Map::new();
    rendered.insert(
        "tabs_listed".into(),
        rendered_result("browser_tabs", arguments, &result),
    );

    let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: Some(PhysicalActionSubject {
            role: "button".into(),
            name: "Delete".into(),
            rebound: true,
        }),
    }));
    let arguments = json!({"tab":tab_handle,"target":target});
    let result = executor.execute(
        &workspace,
        "browser_click",
        arguments.clone(),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    assert_eq!(result.facts["rebound"], true);
    rendered.insert(
        "followed_click".into(),
        rendered_result("browser_click", arguments, &result),
    );
    check_golden(
        SUCCESS_GOLDEN,
        &golden_text(&serde_json::Value::Object(rendered)),
    );
}

/// The model surface of a find over repeated controls (`find_repeated.json`): each control's
/// row and position, which the model reads to tell them apart.
#[test]
fn a_find_of_repeated_controls_matches_its_golden() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        &CancellationToken::default(),
    );
    let menu = |locator: &str, context: &str, nth: Option<u32>| ObservedTarget {
        locator: format!("0:doc_top:{locator}"),
        role: "button".into(),
        name: "More options".into(),
        state: vec!["collapsed".into()],
        context: Some(context.into()),
        nth,
        ..Default::default()
    };
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![
            menu("first", "Morning Light", Some(1)),
            menu("second", "Morning Light", Some(2)),
            menu("third", "Night Drive", None),
        ],
    )));
    let arguments = json!({"text":"More options"});
    let result = executor.execute(
        &workspace,
        "browser_find",
        arguments.clone(),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    check_golden(
        REPEATED_FIND_GOLDEN,
        &golden_text(&rendered_result("browser_find", arguments, &result)),
    );
}

/// The model surface of a call that does not match the catalog (`refusal.json`), its decoder
/// guidance included (d196b226).
#[test]
fn a_call_that_does_not_decode_matches_the_refusal_golden() {
    let (executor, _, _, workspace, _) = fixture();
    let arguments = json!({"action":"rearrange"});
    let result = executor.execute(
        &workspace,
        "browser_tabs",
        arguments.clone(),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Failed, "{result:?}");
    assert_eq!(result.facts["reason"], "invalid_input");
    assert!(
        result.facts["detail"].is_string(),
        "the refusal carries the detail its guidance points at: {result:?}"
    );
    check_golden(
        REFUSAL_GOLDEN,
        &golden_text(&rendered_result("browser_tabs", arguments, &result)),
    );
}

/// The audit line a real navigate writes matches `tests/golden/audit/success.json`, the record
/// shape the SIEM guide documents and the audit readers parse (5472f3b5).
#[test]
fn a_navigate_receipt_matches_the_audit_success_golden() {
    let (executor, browser, _, workspace, audit) = fixture();
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
    let handle = opened.facts["tab"].as_str().expect("an opened tab handle");
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    let navigated = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/","tab":handle}),
        &CancellationToken::default(),
    );
    assert_eq!(navigated.status, Status::Succeeded, "{navigated:?}");
    let written = serde_json::to_value(
        audit
            .0
            .lock()
            .unwrap()
            .last()
            .expect("the navigate left a receipt"),
    )
    .unwrap();
    let golden: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(crate::language::self_check::golden::repository_path(
            "tests/golden/audit/success.json",
        ))
        .expect("the audit success golden exists"),
    )
    .unwrap();
    for key in [
        "tool",
        "capabilities",
        "allowed",
        "status",
        "effect",
        "summary",
    ] {
        assert_eq!(
            written[key], golden[key],
            "the audit golden drifted on {key}"
        );
    }
}

/// What a click on a handle that went stale reads as, for each of the four words, rendered the way
/// a client receives it.
#[test]
fn a_stale_handle_matches_its_golden_for_each_word() {
    let mut rendered = serde_json::Map::new();
    for reason in StaleReason::ALL {
        let (executor, browser, workspaces, workspace, tab_handle, target) =
            workspace_with_a_target();
        match reason {
            StaleReason::Removed | StaleReason::Unknown => {
                browser.push(Err(BrowserError::stale(reason)));
            }
            StaleReason::Navigated => {
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
            }
            StaleReason::TabClosed => workspaces.apply_browser_close(FAKE_BROWSER, 7),
        }
        // A closed tab's own handle would refuse first, so that call names only the target.
        let arguments = if reason == StaleReason::TabClosed {
            json!({"target":target})
        } else {
            json!({"tab":tab_handle,"target":target})
        };
        let result = executor.execute(
            &workspace,
            "browser_click",
            arguments.clone(),
            &CancellationToken::default(),
        );
        assert_eq!(result.facts["stale"], reason.word(), "{result:?}");
        rendered.insert(
            reason.word().into(),
            rendered_result("browser_click", arguments, &result),
        );
    }
    for (case, proven) in [
        ("removed_with_replacement", true),
        ("removed_with_candidates", false),
    ] {
        let (executor, browser, _, workspace, tab_handle, target) = workspace_with_a_target();
        browser.push(Err(BrowserError::StaleLocator {
            reason: StaleReason::Removed,
            lookup: Some(row_lookup(proven)),
        }));
        let arguments = json!({"tab":tab_handle,"target":target});
        let result = executor.execute(
            &workspace,
            "browser_click",
            arguments.clone(),
            &CancellationToken::default(),
        );
        assert_eq!(result.facts["stale"], "removed", "{result:?}");
        rendered.insert(
            case.into(),
            rendered_result("browser_click", arguments, &result),
        );
    }
    check_golden(
        STALE_REFUSALS_GOLDEN,
        &golden_text(&serde_json::Value::Object(rendered)),
    );
}

/// What a page finds when it looks again for a removed menu button of a song row: the take of
/// the same title in the same row, which it proves or not, and the other take.
fn row_lookup(proven: bool) -> crate::browser::adapter::StaleLookup {
    use crate::browser::adapter::{StaleCandidate, StaleLookup};
    let take = |locator: &str, nth: u32, same: bool| StaleCandidate {
        target: ObservedTarget {
            locator: locator.into(),
            role: "button".into(),
            name: "Delete".into(),
            context: Some("Neon Rain".into()),
            nth: Some(nth),
            ..Default::default()
        },
        same,
    };
    StaleLookup {
        matched: 2,
        same_count: usize::from(proven),
        truncated: false,
        candidates: vec![take("take-two", 2, proven), take("take-one", 1, false)],
    }
}
