//! Core executor dispatch, invocation lifecycle, and error mapping tests.

use super::*;

/// A primitive adapter error routes to an honest refusal that carries the browser's own
/// detail, instead of falling through to the browser-stopped rendering.
#[test]
fn primitive_adapter_errors_route_to_an_honest_refusal_with_detail() {
    let (refusal, facts) = routing_refusal(&BrowserError::Primitive(
        "target is not visible for focus".into(),
    ))
    .expect("primitive errors route to a refusal");
    let crate::language::outcome::Refusal::BrowserPrimitive { detail } = refusal else {
        panic!("primitive errors must route to BrowserPrimitive");
    };
    assert_eq!(detail, "target is not visible for focus");
    assert_eq!(facts["reason"], "browser_primitive_failed");
    assert_eq!(facts["detail"], "target is not visible for focus");
}

/// Only a true pre-dispatch disconnection may claim disconnection in the reason vocabulary,
/// and exactly the after-dispatch classes may claim an unknown effect. One evening of
/// debugging a phantom "disconnected" sentence bought this pin.
#[test]
fn error_reasons_stay_truthful_about_disconnection_and_unknown_effects() {
    use crate::browser::recovery::RecoveryFailure;
    let cases: Vec<(&str, BrowserError)> = vec![
        ("before_dispatch", BrowserError::DisconnectedBeforeDispatch),
        ("after_dispatch", BrowserError::DisconnectedAfterDispatch),
        ("cancelled_before", BrowserError::CancelledBeforeDispatch),
        ("cancelled_after", BrowserError::CancelledAfterDispatch),
        ("deadline_before", BrowserError::DeadlineBeforeDispatch),
        ("deadline_after", BrowserError::DeadlineAfterDispatch),
        ("primitive", BrowserError::Primitive("x".into())),
        ("interlock", BrowserError::LocalInterlock("x".into())),
        ("effect_unknown", BrowserError::EffectUnknown("x".into())),
        ("protocol", BrowserError::Protocol("x".into())),
        ("authentication", BrowserError::Authentication),
        (
            "unknown_browser",
            BrowserError::UnknownBrowser("browser_x".into()),
        ),
        ("pinned", BrowserError::BrowserPinned),
        (
            "ambiguous",
            BrowserError::AmbiguousBrowser(vec!["a".into()]),
        ),
        (
            "manual",
            BrowserError::RecoveryManual {
                browsers: Vec::new(),
            },
        ),
        (
            "recovery_failed",
            BrowserError::RecoveryFailed {
                reason: RecoveryFailure::LaunchFailed,
                details: vec![],
            },
        ),
        (
            "incompatible",
            BrowserError::Incompatible {
                offered: 1,
                required: 2,
            },
        ),
        (
            "capability_version",
            BrowserError::CapabilityVersion {
                capability: "pointer_input".into(),
                required: 2,
                advertised: 1,
            },
        ),
    ];
    for (name, error) in &cases {
        let claims = browser_reason(error) == "browser_disconnected";
        let truth = matches!(error, BrowserError::DisconnectedBeforeDispatch);
        assert_eq!(
            claims, truth,
            "{name} must not claim disconnection dishonestly"
        );
        assert_eq!(
            error.effect_unknown(),
            matches!(
                error,
                BrowserError::DisconnectedAfterDispatch
                    | BrowserError::CancelledAfterDispatch
                    | BrowserError::DeadlineAfterDispatch
                    | BrowserError::EffectUnknown(_)
            ),
            "{name} effect-unknown classification drifted"
        );
    }
}

/// A stale target handle arrives pre-recovered: fresh candidates matching what the dead
/// handle used to be called ride in the refusal facts, taken from the live page.
#[test]
fn a_stale_target_refusal_offers_fresh_candidates_from_the_live_page() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);

    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "btn-1".into(),
            role: "button".into(),
            name: "Save".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
    let inspected = executor.execute(
        &workspace,
        "browser_inspect",
        json!({"tab":tab_handle,"scope":"controls","max_items":10}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(
        inspected.status,
        Status::Succeeded,
        "inspect failed: {}",
        inspected.summary
    );

    let target_handle = inspected.facts["items"][0]["target"]
        .as_str()
        .expect("no target handle in inspect facts")
        .to_owned();

    // Commit a navigation: the document generation moves and the old handle goes stale.
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.com/next"),
        committed_urls: vec![],
    }));
    let _ = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/next","tab":tab_handle}),
        None,
        &CancellationToken::default(),
    );

    // The page still has its Save button under a fresh locator. Clicking by the stale
    // handle fails -- with the live candidate riding in the facts.
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "btn-2".into(),
            role: "button".into(),
            name: "Save".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
    let clicked = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target_handle}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(clicked.status, Status::Failed);
    assert_eq!(clicked.facts["reason"], "stale_target");
    let candidates = clicked.facts["recovery_candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["name"], "Save");
    assert_eq!(candidates[0]["role"], "button");
}

/// Waiting on a typed semantic selector polls the live page and succeeds the moment the
/// control exists -- no handle pre-resolution required.
#[test]
fn waiting_on_a_semantic_selector_polls_the_live_page() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

    // One miss, then the control appears.
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![],
    }));
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "b-1".into(),
            role: "button".into(),
            name: "Ready".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
    let waited = executor.execute(
            &workspace,
            "browser_wait",
            json!({"tab":tab_handle,"condition":"selector_present","selector":{"name":"Ready","role":"button"}}),
            None,
            &CancellationToken::default(),
        );
    assert_eq!(waited.status, Status::Succeeded);
    assert_eq!(waited.facts["condition"], "selector_present");
    assert_eq!(waited.facts["satisfied"], true);

    // A selector that never shows up inside its budget fails decisively.
    for _ in 0..5 {
        browser.push(Ok(BrowserOutcome::Targets {
            tab_id: 7,
            targets: vec![],
        }));
    }
    let missed = executor.execute(
            &workspace,
            "browser_wait",
            json!({"tab":tab_handle,"condition":"selector_present","selector":{"name":"Never","role":"button"},"timeout_ms":250}),
            None,
            &CancellationToken::default(),
        );
    assert_eq!(missed.status, Status::Failed);
    assert_eq!(missed.facts["satisfied"], false);
}

/// Deadlines speak for themselves instead of claiming disconnection, and carry their
/// phase so the caller can tell a spent budget from a silent adapter.
#[test]
fn deadlines_speak_for_themselves_instead_of_claiming_disconnection() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);

    browser.push(Err(BrowserError::DeadlineBeforeDispatch));
    let before = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(before.status, Status::Failed);
    assert!(before
        .summary
        .contains("ran out of time before reaching the browser"));
    assert!(!before.summary.contains("disconnected"));
    assert_eq!(before.facts["reason"], "deadline");
    assert_eq!(before.facts["phase"], "before_dispatch");

    browser.push(Err(BrowserError::DeadlineAfterDispatch));
    let after = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(after.status, Status::Unknown);
    assert!(after
        .summary
        .contains("ran out of time before the browser confirmed"));
    assert!(!after.summary.contains("disconnected"));
    assert_eq!(after.facts["reason"], "deadline");
    assert_eq!(after.facts["phase"], "after_dispatch");
}

/// Navigating by a durable tab handle that points at a closed tab recreates it through
/// the governed open path, rebinds the same handle, and reports the recovery plainly.
#[test]
fn navigating_to_a_dead_tab_recreates_it_under_the_same_handle() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(9, "https://example.com/again"),
        committed_urls: vec!["https://example.com/again".into()],
    }));

    let recovered = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/again","tab":"tab_deadbeefdeadbeefdeadbeefdeadbeef"}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(recovered.status, Status::Succeeded);
    assert_eq!(recovered.effect, Effect::Applied);
    assert!(!recovered.repeat_safe);
    assert_eq!(
        recovered.facts["tab"],
        json!("tab_deadbeefdeadbeefdeadbeefdeadbeef")
    );
    assert_eq!(recovered.facts["recovered"], "new_tab");
    assert!(recovered.summary.contains("That tab was gone"));
    assert!(matches!(browser.calls()[0], BrowserCommand::OpenTab { .. }));
}

/// Closing an already-gone tab achieves the desired state without touching the browser.
#[test]
fn closing_an_already_gone_tab_succeeds_without_touching_the_browser() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);

    let closed = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"close","tab":"tab_gone"}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(closed.status, Status::Succeeded);
    assert_eq!(closed.effect, Effect::None);
    assert_eq!(closed.summary, "That tab was already closed.");
    assert_eq!(closed.facts["already_gone"], true);
    assert_eq!(browser.calls().len(), 0);
}

/// An adapter's honest effect-unknown receipt renders as unknown with the browser's own
/// reason, never as an incompatible receipt.
#[test]
fn effect_unknown_receipts_render_as_unknown_with_the_browser_reason() {
    let (executor, browser, _workspaces, workspace, _) = fixture();
    browser.connect(vec![summary("browser_chrome", true)]);
    browser.push(Ok(BrowserOutcome::EffectUnknown {
        reason: "the page stopped responding after dispatch".into(),
    }));

    let terminal = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(terminal.status, Status::Unknown);
    assert_eq!(terminal.facts["reason"], "browser_effect_unknown");
    assert_eq!(
        terminal.facts["detail"],
        "the page stopped responding after dispatch"
    );
}

