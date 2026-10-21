//! Explicit credential acknowledgement is per request and preserves configured human authority.

use super::*;
use crate::browser::adapter::{PhysicalFill, PhysicalTypeFocused, PhysicalTypeText};

fn credential() -> ObservedTarget {
    ObservedTarget {
        locator: "password".into(),
        role: "textbox".into(),
        name: "Password".into(),
        state: vec![],
        credential_class: true,
        ..Default::default()
    }
}

#[test]
fn user_authorized_credentials_reach_fill_target_and_focused_typing_without_entering_audit() {
    for mode in ["fill", "target", "focused"] {
        for composed in [false, true] {
            let (executor, browser, _, workspace, audit) = fixture();
            browser.push(Ok(BrowserOutcome::TabOpened {
                tab: tab(7, "https://example.com/"),
                committed_urls: vec![],
            }));
            let opened = executor.execute(
                &workspace,
                "browser_navigate",
                json!({"url":"https://example.com/"}),
                &CancellationToken::default(),
            );
            assert_eq!(opened.status, Status::Succeeded);
            browser.push(Ok(BrowserOutcome::targets(7, vec![credential()])));
            let inspected = executor.execute(
                &workspace,
                "browser_inspect",
                json!({}),
                &CancellationToken::default(),
            );
            let target = &inspected.facts["items"][0]["target"];
            let (tool, args) = match mode {
                "fill" => (
                    "browser_fill_form",
                    json!({"fields":[{"target":target,"value":"PRIVATE_CREDENTIAL"}],"user_authorized_credentials":true}),
                ),
                "target" => (
                    "browser_type_text",
                    json!({"target":target,"text":"PRIVATE_CREDENTIAL","user_authorized_credentials":true}),
                ),
                _ => (
                    "browser_type_text",
                    json!({"focused":true,"text":"PRIVATE_CREDENTIAL","user_authorized_credentials":true}),
                ),
            };
            browser.push(Ok(BrowserOutcome::TargetsDescribed {
                rebound: false,
                tab_id: 7,
                targets: vec![credential()],
            }));
            browser.push(Ok(if mode == "fill" {
                BrowserOutcome::Filled {
                    rebound: false,
                    tab: tab(7, "https://example.com/"),
                    filled_count: 1,
                    held: Vec::new(),
                    submitted: false,
                    committed_urls: vec![],
                }
            } else {
                BrowserOutcome::Typed {
                    tab: tab(7, "https://example.com/"),
                    character_count: 18,
                    subject: None,
                    committed_urls: vec![],
                }
            }));
            let result = executor.execute(
                &workspace,
                if composed { "browser_flow" } else { tool },
                if composed {
                    json!({"steps":[{"tool":tool,"arguments":args}]})
                } else {
                    args
                },
                &CancellationToken::default(),
            );
            assert_eq!(
                result.status,
                Status::Succeeded,
                "{mode}, composed={composed}: {result:?}"
            );
            assert!(browser.calls().iter().any(|command| matches!(
                command,
                BrowserCommand::Fill(PhysicalFill {
                    allow_credentials: true,
                    ..
                }) | BrowserCommand::TypeText(PhysicalTypeText {
                    allow_credentials: true,
                    ..
                }) | BrowserCommand::TypeFocused(PhysicalTypeFocused {
                    allow_credentials: true,
                    ..
                })
            )));
            assert!(!serde_json::to_string(&*audit.0.lock().unwrap())
                .unwrap()
                .contains("PRIVATE_CREDENTIAL"));
            assert!(!serde_json::to_string(&result)
                .unwrap()
                .contains("PRIVATE_CREDENTIAL"));

            // An earlier acknowledgement does not authorize a later credential request.
            browser.push(Ok(BrowserOutcome::TargetsDescribed {
                rebound: false,
                tab_id: 7,
                targets: vec![credential()],
            }));
            let later = executor.execute(
                &workspace,
                "browser_type_text",
                json!({"focused":true,"text":"NOT_AUTHORIZED"}),
                &CancellationToken::default(),
            );
            assert_eq!(later.status, Status::Blocked);
            assert_eq!(later.facts["values_sent"], false);
            assert!(later
                .next_steps
                .iter()
                .any(|step| step.contains("user_authorized_credentials")));
        }
    }
}

#[test]
fn credential_guidance_obeys_flow_continue_without_pausing_later_work() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        &CancellationToken::default(),
    );
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: vec![credential()],
    }));
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "ordinary page".into(),
        truncated: false,
        title: "Example".into(),
        url: "https://example.com/".into(),
    }));
    let result = executor.execute(
        &workspace,
        "browser_flow",
        json!({"on_error":"continue","steps":[
            {"tool":"browser_type_text","arguments":{"focused":true,"text":"not-sent"}},
            {"tool":"browser_read","arguments":{}}
        ]}),
        &CancellationToken::default(),
    );
    assert_eq!(result.facts["steps"][0]["result"]["status"], "blocked");
    assert_eq!(result.facts["steps"][1]["result"]["status"], "succeeded");
    assert!(!browser.calls().iter().any(|command| matches!(
        command,
        BrowserCommand::TypeFocused(PhysicalTypeFocused { .. })
    )));
}

#[test]
fn credential_acknowledgement_cannot_override_configured_policy_or_human_controls() {
    for boundary in ["policy", "pause", "end"] {
        let policy = TestPolicy::new();
        let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
        browser.push(Ok(BrowserOutcome::TabOpened {
            tab: tab(7, "https://example.com/"),
            committed_urls: vec![],
        }));
        executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/"}),
            &CancellationToken::default(),
        );
        match boundary {
            "policy" => policy.set(json!(["read"])),
            "pause" => {
                executor
                    .governance
                    .apply_runtime_intent(RuntimeControlIntent::Hold);
            }
            _ => {
                executor
                    .governance
                    .apply_runtime_intent(RuntimeControlIntent::EndSession);
            }
        }
        let before = browser.calls().len();
        let result = executor.execute(
            &workspace,
            "browser_type_text",
            json!({"focused":true,"text":"not-sent","user_authorized_credentials":true}),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked, "{boundary}: {result:?}");
        assert_eq!(result.effect, Effect::None);
        assert_eq!(
            browser.calls().len(),
            before,
            "{boundary}: acknowledgement cannot grant authority"
        );
    }
}

#[test]
fn an_unauthorized_credential_fill_names_the_field_its_handle_and_the_way_forward() {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        &CancellationToken::default(),
    );
    browser.push(Ok(BrowserOutcome::targets(7, vec![credential()])));
    let inspected = executor.execute(
        &workspace,
        "browser_inspect",
        json!({}),
        &CancellationToken::default(),
    );
    let target = inspected.facts["items"][0]["target"]
        .as_str()
        .unwrap()
        .to_owned();
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: vec![credential()],
    }));

    let result = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"fields":[{"target":target,"value":"PRIVATE_CREDENTIAL"}]}),
        &CancellationToken::default(),
    );

    assert_eq!(result.status, Status::Blocked);
    assert_eq!(result.effect, Effect::None);
    assert_eq!(
        result.summary,
        format!("Nothing was typed. The \"Password\" text field ({target}) is credential-class.")
    );
    assert_eq!(
        result.next_steps,
        vec!["If the user explicitly authorized it, repeat with user_authorized_credentials true; otherwise ask the user to type it."]
    );
    assert_eq!(result.facts["values_sent"], false);
    let recorded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(!recorded.contains("Password"));
    assert!(!recorded.contains("PRIVATE_CREDENTIAL"));
}
