//! Human connection details authored from retained evidence, separate from caller claims, the
//! words that set a claim beside the source Ghostlight saw start the connection, and the words the
//! window names an agent by when no roster row names it.

use crate::intake::session::IntakeChannel;
use serde::Serialize;

use crate::agent::Agent;
use crate::provenance::{Attribution, Source};

const MCP_EXPLANATION: &str =
    "The application name is the MCP client's own claim. Ghostlight does not verify it.";
/// What a command-line session is, when nothing says where it was started from.
const CLI_SESSION: &str = "A ghostlight call session is a local program.";
/// What Ghostlight cannot vouch for in any command-line session.
const CLI_SCRIPTS_UNVERIFIED: &str =
    "Ghostlight does not verify the scripts and instructions it runs.";
const LEGACY_EXPLANATION: &str = "Earlier history did not establish the connecting executable.";

/// What a live MCP connection establishes about the name its client `reported`: the name is the
/// agent's own claim, which Ghostlight does not verify. Durable history keeps no name and says
/// the same without one.
#[must_use]
pub fn claimed_name_explanation(reported: &str) -> String {
    format!("\"{reported}\" is what the agent calls itself; Ghostlight does not verify it.")
}

/// The name of a command-line session's agent ([`crate::agent::Agent::command_line`]).
pub const COMMAND_LINE_AGENT: &str = "Command line";

/// The name a client `reported` for itself, trimmed, or none when it reported none: a name that is
/// empty after trimming, and Ghostlight's own label for a client that sent no name
/// ([`crate::intake::mcp::DEFAULT_CLIENT_LABEL`]), name nobody.
#[must_use]
pub fn claimed_name(reported: &str) -> Option<&str> {
    let name = reported.trim();
    (!name.is_empty() && name != crate::intake::mcp::DEFAULT_CLIENT_LABEL).then_some(name)
}

/// The name of an agent nothing names: a client that reported no name, or a record that kept
/// none ([`crate::agent::Agent::unknown`]).
pub const UNKNOWN_AGENT: &str = "Unknown agent";

/// Details for local human surfaces only; a live caller claim must never become audit content.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConnectionDetails {
    /// Opaque service connection identity, when retained.
    pub connection_id: Option<String>,
    /// Bounded live claim, absent when details come from durable history.
    pub reported_application: Option<String>,
    /// Declared intake, never an authorization fact.
    pub channel: IntakeChannel,
    /// What the connection establishes and what it cannot establish.
    pub explanation: String,
}

/// Author retained evidence details without copying any live application claim. Only the command
/// line is named from durable evidence alone, because its channel says who it is. The source
/// Ghostlight saw, when the record kept one, says what started the connection.
#[must_use]
pub fn details(attribution: &Attribution) -> ConnectionDetails {
    let explanation = if attribution.connection_id.is_none() {
        LEGACY_EXPLANATION.to_owned()
    } else {
        match (attribution.channel, attribution.source.as_ref()) {
            (IntakeChannel::Mcp, Some(source)) => started_by(source),
            (IntakeChannel::Mcp, None) => MCP_EXPLANATION.to_owned(),
            (IntakeChannel::Cli, Some(source)) => command_line_from(source),
            (IntakeChannel::Cli, None) => format!("{CLI_SESSION} {CLI_SCRIPTS_UNVERIFIED}"),
        }
    };
    ConnectionDetails {
        connection_id: attribution.connection_id.clone(),
        reported_application: None,
        channel: attribution.channel,
        explanation,
    }
}

/// Author a live connection's details: the name its client `claimed`, if any, set beside the
/// source Ghostlight saw start it.
///
/// For an MCP connection the explanation says the true thing plainly. When the source is the
/// claimed agent's own app, the claim is corroborated and the sentence says where it was seen.
/// When the claimed agent runs inside another agent's app, it says both. When the claim and the
/// source do not line up (a name the catalog does not know, or a source that is no agent's app),
/// it says both without judging either. With no source it keeps the caveat that the name is the
/// client's own claim.
#[must_use]
pub fn live_details(attribution: &Attribution, claimed: Option<&str>) -> ConnectionDetails {
    let mut details = details(attribution);
    let Some(claim) = claimed else {
        return details;
    };
    if details.channel == IntakeChannel::Mcp {
        details.explanation = match attribution.source.as_ref() {
            None => claimed_name_explanation(claim),
            Some(source) => claimed_beside(claim, source),
        };
    }
    details.reported_application = Some(claim.to_owned());
    details
}

/// The words for a client that claimed `claim` and was started by `source`.
fn claimed_beside(claim: &str, source: &Source) -> String {
    let claimed = Agent::of(Some(claim), IntakeChannel::Mcp);
    let known = crate::install::agent_called(&claimed.id).is_some();
    match source.agent.as_deref() {
        Some(agent) if agent == claimed.id => started_by(source),
        Some(_) if known => format!("{}, in {}.", claimed.name, source_name(source)),
        _ => format!(
            "\"{claim}\" is what the agent calls itself. {}",
            started_by(source)
        ),
    }
}

