//! The shell's stream to the service: the `desktop` role of relay protocol 4 (ADR-0199
//! Decision 7 and its amendment of 2026-10-15).
//!
//! [`open_link`] connects to the service of a Ghostlight home, opens it as the desktop shell and
//! reads the one frame that matters: where the workbench is, the bearer that makes its page the
//! window's and the tray to draw, or `already_open` when another shell holds the service's one
//! desktop place. After that the stream carries a goodbye, a new tray, a request to show the
//! window, a notice and an act to carry out from the service, and from the shell a click on a tray
//! entry, the result of each act, that it cannot show a window and that the person closed the
//! window. A frame this build cannot read is skipped, so a newer service's frame never ends an
//! older shell during a hand-over.
//!
//! The frames are the service's `intake::desktop` contract, written again here because the shell
//! depends on no workspace crate but the relay. `tests/golden/desktop/` holds the bytes both
//! sides read. The bearer never leaves this process except into the workbench page: it is not
//! printed by any `Debug`, and no error text names a frame's content.

use std::fmt;
use std::io::{self, Read};
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::endpoint::{ConnectError, Connection};
use ghostlight_relay::frame::{self, FrameError};
use ghostlight_relay::protocol::{self, Edge, GoodbyeReason, Opening, Program, RefusalCode, Role};
use serde::{Deserialize, Serialize};

use crate::tray::TrayModel;

/// How long the shell waits for the service to accept it and send the workbench.
pub const OPEN_DEADLINE: Duration = Duration::from_secs(5);

/// The start of the only address the workbench is served at.
const WORKBENCH_URL_PREFIX: &str = "http://127.0.0.1:";

/// The length of the bearer, in characters: 32 random bytes in URL-safe base64 with no padding.
const BEARER_LENGTH: usize = 43;

/// The name of the thread that reads the stream.
const READER_THREAD_NAME: &str = "ghostlight-desktop-link";

/// What a redacted secret prints as.
const REDACTED: &str = "[redacted]";

/// One frame on a `desktop` stream, in either direction.
///
/// A frame is one JSON object whose `kind` names it. A reader skips a kind it does not know.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DesktopFrame {
    /// From the service, once, right after the opening is accepted.
    Workbench {
        /// The workbench address.
        url: String,
        /// The secret that makes a request the window's.
        bearer: String,
        /// The tray to draw.
        tray: TrayModel,
    },
    /// From the service: the tray changed. It replaces the one drawn.
    TrayModel {
        /// The whole tray as it is now.
        model: TrayModel,
    },
    /// From the service, once, in place of the workbench: another shell holds the desktop place.
    /// The stream ends right after and this shell leaves.
    AlreadyOpen,
    /// From the service: it is about to close the stream.
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
        /// Where in the page to open it. The service has checked it against the page's list.
        address: Option<String>,
    },
    /// From the service: show one notice, best effort.
    Notice {
        /// The notice's title.
        title: String,
        /// The notice's body.
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

/// What the shell cannot show, as it tells the service.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The session has no display.
    NoDisplay,
    /// The computer has no web view runtime, or the window could not be built.
    NoWebview,
    /// The desktop has no tray to draw the icon in.
    NoTray,
}

/// What the service asks of this computer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeAct {
    /// Open a secure address in the person's browser.
    OpenUrl {
        /// The address.
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
    /// An act this build does not know. It is answered as failed.
    #[serde(other)]
    Unknown,
}

/// How an act ended.
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

impl fmt::Debug for DesktopFrame {
    /// Prints the frame without the bearer.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workbench { url, tray, .. } => formatter
                .debug_struct("Workbench")
                .field("url", url)
                .field("bearer", &REDACTED)
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

/// The entries of the tray, a closed set the service defines.
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
    /// Every entry.
    pub const ALL: [Self; 4] = [Self::Open, Self::Hold, Self::Resume, Self::Quit];

    /// The entry's id as the tray menu and the wire carry it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Hold => "hold",
            Self::Resume => "resume",
            Self::Quit => "quit",
        }
    }

    /// The entry named `text`, or `None` for an id this build does not use.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|item| item.as_str() == text)
    }
}

/// What the service handed the shell: the workbench address, the bearer for its page and the
/// tray to draw.
#[derive(Clone, Eq, PartialEq)]
pub struct Workbench {
    /// The workbench address: `http://127.0.0.1:<port>/`.
    pub url: String,
    /// The secret that makes a request the window's. It goes to the workbench page only.
    pub bearer: String,
    /// The tray to draw.
    pub tray: TrayModel,
}

