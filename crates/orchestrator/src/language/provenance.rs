//! Human connection details authored from retained evidence, separate from caller claims, and
//! the words the window names an agent by when no roster row names it.

use crate::intake::session::IntakeChannel;
use serde::Serialize;

use crate::provenance::Attribution;

const MCP_EXPLANATION: &str =
    "The application name is the MCP client's own claim. Ghostlight does not verify it.";
const CLI_EXPLANATION: &str = "A ghostlight call session is a local program. Ghostlight does not verify the scripts and instructions it runs.";
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
        channel: attribution.channel,
        explanation: explanation.into(),
    }
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
}
