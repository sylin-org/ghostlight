// SPDX-License-Identifier: Apache-2.0 OR MIT
//! End-to-end proof of `docs/tasks/hub/BOOTSTRAP.md`'s "Completion criteria" for the H0-H9
//! Ghostlight Hub batch: "Two MCP clients multiplex concurrently through one service (H2), each
//! with its own GUID-keyed session and, at H7, its own tab group; a kill fans out one audit
//! record per live subject."
//!
//! This is NOT one of the pinned H0-H9 batch tasks -- there is no task file to cite and no
//! author-pinned oracle here. Every assertion below is grounded directly in the CURRENT shipped
//! implementation (not invented): the wire shapes in `src/hub/outbound/browser.rs`
//! (`tool_request`/`tool_response`/`group_request`, all doc-commented at that module's top), the
//! tab-claim/group-emit logic in `src/hub/session.rs` (`claim_tab`/`owned_tab_ids`) and
//! `src/transport/mcp/server.rs` (`check_tab_ownership`/`emit_group_request`), and the global
//! kill-fan-out semantics in `src/hub/outbound/browser.rs::handle_session_killed` (ADR-0030
//! Decision 7: `killed`/`held`/`connected` stay GLOBAL on the one shared browser link, since
//! there is exactly one physical extension attachment multiplexed by many sessions -- a single
//! `session_killed` event therefore ends every live session at once, each getting its own
//! fanned-out audit hook invocation).
//!
//! Existing per-task tests already cover each of these properties in isolation, mostly with an
//! in-process `Browser` clone standing in for a session (`tests/hub_multiplex.rs`) or a single
//! adapter (`tests/hub_lifecycle.rs`). What no existing test does is drive TWO real, concurrently
//! spawned `ghostlight` adapter subprocesses through one real, standalone `ghostlight service`
//! process (H6's actual topology) at once and observe the combined behavior on the wire.

mod support;

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::ChildStdin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static SEQ: AtomicU32 = AtomicU32::new(0);

fn write_line(stdin: &mut ChildStdin, req: &Value) {
    stdin
        .write_all(serde_json::to_string(req).unwrap().as_bytes())
        .unwrap();
    stdin.write_all(b"\n").unwrap();
}

fn read_line<R: BufRead>(reader: &mut R) -> Value {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("a reply line from the adapter");
    serde_json::from_str(line.trim_end()).expect("the reply line is well-formed JSON")
}

