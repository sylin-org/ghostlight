//! The shell's stream to the service: the `desktop` role of relay protocol 4 (ADR-0199
//! Decision 7 and its amendment of 2026-10-15).
//!
//! [`open_link`] connects to the service of a Ghostlight home, opens it as the desktop shell and
//! reads the one frame that matters: where the workbench is and the bearer that makes its page
//! the window's. After that the stream carries a goodbye from the service and a click on a tray
//! entry from the shell, and nothing else.
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
use ghostlight_relay::protocol::{self, Edge, GoodbyeReason, Opening, Program, Role};
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
        /// The tray to draw, when the service has none of its own.
        tray: Option<TrayModel>,
    },
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
    /// A frame kind this build does not know. Readers skip it.
    #[serde(other)]
    Unknown,
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
            Self::Goodbye { reason } => formatter
                .debug_struct("Goodbye")
                .field("reason", reason)
                .finish(),
            Self::Tray { item } => formatter.debug_struct("Tray").field("item", item).finish(),
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
    /// The tray to draw. `None` while the service has a tray of its own.
    pub tray: Option<TrayModel>,
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
    /// The service refused the shell; this is its own sentence.
    #[error("{0}")]
    Refused(String),
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

/// The shell's end of the stream, after the workbench was handed over.
pub struct Link {
    connection: Connection,
}

