//! The words a person reads after using a runtime control: Pause, Resume, End session, and Start
//! session.
//!
//! A confirmation states the runtime state the authority now enforces, and nothing about browsers.
//! The authority checks that state before every browser effect, so it holds whether or not a
//! browser is connected. Publishing it to browser adapters has no receipt, and the relay hands it to
//! every adapter that connects later, so a confirmation neither claims nor denies that a browser
//! received it (ADR-0187 Decision 4, ADR-0196).

use crate::browser::adapter::RuntimeControlState;

/// Confirm the runtime state a person's control left in force.
#[must_use]
pub const fn confirmation(state: RuntimeControlState) -> &'static str {
    match state {
        RuntimeControlState::Active => {
            "Running. Agents can ask for browser work again. Earlier actions are not repeated."
        }
        RuntimeControlState::Held => {
            "Paused. Agents' browser actions will be refused until you resume."
        }
        RuntimeControlState::Ended => {
            "Session ended. Agents' browser actions will be refused until you start a new session."
        }
    }
}
