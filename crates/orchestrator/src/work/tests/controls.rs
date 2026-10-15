//! Inspect and find items carry a control's value and what it accepts, only when the page sent
//! them, and a credential-class control carries nothing but its masked value (ADR-0198
//! decision 7).

use super::*;

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
