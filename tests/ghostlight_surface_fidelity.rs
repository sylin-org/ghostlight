// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Cross-crate fidelity gate for Ghostlight's sole model-facing tool surface.

use ghostlight::browser::mechanism::operation_plan;
use ghostlight_transport::bridge::{BridgeError, BridgeErrorKind};
use ghostlight_transport::operation::{
    BrowserResult, BrowserResultStatus, CaptureScope, FlowResultData, FlowStepResult,
    FlowStepStatus, FlowTermination, FlowTerminationReason, InspectPageArguments, Operation,
    OperationEffect, OperationKind, OperationResult, Readiness, ReadinessSettlement,
    ReadinessStatus, ResultProblem, ResultProblemCode, ResultTab, RetryDisposition,
    SettlementStatus, SuggestedNextStep, TabHandle, WaitState,
};
use ghostlight_transport::workspace_id::WorkspaceId;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum McpRevision {
    Mcp2025_11_25,
    Mcp2026_07_28,
}

#[path = "../crates/mcp-connector/src/surface/schema.rs"]
mod schema;
#[allow(clippy::single_match, dead_code, unused_imports)]
#[path = "../crates/mcp-connector/src/surface/ghostlight.rs"]
mod surface;

const TOOLS: [&str; 24] = [
    "browser_get_status",
    "browser_open_tab",
    "browser_list_tabs",
    "browser_focus_tab",
    "browser_close_tab",
    "browser_navigate",
    "browser_go_back",
    "browser_go_forward",
    "browser_reload_page",
    "browser_inspect_page",
    "browser_read_page",
    "browser_take_screenshot",
    "browser_click",
    "browser_hover",
    "browser_scroll_to_target",
    "browser_scroll_page",
    "browser_press_key",
    "browser_press_escape",
    "browser_drag",
    "browser_fill_form",
    "browser_wait_for",
    "browser_run_sequence",
    "browser_get_dialog",
    "browser_handle_dialog",
];

#[test]
fn sole_surface_is_exactly_the_canonical_24_tool_catalog() {
    for revision in [McpRevision::Mcp2025_11_25, McpRevision::Mcp2026_07_28] {
        let declarations = surface::declarations(revision)["tools"]
            .as_array()
            .expect("tools array");
        let names = declarations
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<Vec<_>>();
        assert_eq!(names, TOOLS, "{revision:?} catalog drifted");
        for declaration in declarations {
            assert!(declaration.get("pack").is_none());
            assert_eq!(declaration["inputSchema"]["additionalProperties"], false);
            let required = declaration["outputSchema"]["required"]
                .as_array()
                .expect("output required array");
            for field in ["status", "summary", "effect", "repeat"] {
                assert!(required.iter().any(|required| required == field));
            }
            assert!(declaration["outputSchema"]["properties"]
                .get("operation")
                .is_none());
            assert_no_conditional_schema(&declaration["inputSchema"]);
        }
    }
}

