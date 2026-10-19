//! What a person reads about the policy's channel switches (ADR-0202 Decision 22a).
//!
//! A switch turns off work from one channel: MCP agents, or the command line. The governance
//! facade refuses a request on a switched-off channel before any browser work, and every surface a
//! person looks at says so in these words: the band and the tray, the Policy page, the Status
//! rules check, and the history row of a refused call. The model hears its own refusal sentence,
//! authored in `language/outcome.rs`.
//!
//! The facts come from the one published authority the facade enforces per request
//! ([`crate::governance::AuthoritySnapshot::channels_off`]), never from a second reading of the
//! policy files. The channel is what Ghostlight's own program declared when the agent connected,
//! so a switch keeps well-behaved agents in line and is not a barrier against other software
//! running as the same user. These sentences say what the policy turns off and claim no more.

use crate::intake::session::IntakeChannel;
use crate::language::outcome::PolicyAuthor;

/// Which channel switches the policy in force turns off, and whose policy turned each one off.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChannelsOff<'a> {
    /// Whose policy turned off work from MCP agents, when a policy did.
    pub mcp: Option<PolicyAuthor>,
    /// Whose policy turned off work from the command line, when a policy did.
    pub cli: Option<PolicyAuthor>,
    /// The organization's name, when its policy names one, so its refusal is said in its name.
    pub organization: Option<&'a str>,
}

impl ChannelsOff<'_> {
    /// Whose policy turned `channel` off, or `None` while work from it may run.
    #[must_use]
    pub const fn off(&self, channel: IntakeChannel) -> Option<PolicyAuthor> {
        match channel {
            IntakeChannel::Mcp => self.mcp,
            IntakeChannel::Cli => self.cli,
        }
    }

    /// Whether `channel` is turned off.
    #[must_use]
    pub const fn is_off(&self, channel: IntakeChannel) -> bool {
        self.off(channel).is_some()
    }

    /// Whether any switch is off.
    #[must_use]
    pub const fn any(&self) -> bool {
        self.mcp.is_some() || self.cli.is_some()
    }

    /// Whether every channel is turned off, so every request is refused whatever it declares.
    #[must_use]
    pub const fn all(&self) -> bool {
        self.mcp.is_some() && self.cli.is_some()
    }

    /// The one channel still on while the other is off, or `None` when both or neither are off.
    #[must_use]
    pub const fn only_on(&self) -> Option<IntakeChannel> {
        match (self.mcp, self.cli) {
            (Some(_), None) => Some(IntakeChannel::Cli),
            (None, Some(_)) => Some(IntakeChannel::Mcp),
            _ => None,
        }
    }

    /// The sentences that say whose policy turns off which work, one per policy that turned a
    /// switch off: "Your policy turns off work from MCP agents.", or "Northwind's policy turns off
    /// work from MCP agents and the command line.". `None` while every switch is on.
    #[must_use]
    pub fn said(&self) -> Option<String> {
        match (self.mcp, self.cli) {
            (None, None) => None,
            (Some(author), None) => Some(refused(IntakeChannel::Mcp, author, self.organization)),
            (None, Some(author)) => Some(refused(IntakeChannel::Cli, author, self.organization)),
            (Some(mcp), Some(cli)) if mcp == cli => Some(format!(
                "{} turns off work from {} and {}.",
                whose(mcp, self.organization),
                from(IntakeChannel::Mcp),
                from(IntakeChannel::Cli)
            )),
            (Some(mcp), Some(cli)) => Some(format!(
                "{} {}",
                refused(IntakeChannel::Mcp, mcp, self.organization),
                refused(IntakeChannel::Cli, cli, self.organization)
            )),
        }
    }

    /// What the band says while one channel is off and the other can still work: "MCP agents are
    /// turned off; command-line agents can work.". `None` unless exactly one switch is off.
    #[must_use]
    pub const fn ready_line(&self) -> Option<&'static str> {
        match self.only_on() {
            Some(IntakeChannel::Cli) => {
                Some("MCP agents are turned off; command-line agents can work.")
            }
            Some(IntakeChannel::Mcp) => {
                Some("The command line is turned off; MCP agents can work.")
            }
            None => None,
        }
    }

    /// What the Policy page's headline and the Status rules check add while a switch is off:
    /// "MCP agents are turned off. Command-line work may continue.". `None` while every switch is on.
    #[must_use]
    pub const fn policy_line(&self) -> Option<&'static str> {
        match (self.mcp.is_some(), self.cli.is_some()) {
            (false, false) => None,
            (true, false) => Some("MCP agents are turned off. Command-line work may continue."),
            (false, true) => Some("The command line is turned off. MCP agents may continue."),
            (true, true) => {
                Some("MCP agents and the command line are turned off, so no agent may work.")
            }
        }
    }

    /// What one capability card adds while exactly one channel is off: who may still use it.
    #[must_use]
    pub const fn capability_line(&self) -> Option<&'static str> {
        match self.only_on() {
            Some(IntakeChannel::Cli) => {
                Some("Only command-line work may use it: MCP agents are turned off.")
            }
            Some(IntakeChannel::Mcp) => {
                Some("Only MCP agents may use it: the command line is turned off.")
            }
            None => None,
        }
    }
}

