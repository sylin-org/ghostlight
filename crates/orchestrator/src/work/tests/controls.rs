//! Inspect and find items carry a control's value and what it accepts, only when the page sent
//! them, and a credential-class control carries nothing but its masked value (ADR-0198
//! decision 7).

use super::*;
use crate::browser::adapter::Listing;
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
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    let opened = executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        None,
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

        let result = executor.execute(
            &workspace,
            tool,
            arguments,
            None,
            &CancellationToken::default(),
        );

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
        None,
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

#[test]
fn a_credential_class_item_shows_only_its_marker_even_if_the_adapter_sent_more() {
    let (executor, browser, _, workspace, _) = fixture();
    open_page(&executor, &browser, &workspace);
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![ObservedTarget {
            locator: "0:doc_top:locator_pw".into(),
            role: "textbox".into(),
            name: "Password".into(),
            state: vec!["invalid".into()],
            credential_class: true,
            value: Some(json!("hunter2")),
            hint: Some("your secret".into()),
            error: Some("is too short".into()),
            ..Default::default()
        }],
    )));

    let result = executor.execute(
        &workspace,
        "browser_inspect",
        json!({}),
        None,
        &CancellationToken::default(),
    );

    let item = &result.facts["items"][0];
    assert_eq!(item["value"], json!("[hidden]"));
    assert_eq!(item["state"], json!(["invalid"]));
    assert!(item.get("hint").is_none());
    assert!(item.get("error").is_none());
    assert!(!result.facts.to_string().contains("hunter2"));
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
            ..Listing::default()
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
        GovernanceFacade::new(None, None),
        GovernanceFacade::new(Some(hidden_names.clone()), None),
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
            None,
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
        None,
        &CancellationToken::default(),
    );

    assert!(result.facts.get("left_out").is_none());
    assert!(result.facts.get("modal_open").is_none());
    assert!(result.next_steps.is_empty());
}
