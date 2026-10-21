//! Console interrupts for an edge that outlives its child (ADR-0199 Decision 2).
//!
//! On Windows an edge runs the service image as a child on the same console. Ctrl-C and
//! Ctrl-Break reach both processes; the child handles them and exits, and the edge must stay alive
//! long enough to return the child's exit code. [`forward_console_interrupts`] makes the edge
//! swallow exactly those two events. It does not use `SetConsoleCtrlHandler(NULL, TRUE)`, which
//! puts the process in an "ignore Ctrl-C" state that its children inherit. Close, logoff and
//! shutdown are left to the default exit (ADR-0180). Unix edges exec the child and have nothing
//! to forward.

use std::io;

/// `CTRL_C_EVENT`.
#[cfg(windows)]
const CTRL_C_EVENT: u32 = 0;

/// `CTRL_BREAK_EVENT`.
#[cfg(windows)]
const CTRL_BREAK_EVENT: u32 = 1;

/// Whether the edge swallows the console event `event`: Ctrl-C and Ctrl-Break only, never a
/// close, a logoff or a shutdown, whose default exit must run.
#[cfg(windows)]
pub(super) fn swallows_console_event(event: u32) -> bool {
    event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT
}

/// Keep this process alive through Ctrl-C and Ctrl-Break so it can return its child's exit code.
///
/// On Windows this registers, once per process, a console control handler that answers TRUE for
/// those two events and FALSE for every other, so close, logoff and shutdown keep their default
/// exit. The console delivers the same event to the child, which is not told to ignore it.
/// Calling it again does nothing. Elsewhere it does nothing and returns `Ok`.
pub fn forward_console_interrupts() -> io::Result<()> {
    register()
}

#[cfg(windows)]
fn register() -> io::Result<()> {
    use std::sync::Mutex;

    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
    }

    /// The answer that says the event was handled.
    const HANDLED: i32 = 1;
    /// The answer that says the event was not handled, so the next handler or the default runs.
    const NOT_HANDLED: i32 = 0;

    unsafe extern "system" fn handler(ctrl_type: u32) -> i32 {
        if swallows_console_event(ctrl_type) {
            HANDLED
        } else {
            NOT_HANDLED
        }
    }

    static REGISTERED: Mutex<bool> = Mutex::new(false);

    let mut registered = REGISTERED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if *registered {
        return Ok(());
    }
    // SAFETY: `handler` is a valid callback with the signature the system expects, and it lives
    // for the whole process. Adding a handler passes no pointer the call keeps.
    let added = unsafe { SetConsoleCtrlHandler(Some(handler), 1) };
    if added == 0 {
        return Err(io::Error::last_os_error());
    }
    *registered = true;
    Ok(())
}

#[cfg(not(windows))]
fn register() -> io::Result<()> {
    Ok(())
}
