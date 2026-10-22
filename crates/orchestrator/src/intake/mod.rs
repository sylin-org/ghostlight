//! Intake: how callers' requests enter the service.
//!
//! MCP is served inside the service (ADR-0197 Decision 17). Every peer reaches it through the one
//! relay endpoint: the service's accept loop hands each connection to [`listener`], which judges
//! its opening and writes the accept, refuse and control replies. An edge forwards each client line unchanged over relay
//! protocol 4, and [`mcp::McpSession`] turns those lines into the session vocabulary the service
//! queue consumes, then renders the service's answers back into MCP. The engine owns no
//! transport: whoever holds the stream feeds it lines and writes what it returns. A shell of the
//! same build reaches the workbench through the `desktop` role, whose frames are in [`desktop`].

pub mod desktop;
pub mod listener;
pub mod mcp;
pub mod opening;
pub mod session;

use crate::intake::session::ServerProfile;

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
