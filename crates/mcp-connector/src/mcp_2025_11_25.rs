//! Exact MCP 2025-11-25 negotiation and JSON-RPC rendering.

use ghostlight_bridge::service::ServerProfile;
use serde_json::{json, Value};

/// MCP revision implemented by this exact edge state machine.
pub const PROTOCOL_VERSION: &str = "2025-11-25";
const JSONRPC_VERSION: &str = "2.0";

/// Validated initialization facts needed outside the MCP revision module.
pub struct Initialization {
    /// JSON-RPC request id to echo in the initialize response.
    pub id: Value,
    /// Bounded human-readable label forwarded for presentation and audit only.
    pub client_label: String,
}

/// Validate the first JSON-RPC request and extract generic session facts.
pub fn parse_initialize(message: &Value) -> Result<Initialization, String> {
    if message.get("jsonrpc").and_then(Value::as_str) != Some(JSONRPC_VERSION) {
        return Err("MCP initialize requires JSON-RPC 2.0".into());
    }
    if message.get("method").and_then(Value::as_str) != Some("initialize") {
        return Err("MCP initialize must be the first request".into());
    }
    let id = message
        .get("id")
        .cloned()
        .ok_or_else(|| "initialize request requires id".to_owned())?;
    if message
        .pointer("/params/protocolVersion")
        .and_then(Value::as_str)
        .is_none()
    {
        return Err("initialize requires params.protocolVersion".into());
    }
    let client_label = message
        .pointer("/params/clientInfo/name")
        .and_then(Value::as_str)
        .unwrap_or("MCP client")
        .chars()
        .take(100)
        .collect();
    Ok(Initialization { id, client_label })
}

/// Render the initialize response from orchestrator-owned product metadata.
pub fn initialize_result(id: Value, server: &ServerProfile) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {"listChanged": true}},
            "serverInfo": {"name": server.name, "version": server.version},
            "instructions": server.instructions
        }
    })
}

/// Render a successful JSON-RPC response.
pub fn success(id: Value, result: Value) -> Value {
    json!({"jsonrpc":JSONRPC_VERSION,"id":id,"result":result})
}

/// Render the standard notification emitted after a changed reconnect catalog.
pub fn tools_list_changed() -> Value {
    json!({"jsonrpc":JSONRPC_VERSION,"method":"notifications/tools/list_changed"})
}

/// Render a bounded JSON-RPC error response.
pub fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message.chars().take(500).collect::<String>()
        }
    })
}

#[cfg(test)]
mod tests {
    use ghostlight_bridge::service::ServerProfile;
    use serde_json::json;

    use super::{initialize_result, parse_initialize, PROTOCOL_VERSION};

    #[test]
    fn negotiation_selects_the_exact_supported_revision() {
        let request = json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"initialize",
            "params":{
                "protocolVersion":"future-revision",
                "clientInfo":{"name":"test client","version":"1"},
                "capabilities":{}
            }
        });
        let initialization = parse_initialize(&request).unwrap();
        let response = initialize_result(
            initialization.id,
            &ServerProfile {
                name: "ghostlight".into(),
                version: "1.0.0".into(),
                instructions: "service-owned".into(),
            },
        );
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(response["result"]["instructions"], "service-owned");
        assert_eq!(
            response["result"]["capabilities"]["tools"]["listChanged"],
            true
        );
    }
}
