//! The one place a connection's agent is named and its explanation authored, live and from
//! history alike ([`details`]): from the channel, the claim and the source Ghostlight saw start the
//! connection, through a closed relation between the agent and that source. Also the words the
//! window names an agent by when no roster row names it, and the title of the browser tab group
//! named for a workspace's opener ([`group_title`]).

use crate::intake::session::IntakeChannel;
use serde::Serialize;

use crate::agent::Agent;
use crate::provenance::{Attribution, Source};

/// The caveat of an agent a claim named, when nothing else Ghostlight saw says more.
const CLAIM_CAVEAT: &str = "The agent's name is its own claim. Ghostlight does not verify it.";
/// What a command-line session is, when nothing says where it was started from.
const CLI_SESSION: &str = "A ghostlight call session is a local program.";
/// What Ghostlight cannot vouch for in any command-line session.
const CLI_SCRIPTS_UNVERIFIED: &str =
    "Ghostlight does not verify the scripts and instructions it runs.";
const LEGACY_EXPLANATION: &str = "Earlier history did not establish the connecting executable.";

/// What an MCP connection with no source establishes about the name its client `reported`: the
/// name is the agent's own claim, which Ghostlight does not verify.
#[must_use]
pub fn claimed_name_explanation(reported: &str) -> String {
    format!("\"{reported}\" is what the agent calls itself; Ghostlight does not verify it.")
}

/// The name of a command-line session's agent ([`crate::agent::Agent::command_line`]).
pub const COMMAND_LINE_AGENT: &str = "Command line";

/// The name a client `reported` for itself, trimmed, or none when it reported none: a name that is
/// empty after trimming names nobody.
#[must_use]
pub fn claimed_name(reported: &str) -> Option<&str> {
    let name = reported.trim();
    (!name.is_empty()).then_some(name)
}

/// The name of an agent nothing names: a live client that reported no name, or a record whose
/// source says no agent's app started it ([`crate::agent::Agent::unknown`]).
pub const UNKNOWN_AGENT: &str = "Unknown agent";

/// The name of an action read back from an earlier service run whose record kept neither a
/// claimed agent nor a source ([`crate::agent::Agent::earlier`]).
pub const EARLIER_SESSION: &str = "Earlier session";

/// What the window says on hover beside [`EARLIER_SESSION`]. It says only what the record shows:
/// a record written before agent names were kept and one whose client claimed a name the catalog
/// does not know, with no source read, look the same.
pub const EARLIER_SESSION_NOTE: &str =
    "Recorded in an earlier session. Ghostlight kept no agent name for it.";

/// What the reader of a connection's details knows of its client's claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Claimed<'a> {
    /// A live connection, whose client claimed this bounded name, or none.
    Live(Option<&'a str>),
    /// A record read back from history, which keeps no claim.
    Unkept,
}

/// What every tab group Ghostlight names begins with: the ghost emoji (U+1F47B) and a space. The
/// popup reads the agent's name after it (ADR-0098 amendment of 2026-10-21). Chrome's group label
/// takes only text and a color, so the ghost is the one mark of Ghostlight it can carry. 2.0.0 wrote
/// "Ghostlight - " instead; the extension renames a group it finds under that prefix
/// (`legacy_group_title_prefixes` in the protocol 4 vocabulary), so each agent keeps one group.
pub(crate) const GROUP_TITLE_PREFIX: &str = "\u{1F47B} ";

/// The longest agent name a tab group's title carries, in characters.
const GROUP_TITLE_AGENT_CHARS: usize = 80;

/// The title of the browser tab group a workspace's tabs sit in: [`GROUP_TITLE_PREFIX`] and the
/// name of the `agent` that opened the workspace, as the window names it ([`details`]), such as the
/// ghost and "Claude Code", the ghost and "Command line" and, for an opener nothing names, the
/// ghost and "Unknown agent".
///
/// It depends on that one name (ADR-0066, ADR-0203 Decision 9), so every session of one agent
/// shares one group, and a later connection to the same workspace renames nothing.
#[must_use]
pub fn group_title(agent: &Agent) -> String {
    let name: String = agent.name.chars().take(GROUP_TITLE_AGENT_CHARS).collect();
    format!("{GROUP_TITLE_PREFIX}{name}")
}

