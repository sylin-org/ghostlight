// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight transport: the small, stable substrate the three product executables share.
//! Typed edge/service bridge messages, browser framing, dialing, the resilient browser relay,
//! workspace identity, and process-lifecycle primitives live here. The
//! `ghostlight-mcp-connector` and `ghostlight-browser-connector` shores depend on this crate,
//! never `ghostlight-core`.

pub mod antisquat;
pub mod bridge;
pub mod error;
pub mod handshake;
pub mod host;
pub mod instance;
pub mod ipc;
pub mod observability;
pub mod proc;
pub mod supervisor;
#[cfg(unix)]
mod user_session;
pub mod watchdog;
pub mod workspace_id;

pub use error::{Error, Result, ToolError};

/// Initialize operational (debug) logging to stderr (moved from the root crate; same body).
pub fn init_tracing(verbose: bool) {
    use tracing_subscriber::EnvFilter;
    let default = if verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
