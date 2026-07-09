// SPDX-License-Identifier: Apache-2.0 OR MIT
//! End-to-end MCP protocol checks: spawn the standalone SERVICE + a thin ADAPTER (ADR-0030
//! Decision 8 amendment; movable harness at H6, PINS.md/BOOTSTRAP "only delight is sacred") and
//! drive the ADAPTER over stdio.
//!
//! Most tests here connect no extension/native-host, so `tools/call` waits out the bounded
//! handshake window and returns an MCP tool error result (the request/response bridge itself is
//! covered by the `browser` and `ipc` unit tests). One test below connects a fake extension over
//! the real IPC (to the EXTENSION endpoint the SERVICE owns) to exercise the late-connect /
//! truthful-wait-note path. Each spawned service pair gets a unique IPC endpoint so the tests
//! never contend for one.

mod support;

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn unique_endpoint() -> String {
    format!(
        "ghostlight-it-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// Spawn a SERVICE + ADAPTER pair (with an isolated IPC endpoint), send each request as a line to
/// the ADAPTER's stdin, and collect exactly one reply per `id`-bearing request (a notification, no
/// `id` key at all, gets none). Reads all expected replies BEFORE closing stdin: `relay_adapter`
/// races its two copy directions (PINS.md SS1 pin 3's lifecycle-shape mirror of
/// `relay_native_host`), so closing the client's write side early would tear the whole relay down
/// -- and this process's own reply -- before a still-in-flight call (e.g. the ~5s
/// extension-handshake wait below) is delivered; keeping stdin open until every expected reply has
/// arrived avoids that race entirely, mirroring this file's own
/// `tools_call_waits_for_a_late_extension_and_notes_the_wait` pattern. Kills the service in
/// teardown.
fn drive(requests: &[Value]) -> Vec<Value> {
    drive_with_manifest(None, requests)
}

/// Like [`drive`], but optionally launches the SERVICE under a schema-3 `--manifest` (PINS.md
/// SS5.1: `--manifest` is forwarded to the SERVICE; the ADAPTER ignores it). `None` is the
/// all-open posture (no `--manifest` argument).
fn drive_with_manifest(manifest: Option<&str>, requests: &[Value]) -> Vec<Value> {
    let endpoint = unique_endpoint();
    let manifest_path = manifest.map(|body| {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-mcp-protocol-{}-{}.json",
            std::process::id(),
            SEQ.load(Ordering::Relaxed)
        ));
        std::fs::write(&path, body).unwrap();
        path
    });
    let manifest_uri = manifest_path.as_ref().map(|path| {
        // `file://` source form: forward slashes, and a leading `/` before a Windows drive letter.
        let forward = path.to_string_lossy().replace('\\', "/");
        match forward.strip_prefix('/') {
            Some(rest) => format!("file:///{rest}"),
            None => format!("file:///{forward}"),
        }
    });

    let mut service = support::spawn_service_with_manifest(&endpoint, manifest_uri.as_deref());
    let mut adapter = support::spawn_adapter(&endpoint);

    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    for req in requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let expected = requests.iter().filter(|r| r.get("id").is_some()).count();
    let stdout = adapter.stdout.take().expect("adapter stdout");
    let mut lines = BufReader::new(stdout).lines();
    let mut responses = Vec::with_capacity(expected);
    for _ in 0..expected {
        let line = lines
            .next()
            .expect("the adapter's stdout closed before every expected reply arrived")
            .expect("read a stdout line");
        responses.push(serde_json::from_str(&line).expect("each stdout line is JSON"));
    }

    drop(stdin);
    let _ = adapter.wait();
    let _ = service.kill();
    let _ = service.wait();
    if let Some(path) = &manifest_path {
        std::fs::remove_file(path).ok();
    }
    responses
}

