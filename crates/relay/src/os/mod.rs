//! The relay's audited platform module (ADR-0199 Decisions 3, 5 and 10).
//!
//! Seven things the relay does have no safe standard-library form: starting the service
//! with no inherited handles, watching a parent process by identity (Windows), the objects of
//! the relay endpoint, which are Windows named pipes with their token checks and the credentials
//! of a Unix socket's peer, reading the executables a process descends from, hearing that the
//! session is ending (a hidden window and the console handler on Windows, SIGTERM and SIGHUP on
//! Linux), naming the process's AppUserModelID for the desktop shell's window (Windows), and
//! keeping an edge alive through Ctrl-C while its child exits (Windows). All of them call the
//! operating system directly, so this module is the only place in the crate allowed to use
//! `unsafe`. The crate sets `unsafe_code = "deny"` in its manifest, and the expectation below
//! lifts it for this module alone. Every foreign function is declared by hand against system
//! libraries, with a `// SAFETY:` note at each call; no crate is added for it.
//!
//! [`spawn_detached`] creates a process that holds nothing of its starter: no standard handle, no
//! console, no inherited pipe, file or socket. [`ProcessIdentity`] and the functions around it
//! (Windows only) bind a process id to its creation time, so a reused id never matches.
//! [`ancestors`] reads the ids, start times and executable paths of a process's ancestors on both
//! systems.
//! [`PipeStream`], [`PipeListener`], [`connect_pipe`] and [`pipe_pair`] (Windows only) are the
//! pipe behind `crate::endpoint`; [`current_uid`] and [`peer_credentials`] (Linux only) are its
//! user check and the peer's process id. [`session`] is the listener every Ghostlight process registers and the process-wide
//! state that says the session is ending; [`spawn_detached`] refuses while it holds.
//! [`set_app_user_model_id`] names the process's AppUserModelID for the desktop shell's window.
//! [`forward_console_interrupts`] swallows Ctrl-C and Ctrl-Break in an edge that waits for its
//! child.
//!
//! The relay supports Windows and Linux. Each has its own files here.
#![expect(
    unsafe_code,
    reason = "the audited platform calls: process creation without inherited handles, Windows \
              process identity, ancestry and named pipes, Unix peer credentials, the session-end \
              window and signal handlers, the AppUserModelID call, and the console interrupt \
              handler"
)]

mod ancestry;
mod app_id;
mod console;
pub mod session;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod pipe;
#[cfg(windows)]
mod token;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux::process_facts_reader as platform_facts;
#[cfg(windows)]
use windows::process_facts_reader as platform_facts;

pub use ancestry::{ancestors, Ancestor};
#[cfg(target_os = "linux")]
pub use linux::{current_uid, peer_credentials, spawn_detached, PeerCredentials};
#[cfg(windows)]
pub use pipe::{
    connect_pipe, current_user_sid, pipe_pair, PipeConnectError, PipeListener, PipeStream,
};
#[cfg(windows)]
pub use windows::{
    parent_process, process_identity, process_is_alive, spawn_detached, ProcessIdentity,
};

pub use app_id::set_app_user_model_id;
pub use console::{forward_console_interrupts, swallows_console_event};

#[cfg(test)]
mod tests;
