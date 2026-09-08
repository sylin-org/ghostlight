//! Configured authority preserves form, upload, and typing boundaries.

use super::*;

#[test]
fn selector_fills_find_ordinary_controls_without_weakening_credentials_or_submit_authority() {
    for composed in [false, true] {
        for (case, credential, submit, restrictions, expected) in [
            (
                "ordinary draft",
                false,
                false,
                json!(["read", "write"]),
                Status::Succeeded,
            ),
            (
                "credential",
                true,
                false,
                json!(["read", "write"]),
                Status::AttentionRequired,
            ),
            (
                "missing read",
                false,
                false,
                json!(["write"]),
                Status::Blocked,
            ),
            (
                "missing write",
                false,
                false,
                json!(["read"]),
                Status::Blocked,
            ),
            (
                "explicit submit",
                false,
                true,
                json!(["read", "action", "write"]),
                Status::Succeeded,
            ),
            (
                "missing submit action",
                false,
                true,
                json!(["read", "write"]),
                Status::Blocked,
            ),
        ] {
            let policy = TestPolicy::new();
            let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
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
            assert_eq!(opened.status, Status::Succeeded);
            let field = ObservedTarget {
                locator: "standalone-editor".into(),
                role: "textbox".into(),
                name: "Visible draft".into(),
                state: vec![],
                credential_class: credential,
            };
            let button = ObservedTarget {
                locator: "explicit-submit".into(),
                role: "button".into(),
                name: "Submit".into(),
                state: vec![],
                credential_class: false,
            };
            let mut arguments = json!({"tab":opened.facts["tab"],"fields":[{
                "selector":{"name":"Visible draft","role":"textbox","exact":true},
                "value":"PRIVATE_DRAFT"
            }]});
            if submit {
                browser.push(Ok(BrowserOutcome::Targets {
                    tab_id: 7,
                    targets: vec![button.clone()],
                }));
                let inspected = executor.execute(
                    &workspace,
                    "browser_inspect",
                    json!({}),
                    None,
                    &CancellationToken::default(),
                );
                assert_eq!(inspected.status, Status::Succeeded);
                arguments["submit_target"] = inspected.facts["items"][0]["target"].clone();
            }
            let before = browser.calls().len();
            browser.push(Ok(BrowserOutcome::Targets {
                tab_id: 7,
                targets: vec![field.clone()],
            }));
            if expected != Status::Blocked {
                browser.push(Ok(BrowserOutcome::TargetsDescribed {
                    tab_id: 7,
                    targets: if submit {
                        vec![field, button]
                    } else {
                        vec![field]
                    },
                }));
                if !credential {
                    browser.push(Ok(BrowserOutcome::Filled {
                        tab: tab(7, "https://example.com/"),
                        filled_count: 1,
                        submitted: submit,
                        committed_urls: vec![],
                    }));
                }
            }
            let invocation = if composed {
                json!({"steps":[{"id":"draft","tool":"browser_fill_form","arguments":arguments}]})
            } else {
                arguments
            };
            policy.set(restrictions);
            let result = executor.execute(
                &workspace,
                if composed {
                    "browser_flow"
                } else {
                    "browser_fill_form"
                },
                invocation,
                None,
                &CancellationToken::default(),
            );
            let label = format!("{case}, composed={composed}");
            assert_eq!(result.status, expected, "{label}: {result:?}");
            let commands = browser.calls();
            let requested = &commands[before..];
            if expected == Status::Blocked {
                assert_eq!(result.effect, Effect::None, "{label}");
                assert!(
                    requested.is_empty(),
                    "{label}: insufficient authority never queries or fills a control"
                );
            } else {
                assert!(
                    matches!(requested.first(), Some(BrowserCommand::QuerySemantic {
                    tab_id: 7, name, role: Some(role), exact: true, form_scope: false,
                }) if name == "Visible draft" && role == "textbox"),
                    "{label}: ordinary labeled controls need no form ancestor: {requested:?}"
                );
                assert!(
                    matches!(requested.get(1), Some(BrowserCommand::DescribeTargets { locators, .. })
                    if locators == &if submit { vec!["standalone-editor".to_string(), "explicit-submit".to_string()] } else { vec!["standalone-editor".to_string()] }),
                    "{label}: final credential preflight still checks every intended control"
                );
                if credential {
                    assert_eq!(result.effect, Effect::None);
                    assert_eq!(
                        requested.len(),
                        2,
                        "credential handoff sends no value or submission"
                    );
                    assert!(executor.workspaces.attention(&workspace).is_some());
                } else {
                    assert_eq!(
                        requested.len(),
                        3,
                        "{label}: one lookup, preflight, and fill"
                    );
                    assert!(
                        matches!(requested.last(), Some(BrowserCommand::Fill { fields, submit_locator, .. })
                        if fields.len() == 1 && fields[0].locator == "standalone-editor" && fields[0].value == "PRIVATE_DRAFT"
                            && submit_locator.as_deref() == if submit { Some("explicit-submit") } else { None }),
                        "{label}: only an explicit submit target permits submission"
                    );
                    let facts = if composed {
                        &result.facts["steps"][0]["result"]["facts"]
                    } else {
                        &result.facts
                    };
                    assert_eq!(facts["filled_count"], 1);
                    assert_eq!(facts["submitted"], submit);
                }
            }
            assert!(
                !serde_json::to_string(&*audit.0.lock().unwrap())
                    .unwrap()
                    .contains("PRIVATE_DRAFT"),
                "{label}: draft text never enters durable audit"
            );
        }
    }
}

