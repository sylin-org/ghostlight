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
        RuntimeControlState::Attention => {
            "Waiting for you. Agents' browser actions will be refused until you resume."
        }
        RuntimeControlState::Ended => {
            "Session ended. Agents' browser actions will be refused until you start a new session."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATES: [RuntimeControlState; 4] = [
        RuntimeControlState::Active,
        RuntimeControlState::Held,
        RuntimeControlState::Attention,
        RuntimeControlState::Ended,
    ];

    #[test]
    fn every_state_has_its_own_confirmation() {
        for (index, state) in STATES.iter().enumerate() {
            let sentence = confirmation(*state);
            assert!(!sentence.is_empty(), "{state:?}");
            assert!(sentence.is_ascii(), "{state:?}");
            for other in &STATES[index + 1..] {
                assert_ne!(sentence, confirmation(*other), "{state:?} and {other:?}");
            }
        }
    }

    #[test]
    fn a_confirmation_never_speaks_for_a_browser() {
        for state in STATES {
            let sentence = confirmation(state).to_ascii_lowercase();
            for claim in ["browser will receive", "reconnect", "delivered", "notified"] {
                assert!(!sentence.contains(claim), "{state:?}: {sentence}");
            }
        }
    }

    #[test]
    fn a_hold_says_what_is_refused_and_how_it_ends() {
        for state in [
            RuntimeControlState::Held,
            RuntimeControlState::Attention,
            RuntimeControlState::Ended,
        ] {
            assert!(
                confirmation(state).contains("refused until you"),
                "{state:?}"
            );
        }
        assert!(!confirmation(RuntimeControlState::Active).contains("refused"));
    }
}
