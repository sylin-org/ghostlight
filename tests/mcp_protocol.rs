// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Product-invariant coverage retained from the former service-side MCP protocol suite.
//!
//! ADR-0096 moved framing, lifecycle, revision negotiation, JSON-RPC errors, and exact response
//! envelopes to the date-named handlers in `crates/mcp-connector`. This integration test now covers only
//! the neutral service facts that both handlers consume: one canonical catalog, local tools, and
//! pre-dispatch validation. It deliberately does not recreate a protocol loop in the core fixture.

mod support;

use ghostlight_transport::operation::{BrowserOperation, IntentId, OperationId};
use serde_json::{json, Value};
use support::inproc::{by_id, init_and_call, manifest_from_value, text_of, Harness};

#[tokio::test]
async fn canonical_catalog_projection_preserves_the_full_ordered_surface() {
    let responses = Harness::all_open()
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        ])
        .await;

    let list = by_id(&responses, 2);
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        ghostlight::browser::directory::advertised_tool_count()
    );
    assert_eq!(tools[0]["name"], "tabs_context_mcp");
    assert_eq!(
        list["result"],
        ghostlight::tool::tools::advertised_tools_json(),
        "the neutral projection must retain the canonical declarations byte for byte"
    );
}

#[tokio::test]
async fn explain_is_advertised_last_and_runs_without_a_browser() {
    let responses = Harness::all_open()
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"explain","arguments":{}}}),
        ])
        .await;

    let list = by_id(&responses, 2);
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.last().and_then(|tool| tool["name"].as_str()),
        Some("explain")
    );

    let call = by_id(&responses, 3);
    assert_ne!(call["result"]["isError"], true);
    let text = text_of(call);
    assert!(text.starts_with("Capabilities: read = "), "{text}");
    assert!(text.contains("navigate: requires read."), "{text}");
    assert!(text.contains("explain: requires nothing."), "{text}");
}

async fn explain_text_under_manifest(manifest: Option<&Value>) -> String {
    let harness = match manifest {
        Some(value) => Harness::governed(manifest_from_value(value)),
        None => Harness::all_open(),
    };
    let responses = harness.drive(&init_and_call("explain", json!({}))).await;
    text_of(by_id(&responses, 2)).to_string()
}

/// `explain` is a service-local product guide, not an authority projection. Its output remains the
/// same under every policy posture even though the advertised callable catalog is grant-filtered.
#[tokio::test]
async fn explain_output_is_byte_identical_across_manifest_postures() {
    let open = explain_text_under_manifest(None).await;
    let empty_grants = explain_text_under_manifest(Some(&json!({
        "schema": 3, "name": "empty", "version": "1", "grants": []
    })))
    .await;
    let read_only = explain_text_under_manifest(Some(&json!({
        "schema": 3, "name": "ro", "version": "1",
        "grants": [{"id":"read-only","hosts":{"allow":["example.com"]},"allowed":["read"]}]
    })))
    .await;

    assert_eq!(open, empty_grants);
    assert_eq!(open, read_only);
}

#[tokio::test]
async fn unknown_operation_is_rejected_before_browser_dispatch() {
    let started = std::time::Instant::now();
    let result = Harness::all_open()
        .execute_unscoped_canonical(BrowserOperation::new(
            OperationId::BrowserContext,
            IntentId::ActClick,
            json!({}),
        ))
        .await;
    let elapsed = started.elapsed();

    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"]
        .as_str()
        .expect("canonical rejection text");
    assert!(text.starts_with("[hop: invalid-request]"), "{text}");
    assert!(text.contains("Unknown operation pair"), "{text}");
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "registry misses must not wait for the browser channel: {elapsed:?}"
    );
}
