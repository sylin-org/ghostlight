//! End-to-end MCP protocol checks: spawn the binary as an mcp-server and drive it over stdio.
//!
//! Most tests here connect no extension/native-host, so `tools/call` waits out the bounded
//! handshake window and returns an MCP tool error result (the request/response bridge itself is
//! covered by the `browser` and `ipc` unit tests). One test below connects a fake extension over
//! the real IPC to exercise the late-connect / truthful-wait-note path. Each spawned binary gets
//! a unique IPC endpoint so the tests never contend for one.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// Spawn the binary (with an isolated IPC endpoint), send each request as a line, close stdin, and
/// collect the response lines.
fn drive(requests: &[Value]) -> Vec<Value> {
    let endpoint = format!(
        "ghostlight-it-{}-{}",
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
    for req in requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    drop(stdin); // EOF -> the server loop ends

    let stdout = child.stdout.take().expect("stdout");
    let responses: Vec<Value> = BufReader::new(stdout)
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).expect("each stdout line is JSON"))
        .collect();
    child.wait().expect("wait for child");
    responses
}

#[test]
fn initialize_tools_list_and_tool_call_over_stdio() {
    let responses = drive(&[
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}), // no response
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{}}}),
    ]);

    assert_eq!(
        responses.len(),
        3,
        "expected 3 responses, got {responses:?}"
    );

    let init = &responses[0];
    assert_eq!(init["id"], 1);
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["serverInfo"]["name"], "ghostlight");

    let list = &responses[1];
    assert_eq!(list["id"], 2);
    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        14,
        "13 trained tools plus the ADR-0022 Decision 7 explain addition"
    );
    assert_eq!(tools[0]["name"], "tabs_context_mcp");
    // The advertised surface must equal the embedded sacred fixture, byte for byte.
    let fixture: Value = serde_json::from_str(ghostlight::mcp::tools::TOOLS_JSON).unwrap();
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

/// Like [`drive`], but optionally launches the server under a schema-3 `--manifest` (written to a
/// temp file and cleaned up after). `None` is the all-open posture (no `--manifest` argument).
fn drive_with_manifest(manifest: Option<&str>, requests: &[Value]) -> Vec<Value> {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let endpoint = format!("ghostlight-it-{}-{}", std::process::id(), seq);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ghostlight"));
    cmd.env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let manifest_path = manifest.map(|body| {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-mcp-protocol-{}-{}.json",
            std::process::id(),
            seq
        ));
        std::fs::write(&path, body).unwrap();
        path
    });
    if let Some(path) = &manifest_path {
        // `file://` source form: forward slashes, and a leading `/` before a Windows drive letter.
        let forward = path.to_string_lossy().replace('\\', "/");
        let uri = match forward.strip_prefix('/') {
            Some(rest) => format!("file:///{rest}"),
            None => format!("file:///{forward}"),
        };
        cmd.arg("--manifest").arg(uri);
    }
    let mut child = cmd.spawn().expect("spawn ghostlight");

    let mut stdin = child.stdin.take().expect("stdin");
    for req in requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    drop(stdin); // EOF -> the server loop ends

    let stdout = child.stdout.take().expect("stdout");
    let responses: Vec<Value> = BufReader::new(stdout)
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).expect("each stdout line is JSON"))
        .collect();
    child.wait().expect("wait for child");
    if let Some(path) = &manifest_path {
        std::fs::remove_file(path).ok();
    }
    responses
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
    let endpoint = format!(
        "ghostlight-it-{}-{}",
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

    // Unlike `drive`, stdin is kept open for the whole test: the tools/call response only
    // arrives after the fake extension below connects, several hundred ms into the test.
    let mut stdin = child.stdin.take().expect("stdin");
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"url":"https://example.com"}}}),
    ];
    for req in &requests {
        stdin
            .write_all(serde_json::to_string(req).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }

    // Fake extension: connects late (after the tools/call is already queued and waiting) over
    // the real IPC, reads the one framed tool_request, and answers it. Runs on its own thread
    // with its own runtime, mirroring the fake-extension pattern in `src/browser.rs` and
    // `src/native/ipc.rs`, since this test file (like its other tests) drives the child
    // synchronously.
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

    let stdout = child.stdout.take().expect("stdout");
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
    let _ = child.wait();
}
