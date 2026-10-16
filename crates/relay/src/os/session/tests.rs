//! Tests of the session listener: the ending state, the refusal to start anything while it holds,
//! the fallback exit, and the platform sources (a window and the console handler on Windows, the
//! signal handler's self-pipe on Linux).
//!
//! The ending state is process-wide, so every test that touches it holds [`EXCLUSIVE`] and leaves
//! the state cleared.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::MutexGuard;
use std::thread;
use std::time::Duration;

use super::*;
use crate::protocol::StartReason;

/// How long a test waits for something that should happen at once.
const PROMPTLY: Duration = Duration::from_secs(5);

/// How long a test waits to be sure something does not happen.
const NOT_AT_ALL: Duration = Duration::from_millis(200);

/// Take the test lock and make sure the state starts cleared.
fn exclusive() -> MutexGuard<'static, ()> {
    let guard = lock(&EXCLUSIVE);
    ENDING.store(false, Ordering::SeqCst);
    guard
}

/// A listener whose events arrive on the returned receiver.
fn recording() -> (SessionListener, Receiver<SessionEvent>) {
    let (sender, receiver) = channel();
    let listener = listen(move |event| {
        let _ = sender.send(event);
    })
    .expect("the session listener starts");
    (listener, receiver)
}

#[test]
fn a_query_starts_the_ending_state_and_only_a_cancel_lifts_it() {
    let _exclusive = exclusive();
    let (listener, events) = recording();
    assert!(!session_ending());

    dispatch(SessionEvent::Query);
    assert!(session_ending(), "the query counts as the start of the end");
    dispatch(SessionEvent::Terminating);
    assert!(session_ending());
    dispatch(SessionEvent::Cancelled);
    assert!(
        !session_ending(),
        "a cancel from the system lifts the state"
    );

    let seen: Vec<SessionEvent> = events.try_iter().collect();
    assert_eq!(
        seen,
        [
            SessionEvent::Query,
            SessionEvent::Terminating,
            SessionEvent::Cancelled
        ]
    );
    drop(listener);
}

#[test]
fn the_state_is_set_before_a_callback_sees_the_event() {
    let _exclusive = exclusive();
    let (sender, seen) = channel();
    let listener = listen(move |event| {
        let _ = sender.send((event, session_ending()));
    })
    .expect("the session listener starts");

    dispatch(SessionEvent::Terminating);
    dispatch(SessionEvent::Cancelled);

    let seen: Vec<(SessionEvent, bool)> = seen.try_iter().collect();
    assert_eq!(
        seen,
        [
            (SessionEvent::Terminating, true),
            (SessionEvent::Cancelled, false)
        ]
    );
    drop(listener);
}

#[test]
fn a_dropped_listener_hears_nothing_more() {
    let _exclusive = exclusive();
    let (listener, events) = recording();
    drop(listener);

    dispatch(SessionEvent::Terminating);
    dispatch(SessionEvent::Cancelled);

    assert!(events.recv_timeout(NOT_AT_ALL).is_err());
}

#[test]
fn every_registered_listener_hears_an_event() {
    let _exclusive = exclusive();
    let (first, first_events) = recording();
    let (second, second_events) = recording();

    dispatch(SessionEvent::Terminating);

    assert_eq!(
        first_events.recv_timeout(PROMPTLY),
        Ok(SessionEvent::Terminating)
    );
    assert_eq!(
        second_events.recv_timeout(PROMPTLY),
        Ok(SessionEvent::Terminating)
    );
    dispatch(SessionEvent::Cancelled);
    drop((first, second));
}

#[test]
fn starting_the_service_refuses_while_the_session_is_ending_and_spawns_nothing() {
    let _exclusive = exclusive();
    dispatch(SessionEvent::Query);

    let refused = crate::spawn::start_service(StartReason::Agent)
        .expect_err("no service starts while the session is ending");
    assert_eq!(refused.to_string(), START_REFUSED);

    dispatch(SessionEvent::Cancelled);
}

