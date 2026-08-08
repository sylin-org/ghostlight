// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Bridge-major-2 integration guards for canonical browser operations.

use ghostlight_transport::bridge::{
    BridgeSequence, CatalogProjection, EdgeMessage, OperationAvailability, RequestContext,
    ServiceMessage, TerminalOutcome, WorkId, WorkspaceUse, BRIDGE_MAJOR,
};
use ghostlight_transport::operation::{
    BrowserOperation, BrowserResult, BrowserResultStatus, IntentId, InvocationPresentation,
    OperationEffect, OperationId, ResultPart,
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
    let operation = BrowserOperation::new(
        OperationId::BrowserFlow,
        IntentId::FlowExecute,
        json!({
            "tab": 7,
            "steps": [
                BrowserOperation::new(
                    OperationId::BrowserFind,
                    IntentId::FindQuery,
                    json!({"tab":7,"query":"Save"}),
                ),
                BrowserOperation::new(
                    OperationId::BrowserAct,
                    IntentId::ActClick,
                    json!({"tab":7,"target":{"ref":"$prev.results.0.ref"}}),
                ),
            ]
        }),
    );
    assert_eq!(operation.id, OperationId::BrowserFlow);
    assert_eq!(operation.intent, IntentId::FlowExecute);
    assert_no_nested_surface_identity(&operation.arguments);

    let message = EdgeMessage::Start {
        sequence: BridgeSequence(4),
        operation,
        presentation: Some(
            InvocationPresentation::new("ghostlight-legacy", 1, "script", None)
                .expect("bounded presentation"),
        ),
        workspace: None,
        context: RequestContext::default(),
    };
    let wire = serde_json::to_value(message).expect("start serializes");
    assert_eq!(wire["operation"]["id"], "browser.flow");
    assert_eq!(
        wire["operation"]["arguments"]["steps"][0]["id"],
        "browser.find"
    );
    assert_eq!(
        wire["operation"]["arguments"]["steps"][1]["id"],
        "browser.act"
    );
    assert_eq!(wire["presentation"]["externalTool"], "script");
}

#[test]
fn catalog_and_success_wire_are_protocol_neutral_and_declaration_free() {
    assert_eq!(BRIDGE_MAJOR, 2);
    let projection = CatalogProjection {
        generation: 9,
        operations: vec![OperationAvailability {
            id: OperationId::BrowserSnapshot,
            intent: IntentId::SnapshotCapture,
            workspace_use: WorkspaceUse::Uses,
        }],
        restricted: false,
    };
    let catalog = serde_json::to_value(projection).expect("projection serializes");
    assert_eq!(catalog["operations"][0]["id"], "browser.snapshot");
    let catalog_wire = catalog.to_string();
    for forbidden in ["description", "inputSchema", "annotations", "instructions"] {
        assert!(!catalog_wire.contains(forbidden));
    }

    let mut result = BrowserResult::new(
        OperationId::BrowserSnapshot,
        IntentId::SnapshotCapture,
        BrowserResultStatus::Ok,
        OperationEffect::None,
    );
    result.parts.push(ResultPart::Text {
        text: "snapshot ready".into(),
    });
    result.data = json!({"revision": 3});
    let completed = ServiceMessage::Completed {
        work_id: WorkId(11),
        outcome: TerminalOutcome::Success {
            result: Box::new(result),
        },
    };
    let wire = serde_json::to_value(&completed).expect("completion serializes");
    assert_eq!(
        serde_json::from_value::<ServiceMessage>(wire.clone()).expect("completion round trips"),
        completed
    );
    assert_eq!(wire["outcome"]["result"]["operation"], "browser.snapshot");
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