impl Workbench {
    /// The workbench origin: the address without its trailing slash.
    pub fn origin(&self) -> &str {
        self.url.strip_suffix('/').unwrap_or(&self.url)
    }
}

impl fmt::Debug for Workbench {
    /// Prints the workbench without the bearer.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Workbench")
            .field("url", &self.url)
            .field("bearer", &REDACTED)
            .field("tray", &self.tray)
            .finish()
    }
}

/// Why the shell has no link to the service.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    /// The service could not be reached.
    #[error("the Ghostlight service is not reachable: {0}")]
    Connect(#[from] ConnectError),
    /// The service refused the shell.
    #[error("{say}")]
    Refused {
        /// Why, when the service gave a code this build knows.
        code: Option<RefusalCode>,
        /// The service's own sentence.
        say: String,
    },
    /// Another shell holds the service's desktop place.
    #[error("another Ghostlight window is already open")]
    AlreadyOpen,
    /// The service sent something the shell cannot use.
    #[error("the service did not hand over the workbench: {reason}")]
    Contract {
        /// Which rule the first frame broke.
        reason: &'static str,
    },
    /// The stream failed.
    #[error("the link to the service failed: {0}")]
    Io(#[from] io::Error),
}

/// The reason for a first frame that is not a workbench frame.
const NOT_A_WORKBENCH: &str = "the first frame is not a workbench frame";

/// The reason for a first frame the shell cannot read at all.
const UNREADABLE_FRAME: &str = "the first frame cannot be read";

/// The reason for a workbench address other than the loopback address.
const BAD_ADDRESS: &str = "the address is not a loopback workbench address";

/// The reason for a bearer of the wrong shape.
const BAD_BEARER: &str = "the bearer is not the expected length or alphabet";

/// The port of a workbench address, or why the address is not one.
///
/// The address is exactly `http://127.0.0.1:<port>/` with a port from 1 to 65535 written in
/// plain digits: no other host name, scheme, path, query or user information.
pub fn validate_workbench_url(url: &str) -> Result<u16, LinkError> {
    let bad = || LinkError::Contract {
        reason: BAD_ADDRESS,
    };
    let digits = url
        .strip_prefix(WORKBENCH_URL_PREFIX)
        .and_then(|rest| rest.strip_suffix('/'))
        .ok_or_else(bad)?;
    match digits.parse::<u16>() {
        Ok(port) if port != 0 && port.to_string() == digits => Ok(port),
        _ => Err(bad()),
    }
}

/// Whether `bearer` has the shape the service makes: 43 URL-safe base64 characters.
fn well_formed_bearer(bearer: &str) -> bool {
    bearer.len() == BEARER_LENGTH
        && bearer
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// How a stream ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkEnd {
    /// The service said goodbye, for this reason.
    Goodbye(GoodbyeReason),
    /// The stream ended or failed with no goodbye.
    Lost,
}

/// What the shell shows, said in the `channel` of its opening. The service believes it: `status`
/// reads it, and a shell started for the tray alone that finds the place taken brings no window
/// forward. The words are the service's `ShellView`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    /// The shell shows its window to a person, or is about to.
    Window,
    /// The shell shows only the tray icon.
    Tray,
}

impl View {
    /// The word in the opening's `channel`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Tray => "tray",
        }
    }
}

/// The shell's end of the stream, after the workbench was handed over.
pub struct Link {
    connection: Connection,
}

