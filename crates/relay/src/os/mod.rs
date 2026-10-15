//! The relay's audited platform module (ADR-0199 Decisions 3, 5 and 10).
//!
//! Three things the frozen relay does have no safe standard-library form: starting the service
//! with no inherited handles, watching a parent process by identity (Windows), and the objects of
//! the relay endpoint, which are Windows named pipes with their token checks and the user id of a
//! Unix socket's peer. All of them call the operating system directly, so this module is the only
//! place in the crate allowed to use `unsafe`. The crate sets `unsafe_code = "deny"` in its
//! manifest, and the expectation below lifts it for this module alone. Every foreign function is
//! declared by hand against system libraries, with a `// SAFETY:` note at each call; no crate is
//! added for it.
//!
//! [`spawn_detached`] creates a process that holds nothing of its starter: no standard handle, no
//! console, no inherited pipe, file or socket. [`ProcessIdentity`] and the functions around it
//! (Windows only) bind a process id to its creation time, so a reused id never matches.
//! [`PipeStream`], [`PipeListener`], [`connect_pipe`] and [`pipe_pair`] (Windows only) are the
//! pipe behind `crate::endpoint`; [`current_uid`] and [`peer_uid`] (Linux only) are its user
//! check.
//!
//! The relay supports Windows and Linux. Each has its own files here.
#![expect(
    unsafe_code,
    reason = "the audited platform calls: process creation without inherited handles, Windows \
              process identity and named pipes, and Unix peer credentials"
)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod pipe;
#[cfg(windows)]
mod token;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{current_uid, peer_uid, spawn_detached};
#[cfg(windows)]
pub use pipe::{
    connect_pipe, current_user_sid, pipe_pair, PipeConnectError, PipeListener, PipeStream,
};
#[cfg(windows)]
pub use windows::{
    parent_process, process_identity, process_is_alive, spawn_detached, ProcessIdentity,
};

#[cfg(test)]
mod tests;