#[test]
fn failed_flow_audit_excludes_prior_read_and_error_payloads() {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(
            7,
            "https://example.com/PRIVATE_PATH?PRIVATE_QUERY#PRIVATE_FRAGMENT",
        ),
        committed_urls: vec![
            "https://example.com/PRIVATE_PATH?PRIVATE_QUERY#PRIVATE_FRAGMENT".into(),
        ],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        None,
        &CancellationToken::default(),
    );
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "PRIVATE_PAGE_SENTINEL".into(),
        truncated: false,
        title: "PRIVATE_TITLE_SENTINEL".into(),
        url: "https://example.com/PRIVATE_PATH?PRIVATE_QUERY#PRIVATE_FRAGMENT".into(),
    }));
    browser.push(Err(BrowserError::EffectUnknown(
        "PRIVATE_EXCEPTION_SENTINEL".into(),
    )));
    let result = executor.execute(&workspace, "browser_flow", json!({"steps":[
            {"id":"PRIVATE_STEP_ID", "tool":"browser_read", "arguments":{}},
            {"id":"execute", "tool":"browser_execute", "arguments":{"script":"throw 'PRIVATE_SCRIPT_SENTINEL'"}}
        ]}), None, &CancellationToken::default());
    assert_ne!(result.status, Status::Succeeded);
    let client = serde_json::to_string(&result).unwrap();
    assert!(client.contains("PRIVATE_PAGE_SENTINEL"));
    assert!(client.contains("PRIVATE_EXCEPTION_SENTINEL"));
    let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(
        !encoded.contains("PRIVATE_"),
        "audit copied a flow payload: {encoded}"
    );
}

#[test]
fn audit_excludes_primitive_exception_text_and_unknown_tool_names() {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.push(Err(BrowserError::Primitive(
        "PRIVATE_EXCEPTION_SENTINEL".into(),
    )));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/"}),
        None,
        &CancellationToken::default(),
    );
    assert!(serde_json::to_string(&result)
        .unwrap()
        .contains("PRIVATE_EXCEPTION_SENTINEL"));
    executor.execute(
        &workspace,
        "PRIVATE_TOOL_SENTINEL",
        json!({"PRIVATE_KEY":"PRIVATE_VALUE"}),
        None,
        &CancellationToken::default(),
    );
    let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(
        !encoded.contains("PRIVATE_"),
        "audit copied caller/browser text: {encoded}"
    );
}