#[test]
fn selector_uploads_find_standalone_inputs_and_keep_credential_preflight() {
    for composed in [false, true] {
        for credential in [false, true] {
            let policy = TestPolicy::new();
            let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
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
            assert_eq!(opened.status, Status::Succeeded);
            let input = ObservedTarget {
                locator: "standalone-file".into(),
                role: "button".into(),
                name: "Attachment".into(),
                state: vec![],
                credential_class: credential,
            };
            browser.push(Ok(BrowserOutcome::Targets {
                tab_id: 7,
                targets: vec![input.clone()],
            }));
            browser.push(Ok(BrowserOutcome::TargetsDescribed {
                tab_id: 7,
                targets: vec![input],
            }));
            if !credential {
                browser.push(Ok(BrowserOutcome::FilesUploaded {
                    tab_id: 7,
                    uploaded_count: 1,
                    uploaded_bytes: 1,
                    subject: None,
                }));
            }
            let before = browser.calls().len();
            let arguments = json!({
                "tab":opened.facts["tab"],
                "selector":{"name":"Attachment","role":"button","exact":true},
                "files":[{"name":"PRIVATE_FILE.txt","data_base64":"eA=="}]
            });
            let invocation = if composed {
                json!({"steps":[{"id":"attach","tool":"browser_upload","arguments":arguments}]})
            } else {
                arguments
            };

            let result = executor.execute(
                &workspace,
                if composed {
                    "browser_flow"
                } else {
                    "browser_upload"
                },
                invocation,
                None,
                &CancellationToken::default(),
            );
            assert_eq!(
                result.status,
                if credential {
                    Status::AttentionRequired
                } else {
                    Status::Succeeded
                },
                "composed={composed}, credential={credential}: {result:?}"
            );
            let commands = browser.calls();
            let requested = &commands[before..];
            assert!(
                matches!(requested.first(), Some(BrowserCommand::QuerySemantic {
                tab_id: 7, name, role: Some(role), exact: true, form_scope: false,
            }) if name == "Attachment" && role == "button"),
                "standalone file inputs need no form ancestor: {requested:?}"
            );
            assert!(
                matches!(requested.get(1), Some(BrowserCommand::DescribeTargets { tab_id: 7, locators }) if locators == &["standalone-file"])
            );
            if credential {
                assert_eq!(result.effect, Effect::None);
                assert_eq!(requested.len(), 2, "credential handoff sends no bytes");
                assert!(executor.workspaces.attention(&workspace).is_some());
            } else {
                assert_eq!(requested.len(), 3);
                assert!(
                    matches!(requested.last(), Some(BrowserCommand::UploadFiles { tab_id: 7, locator, files }) if locator == "standalone-file" && files.len() == 1 && files[0].name == "PRIVATE_FILE.txt" && files[0].data == "eA==" && files[0].size == 1)
                );
                let facts = if composed {
                    &result.facts["steps"][0]["result"]["facts"]
                } else {
                    &result.facts
                };
                assert_eq!(facts["uploaded_count"], 1);
                assert_eq!(facts["uploaded_bytes"], 1);
            }
            let saved = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
            assert!(!saved.contains("PRIVATE_FILE") && !saved.contains("eA=="));
        }
    }
}

