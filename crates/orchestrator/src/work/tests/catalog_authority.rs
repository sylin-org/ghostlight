//! Every advertised authority variant must admit ordinary work with its stated capabilities.

use super::*;
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

fn field(locator: &str, role: &str) -> ObservedTarget {
    ObservedTarget {
        locator: locator.into(),
        role: role.into(),
        name: "Ordinary control".into(),
        state: vec![],
        credential_class: false,
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
    let targets = || BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![field("draft", "textbox")],
    };
    let describe = |submit| BrowserOutcome::TargetsDescribed {
        tab_id: 7,
        targets: if submit {
            vec![field("draft", "textbox"), field("submit", "button")]
        } else {
            vec![field("draft", "textbox")]
        },
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
            BrowserOutcome::Targets {
                tab_id: 7,
                targets: vec![field("submit", "button")],
            },
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
                    tab: tab(7, PAGE),
                    filled_count: 1,
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
            committed_urls: vec![PAGE.into()],
        }],
    );
    add(
        "browser_wait",
        None,
        &["read"],
        json!({"condition":"load_ready"}),
        vec![observed()],
    );
    // Wrappers need no capability of their own. Their concrete children still need Read.
    add(
        "browser_sequence",
        None,
        &["read"],
        json!({"steps":[{"action":"wait","condition":"load_ready"},{"action":"wait","condition":"load_ready"}]}),
        vec![observed(), observed()],
    );
    add(
        "browser_flow",
        None,
        &["read"],
        json!({"steps":[{"id":"PRIVATE_STEP_LABEL","tool":"browser_wait","arguments":{"condition":"load_ready"}}]}),
        vec![observed()],
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
            },
            BrowserOutcome::DialogHandled {
                tab_id: 7,
                accepted: false,
                dialog_type: "confirm".into(),
            },
        ],
    );
    add(
        "browser_record",
        Some("start"),
        &["read"],
        json!({"action":"start"}),
        vec![BrowserOutcome::RecordingStarted {
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
        reused: false,
    }));
    let opened = executor.execute(
        workspace,
        "browser_navigate",
        json!({"url":PAGE}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(opened.status, Status::Succeeded);
    browser.push(Ok(BrowserOutcome::Targets {
        tab_id: 7,
        targets: vec![field("draft", "textbox"), field("submit", "button")],
    }));
    let inspected = executor.execute(
        workspace,
        "browser_inspect",
        json!({}),
        None,
        &CancellationToken::default(),
    );
    assert_eq!(inspected.status, Status::Succeeded);
    (
        opened.facts["tab"].clone(),
        inspected.facts["items"][0]["target"].clone(),
        inspected.facts["items"][1]["target"].clone(),
    )
}

#[test]
fn every_catalog_variant_has_positive_and_missing_capability_executor_coverage() {
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
        let requirements: BTreeSet<_> = if matches!(case.tool, "browser_flow" | "browser_sequence")
        {
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
        let exact = if template.required.is_empty() {
            vec!["execute"]
        } else {
            template.required.to_vec()
        };
        let restrictions = std::iter::once(None)
            .chain(std::iter::once(Some(exact)))
            .chain(template.required.iter().map(|missing| {
                Some(
                    RAWX.into_iter()
                        .filter(|candidate| candidate != missing)
                        .collect(),
                )
            }));
        for (restriction_index, restriction) in restrictions.enumerate() {
            for composed in [false, true] {
                if composed && matches!(template.tool, "browser_flow" | "browser_sequence") {
                    continue;
                }
                let (executor, browser, _, workspace, audit) = fixture();
                let (tab_handle, target, button) = established(&executor, &browser, &workspace);
                let case = cases(&target, &button, &tab_handle).swap_remove(index);
                let permitted = restriction_index < 2;
                for reply in case.replies.clone() {
                    browser.push(Ok(reply));
                }
                let before = browser.calls().len();
                let mut arguments = if composed {
                    json!({"steps":[{"id":"PRIVATE_STEP_LABEL","tool":case.tool,"arguments":case.arguments}]})
                } else {
                    case.arguments
                };
                if let Some(restriction) = &restriction {
                    arguments["restrict_capabilities"] = json!(restriction);
                }
                let result = executor.execute(
                    &workspace,
                    if composed { "browser_flow" } else { case.tool },
                    arguments,
                    None,
                    &CancellationToken::default(),
                );
                let label = format!(
                    "{} {:?} restriction={restriction:?} composed={composed}",
                    case.tool, case.variant
                );
                if result.status
                    != if permitted {
                        Status::Succeeded
                    } else {
                        Status::Blocked
                    }
                {
                    failures.push(format!("{label}: {result:?}"));
                    continue;
                }
                let commands = browser.calls();
                if permitted {
                    assert_eq!(
                        commands.len() - before,
                        case.replies.len(),
                        "{label}: all physical work is acknowledged exactly once"
                    );
                } else {
                    assert_eq!(result.effect, Effect::None, "{label}");
                    assert!(commands[before..].is_empty(), "{label}: denied work must not extract, edit, stop capture, or export: {:?}", &commands[before..]);
                    if case.tool == "browser_sequence" {
                        assert!(
                            audit
                                .0
                                .lock()
                                .unwrap()
                                .iter()
                                .any(|record| record.step.is_some()
                                    && record.summary.contains("restrict_capabilities")),
                            "{label}: retain the child's actual restriction"
                        );
                    } else {
                        let refused = if composed || case.tool == "browser_flow" {
                            &result.facts["steps"][0]["result"]["facts"]
                        } else {
                            &result.facts
                        };
                        assert_eq!(
                            refused["restriction"], "restrict_capabilities",
                            "{label}: name the caller's restriction"
                        );
                        assert_eq!(
                            refused["required_capabilities"].as_array().unwrap().iter().map(|value| value.as_str().unwrap()).collect::<BTreeSet<_>>(),
                            case.required.iter().copied().collect::<BTreeSet<_>>(),
                            "{label}: report the complete requirements, including the missing capability"
                        );
                    }
                }
                assert!(
                    !serde_json::to_string(&*audit.0.lock().unwrap())
                        .unwrap()
                        .contains("PRIVATE_"),
                    "{label}: browser payloads and caller labels cannot enter audit"
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} authority regressions:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn independent_configured_grants_admit_every_catalog_variant_and_its_flow_child() {
    let templates = cases(&json!("target_1"), &json!("target_2"), &json!("tab_1"));
    let mut failures = Vec::new();
    for (index, template) in templates.iter().enumerate() {
        for composed in [false, true] {
            if composed && matches!(template.tool, "browser_flow" | "browser_sequence") {
                continue;
            }
            let (mut executor, browser, _, workspace, audit) = fixture();
            let (tab_handle, target, button) = established(&executor, &browser, &workspace);
            let case = cases(&target, &button, &tab_handle).swap_remove(index);
            let policy = temporary_policy("catalog-independent-authority");
            let grants = if case.required.is_empty() {
                json!([])
            } else {
                json!([{"id":"required","hosts":{"allow":["*"]},"allowed":case.required}])
            };
            fs::write(
                &policy,
                serde_json::to_vec(&json!({
                    "schema":3,"name":"Exact ordinary permission","version":"1",
                    "grants":grants
                }))
                .unwrap(),
            )
            .unwrap();
            // Establish ordinary handles, then authorize the requested work with only its
            // independent grants. Discovery in an earlier call does not grant this call Read.
            executor.governance = GovernanceFacade::new(Some(policy.clone()), None);
            for reply in case.replies.clone() {
                browser.push(Ok(reply));
            }
            let before = browser.calls().len();
            let arguments = if composed {
                json!({"steps":[{"id":"PRIVATE_STEP_LABEL","tool":case.tool,"arguments":case.arguments}]})
            } else {
                case.arguments
            };
            let result = executor.execute(
                &workspace,
                if composed { "browser_flow" } else { case.tool },
                arguments,
                None,
                &CancellationToken::default(),
            );
            fs::remove_file(policy).unwrap();
            let label = format!(
                "{} {:?} with {:?} composed={composed}",
                case.tool, case.variant, case.required
            );
            if result.status != Status::Succeeded {
                failures.push(format!("{label}: {result:?}"));
                continue;
            }
            assert_eq!(
                browser.calls().len() - before,
                case.replies.len(),
                "{label}"
            );
            let records = audit.0.lock().unwrap();
            let receipts = records
                .iter()
                .filter(|record| record.invocation == result.invocation);
            assert!(receipts.clone().all(|record| record.allowed), "{label}");
            assert!(
                receipts
                    .flat_map(|record| &record.permissions.checks)
                    .any(|check| check.allowed
                        && check
                            .layers
                            .iter()
                            .any(|layer| layer.grants.iter().any(|grant| grant == "required")))
                    || case.required.is_empty(),
                "{label}: retain actual positive grant evidence"
            );
            assert!(
                !serde_json::to_string(&*records)
                    .unwrap()
                    .contains("PRIVATE_"),
                "{label}"
            );
        }
    }
    assert!(
        failures.is_empty(),
        "{} configured authority regressions:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn observation_variants_reject_missing_configured_capabilities_before_browser_effects() {
    let templates = cases(&json!("target_1"), &json!("target_2"), &json!("tab_1"));
    for (index, template) in templates
        .iter()
        .enumerate()
        .filter(|(_, case)| matches!(case.variant, Some("selector" | "expect")))
    {
        for missing in template.required {
            for composed in [false, true] {
                let (mut executor, browser, _, workspace, audit) = fixture();
                let (tab_handle, target, button) = established(&executor, &browser, &workspace);
                let case = cases(&target, &button, &tab_handle).swap_remove(index);
                let permitted: Vec<_> = case
                    .required
                    .iter()
                    .copied()
                    .filter(|capability| capability != missing)
                    .collect();
                let policy = temporary_policy("observation-missing-authority");
                fs::write(
                    &policy,
                    serde_json::to_vec(&json!({
                        "schema":3,"name":"Incomplete named work","version":"1",
                        "grants":[{"id":"partial","hosts":{"allow":["*"]},"allowed":permitted}]
                    }))
                    .unwrap(),
                )
                .unwrap();
                executor.governance = GovernanceFacade::new(Some(policy.clone()), None);
                for reply in case.replies {
                    browser.push(Ok(reply));
                }
                let before = browser.calls().len();
                let invocation = if composed {
                    json!({"steps":[{"id":"PRIVATE_STEP_LABEL","tool":case.tool,"arguments":case.arguments}]})
                } else {
                    case.arguments
                };
                let result = executor.execute(
                    &workspace,
                    if composed { "browser_flow" } else { case.tool },
                    invocation,
                    None,
                    &CancellationToken::default(),
                );
                fs::remove_file(policy).unwrap();
                let label = format!(
                    "{} {:?}, missing {missing}, composed={composed}",
                    case.tool, case.variant
                );
                assert_eq!(result.status, Status::Blocked, "{label}: {result:?}");
                assert_eq!(result.effect, Effect::None, "{label}");
                assert_eq!(
                    browser.calls().len(),
                    before,
                    "{label}: admit the whole request before lookup or effect"
                );
                let facts = if composed {
                    &result.facts["steps"][0]["result"]["facts"]
                } else {
                    &result.facts
                };
                assert!(
                    facts.get("restriction").is_none(),
                    "{label}: configured grant failure is not a caller restriction"
                );
                let records = audit.0.lock().unwrap();
                assert!(
                    records
                        .iter()
                        .any(|record| record.invocation == result.invocation && !record.allowed),
                    "{label}: retain denied authority evidence"
                );
                assert!(
                    !serde_json::to_string(&*records)
                        .unwrap()
                        .contains("PRIVATE_"),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn recording_attachment_checks_read_sources_and_write_destination_independently() {
    for (source_read, destination_write) in [(true, true), (false, true), (true, false)] {
        for composed in [false, true] {
            let (mut executor, browser, _, workspace, audit) = fixture();
            let (tab_handle, target, button) = established(&executor, &browser, &workspace);
            let policy = temporary_policy("recording-distinct-source-and-target");
            fs::write(&policy, serde_json::to_vec(&json!({
                "schema":3,"name":"Separate source and destination","version":"1","grants":[
                    {"id":"source","hosts":{"allow":["source.example"]},"allowed":[if source_read { "read" } else { "write" }]},
                    {"id":"destination","hosts":{"allow":["example.com"]},"allowed":[if destination_write { "write" } else { "read" }]},
                    {"id":"elsewhere","hosts":{"allow":["unrelated.example"]},"allowed":["read","write"]}
                ]
            })).unwrap()).unwrap();
            executor.governance = GovernanceFacade::new(Some(policy.clone()), None);
            let mut case = cases(&target, &button, &tab_handle)
                .into_iter()
                .find(|case| case.variant == Some("save_target"))
                .unwrap();
            for reply in &mut case.replies {
                match reply {
                    BrowserOutcome::RecordingStopped { summary, .. }
                    | BrowserOutcome::RecordingExported { summary, .. } => {
                        summary.source_urls =
                            vec!["https://source.example/PRIVATE_SOURCE_PATH".into()];
                    }
                    _ => {}
                }
            }
            for reply in case.replies {
                browser.push(Ok(reply));
            }
            let before = browser.calls().len();
            let mut arguments = if composed {
                json!({"steps":[{"id":"attach","tool":"browser_record","arguments":case.arguments}]})
            } else {
                case.arguments
            };
            arguments["restrict_capabilities"] = json!(["read", "write"]);
            let result = executor.execute(
                &workspace,
                if composed {
                    "browser_flow"
                } else {
                    "browser_record"
                },
                arguments,
                None,
                &CancellationToken::default(),
            );
            fs::remove_file(policy).unwrap();
            let permitted = source_read && destination_write;
            assert_eq!(result.status, if permitted { Status::Succeeded } else { Status::Blocked }, "source_read={source_read} destination_write={destination_write} composed={composed}: {result:?}");
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
                    "no target preflight or export after a source/destination refusal"
                );
                assert!(matches!(
                    commands.last().unwrap(),
                    BrowserCommand::StopRecording { .. }
                ));
                assert_eq!(result.effect, Effect::None);
            }
            assert!(!serde_json::to_string(&*audit.0.lock().unwrap())
                .unwrap()
                .contains("PRIVATE_"));
        }
    }
}
