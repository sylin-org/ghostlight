//! Caller restriction failures retain their source and the form operation's complete requirements.

use super::*;

#[test]
fn targeted_and_focused_typing_keep_action_authority_through_landing() {
    for focused in [false, true] {
        for (request, destination_action) in [("action", true), ("read", true), ("action", false)] {
            let policy = temporary_policy("typing-action-landing");
            fs::write(&policy, serde_json::to_vec(&json!({
                "schema":3,"name":"typing without write","version":"1","grants":[
                    {"id":"source","hosts":{"allow":["example.com"]},"allowed":["read","action"]},
                    {"id":"landing","hosts":{"allow":["landing.example"]},"allowed":[if destination_action {"action"} else {"read"}]}
                ]
            })).unwrap()).unwrap();
            let (executor, browser, _, workspace, audit) =
                fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
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
            let handle = &opened.facts["tab"];
            let field = ObservedTarget {
                locator: "draft".into(),
                role: "textbox".into(),
                name: "Draft".into(),
                state: vec![],
                credential_class: false,
            };
            let mut input =
                json!({"tab":handle,"text":"PRIVATE_DRAFT","restrict_capabilities":[request]});
            if focused {
                input["focused"] = json!(true);
            } else {
                browser.push(Ok(BrowserOutcome::Targets {
                    tab_id: 7,
                    targets: vec![field.clone()],
                }));
                let inspected = executor.execute(
                    &workspace,
                    "browser_inspect",
                    json!({"tab":handle}),
                    None,
                    &CancellationToken::default(),
                );
                input["target"] = inspected.facts["items"][0]["target"].clone();
            }
            let before = browser.calls().len();
            if request == "action" {
                browser.push(Ok(BrowserOutcome::TargetsDescribed {
                    tab_id: 7,
                    targets: vec![field],
                }));
                browser.push(Ok(BrowserOutcome::Typed {
                    tab: tab(7, "https://landing.example/"),
                    character_count: 13,
                    subject: None,
                    committed_urls: vec!["https://landing.example/".into()],
                }));
            }
            let result = executor.execute(
                &workspace,
                "browser_type_text",
                input,
                None,
                &CancellationToken::default(),
            );
            if request != "action" {
                assert_eq!(result.status, Status::Blocked);
                assert_eq!(result.effect, Effect::None);
                assert_eq!(browser.calls().len(), before);
                assert_eq!(result.facts["restriction"], "restrict_capabilities");
            } else {
                assert_eq!(result.effect, Effect::Applied);
                assert!(!result.repeat_safe);
                assert_eq!(
                    result.status,
                    if destination_action {
                        Status::Succeeded
                    } else {
                        Status::Blocked
                    },
                    "{result:?}"
                );
                assert_eq!(browser.calls().len(), before + 2);
                if !destination_action {
                    assert_eq!(result.facts["held"], true);
                }
                let records = audit.0.lock().unwrap();
                let checks = &records.last().unwrap().permissions.checks;
                assert!(checks
                    .iter()
                    .all(|check| check.requirements == crate::governance::CapabilitySet::ACTION));
                assert_eq!(checks.last().unwrap().allowed, destination_action);
            }
            fs::remove_file(policy).unwrap();
        }
    }
}

#[test]
fn an_unsent_form_needs_read_and_write_and_names_a_callers_missing_capability() {
    for restriction in [json!(["read"]), json!(["write"]), json!(["read", "write"])] {
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
        let handle = &opened.facts["tab"];
        let field = ObservedTarget {
            locator: "draft".into(),
            role: "textbox".into(),
            name: "Draft".into(),
            state: vec![],
            credential_class: false,
        };
        browser.push(Ok(BrowserOutcome::Targets {
            tab_id: 7,
            targets: vec![field.clone()],
        }));
        let inspected = executor.execute(
            &workspace,
            "browser_inspect",
            json!({"tab":handle}),
            None,
            &CancellationToken::default(),
        );
        let target = &inspected.facts["items"][0]["target"];
        let permitted = restriction.as_array().unwrap().len() == 2;
        if permitted {
            browser.push(Ok(BrowserOutcome::TargetsDescribed {
                tab_id: 7,
                targets: vec![field],
            }));
            browser.push(Ok(BrowserOutcome::Filled {
                tab: tab(7, "https://example.com/"),
                filled_count: 1,
                submitted: false,
                committed_urls: vec![],
            }));
        }
        let calls_before = browser.calls().len();
        let result = executor.execute(&workspace, "browser_fill_form", json!({"tab":handle,"restrict_capabilities":restriction,"fields":[{"target":target,"value":"PRIVATE_DRAFT"}]}), None, &CancellationToken::default());
        if permitted {
            assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
            assert_eq!(result.facts["submitted"], false);
            assert!(browser.calls().iter().any(|call| matches!(
                call,
                BrowserCommand::Fill {
                    submit_locator: None,
                    ..
                }
            )));
        } else {
            assert_eq!(result.status, Status::Blocked);
            assert_eq!(result.effect, Effect::None);
            assert_eq!(browser.calls().len(), calls_before);
            assert_eq!(result.facts["restriction"], "restrict_capabilities");
            assert_eq!(
                result.facts["required_capabilities"],
                json!(["read", "write"])
            );
            assert_eq!(result.summary, "Blocked by this call's restrict_capabilities; this operation requires read + write.");
            assert!(result.next_steps[0].contains("user's intended limits"));
            let records = audit.0.lock().unwrap();
            assert_eq!(records.last().unwrap().summary, result.summary);
            assert!(!serde_json::to_string(&*records)
                .unwrap()
                .contains("PRIVATE_DRAFT"));
        }
    }
}

#[test]
fn request_host_denial_does_not_blame_configured_policy_or_reveal_excluded_hosts() {
    let (executor, browser, _, workspace, _) = fixture();
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://excluded.example/","restrict_hosts":["allowed.example"]}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked);
    assert_eq!(result.facts["restriction"], "restrict_hosts");
    assert_eq!(result.summary, "Blocked by this call's restrict_hosts.");
    assert!(!result.summary.contains("excluded.example"));
    assert!(browser.calls().is_empty());
}
