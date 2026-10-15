//! The service's one relay listener (ADR-0199 Decision 5).
//!
//! Every peer reaches the service through one endpoint: the MCP edge, the browser edge and the
//! command line alike. The endpoint has already admitted only the same user. Each connection then
//! opens with one relay protocol 4 opening. This module judges an opening before anything else is
//! read, and writes the one reply that accepts or refuses it. What an accepted stream carries is
//! the service's to serve.

use ghostlight_relay::endpoint::Connection;
use ghostlight_relay::frame::{self, FrameError};
use ghostlight_relay::protocol::{
    Control, ControlResult, Opening, Program, QuitReason, RefusalCode, Reply, Role,
};
use ghostlight_relay::{OPENING_DEADLINE, RELAY};
use serde::Serialize;

use crate::intake::opening::{read_opening, OpeningError};
use crate::language::intake::OpeningRefusal;

/// The values control operations answer with.
pub mod control {
    use ghostlight_relay::protocol::StartReason;
    use serde::{Deserialize, Serialize};

    use crate::language::workbench::ConnectionCounts;

    /// Where the workbench is served and whether other screens may watch it. The bearer that
    /// opens the window is never part of it.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct WorkbenchStatus {
        /// The address a person opens, ending in a slash.
        pub url: String,
        /// Whether this is the usual port; it is not when the usual port was taken.
        pub usual_port: bool,
        /// Whether other screens on this computer may watch.
        pub watching: bool,
    }

    /// The answer to `status`: which service answered, and which start of it.
    ///
    /// A service that predates the connection counts and the workbench answers without them, and
    /// its answer still reads.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct Status {
        /// The service's product version.
        pub service: String,
        /// The value that changes on every service start.
        pub instance: String,
        /// The service's process id.
        pub pid: u32,
        /// Why this service was started.
        pub started_by: StartReason,
        /// How many of each kind of connection the service has.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub connections: Option<ConnectionCounts>,
        /// Where the workbench is served, once it is.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub workbench: Option<WorkbenchStatus>,
    }

    /// The answer to `show`: whether a window was there to come forward.
    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct Shown {
        /// Whether a window was there.
        pub shown: bool,
    }

    /// The answer to `quit`: the service has begun to quit.
    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct Quitting {
        /// Always true: the answer is only given once the quit has begun.
        pub quitting: bool,
    }
}

/// What the service does with a new connection once its opening is read.
#[derive(Clone, Debug, PartialEq)]
pub enum Judgement {
    /// The opening is supported; serve this stream after accepting it.
    Admit(Opening),
    /// Answer with this refusal and close.
    Refuse(OpeningRefusal),
    /// The peer sent nothing usable in time or went away; close without a word.
    Drop,
}

/// Whether `opening` comes from the Ghostlight shell of this very build: the only peer the
/// `desktop` role is served to.
fn is_this_builds_shell(opening: &Opening) -> bool {
    opening.edge.program == Program::Desktop && opening.edge.build == env!("CARGO_PKG_VERSION")
}

/// Read the opening of a new connection and judge it.
///
/// The opening must arrive within [`OPENING_DEADLINE`]. Its relay revision must be one this
/// service speaks (this one and later), its role must be one a 2.0 service serves, and a `control`
/// opening must name an operation, and for `quit` a reason, that the service knows. A `desktop`
/// opening must come from the shell of this service's own build. A quitting service takes no new
/// connection.
pub fn judge(connection: &Connection, quitting: bool) -> Judgement {
    let opening = match read_opening(connection, OPENING_DEADLINE) {
        Ok(opening) => opening,
        Err(OpeningError::Malformed(_) | OpeningError::TooLarge) => {
            return Judgement::Refuse(OpeningRefusal::Malformed)
        }
        Err(OpeningError::Deadline | OpeningError::Closed | OpeningError::Io(_)) => {
            return Judgement::Drop
        }
    };
    if opening.relay < RELAY {
        return Judgement::Refuse(OpeningRefusal::Unsupported);
    }
    match (opening.role, opening.control) {
        (Role::Mcp | Role::Browser, _) => {}
        (Role::Desktop, _) if is_this_builds_shell(&opening) => {}
        (Role::Control, None) => return Judgement::Refuse(OpeningRefusal::Malformed),
        (Role::Control, Some(Control::Unknown)) => {
            return Judgement::Refuse(OpeningRefusal::Unsupported)
        }
        (
            Role::Control,
            Some(Control::Quit {
                reason: QuitReason::Unknown,
            }),
        ) => return Judgement::Refuse(OpeningRefusal::Unsupported),
        (Role::Control, Some(_)) => {}
        (Role::Desktop | Role::Command | Role::Unknown, _) => {
            return Judgement::Refuse(OpeningRefusal::Unsupported)
        }
    }
    if quitting {
        return Judgement::Refuse(OpeningRefusal::ShuttingDown);
    }
    Judgement::Admit(opening)
}

