// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Integration test for the `script` tool (ADR-0035, PINS.md SS7): drives the real pipeline with no
//! extension connected (so the dispatched steps fail at execution) and asserts the compact result's
//! honest per-step status plus the correlated audit records.
//!
//! The test uses the ADR-0096 protocol-neutral `support::inproc::Harness`. Both cases stay
//! governed by a manifest carrying its own `audit.*`
//! config (a broad grant so the navigate step is allowed and reaches dispatch); the harness writes
//! the correlated audit to that temp file exactly as a `--manifest file://` spawn would, so every
//! assertion is verbatim, now with no OS process.

mod support;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use support::inproc::{by_id, manifest_from_value, Harness};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-script-tool-{}-{tag}-{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn manifest_with_audit(name: &str, audit_path: &Path) -> Value {
    json!({
        "schema": 3,
        "name": name,
        "version": "1",
        // A broad grant so the navigate step is ALLOWED (reaches dispatch and fails at "not
        // connected" rather than being denied by policy); audit is enabled so the correlated
        // records land in the file.
        "grants": [
            { "id": "script-test-full", "hosts": {"allow": ["example.com", "*.example.com"]}, "allowed": ["read", "action", "write"] }
        ],
        "config": [
            { "key": "audit.enabled", "value": true, "level": "mandatory" },
            { "key": "audit.destination", "value": "file", "level": "mandatory" },
            { "key": "audit.file.path", "value": audit_path.to_string_lossy(), "level": "mandatory" },
        ],
    })
}

fn read_audit_lines(path: &Path) -> Vec<Value> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content
        .lines()
        .map(|l| serde_json::from_str(l).expect("each audit line is a JSON object"))
        .collect()
}

/// The script tool with two extension-forwarded steps and no extension connected: step 1 (navigate)
/// fails at execution with an extension hop error; step 2 (find) never runs. The compact result
/// reports the honest per-step statuses, and the audit log carries exactly the parent `script`
/// record plus the one step that actually ran (navigate), correlated by batch_id -- NO record for
/// `find` (it was never dispatched).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn script_reports_step_error_and_not_run_with_correlated_audit() {
    let audit_path = temp_path("script-audit");
    let _ = std::fs::remove_file(&audit_path);
    let harness = Harness::governed(manifest_from_value(&manifest_with_audit(
        "script-audit",
        &audit_path,
    )));

    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"script","arguments":{
            "tabId": 0,
            "steps": [
                {"tool":"navigate","args":{"url":"https://example.com"}},
                {"tool":"find","args":{"query":"x"}}
            ]
        }}}),
    ];
    let responses = harness.drive(&requests).await;
    let call = by_id(&responses, 2);
    assert_ne!(
        call["result"]["isError"], true,
        "script itself succeeds: {call:?}"
    );

    // The compact result is the first text content block, parsed back as JSON.
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("compact result text");
    let compact: Value = serde_json::from_str(text).expect("compact result is JSON");
    let results = compact["results"].as_array().expect("results array");
    let status: Vec<&str> = results
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(status, vec!["error", "not_run"], "got: {status:?}");
    let step1_text = results[0]["result"].as_str().unwrap_or("");
    assert!(
        step1_text.contains("extension"),
        "step 1 text should name the extension hop failure: {step1_text}"
    );
    assert_eq!(
        compact["summary"], "0/2 steps completed; step 1 failed",
        "got: {}",
        compact["summary"]
    );

    // Correlated audit: exactly the parent script record + the navigate step record. No find record
    // (find was never dispatched -- onError stop halted the chain at step 1's failure).
    let lines = read_audit_lines(&audit_path);
    assert_eq!(lines.len(), 2, "parent + one step: {lines:?}");

    let parent = lines
        .iter()
        .find(|l| l["tool"] == "script")
        .unwrap_or_else(|| panic!("no script parent record in {lines:?}"));
    assert_eq!(parent["tool"], "script");
    assert!(parent["batch_id"].is_string(), "parent batch_id set");
    assert!(
        parent["orchestrator"].is_null(),
        "parent has no orchestrator"
    );
    assert!(parent["step"].is_null(), "parent has no step number");
    let batch_id = parent["batch_id"].as_str().unwrap();

    let step1 = lines
        .iter()
        .find(|l| l["tool"] == "navigate")
        .unwrap_or_else(|| panic!("no navigate step record in {lines:?}"));
    assert_eq!(step1["tool"], "navigate");
    assert_eq!(step1["orchestrator"], "script");
    assert_eq!(
        step1["batch_id"], batch_id,
        "step shares the parent's batch_id"
    );
    assert_eq!(step1["step"], 1, "step 1 is numbered 1");

    assert!(
        !lines.iter().any(|l| l["tool"] == "find"),
        "no audit record for the not-run find step: {lines:?}"
    );

    std::fs::remove_file(&audit_path).ok();
}

/// A dry run evaluates every step's verdict through the REAL governance decision but dispatches
/// nothing: no extension frame, no step audit records. The audit log carries exactly ONE record --
/// the parent `script` call with `dry_run: true`. `find` (tab-scoped, no extension -> tab URL
/// unknowable) is `would_deny`; navigate to the granted `example.com` is `would_allow` (the real
/// authorize verdict, not a guess).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dry_run_verdicts_without_step_records() {
    let audit_path = temp_path("script-dry-audit");
    let _ = std::fs::remove_file(&audit_path);
    let harness = Harness::governed(manifest_from_value(&manifest_with_audit(
        "script-dry",
        &audit_path,
    )));

    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"script","arguments":{
            "tabId": 0,
            "dry_run": true,
            "steps": [
                {"tool":"find","args":{"query":"x"}},
                {"tool":"navigate","args":{"url":"https://example.com"}}
            ]
        }}}),
    ];
    let responses = harness.drive(&requests).await;
    let call = by_id(&responses, 2);
    assert_ne!(
        call["result"]["isError"], true,
        "dry run succeeds: {call:?}"
    );

    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("compact result text");
    let compact: Value = serde_json::from_str(text).expect("compact result is JSON");
    let status: Vec<&str> = compact["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        status,
        vec!["would_deny", "would_allow"],
        "the real authorize verdict per step: got {status:?}"
    );

    // Exactly one audit record: the parent script call, marked dry_run. No step records (nothing
    // dispatched -- the audit scopes for steps dropped without complete()).
    let lines = read_audit_lines(&audit_path);
    assert_eq!(
        lines.len(),
        1,
        "dry run writes only the parent record: {lines:?}"
    );
    assert_eq!(lines[0]["tool"], "script");
    assert_eq!(lines[0]["dry_run"], true, "parent is marked dry_run");

    std::fs::remove_file(&audit_path).ok();
}