#[test]
fn every_tool_has_a_total_canonical_mapping_and_physical_plan() {
    let cases = [
        ("browser_get_status", json!({})),
        ("browser_open_tab", json!({})),
        ("browser_list_tabs", json!({})),
        ("browser_focus_tab", json!({"tab":"t_example_tab"})),
        ("browser_close_tab", json!({"tab":"t_example_tab"})),
        ("browser_navigate", json!({"url":"https://example.com"})),
        ("browser_go_back", json!({"tab":"t_example_tab"})),
        ("browser_go_forward", json!({"tab":"t_example_tab"})),
        ("browser_reload_page", json!({"tab":"t_example_tab"})),
        ("browser_inspect_page", json!({"tab":"t_example_tab"})),
        ("browser_read_page", json!({"tab":"t_example_tab"})),
        ("browser_take_screenshot", json!({"tab":"t_example_tab"})),
        (
            "browser_click",
            json!({"tab":"t_example_tab","target":"Save"}),
        ),
        (
            "browser_hover",
            json!({"tab":"t_example_tab","target":"Help"}),
        ),
        (
            "browser_scroll_to_target",
            json!({"tab":"t_example_tab","target":"Footer"}),
        ),
        (
            "browser_scroll_page",
            json!({"tab":"t_example_tab","direction":"down"}),
        ),
        (
            "browser_press_key",
            json!({"tab":"t_example_tab","target":"Search","key":"Enter"}),
        ),
        ("browser_press_escape", json!({"tab":"t_example_tab"})),
        (
            "browser_drag",
            json!({"tab":"t_example_tab","from":"Card","to":"Done"}),
        ),
        (
            "browser_fill_form",
            json!({"tab":"t_example_tab","fields":[{"field":"Email","value":"a@example.com"}]}),
        ),
        (
            "browser_wait_for",
            json!({"tab":"t_example_tab","condition":"Ready"}),
        ),
        (
            "browser_run_sequence",
            json!({"tab":"t_example_tab","steps":[{"tool":"browser_read_page"},{"tool":"browser_take_screenshot"}]}),
        ),
        ("browser_get_dialog", json!({"tab":"t_example_tab"})),
        (
            "browser_handle_dialog",
            json!({"tab":"t_example_tab","action":"accept"}),
        ),
    ];

    for (tool, arguments) in cases {
        let operation = surface::decode_call(McpRevision::Mcp2025_11_25, tool, arguments)
            .unwrap_or_else(|error| panic!("{tool} did not decode: {error}"));
        let descriptor = ghostlight::operation::registry::descriptor(operation.kind());
        assert_eq!(descriptor.operation, operation.kind());
        let _plan = operation_plan(operation.kind());
    }
}

#[test]
fn ghostlight_results_are_concise_and_do_not_leak_internal_identity() {
    let mut result = BrowserResult::new(
        OperationKind::BrowserClick,
        BrowserResultStatus::Blocked,
        OperationEffect::None,
    );
    result.repeat = RetryDisposition::AfterStateChange;
    result.tab = Some(test_tab());
    result.problem = Some(ResultProblem {
        code: ResultProblemCode::TargetStale,
        message: "The target belongs to an older page revision.".into(),
    });
    result.suggested_next_steps = vec![SuggestedNextStep::Call {
        reason: "Refresh the page targets before acting again.".into(),
        operation: Operation::BrowserInspectPage(InspectPageArguments::default()),
    }];
    let rendered = surface::encode_result(McpRevision::Mcp2025_11_25, result)
        .expect("Ghostlight result renders");
    let structured = &rendered["structuredContent"];
    assert_eq!(structured["status"], "blocked");
    assert_eq!(structured["effect"], "none");
    assert_eq!(structured["repeat"], "check_state_first");
    assert_eq!(structured["problem"]["code"], "target_stale");
    assert_eq!(
        structured["suggested_next_steps"][0]["tool"],
        "browser_inspect_page"
    );
    for internal in [
        "schema",
        "operation",
        "intent",
        "profile",
        "retry",
        "recovery",
    ] {
        assert!(structured.get(internal).is_none(), "leaked {internal}");
    }
}

#[test]
fn ghostlight_readiness_projects_only_model_facing_facts() {
    let mut result = BrowserResult::new(
        OperationKind::BrowserNavigate,
        BrowserResultStatus::Ok,
        OperationEffect::Committed,
    );
    result.tab = Some(test_tab());
    result.result = Some(OperationResult::BrowserNavigate { landed: true });
    result.readiness = Some(Readiness {
        status: ReadinessStatus::Ready,
        condition: None,
        settlement: Some(ReadinessSettlement {
            requested: true,
            status: SettlementStatus::Settled,
        }),
        elapsed_ms: Some(250),
    });
    let rendered = surface::encode_result(McpRevision::Mcp2025_11_25, result)
        .expect("Ghostlight readiness renders");
    assert_eq!(
        rendered["structuredContent"]["readiness"],
        json!({"status":"ready","elapsed_ms":250})
    );
}

