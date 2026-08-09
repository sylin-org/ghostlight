// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The one Ghostlight model-facing tool surface.
//!
//! This edge owns Ghostlight declarations, external validation and normalization, and result
//! rendering. Capability requirements, routing, scheduling, policy, and browser mechanisms stay
//! in the service.

use ghostlight_transport::bridge::{BridgeError, WorkspaceId};
use ghostlight_transport::operation::{BrowserResult, Operation, OperationKind};
use serde_json::Value;

pub(crate) mod ghostlight;
mod schema;

/// MCP revision whose exact declaration grammar the Ghostlight surface must render.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum McpRevision {
    Mcp2025_11_25,
    Mcp2026_07_28,
}

/// Return the initialization guidance for the one installed surface.
pub(crate) const fn agent_guide() -> &'static str {
    ghostlight::agent_guide()
}

/// Decode one public call into exactly one Ghostlight operation.
pub(crate) fn decode_call(
    revision: McpRevision,
    external_tool: &str,
    arguments: Value,
) -> Result<Operation, String> {
    ghostlight::decode_call(revision, external_tool, arguments).map_err(|error| error.to_string())
}

/// Encode one Ghostlight success without weakening its semantic disposition.
pub(crate) fn encode_result(revision: McpRevision, result: BrowserResult) -> Result<Value, String> {
    ghostlight::encode_result(revision, result).map_err(|error| error.to_string())
}

/// Encode a pre-start tool rejection with its proven no-effect disposition.
pub(crate) fn encode_rejection(
    revision: McpRevision,
    error: &BridgeError,
    expected: Option<OperationKind>,
    workspace: Option<&WorkspaceId>,
) -> Result<Value, String> {
    ghostlight::encode_rejection(revision, error, expected, workspace)
        .map_err(|error| error.to_string())
}
