// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The manage.web adapter -- a standalone loopback HTTP UI for observing runtime state.
//!
//! This listener is not an MCP transport. It never upgrades to WebSocket and never admits tool
//! work. It exposes only read-only management routes:
//! - `GET /` -- the embedded HTML shell.
//! - `GET /manage.css`, `GET /manage.js` -- the shell's static assets.
//! - `GET /api/v1/config` -- the provenance-aware config view.
//! - `GET /api/v1/sessions` -- the live-sessions/groups view.

use crate::hub::manage::assets;
use crate::hub::ServiceContext;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The only address the management listener may bind.
pub const DEFAULT_BIND: &str = "127.0.0.1";

/// The default management listener port.
pub const DEFAULT_PORT: u16 = 4180;

const MAX_REQUEST_HEAD_BYTES: usize = 16 * 1024;

/// Resolve the management listener port. Tests may request port zero for an OS-assigned port.
fn resolve_port() -> u16 {
    std::env::var("GHOSTLIGHT_MANAGE_WEB_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// The live `manage.web.enabled` resolution. An org can take the Console off-line without
/// affecting local tool ingestion over the OS-authenticated pipe.
pub fn enabled(store: &crate::governance::config::reload::ConfigStore) -> bool {
    let resolution = store.current_resolution();
    let resolved = resolution
        .get(crate::governance::config::MANAGE_WEB_ENABLED)
        .expect("registered key resolves");
    resolved.value.as_bool().unwrap_or(true)
}

/// Run the standalone loopback management listener. Bind failures are logged and do not affect
/// local MCP-edge bridge streams.
pub async fn run(ctx: ServiceContext) {
    let port = resolve_port();
    let addr = format!("{DEFAULT_BIND}:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::warn!(%error, %addr, "manage.web listener failed to bind");
            return;
        }
    };

    let actual_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    ctx.debug_sink.set_manage_web_port(actual_port);
    tracing::info!(addr = %DEFAULT_BIND, port = actual_port, "manage.web listening");

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(error) => {
                tracing::warn!(%error, "manage.web accept failed");
                continue;
            }
        };
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, peer_addr, &ctx).await {
                tracing::debug!(%error, "manage.web connection ended with an error");
            }
        });
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    ctx: &ServiceContext,
) -> crate::Result<()> {
    let mut buffer = Vec::with_capacity(4096);
    loop {
        if buffer.len() >= MAX_REQUEST_HEAD_BYTES {
            write_http_error(&mut stream, 431, "Request Header Fields Too Large").await?;
            return Ok(());
        }

        let mut chunk = [0_u8; 4096];
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..count]);
        if buffer.len() > MAX_REQUEST_HEAD_BYTES {
            write_http_error(&mut stream, 431, "Request Header Fields Too Large").await?;
            return Ok(());
        }

        let Some((request, _consumed)) = parse_http_request(&buffer) else {
            continue;
        };

        let websocket_attempt = header(&request.headers, "Upgrade")
            .map(|value| value.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false)
            || header(&request.headers, "Connection")
                .map(|value| {
                    value
                        .split(',')
                        .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
                })
                .unwrap_or(false);
        if websocket_attempt {
            write_http_error(&mut stream, 400, "Bad Request").await?;
            return Ok(());
        }

        let path = request
            .path
            .split_once('?')
            .map_or(request.path.as_str(), |p| p.0);
        return route(
            &mut stream,
            &request.method,
            path,
            &request.headers,
            ctx,
            peer_addr,
        )
        .await;
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

