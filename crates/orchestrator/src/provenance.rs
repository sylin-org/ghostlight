//! Connection evidence: a service-issued identity and channel, the app Ghostlight saw start the
//! connection, and transient claims kept apart from durable attribution (ADR-0161).
//!
//! A connection's [`Attribution`] is minted when the service accepts its opening, with the
//! connection id, the accept time and the declared channel. Its [`Source`] is observed, never
//! claimed: once per connection, after the service has answered the opening and, for a fresh
//! session, `initialize`, the service reads the ancestors of the connection's peer process from
//! the operating system and keeps the first that lies in a catalog agent's installed app
//! ([`crate::install::apps`]), else the first that is not a wrapper. No field a client writes takes
//! part. Like the channel, it is provenance for display and history and never an input to
//! governance.
//!
//! When the name a connection's client claims is a catalog agent's, the attribution also keeps that
//! agent's catalog id as [`Attribution::claimed_agent`], so history names the agent after a restart
//! as the live row did (ADR-0202 Decision 14a). It is a claim, closed and bounded; the client's own
//! words stay in memory.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::install::apps::{is_wrapper, AppLocations, PATH_SEPARATORS};
use crate::install::icons::IconPlaces;
use crate::intake::session::IntakeChannel;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

pub use crate::language::provenance::ConnectionDetails;

const REPORTED_APPLICATION_MAX_CHARS: usize = 100;
const CONNECTION_ID_PREFIX: &str = "connection_";

/// The most ancestors of a connection's peer read for its source.
const ANCESTOR_LIMIT: usize = 16;

/// The longest executable file name a source keeps, in characters.
const IMAGE_MAX_CHARS: usize = 120;

/// The longest catalog agent id a source keeps, in characters.
const AGENT_ID_MAX_CHARS: usize = 64;

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
    /// The catalog id of the agent the connection's client claimed to be, when its claim names a
    /// catalog agent on an MCP connection; absent for any other claim and on records written
    /// before claims were kept. A claim, never verified, never identity and never authority.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_agent_id"
    )]
    pub claimed_agent: Option<String>,
    /// The app Ghostlight saw start the connection; absent when it could not be read and on
    /// records written before sources were kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

/// The app Ghostlight saw start a connection, from the operating system's facts about the
/// connection's peer process.
///
/// It is decided once per connection ([`Attribution::observed`]), and every request on the
/// connection inherits it. It holds a file name and never a path, which would carry the person's name. Reading bounds
/// both fields again, so an edited record cannot widen them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Source {
    /// The catalog id of the agent whose installed app started the connection, when one did.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_agent_id"
    )]
    pub agent: Option<String>,
    /// The executable file name of that app, or of the nearest ancestor that is not a wrapper
    /// when no agent's app started it.
    #[serde(deserialize_with = "deserialize_image")]
    pub image: String,
}

impl Source {
    /// The source of a connection whose peer is the process `process_id`: its ancestors are read
    /// (at most [`ANCESTOR_LIMIT`]), and the first that lies in a catalog agent's installed app
    /// wins, else the first that is not a wrapper. None when no process id is known or no ancestor
    /// but wrappers can be read; a failed read never fails the connection.
    #[must_use]
    pub fn observe(process_id: Option<u32>) -> Option<Self> {
        let ancestors = ghostlight_relay::os::ancestor_images(process_id?, ANCESTOR_LIMIT);
        if ancestors.iter().all(|image| is_wrapper(image)) {
            return None;
        }
        let apps = AppLocations::discover(&IconPlaces::system());
        Self::among(&ancestors, |image| apps.agent_of(image))
    }

    /// The source among `ancestors`, nearest first, with `agent_of` saying whose installed app a
    /// program is: the first ancestor that is an agent's app, else the first that is not a
    /// wrapper.
    fn among(
        ancestors: &[PathBuf],
        agent_of: impl Fn(&Path) -> Option<&'static str>,
    ) -> Option<Self> {
        let mut nearest = None;
        for image in ancestors.iter().filter(|image| !is_wrapper(image)) {
            if let Some(agent) = agent_of(image) {
                return Self::of(image, Some(agent));
            }
            nearest.get_or_insert(image);
        }
        Self::of(nearest?, None)
    }

