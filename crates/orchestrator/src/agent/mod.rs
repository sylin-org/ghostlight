//! Who an agent is and what the window shows for it.
//!
//! The roster in [`crate::install`] is the catalog of agents Ghostlight can set up; the command
//! line and an unknown agent are the catalog's two reserved entries. Wherever the window names an
//! agent (a session, an operation and a history item, an Integrations row) it carries one
//! [`Agent`], built here or from a roster row through [`Agent::listed`], so every surface draws the
//! same name and the same symbol. The agent of work is named once, from its connection evidence,
//! by [`crate::language::provenance::details`]. The page decides
//! nothing about an agent's looks: it draws the [`Symbol`] it is given.
//!
//! A roster agent's symbol is one address, the agent's under [`ICON_DIRECTORY`], with the version
//! of the image the window serves there ([`image`]): the app's own icon when the service has one
//! cached for it, with the sentence that says which app it came from, and otherwise the agent's
//! packaged monogram tile. The page fetches every address the same way and decides nothing.
//!
//! An agent is projected for a person when a snapshot is built and is never persisted: the audit
//! keeps the connection's evidence (its channel, the claimed catalog agent and the source), from
//! which the agent is named again.

pub mod image;

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use self::image::{AgentImage, Monogram};
use crate::install::icons::IconCache;
use crate::intake::session::IntakeChannel;
use crate::language::provenance::{claimed_name, COMMAND_LINE_AGENT, UNKNOWN_AGENT};

/// The id of the command line, the catalog's reserved entry for `ghostlight call` sessions. No
/// roster row uses it.
pub const COMMAND_LINE_ID: &str = "command_line";

/// The id of an agent nothing names, the catalog's other reserved entry. No roster row uses it.
pub const UNKNOWN_ID: &str = "unknown";

/// What joins [`UNKNOWN_ID`] to the name an agent Ghostlight does not know reported, in that
/// agent's id. No roster id holds it.
pub const UNKNOWN_NAME_SEPARATOR: char = ':';

/// The directory beside the window's page that serves each catalog agent's image, by agent id.
pub const ICON_DIRECTORY: &str = "agent-icons";

/// The query key that carries a served image's version, so a changed image gets a new address.
const ICON_VERSION_KEY: &str = "v";

/// The key of an image symbol: the address the window serves the agent's image at.
const ICON_KEY: &str = "icon";

/// The key of an image symbol's sentence about which installed app the image came from.
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
/// It serializes as `{ "icon": "<address>" }` for a catalog agent's image the window serves, with
/// `"about": "<sentence>"` beside it when that image is an installed app's own icon, or as
/// `{ "glyph": "<word>" }` for a glyph the page draws.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Symbol {
    /// A catalog agent's image, which the window serves at the address and the page draws whole.
    Icon {
        /// The agent's address beside the page, with the served image's version.
        address: String,
        /// The sentence the Integrations pane shows for an installed app's own icon, saying which
        /// app it came from; none for a monogram. Every image served at the address is a whole
        /// tile, so the page draws an app icon and a monogram the same way.
        about: Option<String>,
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
    /// The image of the catalog agent `id`: its address under [`ICON_DIRECTORY`] at the version
    /// of the `image` the window serves there, and the sentence an app's own icon carries.
    #[must_use]
    pub fn image(id: &str, image: &AgentImage) -> Self {
        Self::Icon {
            address: format!(
                "{ICON_DIRECTORY}/{id}?{ICON_VERSION_KEY}={}",
                image.version()
            ),
            about: image.about(),
        }
    }
}

impl Serialize for Symbol {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let entries: Vec<(&str, &str)> = match self {
            Self::Icon { address, about } => std::iter::once((ICON_KEY, address.as_str()))
                .chain(about.as_deref().map(|about| (ABOUT_KEY, about)))
                .collect(),
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
    /// A roster agent: its catalog id, its product name, and the image the window serves for it
    /// from the cache the service published: the app's own icon when one is cached, else its
    /// packaged `monogram`.
    #[must_use]
    pub fn listed(id: &str, name: &str, monogram: Monogram) -> Self {
        Self::shown(id, name, monogram, crate::install::icons::published())
    }

