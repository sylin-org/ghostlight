// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ADR-0048: the development override. An UNPINNED adapter resolves its service at connect time,
//! walking an ordered candidate list (dev then default in production; test-unique endpoints here
//! via the GHOSTLIGHT_ENDPOINTS seam) -- preferring the first live candidate and falling back to
//! the next, both at first connect and on every reconnect episode.
//!
//! Isolation notes: everything runs on per-run unique endpoints (never the machine's real
//! `org.sylin.ghostlight*` names); one shared GHOSTLIGHT_LOG_DIR gives every process the same
//! anti-squat hub-key; unique GHOSTLIGHT_INSTANCE names per service make `serverInfo.name` a
//! which-service-answered oracle (`ghostlight-<instance>`); the adapter also carries a per-run
//! instance so its self-heal supervisor kick targets a guaranteed-nonexistent unit (a harmless
//! failed no-op) instead of this machine's real "Ghostlight Service" -- GHOSTLIGHT_ENDPOINTS
//! outranks the selection, so the candidate walk is still fully exercised.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ghostlight")
}

/// The `ghostlight-relay` sibling of the `ghostlight` test binary (ADR-0046 + ADR-0051 Phase 3);
/// built by `cargo test --workspace` into the same `target/<profile>/` directory. Launched with
/// `--role agent` for the MCP-side pass-through.
fn adapter_bin() -> PathBuf {
    let dir = Path::new(bin())
        .parent()
        .expect("the test binary has a parent directory");
    let name = if cfg!(windows) {
        "ghostlight-relay.exe"
    } else {
        "ghostlight-relay"
    };
    dir.join(name)
}

/// A fresh (endpoint_a, endpoint_b, instance_a, instance_b, log_dir) set for one test run.
fn unique() -> (String, String, String, String, PathBuf) {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tag = format!("{}-{n}", std::process::id());
    (
        format!("ghostlight-override-a-{tag}"),
        format!("ghostlight-override-b-{tag}"),
        format!("ovra{n}"),
        format!("ovrb{n}"),
        std::env::temp_dir().join(format!("ghostlight-override-log-{tag}")),
    )
}

