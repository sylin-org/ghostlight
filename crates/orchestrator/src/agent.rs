//! Who an agent is and what the window shows for it.
//!
//! The roster in [`crate::install`] is the catalog of agents Ghostlight can set up; the command
//! line and an unknown agent are the catalog's two reserved entries. Wherever the window names an
//! agent (a session, an operation and a history item from their connection evidence, an
//! Integrations row) it carries one [`Agent`], built here or from a roster row through
//! [`Agent::listed`], so every surface draws the same name and the same symbol. The page decides
//! nothing about an agent's looks: it draws the [`Symbol`] it is given.
//!
//! A roster agent's symbol is the app's own icon when the service has one cached for it
//! ([`crate::install::icons`]), served by the window under [`ICON_DIRECTORY`] with its version in
//! the address and with the sentence that says which app it came from, and otherwise the agent's
//! packaged monogram tile.
//!
//! An agent is projected for a person when a snapshot is built and is never persisted: the audit
//! keeps only the channel and the connection identity.

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::install::icons::FoundIcon;
use crate::intake::session::IntakeChannel;
use crate::language::agent_icon::icon_from;
use crate::language::provenance::{claimed_name, COMMAND_LINE_AGENT, UNKNOWN_AGENT};
use crate::provenance::ConnectionDetails;

/// The id of the command line, the catalog's reserved entry for `ghostlight call` sessions. No
/// roster row uses it.
pub const COMMAND_LINE_ID: &str = "command_line";

/// The id of an agent nothing names, the catalog's other reserved entry. No roster row uses it.
pub const UNKNOWN_ID: &str = "unknown";

/// What joins [`UNKNOWN_ID`] to the name an agent Ghostlight does not know reported, in that
/// agent's id. No roster id holds it.
pub const UNKNOWN_NAME_SEPARATOR: char = ':';

/// The directory beside the window's page that serves the packaged agent tiles.
pub const TILE_DIRECTORY: &str = "integrations";

/// The directory beside the window's page that serves the agents' own icons, by agent id.
pub const ICON_DIRECTORY: &str = "agent-icons";

/// The query key that carries a served icon's version, so a changed icon gets a new address.
const ICON_VERSION_KEY: &str = "v";

/// The key of a tile symbol: the path the window serves it at.
const SRC_KEY: &str = "src";

/// The key of an icon symbol: the path the window serves it at.
const ICON_KEY: &str = "icon";

/// The key of an icon symbol's sentence about where the icon came from.
const ABOUT_KEY: &str = "about";

/// The key of a glyph symbol: the glyph's word.
const GLYPH_KEY: &str = "glyph";

/// The names of the catalog's reserved entries, which no client's claim may take.
const RESERVED_NAMES: [&str; 2] = [COMMAND_LINE_AGENT, UNKNOWN_AGENT];

/// One agent as the window names and shows it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Agent {
    /// The catalog id, the one key that says two places name the same agent: a roster agent's id,
    /// shared by every host it runs on; [`COMMAND_LINE_ID`]; [`UNKNOWN_ID`] for an agent nothing
    /// names; or, for an agent Ghostlight does not know, [`UNKNOWN_ID`] and the name it reported
    /// joined by [`UNKNOWN_NAME_SEPARATOR`], so two such agents are one only when they report the
    /// same name.
    pub id: String,
    /// The name a person knows the agent by. An agent Ghostlight does not know keeps the name its
    /// client reported, which stays the client's own claim.
    pub name: String,
    /// What to show for the agent.
    pub symbol: Symbol,
}

/// What the window shows for an agent: a closed vocabulary the page draws as given.
///
/// It serializes as `{ "src": "<path>" }` for a packaged tile the window serves, as
/// `{ "icon": "<path>", "about": "<sentence>" }` for an app's own icon the window serves, or as
/// `{ "glyph": "<word>" }` for a glyph the page draws.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Symbol {
    /// A packaged tile the window serves, by its path beside the page, drawn as it is.
    Tile(String),
    /// An app's own icon the window serves, drawn whole on a neutral tile of the monogram's shape.
    Icon {
        /// Its path beside the page.
        src: String,
        /// The sentence that says which installed app it came from.
        about: String,
    },
    /// A glyph the page draws.
    Glyph(Glyph),
}

