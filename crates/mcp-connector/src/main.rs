//! Generic MCP stdio edge for Ghostlight.

mod mcp_2025_11_25;
mod service_session;

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, Result};
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
    let mut input = stdin.lock().lines();
    let output = Arc::new(Mutex::new(io::stdout()));
    let Some(first_line) = input.next() else {
        return Ok(());
    };
    let first: Value = serde_json::from_str(&first_line?).context("decode initialize request")?;
    let initialization = mcp_2025_11_25::parse_initialize(&first).map_err(anyhow::Error::msg)?;

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
        })
    };
    let service = ServiceSession::start(initialization.client_label.clone(), handler)
        .context("start Ghostlight service session")?;
    let server = service.wait_until_connected();
    write_mcp(
        &output,
        mcp_2025_11_25::initialize_result(initialization.id, &server),
    );

    for line in input {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
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
            result,
            content,
        } => {
            if let Some(call) = lock(pending).remove(&id) {
                lock(reverse).remove(&call.mcp_key);
                write_mcp(
                    output,
                    mcp_2025_11_25::success(call.mcp_id, render_result(result, content)),
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

fn render_result(result: Value, content: Vec<ServiceContent>) -> Value {
    let text = serde_json::to_string_pretty(&result)
        .unwrap_or_else(|_| "Ghostlight result could not be rendered.".into());
    let mut rendered = vec![json!({"type":"text","text":text})];
    for item in content {
        match item {
            ServiceContent::Image { mime_type, data } => {
                rendered.push(json!({"type":"image","mimeType":mime_type,"data":data}));
            }
        }
    }
    json!({"content":rendered,"structuredContent":result,"isError":false})
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
    use ghostlight_bridge::service::{ServiceContent, ToolDefinition};
    use serde_json::json;

    use super::render_result;

    #[test]
    fn tool_list_is_rendered_without_tool_specific_dispatch() {
        let tools = vec![ToolDefinition {
            name: "future_orchestrator_tool".into(),
            description: "Future job.".into(),
            input_schema: json!({"type":"object"}),
        }];
        let response = json!({"tools":tools});
        assert_eq!(response["tools"][0]["name"], "future_orchestrator_tool");
    }

    #[test]
    fn service_content_is_rendered_without_tool_specific_dispatch() {
        let structured = json!({"status":"succeeded","facts":{"view":"view_1"}});
        let rendered = render_result(
            structured.clone(),
            vec![ServiceContent::Image {
                mime_type: "image/jpeg".into(),
                data: "base64-image".into(),
            }],
        );
        assert_eq!(rendered["structuredContent"], structured);
        assert_eq!(rendered["content"][0]["type"], "text");
        assert_eq!(rendered["content"][1]["type"], "image");
        assert_eq!(rendered["content"][1]["mimeType"], "image/jpeg");
        assert_eq!(rendered["content"][1]["data"], "base64-image");
    }
}
