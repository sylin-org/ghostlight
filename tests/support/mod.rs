// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Shared spawn helpers for the standalone-SERVICE + thin-ADAPTER topology (ADR-0030 Decision 8
//! amendment, "the always-ready-service amendment"; H6 task file's "Tests" section). Every
//! integration test that used to spawn ONE bare `ghostlight` invocation now needs TWO cooperating
//! processes: the standalone SERVICE (`ghostlight service`, which owns the browser link and the
//! adapter/control endpoint) and a thin ADAPTER (a bare `ghostlight` invocation) that connects to
//! it and relays stdio. `#![allow(dead_code)]`: not every test binary that includes this module
//! (via `mod support;`) uses every helper.

#![allow(dead_code)]

/// The in-process session fixture (ADR-0051 Phase 4): drives the real `serve_session` chokepoint
/// over an in-memory duplex, no spawned process. The migration target for the incidentally-E2E
/// wiring tests.
pub mod inproc;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ghostlight")
}

/// The single `ghostlight-relay` executable beside the `ghostlight` test binary (ADR-0046 +
/// ADR-0051 Phase 3: it carries both former adapter roles). Cargo does not expose a
/// `CARGO_BIN_EXE_*` for another workspace member's bin, so derive the sibling path in the same
/// `target/<profile>/` directory; `cargo test --workspace` builds it before tests. The AGENT role is
/// selected with `--role agent` (see [`spawn_adapter`]); the BROWSER role is auto-detected from the
/// `chrome-extension://` origin, exactly as Chrome launches it.
pub fn relay_bin() -> PathBuf {
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

/// The isolated `GHOSTLIGHT_LOG_DIR` a given test's service uses, deterministic from `endpoint`
/// (which every caller already makes unique per test): lets a caller poll the SAME service's debug
/// state after [`spawn_service`] hands back only a bare `Child`.
pub fn log_dir_for(endpoint: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ghostlight-test-logdir-{endpoint}"))
}

/// The isolated audit file a given test's service writes to (ADR-0051 Phase 1): every `spawn_service*`
/// helper sets `GHOSTLIGHT_AUDIT_DIR` to the endpoint's [`log_dir_for`], so audit lands in the test's
/// own dir instead of the machine's REAL default audit path (which `dirs::data_local_dir()` resolves
/// ignoring env, and which parallel E2E tests would otherwise contend on). A test that inspects the
/// audit stream reads it here.
pub fn audit_path_for(endpoint: &str) -> PathBuf {
    log_dir_for(endpoint).join("audit.jsonl")
}

/// Connect to a service's TCP web API. No retry is needed: the `port` comes from
/// [`wait_for_webapi_port`], which the spawn helpers call before returning it, and the service
/// publishes that port only AFTER its listener has bound -- so by the time a caller holds a port,
/// the listener is up and its backlog accepts this connection immediately.
pub fn connect_webapi(port: u16) -> std::net::TcpStream {
    std::net::TcpStream::connect(("127.0.0.1", port))
        .unwrap_or_else(|e| panic!("connect to the web API on port {port}: {e}"))
}

/// Poll `log_dir`'s newest debug state until the service publishes its inbound.web listener's
/// actual bound port (done right after a successful bind), and return it. Subsumes
/// [`wait_for_debug_state`] for web-API tests: a published port proves both that the service
/// started AND that its listener is up. Panics after `within` (the service never started, or its
/// web-API bind failed -- which is deliberately non-fatal, so it would otherwise hang forever).
pub fn wait_for_webapi_port(log_dir: &Path, within: Duration) -> u16 {
    let deadline = Instant::now() + within;
    loop {
        if let Some(state) = newest_state(log_dir) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&state) {
                if let Some(port) = v.get("webapi_port").and_then(|p| p.as_u64()) {
                    return port as u16;
                }
            }
        }
        if Instant::now() >= deadline {
            panic!(
                "the service never published a webapi_port within {within:?} (bind failed?); log_dir={}",
                log_dir.display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Spawn `ghostlight service` bound to `endpoint`: debug on, an isolated `GHOSTLIGHT_LOG_DIR`
/// ([`log_dir_for`], so the hub-key + debug files are test-isolated), stdio null. BLOCKS until the
/// service's debug snapshot exists (poll up to ~15s). Returns the `Child` -- the caller kills it in
/// teardown; never waits out `IDLE_GRACE`.
pub fn spawn_service(endpoint: &str) -> Child {
    spawn_service_with_manifest(endpoint, None)
}

/// Like [`spawn_service`], but with `--manifest <src>` forwarded to the SERVICE (PINS.md SS5.1: a
/// `--manifest` on the ADAPTER is a no-op with a warning -- only the SERVICE ever loads policy).
pub fn spawn_service_with_manifest(endpoint: &str, manifest: Option<&str>) -> Child {
    let log_dir = log_dir_for(endpoint);
    let _ = std::fs::remove_dir_all(&log_dir);
    let mut cmd = Command::new(bin());
    // PINS.md SS5.1: `--manifest` is a TOP-LEVEL `Cli` field, not scoped to the `service`
    // subcommand -- it MUST precede the subcommand token on the command line (usage:
    // `ghostlight --manifest <src> service`), or clap rejects it as an unexpected argument.
    if let Some(src) = manifest {
        cmd.arg("--manifest").arg(src);
    }
    cmd.arg("service")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", &log_dir)
        .env("GHOSTLIGHT_AUDIT_DIR", &log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = cmd.spawn().expect("spawn ghostlight service");
    wait_for_debug_state(&log_dir, Duration::from_secs(15));
    child
}

/// Like [`spawn_service`], but with an extra `ProgramData` env var forwarded (D, H6 forced: only
/// the SERVICE resolves the org policy path now, ADR-0030 Decision 8 amendment; the pre-H6 org-
/// policy-boot regression tests -- `tests/manifest_validation.rs`, `tests/hot_reload.rs` -- relied
/// on a bare invocation reading `ProgramData` directly, which no longer holds). `spawn_service`
/// itself takes no extra env vars, so this is a small, separate spawn rather than a parameter on
/// that one.
pub fn spawn_service_with_program_data(endpoint: &str, program_data_dir: &Path) -> Child {
    let log_dir = log_dir_for(endpoint);
    let _ = std::fs::remove_dir_all(&log_dir);
    let child = Command::new(bin())
        .arg("service")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("ProgramData", program_data_dir)
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", &log_dir)
        .env("GHOSTLIGHT_AUDIT_DIR", &log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight service");
    wait_for_debug_state(&log_dir, Duration::from_secs(15));
    child
}

/// Like [`spawn_service`], but the SERVICE binds an OS-assigned ephemeral web-API port
/// (`GHOSTLIGHT_WEBAPI_PORT=0`) and this helper returns the ACTUAL bound port it published
/// (PINS.md CS11, `docs/tasks/console`). Replaces the old fixed pid+seq port guess, which could
/// collide across the parallel test binaries `cargo test` runs -- on collision one service's bind
/// failed (deliberately non-fatal), leaving a test connecting to a port with no listener. Returns
/// `(Child, port)`; the caller kills the child in teardown.
pub fn spawn_service_with_webapi_port(endpoint: &str) -> (Child, u16) {
    let log_dir = log_dir_for(endpoint);
    let _ = std::fs::remove_dir_all(&log_dir);
    let child = Command::new(bin())
        .arg("service")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_WEBAPI_PORT", "0")
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", &log_dir)
        .env("GHOSTLIGHT_AUDIT_DIR", &log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight service");
    let port = wait_for_webapi_port(&log_dir, Duration::from_secs(15));
    (child, port)
}

/// Combines [`spawn_service_with_program_data`] and [`spawn_service_with_webapi_port`] (K3,
/// `docs/tasks/console`): a real spawned service with BOTH an isolated org-policy `ProgramData`
/// override AND a test-unique web API port, for tests that need to fetch a real org-mandatory
/// config override over a real TCP connection.
pub fn spawn_service_with_program_data_and_webapi_port(
    endpoint: &str,
    program_data_dir: &Path,
) -> (Child, u16) {
    let log_dir = log_dir_for(endpoint);
    let _ = std::fs::remove_dir_all(&log_dir);
    let child = Command::new(bin())
        .arg("service")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("ProgramData", program_data_dir)
        .env("GHOSTLIGHT_WEBAPI_PORT", "0")
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", &log_dir)
        .env("GHOSTLIGHT_AUDIT_DIR", &log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight service");
    let port = wait_for_webapi_port(&log_dir, Duration::from_secs(15));
    (child, port)
}

/// Combines [`spawn_service_with_webapi_port`] with the `GHOSTLIGHT_USER_CONFIG_DIR` override
/// (K5, `docs/tasks/console`: `governance::config::load::user_config_path`'s escalation, added
/// after K5 found `dirs::config_dir()` does not honor a platform env var override for the
/// current process the way `org_policy_path`'s `ProgramData` read does). Isolates a real write to
/// the user config layer to `user_config_dir` -- NEVER this machine's real config file.
pub fn spawn_service_with_user_config_dir_and_webapi_port(
    endpoint: &str,
    user_config_dir: &Path,
) -> (Child, u16) {
    let log_dir = log_dir_for(endpoint);
    let _ = std::fs::remove_dir_all(&log_dir);
    let child = Command::new(bin())
        .arg("service")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_USER_CONFIG_DIR", user_config_dir)
        .env("GHOSTLIGHT_WEBAPI_PORT", "0")
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", &log_dir)
        .env("GHOSTLIGHT_AUDIT_DIR", &log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight service");
    let port = wait_for_webapi_port(&log_dir, Duration::from_secs(15));
    (child, port)
}

/// Combines [`spawn_service_with_program_data_and_webapi_port`] with the
/// `GHOSTLIGHT_USER_CONFIG_DIR` override (K5, `docs/tasks/console`): a real spawned service with
/// an isolated org-policy override, an isolated user-config-write destination, AND a test-unique
/// web API port -- for a test that needs BOTH an org-mandatory lock and write-path isolation
/// (e.g. confirming a refused write never touches the isolated user config file either).
pub fn spawn_service_with_program_data_user_config_dir_and_webapi_port(
    endpoint: &str,
    program_data_dir: &Path,
    user_config_dir: &Path,
) -> (Child, u16) {
    let log_dir = log_dir_for(endpoint);
    let _ = std::fs::remove_dir_all(&log_dir);
    let child = Command::new(bin())
        .arg("service")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("ProgramData", program_data_dir)
        .env("GHOSTLIGHT_USER_CONFIG_DIR", user_config_dir)
        .env("GHOSTLIGHT_WEBAPI_PORT", "0")
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", &log_dir)
        .env("GHOSTLIGHT_AUDIT_DIR", &log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight service");
    let port = wait_for_webapi_port(&log_dir, Duration::from_secs(15));
    (child, port)
}

/// Spawn a bare `ghostlight` invocation (the thin ADAPTER) with piped stdin/stdout, relaying to the
/// SERVICE already running on `endpoint`. Because the service is spawned FIRST and awaited-ready
/// ([`spawn_service`]), the adapter's first dial succeeds and the self-heal path is never taken --
/// correct, since tests must never touch a real OS supervisor. Shares the SAME `GHOSTLIGHT_LOG_DIR`
/// ([`log_dir_for`]) the matching [`spawn_service`] call used: anti-squat (PINS.md SS5.3) requires
/// both sides to read the SAME per-install `hub-key`, so a mismatched log dir here would make
/// every real adapter/service pair fail the proof, not just an intentional impostor scenario.
pub fn spawn_adapter(endpoint: &str) -> Child {
    Command::new(relay_bin())
        .arg("--role")
        .arg("agent")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_LOG_DIR", log_dir_for(endpoint))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight adapter")
}

/// BLOCK until `log_dir` holds at least one `debug-state-*.json` file (the service has written its
/// first snapshot), or panic after `within`.
fn wait_for_debug_state(log_dir: &Path, within: Duration) {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if newest_state(log_dir).is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!(
        "the service never wrote a debug-state file within {within:?} (log_dir={})",
        log_dir.display()
    );
}

/// The newest `debug-state-*.json` contents under `dir`, if any.
pub fn newest_state(dir: &Path) -> Option<String> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
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

/// The newest `debug-state-*.json` under `dir` written by a process whose `role` matches
/// ("adapter", "mcp-server", "native-host"), parsed as JSON. The adapter and the service both write
/// state files into a shared test log dir, so a test that wants the ADAPTER's structured counters
/// (ADR-0051 P4.3b) must filter on role rather than take [`newest_state`] (which may return the
/// service's file). `None` if none match yet.
pub fn newest_state_for_role(dir: &Path, role: &str) -> Option<serde_json::Value> {
    let mut newest: Option<(std::time::SystemTime, serde_json::Value)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("debug-state-") && name.ends_with(".json")) {
            continue;
        }
        let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        let Ok(raw) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if v.get("role").and_then(|r| r.as_str()) != Some(role) {
            continue;
        }
        if newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
            newest = Some((mtime, v));
        }
    }
    newest.map(|(_, v)| v)
}

/// Poll [`newest_state_for_role`] until `pred` holds on the parsed state (returning it), or panic
/// after `within`. Bridges the brief window between the adapter forcing a snapshot on a lifecycle
/// note and the test reading it back (ADR-0051 P4.3b).
pub fn wait_state_for_role_until(
    dir: &Path,
    role: &str,
    within: Duration,
    pred: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = Instant::now() + within;
    loop {
        if let Some(v) = newest_state_for_role(dir, role) {
            if pred(&v) {
                return v;
            }
        }
        if Instant::now() >= deadline {
            panic!(
                "no '{role}' debug-state satisfying the predicate under {} within {within:?}",
                dir.display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The fake-extension attach preamble (ADR-0058/0061): the browser-role hello the relay would
/// send, then the extension's persistent-identity frame. Since ADR-0061 the service admits a
/// browser connection only after BOTH frames arrive (fail-closed if the identity never comes),
/// so every spawn-tier fake extension must send these before reading its first frame.
pub async fn send_extension_attach_frames<W>(write_half: &mut W)
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let hello = ghostlight::handshake::browser_hello_bytes(
        std::process::id(),
        Some(ghostlight::proc::ProcId {
            pid: std::process::id(),
            created: 0,
        }),
    );
    ghostlight::native::host::write_message(write_half, &hello)
        .await
        .expect("send the browser-role hello");
    let identity = serde_json::to_vec(&serde_json::json!({
        "type": ghostlight::handshake::EXTENSION_IDENTITY_TYPE,
        ghostlight::handshake::BROWSER_ID_FIELD: format!("fake-ext-{}", std::process::id()),
    }))
    .expect("identity frame serializes");
    ghostlight::native::host::write_message(write_half, &identity)
        .await
        .expect("send the extension identity frame");
}

/// Answer one `tab_url_request` frame the way the real extension answers for a live, in-group
/// tab: report a synthetic https URL derived from the requested tabId. The service probes a
/// call's tab URL before dispatching the tool_request itself (domain resolution for audit/grants,
/// and navigate's unknown-tab auto-create check, CAP-MED-02); replying `url: null` would read as
/// an unknown/closed tab and make navigate auto-create a fresh one, changing the frame sequence
/// a test observes.
pub async fn answer_tab_url<W>(write_half: &mut W, request: &serde_json::Value)
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let reply = serde_json::json!({
        "id": request["id"],
        "type": "tab_url_response",
        "result": { "url": format!("https://tab-{}.example.com/", request["tabId"]) },
    });
    ghostlight::native::host::write_message(write_half, &serde_json::to_vec(&reply).unwrap())
        .await
        .expect("send the tab_url_response");
}

/// Read frames until one of type `wanted` arrives, transparently answering any interleaved
/// `tab_url_request` via [`answer_tab_url`]. Panics on any other frame type, same posture as the
/// fake-extension loops this serves.
pub async fn read_frame_answering_tab_urls<R, W>(
    read_half: &mut R,
    write_half: &mut W,
    wanted: &str,
) -> serde_json::Value
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    loop {
        let frame = ghostlight::native::host::read_message(read_half)
            .await
            .unwrap()
            .expect("a framed message from the service");
        let v: serde_json::Value = serde_json::from_slice(&frame).unwrap();
        if v["type"] == "tab_url_request" {
            answer_tab_url(write_half, &v).await;
            continue;
        }
        assert_eq!(v["type"], wanted, "unexpected frame type: {v:?}");
        return v;
    }
}

/// Poll `log_dir`'s newest debug state until it reports `"extension_connected": true`, or return
/// `false` after `within`.
pub fn wait_extension_connected(log_dir: &Path, within: Duration) -> bool {
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
