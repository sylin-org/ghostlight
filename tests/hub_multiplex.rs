// SPDX-License-Identifier: Apache-2.0 OR MIT
//! H2 multiplex tests (ADR-0030 Decision 1, Decision 2, Decision 3, Decision 7;
//! `docs/tasks/hub/H2-service-adapter-multiplex.md`).
//!
//! 1. `two_sessions_route_replies_independently` -- two sessions sharing ONE `Browser` never get
//!    each other's reply (Decision 2: the shared `Arc<AtomicU64>`/`Arc<Mutex<HashMap>>`
//!    correlation needs no new code for multiplex).
//! 2. `one_kill_emits_one_audit_record_per_live_session` -- the kill-hook fan-out (Decision 7)
//!    writes exactly one `session_killed` record per live session's subject.
//! 3. `adapter_endpoint_two_phase_wire_round_trips` -- the ADAPTER/CONTROL endpoint's wire is
//!    framed for the session-hello (and, since H6, the SERVICE's anti-squat proof) ONLY, then raw
//!    newline-delimited JSON-RPC (PINS.md SS1 pin 3, SS5.3).
//!
//! D (H6, forced): `adapter_endpoint_two_phase_wire_round_trips` is not named by H6's own task
//! file, but H6's argv-dispatch reshape makes a bare `ghostlight` invocation ALWAYS the thin
//! ADAPTER (never a role election), so the bare-invocation-becomes-the-service assumption this
//! test's spawn choreography relied on no longer holds; separately, H6 also inserts a NEW framed
//! anti-squat proof message between the hello and the raw phase, which this test's own hand-rolled
//! wire walk must now consume. Updated to spawn `ghostlight service` (via `support::spawn_service`)
//! and to read+consume that one new framed message, preserving every original assertion verbatim
//! (the two-phase framing shape, the raw reply's `id` echo). Impact on later tasks: none -- H7/H8's
//! own tests should follow the SAME `support::spawn_service`/`spawn_adapter` pattern.

mod support;

use ghostlight::governance::audit::Recorder;
use ghostlight::governance::dispatch::Governance;
use ghostlight::governance::ports::AuditSink;
use ghostlight::hub::outbound::browser::Browser;
use ghostlight::native::host;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-hub-multiplex-test-{}-{tag}.jsonl",
        std::process::id()
    ))
}