#[test]
fn a_detached_start_refuses_while_the_session_is_ending_and_not_after_the_cancel() {
    let _exclusive = exclusive();
    let missing = Path::new("ghostlight-no-such-program");
    let arguments: [&OsStr; 0] = [];
    let directory = std::env::temp_dir();

    dispatch(SessionEvent::Terminating);
    let refused = crate::os::spawn_detached(missing, &arguments, &directory)
        .expect_err("a program is not started while the session is ending");
    assert_eq!(
        refused.to_string(),
        START_REFUSED,
        "the refusal comes before the program is looked up"
    );

    dispatch(SessionEvent::Cancelled);
    let missing_now = crate::os::spawn_detached(missing, &arguments, &directory)
        .expect_err("a program that does not exist cannot start");
    assert_ne!(
        missing_now.to_string(),
        START_REFUSED,
        "once the system cancels, the start is attempted again"
    );
}

#[test]
fn a_process_born_after_the_broadcast_still_counts_the_session_as_ending() {
    let _exclusive = exclusive();
    assert!(!session_ending(), "no signal heard");

    assert!(
        ending_by(|| true),
        "the system says the session is shutting down"
    );
    assert!(!ending_by(|| false));

    let refused = refuse_start_by(|| true).expect_err("a late process starts nothing");
    assert_eq!(refused.to_string(), START_REFUSED);
    assert!(refuse_start_by(|| false).is_ok());
}

#[test]
fn a_heard_signal_counts_whatever_the_system_says() {
    let _exclusive = exclusive();
    dispatch(SessionEvent::Query);
    assert!(ending_by(|| false));
    assert!(refuse_start_by(|| false).is_err());
    dispatch(SessionEvent::Cancelled);
    assert!(!ending_by(|| false));
}

#[test]
fn the_final_signal_arms_the_fallback_before_it_stops() {
    let order = std::sync::Mutex::new(Vec::new());
    stop_after(
        SessionEvent::Terminating,
        || order.lock().unwrap().push("fallback"),
        || order.lock().unwrap().push("stop"),
    );
    assert_eq!(*order.lock().unwrap(), ["fallback", "stop"]);
}

#[test]
fn a_query_or_a_cancel_stops_nothing() {
    for event in [SessionEvent::Query, SessionEvent::Cancelled] {
        stop_after(
            event,
            || panic!("the fallback is armed only for the final signal"),
            || panic!("nothing stops before the final signal"),
        );
    }
}

/// Set by [`mark_exit`].
static EXITED: AtomicBool = AtomicBool::new(false);

/// Stands in for the process exit.
fn mark_exit() {
    EXITED.store(true, Ordering::SeqCst);
}

