//! All-open golden guard for the A1 module reorg and the A3 governance facade. Neither the
//! regroup into governance/ browser/ transport/ (A1) nor the introduction of the `Governance`
//! facade at the dispatch chokepoint (A3) may change anything observable. Invariants, reached
//! through the NEW module locations:
//!   1. tools/list byte-stability -- the advertised tool surface is the same 13 tools in
//!      the same order, and `directory::descriptor` still resolves them.
//!   2. facade decide round-trip -- `Governance::all_open()` resolves every call to
//!      `Decision::Allow { grant_id: None }` without touching any decision port (audit is
//!      orthogonal to all-open, shared format doc section 4.5, so the facade still carries an
//!      audit sink).
//!   3. `read_page` secret redaction is still wired at the chokepoint (governed by the
//!      unchanged `content.security.secrets.redact` key), exercised end-to-end over stdio.

use ghostlight::browser::directory::descriptor;
use ghostlight::governance::dispatch::Governance;
use ghostlight::governance::ports::{
    AuditRecord, AuditSink, Decision, EffectiveMode, GoverningResource,
};
use ghostlight::transport::mcp::tools::TOOLS_JSON;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// The 14 tool names in advertised order (the 13 trained tools plus ADR-0022 Decision 7's
/// sanctioned `explain` addition, positioned last, landed in stage-3 task s07), copied from the
/// parsed `TOOLS_JSON` fixture (the sacred fixture is the source of truth for the exact order).
const GOLDEN_TOOL_NAMES: [&str; 14] = [
    "tabs_context_mcp",
    "tabs_create_mcp",
    "navigate",
    "computer",
    "find",
    "form_input",
    "get_page_text",
    "javascript_tool",
    "read_console_messages",
    "read_network_requests",
    "read_page",
    "resize_window",
    "update_plan",
    "explain",
];

#[test]
fn tools_list_is_byte_stable_through_the_move() {
    let v: Value = serde_json::from_str(TOOLS_JSON).expect("TOOLS_JSON parses");
    let tools = v["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        GOLDEN_TOOL_NAMES.len(),
        "all 14 tools advertised (13 trained plus explain)"
    );
    for (i, name) in GOLDEN_TOOL_NAMES.iter().enumerate() {
        assert_eq!(
            tools[i]["name"], *name,
            "tool #{i} name and order preserved"
        );
        assert!(descriptor(name).is_some(), "{name} must be a known tool");
    }
    assert!(
        descriptor("bogus_tool").is_none(),
        "unknown tools stay unknown"
    );
}

/// A sink that drops every record; enough to construct an all-open facade for this test
/// without pulling in the real file/stderr recorders.
struct NullAuditSink;
impl AuditSink for NullAuditSink {
    fn record(&self, _record: &AuditRecord) {}
    fn record_session_event(&self, _record: &ghostlight::governance::ports::SessionEventRecord) {}
}

#[test]
fn facade_decide_is_all_open_after_the_move() {
    let governance = Governance::all_open(std::sync::Arc::new(NullAuditSink));
    for name in GOLDEN_TOOL_NAMES {
        assert!(
            matches!(
                governance.decide(
                    name,
                    None,
                    &[],
                    GoverningResource::None,
                    EffectiveMode::Enforce
                ),
                Decision::Allow { grant_id: None }
            ),
            "{name} must be allowed in the all-open engine"
        );
    }
}

/// Proves the facade change at the dispatch chokepoint did not disturb the `read_page`
/// secret-redaction overlay: a fake extension answers with a result carrying the engine's
/// `secret_value="..."` marker, and the client-visible text must come back redacted (the safe
/// default keeps `content.security.secrets.redact` on) with the marker gone.
#[test]
fn read_page_redaction_is_still_wired_at_the_chokepoint() {
    let endpoint = format!(
        "ghostlight-golden-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_ghostlight"))
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight");

    let mut stdin = child.stdin.take().expect("stdin");
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"read_page","arguments":{}}}),
    ];
    for req in &requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let fake_endpoint = endpoint.clone();
    let fake_ext = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("build a tokio runtime");
        rt.block_on(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let stream = ghostlight::native::ipc::connect(&fake_endpoint)
                .await
                .expect("fake extension connects to the real IPC endpoint");
            let (mut read_half, mut write_half) = tokio::io::split(stream);
            let req = ghostlight::native::host::read_message(&mut read_half)
                .await
                .unwrap()
                .expect("one framed tool_request");
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({
                "id": v["id"],
                "type": "tool_response",
                "result": { "content": [ {
                    "type": "text",
                    "text": "textbox \"Password\" [ref_3] secret_value=\"hunter2\" type=\"password\""
                } ] },
            });
            ghostlight::native::host::write_message(
                &mut write_half,
                &serde_json::to_vec(&reply).unwrap(),
            )
            .await
            .unwrap();
        });
    });

    let stdout = child.stdout.take().expect("stdout");
    let mut lines = BufReader::new(stdout).lines();

    let first: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(first["id"], 1, "first response is the initialize reply");

    let second: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    assert_eq!(second["id"], 2, "second response is the read_page reply");
    assert_ne!(
        second["result"]["isError"], true,
        "the call must succeed: {second:?}"
    );
    let text = second["result"]["content"][0]["text"]
        .as_str()
        .expect("first content block carries text");
    assert!(
        text.contains("value=\"[value redacted]\""),
        "secret value must be redacted: {text}"
    );
    assert!(
        !text.contains("secret_value="),
        "the raw marker must never reach the client: {text}"
    );
    assert!(
        !text.contains("hunter2"),
        "the raw secret value must never reach the client: {text}"
    );

    fake_ext.join().expect("fake-extension thread panicked");
    drop(stdin);
    let _ = child.wait();
}
