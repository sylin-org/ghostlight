// SPDX-License-Identifier: Apache-2.0 OR MIT
//! In-process contract tests for explicit JavaScript dialog control (ADR-0078 D7).

mod support;

use ghostlight_transport::operation::{IntentId, OperationId};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use support::inproc::{by_id, manifest_from_value, Harness};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn temp_audit_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-dialog-{}-{tag}-{}.jsonl",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn manifest(allowed: &[&str], audit_path: Option<&Path>) -> Value {
    let mut value = json!({
        "schema": 3,
        "name": "dialog-test",
        "version": "1",
        "grants": [{
            "id": "dialog",
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

fn page_result(text: &str, structured: Value) -> Value {
    json!({
        "content": [{"type":"text","text":text}],
        "structuredContent": structured
    })
}

fn dialog_response(request: &Value) -> Value {
    if request["type"] == "tab_url_request" {
        return json!({"url":"https://example.com/dialogs"});
    }
    let action = request["args"]["action"].as_str().unwrap_or("status");
    if action == "status" {
        page_result(
            "JavaScript prompt dialog is blocking the tab: \"Private question\".",
            json!({
                "open":true,
                "type":"prompt",
                "message":"Private question",
                "page":{"tabId":1,"url":"https://example.com/dialogs","origin":"https://example.com","title":"Dialogs","renderSerial":3}
            }),
        )
    } else {
        page_result(
            &format!("JavaScript dialog {action} dispatched."),
            json!({
                "open":false,
                "resolved":true,
                "action":action,
                "type":"prompt",
                "page":{"tabId":1,"url":"https://example.com/dialogs","origin":"https://example.com","title":"Dialogs","renderSerial":3}
            }),
        )
    }
}

fn call(id: i64, action: &str, text: Option<&str>) -> Value {
    let mut arguments = json!({"tabId":1,"action":action});
    if let Some(text) = text {
        arguments["text"] = json!(text);
    }
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{
        "name":"dialog","arguments":arguments
    }})
}

#[tokio::test]
async fn status_and_each_resolution_action_are_governed_and_content_minimized_in_audit() {
    let audit_path = temp_audit_path("actions");
    let _ = std::fs::remove_file(&audit_path);
    let harness = Harness::governed(manifest_from_value(&manifest(
        &["read", "action"],
        Some(&audit_path),
    )));
    harness.attach_fake_extension(dialog_response).await;

    let responses = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            call(2, "status", None),
            call(3, "accept", None),
            call(4, "dismiss", None),
            call(5, "respond", Some("Private reply")),
        ])
        .await;

    let status = &by_id(&responses, 2)["result"];
    assert_eq!(
        status.pointer("/structuredContent/open"),
        Some(&json!(true))
    );
    assert_eq!(
        status.pointer("/structuredContent/provenance/untrusted"),
        Some(&json!(true))
    );
    assert!(status["content"][0]["text"]
        .as_str()
        .is_some_and(|text| text.contains("GHOSTLIGHT PAGE CONTENT")));
    for id in 3..=5 {
        assert_eq!(
            by_id(&responses, id).pointer("/result/structuredContent/resolved"),
            Some(&json!(true))
        );
    }

    let audit = std::fs::read_to_string(&audit_path).expect("audit file");
    let records: Vec<Value> = audit
        .lines()
        .map(|line| serde_json::from_str(line).expect("audit JSON"))
        .filter(|record: &Value| record["tool"] == json!(OperationId::BrowserDialog))
        .collect();
    let intents: Vec<&str> = records
        .iter()
        .map(|record| record["action"].as_str().expect("dialog intent"))
        .collect();
    assert_eq!(intents.len(), 4);
    for expected in [
        IntentId::DialogStatus,
        IntentId::DialogAccept,
        IntentId::DialogDismiss,
        IntentId::DialogRespond,
    ] {
        assert!(
            intents.contains(&expected.as_str()),
            "missing {expected}: {intents:?}"
        );
    }
    for forbidden in ["Private question", "Private reply", "sessionNonce"] {
        assert!(!audit.contains(forbidden), "audit leaked {forbidden}");
    }
    std::fs::remove_file(&audit_path).ok();
}

#[tokio::test]
async fn read_only_policy_allows_status_but_denies_resolution_before_dispatch() {
    let harness = Harness::governed(manifest_from_value(&manifest(&["read"], None)));
    let dispatched = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&dispatched);
    harness
        .attach_fake_extension(move |request| {
            if request.get("tool").is_some() {
                seen.lock().unwrap().push(request["args"]["action"].clone());
            }
            dialog_response(request)
        })
        .await;

    let responses = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            call(2, "status", None),
            call(3, "accept", None),
        ])
        .await;
    assert_ne!(by_id(&responses, 2)["result"]["isError"], true);
    let denied = by_id(&responses, 3)["result"]["content"][0]["text"]
        .as_str()
        .expect("denial text");
    assert!(denied.contains("Denied (D-"), "{denied}");
    assert_eq!(dispatched.lock().unwrap().as_slice(), [json!("status")]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dialog_composes_through_script_and_browser_batch() {
    let harness = Harness::governed(manifest_from_value(&manifest(&["read", "action"], None)));
    let dispatched = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&dispatched);
    harness
        .attach_fake_extension(move |request| {
            if request.get("tool").is_some() {
                seen.lock().unwrap().push(request["args"]["action"].clone());
            }
            dialog_response(request)
        })
        .await;

    let responses = harness
        .drive(&[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
                "name":"script","arguments":{"tabId":1,"steps":[
                    {"tool":"dialog","args":{"action":"status"}},
                    {"tool":"dialog","args":{"action":"dismiss"}}
                ]}
            }}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
                "name":"browser_batch","arguments":{"actions":[
                    {"name":"dialog","input":{"tabId":1,"action":"status"}},
                    {"name":"dialog","input":{"tabId":1,"action":"accept"}}
                ]}
            }}),
        ])
        .await;
    assert_ne!(by_id(&responses, 2)["result"]["isError"], true);
    assert_ne!(by_id(&responses, 3)["result"]["isError"], true);
    let actions = dispatched.lock().unwrap();
    assert_eq!(
        actions.len(),
        4,
        "both compositions dispatched: {actions:?}"
    );
    assert_eq!(
        actions
            .iter()
            .filter(|action| **action == json!("status"))
            .count(),
        2
    );
    assert!(actions.contains(&json!("dismiss")));
    assert!(actions.contains(&json!("accept")));
}
