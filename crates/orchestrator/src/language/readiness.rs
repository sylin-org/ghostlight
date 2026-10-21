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

    const MCP_OFF: ChannelsOff<'static> = ChannelsOff {
        mcp: Some(PolicyAuthor::User),
        cli: None,
        organization: None,
    };

    /// Both switches off, by the person's own policy.
    const BOTH_OFF: ChannelsOff<'static> = ChannelsOff {
        cli: Some(PolicyAuthor::User),
        ..MCP_OFF
    };

    /// What a request on no connected browser would start.
    const STARTS: Option<&str> = Some("Google Chrome");

    /// The module order, ACCEPTANCE D18, ADR-0187 Decision 4 (amended 2026-10-18) and ADR-0202
    /// Decision 22a: an ended session, then a pause, then a policy that refuses every agent that
    /// could ask, then no browser Ghostlight could start, then an invalid policy, missing history,
    /// running work, and otherwise Ready.
    #[test]
    fn the_answer_follows_the_documented_precedence() {
        let none = ReadinessFacts::default();
        let cases = [
            ("nothing connected", none, Readiness::NotConnected),
            (
                "a pause without a browser",
                ReadinessFacts {
                    paused: true,
                    ..none
                },
                Readiness::Paused,
            ),
            (
                "an ended session and a pause",
                ReadinessFacts {
                    session_ended: true,
                    paused: true,
                    ..none
                },
                Readiness::SessionEnded,
            ),
            (
                "no browser over policy, history and work",
                ReadinessFacts {
                    authority_invalid: true,
                    audit_required_unavailable: true,
                    working: true,
                    ..none
                },
                Readiness::NotConnected,
            ),
            (
                "a browser that starts on request",
                ReadinessFacts {
                    starts: STARTS,
                    ..none
                },
                Readiness::Ready,
            ),
            (
                "an invalid policy over a browser that would start",
                ReadinessFacts {
                    starts: STARTS,
                    authority_invalid: true,
                    ..none
                },
                Readiness::PolicyInvalid,
            ),
            ("connected and idle", connected(), Readiness::Ready),
            (
                "running work",
                ReadinessFacts {
                    working: true,
                    ..connected()
                },
                Readiness::Working,
            ),
            (
                "missing history over running work",
                ReadinessFacts {
                    audit_required_unavailable: true,
                    working: true,
                    ..connected()
                },
                Readiness::HistoryUnavailable,
            ),
            (
                "an invalid policy",
                ReadinessFacts {
                    authority_invalid: true,
                    ..connected()
                },
                Readiness::PolicyInvalid,
            ),
            (
                "an invalid policy over history and work",
                ReadinessFacts {
                    authority_invalid: true,
                    audit_required_unavailable: true,
                    working: true,
                    ..connected()
                },
                Readiness::PolicyInvalid,
            ),
            (
                "an ended session over an invalid policy",
                ReadinessFacts {
                    authority_invalid: true,
                    session_ended: true,
                    ..connected()
                },
                Readiness::SessionEnded,
            ),
            (
                "a pause over an invalid policy",
                ReadinessFacts {
                    authority_invalid: true,
                    paused: true,
                    ..connected()
                },
                Readiness::Paused,
            ),
            (
                "every connected agent on the channel turned off",
                ReadinessFacts {
                    channels_off: MCP_OFF,
                    agents_on_mcp: true,
                    ..connected()
                },
                Readiness::TurnedOff,
            ),
            (
                "another connected agent can still work",
                ReadinessFacts {
                    channels_off: MCP_OFF,
                    agents_on_mcp: true,
                    agents_on_cli: true,
                    ..connected()
                },
                Readiness::Ready,
            ),
            (
                "work from the channel still on",
                ReadinessFacts {
                    channels_off: MCP_OFF,
                    agents_on_mcp: true,
                    agents_on_cli: true,
                    working: true,
                    ..connected()
                },
                Readiness::Working,
            ),
            (
                "one channel off and no agent connected",
                ReadinessFacts {
                    channels_off: MCP_OFF,
                    ..connected()
                },
                Readiness::Ready,
            ),
            (
                "both channels off",
                ReadinessFacts {
                    channels_off: BOTH_OFF,
                    ..connected()
                },
                Readiness::TurnedOff,
            ),
            (
                "both channels off without a browser",
                ReadinessFacts {
                    channels_off: BOTH_OFF,
                    agents_on_cli: true,
                    ..none
                },
                Readiness::TurnedOff,
            ),
            (
                "a pause over both channels off",
                ReadinessFacts {
                    channels_off: BOTH_OFF,
                    paused: true,
                    ..connected()
                },
                Readiness::Paused,
            ),
        ];
        for (case, facts, expected) in cases {
            assert_eq!(resolve(&facts), expected, "{case}");
        }
    }

    /// ADR-0187 Decision 4 and ADR-0202 Decision 22a (842ef606): a Ready with no browser names the
    /// browser that starts, a Ready with one channel off says which agents can work, Turned off
    /// names whose policy turned which work off, in the held tone, and an invalid policy is said in
    /// the attention tone (ACCEPTANCE D18).
    #[test]
    fn the_sentence_says_what_the_answer_means() {
        const STARTS_CHROME: &str = "Starts Google Chrome when an agent asks.";
        const MCP_OFF_READY: &str = "MCP agents are turned off; command-line agents can work.";
        let starts = ReadinessFacts {
            starts: STARTS,
            ..ReadinessFacts::default()
        };
        let mcp_off = ReadinessFacts {
            channels_off: MCP_OFF,
            agents_on_mcp: true,
            agents_on_cli: true,
            ..connected()
        };
        let organization = ReadinessFacts {
            channels_off: ChannelsOff {
                mcp: Some(PolicyAuthor::Organization),
                organization: Some("Northwind"),
                ..MCP_OFF
            },
            agents_on_mcp: true,
            ..connected()
        };
        let cases = [
            ("a browser starts", starts, STARTS_CHROME.to_owned()),
            (
                "a connected browser names none that starts",
                ReadinessFacts {
                    browser_connected: true,
                    ..starts
                },
                Readiness::Ready.detail().to_owned(),
            ),
            ("one channel off", mcp_off, MCP_OFF_READY.to_owned()),
            (
                "one channel off and no agent connected",
                ReadinessFacts {
                    channels_off: MCP_OFF,
                    ..connected()
                },
                MCP_OFF_READY.to_owned(),
            ),
            (
                "a browser starts and one channel is off",
                ReadinessFacts {
                    browser_connected: false,
                    starts: STARTS,
                    ..mcp_off
                },
                format!("{STARTS_CHROME} {MCP_OFF_READY}"),
            ),
            (
                "the person's policy turned one channel off",
                ReadinessFacts {
                    agents_on_cli: false,
                    ..mcp_off
                },
                MCP_OFF.said().unwrap(),
            ),
            (
                "the organization's policy turned one channel off",
                organization,
                organization.channels_off.said().unwrap(),
            ),
            (
                "both channels off",
                ReadinessFacts {
                    channels_off: BOTH_OFF,
                    ..connected()
                },
                BOTH_OFF.said().unwrap(),
            ),
            (
                "nothing connected",
                ReadinessFacts::default(),
                Readiness::NotConnected.detail().to_owned(),
            ),
        ];
        for (case, facts, expected) in cases {
            assert_eq!(explain(resolve(&facts), &facts), expected, "{case}");
        }
        assert_eq!(Readiness::TurnedOff.tone(), "held");
        assert_eq!(Readiness::PolicyInvalid.tone(), "attention");
    }
}