fn parse_http_request(buffer: &[u8]) -> Option<(HttpRequest, usize)> {
    let text = std::str::from_utf8(buffer).ok()?;
    let header_end = text.find("\r\n\r\n")?;
    let mut lines = text[..header_end].split("\r\n");
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_string();
    let path = request_line.next()?.to_string();
    request_line.next()?;
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect();
    Some((
        HttpRequest {
            method,
            path,
            headers,
        },
        header_end + 4,
    ))
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// Whether an HTTP `Host` header names this loopback listener.
fn host_is_loopback(host: &str) -> bool {
    let normalized = host.trim().to_ascii_lowercase();
    let hostname = if let Some(rest) = normalized.strip_prefix('[') {
        match rest.split_once(']') {
            Some((inside, _)) => format!("[{inside}]"),
            None => return false,
        }
    } else {
        normalized
            .rsplit_once(':')
            .map_or(normalized.as_str(), |(name, _)| name)
            .to_string()
    };
    matches!(hostname.as_str(), "localhost" | "127.0.0.1" | "[::1]")
}

/// Every path this read-only router recognizes.
pub(crate) fn is_known_path(stripped_path: &str) -> bool {
    matches!(
        stripped_path,
        "/" | "/manage.css" | "/manage.js" | "/api/v1/config" | "/api/v1/sessions"
    )
}

async fn route(
    stream: &mut TcpStream,
    method: &str,
    stripped_path: &str,
    headers: &[(String, String)],
    ctx: &ServiceContext,
    peer_addr: SocketAddr,
) -> crate::Result<()> {
    if !enabled(&ctx.store) {
        write_plain_error(stream, 404, "Not Found", "not found").await?;
        finish_response(stream).await;
        return Ok(());
    }

    if !peer_addr.ip().is_loopback() {
        write_http_error(stream, 403, "Forbidden").await?;
        return Ok(());
    }

    if !header(headers, "Host")
        .map(host_is_loopback)
        .unwrap_or(false)
    {
        tracing::info!("manage.web request refused: missing or non-loopback Host header");
        write_http_error(stream, 403, "Forbidden").await?;
        return Ok(());
    }

    let result = match (method, stripped_path) {
        ("GET", "/") => write_asset(stream, "text/html; charset=utf-8", assets::INDEX_HTML).await,
        ("GET", "/manage.css") => {
            write_asset(stream, "text/css; charset=utf-8", assets::MANAGE_CSS).await
        }
        ("GET", "/manage.js") => {
            write_asset(
                stream,
                "application/javascript; charset=utf-8",
                assets::MANAGE_JS,
            )
            .await
        }
        ("GET", "/api/v1/config") => write_config_response(stream, ctx).await,
        ("GET", "/api/v1/sessions") => write_sessions_response(stream, ctx).await,
        _ if is_known_path(stripped_path) => {
            write_plain_error(stream, 405, "Method Not Allowed", "method not allowed").await
        }
        _ => write_plain_error(stream, 404, "Not Found", "not found").await,
    };
    result?;
    finish_response(stream).await;
    Ok(())
}

async fn finish_response(stream: &mut TcpStream) {
    stream.flush().await.ok();
    stream.shutdown().await.ok();
}

async fn write_config_response(stream: &mut TcpStream, ctx: &ServiceContext) -> crate::Result<()> {
    let payload = config_payload(&ctx.store.current_resolution()).to_string();
    write_json(stream, 200, "OK", &payload).await
}

fn config_payload(resolution: &crate::governance::config::layers::Resolution) -> serde_json::Value {
    let keys: Vec<serde_json::Value> = resolution
        .iter()
        .map(|(key, resolved)| {
            let description = crate::governance::config::key_def(key)
                .map(|def| def.description)
                .unwrap_or_default();
            serde_json::json!({
                "key": key,
                "value": resolved.value,
                "source": resolved.source.as_str(),
                "locked": resolved.locked,
                "description": description,
            })
        })
        .collect();
    serde_json::json!({ "keys": keys })
}

async fn write_sessions_response(
    stream: &mut TcpStream,
    ctx: &ServiceContext,
) -> crate::Result<()> {
    let live_session_count = ctx.live_sessions.load(std::sync::atomic::Ordering::Relaxed);
    let summaries = ctx.workspaces.summaries();
    let payload = sessions_payload(&summaries, live_session_count).to_string();
    write_json(stream, 200, "OK", &payload).await
}

fn sessions_payload(
    summaries: &[crate::hub::workspace::WorkspaceSummary],
    live_session_count: usize,
) -> serde_json::Value {
    let workspaces: Vec<serde_json::Value> = summaries
        .iter()
        .map(|summary| {
            serde_json::json!({
                "attached": summary.attached,
                "active": summary.active,
                "owned_tab_ids": summary.owned_tab_ids,
            })
        })
        .collect();
    serde_json::json!({
        "live_session_count": live_session_count,
        "workspaces": workspaces,
        "note": "Workspace bearer handles and OS-user principals are intentionally omitted. \
                 Management HTTP requests are not client sessions.",
    })
}

async fn write_asset(stream: &mut TcpStream, content_type: &str, body: &str) -> crate::Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn write_json(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    payload: &str,
) -> crate::Result<()> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn write_plain_error(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &str,
) -> crate::Result<()> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn write_http_error(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
) -> std::io::Result<()> {
    let response =
        format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
    stream.write_all(response.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::config::layers::{resolve, LayerInputs};

    #[test]
    fn parses_complete_http_request_head() {
        let raw = b"GET /api/v1/config?full=1 HTTP/1.1\r\nHost: localhost:4180\r\n\r\n";
        let (request, consumed) = parse_http_request(raw).expect("complete request");
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, "/api/v1/config?full=1");
        assert_eq!(header(&request.headers, "host"), Some("localhost:4180"));
        assert_eq!(consumed, raw.len());
    }

    #[test]
    fn loopback_host_check_rejects_rebound_names() {
        assert!(host_is_loopback("localhost:4180"));
        assert!(host_is_loopback("127.0.0.1"));
        assert!(host_is_loopback("[::1]:4180"));
        assert!(!host_is_loopback("attacker.example"));
        assert!(!host_is_loopback("localhost.attacker.example"));
    }

    #[test]
    fn config_payload_emits_every_registered_key_in_registry_order() {
        let resolution = resolve(&LayerInputs::default());
        let payload = config_payload(&resolution);
        let keys = payload["keys"].as_array().expect("keys array");
        let expected: Vec<&str> = crate::governance::config::KEYS
            .iter()
            .map(|definition| definition.key)
            .collect();
        assert_eq!(keys.len(), expected.len());
        for (entry, key) in keys.iter().zip(expected.iter()) {
            assert_eq!(entry["key"], *key);
            assert!(entry.get("value").is_some(), "{key}: value present");
            assert!(entry["source"].is_string(), "{key}: source string");
            assert!(entry["locked"].is_boolean(), "{key}: locked bool");
            assert!(
                entry["description"].is_string(),
                "{key}: description string"
            );
        }
    }

    #[test]
    fn config_payload_reflects_an_org_mandatory_key_as_locked() {
        let mut inputs = LayerInputs::default();
        inputs
            .org_mandatory
            .insert("audit.enabled".to_string(), serde_json::json!(true));
        let payload = config_payload(&resolve(&inputs));
        let entry = payload["keys"]
            .as_array()
            .unwrap()
            .iter()
            .find(|key| key["key"] == "audit.enabled")
            .expect("audit.enabled is registered");
        assert_eq!(entry["source"], "org_mandatory");
        assert_eq!(entry["locked"], true);
        assert_eq!(entry["value"], true);
    }

    #[test]
    fn sessions_payload_serialises_count_bindings_and_note() {
        let summaries = vec![crate::hub::workspace::WorkspaceSummary {
            attached: 1,
            active: 2,
            owned_tab_ids: vec![7, 9],
        }];
        let payload = sessions_payload(&summaries, 3);
        assert_eq!(payload["live_session_count"], 3);
        let workspaces = payload["workspaces"].as_array().unwrap();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0]["attached"], 1);
        assert_eq!(workspaces[0]["active"], 2);
        assert_eq!(workspaces[0]["owned_tab_ids"], serde_json::json!([7, 9]));
        assert!(payload["note"].as_str().unwrap().contains("bearer handles"));
    }
}