/// Like [`drive`], but sends RAW lines verbatim (so a malformed frame or a JSON-RPC array batch can
/// be exercised) and reads EXACTLY `expected` responses. The ADR-0049 parse-error / batch rejects
/// reply with `id: null`, so they cannot be counted by id-presence the way [`drive`] does.
fn drive_raw(lines: &[&str], expected: usize) -> Vec<Value> {
    let endpoint = unique_endpoint();
    let mut service = support::spawn_service_with_manifest(&endpoint, None);
    let mut adapter = support::spawn_adapter(&endpoint);

    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    for line in lines {
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    let stdout = adapter.stdout.take().expect("adapter stdout");
    let mut reader = BufReader::new(stdout).lines();
    let mut responses = Vec::with_capacity(expected);
    for _ in 0..expected {
        let line = reader
            .next()
            .expect("the adapter's stdout closed before every expected reply arrived")
            .expect("read a stdout line");
        responses.push(serde_json::from_str(&line).expect("each stdout line is JSON"));
    }

    drop(stdin);
    let _ = adapter.wait();
    let _ = service.kill();
    let _ = service.wait();
    responses
}

/// ADR-0049: a JSON-RPC batch (a top-level array of requests) is rejected with -32600 and a
/// teaching message (send one per line; use `script` for multi-step), not dropped silently.
#[test]
fn batch_array_frame_is_rejected_with_a_teaching_message() {
    let batch =
        r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#;
    let responses = drive_raw(&[batch], 1);
    let err = &responses[0];
    assert_eq!(err["id"], Value::Null);
    assert_eq!(err["error"]["code"], -32600);
    let msg = err["error"]["message"].as_str().expect("error message");
    assert!(
        msg.contains("one JSON-RPC message per line"),
        "teaches the one-per-line rule: {msg}"
    );
    assert!(
        msg.contains("`script`"),
        "teaches the script-tool alternative: {msg}"
    );
}

/// ADR-0049: an unparseable NON-empty line gets an addressable -32700 (id:null); a blank line is a
/// benign keepalive that draws NO response. Sending the blank first proves it is silent -- the sole
/// reply is the malformed line's -32700, not a response to the blank.
#[test]
fn parse_error_answers_32700_and_blank_lines_stay_silent() {
    let responses = drive_raw(&["", "{ this is not valid json"], 1);
    let err = &responses[0];
    assert_eq!(err["id"], Value::Null);
    assert_eq!(err["error"]["code"], -32700);
}

#[test]
fn initialize_tools_list_and_tool_call_over_stdio() {
    let responses = drive(&[
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}), // no response
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        // o04: inputSchema validation now runs before dispatch; navigate needs url + tabId.
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://example.com","tabId":1}}}),
    ]);

    assert_eq!(
        responses.len(),
        3,
        "expected 3 responses, got {responses:?}"
    );

    let init = &responses[0];
    assert_eq!(init["id"], 1);
    // ADR-0049: with no protocolVersion requested (params:{}), the latest supported is offered.
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(init["result"]["capabilities"]["tools"]["listChanged"], true);
    assert_eq!(init["result"]["serverInfo"]["name"], "ghostlight");

    let list = &responses[1];
    assert_eq!(list["id"], 2);
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        17,
        "13 trained tools plus wait_for, script, form_fill, and the explain addition"
    );
    assert_eq!(tools[0]["name"], "tabs_context_mcp");
    // The advertised surface must equal the embedded sacred fixture, byte for byte.
    let fixture = ghostlight::mcp::tools::advertised_tools_json();
    assert_eq!(
        list["result"], fixture,
        "tools/list must equal the sacred fixture"
    );

    // No extension is connected, so the tool call waits the bounded window (about 5s), falls
    // through to Browser::call's fail-fast "not connected" path, and returns an MCP tool error
    // result (isError) with the exact hop-attributed message.
    let call = &responses[2];
    assert_eq!(call["id"], 3);
    assert_eq!(call["result"]["isError"], true, "no extension -> isError");
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("error result carries a text block");
    assert!(
        text.starts_with("[hop: extension]"),
        "hop-attributed message: {text}"
    );
    assert_eq!(
        text,
        "[hop: extension] Browser extension not connected. \
         Next step: check chrome://extensions and that Chrome is running.",
        "exact message: {text}"
    );
}

/// ADR-0022 Decision 7: `explain` appears in `tools/list` last (the one sanctioned addition to
/// the sacred surface) and `tools/call explain` returns the directory text without ever needing
/// an extension attached -- proving the tool is handled entirely server-side, with zero
/// native-messaging traffic.
#[test]
fn explain_is_advertised_last_and_answers_with_no_extension_attached() {
    let responses = drive(&[
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"explain","arguments":{}}}),
    ]);
    assert_eq!(responses.len(), 3, "got {responses:?}");

    let list = &responses[1];
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.last().expect("at least one tool")["name"],
        "explain",
        "explain must be the last advertised tool"
    );

    let call = &responses[2];
    assert_eq!(call["id"], 3);
    assert_ne!(call["result"]["isError"], true, "explain must never error");
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block");
    assert!(
        text.starts_with("Capabilities: read = "),
        "explain's response opens with the capability vocabulary: {text}"
    );
    assert!(
        text.trim_end().ends_with(
            "explain: requires nothing. Show every action available here and the capability \
             each one requires."
        ),
        "explain's response lists its own row last: {text}"
    );
}

/// Run `explain` under a given manifest posture and return its response text, asserting along the
/// way that it is advertised last and never errors regardless of posture.
fn explain_text_under_manifest(manifest: Option<&str>) -> String {
    let responses = drive_with_manifest(
        manifest,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"explain","arguments":{}}}),
        ],
    );
    assert_eq!(responses.len(), 3, "got {responses:?}");
    let list = responses
        .iter()
        .find(|r| r["id"] == 2)
        .expect("tools/list reply");
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.last().expect("at least one tool")["name"],
        "explain",
        "explain must be advertised (last) under every posture"
    );
    let call = responses
        .iter()
        .find(|r| r["id"] == 3)
        .expect("explain tools/call reply");
    assert_ne!(
        call["result"]["isError"], true,
        "explain must never error under any posture: {call:?}"
    );
    call["result"]["content"][0]["text"]
        .as_str()
        .expect("explain text content block")
        .to_string()
}

