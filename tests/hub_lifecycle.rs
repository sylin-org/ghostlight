// SPDX-License-Identifier: Apache-2.0 OR MIT
//! H6 always-ready service + thin adapters + anti-squat tests (ADR-0030 Decision 8 amendment, "the
//! always-ready-service amendment"; `docs/tasks/hub/H6-detached-lifecycle-antisquat.md`; oracles
//! PINNED in `docs/tasks/hub/PINS.md` SS5 and SS8).
//!
//! 1. `service_survives_the_spawning_adapter_exit` -- the SERVICE's lifetime is independent of any
//!    one client (Decision 8: "shuts down on an idle-grace window ... never on parent-death").
//! 2. `adapter_cannot_complete_handshake_with_an_impostor_service` -- anti-squat (PINS.md SS5.3):
//!    an adapter that cannot read the SAME per-install `hub-key` the service used aborts with the
//!    pinned refusal text, never relaying.
//! 3. `supervisor_start_asserts_adapter_role` -- text-scan guarding the PINS.md SS8 wiring (the
//!    assertion LOGIC itself is guarded by `src/hub/role.rs`'s own unit tests).

mod support;

use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// ADR-0030 Decision 8: the standalone SERVICE's lifetime is independent of any one client. Kill
/// the ADAPTER that spawned no one (it is not this process's child in any OS-job sense, but it IS
/// the "spawning client" from the service's point of view) and confirm the SERVICE process is
/// still alive shortly after -- well within `IDLE_GRACE` (30s, PINS.md SS5.4).
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn service_survives_the_spawning_adapter_exit() {
    let endpoint = format!("ghostlight-hub-lifecycle-survive-{}", std::process::id());
    let mut service = support::spawn_service(&endpoint);
    let mut adapter = support::spawn_adapter(&endpoint);
    let service_pid = service.id();

    // Confirm the adapter actually holds a real, live session before killing it (else the kill
    // would prove nothing): drive one `initialize` through it and read back its reply.
    let mut stdin = adapter.stdin.take().expect("adapter stdin");
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
        .expect("write the initialize request");
    let stdout = adapter.stdout.take().expect("adapter stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("the adapter relays an initialize reply");
    let reply: Value =
        serde_json::from_str(line.trim_end()).expect("the reply is well-formed JSON");
    assert_eq!(reply["id"], 1, "the adapter session is real: {reply:?}");

    // Kill the ADAPTER (the spawning client) and reap it.
    let _ = adapter.kill();
    let _ = adapter.wait();

    // Give the OS a moment to have actually reaped the adapter, well within IDLE_GRACE (30s).
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        ghostlight::proc::pid_exists(service_pid),
        "the SERVICE must survive the spawning adapter's exit (ADR-0030 Decision 8)"
    );

    let _ = service.kill();
    let _ = service.wait();
}

/// ADR-0030 Decision 8 anti-squat (PINS.md SS5.3): the SERVICE proves possession of a per-install
/// secret before the ADAPTER relays a single byte. Here the ADAPTER is pointed at a genuinely
/// EMPTY `GHOSTLIGHT_LOG_DIR` of its own -- distinct from the real (impostor-standing-in-for-
/// genuine) SERVICE's -- so it can never read the SAME `hub-key` the service used to build its
/// proof (PINS.md SS5.3's "missing/unreadable key" failure mode collapses to the SAME refusal as
/// a genuine cross-install mismatch). The adapter must abort past the handshake, never relay, and
/// surface the PINNED refusal text.
#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn adapter_cannot_complete_handshake_with_an_impostor_service() {
    let endpoint = format!("ghostlight-hub-lifecycle-impostor-{}", std::process::id());
    let mut impostor = support::spawn_service(&endpoint);

    let adapter_log_dir =
        std::env::temp_dir().join(format!("ghostlight-test-logdir-{endpoint}-adapter-side"));
    let _ = std::fs::remove_dir_all(&adapter_log_dir);

    let mut adapter = Command::new(support::relay_bin())
        .arg("--role")
        .arg("agent")
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .env("GHOSTLIGHT_LOG_DIR", &adapter_log_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the adapter");

    let mut stderr = adapter.stderr.take().expect("adapter stderr");
    let stderr_reader = std::thread::spawn(move || {
        let mut captured = String::new();
        let _ = stderr.read_to_string(&mut captured);
        captured
    });

    // The adapter must abort on its own -- never relay, never hang -- within a few seconds (the
    // dial itself succeeds against the impostor, so self-heal's retry window is never entered;
    // only the anti-squat proof check fails).
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut exited = false;
    while Instant::now() < deadline {
        if let Ok(Some(_)) = adapter.try_wait() {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if !exited {
        let _ = adapter.kill();
    }
    let _ = adapter.wait();
    let captured = stderr_reader.join().expect("stderr reader thread panicked");

    let _ = impostor.kill();
    let _ = impostor.wait();
    let _ = std::fs::remove_dir_all(&adapter_log_dir);

    assert!(
        exited,
        "the adapter must abort on its own on an anti-squat mismatch, never hang: {captured}"
    );
    assert!(
        captured.contains(
            "refusing to connect: the Ghostlight service on this endpoint is not the one this user installed"
        ),
        "the adapter must surface the pinned anti-squat refusal text verbatim: {captured}"
    );
}

/// PINS.md SS8 wiring guard (text-scan, NOT a live-process test; mirrors
/// `tests/hub_role_wiring.rs`'s own pattern): `start_service`'s SS5.2 role assertion must actually
/// be present in the source. `src/hub/role.rs`'s own unit tests guard the assertion LOGIC.
#[test]
fn supervisor_start_asserts_adapter_role() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates")
        .join("transport")
        .join("src")
        .join("supervisor.rs");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(
        source.contains("assert_adapter_role"),
        "src/hub/supervisor.rs must call assert_adapter_role (PINS.md SS8 wiring guard)"
    );
}
