//! Connection evidence: a service-issued identity and channel, with transient claims kept apart
//! from durable attribution.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::intake::session::IntakeChannel;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::language::provenance::ConnectionDetails;

const REPORTED_APPLICATION_MAX_CHARS: usize = 100;
const CONNECTION_ID_PREFIX: &str = "connection_";

/// Evidence captured once for one accepted connection. Caller claims remain memory-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionEvidence {
    reported_application: String,
    attribution: Attribution,
}

/// Bounded connection evidence retained beside an action, never an admission decision.
///
/// Reading ignores fields it does not know.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Attribution {
    /// Service-issued opaque connection identity, absent when the receipt has none.
    #[serde(deserialize_with = "deserialize_connection_id")]
    pub connection_id: Option<String>,
    /// Time the connection was accepted, absent when the receipt has none.
    pub observed_at_ms: Option<u64>,
    /// Declared intake, never authority.
    pub channel: IntakeChannel,
}

impl ConnectionEvidence {
    /// Capture a new connection with a fresh identity and time.
    #[must_use]
    pub fn new(label: &str, channel: IntakeChannel) -> Self {
        Self {
            reported_application: bounded_label(label, REPORTED_APPLICATION_MAX_CHARS),
            attribution: Attribution {
                connection_id: Some(format!("{CONNECTION_ID_PREFIX}{}", Uuid::new_v4().simple())),
                observed_at_ms: Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                        .try_into()
                        .unwrap_or(u64::MAX),
                ),
                channel,
            },
        }
    }

    /// Borrow the durable evidence for this connection; it contains no caller-supplied label.
    #[must_use]
    pub fn attribution(&self) -> &Attribution {
        &self.attribution
    }

    /// Borrow the bounded live application claim for familiar session labels.
    #[must_use]
    pub fn reported_application(&self) -> &str {
        &self.reported_application
    }

    /// Present the live connection with its separately identified, bounded application claim and
    /// the name a person knows its agent by.
    #[must_use]
    pub fn details(&self) -> ConnectionDetails {
        let mut details = self.attribution.details();
        if !self.reported_application.is_empty() {
            details.agent = Some(crate::language::provenance::agent_name(
                &self.reported_application,
                details.channel,
            ));
            if details.channel == IntakeChannel::Mcp {
                details.explanation = crate::language::provenance::claimed_name_explanation(
                    &self.reported_application,
                );
            }
            details.reported_application = Some(self.reported_application.clone());
        }
        details
    }
}

impl Attribution {
    /// Read history written before connection identities existed: only the channel is known.
    #[must_use]
    pub fn legacy(channel: IntakeChannel) -> Self {
        Self {
            connection_id: None,
            observed_at_ms: None,
            channel,
        }
    }

    /// Present retained evidence without reconstructing or persisting an application claim.
    #[must_use]
    pub fn details(&self) -> ConnectionDetails {
        crate::language::provenance::details(self)
    }
}

fn bounded_label(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}'
                        | '\u{2060}'..='\u{206f}' | '\u{feff}'
                )
        })
        .take(limit)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn deserialize_connection_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    match value {
        Some(value)
            if value
                .strip_prefix(CONNECTION_ID_PREFIX)
                .is_some_and(|suffix| {
                    suffix.len() == 32
                        && Uuid::try_parse(suffix).is_ok_and(|id| id.simple().to_string() == suffix)
                }) =>
        {
            Ok(Some(value))
        }
        Some(_) => Err(serde::de::Error::custom("invalid connection identity")),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_claim_and_durable_evidence_are_separate() {
        let evidence = ConnectionEvidence::new("claimed application secret", IntakeChannel::Mcp);
        let encoded = serde_json::to_value(evidence.attribution()).unwrap();
        assert!(!encoded.to_string().contains("claimed application secret"));
        let mut keys: Vec<_> = encoded.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["channel", "connection_id", "observed_at_ms"]);
        assert_eq!(
            evidence.details().reported_application.as_deref(),
            Some("claimed application secret")
        );
        assert_eq!(evidence.attribution().details().reported_application, None);
        let restored: Attribution = serde_json::from_value(encoded).unwrap();
        assert_eq!(&restored, evidence.attribution());
    }

    #[test]
    fn every_connection_gets_fresh_evidence_even_for_the_same_claim() {
        let first = ConnectionEvidence::new("application", IntakeChannel::Mcp);
        let retained = first.attribution().clone();
        let second = ConnectionEvidence::new("application", IntakeChannel::Mcp);
        assert_ne!(retained.connection_id, second.attribution().connection_id);
        assert_eq!(retained, *first.attribution());
        assert!(retained.observed_at_ms.unwrap() > 0);
    }

    #[test]
    fn claims_are_bounded_and_stripped_of_control_text() {
        let evidence = ConnectionEvidence::new(
            &format!("\n\u{202e}{}\0", "x".repeat(150)),
            IntakeChannel::Cli,
        );
        assert_eq!(evidence.reported_application(), "x".repeat(100));
        assert_eq!(
            evidence.details().reported_application,
            Some("x".repeat(100))
        );
    }

    #[test]
    fn historical_evidence_does_not_invent_connection_information() {
        let absent = Attribution::legacy(IntakeChannel::Mcp);
        assert_eq!(absent.connection_id, None);
        assert_eq!(absent.observed_at_ms, None);
        assert_eq!(absent.details().reported_application, None);
        assert_eq!(absent.details().agent, None);
    }

    #[test]
    fn a_live_connection_names_its_agent_and_history_names_only_the_command_line() {
        let codex = ConnectionEvidence::new("codex-mcp-client", IntakeChannel::Mcp);
        assert_eq!(codex.details().agent.as_deref(), Some("Codex"));
        assert_eq!(
            codex.details().reported_application.as_deref(),
            Some("codex-mcp-client")
        );
        assert_eq!(codex.attribution().details().agent, None);
        let unknown = ConnectionEvidence::new(" my agent ", IntakeChannel::Mcp);
        assert_eq!(unknown.details().agent.as_deref(), Some("my agent"));
        let script = ConnectionEvidence::new("ghostlight call", IntakeChannel::Cli);
        assert_eq!(script.details().agent.as_deref(), Some("Command line"));
        assert_eq!(
            script.attribution().details().agent.as_deref(),
            Some("Command line")
        );
        let unnamed = ConnectionEvidence::new("", IntakeChannel::Mcp);
        assert_eq!(unnamed.details().agent, None);
    }

    #[test]
    fn a_line_written_with_peer_fields_still_reads() {
        let line = serde_json::json!({
            "connection_id": format!("{CONNECTION_ID_PREFIX}{}", Uuid::new_v4().simple()),
            "observed_at_ms": 7,
            "channel": "mcp",
            "peer": { "state": "observed", "executable": "client.exe" },
            "signature": "not_checked",
        });
        let restored: Attribution = serde_json::from_value(line).unwrap();
        assert_eq!(restored.observed_at_ms, Some(7));
        assert_eq!(restored.channel, IntakeChannel::Mcp);
        let rewritten = serde_json::to_value(&restored).unwrap();
        assert!(rewritten.get("peer").is_none());
        assert!(rewritten.get("signature").is_none());
    }

    #[test]
    fn restored_evidence_rejects_arbitrary_connection_claims() {
        let evidence = ConnectionEvidence::new("transient claim", IntakeChannel::Mcp);
        let mut invalid = serde_json::to_value(evidence.attribution()).unwrap();
        invalid["connection_id"] = "private supplied text".into();
        assert!(serde_json::from_value::<Attribution>(invalid).is_err());
    }
}
