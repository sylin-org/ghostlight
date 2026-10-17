//! Human connection details authored from retained evidence, separate from caller claims, and
//! the name a person knows a connection's agent by.

use crate::intake::session::IntakeChannel;
use serde::Serialize;

use crate::provenance::Attribution;

const MCP_EXPLANATION: &str =
    "The application name is the MCP client's own claim. Ghostlight does not verify it.";
const CLI_EXPLANATION: &str = "A ghostlight call session is a local program. Ghostlight does not verify the scripts and instructions it runs.";
const LEGACY_EXPLANATION: &str = "Earlier history did not establish the connecting executable.";

/// The name of a command-line session's agent.
pub const COMMAND_LINE_AGENT: &str = "Command line";

/// The name a person knows a connection's agent by: the command line for a `ghostlight call`
/// session, the product name for an agent Ghostlight knows, and otherwise the name the client
/// `reported`, trimmed. Ghostlight never invents one.
#[must_use]
pub fn agent_name(reported: &str, channel: IntakeChannel) -> String {
    match channel {
        IntakeChannel::Cli => COMMAND_LINE_AGENT.to_owned(),
        IntakeChannel::Mcp => crate::install::agent_name(reported)
            .map_or_else(|| reported.trim().to_owned(), str::to_owned),
    }
}

/// Details for local human surfaces only; a live caller claim must never become audit content.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConnectionDetails {
    /// Opaque service connection identity, when retained.
    pub connection_id: Option<String>,
    /// Bounded live claim, absent when details come from durable history.
    pub reported_application: Option<String>,
    /// The name a person knows the connection's agent by ([`agent_name`]), absent when durable
    /// history retained no name for it.
    pub agent: Option<String>,
    /// Declared intake, never an authorization fact.
    pub channel: IntakeChannel,
    /// What the connection establishes and what it cannot establish.
    pub explanation: String,
}

/// Author retained evidence details without copying any live application claim. Only the command
/// line is named from durable evidence alone, because its channel says who it is.
#[must_use]
pub fn details(attribution: &Attribution) -> ConnectionDetails {
    let explanation = if attribution.connection_id.is_none() {
        LEGACY_EXPLANATION
    } else {
        match attribution.channel {
            IntakeChannel::Mcp => MCP_EXPLANATION,
            IntakeChannel::Cli => CLI_EXPLANATION,
        }
    };
    ConnectionDetails {
        connection_id: attribution.connection_id.clone(),
        reported_application: None,
        agent: (attribution.channel == IntakeChannel::Cli).then(|| COMMAND_LINE_AGENT.to_owned()),
        channel: attribution.channel,
        explanation: explanation.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_is_named_as_a_person_knows_it_and_an_unknown_one_as_it_reported() {
        assert_eq!(agent_name("codex-mcp-client", IntakeChannel::Mcp), "Codex");
        assert_eq!(agent_name("Claude-Code", IntakeChannel::Mcp), "Claude Code");
        assert_eq!(
            agent_name("  Some Agent  ", IntakeChannel::Mcp),
            "Some Agent"
        );
        assert_eq!(
            agent_name("ghostlight call", IntakeChannel::Cli),
            "Command line"
        );
        assert_eq!(
            agent_name("codex-mcp-client", IntakeChannel::Cli),
            "Command line"
        );
    }
}
