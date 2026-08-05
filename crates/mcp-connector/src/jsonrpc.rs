// SPDX-License-Identifier: Apache-2.0 OR MIT
//! JSON-RPC 2.0 parsing and rendering for Ghostlight's MCP stdio shore.
//!
//! This module knows envelopes, ids, and line framing. It deliberately knows neither MCP revision
//! nor Ghostlight product operations.

use serde_json::{json, Map, Number, Value};
use std::hash::{Hash, Hasher};
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// JSON-RPC parse error.
pub const PARSE_ERROR: i64 = -32700;
/// JSON-RPC invalid request error.
pub const INVALID_REQUEST: i64 = -32600;
/// JSON-RPC method-not-found error.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// JSON-RPC invalid-params error.
pub const INVALID_PARAMS: i64 = -32602;
/// A JSON-RPC request id preserved without string coercion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestId {
    /// A JSON number.
    Number(Number),
    /// A JSON string.
    String(String),
}

impl Hash for RequestId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Number(number) => {
                0_u8.hash(state);
                number.to_string().hash(state);
            }
            Self::String(value) => {
                1_u8.hash(state);
                value.hash(state);
            }
        }
    }
}

impl RequestId {
    /// Parse an id value, rejecting JSON-RPC-invalid id kinds.
    pub fn parse(value: &Value) -> Option<Self> {
        match value {
            Value::Number(number) if number.is_i64() || number.is_u64() => {
                Some(Self::Number(number.clone()))
            }
            Value::String(value) => Some(Self::String(value.clone())),
            _ => None,
        }
    }

    /// Return the exact JSON value represented by this id.
    pub fn to_value(&self) -> Value {
        match self {
            Self::Number(number) => Value::Number(number.clone()),
            Self::String(value) => Value::String(value.clone()),
        }
    }

    /// Whether this id is legal in MCP `2026-07-28`.
    pub fn is_2026_07_28_legal(&self) -> bool {
        match self {
            Self::String(_) => true,
            Self::Number(number) => number.is_i64() || number.is_u64(),
        }
    }
}

/// One parsed JSON-RPC request or notification.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// Request id, or `None` for a notification.
    pub id: Option<RequestId>,
    /// Method name.
    pub method: String,
    /// Params value; missing params remain `null` for method-specific validation.
    pub params: Value,
}

/// Result of parsing one line from MCP stdio.
#[derive(Debug, PartialEq)]
pub enum ParsedLine {
    /// A valid JSON-RPC request envelope.
    Request(Request),
    /// A JSON-RPC error response that should be written immediately.
    Error(Value),
    /// Whitespace-only input.
    Empty,
}

/// Parse one line-delimited JSON-RPC envelope.
pub fn parse_line(line: &str) -> ParsedLine {
    if line.trim().is_empty() {
        return ParsedLine::Empty;
    }
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            return ParsedLine::Error(error_response(
                None,
                PARSE_ERROR,
                format!("Parse error: {error}"),
                None,
            ));
        }
    };
    parse_value(value)
}

/// Parse an already-decoded JSON value as one JSON-RPC request.
pub fn parse_value(value: Value) -> ParsedLine {
    let Some(object) = value.as_object() else {
        return ParsedLine::Error(error_response(
            None,
            INVALID_REQUEST,
            "Invalid Request",
            None,
        ));
    };
    let candidate_id = object.get("id").and_then(RequestId::parse);
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return ParsedLine::Error(error_response(
            candidate_id.as_ref(),
            INVALID_REQUEST,
            "Invalid Request: jsonrpc must be 2.0",
            None,
        ));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return ParsedLine::Error(error_response(
            candidate_id.as_ref(),
            INVALID_REQUEST,
            "Invalid Request: method must be a string",
            None,
        ));
    };
    if object.contains_key("id") && candidate_id.is_none() {
        return ParsedLine::Error(error_response(
            None,
            INVALID_REQUEST,
            "Invalid Request: id must be a non-null string or integer",
            None,
        ));
    }
    let params = match object.get("params") {
        None => Value::Null,
        Some(params) if params.is_object() => params.clone(),
        Some(_) => {
            return ParsedLine::Error(error_response(
                candidate_id.as_ref(),
                INVALID_REQUEST,
                "Invalid Request: params must be an object when present",
                None,
            ));
        }
    };
    ParsedLine::Request(Request {
        id: candidate_id,
        method: method.to_owned(),
        params,
    })
}

/// Render a JSON-RPC success response.
pub fn success_response(id: &RequestId, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id.to_value(), "result": result})
}

/// Render a JSON-RPC error response.
pub fn error_response(
    id: Option<&RequestId>,
    code: i64,
    message: impl Into<String>,
    data: Option<Value>,
) -> Value {
    let mut error = Map::new();
    error.insert("code".into(), Value::Number(code.into()));
    error.insert("message".into(), Value::String(message.into()));
    if let Some(data) = data {
        error.insert("data".into(), data);
    }
    json!({
        "jsonrpc": "2.0",
        "id": id.map(RequestId::to_value).unwrap_or(Value::Null),
        "error": error,
    })
}

/// Render a JSON-RPC notification.
pub fn notification(method: &str, params: Option<Value>) -> Value {
    match params {
        Some(params) => json!({"jsonrpc": "2.0", "method": method, "params": params}),
        None => json!({"jsonrpc": "2.0", "method": method}),
    }
}

/// Write one compact JSON value followed by a newline and flush it.
pub async fn write_line<W>(writer: &mut W, value: &Value) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let rendered = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    writer.write_all(&rendered).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request_and_preserves_numeric_id() {
        let ParsedLine::Request(request) =
            parse_line(r#"{"jsonrpc":"2.0","id":7,"method":"ping","params":{}}"#)
        else {
            panic!("request expected");
        };
        assert_eq!(request.id, Some(RequestId::Number(7.into())));
        assert_eq!(request.method, "ping");
    }

    #[test]
    fn malformed_json_returns_parse_error_with_null_id() {
        let ParsedLine::Error(response) = parse_line("{") else {
            panic!("error expected");
        };
        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["error"]["code"], PARSE_ERROR);
    }

    #[test]
    fn arrays_are_rejected_instead_of_treated_as_batches() {
        let ParsedLine::Error(response) = parse_line("[]") else {
            panic!("error expected");
        };
        assert_eq!(response["error"]["code"], INVALID_REQUEST);
    }

    #[test]
    fn only_non_null_string_or_integer_ids_parse() {
        assert!(RequestId::String("x".into()).is_2026_07_28_legal());
        assert!(RequestId::Number(4.into()).is_2026_07_28_legal());
        assert!(RequestId::parse(&Value::Null).is_none());
        assert!(RequestId::parse(&json!(1.5)).is_none());
    }

    #[test]
    fn mcp_rejects_null_and_array_params_but_allows_omission() {
        for line in [
            r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":null}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":[]}"#,
        ] {
            let ParsedLine::Error(response) = parse_line(line) else {
                panic!("invalid params must fail");
            };
            assert_eq!(response["error"]["code"], INVALID_REQUEST);
        }
        let ParsedLine::Request(request) =
            parse_line(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
        else {
            panic!("omitted params remain legal");
        };
        assert!(request.params.is_null());
    }
}
