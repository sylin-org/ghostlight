//! JSON-RPC 2.0 server over stdio (the mcp-server role).
//!
//! Reads newline-delimited JSON-RPC from stdin, handles `initialize` / `tools/list` / `tools/call`,
//! and writes responses to stdout (one compact JSON object per line). `tools/call` routes through
//! [`crate::governance::dispatch`] (the v1.0 no-op policy/audit seams) and then forwards to the
//! extension via the [`Browser`] handle. stdout is reserved for the protocol stream; operational
//! logs go to stderr.
//!
//! `tools/call` runs concurrently: each call is spawned on its own task (so a slow or waiting call
//! never blocks `initialize`, `ping`, or later requests) and every response -- inline or from a
//! spawned call -- funnels through a single writer task that owns stdout, so lines are never
//! interleaved mid-write.

use crate::browser::redact;
use crate::governance::dispatch;
use crate::governance::policy::Config;
use crate::transport::executor::Browser;
use crate::transport::mcp::tools::{is_known_tool, TOOLS_JSON};
use crate::transport::mcp::types::{text_content, JsonRpcResponse};
use crate::{Result, ToolError};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

/// MCP protocol version this server speaks.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// How long a `tools/call` waits for the extension channel to come up before failing. The
/// first call of a session races the native-messaging handshake; waiting briefly turns the
/// single most common spurious failure into a success. Slated to become governance config
/// key `engine.connection.first_call_wait_ms` per ADR-0019 (proposed); a hardcoded constant
/// until the config plumbing lands.
const FIRST_CALL_WAIT_MS: u64 = 5000;

/// Run the stdio MCP server loop until stdin closes. `browser` is the (shared) handle to the
/// extension; tool calls are forwarded through it.
pub async fn run(browser: Browser) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    // Governance config in force. The policy engine ships in a later stage, so this is the
    // built-in "Minimal" preset (safe-by-default). When the manifest engine lands it resolves
    // this per session.
    let config = Config::default();

    let (tx, mut rx) = mpsc::unbounded_channel::<JsonRpcResponse>();

    // A single writer owns stdout so responses -- including those from spawned `tools/call`
    // tasks -- never interleave mid-write. `debug` is cloned before the spawn so both the
    // writer and the read loop below can record the MCP boundary.
    let debug = browser.debug().clone();
    let writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(resp) = rx.recv().await {
            let mut buf = match serde_json::to_string(&resp) {
                Ok(buf) => buf,
                Err(e) => {
                    tracing::warn!(error = %e, "dropping unserializable response");
                    continue;
                }
            };
            if debug.is_enabled() {
                // Use the already-typed id (do not re-parse the whole -- possibly large -- body).
                let id = resp.id.as_ref().map(Value::to_string).unwrap_or_default();
                debug.mcp_response(&id, &buf);
            }
            buf.push('\n');
            if stdout.write_all(buf.as_bytes()).await.is_err() || stdout.flush().await.is_err() {
                break;
            }
        }
    });

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(resp) = handle_line(&browser, config, line, &tx).await {
            let _ = tx.send(resp);
        }
    }
    drop(tx);
    let _ = writer.await;
    Ok(())
}

