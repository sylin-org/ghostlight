//! Human connection details authored from retained evidence, separate from caller claims.

use crate::intake::session::IntakeChannel;
use serde::Serialize;

use crate::provenance::Attribution;

const MCP_EXPLANATION: &str =
    "The application name is the MCP client's own claim. Ghostlight does not verify it.";
const CLI_EXPLANATION: &str = "A ghostlight call session is a local program. Ghostlight does not verify the scripts and instructions it runs.";
const LEGACY_EXPLANATION: &str = "Earlier history did not establish the connecting executable.";

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

/// Author retained evidence details without copying any live application claim.
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
