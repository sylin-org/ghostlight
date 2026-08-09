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
        .any(|entry| entry["operation"] == "browser_inspect_page"));
    assert!(!operations
        .iter()
        .any(|entry| entry["operation"] == "browser_open_tab"));
    assert!(!operations
        .iter()
        .any(|entry| entry["operation"] == "browser_fill_form"));
    assert!(!operations
        .iter()
        .any(|entry| entry["operation"] == "browser_run_javascript_unsafe"));
    for entry in operations {
        let operation = ghostlight_transport::operation::OperationKind::parse(
            entry["operation"].as_str().expect("canonical operation"),
        )
        .expect("known canonical operation");
        let descriptor = ghostlight::operation::registry::descriptor(operation);
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
        let operation = ghostlight_transport::operation::OperationKind::parse(
            entry["operation"].as_str().expect("canonical operation"),
        )
        .expect("known canonical operation");
        assert!(ghostlight::operation::registry::descriptor(operation)
            .requires
            .is_empty());
    }
}

/// The model guide teaches the shortest high-value path and avoids ritual preflight calls.
#[test]
fn instructions_are_delight_first() {
    let text =
        include_str!("../crates/mcp-connector/src/surface/data/ghostlight-v1-agent-guide.txt");
    assert!(text.contains("Start with the user's job"), "{text}");
    assert!(text.contains("you do not need a status check"), "{text}");
}
