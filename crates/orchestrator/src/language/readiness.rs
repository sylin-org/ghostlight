//! The one answer to "is Ghostlight working right now", and the words for it.
//!
//! Two surfaces ask this question: the workbench, which shows it as its front door, and `doctor`,
//! which prints it in a terminal. Before this module the window computed the word in JavaScript and
//! `doctor` did not answer the question at all, so there was no shared vocabulary to disagree with.
//!
//! The states are ordered by what a person needs to hear first. Being disconnected outranks being
//! paused, because a paused product that cannot reach a browser has a bigger problem than the
//! pause. Attention comes after a human pause, because a person who paused Ghostlight already knows
//! why it stopped.

use serde::{Deserialize, Serialize};

/// The aggregate answer, in the order it takes precedence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// No browser is connected, so no browser work can happen.
    NotConnected,
    /// A person ended the session. Terminal until a new one starts.
    SessionEnded,
    /// A person paused browser work.
    Paused,
    /// Authority stopped work in a way that needs a person.
    NeedsYou,
    /// Work is running right now.
    Working,
    /// Connected, unpaused, and idle.
    Ready,
}

impl Readiness {
    /// Every state, in precedence order. Exhaustiveness guards iterate this.
    pub const ALL: &'static [Readiness] = &[
        Readiness::NotConnected,
        Readiness::SessionEnded,
        Readiness::Paused,
        Readiness::NeedsYou,
        Readiness::Working,
        Readiness::Ready,
    ];

    /// The word a surface shows. Both the window and `doctor` use exactly this.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Readiness::NotConnected => "Not connected",
            Readiness::SessionEnded => "Session ended",
            Readiness::Paused => "Paused",
            Readiness::NeedsYou => "Needs you",
            Readiness::Working => "Working",
            Readiness::Ready => "Ready",
        }
    }

    /// One sentence explaining the word, and where it is true, what to do about it.
    #[must_use]
    pub const fn detail(self) -> &'static str {
        match self {
            Readiness::NotConnected => {
                "No browser is connected. Open a supported Chromium browser with the Ghostlight extension installed."
            }
            Readiness::SessionEnded => {
                "This session was ended. Start a new session to let agents work again."
            }
            Readiness::Paused => "Browser work is paused. Resume when you are ready.",
            Readiness::NeedsYou => "Ghostlight stopped and is waiting for you to decide what happens next.",
            Readiness::Working => "An agent is working in your browser right now.",
            Readiness::Ready => "Connected and idle. Agents can work when they ask.",
        }
    }

    /// The presentation tone, so the surface styles without classifying.
    #[must_use]
    pub const fn tone(self) -> &'static str {
        match self {
            Readiness::NotConnected => "offline",
            Readiness::SessionEnded | Readiness::Paused => "held",
            Readiness::NeedsYou => "attention",
            Readiness::Working => "working",
            Readiness::Ready => "quiet",
        }
    }

    /// Whether a person can act on this state from the front door.
    #[must_use]
    pub const fn invites_control(self) -> bool {
        !matches!(self, Readiness::NotConnected)
    }
}

/// The content-free facts the answer is derived from.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadinessFacts {
    /// Whether at least one browser adapter is connected.
    pub browser_connected: bool,
    /// Whether a person ended the session.
    pub session_ended: bool,
    /// Whether a person paused browser work.
    pub paused: bool,
    /// Whether authority stopped work pending a person.
    pub needs_attention: bool,
    /// Whether at least one operation is running.
    pub working: bool,
}

/// Resolve the one answer.
#[must_use]
pub const fn resolve(facts: &ReadinessFacts) -> Readiness {
    if !facts.browser_connected {
        return Readiness::NotConnected;
    }
    if facts.session_ended {
        return Readiness::SessionEnded;
    }
    if facts.paused {
        return Readiness::Paused;
    }
    if facts.needs_attention {
        return Readiness::NeedsYou;
    }
    if facts.working {
        return Readiness::Working;
    }
    Readiness::Ready
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected() -> ReadinessFacts {
        ReadinessFacts {
            browser_connected: true,
            ..ReadinessFacts::default()
        }
    }

    #[test]
    fn every_state_has_a_word_a_detail_and_a_tone() {
        for state in Readiness::ALL {
            assert!(!state.word().is_empty(), "{state:?} has no word");
            assert!(!state.detail().is_empty(), "{state:?} has no detail");
            assert!(!state.tone().is_empty(), "{state:?} has no tone");
            // A detail is a sentence, not a repeat of the word.
            assert_ne!(state.detail(), state.word());
        }
    }

    #[test]
    fn disconnection_outranks_every_other_answer() {
        let facts = ReadinessFacts {
            browser_connected: false,
            session_ended: true,
            paused: true,
            needs_attention: true,
            working: true,
        };
        assert_eq!(resolve(&facts), Readiness::NotConnected);
    }

    #[test]
    fn a_human_pause_outranks_an_authority_attention_hold() {
        let facts = ReadinessFacts {
            paused: true,
            needs_attention: true,
            ..connected()
        };
        assert_eq!(resolve(&facts), Readiness::Paused);
    }

    #[test]
    fn an_ended_session_outranks_a_pause() {
        let facts = ReadinessFacts {
            session_ended: true,
            paused: true,
            ..connected()
        };
        assert_eq!(resolve(&facts), Readiness::SessionEnded);
    }

    #[test]
    fn a_connected_idle_stack_is_ready() {
        assert_eq!(resolve(&connected()), Readiness::Ready);
    }

    #[test]
    fn running_work_reads_as_working() {
        let facts = ReadinessFacts {
            working: true,
            ..connected()
        };
        assert_eq!(resolve(&facts), Readiness::Working);
    }

    #[test]
    fn only_a_disconnected_stack_offers_no_control() {
        for state in Readiness::ALL {
            assert_eq!(
                state.invites_control(),
                *state != Readiness::NotConnected,
                "{state:?}"
            );
        }
    }
}
