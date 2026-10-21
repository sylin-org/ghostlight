//! Connection evidence: a service-issued identity and channel, the app Ghostlight saw start the
//! connection, and transient claims kept apart from durable attribution (ADR-0161).
//!
//! A connection's [`Attribution`] is minted when the service accepts its opening, with the
//! connection id, the accept time and the declared channel. Its [`Source`] is observed, never
//! claimed: once per connection, after the service has answered the opening and, for a fresh
//! session, `initialize`, the service reads the ancestors of the connection's peer process from
//! the operating system and keeps the first that lies in a catalog agent's installed app
//! ([`crate::install::apps`]), else the first that is not a wrapper. No field a client writes takes
//! part. It is provenance for display and history and never an input to governance. The channel
//! is too, except that a policy's channel switch admits or refuses a request by it (ADR-0202
//! Decision 22a).
//!
//! The same read names the source's running process, its id and start time ([`SourceProcess`]).
//! It stays in memory and is never recorded: the service reads it only to offer one agent one set
//! of tools (ADR-0203, amendment of 2026-10-19).
//!
//! When the name a connection's client claims is a catalog agent's, the attribution also keeps that
//! agent's catalog id as [`Attribution::claimed_agent`], so history names the agent after a restart
//! as the live row did (ADR-0202 Decision 14a). It is a claim, closed and bounded; the client's own
//! words stay in memory.
//!
//! The name a client claims is bounded once, here ([`bounded_claim`]), wherever it is read: from
//! the client's `initialize`, from a resume token, and into a connection's evidence. A client that
//! names nothing claims nothing (ADR-0203 Decision 9).

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agent::Agent;
use crate::install::apps::{is_wrapper, AppLocations, PATH_SEPARATORS};
use crate::install::icons::IconPlaces;
use crate::intake::session::IntakeChannel;
use ghostlight_relay::os::Ancestor;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::language::provenance::Claimed;
pub use crate::language::provenance::ConnectionDetails;

/// The longest name a client's claim keeps, in characters.
const CLAIM_MAX_CHARS: usize = 100;
const CONNECTION_ID_PREFIX: &str = "connection_";

/// The most ancestors of a connection's peer read for its source.
const ANCESTOR_LIMIT: usize = 16;

/// The longest executable file name a source keeps, in characters.
const IMAGE_MAX_CHARS: usize = 120;

/// The longest catalog agent id a source keeps, in characters.
const AGENT_ID_MAX_CHARS: usize = 64;

/// The name a client `reported` for itself, as Ghostlight keeps it: stripped of control and
/// direction text, cut to [`CLAIM_MAX_CHARS`] and trimmed. None when nothing is left, so an empty
/// name and no name are one claim: none.
#[must_use]
pub fn bounded_claim(reported: &str) -> Option<String> {
    let claim = bounded_label(reported, CLAIM_MAX_CHARS);
    (!claim.is_empty()).then_some(claim)
}

/// Evidence captured once for one accepted connection. Caller claims remain memory-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionEvidence {
    claim: Option<String>,
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

/// The running process of a connection's [`Source`]: its id and its start time, which together
/// name one process, since an id the system hands out again belongs to a later process.
///
/// Observed with the source and never claimed. It stays in memory: no record, line or surface
/// carries it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceProcess {
    /// The process id.
    pub id: u32,
    /// When the process started, in the system's own unit.
    pub started: u64,
}

/// What one read of a connection's peer found: the [`Source`] the record keeps, and the
/// [`SourceProcess`] it names, which stays in memory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    /// The app that started the connection.
    pub source: Source,
    /// The running process of that app.
    pub process: SourceProcess,
}

impl Observation {
    /// What started a connection whose peer is the process `process_id`: its ancestors are read
    /// (at most [`ANCESTOR_LIMIT`]), and the first that lies in a catalog agent's installed app
    /// wins, else the first that is not a wrapper. None when no process id is known or no ancestor
    /// but wrappers can be read; a failed read never fails the connection.
    #[must_use]
    pub fn of_peer(process_id: Option<u32>) -> Option<Self> {
        let ancestors = ghostlight_relay::os::ancestors(process_id?, ANCESTOR_LIMIT);
        if ancestors.iter().all(|ancestor| is_wrapper(&ancestor.image)) {
            return None;
        }
        let apps = AppLocations::discover(&IconPlaces::system());
        Self::among(&ancestors, |image| apps.agent_of(image))
    }

    /// The observation among `ancestors`, nearest first, with `agent_of` saying whose installed
    /// app a program is: the first ancestor that is an agent's app, else the first that is not a
    /// wrapper.
    fn among(
        ancestors: &[Ancestor],
        agent_of: impl Fn(&Path) -> Option<&'static str>,
    ) -> Option<Self> {
        let mut nearest = None;
        for ancestor in ancestors
            .iter()
            .filter(|ancestor| !is_wrapper(&ancestor.image))
        {
            if let Some(agent) = agent_of(&ancestor.image) {
                return Self::of(ancestor, Some(agent));
            }
            nearest.get_or_insert(ancestor);
        }
        Self::of(nearest?, None)
    }

    /// The observation of `ancestor`, the installed app of `agent` when one is named.
    fn of(ancestor: &Ancestor, agent: Option<&str>) -> Option<Self> {
        Some(Self {
            source: Source {
                agent: agent.and_then(bounded_agent_id),
                image: bounded_file_name(&ancestor.image.to_string_lossy())?,
            },
            process: SourceProcess {
                id: ancestor.process_id,
                started: ancestor.started,
            },
        })
    }
}

