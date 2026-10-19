//! The same-build desktop contract: what the service and the Ghostlight shell say to each other
//! on a relay `desktop` stream (ADR-0199 Decision 7 and its amendment of 2026-10-15).
//!
//! The relay opening and the goodbye frame this stream rides on are frozen. The frames below are
//! not: the service serves the `desktop` role only to a shell of its own build, so the contract
//! may change with any build. The service sends the workbench address with the secret that lets
//! the window make changes, and the tray the shell draws: its icon state, its tooltip and its
//! entries. The service owns the tray model and sends a new one whenever it changes; the shell
//! only draws it. The service then asks the shell to show the window (at a section of the page),
//! to show a notice, and to carry out an act only this computer can do: open a link, open a
//! folder, pick a file. The shell reports a click on a tray entry, the result of each act, that it
//! cannot show a window (`unavailable`) and that the person closed the window (`closed`). The
//! shell opens its own window, so an `open` click is only heard. The service has one desktop
//! place: a shell that finds it taken is sent `already_open` in place of the workbench and
//! leaves, and the shell that holds the place is asked to show its window.
//!
//! The shell's opening says what it shows in its `channel` ([`ShellView`]): `window` while it
//! shows its window to a person or is about to, `tray` for the icon alone. The service believes
//! it, so `status` tells the truth after a restart, and a shell started for the tray alone that
//! finds the place taken brings no window forward.
//!
//! A reader skips a frame kind it does not know. The service ends a stream only for a frame that
//! does not decode.
//!
//! The secret never leaves this stream: [`DesktopFrame`] prints it as redacted, and nothing on the
//! service side logs a desktop frame.

use std::fmt;

use ghostlight_relay::protocol::GoodbyeReason;
use serde::{Deserialize, Serialize};

use crate::browser::adapter::{RuntimeControlIntent, RuntimeControlState};
use crate::governance::offers;
use crate::language::readiness::Readiness;
use crate::language::workbench::{self as words, ConnectionCounts};
use crate::workbench::PageAddress;

/// The longest tooltip the shell draws, in characters: the Windows limit. The tooltip is the
/// band's word for the state and the service line that counts connections, which stay inside it.
pub const MAX_TOOLTIP_LENGTH: usize = 127;

/// One frame on a `desktop` stream, in either direction.
///
/// A frame is one JSON object whose `kind` names it. A reader skips a kind it does not know.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesktopFrame {
    /// From the service, once, right after the opening is accepted: where the workbench is, the
    /// secret for its changes, and the tray to draw.
    Workbench {
        /// The workbench address, ending in a slash.
        url: String,
        /// The secret that makes a request the window's. It goes to the shell's own page only.
        bearer: String,
        /// The tray the shell draws.
        tray: TrayModel,
    },
    /// From the service: the tray changed. The shell draws this model in place of the last one.
    TrayModel {
        /// The whole tray as it is now.
        model: TrayModel,
    },
    /// From the service, once, in place of the workbench frame: another shell holds the desktop
    /// place. The stream ends right after, and the shell leaves. The shell that holds the place is
    /// asked to show its window.
    AlreadyOpen,
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
    /// From the service: bring the window forward, opening it first when it is not open.
    Show {
        /// Where in the page to open it. `None` leaves the window where it was.
        address: Option<PageAddress>,
    },
    /// From the service: show one notice. Delivery is best effort and no answer comes back.
    Notice {
        /// The notice's title, authored by Ghostlight.
        title: String,
        /// The notice's body, authored by Ghostlight. It names no site and no action.
        body: String,
    },
    /// From the service: carry out one act on this computer, then answer with an
    /// [`DesktopFrame::ActResult`] of the same `id`.
    Act {
        /// Names this act in its result.
        id: u64,
        /// What to do.
        act: NativeAct,
    },
    /// From the shell: how the act with this `id` ended.
    ActResult {
        /// The act it answers.
        id: u64,
        /// How it ended.
        outcome: ActOutcome,
    },
    /// From the shell: it cannot show what the service wants shown.
    Unavailable {
        /// What is missing.
        reason: UnavailableReason,
    },
    /// From the shell: the person closed the window.
    Closed {
        /// Whether the shell stays with its tray. When it does not, it leaves right after.
        stays: bool,
    },
    /// A frame kind this build does not know. Readers skip it.
    #[serde(other)]
    Unknown,
}

