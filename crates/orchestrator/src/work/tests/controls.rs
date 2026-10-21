//! Inspect and find items carry a control's value and what it accepts, only when the page sent
//! them, and a credential-class control carries nothing but its masked value (ADR-0198
//! decision 7).

use super::*;
use crate::browser::adapter::Listing;
use crate::browser::adapter::PhysicalSemantic;
use crate::language::area::{LeftOut, LeftOutKey};

fn role_select() -> ObservedTarget {
    ObservedTarget {
        locator: "0:doc_top:locator_role".into(),
        role: "combobox".into(),
        name: "Role".into(),
        state: vec!["required".into()],
        credential_class: false,
        value: Some(json!("")),
        hint: Some("Choose one...".into()),
        options: Some(json!(["Developer", "Administrator", "Researcher", "Other"])),
        ..Default::default()
    }
}

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

#[test]
fn inspect_and_find_pass_a_selects_value_hint_and_options_through() {
    for (tool, arguments, key) in [
        ("browser_inspect", json!({}), "items"),
        ("browser_find", json!({"text":"Role"}), "matches"),
    ] {
        let (executor, browser, _, workspace, _) = fixture();
        open_page(&executor, &browser, &workspace);
        browser.push(Ok(BrowserOutcome::targets(7, vec![role_select()])));

        let result = executor.execute(&workspace, tool, arguments, &CancellationToken::default());

        assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
        let item = &result.facts[key][0];
        assert_eq!(item["state"], json!(["required"]), "{tool}");
        assert_eq!(item["value"], json!(""), "{tool}");
        assert_eq!(item["hint"], json!("Choose one..."), "{tool}");
        assert_eq!(
            item["options"],
            json!(["Developer", "Administrator", "Researcher", "Other"]),
            "{tool}"
        );
        assert!(item.get("options_total").is_none(), "{tool}");
        assert!(item.get("locator").is_none(), "{tool}");
    }
}

#[test]
fn a_control_without_the_new_keys_reads_as_before() {
    let (executor, browser, _, workspace, _) = fixture();
    open_page(&executor, &browser, &workspace);
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![ObservedTarget {
            locator: "0:doc_top:locator_go".into(),
            role: "button".into(),
            name: "Go".into(),
            ..Default::default()
        }],
    )));

    let result = executor.execute(
        &workspace,
        "browser_inspect",
        json!({}),
        &CancellationToken::default(),
    );

    let item = result.facts["items"][0].as_object().unwrap();
    let mut keys: Vec<_> = item.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["credential_class", "name", "role", "state", "target"]
    );
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

/// A listing of one nav link, for a page the cap cut.
fn nav_link() -> ObservedTarget {
    ObservedTarget {
        locator: "0:doc_top:locator_home".into(),
        role: "link".into(),
        name: "Patients".into(),
        area: Some("nav".into()),
        ..Default::default()
    }
}

fn cut_listing(left_out: LeftOut, modal_open: bool) -> BrowserOutcome {
    BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![nav_link()],
        listing: Listing {
            left_out,
            modal_open,
            truncated: true,
        },
    }
}

#[test]
fn a_cut_listing_reports_what_was_left_out_with_target_names_on_and_off() {
    let hidden_names = temporary_policy("left-out-names");
    fs::write(
        &hidden_names,
        all_open_policy_with(
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    for governance in [
        GovernanceFacade::new(None),
        GovernanceFacade::new(Some(hidden_names.to_path_buf())),
    ] {
        let (executor, browser, _, workspace, audit) = fixture_with_governance(governance);
        open_page(&executor, &browser, &workspace);
        let mut left_out = LeftOut::default();
        left_out.add(LeftOutKey::Chrome, 96);
        left_out.add(LeftOutKey::BehindDialog, 4);
        browser.push(Ok(cut_listing(left_out, true)));

        let result = executor.execute(
            &workspace,
            "browser_inspect",
            json!({}),
            &CancellationToken::default(),
        );

        assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
        assert_eq!(result.summary, "Found 1 control on example.com.");
        assert_eq!(
            result.facts["left_out"],
            json!({"chrome": 96, "behind_dialog": 4})
        );
        assert_eq!(result.facts["modal_open"], json!(true));
        assert_eq!(
            result.next_steps,
            vec!["96 header, nav and footer controls were left out to fit; raise max_items (up to 200) or use browser_find for one by name."]
        );
        assert!(!result.next_steps.concat().contains("Patients"));
        assert!(!result.summary.contains("Patients"));
        let audit = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(
            !audit.contains("left_out") && !audit.contains("96 header"),
            "{audit}"
        );
    }
}

#[test]
fn a_whole_listing_carries_no_left_out_keys() {
    let (executor, browser, _, workspace, _) = fixture();
    open_page(&executor, &browser, &workspace);
    browser.push(Ok(cut_listing(LeftOut::default(), false)));

    let result = executor.execute(
        &workspace,
        "browser_inspect",
        json!({}),
        &CancellationToken::default(),
    );

    assert!(result.facts.get("left_out").is_none());
    assert!(result.facts.get("modal_open").is_none());
    assert!(result.next_steps.is_empty());
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

#[test]
fn a_selector_matching_thirty_controls_says_at_least_eight() {
    for (truncated, summary) in [
        (
            true,
            "At least 8 visible controls matched the semantic selector; none was chosen.",
        ),
        (
            false,
            "8 visible controls matched the semantic selector; none was chosen.",
        ),
    ] {
        let (executor, browser, _, workspace, _) = fixture();
        open_page(&executor, &browser, &workspace);
        let save = |index: usize| ObservedTarget {
            locator: format!("0:doc_top:locator_save_{index}"),
            role: "button".into(),
            name: "Save".into(),
            ..Default::default()
        };
        browser.push(Ok(BrowserOutcome::Targets {
            tab_id: 7,
            targets: (0..8).map(save).collect(),
            listing: Listing {
                truncated,
                ..Listing::default()
            },
        }));
        let result = executor.execute(
            &workspace,
            "browser_click",
            json!({"selector":{"name":"Save"}}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Failed, "{}", result.summary);
        assert_eq!(result.summary, summary);
        assert!(browser.calls().iter().any(|call| matches!(
            call,
            BrowserCommand::QuerySemantic(PhysicalSemantic { max_results: 8, .. })
        )));
    }
}
