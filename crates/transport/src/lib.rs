// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight transport: the small, stable substrate the role executables share (ADR-0046).
//! Wire framing, dialing, the resilient relay, identity, and process-lifecycle primitives.
//! The adapters depend on THIS crate only; a dependency on ghostlight-core here or in an
//! adapter is a design error (it would reintroduce the exe-lock ADR-0046 removes).

pub mod error;
pub mod instance;
pub mod observability;
pub mod proc;
pub mod role;
pub mod watchdog;

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