impl DesktopFrame {
    /// The frame that hands the shell the workbench and the tray to draw.
    #[must_use]
    pub fn workbench(url: &str, bearer: &str, tray: TrayModel) -> Self {
        Self::Workbench {
            url: url.to_owned(),
            bearer: bearer.to_owned(),
            tray,
        }
    }
}

/// What a shell shows, as the `channel` of its `desktop` opening says. A shell that says nothing,
/// as an older build does, is taken as it was before: by how it was started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellView {
    /// The shell shows its window to a person, or is about to: a person started it, or its window
    /// was open when it connected again.
    Window,
    /// The shell shows only the tray icon.
    Tray,
}

impl ShellView {
    /// The word in the opening's `channel`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Tray => "tray",
        }
    }

    /// The view an opening's `channel` names, or `None` for none or a word this build does not
    /// know.
    #[must_use]
    pub fn from_channel(channel: Option<&str>) -> Option<Self> {
        [Self::Window, Self::Tray]
            .into_iter()
            .find(|view| channel == Some(view.as_str()))
    }
}

/// What the shell cannot show, as it tells the service.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The session has no display.
    NoDisplay,
    /// The computer has no web view runtime, or the window could not be built.
    NoWebview,
    /// The desktop has no tray to draw the icon in.
    NoTray,
}

impl UnavailableReason {
    /// The reason the window is missing, as `status` says it.
    #[must_use]
    pub const fn window_reason(self) -> words::WindowReason {
        match self {
            Self::NoDisplay => words::WindowReason::NoDisplay,
            Self::NoWebview => words::WindowReason::NoWebview,
            Self::NoTray => words::WindowReason::NoTray,
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
            Self::TrayModel { model } => formatter
                .debug_struct("TrayModel")
                .field("model", model)
                .finish(),
            Self::AlreadyOpen => formatter.write_str("AlreadyOpen"),
            Self::Goodbye { reason } => formatter
                .debug_struct("Goodbye")
                .field("reason", reason)
                .finish(),
            Self::Tray { item } => formatter.debug_struct("Tray").field("item", item).finish(),
            Self::Show { address } => formatter
                .debug_struct("Show")
                .field("address", address)
                .finish(),
            Self::Notice { title, body } => formatter
                .debug_struct("Notice")
                .field("title", title)
                .field("body", body)
                .finish(),
            Self::Act { id, act } => formatter
                .debug_struct("Act")
                .field("id", id)
                .field("act", act)
                .finish(),
            Self::ActResult { id, outcome } => formatter
                .debug_struct("ActResult")
                .field("id", id)
                .field("outcome", outcome)
                .finish(),
            Self::Unavailable { reason } => formatter
                .debug_struct("Unavailable")
                .field("reason", reason)
                .finish(),
            Self::Closed { stays } => formatter
                .debug_struct("Closed")
                .field("stays", stays)
                .finish(),
            Self::Unknown => formatter.write_str("Unknown"),
        }
    }
}

/// What only this computer can do for the page, as the service asks the shell for it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeAct {
    /// Open a secure address in the person's browser.
    OpenUrl {
        /// The address. The service passes only `https://` addresses of its own choosing.
        url: String,
    },
    /// Show a folder in the file manager.
    OpenFolder {
        /// The folder.
        path: String,
    },
    /// Ask the person to choose a file.
    PickFile {
        /// The dialog's title.
        title: String,
    },
    /// An act this build does not know. A shell answers it as failed.
    #[serde(other)]
    Unknown,
}

