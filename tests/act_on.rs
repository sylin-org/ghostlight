// SPDX-License-Identifier: Apache-2.0 OR MIT
//! In-process journey tests for the additive `act_on` tool (ADR-0078 D3).

mod support;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use support::inproc::{by_id, manifest_from_value, Harness};

fn text_result(text: impl Into<String>) -> Value {
    json!({"content":[{"type":"text","text":text.into()}]})
}

fn temp_audit_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-act-on-{}-{tag}.jsonl",
        std::process::id()
    ))
}

fn manifest(audit_path: &Path) -> Value {
    json!({
        "schema": 3,
        "name": "act-on-test",
        "version": "1",
        "grants": [{
            "id": "full",
            "hosts": {"allow": ["example.com"]},
            "allowed": ["read", "action", "write"]
        }],
        "config": [
            {"key":"audit.enabled","value":true,"level":"mandatory"},
            {"key":"audit.destination","value":"file","level":"mandatory"},
            {"key":"audit.file.path","value":audit_path.to_string_lossy(),"level":"mandatory"}
        ]
    })
}

fn call(arguments: Value) -> [Value; 2] {
    [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"act_on","arguments":arguments
        }}),
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unique_semantic_target_acts_waits_and_audits_only_content_free_outcome() {
    let audit_path = temp_audit_path("unique");
    let _ = std::fs::remove_file(&audit_path);
    let harness = Harness::governed(manifest_from_value(&manifest(&audit_path)));
    let tools = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&tools);
    harness
        .attach_fake_extension(move |request| {
            if request["type"] == "tab_url_request" {
                return json!({"url":"https://example.com/edit"});
            }
            if request.get("tool").is_none() {
                return json!({});
            }
            let tool = request["tool"].as_str().expect("tool request");
            seen.lock().unwrap().push(tool.to_string());
            match tool {
                "resolve_actionable_internal" => text_result(json!({
                    "target": {
                        "ref":"ref_7", "role":"button", "name":"Save private draft",
                        "visible":true, "enabled":true,
                        "box":{"x":10,"y":20,"width":30,"height":40},
                        "renderSerial":4, "mechanicalActions":["left_click"], "x":25, "y":40
                    },
                    "candidates":[], "ambiguous":false,
                    "page":{"url":"https://example.com/edit","origin":"https://example.com","title":"Edit","renderSerial":4}
                }).to_string()),
                "target_cue_internal" => text_result("Target cue shown."),
                "computer" => json!({
                    "content":[{"type":"text","text":"interaction receipt: observed after left_click: DOM activity."}],
                    "structuredContent":{"interactionReceipt":{
                        "targetAssurance":"ref", "action":"left_click",
                        "observedAfter":{"mutations":3,"renderAdvanced":true},
                        "blockers":[],
                        "page":{"tabId":1,"url":"https://example.com/edit","origin":"https://example.com","title":"Edit","renderSerial":5},
                        "more":false
                    }}
                }),
                "wait_for" => json!({
                    "content":[{"type":"text","text":"Condition observed."}],
                    "structuredContent":{"found":true,"settled":true,"elapsed_ms":500}
                }),
                other => panic!("unexpected internal tool: {other}"),
            }
        })
        .await;

    let responses = harness
        .drive(&call(json!({
            "tabId":1,
            "target":{"name":"Save private draft","role":"button"},
            "action":"left_click",
            "expect":{"text":"Secret success text","state":"visible","timeout_ms":5000}
        })))
        .await;
    let result = &by_id(&responses, 2)["result"];
    assert_eq!(
        result.pointer("/structuredContent/interactionReceipt/observedAfter/expectMet"),
        Some(&json!(true))
    );
    assert!(result.get("_batch_id").is_none());
    assert_eq!(
        tools.lock().unwrap().as_slice(),
        [
            "resolve_actionable_internal",
            "target_cue_internal",
            "computer",
            "wait_for"
        ]
    );

    let audit_text = std::fs::read_to_string(&audit_path).expect("audit file");
    let records: Vec<Value> = audit_text
        .lines()
        .map(|line| serde_json::from_str(line).expect("audit JSON"))
        .collect();
    let parents: Vec<&Value> = records
        .iter()
        .filter(|record| record["tool"] == "act_on")
        .collect();
    assert_eq!(parents.len(), 1, "one parent decision: {records:?}");
    assert_eq!(parents[0]["target_assurance"], "semantic");
    assert_eq!(parents[0]["outcome"], "expect_met");
    assert_eq!(parents[0]["action"], "left_click");
    assert!(records
        .iter()
        .filter(|record| record.get("orchestrator") == Some(&json!("act_on")))
        .all(|record| record["batch_id"] == parents[0]["batch_id"]));
    for forbidden in [
        "Save private draft",
        "Secret success text",
        "ref_7",
        "mechanicalActions",
        "renderSerial",
        "sessionNonce",
    ] {
        assert!(!audit_text.contains(forbidden), "audit leaked {forbidden}");
    }
    std::fs::remove_file(&audit_path).ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn best_rank_tie_returns_candidates_without_cue_or_action() {
    let harness = Harness::all_open();
    let tools = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&tools);
    harness
        .attach_fake_extension(move |request| {
            if request["type"] == "tab_url_request" {
                return json!({"url":"https://example.com/"});
            }
            if request.get("tool").is_none() {
                return json!({});
            }
            let tool = request["tool"].as_str().expect("tool request");
            seen.lock().unwrap().push(tool.to_string());
            assert_eq!(tool, "resolve_actionable_internal");
            text_result(json!({
                "target":null, "ambiguous":true, "more":false,
                "candidates":[
                    {"ref":"ref_1","role":"button","name":"Save","visible":true,"enabled":true,"box":{"x":0,"y":0,"width":10,"height":10},"renderSerial":1,"mechanicalActions":["left_click"]},
                    {"ref":"ref_2","role":"button","name":"Save","visible":true,"enabled":true,"box":{"x":20,"y":0,"width":10,"height":10},"renderSerial":1,"mechanicalActions":["left_click"]}
                ],
                "page":{"url":"https://example.com/","origin":"https://example.com","title":"Two saves","renderSerial":1}
            }).to_string())
        })
        .await;

    let responses = harness
        .drive(&call(json!({
            "tabId":1,"target":{"name":"Save","role":"button"},"action":"left_click"
        })))
        .await;
    let result = &by_id(&responses, 2)["result"];
    assert_eq!(result["isError"], true);
    assert_eq!(
        result.pointer("/structuredContent/interactionReceipt/blockers/0/kind"),
        Some(&json!("ambiguous_target"))
    );
    assert_eq!(
        result
            .pointer("/structuredContent/candidates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        tools.lock().unwrap().as_slice(),
        ["resolve_actionable_internal"]
    );
}

