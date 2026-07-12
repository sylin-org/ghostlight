// SPDX-License-Identifier: Apache-2.0 OR MIT
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

mod support;

use ghostlight::browser::directory::{descriptor, requires};
use ghostlight::governance::dispatch::Governance;
use ghostlight::governance::ports::{
    AuditRecord, AuditSink, Capability, Decision, EffectiveMode, GoverningResource,
};
use ghostlight::transport::mcp::tools::advertised_tools_json;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// The 21 tool names in advertised order (the 13 trained tools plus `wait_for`, `script`,
/// `form_fill`, `file_upload` (ADR-0050 Decision 2), `browser_batch` (ADR-0050 Decision 3),
/// `upload_image` (ADR-0050 Decision 4), `gif_creator` (ADR-0050 Decision 5), and ADR-0022
/// Decision 7's sanctioned `explain` addition, positioned last), copied from the code-declared
/// registry (`browser::directory::REGISTRY`), in declared order.
const GOLDEN_TOOL_NAMES: [&str; 21] = [
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
    "wait_for",
    "script",
    "form_fill",
    "file_upload",
    "browser_batch",
    "upload_image",
    "gif_creator",
    "explain",
];

#[test]
fn tools_list_is_byte_stable_through_the_move() {
    let v = advertised_tools_json();
    let tools = v["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        GOLDEN_TOOL_NAMES.len(),
        "all 21 tools advertised (13 trained plus wait_for, script, form_fill, file_upload, browser_batch, upload_image, gif_creator, and explain)"
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

/// ADR-0050 Decision 2: `file_upload` is a new additive tool. It is allowed under the all-open
/// engine (no manifest = no denials) and classifies as a Write capability (bytes leave the user's
/// control into a web destination; the `ref` was located by a separately-governed read).
#[test]
fn file_upload_is_all_open_allowed_and_classifies_write() {
    let governance = Governance::all_open(std::sync::Arc::new(NullAuditSink));
    assert!(
        matches!(
            governance.decide(
                "file_upload",
                None,
                &[],
                GoverningResource::None,
                EffectiveMode::Enforce
            ),
            Decision::Allow { grant_id: None }
        ),
        "file_upload must be allowed in the all-open engine"
    );
    assert_eq!(
        requires("file_upload", None),
        Some(&[Capability::Write][..]),
        "file_upload classifies as a Write capability"
    );
}

/// Proves the facade change at the dispatch chokepoint did not disturb the `read_page`
/// secret-redaction overlay: a fake extension answers with a result carrying the engine's
/// `secret_value="..."` marker, and the client-visible text must come back redacted (the safe
/// default keeps `content.security.secrets.redact` on) with the marker gone.
///
/// H6 (ADR-0030 Decision 8 amendment; the ONE sanctioned exception to this file's otherwise-frozen
/// spawn choreography, BOOTSTRAP "only delight is sacred"): drives the standalone SERVICE + thin
/// ADAPTER topology. Every assertion below is verbatim -- the redaction wiring is the invariant;
/// only WHICH two processes are spawned changed.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn read_page_redaction_is_still_wired_at_the_chokepoint() {
    let endpoint = format!(
        "ghostlight-golden-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let mut service = support::spawn_service(&endpoint);
    let mut adapter = support::spawn_adapter(&endpoint);

    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        // o04 (ADR-0031 Decision 4): inputSchema validation now runs before dispatch, so the
        // read_page call needs a tabId to reach the redaction chokepoint (previously the empty
        // arguments object was forwarded as-is; the validator now catches it earlier). The test's
        // oracle -- the redacted text in the extension's reply -- is unchanged.
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"read_page","arguments":{"tabId":1}}}),
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
            support::send_extension_attach_frames(&mut write_half).await;
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

    let stdout = adapter.stdout.take().expect("adapter stdout");
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
    let _ = adapter.wait();
    let _ = service.kill();
    let _ = service.wait();
}
