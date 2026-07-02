//! The binary <-> extension wire protocol (reference documentation).
//!
//! Both directions carry UTF-8 JSON, one object per native message (Chrome frames each with a
//! 4-byte little-endian length prefix; see [`super::host`]). The native-host relays these objects
//! verbatim; only the mcp-server (in [`crate::transport::executor`]) constructs and parses them, so
//! they are documented here rather than modeled as types.
//!
//! ## binary -> extension
//! ```json
//! { "id": "<string>", "type": "tool_request", "tool": "<tool name>", "args": { ... } }
//! ```
//!
//! ## extension -> binary
//! ```json
//! { "id": "<string>", "type": "tool_response", "result": { "content": [ ... ] } }
//! { "id": "<string>", "type": "tool_error",    "error":  "<message>", "hop": "<cdp|page>", "detail": "<string>" }
//! ```
//!
//! `result` is an MCP tool result object. Replies without an `id` (events, heartbeats) are ignored
//! by the mcp-server in v1.0; Phase 3 will buffer console/network events pushed this way.
//!
//! `hop` and `detail` on a `tool_error` reply are both optional. `hop` is only ever `"cdp"` or
//! `"page"` -- the extension tags mechanism (which layer threw), never policy; an absent `hop`
//! means the binary attributes the failure to the extension itself (see
//! [`crate::ToolError::from_extension_wire`]). `detail` is debug-log-only material (logged with
//! `tracing::debug!` in [`crate::transport::executor`]) and must never appear in a tool result
//! surfaced to the MCP client.
