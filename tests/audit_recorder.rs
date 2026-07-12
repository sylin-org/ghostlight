// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Integration test for the audit flight recorder (G06) at its public-API boundary: the
//! `Governance` facade wired to a file-backed `Recorder`, exactly as `transport::mcp::server`
//! wires it in production. Adapts the g06 spec's test 13
//! (`a_recorded_call_lands_as_one_wellformed_jsonl_line`) to the post-A3/A5 architecture, where
//! `set_client`/`record_call` live on `Governance`, not on `Recorder` directly (Recorder only
//! implements the bare `AuditSink::record`).

use ghostlight::browser::directory;
use ghostlight::governance::dispatch::Governance;
use ghostlight::governance::ports::AuditSink;
use serde_json::Value;
use std::sync::Arc;

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ghostlight-audit-recorder-test-{}-{tag}.jsonl",
        std::process::id()
    ))
}

#[test]
fn a_recorded_call_lands_as_one_wellformed_jsonl_line() {
    let path = temp_path("one-line");
    let _ = std::fs::remove_file(&path);

    let recorder = ghostlight::governance::audit::Recorder::to_file(path.clone());
    let governance = Governance::all_open(Arc::new(recorder) as Arc<dyn AuditSink>);

    governance.set_client("claude-code", "2.1.0");
    let mut audit = governance.begin(
        "computer",
        Some("left_click"),
        directory::requires("computer", Some("left_click")),
    );
    audit.dispatch_finished();
    audit.complete();

    let content = std::fs::read_to_string(&path).expect("audit file exists");
    assert!(content.ends_with('\n'), "file ends with a single LF");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one line after one recorded call");

    let rec: Value = serde_json::from_str(lines[0]).expect("line is a JSON object");
    let keys: Vec<&String> = rec
        .as_object()
        .expect("record is an object")
        .keys()
        .collect();
    assert_eq!(
        keys,
        vec![
            "event_id",
            "ts",
            "identity",
            "client",
            "tool",
            "action",
            "capability",
            "domain",
            "decision",
            "grant_id",
            "denial_id",
            "duration_ms",
            "manifest",
            "held",
            "orchestrator",
            "batch_id",
            "step",
            "dry_run"
        ],
        "field order matches the shared format"
    );

    assert_eq!(rec["tool"], "computer");
    assert_eq!(rec["action"], "left_click");
    assert_eq!(rec["capability"], "action");
    assert_eq!(rec["decision"], "allow");
    // t03 (ADR-0024 Decision 3): the two-phase API owns the clock (dispatch_finished/complete
    // derive the real elapsed time), so an injected literal duration is no longer expressible;
    // only presence is pinned here.
    assert!(rec["duration_ms"].as_u64().is_some());
    assert_eq!(rec["held"], false);
    assert_eq!(rec["client"]["name"], "claude-code");
    assert_eq!(rec["client"]["version"], "2.1.0");
    for field in ["identity", "domain", "grant_id", "denial_id", "manifest"] {
        assert!(rec[field].is_null(), "{field} must be null");
    }

    let event_id = rec["event_id"].as_str().expect("event_id is a string");
    assert_eq!(event_id.len(), 36, "event_id: {event_id}");
    for offset in [8, 13, 18, 23] {
        assert_eq!(event_id.as_bytes()[offset], b'-', "event_id: {event_id}");
    }
    let ts = rec["ts"].as_str().expect("ts is a string");
    assert_eq!(ts.len(), 24, "ts: {ts}");
    assert!(ts.ends_with('Z'), "ts: {ts}");
    chrono::DateTime::parse_from_rfc3339(ts).expect("ts parses as rfc3339");

    // Append, not truncate: a second call must add a second line.
    let mut audit2 = governance.begin("navigate", None, directory::requires("navigate", None));
    audit2.dispatch_finished();
    audit2.complete();
    let content = std::fs::read_to_string(&path).expect("audit file exists");
    assert_eq!(content.lines().count(), 2, "second call appends a line");

    std::fs::remove_file(&path).ok();
}

