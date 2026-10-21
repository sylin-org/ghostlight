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
    /// From the service: it is about to close the stream. The service writes this frame to the
    /// shell itself; an `mcp` stream gets the relay's own goodbye frame, written separately.
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
    /// The computer has no web view runtime, as the shell finds before it opens.
    NoWebview,
    /// The runtime is there, but the window could not be built.
    WindowFailed,
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
            Self::WindowFailed => words::WindowReason::WindowFailed,
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
    /// The tray while the window cannot be built: the tooltip says so and the rest is as the
    /// service describes it.
    #[must_use]
    pub fn while_window_fails(&self) -> Self {
        Self {
            tooltip: words::TRAY_WINDOW_FAILS.to_owned(),
            ..self.clone()
        }
    }

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
    use super::*;
    use crate::language::self_check::golden::{check_golden_with, golden_text};

    /// The environment variable that rewrites the desktop goldens instead of comparing them.
    const UPDATE_VARIABLE: &str = "GHOSTLIGHT_DESKTOP_GOLDEN_UPDATE";

    /// What the goldens describe.
    const SUBJECT: &str = "the desktop contract";

    /// The port of the workbench address the goldens carry, written the way the service writes it.
    const GOLDEN_PORT: u16 = 41234;

    /// The secret the goldens carry, in the shape the service mints: 43 URL-safe base64
    /// characters, which the shell's own check reads in its golden test.
    const GOLDEN_BEARER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNO-_";

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

    /// The longest tray the service can draw, which the shell's golden test checks against its own
    /// limits: the longest tooltip any readiness gives at the service's session capacity, with as
    /// many browsers or none, and the labels of an ended session, which offer Start a new session.
    fn longest_tray() -> TrayModel {
        [999, 0]
            .into_iter()
            .flat_map(|browsers| {
                Readiness::ALL.iter().map(move |readiness| {
                    let counts = ConnectionCounts {
                        agents: 1998,
                        mcp_clients: 999,
                        command_line: 999,
                        browsers,
                    };
                    TrayModel::of(*readiness, RuntimeControlState::Ended, counts)
                })
            })
            .max_by_key(|tray| tray.tooltip.chars().count())
            .expect("a readiness")
    }

    /// The tray the goldens carry: ready, with two MCP clients and two browsers.
    fn golden_tray() -> TrayModel {
        TrayModel::of(
            Readiness::Ready,
            RuntimeControlState::Active,
            golden_counts(),
        )
    }

    /// Contract: every frame of the desktop stream matches its golden in tests/golden/desktop/,
    /// the bytes the shell decodes and checks in its own tests (crates/desktop/src/link.rs): what
    /// the service sends (the workbench at the address it serves, with its bearer and tray, a new
    /// tray, the longest tray it can draw, already open, goodbye, show with and without a
    /// section, a notice, each act) and what the shell sends (a tray click, unavailable, closed,
    /// each act result).
    #[test]
    fn every_desktop_frame_matches_the_golden_the_shell_reads() {
        let address = crate::workbench::web::WorkbenchAddress::new(GOLDEN_PORT, true);
        let frames = [
            (
                "workbench.json",
                DesktopFrame::workbench(address.url(), GOLDEN_BEARER, golden_tray()),
            ),
            (
                "tray-model-longest.json",
                DesktopFrame::TrayModel {
                    model: longest_tray(),
                },
            ),
            (
                "tray-model.json",
                DesktopFrame::TrayModel {
                    model: TrayModel::of(
                        Readiness::Paused,
                        RuntimeControlState::Held,
                        golden_counts(),
                    ),
                },
            ),
            (
                "unavailable.json",
                DesktopFrame::Unavailable {
                    reason: UnavailableReason::NoTray,
                },
            ),
            (
                "unavailable-window-failed.json",
                DesktopFrame::Unavailable {
                    reason: UnavailableReason::WindowFailed,
                },
            ),
            (
                "tray-model-window-fails.json",
                DesktopFrame::TrayModel {
                    model: golden_tray().while_window_fails(),
                },
            ),
            ("closed.json", DesktopFrame::Closed { stays: true }),
            ("already-open.json", DesktopFrame::AlreadyOpen),
            (
                "goodbye.json",
                DesktopFrame::Goodbye {
                    reason: GoodbyeReason::Quit,
                },
            ),
            (
                "tray.json",
                DesktopFrame::Tray {
                    item: TrayItemId::Hold,
                },
            ),
            (
                "tray-open.json",
                DesktopFrame::Tray {
                    item: TrayItemId::Open,
                },
            ),
            (
                "show.json",
                DesktopFrame::Show {
                    address: Some(PageAddress::SettingsDiagnostics),
                },
            ),
            ("show-no-address.json", DesktopFrame::Show { address: None }),
            (
                "notice.json",
                DesktopFrame::Notice {
                    title: words::NOTICE_CHECKUP_TITLE.to_owned(),
                    body: words::NOTICE_CHECKUP_BODY.to_owned(),
                },
            ),
            (
                "act-open-url.json",
                DesktopFrame::Act {
                    id: 1,
                    act: NativeAct::OpenUrl {
                        url: GOLDEN_LINK.to_owned(),
                    },
                },
            ),
            (
                "act-open-folder.json",
                DesktopFrame::Act {
                    id: 2,
                    act: NativeAct::OpenFolder {
                        path: GOLDEN_FOLDER.to_owned(),
                    },
                },
            ),
            (
                "act-pick-file.json",
                DesktopFrame::Act {
                    id: 3,
                    act: NativeAct::PickFile {
                        title: words::LOCATE_DIALOG_TITLE.to_owned(),
                    },
                },
            ),
            (
                "act-result.json",
                DesktopFrame::ActResult {
                    id: 3,
                    outcome: ActOutcome::Picked {
                        path: GOLDEN_FILE.to_owned(),
                    },
                },
            ),
            (
                "act-result-done.json",
                DesktopFrame::ActResult {
                    id: 1,
                    outcome: ActOutcome::Done,
                },
            ),
            (
                "act-result-cancelled.json",
                DesktopFrame::ActResult {
                    id: 3,
                    outcome: ActOutcome::Cancelled,
                },
            ),
            (
                "act-result-failed.json",
                DesktopFrame::ActResult {
                    id: 2,
                    outcome: ActOutcome::Failed,
                },
            ),
        ];
        for (name, frame) in &frames {
            golden(name, frame);
        }
    }

    /// Defect A-01 (37554eab, ADR-0203 Decisions 12.1 and 7.9) and ACCEPTANCE X6 and D19: the
    /// tray offers exactly the runtime act its state accepts, labelled with that act's words:
    /// Pause while browser work is active, Resume while it is paused, and once the session ended
    /// only Start a new session, since a Resume would leave it ended. Open and Quit can always be
    /// chosen.
    #[test]
    fn the_tray_offers_exactly_the_runtime_act_its_state_accepts() {
        for (readiness, runtime, act, label) in [
            (
                Readiness::Ready,
                RuntimeControlState::Active,
                RuntimeControlIntent::Hold,
                words::TRAY_HOLD,
            ),
            (
                Readiness::Paused,
                RuntimeControlState::Held,
                RuntimeControlIntent::Resume,
                words::TRAY_RESUME,
            ),
            (
                Readiness::SessionEnded,
                RuntimeControlState::Ended,
                RuntimeControlIntent::StartSession,
                words::TRAY_START_SESSION,
            ),
        ] {
            let tray = TrayModel::of(readiness, runtime, golden_counts());
            let offered: Vec<_> = tray
                .items
                .iter()
                .filter(|item| item.enabled)
                .filter_map(|item| {
                    item.id
                        .intent(runtime)
                        .map(|intent| (intent, item.label.as_str()))
                })
                .collect();
            assert_eq!(offered, [(act, label)], "{runtime:?}");
            for always in [TrayItemId::Open, TrayItemId::Quit] {
                assert!(
                    tray.items
                        .iter()
                        .any(|item| item.id == always && item.enabled),
                    "{always:?} while {runtime:?}"
                );
            }
        }
    }
}
