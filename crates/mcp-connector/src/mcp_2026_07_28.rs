//! MCP 2026-07-28 compatibility discovery for the initialized tools-only edge.

use ghostlight_bridge::service::ServerProfile;
use serde_json::{json, Value};

use crate::mcp_2025_11_25::SUPPORTED_PROTOCOL_VERSIONS;

/// MCP revision whose stateless discovery request this compatibility shore accepts.
pub const PROTOCOL_VERSION: &str = "2026-07-28";
const JSONRPC_VERSION: &str = "2.0";

/// Validated facts from a compatibility discovery request.
pub struct Discovery {
    /// JSON-RPC request id to echo in the response.
    pub id: Value,
    /// Bounded human-readable label forwarded for presentation and audit only.
    pub client_label: String,
}

/// Validate a 2026-07-28 compatibility discovery request.
pub fn parse_discovery(message: &Value) -> Result<Discovery, String> {
    if message.get("jsonrpc").and_then(Value::as_str) != Some(JSONRPC_VERSION) {
        return Err("MCP discovery requires JSON-RPC 2.0".into());
    }
    if message.get("method").and_then(Value::as_str) != Some("server/discover") {
        return Err("MCP discovery requires server/discover".into());
    }
    let id = message
        .get("id")
        .filter(|id| !id.is_null())
        .cloned()
        .ok_or_else(|| "server/discover requires a non-null id".to_owned())?;
    if message
        .pointer("/params/_meta/io.modelcontextprotocol~1protocolVersion")
        .and_then(Value::as_str)
        != Some(PROTOCOL_VERSION)
    {
        return Err(format!(
            "server/discover requires protocol version {PROTOCOL_VERSION}"
        ));
    }
    if !message
        .pointer("/params/_meta/io.modelcontextprotocol~1clientCapabilities")
        .is_some_and(Value::is_object)
    {
        return Err("server/discover requires client capabilities".into());
    }
    let client_label = message
        .pointer("/params/_meta/io.modelcontextprotocol~1clientInfo/name")
        .and_then(Value::as_str)
        .unwrap_or("MCP client")
        .chars()
        .take(100)
        .collect();
    Ok(Discovery { id, client_label })
}

/// Advertise the initialized revisions this connector can serve after compatibility discovery.
pub fn discovery_result(id: Value, server: &ServerProfile) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": {
            "resultType": "complete",
            "supportedVersions": SUPPORTED_PROTOCOL_VERSIONS,
            "capabilities": {"tools": {"listChanged": true}},
            "_meta": {
                "io.modelcontextprotocol/serverInfo": {
                    "name": server.name,
                    "version": server.version
                }
            },
            "instructions": server.instructions,
            "ttlMs": 0,
            "cacheScope": "private"
        }
    })
}

#[cfg(test)]
mod tests {
    use ghostlight_bridge::service::ServerProfile;
    use serde_json::json;

    use super::{discovery_result, parse_discovery};

    #[test]
    fn compatibility_discovery_advertises_only_served_initialized_revisions() {
        let discovery = parse_discovery(&json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"server/discover",
            "params":{"_meta":{
                "io.modelcontextprotocol/protocolVersion":"2026-07-28",
                "io.modelcontextprotocol/clientInfo":{"name":"Antigravity","version":"1"},
                "io.modelcontextprotocol/clientCapabilities":{}
            }}
        }))
        .unwrap();
        assert_eq!(discovery.client_label, "Antigravity");
        let response = discovery_result(
            discovery.id,
            &ServerProfile {
                name: "ghostlight".into(),
                version: "1.0.0".into(),
                instructions: "service-owned".into(),
            },
        );
        assert_eq!(response["result"]["resultType"], "complete");
        assert_eq!(response["result"]["supportedVersions"][3], "2025-11-25");
        assert_eq!(response["result"]["cacheScope"], "private");
        assert_eq!(response["result"]["ttlMs"], 0);
    }
}