/// g11 test 2 (spec section 9): the kill hook, registered on `Browser` exactly as
/// `transport::mcp::server::run` registers it, writes exactly one well-formed session-event
/// audit line when the extension signals `session_killed` over a real duplex connection.
#[test]
fn session_killed_writes_one_session_event_record() {
    let path = temp_path("session-killed");
    let _ = std::fs::remove_file(&path);

    let recorder = ghostlight::governance::audit::Recorder::to_file(path.clone());
    let governance = Arc::new(Governance::all_open(
        Arc::new(recorder) as Arc<dyn AuditSink>
    ));
    governance.set_client("claude-code", "2.1.0");

    let browser = ghostlight::hub::outbound::browser::Browser::new();
    {
        let governance = Arc::clone(&governance);
        browser.on_session_killed(move || governance.record_session_killed());
    }

    let rt = tokio::runtime::Runtime::new().expect("build a tokio runtime");
    rt.block_on(async {
        let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
        let attached = browser.clone();
        tokio::spawn(async move {
            let _ = attached.attach(browser_side).await;
        });
        // ADR-0058: the first frame on this endpoint is now this session's hello.
        let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
        ghostlight::native::host::write_message(&mut ext_side, &hello)
            .await
            .unwrap();
        for _ in 0..200 {
            if browser.is_connected() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(browser.is_connected(), "browser never reported connected");

        let event = serde_json::json!({ "type": "session_killed" });
        ghostlight::native::host::write_message(
            &mut ext_side,
            &serde_json::to_vec(&event).unwrap(),
        )
        .await
        .unwrap();

        for _ in 0..200 {
            if browser.is_killed() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(browser.is_killed(), "the kill event was never routed");
    });

    let content = std::fs::read_to_string(&path).expect("audit file exists");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one session-event line");

    let rec: Value = serde_json::from_str(lines[0]).expect("line is a JSON object");
    assert_eq!(rec["event"], "session_killed");
    assert_eq!(rec["client"]["name"], "claude-code");
    let event_id = rec["event_id"].as_str().expect("event_id is a string");
    assert_eq!(event_id.len(), 36, "event_id: {event_id}");
    let ts = rec["ts"].as_str().expect("ts is a string");
    chrono::DateTime::parse_from_rfc3339(ts).expect("ts parses as rfc3339");
    for field in ["identity", "manifest"] {
        assert!(rec[field].is_null(), "{field} must be null");
    }
    for field in [
        "tool",
        "action",
        "capability",
        "domain",
        "decision",
        "grant_id",
        "denial_id",
        "duration_ms",
    ] {
        assert!(
            rec.get(field).is_none(),
            "{field} must not appear on a session event record"
        );
    }

    std::fs::remove_file(&path).ok();
}

/// C1 (PINS SS3): with no orchestration ever stamped, a normal begin/complete record's
/// serialized line ends with the four new keys, in order, all null/false.
#[test]
fn orchestration_keys_serialize_last_in_order() {
    let path = temp_path("orchestration-keys");
    let _ = std::fs::remove_file(&path);

    let recorder = ghostlight::governance::audit::Recorder::to_file(path.clone());
    let governance = Governance::all_open(Arc::new(recorder) as Arc<dyn AuditSink>);

    let mut audit = governance.begin(
        "computer",
        Some("left_click"),
        directory::requires("computer", Some("left_click")),
    );
    audit.dispatch_finished();
    audit.complete();

    let content = std::fs::read_to_string(&path).expect("audit file exists");
    let line = content.lines().next().expect("one line");
    assert!(
        line.ends_with(
            r#""held":false,"orchestrator":null,"batch_id":null,"step":null,"dry_run":false}"#
        ),
        "line: {line}"
    );

    std::fs::remove_file(&path).ok();
}

/// C1 (PINS SS3): `orchestrated`/`mark_dry_run`/`attribute_grant` each stamp their field, and
/// all three stick through to the completed record.
#[test]
fn orchestrated_setters_stamp_fields() {
    let path = temp_path("orchestrated-setters");
    let _ = std::fs::remove_file(&path);

    let recorder = ghostlight::governance::audit::Recorder::to_file(path.clone());
    let governance = Governance::all_open(Arc::new(recorder) as Arc<dyn AuditSink>);

    let mut audit = governance.begin(
        "computer",
        Some("left_click"),
        directory::requires("computer", Some("left_click")),
    );
    audit.orchestrated("script", "00000000-0000-4000-8000-000000000001", Some(3));
    audit.mark_dry_run();
    audit.attribute_grant(Some("g-1".to_string()));
    audit.complete();

    let content = std::fs::read_to_string(&path).expect("audit file exists");
    let line = content.lines().next().expect("one line");
    assert!(line.contains(r#""orchestrator":"script""#), "line: {line}");
    assert!(
        line.contains(r#""batch_id":"00000000-0000-4000-8000-000000000001""#),
        "line: {line}"
    );
    assert!(line.contains(r#""step":3"#), "line: {line}");
    assert!(line.contains(r#""dry_run":true"#), "line: {line}");
    assert!(line.contains(r#""grant_id":"g-1""#), "line: {line}");

    std::fs::remove_file(&path).ok();
}
