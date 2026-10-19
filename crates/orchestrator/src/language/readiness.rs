//! The one answer to "is Ghostlight working right now", and the words for it.
//!
//! Two surfaces ask this question: the workbench, which shows it as its front door, and `doctor`,
//! which prints it in a terminal. Both read this one answer, so they share one vocabulary.
//!
//! The states are ordered by what a person needs to hear first. A person's own pause or ended
//! session comes first: it is what they did, and the control beside the word undoes it. A policy
//! that turns off the channel of every connected agent, or every channel, comes next: like a pause
//! it was chosen, so it is held rather than in need of attention, and it refuses those agents
//! whatever the browser or the rest of the policy says, which is the order the executor checks.
//! While another channel can still work the answer stays Ready and says which agents are off.
//! No browser comes next, but only when Ghostlight cannot start one itself; a browser an agent's request
//! starts is nothing to report, so that answer is Ready and says which browser starts. An invalid
//! policy comes after: the control beside the word cannot fix it, and it refuses everything, so it
//! outranks missing history, which refuses only what policy says must be recorded.
//!
//! Pause and Resume are the authority's own state, not something a browser holds, so the front door
//! offers them whenever this authority answers, with or without a connected browser.

use serde::{Deserialize, Serialize};

use super::channel_switch::ChannelsOff;

/// The aggregate answer, in the order it takes precedence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    /// A person ended the session. Terminal until a new one starts.
    SessionEnded,
    /// A person paused browser work.
    Paused,
    /// Policy turns off the channel of every connected agent, or every channel.
    TurnedOff,
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
        Readiness::TurnedOff,
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
            Readiness::TurnedOff => "Turned off",
            Readiness::PolicyInvalid => "Policy not valid",
            Readiness::HistoryUnavailable => "History unavailable",
            Readiness::Working => "Working",
            Readiness::Ready => "Ready",
        }
    }

    /// One sentence explaining the word, and where it is true, what to do about it.
    ///
    /// A Ready with no browser connected says which browser starts instead, Turned off names whose
    /// policy turned off which work, and a Ready with one channel off says which agents can work;
    /// [`explain`] gives the sentence for a resolved answer.
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
            Readiness::TurnedOff => {
                "A policy turns off work from the agents that are connected. Open Policy to see the switches."
            }
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
            Readiness::SessionEnded | Readiness::Paused | Readiness::TurnedOff => "held",
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
    /// The channel switches the policy in force turns off.
    pub channels_off: ChannelsOff<'a>,
    /// Whether a connected agent works over MCP.
    pub agents_on_mcp: bool,
    /// Whether a connected agent works from the command line.
    pub agents_on_cli: bool,
}

/// Whether policy refuses every agent that could ask: every channel is off, or every connected
/// agent's channel is.
const fn turned_off(facts: &ReadinessFacts<'_>) -> bool {
    let off = &facts.channels_off;
    if off.all() {
        return true;
    }
    (facts.agents_on_mcp || facts.agents_on_cli)
        && (!facts.agents_on_mcp || off.mcp.is_some())
        && (!facts.agents_on_cli || off.cli.is_some())
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
    if turned_off(facts) {
        return Readiness::TurnedOff;
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
/// that Turned off names whose policy turned off which work, and a Ready names the browser an
/// agent's request starts while none is connected and which agents can work while one channel is
/// off.
#[must_use]
pub fn explain(state: Readiness, facts: &ReadinessFacts<'_>) -> String {
    match state {
        Readiness::TurnedOff => facts
            .channels_off
            .said()
            .unwrap_or_else(|| state.detail().to_owned()),
        Readiness::Ready => {
            let starts = facts
                .starts
                .filter(|_| !facts.browser_connected)
                .map(starts_when_asked);
            match (starts, facts.channels_off.ready_line()) {
                (Some(starts), Some(line)) => format!("{starts} {line}"),
                (Some(starts), None) => starts,
                (None, Some(line)) => line.to_owned(),
                (None, None) => state.detail().to_owned(),
            }
        }
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
    use crate::language::outcome::PolicyAuthor;

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

    const MCP_OFF: ChannelsOff<'static> = ChannelsOff {
        mcp: Some(PolicyAuthor::User),
        cli: None,
        organization: None,
    };

    #[test]
    fn one_channel_off_holds_when_every_connected_agent_is_on_it() {
        let facts = ReadinessFacts {
            channels_off: MCP_OFF,
            agents_on_mcp: true,
            ..connected()
        };
        let state = resolve(&facts);
        assert_eq!(state, Readiness::TurnedOff);
        // Chosen, like a pause: held, not in need of attention.
        assert_eq!(state.tone(), "held");
        assert_eq!(
            explain(state, &facts),
            "Your policy turns off work from MCP agents."
        );
        let organization = ReadinessFacts {
            channels_off: ChannelsOff {
                mcp: Some(PolicyAuthor::Organization),
                organization: Some("Northwind"),
                ..MCP_OFF
            },
            ..facts
        };
        assert_eq!(
            explain(resolve(&organization), &organization),
            "Northwind's policy turns off work from MCP agents."
        );
    }

    #[test]
    fn one_channel_off_stays_ready_while_another_agent_can_work() {
        let facts = ReadinessFacts {
            channels_off: MCP_OFF,
            agents_on_mcp: true,
            agents_on_cli: true,
            ..connected()
        };
        let state = resolve(&facts);
        assert_eq!(state, Readiness::Ready);
        assert_eq!(
            explain(state, &facts),
            "MCP agents are turned off; command-line agents can work."
        );
        // A browser that starts on request is named before it.
        let starts = ReadinessFacts {
            browser_connected: false,
            starts: Some("Google Chrome"),
            ..facts
        };
        assert_eq!(
            explain(resolve(&starts), &starts),
            "Starts Google Chrome when an agent asks. MCP agents are turned off; command-line agents can work."
        );
        // Work running from the channel still on reads as working.
        let working = ReadinessFacts {
            working: true,
            ..facts
        };
        assert_eq!(resolve(&working), Readiness::Working);
    }

    #[test]
    fn an_off_channel_with_no_connected_agent_stays_ready_and_says_so() {
        let facts = ReadinessFacts {
            channels_off: MCP_OFF,
            ..connected()
        };
        let state = resolve(&facts);
        assert_eq!(state, Readiness::Ready);
        assert_eq!(
            explain(state, &facts),
            "MCP agents are turned off; command-line agents can work."
        );
    }

    #[test]
    fn both_channels_off_hold_with_or_without_a_connected_agent_or_a_browser() {
        let both = ChannelsOff {
            cli: Some(PolicyAuthor::User),
            ..MCP_OFF
        };
        for facts in [
            ReadinessFacts {
                channels_off: both,
                ..connected()
            },
            ReadinessFacts {
                channels_off: both,
                agents_on_cli: true,
                ..ReadinessFacts::default()
            },
        ] {
            let state = resolve(&facts);
            assert_eq!(state, Readiness::TurnedOff);
            assert_eq!(
                explain(state, &facts),
                "Your policy turns off work from MCP agents and the command line."
            );
        }
        // The person's own pause still comes first.
        let paused = ReadinessFacts {
            channels_off: both,
            paused: true,
            ..connected()
        };
        assert_eq!(resolve(&paused), Readiness::Paused);
    }

    #[test]
    fn the_reachable_authority_offers_control_even_without_a_browser() {
        assert!(resolve(&ReadinessFacts::default()).invites_control());
        for state in Readiness::ALL {
            assert!(state.invites_control(), "{state:?}");
        }
    }
}