#[test]
fn targeted_focused_and_selector_typing_keep_action_authority_through_landing() {
    for location in ["target", "focused", "selector"] {
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
            let mut input = json!({"tab":handle,"text":"PRIVATE_DRAFT"});
            if location == "focused" {
                input["focused"] = json!(true);
            } else if location == "selector" {
                input["selector"] = json!({"name":"Draft","role":"textbox","exact":true});
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
                if location == "selector" {
                    browser.push(Ok(BrowserOutcome::Targets {
                        tab_id: 7,
                        targets: vec![field.clone()],
                    }));
                }
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
            if request != "action" {
                let mut document: serde_json::Value =
                    serde_json::from_slice(&fs::read(&policy).unwrap()).unwrap();
                document["grants"][0]["allowed"] = json!(["read"]);
                fs::write(&policy, serde_json::to_vec(&document).unwrap()).unwrap();
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
                assert_eq!(result.facts["policy_rule"], "capability");
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
                assert_eq!(
                    browser.calls().len(),
                    before + if location == "selector" { 3 } else { 2 }
                );
                if !destination_action {
                    assert_eq!(result.facts["held"], true);
                } else {
                    assert!(result.facts.get("held").is_none());
                }
                let records = audit.0.lock().unwrap();
                let checks = &records.last().unwrap().permissions.checks;
                if location != "selector" {
                    assert!(checks.iter().all(
                        |check| check.requirements == crate::governance::CapabilitySet::ACTION
                    ));
                }
                assert_eq!(
                    checks.last().unwrap().requirements,
                    crate::governance::CapabilitySet::ACTION
                );
                assert_eq!(checks.last().unwrap().allowed, destination_action);
            }
            fs::remove_file(policy).unwrap();
        }
    }
}

