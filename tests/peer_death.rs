// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Regression test for the IPC peer-death defect.
//!
//! Before the tokio-native transport migration, a force-killed peer left the native-host as a
//! zombie: the `interprocess` crate's async Windows named-pipe read never woke on peer death, so
//! the relay never observed the disconnect. This test spawns the real, standalone SERVICE
//! (ADR-0030 Decision 8 amendment: the native-host's real peer is now the SERVICE, not a bare
//! invocation -- movable harness at H6, BOOTSTRAP "only delight is sacred") plus a native-host
//! relay over the actual IPC, confirms they connect (via the service's debug state), force-kills
//! the SERVICE, and asserts the native-host exits on its own within a few seconds. Every pinned
//! assertion (`connected`, `exited`) is preserved verbatim -- only the process that is
//! spawned-and-killed changed.

mod support;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn native_host_exits_when_server_dies() {
    let endpoint = format!("ghostlight-peerdeath-{}", std::process::id());
    let log_dir = support::log_dir_for(&endpoint);

    // The SERVICE: the native-host's real IPC peer. Blocks (inside `spawn_service`) until it has
    // written its first debug snapshot, so the connection race below is real, not a startup race.
    let mut service = support::spawn_service(&endpoint);

    // browser role: the chrome-extension:// positional arg auto-selects it (ADR-0051 Phase 3),
    // exactly as Chrome launches the native host. stdin held open so the upstream (Chrome -> IPC)
    // reader does not EOF.
    let mut host = Command::new(support::relay_bin())
        .arg(format!("chrome-extension://{}/", "a".repeat(32)))
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn native-host");
    let _host_stdin = host.stdin.take(); // keep the pipe open

    // The native-host must actually connect to the SERVICE first (else the kill would just
    // interrupt a retry loop and prove nothing).
    let connected = support::wait_extension_connected(&log_dir, Duration::from_secs(15));

    // Force-kill the SERVICE (the native-host's real peer); the native-host must notice the dead
    // peer and exit on its own.
    let _ = service.kill();
    let _ = service.wait();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut exited = false;
    while Instant::now() < deadline {
        match host.try_wait() {
            Ok(Some(_)) => {
                exited = true;
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => break,
        }
    }

    // Clean up regardless of the outcome.
    let _ = host.kill();
    let _ = host.wait();
    let _ = std::fs::remove_dir_all(&log_dir);

    assert!(connected, "native-host never connected to the mcp-server");
    assert!(
        exited,
        "native-host did NOT exit within 5s of the server dying (peer-death zombie regression)"
    );
}
