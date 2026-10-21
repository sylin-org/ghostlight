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

fn ordinary() -> ObservedTarget {
    ObservedTarget {
        locator: "email".into(),
        role: "textbox".into(),
        name: "Email".into(),
        state: vec![],
        credential_class: false,
        ..Default::default()
    }
}

/// A session with one tab on example.com whose inspection listed `targets`; answers their handles.
fn inspected(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
    targets: Vec<ObservedTarget>,
) -> Vec<String> {
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
    assert_eq!(opened.status, Status::Succeeded);
    browser.push(Ok(BrowserOutcome::targets(7, targets)));
    let listed = executor.execute(
        workspace,
        "browser_inspect",
        json!({}),
        &CancellationToken::default(),
    );
    listed.facts["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["target"].as_str().unwrap().to_owned())
        .collect()
}

/// Whether any command the browser received would put text into a page.
fn typed_anything(browser: &FakeBrowser) -> bool {
    browser.calls().iter().any(|command| {
        matches!(
            command,
            BrowserCommand::Fill(_) | BrowserCommand::TypeText(_) | BrowserCommand::TypeFocused(_)
        )
    })
}

/// ADR-0185 item 3: credential input needs the user's word on each call.
///
/// Without `user_authorized_credentials`, a fill by target, a grouped fill whose later field is a
/// credential, and focused typing send nothing, say nothing was typed, name the acknowledgement
/// as the way forward, and keep the field's name and the value out of the audit; a fill by
/// selector is checked the same way. An upload is not typed text and needs no acknowledgement,
/// even on a credential-like input. With it, the
/// value reaches the browser for fill, targeted and focused typing, directly and in a flow, and
/// stays out of the audit and the result; the next call without it is refused again. Guidance
/// in a flow under `continue` holds no later step, and the acknowledgement never lifts a policy
/// refusal or the person's Pause or End.
#[test]
fn credential_input_needs_the_users_word_on_each_call() {
    // Without the acknowledgement nothing is typed, a grouped fill's ordinary field included.
    for fields in [vec![credential()], vec![ordinary(), credential()]] {
        let (executor, browser, _, workspace, audit) = fixture();
        let handles = inspected(&executor, &browser, &workspace, fields.clone());
        browser.push(Ok(BrowserOutcome::TargetsDescribed {
            rebound: false,
            tab_id: 7,
            targets: fields,
        }));
        let values: Vec<_> = handles
            .iter()
            .map(|handle| json!({"target":handle,"value":"PRIVATE_CREDENTIAL"}))
            .collect();
        let result = executor.execute(
            &workspace,
            "browser_fill_form",
            json!({ "fields": values }),
            &CancellationToken::default(),
        );
        assert_eq!(result.status, Status::Blocked, "{result:?}");
        assert_eq!(result.effect, Effect::None);
        assert_eq!(result.facts["values_sent"], false);
        assert_eq!(result.facts["reason"], "credential_authorization_required");
        let credential_handle = handles.last().unwrap();
        assert!(
            result.summary.contains(credential_handle.as_str()),
            "the refusal names the credential control's handle: {}",
            result.summary
        );
        assert!(result
            .next_steps
            .iter()
            .any(|step| step.contains("user_authorized_credentials")));
        assert!(!typed_anything(&browser), "nothing reaches the page");
        let recorded = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
        assert!(!recorded.contains("Password"));
        assert!(!recorded.contains("PRIVATE_CREDENTIAL"));
    }

    // A fill by selector looks the control up and checks it, and still types nothing.
    let (executor, browser, _, workspace, audit) = fixture();
    inspected(&executor, &browser, &workspace, vec![]);
    browser.push(Ok(BrowserOutcome::targets(7, vec![credential()])));
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: vec![credential()],
    }));
    let result = executor.execute(
        &workspace,
        "browser_fill_form",
        json!({"fields":[{
            "selector":{"name":"Password","role":"textbox","exact":true},
            "value":"PRIVATE_CREDENTIAL"
        }]}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked, "{result:?}");
    assert_eq!(result.facts["values_sent"], false);
    assert!(!typed_anything(&browser));
    assert!(!serde_json::to_string(&*audit.0.lock().unwrap())
        .unwrap()
        .contains("PRIVATE_CREDENTIAL"));

    // A file is not typed text: an upload to a credential-like input needs no acknowledgement,
    // and the file's name and bytes stay out of the audit.
    let (executor, browser, _, workspace, audit) = fixture();
    inspected(&executor, &browser, &workspace, vec![]);
    let input = ObservedTarget {
        locator: "attachment".into(),
        role: "button".into(),
        name: "Attachment".into(),
        credential_class: true,
        ..Default::default()
    };
    browser.push(Ok(BrowserOutcome::targets(7, vec![input.clone()])));
    browser.push(Ok(BrowserOutcome::TargetsDescribed {
        rebound: false,
        tab_id: 7,
        targets: vec![input],
    }));
    browser.push(Ok(BrowserOutcome::FilesUploaded {
        tab_id: 7,
        uploaded_count: 1,
        uploaded_bytes: 1,
        subject: None,
    }));
    let result = executor.execute(
        &workspace,
        "browser_upload",
        json!({
            "selector":{"name":"Attachment","role":"button","exact":true},
            "files":[{"name":"PRIVATE_FILE.txt","data_base64":"eA=="}]
        }),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Succeeded, "{result:?}");
    assert!(browser
        .calls()
        .iter()
        .any(|command| matches!(command, BrowserCommand::UploadFiles { locator, .. } if locator == "attachment")));
    let saved = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(!saved.contains("PRIVATE_FILE") && !saved.contains("eA=="));

    // With it, the value reaches the browser and nowhere else, once.
    for mode in ["fill", "target", "focused"] {
        for composed in [false, true] {
            let (executor, browser, _, workspace, audit) = fixture();
            let target = &inspected(&executor, &browser, &workspace, vec![credential()])[0];
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
        }
    }

    // Guidance inside a flow under `continue` holds no later step.
    let (executor, browser, _, workspace, _) = fixture();
    inspected(&executor, &browser, &workspace, vec![]);
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
    assert!(!typed_anything(&browser));

    // The acknowledgement never lifts a policy refusal or the person's Pause or End.
    for boundary in ["policy", "pause", "end"] {
        let policy = TestPolicy::new();
        let (executor, browser, _, workspace, _) = fixture_with_governance(policy.facade());
        inspected(&executor, &browser, &workspace, vec![]);
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
        assert_eq!(browser.calls().len(), before, "{boundary}");
    }
}
