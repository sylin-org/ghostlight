//! Starting the service: the backoff schedule and the detached `serve` launch.
//!
//! A connector that finds no service runs `bin/ghostlight serve` and does not wait for it: the
//! service publishes its runtime file when it is ready, and the connector keeps trying to
//! connect on the [`Backoff`] schedule. The child is detached so it outlives the connector that
//! started it.
//!
//! [`spawn_serve`] is the one place the service is started; the connectors, the installer and
//! `ghostlight start` all use it. A detached service must not hold its starter's pipes. On
//! Windows the child would inherit them (see `ghostlight_win_peer::clear_standard_handle_inheritance`),
//! so a caller that captures output never sees end of output, and a connector's MCP or
//! native-messaging pipes would stay open as long as the service runs. [`spawn_serve`] clears
//! inheritance on the starter's standard handles first. On Unix the child's descriptors 0-2 are
//! replaced by null and Rust opens every other descriptor close-on-exec, so nothing leaks and
//! nothing needs clearing.

use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::{home, BACKOFF_FIRST, BACKOFF_MAX};

/// The one argument the service executable is started with.
pub const SERVE_ARGUMENT: &str = "serve";

/// Windows process creation flag: the child gets no console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Windows process creation flag: the child does not inherit the parent's console.
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// Windows process creation flag: the child is not part of the parent's Ctrl+C group.
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// The Windows creation flags of the detached `serve` launch.
#[cfg(windows)]
pub const SERVE_CREATION_FLAGS: u32 =
    CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;

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

/// The command that starts the service from `executable`: `serve`, null stdio, detached, with
/// `directory` as its working directory.
///
/// The service outlives the client that started it, and a running process holds its working
/// directory open on Windows. `directory` must therefore be one the service owns, never the
/// connector's own (usually the user's project folder). Building the command does not start
/// anything; [`spawn_serve`] does.
#[must_use]
fn build_command(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .arg(SERVE_ARGUMENT)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut command);
    command
}

/// Start `executable serve` detached, with `directory` as its working directory, and return the
/// child without waiting for it.
///
/// The child holds none of this process's standard handles: on Windows their inheritance is
/// cleared before the child is created. Success means the process was created, not that the
/// service is ready. The service is ready when the runtime file names a listener that accepts an
/// opening.
pub fn spawn_serve(executable: &Path, directory: &Path) -> io::Result<Child> {
    #[cfg(windows)]
    ghostlight_win_peer::clear_standard_handle_inheritance();
    build_command(executable, directory).spawn()
}

/// Start the installed service (`home/bin/ghostlight serve`, working directory `home`) and
/// return without waiting for it.
///
/// Success means the process was created, not that the service is ready. See [`spawn_serve`].
pub fn start_service() -> io::Result<()> {
    let mut child = spawn_serve(&home::service_executable()?, &home::home()?)?;
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

    command.creation_flags(SERVE_CREATION_FLAGS);
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
        let command = build_command(Path::new("ghostlight"), Path::new("home"));
        assert_eq!(command.get_program(), OsStr::new("ghostlight"));
        let arguments: Vec<_> = command.get_args().collect();
        assert_eq!(arguments, [OsStr::new("serve")]);
    }

    #[test]
    fn the_service_starts_in_the_directory_it_is_given() {
        let command = build_command(Path::new("ghostlight"), Path::new("home"));
        assert_eq!(command.get_current_dir(), Some(Path::new("home")));
    }

    #[test]
    #[cfg(windows)]
    fn the_windows_launch_is_windowless_detached_and_in_its_own_group() {
        assert_eq!(
            SERVE_CREATION_FLAGS,
            CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        );
        assert_eq!(CREATE_NO_WINDOW, 0x0800_0000);
        assert_eq!(DETACHED_PROCESS, 0x0000_0008);
        assert_eq!(CREATE_NEW_PROCESS_GROUP, 0x0000_0200);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn the_linux_launch_has_null_stdio_and_its_own_process_group() {
        let command = build_command(Path::new("ghostlight"), Path::new("home"));
        // The pretty Debug form spreads each field over several lines; compare it without
        // whitespace so the assertions do not depend on its indentation.
        let description: String = format!("{command:#?}").split_whitespace().collect();
        assert!(description.contains("stdin:Some(Null"), "{description}");
        assert!(description.contains("stdout:Some(Null"), "{description}");
        assert!(description.contains("stderr:Some(Null"), "{description}");
        assert!(description.contains("pgroup:Some(0"), "{description}");
    }
}
