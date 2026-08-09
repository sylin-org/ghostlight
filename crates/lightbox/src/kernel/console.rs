// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Read-only loopback Console parity scenarios migrated from the legacy spawn tier.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use anyhow::{anyhow, ensure};

use crate::scenarios::Scenario;
use crate::support::{self, ChildGuard, TempRoot};

pub(super) fn registry() -> Vec<Scenario> {
    vec![
        ("kernel-console-index", console_index),
        ("kernel-console-assets", console_assets),
        ("kernel-console-not-found", console_not_found),
        (
            "kernel-console-method-not-allowed",
            console_method_not_allowed,
        ),
        ("kernel-console-websocket-rejected", websocket_rejected),
        ("kernel-console-config-registry", config_registry),
        ("kernel-console-dns-rebind-denied", dns_rebind_denied),
        ("kernel-console-live-sessions", live_sessions),
    ]
}

struct Console {
    _root: TempRoot,
    _service: ChildGuard,
    endpoint: String,
    log_dir: std::path::PathBuf,
    port: u16,
}

impl Console {
    fn start(tag: &str) -> anyhow::Result<Self> {
        let root = TempRoot::new(tag)?;
        let endpoint = support::unique_endpoint(tag);
        let log_dir = root.path().join("logs");
        let (service, port) = support::spawn_service_with_manage_web(&endpoint, &log_dir, None)?;
        Ok(Self {
            _root: root,
            _service: service,
            endpoint,
            log_dir,
            port,
        })
    }

    fn mcp_edge(&self) -> anyhow::Result<ChildGuard> {
        support::spawn_mcp_edge(&self.endpoint, &self.log_dir)
    }
}

fn request(
    port: u16,
    method: &str,
    path: &str,
    headers: &str,
    body: &str,
) -> anyhow::Result<String> {
    request_with_host(
        port,
        method,
        path,
        &format!("127.0.0.1:{port}"),
        headers,
        body,
    )
}

fn request_with_host(
    port: u16,
    method: &str,
    path: &str,
    host: &str,
    headers: &str,
    body: &str,
) -> anyhow::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn get(port: u16, path: &str, headers: &str) -> anyhow::Result<String> {
    request(port, "GET", path, headers, "")
}

fn status(response: &str) -> &str {
    response.lines().next().unwrap_or_default()
}

fn body(response: &str) -> &str {
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default()
}

fn header<'a>(response: &'a str, wanted: &str) -> Option<&'a str> {
    response
        .split("\r\n")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case(wanted))
        })
        .map(|(_, value)| value.trim())
}

fn websocket_response(port: u16) -> anyhow::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = format!(
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    let mut buffer = [0u8; 512];
    let count = stream.read(&mut buffer)?;
    Ok(String::from_utf8_lossy(&buffer[..count]).into_owned())
}

fn console_index() -> anyhow::Result<()> {
    let console = Console::start("console-index")?;
    let response = get(console.port, "/", "")?;
    ensure!(status(&response) == "HTTP/1.1 200 OK");
    ensure!(header(&response, "Content-Type") == Some("text/html; charset=utf-8"));
    ensure!(body(&response).contains("/manage.css"));
    ensure!(body(&response).contains("/manage.js"));
    Ok(())
}

fn console_assets() -> anyhow::Result<()> {
    let console = Console::start("console-assets")?;
    let css = get(console.port, "/manage.css", "")?;
    let js = get(console.port, "/manage.js", "")?;
    ensure!(status(&css) == "HTTP/1.1 200 OK");
    ensure!(header(&css, "Content-Type") == Some("text/css; charset=utf-8"));
    ensure!(status(&js) == "HTTP/1.1 200 OK");
    ensure!(header(&js, "Content-Type") == Some("application/javascript; charset=utf-8"));
    Ok(())
}

fn console_not_found() -> anyhow::Result<()> {
    let console = Console::start("console-not-found")?;
    let response = get(console.port, "/api/v1/nope", "")?;
    ensure!(status(&response) == "HTTP/1.1 404 Not Found");
    ensure!(body(&response) == "not found");
    let outside = get(console.port, "/nope", "")?;
    ensure!(status(&outside) == "HTTP/1.1 404 Not Found");
    Ok(())
}

