//! The one answer to "is Ghostlight working right now", and the words for it.
//!
//! Two surfaces ask this question: the workbench, which shows it as its front door, and `doctor`,
//! which prints it in a terminal. Both read this one answer, so they share one vocabulary.
//!
//! The states are ordered by what a person needs to hear first. A person's own pause or ended
//! session comes first: it is what they did, and the control beside the word undoes it. No browser
//! comes next, but only when Ghostlight cannot start one itself; a browser an agent's request
//! starts is nothing to report, so that answer is Ready and says which browser starts. An invalid
//! policy comes after: the control beside the word cannot fix it, and it refuses everything, so it
//! outranks missing history, which refuses only what policy says must be recorded.
//!
//! Pause and Resume are the authority's own state, not something a browser holds, so the front door
//! offers them whenever this authority answers, with or without a connected browser.

use serde::{Deserialize, Serialize};

/// The aggregate answer, in the order it takes precedence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// A person ended the session. Terminal until a new one starts.
    SessionEnded,
    /// A person paused browser work.
    Paused,
    /// No browser is connected and Ghostlight would start none, so no browser work can happen.
    NotConnected,
    /// A configured policy is not valid, so governance refuses every governed request.
    PolicyInvalid,
    /// Policy requires saving history and the destination is currently unavailable.
    HistoryUnavailable,
    /// Work is running right now.
    Working,
    /// Connected, unpaused, and idle.
    Ready,
}

impl Readiness {
    /// Every state, in precedence order. Exhaustiveness guards iterate this.
    pub const ALL: &'static [Readiness] = &[
        Readiness::SessionEnded,
        Readiness::Paused,
        Readiness::NotConnected,
        Readiness::PolicyInvalid,
        Readiness::HistoryUnavailable,
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
            Readiness::PolicyInvalid => "Policy not valid",
            Readiness::HistoryUnavailable => "History unavailable",
            Readiness::Working => "Working",
            Readiness::Ready => "Ready",
        }
    }

    /// One sentence explaining the word, and where it is true, what to do about it.
    ///
    /// A Ready with no browser connected says which browser starts instead; [`explain`] gives the
    /// sentence for a resolved answer.
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
            Readiness::PolicyInvalid => "A configured policy is not valid, so Ghostlight refuses all browser work. Open Policy to see what applies. A policy file still in schema 3 needs ghostlight policy migrate <file>.",
            Readiness::HistoryUnavailable => "Policy requires saved history. Ghostlight checks storage automatically before new browser work can proceed.",
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
            Readiness::PolicyInvalid | Readiness::HistoryUnavailable => "attention",
            Readiness::Working => "working",
            Readiness::Ready => "quiet",
        }
    }

    /// Whether the front door offers Pause, Resume, or Start session in this state.
    ///
    /// Always, because runtime control is held by this authority and applies to the next request
    /// whatever the browser is doing. A surface that cannot reach the authority has no state to ask
    /// about, and disables the control itself.
    #[must_use]
    pub const fn invites_control(self) -> bool {
        true
    }
}

/// The content-free facts the answer is derived from.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadinessFacts<'a> {
    /// Strict audit policy currently prevents browser dispatch.
    pub audit_required_unavailable: bool,
    /// Whether at least one browser adapter is connected.
    pub browser_connected: bool,
    /// While no browser is connected, the name of the browser an agent's request would start, when
    /// Ghostlight would start one.
    pub starts: Option<&'a str>,
    /// Whether a person ended the session.
    pub session_ended: bool,
    /// Whether a person paused browser work.
    pub paused: bool,
    /// Whether configured authority fails closed, so every governed request is refused.
    pub authority_invalid: bool,
    /// Whether at least one operation is running.
    pub working: bool,
}

/// Resolve the one answer.
#[must_use]
pub const fn resolve(facts: &ReadinessFacts<'_>) -> Readiness {
    if facts.session_ended {
        return Readiness::SessionEnded;
    }
    if facts.paused {
        return Readiness::Paused;
    }
    if !facts.browser_connected && facts.starts.is_none() {
        return Readiness::NotConnected;
    }
    if facts.authority_invalid {
        return Readiness::PolicyInvalid;
    }
    if facts.audit_required_unavailable {
        return Readiness::HistoryUnavailable;
    }
    if facts.working {
        return Readiness::Working;
    }
    Readiness::Ready
}

