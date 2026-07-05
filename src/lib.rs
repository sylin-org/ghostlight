// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight -- governed browser automation over the user's own authenticated Chromium session.
//!
//! This is the **library crate**; the `ghostlight` binary (`src/main.rs`) is a thin shell over it.
//!
//! ## Layering (see `docs/research/NORTH-STAR.md`)
//! v1.0 is the **engine only** -- full capability, no governance overlay. Governance (policy,
//! audit, domain enforcement) is a *separable* v1.5 overlay that attaches at the seams in
//! [`governance::dispatch`] without touching tool code. In v1.0 those seams are no-ops (all-open).
//!
//! ## Dual-role binary
//! The same executable runs in two roles depending on how it is launched (see [`native`]):
//! the **mcp-server** role (launched by the MCP client over stdio) and the **native-host** role
//! (launched by Chrome via `connectNative`). They bridge over a named pipe / Unix domain socket.

pub mod browser;
pub mod debug;
pub mod doctor;
pub mod error;
pub mod governance;
pub mod hub;
pub mod install;
pub mod origin;
pub mod proc;
pub mod transport;

pub use error::{Error, Result, ToolError};

/// Compatibility facade: transport-owned submodules whose paths external consumers
/// (integration tests, including the sacred `tool_schema_fidelity` guard) import at the
/// crate root. Internal code uses the real `crate::transport::...` paths; these aliases
/// exist only so the move is byte-transparent to callers outside the crate. They are
/// public API, so they raise no unused-import warning.
pub use transport::{mcp, native};

/// Initialize operational (debug) logging to **stderr**.
///
/// This is `tracing`-based debug/operational logging, deliberately distinct from the audit
/// subsystem (a v1.5 governance-overlay concern). stdout is reserved for the MCP JSON-RPC stream.
///
/// `verbose` (debug mode) lifts the default level to `debug`; an explicit `RUST_LOG` always wins.
pub fn init_tracing(verbose: bool) {
    use tracing_subscriber::EnvFilter;
    let default = if verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
