//! The relay's audited platform module (ADR-0199 Decisions 3 and 10).
//!
//! Two things the frozen relay does have no safe standard-library form: starting the service
//! with no inherited handles, and (on Windows) watching a parent process by identity. Both call
//! the operating system directly, so this module is the only place in the crate allowed to use
//! `unsafe`. The crate sets `unsafe_code = "deny"` in its manifest, and the expectation below
//! lifts it for this module alone. Every foreign function is declared by hand against system
//! libraries, with a `// SAFETY:` note at each call; no crate is added for it.
//!
//! [`spawn_detached`] creates a process that holds nothing of its starter: no standard handle, no
//! console, no inherited pipe, file or socket. [`ProcessIdentity`] and the functions around it
//! (Windows only) bind a process id to its creation time, so a reused id never matches.
//!
//! The relay supports Windows and Linux. Each has its own file here.
#![expect(
    unsafe_code,
    reason = "the audited platform calls: process creation without inherited handles and \
              Windows process identity"
)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::spawn_detached;
#[cfg(windows)]
pub use windows::{
    parent_process, process_identity, process_is_alive, spawn_detached, ProcessIdentity,
};

#[cfg(test)]
mod tests;
