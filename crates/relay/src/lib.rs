//! Relay protocol 3: the frozen contracts between the Ghostlight connectors, the CLI and the
//! service (ADR-0197 Decisions 1 and 3).
//!
//! This crate is frozen. It holds only what a connector must share with the service to stay
//! stable across service upgrades: length-prefixed framing, the fixed home and its file names,
//! the tolerant runtime-file reader, the opening frame and its replies, and the failure-template
//! file, plus the mechanics every connector repeats: starting the service, watching the parent,
//! writing diagnostics, opening a connection, and the service's opening reader and lease. It
//! authors no words and makes no product decisions.
//!
//! A change here needs a relay revision bump (`RELAY` and the crate version) or a deliberate
//! update of the recorded frozen hash. Fields are never removed or repurposed, readers ignore
//! unknown fields, and new behavior gets new names.

use std::time::Duration;

pub mod client;
mod file;
pub mod frame;
pub mod home;
pub mod log;
pub mod opening;
pub mod runtime;
pub mod server;
pub mod spawn;
pub mod templates;
pub mod watchdog;

/// The relay revision spoken by this crate. Carried in the runtime file, the opening and the
/// failure-template file.
pub const RELAY: u16 = 3;

/// Largest frame, in bytes, either side reads or writes: 8 MiB.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Most bytes an MCP connector holds for a service that is away: 4 MiB.
pub const HOLD_LIMIT_BYTES: usize = 4 * 1024 * 1024;

/// Longest an MCP connector holds a line for a service that is away.
pub const HOLD_LIMIT: Duration = Duration::from_secs(30);

/// First wait between attempts to start the service. The wait doubles up to [`BACKOFF_MAX`].
pub const BACKOFF_FIRST: Duration = Duration::from_millis(500);

/// Longest wait between attempts to start the service.
pub const BACKOFF_MAX: Duration = Duration::from_millis(30_000);

/// Time a peer has to send its opening frame before the service drops the connection.
pub const OPENING_DEADLINE: Duration = Duration::from_millis(5_000);
