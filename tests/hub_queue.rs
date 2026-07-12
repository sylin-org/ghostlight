// SPDX-License-Identifier: Apache-2.0 OR MIT
//! H5 reconnect grace window + honest bounded queue tests (ADR-0030 Decision 3, "D1 -- the
//! honest singleton queue"; `docs/tasks/hub/H5-grace-window-honest-queue.md`; oracles PINNED in
//! `docs/tasks/hub/PINS.md` SS4).
//!
//! Decision 3 (verbatim excerpt, ADR-0030): "fair ordering, truthful failure on a real drop,
//! per-peer-identity mint/group quotas (never a single global cap, which is itself a lockout
//! DoS), and MANDATORY screenshot chunking so a large payload ... cannot head-of-line-block the
//! shared port and starve honest sessions. We do not engineer around the singleton; we queue
//! honestly."
//!
//! The bounded reconnect grace window itself (`hub::outbound::browser::Browser::attach`,
//! `GRACE_WINDOW`) is exercised by `src/hub/outbound/browser.rs`'s own inline tests (not named by
//! the task file for this integration suite); this file covers the two tests the task file DOES
//! name by their exact names:
//!
//! 1. `per_peer_mint_cap_denies_a_flooding_peer_without_locking_out_others` -- a per-peer (never
//!    global) mint quota keyed on the peer credential.
//! 2. `oversized_screenshot_is_chunked_not_head_of_line_blocking` -- an oversize reply is relayed
//!    in more than one `write_all` call, and a concurrent small call is never head-of-line-blocked
//!    behind it.

use ghostlight::governance::audit::Recorder;
use ghostlight::governance::manifest::source::LoadedPolicy;
use ghostlight::hub::outbound::browser::Browser;
use ghostlight::hub::session::{PeerUser, SessionGuid, SessionRegistry};
use ghostlight::hub::{try_mint, ServiceContext, MINT_QUOTA_EXCEEDED, PER_PEER_MINT_CAP};
use ghostlight::native::host;
use ghostlight::transport::mcp::server::serve_session;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// `serve_session` asserts `crate::hub::role::assert_service_role` as its first line (PINS.md
/// SS8): this test drives it directly (never through `run_service`/`run_service_loop`), so it must set the
/// ONE-per-process role marker itself, exactly once for the whole test binary (mirrors
/// `tests/hub_isolation.rs`'s own `ensure_service_role`).
static SET_SERVICE_ROLE: Once = Once::new();
fn ensure_service_role() {
    SET_SERVICE_ROLE.call_once(|| {
        ghostlight::hub::role::set_role(ghostlight::hub::role::Role::Service);
    });
}

/// Build a `ServiceContext` for the test (mirrors `tests/hub_isolation.rs`'s own `build_ctx`): a
/// fresh `Browser`, an all-open `LoadedPolicy`, the real `ConfigStore::load_initial` resolution, a
/// disabled `Recorder`, and fresh H3/H4/H5 shared tables.
fn build_ctx(browser: Browser) -> ServiceContext {
    let store = ghostlight::governance::config::reload::ConfigStore::load_initial(
        ghostlight::browser::pattern::is_valid_pattern,
    )
    .expect("load_initial resolves to all-open with no manifest present");
    ServiceContext {
        capabilities: ghostlight::hub::outbound::Registry::new(vec![std::sync::Arc::new(
            ghostlight::hub::outbound::browser::BrowserCapability::new(browser.clone()),
        )]),
        browser,
        store,
        recorder: Arc::new(Recorder::disabled()),
        initial_policy: LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        },
        session_registry: Arc::new(Mutex::new(SessionRegistry::new())),
        owned_tabs: Arc::new(Mutex::new(HashMap::new())),
        session_titles: Arc::new(Mutex::new(HashMap::new())),
        live_guids: Arc::new(Mutex::new(HashMap::new())),
        mint_quota: Arc::new(Mutex::new(HashMap::new())),
        live_sessions: Arc::new(AtomicUsize::new(0)),
        debug_sink: ghostlight::observability::DebugSink::disabled(),
    }
}