/// The core completion-criteria scenario, driven through the real standalone-service +
/// two-thin-adapter topology (H6), with ONE fake extension standing in for Chrome on the ONE
/// physical extension link both sessions multiplex over.
#[test]
fn two_real_adapters_multiplex_get_own_tab_groups_and_share_one_kill() {
    let endpoint = format!(
        "ghostlight-completion-criteria-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let mut service = support::spawn_service(&endpoint);
    let mut adapter_a = support::spawn_adapter(&endpoint);
    let mut adapter_b = support::spawn_adapter(&endpoint);

    let mut stdin_a = adapter_a.stdin.take().expect("adapter A stdin");
    let mut stdin_b = adapter_b.stdin.take().expect("adapter B stdin");
    let mut reader_a = BufReader::new(adapter_a.stdout.take().expect("adapter A stdout"));
    let mut reader_b = BufReader::new(adapter_b.stdout.take().expect("adapter B stdout"));

    // Each session initializes independently (each is its own GUID-keyed session, ADR-0030
    // Decision 4; `initialize` never touches the browser, so no fake extension is needed yet).
    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    assert_eq!(read_line(&mut reader_a)["id"], 1, "session A's own reply");
    assert_eq!(read_line(&mut reader_b)["id"], 1, "session B's own reply");

    // ONE fake extension: the single physical link both sessions multiplex over (H2 Decision 2).
    // It must observe exactly 2 group_request frames (H7: one per session's first-touched tab,
    // `check_tab_ownership`'s `TabClaim::Adopted` branch) and exactly 2 tool_request frames (one
    // per session's `navigate` call), in whichever order they interleave on the shared link, then
    // fire the ONE global `session_killed` event.
    let fake_endpoint = endpoint.clone();
    let fake_ext = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("build a tokio runtime");
        rt.block_on(async move {
            let stream = ghostlight::native::ipc::connect(&fake_endpoint)
                .await
                .expect("fake extension connects to the real extension endpoint");
            let (mut read_half, mut write_half) = tokio::io::split(stream);

            let mut group_requests: Vec<Value> = Vec::new();
            let mut tool_requests: Vec<Value> = Vec::new();
            while group_requests.len() < 2 || tool_requests.len() < 2 {
                let frame = ghostlight::native::host::read_message(&mut read_half)
                    .await
                    .unwrap()
                    .expect("a framed message from the service");
                let v: Value = serde_json::from_slice(&frame).unwrap();
                match v.get("type").and_then(Value::as_str) {
                    Some("group_request") => group_requests.push(v),
                    Some("tool_request") => {
                        let reply = json!({
                            "id": v["id"],
                            "type": "tool_response",
                            "result": {
                                "content": [{
                                    "type": "text",
                                    "text": format!("navigated tabId={}", v["args"]["tabId"]),
                                }]
                            },
                        });
                        ghostlight::native::host::write_message(
                            &mut write_half,
                            &serde_json::to_vec(&reply).unwrap(),
                        )
                        .await
                        .unwrap();
                        tool_requests.push(v);
                    }
                    other => panic!("unexpected frame type from the service: {other:?} ({v:?})"),
                }
            }

            ghostlight::native::host::write_message(
                &mut write_half,
                &serde_json::to_vec(&json!({ "type": "session_killed" })).unwrap(),
            )
            .await
            .unwrap();

            (group_requests, tool_requests)
        })
    });

    // Give the fake extension a moment to actually connect before the two adapters' first calls
    // dial in (mirrors `tests/all_open_golden.rs`'s own grace before its first framed exchange).
    std::thread::sleep(Duration::from_millis(200));

    // Each session touches a DISTINCT tabId for the first time: H4 first-touch adoption, H7
    // group-request emission on the `Adopted` transition only.
    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":101,"url":"https://a.example.com"}}}),
    );
    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":202,"url":"https://b.example.com"}}}),
    );

    let reply_a = read_line(&mut reader_a);
    let reply_b = read_line(&mut reader_b);
    assert_eq!(reply_a["id"], 2, "session A's own reply id");
    assert_eq!(reply_b["id"], 2, "session B's own reply id");
    let text_a = reply_a["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let text_b = reply_b["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text_a.contains("101") && !text_a.contains("202"),
        "session A gets its OWN reply, never session B's: {text_a}"
    );
    assert!(
        text_b.contains("202") && !text_b.contains("101"),
        "session B gets its OWN reply, never session A's: {text_b}"
    );

    let (group_requests, tool_requests) = fake_ext.join().expect("fake-extension thread panicked");

    assert_eq!(tool_requests.len(), 2, "one tool_request per session");
    let mut seen_tabs: Vec<i64> = tool_requests
        .iter()
        .map(|r| r["args"]["tabId"].as_i64().unwrap())
        .collect();
    seen_tabs.sort_unstable();
    assert_eq!(seen_tabs, vec![101, 202], "both sessions' tabIds observed");

    assert_eq!(
        group_requests.len(),
        2,
        "exactly one H7 group_request per session's first-touched tab, never merged"
    );
    let guid_a = group_requests[0]["guid"].as_str().unwrap();
    let guid_b = group_requests[1]["guid"].as_str().unwrap();
    assert_ne!(
        guid_a, guid_b,
        "the two sessions' group_requests carry DISTINCT GUIDs"
    );
    let mut tab_id_sets: Vec<Vec<i64>> = group_requests
        .iter()
        .map(|r| {
            r["tabIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect()
        })
        .collect();
    tab_id_sets.sort();
    assert_eq!(
        tab_id_sets,
        vec![vec![101], vec![202]],
        "each session's group contains ONLY its own tab, never the other session's"
    );
    for r in &group_requests {
        let title = r["title"].as_str().unwrap();
        assert!(
            title.starts_with('\u{1F47B}'),
            "the pinned ghost-glyph group title prefix (PINS.md SS6): {title}"
        );
    }

    // ADR-0030 Decision 7: the ONE global `session_killed` event (already sent by the fake
    // extension above, once both group/tool request pairs were observed) must end BOTH live
    // sessions -- there is exactly one physical extension link, multiplexed, so a kill is never
    // scoped to a single session. Re-touching each session's OWN already-owned tab (no new
    // group_request expected) must now fail with the truthful kill-switch error on both.
    write_line(
        &mut stdin_a,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":101,"url":"https://a.example.com"}}}),
    );
    write_line(
        &mut stdin_b,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"navigate","arguments":{"tabId":202,"url":"https://b.example.com"}}}),
    );
    let killed_a = read_line(&mut reader_a);
    let killed_b = read_line(&mut reader_b);
    assert_eq!(killed_a["id"], 3);
    assert_eq!(killed_b["id"], 3);
    for (label, reply) in [("A", &killed_a), ("B", &killed_b)] {
        assert_eq!(
            reply["result"]["isError"], true,
            "session {label} must observe the kill as a tool error: {reply:?}"
        );
        let text = reply["result"]["content"][0]["text"].as_str().unwrap_or("");
        assert!(
            text.contains("ended the browser session"),
            "session {label} must observe the truthful kill-switch text: {text}"
        );
    }

    drop(stdin_a);
    drop(stdin_b);
    let _ = adapter_a.wait();
    let _ = adapter_b.wait();
    let _ = service.kill();
    let _ = service.wait();
}
