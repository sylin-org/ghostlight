//! Binds the relay crate to `tests/golden/relay5/vocabulary.json`, the vocabulary of revision 5.
//!
//! Revision 5 is revision 4 with the wake endpoints (ADR-0199 amendment of 2026-10-20). Its frames
//! and openings are revision 4's under a new number, so `golden4.rs` keeps checking those. This file
//! checks that revision 5 adds only the wake, and that the crate's constants say what it adds.

use std::fs;
use std::path::PathBuf;

use ghostlight_relay::endpoint::WAKE_PIPE_INFIX;
use ghostlight_relay::home::WAKE_DIRECTORY_NAME;
use ghostlight_relay::wake::{ID_LENGTH, WAKE_DEADLINE};
use ghostlight_relay::RELAY;
use serde_json::{json, Value};

fn vocabulary(revision: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
        .join(revision)
        .join("vocabulary.json");
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The contract both sides read, ADR-0199's amendment of 2026-10-20: revision 5's vocabulary is
/// revision 4's with the wake endpoints added and nothing else changed, and the crate speaks
/// it: its revision, the wake directory, the id length, the pipe infix and the wake deadline are
/// the ones the vocabulary names.
#[test]
fn revision_5_is_revision_4_with_the_wake_and_the_crate_speaks_it() {
    let mut expected = vocabulary("relay4");
    let five = vocabulary("relay5");
    expected["relay"] = json!(5);
    expected["files"]["wake_directory"] = five["files"]["wake_directory"].clone();
    expected["endpoint"]["wake"] = five["endpoint"]["wake"].clone();
    expected["wake_deadline_ms"] = five["wake_deadline_ms"].clone();
    assert_eq!(five, expected, "revision 5 adds only the wake");

    assert_eq!(five["relay"], json!(RELAY));
    assert_eq!(five["files"]["wake_directory"], json!(WAKE_DIRECTORY_NAME));
    assert_eq!(five["endpoint"]["wake"]["id_length"], json!(ID_LENGTH));
    assert_eq!(
        five["endpoint"]["wake"]["pipe_infix"],
        json!(WAKE_PIPE_INFIX)
    );
    assert_eq!(five["wake_deadline_ms"], json!(WAKE_DEADLINE.as_millis()));
}
