//! Handing an invocation to a service image (ADR-0199 Decision 2).
//!
//! The edge knows no verb: a carrier copy and an installed edge both run a service image with the
//! arguments they were given, and the image's verb table decides what they mean. On Unix the edge
//! becomes the image with an exec. On Windows there is no exec, so the edge runs the image as a
//! child on the same console and standard streams, swallows Ctrl-C and Ctrl-Break while it waits
//! (the console delivers them to the child too), and returns the child's exit code. Nothing is
//! started once the session is ending.

use std::ffi::OsString;
use std::path::Path;

use anyhow::{bail, Context, Result};
use ghostlight_relay::os::session::{session_ending, START_REFUSED};

/// Run `executable` with `arguments` in place of this process and return its exit code.
///
/// On Unix a successful hand-over never returns. An error means the image could not be started
/// or the session is ending.
pub fn run(executable: &Path, arguments: &[OsString]) -> Result<i32> {
    if session_ending() {
        bail!("{START_REFUSED}");
    }
    run_image(executable, arguments)
        .with_context(|| format!("run the Ghostlight service image {}", executable.display()))
}

#[cfg(unix)]
fn run_image(executable: &Path, arguments: &[OsString]) -> Result<i32> {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let error = Command::new(executable).args(arguments).exec();
    Err(error.into())
}

#[cfg(windows)]
fn run_image(executable: &Path, arguments: &[OsString]) -> Result<i32> {
    use std::process::Command;

    /// The code reported for a child that ended without one.
    const FAILURE: i32 = 1;

    ghostlight_relay::os::forward_console_interrupts()
        .context("keep running through Ctrl-C to return the exit code")?;
    let status = Command::new(executable).args(arguments).status()?;
    Ok(status.code().unwrap_or(FAILURE))
}
