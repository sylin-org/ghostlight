//! Binds the relay crate to `tests/golden/relay5`, what revision 5 adds to revision 4.
//!
//! Revision 5 is revision 4 with the wake endpoints (ADR-0199 amendment of 2026-10-20) and the
//! `too_large` failure template (ADR-0197 amendment of 2026-10-21). Its frames and openings are
//! revision 4's under a new number, so `golden4.rs` keeps checking those. This file checks that
//! revision 5 adds only those two, and that the crate's constants and types say what it adds.

use std::fs;
use std::path::PathBuf;

use ghostlight_relay::endpoint::WAKE_PIPE_INFIX;
use ghostlight_relay::home::WAKE_DIRECTORY_NAME;
use ghostlight_relay::protocol::{Failure, Reply, Templates};
use ghostlight_relay::wake::{ID_LENGTH, WAKE_DEADLINE};
use ghostlight_relay::RELAY;
use serde_json::{json, Value};

fn golden(revision: &str, name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("golden")
        .join(revision)
        .join(name);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The keys a value of `templates` serializes with.
fn kinds(templates: &Templates) -> Vec<String> {
    let value = serde_json::to_value(templates).unwrap();
    value.as_object().unwrap().keys().cloned().collect()
}

/// The contract both sides read, ADR-0199's amendment of 2026-10-20 and ADR-0197's of
/// 2026-10-21: revision 5's vocabulary is revision 4's with the wake endpoints and the
/// `too_large` template kind added and nothing else changed, and its accepted reply is revision
/// 4's with a `too_large` table. The crate speaks it: its revision, the wake directory, the id
/// length, the pipe infix and the wake deadline are the ones the vocabulary names, the reply
/// round-trips, its `too_large` table answers by method and by default, and templates with no
/// `too_large` table serialize as revision 4's kinds alone.
#[test]
fn revision_5_is_revision_4_with_the_wake_and_too_large_and_the_crate_speaks_it() {
    let mut expected = golden("relay4", "vocabulary.json");
    let five = golden("relay5", "vocabulary.json");
    expected["relay"] = json!(5);
    expected["files"]["wake_directory"] = five["files"]["wake_directory"].clone();
    expected["endpoint"]["wake"] = five["endpoint"]["wake"].clone();
    expected["wake_deadline_ms"] = five["wake_deadline_ms"].clone();
    expected["failure_template_kinds"]
        .as_array_mut()
        .unwrap()
        .push(json!("too_large"));
    assert_eq!(
        five, expected,
        "revision 5 adds only the wake and too_large"
    );

    assert_eq!(five["relay"], json!(RELAY));
    assert_eq!(five["files"]["wake_directory"], json!(WAKE_DIRECTORY_NAME));
    assert_eq!(five["endpoint"]["wake"]["id_length"], json!(ID_LENGTH));
    assert_eq!(
        five["endpoint"]["wake"]["pipe_infix"],
        json!(WAKE_PIPE_INFIX)
    );
    assert_eq!(five["wake_deadline_ms"], json!(WAKE_DEADLINE.as_millis()));

    let reply4 = golden("relay4", "reply.accepted.json");
    let reply5 = golden("relay5", "reply.accepted.json");
    let mut expected = reply4.clone();
    expected["service"] = reply5["service"].clone();
    expected["templates"]["too_large"] = reply5["templates"]["too_large"].clone();
    assert_eq!(reply5, expected, "the reply adds only the too_large table");
    let typed: Reply = serde_json::from_value(reply5.clone()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), reply5);
    let templates = typed.templates.unwrap();
    let call = templates.template(Failure::TooLarge, "tools/call").unwrap();
    let fallback = templates
        .template(Failure::TooLarge, "resources/list")
        .unwrap();
    assert_eq!(call["id"], Value::Null);
    assert_ne!(call, fallback);
    assert_eq!(json!(kinds(&templates)), five["failure_template_kinds"]);

    let four: Reply = serde_json::from_value(reply4.clone()).unwrap();
    let four = four.templates.unwrap();
    assert_eq!(four.template(Failure::TooLarge, "tools/call"), None);
    assert_eq!(
        json!(kinds(&four)),
        golden("relay4", "vocabulary.json")["failure_template_kinds"]
    );
}