/// ADR-0022 Decision 7 (the map is always the same map): `explain` returns byte-identical output
/// regardless of manifest posture. It requires nothing and is answered server-side before any
/// grant machinery, so a locked-down session sees the identical directory an all-open one does.
/// Pins the actual invariant (same output everywhere), not merely that `explain` is present.
#[test]
fn explain_output_is_byte_identical_across_manifest_postures() {
    let open = explain_text_under_manifest(None);
    let empty_grants = explain_text_under_manifest(Some(
        r#"{"schema":3,"name":"empty","version":"1","grants":[]}"#,
    ));
    let read_only = explain_text_under_manifest(Some(
        r#"{"schema":3,"name":"ro","version":"1","grants":[{"id":"read-only","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
    ));

    assert!(
        open.starts_with("Capabilities: read = "),
        "sanity: explain opens with the vocabulary block: {open}"
    );
    assert_eq!(
        open, empty_grants,
        "explain output must not change under an empty-grants manifest"
    );
    assert_eq!(
        open, read_only,
        "explain output must not change under a restrictive read-only manifest"
    );
}

#[test]
fn unknown_tool_name_is_rejected_before_dispatch() {
    // No extension is ever connected in this test. If the unknown-tool pre-check ran AFTER the
    // bounded extension-channel wait (or not at all), this would instead time out and surface
    // "[hop: extension] Browser extension not connected. ...". Getting the invalid-request hop
    // back proves the pre-check runs first.
    let started = std::time::Instant::now();
    let responses = drive(&[
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"bogus_tool","arguments":{}}}),
    ]);
    let elapsed = started.elapsed();

    assert_eq!(responses.len(), 2, "got {responses:?}");
    let call = &responses[1];
    assert_eq!(call["id"], 2);
    assert_eq!(call["result"]["isError"], true, "unknown tool -> isError");
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("error result carries a text block");
    assert!(
        text.starts_with("[hop: invalid-request]"),
        "hop-attributed message: {text}"
    );
    assert!(
        text.contains("Unknown tool: bogus_tool"),
        "names the unknown tool: {text}"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "the pre-check must return well before the 5s extension-channel wait: {elapsed:?}"
    );
}

#[test]
fn malformed_method_and_null_id_follow_jsonrpc_rules() {
    let responses = drive(&[
        json!({"jsonrpc":"2.0","id":7,"params":{}}), // id present, method missing
        json!({"jsonrpc":"2.0","id":null,"method":"ping"}), // legal null-id request
        json!({"method":"notifications/initialized"}), // notification -> no response
    ]);

    // The notification yields nothing; the other two are addressable.
    assert_eq!(responses.len(), 2, "got {responses:?}");

    // Missing method, but the id is recoverable -> -32600 addressed to id 7.
    assert_eq!(responses[0]["id"], 7);
    assert_eq!(responses[0]["error"]["code"], -32600);

    // id: null is a legal request; the response must echo the id as null (present, not omitted).
    assert!(
        responses[1].as_object().unwrap().contains_key("id"),
        "a null-id request must get an id back, not an omitted field"
    );
    assert_eq!(responses[1]["id"], Value::Null);
}

#[test]
fn tools_call_waits_for_a_late_extension_and_notes_the_wait() {
    let endpoint = unique_endpoint();
    let mut service = support::spawn_service(&endpoint);
    let mut adapter = support::spawn_adapter(&endpoint);

    // Unlike `drive`, stdin is kept open for the whole test: the tools/call response only
    // arrives after the fake extension below connects, several hundred ms into the test.
    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        // o04: inputSchema validation now runs before dispatch; navigate needs url + tabId.
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://example.com","tabId":1}}}),
    ];
    for req in &requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    // Fake extension: connects late (after the tools/call is already queued and waiting) over
    // the real IPC, to the EXTENSION endpoint the SERVICE owns (the plain `endpoint`, unrelated
    // to the `-adapter` endpoint the adapter dials), reads the one framed tool_request, and
    // answers it. Runs on its own thread with its own runtime, mirroring the fake-extension
    // pattern in `src/browser.rs` and `src/native/ipc.rs`, since this test file (like its other
    // tests) drives the children synchronously.
    let fake_endpoint = endpoint.clone();
    let fake_ext = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("build a tokio runtime");
        rt.block_on(async move {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
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
                "result": { "content": [ { "type": "text", "text": "navigated" } ] },
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
    assert_eq!(second["id"], 2, "second response is the tools/call reply");
    assert_ne!(
        second["result"]["isError"], true,
        "the late-connected call must succeed: {second:?}"
    );
    let content = second["result"]["content"]
        .as_array()
        .expect("content array");
    assert_eq!(
        content[0]["text"], "navigated",
        "first block is the tool's own result"
    );
    let last_text = content
        .last()
        .expect("at least one content block")
        .get("text")
        .and_then(Value::as_str)
        .expect("last block carries text");
    assert!(
        last_text.starts_with("(waited ")
            && last_text.ends_with("s for browser extension handshake)"),
        "last block is the truthful wait note: {last_text}"
    );

    fake_ext.join().expect("fake-extension thread panicked");
    drop(stdin);
    let _ = adapter.wait();
    let _ = service.kill();
    let _ = service.wait();
}
