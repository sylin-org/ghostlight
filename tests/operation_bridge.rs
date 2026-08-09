// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Bridge-major-3 integration guards for typed canonical browser operations.

use ghostlight_transport::bridge::{
    BridgeSequence, CatalogProjection, EdgeMessage, OperationAvailability, RequestContext,
    ServiceMessage, TerminalOutcome, WorkId, WorkspaceUse, BRIDGE_MAJOR,
};
use ghostlight_transport::operation::{
    BrowserResult, BrowserResultStatus, ClickArguments, ClickButton, InspectPageArguments,
    Operation, OperationEffect, OperationKind, OperationResult, OperationTarget, ResultPart,
    RunSequenceArguments, TabHandle,
};
use serde_json::{json, Value};

fn assert_no_nested_surface_identity(value: &Value) {
    match value {
        Value::Object(object) => {
            assert!(!object.contains_key("tool"));
            assert!(!object.contains_key("name"));
            for value in object.values() {
                assert_no_nested_surface_identity(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                assert_no_nested_surface_identity(value);
            }
        }
        Value::String(value) => {
            assert!(!matches!(value.as_str(), "computer" | "find" | "act_on"));
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[test]
fn recursive_canonical_flow_crosses_the_bridge_without_surface_identity() {
    let tab = TabHandle::parse("t_12345678").expect("valid tab");
    let operation = Operation::BrowserRunSequence(RunSequenceArguments {
        tab: Some(tab.clone()),
        steps: vec![
            Operation::BrowserInspectPage(InspectPageArguments {
                cursor: None,
                tab: Some(tab.clone()),
                query: Some("Save".into()),
                target: None,
                include: Default::default(),
            }),
            Operation::BrowserClick(ClickArguments {
                target: OperationTarget::parse("Save").expect("valid target"),
                tab: Some(tab),
                button: ClickButton::Left,
                clicks: 1,
                modifiers: Vec::new(),
            }),
        ],
    });
    assert_eq!(operation.kind(), OperationKind::BrowserRunSequence);
    assert_no_nested_surface_identity(
        &serde_json::to_value(&operation).expect("operation serializes"),
    );

    let message = EdgeMessage::Start {
        sequence: BridgeSequence(4),
        operation,
        workspace: None,
        context: RequestContext::default(),
    };
    let wire = serde_json::to_value(message).expect("start serializes");
    assert_eq!(wire["operation"]["operation"], "browser_run_sequence");
    assert_eq!(
        wire["operation"]["arguments"]["steps"][0]["operation"],
        "browser_inspect_page"
    );
    assert_eq!(
        wire["operation"]["arguments"]["steps"][1]["operation"],
        "browser_click"
    );
    assert!(wire.get("presentation").is_none());
}

#[test]
fn catalog_and_success_wire_are_protocol_neutral_and_declaration_free() {
    assert_eq!(BRIDGE_MAJOR, 3);
    let projection = CatalogProjection {
        generation: 9,
        operations: vec![OperationAvailability {
            operation: OperationKind::BrowserInspectPage,
            workspace_use: WorkspaceUse::Uses,
        }],
        restricted: false,
    };
    let catalog = serde_json::to_value(projection).expect("projection serializes");
    assert_eq!(
        catalog["operations"][0]["operation"],
        "browser_inspect_page"
    );
    let catalog_wire = catalog.to_string();
    for forbidden in ["description", "inputSchema", "annotations", "instructions"] {
        assert!(!catalog_wire.contains(forbidden));
    }

    let mut result = BrowserResult::new(
        OperationKind::BrowserInspectPage,
        BrowserResultStatus::Ok,
        OperationEffect::None,
    );
    result.parts.push(ResultPart::Text {
        text: "snapshot ready".into(),
    });
    result.result = Some(OperationResult::BrowserInspectPage {
        targets: Vec::new(),
        more: false,
        cursor: None,
    });
    let completed = ServiceMessage::Completed {
        work_id: WorkId(11),
        outcome: TerminalOutcome {
            result: Box::new(result),
        },
    };
    let wire = serde_json::to_value(&completed).expect("completion serializes");
    assert_eq!(
        serde_json::from_value::<ServiceMessage>(wire.clone()).expect("completion round trips"),
        completed
    );
    assert_eq!(
        wire["outcome"]["result"]["operation"],
        "browser_inspect_page"
    );
    let rendered = wire.to_string();
    for forbidden in [
        "jsonrpc",
        "protocolVersion",
        "structuredContent",
        "mimeType",
    ] {
        assert!(!rendered.contains(forbidden));
    }
}

#[test]
fn cancellation_uses_only_the_stream_local_work_identity() {
    let message = EdgeMessage::Cancel { work_id: WorkId(5) };
    assert_eq!(
        serde_json::to_value(message).expect("cancel serializes"),
        json!({"type":"cancel","work_id":5})
    );
}
