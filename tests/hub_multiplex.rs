// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Shared browser-shore correlation tests (ADR-0030, retained by ADR-0096).
//!
//! `two_sessions_route_replies_independently` -- two workspace calls sharing ONE `Browser` never get
//!    each other's reply (Decision 2: the shared `Arc<AtomicU64>`/`Arc<Mutex<HashMap>>`
//!    correlation needs no new code for multiplex).
//!
//! Process-boundary kill fan-out and two-phase wire coverage lives in the ADR-0056 Lightbox
//! scenario library.

use ghostlight::hub::outbound::browser::Browser;
use ghostlight::native::host;
use serde_json::{json, Value};
use std::time::Duration;

/// Two workspace calls sharing ONE `Browser` (one `.clone()` each) must never receive each other's
/// reply. Both calls are
/// framed and routed through the SAME `next_id`/`pending` map the `Browser` already carries as
/// `Arc` fields across clones -- multiplex needs no new correlation code.
#[tokio::test]
async fn two_sessions_route_replies_independently() {
    let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
    let browser = Browser::new();

    let attached = browser.clone();
    tokio::spawn(async move {
        let _ = attached.attach(browser_side).await;
    });

    // Fake extension: reads TWO framed requests (in whichever order they arrive on the one
    // shared physical link) and replies to each by id, echoing its own tool name back -- the
    // exact pattern `browser.rs::call_round_trips_a_tool_response` uses for a single call.
    let fake_ext = tokio::spawn(async move {
        // ADR-0058/0061: relay hello then the extension identity frame; plain un-encoded small
        // tabIds decode to slot 0, which resolve_target routes to this sole focus-front browser.
        let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
        host::write_message(&mut ext_side, &hello).await.unwrap();
        let identity = serde_json::to_vec(&json!({
            "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
            ghostlight_transport::handshake::BROWSER_ID_FIELD: "hub-multiplex-fixture",
        }))
        .unwrap();
        host::write_message(&mut ext_side, &identity).await.unwrap();
        for _ in 0..2 {
            let req = host::read_message(&mut ext_side).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({ "id": v["id"], "type": "tool_response", "result": { "echoed": v["tool"] } });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        }
    });

    for _ in 0..200 {
        if browser.is_connected() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(browser.is_connected(), "browser never reported connected");

    // Workspace A and workspace B: two independent clones of the ONE Browser.
    let session_a = browser.clone();
    let session_b = browser.clone();
    let args_a = json!({});
    let args_b = json!({});
    let (result_a, result_b) = tokio::join!(
        session_a.call("session-a", "navigate", &args_a),
        session_b.call("session-b", "find", &args_b)
    );

    let result_a = result_a.expect("session A's call succeeds");
    let result_b = result_b.expect("session B's call succeeds");

    assert_eq!(
        result_a,
        json!({ "echoed": "navigate" }),
        "session A gets its OWN reply, never session B's"
    );
    assert_eq!(
        result_b,
        json!({ "echoed": "find" }),
        "session B gets its OWN reply, never session A's"
    );

    fake_ext.await.unwrap();
}
