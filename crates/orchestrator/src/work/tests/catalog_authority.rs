//! Every advertised authority variant admits ordinary work with exactly its stated capabilities,
//! and a recording save checks its sources and its destination apart.

use super::*;
use crate::governance::CapabilitySet;
use crate::language::capability_map::DIRECTORY;
use serde_json::Value;
use std::collections::BTreeSet;

const RAWX: [&str; 4] = ["read", "action", "write", "execute"];
const PAGE: &str = "https://example.com/";

struct Case {
    tool: &'static str,
    variant: Option<&'static str>,
    required: &'static [&'static str],
    arguments: Value,
    replies: Vec<BrowserOutcome>,
}

/// The row every listed control sits in, which only the model may read.
const ROW: &str = "PRIVATE_ROW";

/// An ordinary control in the second of two repeated rows.
fn field(locator: &str, role: &str) -> ObservedTarget {
    ObservedTarget {
        locator: locator.into(),
        role: role.into(),
        name: "Ordinary control".into(),
        state: vec![],
        credential_class: false,
        context: Some(ROW.into()),
        nth: Some(2),
        ..Default::default()
    }
}

fn cases(target: &Value, button: &Value, tab_handle: &Value) -> Vec<Case> {
    let mut result = Vec::new();
    let mut add = |tool, variant, required, arguments, replies| {
        result.push(Case {
            tool,
            variant,
            required,
            arguments,
            replies,
        });
    };
    let navigated = || BrowserOutcome::Navigated {
        tab: tab(7, PAGE),
        committed_urls: vec![PAGE.into()],
    };
    let targets = || BrowserOutcome::targets(7, vec![field("draft", "textbox")]);
    let describe = |submit| BrowserOutcome::TargetsDescribed {
        tab_id: 7,
        targets: if submit {
            vec![field("draft", "textbox"), field("submit", "button")]
        } else {
            vec![field("draft", "textbox")]
        },
        rebound: false,
    };
    let observed = || BrowserOutcome::Observed {
        tab_id: 7,
        satisfied: true,
        elapsed_ms: 1,
        readiness: BrowserReadiness::Complete,
    };
    add(
        "browser_tabs",
        Some("list"),
        &["read"][..],
        json!({"action":"list"}),
        vec![BrowserOutcome::Tabs {
            tabs: vec![tab(7, PAGE)],
        }],
    );
    add(
        "browser_tabs",
        Some("focus"),
        &[],
        json!({"action":"focus","tab":tab_handle}),
        vec![BrowserOutcome::TabFocused {
            tab_id: 7,
            active: true,
            window_focused: true,
        }],
    );
    add(
        "browser_tabs",
        Some("close"),
        &["action"],
        json!({"action":"close","tab":tab_handle}),
        vec![BrowserOutcome::TabClosed { tab_id: 7 }],
    );
    add(
        "browser_navigate",
        None,
        &["read"],
        json!({"url":PAGE,"tab":tab_handle}),
        vec![navigated()],
    );
    add(
        "browser_history",
        None,
        &["action"],
        json!({"action":"back"}),
        vec![navigated()],
    );
    add(
        "browser_window",
        Some("zoom"),
        &["read"],
        json!({"action":"zoom","percent":100}),
        vec![BrowserOutcome::Zoomed {
            tab_id: 7,
            zoom: 1.0,
        }],
    );
    add(
        "browser_window",
        Some("resize"),
        &[],
        json!({"action":"resize","width":800,"height":600}),
        vec![BrowserOutcome::WindowResized {
            tab_id: 7,
            width: 800,
            height: 600,
            affected_tab_ids: vec![7],
        }],
    );
    add(
        "browser_read",
        None,
        &["read"],
        json!({}),
        vec![BrowserOutcome::Text {
            tab_id: 7,
            text: "PRIVATE_PAGE_TEXT".into(),
            truncated: false,
            title: "Example".into(),
            url: PAGE.into(),
        }],
    );
    add(
        "browser_inspect",
        None,
        &["read"],
        json!({}),
        vec![targets()],
    );
    add(
        "browser_find",
        None,
        &["read"],
        json!({"text":"Ordinary"}),
        vec![targets()],
    );
    add(
        "browser_screenshot",
        None,
        &["read"],
        json!({}),
        vec![BrowserOutcome::Screenshot {
            tab_id: 7,
            mime_type: "image/jpeg".into(),
            data: "PRIVATE_IMAGE_BYTES".into(),
            width: 800,
            height: 600,
            viewport: ViewportGeometry {
                scope: CaptureScope::Viewport,
                page_x: 0.0,
                page_y: 0.0,
                css_width: 800.0,
                css_height: 600.0,
                visual_page_x: 0.0,
                visual_page_y: 0.0,
                visual_css_width: 800.0,
                visual_css_height: 600.0,
                device_scale: 1.0,
                zoom: 1.0,
                output_scale: 1.0,
            },
        }],
    );
    add(
        "browser_click",
        None,
        &["action"],
        json!({"target":button}),
        vec![BrowserOutcome::Activated {
            tab: tab(7, PAGE),
            subject: None,
            committed_urls: vec![PAGE.into()],
        }],
    );
    add(
        "browser_click",
        Some("selector"),
        &["read", "action"],
        json!({"selector":{"name":"Ordinary control","role":"button","exact":true}}),
        vec![
            BrowserOutcome::targets(7, vec![field("submit", "button")]),
            BrowserOutcome::Activated {
                tab: tab(7, PAGE),
                subject: None,
                committed_urls: vec![PAGE.into()],
            },
        ],
    );
    add(
        "browser_click",
        Some("expect"),
        &["read", "action"],
        json!({"target":button,"expect":{"condition":"load_ready"}}),
        vec![
            BrowserOutcome::Activated {
                tab: tab(7, PAGE),
                subject: None,
                committed_urls: vec![PAGE.into()],
            },
            observed(),
        ],
    );
    add(
        "browser_scroll",
        None,
        &["read"],
        json!({}),
        vec![BrowserOutcome::Scrolled {
            tab_id: 7,
            x: 0.0,
            y: 50.0,
            subject: None,
        }],
    );
    add(
        "browser_hover",
        None,
        &["read"],
        json!({"target":button}),
        vec![BrowserOutcome::Hovered {
            tab_id: 7,
            subject: None,
        }],
    );
    for submit in [false, true] {
        let mut arguments = json!({"fields":[{"target":target,"value":"PRIVATE_DRAFT"}]});
        if submit {
            arguments["submit_target"] = button.clone();
        }
        add(
            "browser_fill_form",
            Some(if submit { "submit" } else { "fill" }),
            if submit {
                &["read", "write", "action"]
            } else {
                &["read", "write"]
            },
            arguments,
            vec![
                describe(submit),
                BrowserOutcome::Filled {
                    rebound: false,
                    tab: tab(7, PAGE),
                    filled_count: 1,
                    held: Vec::new(),
                    submitted: submit,
                    committed_urls: vec![PAGE.into()],
                },
            ],
        );
    }
    add(
        "browser_type_text",
        None,
        &["action"],
        json!({"target":target,"text":"PRIVATE_DRAFT"}),
        vec![
            describe(false),
            BrowserOutcome::Typed {
                tab: tab(7, PAGE),
                character_count: 13,
                subject: None,
                committed_urls: vec![PAGE.into()],
            },
        ],
    );
    add(
        "browser_type_text",
        Some("selector"),
        &["read", "action"],
        json!({"selector":{"name":"Ordinary control","role":"textbox","exact":true},"text":"PRIVATE_DRAFT"}),
        vec![
            targets(),
            describe(false),
            BrowserOutcome::Typed {
                tab: tab(7, PAGE),
                character_count: 13,
                subject: None,
                committed_urls: vec![PAGE.into()],
            },
        ],
    );
    add(
        "browser_type_text",
        Some("expect"),
        &["read", "action"],
        json!({"focused":true,"text":"PRIVATE_DRAFT","expect":{"condition":"load_ready"}}),
        vec![
            describe(false),
            BrowserOutcome::Typed {
                tab: tab(7, PAGE),
                character_count: 13,
                subject: None,
                committed_urls: vec![PAGE.into()],
            },
            observed(),
        ],
    );
    add(
        "browser_press_key",
        None,
        &["action"],
        json!({"key":"Tab"}),
        vec![BrowserOutcome::KeyPressed {
            tab: tab(7, PAGE),
            key: "Tab".into(),
            subject: None,
            committed_urls: vec![PAGE.into()],
        }],
    );
    add(
        "browser_press_key",
        Some("expect"),
        &["read", "action"],
        json!({"key":"Tab","expect":{"condition":"load_ready"}}),
        vec![
            BrowserOutcome::KeyPressed {
                tab: tab(7, PAGE),
                key: "Tab".into(),
                subject: None,
                committed_urls: vec![PAGE.into()],
            },
            observed(),
        ],
    );
    add(
        "browser_drag",
        None,
        &["action"],
        json!({"source_target":target,"destination_target":button}),
        vec![BrowserOutcome::Dragged {
            tab: tab(7, PAGE),
            source_subject: None,
            destination_subject: None,
            committed_urls: vec![PAGE.into()],
        }],
    );
    add(
        "browser_upload",
        None,
        &["write"],
        json!({"target":target,"files":[{"name":"PRIVATE_FILE.txt","data_base64":"eA=="}]}),
        vec![
            describe(false),
            BrowserOutcome::FilesUploaded {
                tab_id: 7,
                uploaded_count: 1,
                uploaded_bytes: 1,
                subject: None,
            },
        ],
    );
    add(
        "browser_upload",
        Some("selector"),
        &["read", "write"],
        json!({"selector":{"name":"Ordinary control","role":"textbox","exact":true},"files":[{"name":"PRIVATE_FILE.txt","data_base64":"eA=="}]}),
        vec![
            targets(),
            describe(false),
            BrowserOutcome::FilesUploaded {
                tab_id: 7,
                uploaded_count: 1,
                uploaded_bytes: 1,
                subject: None,
            },
        ],
    );
    add(
        "browser_execute",
        None,
        &["execute"],
        json!({"script":"'PRIVATE_SCRIPT'"}),
        vec![BrowserOutcome::ScriptEvaluated {
            tab: tab(7, PAGE),
            value: "\"PRIVATE_SCRIPT_RESULT\"".into(),
            truncated: false,
            form: crate::browser::adapter::ScriptForm::AsWritten,
            committed_urls: vec![PAGE.into()],
        }],
    );
    add(
        "browser_wait",
        None,
        &["read"],
        json!({"condition":"load_ready"}),
        vec![observed(), observed()],
    );
    // Wrappers need no capability of their own. Their concrete children still need Read.
    add(
        "browser_flow",
        None,
        &["read"],
        json!({"steps":[{"id":"PRIVATE_STEP_LABEL","tool":"browser_wait","arguments":{"condition":"load_ready"}}]}),
        vec![observed(), observed()],
    );
    add(
        "browser_dialog",
        Some("status"),
        &["read"],
        json!({"action":"status"}),
        vec![BrowserOutcome::Dialog {
            tab_id: 7,
            present: true,
            dialog_type: "confirm".into(),
            message: None,
            url: Some(PAGE.into()),
            default_prompt: None,
        }],
    );
    add(
        "browser_dialog",
        Some("resolve"),
        &["action"],
        json!({"action":"dismiss"}),
        vec![
            BrowserOutcome::Dialog {
                tab_id: 7,
                present: true,
                dialog_type: "confirm".into(),
                message: None,
                url: Some(PAGE.into()),
                default_prompt: None,
            },
            BrowserOutcome::DialogHandled {
                tab_id: 7,
                accepted: false,
                dialog_type: "confirm".into(),
                url: Some(PAGE.into()),
            },
        ],
    );
    add(
        "browser_record",
        Some("start"),
        &["read"],
        json!({"action":"start"}),
        vec![BrowserOutcome::RecordingStarted {
            recording_id: "recording_1".into(),
            summary: recording_summary(RecordingState::Recording, PAGE),
            existing: false,
        }],
    );
    add(
        "browser_record",
        Some("inspect"),
        &[],
        json!({"action":"status"}),
        vec![BrowserOutcome::RecordingStatus {
            summary: recording_summary(RecordingState::Frozen, PAGE),
        }],
    );
    for attached in [false, true] {
        let mut arguments = json!({"action":"save"});
        let mut replies = vec![BrowserOutcome::RecordingStopped {
            summary: recording_summary(RecordingState::Frozen, PAGE),
            changed: false,
        }];
        if attached {
            arguments["target"] = target.clone();
            replies.push(describe(false));
        }
        replies.push(BrowserOutcome::RecordingExported {
            summary: recording_summary(RecordingState::Frozen, PAGE),
            encoded: EncodedRecording {
                frame_count: 1,
                captured_frame_count: 1,
                duration_ms: 500,
                width: 800,
                height: 600,
                byte_count: 6,
            },
            delivery: if attached {
                RecordingDelivery::Attached { tab_id: 7 }
            } else {
                RecordingDelivery::Returned {
                    mime_type: "image/gif".into(),
                    data: "R0lGODlh".into(),
                }
            },
        });
        add(
            "browser_record",
            Some(if attached {
                "save_target"
            } else {
                "save_client"
            }),
            if attached {
                &["read", "write"]
            } else {
                &["read"]
            },
            arguments,
            replies,
        );
    }
    add(
        "browser_diagnose",
        None,
        &["read"],
        json!({}),
        vec![BrowserOutcome::DiagnosticsRead {
            tab_id: 7,
            entries: vec![],
            cursor: None,
            truncated: false,
            evicted: false,
            capture_started: true,
            omitted_count: 0,
        }],
    );
    add("policy_explain", None, &[], json!({}), vec![]);
    result
}

