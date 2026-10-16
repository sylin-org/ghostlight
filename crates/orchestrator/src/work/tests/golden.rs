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

#[test]
fn a_listed_tab_matches_the_success_golden() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        None,
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
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    check_golden(
        SUCCESS_GOLDEN,
        &golden_text(&rendered_result("browser_tabs", arguments, &result)),
    );
}

#[test]
fn a_find_of_repeated_controls_matches_its_golden() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        None,
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
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    check_golden(
        REPEATED_FIND_GOLDEN,
        &golden_text(&rendered_result("browser_find", arguments, &result)),
    );
}

#[test]
fn a_call_that_does_not_decode_matches_the_refusal_golden() {
    let (executor, _, _, workspace, _) = fixture();
    let arguments = json!({"action":"rearrange"});
    let result = executor.execute(
        &workspace,
        "browser_tabs",
        arguments.clone(),
        None,
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

#[test]
fn a_navigate_receipt_matches_the_audit_success_golden() {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        None,
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
        None,
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
                browser.push(Err(BrowserError::StaleLocator { reason }));
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
                    None,
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
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.facts["stale"], reason.word(), "{result:?}");
        rendered.insert(
            reason.word().into(),
            rendered_result("browser_click", arguments, &result),
        );
    }
    check_golden(
        STALE_REFUSALS_GOLDEN,
        &golden_text(&serde_json::Value::Object(rendered)),
    );
}
