//! Starting the service: the backoff schedule and the detached `serve` launch.
//!
//! A connector that finds no service runs `bin/ghostlight serve` and does not wait for it: the
//! service publishes its runtime file when it is ready, and the connector keeps trying to
//! connect on the [`Backoff`] schedule. The child is detached so it outlives the connector that
//! started it.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::{home, BACKOFF_FIRST, BACKOFF_MAX};

/// The one argument the service executable is started with.
pub const SERVE_ARGUMENT: &str = "serve";

/// Windows process creation flag: the child gets no console window.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Windows process creation flag: the child does not inherit the parent's console.
#[cfg(windows)]
pub const DETACHED_PROCESS: u32 = 0x0000_0008;

/// Windows process creation flag: the child is not part of the parent's Ctrl+C group.
#[cfg(windows)]
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

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

/// The command that starts the service from `executable`: `serve`, null stdio, detached.
///
/// Building the command does not start anything; [`start_service`] does.
#[must_use]
pub fn build_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .arg(SERVE_ARGUMENT)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut command);
    command
}

/// Start the installed service (`home/bin/ghostlight serve`) and return without waiting for it.
///
/// Success means the process was created, not that the service is ready. The service is ready
/// when the runtime file names a listener that accepts an opening.
pub fn start_service() -> io::Result<()> {
    let mut child = build_command(&home::service_executable()?).spawn()?;
    // The caller does not wait. A thread collects the exit status so a service that exits early
    // (for example one that lost the lease race) does not linger as a zombie.
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Detach the child from this process's console and process group.
#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

/// Detach the child from this process's process group.
#[cfg(target_os = "linux")]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

/// Other platforms have no detach step.
#[cfg(not(any(windows, target_os = "linux")))]
fn detach(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn the_backoff_starts_at_the_first_wait_and_doubles_to_the_cap() {
        let mut backoff = Backoff::new();
        let waits: Vec<u64> = (0..9)
            .map(|_| u64::try_from(backoff.next_wait().as_millis()).unwrap())
            .collect();
        assert_eq!(
            waits,
            [500, 1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000, 30_000]
        );
    }

    #[test]
    fn a_reset_returns_to_the_first_wait() {
        let mut backoff = Backoff::new();
        for _ in 0..4 {
            backoff.next_wait();
        }
        backoff.reset();
        assert_eq!(backoff.next_wait(), BACKOFF_FIRST);
        assert_eq!(backoff.next_wait(), BACKOFF_FIRST * 2);
    }

    #[test]
    fn the_command_runs_the_executable_with_the_single_serve_argument() {
        let command = build_command(Path::new("ghostlight"));
        assert_eq!(command.get_program(), OsStr::new("ghostlight"));
        let arguments: Vec<_> = command.get_args().collect();
        assert_eq!(arguments, [OsStr::new("serve")]);
    }

    #[test]
    #[cfg(windows)]
    fn the_windows_flags_are_the_documented_values() {
        assert_eq!(CREATE_NO_WINDOW, 0x0800_0000);
        assert_eq!(DETACHED_PROCESS, 0x0000_0008);
        assert_eq!(CREATE_NEW_PROCESS_GROUP, 0x0000_0200);
    }
}
