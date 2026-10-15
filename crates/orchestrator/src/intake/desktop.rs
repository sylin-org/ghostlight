//! The same-build desktop contract: what the service and the Ghostlight shell say to each other
//! on a relay `desktop` stream (ADR-0199 Decision 7 and its amendment of 2026-10-15).
//!
//! The relay opening and the goodbye frame this stream rides on are frozen. The frames below are
//! not: the service serves the `desktop` role only to a shell of its own build, so the contract
//! may change with any build. The service sends the workbench address with the secret that lets
//! the window make changes, and optionally the tray the shell should draw. The shell reports a
//! click on a tray entry. The shell opens its own window, so an `open` click is only heard.
//!
//! The secret never leaves this stream: [`DesktopFrame`] prints it as redacted, and nothing on the
//! service side logs a desktop frame.

use std::fmt;

use ghostlight_relay::protocol::GoodbyeReason;
use serde::{Deserialize, Serialize};

use crate::language::workbench as words;

/// One frame on a `desktop` stream, in either direction.
///
/// A frame is one JSON object whose `kind` names it. A reader skips a kind it does not know.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesktopFrame {
    /// From the service, once, right after the opening is accepted: where the workbench is, the
    /// secret for its changes, and the tray to draw when the shell is to draw one.
    Workbench {
        /// The workbench address, ending in a slash.
        url: String,
        /// The secret that makes a request the window's. It goes to the shell's own page only.
        bearer: String,
        /// The tray the shell draws. `None` while the service has a tray of its own.
        tray: Option<TrayModel>,
    },
    /// From the service: it is about to close the stream. Byte for byte the goodbye of an `mcp`
    /// stream, so the service writes both with one code path.
    Goodbye {
        /// Why.
        reason: GoodbyeReason,
    },
    /// From the shell: the person chose an entry of the tray.
    Tray {
        /// The entry.
        item: TrayItemId,
    },
    /// A frame kind this build does not know. Readers skip it.
    #[serde(other)]
    Unknown,
}

impl DesktopFrame {
    /// The frame that hands the shell the workbench, with the full tray when the service has no
    /// tray of its own to show.
    #[must_use]
    pub fn workbench(url: &str, bearer: &str, native_tray: bool) -> Self {
        Self::Workbench {
            url: url.to_owned(),
            bearer: bearer.to_owned(),
            tray: (!native_tray).then(TrayModel::standard),
        }
    }
}

impl fmt::Debug for DesktopFrame {
    /// Prints the frame without the secret.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workbench { url, tray, .. } => formatter
                .debug_struct("Workbench")
                .field("url", url)
                .field("bearer", &"[redacted]")
                .field("tray", tray)
                .finish(),
            Self::Goodbye { reason } => formatter
                .debug_struct("Goodbye")
                .field("reason", reason)
                .finish(),
            Self::Tray { item } => formatter.debug_struct("Tray").field("item", item).finish(),
            Self::Unknown => formatter.write_str("Unknown"),
        }
    }
}

/// The entries of the tray, a closed set. The service decides what they say and whether they can
/// be chosen; the shell only draws them and reports a click.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayItemId {
    /// Bring the Ghostlight window forward.
    Open,
    /// Pause the agents' browser work.
    Hold,
    /// Let the agents' browser work continue.
    Resume,
    /// Quit Ghostlight.
    Quit,
}

impl TrayItemId {
    /// Every entry, in the order the tray lists them.
    pub const ALL: [Self; 4] = [Self::Open, Self::Hold, Self::Resume, Self::Quit];

    /// The entry's id as the tray menu and the wire carry it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Hold => "hold",
            Self::Resume => "resume",
            Self::Quit => "quit",
        }
    }

    /// The entry named `text`, or `None` for an id this build does not use.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|item| item.as_str() == text)
    }

    /// What the entry says.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => words::TRAY_OPEN,
            Self::Hold => words::TRAY_HOLD,
            Self::Resume => words::TRAY_RESUME,
            Self::Quit => words::TRAY_QUIT,
        }
    }
}

/// One entry of the tray as the shell draws it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrayItem {
    /// Which entry; the shell reports it back on a click.
    pub id: TrayItemId,
    /// What the entry says.
    pub label: String,
    /// Whether the entry can be chosen.
    pub enabled: bool,
}

/// The whole tray as the service describes it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrayModel {
    /// The hover text.
    pub tooltip: String,
    /// The entries in order.
    pub items: Vec<TrayItem>,
}