    /// A roster agent wearing the best image `cache` gives it ([`AgentImage::best`]).
    fn shown(id: &str, name: &str, monogram: Monogram, cache: Option<&IconCache>) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            symbol: Symbol::image(id, &AgentImage::best(id, monogram, cache)),
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

    /// The address the window serves a monogram at, for the agent `id` whose tile is `monogram`.
    fn monogram_address(id: &str, monogram: Monogram) -> String {
        format!(
            "agent-icons/{id}?v={}",
            AgentImage::Monogram(monogram).version()
        )
    }

    #[test]
    fn a_symbol_serializes_as_what_to_show() {
        assert_eq!(
            serde_json::to_value(Symbol::Icon {
                address: "agent-icons/codex?v=0123456789ab".into(),
                about: None,
            })
            .unwrap(),
            serde_json::json!({ "icon": "agent-icons/codex?v=0123456789ab" })
        );
        assert_eq!(
            serde_json::to_value(Symbol::Icon {
                address: "agent-icons/zed?v=0123456789ab".into(),
                about: Some("Icon from the installed Zed app.".into()),
            })
            .unwrap(),
            serde_json::json!({
                "icon": "agent-icons/zed?v=0123456789ab",
                "about": "Icon from the installed Zed app.",
            })
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
        use crate::install::icons::{FoundIcon, IconSourceKind};
        use std::sync::Arc;

        let found = |app_id, app_name| AgentImage::AppIcon {
            found: FoundIcon {
                sha256: "0".repeat(64),
                app_id,
                app_name,
                found_in: IconSourceKind::DesktopEntry,
            },
            image: Arc::new(image::ServedImage {
                media_type: "image/png",
                bytes: Vec::new(),
                version: "0123456789ab".into(),
            }),
        };
        assert_eq!(
            Symbol::image("zed", &found("zed", "Zed")),
            Symbol::Icon {
                address: "agent-icons/zed?v=0123456789ab".into(),
                about: Some("Icon from the installed Zed app.".into()),
            }
        );
        assert_eq!(
            serde_json::to_value(Symbol::image(
                "claude-code",
                &found("claude-desktop", "Claude Desktop"),
            ))
            .unwrap(),
            serde_json::json!({
                "icon": "agent-icons/claude-code?v=0123456789ab",
                "about": "Icon from the installed Claude Desktop app.",
            }),
            "an icon its family's app gave names that app"
        );
        let zed = image::monogram!("zed.svg");
        let monogram = Symbol::Icon {
            address: monogram_address("zed", zed),
            about: None,
        };
        assert_eq!(Agent::shown("zed", "Zed", zed, None).symbol, monogram);
        assert_eq!(
            serde_json::to_value(&monogram).unwrap(),
            serde_json::json!({ "icon": monogram_address("zed", zed) }),
            "a monogram says nothing about where it came from"
        );
        assert_eq!(
            Agent::listed("zed", "Zed", zed).symbol,
            monogram,
            "no test publishes a cache, so every agent keeps its monogram"
        );
    }

    #[test]
    fn the_page_draws_every_glyph_the_service_names() {
        let words = include_str!("../../ui/lib/words.js");
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
            let evidence = ConnectionEvidence::claimed(claimed, IntakeChannel::Mcp).details();
            assert_eq!(evidence.agent, Agent::unknown());
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
    fn a_known_agent_is_its_roster_row_and_an_unknown_one_keeps_the_name_it_reported() {
        let codex = Agent::of(Some("codex-mcp-client"), IntakeChannel::Mcp);
        assert_eq!(
            codex,
            Agent {
                id: "codex".into(),
                name: "Codex".into(),
                symbol: Symbol::Icon {
                    address: monogram_address("codex", image::monogram!("codex.svg")),
                    about: None,
                },
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