#[test]
fn partial_open_tab_keeps_the_created_tab_and_does_not_invite_replay() {
    let mut result = BrowserResult::new(
        OperationKind::BrowserOpenTab,
        BrowserResultStatus::Partial,
        OperationEffect::Committed,
    );
    result.tab = Some(test_tab());
    result.result = Some(OperationResult::BrowserOpenTab {
        created: true,
        navigated: Some(false),
    });

    let rendered = surface::encode_result(McpRevision::Mcp2025_11_25, result)
        .expect("a committed creation with failed navigation remains renderable");
    let structured = &rendered["structuredContent"];
    assert_eq!(structured["status"], "partial");
    assert_eq!(structured["effect"], "committed");
    assert_eq!(structured["repeat"], "do_not_repeat");
    assert_eq!(structured["tab"]["id"], "t_example_tab");
    assert_eq!(
        structured["result"],
        json!({"created":true,"navigated":false})
    );
}

#[test]
fn not_met_is_a_normal_terminal_with_a_structured_problem() {
    let mut result = BrowserResult::new(
        OperationKind::BrowserWaitFor,
        BrowserResultStatus::NotMet,
        OperationEffect::None,
    );
    result.repeat = RetryDisposition::Safe;
    result.tab = Some(test_tab());
    result.result = Some(OperationResult::BrowserWaitFor {
        condition: "Ready".into(),
        state: WaitState::Visible,
        met: false,
        elapsed_ms: 10_000,
    });
    let rendered = surface::encode_result(McpRevision::Mcp2025_11_25, result)
        .expect("Ghostlight result renders");
    assert!(rendered.get("isError").is_none());
    assert_eq!(
        rendered["structuredContent"]["problem"]["code"],
        "condition_not_met"
    );
}