/// How the shell's attempt at an act ended.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActOutcome {
    /// The link or the folder was opened.
    Done,
    /// The person chose a file.
    Picked {
        /// The file.
        path: String,
    },
    /// The person closed the dialog without choosing.
    Cancelled,
    /// This computer would not do it, or the session is ending.
    Failed,
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
    /// Let the agents' browser work continue: resume a pause, or start a new session after one
    /// ended.
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

    /// What the entry says while browser work is `runtime`: Resume starts a new session once the
    /// session ended, in the words the window and the browser toolbar use for it.
    #[must_use]
    pub const fn label(self, runtime: RuntimeControlState) -> &'static str {
        match (self, runtime) {
            (Self::Open, _) => words::TRAY_OPEN,
            (Self::Hold, _) => words::TRAY_HOLD,
            (Self::Resume, RuntimeControlState::Ended) => words::TRAY_START_SESSION,
            (Self::Resume, _) => words::TRAY_RESUME,
            (Self::Quit, _) => words::TRAY_QUIT,
        }
    }

    /// The runtime control a click on the entry asks for while browser work is `runtime`, as its
    /// label says ([`Self::label`]); none for Open and Quit, which control no browser work.
    #[must_use]
    pub const fn intent(self, runtime: RuntimeControlState) -> Option<RuntimeControlIntent> {
        match (self, runtime) {
            (Self::Hold, _) => Some(RuntimeControlIntent::Hold),
            (Self::Resume, RuntimeControlState::Ended) => Some(RuntimeControlIntent::StartSession),
            (Self::Resume, _) => Some(RuntimeControlIntent::Resume),
            (Self::Open | Self::Quit, _) => None,
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

/// The state the tray icon shows, the band's four states.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayState {
    /// Ghostlight is ready. The icon is the plain ghost.
    Ready,
    /// An agent is working in the browser.
    Working,
    /// Browser work is paused, the session ended, or policy turned off every agent's channel.
    Paused,
    /// Something needs the person's attention.
    Attention,
}

impl TrayState {
    /// The state the band shows for `readiness`, by its tone: quiet is ready, working is working,
    /// held is paused and attention is attention. With no browser connected the icon looks ready
    /// and the tooltip says so.
    #[must_use]
    pub const fn of(readiness: Readiness) -> Self {
        match readiness {
            Readiness::Ready | Readiness::NotConnected => Self::Ready,
            Readiness::Working => Self::Working,
            Readiness::Paused | Readiness::SessionEnded | Readiness::TurnedOff => Self::Paused,
            Readiness::PolicyInvalid | Readiness::HistoryUnavailable => Self::Attention,
        }
    }
}

/// The whole tray as the service describes it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrayModel {
    /// The hover text: the band's word for the state and the sentence that counts the service's
    /// connections.
    pub tooltip: String,
    /// The entries in order.
    pub items: Vec<TrayItem>,
    /// The state the icon shows.
    pub state: TrayState,
}