#[test]
fn audit_excludes_script_results_and_invalid_input_context() {
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
    let tab_handle = opened.facts["tab"].as_str().unwrap();
    browser.push(Ok(BrowserOutcome::ScriptEvaluated {
        tab: tab(7, "https://example.com/"),
        value: json!({"PRIVATE_RESULT_KEY":"PRIVATE_RESULT_VALUE"}).to_string(),
        truncated: false,
        committed_urls: vec![],
    }));
    let result = executor.execute(
        &workspace,
        "browser_execute",
        json!({"script":"'PRIVATE_SCRIPT'"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    assert_eq!(
        result.facts["value"]["PRIVATE_RESULT_KEY"],
        "PRIVATE_RESULT_VALUE"
    );

    for (tool, arguments) in [
        (
            "browser_fill_form",
            json!({"fields":[{"target":"PRIVATE_TARGET_HANDLE","value":"PRIVATE_TYPED_VALUE"}]}),
        ),
        (
            "browser_upload",
            json!({"target":"PRIVATE_TARGET_HANDLE","paths":["C:/PRIVATE_FILE_PATH"]}),
        ),
        (
            "browser_click",
            json!({"selector":{"name":"PRIVATE_SELECTOR","PRIVATE_KEY":"PRIVATE_VALUE"}}),
        ),
        (
            "browser_wait",
            json!({"condition":"PRIVATE_CONDITION","value":"PRIVATE_WAIT_VALUE"}),
        ),
    ] {
        let result = executor.execute(
            &workspace,
            tool,
            arguments,
            None,
            &CancellationToken::default(),
        );
        assert_ne!(result.status, Status::Succeeded, "{result:?}");
    }
    let records = audit.0.lock().unwrap();
    let encoded = serde_json::to_string(&*records).unwrap();
    assert!(
        !encoded.contains("PRIVATE_"),
        "audit copied input/result context: {encoded}"
    );
    assert!(!encoded.contains(tab_handle));
    assert_eq!(records[1].summary, "Executed JavaScript on example.com.");
}

/// Audit retains a closed failure category; browser-authored details belong to the client.
#[test]
fn audit_records_carry_refusal_facts_for_failures_and_omit_them_for_successes() {
    let (executor, browser, _workspaces, workspace, audit) = fixture();
    browser.connect(vec![summary("browser_chrome", true)]);

    browser.push(Err(BrowserError::Primitive(
        "target is not visible for focus".into(),
    )));
    let _ = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );

    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let _ = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );

    let records = audit.0.lock().unwrap();
    assert_eq!(records.len(), 2);
    let failure = &records[0];
    assert_eq!(failure.status, "failed");
    let facts = failure
        .refusal_facts
        .as_ref()
        .expect("failure carries facts");
    assert_eq!(
        facts,
        &crate::language::audit::AuditRefusal::BrowserPrimitiveFailed
    );
    assert_eq!(
        failure.summary,
        "The browser could not complete this operation."
    );
    let success = &records[1];
    assert_eq!(success.status, "succeeded");
    assert!(success.refusal_facts.is_none());
}

#[test]
fn retired_tools_and_inputs_never_dispatch_or_trigger_policy_attention() {
    let (executor, browser, workspaces, workspace, audit) = fixture();
    let mut cases = vec![
        (
            "browser_sequence".to_string(),
            json!({"steps":[{"action":"click","target":"target_one"}]}),
        ),
        (
            "browser_flow".to_string(),
            json!({"dry_run":true,"steps":[{"tool":"browser_tabs"}]}),
        ),
        (
            "browser_flow".to_string(),
            json!({"dry_run":false,"steps":[{"tool":"browser_tabs"}]}),
        ),
    ];
    for tool in crate::language::catalog() {
        for field in ["restrict_hosts", "restrict_capabilities"] {
            let mut input = tool
                .input_schema
                .get("examples")
                .and_then(|examples| examples.get(0))
                .cloned()
                .unwrap_or_else(|| json!({}));
            input[field] = json!(["read"]);
            cases.push((tool.name.clone(), input));
        }
    }
    for field in ["restrict_hosts", "restrict_capabilities"] {
        let mut arguments = json!({"script":"MUST_NOT_RUN"});
        arguments[field] = json!(["read"]);
        cases.push(("browser_flow".to_string(), json!({"steps":[{"tool":"browser_tabs","arguments":{"action":"list"}},{"tool":"browser_execute","arguments":arguments}]})));
    }
    for (tool, input) in cases {
        let result = executor.execute(
            &workspace,
            &tool,
            input,
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Failed, "{tool}: {result:?}");
        assert_eq!(result.effect, Effect::None);
        assert!(
            result
                .next_steps
                .iter()
                .any(|step| step.contains("removed")),
            "{tool}: {result:?}"
        );
    }
    assert!(browser.calls().is_empty());
    assert!(workspaces.attention(&workspace).is_none());
    assert!(audit
        .0
        .lock()
        .unwrap()
        .iter()
        .all(|record| record.permissions.checks.is_empty()));
}

#[test]
fn flow_stops_after_a_refused_child_without_reverting_prior_effects() {
    let policy = TestPolicy::new();
    policy.set(json!(["read"]));
    let (executor, browser, _workspaces, workspace, _) = fixture_with_governance(policy.facade());
    for id in [7, 8] {
        browser.push(Ok(BrowserOutcome::TabOpened {
            reused: false,
            tab: tab(id, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
    }

    let result = executor.execute(
            &workspace,
            "browser_flow",
            json!({

                "steps":[
                    {"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}},
                    {"id":"denied","tool":"browser_execute","arguments":{"script":"42"}},
                    {"id":"after","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}
                ]
            }),
            None,
            &CancellationToken::default(),
        );

    let calls = browser.calls();
    assert_eq!(calls.len(), 1, "neither refused nor later work dispatches");
    assert!(matches!(calls[0], BrowserCommand::OpenTab { .. }));
    assert_eq!(result.facts["stopped"], true);
    assert_eq!(result.facts["completed"], 1);
    assert_eq!(result.facts["total"], 3);
    let steps = result.facts["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);
    assert_eq!(result.status, Status::Blocked);
    assert_eq!(steps[2]["status"], "not_run");
    assert_eq!(steps[0]["result"]["status"], "succeeded");
    assert_eq!(steps[0]["result"]["effect"], "applied");
    assert_eq!(steps[1]["result"]["status"], "blocked");
    assert_eq!(steps[1]["result"]["effect"], "none");
    assert_eq!(result.effect, Effect::Partial);
    assert!(!result.repeat_safe);
}

#[test]
fn flow_error_policy_controls_runtime_argument_failures() {
    // A tab handle resolves but is not a numeric read limit. A missing field fails
    // reference resolution instead. Both failures must honor the same error policy.
    for pointer in ["/facts/tab", "/facts/missing"] {
        for on_error in ["stop", "continue"] {
            let (executor, browser, _workspaces, workspace, _) = fixture();
            for id in [7, 8] {
                browser.push(Ok(BrowserOutcome::TabOpened {
                    reused: false,
                    tab: tab(id, "https://example.com/"),
                    committed_urls: vec!["https://example.com/".into()],
                }));
            }

            let result = executor.execute(
                    &workspace,
                    "browser_flow",
                    json!({
                        "on_error":on_error,
                        "steps":[
                            {"id":"open","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}},
                            {"id":"invalid","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"open","pointer":pointer}}}},
                            {"id":"after","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}
                        ]
                    }),
                    None,
                    &CancellationToken::default(),
                );

            let should_stop = on_error == "stop";
            let calls = browser.calls();
            assert_eq!(
                calls.len(),
                if should_stop { 1 } else { 2 },
                "{on_error} after {pointer}: {}",
                result.summary
            );
            assert!(calls
                .iter()
                .all(|call| matches!(call, BrowserCommand::OpenTab { .. })));
            assert_eq!(result.facts["stopped"], should_stop);
            let steps = result.facts["steps"].as_array().unwrap();
            assert_eq!(steps.len(), 3);
            assert_eq!(result.status, Status::Failed);
            assert_eq!(result.facts["completed"], if should_stop { 1 } else { 2 });
            assert_eq!(result.effect, Effect::Partial);
            assert_eq!(result.facts["progress"]["counts"]["not_started"], 1);
            assert_eq!(steps[0]["result"]["effect"], "applied");
            assert!(steps[1]["error"].is_string());
            assert!(!result.repeat_safe);
            if should_stop {
                assert_eq!(result.effect, Effect::Partial);
            } else {
                assert_eq!(steps[2]["id"], "after");
                assert_eq!(steps[2]["result"]["status"], "succeeded");
            }
        }
    }
}

#[test]
fn continue_reports_policy_denial_after_later_independent_success() {
    let policy = TestPolicy::new();
    policy.set(json!(["read"]));
    let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
    for id in [7, 8] {
        browser.push(Ok(BrowserOutcome::TabOpened {
            reused: false,
            tab: tab(id, "https://example.com/"),
            committed_urls: vec!["https://example.com/".into()],
        }));
    }
    let result = executor.execute(&workspace, "browser_flow", json!({
            "on_error":"continue",  "steps":[
                {"id":"first","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}},
                {"id":"denied","tool":"browser_execute","arguments":{"script":"PRIVATE_SCRIPT"}},
                {"id":"last","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}
            ]
        }), None, &CancellationToken::default());
    assert_eq!(browser.calls().len(), 2);
    assert_eq!(result.status, Status::Blocked);
    assert_eq!(result.effect, Effect::Partial);
    assert_eq!(result.summary, "Completed 2 of 3 steps. 1 blocked.");
    assert_eq!(result.facts["completed"], 2);
    assert_eq!(result.facts["stopped"], false);
    assert_eq!(result.facts["progress"]["counts"]["blocked"], 1);
    assert!(!result.repeat_safe);
    let records = audit.0.lock().unwrap();
    let record = records.last().unwrap();
    assert!(
        !record.allowed,
        "later admission cannot erase the child's policy denial"
    );
    assert_eq!(
        serde_json::to_value(record.composition).unwrap(),
        result.facts["progress"]
    );
    assert!(!serde_json::to_string(record).unwrap().contains("PRIVATE_"));
}

#[test]
fn flow_result_budget_preserves_step_metadata_and_reference_values() {
    let (executor, browser, _, workspace, audit) = fixture();
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
    browser.push(Ok(BrowserOutcome::ScriptEvaluated {
        tab: tab(7, "https://example.com/"),
        value: json!({"PRIVATE_PAYLOAD":"x".repeat(110_000),"limit":500}).to_string(),
        truncated: false,
        committed_urls: vec![],
    }));
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "visible".into(),
        truncated: false,
        title: "Example".into(),
        url: "https://example.com/".into(),
    }));
    let result = executor.execute(&workspace, "browser_flow", json!({"steps":[
            {"id":"large","tool":"browser_execute","arguments":{"script":"({limit:500})"}},
            {"id":"read","tool":"browser_read","arguments":{"max_chars":{"flow_ref":{"step":"large","pointer":"/facts/value/limit"}}}}
        ]}), None, &CancellationToken::default());
    assert_eq!(result.status, Status::Succeeded, "{}", result.summary);
    assert_eq!(result.facts["completed"], 2);
    for row in result.facts["steps"].as_array().unwrap() {
        assert_eq!(row["omitted"], true);
        assert_eq!(row["status"], "succeeded");
        assert!(row["effect"].is_string());
        assert!(row["repeat_safe"].is_boolean());
        assert!(row.get("result").is_none());
    }
    assert_eq!(result.facts["steps"][0]["effect"], "applied");
    assert_eq!(result.facts["steps"][1]["effect"], "none");
    assert!(!serde_json::to_string(&*audit.0.lock().unwrap())
        .unwrap()
        .contains("PRIVATE_"));
}

#[test]
fn short_flow_uses_its_default_tab_with_multiple_controlled_pages() {
    let (executor, browser, _, workspace, _) = fixture();
    let mut handles = Vec::new();
    for id in [7, 8] {
        browser.push(Ok(BrowserOutcome::TabOpened {
            reused: false,
            tab: tab(id, "https://example.com/"),
            committed_urls: vec![],
        }));
        let opened = executor.execute(
            &workspace,
            "browser_navigate",
            json!({"url":"https://example.com/","new_tab":true}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        handles.push(opened.facts["tab"].clone());
    }
    for id in [7, 8, 7] {
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: id,
            text: "read".into(),
            title: "Example".into(),
            url: "https://example.com/".into(),
            truncated: false,
        }));
    }
    let before = browser.calls().len();
    let result = executor.execute(
        &workspace,
        "browser_flow",
        json!({"tab":handles[0],"steps":[
            {"tool":"browser_read"},
            {"tool":"browser_read","arguments":{"tab":handles[1]}},
            {"tool":"browser_read"}
        ]}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    let actual: Vec<_> = browser.calls()[before..]
        .iter()
        .filter_map(|command| {
            if let BrowserCommand::ReadDocument { tab_id, .. } = command.primitive() {
                Some(*tab_id)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(actual, [7, 8, 7]);
    assert_eq!(result.facts["steps"][2]["id"], "step_3");
}

#[test]
fn read_only_compositions_are_repeat_safe_only_when_fully_successful() {
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
    for fail_last in [false, true] {
        for index in 0..2 {
            browser.push(if fail_last && index == 1 {
                Err(BrowserError::Primitive("PRIVATE_ERROR".into()))
            } else {
                Ok(BrowserOutcome::Text {
                    tab_id: 7,
                    text: "read".into(),
                    truncated: false,
                    title: "Example".into(),
                    url: "https://example.com/".into(),
                })
            });
        }
        let result = executor.execute(
            &workspace,
            "browser_flow",
            json!({"steps":[
                {"id":"one","tool":"browser_read","arguments":{}},
                {"id":"two","tool":"browser_read","arguments":{}}
            ]}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(
            result.status,
            if fail_last {
                Status::Failed
            } else {
                Status::Succeeded
            }
        );
        assert_eq!(result.effect, Effect::None);
        assert_eq!(result.repeat_safe, !fail_last);
        assert_eq!(result.facts["completed"], if fail_last { 1 } else { 2 });
    }
}

#[test]
fn continue_honors_human_attention_and_invocation_limits() {
    for (error, cause, status, effect) in [
        (
            BrowserError::LocalInterlock("PRIVATE_INTERLOCK".into()),
            "attention_required",
            Status::Blocked,
            Effect::None,
        ),
        (
            BrowserError::CancelledBeforeDispatch,
            "cancelled",
            Status::Cancelled,
            Effect::None,
        ),
        (
            BrowserError::CancelledAfterDispatch,
            "cancelled",
            Status::Unknown,
            Effect::Unknown,
        ),
        (
            BrowserError::DeadlineBeforeDispatch,
            "deadline",
            Status::Failed,
            Effect::None,
        ),
        (
            BrowserError::DeadlineAfterDispatch,
            "deadline",
            Status::Unknown,
            Effect::Unknown,
        ),
    ] {
        let (executor, browser, _, workspace, _) = fixture();
        browser.push(Err(error));
        let result = executor.execute(&workspace, "browser_flow", json!({"on_error":"continue","steps":[
                {"id":"first","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}},
                {"id":"later","tool":"browser_navigate","arguments":{"url":"https://example.com/","new_tab":true}}
            ]}), None, &CancellationToken::default());
        assert_eq!(browser.calls().len(), 1, "{cause}: {}", result.summary);
        assert_eq!(result.status, status);
        assert_eq!(result.effect, effect);
        assert_eq!(result.facts["progress"]["issue"]["cause"], cause);
        assert_eq!(result.facts["steps"][1]["status"], "not_run");
    }
}

#[test]
fn focused_typing_names_the_control_it_described_and_settles() {
    let (executor, browser, _workspaces, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );

    // The describe step observes the focused control; the typed outcome itself carries
    // no subject. The receipt must name the described control instead of panicking
    // after the effect has already landed.
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "0:locator_1".into(),
            role: "textbox".into(),
            name: "Ledger project".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
    browser.push(Ok(BrowserOutcome::Typed {
        tab: tab(7, "https://example.com/"),
        character_count: 5,
        subject: None,
        committed_urls: vec![],
    }));

    let typed = executor.execute(
        &workspace,
        "browser_type_text",
        json!({"text":"Ember","focused":true,"clear_first":true}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(typed.status, Status::Succeeded);
    assert_eq!(typed.effect, Effect::Applied);
    assert!(
        typed.summary.contains("Ledger project"),
        "summary must name the described control: {}",
        typed.summary
    );
    let calls = browser.calls();
    assert!(matches!(calls[1], BrowserCommand::DescribeFocused { .. }));
    assert!(matches!(calls[2], BrowserCommand::TypeFocused { .. }));
}

#[test]
fn work_follows_the_attended_browser_and_then_stays_where_it_started() {
    let (executor, browser, workspaces, workspace, _) = fixture();
    browser.connect(vec![
        summary("browser_chrome", false),
        summary("browser_edge", true),
    ]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));

    // Nothing named a browser, so the work goes where the person last was.
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded);
    assert_eq!(browser.routed(), vec!["browser_edge"]);
    assert_eq!(
        workspaces.browser_of(workspace.as_str()).as_deref(),
        Some("browser_edge")
    );

    // The person turns to Chrome. Established work does not follow them there.
    browser.connect(vec![
        summary("browser_chrome", true),
        summary("browser_edge", false),
    ]);
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.com/next"),
        committed_urls: vec!["https://example.com/next".into()],
    }));
    let followed = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/next"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(followed.status, Status::Succeeded);
    assert_eq!(browser.routed(), vec!["browser_edge", "browser_edge"]);
}

#[test]
fn an_ambiguous_bootstrap_names_the_choices_and_touches_no_browser() {
    let (executor, browser, workspaces, workspace, _) = fixture();
    browser.connect(vec![
        summary("browser_chrome", false),
        summary("browser_edge", false),
    ]);

    let refused = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.effect, Effect::None);
    assert_eq!(refused.facts["reason"], json!("browser_ambiguous"));
    assert_eq!(
        refused.facts["browsers"],
        json!(["browser_chrome", "browser_edge"])
    );
    assert!(refused.repeat_safe);
    // Nothing was dispatched and nothing was bound, so naming a browser next still works.
    assert!(browser.routed().is_empty());
    assert_eq!(workspaces.browser_of(workspace.as_str()), None);
}

#[test]
fn recovery_is_requested_from_one_seam_only() {
    let source = include_str!("../mod.rs");
    let marker = [".recovery", ".request("].concat();
    assert_eq!(source.matches(&marker).count(), 1);
    let seam = source
        .split("fn target_browser")
        .nth(1)
        .and_then(|tail| tail.split("fn observe").next())
        .expect("target-browser seam remains explicit");
    assert!(seam.contains(&marker));
}

#[test]
fn manual_recovery_maps_to_model_directed_summary_and_stable_facts() {
    let (refusal, facts) = routing_refusal(&BrowserError::RecoveryManual {
        browsers: vec!["Chromium".into()],
    })
    .expect("manual recovery is a model-facing refusal");

    assert_eq!(
        facts,
        json!({
            "reason":"browser_startup_manual",
            "browser":"Chromium",
            "browsers":["Chromium"]
        })
    );
    assert_eq!(
            refusal.summary(),
            "No browser is connected. Ask the user to open a Chromium browser window with the Ghostlight extension installed, then repeat the call."
        );
    assert!(refusal.next_steps().is_empty());

    let (plural, plural_facts) = routing_refusal(&BrowserError::RecoveryManual {
        browsers: vec!["Google Chrome".into(), "Microsoft Edge".into()],
    })
    .expect("plural manual recovery is model-facing");
    assert_eq!(
            plural.summary(),
            "No browser is connected. Ask the user to open a Google Chrome or Microsoft Edge browser window with the Ghostlight extension installed, then repeat the call."
        );
    assert_eq!(
        plural_facts,
        json!({
            "reason":"browser_startup_manual",
            "browsers":["Google Chrome", "Microsoft Edge"]
        })
    );
}

#[test]
fn a_named_browser_opens_there_and_a_named_stranger_is_refused() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![
        summary("browser_chrome", false),
        summary("browser_edge", true),
    ]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));

    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"browser":"browser_chrome"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded);
    assert_eq!(browser.routed(), vec!["browser_chrome"]);

    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary("browser_edge", true)]);
    let refused = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true,"browser":"browser_absent"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(refused.status, Status::Failed);
    assert_eq!(refused.facts["reason"], json!("browser_unknown"));
    assert!(browser.routed().is_empty());
}

#[test]
fn listing_tabs_waits_for_a_waking_relay_then_refuses_from_absence() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![]);
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![tab(7, "https://example.com/")],
    }));

    // A suspended MV3 worker reattaches shortly after the read begins.
    {
        let browser = browser.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            browser.connect(vec![summary(FAKE_BROWSER, true)]);
        });
    }
    let woke = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(woke.status, Status::Succeeded);
    assert_eq!(woke.facts["tabs"], json!([]));
    assert_eq!(
        woke.facts["browsers"],
        json!([{"browser":FAKE_BROWSER,"name":null,"attended":true}])
    );

    // With nothing reattaching inside the wake budget, the read refuses honestly instead
    // of answering from remembered state.
    let policy = temporary_policy("manual-browser-startup");
    fs::write(
        &policy,
        all_open_policy_with(r#"[{"key":"browser.startup","value":"manual","level":"mandatory"}]"#),
    )
    .unwrap();
    let (executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
    browser.connect(vec![]);
    let absent = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(absent.status, Status::Failed);
    assert_eq!(absent.facts["reason"], "browser_startup_manual");
    let _ = fs::remove_file(policy);
}

#[test]
fn listing_tabs_reports_live_titles_and_drops_bindings_that_are_gone() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    let handle = opened.facts["tab"].as_str().unwrap();

    // The live browser says the bound tab moved and was retitled: the list shows the
    // current truth, not the remembered landing facts.
    browser.push(Ok(BrowserOutcome::Tabs {
        tabs: vec![PhysicalTab {
            tab_id: 7,
            title: "Example Domain moved".into(),
            url: "https://example.com/moved".into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        }],
    }));
    let listed = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(
        listed.facts["tabs"],
        json!([{
            "tab":handle,
            "title":"Example Domain moved",
            "url":"https://example.com/moved",
            "active":true,
            "readiness":"complete"
        }])
    );

    // The live browser says the tab is gone; remembered state would keep showing it.
    browser.push(Ok(BrowserOutcome::Tabs { tabs: vec![] }));
    let gone = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(gone.status, Status::Succeeded);
    assert_eq!(gone.facts["tabs"], json!([]));
}

#[test]
fn direct_and_composed_reads_keep_equivalent_safe_receipts() {
    let (executor, browser, _, workspace, audit) = fixture();
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
    for composed in [false, true] {
        browser.push(Ok(BrowserOutcome::Text {
            tab_id: 7,
            text: "PRIVATE_PAGE_CONTENT".into(),
            truncated: false,
            title: "PRIVATE_TITLE".into(),
            url: "https://example.com/PRIVATE_PATH".into(),
        }));
        let (tool, arguments) = if composed {
            (
                "browser_flow",
                json!({"steps":[{"id":"PRIVATE_LABEL","tool":"browser_read","arguments":{}}]}),
            )
        } else {
            ("browser_read", json!({}))
        };
        let result = executor.execute(
            &workspace,
            tool,
            arguments,
            None,
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Succeeded);
        assert!(serde_json::to_string(&result)
            .unwrap()
            .contains("PRIVATE_PAGE_CONTENT"));
    }
    let records = audit.0.lock().unwrap();
    assert_eq!(records.len(), 4);
    assert!(records[1].step.is_none());
    assert_eq!(records[2].step.unwrap().position, 1);
    let safe = |record: &AuditRecord| {
        json!({
            "tool":record.tool,"requirements":record.requirements(),"status":record.status,
            "effect":record.effect,"summary":record.summary,"observed":record.observed,
            "permissions":record.permissions,"reason":record.reason
        })
    };
    assert_eq!(safe(&records[1]), safe(&records[2]));
    assert!(!serde_json::to_string(&*records)
        .unwrap()
        .contains("PRIVATE_"));
}

#[test]
fn child_receipts_share_authority_and_count_denials_once() {
    let path = temporary_policy("h4-denials");
    fs::write(
        &path,
        r#"{"schema":3,"name":"deny","version":"1","grants":[]}"#,
    )
    .unwrap();
    let governance = GovernanceFacade::new(Some(path.clone()), None);
    let (executor, browser, _, workspace, audit) = fixture_with_governance(governance.clone());
    let result = executor.execute(
        &workspace,
        "browser_flow",
        json!({"on_error":"continue","steps":[
            {"id":"PRIVATE_LABEL_ONE","tool":"browser_tabs","arguments":{"action":"list"}},
            {"id":"PRIVATE_LABEL_TWO","tool":"browser_tabs","arguments":{"action":"list"}}
        ]}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked);
    assert!(browser.calls().is_empty());
    assert_eq!(
        governance.runtime_state(),
        RuntimeControlState::Active,
        "parent cannot count either child twice"
    );
    {
        let records = audit.0.lock().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].step.unwrap().position, 1);
        assert_eq!(records[1].step.unwrap().position, 2);
        assert!(records[2].step.is_none());
        assert!(records
            .iter()
            .all(|record| record.authority == records[0].authority));
        assert!(records[..2]
            .iter()
            .all(|record| record.requirements() == crate::governance::CapabilitySet::READ));
        assert!(!serde_json::to_string(&*records)
            .unwrap()
            .contains("PRIVATE_"));
    }
    executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(
        governance.runtime_state(),
        RuntimeControlState::Active,
        "the next direct denial is exactly the third"
    );
    assert!(executor.workspaces.attention(&workspace).is_some());
    fs::remove_file(path).unwrap();
}

