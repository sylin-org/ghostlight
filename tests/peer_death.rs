// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The browser relay's lifecycle contract across peer death, updated for ADR-0062.
//!
//! History: before the tokio-native transport migration, a force-killed peer left the native-host
//! as a zombie (the `interprocess` crate's async Windows named-pipe read never woke on peer
//! death). The original test pinned "the relay exits when the service dies". ADR-0062 deliberately
//! INVERTED that: the relay now holds Chrome's native port open across a service death, reconnects
//! to the replacement service, and replays the extension's cached identity frame -- so a service
//! restart or upgrade never forces a native-port drop or an extension reload. The one thing that
//! ends the relay is the BROWSER going away (its stdin EOF).
//!
//! This test pins the full contract end to end with real processes: attach (with the ADR-0061
//! identity frame Chrome would send), service killed -> relay survives, replacement service ->
//! relay re-attaches by replaying the cached identity, browser stdin closed -> relay exits
//! promptly (the anti-zombie half of the original test, preserved).

mod support;

use std::io::Write;
use std::process::{ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

/// Write one Chrome-side native-messaging frame (4-byte LE length prefix + JSON payload) into the
/// relay's stdin, exactly as Chrome frames messages to a native host.
fn write_chrome_frame(stdin: &mut ChildStdin, payload: &[u8]) {
    let len = (payload.len() as u32).to_le_bytes();
    stdin.write_all(&len).expect("write frame length");
    stdin.write_all(payload).expect("write frame payload");
    stdin.flush().expect("flush the chrome frame");
}

#[ignore = "e2e: spawns a real ghostlight service/adapter; run via the e2e tier -- cargo test -- --ignored"]
#[test]
fn native_host_rides_a_service_restart_and_exits_on_browser_eof() {
    let endpoint = format!("ghostlight-peerdeath-{}", std::process::id());
    let log_dir = support::log_dir_for(&endpoint);

    // The SERVICE: the native-host's real IPC peer. Blocks (inside `spawn_service`) until it has
    // written its first debug snapshot, so the connection race below is real, not a startup race.
    let mut service = support::spawn_service(&endpoint);

    // browser role: the chrome-extension:// positional arg auto-selects it (ADR-0051 Phase 3),
    // exactly as Chrome launches the native host. stdin is Chrome's side of the native port.
    let mut host = Command::new(support::relay_bin())
        .arg(format!("chrome-extension://{}/", "a".repeat(32)))
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn native-host");
    let mut host_stdin = host.stdin.take().expect("host stdin");

    // ADR-0061: the service admits a browser only after the extension's identity frame. Play
    // Chrome's part and send it through the relay, which forwards it verbatim and caches it as
    // the opening frame it will replay on every reconnect (ADR-0062).
    let identity = serde_json::to_vec(&serde_json::json!({
        "type": ghostlight::handshake::EXTENSION_IDENTITY_TYPE,
        ghostlight::handshake::BROWSER_ID_FIELD: "fake-ext-peer-death",
    }))
    .expect("identity frame serializes");
    write_chrome_frame(&mut host_stdin, &identity);

    let connected = support::wait_extension_connected(&log_dir, Duration::from_secs(15));
    assert!(connected, "native-host never connected to the mcp-server");

    // Force-kill the SERVICE. Since ADR-0062 the relay must NOT exit: Chrome's native port stays
    // open and the relay reconnects. (The pre-0062 exit-on-peer-death contract is retired.)
    let _ = service.kill();
    let _ = service.wait();
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        matches!(host.try_wait(), Ok(None)),
        "the relay must survive a service death and keep reconnecting (ADR-0062), not exit"
    );

    // A replacement service on the same endpoint: the relay reconnects on its own and REPLAYS the
    // cached identity frame, so the new service reports the extension connected without Chrome
    // resending anything.
    let mut service2 = support::spawn_service(&endpoint);
    let reconnected = support::wait_extension_connected(&log_dir, Duration::from_secs(20));

    // The one event that ends the relay: the browser going away. Close Chrome's side of the
    // native port; the relay must exit promptly (the anti-zombie half, preserved).
    drop(host_stdin);
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
    let _ = service2.kill();
    let _ = service2.wait();
    let _ = std::fs::remove_dir_all(&log_dir);

    assert!(
        reconnected,
        "the relay never re-attached to the replacement service (identity replay, ADR-0062)"
    );
    assert!(
        exited,
        "the relay did NOT exit within 5s of the browser closing its stdin (zombie regression)"
    );
}
