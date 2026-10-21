//! What a listed control may carry: a credential-class control carries nothing but its masked
//! value (ADR-0198 decision 7), and a repeated control's row reaches the model and never the audit
//! or history.

use super::*;

fn open_page(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
) {
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    let opened = executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded, "{}", opened.summary);
}

/// ACCEPTANCE Executor 7 and ADR-0198 Decision 7: a credential-class control shows only its
/// marker, whatever the adapter sent. Its value reads as hidden (an empty value stays empty, and
/// no value stays absent), its structure (area, form, state) stays, and every page-authored word
/// that could carry the secret or point at it (hint, error, options, limits, input type, group,
/// context, row) is dropped.
#[test]
fn a_credential_class_control_shows_only_its_marker_whatever_the_adapter_sent() {
    let secret = |value: Option<serde_json::Value>| ObservedTarget {
        locator: "0:doc_top:locator_pw".into(),
        role: "textbox".into(),
        name: "Password".into(),
        state: vec!["invalid".into()],
        credential_class: true,
        value,
        value_chars: Some(7),
        hint: Some("PRIVATE_HINT".into()),
        error: Some("PRIVATE_ERROR".into()),
        options: Some(json!(["PRIVATE_OPTION"])),
        limits: Some(json!({"maxlength":"20"})),
        input_type: Some("email".into()),
        group: Some("PRIVATE_GROUP".into()),
        context: Some("PRIVATE_CONTEXT".into()),
        nth: Some(1),
        area: Some("dialog".into()),
        form: Some(1),
        ..Default::default()
    };
    for (sent, shown) in [
        (
            Some(json!("hunter2")),
            Some(json!(crate::work::item::MASKED_VALUE)),
        ),
        (Some(json!("")), Some(json!(""))),
        (None, None),
    ] {
        let (executor, browser, _, workspace, _) = fixture();
        open_page(&executor, &browser, &workspace);
        browser.push(Ok(BrowserOutcome::targets(7, vec![secret(sent)])));
        let result = executor.execute(
            &workspace,
            "browser_inspect",
            json!({}),
            &CancellationToken::default(),
        );
        let item = &result.facts["items"][0];
        assert_eq!(item.get("value").cloned(), shown);
        assert_eq!(item["state"], json!(["invalid"]));
        assert_eq!(item["area"], json!("dialog"));
        assert_eq!(item["form"], json!(1));
        for dropped in [
            "hint",
            "error",
            "options",
            "limits",
            "input_type",
            "group",
            "context",
            "nth",
            "value_chars",
        ] {
            assert!(item.get(dropped).is_none(), "{dropped} in {item}");
        }
        let text = result.facts.to_string();
        assert!(
            !text.contains("hunter2") && !text.contains("PRIVATE_"),
            "{text}"
        );
    }
}

/// A menu button of a list row, as the page reports it when the name repeats.
fn row_button(locator: &str, context: &str, nth: Option<u32>) -> ObservedTarget {
    ObservedTarget {
        locator: format!("0:doc_top:{locator}"),
        role: "button".into(),
        name: "More options".into(),
        state: vec!["collapsed".into()],
        context: Some(context.into()),
        nth,
        ..Default::default()
    }
}

#[test]
fn repeated_controls_show_their_row_in_the_result_and_never_in_audit_or_history() {
    for (tool, arguments, key) in [
        ("browser_inspect", json!({}), "items"),
        ("browser_find", json!({"text":"More options"}), "matches"),
    ] {
        let (executor, browser, _, workspace, audit) = fixture();
        open_page(&executor, &browser, &workspace);
        browser.push(Ok(BrowserOutcome::targets(
            7,
            vec![
                row_button("a1", "Alpha", Some(1)),
                row_button("a2", "Alpha", Some(2)),
                row_button("b1", "Beta", None),
            ],
        )));

        let result = executor.execute(&workspace, tool, arguments, &CancellationToken::default());

        assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
        let places: Vec<_> = result.facts[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| (item["context"].clone(), item["nth"].clone()))
            .collect();
        assert_eq!(
            places,
            [
                (json!("Alpha"), json!(1)),
                (json!("Alpha"), json!(2)),
                (json!("Beta"), json!(null))
            ],
            "{tool}"
        );
        let records = audit.0.lock().unwrap();
        let history: Vec<_> = records
            .iter()
            .cloned()
            .map(crate::workbench::HistoryItem::from)
            .collect();
        for kept in [
            serde_json::to_string(&*records).unwrap(),
            serde_json::to_string(&history).unwrap(),
        ] {
            assert!(!kept.contains("Alpha") && !kept.contains("Beta"), "{tool}");
            assert!(
                !kept.contains("\"context\"") && !kept.contains("\"nth\""),
                "{tool}"
            );
        }
    }
}