/// The glyphs the page draws for the catalog's reserved entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Glyph {
    /// The prompt of the command line.
    CommandLine,
    /// The mark of an agent Ghostlight does not know.
    Unknown,
}

impl Glyph {
    /// The glyph's word on the wire, the key the page draws it by.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::CommandLine => "command_line",
            Self::Unknown => "unknown",
        }
    }
}

impl Symbol {
    /// The packaged tile `file`, served under [`TILE_DIRECTORY`].
    #[must_use]
    pub fn tile(file: &str) -> Self {
        Self::Tile(format!("{TILE_DIRECTORY}/{file}"))
    }

    /// The cached icon of agent `id` that the cache `found`, served under [`ICON_DIRECTORY`] at
    /// its version, with the sentence that says which app it came from.
    #[must_use]
    pub fn icon(id: &str, found: &FoundIcon) -> Self {
        Self::Icon {
            src: format!("{ICON_DIRECTORY}/{id}?{ICON_VERSION_KEY}={}", found.version),
            about: icon_from(found.app_name),
        }
    }
}

impl Serialize for Symbol {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entries: Vec<(&str, &str)> = match self {
            Self::Tile(path) => vec![(SRC_KEY, path)],
            Self::Icon { src, about } => vec![(ICON_KEY, src), (ABOUT_KEY, about)],
            Self::Glyph(glyph) => vec![(GLYPH_KEY, glyph.word())],
        };
        let mut map = serializer.serialize_map(Some(entries.len()))?;
        for (key, value) in entries {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl Agent {
    /// A roster agent: its catalog id, its product name, and the app's own icon when the service
    /// has one cached for it, else its packaged monogram tile.
    #[must_use]
    pub fn listed(id: &str, name: &str, monogram: &str) -> Self {
        Self::shown(
            id,
            name,
            monogram,
            crate::install::icons::published(id).as_ref(),
        )
    }

    /// A roster agent wearing the icon the cache `found` for it, or its monogram without one.
    fn shown(id: &str, name: &str, monogram: &str, found: Option<&FoundIcon>) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            symbol: found.map_or_else(|| Symbol::tile(monogram), |found| Symbol::icon(id, found)),
        }
    }

    /// The command line, the agent of every `ghostlight call` session.
    #[must_use]
    pub fn command_line() -> Self {
        Self {
            id: COMMAND_LINE_ID.to_owned(),
            name: COMMAND_LINE_AGENT.to_owned(),
            symbol: Symbol::Glyph(Glyph::CommandLine),
        }
    }

    /// An agent nothing names: a client that reported no name, or a record that kept none.
    #[must_use]
    pub fn unknown() -> Self {
        Self {
            id: UNKNOWN_ID.to_owned(),
            name: UNKNOWN_AGENT.to_owned(),
            symbol: Symbol::Glyph(Glyph::Unknown),
        }
    }

    /// An agent Ghostlight does not know, under the `name` its client reported, with the unknown
    /// glyph.
    fn unknown_named(name: &str) -> Self {
        Self {
            id: format!("{UNKNOWN_ID}{UNKNOWN_NAME_SEPARATOR}{name}"),
            name: name.to_owned(),
            symbol: Symbol::Glyph(Glyph::Unknown),
        }
    }

    /// The agent of a connection on `channel` whose client `reported` a name for itself, if it
    /// is known: the command line for a `ghostlight call` session; the roster's agent for a
    /// name it knows; an agent Ghostlight does not know under the name it reported
    /// ([`claimed_name`]), with the unknown glyph; and the unknown agent when no name was
    /// reported, none was kept, or the client claimed a reserved entry's name. Ghostlight never
    /// invents a name.
    ///
    /// The name stays the client's own claim: it names the agent for a person and grants nothing.
    /// An MCP client that calls itself "Command line" or "Unknown agent" is not the command line
    /// and does not wear its glyph; its claim still shows in the connection's explanation.
    #[must_use]
    pub fn of(reported: Option<&str>, channel: IntakeChannel) -> Self {
        match channel {
            IntakeChannel::Cli => Self::command_line(),
            IntakeChannel::Mcp => match reported.and_then(claimed_name) {
                None => Self::unknown(),
                Some(name) if takes_a_reserved_name(name) => Self::unknown(),
                Some(name) => {
                    crate::install::agent_called(name).unwrap_or_else(|| Self::unknown_named(name))
                }
            },
        }
    }

