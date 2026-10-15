//! The end of the session (ADR-0199 Decisions 13 and 14, ADR-0180).
//!
//! Every Ghostlight process answers the operating system's end-of-session query at once, never
//! asks the system to wait, arms a fallback exit first at the final signal and is gone in under
//! two seconds, and starts no process once the session is ending. This module holds those rules
//! as pure logic ([`SessionState`], [`handle`]) plus the one fallback thread
//! ([`arm_fallback_exit`]).
//!
//! The ending state begins at the first signal, the query included, and lifts only if the
//! operating system cancels. Every code path that starts a process (the service, a URL, a
//! folder) asks [`SessionState::may_start_process`] first.
//!
//! Signal sources in this spike: on Windows the event loop maps its destroyed event to
//! [`SessionSignal::Final`], because `tao` answers the query itself with true. On Linux
//! `install_unix_signals` maps SIGTERM, SIGHUP and SIGINT to the final signal. The query-time
//! ending state on Windows arrives with the shared session listener in the relay crate when the
//! shell lane wires the shell; that listener then replaces both sources, and no signal ever gets
//! two handlers. Nothing in this crate blocks or delays the end of a session.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// How long after the final signal the fallback thread ends the process.
pub const FALLBACK_EXIT: Duration = Duration::from_millis(1500);

/// Name of the fallback thread.
const FALLBACK_THREAD_NAME: &str = "ghostlight-desktop-exit";

/// A signal about the end of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSignal {
    /// The system asks whether the session may end (Windows `WM_QUERYENDSESSION`).
    Query,
    /// The system cancelled a pending end of session.
    Cancelled,
    /// The session is ending now (Windows `WM_ENDSESSION`, Linux SIGTERM, SIGHUP or SIGINT).
    Final,
}

/// What the caller does about a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAct {
    /// Record that the session is ending.
    MarkEnding,
    /// Record that the session is no longer ending.
    ClearEnding,
    /// Arm the fallback exit, then exit.
    ArmFallbackThenExit,
    /// Nothing to do.
    Nothing,
}

/// The shared, thread-safe session state.
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    flags: Arc<Flags>,
}

/// The two flags behind [`SessionState`].
#[derive(Debug, Default)]
struct Flags {
    ending: AtomicBool,
    finished: AtomicBool,
}

impl SessionState {
    /// A state for a session that is not ending.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a signal and return what the caller does about it.
    pub fn on(&self, signal: SessionSignal) -> SessionAct {
        match signal {
            SessionSignal::Query => {
                self.flags.ending.store(true, Ordering::SeqCst);
                SessionAct::MarkEnding
            }
            SessionSignal::Cancelled => {
                if self.flags.finished.load(Ordering::SeqCst) {
                    SessionAct::Nothing
                } else {
                    self.flags.ending.store(false, Ordering::SeqCst);
                    SessionAct::ClearEnding
                }
            }
            SessionSignal::Final => {
                self.flags.ending.store(true, Ordering::SeqCst);
                if self.flags.finished.swap(true, Ordering::SeqCst) {
                    SessionAct::Nothing
                } else {
                    SessionAct::ArmFallbackThenExit
                }
            }
        }
    }

    /// Whether the session is ending.
    pub fn is_ending(&self) -> bool {
        self.flags.ending.load(Ordering::SeqCst)
    }

    /// Whether a process may be started: false while the session is ending.
    pub fn may_start_process(&self) -> bool {
        !self.is_ending()
    }
}

/// Apply a signal: on the final one, call `arm` and then `exit`, in that order.
pub fn handle(
    state: &SessionState,
    signal: SessionSignal,
    arm: impl FnOnce(),
    exit: impl FnOnce(),
) {
    if state.on(signal) == SessionAct::ArmFallbackThenExit {
        arm();
        exit();
    }
}

/// Spawn a thread that ends the process after `after`. When the thread cannot be spawned the
/// process ends at once.
pub fn arm_fallback_exit(after: Duration) {
    let spawned = std::thread::Builder::new()
        .name(FALLBACK_THREAD_NAME.to_string())
        .spawn(move || {
            std::thread::sleep(after);
            std::process::exit(0);
        });
    if spawned.is_err() {
        std::process::exit(0);
    }
}

/// Send the final signal to `sink` when the process receives SIGTERM, SIGHUP or SIGINT. Call it
/// on the event-loop thread after the GTK toolkit is initialised.
#[cfg(target_os = "linux")]
pub fn install_unix_signals(sink: impl Fn(SessionSignal) + Clone + 'static) {
    for signal in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT] {
        let sink = sink.clone();
        gtk::glib::source::unix_signal_add_local(signal, move || {
            sink(SessionSignal::Final);
            gtk::glib::ControlFlow::Continue
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn run(state: &SessionState, signal: SessionSignal) -> Vec<&'static str> {
        let order = RefCell::new(Vec::new());
        handle(
            state,
            signal,
            || order.borrow_mut().push("arm"),
            || order.borrow_mut().push("exit"),
        );
        order.into_inner()
    }

    #[test]
    fn a_query_marks_the_session_ending_and_blocks_new_processes() {
        let state = SessionState::new();
        assert!(state.may_start_process());
        assert_eq!(state.on(SessionSignal::Query), SessionAct::MarkEnding);
        assert!(state.is_ending());
        assert!(!state.may_start_process());
    }

    #[test]
    fn a_cancel_lifts_the_ending_state() {
        let state = SessionState::new();
        state.on(SessionSignal::Query);
        assert_eq!(state.on(SessionSignal::Cancelled), SessionAct::ClearEnding);
        assert!(!state.is_ending());
        assert!(state.may_start_process());
    }

    #[test]
    fn the_final_signal_arms_before_it_exits_and_stays_ending() {
        let state = SessionState::new();
        state.on(SessionSignal::Query);
        assert_eq!(run(&state, SessionSignal::Final), vec!["arm", "exit"]);
        assert!(state.is_ending());
        assert!(!state.may_start_process());
    }

    #[test]
    fn a_second_final_signal_does_nothing() {
        let state = SessionState::new();
        run(&state, SessionSignal::Final);
        assert!(run(&state, SessionSignal::Final).is_empty());
    }

    #[test]
    fn a_final_signal_without_a_query_still_arms_then_exits() {
        let state = SessionState::new();
        assert_eq!(run(&state, SessionSignal::Final), vec!["arm", "exit"]);
        assert!(state.is_ending());
    }

    #[test]
    fn a_cancel_after_the_final_signal_does_not_lift_the_ending_state() {
        let state = SessionState::new();
        run(&state, SessionSignal::Final);
        assert_eq!(state.on(SessionSignal::Cancelled), SessionAct::Nothing);
        assert!(state.is_ending());
    }

    #[test]
    fn state_clones_share_one_session() {
        let state = SessionState::new();
        let other = state.clone();
        state.on(SessionSignal::Query);
        assert!(other.is_ending());
    }
}
