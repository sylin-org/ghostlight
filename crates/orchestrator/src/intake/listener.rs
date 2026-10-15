//! The service's one relay listener (ADR-0197 Decisions 2 and 3).
//!
//! Every peer reaches the service through one loopback listener: the MCP connector, the browser
//! connector and the command line alike. Each connection opens with one relay protocol 3 opening.
//! This module binds the listener, judges an opening before anything else is read, and writes the
//! one reply that accepts or refuses it. What an accepted stream carries is the service's to serve.

use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};

use ghostlight_relay::frame::{self, FrameError};
use ghostlight_relay::opening::{ControlOp, ControlResult, RefusalCode, Reply, Stream};
use ghostlight_relay::server::{constant_time_eq, read_opening, OpeningError};
use ghostlight_relay::{OPENING_DEADLINE, RELAY};
use serde_json::Value;

use crate::language::intake::OpeningRefusal;

/// The fields of the values control operations answer with.
pub mod control {
    /// `activate_workbench`: whether a workbench was there to come forward.
    pub const AVAILABLE: &str = "available";
    /// `quit`: the service has begun to quit.
    pub const QUITTING: &str = "quitting";
}

/// What the service does with a new connection once its opening is read.
#[derive(Clone, Debug, PartialEq)]
pub enum Judgement {
    /// The opening is authentic and supported; serve this stream after accepting it.
    Admit(Stream),
    /// Answer with this refusal and close.
    Refuse(OpeningRefusal),
    /// The peer sent nothing usable in time or went away; close without a word.
    Drop,
}

/// Bind the one listener on an ephemeral loopback port.
///
/// # Errors
///
/// Returns the socket error when the port cannot be bound.
pub fn bind() -> io::Result<TcpListener> {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
}

/// Read the opening of a new connection and judge it against the service's token.
///
/// The opening must arrive within [`OPENING_DEADLINE`]. The token is compared first, so a peer
/// without it learns nothing else; then the relay revision and the stream kind must be ones this
/// service speaks, and a quitting service takes no new connection.
pub fn judge(connection: &mut TcpStream, token: &str, quitting: bool) -> Judgement {
    let opening = match read_opening(connection, OPENING_DEADLINE) {
        Ok(opening) => opening,
        Err(OpeningError::Malformed(_) | OpeningError::TooLarge) => {
            return Judgement::Refuse(OpeningRefusal::Malformed)
        }
        Err(OpeningError::Deadline | OpeningError::Closed | OpeningError::Io(_)) => {
            return Judgement::Drop
        }
    };
    if !constant_time_eq(opening.token.as_bytes(), token.as_bytes()) {
        return Judgement::Refuse(OpeningRefusal::Authentication);
    }
    if opening.relay != RELAY {
        return Judgement::Refuse(OpeningRefusal::Unsupported);
    }
    if matches!(
        opening.stream,
        Stream::Unknown
            | Stream::Control {
                op: ControlOp::Unknown
            }
    ) {
        return Judgement::Refuse(OpeningRefusal::Unsupported);
    }
    if quitting {
        return Judgement::Refuse(OpeningRefusal::ShuttingDown);
    }
    Judgement::Admit(opening.stream)
}

/// Accept the opening: the stream it named is live from the next frame on.
///
/// # Errors
///
/// Returns the framing error when the reply cannot be written.
pub fn accept(connection: &mut TcpStream) -> Result<(), FrameError> {
    reply(
        connection,
        &Reply::Accepted {
            service_version: env!("CARGO_PKG_VERSION").into(),
        },
    )
}

/// Refuse the opening in the service's words. The caller closes the connection.
pub fn refuse(connection: &mut TcpStream, refusal: OpeningRefusal) {
    let _ = reply(
        connection,
        &Reply::Refused {
            code: refusal_code(refusal),
            message: refusal.message(),
        },
    );
}

/// Answer an accepted control stream with its one result.
///
/// # Errors
///
/// Returns the framing error when the result cannot be written.
pub fn answer_control(connection: &mut TcpStream, value: Value) -> Result<(), FrameError> {
    reply(connection, &ControlResult::Result { value })
}

/// Write one frame within the opening deadline, then lift the write deadline again.
fn reply(connection: &mut TcpStream, value: &impl serde::Serialize) -> Result<(), FrameError> {
    connection.set_write_timeout(Some(OPENING_DEADLINE))?;
    frame::write_frame(connection, value)?;
    connection.set_write_timeout(None)?;
    Ok(())
}