    /// The agent of the work `evidence` describes: [`Self::of`] its bounded claim and its channel,
    /// so live evidence names the agent and durable evidence names only the command line. Work
    /// that kept no evidence is the unknown agent's.
    #[must_use]
    pub fn of_evidence(evidence: Option<&ConnectionDetails>) -> Self {
        evidence.map_or_else(Self::unknown, |evidence| {
            Self::of(evidence.reported_application.as_deref(), evidence.channel)
        })
    }
}

/// Whether `name` is a reserved entry's name, ignoring case and space.
fn takes_a_reserved_name(name: &str) -> bool {
    let squeezed = |text: &str| -> String {
        text.chars()
            .filter(|character| !character.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let name = squeezed(name);
    RESERVED_NAMES
        .iter()
        .any(|reserved| squeezed(reserved) == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symbol_serializes_as_what_to_show() {
        assert_eq!(
            serde_json::to_value(Symbol::tile("codex.svg")).unwrap(),
            serde_json::json!({ "src": "integrations/codex.svg" })
        );
        assert_eq!(
            serde_json::to_value(Symbol::Glyph(Glyph::CommandLine)).unwrap(),
            serde_json::json!({ "glyph": "command_line" })
        );
        assert_eq!(
            serde_json::to_value(Symbol::Glyph(Glyph::Unknown)).unwrap(),
            serde_json::json!({ "glyph": "unknown" })
        );
    }

    #[test]
    fn a_roster_agent_wears_its_cached_icon_and_otherwise_its_monogram() {
        use crate::install::icons::IconSourceKind;

        let found = |app_id, app_name| FoundIcon {
            version: "0123456789ab".into(),
            app_id,
            app_name,
            found_in: IconSourceKind::DesktopEntry,
        };
        assert_eq!(
            Agent::shown("zed", "Zed", "zed.svg", Some(&found("zed", "Zed"))).symbol,
            Symbol::Icon {
                src: "agent-icons/zed?v=0123456789ab".into(),
                about: "Icon from the installed Zed app.".into(),
            }
        );
        assert_eq!(
            serde_json::to_value(Agent::shown(
                "claude-code",
                "Claude Code",
                "claude-code.svg",
                Some(&found("claude-desktop", "Claude Desktop")),
            ))
            .unwrap()["symbol"],
            serde_json::json!({
                "icon": "agent-icons/claude-code?v=0123456789ab",
                "about": "Icon from the installed Claude Desktop app.",
            }),
            "an icon its family's app gave names that app"
        );
        assert_eq!(
            Agent::shown("zed", "Zed", "zed.svg", None).symbol,
            Symbol::tile("zed.svg")
        );
        assert_eq!(
            Agent::listed("zed", "Zed", "zed.svg").symbol,
            Symbol::tile("zed.svg"),
            "no test publishes a cache, so every agent keeps its monogram"
        );
    }

    #[test]
    fn the_page_draws_every_glyph_the_service_names() {
        let words = include_str!("../ui/lib/words.js");
        let vocabulary = words
            .lines()
            .find(|line| line.contains("const GLYPH = "))
            .expect("the page names the glyphs it draws");
        for glyph in [Glyph::CommandLine, Glyph::Unknown] {
            assert!(
                vocabulary.contains(&format!(": \"{}\"", glyph.word())),
                "the page does not draw {}",
                glyph.word()
            );
        }
    }

    #[test]
    fn the_command_line_and_an_unknown_agent_get_their_glyphs() {
        for reported in [None, Some("ghostlight call"), Some("codex-mcp-client")] {
            let agent = Agent::of(reported, IntakeChannel::Cli);
            assert_eq!(agent, Agent::command_line(), "{reported:?}");
            assert_eq!(agent.name, "Command line");
            assert_eq!(agent.symbol, Symbol::Glyph(Glyph::CommandLine));
        }
        for unnamed in [
            None,
            Some(""),
            Some("   "),
            Some(crate::intake::mcp::DEFAULT_CLIENT_LABEL),
        ] {
            let agent = Agent::of(unnamed, IntakeChannel::Mcp);
            assert_eq!(agent, Agent::unknown(), "{unnamed:?}");
            assert_eq!(agent.name, "Unknown agent");
        }
    }

    #[test]
    fn a_client_that_claims_a_reserved_name_is_the_unknown_agent() {
        use crate::provenance::ConnectionEvidence;

        for claimed in [
            "Command line",
            "command line",
            "  COMMAND   LINE ",
            "CommandLine",
            "Unknown agent",
            "unknown agent",
            " UnknownAgent ",
        ] {
            let agent = Agent::of(Some(claimed), IntakeChannel::Mcp);
            assert_eq!(agent, Agent::unknown(), "{claimed:?}");
            assert_ne!(
                agent.symbol,
                Symbol::Glyph(Glyph::CommandLine),
                "{claimed:?}"
            );
            let evidence = ConnectionEvidence::new(claimed, IntakeChannel::Mcp).details();
            assert_eq!(Agent::of_evidence(Some(&evidence)), Agent::unknown());
            assert!(
                evidence
                    .explanation
                    .contains(&format!("\"{}\"", claimed.trim())),
                "the claim still shows in the trust note: {}",
                evidence.explanation
            );
        }
        assert_eq!(
            Agent::of(Some("Command line"), IntakeChannel::Cli),
            Agent::command_line(),
            "only the command line's own channel names it"
        );
    }

    #[test]
    fn work_is_its_evidence_s_agent_and_work_without_evidence_the_unknown_one() {
        use crate::provenance::{Attribution, ConnectionEvidence};

        let live = ConnectionEvidence::new("codex-mcp-client", IntakeChannel::Mcp);
        assert_eq!(Agent::of_evidence(Some(&live.details())).id, "codex");
        assert_eq!(
            Agent::of_evidence(Some(&live.attribution().details())),
            Agent::unknown(),
            "durable evidence keeps no claimed name"
        );
        assert_eq!(
            Agent::of_evidence(Some(&Attribution::legacy(IntakeChannel::Cli).details())),
            Agent::command_line()
        );
        assert_eq!(Agent::of_evidence(None), Agent::unknown());
    }

    #[test]
    fn a_known_agent_is_its_roster_row_and_an_unknown_one_keeps_the_name_it_reported() {
        let codex = Agent::of(Some("codex-mcp-client"), IntakeChannel::Mcp);
        assert_eq!(
            codex,
            Agent {
                id: "codex".into(),
                name: "Codex".into(),
                symbol: Symbol::Tile("integrations/codex.svg".into()),
            }
        );
        assert_eq!(
            Agent::of(Some(" Claude-Code "), IntakeChannel::Mcp).id,
            "claude-code"
        );
        assert_eq!(
            Agent::of(Some("  Some Agent  "), IntakeChannel::Mcp),
            Agent {
                id: "unknown:Some Agent".into(),
                name: "Some Agent".into(),
                symbol: Symbol::Glyph(Glyph::Unknown),
            }
        );
        assert_ne!(
            Agent::of(Some("Some Agent"), IntakeChannel::Mcp).id,
            Agent::of(Some("Other Agent"), IntakeChannel::Mcp).id,
            "two agents Ghostlight does not know stay two"
        );
    }

    #[test]
    fn every_host_of_one_agent_names_one_agent() {
        let cline = Agent::of(Some("cline"), IntakeChannel::Mcp);
        assert_eq!(cline.id, "cline");
        assert_eq!(cline.name, "Cline");
        for host in [
            "cline-cli",
            "cline-vscode",
            "cline-cursor",
            "cline-windsurf",
        ] {
            assert_eq!(Agent::of(Some(host), IntakeChannel::Mcp), cline, "{host}");
        }
    }
}