#[test]
fn repeated_policy_denials_pause_browser_work_until_the_user_resumes() {
    let policy = temporary_policy("denial-attention");
    fs::write(
        &policy,
        r#"{"schema":3,"name":"deny reads","version":"1","grants":[],"config":[]}"#,
    )
    .unwrap();
    let governance = GovernanceFacade::new(Some(policy.clone()), None);
    let (executor, browser, _, workspace, _) = fixture_with_governance(governance.clone());

    for index in 0..3 {
        let denied = executor.execute(
            &workspace,
            "browser_tabs",
            json!({"action":"list"}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(
            denied.status,
            if index == 2 {
                Status::AttentionRequired
            } else {
                Status::Blocked
            }
        );
    }
    assert_eq!(governance.runtime_state(), RuntimeControlState::Active);
    assert_eq!(
        browser.control_states().last(),
        Some(&RuntimeControlState::Active)
    );

    let paused = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(paused.status, Status::AttentionRequired);

    assert_eq!(
        governance.apply_runtime_intent(RuntimeControlIntent::Resume),
        RuntimeControlState::Active
    );
    assert!(
        executor.workspaces.attention(&workspace).is_some(),
        "global resume cannot clear session review"
    );
    let incident = executor.workspaces.attention(&workspace).unwrap();
    assert!(executor
        .workspaces
        .resume_attention(workspace.as_str(), &incident.id)
        .unwrap());
    let denied_again = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(denied_again.status, Status::Blocked);
    assert_eq!(governance.runtime_state(), RuntimeControlState::Active);
    let _ = fs::remove_file(policy);
}

#[test]
fn observation_budget_preserves_time_for_the_physical_receipt() {
    assert_eq!(
        observation_budget_ms(3_000, Duration::from_millis(3_000)),
        2_250
    );
    assert_eq!(observation_budget_ms(100, Duration::from_millis(100)), 0);
    assert_eq!(
        observation_budget_ms(500, Duration::from_millis(5_000)),
        500
    );
}

#[test]
fn record_actions_cross_only_the_extension_owned_request_receipt_seam() {
    let (executor, browser, _, workspace, _) = fixture();
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
    let handle = opened.facts["tab"].as_str().unwrap().to_owned();

    browser.push(Ok(BrowserOutcome::RecordingStarted {
        summary: recording_summary(RecordingState::Recording, "https://example.com/"),
        existing: false,
    }));
    assert_eq!(
        executor
            .execute(
                &workspace,
                "browser_record",
                json!({"action":"start","tab":handle}),
                None,
                &CancellationToken::default(),
            )
            .status,
        Status::Succeeded
    );

    browser.push(Ok(BrowserOutcome::RecordingStatus {
        summary: recording_summary(RecordingState::Recording, "https://example.com/"),
    }));
    executor.execute(
        &workspace,
        "browser_record",
        json!({"action":"status"}),
        None,
        &CancellationToken::default(),
    );

    browser.push(Ok(BrowserOutcome::RecordingStopped {
        summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
        changed: true,
    }));
    executor.execute(
        &workspace,
        "browser_record",
        json!({"action":"stop"}),
        None,
        &CancellationToken::default(),
    );

    browser.push(Ok(BrowserOutcome::RecordingDiscarded {
        recording_id: "recording_one".into(),
        released_bytes: 1,
    }));
    executor.execute(
        &workspace,
        "browser_record",
        json!({"action":"discard"}),
        None,
        &CancellationToken::default(),
    );

    let calls = browser.calls();
    assert!(matches!(
        calls[1],
        BrowserCommand::StartRecording { tab_id: 7 }
    ));
    assert!(matches!(calls[2], BrowserCommand::StatusRecording { .. }));
    assert!(matches!(calls[3], BrowserCommand::StopRecording { .. }));
    assert!(matches!(calls[4], BrowserCommand::DiscardRecording { .. }));
    assert!(!calls
        .iter()
        .any(|call| matches!(call, BrowserCommand::ExportRecording { .. })));
}

#[test]
fn a_paused_runtime_refuses_recording_status_stop_and_discard() {
    // Every other operation in this executor -- even ones needing no capability at all, like
    // activating a tab -- crosses the runtime gate before it can reach the browser. Recording
    // status, stop, and discard used to be the one family that dispatched straight through,
    // so pausing Ghostlight did not actually stop a recording from being stopped or discarded
    // out from under the person who paused it.
    let governance = GovernanceFacade::new(None, None);
    assert_eq!(
        governance.apply_runtime_intent(RuntimeControlIntent::Hold),
        RuntimeControlState::Held
    );
    let (executor, browser, _, workspace, _) = fixture_with_governance(governance);

    for action in ["status", "stop", "discard"] {
        let result = executor.execute(
            &workspace,
            "browser_record",
            json!({"action":action}),
            None,
            &CancellationToken::default(),
        );
        assert_ne!(
            result.status,
            Status::Succeeded,
            "recording {action} must not succeed while paused: {result:?}"
        );
    }
    assert!(
        browser.calls().is_empty(),
        "a paused runtime must never reach the browser at all: {:?}",
        browser.calls()
    );
}

#[test]
fn a_save_asks_the_browser_for_one_finished_replay() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::RecordingStopped {
        summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
        changed: true,
    }));
    browser.push(Ok(BrowserOutcome::RecordingExported {
        summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
        encoded: EncodedRecording {
            frame_count: 17,
            captured_frame_count: 65,
            duration_ms: 30_400,
            width: 1_280,
            height: 800,
            byte_count: 3_804_453,
        },
        delivery: RecordingDelivery::Returned {
            mime_type: "image/gif".into(),
            data: "R0lGODlh".into(),
        },
    }));

    let result = executor.execute(
        &workspace,
        "browser_record",
        json!({"action":"save","recording":"recording_one"}),
        None,
        &CancellationToken::default(),
    );

    // Stop, then export. Nothing in between: the frames never come here to be encoded.
    let calls = browser.calls();
    assert!(matches!(calls[0], BrowserCommand::StopRecording { .. }));
    assert!(matches!(
        calls[1],
        BrowserCommand::ExportRecording {
            destination: RecordingDestination::Client,
            max_output_bytes: RECORDING_TRANSFER_MAX_BYTES,
            ..
        }
    ));
    assert_eq!(calls.len(), 2);
    // The sentence is what a person would say about a replay. The mechanism it was made from
    // is real, and belongs in the facts.
    assert_eq!(
        result.summary,
        "Saved a replay of 30 seconds of page changes."
    );
    assert_eq!(result.facts["frame_count"], json!(17));
    assert_eq!(result.facts["captured_frame_count"], json!(65));
    assert_eq!(result.facts["gif_bytes"], json!(3_804_453));
    assert_eq!(result.facts["delivery"], json!("returned_to_client"));
}