#[test]
fn a_read_denied_typing_postcondition_preserves_the_applied_landing_without_holding_it() {
    for location in ["target", "focused", "selector"] {
        for composed in [false, true] {
            let policy = temporary_policy("typing-postcondition-landing");
            fs::write(&policy, serde_json::to_vec(&json!({
                "schema":3,"name":"Observe source, act at destination","version":"1","grants":[
                    {"id":"source","hosts":{"allow":["example.com"]},"allowed":["read","action"]},
                    {"id":"landing","hosts":{"allow":["landing.example"]},"allowed":["action"]}
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
            assert_eq!(opened.status, Status::Succeeded);
            let field = ObservedTarget {
                locator: "draft".into(),
                role: "textbox".into(),
                name: "Draft".into(),
                state: vec![],
                credential_class: false,
            };
            let mut arguments = json!({"tab":opened.facts["tab"],"text":"PRIVATE_DRAFT","expect":{"condition":"load_ready"}});
            if location == "target" {
                browser.push(Ok(BrowserOutcome::Targets {
                    tab_id: 7,
                    targets: vec![field.clone()],
                }));
                let inspected = executor.execute(
                    &workspace,
                    "browser_inspect",
                    json!({}),
                    None,
                    &CancellationToken::default(),
                );
                assert_eq!(inspected.status, Status::Succeeded);
                arguments["target"] = inspected.facts["items"][0]["target"].clone();
            } else if location == "focused" {
                arguments["focused"] = json!(true);
            } else {
                arguments["selector"] = json!({"name":"Draft","role":"textbox","exact":true});
                browser.push(Ok(BrowserOutcome::Targets {
                    tab_id: 7,
                    targets: vec![field.clone()],
                }));
            }
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
            let before = browser.calls().len();
            let invocation = if composed {
                json!({"steps":[{"id":"draft","tool":"browser_type_text","arguments":arguments}]})
            } else {
                arguments
            };

            let result = executor.execute(
                &workspace,
                if composed {
                    "browser_flow"
                } else {
                    "browser_type_text"
                },
                invocation,
                None,
                &CancellationToken::default(),
            );
            let label = format!("{location}, composed={composed}");
            assert_eq!(result.status, Status::Blocked, "{label}: {result:?}");
            assert_eq!(
                result.effect,
                if composed {
                    Effect::Partial
                } else {
                    Effect::Applied
                },
                "{label}"
            );
            if composed {
                assert_eq!(
                    result.facts["steps"][0]["result"]["effect"], "applied",
                    "{label}: composition retains the child's confirmed effect"
                );
            }
            assert!(!result.repeat_safe, "{label}");
            let facts = if composed {
                &result.facts["steps"][0]["result"]["facts"]
            } else {
                &result.facts
            };
            assert!(
                facts.get("held").is_none(),
                "{label}: failed observation must not masquerade as a landing hold"
            );
            assert_eq!(
                browser.calls().len(),
                before + if location == "selector" { 3 } else { 2 },
                "{label}: no observation of the read-denied landing"
            );
            let records = audit.0.lock().unwrap();
            let checks: Vec<_> = records
                .iter()
                .filter(|record| record.invocation == result.invocation)
                .flat_map(|record| &record.permissions.checks)
                .collect();
            assert!(
                checks.iter().any(|check| check.requirements
                    == crate::governance::CapabilitySet::ACTION
                    && check.allowed),
                "{label}: acknowledged action landing is admitted"
            );
            assert!(
                checks.iter().any(|check| !check.allowed),
                "{label}: separate observation denial remains visible"
            );
            assert!(!serde_json::to_string(&*records)
                .unwrap()
                .contains("PRIVATE_DRAFT"));
            drop(records);
            browser.push(Ok(BrowserOutcome::KeyPressed {
                tab: tab(7, "https://landing.example/"),
                key: "Tab".into(),
                subject: None,
                committed_urls: vec![],
            }));
            let follow_up = executor.execute(
                &workspace,
                "browser_press_key",
                json!({"key":"Tab"}),
                None,
                &CancellationToken::default(),
            );
            assert_eq!(
                follow_up.status,
                Status::Succeeded,
                "{label}: the admitted landing stays usable: {follow_up:?}"
            );
            fs::remove_file(policy).unwrap();
        }
    }
}

#[test]
fn an_unsent_form_needs_read_and_write_under_configured_policy() {
    for restriction in [json!(["read"]), json!(["write"]), json!(["read", "write"])] {
        let policy = TestPolicy::new();
        let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
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
        policy.set(restriction);
        let result = executor.execute(
            &workspace,
            "browser_fill_form",
            json!({"tab":handle,"fields":[{"target":target,"value":"PRIVATE_DRAFT"}]}),
            None,
            &CancellationToken::default(),
        );
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
            assert_eq!(result.facts["policy_rule"], "capability");
            assert_eq!(
                result.summary,
                "Blocked: this session may not take that kind of action."
            );
            let records = audit.0.lock().unwrap();
            assert_eq!(records.last().unwrap().summary, result.summary);
            assert!(!serde_json::to_string(&*records)
                .unwrap()
                .contains("PRIVATE_DRAFT"));
        }
    }
}

#[test]
fn retired_host_input_is_rejected_before_any_browser_work() {
    let (executor, browser, _, workspace, _) = fixture();
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://excluded.example/","restrict_hosts":["allowed.example"]}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Failed);
    assert!(result
        .next_steps
        .iter()
        .any(|step| step.contains("removed")));
    assert!(!result.summary.contains("excluded.example"));
    assert!(browser.calls().is_empty());
}
