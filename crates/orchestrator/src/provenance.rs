//! Connection-bound observations, with transient claims separate from durable attribution.

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use ghostlight_bridge::service::IntakeChannel;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use crate::language::provenance::ConnectionDetails;

const REPORTED_APPLICATION_MAX_CHARS: usize = 100;
const EXECUTABLE_MAX_CHARS: usize = 120;
const CONNECTION_ID_PREFIX: &str = "connection_";

/// Observations captured once for one accepted connection. Caller claims remain memory-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionEvidence {
    reported_application: String,
    attribution: Attribution,
}

/// Bounded connection evidence retained beside an action, never an admission decision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attribution {
    /// Service-issued opaque connection identity, absent from older receipts.
    #[serde(deserialize_with = "deserialize_connection_id")]
    pub connection_id: Option<String>,
    /// Time the connection was observed, absent from older receipts.
    pub observed_at_ms: Option<u64>,
    /// Declared intake, separate from executable observation and never authority.
    pub channel: IntakeChannel,
    /// What the platform could observe about the directly connected executable.
    pub peer: PeerObservation,
    /// Signature verification is deferred; an observed file name is not a verified binary.
    pub signature: SignatureStatus,
}

/// Closed evidence states distinguish missing observation from unsupported platforms.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PeerObservation {
    /// The platform observed the directly connected executable's bounded file name.
    Observed {
        #[serde(deserialize_with = "deserialize_executable")]
        executable: String,
    },
    /// This build has no peer-observation implementation for its platform.
    UnsupportedPlatform,
    /// The supported platform could not identify this connection.
    Unavailable,
    /// The historical record contains no usable observation evidence.
    NotRecorded,
}

/// Signature state is explicit even while verification remains unimplemented.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureStatus {
    NotChecked,
}

impl ConnectionEvidence {
    /// Observe a new socket connection without retaining its PID or executable path.
    #[must_use]
    pub fn observe(
        label: &str,
        channel: IntakeChannel,
        local: Option<SocketAddr>,
        peer: Option<SocketAddr>,
    ) -> Self {
        #[cfg(target_os = "windows")]
        let observation = local
            .zip(peer)
            .and_then(|(local, peer)| ghostlight_win_peer::identify_addresses(local, peer))
            .map_or(PeerObservation::Unavailable, |identity| {
                PeerObservation::Observed {
                    executable: identity.image_name,
                }
            });
        #[cfg(not(target_os = "windows"))]
        let observation = {
            let _ = (local, peer);
            PeerObservation::UnsupportedPlatform
        };
        Self::with_peer(label, channel, observation)
    }

    /// Capture a new connection with an explicit observation, including synthetic test peers.
    #[must_use]
    pub fn with_peer(label: &str, channel: IntakeChannel, peer: PeerObservation) -> Self {
        let peer = match peer {
            PeerObservation::Observed { executable } => executable_name(&executable)
                .map_or(PeerObservation::Unavailable, |executable| {
                    PeerObservation::Observed { executable }
                }),
            other => other,
        };
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
                peer,
                signature: SignatureStatus::NotChecked,
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

    /// Present the live connection with its separately identified, bounded application claim.
    #[must_use]
    pub fn details(&self) -> ConnectionDetails {
        let mut details = self.attribution.details();
        details.reported_application =
            (!self.reported_application.is_empty()).then(|| self.reported_application.clone());
        details
    }
}

impl Attribution {
    /// Read older attribution without promoting the previous observer-side lookup to peer evidence.
    #[must_use]
    pub fn legacy(channel: IntakeChannel, _peer_image: Option<&str>) -> Self {
        Self {
            connection_id: None,
            observed_at_ms: None,
            channel,
            peer: PeerObservation::NotRecorded,
            signature: SignatureStatus::NotChecked,
        }
    }