#[test]
fn a_download_save_never_returns_the_replay_bytes() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::RecordingStopped {
        summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
        changed: true,
    }));
    browser.push(Ok(BrowserOutcome::RecordingExported {
        summary: recording_summary(RecordingState::Frozen, "https://example.com/"),
        encoded: EncodedRecording {
            frame_count: 40,
            captured_frame_count: 40,
            duration_ms: 1_500,
            width: 1_280,
            height: 800,
            byte_count: 9_000_000,
        },
        delivery: RecordingDelivery::Downloaded,
    }));

    let result = executor.execute(
        &workspace,
        "browser_record",
        json!({"action":"save","recording":"recording_one","download":true}),
        None,
        &CancellationToken::default(),
    );

    assert!(matches!(
        browser.calls()[1],
        BrowserCommand::ExportRecording {
            destination: RecordingDestination::Download { .. },
            // A replay that stays in the browser is not bounded by what can cross out of it.
            max_output_bytes: RECORDING_LOCAL_MAX_BYTES,
            ..
        }
    ));
    assert_eq!(
        result.summary,
        "Downloaded a replay of 2 seconds of page changes."
    );
    assert!(
        result.content.is_empty(),
        "a browser-local save must return no bytes: {:?}",
        result.content
    );
}

#[test]
fn client_save_authorizes_source_before_recording_bytes_cross() {
    let policy = TestPolicy::new();
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
    document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
    fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
    let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
    browser.push(Ok(BrowserOutcome::RecordingStopped {
        summary: recording_summary(RecordingState::Frozen, "http://127.0.0.1/private"),
        changed: true,
    }));

    let result = executor.execute(
        &workspace,
        "browser_record",
        json!({"action":"save","recording":"recording_one"}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(result.status, Status::Blocked);
    assert!(matches!(
        browser.calls().as_slice(),
        [BrowserCommand::StopRecording { .. }]
    ));
}

#[test]
fn unsatisfied_wait_is_decisive_before_the_invocation_deadline() {
    let (executor, browser, _, workspace, _) = fixture();
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

    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: false,
        elapsed_ms: 750,
        readiness: BrowserReadiness::Complete,
    }));
    let waited = executor.execute(
        &workspace,
        "browser_wait",
        json!({
            "tab":tab_handle,
            "condition":"text_present",
            "value":"never present",
            "timeout_ms":1_000
        }),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(waited.status, Status::Failed);
    assert_eq!(waited.effect, Effect::None);
    assert!(waited.repeat_safe);
    assert_eq!(waited.facts["satisfied"], false);
    let calls = browser.calls();
    let timeout_ms = calls
        .iter()
        .find_map(|call| match call {
            BrowserCommand::Observe { timeout_ms, .. } => Some(*timeout_ms),
            _ => None,
        })
        .unwrap();
    assert!(timeout_ms <= 750);
    assert!(timeout_ms > 0);
}

#[test]
fn open_read_close_is_one_truthful_result_per_call() {
    let (executor, browser, _, workspace, audit) = fixture();
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
    assert_eq!(opened.status, Status::Succeeded);
    let handle = opened.facts["tab"].as_str().unwrap();
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "Example Domain".into(),
        truncated: false,
        title: "Example".into(),
        url: "https://example.com/".into(),
    }));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":handle}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Succeeded);
    browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
    let closed = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"close","tab":handle}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(closed.status, Status::Succeeded);
    assert_eq!(audit.0.lock().unwrap().len(), 3);
    let calls = browser.calls();
    assert_eq!(calls.len(), 3);
    assert!(matches!(
        &calls[0],
        BrowserCommand::OpenTab { url, group_title, .. }
            if url == "https://example.com" && group_title == "Ghostlight - test"
    ));
    assert!(matches!(
        &calls[1],
        BrowserCommand::ReadDocument { mode, max_chars, .. }
            if mode == "visible" && *max_chars == 8_000
    ));
}

#[test]
fn readiness_names_stay_the_vocabulary_a_result_uses() {
    for value in [
        Readiness::NotApplicable,
        Readiness::Loading,
        Readiness::Interactive,
        Readiness::Complete,
        Readiness::Unknown,
    ] {
        let encoded = serde_json::to_value(value).unwrap();
        assert_eq!(
            encoded.as_str().unwrap(),
            readiness_name(value),
            "the observation and the result would disagree about readiness"
        );
    }
}

#[test]
fn browser_seam_observes_landing_facts_but_not_outcome_measurements() {
    let tabs = observed_from(&BrowserOutcome::Tabs {
        tabs: vec![tab(7, "https://example.com/")],
    });
    assert_eq!(tabs, Observed::default());

    let text = observed_from(&BrowserOutcome::Text {
        tab_id: 7,
        text: "three private words".into(),
        truncated: false,
        title: "Example".into(),
        url: "https://example.com/private?id=3".into(),
    });
    assert_eq!(text.host.as_deref(), Some("example.com"));
    assert_eq!(text.count, None);

    let wait = observed_from(&BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 1_830,
        readiness: BrowserReadiness::Complete,
    });
    assert_eq!(wait.readiness.as_deref(), Some("complete"));
    assert_eq!(wait.count, None);
}

