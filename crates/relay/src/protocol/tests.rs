//! Tests of the opening exchange over a connected pair. The vocabulary itself is bound to its
//! fixtures by `tests/golden4.rs` and `tests/golden5.rs`.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use super::*;
use crate::endpoint::pair;
use crate::frame::{read_frame, write_frame};

/// How long a test waits for something that should happen at once.
const PATIENCE: Duration = Duration::from_secs(10);

/// The deadline the test's openings get.
const OPENING_WAIT: Duration = Duration::from_millis(100);

/// How long after its opening's deadline the test's service writes its later frame.
const LATER: Duration = Duration::from_millis(500);

fn edge() -> Edge {
    Edge::new(Program::Edge, "2.0.0".to_owned())
}

fn accepted() -> Reply {
    Reply::accepted(
        "2.0.0".to_owned(),
        "i1".to_owned(),
        Some(Templates::default()),
    )
}

/// ADR-0199 Decision 5 and relay-protocol-4 section 10: the opening exchange is bounded by its
/// deadline and leaves none behind. `open` writes the opening and returns the service's reply; a
/// service that never answers, and a deadline already past, are the deadline; and once the reply
/// is read the stream keeps no timeout, so a frame that comes long after the opening's deadline
/// still arrives, as every long-lived edge and command stream needs.
#[test]
fn the_opening_exchange_is_bounded_by_its_deadline_and_leaves_none_behind() {
    let (client, mut service) = pair().unwrap();
    let mut opening = Opening::new(Role::Mcp, edge());
    opening.channel = Some(MCP_CHANNEL.to_owned());
    let expected = opening.clone();
    let deadline = Instant::now() + OPENING_WAIT;
    let server = thread::spawn(move || {
        let received: Opening = read_frame(&mut service).unwrap().unwrap();
        write_frame(&mut service, &accepted()).unwrap();
        // Let the opening's deadline pass: the condition the later frame is about.
        while Instant::now() < deadline + LATER {
            thread::sleep(Duration::from_millis(10));
        }
        write_frame(&mut service, &json!({"later": true})).unwrap();
        (received, service)
    });
    assert_eq!(open(&client, &opening, deadline).unwrap(), accepted());
    let mut reader = &client;
    let later: Value = read_frame(&mut reader)
        .expect("no timeout is left on the stream")
        .unwrap();
    assert_eq!(later, json!({"later": true}));
    let (received, _service) = server.join().unwrap();
    assert_eq!(received, expected);

    let (silent, _unanswering) = pair().unwrap();
    let (sent, failed) = mpsc::channel();
    thread::spawn(move || {
        let opening = Opening::new(Role::Browser, edge());
        let unanswered = open(&silent, &opening, Instant::now() + OPENING_WAIT).map(|_| ());
        let already_past = open(&silent, &opening, Instant::now()).map(|_| ());
        let _ = sent.send((unanswered, already_past));
    });
    let (unanswered, already_past) = failed
        .recv_timeout(PATIENCE)
        .expect("open gives up at its deadline");
    assert!(
        matches!(unanswered, Err(ConnectError::Deadline)),
        "{unanswered:?}"
    );
    assert!(
        matches!(already_past, Err(ConnectError::Deadline)),
        "{already_past:?}"
    );
}
