//! Tests of the session listener: the refusal to start anything while the session is ending,
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

/// Take the test lock and make sure the state starts cleared.
fn exclusive() -> MutexGuard<'static, ()> {
    let guard = lock(&EXCLUSIVE);
    ENDING.store(false, Ordering::SeqCst);
    guard
}

/// A listener that reports each event on the returned receiver, with whether the session counted
/// as ending when the callback saw it.
fn recording() -> (SessionListener, Receiver<(SessionEvent, bool)>) {
    let (sender, receiver) = channel();
    let listener = listen(move |event| {
        let _ = sender.send((event, session_ending()));
    })
    .expect("the session listener starts");
    (listener, receiver)
}

/// ADR-0199 Decision 13 and defect aaf1b2dd: nothing starts while the session is ending. A
/// process born after the broadcast refuses too, because the system says the session is shutting
/// down. Once the system has asked whether the session may end, starting the service and starting
/// any program refuse with `START_REFUSED` before they look anything up, whatever the system
/// says; and once the system cancels the end, a start is attempted again.
#[test]
fn nothing_starts_while_the_session_is_ending_and_a_cancel_lets_starts_through() {
    let _exclusive = exclusive();
    assert!(refuse_start_by(|| false).is_ok(), "no signal heard");
    assert_eq!(
        refuse_start_by(|| true)
            .expect_err("a process born after the broadcast starts nothing")
            .to_string(),
        START_REFUSED
    );

    dispatch(SessionEvent::Query);
    assert_eq!(
        crate::spawn::start_service(StartReason::Agent)
            .expect_err("no service starts while the session is ending")
            .to_string(),
        START_REFUSED
    );
    let missing = Path::new("ghostlight-no-such-program");
    let arguments: [&OsStr; 0] = [];
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        crate::os::spawn_detached(missing, &arguments, directory)
            .expect_err("no program starts while the session is ending")
            .to_string(),
        START_REFUSED,
        "the refusal comes before the program is looked up"
    );
    assert!(
        refuse_start_by(|| false).is_err(),
        "a heard signal counts whatever the system says"
    );

    dispatch(SessionEvent::Cancelled);
    assert_ne!(
        crate::os::spawn_detached(missing, &arguments, directory)
            .expect_err("a program that does not exist cannot start")
            .to_string(),
        START_REFUSED,
        "once the system cancels, the start is attempted again"
    );
}

/// Set by [`mark_exit`].
static EXITED: AtomicBool = AtomicBool::new(false);

/// Stands in for the process exit.
fn mark_exit() {
    EXITED.store(true, Ordering::SeqCst);
}

/// ADR-0199 Decisions 13 and 14: only the final signal stops a process, and it arms the fallback
/// exit before the orderly stop, so a stop that blocks can never hold the session open. Here the
/// stop blocks until the fallback fires, which it can only do if the fallback was armed first. A
/// query or a cancel stops nothing.
#[test]
fn the_final_signal_arms_a_fallback_exit_that_fires_while_the_stop_blocks() {
    for event in [SessionEvent::Query, SessionEvent::Cancelled] {
        stop_after(
            event,
            || panic!("the fallback is armed only for the final signal"),
            || panic!("nothing stops before the final signal"),
        );
    }

    EXITED.store(false, Ordering::SeqCst);
    let mut fired_while_the_stop_blocked = false;
    stop_after(
        SessionEvent::Terminating,
        || arm_exit_after(Duration::from_millis(10), mark_exit),
        || {
            let deadline = std::time::Instant::now() + PROMPTLY;
            while !EXITED.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                thread::sleep(Duration::from_millis(1));
            }
            fired_while_the_stop_blocked = EXITED.load(Ordering::SeqCst);
        },
    );
    assert!(
        fired_while_the_stop_blocked,
        "the fallback exit fired while the orderly stop still waited"
    );
}

#[cfg(windows)]
mod windows_source {
    //! The hidden window and the console handlers, driven the way Windows drives them.

    use super::*;
    use crate::os::console::swallows_console_event;
    use crate::os::session::windows::{console_ctrl_handler, Hwnd, Lparam, Lresult, Wparam};

    const WM_QUERYENDSESSION: u32 = 0x0011;
    const WM_ENDSESSION: u32 = 0x0016;
    const CTRL_C_EVENT: u32 = 0;
    const CTRL_BREAK_EVENT: u32 = 1;
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

