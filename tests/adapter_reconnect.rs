// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ADR-0045: the resilient adapter reconnects across a SERVICE restart and replays the captured
//! MCP handshake, so the MCP client rides through a rebuild/upgrade/crash with NO reload.
//!
//! This spawns a SERVICE + a thin ADAPTER over a unique endpoint, drives the adapter over stdio,
//! kills and restarts the service on the SAME endpoint, and asserts that a NEW request on the SAME
//! adapter stdio still gets its reply -- and only its reply, never a duplicate of the replayed
//! `initialize` result (which the adapter swallows).
//!
//! Isolation notes: each run uses a unique IPC endpoint, a unique `GHOSTLIGHT_LOG_DIR` (so both
//! sides share the same anti-squat hub-key), AND a unique `GHOSTLIGHT_INSTANCE` so the adapter's
//! reconnect self-heal targets a guaranteed-nonexistent OS supervisor unit (a harmless failed
//! no-op) instead of this machine's real "Ghostlight Service".

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

/// The `ghostlight-adapter-agent` sibling of the `ghostlight` test binary (ADR-0046); built by
/// `cargo test --workspace` into the same `target/<profile>/` directory.
fn adapter_bin() -> PathBuf {
    let dir = Path::new(bin())
        .parent()
        .expect("the test binary has a parent directory");
    let name = if cfg!(windows) {
        "ghostlight-adapter-agent.exe"
    } else {
        "ghostlight-adapter-agent"
    };
    dir.join(name)
}

/// A fresh (endpoint, instance, log_dir) triple for one test run.
fn unique() -> (String, String, PathBuf) {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tag = format!("{}-{n}", std::process::id());
    let endpoint = format!("ghostlight-reconnect-{tag}");
    // A valid instance name (lowercase letters + digits): the self-heal unit becomes
    // "Ghostlight Service (reconN)" / "ghostlight-reconN.service", which never exists.
    let instance = format!("recon{n}");
    let log_dir = std::env::temp_dir().join(format!("ghostlight-reconnect-log-{tag}"));
    (endpoint, instance, log_dir)
}

fn service_cmd(endpoint: &str, instance: &str, log_dir: &Path) -> Command {
    let mut cmd = Command::new(bin());
    cmd.arg("service")
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

fn spawn_adapter(endpoint: &str, instance: &str, log_dir: &Path) -> Child {
    Command::new(adapter_bin())
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
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

#[test]
fn adapter_reconnects_across_a_service_restart_without_a_client_reload() {
    let (endpoint, instance, log_dir) = unique();
    let _ = std::fs::remove_dir_all(&log_dir);

    let mut service1 = service_cmd(&endpoint, &instance, &log_dir)
        .spawn()
        .expect("spawn service1");
    wait_for_state(&log_dir, Duration::from_secs(15));

    let mut adapter = spawn_adapter(&endpoint, &instance, &log_dir);
    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    let stdout = adapter.stdout.take().expect("adapter stdout");

    // Read replies on a thread so the test can bound its waits: a failed reconnect must fail fast,
    // not hang until the global test timeout.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Handshake + one request against service1.
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    );

    let init = recv(&rx, Duration::from_secs(10));
    assert_eq!(init["id"], 1, "initialize reply: {init:?}");
    let list1 = recv(&rx, Duration::from_secs(10));
    assert_eq!(list1["id"], 2, "pre-restart tools/list reply: {list1:?}");
    assert_eq!(
        list1["result"]["tools"].as_array().map(|t| t.len()),
        Some(17),
        "pre-restart tools/list: {list1:?}"
    );

    // Restart the service on the SAME endpoint. The adapter must detect the drop, reconnect
    // (self-healing the dial), and replay the captured handshake -- all transparently.
    let _ = service1.kill();
    let _ = service1.wait();
    let mut service2 = service_cmd(&endpoint, &instance, &log_dir)
        .spawn()
        .expect("spawn service2");

    // A NEW request on the SAME adapter stdio must get exactly its own reply -- proving the client
    // rode through the restart with no reload (ADR-0045), and that the replayed initialize result
    // was swallowed (the next reply the client sees is id 3, never a duplicate id 1).
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}),
    );
    let list2 = recv(&rx, Duration::from_secs(20));
    assert_eq!(
        list2["id"], 3,
        "post-restart reply must be id 3 (no leaked duplicate initialize result): {list2:?}"
    );
    assert_eq!(
        list2["result"]["tools"].as_array().map(|t| t.len()),
        Some(17),
        "the reconnected session answered a real request: {list2:?}"
    );

    // ADR-0047 D2 (PINS P3): the adapter mints ONE session identity for its whole process and
    // re-presents it on reconnect. Across every debug-events log this run produced, the mint note
    // appears exactly once (never once per connect) and at least one reconnect note is present.
    let mut events = String::new();
    for entry in std::fs::read_dir(&log_dir).expect("read log_dir") {
        let path = entry.expect("dir entry").path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("debug-events-") && name.ends_with(".jsonl") {
                events.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    assert_eq!(
        events
            .matches("session identity minted (stable for this adapter process)")
            .count(),
        1,
        "the adapter mints its session identity exactly once per process"
    );
    assert!(
        events
            .matches("service restart detected; reconnected")
            .count()
            >= 1,
        "at least one reconnect note must be present"
    );

    drop(stdin);
    let _ = adapter.wait();
    let _ = service2.kill();
    let _ = service2.wait();
    let _ = std::fs::remove_dir_all(&log_dir);
}

#[test]
fn adapter_survives_a_five_second_service_gap() {
    // Identical to the restart test above, but with a 5-second gap between kill and respawn --
    // WIDER than the old first-connect 3s self-heal window, so it exercises the ADR-0045 amendment
    // patient reconnect path (120s / 500ms). A rebuild-length gap (Ctrl-C -> cargo build -> rerun)
    // must not force a client reload.
    let (endpoint, instance, log_dir) = unique();
    let _ = std::fs::remove_dir_all(&log_dir);

    let mut service1 = service_cmd(&endpoint, &instance, &log_dir)
        .spawn()
        .expect("spawn service1");
    wait_for_state(&log_dir, Duration::from_secs(15));

    let mut adapter = spawn_adapter(&endpoint, &instance, &log_dir);
    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    let stdout = adapter.stdout.take().expect("adapter stdout");

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    );

    let init = recv(&rx, Duration::from_secs(10));
    assert_eq!(init["id"], 1, "initialize reply: {init:?}");
    let list1 = recv(&rx, Duration::from_secs(10));
    assert_eq!(list1["id"], 2, "pre-gap tools/list reply: {list1:?}");

    // Kill the service, then WAIT 5 seconds (past the old 3s first-connect window) before respawn.
    let _ = service1.kill();
    let _ = service1.wait();
    std::thread::sleep(Duration::from_secs(5));
    let mut service2 = service_cmd(&endpoint, &instance, &log_dir)
        .spawn()
        .expect("spawn service2");

    send(
        &mut stdin,
        &json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}),
    );
    let list2 = recv(&rx, Duration::from_secs(30));
    assert_eq!(
        list2["id"], 3,
        "post-gap reply must be id 3 (the adapter reconnected across a 5s gap): {list2:?}"
    );
    assert_eq!(
        list2["result"]["tools"].as_array().map(|t| t.len()),
        Some(17),
        "the reconnected session answered a real request: {list2:?}"
    );

    drop(stdin);
    let _ = adapter.wait();
    let _ = service2.kill();
    let _ = service2.wait();
    let _ = std::fs::remove_dir_all(&log_dir);
}