/// Parse and route one JSON-RPC line.
///
/// Returns `Some(response)` for requests (an `id` member is present, even if `null`) and `None` for
/// notifications (no `id` member) and for lines we cannot parse at all. Fields are read from a raw
/// [`Value`] so a structurally invalid but id-bearing request still gets an addressable `-32600`.
async fn handle_line(
    browser: &Browser,
    config: Config,
    line: &str,
    tx: &mpsc::UnboundedSender<JsonRpcResponse>,
) -> Option<JsonRpcResponse> {
    let raw: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "dropping unparseable JSON-RPC line");
            return None;
        }
    };

    let is_notification = raw.get("id").is_none();
    let id = raw.get("id").cloned();

    let Some(method) = raw.get("method").and_then(Value::as_str) else {
        return if is_notification {
            tracing::debug!("dropping malformed notification (no method)");
            None
        } else {
            Some(JsonRpcResponse::error(
                id,
                -32600,
                "Invalid Request: missing or non-string 'method'",
            ))
        };
    };

    if browser.debug().is_enabled() {
        let id_str = id.as_ref().map(Value::to_string).unwrap_or_default();
        browser.debug().mcp_request(method, &id_str, line);
    }

    match method {
        "initialize" => {
            // Record the MCP client's self-reported identity (clientInfo.name [+ version]), if it
            // sent one, for `browser-mcp doctor`/`status` to display. Missing params/clientInfo, or
            // non-string fields, are silently fine: this is best-effort observability, not part of
            // the protocol contract, and the response below never depends on it.
            if let Some(client_info) = raw.get("params").and_then(|p| p.get("clientInfo")) {
                if let Some(name) = client_info.get("name").and_then(Value::as_str) {
                    let ident = match client_info.get("version").and_then(Value::as_str) {
                        Some(version) => format!("{name} {version}"),
                        None => name.to_string(),
                    };
                    browser.debug().set_client(&ident);
                }
            }
            // Warm the extension channel while the client finishes its handshake. The extension
            // side initiates the connection (Chrome spawns the native-host, which dials the
            // endpoint this process has served since startup), so there is nothing to dial from
            // here; this watcher verifies readiness and records the outcome.
            tokio::spawn({
                let browser = browser.clone();
                async move {
                    let started = Instant::now();
                    if browser
                        .wait_connected(Duration::from_millis(FIRST_CALL_WAIT_MS))
                        .await
                    {
                        tracing::info!(
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "extension channel ready"
                        );
                    } else {
                        tracing::info!(
                            "extension channel not ready within the warmup window; \
                             the first tools/call will wait for it"
                        );
                    }
                }
            });
            Some(JsonRpcResponse::success(id, initialize_result()))
        }
        "tools/list" => Some(JsonRpcResponse::success(id, tools_list_result())),
        "tools/call" => {
            let browser = browser.clone();
            let tx = tx.clone();
            let params = raw.get("params").cloned();
            tokio::spawn(async move {
                let resp = handle_tools_call(&browser, config, id, params.as_ref()).await;
                let _ = tx.send(resp);
            });
            None
        }
        "ping" => Some(JsonRpcResponse::success(id, json!({}))),
        _ if is_notification => {
            tracing::debug!(method, "ignoring unknown notification");
            None
        }
        other => Some(JsonRpcResponse::error(
            id,
            -32601,
            format!("Method not found: {other}"),
        )),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "browser-mcp", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// The advertised surface: the embedded sacred fixture (`{ "tools": [...] }`) verbatim. In all-open
/// v1.0 the full surface is advertised unconditionally -- there is no overlay to filter it.
fn tools_list_result() -> Value {
    serde_json::from_str(TOOLS_JSON).expect("embedded tools.json is valid")
}

async fn handle_tools_call(
    browser: &Browser,
    config: Config,
    id: Option<Value>,
    params: Option<&Value>,
) -> JsonRpcResponse {
    let Some(name) = params.and_then(|p| p.get("name")).and_then(Value::as_str) else {
        return JsonRpcResponse::error(id, -32602, "tools/call requires a string 'name'");
    };
    let args = params
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or(Value::Null);

    // Unknown tool names are rejected before dispatch (and before waiting on the extension
    // channel at all): this is a client-request problem, not a browser/extension problem, and the
    // client should learn that instantly regardless of whether an extension is even connected.
    // The extension keeps its own `Unknown tool: ...` guard as a safety net (defense in depth);
    // this pre-check just means well-formed clients never round-trip to hit it.
    if !is_known_tool(name) {
        let err = ToolError::invalid_request(format!("Unknown tool: {name}"))
            .next_step("call tools/list and use one of the advertised tool names");
        return JsonRpcResponse::success(id, error_result(err));
    }

    // v1.0 engine: the policy and audit seams are no-ops (all-open). The v1.5 overlay slots in here
    // without touching this code (see src/dispatch.rs).
    let _decision = dispatch::policy_check(name);
    dispatch::audit(name);

    // Bounded first-call wait: the first call of a session races the extension handshake.
    // Wait briefly for the channel instead of failing a healthy session (also covers calls
    // arriving during a mid-session reconnect). If the wait times out, `waited` stays `None` and
    // control falls through to `Browser::call` below, which fails fast with the canonical
    // "extension not connected" `ToolError` -- one hop-attributed message, not two to keep in sync.
    let mut waited: Option<Duration> = None;
    if !browser.is_connected() {
        let started = Instant::now();
        if browser
            .wait_connected(Duration::from_millis(FIRST_CALL_WAIT_MS))
            .await
        {
            waited = Some(started.elapsed());
        } else {
            tracing::warn!(
                tool = name,
                "tools/call failed: extension channel never came up"
            );
        }
    }

    match browser.call(name, &args).await {
        // The extension returns an MCP result object (`{ content: [...] }`). The engine is truthful:
        // read_page carries secret field values under a `secret_value=` marker; the governance
        // overlay rewrites that marker here (redacting per `content.security.secrets.redact`) before
        // the result leaves the binary. Other tools pass through untouched.
        Ok(mut result) => {
            if name == "read_page" {
                redact::apply_to_result(&mut result, config.secrets_redact());
            }
            if let Some(waited) = waited {
                append_wait_note(&mut result, waited);
            }
            JsonRpcResponse::success(id, result)
        }
        // A tool execution failure is an MCP tool error result (isError), not a JSON-RPC error.
        // The rendered text is exactly the hop-attributed ToolError Display: no "Error: " prefix.
        Err(e) => {
            let mut result = error_result(e);
            if let Some(waited) = waited {
                append_wait_note(&mut result, waited);
            }
            JsonRpcResponse::success(id, result)
        }
    }
}

/// Build an MCP tool error result (`{ content: [...], isError: true }`) from a hop-attributed
/// [`ToolError`]. The result text is exactly the error's `Display`:
/// `[hop: <hop>] <message>. Next step: <next step>.`
fn error_result(err: ToolError) -> Value {
    let mut result = text_content(err.to_string());
    if let Some(obj) = result.as_object_mut() {
        obj.insert("isError".into(), json!(true));
    }
    result
}

/// Append the truthful handshake-wait note as a final text block on an MCP tool result.
fn append_wait_note(result: &mut Value, waited: Duration) {
    let note = format!(
        "(waited {:.1}s for browser extension handshake)",
        waited.as_secs_f64()
    );
    if let Some(content) = result.get_mut("content").and_then(Value::as_array_mut) {
        content.push(json!({ "type": "text", "text": note }));
    }
}
