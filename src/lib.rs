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
pub mod governance;
pub mod hub;
pub mod install;
pub mod origin;
pub mod transport;

pub use ghostlight_transport::error::{Error, Result, ToolError};
pub use ghostlight_transport::init_tracing;
pub use ghostlight_transport::{error, instance, observability, proc};

/// Compatibility facade: transport-owned submodules whose paths external consumers
/// (integration tests, including the sacred `tool_schema_fidelity` guard) import at the
/// crate root. Internal code uses the real `crate::transport::...` paths; these aliases
/// exist only so the move is byte-transparent to callers outside the crate. They are
/// public API, so they raise no unused-import warning.
pub use transport::{mcp, native};