/// Accept the opening with `reply`: the stream it named is live from the next frame on.
///
/// # Errors
///
/// Returns the framing error when the reply cannot be written.
pub fn accept(connection: &Connection, reply: &Reply) -> Result<(), FrameError> {
    write_within_deadline(connection, reply)
}

/// Refuse the opening in the service's words. The caller closes the connection.
pub fn refuse(connection: &Connection, refusal: OpeningRefusal) {
    let _ = write_within_deadline(
        connection,
        &Reply::refused(refusal_code(refusal), refusal.message()),
    );
}

/// Answer an accepted control stream with its one result.
///
/// # Errors
///
/// Returns the framing error when the result cannot be written.
pub fn answer_control(connection: &Connection, value: &impl Serialize) -> Result<(), FrameError> {
    let value = serde_json::to_value(value)?;
    write_within_deadline(connection, &ControlResult::Result { value })
}

/// Write one frame within the opening deadline, then lift the write deadline again.
fn write_within_deadline(
    connection: &Connection,
    value: &impl Serialize,
) -> Result<(), FrameError> {
    connection.set_write_timeout(Some(OPENING_DEADLINE))?;
    let mut writer = connection;
    frame::write_frame(&mut writer, value)?;
    connection.set_write_timeout(None)?;
    Ok(())
}