#[test]
fn closed_loop_journey_reduces_calls_and_keeps_next_decision_facts() {
    let low_level_inputs = [
        json!({"name":"find","arguments":{"tabId":1,"query":"Save"}}),
        json!({"name":"computer","arguments":{"tabId":1,"ref":"ref_7","action":"left_click"}}),
        json!({"name":"wait_for","arguments":{"tabId":1,"text":"Saved","state":"visible"}}),
    ];
    let closed_loop_input = json!({
        "name":"act_on","arguments":{
            "tabId":1,"target":{"name":"Save"},"action":"left_click",
            "expect":{"text":"Saved","state":"visible"}
        }
    });
    let receipt = json!({
        "targetAssurance":"semantic", "action":"left_click",
        "observedAfter":{"expectMet":true}, "blockers":[],
        "page":{"tabId":1,"url":"https://example.com/done","origin":"https://example.com","title":"Done","renderSerial":8},
        "provenance":{"pageSourced":true,"untrusted":true,"topOrigin":"https://example.com","sessionNonce":"000000000000000000000000"},
        "more":false
    });
    let low_level_input_bytes: usize = low_level_inputs
        .iter()
        .map(|value| serde_json::to_vec(value).unwrap().len())
        .sum();
    let closed_loop_input_bytes = serde_json::to_vec(&closed_loop_input).unwrap().len();
    let output_bytes = serde_json::to_vec(&receipt).unwrap().len();
    let closed_loop_calls = 1;

    assert_eq!(low_level_inputs.len(), 3);
    assert_eq!(closed_loop_calls, 1);
    assert!(closed_loop_input_bytes < low_level_input_bytes);
    assert!(output_bytes > 0);
    for fact in [
        "targetAssurance",
        "observedAfter",
        "blockers",
        "page",
        "provenance",
    ] {
        assert!(
            receipt.get(fact).is_some(),
            "missing next-decision fact {fact}"
        );
    }
}
