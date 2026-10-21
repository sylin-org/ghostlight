//! Binds the service's protocol-4 types to the golden fixtures the extension's tests read.
//!
//! `tests/golden/protocol4` is the one shared statement of the wire. The extension's `npm test`
//! and the page-asset tests read the same files, so a frame, a primitive, an outcome, a world
//! message or a closed name that drifts on either side fails a test before it reaches a browser.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::link::{FocusTab, Frames, Spec, View, WorldCalls};
use super::wire::{
    self, primitive_name, CallAnswer, CallFailure, CallMode, DiagnosticDetail, DiagnosticSource,
    Event, Failure, Inbound, InputStop, KeyAction, MouseAction, MouseButton, Outbound, Readiness,
    RuntimeControlState,
};
use super::world::{self, kind, AssetErrorCode, Message};
use crate::browser::adapter::{PhysicalField, PhysicalFile, RuntimeControlIntent};
use crate::language::field_problem::{FieldProblemKind, WrongToolRoute};

/// The fixture directory, from the orchestrator crate.
const GOLDEN: &str = "../../tests/golden/protocol4";
/// The page asset that defines the closed asset error codes.
const WORLD_JS: &str = include_str!("assets/world.js");
/// The page asset that dispatches world messages.
const CONTENT_JS: &str = include_str!("assets/content.js");
/// Where `world.js` opens its table of asset error codes.
const CODES_OPEN: &str = "const ERROR_CODES = Object.freeze({";
/// Where `content.js` opens its table of the problem words a refused fill names.
const PROBLEMS_OPEN: &str = "const FIELD_PROBLEMS = Object.freeze({";
/// Where each of those frozen tables closes.
const TABLE_CLOSE: &str = "});";
/// How `content.js` tests the kind of the message it dispatches.
const DISPATCH_TEST: &str = "message.kind === \"";
/// Where `content.js` opens its table of the routes a wrong-tool problem names.
const ROUTES_OPEN: &str = "const WRONG_TOOL_ROUTES = Object.freeze({";
/// The file name suffix of every fixture.
const JSON_SUFFIX: &str = ".json";
/// The case-name prefix of a receipt fixture that carries an outcome.
const OK_CASE: &str = "ok";
/// The primitive name of the receipt fixture for a primitive the extension does not know.
const UNKNOWN_PRIMITIVE: &str = "unknown_primitive";

/// Frame kinds whose fixtures have their own sections: requests, receipts and events.
const SECTIONED_KINDS: [&str; 3] = ["request", "receipt", "event"];

/// Every world message kind the service can build.
const WORLD_KINDS: [&str; 32] = [
    kind::READ_TEXT,
    kind::INSPECT,
    kind::INSPECT_TREE,
    kind::FIND,
    kind::QUERY_SEMANTIC,
    kind::DESCRIBE,
    kind::DESCRIBE_FOCUSED,
    kind::VERIFY_FOCUSED_TEXT,
    kind::BOX,
    kind::PREPARE_FILL,
    kind::VERIFY_FILL_RETAINED,
    kind::OBSERVE,
    kind::ACTIVATE,
    kind::HOVER,
    kind::FOCUS,
    kind::SCROLL_POINT,
    kind::VIEWPORT_POINT,
    kind::SCROLL_OFFSET,
    kind::HIT_POINT,
    kind::OWNER_BOX,
    kind::OWNER_PROBE,
    kind::FOCUS_PROBE,
    kind::REQUIRE_FOCUSED,
    kind::FILL_LOCAL,
    kind::FOCUS_TEXT_FIELD,
    kind::SUBMIT_FILL,
    kind::TYPE_TEXT,
    kind::CLEAR_FOCUSED,
    kind::SCROLL,
    kind::UPLOAD_FILES,
    kind::DROP_FILES,
    kind::ATTACH_FILE,
];

