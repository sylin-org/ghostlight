//! Integration tests for G15 shadow enforcement: the SAME denied call, under two manifests
//! identical except for the top-level `mode`, blocks under `enforce` and runs-but-records
//! under `observe`. No extension is ever connected (mirrors `tests/tool_enforcement.rs`'s own
//! subprocess pattern): under `enforce` the call never dispatches (`Denied (` text,
//! `duration_ms: 0`); under `observe` it DOES dispatch and fails at the ordinary "not
//! connected" execution error after the bounded handshake wait -- a real, non-zero
//! `duration_ms` that proves the tool actually ran, not merely that the response text differs.
//!
//! This file does NOT assert the two runs share a denial id (see the comment at the assertion
//! site): a manifest's own `mode` field is itself hashed into `manifest_hash`, so two manifest
//! FILES that differ only in `mode` are, by the denial-id design (ADR-0020), two different
//! policy versions with two different ids. The same-manifest-hash invariant is proven at the
//! `transport::mcp::server` inline test level instead.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn file_uri(path: &Path) -> String {
    let forward = path.to_string_lossy().replace('\\', "/");
    match forward.strip_prefix('/') {
        Some(rest) => format!("file:///{rest}"),
        None => format!("file:///{forward}"),
    }
}

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "browser-mcp-shadow-mode-{}-{tag}-{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

/// A manifest with one grant covering `example.com` permitting `action`/`write` but not `read`,
/// at manifest-level `mode`, with audit enabled to `audit_path`.
fn manifest_value(mode: &str, audit_path: &Path) -> Value {
    json!({
        "schema": 3,
        "name": "shadow-check",
        "version": "1",
        "mode": mode,
        "grants": [
            {
                "id": "action-write-only",
                "hosts": {"allow": ["example.com"]},
                "allowed": ["action", "write"]
            },
        ],
        "config": [
            { "key": "audit.enabled", "value": true, "level": "mandatory" },
            { "key": "audit.destination", "value": "file", "level": "mandatory" },
            { "key": "audit.file.path", "value": audit_path.to_string_lossy(), "level": "mandatory" },
        ],
    })
}

fn write_manifest(tag: &str, value: &Value) -> PathBuf {
    let path = temp_path(&format!("{tag}-manifest")).with_extension("json");
    std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

fn read_audit_lines(path: &Path) -> Vec<Value> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content
        .lines()
        .map(|l| serde_json::from_str(l).expect("each audit line is a JSON object"))
        .collect()
}

fn drive(manifest_path: &Path, requests: &[Value]) -> Vec<Value> {
    let endpoint = format!(
        "browser-mcp-shadow-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_browser-mcp"))
        .env("BROWSER_MCP_ENDPOINT", &endpoint)
        .arg("--manifest")
        .arg(file_uri(manifest_path))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn browser-mcp");

    let mut stdin = child.stdin.take().expect("stdin");
    for req in requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    drop(stdin);

    let stdout = child.stdout.take().expect("stdout");
    let responses: Vec<Value> = BufReader::new(stdout)
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).expect("each stdout line is JSON"))
        .collect();
    child.wait().expect("wait for child");
    responses
}

fn text_of(resp: &Value) -> &str {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content block in {resp:?}"))
}

/// A call requiring `read` (`tabs_context_mcp`, domain-less, denied via the union rule; the
/// only domain-less tool with a non-empty capability requirement under ADR-0022 --
/// `tabs_create_mcp`/`update_plan`/`resize_window` all require `[]` and short-circuit to Allow
/// unconditionally) under the `action`/`write`-only grant: `capability` denies it under
/// enforce, attributed to `action-write-only`; `manifest_value` above is otherwise
/// byte-identical across both runs.
fn denied_call_requests() -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tabs_context_mcp","arguments":{}}}),
    ]
}