/// The closed relay code of a refusal.
const fn refusal_code(refusal: OpeningRefusal) -> RefusalCode {
    match refusal {
        OpeningRefusal::Unsupported => RefusalCode::Unsupported,
        OpeningRefusal::Malformed => RefusalCode::Malformed,
        OpeningRefusal::Capacity => RefusalCode::Capacity,
        OpeningRefusal::ShuttingDown => RefusalCode::ShuttingDown,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ghostlight_relay::endpoint::pair;
    use ghostlight_relay::protocol::{Edge, Program};
    use serde_json::Value;

    use super::*;

    /// Send `opening` from a client and judge it on the service end.
    fn judged(opening: &Value, quitting: bool) -> Judgement {
        let (mut client, server) = pair().unwrap();
        frame::write_frame_raw(&mut client, &serde_json::to_vec(opening).unwrap()).unwrap();
        let judgement = judge(&server, quitting);
        drop(client);
        judgement
    }

    fn opening(role: Role) -> Value {
        serde_json::to_value(Opening::new(role, Edge::new(Program::Edge, "2.0.0".into()))).unwrap()
    }

    fn fixture(name: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/relay4")
            .join(name);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn every_opening_fixture_the_service_serves_is_admitted() {
        for name in [
            "opening.browser.json",
            "opening.cli.json",
            "opening.control.readiness.json",
            "opening.control.status.json",
            "opening.control.show.json",
            "opening.control.quit.quit.json",
            "opening.control.quit.replace.json",
            "opening.control.quit.uninstall.json",
            "opening.mcp.json",
            "tolerance/opening.extra_fields.json",
        ] {
            let sent = fixture(name);
            let expected: Opening = serde_json::from_value(sent.clone()).unwrap();
            assert_eq!(judged(&sent, false), Judgement::Admit(expected), "{name}");
        }
    }

    #[test]
    fn the_command_role_and_unknown_names_are_unsupported() {
        for name in [
            "opening.command.json",
            "tolerance/opening.unknown_role.json",
            "tolerance/opening.unknown_control_op.json",
            "tolerance/opening.unknown_quit_reason.json",
        ] {
            assert_eq!(
                judged(&fixture(name), false),
                Judgement::Refuse(OpeningRefusal::Unsupported),
                "{name}"
            );
        }
    }

    /// The desktop opening of the frozen golden, with its build replaced by `build`.
    fn desktop_opening(build: &str) -> Value {
        let mut sent = fixture("opening.desktop.json");
        sent["edge"]["build"] = build.into();
        sent
    }

    #[test]
    fn the_frozen_desktop_golden_is_admitted_exactly_when_its_build_is_this_one() {
        let sent = fixture("opening.desktop.json");
        let decoded: Opening = serde_json::from_value(sent.clone()).unwrap();
        let expected = if decoded.edge.build == env!("CARGO_PKG_VERSION") {
            Judgement::Admit(decoded)
        } else {
            Judgement::Refuse(OpeningRefusal::Unsupported)
        };
        assert_eq!(judged(&sent, false), expected);
    }

    #[test]
    fn a_desktop_opening_of_this_build_is_admitted() {
        let sent = desktop_opening(env!("CARGO_PKG_VERSION"));
        let decoded: Opening = serde_json::from_value(sent.clone()).unwrap();
        assert_eq!(judged(&sent, false), Judgement::Admit(decoded));
    }

    #[test]
    fn a_desktop_opening_of_another_build_or_from_another_program_is_unsupported() {
        let other_build = desktop_opening("0.0.1-other");
        let mut edge = desktop_opening(env!("CARGO_PKG_VERSION"));
        edge["edge"]["program"] = "edge".into();
        for sent in [other_build, edge] {
            assert_eq!(
                judged(&sent, false),
                Judgement::Refuse(OpeningRefusal::Unsupported),
                "{sent}"
            );
        }
    }

    #[test]
    fn a_quitting_service_refuses_the_shell_too() {
        assert_eq!(
            judged(&desktop_opening(env!("CARGO_PKG_VERSION")), true),
            Judgement::Refuse(OpeningRefusal::ShuttingDown)
        );
    }

    #[test]
    fn an_opening_that_names_an_earlier_relay_is_unsupported() {
        let mut sent = opening(Role::Browser);
        sent["relay"] = 3.into();
        assert_eq!(
            judged(&sent, false),
            Judgement::Refuse(OpeningRefusal::Unsupported)
        );
        let mut later = opening(Role::Browser);
        later["relay"] = 5.into();
        assert!(matches!(judged(&later, false), Judgement::Admit(_)));
    }

    #[test]
    fn a_control_opening_without_an_operation_is_malformed() {
        assert_eq!(
            judged(&opening(Role::Control), false),
            Judgement::Refuse(OpeningRefusal::Malformed)
        );
    }

    #[test]
    fn a_quitting_service_refuses_new_connections() {
        assert_eq!(
            judged(&opening(Role::Browser), true),
            Judgement::Refuse(OpeningRefusal::ShuttingDown)
        );
    }

    #[test]
    fn a_frame_that_is_not_an_opening_is_malformed() {
        assert_eq!(
            judged(&serde_json::json!([1, 2]), false),
            Judgement::Refuse(OpeningRefusal::Malformed)
        );
    }

    #[test]
    fn a_peer_that_goes_away_before_its_opening_is_dropped() {
        let (client, server) = pair().unwrap();
        drop(client);
        assert_eq!(judge(&server, false), Judgement::Drop);
    }

    #[test]
    fn refusals_carry_their_relay_code_and_the_service_sentence() {
        let (mut client, server) = pair().unwrap();
        refuse(&server, OpeningRefusal::Unsupported);
        let reply: Reply = frame::read_frame(&mut client).unwrap().unwrap();
        assert_eq!(
            reply,
            Reply::refused(
                RefusalCode::Unsupported,
                OpeningRefusal::Unsupported.message()
            )
        );
        let pinned: Reply = serde_json::from_value(fixture("reply.refused.json")).unwrap();
        assert!(!pinned.accepted);
        assert_eq!(pinned.code, reply.code);
        assert!(pinned.say.is_some() && reply.say.is_some());
    }

    #[test]
    fn a_control_answer_is_one_result_frame() {
        let (mut client, server) = pair().unwrap();
        answer_control(&server, &control::Shown { shown: true }).unwrap();
        let result: ControlResult = frame::read_frame(&mut client).unwrap().unwrap();
        let ControlResult::Result { value } = result;
        assert_eq!(value, serde_json::json!({"shown": true}));
    }

    #[test]
    fn the_status_answer_has_the_shape_of_the_golden_result() {
        let golden: ControlResult =
            serde_json::from_value(fixture("control.result.status.json")).unwrap();
        let ControlResult::Result { value } = golden;
        let status: control::Status = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(&status).unwrap(), value);
    }
}
