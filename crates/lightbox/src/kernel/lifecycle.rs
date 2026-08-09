// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! MCP-edge reconnect, anti-squat, and browser-relay lifecycle parity scenarios.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::Path;
use std::process::{ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure};
use serde_json::{json, Value};

use crate::scenarios::Scenario;
use crate::support::{self, ChildGuard, TempRoot};

const NATIVE_SURFACE: &str =
    include_str!("../../../mcp-connector/src/surface/data/ghostlight-v1.json");

fn tool_count() -> usize {
    serde_json::from_str::<Value>(NATIVE_SURFACE).expect("edge-owned native surface parses")
        ["tools"]
        .as_array()
        .expect("edge-owned native surface has tools")
        .len()
}

pub(super) fn registry() -> Vec<Scenario> {
    vec![
        (
            "mcp-edge-reconnects-future-call",
            mcp_edge_reconnects_future_call,
        ),
        ("service-survives-mcp-edge", service_survives_mcp_edge),
        ("mcp-edge-anti-squat", mcp_edge_anti_squat),
        ("mcp-2026-exact-transcript", mcp_2026_exact_transcript),
        ("browser-relay-restart", browser_relay_restart),
    ]
}

fn start_service(endpoint: &str, log_dir: &Path, keep_warm: bool) -> anyhow::Result<ChildGuard> {
    std::fs::create_dir_all(log_dir)?;
    let mut command = support::service_command()?;
    command.arg("service");
    if keep_warm {
        command.arg("--keep-warm");
    }
    command
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", log_dir)
        .env("GHOSTLIGHT_AUDIT_DIR", log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = support::spawn_guard(&mut command)?;
    support::wait_for_debug_states(log_dir, 1, Duration::from_secs(15))?;
    Ok(child)
}

struct McpEdge {
    child: ChildGuard,
    stdin: Option<ChildStdin>,
    replies: Receiver<String>,
}

impl McpEdge {
    fn start(endpoint: &str, log_dir: &Path) -> anyhow::Result<Self> {
        let mut command = support::mcp_command()?;
        command
            .env("GHOSTLIGHT_ENDPOINT", endpoint)
            .env("GHOSTLIGHT_LOG_DIR", log_dir)
            .env("GHOSTLIGHT_DEBUG", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = support::spawn_guard(&mut command)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("MCP edge stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge stdout"))?;
        let (sender, replies) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            replies,
        })
    }

    fn send(&mut self, value: &Value) -> anyhow::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("MCP edge stdin closed"))?;
        serde_json::to_writer(&mut *stdin, value)?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(())
    }

    fn receive(&self, within: Duration) -> anyhow::Result<Value> {
        let line = match self.replies.recv_timeout(within) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => anyhow::bail!("no MCP reply within {within:?}"),
            Err(RecvTimeoutError::Disconnected) => anyhow::bail!("MCP edge stdout closed"),
        };
        Ok(serde_json::from_str(&line)?)
    }

    fn close(mut self) {
        self.stdin.take();
        let _ = self.child.wait();
    }
}

fn initialize(edge: &mut McpEdge) -> anyhow::Result<()> {
    edge.send(&support::initialize_2025(1, "lightbox-lifecycle"))?;
    let initialized = edge.receive(Duration::from_secs(10))?;
    ensure!(initialized["id"] == 1);
    ensure!(initialized["result"]["protocolVersion"] == "2025-11-25");
    edge.send(&support::initialized_2025())?;
    edge.send(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))?;
    let tools = edge.receive(Duration::from_secs(10))?;
    ensure!(tools["id"] == 2);
    ensure!(tools["result"]["tools"].as_array().map(Vec::len) == Some(tool_count()));
    Ok(())
}