/// "Started by" the app `source` names.
fn started_by(source: &Source) -> String {
    format!("Started by {}.", source_name(source))
}

/// What a command-line session says of the program it was started from.
fn command_line_from(source: &Source) -> String {
    format!(
        "A ghostlight call from {}. {CLI_SCRIPTS_UNVERIFIED}",
        source_name(source)
    )
}

/// The name a person knows `source` by: its agent's product name when an agent's installed app
/// started the connection, else the executable's file name.
fn source_name(source: &Source) -> String {
    source
        .agent
        .as_deref()
        .and_then(crate::install::agent_called)
        .map_or_else(|| source.image.clone(), |agent| agent.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_claim_is_named_in_the_sentence_that_says_it_is_not_verified() {
        assert_eq!(
            claimed_name_explanation("codex-mcp-client"),
            "\"codex-mcp-client\" is what the agent calls itself; Ghostlight does not verify it."
        );
    }

    #[test]
    fn a_client_that_reported_no_name_claims_none() {
        for unnamed in [
            "",
            "   ",
            crate::intake::mcp::DEFAULT_CLIENT_LABEL,
            " MCP client ",
        ] {
            assert_eq!(claimed_name(unnamed), None, "{unnamed:?}");
        }
        assert_eq!(claimed_name("  Some Agent  "), Some("Some Agent"));
    }

    /// Durable evidence of a connection on `channel` that `source` started.
    fn started(channel: IntakeChannel, source: Option<Source>) -> Attribution {
        Attribution {
            source,
            ..Attribution::accepted(channel)
        }
    }

    /// A source an agent's installed app is, or a program that is no agent's app.
    fn source(agent: Option<&str>, image: &str) -> Option<Source> {
        Some(Source {
            agent: agent.map(str::to_owned),
            image: image.into(),
        })
    }

    #[test]
    fn a_claim_its_own_app_started_drops_the_caveat_and_says_where_it_was_seen() {
        let attribution = started(
            IntakeChannel::Mcp,
            source(Some("claude-desktop"), "claude.exe"),
        );
        assert_eq!(
            live_details(&attribution, Some("claude-ai")).explanation,
            "Started by Claude Desktop."
        );
    }

    #[test]
    fn a_claimed_agent_inside_another_agent_s_app_says_both() {
        let attribution = started(
            IntakeChannel::Mcp,
            source(Some("claude-desktop"), "claude.exe"),
        );
        let details = live_details(&attribution, Some("claude-code"));
        assert_eq!(details.explanation, "Claude Code, in Claude Desktop.");
        assert_eq!(details.reported_application.as_deref(), Some("claude-code"));
    }

    #[test]
    fn a_claim_and_a_source_that_do_not_line_up_are_both_said_without_judging_either() {
        let by_program = started(IntakeChannel::Mcp, source(None, "WindowsTerminal.exe"));
        assert_eq!(
            live_details(&by_program, Some("claude-code")).explanation,
            "\"claude-code\" is what the agent calls itself. Started by WindowsTerminal.exe."
        );
        let unknown_name = started(IntakeChannel::Mcp, source(Some("cursor"), "Cursor.exe"));
        assert_eq!(
            live_details(&unknown_name, Some("my agent")).explanation,
            "\"my agent\" is what the agent calls itself. Started by Cursor."
        );
    }

    #[test]
    fn an_unknown_source_keeps_the_caveat() {
        let attribution = started(IntakeChannel::Mcp, None);
        assert_eq!(
            live_details(&attribution, Some("claude-code")).explanation,
            claimed_name_explanation("claude-code")
        );
        assert_eq!(details(&attribution).explanation, MCP_EXPLANATION);
    }

    #[test]
    fn history_and_the_command_line_say_what_started_the_connection() {
        let mcp = started(
            IntakeChannel::Mcp,
            source(Some("claude-desktop"), "claude.exe"),
        );
        let recorded = details(&mcp);
        assert_eq!(recorded.explanation, "Started by Claude Desktop.");
        assert_eq!(
            recorded.reported_application, None,
            "history keeps no claim"
        );
        let cli = started(IntakeChannel::Cli, source(None, "WindowsTerminal.exe"));
        assert_eq!(
            live_details(&cli, Some("ghostlight call")).explanation,
            "A ghostlight call from WindowsTerminal.exe. Ghostlight does not verify the scripts \
             and instructions it runs."
        );
        assert_eq!(
            details(&started(IntakeChannel::Cli, None)).explanation,
            "A ghostlight call session is a local program. Ghostlight does not verify the scripts \
             and instructions it runs."
        );
    }
}
