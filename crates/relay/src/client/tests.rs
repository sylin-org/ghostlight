//! Tests of the connector side of an opening against a fake service on loopback.

use std::net::TcpListener;
use std::thread::{self, JoinHandle};

use super::*;
use crate::opening::RefusalCode;

const TOKEN: &str = "secret";

/// A fake service: accepts one connection, reads its opening and answers as `answer` says.
fn fake_service(
    answer: impl FnOnce(&Opening, &mut TcpStream) + Send + 'static,
) -> (u16, JoinHandle<Opening>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let opening: Opening = frame::read_frame(&mut connection).unwrap().unwrap();
        answer(&opening, &mut connection);
        opening
    });
    (port, handle)
}

fn accept_matching_token(opening: &Opening, connection: &mut TcpStream) {
    let reply = if opening.token == TOKEN {
        Reply::Accepted {
            service_version: "2.0.0".to_owned(),
        }
    } else {
        Reply::Refused {
            code: RefusalCode::AuthenticationFailed,
            message: String::new(),
        }
    };
    frame::write_frame(connection, &reply).unwrap();
}

#[test]
fn an_accepted_opening_returns_the_live_stream_and_the_reply() {
    let (port, service) = fake_service(accept_matching_token);
    let (mut socket, reply) = open(
        port,
        TOKEN.to_owned(),
        Stream::Browser,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(
        reply,
        Reply::Accepted {
            service_version: "2.0.0".to_owned()
        }
    );
    let opening = service.join().unwrap();
    assert_eq!(opening, Opening::new(TOKEN.to_owned(), Stream::Browser));
    assert!(socket.read_timeout().unwrap().is_none());
    // The stream is still usable: the service end has closed, which reads as a clean end.
    assert!(frame::read_frame_raw(&mut socket).unwrap().is_none());
}

#[test]
fn a_refused_opening_is_a_reply_not_an_error() {
    let (port, service) = fake_service(accept_matching_token);
    let (_, reply) = open(
        port,
        "wrong".to_owned(),
        Stream::Browser,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(
        reply,
        Reply::Refused {
            code: RefusalCode::AuthenticationFailed,
            message: String::new()
        }
    );
    service.join().unwrap();
}

#[test]
fn a_service_that_never_answers_hits_the_deadline() {
    let (port, service) = fake_service(|_, connection| {
        // Hold the connection open until the client has given up.
        let mut byte = [0_u8; 1];
        let _ = io::Read::read(connection, &mut byte);
    });
    let started = Instant::now();
    let error = open(
        port,
        TOKEN.to_owned(),
        Stream::Browser,
        Duration::from_millis(150),
    )
    .unwrap_err();
    assert!(matches!(error, ConnectError::Deadline), "{error:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    service.join().unwrap();
}

#[test]
fn a_service_that_closes_without_replying_is_closed() {
    let (port, service) = fake_service(|_, _| {});
    let error = open(
        port,
        TOKEN.to_owned(),
        Stream::Browser,
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(matches!(error, ConnectError::Closed), "{error:?}");
    service.join().unwrap();
}

#[test]
fn a_reply_of_an_unknown_kind_is_returned_as_unknown() {
    let (port, service) = fake_service(|_, connection| {
        frame::write_frame(connection, &serde_json::json!({"kind": "queued"})).unwrap();
    });
    let (_, reply) = open(
        port,
        TOKEN.to_owned(),
        Stream::Browser,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(reply, Reply::Unknown);
    service.join().unwrap();
}

#[test]
fn a_closed_port_is_an_io_error() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let error = open(
        port,
        TOKEN.to_owned(),
        Stream::Browser,
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(matches!(error, ConnectError::Io(_)), "{error:?}");
}