    /// The audited session window on Windows (ADR-0199 Decision 13): the hidden top-level
    /// window answers `WM_QUERYENDSESSION` with TRUE at once, so Ghostlight never delays the end,
    /// and reports each stage to every listener with the ending state already set when the
    /// callback runs: the query starts it, the end keeps it, and `WM_ENDSESSION` with FALSE, a
    /// cancelled end, lifts it.
    #[test]
    fn the_window_allows_the_end_at_once_and_reports_each_stage_with_the_state_set() {
        let _exclusive = exclusive();
        let (listener, events) = recording();

        assert_eq!(send(&listener, WM_QUERYENDSESSION, 0), 1, "TRUE, at once");
        assert_eq!(
            events.recv_timeout(PROMPTLY),
            Ok((SessionEvent::Query, true))
        );
        assert_eq!(send(&listener, WM_ENDSESSION, 1), 0);
        assert_eq!(
            events.recv_timeout(PROMPTLY),
            Ok((SessionEvent::Terminating, true))
        );
        assert_eq!(send(&listener, WM_ENDSESSION, 0), 0);
        assert_eq!(
            events.recv_timeout(PROMPTLY),
            Ok((SessionEvent::Cancelled, false)),
            "WM_ENDSESSION with FALSE lifts the state"
        );
        assert!(!session_ending());

        drop(listener);
    }

    /// ADR-0180, ADR-0199 Decision 13 and the desktop workbench gates of ACCEPTANCE: no console
    /// handler of Ghostlight keeps the default process exit from running. The edge that waits for
    /// its child swallows only Ctrl-C and Ctrl-Break, never a close, a logoff or a shutdown; and
    /// the session listener's handler answers FALSE to every event, reporting a logoff or a
    /// shutdown as the end of the session and a closed console window as nothing.
    #[test]
    fn no_console_handler_suppresses_the_default_exit() {
        assert!(swallows_console_event(CTRL_C_EVENT));
        assert!(swallows_console_event(CTRL_BREAK_EVENT));
        for event in [
            CTRL_CLOSE_EVENT,
            CTRL_LOGOFF_EVENT,
            CTRL_SHUTDOWN_EVENT,
            3,
            4,
            7,
            u32::MAX,
        ] {
            assert!(!swallows_console_event(event), "event {event}");
        }

        let _exclusive = exclusive();
        let (listener, events) = recording();
        for control in [CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT] {
            // SAFETY: this exercises the registered handler with a documented control code.
            let handled = unsafe { console_ctrl_handler(control) };
            assert_eq!(handled, 0, "FALSE, so the default exit runs");
            assert_eq!(
                events.try_recv(),
                Ok((SessionEvent::Terminating, true)),
                "event {control}"
            );
        }
        // SAFETY: as above.
        let handled = unsafe { console_ctrl_handler(CTRL_CLOSE_EVENT) };
        assert_eq!(handled, 0, "a closed console is not handled either");
        assert!(
            events.try_recv().is_err(),
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
    const ENOENT: i32 = 2;

    extern "C" {
        fn __errno_location() -> *mut i32;
    }

    /// The audited signal handler on Linux (ADR-0199 Decision 13): a signal handler may do only
    /// async-signal-safe work, so on SIGTERM and SIGHUP it sets the ending state itself and
    /// writes one byte to the self-pipe, keeping `errno` as it found it. The listener's thread
    /// then reports the end of the session to every callback, with the state already set.
    #[test]
    fn the_signal_handler_sets_the_state_wakes_the_listener_and_keeps_errno() {
        let _exclusive = exclusive();
        let (listener, events) = recording();

        for number in [SIGTERM, SIGHUP] {
            // SAFETY: errno is this thread's own.
            unsafe { *__errno_location() = ENOENT };
            handle_signal(number);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(ENOENT),
                "signal {number}: errno is kept"
            );
            assert!(
                session_ending(),
                "signal {number}: the handler sets the state itself"
            );
            assert_eq!(
                events.recv_timeout(PROMPTLY),
                Ok((SessionEvent::Terminating, true)),
                "signal {number}"
            );
            dispatch(SessionEvent::Cancelled);
            assert_eq!(
                events.recv_timeout(PROMPTLY),
                Ok((SessionEvent::Cancelled, false)),
                "signal {number}"
            );
        }

        drop(listener);
    }
}