/// The sentence for `state`, the answer `facts` resolved to: its [`Readiness::detail`], except
/// that a Ready with no browser connected names the browser an agent's request starts.
#[must_use]
pub fn explain(state: Readiness, facts: &ReadinessFacts<'_>) -> String {
    match (state, facts.browser_connected, facts.starts) {
        (Readiness::Ready, false, Some(browser)) => starts_when_asked(browser),
        _ => state.detail().to_owned(),
    }
}

/// What a Ready says while no browser is connected and `browser` starts on an agent's request.
#[must_use]
pub fn starts_when_asked(browser: &str) -> String {
    format!("Starts {browser} when an agent asks.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected() -> ReadinessFacts<'static> {
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
    fn a_pause_or_an_ended_session_outranks_having_no_browser() {
        for (facts, expected) in [
            (
                ReadinessFacts {
                    paused: true,
                    ..ReadinessFacts::default()
                },
                Readiness::Paused,
            ),
            (
                ReadinessFacts {
                    session_ended: true,
                    paused: true,
                    ..ReadinessFacts::default()
                },
                Readiness::SessionEnded,
            ),
        ] {
            assert_eq!(resolve(&facts), expected);
        }
    }

    #[test]
    fn no_browser_that_ghostlight_cannot_start_outranks_policy_history_and_work() {
        let facts = ReadinessFacts {
            audit_required_unavailable: true,
            authority_invalid: true,
            working: true,
            ..ReadinessFacts::default()
        };
        assert_eq!(resolve(&facts), Readiness::NotConnected);
        assert_eq!(
            explain(Readiness::NotConnected, &facts),
            Readiness::NotConnected.detail()
        );
    }

    #[test]
    fn a_browser_that_starts_on_request_reads_as_ready_and_names_the_browser() {
        let facts = ReadinessFacts {
            starts: Some("Google Chrome"),
            ..ReadinessFacts::default()
        };
        let state = resolve(&facts);
        assert_eq!(state, Readiness::Ready);
        assert_eq!(state.tone(), "quiet");
        assert_eq!(
            explain(state, &facts),
            "Starts Google Chrome when an agent asks."
        );
        // A connected browser needs no such sentence, whatever would start.
        let connected = ReadinessFacts {
            browser_connected: true,
            ..facts
        };
        assert_eq!(
            explain(resolve(&connected), &connected),
            Readiness::Ready.detail()
        );
        // An invalid policy still outranks a browser that would start.
        let invalid = ReadinessFacts {
            authority_invalid: true,
            ..facts
        };
        assert_eq!(resolve(&invalid), Readiness::PolicyInvalid);
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
    fn an_invalid_policy_is_never_ready_and_points_to_the_policy_page() {
        let facts = ReadinessFacts {
            authority_invalid: true,
            ..connected()
        };
        let state = resolve(&facts);
        assert_eq!(state, Readiness::PolicyInvalid);
        assert_eq!(state.tone(), "attention");
        assert!(state.detail().contains("Open Policy"), "{}", state.detail());
    }

    #[test]
    fn an_invalid_policy_outranks_running_work_and_missing_history() {
        let facts = ReadinessFacts {
            authority_invalid: true,
            audit_required_unavailable: true,
            working: true,
            ..connected()
        };
        assert_eq!(resolve(&facts), Readiness::PolicyInvalid);
    }

    #[test]
    fn human_controls_outrank_an_invalid_policy() {
        for (facts, expected) in [
            (
                ReadinessFacts {
                    session_ended: true,
                    ..connected()
                },
                Readiness::SessionEnded,
            ),
            (
                ReadinessFacts {
                    paused: true,
                    ..connected()
                },
                Readiness::Paused,
            ),
        ] {
            let facts = ReadinessFacts {
                authority_invalid: true,
                ..facts
            };
            assert_eq!(resolve(&facts), expected);
        }
    }

    #[test]
    fn the_reachable_authority_offers_control_even_without_a_browser() {
        assert!(resolve(&ReadinessFacts::default()).invites_control());
        for state in Readiness::ALL {
            assert!(state.invites_control(), "{state:?}");
        }
    }
}
