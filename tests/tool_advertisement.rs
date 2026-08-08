// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Integration test for canonical operation-availability filtering: proves a restrictive
//! manifest's grants reach the service projection. Exact model-facing tool filtering belongs to
//! the edge profile tests. No extension is connected; catalog projection never touches it.
//!
//! The ADR-0096 protocol-neutral `support::inproc::Harness` drives the canonical service catalog
//! projection with no OS process.

mod support;

use serde_json::json;
use support::inproc::{by_id, manifest_from_value, Harness};

/// A read-only manifest (`allowed: ["read"]`). Per ADR-0022 Decision 8, a read-only grant
/// advertises every tool with a directory variant that is `requires: []` or a subset of `read`
/// -- everything except `form_input` (requires `write`) and `javascript_tool` (requires
/// `execute`).
#[tokio::test]
async fn read_only_manifest_advertises_everything_except_write_and_execute_tools() {
    let harness = Harness::governed(manifest_from_value(&json!({
        "schema": 3,
        "name": "g14-read-only",
        "version": "1",
        "grants": [
            { "id": "r", "hosts": {"allow": ["example.com"]}, "allowed": ["read"] },
        ],
    })));

    let responses = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"operations/list","params":{}}),
        ])
        .await;
    let list = by_id(&responses, 2);
    let operations = list["result"]["operations"]
        .as_array()
        .expect("operations array");
    assert!(operations
        .iter()
        .any(|entry| entry["intent"] == "snapshot.capture"));
    assert!(operations.iter().any(|entry| entry["intent"] == "tabs.new"));
    assert!(!operations
        .iter()
        .any(|entry| entry["intent"] == "fill.field"));
    assert!(!operations
        .iter()
        .any(|entry| entry["intent"] == "evaluate.javascript"));
    for entry in operations {
        let id = ghostlight_transport::operation::OperationId::parse(
            entry["id"].as_str().expect("operation id"),
        )
        .expect("known operation id");
        let intent = ghostlight_transport::operation::IntentId::parse(
            entry["intent"].as_str().expect("intent id"),
        )
        .expect("known intent id");
        let descriptor = ghostlight::operation::registry::descriptor(
            ghostlight_transport::operation::OperationKey::new(id, intent),
        )
        .expect("projected operation has a descriptor");
        assert!(
            descriptor.requires.is_empty()
                || descriptor
                    .requires
                    .iter()
                    .all(|capability| capability.as_str() == "read")
        );
    }
}

/// An empty `grants` array advertises exactly the requires-empty set (ADR-0022 Decision 5 step
/// 2: those actions need no grant at all), not the full surface and not nothing.
#[tokio::test]
async fn empty_grants_manifest_advertises_exactly_the_requires_empty_set() {
    let harness = Harness::governed(manifest_from_value(&json!({
        "schema": 3,
        "name": "g14-empty-grants",
        "version": "1",
        "grants": [],
    })));

    let responses = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"operations/list","params":{}}),
        ])
        .await;
    let list = by_id(&responses, 2);
    let operations = list["result"]["operations"]
        .as_array()
        .expect("operations array");
    let expected = ghostlight::operation::registry::descriptors()
        .iter()
        .filter(|descriptor| descriptor.requires.is_empty())
        .count();
    assert_eq!(operations.len(), expected);
    for entry in operations {
        let id = ghostlight_transport::operation::OperationId::parse(
            entry["id"].as_str().expect("operation id"),
        )
        .expect("known operation id");
        let intent = ghostlight_transport::operation::IntentId::parse(
            entry["intent"].as_str().expect("intent id"),
        )
        .expect("known intent id");
        assert!(ghostlight::operation::registry::descriptor(
            ghostlight_transport::operation::OperationKey::new(id, intent),
        )
        .expect("projected operation has a descriptor")
        .requires
        .is_empty());
    }
}

/// C11 (ADR-0038 Decision 5, PINS.md SS16): the composed guide text -- the exact surface that
/// reaches `initialize.instructions` -- carries the `Cost notes:` paragraph verbatim, and no test
/// under `tests/` pinned the instructions/guide content before this one (grep `instructions`
/// found nothing relevant), so this is the new test the task file names. Pure (never spawned or
/// in-process), unchanged by the P4.2 migration.
#[test]
fn instructions_carry_cost_notes() {
    let text = include_str!(
        "../crates/mcp-connector/src/surface/data/ghostlight-legacy-v1-agent-guide.txt"
    );
    assert!(text.contains("Cost notes:"), "{text}");
    assert!(
        text.contains("get_page_text can return tens of thousands of tokens"),
        "{text}"
    );
}