/// Connects to the service of `home`, opens it as the desktop shell showing `view` and reads the
/// workbench.
///
/// A refused opening is [`LinkError::Refused`] with the service's own words. A first frame that is
/// not a workbench frame, an address that is not `http://127.0.0.1:<port>/` or a bearer of the
/// wrong shape is [`LinkError::Contract`].
pub fn open_link(home: &std::path::Path, view: View) -> Result<(Link, Workbench), LinkError> {
    let deadline = Instant::now() + OPEN_DEADLINE;
    let mut opening = Opening::new(
        Role::Desktop,
        Edge::new(Program::Desktop, env!("CARGO_PKG_VERSION").to_owned()),
    );
    opening.channel = Some(view.as_str().to_owned());
    let (connection, reply) = protocol::connect(home, &opening, deadline)?;
    if !reply.accepted {
        return Err(LinkError::Refused {
            code: reply.code,
            say: reply.say.unwrap_or_default(),
        });
    }
    connection.set_read_timeout(Some(
        deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1)),
    ))?;
    let mut reader = &connection;
    let first = frame::read_frame::<DesktopFrame>(&mut reader);
    connection.set_read_timeout(None)?;
    let workbench = match first {
        Ok(Some(DesktopFrame::Workbench { url, bearer, tray })) => Workbench { url, bearer, tray },
        Ok(Some(DesktopFrame::AlreadyOpen)) => return Err(LinkError::AlreadyOpen),
        Ok(Some(_)) | Ok(None) => {
            return Err(LinkError::Contract {
                reason: NOT_A_WORKBENCH,
            })
        }
        Err(FrameError::Io(error)) => return Err(LinkError::Io(error)),
        Err(_) => {
            return Err(LinkError::Contract {
                reason: UNREADABLE_FRAME,
            })
        }
    };
    validate_workbench_url(&workbench.url)?;
    if !well_formed_bearer(&workbench.bearer) {
        return Err(LinkError::Contract { reason: BAD_BEARER });
    }
    Ok((Link { connection }, workbench))
}

impl Link {
    /// Reads frames on a thread of its own. It hands `on_frame` each frame the service sends that
    /// the shell acts on, and calls `on_end` once, with how the stream ended: a goodbye, or the
    /// end or failure of the stream. A frame this build does not know, and one it cannot read, is
    /// skipped.
    pub fn spawn_reader(
        &self,
        on_frame: impl FnMut(DesktopFrame) + Send + 'static,
        on_end: impl FnOnce(LinkEnd) + Send + 'static,
    ) -> Result<(), LinkError> {
        let mut reader = self.connection.try_clone()?;
        thread::Builder::new()
            .name(READER_THREAD_NAME.to_owned())
            .spawn(move || {
                let end = read_until_end(&mut reader, on_frame);
                on_end(end);
            })?;
        Ok(())
    }

    /// Tells the service the person chose `item` in the tray.
    pub fn send_tray(&self, item: TrayItemId) -> Result<(), LinkError> {
        self.send(&DesktopFrame::Tray { item })
    }

    /// Tells the service that the shell cannot show what it wants shown.
    pub fn send_unavailable(&self, reason: UnavailableReason) -> Result<(), LinkError> {
        self.send(&DesktopFrame::Unavailable { reason })
    }

    /// Tells the service that the person closed the window, and whether the shell stays.
    pub fn send_closed(&self, stays: bool) -> Result<(), LinkError> {
        self.send(&DesktopFrame::Closed { stays })
    }

    /// Tells the service how the act with this `id` ended.
    pub fn send_act_result(&self, id: u64, outcome: ActOutcome) -> Result<(), LinkError> {
        self.send(&DesktopFrame::ActResult { id, outcome })
    }

    fn send(&self, frame: &DesktopFrame) -> Result<(), LinkError> {
        let mut writer = self.connection.try_clone()?;
        frame::write_frame(&mut writer, frame).map_err(|error| match error {
            FrameError::Io(error) => LinkError::Io(error),
            other => LinkError::Io(io::Error::other(other)),
        })
    }
}