#[test]
fn the_fallback_exit_fires_after_its_limit_and_not_before() {
    EXITED.store(false, Ordering::SeqCst);
    arm_exit_after(Duration::from_millis(300), mark_exit);

    thread::sleep(Duration::from_millis(50));
    assert!(!EXITED.load(Ordering::SeqCst), "not before the limit");
    let deadline = std::time::Instant::now() + PROMPTLY;
    while !EXITED.load(Ordering::SeqCst) {
        assert!(
            std::time::Instant::now() < deadline,
            "the fallback exit never fired"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_fallback_is_under_the_two_seconds_a_session_end_allows() {
    assert_eq!(FALLBACK_EXIT, Duration::from_secs(2));
}

#[cfg(windows)]
mod windows_source {
    //! The hidden window and the console handler, driven the way Windows drives them.

    use super::*;
    use crate::os::session::windows::{console_ctrl_handler, Hwnd, Lparam, Lresult, Wparam};

    const WM_QUERYENDSESSION: u32 = 0x0011;
    const WM_ENDSESSION: u32 = 0x0016;
    const CTRL_CLOSE_EVENT: u32 = 2;
    const CTRL_LOGOFF_EVENT: u32 = 5;
    const CTRL_SHUTDOWN_EVENT: u32 = 6;

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageW(hwnd: Hwnd, msg: u32, w_param: Wparam, l_param: Lparam) -> Lresult;
    }

    /// Send `message` to the listener window and return the window procedure's answer.
    fn send(listener: &SessionListener, message: u32, w_param: Wparam) -> Lresult {
        // SAFETY: the window handle is the live listener window, and the message carries no
        // pointer.
        unsafe { SendMessageW(listener._attached.hwnd, message, w_param, 0) }
    }

    #[test]
    fn the_window_answers_the_query_true_at_once_and_reports_each_stage() {
        let _exclusive = exclusive();
        let (listener, events) = recording();

        assert_eq!(send(&listener, WM_QUERYENDSESSION, 0), 1, "TRUE, at once");
        assert_eq!(events.recv_timeout(PROMPTLY), Ok(SessionEvent::Query));
        assert!(session_ending());

        assert_eq!(send(&listener, WM_ENDSESSION, 1), 0);
        assert_eq!(events.recv_timeout(PROMPTLY), Ok(SessionEvent::Terminating));
        assert!(session_ending());

        assert_eq!(send(&listener, WM_ENDSESSION, 0), 0);
        assert_eq!(events.recv_timeout(PROMPTLY), Ok(SessionEvent::Cancelled));
        assert!(
            !session_ending(),
            "WM_ENDSESSION with FALSE lifts the state"
        );

        drop(listener);
    }

    #[test]
    fn the_console_handler_never_suppresses_the_default_exit() {
        let _exclusive = exclusive();
        let (listener, events) = recording();

        for control in [CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT] {
            // SAFETY: this exercises the registered handler with a documented control code.
            let handled = unsafe { console_ctrl_handler(control) };
            assert_eq!(handled, 0, "FALSE, so the default exit runs");
            assert_eq!(events.recv_timeout(PROMPTLY), Ok(SessionEvent::Terminating));
        }

        // SAFETY: as above.
        let handled = unsafe { console_ctrl_handler(CTRL_CLOSE_EVENT) };
        assert_eq!(handled, 0, "a closed console is not handled either");
        assert!(
            events.recv_timeout(NOT_AT_ALL).is_err(),
            "a closed console window is not the end of the session"
        );

        dispatch(SessionEvent::Cancelled);
        drop(listener);
    }
}

#[cfg(target_os = "linux")]
mod linux_source {
    //! The signal handler and its self-pipe, driven by calling the handler directly.

    use super::*;
    use crate::os::session::linux::handle_signal;

    const SIGHUP: i32 = 1;
    const SIGTERM: i32 = 15;

    extern "C" {
        fn __errno_location() -> *mut i32;
    }

    #[test]
    fn a_signal_reaches_the_callback_through_the_self_pipe() {
        let _exclusive = exclusive();
        let (listener, events) = recording();

        for number in [SIGTERM, SIGHUP] {
            handle_signal(number);
            assert!(session_ending(), "the handler sets the state itself");
            assert_eq!(events.recv_timeout(PROMPTLY), Ok(SessionEvent::Terminating));
            dispatch(SessionEvent::Cancelled);
            assert_eq!(events.recv_timeout(PROMPTLY), Ok(SessionEvent::Cancelled));
        }

        drop(listener);
    }

    #[test]
    fn the_handler_leaves_errno_as_it_found_it() {
        let _exclusive = exclusive();
        let (listener, events) = recording();
        const ENOENT: i32 = 2;
        // SAFETY: errno is this thread's own.
        unsafe { *__errno_location() = ENOENT };

        handle_signal(SIGTERM);

        assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(ENOENT));
        assert_eq!(events.recv_timeout(PROMPTLY), Ok(SessionEvent::Terminating));
        dispatch(SessionEvent::Cancelled);
        drop(listener);
    }
}
