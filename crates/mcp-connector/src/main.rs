//! Generic MCP stdio edge for Ghostlight.

mod mcp_2025_11_25;
mod mcp_2026_07_28;
mod service_session;

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, Result};
use ghostlight_bridge::framing::{read_json_line, FrameError};
use ghostlight_bridge::service::{ServiceContent, ServiceRequest, ServiceResponse};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::service_session::{ServiceEvent, ServiceSession};

type Output = Arc<Mutex<io::Stdout>>;

struct PendingCall {
    mcp_id: Value,
    mcp_key: String,
}

fn main() -> Result<()> {
    let stdin = io::stdin();
    // Bounded, typed reads through the same framing every other process boundary in this
    // codebase uses -- never the raw `Lines` iterator this used to be. `Lines`/`read_line` has no
    // size cap of its own (this is the external, real-MCP-client-facing boundary, the one this
    // bound exists to protect) and returns a hard `io::Error` for a single invalid-UTF-8 byte,
    // which propagated straight out of `main` and ended the whole connector process -- no
    // JSON-RPC error, no chance to recover within the same session. Reading bytes through
    // `serde_json::from_slice` instead turns both failures into an ordinary, recoverable parse
    // error.
    let mut input = io::BufReader::new(stdin.lock());
    let output = Arc::new(Mutex::new(io::stdout()));
    let mut first: Value = match read_json_line(&mut input).context("read first MCP request")? {
        Some(value) => value,
        None => return Ok(()),
    };
    let discovery = if first.get("method").and_then(Value::as_str) == Some("server/discover") {
        Some(mcp_2026_07_28::parse_discovery(&first).map_err(anyhow::Error::msg)?)
    } else {
        None
    };
    let initial_client_label = match &discovery {
        Some(discovery) => discovery.client_label.clone(),
        None => {
            mcp_2025_11_25::parse_initialize(&first)
                .map_err(anyhow::Error::msg)?
                .client_label
        }
    };

    let pending: Arc<Mutex<HashMap<String, PendingCall>>> = Arc::new(Mutex::new(HashMap::new()));
    let reverse: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let initialized = Arc::new(AtomicBool::new(false));
    let handler = {
        let output = Arc::clone(&output);
        let pending = Arc::clone(&pending);
        let reverse = Arc::clone(&reverse);
        let initialized = Arc::clone(&initialized);
        Arc::new(move |event| match event {
            ServiceEvent::Response(response) => {
                handle_service_response(response, &output, &pending, &reverse);
            }
            ServiceEvent::Disconnected => {
                fail_pending(&output, &pending, &reverse);
            }
            ServiceEvent::Connected { catalog_changed } => {
                if catalog_changed && initialized.load(Ordering::SeqCst) {
                    write_mcp(&output, mcp_2025_11_25::tools_list_changed());
                }
            }
            ServiceEvent::CatalogChanged => {
                if initialized.load(Ordering::SeqCst) {
                    write_mcp(&output, mcp_2025_11_25::tools_list_changed());
                }
            }
        })
    };
    let service = ServiceSession::start(initial_client_label, handler)
        .context("start Ghostlight service session")?;
    let server = service.wait_until_connected();
    if let Some(discovery) = discovery {
        write_mcp(
            &output,
            mcp_2026_07_28::discovery_result(discovery.id, &server),
        );
        first = match read_json_line(&mut input).context("read initialize after discovery")? {
            Some(value) => value,
            None => return Ok(()),
        };
    }
    let initialization = mcp_2025_11_25::parse_initialize(&first).map_err(anyhow::Error::msg)?;
    write_mcp(
        &output,
        mcp_2025_11_25::initialize_result(
            initialization.id,
            initialization.protocol_version,
            &server,
        ),
    );

    loop {
        let message: Value = match read_json_line(&mut input) {
            Ok(None) => break,
            Ok(Some(value)) => value,
            // A malformed line -- including one that is not valid UTF-8, which used to be a hard
            // `io::Error` that ended the process via `?` -- is answered like any other JSON-RPC
            // parse failure and the session continues.
            Err(FrameError::Json(error)) => {
                write_mcp(
                    &output,
                    mcp_2025_11_25::rpc_error(
                        Value::Null,
                        -32700,
                        &format!("Parse error: {error}"),
                    ),
                );
                continue;
            }
            // A line over the byte bound is read only partially before being rejected, so the
            // stream is left mid-line: there is no safe boundary left to resynchronize on. Tell
            // the client once, then end this session cleanly rather than risk misreading the
            // remainder of the oversized line as a fresh, unrelated message.
            Err(error @ FrameError::TooLarge) => {
                write_mcp(
                    &output,
                    mcp_2025_11_25::rpc_error(
                        Value::Null,
                        -32700,
                        &format!("Parse error: {error}"),
                    ),
                );
                break;
            }
            Err(error) => return Err(error).context("read a request line"),
        };
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = message.get("id").cloned();
        match method {
            "notifications/initialized" => {
                initialized.store(true, Ordering::SeqCst);
            }
            "notifications/cancelled" => {
                if let Some(request_id) = message.pointer("/params/requestId") {
                    let key = serde_json::to_string(request_id).unwrap_or_default();
                    if let Some(bridge_id) = lock(&reverse).get(&key).cloned() {
                        let _ = service.send(&ServiceRequest::Cancel { id: bridge_id });
                    }
                }
            }
            "ping" => {
                if let Some(id) = id {
                    write_mcp(&output, mcp_2025_11_25::success(id, json!({})));
                }
            }
            _ if !initialized.load(Ordering::SeqCst) && id.is_some() => {
                write_mcp(
                    &output,
                    mcp_2025_11_25::rpc_error(
                        id.unwrap_or(Value::Null),
                        -32002,
                        "Server not initialized",
                    ),
                );
            }
            "tools/list" => {
                if let Some(id) = id {
                    match service.catalog() {
                        Some(catalog) => write_mcp(
                            &output,
                            mcp_2025_11_25::success(id, json!({"tools":catalog})),
                        ),
                        None => write_mcp(
                            &output,
                            mcp_2025_11_25::rpc_error(
                                id,
                                -32603,
                                "Ghostlight service is temporarily unavailable",
                            ),
                        ),
                    }
                }
            }
            "tools/call" => dispatch_tool_call(&message, id, &service, &output, &pending, &reverse),
            _ if id.is_none() => {}
            _ => write_mcp(
                &output,
                mcp_2025_11_25::rpc_error(id.unwrap_or(Value::Null), -32601, "Method not found"),
            ),
        }
    }
    Ok(())
}

