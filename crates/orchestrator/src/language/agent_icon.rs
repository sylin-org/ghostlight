//! The words that say where an agent's icon came from: the Integrations pane's line under an
//! agent's own icon, and what `ghostlight doctor` reports for every agent.

use crate::install::icons::{AgentIcon, IconSourceKind};

/// The doctor's line when no agent's own icon was found.
pub const NO_ICON_LINE: &str = "Agent icons: none found -- every agent shows its monogram";

/// The line the Integrations pane shows under an agent's own icon, which came from the installed
/// app named `app`: the agent's own, or its family's.
#[must_use]
pub fn icon_from(app: &str) -> String {
    format!("Icon from the installed {app} app.")
}

/// The words a person reads for the kind of place an icon was read from.
#[must_use]
pub const fn source_words(kind: IconSourceKind) -> &'static str {
    match kind {
        IconSourceKind::Program => "Windows program",
        IconSourceKind::StoreApp => "Microsoft Store app",
        IconSourceKind::DesktopEntry => "desktop entry",
    }
}

/// The doctor's lines about the agents' icons, from what the service's cache holds: one line per
/// agent whose own icon was found, saying from which app and which kind of place, then one line
/// naming the agents that show their monograms.
#[must_use]
pub fn doctor_lines(agents: &[AgentIcon]) -> Vec<String> {
    let mut lines: Vec<String> = agents
        .iter()
        .filter_map(|agent| {
            let found = agent.found.as_ref()?;
            Some(format!(
                "Agent icon: {} -- from the installed {} app ({})",
                agent.name,
                found.app_name,
                source_words(found.found_in)
            ))
        })
        .collect();
    let monograms: Vec<&str> = agents
        .iter()
        .filter(|agent| agent.found.is_none())
        .map(|agent| agent.name)
        .collect();
    if lines.is_empty() {
        lines.push(NO_ICON_LINE.to_owned());
    } else if !monograms.is_empty() {
        lines.push(format!(
            "Agent icons: none found for {} -- they show their monograms",
            monograms.join(", ")
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::icons::FoundIcon;

    fn agent(agent_id: &'static str, name: &'static str, found: Option<FoundIcon>) -> AgentIcon {
        AgentIcon {
            agent_id,
            name,
            found,
        }
    }

    fn found(app_id: &'static str, app_name: &'static str, found_in: IconSourceKind) -> FoundIcon {
        FoundIcon {
            sha256: "0".repeat(64),
            app_id,
            app_name,
            found_in,
        }
    }

    #[test]
    fn the_pane_names_the_app_the_icon_came_from() {
        assert_eq!(
            icon_from("Antigravity"),
            "Icon from the installed Antigravity app."
        );
    }

    #[test]
    fn doctor_names_each_found_icon_s_app_and_the_agents_that_show_monograms() {
        let agents = [
            agent(
                "antigravity",
                "Antigravity",
                Some(found("antigravity", "Antigravity", IconSourceKind::Program)),
            ),
            agent(
                "claude-code",
                "Claude Code",
                Some(found(
                    "claude-desktop",
                    "Claude Desktop",
                    IconSourceKind::StoreApp,
                )),
            ),
            agent(
                "zed",
                "Zed",
                Some(found("zed", "Zed", IconSourceKind::DesktopEntry)),
            ),
            agent("cline", "Cline", None),
            agent("junie", "Junie", None),
        ];
        assert_eq!(
            doctor_lines(&agents),
            [
                "Agent icon: Antigravity -- from the installed Antigravity app (Windows program)",
                "Agent icon: Claude Code -- from the installed Claude Desktop app (Microsoft Store app)",
                "Agent icon: Zed -- from the installed Zed app (desktop entry)",
                "Agent icons: none found for Cline, Junie -- they show their monograms",
            ]
        );
        assert_eq!(
            doctor_lines(&agents[..1]),
            ["Agent icon: Antigravity -- from the installed Antigravity app (Windows program)"],
            "no monogram line when every agent has its icon"
        );
        assert_eq!(doctor_lines(&agents[3..]), [NO_ICON_LINE]);
    }
}
