//! Starting the service: the backoff schedule and the detached `serve` launch.
//!
//! A connector that finds no service runs `bin/ghostlight serve` and does not wait for it: the
//! service publishes its runtime file when it is ready, and the connector keeps trying to
//! connect on the [`Backoff`] schedule. The child is detached so it outlives the connector that
//! started it.
//!
//! [`spawn_serve`] is the one place the service is started; the connectors, the installer and
//! `ghostlight start` all use it, through [`os::spawn_detached`](crate::os::spawn_detached). A
//! detached service must hold nothing of its starter: a pipe it inherited would keep a caller
//! that captures output (an installer piped through a filter, a connector's MCP or
//! native-messaging pipes) waiting for as long as the service runs. The child therefore starts
//! with no inherited handle or descriptor at all, and null standard streams.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::{home, os, BACKOFF_FIRST, BACKOFF_MAX};

/// The one argument the service executable is started with.
pub const SERVE_ARGUMENT: &str = "serve";

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

/// Start `executable serve` detached, with `directory` as its working directory, and return the
/// new process id without waiting for it.
///
/// The `directory` must be one the service owns, never the connector's own (usually the user's
/// project folder): the service outlives its starter, and a running process holds its working
/// directory open on Windows. Success means the process was created, not that the service is
/// ready. The service is ready when the runtime file names a listener that accepts an opening.
pub fn spawn_serve(executable: &Path, directory: &Path) -> io::Result<u32> {
    os::spawn_detached(executable, &[OsStr::new(SERVE_ARGUMENT)], directory)
}

/// Start the installed service (`home/bin/ghostlight serve`, working directory `home`) and
/// return without waiting for it.
///
/// Success means the process was created, not that the service is ready. See [`spawn_serve`].
pub fn start_service() -> io::Result<()> {
    spawn_serve(&home::service_executable()?, &home::home()?).map(|_| ())
}

#[cfg(test)]
mod tests;