/// Reads frames until a goodbye, the end of the stream or a failure of the stream itself, hands
/// each frame the shell acts on to `on_frame`, and says how the stream ended.
///
/// A frame of a kind this build does not know, and one whose JSON this build cannot decode, are
/// skipped. A frame the service sends in the wrong direction (a tray click, an act result, a
/// workbench) is skipped as well.
fn read_until_end(reader: &mut impl Read, mut on_frame: impl FnMut(DesktopFrame)) -> LinkEnd {
    loop {
        match frame::read_frame::<DesktopFrame>(reader) {
            Ok(Some(DesktopFrame::Goodbye { reason })) => return LinkEnd::Goodbye(reason),
            Ok(None) => return LinkEnd::Lost,
            Ok(Some(
                request @ (DesktopFrame::TrayModel { .. }
                | DesktopFrame::Show { .. }
                | DesktopFrame::Notice { .. }
                | DesktopFrame::Act { .. }),
            )) => on_frame(request),
            Ok(Some(_)) | Err(FrameError::Json(_)) => {}
            Err(_) => return LinkEnd::Lost,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;
    use std::sync::mpsc;

    use ghostlight_relay::endpoint::Listener;
    use ghostlight_relay::protocol::{RefusalCode, Reply};

    use super::*;
    use crate::scratch::Scratch;
    use crate::tray::TrayState;

    const URL: &str = "http://127.0.0.1:41234/";

    /// A tray of the right shape.
    fn tray() -> TrayModel {
        TrayModel {
            tooltip: "Ghostlight service is up; nothing is connected yet.".to_owned(),
            items: Vec::new(),
            state: TrayState::Ready,
        }
    }

    /// A bearer of the right shape.
    const BEARER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNO-_";

    fn golden_text(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/desktop")
            .join(name);
        std::fs::read_to_string(&path).expect("the golden is readable")
    }

    fn golden(name: &str) -> DesktopFrame {
        serde_json::from_str(&golden_text(name)).expect("the golden decodes")
    }

    #[test]
    fn each_golden_decodes_to_its_frame() {
        let DesktopFrame::Workbench { url, bearer, tray } = golden("workbench.json") else {
            panic!("workbench.json is a workbench frame");
        };
        assert_eq!((url.as_str(), bearer.as_str()), (URL, "BEARER"));
        assert_eq!(tray.validate(), Ok(()));
        assert_eq!(tray.state, TrayState::Ready);
        assert_eq!(
            tray.tooltip,
            "Ghostlight service is up; 2 MCP clients and 2 browsers connected."
        );
        let ids: Vec<(&str, bool)> = tray
            .items
            .iter()
            .map(|item| (item.id.as_str(), item.enabled))
            .collect();
        assert_eq!(
            ids,
            [
                ("open", true),
                ("hold", true),
                ("resume", false),
                ("quit", true)
            ]
        );
        let DesktopFrame::TrayModel { model } = golden("tray-model.json") else {
            panic!("tray-model.json is a tray model frame");
        };
        assert_eq!(model.validate(), Ok(()));
        assert_eq!(model.state, TrayState::Paused);
        let enabled: Vec<&str> = model
            .items
            .iter()
            .filter(|item| item.enabled)
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(enabled, ["open", "resume", "quit"]);
        assert_eq!(golden("already-open.json"), DesktopFrame::AlreadyOpen);
        assert_eq!(
            golden("goodbye.json"),
            DesktopFrame::Goodbye {
                reason: GoodbyeReason::Quit
            }
        );
        assert_eq!(
            golden("tray.json"),
            DesktopFrame::Tray {
                item: TrayItemId::Hold
            }
        );
        assert_eq!(
            golden("tray-open.json"),
            DesktopFrame::Tray {
                item: TrayItemId::Open
            }
        );
    }

    #[test]
    fn each_request_golden_decodes_to_its_frame() {
        assert_eq!(
            golden("show.json"),
            DesktopFrame::Show {
                address: Some("settings/diagnostics".to_owned())
            }
        );
        assert_eq!(
            golden("show-no-address.json"),
            DesktopFrame::Show { address: None }
        );
        assert_eq!(
            golden("notice.json"),
            DesktopFrame::Notice {
                title: "Ghostlight checkup".to_owned(),
                body: "Notifications are ready.".to_owned(),
            }
        );
        assert_eq!(
            golden("act-open-url.json"),
            DesktopFrame::Act {
                id: 1,
                act: NativeAct::OpenUrl {
                    url: "https://example.com/docs".to_owned()
                }
            }
        );
        assert_eq!(
            golden("act-open-folder.json"),
            DesktopFrame::Act {
                id: 2,
                act: NativeAct::OpenFolder {
                    path: "/home/person/.ghostlight/logs".to_owned()
                }
            }
        );
        assert_eq!(
            golden("act-pick-file.json"),
            DesktopFrame::Act {
                id: 3,
                act: NativeAct::PickFile {
                    title: "Locate the harness application or settings file".to_owned()
                }
            }
        );
    }

    #[test]
    fn the_shell_writes_the_bytes_of_the_act_result_goldens() {
        let results = [
            (
                "act-result.json",
                3,
                ActOutcome::Picked {
                    path: "/home/person/Apps/client.json".to_owned(),
                },
            ),
            ("act-result-done.json", 1, ActOutcome::Done),
            ("act-result-cancelled.json", 3, ActOutcome::Cancelled),
            ("act-result-failed.json", 2, ActOutcome::Failed),
        ];
        for (name, id, outcome) in results {
            let result = DesktopFrame::ActResult { id, outcome };
            assert_eq!(golden(name), result, "{name} decodes to the result");
            assert_eq!(
                serde_json::to_value(&result).unwrap(),
                serde_json::from_str::<serde_json::Value>(&golden_text(name)).unwrap(),
                "{name} is the bytes the shell writes"
            );
        }
    }

    #[test]
    fn the_shell_writes_the_bytes_of_the_unavailable_and_closed_goldens() {
        for (name, frame) in [
            (
                "unavailable.json",
                DesktopFrame::Unavailable {
                    reason: UnavailableReason::NoTray,
                },
            ),
            ("closed.json", DesktopFrame::Closed { stays: true }),
        ] {
            assert_eq!(golden(name), frame, "{name} decodes to the frame");
            assert_eq!(
                serde_json::to_value(&frame).unwrap(),
                serde_json::from_str::<serde_json::Value>(&golden_text(name)).unwrap(),
                "{name} is the bytes the shell writes"
            );
        }
    }

    #[test]
    fn every_reason_the_shell_gives_has_its_wire_word() {
        for (reason, word) in [
            (UnavailableReason::NoDisplay, "no_display"),
            (UnavailableReason::NoWebview, "no_webview"),
            (UnavailableReason::NoTray, "no_tray"),
        ] {
            assert_eq!(serde_json::to_value(reason).unwrap(), word);
        }
    }

    #[test]
    fn a_frame_kind_this_build_does_not_know_is_unknown() {
        let frame: DesktopFrame = serde_json::from_str(r#"{"kind":"ping"}"#).unwrap();
        assert_eq!(frame, DesktopFrame::Unknown);
    }

    #[test]
    fn the_shell_writes_the_bytes_of_the_tray_goldens() {
        for (name, item) in [
            ("tray.json", TrayItemId::Hold),
            ("tray-open.json", TrayItemId::Open),
        ] {
            let written = serde_json::to_value(DesktopFrame::Tray { item }).unwrap();
            let golden: serde_json::Value = serde_json::from_str(&golden_text(name)).unwrap();
            assert_eq!(written, golden, "{name}");
        }
    }

    #[test]
    fn the_bearer_is_never_printed() {
        let secret = "s3cr3t-bearer-value";
        let frame = DesktopFrame::Workbench {
            url: URL.to_owned(),
            bearer: secret.to_owned(),
            tray: tray(),
        };
        let workbench = Workbench {
            url: URL.to_owned(),
            bearer: secret.to_owned(),
            tray: tray(),
        };
        let errors = [
            LinkError::Refused {
                code: None,
                say: "no".to_owned(),
            },
            LinkError::AlreadyOpen,
            LinkError::Contract { reason: BAD_BEARER },
            LinkError::Connect(ConnectError::NoService),
        ];
        let printed = format!("{frame:?} {frame:#?} {workbench:?} {workbench:#?} {errors:?}");
        assert!(!printed.contains(secret), "{printed}");
        assert!(printed.contains(REDACTED));
        for error in errors {
            assert!(!error.to_string().contains(secret));
        }
    }

    #[test]
    fn the_workbench_address_is_exactly_the_loopback_form() {
        assert_eq!(validate_workbench_url(URL).ok(), Some(41234));
        assert_eq!(
            validate_workbench_url("http://127.0.0.1:65535/").ok(),
            Some(65535)
        );
        for url in [
            "https://127.0.0.1:41234/",
            "http://localhost:41234/",
            "http://127.0.0.1:41234/x",
            "http://127.0.0.1:41234",
            "http://127.0.0.1:0/",
            "http://127.0.0.1:65536/",
            "http://127.0.0.1:+80/",
            "http://127.0.0.1:041234/",
            "http://127.0.0.1:/",
            "http://127.0.0.1:41234/?x=1",
            "http://user@127.0.0.1:41234/",
            "",
        ] {
            assert!(
                matches!(validate_workbench_url(url), Err(LinkError::Contract { .. })),
                "{url}"
            );
        }
    }

    #[test]
    fn the_origin_is_the_address_without_its_slash() {
        let workbench = Workbench {
            url: URL.to_owned(),
            bearer: BEARER.to_owned(),
            tray: tray(),
        };
        assert_eq!(workbench.origin(), "http://127.0.0.1:41234");
    }

    #[test]
    fn the_tray_entries_round_trip_through_their_ids() {
        for item in TrayItemId::ALL {
            assert_eq!(TrayItemId::parse(item.as_str()), Some(item));
            assert_eq!(
                serde_json::to_value(item).unwrap(),
                serde_json::Value::from(item.as_str())
            );
        }
        assert_eq!(TrayItemId::parse("settings"), None);
    }

    #[test]
    fn a_bearer_has_43_url_safe_characters() {
        assert!(well_formed_bearer(BEARER));
        assert!(!well_formed_bearer(&BEARER[1..]));
        assert!(!well_formed_bearer(&format!("{BEARER}A")));
        assert!(!well_formed_bearer(&BEARER.replace('-', "+")));
        assert!(!well_formed_bearer(&BEARER.replace('_', "\"")));
    }

    #[test]
    fn the_reader_stops_at_a_goodbye_of_any_reason_and_at_the_end() {
        for reason in [GoodbyeReason::Quit, GoodbyeReason::Unknown] {
            let mut bytes = Vec::new();
            for frame in [
                DesktopFrame::Unknown,
                DesktopFrame::Goodbye { reason },
                DesktopFrame::Tray {
                    item: TrayItemId::Quit,
                },
            ] {
                frame::write_frame(&mut bytes, &frame).unwrap();
            }
            let mut cursor = Cursor::new(bytes);
            let end = read_until_end(&mut cursor, |_| {
                panic!("nothing before the goodbye is a request")
            });
            assert_eq!(end, LinkEnd::Goodbye(reason));
            let rest = frame::read_frame::<DesktopFrame>(&mut cursor).unwrap();
            assert!(rest.is_some(), "the frame after the goodbye is left unread");
        }
        assert_eq!(
            read_until_end(&mut Cursor::new(Vec::new()), |_| panic!("an empty stream")),
            LinkEnd::Lost
        );
        assert_eq!(
            read_until_end(&mut Cursor::new(vec![9, 0, 0, 0, 1]), |_| {
                panic!("a truncated frame")
            }),
            LinkEnd::Lost
        );
    }

    /// Reads `bytes` to the end and gives back the requests the reader handed on.
    fn requests_in(bytes: Vec<u8>) -> Vec<DesktopFrame> {
        let mut handed = Vec::new();
        read_until_end(&mut Cursor::new(bytes), |frame| handed.push(frame));
        handed
    }

    #[test]
    fn the_reader_hands_on_shows_notices_and_acts_in_order() {
        let frames = [
            DesktopFrame::Show {
                address: Some("settings/sharing".to_owned()),
            },
            DesktopFrame::Notice {
                title: "T".to_owned(),
                body: "B".to_owned(),
            },
            DesktopFrame::Act {
                id: 4,
                act: NativeAct::PickFile {
                    title: "Choose".to_owned(),
                },
            },
        ];
        let mut bytes = Vec::new();
        for frame in &frames {
            frame::write_frame(&mut bytes, frame).unwrap();
        }
        assert_eq!(requests_in(bytes), frames);
    }

    #[test]
    fn the_reader_skips_frames_it_does_not_know_or_cannot_read_or_that_go_the_other_way() {
        let show = DesktopFrame::Show { address: None };
        let mut bytes = Vec::new();
        frame::write_frame(&mut bytes, &serde_json::json!({"kind": "ping"})).unwrap();
        frame::write_frame(
            &mut bytes,
            &serde_json::json!({"kind": "act", "id": "seven", "act": {"kind": "pick_file"}}),
        )
        .unwrap();
        frame::write_frame(&mut bytes, &serde_json::json!(["not", "a", "frame"])).unwrap();
        for frame in [
            DesktopFrame::Tray {
                item: TrayItemId::Quit,
            },
            DesktopFrame::ActResult {
                id: 1,
                outcome: ActOutcome::Done,
            },
            workbench_frame(URL),
            DesktopFrame::Unknown,
            show.clone(),
        ] {
            frame::write_frame(&mut bytes, &frame).unwrap();
        }
        assert_eq!(requests_in(bytes), [show]);
    }

    #[test]
    fn an_act_this_build_does_not_know_arrives_as_unknown_so_it_can_be_answered() {
        let frame: DesktopFrame =
            serde_json::from_str(r#"{"kind":"act","id":9,"act":{"kind":"print"}}"#).unwrap();
        assert_eq!(
            frame,
            DesktopFrame::Act {
                id: 9,
                act: NativeAct::Unknown
            }
        );
    }

    /// A fresh home directory, removed on drop.
    fn fresh_home() -> Scratch {
        Scratch::named("link")
    }

    /// A service that accepts one shell, answers `reply` and writes `frames`. Joining it returns
    /// the opening it read and its end of the stream, which stays open until the caller drops it.
    fn fake_service(
        home: &Path,
        reply: Reply,
        frames: Vec<DesktopFrame>,
    ) -> thread::JoinHandle<(Opening, Connection)> {
        let listener = Listener::bind_first(home).expect("the fake service binds");
        thread::spawn(move || {
            let mut shell = listener.accept().expect("the shell connects");
            let opening: Opening = frame::read_frame(&mut shell).unwrap().unwrap();
            frame::write_frame(&mut shell, &reply).unwrap();
            for frame in &frames {
                frame::write_frame(&mut shell, frame).unwrap();
            }
            (opening, shell)
        })
    }

    fn accepted() -> Reply {
        Reply::accepted("2.0.0".to_owned(), "instance".to_owned(), None)
    }

    fn workbench_frame(url: &str) -> DesktopFrame {
        DesktopFrame::Workbench {
            url: url.to_owned(),
            bearer: BEARER.to_owned(),
            tray: tray(),
        }
    }

    #[test]
    fn the_link_opens_as_the_desktop_shell_and_ends_once_at_the_goodbye() {
        let home = fresh_home();
        let service = fake_service(
            home.path(),
            accepted(),
            vec![
                workbench_frame(URL),
                DesktopFrame::Goodbye {
                    reason: GoodbyeReason::Replace,
                },
            ],
        );
        let (link, workbench) =
            open_link(home.path(), View::Window).expect("the workbench is handed over");
        assert_eq!(workbench.url, URL);
        assert_eq!(workbench.bearer, BEARER);
        assert_eq!(workbench.tray, tray());
        let (opening, _shell) = service.join().unwrap();
        assert_eq!(opening.role, Role::Desktop);
        assert_eq!(opening.edge.program, Program::Desktop);
        assert_eq!(opening.edge.build, env!("CARGO_PKG_VERSION"));
        assert_eq!(
            opening.channel.as_deref(),
            Some("window"),
            "the shell says what it shows"
        );
        let (ended, rx) = mpsc::channel();
        link.spawn_reader(|_| {}, move |end| ended.send(end).unwrap())
            .unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5))
                .expect("the goodbye ends the link"),
            LinkEnd::Goodbye(GoodbyeReason::Replace)
        );
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "the end is reported once"
        );
    }

    #[test]
    fn a_tray_click_reaches_the_service() {
        let home = fresh_home();
        let listener = Listener::bind_first(home.path()).unwrap();
        let (clicked, rx) = mpsc::channel();
        let service = thread::spawn(move || {
            let mut shell = listener.accept().unwrap();
            let _: Opening = frame::read_frame(&mut shell).unwrap().unwrap();
            frame::write_frame(&mut shell, &accepted()).unwrap();
            frame::write_frame(&mut shell, &workbench_frame(URL)).unwrap();
            let heard: DesktopFrame = frame::read_frame(&mut shell).unwrap().unwrap();
            clicked.send(heard).unwrap();
        });
        let (link, _) = open_link(home.path(), View::Window).unwrap();
        link.send_tray(TrayItemId::Resume).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            DesktopFrame::Tray {
                item: TrayItemId::Resume
            }
        );
        service.join().unwrap();
    }

    #[test]
    fn an_act_result_reaches_the_service_and_a_request_reaches_the_shell() {
        let home = fresh_home();
        let listener = Listener::bind_first(home.path()).unwrap();
        let (heard, rx) = mpsc::channel();
        let service = thread::spawn(move || {
            let mut shell = listener.accept().unwrap();
            let _: Opening = frame::read_frame(&mut shell).unwrap().unwrap();
            frame::write_frame(&mut shell, &accepted()).unwrap();
            frame::write_frame(&mut shell, &workbench_frame(URL)).unwrap();
            frame::write_frame(
                &mut shell,
                &DesktopFrame::Act {
                    id: 8,
                    act: NativeAct::OpenFolder {
                        path: "logs".to_owned(),
                    },
                },
            )
            .unwrap();
            let answer: DesktopFrame = frame::read_frame(&mut shell).unwrap().unwrap();
            heard.send(answer).unwrap();
        });
        let (link, _) = open_link(home.path(), View::Window).unwrap();
        let (asked, requests) = mpsc::channel();
        link.spawn_reader(move |request| asked.send(request).unwrap(), |_| {})
            .unwrap();
        let request = requests.recv_timeout(Duration::from_secs(5)).unwrap();
        let DesktopFrame::Act { id, .. } = request else {
            panic!("the act reaches the shell: {request:?}");
        };
        link.send_act_result(id, ActOutcome::Done).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            DesktopFrame::ActResult {
                id: 8,
                outcome: ActOutcome::Done
            }
        );
        service.join().unwrap();
    }

    #[test]
    fn the_stream_ending_ends_the_link() {
        let home = fresh_home();
        let service = fake_service(home.path(), accepted(), vec![workbench_frame(URL)]);
        let (link, _) = open_link(home.path(), View::Window).unwrap();
        drop(service.join().unwrap());
        let (ended, rx) = mpsc::channel();
        link.spawn_reader(|_| {}, move |end| ended.send(end).unwrap())
            .unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5))
                .expect("the closed stream ends the link"),
            LinkEnd::Lost
        );
    }

    #[test]
    fn a_first_frame_with_another_address_is_a_contract_failure() {
        let home = fresh_home();
        let service = fake_service(
            home.path(),
            accepted(),
            vec![workbench_frame("http://localhost:1/")],
        );
        let failure = open_link(home.path(), View::Window)
            .err()
            .expect("the address is refused");
        assert!(matches!(failure, LinkError::Contract { .. }), "{failure:?}");
        service.join().unwrap();
    }

    #[test]
    fn a_first_frame_of_already_open_is_the_already_open_failure() {
        let home = fresh_home();
        let service = fake_service(home.path(), accepted(), vec![DesktopFrame::AlreadyOpen]);
        let failure = open_link(home.path(), View::Window)
            .err()
            .expect("the shell is turned away");
        assert!(matches!(failure, LinkError::AlreadyOpen), "{failure:?}");
        service.join().unwrap();
    }

    #[test]
    fn a_first_frame_that_is_not_the_workbench_is_a_contract_failure() {
        let home = fresh_home();
        let service = fake_service(home.path(), accepted(), vec![DesktopFrame::Unknown]);
        let failure = open_link(home.path(), View::Window)
            .err()
            .expect("the frame is refused");
        assert!(matches!(failure, LinkError::Contract { .. }), "{failure:?}");
        service.join().unwrap();
    }

    #[test]
    fn a_bearer_of_the_wrong_shape_is_a_contract_failure() {
        let home = fresh_home();
        let service = fake_service(
            home.path(),
            accepted(),
            vec![DesktopFrame::Workbench {
                url: URL.to_owned(),
                bearer: "short".to_owned(),
                tray: tray(),
            }],
        );
        let failure = open_link(home.path(), View::Window)
            .err()
            .expect("the bearer is refused");
        assert!(matches!(failure, LinkError::Contract { .. }), "{failure:?}");
        assert!(!failure.to_string().contains("short"));
        service.join().unwrap();
    }

    #[test]
    fn a_refused_opening_carries_the_services_words() {
        let home = fresh_home();
        let service = fake_service(
            home.path(),
            Reply::refused(RefusalCode::Capacity, "Another window is open.".to_owned()),
            Vec::new(),
        );
        let failure = open_link(home.path(), View::Window)
            .err()
            .expect("the opening is refused");
        match failure {
            LinkError::Refused { code, say } => {
                assert_eq!(code, Some(RefusalCode::Capacity));
                assert_eq!(say, "Another window is open.");
            }
            other => panic!("{other:?}"),
        }
        service.join().unwrap();
    }

    #[test]
    fn no_service_is_a_connect_failure() {
        let home = fresh_home();
        let failure = open_link(home.path(), View::Window)
            .err()
            .expect("nothing listens");
        assert!(
            matches!(failure, LinkError::Connect(ConnectError::NoService)),
            "{failure:?}"
        );
    }
}