fn established(
    executor: &ApplicationExecutor,
    browser: &FakeBrowser,
    workspace: &crate::workspace::WorkspaceId,
) -> (Value, Value, Value) {
    browser.push(Ok(BrowserOutcome::TabOpened {
        tab: tab(7, PAGE),
        committed_urls: vec![PAGE.into()],
    }));
    let opened = executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":PAGE}),
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded);
    browser.push(Ok(BrowserOutcome::targets(
        7,
        vec![field("draft", "textbox"), field("submit", "button")],
    )));
    let inspected = executor.execute(
        workspace,
        "browser_inspect",
        json!({}),
        &CancellationToken::default(),
    );
    assert_eq!(inspected.status, Status::Succeeded);
    assert_eq!(inspected.facts["items"][0]["context"], ROW);
    assert_eq!(inspected.facts["items"][0]["nth"], 2);
    (
        opened.facts["tab"].clone(),
        inspected.facts["items"][0]["target"].clone(),
        inspected.facts["items"][1]["target"].clone(),
    )
}

/// What the audit and the history keep of `audit`: neither may hold a caller's step label, a
/// browser payload, or a control's row.
fn assert_kept_clean(audit: &MemoryAudit, label: &str) {
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
        assert!(!kept.contains("PRIVATE_"), "{label}: {kept}");
        assert!(
            !kept.contains("\"context\"") && !kept.contains("\"nth\""),
            "{label}"
        );
    }
}