fn mcp_edge_reconnects_future_call() -> anyhow::Result<()> {
    let tmp = TempRoot::new("mcp-edge-reconnect")?;
    let endpoint = support::unique_endpoint("mcp-edge-reconnect");
    let log_dir = tmp.path().join("logs");
    let mut first = start_service(&endpoint, &log_dir, false)?;
    let mut edge = McpEdge::start(&endpoint, &log_dir)?;
    initialize(&mut edge)?;

    first.kill()?;
    first.wait()?;
    std::thread::sleep(Duration::from_secs(1));
    let _second = start_service(&endpoint, &log_dir, false)?;
    edge.send(&json!({
        "jsonrpc":"2.0",
        "id":3,
        "method":"tools/call",
        "params":{"name":"explain","arguments":{}}
    }))?;
    let reply = edge.receive(Duration::from_secs(30))?;
    ensure!(reply["id"] == 3, "unexpected future-call response: {reply}");
    ensure!(
        reply["result"]["isError"] != true,
        "future call did not recover after service restart: {reply}"
    );
    edge.close();
    Ok(())
}

fn service_survives_mcp_edge() -> anyhow::Result<()> {
    let tmp = TempRoot::new("service-survives-mcp-edge")?;
    let endpoint = support::unique_endpoint("service-survives-mcp-edge");
    let log_dir = tmp.path().join("logs");
    let service = start_service(&endpoint, &log_dir, false)?;
    let service_pid = service.id();
    let mut edge = McpEdge::start(&endpoint, &log_dir)?;
    edge.send(&support::initialize_2025(1, "lightbox-survival"))?;
    ensure!(edge.receive(Duration::from_secs(10))?["id"] == 1);
    drop(edge);
    std::thread::sleep(Duration::from_secs(2));
    ensure!(ghostlight_transport::proc::pid_exists(service_pid));
    Ok(())
}

fn mcp_edge_anti_squat() -> anyhow::Result<()> {
    let tmp = TempRoot::new("mcp-edge-anti-squat")?;
    let endpoint = support::unique_endpoint("mcp-edge-anti-squat");
    let service_logs = tmp.path().join("service-logs");
    let edge_logs = tmp.path().join("edge-logs");
    std::fs::create_dir_all(&edge_logs)?;
    let _service = start_service(&endpoint, &service_logs, false)?;
    let mut command = support::mcp_command()?;
    command
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .env("GHOSTLIGHT_LOG_DIR", &edge_logs)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut edge = support::spawn_guard(&mut command)?;
    let mut stderr = edge
        .stderr
        .take()
        .ok_or_else(|| anyhow!("MCP edge stderr"))?;
    let reader = std::thread::spawn(move || {
        let mut captured = String::new();
        let _ = stderr.read_to_string(&mut captured);
        captured
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let exited = loop {
        if edge.try_wait()?.is_some() {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !exited {
        edge.kill()?;
    }
    edge.wait()?;
    let captured = reader
        .join()
        .map_err(|_| anyhow!("stderr reader panicked"))?;
    ensure!(exited, "anti-squat mismatch did not terminate the MCP edge");
    ensure!(captured.contains(
        "refusing to connect: the Ghostlight service on this endpoint is not the one this user installed"
    ));
    Ok(())
}

fn params_2026() -> Value {
    json!({
        "_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/clientInfo": {
                "name": "lightbox-2026",
                "version": "1.0.0",
            },
        },
    })
}

fn mcp_2026_exact_transcript() -> anyhow::Result<()> {
    let tmp = TempRoot::new("mcp-2026-transcript")?;
    let endpoint = support::unique_endpoint("mcp-2026-transcript");
    let log_dir = tmp.path().join("logs");
    let _service = start_service(&endpoint, &log_dir, false)?;
    let mut edge = McpEdge::start(&endpoint, &log_dir)?;

    edge.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "server/discover",
        "params": params_2026(),
    }))?;
    let discovery = edge.receive(Duration::from_secs(10))?;
    ensure!(
        discovery["result"]["supportedVersions"] == json!(["2026-07-28", "2025-11-25"]),
        "unexpected discovery response: {discovery}"
    );
    ensure!(discovery["result"]["resultType"] == "complete");
    ensure!(discovery["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"].is_string());

    edge.send(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": params_2026(),
    }))?;
    let tools = edge.receive(Duration::from_secs(10))?;
    ensure!(tools["id"] == 2, "unexpected tools/list response: {tools}");
    ensure!(tools["result"]["resultType"] == "complete");
    ensure!(tools["result"]["cacheScope"] == "private");
    ensure!(tools["result"]["ttlMs"].is_u64());
    ensure!(tools["result"]["tools"].as_array().map(Vec::len) == Some(tool_count()));
    ensure!(tools["result"]["tools"][0]["outputSchema"]["properties"]["workspace"].is_object());

    let mut listen_params = params_2026();
    listen_params["notifications"] = json!({"toolsListChanged": true});
    edge.send(&json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "subscriptions/listen",
        "params": listen_params,
    }))?;
    let acknowledged = edge.receive(Duration::from_secs(10))?;
    ensure!(acknowledged.get("id").is_none());
    ensure!(
        acknowledged["method"] == "notifications/subscriptions/acknowledged",
        "subscription did not acknowledge first: {acknowledged}"
    );
    ensure!(acknowledged["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"] == 3);

    edge.send(&json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/list",
        "params": params_2026(),
    }))?;
    let duplicate = edge.receive(Duration::from_secs(10))?;
    ensure!(duplicate["error"]["code"] == -32600);

    edge.stdin.take();
    let terminal = edge.receive(Duration::from_secs(10))?;
    ensure!(
        terminal["id"] == 3,
        "unexpected subscription close: {terminal}"
    );
    ensure!(terminal["result"]["resultType"] == "complete");
    ensure!(terminal["result"]["_meta"]["io.modelcontextprotocol/subscriptionId"] == 3);
    ensure!(edge.child.wait()?.success());
    Ok(())
}