/// How the service reads what started a connection from its peer's process id: the operating
/// system ([`Observation::of_peer`]) in the service, and a stand-in in tests that hold or name the
/// read.
#[derive(Clone)]
pub struct SourceReader(Arc<dyn Fn(Option<u32>) -> Option<Observation> + Send + Sync>);

impl SourceReader {
    /// The reader that asks the operating system.
    #[must_use]
    pub fn system() -> Self {
        Self(Arc::new(Observation::of_peer))
    }

    /// A reader that answers with `read`, for tests.
    #[cfg(test)]
    pub(crate) fn with(
        read: impl Fn(Option<u32>) -> Option<Observation> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(read))
    }

    /// Read what started the connection whose peer is the process `peer`, once. A failed read is
    /// none and never fails the connection.
    #[must_use]
    pub fn read(&self, peer: Option<u32>) -> Option<Observation> {
        (self.0)(peer)
    }
}

impl ConnectionEvidence {
    /// The one evidence of an accepted connection: its `attribution`, observed and never changed
    /// afterwards, beside the name its client `claim`ed, if any, bounded ([`bounded_claim`]) and
    /// kept in memory only. The attribution keeps only the catalog id that claim names, if any
    /// ([`crate::language::provenance::claimed_agent`]).
    #[must_use]
    pub fn new(attribution: Attribution, claim: Option<&str>) -> Self {
        let claim = claim.and_then(bounded_claim);
        let claimed_agent =
            crate::language::provenance::claimed_agent(claim.as_deref(), attribution.channel);
        Self {
            claim,
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
        Self::new(Attribution::accepted(channel), Some(claim))
    }

    /// Borrow the durable evidence for this connection; it contains no caller-supplied label.
    #[must_use]
    pub fn attribution(&self) -> &Attribution {
        &self.attribution
    }

    /// The bounded name the connection's client claimed, none when it claimed none.
    #[must_use]
    pub fn claim(&self) -> Option<&str> {
        self.claim.as_deref()
    }

    /// The key of the kept tabs a workspace this connection opens may reuse, and leaves for reuse:
    /// the id of the agent its claim names on its channel ([`Agent::of`]). Every command-line
    /// session shares the command line's; a claim the catalog knows gives that agent's catalog
    /// id; any other claim gives an unknown agent under its claimed name. A claim that names no
    /// agent gives none, so such a session reuses no kept tab and no one reuses its tabs.
    ///
    /// It is a claim, a courtesy between clients and never authority. The source Ghostlight saw
    /// takes no part, because the source decides nothing (ADR-0202 Decision 1, ADR-0203
    /// Decision 9).
    #[must_use]
    pub fn reuse_key(&self) -> Option<String> {
        let agent = Agent::of(self.claim(), self.attribution.channel);
        (agent != Agent::unknown()).then_some(agent.id)
    }

    /// Present the live connection: its agent and the words that set its bounded claim beside
    /// the source Ghostlight saw.
    #[must_use]
    pub fn details(&self) -> ConnectionDetails {
        crate::language::provenance::details(&self.attribution, Claimed::Live(self.claim()))
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

    /// This attribution with the source of `observation`, the one read of the connection's peer.
    /// None leaves the source unknown.
    #[must_use]
    pub fn observed(mut self, observation: Option<&Observation>) -> Self {
        self.source = observation.map(|observation| observation.source.clone());
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
        crate::language::provenance::details(self, Claimed::Unkept)
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
    use std::path::PathBuf;

    use super::*;

    /// A chain of ancestors, nearest first: the nearest is process 1, and each is older than the
    /// one below it.
    fn chain(images: &[&str]) -> Vec<Ancestor> {
        (1..)
            .zip(images)
            .map(|(id, image)| Ancestor {
                process_id: id,
                started: u64::from(100 - id),
                image: PathBuf::from(image),
            })
            .collect()
    }

    /// The source [`Observation::among`] finds in `ancestors`.
    fn source_among(
        ancestors: &[Ancestor],
        agent_of: impl Fn(&Path) -> Option<&'static str>,
    ) -> Option<Source> {
        Observation::among(ancestors, agent_of).map(|observation| observation.source)
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

    /// ACCEPTANCE CR (ADR-0161, amendment of 2026-10-18): a connection's source is the nearest
    /// ancestor of its peer that lies in a catalog agent's installed app, past wrappers and other
    /// programs, else the nearest ancestor that is not a wrapper, kept as a file name and never a
    /// path. An app is known by where it is installed, never by its file name; the process kept
    /// is the source's own, never a wrapper below it; and with only wrappers or nothing to read,
    /// the source stays unknown.
    #[test]
    fn a_source_is_the_nearest_agent_app_else_the_nearest_program_past_wrappers() {
        let observation = Observation::among(
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
            observation.source,
            Source {
                agent: Some("claude-desktop".into()),
                image: "claude.exe".into(),
            }
        );
        assert_eq!(
            observation.process,
            SourceProcess { id: 5, started: 95 },
            "the process is the source's own, never a wrapper below it"
        );

        let source = source_among(
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
            source_among(&chain(&["C:/Users/ann/Downloads/claude.exe"]), store_claude).unwrap();
        assert_eq!(
            same_name_elsewhere.agent, None,
            "an app is recognized by where it is installed, not by its file name"
        );
        assert_eq!(same_name_elsewhere.image, "claude.exe");

        assert_eq!(source_among(&[], store_claude), None);
        assert_eq!(
            source_among(
                &chain(&["C:/Windows/System32/cmd.exe", "/usr/bin/python3.12"]),
                store_claude
            ),
            None
        );
        assert_eq!(Observation::of_peer(None), None);
    }
}
