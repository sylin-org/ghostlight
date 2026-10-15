//! The session listener every Ghostlight process registers (ADR-0199 Decisions 13 and 14).
//!
//! Ghostlight never blocks or delays the end of a session, and starts nothing once it is ending.
//! This module is the one place that learns the session is ending, on both platforms, and the one
//! place that remembers it.
//!
//! [`listen`] registers a callback for [`SessionEvent`]s. On Windows the events come from a hidden
//! top-level window on its own thread (never a message-only window, which gets no broadcasts) and
//! from `SetConsoleCtrlHandler`; `WM_QUERYENDSESSION` is always answered TRUE at once and the
//! console handler always returns FALSE, so the default exit is never suppressed. On Linux the
//! events come from SIGTERM and SIGHUP: the signal handler writes one byte to a self-pipe and a
//! thread reads it and calls the callbacks, so no callback runs inside a signal handler.
//!
//! One process-wide state says whether the session is ending. It is set at the first signal (the
//! query included) and cleared only when the system cancels the end (`WM_ENDSESSION` with FALSE).
//! [`session_ending`] reads it, and starting a program refuses while it holds
//! ([`refuse_start`]). A process that registers a listener ends itself from its callback; the
//! final signal replaces the default exit of the signals it handles.
//!
//! A process that starts after the broadcast never hears it, so on Windows the state also asks
//! the system: a session that is shutting down counts as ending in every process, the late ones
//! included. Linux has no equally cheap question, so there only a process that heard the signal
//! refuses.
//!
//! The final signal is the point where a process arms [`arm_fallback_exit`] first and then stops
//! in an orderly way, so a stop that blocks can never hold the session open.

use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(windows)]
use windows as platform;

/// The longest a process takes to leave after the final signal. The fallback exit fires after
/// this long whatever the orderly stop is doing.
pub const FALLBACK_EXIT: Duration = Duration::from_secs(2);

/// Why a program is not started while the session is ending.
pub const START_REFUSED: &str = "the session is ending, so no program is started";

/// What the operating system says about the end of the session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    /// The system asks whether the session can end (`WM_QUERYENDSESSION`). The answer is always
    /// yes. The session counts as ending from here. Linux never sends it.
    Query,
    /// The system cancelled the end (`WM_ENDSESSION` with FALSE). The session is no longer
    /// ending. Linux never sends it.
    Cancelled,
    /// The session is ending now: `WM_ENDSESSION` with TRUE, a console logoff or shutdown, or
    /// SIGTERM or SIGHUP.
    Terminating,
}

/// A registered callback.
type Callback = Arc<dyn Fn(SessionEvent) + Send + Sync + 'static>;

/// Whether the session is ending.
static ENDING: AtomicBool = AtomicBool::new(false);

/// The identity the next registration takes.
static NEXT_REGISTRATION: AtomicUsize = AtomicUsize::new(1);

/// Every live registration, by identity.
static LISTENERS: Mutex<Vec<(usize, Callback)>> = Mutex::new(Vec::new());

/// Keeps the state of the tests that set the ending state or start programs apart, since the
/// state is process-wide and tests share a process.
#[cfg(test)]
pub(crate) static EXCLUSIVE: Mutex<()> = Mutex::new(());

/// Whether the session is ending: the system asked whether it can end, or told this process it
/// is ending, and has not cancelled; or, on Windows, the system says the session is shutting down
/// even though this process started too late to hear it.
#[must_use]
pub fn session_ending() -> bool {
    ending_by(platform::system_ending)
}

/// [`session_ending`] with the question to the system injected.
fn ending_by(system_says_ending: impl FnOnce() -> bool) -> bool {
    ENDING.load(Ordering::SeqCst) || system_says_ending()
}

/// Refuse while the session is ending. Every start of a program checks this first.
pub(crate) fn refuse_start() -> io::Result<()> {
    refuse_start_by(platform::system_ending)
}

/// [`refuse_start`] with the question to the system injected.
fn refuse_start_by(system_says_ending: impl FnOnce() -> bool) -> io::Result<()> {
    if ending_by(system_says_ending) {
        Err(io::Error::other(START_REFUSED))
    } else {
        Ok(())
    }
}

/// Register `callback` for the events of the session and start listening if this is the first
/// registration of the process.
///
/// The callback runs on the listener's own thread (on Windows inside the window procedure, while
/// the system waits for its answer), so it must return quickly: on the final signal it arms the
/// fallback exit and hands the orderly stop to another thread. Dropping the returned listener
/// removes the registration and, on Windows, closes the window.
pub fn listen(
    callback: impl Fn(SessionEvent) + Send + Sync + 'static,
) -> io::Result<SessionListener> {
    let registration = NEXT_REGISTRATION.fetch_add(1, Ordering::SeqCst);
    lock(&LISTENERS).push((registration, Arc::new(callback)));
    match platform::attach(registration) {
        Ok(attached) => Ok(SessionListener {
            registration,
            _attached: attached,
        }),
        Err(error) => {
            unregister(registration);
            Err(error)
        }
    }
}

/// Run `stop` on the final signal, after arming the fallback exit. Any other event does nothing:
/// a query only starts the ending state, and a cancel lifts it.
pub fn stop_on_final(event: SessionEvent, stop: impl FnOnce()) {
    stop_after(event, arm_fallback_exit, stop);
}

/// [`stop_on_final`] with the arming injected.
fn stop_after(event: SessionEvent, arm: impl FnOnce(), stop: impl FnOnce()) {
    if event == SessionEvent::Terminating {
        arm();
        stop();
    }
}

/// End this process with status 0 after [`FALLBACK_EXIT`], whatever the orderly stop is doing.
pub fn arm_fallback_exit() {
    arm_exit_after(FALLBACK_EXIT, exit_now);
}

/// End this process with status 0.
fn exit_now() {
    std::process::exit(0);
}

/// Call `exit` after `limit` on a thread of its own, or at once when no thread can be made: with
/// no way to promise a bounded wait, leaving now is the safe answer.
fn arm_exit_after(limit: Duration, exit: fn()) {
    let armed = thread::Builder::new()
        .name("ghostlight-session-fallback".into())
        .spawn(move || {
            thread::sleep(limit);
            exit();
        });
    if armed.is_err() {
        exit();
    }
}

/// An active session listener. Dropping it removes its registration.
pub struct SessionListener {
    registration: usize,
    _attached: platform::Attached,
}

impl Drop for SessionListener {
    fn drop(&mut self) {
        unregister(self.registration);
    }
}

/// Take the registration `registration` out of the callbacks.
fn unregister(registration: usize) {
    lock(&LISTENERS).retain(|(id, _)| *id != registration);
}

/// Record that the session is ending, without calling anything. Safe in a signal handler.
fn begin_ending() {
    ENDING.store(true, Ordering::SeqCst);
}

/// Apply `event` to the ending state, then call every registered callback with it.
fn dispatch(event: SessionEvent) {
    match event {
        SessionEvent::Query | SessionEvent::Terminating => begin_ending(),
        SessionEvent::Cancelled => ENDING.store(false, Ordering::SeqCst),
    }
    let callbacks: Vec<Callback> = lock(&LISTENERS)
        .iter()
        .map(|(_, callback)| Arc::clone(callback))
        .collect();
    for callback in callbacks {
        callback(event);
    }
}

/// The guard of `mutex`, whether or not a thread panicked while holding it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