/// Details for local human surfaces only; a live caller claim must never become audit content.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConnectionDetails {
    /// Opaque service connection identity, when retained.
    pub connection_id: Option<String>,
    /// Bounded live claim, absent when details come from durable history.
    pub reported_application: Option<String>,
    /// Declared intake, never an authorization fact.
    pub channel: IntakeChannel,
    /// What the connection establishes and what it cannot establish; empty when it names no
    /// agent and so has nothing to vouch for.
    pub explanation: String,
    /// The connection's agent, named by [`details`]. It is never serialized: the summaries that
    /// show an agent copy it into their own `agent` field, so these details keep the keys a
    /// watcher already sees.
    #[serde(skip)]
    pub agent: Agent,
    /// The catalog agent whose installed app Ghostlight saw start the connection, when one did,
    /// so the window can tell which host app a session runs inside. Never serialized, like
    /// `agent`.
    #[serde(skip)]
    pub source_agent: Option<Agent>,
}

/// The catalog id of the agent a connection's client claims to be when it `reported` a name on
/// `channel`: the roster agent an MCP client's name names, and none for a name the catalog does
/// not know, for a client that reported none, and on the command line, whose channel names its
/// agent. The attribution keeps it as a claim ([`Attribution::claimed_agent`]).
#[must_use]
pub fn claimed_agent(reported: Option<&str>, channel: IntakeChannel) -> Option<String> {
    match channel {
        IntakeChannel::Cli => None,
        IntakeChannel::Mcp => reported
            .and_then(claimed_name)
            .and_then(crate::install::agent_called)
            .map(|agent| agent.id),
    }
}

/// How a connection's agent stands to the app Ghostlight saw start the connection.
enum Relation {
    /// No source was seen.
    Unseen,
    /// The source is the agent's own app.
    Own,
    /// The catalog knows the agent runs inside the source's app ([`crate::install::runs_inside`]).
    Inside,
    /// The agent and the source stand apart, or the catalog does not say how they relate.
    Apart,
}

impl Relation {
    /// How `agent` stands to `source`.
    fn of(agent: &Agent, source: Option<&Source>) -> Self {
        let Some(source) = source else {
            return Self::Unseen;
        };
        match source.agent.as_deref() {
            Some(host) if host == agent.id => Self::Own,
            Some(host) if crate::install::runs_inside(&agent.id, host) => Self::Inside,
            _ => Self::Apart,
        }
    }
}

/// Name a connection's agent and author what it establishes, live and from history alike, from
/// its `attribution` and what the reader knows of its client's claim (`known`), which only a
/// live connection knows.
///
/// The agent is the command line's on the command line. On an MCP connection the claim names it:
/// the live claim, else the catalog agent the record kept as claimed. Without a claim the source's
/// catalog agent names it. Without either it is the unknown agent, except for a record read back
/// from an earlier run that kept neither a claimed agent nor a source, which is the earlier
/// session's.
///
/// The explanation of an MCP connection follows how that agent stands to the source. When the
/// source is the agent's own app, the sentence says where it was seen. When the catalog knows the
/// agent runs inside the source's app, it says both. When they stand apart (a name the catalog
/// does not know, a source that is no agent's app, or an app the catalog does not know the agent
/// runs inside), it says the source, and the words a live client claimed, without judging either.
/// With no source it keeps the caveat that the name is the agent's own claim, and says nothing
/// when nothing named the agent. The words a live client sent are quoted only where they say more
/// than the agent's name ([`says_more`]); a record keeps no words, so its sentence never quotes
/// any. The command line says where it was called from.
#[must_use]
pub fn details(attribution: &Attribution, known: Claimed<'_>) -> ConnectionDetails {
    let (live, reported) = match known {
        Claimed::Live(reported) => (true, reported.and_then(claimed_name)),
        Claimed::Unkept => (false, None),
    };
    let source = attribution.source.as_ref();
    let source_agent = source
        .and_then(|source| source.agent.as_deref())
        .and_then(crate::install::catalog_agent);
    let (agent, explanation) = match attribution.channel {
        // A command-line session is named for the agent whose app runs it, so its tabs sit in that
        // agent's group, and is "Command line" for a person's terminal (ADR-0106, amendment of
        // 2026-10-21).
        IntakeChannel::Cli => (
            source_agent.clone().unwrap_or_else(Agent::command_line),
            source.map_or_else(
                || format!("{CLI_SESSION} {CLI_SCRIPTS_UNVERIFIED}"),
                command_line_from,
            ),
        ),
        IntakeChannel::Mcp => {
            let claim = claimed(attribution, reported);
            let named = claim.is_some();
            let agent = claim.unwrap_or_else(|| {
                source_agent.clone().unwrap_or_else(|| {
                    if live || source.is_some() || attribution.claimed_agent.is_some() {
                        Agent::unknown()
                    } else {
                        Agent::earlier()
                    }
                })
            });
            let quoted = reported.filter(|reported| says_more(reported, &agent));
            let explanation = explain(&agent, quoted, named, source);
            (agent, explanation)
        }
    };
    ConnectionDetails {
        connection_id: attribution.connection_id.clone(),
        reported_application: reported.map(str::to_owned),
        channel: attribution.channel,
        explanation: if attribution.connection_id.is_none() && !explanation.is_empty() {
            LEGACY_EXPLANATION.to_owned()
        } else {
            explanation
        },
        agent,
        source_agent,
    }
}

