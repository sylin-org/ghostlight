//! Human connection details authored from observed evidence, separate from caller claims.

use ghostlight_bridge::service::IntakeChannel;
use serde::Serialize;

use crate::provenance::{Attribution, PeerObservation, SignatureStatus};

const CONNECTOR_EXPLANATION: &str = "The observed executable is the directly connected program. For MCP, this is normally Ghostlight's connector. It does not verify the reported application or its instructions.";
const LOCAL_PROGRAM_EXPLANATION: &str = "The observed executable is the directly connected program. It does not verify the reported application or the scripts and instructions it runs.";
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
    /// Observed file name only; never an executable path.
    pub observed_executable: Option<String>,
    /// Why an observation is present or absent.
    pub observation: String,
    /// Whether the executable signature has been checked.
    pub signature: String,
    /// What the observation establishes and what it cannot establish.
    pub explanation: String,
}

/// Author retained evidence details without copying any live application claim.
#[must_use]
pub fn details(attribution: &Attribution) -> ConnectionDetails {
    let observed_executable = attribution.observed_executable().map(str::to_owned);
    let observation = match &attribution.peer {
        PeerObservation::Observed { .. } if observed_executable.is_some() => "Observed",
        PeerObservation::Observed { .. } | PeerObservation::NotRecorded => "Not recorded",
        PeerObservation::UnsupportedPlatform => "Unavailable on this platform",
        PeerObservation::Unavailable => "Could not identify this connection",
    };
    let signature = match attribution.signature {
        SignatureStatus::NotChecked => "Not checked",
    };
    ConnectionDetails {
        connection_id: attribution.connection_id.clone(),
        reported_application: None,
        channel: attribution.channel,
        observed_executable,
        observation: observation.into(),
        signature: signature.into(),
        explanation: if attribution.connection_id.is_none() {
            LEGACY_EXPLANATION
        } else {
            match attribution.channel {
                IntakeChannel::Mcp => CONNECTOR_EXPLANATION,
                IntakeChannel::Cli => LOCAL_PROGRAM_EXPLANATION,
            }
        }
        .into(),
    }
}
