// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Shared spawn helpers for ADR-0096's three-process topology: the persistent `ghostlight`
//! service, the protocol-versioned `ghostlight-mcp-connector` stdio edge, and the browser-side
//! `ghostlight-browser-connector`. Most process integration tests need only the service plus MCP edge; fake
//! browser helpers exercise the third shore when browser routing is the subject.
//!
//! `#![allow(dead_code)]`: not every test binary that includes this module via `mod support;` uses
//! every helper.

#![allow(dead_code)]

/// The protocol-neutral in-process service fixture used by governance and dispatch tests.
pub mod inproc;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn ghostlight_service_bin() -> &'static str {
    env!("CARGO_BIN_EXE_ghostlight")
}

/// Locate the sibling `ghostlight-mcp-connector` executable built for the same target profile.
///
/// Cargo does not expose another workspace member's `CARGO_BIN_EXE_*` value to this package's
/// integration tests, so derive the path beside the known service executable. Workspace tests
/// build all three binaries before exercising process scenarios.
pub fn ghostlight_mcp_bin() -> PathBuf {
    let dir = Path::new(ghostlight_service_bin())
        .parent()
        .expect("the test binary has a parent directory");
    let name = if cfg!(windows) {
        "ghostlight-mcp-connector.exe"
    } else {
        "ghostlight-mcp-connector"
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

/// Spawn `ghostlight service` bound to `endpoint`: debug on, an isolated `GHOSTLIGHT_LOG_DIR`
/// ([`log_dir_for`], so the hub-key + debug files are test-isolated), stdio null. BLOCKS until the
/// service's debug snapshot exists (poll up to ~15s). Returns the `Child` -- the caller kills it in
/// teardown; never waits out `IDLE_GRACE`.
pub fn spawn_service(endpoint: &str) -> Child {
    spawn_service_with_manifest(endpoint, None)
}

/// Like [`spawn_service`], but with `--manifest <src>` forwarded to the service. The MCP edge never
/// loads policy.
pub fn spawn_service_with_manifest(endpoint: &str, manifest: Option<&str>) -> Child {
    let log_dir = log_dir_for(endpoint);
    let _ = std::fs::remove_dir_all(&log_dir);
    let mut cmd = Command::new(ghostlight_service_bin());
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
    let child = Command::new(ghostlight_service_bin())
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

/// Spawn `ghostlight-mcp-connector` with piped stdio and connect it to the ready service at `endpoint`.
///
/// The matching [`spawn_service`] must run first so tests do not invoke supervisor self-heal. Both
/// processes share [`log_dir_for`] because the owner-only bridge anti-squat proof reads the same
/// per-install key from that directory.
pub fn spawn_mcp_edge(endpoint: &str) -> Child {
    Command::new(ghostlight_mcp_bin())
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_LOG_DIR", log_dir_for(endpoint))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ghostlight MCP edge")
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
