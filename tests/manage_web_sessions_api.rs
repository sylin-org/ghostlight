// SPDX-License-Identifier: Apache-2.0 OR MIT
//! K4 (`docs/tasks/console/K4-live-sessions-api.md`; PINS.md CS1, CS3, CS9): `GET
//! /api/v1/sessions`, the live-sessions/groups view. Read only.

mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static SEQ: AtomicU32 = AtomicU32::new(0);

fn test_webapi_port(seq: u32) -> u16 {
    20000 + ((std::process::id()).wrapping_add(seq) % 10000) as u16
}

fn http_get(port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to the web API");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let request =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn status_line(response: &str) -> &str {
    response.lines().next().unwrap_or_default()
}

fn body(response: &str) -> &str {
    response.split("\r\n\r\n").nth(1).unwrap_or_default()
}

/// PINS.md CS3/CS9: a real adapter session's first touch of a tabId (via a `tools/call` naming
/// it) is reported with a TRUNCATED (8-character) guid, its OS pid, and the tabId -- reachable
/// with NO fake extension attached, since `check_tab_ownership`'s `claim_tab` gate runs
/// synchronously BEFORE any dispatch to the browser (`src/transport/mcp/server.rs`); the
/// underlying tool call itself is never awaited or read back here.
#[test]
fn sessions_api_reports_a_live_adapter_session_with_truncated_guid() {
    let endpoint = format!(
        "ghostlight-console-sessions-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let port = test_webapi_port(20);
    let mut service = support::spawn_service_with_webapi_port(&endpoint, port);
    let mut adapter = support::spawn_adapter(&endpoint);

    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
        .unwrap();
    stdin
        .write_all(
            b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\
              \"params\":{\"name\":\"navigate\",\"arguments\":{\"tabId\":424242}}}\n",
        )
        .unwrap();
    // Give the service a brief moment to process both lines (initialize + the tools/call's
    // synchronous claim_tab gate) before querying the sessions view; the tool call's own
    // (eventual, extension-less) failure is never read back.
    std::thread::sleep(Duration::from_millis(300));

    let response = http_get(port, "/api/v1/sessions");
    assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
    let parsed: serde_json::Value = serde_json::from_str(body(&response)).expect("valid JSON");

    assert!(
        parsed["live_session_count"].as_u64().unwrap_or(0) >= 1,
        "at least the one live adapter session: {parsed}"
    );
    let bindings = parsed["adapter_bindings"].as_array().expect("array");
    let entry = bindings
        .iter()
        .find(|b| {
            b["owned_tab_ids"]
                .as_array()
                .map(|ids| ids.iter().any(|id| id == 424242))
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("no binding owns tabId 424242: {parsed}"));
    let guid = entry["guid"].as_str().expect("guid is a string");
    assert_eq!(
        guid.len(),
        8,
        "guid must be truncated to 8 characters: {guid}"
    );
    assert!(entry["pid"].as_u64().unwrap() > 0);

    let _ = adapter.kill();
    let _ = adapter.wait();
    let _ = service.kill();
    let _ = service.wait();
}

// K4's task file notes that `session::tests::live_session_summaries_reports_truncated_guid_pid_
// and_owned_tabs` (added at K1) already covers the never-a-full-guid unit-level proof; per that
// task file's own instruction, this file does not duplicate it.
