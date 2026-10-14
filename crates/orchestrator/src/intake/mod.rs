//! Intake: how callers' requests enter the service.
//!
//! MCP is served inside the service (ADR-0197 Decision 17). A connector forwards each client line
//! unchanged over relay protocol 3, and [`mcp::McpSession`] turns those lines into the session
//! vocabulary the service queue already consumes, then renders the service's answers back into
//! MCP. The engine owns no transport: whoever holds the stream feeds it lines and writes what it
//! returns.

pub mod mcp;

use ghostlight_bridge::service::ServerProfile;

use crate::language::SERVER_INSTRUCTIONS;

/// Stable product name every intake reports.
pub const SERVER_NAME: &str = "ghostlight";

/// The product metadata every intake hands its clients: name, build version and instructions.
#[must_use]
pub fn server_profile() -> ServerProfile {
    ServerProfile {
        name: SERVER_NAME.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        instructions: SERVER_INSTRUCTIONS.into(),
    }
}
