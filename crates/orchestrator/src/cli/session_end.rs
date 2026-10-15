//! The session listener of every `ghostlight` process that is not the service.
//!
//! A command, a launch, an install and an entry rule that forwards to another build all run for
//! a short time, and none of them holds anything the session end must wait for. Each leaves at
//! once on the final signal and starts nothing once the session is ending (ADR-0199 Decisions 13
//! and 14). `serve` is the one process that is the service: it registers its own listener, which
//! says goodbye on every stream before it leaves.

use std::ffi::OsString;

use ghostlight_relay::os::session::{self, SessionEvent, SessionListener};

use crate::cli::parse::{self, LaunchMode};

/// Register the leave-at-once listener for the command in `arguments`, unless it is `serve`.
///
/// Returns the listener, which the caller keeps for the life of the process. A process that
/// cannot listen still runs its command: it only loses the early exit.
#[must_use]
pub fn listen_unless_serving(arguments: &[OsString]) -> Option<SessionListener> {
    let serving = matches!(
        parse::launch_mode(arguments.iter().cloned()),
        Ok(LaunchMode::Serve(_))
    );
    if serving {
        return None;
    }
    session::listen(|event| leave_on_final(event, || std::process::exit(0))).ok()
}

/// Run `leave` on the final signal and do nothing for a query or a cancel.
fn leave_on_final(event: SessionEvent, leave: impl FnOnce()) {
    if event == SessionEvent::Terminating {
        leave();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn only_the_final_signal_makes_a_command_leave() {
        let left = Cell::new(0);
        for event in [SessionEvent::Query, SessionEvent::Cancelled] {
            leave_on_final(event, || left.set(left.get() + 1));
        }
        assert_eq!(left.get(), 0);
        leave_on_final(SessionEvent::Terminating, || left.set(left.get() + 1));
        assert_eq!(left.get(), 1);
    }

    #[test]
    fn serve_registers_its_own_listener_and_every_other_command_leaves_at_once() {
        let serve: Vec<OsString> = ["serve", "--reason", "agent"].map(OsString::from).to_vec();
        assert!(listen_unless_serving(&serve).is_none());
        let version: Vec<OsString> = vec![OsString::from("--version")];
        assert!(listen_unless_serving(&version).is_some());
    }
}