fn dispatch_tool_call(
    message: &Value,
    id: Option<Value>,
    service: &ServiceSession,
    output: &Output,
    pending: &Mutex<HashMap<String, PendingCall>>,
    reverse: &Mutex<HashMap<String, String>>,
) {
    let Some(id) = id else { return };
    let Some(name) = message.pointer("/params/name").and_then(Value::as_str) else {
        write_mcp(
            output,
            mcp_2025_11_25::rpc_error(id, -32602, "tools/call requires params.name"),
        );
        return;
    };
    let arguments = message
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        write_mcp(
            output,
            mcp_2025_11_25::rpc_error(id, -32602, "params.arguments must be an object"),
        );
        return;
    }
    let bridge_id = format!("edge_{}", Uuid::new_v4().simple());
    let key = serde_json::to_string(&id).unwrap_or_default();
    lock(pending).insert(
        bridge_id.clone(),
        PendingCall {
            mcp_id: id,
            mcp_key: key.clone(),
        },
    );
    lock(reverse).insert(key, bridge_id.clone());
    if let Err(error) = service.send(&ServiceRequest::Invoke {
        id: bridge_id.clone(),
        tool: name.into(),
        input: arguments,
        deadline_ms: None,
    }) {
        if let Some(call) = lock(pending).remove(&bridge_id) {
            lock(reverse).remove(&call.mcp_key);
            write_mcp(
                output,
                mcp_2025_11_25::rpc_error(
                    call.mcp_id,
                    -32603,
                    &format!("Ghostlight service dispatch is unavailable: {error}"),
                ),
            );
        }
    }
}