impl TrayModel {
    /// The tray with every entry, in the order of [`TrayItemId::ALL`], all enabled.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            tooltip: words::TRAY_TOOLTIP.to_owned(),
            items: TrayItemId::ALL
                .into_iter()
                .map(|id| TrayItem {
                    id,
                    label: id.label().to_owned(),
                    enabled: true,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use ghostlight_relay::protocol::McpFrame;
    use serde_json::json;

    use super::*;
    use crate::language::self_check::golden::{check_golden_with, golden_text};

    /// The environment variable that rewrites the desktop goldens instead of comparing them.
    const UPDATE_VARIABLE: &str = "GHOSTLIGHT_DESKTOP_GOLDEN_UPDATE";

    /// What the goldens describe.
    const SUBJECT: &str = "the desktop contract";

    /// The address the goldens carry.
    const GOLDEN_URL: &str = "http://127.0.0.1:41234/";

    /// The secret the goldens carry.
    const GOLDEN_BEARER: &str = "BEARER";

    fn golden(name: &str, frame: &DesktopFrame) {
        check_golden_with(
            &format!("tests/golden/desktop/{name}"),
            &golden_text(frame),
            UPDATE_VARIABLE,
            SUBJECT,
        );
    }

    #[test]
    fn the_frames_match_their_goldens() {
        golden(
            "workbench.json",
            &DesktopFrame::Workbench {
                url: GOLDEN_URL.to_owned(),
                bearer: GOLDEN_BEARER.to_owned(),
                tray: Some(TrayModel {
                    tooltip: words::TRAY_TOOLTIP.to_owned(),
                    items: [TrayItemId::Open, TrayItemId::Quit]
                        .into_iter()
                        .map(|id| TrayItem {
                            id,
                            label: id.label().to_owned(),
                            enabled: true,
                        })
                        .collect(),
                }),
            },
        );
        golden(
            "workbench-no-tray.json",
            &DesktopFrame::workbench(GOLDEN_URL, GOLDEN_BEARER, true),
        );
        golden(
            "goodbye.json",
            &DesktopFrame::Goodbye {
                reason: GoodbyeReason::Quit,
            },
        );
        golden(
            "tray.json",
            &DesktopFrame::Tray {
                item: TrayItemId::Hold,
            },
        );
    }

    #[test]
    fn the_standard_tray_lists_the_four_entries_with_their_words() {
        let tray = TrayModel::standard();
        assert_eq!(tray.tooltip, "Ghostlight");
        let listed: Vec<(&str, &str, bool)> = tray
            .items
            .iter()
            .map(|item| (item.id.as_str(), item.label.as_str(), item.enabled))
            .collect();
        assert_eq!(
            listed,
            [
                ("open", "Open Ghostlight", true),
                ("hold", "Pause browser work", true),
                ("resume", "Resume browser work", true),
                ("quit", "Quit Ghostlight", true),
            ]
        );
    }

    #[test]
    fn an_entry_id_is_its_wire_name() {
        for item in TrayItemId::ALL {
            assert_eq!(json!(item), json!(item.as_str()));
            assert_eq!(TrayItemId::parse(item.as_str()), Some(item));
        }
        assert_eq!(TrayItemId::parse("settings"), None);
    }

    #[test]
    fn the_goodbye_is_byte_for_byte_the_goodbye_of_an_mcp_stream() {
        for reason in [
            GoodbyeReason::Quit,
            GoodbyeReason::Replace,
            GoodbyeReason::Uninstall,
            GoodbyeReason::SessionEnd,
        ] {
            assert_eq!(
                serde_json::to_vec(&DesktopFrame::Goodbye { reason }).unwrap(),
                serde_json::to_vec(&McpFrame::Goodbye { reason }).unwrap(),
                "{reason:?}"
            );
        }
    }

    #[test]
    fn a_frame_kind_this_build_does_not_know_is_unknown() {
        let frame: DesktopFrame = serde_json::from_value(json!({"kind": "ping"})).unwrap();
        assert_eq!(frame, DesktopFrame::Unknown);
    }

    #[test]
    fn a_tray_click_on_an_entry_this_build_does_not_have_does_not_decode() {
        let frame = serde_json::from_value::<DesktopFrame>(json!({"kind": "tray", "item": "x"}));
        assert!(frame.is_err());
    }

    #[test]
    fn the_secret_is_not_printed() {
        let frame = DesktopFrame::workbench(GOLDEN_URL, "s3cr3t-bearer-value", false);
        let printed = format!("{frame:?} {frame:#?}");
        assert!(!printed.contains("s3cr3t-bearer-value"), "{printed}");
        assert!(printed.contains("[redacted]"));
        assert!(printed.contains(GOLDEN_URL));
    }
}