    /// Return a valid bounded observed basename, including when examining restored evidence.
    #[must_use]
    pub fn observed_executable(&self) -> Option<&str> {
        match &self.peer {
            PeerObservation::Observed { executable }
                if executable_name(executable).as_deref() == Some(executable.as_str()) =>
            {
                Some(executable)
            }
            _ => None,
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

fn executable_name(value: &str) -> Option<String> {
    if value.contains(['/', '\\', ':']) {
        return None;
    }
    let mut name = bounded_label(value, EXECUTABLE_MAX_CHARS);
    name.make_ascii_lowercase();
    (!name.is_empty() && name != "." && name != "..").then_some(name)
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

fn deserialize_executable<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    executable_name(&value)
        .ok_or_else(|| serde::de::Error::custom("invalid observed executable name"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_claim_and_durable_observation_are_separate() {
        let evidence = ConnectionEvidence::with_peer(
            "claimed application secret",
            IntakeChannel::Mcp,
            PeerObservation::Observed {
                executable: "Ghostlight-MCP-Connector.EXE".into(),
            },
        );
        let encoded = serde_json::to_string(evidence.attribution()).unwrap();
        assert!(!encoded.contains("claimed application secret"));
        assert!(!encoded.contains("reported_application"));
        assert!(!encoded.contains("process_id"));
        assert!(!encoded.contains("path"));
        assert_eq!(
            evidence.details().reported_application.as_deref(),
            Some("claimed application secret")
        );
        assert_eq!(evidence.attribution().details().reported_application, None);
        assert_eq!(
            evidence.attribution().observed_executable(),
            Some("ghostlight-mcp-connector.exe")
        );
        let restored: Attribution = serde_json::from_str(&encoded).unwrap();
        assert_eq!(&restored, evidence.attribution());
        assert_eq!(restored.signature, SignatureStatus::NotChecked);
    }

    #[test]
    fn every_connection_gets_fresh_evidence_even_for_the_same_claim() {
        let first = ConnectionEvidence::with_peer(
            "application",
            IntakeChannel::Mcp,
            PeerObservation::Unavailable,
        );
        let retained = first.attribution().clone();
        let second = ConnectionEvidence::with_peer(
            "application",
            IntakeChannel::Mcp,
            PeerObservation::UnsupportedPlatform,
        );
        assert_ne!(retained.connection_id, second.attribution().connection_id);
        assert_eq!(retained, *first.attribution());
        assert!(retained.observed_at_ms.unwrap() > 0);
        assert_eq!(retained.peer, PeerObservation::Unavailable);
        assert_ne!(first.details().observation, second.details().observation);
    }

    #[test]
    fn claims_and_observed_names_are_bounded_and_path_free() {
        let evidence = ConnectionEvidence::with_peer(
            &format!("\n\u{202e}{}\0", "x".repeat(150)),
            IntakeChannel::Cli,
            PeerObservation::Observed {
                executable: format!("\n{}\u{202e}", "X".repeat(150)),
            },
        );
        assert_eq!(
            evidence.details().reported_application,
            Some("x".repeat(100))
        );
        assert_eq!(
            evidence.attribution().observed_executable(),
            Some("x".repeat(120).as_str())
        );
        for name in [
            "C:\\Users\\private\\client.exe",
            "/home/private/client",
            "C:client.exe",
            "\n\0",
            ".",
            "..",
        ] {
            let evidence = ConnectionEvidence::with_peer(
                "claim",
                IntakeChannel::Mcp,
                PeerObservation::Observed {
                    executable: name.into(),
                },
            );
            assert_eq!(evidence.attribution().peer, PeerObservation::Unavailable);
            assert_eq!(evidence.details().observed_executable, None);
        }
    }

    #[test]
    fn historical_evidence_does_not_invent_connection_information() {
        let absent = Attribution::legacy(IntakeChannel::Mcp, None);
        assert_eq!(absent.peer, PeerObservation::NotRecorded);
        assert_eq!(absent.connection_id, None);
        assert_eq!(absent.observed_at_ms, None);
        assert_eq!(absent.details().reported_application, None);
        let observed = Attribution::legacy(IntakeChannel::Cli, Some("ghostlight.exe"));
        assert_eq!(observed.observed_executable(), None);
        assert_eq!(observed.peer, PeerObservation::NotRecorded);
        assert_eq!(
            observed.details().explanation,
            "Earlier history did not establish the connecting executable."
        );
        assert_eq!(observed.connection_id, None);
        assert_eq!(observed.observed_at_ms, None);
        assert_eq!(
            Attribution::legacy(IntakeChannel::Cli, Some("C:\\private.exe")).peer,
            PeerObservation::NotRecorded
        );
    }

    #[test]
    fn restored_evidence_rejects_paths_and_arbitrary_connection_claims() {
        let evidence = ConnectionEvidence::with_peer(
            "transient claim",
            IntakeChannel::Mcp,
            PeerObservation::Observed {
                executable: "connector.exe".into(),
            },
        );
        let original = serde_json::to_value(evidence.attribution()).unwrap();
        let mut invalid = original.clone();
        invalid["connection_id"] = "private supplied text".into();
        assert!(serde_json::from_value::<Attribution>(invalid).is_err());
        let mut invalid = original.clone();
        invalid["peer"]["executable"] = "C:\\Users\\private\\connector.exe".into();
        assert!(serde_json::from_value::<Attribution>(invalid).is_err());
        let mut bounded = original;
        bounded["peer"]["executable"] = format!("\n{}\u{202e}", "x".repeat(500)).into();
        let restored: Attribution = serde_json::from_value(bounded).unwrap();
        assert_eq!(
            restored.observed_executable(),
            Some("x".repeat(120).as_str())
        );
    }

    #[test]
    fn missing_socket_information_has_an_explicit_platform_state() {
        let evidence = ConnectionEvidence::observe("application", IntakeChannel::Mcp, None, None);
        #[cfg(target_os = "windows")]
        assert_eq!(evidence.attribution().peer, PeerObservation::Unavailable);
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            evidence.attribution().peer,
            PeerObservation::UnsupportedPlatform
        );
        assert_eq!(
            evidence.attribution().signature,
            SignatureStatus::NotChecked
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_live_connection_observes_the_executable_without_retaining_process_details() {
        use std::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let _client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (accepted, _) = listener.accept().unwrap();
        let evidence = ConnectionEvidence::observe(
            "upstream application claim",
            IntakeChannel::Mcp,
            accepted.local_addr().ok(),
            accepted.peer_addr().ok(),
        );
        let executable = evidence.attribution().observed_executable().unwrap();
        let expected = std::env::current_exe().unwrap();
        assert_eq!(
            executable_name(expected.file_name().unwrap().to_str().unwrap()).as_deref(),
            Some(executable)
        );
        assert_eq!(
            evidence.attribution().signature,
            SignatureStatus::NotChecked
        );
    }
}
