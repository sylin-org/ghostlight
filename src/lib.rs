//! Browser MCP -- governed browser automation over the user's own authenticated Chromium session.
//!
//! This is the **library crate**; the `browser-mcp` binary (`src/main.rs`) is a thin shell over it.
//!
//! ## Layering (see `docs/research/NORTH-STAR.md`)
//! v1.0 is the **engine only** -- full capability, no governance overlay. Governance (policy,
//! audit, domain enforcement) is a *separable* v1.5 overlay that attaches at the seams in
//! [`dispatch`] without touching tool code. In v1.0 those seams are no-ops (all-open).
//!
//! ## Dual-role binary
//! The same executable runs in two roles depending on how it is launched (see [`native`]):
//! the **mcp-server** role (launched by the MCP client over stdio) and the **native-host** role
//! (launched by Chrome via `connectNative`). They bridge over a named pipe / Unix domain socket.

pub mod browser;
pub mod dispatch;
pub mod error;
pub mod install;
pub mod mcp;
pub mod native;
pub mod origin;
pub mod tools;

pub use error::{Error, Result};

/// Initialize operational (debug) logging to **stderr**.
///
/// This is `tracing`-based debug/operational logging, deliberately distinct from the audit
/// subsystem (a v1.5 governance-overlay concern). stdout is reserved for the MCP JSON-RPC stream.
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
