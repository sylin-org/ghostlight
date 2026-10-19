//! The one place a connection's agent is named and its explanation authored, live and from
//! history alike ([`details`]): from the channel, the claim and the source Ghostlight saw start the
//! connection, through a closed relation between the agent and that source. Also the words the
//! window names an agent by when no roster row names it.

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
/// empty after trimming, and Ghostlight's own label for a client that sent no name
/// ([`crate::intake::mcp::DEFAULT_CLIENT_LABEL`]), name nobody.
#[must_use]
pub fn claimed_name(reported: &str) -> Option<&str> {
    let name = reported.trim();
    (!name.is_empty() && name != crate::intake::mcp::DEFAULT_CLIENT_LABEL).then_some(name)
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
}

/// The catalog id of the agent a connection's client claims to be when it `reported` a name on
/// `channel`: the roster agent an MCP client's name names, and none for a name the catalog does
/// not know, for a client that reported none, and on the command line, whose channel names its
/// agent. The attribution keeps it as a claim ([`Attribution::claimed_agent`]).
#[must_use]
pub fn claimed_agent(reported: &str, channel: IntakeChannel) -> Option<String> {
    match channel {
        IntakeChannel::Cli => None,
        IntakeChannel::Mcp => claimed_name(reported)
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
/// its `attribution` and the name its client `reported`, which only a live connection has.
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
pub fn details(attribution: &Attribution, reported: Option<&str>) -> ConnectionDetails {
    let live = reported.is_some();
    let reported = reported.and_then(claimed_name);
    let source = attribution.source.as_ref();
    let (agent, explanation) = match attribution.channel {
        IntakeChannel::Cli => (
            Agent::command_line(),
            source.map_or_else(
                || format!("{CLI_SESSION} {CLI_SCRIPTS_UNVERIFIED}"),
                command_line_from,
            ),
        ),
        IntakeChannel::Mcp => {
            let claim = claimed(attribution, reported);
            let named = claim.is_some();
            let agent = claim.unwrap_or_else(|| {
                source
                    .and_then(|source| source.agent.as_deref())
                    .and_then(crate::install::catalog_agent)
                    .unwrap_or_else(|| {
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
            details(&attribution, Some("claude-ai")).explanation,
            "Started by Claude Desktop."
        );
    }

    #[test]
    fn a_claimed_agent_inside_another_agent_s_app_says_both() {
        let attribution = started(
            IntakeChannel::Mcp,
            source(Some("claude-desktop"), "claude.exe"),
        );
        let said = details(&attribution, Some("claude-code"));
        assert_eq!(said.explanation, "Claude Code, in Claude Desktop.");
        assert_eq!(said.reported_application.as_deref(), Some("claude-code"));
    }

    #[test]
    fn a_claim_and_a_source_that_do_not_line_up_are_both_said_without_judging_either() {
        let by_program = started(IntakeChannel::Mcp, source(None, "WindowsTerminal.exe"));
        assert_eq!(
            details(&by_program, Some("codex-mcp-client")).explanation,
            "\"codex-mcp-client\" is what the agent calls itself. Started by WindowsTerminal.exe."
        );
        let unknown_name = started(IntakeChannel::Mcp, source(Some("cursor"), "Cursor.exe"));
        assert_eq!(
            details(&unknown_name, Some("my agent")).explanation,
            format!("{CLAIM_CAVEAT} Started by Cursor."),
            "a name the card already shows is not quoted again, and stays a claim"
        );
    }

    #[test]
    fn an_unknown_source_keeps_the_caveat() {
        let attribution = started(IntakeChannel::Mcp, None);
        assert_eq!(
            details(&attribution, Some("codex-mcp-client")).explanation,
            claimed_name_explanation("codex-mcp-client")
        );
        assert_eq!(
            details(&attribution, Some("my agent")).explanation,
            CLAIM_CAVEAT
        );
    }

    #[test]
    fn a_claim_that_reads_as_its_agent_s_name_is_not_said_again() {
        let unseen = started(IntakeChannel::Mcp, None);
        let said = details(&unseen, Some("claude-code"));
        assert_eq!(said.agent.name, "Claude Code");
        assert_eq!(said.explanation, CLAIM_CAVEAT);
        let by_program = started(IntakeChannel::Mcp, source(None, "WindowsTerminal.exe"));
        assert_eq!(
            details(&by_program, Some("Claude Code")).explanation,
            format!("{CLAIM_CAVEAT} Started by WindowsTerminal.exe.")
        );
        assert_eq!(
            details(&unseen, Some("Unknown agent")).explanation,
            claimed_name_explanation("Unknown agent"),
            "a reserved name names no agent, so it is still said"
        );
    }

    #[test]
    fn earlier_work_that_kept_no_agent_says_only_what_its_record_shows() {
        let unlisted = crate::provenance::ConnectionEvidence::new(
            Attribution::accepted(IntakeChannel::Mcp),
            "my agent",
        );
        assert_eq!(unlisted.details().agent.name, "my agent");
        let restored = unlisted.attribution().details();
        assert_eq!(
            restored.agent,
            Agent::earlier(),
            "a new record whose name the catalog does not know, with no source, keeps no agent"
        );
        assert_eq!(
            restored.agent,
            Attribution::legacy(IntakeChannel::Mcp).details().agent
        );
        let note = restored.agent.note.unwrap_or_default();
        assert_eq!(note, EARLIER_SESSION_NOTE);
        assert!(
            !note.contains("before"),
            "the record does not show when it was written: {note}"
        );
    }

    #[test]
    fn work_nothing_named_says_nothing_to_vouch_for() {
        let live = details(
            &started(IntakeChannel::Mcp, None),
            Some(crate::intake::mcp::DEFAULT_CLIENT_LABEL),
        );
        assert_eq!(live.agent, Agent::unknown());
        assert_eq!(live.explanation, "");
        let recorded = details(&started(IntakeChannel::Mcp, None), None);
        assert_eq!(recorded.agent, Agent::earlier());
        assert_eq!(recorded.explanation, "");
        let legacy = details(&Attribution::legacy(IntakeChannel::Mcp), None);
        assert_eq!(legacy.agent, Agent::earlier());
        assert_eq!(legacy.explanation, "");
        assert_eq!(
            details(&Attribution::legacy(IntakeChannel::Cli), None).explanation,
            LEGACY_EXPLANATION
        );
        let by_program = details(&started(IntakeChannel::Mcp, source(None, "node.exe")), None);
        assert_eq!(
            by_program.agent,
            Agent::unknown(),
            "a record with a source was not before names were kept"
        );
        assert_eq!(by_program.explanation, "Started by node.exe.");
    }

    #[test]
    fn history_and_the_command_line_say_what_started_the_connection() {
        let mcp = started(
            IntakeChannel::Mcp,
            source(Some("claude-desktop"), "claude.exe"),
        );
        let recorded = details(&mcp, None);
        assert_eq!(recorded.explanation, "Started by Claude Desktop.");
        assert_eq!(
            recorded.reported_application, None,
            "history keeps no claim"
        );
        let cli = started(IntakeChannel::Cli, source(None, "WindowsTerminal.exe"));
        assert_eq!(
            details(&cli, Some("ghostlight call")).explanation,
            "A ghostlight call from WindowsTerminal.exe. Ghostlight does not verify the scripts \
             and instructions it runs."
        );
        assert_eq!(
            details(&started(IntakeChannel::Cli, None), None).explanation,
            "A ghostlight call session is a local program. Ghostlight does not verify the scripts \
             and instructions it runs."
        );
    }

    #[test]
    fn a_claim_inside_its_known_host_says_both() {
        let desktop = started(
            IntakeChannel::Mcp,
            source(Some("claude-desktop"), "claude.exe"),
        );
        let code = details(&desktop, Some("claude-code"));
        assert_eq!(code.explanation, "Claude Code, in Claude Desktop.");
        assert_eq!(code.agent.id, "claude-code");
        let editor = started(IntakeChannel::Mcp, source(Some("vscode"), "Code.exe"));
        let cline = details(&editor, Some("cline-vscode"));
        assert_eq!(cline.explanation, "Cline, in Visual Studio Code.");
        assert_eq!(cline.agent.id, "cline");
    }

    #[test]
    fn a_known_claim_beside_another_agents_app_does_not_say_in() {
        let desktop = started(
            IntakeChannel::Mcp,
            source(Some("claude-desktop"), "claude.exe"),
        );
        let cline = details(&desktop, Some("cline"));
        assert_eq!(
            cline.explanation,
            format!("{CLAIM_CAVEAT} Started by Claude Desktop.")
        );
        assert_eq!(cline.agent.id, "cline");
        let editor = started(IntakeChannel::Mcp, source(Some("cursor"), "Cursor.exe"));
        assert_eq!(
            details(&editor, Some("claude-code")).explanation,
            format!("{CLAIM_CAVEAT} Started by Cursor."),
            "the catalog does not know Claude Code inside Cursor"
        );
    }

    #[test]
    fn a_live_claim_names_the_agent_before_its_source() {
        let desktop = started(
            IntakeChannel::Mcp,
            source(Some("claude-desktop"), "claude.exe"),
        );
        assert_eq!(
            details(&desktop, Some("codex-mcp-client")).agent.id,
            "codex"
        );
        assert_eq!(
            details(&desktop, Some("my agent")).agent.id,
            "unknown:my agent",
            "a name the catalog does not know still names the agent"
        );
        assert_eq!(
            details(&desktop, Some("Command line")).agent,
            Agent::unknown(),
            "a reserved name names the unknown agent, not the source's"
        );
        for unnamed in [
            None,
            Some(""),
            Some(crate::intake::mcp::DEFAULT_CLIENT_LABEL),
        ] {
            assert_eq!(
                details(&desktop, unnamed).agent.id,
                "claude-desktop",
                "{unnamed:?}"
            );
        }
        let cli = started(
            IntakeChannel::Cli,
            source(Some("claude-desktop"), "claude.exe"),
        );
        assert_eq!(details(&cli, Some("codex")).agent, Agent::command_line());
    }

    /// Durable evidence of an MCP connection whose client claimed the catalog agent `claimed`,
    /// beside the `source` that started it.
    fn restored(claimed: &str, source: Option<Source>) -> Attribution {
        Attribution {
            claimed_agent: Some(claimed.to_owned()),
            ..started(IntakeChannel::Mcp, source)
        }
    }

    #[test]
    fn a_restored_claim_names_its_agent_and_quotes_no_words_the_client_never_sent() {
        let apart = details(
            &restored("claude-code", source(Some("cursor"), "Cursor.exe")),
            None,
        );
        assert_eq!(apart.agent.id, "claude-code");
        assert_eq!(
            apart.explanation,
            format!("{CLAIM_CAVEAT} Started by Cursor.")
        );
        assert_eq!(apart.reported_application, None);
        let by_program = details(
            &restored("claude-code", source(None, "WindowsTerminal.exe")),
            None,
        );
        assert_eq!(
            by_program.explanation,
            format!("{CLAIM_CAVEAT} Started by WindowsTerminal.exe.")
        );
        let unseen = details(&restored("claude-code", None), None);
        assert_eq!(unseen.agent.id, "claude-code");
        assert_eq!(unseen.explanation, CLAIM_CAVEAT);
        for said in [&apart, &by_program, &unseen] {
            assert!(!said.explanation.contains('"'), "{}", said.explanation);
        }
    }

    #[test]
    fn a_restored_claim_reads_as_its_live_row_where_no_words_are_quoted() {
        let desktop = source(Some("claude-desktop"), "claude.exe");
        let inside = restored("claude-code", desktop.clone());
        assert_eq!(
            details(&inside, None).explanation,
            details(&inside, Some("claude-code")).explanation
        );
        assert_eq!(
            details(&inside, None).explanation,
            "Claude Code, in Claude Desktop."
        );
        let own = restored("claude-desktop", desktop);
        assert_eq!(
            details(&own, None).explanation,
            "Started by Claude Desktop."
        );
    }

    #[test]
    fn only_a_catalog_claim_on_an_mcp_connection_is_kept_as_claimed() {
        assert_eq!(
            claimed_agent(" Claude-Code ", IntakeChannel::Mcp).as_deref(),
            Some("claude-code")
        );
        assert_eq!(
            claimed_agent("cline-cursor", IntakeChannel::Mcp).as_deref(),
            Some("cline")
        );
        for free in [
            "my agent",
            "Command line",
            "",
            crate::intake::mcp::DEFAULT_CLIENT_LABEL,
        ] {
            assert_eq!(claimed_agent(free, IntakeChannel::Mcp), None, "{free:?}");
        }
        assert_eq!(claimed_agent("codex", IntakeChannel::Cli), None);
    }
}
