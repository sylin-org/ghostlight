// SPDX-License-Identifier: Apache-2.0 OR MIT
//! K4 (`docs/tasks/console/K4-live-sessions-api.md`; PINS.md CS1, CS3, CS9): `GET
//! /api/v1/sessions`, the live-sessions/groups view. Read only.

mod support;

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static SEQ: AtomicU32 = AtomicU32::new(0);

fn http_get(port: u16, path: &str) -> String {
    let mut stream = support::connect_webapi(port);
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
    // split_once: everything after the FIRST header/body delimiter, even when the body itself
    // contains a blank line (a "\r\n\r\n" run). A plain split(..).nth(1) would return only the
    // segment up to the body's first blank line and silently truncate it.
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default()
}

/// PINS.md CS3/CS9: a real adapter session's first touch of a tabId (via a `tools/call` naming
/// it) is reported with a TRUNCATED (8-character) guid, its OS pid, and the tabId -- reachable
/// with NO fake extension attached, since `check_tab_ownership`'s `claim_tab` gate runs
/// synchronously BEFORE any dispatch to the browser (`src/transport/mcp/server.rs`); the
/// underlying tool call itself is never awaited or read back here.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn sessions_api_reports_a_live_adapter_session_with_truncated_guid() {
    let endpoint = format!(
        "ghostlight-console-sessions-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let (mut service, port) = support::spawn_service_with_webapi_port(&endpoint);
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
    // Poll the sessions view until the adapter's binding for tab 424242 appears: the adapter's
    // registration and the tools/call's synchronous claim_tab gate must propagate first, and that
    // timing varies by platform, so wait on the observable outcome rather than a fixed sleep (the
    // tool call's own eventual, extension-less failure is never read back).
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let parsed = loop {
        let response = http_get(port, "/api/v1/sessions");
        assert_eq!(status_line(&response), "HTTP/1.1 200 OK");
        let parsed: serde_json::Value = serde_json::from_str(body(&response)).expect("valid JSON");
        let has_binding = parsed["adapter_bindings"]
            .as_array()
            .map(|bs| {
                bs.iter().any(|b| {
                    b["owned_tab_ids"]
                        .as_array()
                        .map(|ids| ids.iter().any(|id| id == 424242))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if has_binding {
            break parsed;
        }
        if std::time::Instant::now() >= deadline {
            panic!("no binding owns tabId 424242 within the deadline: {parsed}");
        }
        std::thread::sleep(Duration::from_millis(100));
    };

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
    let pid = entry["pid"].as_u64().expect("pid is a number");
    // macOS's getpeereid() reports no pid, so capture_peer_cred records a documented, logging-only
    // pid: 0 there (ADR-0030 Decision 4 amendment). Linux (SO_PEERCRED) and Windows both provide
    // it. Assert the positive pid only where the OS actually supplies one.
    if cfg!(not(target_os = "macos")) {
        assert!(pid > 0, "peer pid must be reported on this platform: {pid}");
    }

    let _ = adapter.kill();
    let _ = adapter.wait();
    let _ = service.kill();
    let _ = service.wait();
}

// K4's task file notes that `session::tests::live_session_summaries_reports_truncated_guid_pid_
// and_owned_tabs` (added at K1) already covers the never-a-full-guid unit-level proof; per that
// task file's own instruction, this file does not duplicate it.