/// Whether the name a live client `reported` says more than the `agent` it names: it reads
/// differently once case, space and punctuation are set aside ("codex-mcp-client" for Codex, but
/// not "claude-code" for Claude Code), or it named no agent, being a reserved entry's name.
fn says_more(reported: &str, agent: &Agent) -> bool {
    let plain = |text: &str| -> String {
        text.chars()
            .filter(|character| character.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    *agent == Agent::unknown() || plain(reported) != plain(&agent.name)
}

/// The agent an MCP connection's claim names: the one the name its client `reported` live names
/// ([`Agent::of`]), else the catalog agent its `attribution` kept as claimed; none when neither
/// names one.
fn claimed(attribution: &Attribution, reported: Option<&str>) -> Option<Agent> {
    match reported {
        Some(reported) => Some(Agent::of(Some(reported), IntakeChannel::Mcp)),
        None => attribution
            .claimed_agent
            .as_deref()
            .and_then(crate::install::catalog_agent),
    }
}

/// What an MCP connection establishes about `agent` beside the `source` Ghostlight saw start it.
/// Only `quoted`, words a live client sent that say more than the agent's name, is ever quoted: a
/// record keeps no words, so its sentence says only what the source and the caveat say. An agent a
/// claim `named` keeps the caveat unless its own app, or an app the catalog knows it runs inside,
/// started the connection; an agent nothing named gets no caveat, and with no source no sentence.
fn explain(agent: &Agent, quoted: Option<&str>, named: bool, source: Option<&Source>) -> String {
    match (Relation::of(agent, source), source) {
        (Relation::Own, Some(source)) => started_by(source),
        (Relation::Inside, Some(source)) => format!("{}, in {}.", agent.name, source_name(source)),
        (Relation::Apart, Some(source)) => match quoted {
            Some(quoted) => format!(
                "\"{quoted}\" is what the agent calls itself. {}",
                started_by(source)
            ),
            None if named => format!("{CLAIM_CAVEAT} {}", started_by(source)),
            None => started_by(source),
        },
        _ => match quoted {
            Some(quoted) => claimed_name_explanation(quoted),
            None if named => CLAIM_CAVEAT.to_owned(),
            None => String::new(),
        },
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
        .and_then(crate::install::catalog_agent)
        .map_or_else(|| source.image.clone(), |agent| agent.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{UNKNOWN_ID, UNKNOWN_NAME_SEPARATOR};

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

    /// Durable evidence of an MCP connection whose client claimed the catalog agent `claimed`,
    /// beside the `source` that started it.
    fn restored(claimed: &str, source: Option<Source>) -> Attribution {
        Attribution {
            claimed_agent: Some(claimed.to_owned()),
            ..started(IntakeChannel::Mcp, source)
        }
    }

    /// One row: the durable evidence of a connection, the ways its client's claim reaches the
    /// reader (live, with the name the client sent, or unkept, as history reads it back), the
    /// agent id they all name and the sentence, when the row asserts one.
    type Row<'a> = (Attribution, Vec<Claimed<'a>>, String, Option<String>);

    /// ACCEPTANCE Connection reporting: a connection's details name its agent once (its claim,
    /// else the app that started it, else Unknown agent) and say in one sentence what the
    /// connection, or its record read back from history, establishes. "in" only when the catalog
    /// knows the agent runs inside the source's app (118d5070), the caveat while the name is only
    /// a claim, and the client's own words quoted only where they say more than the agent's name;
    /// a name that is empty or blank names nobody (233e7569). A record keeps no client words, so
    /// its details quote none and report no application (80868c31), and it reads as its live row
    /// where that row quotes nothing. A record that kept neither a claim nor a source is the
    /// Earlier session, whose hover says no agent name was kept and makes no claim about when it
    /// was written (window-7, 2dfbe8d8), and command-line history from before connection
    /// identities says so. I-D4 (ADR-0106, amendment of 2026-10-21): a command-line session is
    /// named for the agent whose app runs it, whatever its client claims, and is the command line
    /// for a terminal.
    #[test]
    fn details_name_the_agent_once_and_say_only_what_the_connection_or_its_record_establishes() {
        let desktop = || source(Some("claude-desktop"), "claude.exe");
        let terminal = || source(None, "WindowsTerminal.exe");
        let cursor = || source(Some("cursor"), "Cursor.exe");
        let mcp = |from: Option<Source>| started(IntakeChannel::Mcp, from);
        let cli = |from: Option<Source>| started(IntakeChannel::Cli, from);
        let live = |claim: &'static str| vec![Claimed::Live(Some(claim))];
        let unknown_named = |name: &str| format!("{UNKNOWN_ID}{UNKNOWN_NAME_SEPARATOR}{name}");
        let unnamed = || {
            vec![
                Claimed::Live(None),
                Claimed::Live(Some("")),
                Claimed::Live(Some("   ")),
            ]
        };
        let rows: Vec<Row<'_>> = vec![
            (
                mcp(desktop()),
                live("claude-ai"),
                "claude-desktop".into(),
                Some("Started by Claude Desktop.".into()),
            ),
            (
                mcp(desktop()),
                live("claude-code"),
                "claude-code".into(),
                Some("Claude Code, in Claude Desktop.".into()),
            ),
            (
                restored("claude-code", desktop()),
                vec![Claimed::Live(Some("claude-code")), Claimed::Unkept],
                "claude-code".into(),
                Some("Claude Code, in Claude Desktop.".into()),
            ),
            (
                mcp(source(Some("vscode"), "Code.exe")),
                live("cline-vscode"),
                "cline".into(),
                Some("Cline, in Visual Studio Code.".into()),
            ),
            (
                mcp(desktop()),
                live("cline"),
                "cline".into(),
                Some(format!("{CLAIM_CAVEAT} Started by Claude Desktop.")),
            ),
            (
                mcp(cursor()),
                live("claude-code"),
                "claude-code".into(),
                Some(format!("{CLAIM_CAVEAT} Started by Cursor.")),
            ),
            (
                restored("claude-code", cursor()),
                vec![Claimed::Unkept],
                "claude-code".into(),
                Some(format!("{CLAIM_CAVEAT} Started by Cursor.")),
            ),
            (
                mcp(cursor()),
                live("my agent"),
                unknown_named("my agent"),
                Some(format!("{CLAIM_CAVEAT} Started by Cursor.")),
            ),
            (
                mcp(terminal()),
                live("codex-mcp-client"),
                "codex".into(),
                Some(
                    "\"codex-mcp-client\" is what the agent calls itself. Started by \
                     WindowsTerminal.exe."
                        .into(),
                ),
            ),
            (
                mcp(terminal()),
                live("Claude Code"),
                "claude-code".into(),
                Some(format!("{CLAIM_CAVEAT} Started by WindowsTerminal.exe.")),
            ),
            (
                restored("claude-code", terminal()),
                vec![Claimed::Unkept],
                "claude-code".into(),
                Some(format!("{CLAIM_CAVEAT} Started by WindowsTerminal.exe.")),
            ),
            (
                mcp(None),
                live("codex-mcp-client"),
                "codex".into(),
                Some(
                    "\"codex-mcp-client\" is what the agent calls itself; Ghostlight does not \
                     verify it."
                        .into(),
                ),
            ),
            (
                mcp(None),
                live("my agent"),
                unknown_named("my agent"),
                Some(CLAIM_CAVEAT.into()),
            ),
            (
                mcp(None),
                live("claude-code"),
                "claude-code".into(),
                Some(CLAIM_CAVEAT.into()),
            ),
            (
                restored("claude-code", None),
                vec![Claimed::Unkept],
                "claude-code".into(),
                Some(CLAIM_CAVEAT.into()),
            ),
            (
                mcp(None),
                live("Unknown agent"),
                Agent::unknown().id,
                Some(
                    "\"Unknown agent\" is what the agent calls itself; Ghostlight does not \
                     verify it."
                        .into(),
                ),
            ),
            (
                mcp(desktop()),
                live("codex-mcp-client"),
                "codex".into(),
                None,
            ),
            (
                mcp(desktop()),
                live("Command line"),
                Agent::unknown().id,
                None,
            ),
            (
                mcp(desktop()),
                unnamed(),
                "claude-desktop".into(),
                Some("Started by Claude Desktop.".into()),
            ),
            (
                restored("claude-desktop", desktop()),
                vec![Claimed::Unkept],
                "claude-desktop".into(),
                Some("Started by Claude Desktop.".into()),
            ),
            (
                mcp(desktop()),
                vec![Claimed::Unkept],
                "claude-desktop".into(),
                Some("Started by Claude Desktop.".into()),
            ),
            (
                mcp(source(None, "node.exe")),
                vec![Claimed::Unkept],
                Agent::unknown().id,
                Some("Started by node.exe.".into()),
            ),
            (
                mcp(None),
                unnamed(),
                Agent::unknown().id,
                Some(String::new()),
            ),
            (
                mcp(None),
                vec![Claimed::Unkept],
                Agent::earlier().id,
                Some(String::new()),
            ),
            (
                Attribution::legacy(IntakeChannel::Mcp),
                vec![Claimed::Unkept],
                Agent::earlier().id,
                Some(String::new()),
            ),
            (
                Attribution::legacy(IntakeChannel::Cli),
                vec![Claimed::Unkept],
                Agent::command_line().id,
                Some(LEGACY_EXPLANATION.into()),
            ),
            (
                cli(terminal()),
                live("ghostlight call"),
                Agent::command_line().id,
                Some(format!(
                    "A ghostlight call from WindowsTerminal.exe. {CLI_SCRIPTS_UNVERIFIED}"
                )),
            ),
            (
                cli(desktop()),
                live("codex"),
                "claude-desktop".into(),
                Some(format!(
                    "A ghostlight call from Claude Desktop. {CLI_SCRIPTS_UNVERIFIED}"
                )),
            ),
            (
                cli(source(None, "pwsh.exe")),
                vec![Claimed::Live(None)],
                Agent::command_line().id,
                Some(format!(
                    "A ghostlight call from pwsh.exe. {CLI_SCRIPTS_UNVERIFIED}"
                )),
            ),
            (
                cli(None),
                vec![Claimed::Unkept],
                Agent::command_line().id,
                Some(format!("{CLI_SESSION} {CLI_SCRIPTS_UNVERIFIED}")),
            ),
        ];
        for (attribution, claims, agent, explanation) in &rows {
            for &claim in claims {
                let said = details(attribution, claim);
                let row = format!("{attribution:?} {claim:?}");
                assert_eq!(&said.agent.id, agent, "{row}");
                if let Some(explanation) = explanation {
                    assert_eq!(&said.explanation, explanation, "{row}");
                }
                match claim {
                    Claimed::Live(name) => {
                        let kept = name.filter(|name| !name.trim().is_empty());
                        assert_eq!(said.reported_application.as_deref(), kept, "{row}");
                    }
                    Claimed::Unkept => {
                        assert_eq!(said.reported_application, None, "{row}");
                        assert!(!said.explanation.contains('"'), "{row}");
                    }
                }
            }
        }

        let unlisted = crate::provenance::ConnectionEvidence::new(
            Attribution::accepted(IntakeChannel::Mcp),
            Some("my agent"),
        );
        assert_eq!(unlisted.details().agent.name, "my agent");
        let earlier = unlisted.attribution().details().agent;
        assert_eq!(
            earlier,
            Agent::earlier(),
            "a new record whose name the catalog does not know, with no source, keeps no agent"
        );
        let note = earlier.note.unwrap_or_default();
        assert_eq!(note, EARLIER_SESSION_NOTE);
        assert!(
            !note.contains("before"),
            "the record does not show when it was written: {note}"
        );
    }

    /// ACCEPTANCE Connection reporting: only a catalog agent id a client's name names becomes the
    /// durable claimed agent; free text, a reserved name, no name and any claim on the command
    /// line are never kept (118d5070).
    #[test]
    fn only_a_catalog_claim_on_an_mcp_connection_is_kept_as_claimed() {
        assert_eq!(
            claimed_agent(Some(" Claude-Code "), IntakeChannel::Mcp).as_deref(),
            Some("claude-code")
        );
        assert_eq!(
            claimed_agent(Some("cline-cursor"), IntakeChannel::Mcp).as_deref(),
            Some("cline")
        );
        for free in [Some("my agent"), Some("Command line"), Some(""), None] {
            assert_eq!(claimed_agent(free, IntakeChannel::Mcp), None, "{free:?}");
        }
        assert_eq!(claimed_agent(Some("codex"), IntakeChannel::Cli), None);
    }
}