    /// The source whose executable is at `image`, the installed app of `agent` when one is named.
    fn of(image: &Path, agent: Option<&str>) -> Option<Self> {
        Some(Self {
            agent: agent.and_then(bounded_agent_id),
            image: bounded_file_name(&image.to_string_lossy())?,
        })
    }
}

/// How the service reads a connection's [`Source`] from its peer's process id: the operating
/// system ([`Source::observe`]) in the service, and a stand-in in tests that hold or name the read.
#[derive(Clone)]
pub struct SourceReader(Arc<dyn Fn(Option<u32>) -> Option<Source> + Send + Sync>);

impl SourceReader {
    /// The reader that asks the operating system.
    #[must_use]
    pub fn system() -> Self {
        Self(Arc::new(Source::observe))
    }

    /// A reader that answers with `read`, for tests.
    #[cfg(test)]
    pub(crate) fn with(
        read: impl Fn(Option<u32>) -> Option<Source> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(read))
    }
}

impl ConnectionEvidence {
    /// The one evidence of an accepted connection: its `attribution`, observed and never changed
    /// afterwards, beside the bounded name its client `claim`ed, which stays memory-only. The
    /// attribution keeps only the catalog id that claim names, if any
    /// ([`crate::language::provenance::claimed_agent`]).
    #[must_use]
    pub fn new(attribution: Attribution, claim: &str) -> Self {
        let reported_application = bounded_label(claim, REPORTED_APPLICATION_MAX_CHARS);
        let claimed_agent =
            crate::language::provenance::claimed_agent(&reported_application, attribution.channel);
        Self {
            reported_application,
            attribution: Attribution {
                claimed_agent,
                ..attribution
            },
        }
    }

    /// Evidence of a connection on `channel` accepted now, whose client claimed `claim`, and with
    /// no source, for tests.
    #[cfg(test)]
    pub(crate) fn claimed(claim: &str, channel: IntakeChannel) -> Self {
        Self::new(Attribution::accepted(channel), claim)
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

    /// Present the live connection: its agent and the words that set its bounded application
    /// claim beside the source Ghostlight saw.
    #[must_use]
    pub fn details(&self) -> ConnectionDetails {
        crate::language::provenance::details(&self.attribution, Some(&self.reported_application))
    }
}

impl Attribution {
    /// The attribution of a connection on `channel` that the service accepts now: a fresh
    /// connection id and the accept time, and no source yet.
    #[must_use]
    pub fn accepted(channel: IntakeChannel) -> Self {
        Self {
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
            claimed_agent: None,
            source: None,
        }
    }

    /// This attribution with the source `reader` finds above the connection's `peer` process,
    /// read once. A failed read leaves the source unknown and never fails the connection.
    #[must_use]
    pub fn observed(mut self, peer: Option<u32>, reader: &SourceReader) -> Self {
        self.source = (reader.0)(peer);
        self
    }

    /// Read history written before connection identities existed: only the channel is known.
    #[must_use]
    pub fn legacy(channel: IntakeChannel) -> Self {
        Self {
            connection_id: None,
            observed_at_ms: None,
            channel,
            claimed_agent: None,
            source: None,
        }
    }

    /// Present retained evidence: its agent, named from the claimed catalog agent or the source as
    /// a live connection's is, without reconstructing any application claim.
    #[must_use]
    pub fn details(&self) -> ConnectionDetails {
        crate::language::provenance::details(self, None)
    }
}

/// What follows the last path separator of `value`, bounded and stripped of control and
/// direction text; none when nothing is left.
fn bounded_file_name(value: &str) -> Option<String> {
    let name = value.rsplit(PATH_SEPARATORS).next().unwrap_or_default();
    let name = bounded_label(name, IMAGE_MAX_CHARS);
    (!name.is_empty()).then_some(name)
}

/// `value` as a catalog agent id: lowercase ASCII letters, digits and dashes, bounded; none for
/// anything else.
fn bounded_agent_id(value: &str) -> Option<String> {
    let valid = !value.is_empty()
        && value.len() <= AGENT_ID_MAX_CHARS
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        });
    valid.then(|| value.to_owned())
}

