//! Relay protocol 5: the frozen contracts between the Ghostlight edges, the command line and the
//! service (ADR-0199 Decisions 3 and 5). Revision 5 is revision 4 with the [`wake`] endpoints
//! (ADR-0199 amendment of 2026-10-20) and the `too_large` failure template (ADR-0197 amendment of
//! 2026-10-21): its frames and openings are revision 4's.
//!
//! This crate is frozen. It holds only what a program must share with the service to stay stable
//! across service upgrades: length-prefixed framing, the fixed home and its names, the
//! [`endpoint`] (a named pipe on Windows and a Unix socket on Linux, with the connection type both
//! ends share), the [`protocol`] vocabulary that runs over it, and the mechanics every edge
//! repeats: starting the service, watching the parent, being woken by a ready service, writing
//! diagnostics and deciding where an edge's invocation goes ([`route`]). It authors no
//! words and makes no product decisions. The `os` module is the audited platform layer: the only
//! `unsafe` in the crate, it starts the service with no inherited handles, identifies a process on
//! Windows, and holds the pipe and peer-user calls the endpoint needs.
//!
//! A change under a published relay revision needs a revision bump (`RELAY` and the crate
//! version); `tests/guards/connectors.test.mjs` checks the source against the last release tag
//! at that revision. Fields are never removed or repurposed, readers ignore unknown fields, and
//! new behavior gets new names.

use std::time::Duration;

pub mod endpoint;
pub mod frame;
pub mod home;
pub mod log;
pub mod os;
pub mod private;
pub mod protocol;
pub mod route;
pub mod spawn;
pub mod wake;
pub mod watchdog;

/// The AppUserModelID Windows groups Ghostlight under: the shell sets it on its window and the
/// installer writes it into the Start-menu shortcut, and Windows shows an unpackaged program's
/// notification only when the two match (ADR-0199 Decision 7). Shortcuts persist it.
pub const APP_USER_MODEL_ID: &str = "Sylin.Ghostlight";

/// The application id on Linux: the Wayland app id and the X11 class the shell sets, and the file
/// name of the Applications entry the installer writes, so the shell groups under that entry.
pub const APPLICATION_ID: &str = "org.sylin.ghostlight";

/// The relay revision spoken by this crate. Carried in every opening.
pub const RELAY: u16 = 5;

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

/// Time a peer has to send its opening frame before the service drops the connection, and time a
/// client has to complete the opening exchange.
pub const OPENING_DEADLINE: Duration = Duration::from_millis(5_000);

// The crate version major is the relay revision, so a published edge names its revision.
// Checked when the crate compiles.
const _: () = assert!(
    decimal(env!("CARGO_PKG_VERSION_MAJOR")) == RELAY,
    "the relay crate's version major is the relay revision"
);

/// The number the decimal digits of `text` spell, for the compile-time check above.
const fn decimal(text: &str) -> u16 {
    let digits = text.as_bytes();
    let mut value = 0_u16;
    let mut index = 0;
    while index < digits.len() {
        assert!(
            digits[index].is_ascii_digit(),
            "a version number is decimal"
        );
        value = value * 10 + (digits[index] - b'0') as u16;
        index += 1;
    }
    value
}

#[cfg(test)]
mod tests;