async fn wait_connected(browser: &Browser) {
    for _ in 0..200 {
        if browser.is_connected() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("browser never reported connected");
}

/// Attach a fake extension answering exactly ONE tool with the given canned result (mirrors the
/// `attach_fake_extension` pattern already established in `tests/hub_isolation.rs` and
/// `src/transport/mcp/pipeline.rs`'s own tests).
fn attach_fake_extension(
    browser: &Browser,
    tool: &'static str,
    result: Value,
) -> tokio::task::JoinHandle<()> {
    let (browser_side, mut ext_side) = tokio::io::duplex(32 * 1024 * 1024);
    let attached = browser.clone();
    tokio::spawn(async move {
        let _ = attached.attach(browser_side).await;
    });
    tokio::spawn(async move {
        // ADR-0058/0061: relay hello then the extension identity frame; plain un-encoded small
        // tabIds decode to slot 0, which resolve_target routes to this sole focus-front browser.
        let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
        host::write_message(&mut ext_side, &hello).await.unwrap();
        let identity = serde_json::to_vec(&serde_json::json!({
            "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
            ghostlight_transport::handshake::BROWSER_ID_FIELD: "hub-queue-fixture",
        }))
        .unwrap();
        host::write_message(&mut ext_side, &identity).await.unwrap();
        loop {
            let Some(req) = host::read_message(&mut ext_side).await.unwrap() else {
                break;
            };
            let v: Value = serde_json::from_slice(&req).unwrap();
            let frame_type = v["type"].as_str().unwrap_or_default();
            // o04: a TabScoped tool call with a tabId triggers lazy extension probes
            // (tab_url_request, request_group) for audit attribution even under all-open. Answer
            // any non-tool frame generically so the call proceeds to the actual tool dispatch
            // under test; only the tool_request carries the oversize reply this test measures.
            if frame_type == "tool_request" {
                let seen_tool = v["tool"].as_str().unwrap_or_default();
                assert_eq!(seen_tool, tool, "unexpected tool_request for '{seen_tool}'");
                let reply = json!({ "id": v["id"], "type": "tool_response", "result": result });
                host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                    .await
                    .unwrap();
            } else {
                // Echo the frame's id with a benign response shaped to its type.
                let (resp_type, result) = match frame_type {
                    "tab_url_request" => {
                        ("tab_url_response", json!({ "url": "https://example.com/" }))
                    }
                    "request_group" | "group_request" => {
                        ("group_response", json!({ "groupId": 1, "tabs": [] }))
                    }
                    other => {
                        panic!("fake extension cannot answer frame type '{other}'; full frame: {v}")
                    }
                };
                let reply = json!({ "id": v["id"], "type": resp_type, "result": result });
                host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                    .await
                    .unwrap();
            }
        }
    })
}

async fn write_line<W: AsyncWrite + Unpin>(w: &mut W, value: &Value) {
    let mut line = serde_json::to_string(value).expect("value serializes");
    line.push('\n');
    w.write_all(line.as_bytes())
        .await
        .expect("write the raw JSON-RPC request line");
}

/// `tests/hub_queue.rs::per_peer_mint_cap_denies_a_flooding_peer_without_locking_out_others`
/// (task file, BY NAME).
///
/// Pinned assertions (task file + PINS.md SS4, transcribed):
/// - Peer A mints up to `PER_PEER_MINT_CAP` (32) successfully; its next mint is denied with the
///   pinned quota tool-error text `session limit reached for this client` (assertion 1).
/// - Peer B, a distinct peer, mints and is served successfully while A is over its cap --
///   proving the cap is per-peer, never global (assertion 2).
#[test]
fn per_peer_mint_cap_denies_a_flooding_peer_without_locking_out_others() {
    let mint_quota: ghostlight::hub::MintQuota = Arc::new(Mutex::new(HashMap::new()));
    let peer_a = PeerUser("peer-a".to_string());
    let peer_b = PeerUser("peer-b".to_string());

    let mut held = Vec::new();
    for _ in 0..PER_PEER_MINT_CAP {
        let guard = try_mint(&mint_quota, &peer_a)
            .expect("peer A must be admitted for every mint up to its cap");
        held.push(guard);
    }

    // Pinned assertion 1: A's over-cap mint is denied with the pinned quota text, verbatim.
    let denied = try_mint(&mint_quota, &peer_a);
    assert_eq!(
        denied.err(),
        Some(MINT_QUOTA_EXCEEDED.to_string()),
        "A's over-cap mint result must equal the pinned quota tool-error text"
    );

    // Pinned assertion 2: peer B, a distinct peer, is unaffected and still succeeds while A is
    // over its cap -- the cap is per-peer, never global.
    let guard_b = try_mint(&mint_quota, &peer_b);
    assert!(
        guard_b.is_ok(),
        "a different peer must not be locked out by A's cap"
    );

    // Freeing one of A's slots (a session ending) must let A mint again -- the cap counts
    // CONCURRENT sessions, not lifetime mints.
    held.pop();
    assert!(
        try_mint(&mint_quota, &peer_a).is_ok(),
        "freeing a slot must let the same peer mint again"
    );
}

/// A wrapped `AsyncWrite` that counts every `poll_write` invocation, so a test can observe the
/// NUMBER of underlying write calls a relay makes -- not just the bytes eventually delivered
/// (chunked and unchunked writes are byte-identical on the wire; PINS.md SS9's clarification for
/// this exact test).
struct CountingWriter<W> {
    inner: W,
    calls: Arc<AtomicUsize>,
}

impl<W: AsyncWrite + Unpin> AsyncWrite for CountingWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        this.calls.fetch_add(1, Ordering::SeqCst);
        Pin::new(&mut this.inner).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

/// `tests/hub_queue.rs::oversized_screenshot_is_chunked_not_head_of_line_blocking` (task file, BY
/// NAME).
///
/// Two concurrent sessions through the hub, each its own `Browser`/`ServiceContext` (H5's
/// chunking is a property of EACH session's own writer task, PINS.md SS9: "the actual relay to
/// the outside world is each session's OWN writer task ... Sessions already run as independent
/// tokio tasks"). Session 1's fake extension answers one `computer` (`screenshot`) call with a
/// reply `>= SCREENSHOT_CHUNK_THRESHOLD`; session 1's own server-side stream write half is
/// wrapped in [`CountingWriter`] so the test observes write CALLS. Session 2 issues a bare `ping`
/// (no extension involvement at all) concurrently. `current_thread` (a single OS thread) is the
/// flavor that actually needs an explicit yield between chunks to let session 2's task run in
/// between -- the property H5 pins.
///
/// Pinned assertions (task file + PINS.md SS4, transcribed):
/// - Session 2's `ping` completes in `< 2s` (PINNED in PINS.md SS4), well under the 60s
///   `TOOL_TIMEOUT`, while session 1's oversize reply may still be relaying.
/// - Session 1's oversize reply is delivered in `> 1` `write_all`/`poll_write` call.
#[tokio::test(flavor = "current_thread")]
async fn oversized_screenshot_is_chunked_not_head_of_line_blocking() {
    ensure_service_role();

    // Session 1: a fake extension answering ONE `computer` (`screenshot`) call with an oversize
    // (>= SCREENSHOT_CHUNK_THRESHOLD = 8 MiB) text payload.
    let browser1 = Browser::new();
    let huge_text = "x".repeat(9 * 1024 * 1024);
    let _fake_ext1 = attach_fake_extension(
        &browser1,
        "computer",
        json!({ "content": [{ "type": "text", "text": huge_text }] }),
    );
    wait_connected(&browser1).await;

    let ctx1 = build_ctx(browser1);
    let guid1 = SessionGuid::mint();

    let (mut client_1, server_1) = tokio::io::duplex(32 * 1024 * 1024);
    let (server_1_read, server_1_write) = tokio::io::split(server_1);
    let write_calls = Arc::new(AtomicUsize::new(0));
    let counting_write = CountingWriter {
        inner: server_1_write,
        calls: Arc::clone(&write_calls),
    };
    let wrapped_1 = tokio::io::join(server_1_read, counting_write);
    tokio::spawn(async move {
        let _ = serve_session(wrapped_1, ctx1, guid1).await;
    });

    // Session 2: a plain, small session -- no extension needed for a bare `ping`.
    let browser2 = Browser::new();
    let ctx2 = build_ctx(browser2);
    let guid2 = SessionGuid::mint();
    let (client_2, server_2) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        let _ = serve_session(server_2, ctx2, guid2).await;
    });
    let mut reader_2 = BufReader::new(client_2);

    // Trigger session 1's oversize reply. `client_1` is held (never read, never dropped) for the
    // whole test so the server side never sees a broken pipe.
    write_line(
        &mut client_1,
        &json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            // o04 (ADR-0031 Decision 4): inputSchema validation now runs before dispatch; computer
            // needs action + tabId.
            "params": { "name": "computer", "arguments": { "action": "screenshot", "tabId": 1 } },
        }),
    )
    .await;

    // Session 2's small, honest call, concurrent with session 1's still-relaying reply.
    let start = Instant::now();
    write_line(
        reader_2.get_mut(),
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }),
    )
    .await;
    let mut reply_line = String::new();
    reader_2
        .read_line(&mut reply_line)
        .await
        .expect("session 2's ping reply");
    let elapsed = start.elapsed();
    let reply2: Value =
        serde_json::from_str(reply_line.trim_end()).expect("session 2's reply is well-formed");
    assert_eq!(reply2["id"], 1, "session 2's own reply: {reply2:?}");
    assert!(
        elapsed < Duration::from_secs(2),
        "PINS.md SS4: session 2's small call must complete in < 2s (well under TOOL_TIMEOUT), \
         not be head-of-line-blocked behind session 1's oversize reply; took {elapsed:?}"
    );

    // Let session 1's oversize relay settle, then confirm it took MORE than one write call --
    // proof of the mandatory chunking (ADR-0030 Decision 3; PINS.md SS4), never a single
    // unchunked write for a payload >= SCREENSHOT_CHUNK_THRESHOLD.
    for _ in 0..500 {
        if write_calls.load(Ordering::SeqCst) > 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(
        write_calls.load(Ordering::SeqCst) > 1,
        "an oversize reply must be relayed in more than one write_all call, got {}",
        write_calls.load(Ordering::SeqCst)
    );
}
