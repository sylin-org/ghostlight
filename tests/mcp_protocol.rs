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
use support::inproc::{
    by_id, init_and_call, manifest_from_value, operation, operation_call, Harness,
};

#[tokio::test]
async fn canonical_catalog_projection_preserves_the_full_ordered_surface() {
    let responses = Harness::all_open()
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"operations/list","params":{}}),
        ])
        .await;

    let list = by_id(&responses, 2);
    let operations = list["result"]["operations"]
        .as_array()
        .expect("operations array");
    assert_eq!(
        operations.len(),
        ghostlight::operation::registry::descriptors().len()
    );
    assert_eq!(operations[0]["id"], "browser.tabs");
    assert_eq!(operations[0]["intent"], "tabs.list");
    assert_eq!(
        operations.last().expect("last operation")["id"],
        "browser.context"
    );
    assert_eq!(
        operations.last().expect("last operation")["intent"],
        "context.describe"
    );
}

#[tokio::test]
async fn context_description_is_projected_last_and_runs_without_a_browser() {
    let responses = Harness::all_open()
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"operations/list","params":{}}),
            operation_call(
                3,
                operation(
                    OperationId::BrowserContext,
                    IntentId::ContextDescribe,
                    json!({}),
                ),
            ),
        ])
        .await;

    let list = by_id(&responses, 2);
    let operations = list["result"]["operations"]
        .as_array()
        .expect("operations array");
    assert_eq!(
        operations.last().and_then(|entry| entry["intent"].as_str()),
        Some("context.describe")
    );

    let call = by_id(&responses, 3);
    assert_ne!(call["result"]["isError"], true);
    let context = &call["result"]["structuredContent"];
    assert_eq!(context["schema"], "ghostlight.browser.context/v1");
    assert_eq!(context["capabilities"][0]["id"], "read");
    assert_eq!(context["operations"][0]["id"], "browser.tabs");
    assert_eq!(context["operations"][0]["intent"], "tabs.list");
    assert_eq!(
        context["operations"]
            .as_array()
            .and_then(|entries| entries.last())
            .and_then(|entry| entry["intent"].as_str()),
        Some("context.describe")
    );
    assert!(
        call["result"].get("content").is_none(),
        "legacy explain prose belongs to the edge profile"
    );
}

async fn context_data_under_manifest(manifest: Option<&Value>) -> Value {
    let harness = match manifest {
        Some(value) => Harness::governed(manifest_from_value(value)),
        None => Harness::all_open(),
    };
    let responses = harness
        .drive(&init_and_call(operation(
            OperationId::BrowserContext,
            IntentId::ContextDescribe,
            json!({}),
        )))
        .await;
    by_id(&responses, 2)["result"]["structuredContent"].clone()
}

/// Canonical context is a service-local product description, not an authority projection. Its
/// output remains the same under every policy posture even though an edge may filter declarations.
#[tokio::test]
async fn context_output_is_identical_across_manifest_postures() {
    let open = context_data_under_manifest(None).await;
    let empty_grants = context_data_under_manifest(Some(&json!({
        "schema": 3, "name": "empty", "version": "1", "grants": []
    })))
    .await;
    let read_only = context_data_under_manifest(Some(&json!({
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