/// ACCEPTANCE Governance 3, 10 and 13, ADR-0185 items 1-2, ADR-0156 items 1-2 and C10b: every
/// catalog variant is admitted by exactly its RAWX set, directly and as a flow step.
///
/// The directory advertises what each variant is proven to need. With no policy every variant
/// runs. Under configured policy, a variant missing any one capability is refused before any
/// browser command, with the whole requirement and the configured layer in its refused check and
/// no caller restriction named; the refusals neither hold the session nor tell any adapter about
/// control state, and the same call then runs once its exact grant is published. An admitted
/// call writes its own record, classified as the catalog says, admitted first against that set
/// with the grant that allowed it, and a single-stage variant checks nothing else. Neither the
/// audit nor the history ever keeps a caller's label, a browser payload or a control's row,
/// which the model does read.
#[test]
fn every_catalog_variant_is_admitted_by_exactly_its_rawx_set() {
    // These variants check a later stage after their admission, under that stage's own
    // requirement: an action's landing on the pages it committed asks for Action alone, and a
    // recording placed on a page asks for Read on its sources and Write on its destination.
    const STAGED: &[(&str, Option<&str>)] = &[
        ("browser_click", Some("selector")),
        ("browser_click", Some("expect")),
        ("browser_type_text", Some("selector")),
        ("browser_type_text", Some("expect")),
        ("browser_press_key", Some("expect")),
        ("browser_record", Some("save_target")),
    ];
    let templates = cases(&json!("target_1"), &json!("target_2"), &json!("tab_1"));
    let expected: BTreeSet<_> = DIRECTORY
        .iter()
        .map(|entry| (entry.tool, entry.variant))
        .collect();
    assert_eq!(
        templates
            .iter()
            .map(|case| (case.tool, case.variant))
            .collect::<BTreeSet<_>>(),
        expected,
        "A new advertised variant needs an ordinary browser journey and denial control here"
    );
    for case in &templates {
        let advertised = DIRECTORY
            .iter()
            .find(|entry| (entry.tool, entry.variant) == (case.tool, case.variant))
            .unwrap();
        let requirements: BTreeSet<_> = if matches!(case.tool, "browser_flow") {
            BTreeSet::new()
        } else {
            case.required.iter().copied().collect()
        };
        assert_eq!(
            advertised
                .requirements
                .iter()
                .map(|capability| capability.as_str())
                .collect::<BTreeSet<_>>(),
            requirements,
            "{} {:?}: advertised requirements must match the proven browser behavior",
            case.tool,
            case.variant
        );
    }

    let mut failures = Vec::new();
    for (index, template) in templates.iter().enumerate() {
        for composed in [false, true] {
            if composed && matches!(template.tool, "browser_flow") {
                continue;
            }
            let label = format!(
                "{} {:?} composed={composed}",
                template.tool, template.variant
            );
            let invoke = |executor: &ApplicationExecutor,
                          workspace: &crate::workspace::WorkspaceId,
                          case: &Case| {
                let arguments = if composed {
                    json!({"steps":[{"id":"PRIVATE_STEP_LABEL","tool":case.tool,"arguments":case.arguments}]})
                } else {
                    case.arguments.clone()
                };
                executor.execute(
                    workspace,
                    if composed { "browser_flow" } else { case.tool },
                    arguments,
                    &CancellationToken::default(),
                )
            };

            // With no policy, the variant runs.
            let (executor, browser, _, workspace, audit) = fixture();
            let (tab_handle, target, button) = established(&executor, &browser, &workspace);
            let case = cases(&target, &button, &tab_handle).swap_remove(index);
            if case.tool == "browser_record" && case.variant != Some("start") {
                holding_recording(&executor, &workspace);
            }
            for reply in case.replies.clone() {
                browser.push(Ok(reply));
            }
            let result = invoke(&executor, &workspace, &case);
            if result.status != Status::Succeeded {
                failures.push(format!("{label}, no policy: {result:?}"));
                continue;
            }
            assert_kept_clean(&audit, &label);

            // Under configured policy: refused without each capability, then admitted by its set.
            let policy = TestPolicy::new();
            let (executor, browser, _, workspace, audit) = fixture_with_governance(policy.facade());
            executor.governance.relay_runtime_state(browser.clone());
            let (tab_handle, target, button) = established(&executor, &browser, &workspace);
            let case = cases(&target, &button, &tab_handle).swap_remove(index);
            if case.tool == "browser_record" && case.variant != Some("start") {
                holding_recording(&executor, &workspace);
            }
            for reply in case.replies.clone() {
                browser.push(Ok(reply));
            }
            let before = browser.calls().len();
            for missing in case.required {
                policy.set(json!(RAWX
                    .into_iter()
                    .filter(|candidate| candidate != missing)
                    .collect::<Vec<_>>()));
                let result = invoke(&executor, &workspace, &case);
                let label = format!("{label}, missing {missing}");
                if result.status != Status::Blocked {
                    failures.push(format!("{label}: {result:?}"));
                    continue;
                }
                assert_eq!(result.effect, Effect::None, "{label}");
                assert_eq!(browser.calls().len(), before, "{label}: no browser work");
                let facts = if composed {
                    &result.facts["steps"][0]["result"]["facts"]
                } else {
                    &result.facts
                };
                assert!(facts.get("restriction").is_none(), "{label}");
                let refused: Vec<_> = audit
                    .0
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|record| record.invocation == result.invocation)
                    .flat_map(|record| record.permissions.checks.clone())
                    .filter(|check| !check.allowed)
                    .collect();
                assert!(
                    refused.iter().any(|check| check
                        .requirements
                        .iter()
                        .map(|cap| cap.as_str())
                        .collect::<BTreeSet<_>>()
                        == case.required.iter().copied().collect::<BTreeSet<_>>()),
                    "{label}: the refused check keeps the whole requirement: {refused:?}"
                );
                assert!(
                    refused.iter().all(|check| !check.layers.is_empty()),
                    "{label}: configured policy owns the refusal"
                );
                assert_eq!(
                    executor.governance.runtime_state(),
                    RuntimeControlState::Active,
                    "{label}: a refusal holds nothing"
                );
            }
            policy.set(json!(if case.required.is_empty() {
                vec!["execute"]
            } else {
                case.required.to_vec()
            }));
            let result = invoke(&executor, &workspace, &case);
            if result.status != Status::Succeeded {
                failures.push(format!("{label}, exact grant: {result:?}"));
                continue;
            }
            assert_eq!(
                browser.calls().len() - before,
                case.replies.len(),
                "{label}: all physical work is acknowledged exactly once"
            );
            assert!(browser.control_states().is_empty(), "{label}");
            if case.tool != "browser_flow" {
                let classified = crate::language::capability_map::requirements(
                    &crate::language::decode(case.tool, case.arguments.clone()).unwrap(),
                );
                let records = audit.0.lock().unwrap();
                let record = records
                    .iter()
                    .find(|record| {
                        record.invocation == result.invocation && record.tool == case.tool
                    })
                    .unwrap_or_else(|| panic!("{label}: the call writes its own record"));
                assert_eq!(record.capabilities, classified, "{label}");
                let checks: Vec<_> = record
                    .permissions
                    .checks
                    .iter()
                    .map(|check| check.requirements)
                    .collect();
                let single_stage = !STAGED.contains(&(case.tool, case.variant));
                if checks.first() != Some(&classified)
                    || (single_stage && checks.iter().any(|checked| *checked != classified))
                {
                    failures.push(format!(
                        "{label}: classified {classified:?}, checked {checks:?}"
                    ));
                }
                assert!(
                    case.required.is_empty()
                        || record.permissions.checks.iter().any(|check| check.allowed
                            && check
                                .layers
                                .iter()
                                .any(|layer| layer.grants.iter().any(|grant| grant == "test"))),
                    "{label}: the grant that allowed it is kept"
                );
            }
            assert_kept_clean(&audit, &label);
        }
    }
    assert!(
        failures.is_empty(),
        "{} authority regressions:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// ADR-0158 item 4 and C10b: a recording save needs Read on every page it recorded and Write on
/// the page it is placed on, each checked on its own host. A refused save stops the capture but
/// exports nothing, and its record holds one refused check, the deciding one, with the configured
/// layer, the reason the result gives, and no source host, since a source may be embedded.
#[test]
fn a_recording_save_needs_read_on_its_sources_and_write_on_its_destination() {
    for (source_read, destination_write) in [(true, true), (false, true), (true, false)] {
        for composed in [false, true] {
            let (mut executor, browser, _, workspace, audit) = fixture();
            let (tab_handle, target, button) = established(&executor, &browser, &workspace);
            holding_recording(&executor, &workspace);
            let policy = temporary_policy("recording-distinct-source-and-target");
            fs::write(&policy, serde_json::to_vec(&json!({
                "schema":4,"name":"Separate source and destination","version":"1","grants":[
                    {"id":"source","hosts":{"allow":["source.example"]},"allowed":[if source_read { "read" } else { "write" }]},
                    {"id":"destination","hosts":{"allow":["example.com"]},"allowed":[if destination_write { "write" } else { "read" }]},
                    {"id":"elsewhere","hosts":{"allow":["unrelated.example"]},"allowed":["read","write"]}
                ]
            })).unwrap()).unwrap();
            executor.governance = GovernanceFacade::new(Some(policy.to_path_buf()));
            let mut case = cases(&target, &button, &tab_handle)
                .into_iter()
                .find(|case| case.variant == Some("save_target"))
                .unwrap();
            for reply in &mut case.replies {
                if let BrowserOutcome::RecordingStopped { summary, .. }
                | BrowserOutcome::RecordingExported { summary, .. } = reply
                {
                    summary.source_urls = vec!["https://source.example/PRIVATE_SOURCE_PATH".into()];
                }
            }
            for reply in case.replies {
                browser.push(Ok(reply));
            }
            let before = browser.calls().len();
            let arguments = if composed {
                json!({"steps":[{"id":"attach","tool":"browser_record","arguments":case.arguments}]})
            } else {
                case.arguments
            };

            let result = executor.execute(
                &workspace,
                if composed {
                    "browser_flow"
                } else {
                    "browser_record"
                },
                arguments,
                &CancellationToken::default(),
            );
            drop(policy);
            let label = format!(
                "source_read={source_read} destination_write={destination_write} composed={composed}"
            );
            let permitted = source_read && destination_write;
            assert_eq!(
                result.status,
                if permitted {
                    Status::Succeeded
                } else {
                    Status::Blocked
                },
                "{label}: {result:?}"
            );
            let commands = browser.calls();
            if permitted {
                assert_eq!(commands.len() - before, 3);
                assert!(matches!(
                    commands.last().unwrap(),
                    BrowserCommand::ExportRecording {
                        destination: RecordingDestination::Target { .. },
                        ..
                    }
                ));
                assert_eq!(result.effect, Effect::Applied);
                assert!(!result.repeat_safe);
            } else {
                assert_eq!(
                    commands.len() - before,
                    1,
                    "{label}: no target preflight or export after a refusal"
                );
                assert!(matches!(
                    commands.last().unwrap(),
                    BrowserCommand::StopRecording { .. }
                ));
                assert_eq!(result.effect, Effect::None);
                let records = audit.0.lock().unwrap();
                let refused: Vec<_> = records
                    .iter()
                    .filter(|record| record.tool == "browser_record")
                    .flat_map(|record| record.permissions.checks.iter())
                    .filter(|check| !check.allowed)
                    .collect();
                assert_eq!(refused.len(), 1, "{label}: {refused:?}");
                let deciding = refused[0];
                assert_eq!(
                    deciding.requirements,
                    if source_read {
                        CapabilitySet::WRITE
                    } else {
                        CapabilitySet::READ
                    },
                    "{label}"
                );
                assert!(!deciding.layers.is_empty(), "{label}");
                if !composed {
                    assert_eq!(deciding.reason.as_str(), result.facts["reason"], "{label}");
                }
                if !source_read {
                    assert_eq!(deciding.host, None, "{label}: a source may be embedded");
                }
                assert!(
                    !serde_json::to_string(&*records)
                        .unwrap()
                        .contains("source.example"),
                    "{label}"
                );
            }
            assert_kept_clean(&audit, &label);
        }
    }
}