fn handle_service_response(
    response: ServiceResponse,
    output: &Output,
    pending: &Mutex<HashMap<String, PendingCall>>,
    reverse: &Mutex<HashMap<String, String>>,
) {
    match response {
        ServiceResponse::Result {
            id,
            text,
            result,
            is_error,
            content,
        } => {
            if let Some(call) = lock(pending).remove(&id) {
                lock(reverse).remove(&call.mcp_key);
                write_mcp(
                    output,
                    mcp_2025_11_25::success(
                        call.mcp_id,
                        render_result(text, result, is_error, content),
                    ),
                );
            }
        }
        ServiceResponse::Error {
            id: Some(id),
            code,
            message,
        } => {
            if let Some(call) = lock(pending).remove(&id) {
                lock(reverse).remove(&call.mcp_key);
                write_mcp(
                    output,
                    mcp_2025_11_25::rpc_error(call.mcp_id, -32603, &format!("{code}: {message}")),
                );
            }
        }
        _ => {}
    }
}

fn fail_pending(
    output: &Output,
    pending: &Mutex<HashMap<String, PendingCall>>,
    reverse: &Mutex<HashMap<String, String>>,
) {
    let calls: Vec<_> = lock(pending).drain().map(|(_, call)| call).collect();
    lock(reverse).clear();
    for call in calls {
        write_mcp(
            output,
            mcp_2025_11_25::rpc_error(
                call.mcp_id,
                -32603,
                "Ghostlight service disconnected after dispatch; the request outcome is unavailable",
            ),
        );
    }
}

fn render_result(
    text: String,
    result: Value,
    is_error: bool,
    content: Vec<ServiceContent>,
) -> Value {
    let mut rendered = vec![json!({"type":"text","text":text})];
    for item in content {
        match item {
            ServiceContent::Image { mime_type, data } => {
                rendered.push(json!({"type":"image","mimeType":mime_type,"data":data}));
            }
        }
    }
    json!({"content":rendered,"structuredContent":result,"isError":is_error})
}

fn write_mcp(output: &Mutex<io::Stdout>, value: Value) {
    let mut output = lock(output);
    let _ = serde_json::to_writer(&mut *output, &value);
    let _ = output.write_all(b"\n");
    let _ = output.flush();
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use ghostlight_bridge::service::{ServiceContent, ToolAnnotations, ToolDefinition};
    use serde_json::json;

    use super::render_result;

    #[test]
    fn tool_list_is_rendered_without_tool_specific_dispatch() {
        let tools = vec![ToolDefinition {
            name: "future_orchestrator_tool".into(),
            description: "Future job.".into(),
            input_schema: json!({"type":"object"}),
            output_schema: Some(json!({"type":"object","required":["status"]})),
            annotations: Some(ToolAnnotations {
                title: Some("Future job".into()),
                read_only_hint: Some(true),
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
            }),
        }];
        let response = json!({"tools":tools});
        assert_eq!(response["tools"][0]["name"], "future_orchestrator_tool");
        assert_eq!(response["tools"][0]["outputSchema"]["type"], "object");
        assert_eq!(response["tools"][0]["annotations"]["readOnlyHint"], true);
    }

    #[test]
    fn service_content_is_rendered_without_tool_specific_dispatch() {
        let structured = json!({"status":"succeeded","facts":{"view":"view_1"}});
        let rendered = render_result(
            "Opened the page.".into(),
            structured.clone(),
            false,
            vec![ServiceContent::Image {
                mime_type: "image/jpeg".into(),
                data: "base64-image".into(),
            }],
        );
        assert_eq!(rendered["structuredContent"], structured);
        assert_eq!(rendered["content"][0]["type"], "text");
        assert_eq!(rendered["content"][0]["text"], "Opened the page.");
        assert_eq!(rendered["content"][1]["type"], "image");
        assert_eq!(rendered["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(rendered["content"][1]["data"], "base64-image");
        assert_eq!(rendered["isError"], false);
    }

    #[test]
    fn authored_failure_is_rendered_as_a_tool_error() {
        let structured = json!({"status":"refused","reason":"host_not_allowed"});
        let rendered = render_result(
            "Refused navigation because the host is not allowed.".into(),
            structured.clone(),
            true,
            vec![],
        );
        assert_eq!(rendered["structuredContent"], structured);
        assert_eq!(
            rendered["content"][0]["text"],
            "Refused navigation because the host is not allowed."
        );
        assert_eq!(rendered["isError"], true);
    }
}
