//! MCP JSON-RPC 2.0 response type and small result builders.
//!
//! Requests are parsed field-by-field from a raw `serde_json::Value` in `server::handle_line` (so
//! a structurally invalid but id-bearing request still gets an addressable error), so there is no
//! typed request struct here -- only the response and result builders.

use serde::Serialize;
use serde_json::{json, Value};

/// A JSON-RPC 2.0 response to the MCP client.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

impl JsonRpcResponse {
    /// A success response carrying `result`. `id` is echoed as-is (including a present `null`).
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// An error response with a JSON-RPC error `code` and `message`.
    pub fn error(id: Option<Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(json!({ "code": code, "message": message.into() })),
        }
    }
}

/// Build an MCP tool result carrying a single text block:
/// `{ "content": [ { "type": "text", "text": ... } ] }`.
pub fn text_content(text: impl Into<String>) -> Value {
    json!({ "content": [ { "type": "text", "text": text.into() } ] })
}