#[test]
fn every_canonical_success_validates_against_both_exact_output_schemas() {
    let target = || json!({"ref":"r_test_target","role":"button","name":"Example"});
    let cases = [
        (
            OperationKind::BrowserGetStatus,
            OperationEffect::None,
            json!({"browser":"connected","authority":{"policy_source":"none","mode":"open"},"operations":[],"packs":[],"limits":{"max_sequence_steps":10,"max_tabs":64,"max_read_chars":50000}}),
        ),
        (
            OperationKind::BrowserOpenTab,
            OperationEffect::Committed,
            json!({"created":true}),
        ),
        (
            OperationKind::BrowserListTabs,
            OperationEffect::None,
            json!({"count":2}),
        ),
        (
            OperationKind::BrowserFocusTab,
            OperationEffect::Committed,
            json!({"focused":true}),
        ),
        (
            OperationKind::BrowserCloseTab,
            OperationEffect::Committed,
            json!({"closed":true}),
        ),
        (
            OperationKind::BrowserNavigate,
            OperationEffect::Committed,
            json!({"landed":true}),
        ),
        (
            OperationKind::BrowserGoBack,
            OperationEffect::Committed,
            json!({"moved":true}),
        ),
        (
            OperationKind::BrowserGoForward,
            OperationEffect::Committed,
            json!({"moved":true}),
        ),
        (
            OperationKind::BrowserReloadPage,
            OperationEffect::Committed,
            json!({"reloaded":true}),
        ),
        (
            OperationKind::BrowserInspectPage,
            OperationEffect::None,
            json!({"targets":[],"more":false}),
        ),
        (
            OperationKind::BrowserReadPage,
            OperationEffect::None,
            json!({"text":"Example","more":false}),
        ),
        (
            OperationKind::BrowserTakeScreenshot,
            OperationEffect::None,
            json!({"frame":"f_test_frame","width":800,"height":600,"scope":"viewport"}),
        ),
        (
            OperationKind::BrowserClick,
            OperationEffect::Committed,
            json!({"target":target(),"clicked":true,"page_changed":false}),
        ),
        (
            OperationKind::BrowserHover,
            OperationEffect::Committed,
            json!({"target":target(),"hovered":true,"page_changed":false}),
        ),
        (
            OperationKind::BrowserScrollToTarget,
            OperationEffect::Committed,
            json!({"target":target(),"visible":true,"moved":true,"page_changed":false}),
        ),
        (
            OperationKind::BrowserScrollPage,
            OperationEffect::Committed,
            json!({"direction":"down","amount":"page","moved":true,"page_changed":false}),
        ),
        (
            OperationKind::BrowserPressKey,
            OperationEffect::Committed,
            json!({"key":"Enter","target":target(),"pressed":true,"page_changed":false}),
        ),
        (
            OperationKind::BrowserPressEscape,
            OperationEffect::Committed,
            json!({"pressed":true,"page_changed":false}),
        ),
        (
            OperationKind::BrowserDrag,
            OperationEffect::Committed,
            json!({"from":target(),"to":target(),"dragged":true,"page_changed":false}),
        ),
        (
            OperationKind::BrowserFillForm,
            OperationEffect::Committed,
            json!({"filled":[{"field":"Email"}],"skipped":[],"submitted":false}),
        ),
        (
            OperationKind::BrowserWaitFor,
            OperationEffect::None,
            json!({"condition":"Ready","state":"visible","met":true,"elapsed_ms":25}),
        ),
        (
            OperationKind::BrowserRunSequence,
            OperationEffect::None,
            json!({"termination":"complete","steps":[
                {"index":0,"tool":"browser_read_page","status":"ok","summary":"Read the page.","effect":"none","repeat":"safe","result":{"text":"Example","more":false}},
                {"index":1,"tool":"browser_take_screenshot","status":"ok","summary":"Captured the page.","effect":"none","repeat":"safe","result":{"frame":"f_test_frame","width":800,"height":600,"scope":"viewport"}}
            ]}),
        ),
        (
            OperationKind::BrowserGetDialog,
            OperationEffect::None,
            json!({"open":false}),
        ),
        (
            OperationKind::BrowserHandleDialog,
            OperationEffect::Committed,
            json!({"action":"accept","resolved":true}),
        ),
    ];

    for revision in [McpRevision::Mcp2025_11_25, McpRevision::Mcp2026_07_28] {
        for (operation, effect, data) in cases.clone() {
            let mut result = BrowserResult::new(operation, BrowserResultStatus::Ok, effect);
            result.result = Some(if operation == OperationKind::BrowserRunSequence {
                let mut read = BrowserResult::new(
                    OperationKind::BrowserReadPage,
                    BrowserResultStatus::Ok,
                    OperationEffect::None,
                );
                read.result = Some(OperationResult::BrowserReadPage {
                    text: "Example".into(),
                    more: false,
                    cursor: None,
                });
                let mut screenshot = BrowserResult::new(
                    OperationKind::BrowserTakeScreenshot,
                    BrowserResultStatus::Ok,
                    OperationEffect::None,
                );
                screenshot.result = Some(OperationResult::BrowserTakeScreenshot {
                    frame: "f_test_frame".into(),
                    width: 800,
                    height: 600,
                    scope: CaptureScope::Viewport,
                    target: None,
                });
                OperationResult::BrowserRunSequence(FlowResultData {
                    steps: vec![
                        FlowStepResult {
                            step: 1,
                            status: FlowStepStatus::Ok,
                            result: read,
                        },
                        FlowStepResult {
                            step: 2,
                            status: FlowStepStatus::Ok,
                            result: screenshot,
                        },
                    ],
                    summary: "Completed 2 of 2 steps.".into(),
                    duration_ms: 1,
                    termination: FlowTermination {
                        reason: FlowTerminationReason::Completed,
                        step: None,
                    },
                })
            } else {
                serde_json::from_value(json!({
                    "operation": operation.as_str(),
                    "result": data
                }))
                .expect("fixture is a typed operation result")
            });
            if operation != OperationKind::BrowserGetStatus {
                result.workspace = Some(WorkspaceId::mint());
            }
            if !matches!(
                operation,
                OperationKind::BrowserGetStatus
                    | OperationKind::BrowserListTabs
                    | OperationKind::BrowserCloseTab
                    | OperationKind::BrowserRunSequence
            ) {
                result.tab = Some(test_tab());
            }
            if operation == OperationKind::BrowserListTabs {
                let mut current = test_tab();
                current.current = true;
                let mut redacted = ResultTab {
                    id: TabHandle::parse("t_redacted_tab").unwrap(),
                    url: None,
                    title: None,
                    current: false,
                    redacted: Some(ghostlight_transport::operation::TabFactRedaction::Policy),
                };
                redacted.current = false;
                result.tabs = vec![current, redacted];
            }
            surface::encode_result(revision, result)
                .unwrap_or_else(|error| panic!("{revision:?} {operation} output failed: {error}"));
        }
    }
}

