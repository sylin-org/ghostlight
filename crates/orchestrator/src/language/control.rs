//! The runtime words: the acts a person uses to control agents' browser work (Pause, Resume, End
//! session, and Start session), the word for the state they leave in force, and the confirmation a
//! person reads after using one.
//!
//! The tray, the window and the browser toolbar say these words; the window and the toolbar keep
//! copies (`ui/lib/words.js`, `extension/lib/popup-view.js`) that a test in
//! `language::workbench` pins to these.
//!
//! A confirmation states the runtime state the authority now enforces, and nothing about browsers.
//! The authority checks that state before every browser effect, so it holds whether or not a
//! browser is connected. Publishing it to browser adapters has no receipt, and the relay hands it to
//! every adapter that connects later, so a confirmation neither claims nor denies that a browser
//! received it (ADR-0187 Decision 4, ADR-0196).

use crate::browser::adapter::RuntimeControlState;

/// The act that pauses agents' browser work.
pub const PAUSE: &str = "Pause agents";

/// The act that lets paused browser work continue.
pub const RESUME: &str = "Resume agents";

/// The act that starts a new session once one ended. A Resume would leave the session ended.
pub const START_SESSION: &str = "Start a new session";

/// The word for the runtime state `state`: what the window's Settings row and search say, and the
/// title the browser toolbar shows while browser work is paused or the session ended.
#[must_use]
pub const fn state_word(state: RuntimeControlState) -> &'static str {
    match state {
        RuntimeControlState::Active => "Running",
        RuntimeControlState::Held => "Paused",
        RuntimeControlState::Ended => "Session ended",
    }
}

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