fn deserialize_agent_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?
        .as_deref()
        .and_then(bounded_agent_id))
}

fn deserialize_image<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    bounded_file_name(&String::deserialize(deserializer)?)
        .ok_or_else(|| serde::de::Error::custom("an empty executable name"))
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
    use crate::agent::Agent;

    /// The agent the window shows for a connection these details describe.
    fn agent_of(details: &ConnectionDetails) -> Agent {
        details.agent.clone()
    }

    #[test]
    fn live_claim_and_durable_evidence_are_separate() {
        let evidence =
            ConnectionEvidence::claimed("claimed application secret", IntakeChannel::Mcp);
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
        let first = ConnectionEvidence::claimed("application", IntakeChannel::Mcp);
        let retained = first.attribution().clone();
        let second = ConnectionEvidence::claimed("application", IntakeChannel::Mcp);
        assert_ne!(retained.connection_id, second.attribution().connection_id);
        assert_eq!(retained, *first.attribution());
        assert!(retained.observed_at_ms.unwrap() > 0);
    }

    #[test]
    fn claims_are_bounded_and_stripped_of_control_text() {
        let evidence = ConnectionEvidence::claimed(
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
        assert_eq!(agent_of(&absent.details()), Agent::unknown());
    }

    #[test]
    fn a_live_connection_names_its_agent_and_history_names_the_catalog_agent_it_claimed() {
        let codex = ConnectionEvidence::claimed("codex-mcp-client", IntakeChannel::Mcp);
        assert_eq!(agent_of(&codex.details()).name, "Codex");
        assert_eq!(agent_of(&codex.details()).id, "codex");
        assert_eq!(
            codex.details().reported_application.as_deref(),
            Some("codex-mcp-client")
        );
        assert_eq!(codex.attribution().claimed_agent.as_deref(), Some("codex"));
        assert_eq!(agent_of(&codex.attribution().details()).id, "codex");
        assert_eq!(codex.attribution().details().reported_application, None);
        let unknown = ConnectionEvidence::claimed(" my agent ", IntakeChannel::Mcp);
        assert_eq!(agent_of(&unknown.details()).name, "my agent");
        assert_eq!(agent_of(&unknown.details()).id, "unknown:my agent");
        assert_eq!(unknown.attribution().claimed_agent, None);
        assert_eq!(
            agent_of(&unknown.attribution().details()),
            Agent::unknown(),
            "a name the catalog does not know is never kept"
        );
        let script = ConnectionEvidence::claimed("ghostlight call", IntakeChannel::Cli);
        assert_eq!(agent_of(&script.details()), Agent::command_line());
        assert_eq!(
            agent_of(&script.attribution().details()),
            Agent::command_line()
        );
        let unnamed = ConnectionEvidence::claimed("", IntakeChannel::Mcp);
        assert_eq!(agent_of(&unnamed.details()), Agent::unknown());
    }

    /// The evidence of an MCP connection whose client claimed `claim`, started by `source`.
    fn connected(claim: &str, source: Option<Source>) -> ConnectionEvidence {
        ConnectionEvidence::new(
            Attribution {
                source,
                ..Attribution::accepted(IntakeChannel::Mcp)
            },
            claim,
        )
    }

    /// Claude Desktop's installed app, as a source.
    fn claude_desktop() -> Option<Source> {
        Some(Source {
            agent: Some("claude-desktop".into()),
            image: "claude.exe".into(),
        })
    }

    /// `attribution` as it reads back from a record.
    fn restored(attribution: &Attribution) -> Attribution {
        serde_json::from_value(serde_json::to_value(attribution).unwrap()).unwrap()
    }

    #[test]
    fn restored_mcp_history_is_named_for_the_app_that_started_it() {
        let code = connected("claude-code", claude_desktop());
        let live = code.details();
        let history = restored(code.attribution()).details();
        assert_eq!(live.agent.id, "claude-code");
        assert_eq!(
            history.agent, live.agent,
            "the same name live and after a restart"
        );
        assert_eq!(history.explanation, "Claude Code, in Claude Desktop.");
        assert_eq!(history.explanation, live.explanation);

        let unnamed = restored(connected("my agent", claude_desktop()).attribution()).details();
        assert_eq!(unnamed.agent.id, "claude-desktop");
        assert_eq!(unnamed.explanation, "Started by Claude Desktop.");
    }

    #[test]
    fn restored_mcp_history_without_a_source_stays_unknown() {
        let unseen = restored(connected("my agent", None).attribution()).details();
        assert_eq!(unseen.agent, Agent::unknown());
        let program = restored(
            connected(
                "my agent",
                Some(Source {
                    agent: None,
                    image: "WindowsTerminal.exe".into(),
                }),
            )
            .attribution(),
        )
        .details();
        assert_eq!(program.agent, Agent::unknown());
        assert_eq!(program.explanation, "Started by WindowsTerminal.exe.");
        assert_eq!(
            restored(&Attribution::legacy(IntakeChannel::Mcp))
                .details()
                .agent,
            Agent::unknown()
        );
    }

    #[test]
    fn the_claimed_agent_is_kept_as_a_catalog_id_and_an_edited_one_is_dropped() {
        let code = connected("claude-code", None);
        let mut encoded = serde_json::to_value(code.attribution()).unwrap();
        assert_eq!(encoded["claimed_agent"], "claude-code");
        assert!(!encoded.to_string().contains("\"claude-code\" is"));
        encoded["claimed_agent"] = "Not An Id!".into();
        let edited: Attribution = serde_json::from_value(encoded).unwrap();
        assert_eq!(edited.claimed_agent, None);
        let unlisted = Attribution {
            claimed_agent: Some("retired-agent".into()),
            ..Attribution::accepted(IntakeChannel::Mcp)
        };
        assert_eq!(
            unlisted.details().agent,
            Agent::unknown(),
            "an id the catalog no longer lists names nobody"
        );
    }

    #[test]
    fn a_client_that_reported_no_name_is_the_unknown_agent_and_claims_nothing() {
        for unnamed in ["", " \t ", crate::intake::mcp::DEFAULT_CLIENT_LABEL] {
            let details = ConnectionEvidence::claimed(unnamed, IntakeChannel::Mcp).details();
            assert_eq!(agent_of(&details), Agent::unknown(), "{unnamed:?}");
            assert_eq!(details.reported_application, None, "{unnamed:?}");
            assert!(!details.explanation.contains('"'), "{unnamed:?}");
        }
    }

    #[test]
    fn an_agent_is_named_from_the_claim_the_card_shows() {
        let details =
            ConnectionEvidence::claimed("Some\u{202e}Agent", IntakeChannel::Mcp).details();
        assert_eq!(agent_of(&details).name, "SomeAgent");
        assert_eq!(details.reported_application.as_deref(), Some("SomeAgent"));
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

    /// A chain of ancestors, nearest first.
    fn chain(images: &[&str]) -> Vec<PathBuf> {
        images.iter().map(PathBuf::from).collect()
    }

    /// Claude Desktop's program as the Store installs it.
    const STORE_CLAUDE: &str =
        "C:/Program Files/WindowsApps/Claude_1.0.0.0_x64__pzs8sxrjxfjjc/app/claude.exe";

    /// Whose app a program is, by where it lies: Claude Desktop's Store package only.
    fn store_claude(image: &Path) -> Option<&'static str> {
        image
            .starts_with("C:/Program Files/WindowsApps/Claude_1.0.0.0_x64__pzs8sxrjxfjjc")
            .then_some("claude-desktop")
    }

    #[test]
    fn a_source_is_the_nearest_agent_app_past_wrappers_and_other_programs() {
        let source = Source::among(
            &chain(&[
                "C:/Users/ann/.ghostlight/bin/ghostlight.exe",
                "C:/Windows/System32/cmd.exe",
                "C:/Windows/System32/timeout.exe",
                "C:/Users/ann/AppData/Roaming/npm/node_modules/x/bin/agent.exe",
                STORE_CLAUDE,
                "C:/Windows/explorer.exe",
            ]),
            store_claude,
        )
        .unwrap();
        assert_eq!(
            source,
            Source {
                agent: Some("claude-desktop".into()),
                image: "claude.exe".into(),
            }
        );
    }

    #[test]
    fn with_no_agent_app_the_source_is_the_nearest_program_that_is_not_a_wrapper() {
        let source = Source::among(
            &chain(&[
                "/usr/bin/node",
                "/usr/bin/bash",
                "/usr/libexec/gnome-terminal-server",
                "/usr/lib/systemd/systemd",
            ]),
            store_claude,
        )
        .unwrap();
        assert_eq!(
            source,
            Source {
                agent: None,
                image: "gnome-terminal-server".into(),
            }
        );
        let same_name_elsewhere =
            Source::among(&chain(&["C:/Users/ann/Downloads/claude.exe"]), store_claude).unwrap();
        assert_eq!(
            same_name_elsewhere.agent, None,
            "an app is recognized by where it is installed, not by its file name"
        );
        assert_eq!(same_name_elsewhere.image, "claude.exe");
    }

    #[test]
    fn only_wrappers_or_nothing_leave_the_source_unknown() {
        assert_eq!(Source::among(&[], store_claude), None);
        assert_eq!(
            Source::among(
                &chain(&["C:/Windows/System32/cmd.exe", "/usr/bin/python3.12"]),
                store_claude
            ),
            None
        );
        assert_eq!(Source::observe(None), None);
    }

    /// How long the source observation takes for a cargo test peer on this machine. It reads the
    /// real process tree and the installed apps, so it stays out of the default run:
    /// `cargo test -p ghostlight-service source_observation_time -- --ignored --nocapture`.
    #[test]
    #[ignore = "measures this machine; run on demand"]
    fn the_source_observation_time_on_this_machine() {
        const RUNS: usize = 10;
        let peer = Some(std::process::id());
        let mut spans: Vec<std::time::Duration> = (0..RUNS)
            .map(|_| {
                let start = std::time::Instant::now();
                assert!(Source::observe(peer).is_some());
                start.elapsed()
            })
            .collect();
        let first = spans[0];
        spans.sort();
        println!(
            "source observation, {RUNS} runs: first {first:?}, median {:?}, slowest {:?}",
            spans[RUNS / 2],
            spans[RUNS - 1]
        );
    }

    #[test]
    fn a_source_is_kept_on_the_record_as_a_file_name_and_a_record_without_one_still_reads() {
        let evidence = ConnectionEvidence::new(
            Attribution {
                source: Some(Source {
                    agent: Some("claude-desktop".into()),
                    image: "claude.exe".into(),
                }),
                ..Attribution::accepted(IntakeChannel::Mcp)
            },
            "claude-code",
        );
        let encoded = serde_json::to_value(evidence.attribution()).unwrap();
        assert_eq!(
            encoded["source"],
            serde_json::json!({ "agent": "claude-desktop", "image": "claude.exe" })
        );
        let restored: Attribution = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(&restored, evidence.attribution());

        let mut without = encoded.clone();
        without.as_object_mut().unwrap().remove("source");
        let restored: Attribution = serde_json::from_value(without).unwrap();
        assert_eq!(restored.source, None);
        assert!(serde_json::to_value(&restored)
            .unwrap()
            .get("source")
            .is_none());

        let mut edited = encoded;
        edited["source"] = serde_json::json!({
            "agent": "Not An Id!",
            "image": "C:\\Users\\ann\\AppData\\Local\\agent.exe",
        });
        let restored: Attribution = serde_json::from_value(edited).unwrap();
        assert_eq!(
            restored.source,
            Some(Source {
                agent: None,
                image: "agent.exe".into(),
            }),
            "a path is cut to its file name and an id that is not one is dropped"
        );
    }

    #[test]
    fn restored_evidence_rejects_arbitrary_connection_claims() {
        let evidence = ConnectionEvidence::claimed("transient claim", IntakeChannel::Mcp);
        let mut invalid = serde_json::to_value(evidence.attribution()).unwrap();
        invalid["connection_id"] = "private supplied text".into();
        assert!(serde_json::from_value::<Attribution>(invalid).is_err());
    }
}