impl TrayModel {
    /// The tray for a service in `readiness` with `runtime` control and `counts` connected.
    ///
    /// Open and Quit can always be chosen. A runtime entry can be chosen only when the act it asks
    /// for is one [`offers`] names for `runtime`: Pause while browser work is active, Resume while
    /// it is paused, and Start a new session once the session ended.
    #[must_use]
    pub fn of(
        readiness: Readiness,
        runtime: RuntimeControlState,
        counts: ConnectionCounts,
    ) -> Self {
        Self {
            tooltip: words::tray_tooltip(readiness, counts),
            items: TrayItemId::ALL
                .into_iter()
                .map(|id| TrayItem {
                    id,
                    label: id.label(runtime).to_owned(),
                    enabled: match id.intent(runtime) {
                        Some(intent) => offers(runtime).contains(&intent),
                        None => true,
                    },
                })
                .collect(),
            state: TrayState::of(readiness),
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

    /// The link the goldens carry.
    const GOLDEN_LINK: &str = "https://example.com/docs";

    /// The folder the goldens carry.
    const GOLDEN_FOLDER: &str = "/home/person/.ghostlight/logs";

    /// The file the goldens carry.
    const GOLDEN_FILE: &str = "/home/person/Apps/client.json";

    fn golden(name: &str, frame: &DesktopFrame) {
        check_golden_with(
            &format!("tests/golden/desktop/{name}"),
            &golden_text(frame),
            UPDATE_VARIABLE,
            SUBJECT,
        );
    }

    /// The counts the goldens' tray describes.
    fn golden_counts() -> ConnectionCounts {
        ConnectionCounts {
            agents: 2,
            mcp_clients: 2,
            command_line: 0,
            browsers: 2,
        }
    }

    /// The tray the goldens carry: ready, with two MCP clients and two browsers.
    fn golden_tray() -> TrayModel {
        TrayModel::of(
            Readiness::Ready,
            RuntimeControlState::Active,
            golden_counts(),
        )
    }

    #[test]
    fn the_frames_match_their_goldens() {
        golden(
            "workbench.json",
            &DesktopFrame::workbench(GOLDEN_URL, GOLDEN_BEARER, golden_tray()),
        );
        golden(
            "tray-model.json",
            &DesktopFrame::TrayModel {
                model: TrayModel::of(
                    Readiness::Paused,
                    RuntimeControlState::Held,
                    golden_counts(),
                ),
            },
        );
        golden(
            "unavailable.json",
            &DesktopFrame::Unavailable {
                reason: UnavailableReason::NoTray,
            },
        );
        golden("closed.json", &DesktopFrame::Closed { stays: true });
        golden("already-open.json", &DesktopFrame::AlreadyOpen);
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
        golden(
            "tray-open.json",
            &DesktopFrame::Tray {
                item: TrayItemId::Open,
            },
        );
    }

    #[test]
    fn the_shows_notices_acts_and_results_match_their_goldens() {
        golden(
            "show.json",
            &DesktopFrame::Show {
                address: Some(PageAddress::SettingsDiagnostics),
            },
        );
        golden(
            "show-no-address.json",
            &DesktopFrame::Show { address: None },
        );
        golden(
            "notice.json",
            &DesktopFrame::Notice {
                title: words::NOTICE_CHECKUP_TITLE.to_owned(),
                body: words::NOTICE_CHECKUP_BODY.to_owned(),
            },
        );
        golden(
            "act-open-url.json",
            &DesktopFrame::Act {
                id: 1,
                act: NativeAct::OpenUrl {
                    url: GOLDEN_LINK.to_owned(),
                },
            },
        );
        golden(
            "act-open-folder.json",
            &DesktopFrame::Act {
                id: 2,
                act: NativeAct::OpenFolder {
                    path: GOLDEN_FOLDER.to_owned(),
                },
            },
        );
        golden(
            "act-pick-file.json",
            &DesktopFrame::Act {
                id: 3,
                act: NativeAct::PickFile {
                    title: words::LOCATE_DIALOG_TITLE.to_owned(),
                },
            },
        );
        golden(
            "act-result.json",
            &DesktopFrame::ActResult {
                id: 3,
                outcome: ActOutcome::Picked {
                    path: GOLDEN_FILE.to_owned(),
                },
            },
        );
        golden(
            "act-result-done.json",
            &DesktopFrame::ActResult {
                id: 1,
                outcome: ActOutcome::Done,
            },
        );
        golden(
            "act-result-cancelled.json",
            &DesktopFrame::ActResult {
                id: 3,
                outcome: ActOutcome::Cancelled,
            },
        );
        golden(
            "act-result-failed.json",
            &DesktopFrame::ActResult {
                id: 2,
                outcome: ActOutcome::Failed,
            },
        );
    }

    #[test]
    fn an_act_this_build_does_not_know_decodes_as_unknown() {
        let frame: DesktopFrame =
            serde_json::from_value(json!({"kind": "act", "id": 9, "act": {"kind": "print"}}))
                .unwrap();
        assert_eq!(
            frame,
            DesktopFrame::Act {
                id: 9,
                act: NativeAct::Unknown
            }
        );
    }

    #[test]
    fn a_show_for_a_section_that_is_not_one_does_not_decode() {
        let frame = serde_json::from_value::<DesktopFrame>(
            json!({"kind": "show", "address": "settings/nowhere"}),
        );
        assert!(frame.is_err());
    }

    /// The entries of `model` as (id, label, enabled).
    fn listed(model: &TrayModel) -> Vec<(&str, &str, bool)> {
        model
            .items
            .iter()
            .map(|item| (item.id.as_str(), item.label.as_str(), item.enabled))
            .collect()
    }

    #[test]
    fn an_active_tray_lists_the_four_entries_and_offers_pause_but_not_resume() {
        let tray = golden_tray();
        assert_eq!(
            tray.tooltip,
            "Ready. Ghostlight is running; 2 agents and 2 browsers connected."
        );
        assert_eq!(tray.state, TrayState::Ready);
        assert_eq!(
            listed(&tray),
            [
                ("open", "Open Ghostlight", true),
                ("hold", "Pause agents", true),
                ("resume", "Resume agents", false),
                ("quit", "Quit Ghostlight", true),
            ]
        );
    }

    #[test]
    fn a_paused_tray_offers_resume_but_not_pause() {
        let tray = TrayModel::of(
            Readiness::Paused,
            RuntimeControlState::Held,
            golden_counts(),
        );
        assert_eq!(tray.state, TrayState::Paused);
        assert_eq!(
            listed(&tray),
            [
                ("open", "Open Ghostlight", true),
                ("hold", "Pause agents", false),
                ("resume", "Resume agents", true),
                ("quit", "Quit Ghostlight", true),
            ]
        );
        assert_eq!(
            TrayItemId::Resume.intent(RuntimeControlState::Held),
            Some(RuntimeControlIntent::Resume)
        );
    }

    #[test]
    fn an_ended_session_offers_no_resume_in_the_tray() {
        let runtime = RuntimeControlState::Ended;
        let tray = TrayModel::of(Readiness::SessionEnded, runtime, golden_counts());
        assert_eq!(tray.state, TrayState::Paused);
        assert_eq!(
            listed(&tray),
            [
                ("open", "Open Ghostlight", true),
                ("hold", "Pause agents", false),
                ("resume", "Start a new session", true),
                ("quit", "Quit Ghostlight", true),
            ]
        );
        let asked: Vec<_> = tray
            .items
            .iter()
            .filter(|item| item.enabled)
            .filter_map(|item| item.id.intent(runtime))
            .collect();
        assert_eq!(
            asked,
            [RuntimeControlIntent::StartSession],
            "an ended session accepts only a new session; a Resume would leave it ended"
        );
    }

    #[test]
    fn a_runtime_entry_can_be_chosen_exactly_when_the_state_offers_its_act() {
        for runtime in [
            RuntimeControlState::Active,
            RuntimeControlState::Held,
            RuntimeControlState::Ended,
        ] {
            let tray = TrayModel::of(Readiness::Ready, runtime, golden_counts());
            for item in &tray.items {
                if let Some(intent) = item.id.intent(runtime) {
                    assert_eq!(
                        item.enabled,
                        offers(runtime).contains(&intent),
                        "{runtime:?} {intent:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn only_the_runtime_entries_ask_for_a_runtime_control() {
        for runtime in [
            RuntimeControlState::Active,
            RuntimeControlState::Held,
            RuntimeControlState::Ended,
        ] {
            assert_eq!(
                TrayItemId::Hold.intent(runtime),
                Some(RuntimeControlIntent::Hold)
            );
            assert_eq!(TrayItemId::Open.intent(runtime), None);
            assert_eq!(TrayItemId::Quit.intent(runtime), None);
        }
    }

    #[test]
    fn the_icon_state_follows_the_tone_of_the_readiness() {
        for readiness in Readiness::ALL {
            let expected = match readiness.tone() {
                "quiet" | "offline" => TrayState::Ready,
                "working" => TrayState::Working,
                "held" => TrayState::Paused,
                "attention" => TrayState::Attention,
                other => panic!("a tone the tray does not know: {other}"),
            };
            assert_eq!(TrayState::of(*readiness), expected, "{readiness:?}");
        }
    }

    #[test]
    fn the_longest_tooltip_fits_the_windows_limit() {
        // The most the service admits at once: its session capacity of command-line or MCP
        // sessions, and as many browsers or none, in every state.
        for browsers in [999, 0] {
            let counts = ConnectionCounts {
                agents: 1998,
                mcp_clients: 999,
                command_line: 999,
                browsers,
            };
            for readiness in Readiness::ALL {
                let tray = TrayModel::of(*readiness, RuntimeControlState::Active, counts);
                assert!(
                    tray.tooltip.chars().count() <= MAX_TOOLTIP_LENGTH,
                    "{} characters: {}",
                    tray.tooltip.chars().count(),
                    tray.tooltip
                );
            }
        }
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
        let frame = DesktopFrame::workbench(GOLDEN_URL, "s3cr3t-bearer-value", golden_tray());
        let printed = format!("{frame:?} {frame:#?}");
        assert!(!printed.contains("s3cr3t-bearer-value"), "{printed}");
        assert!(printed.contains("[redacted]"));
        assert!(printed.contains(GOLDEN_URL));
    }
}