/// Every primitive name the service can send.
const PRIMITIVES: [&str; 17] = [
    primitive_name::LIST_TABS,
    primitive_name::OPEN_TAB,
    primitive_name::NAVIGATE,
    primitive_name::FOCUS_TAB,
    primitive_name::CLOSE_TAB,
    primitive_name::SET_ZOOM,
    primitive_name::RESIZE_WINDOW,
    primitive_name::FRAMES,
    primitive_name::CALL,
    primitive_name::EVALUATE_MAIN,
    primitive_name::VIEW,
    primitive_name::INPUT,
    primitive_name::CAPTURE,
    primitive_name::DIALOG,
    primitive_name::DIAGNOSTICS,
    primitive_name::RECORDING,
    primitive_name::ACCESSIBILITY_SNAPSHOT,
];

/// The serialized names of every variant of a unit enum, listed through an exhaustive match so
/// a variant added in Rust fails to compile here until the vocabulary test sees it.
macro_rules! names {
    ($type:ident: $($variant:ident),+ $(,)?) => {{
        fn exhaustive(value: $type) {
            match value {
                $($type::$variant)|+ => {}
            }
        }
        let _ = exhaustive;
        vec![$(name(&$type::$variant)),+]
    }};
}

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN)
}

fn read(relative: &str) -> Value {
    let path = golden().join(relative);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{path:?}: {error}"))
}

/// Every fixture in one section, by file stem, in name order.
fn section(name: &str) -> Vec<(String, Value)> {
    let mut stems: Vec<String> = fs::read_dir(golden().join(name))
        .unwrap_or_else(|error| panic!("{name}: {error}"))
        .map(|entry| entry.expect("a directory entry").file_name())
        .filter_map(|file| file.to_str()?.strip_suffix(JSON_SUFFIX).map(str::to_owned))
        .collect();
    stems.sort();
    stems
        .into_iter()
        .map(|stem| {
            let value = read(&format!("{name}/{stem}{JSON_SUFFIX}"));
            (stem, value)
        })
        .collect()
}

fn vocabulary() -> Value {
    read("vocabulary.json")
}

/// One vocabulary list as a set of names.
fn listed(vocabulary: &Value, key: &str) -> BTreeSet<String> {
    vocabulary[key]
        .as_array()
        .unwrap_or_else(|| panic!("vocabulary.{key} is a list"))
        .iter()
        .map(|name| name.as_str().expect("a name").to_owned())
        .collect()
}

/// The serialized name of one unit variant.
fn name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value).expect("a closed name serializes") {
        Value::String(name) => name,
        other => panic!("not a name: {other}"),
    }
}

fn set<I: IntoIterator<Item = S>, S: Into<String>>(names: I) -> BTreeSet<String> {
    names.into_iter().map(Into::into).collect()
}