#[test]
fn outcome_language_and_the_seam_observe_without_carrying_page_detail() {
    let (executor, browser, _, workspace, audit) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://Example.com/patients/48219?ssn=1#note"),
        committed_urls: vec!["https://example.com/patients/48219?ssn=1#note".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com/patients/48219?ssn=1#note"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded);
    let handle = opened.facts["tab"].as_str().unwrap().to_owned();

    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 7,
        text: "Patient 48219 has an appointment".into(),
        truncated: false,
        title: "Example".into(),
        url: "https://example.com/patients/48219?ssn=1#note".into(),
    }));
    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":handle}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(read.status, Status::Succeeded);
    assert_eq!(read.summary, "Read 5 words from example.com.");

    let records = audit.0.lock().unwrap();
    let landing = &records[0].observed;
    assert_eq!(landing.host.as_deref(), Some("example.com"));
    assert_eq!(landing.readiness.as_deref(), Some("complete"));
    let text = &records[1].observed;
    assert_eq!(text.host.as_deref(), Some("example.com"));
    assert_eq!(text.count, Some(5));

    // The model-facing facts legitimately carry the URL and the text. The audit carries the
    // same action, and none of it.
    assert!(read.facts["url"].as_str().unwrap().contains("48219"));
    let encoded = serde_json::to_string(&*records).unwrap();
    for detail in ["patients", "48219", "ssn", "note", "appointment"] {
        assert!(
            !encoded.contains(detail),
            "the audit leaked {detail} from a page"
        );
    }
}

#[test]
fn an_observation_never_outlives_the_invocation_it_describes() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    // A failure before any browser crossing must not leave a key behind either.
    executor.execute(
        &workspace,
        "browser_navigate",
        json!({"nonsense":true}),
        None,
        &CancellationToken::default(),
    );
    assert!(
        executor.observations().is_empty(),
        "the registry grows with every invocation instead of with work in flight"
    );
}

#[test]
fn a_capture_reports_its_size_and_a_wait_reports_how_long_it_waited() {
    let (executor, browser, _, workspace, audit) = fixture();
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
    let handle = opened.facts["tab"].as_str().unwrap().to_owned();
    browser.push(Ok(BrowserOutcome::Screenshot {
        tab_id: 7,
        mime_type: "image/jpeg".into(),
        data: "image".into(),
        width: 1280,
        height: 720,
        viewport: ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 0.0,
            page_y: 0.0,
            css_width: 1280.0,
            css_height: 720.0,
            visual_page_x: 0.0,
            visual_page_y: 0.0,
            visual_css_width: 1280.0,
            visual_css_height: 720.0,
            device_scale: 1.0,
            zoom: 1.0,
            output_scale: 1.0,
        },
    }));
    let captured = executor.execute(
        &workspace,
        "browser_screenshot",
        json!({"tab":handle}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(captured.summary, "Captured the viewport at 1280x720.");

    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 1_830,
        readiness: BrowserReadiness::Complete,
    }));
    let waited = executor.execute(
        &workspace,
        "browser_wait",
        json!({"tab":handle,"condition":"load_ready","timeout_ms":5_000}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(waited.summary, "example.com finished loading in 2 seconds.");

    let records = audit.0.lock().unwrap();
    let capture = &records[1].observed;
    assert_eq!((capture.width, capture.height), (Some(1280), Some(720)));
    // A capture is its own invocation. It reports the size it took and leaves the landing to
    // the invocation that navigated, because an observation never outlives its invocation.
    assert_eq!(capture.host.as_deref(), None);
    let wait = &records[2].observed;
    assert_eq!(wait.count, Some(2));
    assert_eq!(wait.readiness.as_deref(), Some("complete"));
}

#[test]
fn tab_close_policy_blocks_before_browser_dispatch() {
    let policy = temporary_policy("tab-close");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
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
    let handle = opened.facts["tab"].as_str().unwrap();
    let closed = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"close","tab":handle}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(closed.status, Status::Blocked);
    assert_eq!(closed.effect, Effect::None);
    assert_eq!(closed.facts["reason"], "tab_close_denied");
    assert_eq!(browser.calls().len(), 1);
    let _ = fs::remove_file(policy);
}

#[test]
fn refused_navigation_audits_only_the_attempted_host() {
    let policy = TestPolicy::new();
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
    document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
    fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
    let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"http://127.0.0.1/private/record-42?token=secret#detail"}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(result.status, Status::Blocked);
    assert!(result.summary.contains("127.0.0.1"));
    assert!(browser.calls().is_empty());

    let records = audit.0.lock().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].observed.host.as_deref(), Some("127.0.0.1"));
    let encoded = serde_json::to_string(&records[0]).unwrap();
    assert!(!encoded.contains("record-42"));
    assert!(!encoded.contains("token"));
    assert!(!encoded.contains("secret"));
}

#[test]
fn a_tab_whose_landing_is_not_yet_known_is_refused_rather_than_checked_by_capability_alone() {
    // Acquiring a tab does not grant destination authority. Until a known landing has
    // been checked, an empty URL must fail closed rather than union host-specific grants.
    let policy = temporary_policy("unknown-landing");
    fs::write(
            &policy,
            r#"{"schema":3,"name":"work test","version":"1","grants":[{"id":"approved","hosts":{"allow":["approved.example"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
    let (executor, browser, workspaces, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));

    // An ordinary, fully governed opener tab, admitted under the narrow policy.
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://approved.example/"),
        committed_urls: vec!["https://approved.example/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://approved.example","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded, "{opened:?}");
    // Even an explicitly acquired tab without a known destination must fail closed.
    let handle = workspaces
        .acquire(&workspace)
        .unwrap()
        .add_tab(&tab(8, ""))
        .unwrap()
        .handle;

    let read = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab": handle.as_str()}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(
        read.status,
        Status::Blocked,
        "a tab with no known landing must be refused, not checked by capability alone: {read:?}"
    );
    assert!(
        browser
            .calls()
            .iter()
            .all(|call| !matches!(call, BrowserCommand::Observe { .. })),
        "the browser must never be asked to read a tab whose destination was never checked"
    );
    let _ = fs::remove_file(policy);
}

#[test]
fn physical_action_receipt_names_the_target_without_trusting_its_role() {
    let (executor, browser, _, workspace, audit) = fixture();
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

    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "hostile-role".into(),
            role: "Save my document".into(),
            name: "private patient action".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
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

    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: Some(PhysicalActionSubject {
            role: "Save my document".into(),
            name: "Save patient record".into(),
        }),
    }));
    let clicked = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(
        clicked.summary,
        "Clicked the \"Save patient record\" control on example.com."
    );
    let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(!encoded.contains("Save my document"));
    assert!(encoded.contains("Save patient record"));
    assert!(!encoded.contains("private patient action"));
}

#[test]
fn governance_can_remove_target_names_without_losing_the_safe_role() {
    let policy = temporary_policy("hide-target-names");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
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
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "save".into(),
            role: "button".into(),
            name: "Save patient record".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
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
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: Some(PhysicalActionSubject {
            role: "button".into(),
            name: "Save patient record".into(),
        }),
    }));
    let clicked = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(clicked.summary, "Clicked a button on example.com.");
    let encoded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(!encoded.contains("Save patient record"));
    let _ = fs::remove_file(policy);
}

#[test]
fn local_preserve_tabs_refusal_is_blocked_without_an_effect() {
    let (executor, browser, _, workspace, _) = fixture();
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
    let handle = opened.facts["tab"].as_str().unwrap();
    browser.push(Err(crate::browser::BrowserError::LocalInterlock(
        "preserved".into(),
    )));
    let closed = executor.execute(
        &workspace,
        "browser_tabs",
        json!({"action":"close","tab":handle}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(closed.status, Status::Blocked);
    assert_eq!(closed.effect, Effect::None);
    assert!(closed.repeat_safe);
    assert_eq!(closed.facts["reason"], "browser_local_interlock");
    assert_eq!(closed.next_steps.len(), 1);
}

#[test]
fn uncertain_effect_guides_recovery_without_replay() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.push(Err(crate::browser::BrowserError::DisconnectedAfterDispatch));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Unknown);
    assert_eq!(result.effect, Effect::Unknown);
    assert!(!result.repeat_safe);
    assert_eq!(
            result.next_steps,
            vec![
                "If a JavaScript dialog may be open on the page, handle it with browser_dialog; handling checks the page directly.".to_string(),
                "Then observe the page with browser_read or browser_inspect to learn what happened.".to_string(),
            ]
        );
}

#[test]
fn direct_and_flow_actions_use_the_same_physical_executor_path() {
    let (executor, browser, _, workspace, _) = fixture();
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
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "button-1".into(),
            role: "button".into(),
            name: "Go".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
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

    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: None,
    }));
    let direct = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(direct.status, Status::Succeeded);

    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: None,
    }));
    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 5,
        readiness: BrowserReadiness::Complete,
    }));
    let sequence = executor.execute(&workspace, "browser_flow", json!({"tab":tab_handle,"steps":[{"tool":"browser_click","arguments":{"target":target}},{"tool":"browser_wait","arguments":{"condition":"load_ready","visual_settle":false}}]}), None, &CancellationToken::default());
    assert_eq!(sequence.status, Status::Succeeded);
    let calls = browser.calls();
    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, BrowserCommand::Activate { .. }))
            .count(),
        2
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, BrowserCommand::Observe { .. }))
            .count(),
        1
    );
    for unknown in [false, true] {
        for named in [false, true] {
            let tool = "browser_flow";
            browser.push(Ok(BrowserOutcome::Activated {
                tab: tab(7, "https://example.com/"),
                committed_urls: vec![],
                subject: None,
            }));
            browser.push(Err(if unknown {
                BrowserError::DisconnectedAfterDispatch
            } else {
                BrowserError::Primitive("PRIVATE_WAIT_FAILURE".into())
            }));
            let before = browser.calls().len();
            let mut arguments = json!({"steps":[
                {"id":"click","tool":"browser_click","arguments":{"tab":tab_handle,"target":target}},
                {"id":"wait","tool":"browser_wait","arguments":{"tab":tab_handle,"condition":"load_ready"}},
                {"id":"later","tool":"browser_click","arguments":{"tab":tab_handle,"target":target}}
            ]});
            if !named {
                for step in arguments["steps"].as_array_mut().unwrap() {
                    step.as_object_mut().unwrap().remove("id");
                }
            }
            let result = executor.execute(
                &workspace,
                tool,
                arguments,
                None,
                &CancellationToken::default(),
            );
            assert_eq!(browser.calls().len() - before, 2, "{tool}");
            assert_eq!(
                result.status,
                if unknown {
                    Status::Unknown
                } else {
                    Status::Failed
                }
            );
            assert_eq!(
                result.effect,
                if unknown {
                    Effect::Unknown
                } else {
                    Effect::Partial
                }
            );
            assert_eq!(result.facts["progress"]["counts"]["succeeded"], 1);
            assert_eq!(result.facts["progress"]["effects"]["applied"], 1);
            assert_eq!(result.facts["steps"][2]["status"], "not_run");
            assert!(!result.repeat_safe);
            assert!(result.summary.contains(if unknown {
                "Connection lost during step 2."
            } else {
                "Step 2 failed."
            }));
        }
    }
}