/// Connects to the service of `home`, opens it as the desktop shell and reads the workbench.
///
/// A refused opening is [`LinkError::Refused`] with the service's own words. A first frame that is
/// not a workbench frame, an address that is not `http://127.0.0.1:<port>/` or a bearer of the
/// wrong shape is [`LinkError::Contract`].
pub fn open_link(home: &std::path::Path) -> Result<(Link, Workbench), LinkError> {
    let deadline = Instant::now() + OPEN_DEADLINE;
    let opening = Opening::new(
        Role::Desktop,
        Edge::new(Program::Desktop, env!("CARGO_PKG_VERSION").to_owned()),
    );
    let (connection, reply) = protocol::connect(home, &opening, deadline)?;
    if !reply.accepted {
        return Err(LinkError::Refused(reply.say.unwrap_or_default()));
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
    /// Reads frames on a thread of its own and calls `on_end` once, when the service says goodbye
    /// (whatever the reason) or the stream ends or fails. Every other frame is skipped.
    pub fn spawn_reader(&self, on_end: impl FnOnce() + Send + 'static) -> Result<(), LinkError> {
        let mut reader = self.connection.try_clone()?;
        thread::Builder::new()
            .name(READER_THREAD_NAME.to_owned())
            .spawn(move || {
                read_until_end(&mut reader);
                on_end();
            })?;
        Ok(())
    }

    /// Tells the service the person chose `item` in the tray.
    pub fn send_tray(&self, item: TrayItemId) -> Result<(), LinkError> {
        let mut writer = self.connection.try_clone()?;
        frame::write_frame(&mut writer, &DesktopFrame::Tray { item }).map_err(|error| match error {
            FrameError::Io(error) => LinkError::Io(error),
            other => LinkError::Io(io::Error::other(other)),
        })
    }
}

/// Reads frames until a goodbye, the end of the stream or a failure.
fn read_until_end(reader: &mut impl Read) {
    loop {
        match frame::read_frame::<DesktopFrame>(reader) {
            Ok(Some(DesktopFrame::Goodbye { .. }) | None) | Err(_) => return,
            Ok(Some(_)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    use ghostlight_relay::endpoint::Listener;
    use ghostlight_relay::protocol::{RefusalCode, Reply};

    use super::*;

    const URL: &str = "http://127.0.0.1:41234/";

    /// A bearer of the right shape.
    const BEARER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNO-_";

    fn golden(name: &str) -> DesktopFrame {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/desktop")
            .join(name);
        let text = std::fs::read_to_string(&path).expect("the golden is readable");
        serde_json::from_str(&text).expect("the golden decodes")
    }

    #[test]
    fn each_golden_decodes_to_its_frame() {
        assert_eq!(
            golden("workbench-no-tray.json"),
            DesktopFrame::Workbench {
                url: URL.to_owned(),
                bearer: "BEARER".to_owned(),
                tray: None,
            }
        );
        let DesktopFrame::Workbench { tray, .. } = golden("workbench.json") else {
            panic!("workbench.json is a workbench frame");
        };
        let tray = tray.expect("the golden carries a tray");
        assert_eq!(tray.validate(), Ok(()));
        let ids: Vec<&str> = tray.items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["open", "quit"]);
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
    }

    #[test]
    fn a_frame_kind_this_build_does_not_know_is_unknown() {
        let frame: DesktopFrame = serde_json::from_str(r#"{"kind":"ping"}"#).unwrap();
        assert_eq!(frame, DesktopFrame::Unknown);
    }

    #[test]
    fn the_shell_writes_the_bytes_of_the_tray_golden() {
        let written = serde_json::to_value(DesktopFrame::Tray {
            item: TrayItemId::Hold,
        })
        .unwrap();
        let golden: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/desktop/tray.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(written, golden);
    }

    #[test]
    fn the_bearer_is_never_printed() {
        let secret = "s3cr3t-bearer-value";
        let frame = DesktopFrame::Workbench {
            url: URL.to_owned(),
            bearer: secret.to_owned(),
            tray: None,
        };
        let workbench = Workbench {
            url: URL.to_owned(),
            bearer: secret.to_owned(),
            tray: None,
        };
        let errors = [
            LinkError::Refused("no".to_owned()),
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
            tray: None,
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
            read_until_end(&mut cursor);
            let rest = frame::read_frame::<DesktopFrame>(&mut cursor).unwrap();
            assert!(rest.is_some(), "the frame after the goodbye is left unread");
        }
        read_until_end(&mut Cursor::new(Vec::new()));
        read_until_end(&mut Cursor::new(vec![9, 0, 0, 0, 1]));
    }

    static HOMES: AtomicUsize = AtomicUsize::new(0);

    /// A fresh home directory, removed on drop.
    struct Home(PathBuf);

    impl Home {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "ghostlight-desktop-link-{}-{}",
                std::process::id(),
                HOMES.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&directory).unwrap();
            Self(directory)
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
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
            tray: None,
        }
    }

    #[test]
    fn the_link_opens_as_the_desktop_shell_and_ends_once_at_the_goodbye() {
        let home = Home::new();
        let service = fake_service(
            &home.0,
            accepted(),
            vec![
                workbench_frame(URL),
                DesktopFrame::Goodbye {
                    reason: GoodbyeReason::Replace,
                },
            ],
        );
        let (link, workbench) = open_link(&home.0).expect("the workbench is handed over");
        assert_eq!(workbench.url, URL);
        assert_eq!(workbench.bearer, BEARER);
        assert_eq!(workbench.tray, None);
        let (opening, _shell) = service.join().unwrap();
        assert_eq!(opening.role, Role::Desktop);
        assert_eq!(opening.edge.program, Program::Desktop);
        assert_eq!(opening.edge.build, env!("CARGO_PKG_VERSION"));
        let (ended, rx) = mpsc::channel();
        link.spawn_reader(move || ended.send(()).unwrap()).unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("the goodbye ends the link");
        assert!(
            rx.recv_timeout(Duration::from_millis(200)).is_err(),
            "the end is reported once"
        );
    }

    #[test]
    fn a_tray_click_reaches_the_service() {
        let home = Home::new();
        let listener = Listener::bind_first(&home.0).unwrap();
        let (clicked, rx) = mpsc::channel();
        let service = thread::spawn(move || {
            let mut shell = listener.accept().unwrap();
            let _: Opening = frame::read_frame(&mut shell).unwrap().unwrap();
            frame::write_frame(&mut shell, &accepted()).unwrap();
            frame::write_frame(&mut shell, &workbench_frame(URL)).unwrap();
            let heard: DesktopFrame = frame::read_frame(&mut shell).unwrap().unwrap();
            clicked.send(heard).unwrap();
        });
        let (link, _) = open_link(&home.0).unwrap();
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
    fn the_stream_ending_ends_the_link() {
        let home = Home::new();
        let service = fake_service(&home.0, accepted(), vec![workbench_frame(URL)]);
        let (link, _) = open_link(&home.0).unwrap();
        drop(service.join().unwrap());
        let (ended, rx) = mpsc::channel();
        link.spawn_reader(move || ended.send(()).unwrap()).unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("the closed stream ends the link");
    }

    #[test]
    fn a_first_frame_with_another_address_is_a_contract_failure() {
        let home = Home::new();
        let service = fake_service(
            &home.0,
            accepted(),
            vec![workbench_frame("http://localhost:1/")],
        );
        let failure = open_link(&home.0).err().expect("the address is refused");
        assert!(matches!(failure, LinkError::Contract { .. }), "{failure:?}");
        service.join().unwrap();
    }

    #[test]
    fn a_first_frame_that_is_not_the_workbench_is_a_contract_failure() {
        let home = Home::new();
        let service = fake_service(&home.0, accepted(), vec![DesktopFrame::Unknown]);
        let failure = open_link(&home.0).err().expect("the frame is refused");
        assert!(matches!(failure, LinkError::Contract { .. }), "{failure:?}");
        service.join().unwrap();
    }

    #[test]
    fn a_bearer_of_the_wrong_shape_is_a_contract_failure() {
        let home = Home::new();
        let service = fake_service(
            &home.0,
            accepted(),
            vec![DesktopFrame::Workbench {
                url: URL.to_owned(),
                bearer: "short".to_owned(),
                tray: None,
            }],
        );
        let failure = open_link(&home.0).err().expect("the bearer is refused");
        assert!(matches!(failure, LinkError::Contract { .. }), "{failure:?}");
        assert!(!failure.to_string().contains("short"));
        service.join().unwrap();
    }

    #[test]
    fn a_refused_opening_carries_the_services_words() {
        let home = Home::new();
        let service = fake_service(
            &home.0,
            Reply::refused(RefusalCode::Capacity, "Another window is open.".to_owned()),
            Vec::new(),
        );
        let failure = open_link(&home.0).err().expect("the opening is refused");
        match failure {
            LinkError::Refused(say) => assert_eq!(say, "Another window is open."),
            other => panic!("{other:?}"),
        }
        service.join().unwrap();
    }

    #[test]
    fn no_service_is_a_connect_failure() {
        let home = Home::new();
        let failure = open_link(&home.0).err().expect("nothing listens");
        assert!(
            matches!(failure, LinkError::Connect(ConnectError::NoService)),
            "{failure:?}"
        );
    }
}