#[test]
fn enforce_blocks_observe_dispatches_and_records_shadow_deny() {
    let enforce_audit = temp_path("enforce-audit");
    let enforce_manifest = write_manifest("enforce", &manifest_value("enforce", &enforce_audit));
    let enforce_responses = drive(&enforce_manifest, &denied_call_requests());
    assert_eq!(enforce_responses.len(), 2, "got {enforce_responses:?}");

    let enforce_text = text_of(&enforce_responses[1]);
    assert!(enforce_text.starts_with("Denied (D-"), "{enforce_text}");
    assert_ne!(
        enforce_responses[1]["result"]["isError"], true,
        "a denial is not isError"
    );

    let enforce_lines = read_audit_lines(&enforce_audit);
    assert_eq!(enforce_lines.len(), 1, "{enforce_lines:?}");
    assert_eq!(enforce_lines[0]["decision"], "deny");
    assert_eq!(enforce_lines[0]["capability"], "read");
    assert_eq!(enforce_lines[0]["duration_ms"], 0);
    assert_eq!(enforce_lines[0]["grant_id"], "action-write-only");
    let enforce_denial_id = enforce_lines[0]["denial_id"]
        .as_str()
        .expect("denial_id present")
        .to_string();
    assert!(enforce_text.contains(&enforce_denial_id));

    let observe_audit = temp_path("observe-audit");
    let observe_manifest = write_manifest("observe", &manifest_value("observe", &observe_audit));
    let observe_responses = drive(&observe_manifest, &denied_call_requests());
    assert_eq!(observe_responses.len(), 2, "got {observe_responses:?}");

    let observe_text = text_of(&observe_responses[1]);
    assert!(
        !observe_text.starts_with("Denied ("),
        "shadow mode must not leak denial text: {observe_text}"
    );
    assert!(
        observe_text.contains("not connected"),
        "the call dispatched and failed at ordinary execution: {observe_text}"
    );
    assert_eq!(
        observe_responses[1]["result"]["isError"], true,
        "the dispatched-but-failed call is isError, same as any other execution failure"
    );

    let observe_lines = read_audit_lines(&observe_audit);
    assert_eq!(observe_lines.len(), 1, "{observe_lines:?}");
    assert_eq!(observe_lines[0]["decision"], "shadow_deny");
    assert_eq!(observe_lines[0]["capability"], "read");
    assert!(
        observe_lines[0]["duration_ms"].as_u64().unwrap_or(0) > 0,
        "a shadow-denied call actually ran and waited out the handshake window: {:?}",
        observe_lines[0]["duration_ms"]
    );
    assert_eq!(observe_lines[0]["grant_id"], "action-write-only");

    // NOT asserted here: that the two denial ids match. They deliberately do NOT: the manifest's
    // own top-level `mode` field is itself part of the canonical bytes `manifest_hash` is
    // computed over (g09's `canonical_hash`), so changing `mode` between these two manifest
    // FILES changes `manifest_hash`, which changes the denial id by design (ADR-0020: a denial
    // id is attributable to "the exact policy version that made it", and a manifest with a
    // different `mode` is a different version). The invariant the g15 doc's own manual
    // verification narrative actually describes -- SAME manifest_hash and grant, enforce vs
    // observe agree on the denial id -- holds only when `mode` is supplied OUTSIDE the hashed
    // manifest bytes (a per-grant `mode` override, or the `governance.mode` config key on an
    // unchanged manifest); `transport::mcp::server`'s own inline test
    // (`grant_shadow_deny_runs_the_tool_and_matches_the_enforce_denial_id`) proves exactly that
    // scenario directly, holding `manifest_hash` and `grants` fixed and varying only the
    // `manifest_mode` parameter `Governance::governed` takes.

    std::fs::remove_file(&enforce_audit).ok();
    std::fs::remove_file(&enforce_manifest).ok();
    std::fs::remove_file(&observe_audit).ok();
    std::fs::remove_file(&observe_manifest).ok();
}