/// ADR-0030 Decision 2: two sessions sharing ONE `Browser` (one `.clone()` each, standing in for
/// two multiplexed `serve_session` callers) must never receive each other's reply. Both calls are
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
    // exact pattern `browser.rs::call_round_trips_a_tool_response` uses for a single session.
    let fake_ext = tokio::spawn(async move {
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

    // Session A and session B: two independent clones of the ONE Browser.
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

/// ADR-0030 Decision 7: the kill-hook fan-out registry writes exactly one `session_killed`
/// session-event record per LIVE session's subject on a single kill. Three sessions (each an
/// all-open `Governance` with a distinct client name, per PINS.md's resolved index: "use
/// `Governance::all_open` + `set_client(name, version)` as today") register via the NEW
/// `register_session_kill_hook` on the ONE shared `Browser`; a single `session_killed` frame must
/// produce exactly three records, none cross-written, each with the 6-key `SessionEventRecord`
/// order transcribed verbatim from ADR-0030 "Preserved invariants".
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[tokio::test]
async fn one_kill_emits_one_audit_record_per_live_session() {
    let names = ["client-a", "client-b", "client-c"];
    let browser = Browser::new();

    let mut paths = Vec::new();
    let mut handles = Vec::new();
    for name in names {
        let path = temp_path(name);
        let _ = std::fs::remove_file(&path);

        let recorder = Recorder::to_file(path.clone());
        let governance = Arc::new(Governance::all_open(
            Arc::new(recorder) as Arc<dyn AuditSink>
        ));
        governance.set_client(name, "1.0.0");

        let handle = {
            let governance = Arc::clone(&governance);
            browser.register_session_kill_hook(move || governance.record_session_killed())
        };

        paths.push(path);
        handles.push(handle);
    }

    let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
    let attached = browser.clone();
    tokio::spawn(async move {
        let _ = attached.attach(browser_side).await;
    });
    for _ in 0..200 {
        if browser.is_connected() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(browser.is_connected(), "browser never reported connected");

    host::write_message(
        &mut ext_side,
        &serde_json::to_vec(&json!({ "type": "session_killed" })).unwrap(),
    )
    .await
    .unwrap();

    for _ in 0..200 {
        if browser.is_killed() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(browser.is_killed(), "the kill event was never routed");
    // The hook fan-out runs synchronously inside the false->true transition, on the same task
    // that routed the event; give it a moment to finish writing all three files before reading
    // them back (the same grace `browser.rs::kill_hook_fires_exactly_once_per_transition` gives
    // a possible second invocation before asserting).
    tokio::time::sleep(Duration::from_millis(50)).await;

    for (name, path) in names.iter().zip(paths.iter()) {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("audit file for {name} exists: {e}"));
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "exactly one session-event line for {name}: {content:?}"
        );

        let rec: Value = serde_json::from_str(lines[0]).expect("line is a JSON object");
        let keys: Vec<&String> = rec
            .as_object()
            .expect("record is an object")
            .keys()
            .collect();
        assert_eq!(
            keys,
            vec!["event_id", "ts", "identity", "client", "event", "manifest"],
            "field order matches the 6-key SessionEventRecord order (ADR-0030 pinned oracle)"
        );
        assert_eq!(rec["event"], "session_killed");
        assert_eq!(rec["client"]["name"], *name);
    }

    drop(handles);
    for path in paths {
        std::fs::remove_file(path).ok();
    }
}

/// PINS.md SS1 pin 3 + SS5.3: the ADAPTER/CONTROL endpoint's wire is framed for the session-hello
/// and (since H6) the SERVICE's anti-squat proof ONLY; everything after is RAW newline-delimited
/// JSON-RPC. A framed data copy would corrupt every multiplexed session's JSON-RPC, so this fences
/// exactly that trap. H6 (forced, see the module doc's "D" note): spawns the real, standalone
/// `ghostlight service` (`support::spawn_service`) instead of a bare invocation (H6's argv dispatch
/// makes a bare invocation ALWAYS the thin ADAPTER, never a role election); no fake extension is
/// needed since `initialize` never calls the browser.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn adapter_endpoint_two_phase_wire_round_trips() {
    let endpoint = format!(
        "ghostlight-hub-adapter-wire-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let mut service = support::spawn_service(&endpoint);

    let rt = tokio::runtime::Runtime::new().expect("build a tokio runtime");
    rt.block_on(async {
        let adapter_endpoint = format!("{endpoint}-adapter");
        let mut stream = ghostlight::native::ipc::connect(&adapter_endpoint)
            .await
            .expect("connect to the service's adapter/control endpoint");

        // Phase 1: the session-hello, FRAMED (PINS.md SS1 pin 3). H3 sanctioned fix (item 3,
        // "SANCTIONED TEST FIX"): this test exercises the two-phase wire mechanics, not guid
        // validity (`tests/hub_identity.rs` covers that separately), so the placeholder empty
        // guid PINS.md SS1 originally anticipated ("before H3 an empty placeholder guid is
        // acceptable and H3 fills it") is replaced with a well-formed v4 UUID literal so this
        // test continues to exercise successful admission once H3's parse-failure refusal lands.
        let hello =
            json!({ "hub": 1, "role": "adapter", "guid": "00000000-0000-4000-8000-000000000000" });
        host::write_message(&mut stream, &serde_json::to_vec(&hello).unwrap())
            .await
            .expect("write the framed session-hello");

        // H6 (PINS.md SS5.3): the SERVICE now sends ONE more FRAMED message -- its anti-squat
        // proof -- before the raw phase begins. Consume it here (a well-formed service-proof
        // frame); its MAC verification is exercised for real by
        // `tests/hub_lifecycle.rs`'s anti-squat tests, not this one.
        let proof_bytes = host::read_message(&mut stream)
            .await
            .expect("read the framed service-proof")
            .expect("the service sends a service-proof frame before the raw phase");
        let proof: Value =
            serde_json::from_slice(&proof_bytes).expect("the proof frame is well-formed JSON");
        assert_eq!(
            proof["role"], "service-proof",
            "the framed message after the hello is the service-proof: {proof:?}"
        );

        // Phase 2: RAW newline-delimited JSON-RPC -- never framed.
        let request = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n";
        stream
            .write_all(request)
            .await
            .expect("write the raw JSON-RPC request line");

        // Read back ONE raw newline-delimited line (never via `host::read_message`): proves the
        // data phase is raw on both sides.
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .expect("read one raw JSON-RPC reply line");

        let reply: Value = serde_json::from_str(line.trim_end())
            .expect("the raw line is well-formed JSON, not length-prefixed");
        assert_eq!(
            reply["id"], 1,
            "the raw JSON-RPC reply echoes the request id: {reply:?}"
        );
    });

    let _ = service.kill();
    let _ = service.wait();
}