/// The JSON value with every number as a float, so `320` and `320.0` compare equal as they do
/// in JavaScript.
fn normalized(value: &Value) -> Value {
    match value {
        Value::Number(number) => serde_json::json!(number.as_f64()),
        Value::Array(items) => Value::Array(items.iter().map(normalized).collect()),
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(key, member)| (key.clone(), normalized(member)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Whether `actual` carries every member of `expected` with an equal value. A field the fixture
/// sends that the service's type drops or misnames is missing here.
fn covers(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => expected.iter().all(|(key, member)| {
            actual
                .get(key)
                .is_some_and(|present| covers(present, member))
        }),
        (Value::Array(actual), Value::Array(expected)) => {
            actual.len() == expected.len()
                && actual
                    .iter()
                    .zip(expected)
                    .all(|(present, member)| covers(present, member))
        }
        _ => normalized(actual) == normalized(expected),
    }
}

fn decode<T: DeserializeOwned>(context: &str, value: &Value) -> T {
    serde_json::from_value(value.clone()).unwrap_or_else(|error| panic!("{context}: {error}"))
}

fn encode<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("a wire type serializes")
}

/// Decode an `ok` object as `T` and serialize it back.
fn through<T: DeserializeOwned + Serialize>(context: &str, ok: &Value) -> Value {
    encode(&decode::<T>(context, ok))
}

/// Decode a receipt's `ok` object as the outcome of the named primitive, and serialize it back.
fn outcome(primitive: &str, ok: &Value) -> Value {
    match primitive {
        primitive_name::LIST_TABS => through::<<wire::ListTabs as Spec>::Outcome>(primitive, ok),
        primitive_name::OPEN_TAB => through::<<wire::OpenTab as Spec>::Outcome>(primitive, ok),
        primitive_name::NAVIGATE => through::<<wire::Navigate as Spec>::Outcome>(primitive, ok),
        primitive_name::FOCUS_TAB => through::<<FocusTab as Spec>::Outcome>(primitive, ok),
        primitive_name::CLOSE_TAB => through::<<wire::CloseTab as Spec>::Outcome>(primitive, ok),
        primitive_name::SET_ZOOM => through::<<wire::SetZoom as Spec>::Outcome>(primitive, ok),
        primitive_name::RESIZE_WINDOW => {
            through::<<wire::ResizeWindow as Spec>::Outcome>(primitive, ok)
        }
        primitive_name::FRAMES => through::<<Frames as Spec>::Outcome>(primitive, ok),
        primitive_name::CALL => through::<<WorldCalls as Spec>::Outcome>(primitive, ok),
        primitive_name::EVALUATE_MAIN => {
            through::<<wire::EvaluateMain as Spec>::Outcome>(primitive, ok)
        }
        primitive_name::VIEW => through::<<View as Spec>::Outcome>(primitive, ok),
        primitive_name::INPUT => through::<<wire::Input as Spec>::Outcome>(primitive, ok),
        primitive_name::CAPTURE => through::<<wire::Capture as Spec>::Outcome>(primitive, ok),
        primitive_name::DIALOG => through::<<wire::Dialog as Spec>::Outcome>(primitive, ok),
        primitive_name::DIAGNOSTICS => {
            through::<<wire::DiagnosticsOp as Spec>::Outcome>(primitive, ok)
        }
        primitive_name::RECORDING => through::<<wire::RecordingOp as Spec>::Outcome>(primitive, ok),
        primitive_name::ACCESSIBILITY_SNAPSHOT => {
            through::<<wire::AccessibilitySnapshot as Spec>::Outcome>(primitive, ok)
        }
        other => panic!("no outcome for primitive {other}"),
    }
}

/// Decode a receipt frame.
fn receipt(context: &str, frame: &Value) -> wire::Receipt {
    match decode::<Inbound>(context, frame) {
        Inbound::Receipt(receipt) => receipt,
        other => panic!("{context}: not a receipt: {other:?}"),
    }
}

/// The receipt with its `ok` object typed by the named primitive's outcome.
fn typed_receipt(primitive: &str, frame: &Value) -> (wire::Receipt, Option<Value>) {
    let mut receipt = receipt(primitive, frame);
    let ok = receipt.ok.take().map(|ok| outcome(primitive, &ok));
    (receipt, ok)
}

#[test]
fn every_request_serializes_back_to_its_fixture() {
    let requests = section("requests");
    assert!(!requests.is_empty());
    let mut named = BTreeSet::new();
    for (stem, fixture) in requests {
        let typed: Outbound = decode(&stem, &fixture);
        let Outbound::Request { primitive, .. } = &typed else {
            panic!("{stem}: not a request");
        };
        named.insert(name_of(primitive));
        assert_eq!(normalized(&encode(&typed)), normalized(&fixture), "{stem}");
    }
    assert_eq!(
        named,
        set(PRIMITIVES),
        "every primitive has a request fixture"
    );
}

/// The wire name of one primitive.
fn name_of(primitive: &wire::Primitive) -> String {
    encode(primitive)["name"]
        .as_str()
        .expect("a primitive names itself")
        .to_owned()
}

#[test]
fn every_receipt_decodes_into_its_primitives_outcome_or_names_its_failure() {
    for (stem, fixture) in section("receipts") {
        let (primitive, case) = stem.split_once('.').expect("primitive.case");
        let raw = receipt(&stem, &fixture);
        if case.starts_with(OK_CASE) {
            assert!(raw.failure.is_none(), "{stem}");
            let ok = raw.ok.as_ref().unwrap_or_else(|| panic!("{stem}: no ok"));
            assert!(covers(&outcome(primitive, ok), ok), "{stem}");
        } else {
            assert!(raw.ok.is_none(), "{stem}");
            let failure = raw.failure.unwrap_or_else(|| panic!("{stem}: no failure"));
            assert_eq!(name(&failure), case, "{stem}");
            if primitive == UNKNOWN_PRIMITIVE {
                assert_eq!(failure, Failure::Unsupported);
                assert!(!raw.dispatched);
            }
        }
        if let Some(tab) = fixture.get("tab") {
            assert!(covers(&encode(&raw.tab), tab), "{stem}");
        }
    }
}

#[test]
fn every_frame_round_trips_through_its_direction() {
    let vocabulary = vocabulary();
    let kinds = &vocabulary["frame_kinds"];
    let inbound = listed(kinds, "extension_to_service");
    let outbound = listed(kinds, "service_to_extension");
    let relayed = listed(kinds, "relay_to_extension");
    let mut seen = BTreeSet::new();
    for (stem, fixture) in section("frames") {
        // A fixture named `<kind>.<variant>` is another shape of the same frame kind.
        let kind = stem.split('.').next().unwrap_or_default().to_owned();
        if inbound.contains(&kind) {
            let typed: Inbound = decode(&stem, &fixture);
            assert!(!matches!(typed, Inbound::Unknown), "{stem}");
            assert!(covers(&encode(&typed), &fixture), "{stem}");
        } else if outbound.contains(&kind) {
            let typed: Outbound = decode(&stem, &fixture);
            assert_eq!(normalized(&encode(&typed)), normalized(&fixture), "{stem}");
        } else {
            assert!(relayed.contains(&kind), "{stem} is in no direction");
            assert!(
                matches!(decode(&stem, &fixture), Inbound::Unknown),
                "{stem}"
            );
        }
        seen.insert(kind);
    }
    let every: BTreeSet<String> = inbound
        .into_iter()
        .chain(outbound)
        .chain(relayed)
        .filter(|kind| !SECTIONED_KINDS.contains(&kind.as_str()))
        .collect();
    assert_eq!(seen, every, "every frame kind has a fixture");
}

#[test]
fn every_event_decodes_and_round_trips() {
    let mut seen = BTreeSet::new();
    for (stem, fixture) in section("events") {
        let typed: Inbound = decode(&stem, &fixture);
        let Inbound::Event { event } = &typed else {
            panic!("{stem}: not an event");
        };
        assert_ne!(event, &Event::Unknown, "{stem}");
        if let Event::RuntimeControlRequested { .. } = event {
            assert!(event.control_intent().is_some(), "{stem}");
        }
        assert!(covers(&encode(&typed), &fixture), "{stem}");
        seen.insert(
            fixture["event"]["event"]
                .as_str()
                .expect("a tag")
                .to_owned(),
        );
    }
    assert_eq!(seen, listed(&vocabulary(), "events"));
}

#[test]
fn tolerance_fixtures_decode_with_unknowns_mapped() {
    let mut handled = BTreeSet::new();
    for (stem, fixture) in section("tolerance") {
        let frame = &fixture["frame"];
        let expect = &fixture["expect"];
        match stem.as_str() {
            "call.unknown_answer_status" => {
                let (_, ok) = typed_receipt(primitive_name::CALL, frame);
                let called: wire::Called = decode(&stem, &ok.expect("ok"));
                assert_eq!(called.answers, vec![CallAnswer::Unknown]);
            }
            "input.unknown_stop_reason" => {
                let (_, ok) = typed_receipt(primitive_name::INPUT, frame);
                let done: wire::InputDone = decode(&stem, &ok.expect("ok"));
                assert_eq!(done.stopped, Some(InputStop::Other));
            }
            "tab.unknown_readiness" => {
                let (_, ok) = typed_receipt(primitive_name::NAVIGATE, frame);
                let navigated: wire::Navigated = decode(&stem, &ok.expect("ok"));
                assert_eq!(navigated.tab.readiness, Readiness::Unknown);
            }
            "receipt.unknown_failure_code" => {
                assert_eq!(receipt(&stem, frame).failure, Some(Failure::Other));
            }
            "event.unknown_event" => {
                assert!(matches!(
                    decode(&stem, frame),
                    Inbound::Event {
                        event: Event::Unknown
                    }
                ));
            }
            "event.unknown_intent" => {
                let Inbound::Event { event } = decode(&stem, frame) else {
                    panic!("{stem}: not an event");
                };
                assert!(matches!(event, Event::RuntimeControlRequested { .. }));
                assert_eq!(event.control_intent(), None);
            }
            "frame.unknown_kind" => {
                assert!(matches!(decode(&stem, frame), Inbound::Unknown));
            }
            "hello.extra_fields" => {
                let target = read(expect["decodes_as"].as_str().expect("a fixture"));
                assert_eq!(decode::<Inbound>(&stem, frame), decode(&stem, &target));
            }
            "hello_accepted.extra_fields" | "request.extra_fields" => {
                let target = read(expect["decodes_as"].as_str().expect("a fixture"));
                assert_eq!(decode::<Outbound>(&stem, frame), decode(&stem, &target));
            }
            "receipt.extra_fields" => {
                let target = read(expect["decodes_as"].as_str().expect("a fixture"));
                assert_eq!(
                    typed_receipt(primitive_name::NAVIGATE, frame),
                    typed_receipt(primitive_name::NAVIGATE, &target)
                );
            }
            "request.unknown_primitive" => {
                assert!(serde_json::from_value::<Outbound>(frame.clone()).is_err());
                let answer = read(expect["answered"].as_str().expect("a fixture"));
                let answer = receipt(&stem, &answer);
                assert_eq!(answer.failure, Some(Failure::Unsupported));
                assert!(!answer.dispatched);
                assert_eq!(
                    Some(answer.correlation.as_str()),
                    frame["correlation"].as_str()
                );
            }
            other => panic!("tolerance fixture {other} has no expectation here"),
        }
        handled.insert(stem);
    }
    // The panic arm above gives every tolerance fixture an explicit expectation.
    assert!(!handled.is_empty(), "the tolerance fixtures were read");
}

/// The tags at one place in every fixture of a section whose stem starts with `prefix`.
fn tags(section_name: &str, prefix: &str, at: impl Fn(&Value) -> Vec<Value>) -> BTreeSet<String> {
    section(section_name)
        .into_iter()
        .filter(|(stem, _)| stem.starts_with(prefix))
        .flat_map(|(_, fixture)| at(&fixture))
        .map(|tag| tag.as_str().expect("a tag").to_owned())
        .collect()
}

/// The value at a JSON pointer, or none.
fn at(pointer: &'static str) -> impl Fn(&Value) -> Vec<Value> {
    move |value| value.pointer(pointer).cloned().into_iter().collect()
}

#[test]
fn closed_names_equal_the_vocabulary() {
    let vocabulary = vocabulary();
    let check = |key: &str, names: BTreeSet<String>| {
        assert_eq!(names, listed(&vocabulary, key), "vocabulary.{key}");
    };
    check("primitives", set(PRIMITIVES));
    check(
        "receipt_failures",
        set(
            names!(Failure: Unsupported, InvalidRequest, TabMissing, DebuggerUnavailable,
            DocumentChanged, DialogOpen, Timeout, Cancelled, LocalInterlock, CaptureTooLarge,
            BrowserRefused, SessionLost, NoHistoryEntry, Other),
        ),
    );
    check(
        "call_failures",
        set(
            names!(CallFailure: DocumentChanged, FrameMissing, OwnerMissing, DialogOpen,
            Timeout, Cancelled, SessionLost, PageException, Unserializable, Other),
        ),
    );
    check(
        "runtime_control_intents",
        set(names!(RuntimeControlIntent: ToggleHold, Hold, Resume, EndSession, StartSession)),
    );
    check(
        "mouse_actions",
        set(names!(MouseAction: Moved, Pressed, Released, Wheel)),
    );
    check(
        "mouse_buttons",
        set(names!(MouseButton: None, Left, Middle, Right)),
    );
    check(
        "key_actions",
        set(names!(KeyAction: Down, Up, RawDown, Char)),
    );
    check(
        "call_modes",
        set(names!(CallMode: Sequence, Parallel, Race)),
    );
    check(
        "diagnostic_sources",
        set(names!(DiagnosticSource: Both, Console, Network)),
    );
    check(
        "diagnostic_details",
        set(names!(DiagnosticDetail: Problems, All)),
    );
    check(
        "readiness",
        set(names!(Readiness: Loading, Complete, Unknown)),
    );
    check(
        "control_states",
        set(names!(RuntimeControlState: Active, Held, Ended)),
    );
    check("world_message_kinds", set(WORLD_KINDS));

    let steps = |value: &Value| {
        value
            .pointer("/primitive/args/steps")
            .and_then(Value::as_array)
            .map(|steps| steps.iter().map(|step| step["step"].clone()).collect())
            .unwrap_or_default()
    };
    check("input_steps", tags("requests", "input", steps));
    check(
        "clips",
        tags("requests", "capture", at("/primitive/args/clip/clip")),
    );
    check(
        "navigation_targets",
        tags("requests", "navigate", at("/primitive/args/to/to")),
    );
    check(
        "dialog_ops",
        tags("requests", "dialog", at("/primitive/args/op/op")),
    );
    check(
        "diagnostics_ops",
        tags("requests", "diagnostics", at("/primitive/args/op")),
    );
    check(
        "recording_ops",
        tags("requests", "recording", at("/primitive/args/op")),
    );
    check(
        "recording_destinations",
        tags(
            "requests",
            "recording.export",
            at("/primitive/args/destination/destination"),
        ),
    );
    let answers = |value: &Value| {
        value
            .pointer("/ok/answers")
            .and_then(Value::as_array)
            .map(|answers| {
                answers
                    .iter()
                    .map(|answer| answer["status"].clone())
                    .collect()
            })
            .unwrap_or_default()
    };
    check("call_answers", tags("receipts", "call", answers));
}

#[test]
fn the_constants_and_limits_equal_the_vocabulary() {
    let vocabulary = vocabulary();
    let limits = &vocabulary["limits"];
    assert_eq!(vocabulary["protocol"], wire::PROTOCOL);
    assert_eq!(vocabulary["world_name"], wire::WORLD_NAME);
    assert_eq!(vocabulary["platform"], wire::PLATFORM);
    assert_eq!(limits["direct_frame_bytes"], wire::DIRECT_FRAME_BYTES);
    assert_eq!(limits["max_world_calls"], wire::MAX_WORLD_CALLS);
    assert_eq!(limits["max_input_steps"], wire::MAX_INPUT_STEPS);
    assert_eq!(limits["max_unseen"], wire::MAX_UNSEEN);
}

/// Every asset error code, through an exhaustive match.
fn asset_error_codes() -> Vec<String> {
    names!(AssetErrorCode: StaleLocator, NotVisible, Disabled, ReadOnly, NotEditable,
        NotFillable, OptionMissing, CredentialAuthorizationRequired, SubmitOutsideForm,
        NotFileInput, NoFocusedControl, FocusLost, OutsideViewport, NoSubject, UnknownKind,
        InvalidRequest, RuntimeMissing, Cancelled, Unsatisfied, NotRetained, PayloadMismatch,
        PageException)
}

/// The quoted names between two markers of a page asset.
fn quoted_between(source: &str, open: &str, close: &str) -> BTreeSet<String> {
    let start = source.find(open).expect("the opening marker") + open.len();
    let end = start + source[start..].find(close).expect("the closing marker");
    source[start..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

#[test]
fn asset_error_codes_agree_across_rust_the_vocabulary_and_world_js() {
    let vocabulary = vocabulary();
    let names = set(asset_error_codes());
    assert_eq!(names, listed(&vocabulary, "asset_error_codes"));
    assert_eq!(quoted_between(WORLD_JS, CODES_OPEN, TABLE_CLOSE), names);
    let pre_effect: BTreeSet<String> = names
        .iter()
        .filter(|code| {
            decode::<AssetErrorCode>(code, &Value::String((*code).clone())).is_pre_effect()
        })
        .cloned()
        .collect();
    assert_eq!(pre_effect, listed(&vocabulary, "pre_effect_codes"));
}

#[test]
fn every_problem_word_content_js_names_is_one_the_service_decodes() {
    let named = quoted_between(CONTENT_JS, PROBLEMS_OPEN, TABLE_CLOSE);
    assert!(!named.is_empty());
    let known = set(
        names!(FieldProblemKind: OptionMissing, OptionAmbiguous, OptionDisabled,
        MultipleValues, NotBoolean, Format, WrongTool, Reverted, SubmitNotOwned),
    );
    assert!(named.is_subset(&known), "{named:?} is not within {known:?}");
}

#[test]
fn every_wrong_tool_route_content_js_names_is_one_the_service_decodes() {
    let named = quoted_between(CONTENT_JS, ROUTES_OPEN, TABLE_CLOSE);
    let known = set(names!(WrongToolRoute: File, Button, Widget));
    assert_eq!(named, known);
}

#[test]
fn content_js_dispatches_exactly_the_kinds_the_service_builds() {
    let dispatched: BTreeSet<String> = CONTENT_JS
        .split(DISPATCH_TEST)
        .skip(1)
        .map(|rest| rest.split('"').next().expect("a kind").to_owned())
        .collect();
    assert_eq!(dispatched, set(WORLD_KINDS));
}

/// The fields of a fill fixture.
fn fields(value: &Value) -> Vec<PhysicalField> {
    decode("fields", value)
}

/// The files of an upload fixture.
fn files(value: &Value) -> Vec<PhysicalFile> {
    decode("files", value)
}

/// Build the world message a fixture describes with the service's constructor.
fn build(kind_name: &str, f: &Value) -> Message {
    let text = |key: &str| {
        f[key]
            .as_str()
            .unwrap_or_else(|| panic!("{kind_name}.{key}"))
    };
    let optional = |key: &str| f[key].as_str();
    let number = |key: &str| {
        f[key]
            .as_f64()
            .unwrap_or_else(|| panic!("{kind_name}.{key}"))
    };
    let count = |key: &str| {
        f[key]
            .as_u64()
            .unwrap_or_else(|| panic!("{kind_name}.{key}"))
    };
    let size = |key: &str| usize::try_from(count(key)).expect("a size");
    let flag = |key: &str| {
        f[key]
            .as_bool()
            .unwrap_or_else(|| panic!("{kind_name}.{key}"))
    };
    let credentials = || flag("allow_credentials");
    let field = || decode::<PhysicalField>("field", &f["field"]);
    let message = match kind_name {
        kind::READ_TEXT => {
            world::read_text(optional("locator"), optional("mode"), size("max_chars"))
        }
        kind::INSPECT => world::inspect(text("inspect_kind"), size("max_items")),
        kind::INSPECT_TREE => {
            world::inspect_tree(optional("locator"), size("max_depth"), size("max_nodes"))
        }
        kind::FIND => world::find(text("text"), text("find_kind"), size("max_results")),
        kind::QUERY_SEMANTIC => world::query_semantic(
            text("name"),
            optional("role"),
            flag("exact"),
            flag("form_scope"),
            size("max_results"),
        ),
        kind::DESCRIBE => world::describe(&decode::<Vec<String>>("locators", &f["locators"])),
        kind::DESCRIBE_FOCUSED => world::describe_focused(),
        kind::VERIFY_FOCUSED_TEXT => world::verify_focused_text(credentials()),
        kind::BOX => world::box_(text("locator")),
        kind::PREPARE_FILL => world::prepare_fill(
            &fields(&f["fields"]),
            credentials(),
            optional("submit_locator"),
        ),
        kind::VERIFY_FILL_RETAINED => {
            world::verify_fill_retained(&fields(&f["fields"]), credentials(), count("limit_ms"))
        }
        kind::OBSERVE => world::observe(
            text("condition"),
            optional("value"),
            optional("locator"),
            count("timeout_ms"),
            flag("until_satisfied"),
        ),
        kind::ACTIVATE => world::activate(text("locator")),
        kind::HOVER => world::hover(text("locator")),
        kind::FOCUS => world::focus(text("locator")),
        kind::SCROLL_POINT => world::scroll_point(number("x"), number("y")),
        kind::VIEWPORT_POINT => world::viewport_point(number("x"), number("y")),
        kind::SCROLL_OFFSET => world::scroll_offset(),
        kind::HIT_POINT => world::hit_point(number("x"), number("y"), flag("page")),
        kind::OWNER_BOX => world::owner_box(),
        kind::OWNER_PROBE => world::owner_probe(number("x"), number("y")),
        kind::FOCUS_PROBE => world::focus_probe(),
        kind::REQUIRE_FOCUSED => world::require_focused(),
        kind::FILL_LOCAL => world::fill_local(&field(), credentials()),
        kind::FOCUS_TEXT_FIELD => world::focus_text_field(&field(), credentials()),
        kind::SUBMIT_FILL => {
            world::submit_fill(&fields(&f["fields"]), text("submit_locator"), credentials())
        }
        kind::TYPE_TEXT => world::type_text(
            text("locator"),
            text("text"),
            flag("clear_first"),
            credentials(),
        ),
        kind::CLEAR_FOCUSED => world::clear_focused(credentials()),
        kind::SCROLL => world::scroll(
            optional("locator"),
            optional("direction"),
            optional("amount"),
        ),
        kind::UPLOAD_FILES => world::upload_files(text("locator"), &files(&f["files"])),
        kind::DROP_FILES => world::drop_files(number("x"), number("y"), &files(&f["files"])),
        kind::ATTACH_FILE => world::attach_file(text("locator"), text("file_name")),
        other => panic!("no constructor for world kind {other}"),
    };
    match f.get("settle_ms").and_then(Value::as_u64) {
        Some(settle_ms) => message.with_settle(
            settle_ms,
            f.get("visual_settle")
                .and_then(Value::as_bool)
                .expect("a fixture with a budget states the agent's intent"),
        ),
        None => message,
    }
}

#[test]
fn every_world_constructor_builds_its_fixture() {
    let mut seen = BTreeSet::new();
    for (stem, fixture) in section("world") {
        let message = build(&stem, &fixture);
        assert_eq!(message.kind(), stem);
        assert_eq!(
            normalized(&message.to_json()),
            normalized(&fixture),
            "{stem}"
        );
        seen.insert(stem);
    }
    assert_eq!(seen, set(WORLD_KINDS), "every world kind has a fixture");
}
