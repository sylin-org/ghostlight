//! Linux session end: SIGTERM and SIGHUP through a self-pipe.
//!
//! A signal handler may only call async-signal-safe functions, so [`handle_signal`] does the two
//! things that are safe, setting the ending state (an atomic store) and writing one byte to a
//! pipe, and keeps `errno` as it found it. A thread that blocks reading the other end of the
//! pipe calls the registered callbacks. The pipe, the thread and the handlers are installed once
//! per process, on the first registration.

use std::ffi::{c_int, c_void};
use std::io;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;

use super::{begin_ending, dispatch, SessionEvent};

/// Signal: the controlling terminal closed.
const SIGHUP: c_int = 1;

/// Signal: the process is asked to terminate.
const SIGTERM: c_int = 15;

/// The signals that end the session for a process.
const SESSION_SIGNALS: [c_int; 2] = [SIGHUP, SIGTERM];

/// `signal` result: the call failed.
const SIG_ERR: usize = usize::MAX;

/// `pipe2` flag: close the descriptors when the process executes another program.
const O_CLOEXEC: c_int = 0o2_000_000;

/// `fcntl` command: set the file status flags.
const F_SETFL: c_int = 4;

/// File status flag: a write that would block fails instead.
const O_NONBLOCK: c_int = 0o4_000;

/// The write end of the self-pipe, or -1 before it exists.
static WRITE_END: AtomicI32 = AtomicI32::new(-1);

/// Whether the pipe, the reading thread and the handlers are installed.
static INSTALLED: Mutex<bool> = Mutex::new(false);

extern "C" {
    fn signal(number: c_int, handler: usize) -> usize;
    fn pipe2(descriptors: *mut c_int, flags: c_int) -> c_int;
    fn fcntl(descriptor: c_int, command: c_int, ...) -> c_int;
    fn read(descriptor: c_int, buffer: *mut c_void, count: usize) -> isize;
    fn write(descriptor: c_int, buffer: *const c_void, count: usize) -> isize;
    fn __errno_location() -> *mut c_int;
}

/// What a registration keeps on Linux: nothing, since the pipe and the handlers live as long as
/// the process.
pub(super) struct Attached;

/// Install the handlers on the first registration of the process.
pub(super) fn attach(_registration: usize) -> io::Result<Attached> {
    let mut installed = INSTALLED.lock().unwrap_or_else(PoisonError::into_inner);
    if !*installed {
        install()?;
        *installed = true;
    }
    Ok(Attached)
}

/// Create the self-pipe and its reading thread, then take over SIGHUP and SIGTERM.
fn install() -> io::Result<()> {
    let mut descriptors: [c_int; 2] = [-1, -1];
    // SAFETY: descriptors is a writable array of the two integers pipe2 fills.
    if unsafe { pipe2(descriptors.as_mut_ptr(), O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let [reader, writer] = descriptors;
    // SAFETY: F_SETFL takes an integer and the descriptor was just created; a full pipe must
    // never block the signal handler.
    if unsafe { fcntl(writer, F_SETFL, O_NONBLOCK) } != 0 {
        return Err(io::Error::last_os_error());
    }
    thread::Builder::new()
        .name("ghostlight-session-listener".into())
        .spawn(move || deliver(reader))?;
    WRITE_END.store(writer, Ordering::SeqCst);
    for number in SESSION_SIGNALS {
        let handler = handle_signal as extern "C" fn(c_int) as usize;
        // SAFETY: signal takes an integer and the address of an async-signal-safe handler.
        if unsafe { signal(number, handler) } == SIG_ERR {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// The signal handler: note that the session is ending and wake the reading thread.
///
/// Only async-signal-safe work happens here: an atomic store, and one `write` of one byte to a
/// non-blocking pipe, with `errno` restored afterwards.
pub(super) extern "C" fn handle_signal(number: c_int) {
    begin_ending();
    let descriptor = WRITE_END.load(Ordering::SeqCst);
    if descriptor < 0 {
        return;
    }
    let byte = u8::try_from(number).unwrap_or(0);
    // SAFETY: __errno_location returns this thread's errno, and write reads one byte from a
    // valid buffer. A full pipe fails with EAGAIN, which is fine: a byte is already waiting.
    unsafe {
        let saved = *__errno_location();
        write(descriptor, (&raw const byte).cast(), 1);
        *__errno_location() = saved;
    }
}

/// Read bytes from the self-pipe and call the callbacks once for each, until the pipe closes.
fn deliver(reader: c_int) {
    let mut byte = 0u8;
    loop {
        // SAFETY: read fills at most one byte of a valid buffer.
        let count = unsafe { read(reader, (&raw mut byte).cast(), 1) };
        if count == 1 {
            dispatch(SessionEvent::Terminating);
        } else if count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        } else {
            return;
        }
    }
}