fn service_cmd(endpoint: &str, instance: &str, log_dir: &Path) -> Command {
    let mut cmd = Command::new(bin());
    cmd.args(["service", "--keep-warm"])
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_INSTANCE", instance)
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

/// Wait until `log_dir` holds a parseable `debug-state-*.json` (the service wrote its first
/// snapshot), or panic after `within`.
fn wait_for_state(log_dir: &Path, within: Duration) {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Ok(entries) = std::fs::read_dir(log_dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with("debug-state-") && name.ends_with(".json") {
                    let has_pid = std::fs::read_to_string(e.path())
                        .ok()
                        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                        .and_then(|v| v.get("pid").and_then(Value::as_u64))
                        .is_some();
                    if has_pid {
                        return;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("no service debug-state within {within:?}");
}

/// Wait until `log_dir` holds at least `count` parseable `debug-state-*.json` files (one per
/// live service), so a second service is provably up before the adapter is spawned.
fn wait_for_states(log_dir: &Path, count: usize, within: Duration) {
    let deadline = std::time::Instant::now() + within;
    loop {
        let n = std::fs::read_dir(log_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        name.starts_with("debug-state-") && name.ends_with(".json")
                    })
                    .filter(|e| {
                        std::fs::read_to_string(e.path())
                            .ok()
                            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                            .is_some()
                    })
                    .count()
            })
            .unwrap_or(0);
        if n >= count {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "expected {count} debug-state files under {} within {within:?}",
            log_dir.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Spawn the agent adapter on the candidate-list seam (ADR-0048 D2's GHOSTLIGHT_ENDPOINTS),
/// which outranks the instance selection -- so the ordered candidate walk is exercised while
/// GHOSTLIGHT_INSTANCE pins the self-heal supervisor target to a unit that never exists on this
/// machine (a harmless failed no-op, never the real "Ghostlight Service" task).
fn spawn_adapter(endpoints: &[String], instance: &str, log_dir: &Path) -> Child {
    Command::new(adapter_bin())
        .arg("--role")
        .arg("agent")
        .env("GHOSTLIGHT_ENDPOINTS", endpoints.join(","))
        .env_remove("GHOSTLIGHT_ENDPOINT")
        .env("GHOSTLIGHT_INSTANCE", instance)
        .env("GHOSTLIGHT_LOG_DIR", log_dir)
        .env("GHOSTLIGHT_DEBUG", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight adapter")
}

fn send(stdin: &mut std::process::ChildStdin, v: &Value) {
    stdin
        .write_all(serde_json::to_string(v).unwrap().as_bytes())
        .unwrap();
    stdin.write_all(b"\n").unwrap();
    stdin.flush().unwrap();
}

fn recv(rx: &Receiver<String>, within: Duration) -> Value {
    match rx.recv_timeout(within) {
        Ok(line) => serde_json::from_str(&line).expect("each reply line is JSON"),
        Err(RecvTimeoutError::Timeout) => panic!("no reply within {within:?}"),
        Err(RecvTimeoutError::Disconnected) => {
            panic!("the adapter's stdout closed (it exited instead of reconnecting)")
        }
    }
}

/// Forward the adapter's stdout lines over a channel so `recv` can timeout (transcribed from
/// tests/adapter_reconnect.rs's inline reader).
fn spawn_reader(stdout: std::process::ChildStdout) -> Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

/// ADR-0048 D3: with BOTH candidates live, an unpinned adapter connects to the FIRST (the dev
/// slot); when that service dies, the reconnect episode fails over to the SECOND (the default
/// slot) without a client reload -- and the debug events record both resolutions.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn unpinned_adapter_prefers_the_first_candidate_and_fails_over() {
    let (ep_a, ep_b, inst_a, inst_b, log_dir) = unique();
    let _ = std::fs::remove_dir_all(&log_dir);

    let mut service_a = service_cmd(&ep_a, &inst_a, &log_dir)
        .spawn()
        .expect("spawn service A");
    wait_for_state(&log_dir, Duration::from_secs(15));
    let mut service_b = service_cmd(&ep_b, &inst_b, &log_dir)
        .spawn()
        .expect("spawn service B");
    wait_for_states(&log_dir, 2, Duration::from_secs(15));

    let mut adapter = spawn_adapter(&[ep_a.clone(), ep_b.clone()], &inst_a, &log_dir);
    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    let rx = spawn_reader(adapter.stdout.take().expect("adapter stdout"));

    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    let init = recv(&rx, Duration::from_secs(20));
    assert_eq!(
        init["result"]["serverInfo"]["name"],
        format!("ghostlight-{inst_a}"),
        "with both candidates live, the FIRST wins: {init:?}"
    );
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );

    // Kill the preferred service: the reconnect episode must fail over to the second candidate.
    let _ = service_a.kill();
    let _ = service_a.wait();
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}),
    );
    let list = recv(&rx, Duration::from_secs(30));
    assert_eq!(
        list["id"], 3,
        "the post-failover reply answers the new request: {list:?}"
    );
    assert_eq!(
        list["result"]["tools"].as_array().map(|t| t.len()),
        Some(ghostlight::browser::directory::advertised_tool_count()),
        "the fallback service answered a real request: {list:?}"
    );

    // The adapter's debug events recorded both resolutions.
    let mut events = String::new();
    for entry in std::fs::read_dir(&log_dir).expect("read log_dir") {
        let path = entry.expect("dir entry").path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("debug-events-") && name.ends_with(".jsonl") {
                events.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    assert!(
        events
            .matches("override resolution: connected to candidate 1/2")
            .count()
            >= 1,
        "the first connect resolved to candidate 1"
    );
    assert!(
        events
            .matches("override resolution: connected to candidate 2/2")
            .count()
            >= 1,
        "the failover resolved to candidate 2"
    );

    drop(stdin);
    let _ = adapter.wait();
    let _ = service_b.kill();
    let _ = service_b.wait();
    let _ = std::fs::remove_dir_all(&log_dir);
}

/// ADR-0048 D3: when the FIRST candidate is absent, an unpinned adapter falls back to the
/// SECOND on the fast path (an absent pipe fails the dial instantly; no retry window burned).
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn unpinned_adapter_falls_back_when_the_first_candidate_is_absent() {
    let (ep_a, ep_b, _inst_a, inst_b, log_dir) = unique();
    let _ = std::fs::remove_dir_all(&log_dir);

    // Only B runs; A's endpoint is never served.
    let mut service_b = service_cmd(&ep_b, &inst_b, &log_dir)
        .spawn()
        .expect("spawn service B");
    wait_for_state(&log_dir, Duration::from_secs(15));

    let mut adapter = spawn_adapter(&[ep_a.clone(), ep_b.clone()], &inst_b, &log_dir);
    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    let rx = spawn_reader(adapter.stdout.take().expect("adapter stdout"));

    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    let init = recv(&rx, Duration::from_secs(20));
    assert_eq!(
        init["result"]["serverInfo"]["name"],
        format!("ghostlight-{inst_b}"),
        "with the first candidate absent, the second wins: {init:?}"
    );

    drop(stdin);
    let _ = adapter.wait();
    let _ = service_b.kill();
    let _ = service_b.wait();
    let _ = std::fs::remove_dir_all(&log_dir);
}