/// The closed relay code of a refusal.
const fn refusal_code(refusal: OpeningRefusal) -> RefusalCode {
    match refusal {
        OpeningRefusal::Authentication => RefusalCode::AuthenticationFailed,
        OpeningRefusal::Unsupported => RefusalCode::Unsupported,
        OpeningRefusal::Malformed => RefusalCode::Malformed,
        OpeningRefusal::Capacity => RefusalCode::Capacity,
        OpeningRefusal::ShuttingDown => RefusalCode::ShuttingDown,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::thread;

    use ghostlight_relay::opening::Opening;

    use super::*;

    const TOKEN: &str = "runtime_test";

    /// Send `opening` from a client and judge it on the accepted end.
    fn judged(opening: &Value, quitting: bool) -> Judgement {
        let listener = bind().unwrap();
        let address = listener.local_addr().unwrap();
        let bytes = serde_json::to_vec(opening).unwrap();
        let client = thread::spawn(move || {
            let mut client = TcpStream::connect(address).unwrap();
            frame::write_frame_raw(&mut client, &bytes).unwrap();
            client
        });
        let (mut connection, _) = listener.accept().unwrap();
        let judgement = judge(&mut connection, TOKEN, quitting);
        drop(client.join().unwrap());
        judgement
    }

    fn opening(stream: Stream) -> Value {
        serde_json::to_value(Opening::new(TOKEN.into(), stream)).unwrap()
    }

    fn fixture(name: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/relay3")
            .join(name);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn every_relay_3_opening_fixture_is_admitted_with_its_token() {
        for name in [
            "opening.browser.json",
            "opening.cli.json",
            "opening.control.activate_workbench.json",
            "opening.control.quit.json",
            "opening.control.readiness.json",
            "opening.mcp.json",
            "opening.mcp.resume.json",
        ] {
            let mut sent = fixture(name);
            let expected: Opening = serde_json::from_value(sent.clone()).unwrap();
            sent["token"] = TOKEN.into();
            assert_eq!(
                judged(&sent, false),
                Judgement::Admit(expected.stream),
                "{name}"
            );
        }
    }

    #[test]
    fn a_wrong_token_is_refused_before_anything_else() {
        let mut sent = opening(Stream::Browser);
        sent["token"] = "runtime_other".into();
        sent["relay"] = 9.into();
        assert_eq!(
            judged(&sent, true),
            Judgement::Refuse(OpeningRefusal::Authentication)
        );
    }

    #[test]
    fn another_relay_revision_and_unknown_streams_are_unsupported() {
        let mut other_relay = opening(Stream::Browser);
        other_relay["relay"] = 4.into();
        let unknown_stream = opening(Stream::Unknown);
        let mut unknown_op = opening(Stream::Browser);
        unknown_op["stream"] = serde_json::json!({"kind": "control", "op": "reboot"});
        for sent in [other_relay, unknown_stream, unknown_op] {
            assert_eq!(
                judged(&sent, false),
                Judgement::Refuse(OpeningRefusal::Unsupported)
            );
        }
    }

    #[test]
    fn a_quitting_service_refuses_new_connections() {
        assert_eq!(
            judged(&opening(Stream::Browser), true),
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
    fn refusals_carry_their_relay_code_and_the_service_sentence() {
        let listener = bind().unwrap();
        let address = listener.local_addr().unwrap();
        let reader = thread::spawn(move || {
            let mut client = TcpStream::connect(address).unwrap();
            frame::read_frame::<Reply>(&mut client).unwrap().unwrap()
        });
        let (mut connection, _) = listener.accept().unwrap();
        refuse(&mut connection, OpeningRefusal::Authentication);
        assert_eq!(
            reader.join().unwrap(),
            Reply::Refused {
                code: RefusalCode::AuthenticationFailed,
                message: OpeningRefusal::Authentication.message(),
            }
        );
        let fixture: Reply = serde_json::from_value(fixture("reply.refused.json")).unwrap();
        assert_eq!(
            fixture,
            Reply::Refused {
                code: RefusalCode::AuthenticationFailed,
                message: OpeningRefusal::Authentication.message(),
            }
        );
    }
}
