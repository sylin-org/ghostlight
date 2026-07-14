// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Integration test for G14 tool advertisement filtering: proves the wiring end to end (a
//! restrictive manifest's grants actually reach `tools/list` through the real server loop), not
//! just the pure filtering logic (`browser::advertise`'s own exhaustive inline unit tests cover
//! that). No extension is ever connected; `tools/list` never touches it.
//!
//! ADR-0051 Phase 4 (P4.2): migrated from the spawn-a-service-plus-adapter pattern onto the
//! in-process `support::inproc::Harness`, which drives the SAME `serve_session` -> tools/list ->
//! `advertise::advertised_tools(grants)` path with no OS process. The filtered name lists below are
//! the pinned oracles this test exists to prove; they stay verbatim.

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
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        ])
        .await;
    let list = by_id(&responses, 2);
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec![
            "tabs_context_mcp",
            "tabs_create_mcp",
            "navigate",
            "computer",
            "find",
            "get_page_text",
            "read_console_messages",
            "read_network_requests",
            "read_page",
            "resize_window",
            "update_plan",
            "narrate",
            "wait_for",
            "script",
            "act_on",
            "dialog",
            "tab_control",
            "browser_batch",
            "gif_creator",
            "explain",
        ]
    );
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
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        ])
        .await;
    let list = by_id(&responses, 2);
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec![
            "tabs_create_mcp",
            "computer",
            "resize_window",
            "update_plan",
            "narrate",
            "script",
            "tab_control",
            "browser_batch",
            "gif_creator",
            "explain",
        ]
    );
}

/// C11 (ADR-0038 Decision 5, PINS.md SS16): the composed guide text -- the exact surface that
/// reaches `initialize.instructions` -- carries the `Cost notes:` paragraph verbatim, and no test
/// under `tests/` pinned the instructions/guide content before this one (grep `instructions`
/// found nothing relevant), so this is the new test the task file names. Pure (never spawned or
/// in-process), unchanged by the P4.2 migration.
#[test]
fn instructions_carry_cost_notes() {
    let text = ghostlight::mcp::tools::agent_guide_text();
    assert!(text.contains("Cost notes:"), "{text}");
    assert!(
        text.contains("get_page_text can return tens of thousands of tokens"),
        "{text}"
    );
}