fn console_method_not_allowed() -> anyhow::Result<()> {
    let console = Console::start("console-method")?;
    let response = request(console.port, "POST", "/", "", "")?;
    ensure!(status(&response) == "HTTP/1.1 405 Method Not Allowed");
    ensure!(body(&response) == "method not allowed");
    Ok(())
}

fn websocket_rejected() -> anyhow::Result<()> {
    let console = Console::start("console-ws-rejected")?;
    ensure!(websocket_response(console.port)?.starts_with("HTTP/1.1 400 Bad Request"));
    Ok(())
}

fn config_registry() -> anyhow::Result<()> {
    let console = Console::start("console-config")?;
    let response = get(console.port, "/api/v1/config", "")?;
    ensure!(status(&response) == "HTTP/1.1 200 OK");
    let parsed: serde_json::Value = serde_json::from_str(body(&response))?;
    let keys = parsed["keys"]
        .as_array()
        .ok_or_else(|| anyhow!("config response has no keys array"))?;
    ensure!(keys.len() == ghostlight_core::governance::config::KEYS.len());
    for (entry, definition) in keys.iter().zip(ghostlight_core::governance::config::KEYS) {
        ensure!(entry["key"] == definition.key);
        ensure!(entry.get("value").is_some());
        ensure!(entry["locked"].is_boolean());
        ensure!(!entry["description"].as_str().unwrap_or_default().is_empty());
        ensure!(matches!(
            entry["source"].as_str(),
            Some("org_mandatory" | "user" | "org_recommended" | "preset" | "builtin")
        ));
    }
    Ok(())
}

fn dns_rebind_denied() -> anyhow::Result<()> {
    let console = Console::start("console-dns-rebind")?;
    let response = request_with_host(
        console.port,
        "GET",
        "/api/v1/config",
        "evil.example.com",
        "",
        "",
    )?;
    ensure!(status(&response) == "HTTP/1.1 403 Forbidden");
    Ok(())
}

fn live_sessions() -> anyhow::Result<()> {
    let console = Console::start("console-sessions")?;
    let mut edge = console.mcp_edge()?;
    let mut stdin = edge
        .stdin
        .take()
        .ok_or_else(|| anyhow!("MCP edge has no stdin"))?;
    let mut stdout = BufReader::new(
        edge.stdout
            .take()
            .ok_or_else(|| anyhow!("MCP edge has no stdout"))?,
    );
    serde_json::to_writer(&mut stdin, &support::initialize_2025(1, "lightbox-console"))?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    let mut initialize_response = String::new();
    stdout.read_line(&mut initialize_response)?;
    let initialize_response: serde_json::Value = serde_json::from_str(&initialize_response)?;
    ensure!(initialize_response["id"] == 1);
    serde_json::to_writer(&mut stdin, &support::initialized_2025())?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let parsed = loop {
        let response = get(console.port, "/api/v1/sessions", "")?;
        ensure!(status(&response) == "HTTP/1.1 200 OK");
        let parsed: serde_json::Value = serde_json::from_str(body(&response))?;
        if parsed["workspaces"]
            .as_array()
            .is_some_and(|workspaces| !workspaces.is_empty())
        {
            break parsed;
        }
        ensure!(
            Instant::now() < deadline,
            "no implicit MCP 2025 workspace appeared: {parsed}"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    ensure!(parsed["live_session_count"].as_u64().unwrap_or(0) >= 1);
    let workspace = parsed["workspaces"]
        .as_array()
        .and_then(|workspaces| workspaces.first())
        .ok_or_else(|| anyhow!("no implicit MCP 2025 workspace"))?;
    ensure!(workspace["attached"] == 1);
    ensure!(workspace["active"] == 0);
    ensure!(workspace["owned_tab_ids"] == serde_json::json!([]));
    ensure!(workspace.get("workspaceId").is_none());
    ensure!(workspace.get("pid").is_none());
    Ok(())
}