#[test]
fn credential_target_requests_handoff_before_any_value_dispatch() {
    let (executor, browser, _, workspace, _) = fixture();
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
    let credential = ObservedTarget {
        locator: "password-1".into(),
        role: "textbox".into(),
        name: "Password".into(),
        state: vec![],
        credential_class: true,
    };
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![credential.clone()],
    }));
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
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        tab_id: 7,
        targets: vec![credential],
    }));
    let result = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"tab":tab_handle,"fields":[{"target":target,"value":"not-sent"}]}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::AttentionRequired);
    assert!(executor.workspaces.attention(&workspace).is_some());
    assert_eq!(result.facts["values_sent"], false);
    assert_eq!(
        browser.control_states().last(),
        Some(&ghostlight_bridge::browser::RuntimeControlState::Active)
    );
    assert!(!browser
        .calls()
        .iter()
        .any(|call| matches!(call, BrowserCommand::Fill { .. })));
}

#[test]
fn denied_redirect_is_compensated_without_replay_risk() {
    let policy = TestPolicy::new();
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
    document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
    fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
    let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "http://127.0.0.1/private"),
        committed_urls: vec![
            "https://example.com/".into(),
            "http://127.0.0.1/private".into(),
        ],
    }));
    browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 7 }));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked);
    assert_eq!(result.effect, Effect::None);
    assert_eq!(result.facts["compensated"], true);
    assert!(result.repeat_safe);
}

#[test]
fn denied_redirect_remains_visibly_open_when_close_policy_refuses_compensation() {
    let policy = temporary_policy("retained-denied-landing");
    fs::write(
        &policy,
        all_open_policy_with(
            r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
        ),
    )
    .unwrap();
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&policy).unwrap()).unwrap();
    document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
    fs::write(&policy, serde_json::to_vec(&document).unwrap()).unwrap();
    let (executor, browser, _, workspace, _) =
        fixture_with_governance(GovernanceFacade::new(Some(policy.clone()), None));
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "http://127.0.0.1/private"),
        committed_urls: vec![
            "https://example.com/".into(),
            "http://127.0.0.1/private".into(),
        ],
    }));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked);
    assert_eq!(result.effect, Effect::Applied);
    assert!(!result.repeat_safe);
    assert_eq!(result.facts["compensated"], false);
    assert_eq!(result.facts["retained"], true);
    assert_eq!(browser.calls().len(), 1);
    let _ = fs::remove_file(policy);
}

#[test]
fn denied_redirect_remains_visibly_open_when_local_preservation_refuses_compensation() {
    let policy = TestPolicy::new();
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&policy.0).unwrap()).unwrap();
    document["grants"][0]["hosts"]["allow"] = json!(["example.com"]);
    fs::write(&policy.0, serde_json::to_vec(&document).unwrap()).unwrap();
    let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "http://127.0.0.1/private"),
        committed_urls: vec![
            "https://example.com/".into(),
            "http://127.0.0.1/private".into(),
        ],
    }));
    browser.push(Err(crate::browser::BrowserError::LocalInterlock(
        "preserved".into(),
    )));
    let result = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked);
    assert_eq!(result.effect, Effect::Applied);
    assert!(!result.repeat_safe);
    assert_eq!(result.facts["compensated"], false);
    assert_eq!(result.facts["retained"], true);
    assert_eq!(browser.calls().len(), 2);
}

#[test]
fn stale_target_fails_before_browser_dispatch() {
    let (executor, browser, _, workspace, _) = fixture();
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
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "old".into(),
            role: "button".into(),
            name: "Old".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
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
    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.org/"),
        committed_urls: vec!["https://example.org/".into()],
    }));
    let navigated = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"tab":tab_handle,"url":"https://example.org"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(navigated.status, Status::Succeeded);
    let before = browser.calls().len();
    // The live page still has a button answering to the old name.
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "new".into(),
            role: "button".into(),
            name: "Old".into(),
            state: vec![],
            credential_class: false,
        }],
    }));
    let stale = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"target":target}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(stale.status, Status::Failed);
    assert_eq!(stale.facts["reason"], "stale_target");
    // Exactly one extra crossing happened: the candidate probe. The click itself never
    // dispatched.
    assert_eq!(browser.calls().len(), before + 1);
    assert!(matches!(
        browser.calls().last(),
        Some(BrowserCommand::QuerySemantic { name, .. })
            if name == "Old"
    ));
    let candidates = stale.facts["recovery_candidates"].as_array().unwrap();
    assert_eq!(candidates[0]["name"], "Old");
}

