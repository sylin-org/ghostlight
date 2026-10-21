//! The shell's end of the session (ADR-0199 Decisions 13 and 14, ADR-0180).
//!
//! Every Ghostlight process answers the operating system's end-of-session query at once, never
//! asks the system to wait, arms a fallback exit first at the final signal and is gone in under
//! two seconds, and starts no process once the session is ending. The relay crate holds the one
//! listener that learns the session is ending and the one process-wide state that remembers it
//! (`ghostlight_relay::os::session`). This module is the shell's glue to it: [`listen`] registers
//! the shell with that listener, and [`Session`] is what the rest of the shell asks before it
//! starts anything.
//!
//! The listener calls back on its own thread, inside the window procedure on Windows. The
//! callback here arms the fallback exit and posts one event, and nothing touches the toolkit off
//! its thread. On Linux the listener handles SIGTERM and SIGHUP; SIGINT is not handled.

use std::io;

use ghostlight_relay::os::session::{self, SessionEvent, SessionListener};

/// Whether the shell may start a process now. Every path that starts one (the service, the shell
/// again, a link, a folder) asks first.
pub trait Session {
    /// False while the session is ending.
    fn may_start_process(&self) -> bool;
}

/// The session this process is in, as the relay's listener knows it.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThisSession;

impl Session for ThisSession {
    fn may_start_process(&self) -> bool {
        !session::session_ending()
    }
}

/// Register the shell with the session listener. On the final signal the listener's callback arms
/// the fallback exit first and then calls `on_final`, which must only post an event to the loop.
/// The returned listener must be kept for as long as the shell runs.
pub fn listen(on_final: impl Fn() + Send + Sync + 'static) -> io::Result<SessionListener> {
    session::listen(move |event: SessionEvent| session::stop_on_final(event, &on_final))
}

/// A session a test can end, for code that takes a [`Session`].
#[cfg(test)]
pub(crate) mod fake {
    use std::cell::Cell;

    use super::Session;

    /// A session that is ending when told so.
    #[derive(Debug, Default)]
    pub(crate) struct FakeSession {
        ending: Cell<bool>,
    }

    impl FakeSession {
        /// A session that is not ending.
        pub(crate) fn new() -> Self {
            Self::default()
        }

        /// A session that is ending.
        pub(crate) fn ending() -> Self {
            let session = Self::default();
            session.ending.set(true);
            session
        }
    }

    impl Session for FakeSession {
        fn may_start_process(&self) -> bool {
            !self.ending.get()
        }
    }
}