/// The person's sentence for a request refused because `author`'s policy turned `channel` off:
/// "Your policy turns off work from MCP agents.", or "Northwind's policy turns off work from the
/// command line." when the organization's policy names it `organization`.
#[must_use]
pub fn refused(channel: IntakeChannel, author: PolicyAuthor, organization: Option<&str>) -> String {
    format!(
        "{} turns off work from {}.",
        whose(author, organization),
        from(channel)
    )
}

/// Whose policy, as the start of a sentence.
fn whose(author: PolicyAuthor, organization: Option<&str>) -> String {
    match (author, organization) {
        (PolicyAuthor::User, _) => "Your policy".to_owned(),
        (PolicyAuthor::Organization, Some(name)) => format!("{name}'s policy"),
        (PolicyAuthor::Organization, None) => "Your organization's policy".to_owned(),
    }
}

/// The work a channel carries, as the end of "turns off work from".
const fn from(channel: IntakeChannel) -> &'static str {
    match channel {
        IntakeChannel::Mcp => "MCP agents",
        IntakeChannel::Cli => "the command line",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MCP_OFF: ChannelsOff<'static> = ChannelsOff {
        mcp: Some(PolicyAuthor::User),
        cli: None,
        organization: None,
    };

    #[test]
    fn a_refusal_names_whose_policy_turned_which_work_off() {
        assert_eq!(
            refused(IntakeChannel::Mcp, PolicyAuthor::User, Some("Northwind")),
            "Your policy turns off work from MCP agents."
        );
        assert_eq!(
            refused(
                IntakeChannel::Cli,
                PolicyAuthor::Organization,
                Some("Northwind")
            ),
            "Northwind's policy turns off work from the command line."
        );
        assert_eq!(
            refused(IntakeChannel::Mcp, PolicyAuthor::Organization, None),
            "Your organization's policy turns off work from MCP agents."
        );
    }

    #[test]
    fn every_switch_on_says_nothing() {
        let on = ChannelsOff::default();
        assert!(!on.any() && !on.all());
        assert_eq!(on.said(), None);
        assert_eq!(on.ready_line(), None);
        assert_eq!(on.policy_line(), None);
        assert_eq!(on.capability_line(), None);
    }

    #[test]
    fn one_switch_off_says_what_still_works() {
        assert_eq!(MCP_OFF.only_on(), Some(IntakeChannel::Cli));
        assert_eq!(
            MCP_OFF.said().as_deref(),
            Some("Your policy turns off work from MCP agents.")
        );
        assert_eq!(
            MCP_OFF.ready_line(),
            Some("MCP agents are turned off; command-line agents can work.")
        );
        assert_eq!(
            MCP_OFF.policy_line(),
            Some("MCP agents are turned off. Command-line work may continue.")
        );
        let cli_off = ChannelsOff {
            mcp: None,
            cli: Some(PolicyAuthor::Organization),
            organization: Some("Northwind"),
        };
        assert_eq!(cli_off.only_on(), Some(IntakeChannel::Mcp));
        assert_eq!(
            cli_off.said().as_deref(),
            Some("Northwind's policy turns off work from the command line.")
        );
        assert_eq!(
            cli_off.ready_line(),
            Some("The command line is turned off; MCP agents can work.")
        );
    }

    #[test]
    fn both_switches_off_are_said_once_per_policy() {
        let mine = ChannelsOff {
            cli: Some(PolicyAuthor::User),
            ..MCP_OFF
        };
        assert!(mine.all());
        assert_eq!(mine.only_on(), None);
        assert_eq!(mine.ready_line(), None);
        assert_eq!(mine.capability_line(), None);
        assert_eq!(
            mine.said().as_deref(),
            Some("Your policy turns off work from MCP agents and the command line.")
        );
        let split = ChannelsOff {
            mcp: Some(PolicyAuthor::Organization),
            cli: Some(PolicyAuthor::User),
            organization: Some("Northwind"),
        };
        assert_eq!(
            split.said().as_deref(),
            Some(
                "Northwind's policy turns off work from MCP agents. Your policy turns off work from the command line."
            )
        );
    }
}