#[test]
fn screenshot_coordinates_and_regions_resolve_once_chain_and_expire() {
    let (executor, browser, _, workspace, _) = fixture();
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
    let viewport = ViewportGeometry {
        scope: CaptureScope::Viewport,
        page_x: 10.0,
        page_y: 20.0,
        css_width: 800.0,
        css_height: 600.0,
        visual_page_x: 10.0,
        visual_page_y: 20.0,
        visual_css_width: 800.0,
        visual_css_height: 600.0,
        device_scale: 1.0,
        zoom: 1.0,
        output_scale: 0.5,
    };
    browser.push(Ok(BrowserOutcome::Screenshot {
        tab_id: 7,
        mime_type: "image/jpeg".into(),
        data: "image".into(),
        width: 400,
        height: 300,
        viewport,
    }));
    let screenshot = executor.execute(
        &workspace,
        "browser_screenshot",
        json!({"tab":tab_handle}),
        None,
        &CancellationToken::default(),
    );
    assert!(screenshot.facts.get("data").is_none());
    assert!(matches!(
        screenshot.content.as_slice(),
        [ServiceContent::Image { mime_type, data }]
            if mime_type == "image/jpeg" && data == "image"
    ));
    let view = screenshot.facts["view"].as_str().unwrap().to_owned();
    browser.push(Ok(BrowserOutcome::Activated {
        tab: tab(7, "https://example.com/"),
        committed_urls: vec![],
        subject: None,
    }));
    let clicked = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"view":view,"x":100,"y":50}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(clicked.status, Status::Succeeded);
    assert!(browser.calls().iter().any(|call| matches!(
        call,
        BrowserCommand::ActivatePoint { point, expected_viewport, .. }
            if point.x == 210.0 && point.y == 120.0 && *expected_viewport == viewport
    )));

    let first_region_viewport = ViewportGeometry {
        scope: CaptureScope::Region,
        page_x: 210.0,
        page_y: 120.0,
        css_width: 400.0,
        css_height: 200.0,
        output_scale: 4.0,
        ..viewport
    };
    browser.push(Ok(BrowserOutcome::Screenshot {
        tab_id: 7,
        mime_type: "image/jpeg".into(),
        data: "magnified".into(),
        width: 1600,
        height: 800,
        viewport: first_region_viewport,
    }));
    let first_region = executor.execute(
        &workspace,
        "browser_screenshot",
        json!({"tab":tab_handle,"view":view,"x":100,"y":50,"width":200,"height":100}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(
        first_region.summary,
        "Captured the magnified region at 1600x800."
    );
    let first_region_view = first_region.facts["view"].as_str().unwrap().to_owned();
    assert!(browser.calls().iter().any(|call| matches!(
        call,
        BrowserCommand::ScreenshotRegion { region, expected_viewport, .. }
            if region.x == 210.0
                && region.y == 120.0
                && region.width == 400.0
                && region.height == 200.0
                && *expected_viewport == viewport
    )));

    browser.push(Ok(BrowserOutcome::Screenshot {
        tab_id: 7,
        mime_type: "image/jpeg".into(),
        data: "magnified-again".into(),
        width: 1600,
        height: 800,
        viewport: ViewportGeometry {
            scope: CaptureScope::Region,
            page_x: 310.0,
            page_y: 170.0,
            css_width: 100.0,
            css_height: 50.0,
            output_scale: 16.0,
            ..viewport
        },
    }));
    let second_region = executor.execute(
        &workspace,
        "browser_screenshot",
        json!({"view":first_region_view,"x":400,"y":200,"width":400,"height":200}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(second_region.status, Status::Succeeded);
    let current_view = second_region.facts["view"].as_str().unwrap().to_owned();
    assert!(matches!(
        browser.calls().last(),
        Some(BrowserCommand::ScreenshotRegion { region, expected_viewport, .. })
            if region.x == 310.0
                && region.y == 170.0
                && region.width == 100.0
                && region.height == 50.0
                && *expected_viewport == first_region_viewport
    ));

    browser.push(Ok(BrowserOutcome::Navigated {
        tab: tab(7, "https://example.org/"),
        committed_urls: vec!["https://example.org/".into()],
    }));
    let _ = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"tab":tab_handle,"url":"https://example.org"}),
        None,
        &CancellationToken::default(),
    );
    let before = browser.calls().len();
    let stale = executor.execute(
        &workspace,
        "browser_click",
        json!({"tab":tab_handle,"view":current_view,"x":1,"y":1}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(stale.facts["reason"], "stale_view");
    assert_eq!(browser.calls().len(), before);
}

#[test]
fn cross_workspace_tab_discovery_names_owning_workspace_and_guidance() {
    let (executor, browser, _, workspace_a, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    let workspace_b = executor
        .workspaces
        .admit("Workspace B".into(), IntakeChannel::Mcp, None);

    // Open a tab in Workspace B
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(42, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace_b,
        "browser_navigate",
        json!({"url": "https://example.com", "new_tab": true}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded);
    let tab_b = opened.facts["tab"].as_str().unwrap();

    // Attempt to use Workspace B's tab from Workspace A
    let result = executor.execute(
        &workspace_a,
        "browser_read",
        json!({"tab": tab_b}),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.facts["reason"], "ownership_mismatch");
    assert_eq!(result.facts["owner_workspace"], "Workspace B");
    assert_eq!(result.summary, "That tab handle belongs to Workspace B.");
    assert_eq!(
        result.next_steps,
        vec!["Switch workspace with browser_workspace, or call browser_tabs with action list to see tabs in the current workspace."]
    );
}

#[test]
fn browser_workspace_list_and_switch_resolves_cross_workspace_tab_ownership() {
    let (executor, browser, _, workspace_a, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    let workspace_b = executor
        .workspaces
        .admit("Workspace B".into(), IntakeChannel::Mcp, None);

    // Workspace B opens a tab.
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(42, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace_b,
        "browser_navigate",
        json!({"url": "https://example.com", "new_tab": true}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded);
    let tab_b = opened.facts["tab"].as_str().unwrap();

    // Workspace A lists workspaces: discovers Workspace B.
    let list_res = executor.execute(
        &workspace_a,
        "browser_workspace",
        json!({"action": "list"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(list_res.status, Status::Succeeded);
    assert_eq!(list_res.facts["count"], 2);
    let workspaces_arr = list_res.facts["workspaces"].as_array().unwrap();
    assert!(workspaces_arr
        .iter()
        .any(|w| w["workspace"] == workspace_b.as_str() && w["tab_count"] == 1));

    // Attempt switch to nonexistent workspace fails cleanly.
    let bad_switch = executor.execute(
        &workspace_a,
        "browser_workspace",
        json!({"action": "switch", "workspace": "workspace_nonexistent"}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(bad_switch.status, Status::Failed);
    assert_eq!(bad_switch.facts["reason"], "workspace_closed");

    // Workspace A switches to Workspace B.
    let switch_res = executor.execute(
        &workspace_a,
        "browser_workspace",
        json!({"action": "switch", "workspace": workspace_b.as_str()}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(switch_res.status, Status::Succeeded);
    assert_eq!(switch_res.facts["switched_workspace"], workspace_b.as_str());

    // Switched workspace now has direct access to read tab_b.
    browser.push(Ok(BrowserOutcome::Text {
        tab_id: 42,
        text: "Delightful Hello".into(),
        truncated: false,
        title: "Example".into(),
        url: "https://example.com/".into(),
    }));
    let read_res = executor.execute(
        &workspace_b,
        "browser_read",
        json!({"tab": tab_b}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(read_res.status, Status::Succeeded);
    assert_eq!(read_res.facts["text"], "Delightful Hello");
}

#[test]
fn wait_condition_visual_settle_observes_view_settlement() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 42,
        readiness: BrowserReadiness::Complete,
    }));
    let waited = executor.execute(
        &workspace,
        "browser_wait",
        json!({
            "tab": tab_handle,
            "condition": "visual_settle",
            "timeout_ms": 1_000
        }),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(waited.status, Status::Succeeded);
    assert_eq!(waited.facts["condition"], "visual_settle");
    assert_eq!(waited.facts["satisfied"], true);
    assert_eq!(waited.facts["elapsed_ms"], 42);
    assert!(waited
        .summary
        .contains("The view settled visually on example.com in under a second."));

    let calls = browser.calls();
    assert!(calls.iter().any(|call| matches!(
        call,
        BrowserCommand::Observe {
            condition,
            ..
        } if condition == "visual_settle"
    )));
}

#[test]
fn wait_composite_visual_settle_chains_after_primary_condition() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

    // Primary observe (load_ready)
    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 50,
        readiness: BrowserReadiness::Interactive,
    }));
    // Follow-up visual settle observe
    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 30,
        readiness: BrowserReadiness::Complete,
    }));

    let waited = executor.execute(
        &workspace,
        "browser_wait",
        json!({
            "tab": tab_handle,
            "condition": "load_ready",
            "visual_settle": true,
            "timeout_ms": 1_000
        }),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(waited.status, Status::Succeeded);
    assert_eq!(waited.facts["condition"], "load_ready");
    assert_eq!(waited.facts["satisfied"], true);
    assert_eq!(waited.facts["visual_settle"], true);
    assert_eq!(waited.facts["elapsed_ms"], 80);

    let calls = browser.calls();
    let observe_conditions: Vec<&str> = calls
        .iter()
        .filter_map(|call| match call {
            BrowserCommand::Observe { condition, .. } => Some(condition.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(observe_conditions, vec!["load_ready", "visual_settle"]);
}

#[test]
fn cross_workspace_tab_discovery_stress_permutations() {
    let (executor, browser, _, _, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);

    let names = ["Finance", "Legal", "Engineering", "Marketing", "Security"];
    let mut workspaces = Vec::new();
    let mut tab_handles = Vec::new();

    for (index, name) in names.iter().enumerate() {
        let ws = executor
            .workspaces
            .admit((*name).into(), IntakeChannel::Mcp, None);
        let pid = u64::try_from(100 + index).unwrap();
        browser.push(Ok(BrowserOutcome::TabOpened {
            reused: false,
            tab: tab(pid, &format!("https://{}.internal/", name.to_lowercase())),
            committed_urls: vec![format!("https://{}.internal/", name.to_lowercase())],
        }));
        let opened = executor.execute(
            &ws,
            "browser_navigate",
            json!({"url": format!("https://{}.internal/", name.to_lowercase()), "new_tab": true}),
            None,
            &CancellationToken::default(),
        );
        assert_eq!(opened.status, Status::Succeeded);
        let handle = opened.facts["tab"].as_str().unwrap().to_owned();
        tab_handles.push(handle);
        workspaces.push(ws);
    }

    // Stress: every workspace attempts to access every tab (25 total accesses)
    for (caller_idx, caller_ws) in workspaces.iter().enumerate() {
        for (target_idx, target_handle) in tab_handles.iter().enumerate() {
            if caller_idx == target_idx {
                // Own workspace access
                browser.push(Ok(BrowserOutcome::Text {
                    tab_id: u64::try_from(100 + target_idx).unwrap(),
                    text: "Authorized workspace content".into(),
                    truncated: false,
                    title: format!("{} Dashboard", names[target_idx]),
                    url: format!("https://{}.internal/", names[target_idx].to_lowercase()),
                }));
                let result = executor.execute(
                    caller_ws,
                    "browser_read",
                    json!({"tab": target_handle}),
                    None,
                    &CancellationToken::default(),
                );
                assert_eq!(result.status, Status::Succeeded);
            } else {
                // Cross-workspace access attempt
                let result = executor.execute(
                    caller_ws,
                    "browser_read",
                    json!({"tab": target_handle}),
                    None,
                    &CancellationToken::default(),
                );
                let expected_owner = names[target_idx];
                assert_eq!(result.status, Status::Failed);
                assert_eq!(result.facts["reason"], "ownership_mismatch");
                assert_eq!(result.facts["owner_workspace"], expected_owner);
                assert_eq!(
                    result.summary,
                    format!("That tab handle belongs to {expected_owner}.")
                );
                assert_eq!(
                    result.next_steps,
                    vec!["Switch workspace with browser_workspace, or call browser_tabs with action list to see tabs in the current workspace."]
                );
            }
        }
    }
}

#[test]
fn composite_visual_settle_stress_timeout_exhaustion() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

    // Primary condition (target_present) satisfies in 20ms
    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 20,
        readiness: BrowserReadiness::Interactive,
    }));
    // Follow-up visual settle fails (times out due to continuous animation) after 80ms
    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: false,
        elapsed_ms: 80,
        readiness: BrowserReadiness::Interactive,
    }));

    let waited = executor.execute(
        &workspace,
        "browser_wait",
        json!({
            "tab": tab_handle,
            "condition": "load_ready",
            "visual_settle": true,
            "timeout_ms": 100
        }),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(waited.status, Status::Failed);
    assert_eq!(waited.facts["satisfied"], false);
    assert_eq!(waited.facts["visual_settle"], true);
    assert_eq!(waited.facts["elapsed_ms"], 100);
    assert!(waited.next_steps.contains(
        &"Read or inspect the page to see its current state before choosing another action.".into()
    ));
}

#[test]
fn composite_visual_settle_stress_semantic_selector_chain() {
    let (executor, browser, _, workspace, _) = fixture();
    browser.connect(vec![summary(FAKE_BROWSER, true)]);
    browser.push(Ok(BrowserOutcome::TabOpened {
        reused: false,
        tab: tab(7, "https://example.com/"),
        committed_urls: vec!["https://example.com/".into()],
    }));
    let opened = executor.execute(
        &workspace,
        "browser_navigate",
        json!({"url":"https://example.com","new_tab":true}),
        None,
        &CancellationToken::default(),
    );
    let tab_handle = opened.facts["tab"].as_str().unwrap().to_owned();

    // Selector query misses once, then finds button
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![],
    }));
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![ObservedTarget {
            locator: "btn-submit".into(),
            role: "button".into(),
            name: "Submit".into(),
            state: vec![],
            credential_class: false,
        }],
    }));

    // Follow-up visual settle succeeds
    browser.push(Ok(BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 35,
        readiness: BrowserReadiness::Complete,
    }));

    let waited = executor.execute(
        &workspace,
        "browser_wait",
        json!({
            "tab": tab_handle,
            "condition": "selector_present",
            "selector": {"name": "Submit", "role": "button"},
            "visual_settle": true,
            "timeout_ms": 1_000
        }),
        None,
        &CancellationToken::default(),
    );

    assert_eq!(waited.status, Status::Succeeded);
    assert_eq!(waited.facts["condition"], "selector_present");
    assert_eq!(waited.facts["satisfied"], true);
    assert_eq!(waited.facts["visual_settle"], true);
    assert!(waited.facts["elapsed_ms"].as_u64().unwrap() >= 35);

    let calls = browser.calls();
    assert!(calls
        .iter()
        .any(|c| matches!(c, BrowserCommand::QuerySemantic { .. })));
    assert!(calls.iter().any(
        |c| matches!(c, BrowserCommand::Observe { condition, .. } if condition == "visual_settle")
    ));
}
