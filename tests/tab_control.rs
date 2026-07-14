// SPDX-License-Identifier: Apache-2.0 OR MIT
//! In-process contract tests for explicit owned-tab lifecycle control (ADR-0078 D7).

mod support;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use support::inproc::{by_id, manifest_from_value, Harness};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn temp_audit_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-tab-control-{}-{tag}-{}.jsonl",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn manifest(allowed: &[&str], audit_path: Option<&Path>) -> Value {
    let mut value = json!({
        "schema": 3,
        "name": "tab-control-test",
        "version": "1",
        "grants": [{
            "id": "tabs",
            "hosts": {"allow": ["example.com"]},
            "allowed": allowed
        }]
    });
    if let Some(path) = audit_path {
        value["config"] = json!([
            {"key":"audit.enabled","value":true,"level":"mandatory"},
            {"key":"audit.destination","value":"file","level":"mandatory"},
            {"key":"audit.file.path","value":path.to_string_lossy(),"level":"mandatory"}
        ]);
    }
    value
}

fn tab_response(request: &Value) -> Value {
    if request["type"] == "tab_url_request" {
        return json!({"url":"https://example.com/private-tab"});
    }
    if request.get("tool").is_none() {
        return json!({});
    }
    let action = request["args"]["action"].as_str().expect("tab action");
    let observed = match action {
        "focus" => json!({"tabFocused":true}),
        "reload" => json!({"tabReloaded":true}),
        "close" => json!({"tabClosed":true}),
        other => panic!("unexpected tab action: {other}"),
    };
    json!({
        "content":[{"type":"text","text":format!("Tab {action} observed.")}],
        "structuredContent":{"interactionReceipt":{
            "targetAssurance":"none",
            "action":action,
            "observedAfter":observed,
            "blockers":[],
            "page":{"tabId":1},
            "more":false
        }}
    })
}

fn call(id: i64, action: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{
        "name":"tab_control","arguments":{"tabId":1,"action":action}
    }})
}

#[tokio::test]
async fn focus_reload_and_close_return_receipts_and_content_free_audit_categories() {
    let audit_path = temp_audit_path("actions");
    let _ = std::fs::remove_file(&audit_path);
    let harness = Harness::governed(manifest_from_value(&manifest(
        &["read", "action"],
        Some(&audit_path),
    )));
    harness.attach_fake_extension(tab_response).await;
    let responses = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            call(2, "focus"),
            call(3, "reload"),
            call(4, "close"),
        ])
        .await;
    for (id, field) in [(2, "tabFocused"), (3, "tabReloaded"), (4, "tabClosed")] {
        assert_eq!(
            by_id(&responses, id).pointer(&format!(
                "/result/structuredContent/interactionReceipt/observedAfter/{field}"
            )),
            Some(&json!(true))
        );
    }

    let audit = std::fs::read_to_string(&audit_path).expect("audit file");
    let records: Vec<Value> = audit
        .lines()
        .map(|line| serde_json::from_str(line).expect("audit JSON"))
        .filter(|record: &Value| record["tool"] == "tab_control")
        .collect();
    let outcomes: Vec<&str> = records
        .iter()
        .map(|record| record["outcome"].as_str().expect("outcome"))
        .collect();
    assert_eq!(outcomes.len(), 3);
    for expected in ["tab_focused", "tab_reloaded", "tab_closed"] {
        assert!(
            outcomes.contains(&expected),
            "missing {expected}: {outcomes:?}"
        );
    }
    for forbidden in ["private-tab", "Private title", "sessionNonce"] {
        assert!(!audit.contains(forbidden), "audit leaked {forbidden}");
    }
    std::fs::remove_file(&audit_path).ok();
}

#[tokio::test]
async fn focus_needs_no_rawx_but_reload_and_close_require_action() {
    let harness = Harness::governed(manifest_from_value(&manifest(&[], None)));
    let dispatched = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&dispatched);
    harness
        .attach_fake_extension(move |request| {
            if request.get("tool").is_some() {
                seen.lock().unwrap().push(request["args"]["action"].clone());
            }
            tab_response(request)
        })
        .await;
    let responses = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            call(2, "focus"),
            call(3, "reload"),
            call(4, "close"),
        ])
        .await;
    assert_ne!(by_id(&responses, 2)["result"]["isError"], true);
    for id in [3, 4] {
        let denied = by_id(&responses, id)["result"]["content"][0]["text"]
            .as_str()
            .expect("denial text");
        assert!(denied.contains("Denied (D-"), "{denied}");
    }
    assert_eq!(dispatched.lock().unwrap().as_slice(), [json!("focus")]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tab_control_composes_through_script_and_browser_batch() {
    let harness = Harness::governed(manifest_from_value(&manifest(&["action"], None)));
    let dispatched = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&dispatched);
    harness
        .attach_fake_extension(move |request| {
            if request.get("tool").is_some() {
                seen.lock().unwrap().push(request["args"]["action"].clone());
            }
            tab_response(request)
        })
        .await;

    let script = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
                "name":"script","arguments":{"tabId":1,"steps":[
                    {"tool":"tab_control","args":{"action":"focus"}},
                    {"tool":"tab_control","args":{"action":"reload"}}
                ]}
            }}),
        ])
        .await;
    assert_ne!(by_id(&script, 2)["result"]["isError"], true);

    let batch = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":3,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{
                "name":"browser_batch","arguments":{"actions":[
                    {"name":"tab_control","input":{"tabId":1,"action":"focus"}},
                    {"name":"tab_control","input":{"tabId":1,"action":"close"}}
                ]}
            }}),
        ])
        .await;
    assert_ne!(by_id(&batch, 4)["result"]["isError"], true);
    let actions = dispatched.lock().unwrap();
    assert_eq!(
        actions.len(),
        4,
        "both compositions dispatched: {actions:?}"
    );
    assert!(actions.contains(&json!("reload")));
    assert!(actions.contains(&json!("close")));
}