#[test]
fn every_pre_start_terminal_validates_and_creator_failures_need_no_workspace() {
    let error = BridgeError {
        kind: BridgeErrorKind::Busy,
        message: "Ghostlight is busy; wait for current browser work to finish.".into(),
        next_step: None,
    };
    let workspace = WorkspaceId::mint();

    for revision in [McpRevision::Mcp2025_11_25, McpRevision::Mcp2026_07_28] {
        for tool in TOOLS {
            let operation = OperationKind::parse(tool).unwrap();
            let needs_existing_workspace = !matches!(
                operation,
                OperationKind::BrowserGetStatus
                    | OperationKind::BrowserOpenTab
                    | OperationKind::BrowserNavigate
            );
            let result_workspace = needs_existing_workspace.then_some(&workspace);
            surface::encode_rejection(revision, &error, Some(operation), result_workspace)
                .unwrap_or_else(|encode| {
                    panic!("{revision:?} {tool} rejection failed its output schema: {encode}")
                });
        }
    }
}

fn test_tab() -> ResultTab {
    ResultTab {
        id: TabHandle::parse("t_example_tab").expect("test tab handle"),
        url: None,
        title: None,
        current: false,
        redacted: None,
    }
}

#[test]
fn defaults_are_materialized_and_risky_intent_is_never_guessed() {
    let read = surface::decode_call(
        McpRevision::Mcp2025_11_25,
        "browser_read_page",
        json!({"tab":"t_example_tab"}),
    )
    .expect("read decodes");
    let Operation::BrowserReadPage(arguments) = read else {
        panic!("browser_read_page decoded to the wrong operation")
    };
    assert_eq!(arguments.max_chars, 20_000);

    let navigate = surface::decode_call(
        McpRevision::Mcp2025_11_25,
        "browser_navigate",
        json!({"url":"https://example.com"}),
    )
    .expect("navigate decodes");
    let Operation::BrowserNavigate(arguments) = navigate else {
        panic!("browser_navigate decoded to the wrong operation")
    };
    assert_eq!(arguments.url, "https://example.com");
    assert!(arguments.tab.is_none());

    assert!(surface::decode_call(
        McpRevision::Mcp2025_11_25,
        "browser_handle_dialog",
        json!({"tab":"t_example_tab"}),
    )
    .is_err());
    assert!(
        surface::decode_call(McpRevision::Mcp2025_11_25, "browser_close_tab", json!({}),).is_err()
    );
}

#[test]
fn request_stateless_revision_requires_workspace_only_when_creation_is_impossible() {
    let workspace = WorkspaceId::mint();
    assert!(surface::decode_call(
        McpRevision::Mcp2026_07_28,
        "browser_read_page",
        json!({"tab":"t_example_tab"}),
    )
    .is_err());
    let read = surface::decode_call(
        McpRevision::Mcp2026_07_28,
        "browser_read_page",
        json!({"workspace":workspace.as_str(),"tab":"t_example_tab"}),
    )
    .expect("stateful call with workspace decodes");
    assert_eq!(read.kind(), OperationKind::BrowserReadPage);
    let Operation::BrowserReadPage(arguments) = read else {
        unreachable!()
    };
    assert_eq!(arguments.max_chars, 20_000);

    assert!(
        surface::decode_call(McpRevision::Mcp2026_07_28, "browser_open_tab", json!({}),).is_ok()
    );
    assert!(surface::decode_call(
        McpRevision::Mcp2026_07_28,
        "browser_navigate",
        json!({"url":"https://example.com"}),
    )
    .is_ok());
}

fn assert_no_conditional_schema(value: &Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                assert!(
                    !matches!(
                        key.as_str(),
                        "allOf" | "anyOf" | "oneOf" | "if" | "then" | "else" | "not"
                    ),
                    "model-facing schema contains conditional keyword {key}"
                );
                assert_no_conditional_schema(value);
            }
        }
        Value::Array(values) => values.iter().for_each(assert_no_conditional_schema),
        _ => {}
    }
}
