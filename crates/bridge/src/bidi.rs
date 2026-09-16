//! W3C WebDriver BiDi protocol messages.
//!
//! This module defines the JSON-RPC structures used for the Ghostlight BiDi translation layer.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A WebDriver BiDi JSON-RPC command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    /// Command identifier.
    pub id: u64,
    /// BiDi method name (e.g., `browsingContext.navigate`).
    pub method: String,
    /// Method parameters.
    pub params: Value,
}

/// A WebDriver BiDi JSON-RPC response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    /// A successful command response.
    Success(SuccessResponse),
    /// An error response.
    Error(ErrorResponse),
    /// An event emitted by the browser.
    Event(Event),
}

/// A successful command response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SuccessResponse {
    /// The ID of the command this responds to.
    pub id: u64,
    /// The result payload.
    pub result: Value,
}

/// An error command response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// The ID of the command that failed, if it could be parsed.
    pub id: Option<u64>,
    /// The error code.
    pub error: String,
    /// The error message.
    pub message: String,
    /// Optional stacktrace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stacktrace: Option<String>,
}

/// An asynchronous event emitted by the browser.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// The event name (e.g., `browsingContext.load`).
    pub method: String,
    /// The event payload.
    pub params: Value,
}
