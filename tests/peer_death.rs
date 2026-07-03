//! Regression test for the IPC peer-death defect.
//!
//! Before the tokio-native transport migration, a force-killed mcp-server left the native-host as a
//! zombie: the `interprocess` crate's async Windows named-pipe read never woke on peer death, so the
//! relay never observed the disconnect. This test spawns a real server + native-host pair over the
//! actual IPC, confirms they connect (via the server's debug state), force-kills the server, and
//! asserts the native-host exits on its own within a few seconds.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ghostlight")
}

#[test]
fn native_host_exits_when_server_dies() {
    let endpoint = format!("ghostlight-peerdeath-{}", std::process::id());
    let log_dir = std::env::temp_dir().join(format!("bmcp-peerdeath-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&log_dir);

    // mcp-server: debug on (so we can observe the connection), stdin held open so the MCP stdio loop
    // does not hit EOF and exit before we are done.
    let mut server = Command::new(bin())
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", &log_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mcp-server");
    let _server_stdin = server.stdin.take(); // keep the pipe open

    // native-host role: the chrome-extension:// positional arg selects the relay role. stdin held
    // open so the upstream (Chrome -> IPC) reader does not EOF.
    let mut host = Command::new(bin())
        .arg(format!("chrome-extension://{}/", "a".repeat(32)))
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn native-host");
    let _host_stdin = host.stdin.take(); // keep the pipe open

    // The native-host must actually connect to the server first (else the kill would just interrupt a
    // retry loop and prove nothing).
    let connected = wait_connected(&log_dir, Duration::from_secs(15));

    // Force-kill the server; the native-host must notice the dead peer and exit on its own.
    let _ = server.kill();
    let _ = server.wait();

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

/// Poll the server's debug snapshot until it reports the extension (native-host) connected.
fn wait_connected(log_dir: &Path, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(state) = newest_state(log_dir) {
            if state.contains("\"extension_connected\": true") {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Read the newest `debug-state-*.json` under `dir`.
fn newest_state(dir: &Path) -> Option<String> {
    let mut newest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("debug-state-") && name.ends_with(".json") {
            if let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) {
                if newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                    newest = Some((mtime, entry.path()));
                }
            }
        }
    }
    let mut contents = String::new();
    std::fs::File::open(newest?.1)
        .ok()?
        .read_to_string(&mut contents)
        .ok()?;
    Some(contents)
}