fn write_chrome_frame(stdin: &mut ChildStdin, payload: &[u8]) -> anyhow::Result<()> {
    stdin.write_all(&(payload.len() as u32).to_le_bytes())?;
    stdin.write_all(payload)?;
    stdin.flush()?;
    Ok(())
}

fn clear_debug_states(log_dir: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(log_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("debug-state-") && name.ends_with(".json") {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn browser_relay_restart() -> anyhow::Result<()> {
    let tmp = TempRoot::new("browser-relay-restart")?;
    let endpoint = support::unique_endpoint("browser-relay-restart");
    let log_dir = tmp.path().join("logs");
    let mut first = start_service(&endpoint, &log_dir, false)?;
    let mut command = support::relay_command()?;
    command
        .arg(format!("chrome-extension://{}/", "a".repeat(32)))
        .env("GHOSTLIGHT_ENDPOINT", &endpoint)
        .env("GHOSTLIGHT_LOG_DIR", &log_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut relay = support::spawn_guard(&mut command)?;
    let mut stdin = relay
        .stdin
        .take()
        .ok_or_else(|| anyhow!("browser relay stdin"))?;
    let identity = serde_json::to_vec(&json!({
        "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
        ghostlight_transport::handshake::BROWSER_ID_FIELD: "lightbox-browser-relay",
    }))?;
    write_chrome_frame(&mut stdin, &identity)?;
    support::wait_extension_connected(&log_dir, Duration::from_secs(15))?;
    first.kill()?;
    first.wait()?;
    std::thread::sleep(Duration::from_secs(2));
    if let Some(status) = relay.try_wait()? {
        let mut captured = String::new();
        if let Some(stderr) = relay.stderr.as_mut() {
            stderr.read_to_string(&mut captured)?;
        }
        anyhow::bail!("browser relay exited with the service ({status}): {captured}");
    }
    clear_debug_states(&log_dir)?;
    let _second = start_service(&endpoint, &log_dir, false)?;
    support::wait_extension_connected(&log_dir, Duration::from_secs(20))?;
    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if relay.try_wait()?.is_some() {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "browser relay survived browser EOF"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
