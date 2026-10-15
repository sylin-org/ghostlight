//! Starting the service: the backoff schedule and the detached `serve` launch.
//!
//! An edge that finds no service runs `bin/ghostlight serve --reason <word>` and does not wait
//! for it. The reason says who is starting the service, so a Quit that should hold is honored
//! (ADR-0199 Decision 10). The service binds its endpoint when it is ready, and the edge keeps
//! trying to connect on the [`Backoff`] schedule. The child is detached so it outlives the edge
//! that started it.
//!
//! [`spawn_serve`] is the one place the service is started; the edges, the installer and the
//! command line all use it, through [`os::spawn_detached`](crate::os::spawn_detached). A
//! detached service must hold nothing of its starter: a pipe it inherited would keep a caller
//! that captures output (an installer piped through a filter, a connector's MCP or
//! native-messaging pipes) waiting for as long as the service runs. The child therefore starts
//! with no inherited handle or descriptor at all, and null standard streams.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::protocol::StartReason;
use crate::{home, os, BACKOFF_FIRST, BACKOFF_MAX};

/// The argument that makes the service executable serve.
pub const SERVE_ARGUMENT: &str = "serve";

/// The option that names why the service is being started.
pub const REASON_OPTION: &str = "--reason";

/// The wait between attempts to start the service.
///
/// The first wait is [`BACKOFF_FIRST`]; each [`Backoff::next_wait`] doubles it up to
/// [`BACKOFF_MAX`]. [`Backoff::reset`] returns to the first wait once a service has answered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Backoff {
    next: Duration,
}

impl Backoff {
    /// A schedule that starts at [`BACKOFF_FIRST`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            next: BACKOFF_FIRST,
        }
    }

    /// The wait to use now. The following call returns twice as much, never more than
    /// [`BACKOFF_MAX`].
    pub fn next_wait(&mut self) -> Duration {
        let wait = self.next;
        self.next = (self.next * 2).min(BACKOFF_MAX);
        wait
    }

    /// Return to the first wait.
    pub fn reset(&mut self) {
        self.next = BACKOFF_FIRST;
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

/// The arguments that start a service for `reason`: `serve --reason <word>`.
#[must_use]
pub fn serve_arguments(reason: StartReason) -> [&'static OsStr; 3] {
    [
        OsStr::new(SERVE_ARGUMENT),
        OsStr::new(REASON_OPTION),
        OsStr::new(reason.as_str()),
    ]
}

/// Start `executable serve --reason <reason>` detached, with `directory` as its working
/// directory, and return the new process id without waiting for it.
///
/// The `directory` must be one the service owns, never the connector's own (usually the user's
/// project folder): the service outlives its starter, and a running process holds its working
/// directory open on Windows. Success means the process was created, not that the service is
/// ready. The service is ready when its endpoint accepts an opening.
pub fn spawn_serve(executable: &Path, directory: &Path, reason: StartReason) -> io::Result<u32> {
    os::spawn_detached(executable, &serve_arguments(reason), directory)
}

/// Start the installed service (`home/bin/ghostlight serve --reason <reason>`, working directory
/// `home`) and return without waiting for it.
///
/// Success means the process was created, not that the service is ready. See [`spawn_serve`].
pub fn start_service(reason: StartReason) -> io::Result<()> {
    spawn_serve(&home::service_executable()?, &home::home()?, reason).map(|_| ())
}

#[cfg(test)]
mod tests;
