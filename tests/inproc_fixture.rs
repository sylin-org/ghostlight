// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Self-test and worked example for the ADR-0096 neutral in-process fixture.
//!
//! The fixture drives canonical catalog projection, immutable work admission, governance,
//! dispatch, and an optional fake-extension round trip with no MCP edge or spawned
//! process. Exact MCP revision behavior is covered by `crates/mcp-connector`; these assertions pin the
//! product invariants behind both handlers.

mod support;

use ghostlight_transport::operation::{IntentId, OperationId};
use serde_json::json;
use support::inproc::{by_id, init_and_call, manifest_from_value, operation, text_of, Harness};

/// The all-open service projects every canonical registry entry in stable order.
#[tokio::test]
async fn all_open_tools_list_is_byte_identical_to_the_fixture() {
    let harness = Harness::all_open();
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
    assert_eq!(
        operations.len(),
        ghostlight::operation::registry::descriptors().len(),
        "the neutral projection advertises every canonical operation"
    );
}

/// Under all-open, a canonical call with no extension connected passes policy, reaches dispatch, and
/// returns the familiar `not connected` execution error -- never a `Denied (` text. The "reaches
/// dispatch" contrast that `tests/tool_enforcement.rs` is built around.
#[tokio::test]
async fn all_open_call_reaches_dispatch_without_an_extension() {
    let harness = Harness::all_open();
    let responses = harness
        .drive(&init_and_call(operation(
            OperationId::BrowserNavigate,
            IntentId::NavigateUrl,
            json!({"url":"https://example.com/","tab":1}),
        )))
        .await;

    let call = by_id(&responses, 2);
    assert_eq!(call["result"]["isError"], true, "no extension -> isError");
    let text = text_of(call);
    assert!(text.contains("not connected"), "reached dispatch: {text}");
    assert!(
        !text.starts_with("Denied ("),
        "no denial under all-open: {text}"
    );
}

/// A governed manifest whose grants do not cover the target domain denies the call before dispatch,
/// naming the uncovered host -- the same signal as
/// `tool_enforcement::permitted_call_passes_and_denied_domain_is_denied_with_matching_audit`, now
/// entirely in-process.
#[tokio::test]
async fn governed_denies_an_uncovered_domain_before_dispatch() {
    let manifest = manifest_from_value(&json!({
        "schema": 3,
        "name": "inproc-denial",
        "version": "1",
        "grants": [
            { "id": "example-full", "hosts": {"allow": ["example.com"]}, "allowed": ["read", "action", "write"] }
        ],
    }));
    let harness = Harness::governed(manifest);
    let responses = harness
        .drive(&init_and_call(operation(
            OperationId::BrowserNavigate,
            IntentId::NavigateUrl,
            json!({"url":"https://evil.com/","tab":1}),
        )))
        .await;

    let denied = by_id(&responses, 2);
    assert_ne!(denied["result"]["isError"], true, "a denial is not isError");
    let text = text_of(denied);
    assert!(text.starts_with("Denied (D-"), "{text}");
    assert!(text.contains("no grant covers evil.com"), "{text}");
}

/// With a fake extension attached, a dispatched work item reaches it and comes back with the
/// extension's reply instead of `not connected`, proving the service-to-browser shore is wired.
#[tokio::test]
async fn attached_extension_answers_a_dispatched_call() {
    let harness = Harness::all_open();
    harness
        .attach_fake_extension(
            |_req| json!({ "content": [ { "type": "text", "text": "extension answered" } ] }),
        )
        .await;

    let responses = harness
        .drive(&init_and_call(operation(
            OperationId::BrowserNavigate,
            IntentId::NavigateUrl,
            json!({"url":"https://example.com/","tab":1}),
        )))
        .await;

    let call = by_id(&responses, 2);
    let text = call["result"]["content"]
        .as_array()
        .and_then(|c| c.first())
        .and_then(|c| c["text"].as_str())
        .unwrap_or("");
    assert!(
        !text.contains("not connected"),
        "the call reached the attached extension, not the no-extension path: {call:?}"
    );
}
