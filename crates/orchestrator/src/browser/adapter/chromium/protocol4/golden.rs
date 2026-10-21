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
    self, CallAnswer, CallFailure, CallMode, DiagnosticDetail, DiagnosticSource, Event, Failure,
    Inbound, InputStop, KeyAction, MouseAction, MouseButton, Outbound, PresentationActivity,
    PresentationKind, PrimitiveName, Readiness, RecordingStopReason, RuntimeControlState,
};
use super::world::{self, AssetErrorCode, Message, WorldKind};
use crate::browser::adapter::{PhysicalField, PhysicalFile, RuntimeControlIntent};
use crate::language::area::{LeftOutKey, PageArea};
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
/// Where `content.js` opens its list of the page areas an item can belong to.
const AREAS_OPEN: &str = "const AREA_WORDS = Object.freeze([";
/// Where `content.js` opens its table of the tiers a capped answer is cut by.
const LEFT_OUT_OPEN: &str = "const LEFT_OUT = Object.freeze({";
/// Where a frozen list closes.
const LIST_CLOSE: &str = "]);";
/// The page asset that draws presentation signals.
const PRESENTATION_JS: &str = include_str!("assets/presentation.js");
/// Where `presentation.js` opens its table of signal kinds.
const SIGNALS_OPEN: &str = "const SIGNAL = Object.freeze({";
/// Where `presentation.js` opens its table of activities.
const ACTIVITIES_OPEN: &str = "const ACTIVITY = Object.freeze({";
/// Where `presentation.js` reads the runtime state the extension reports.
const RUNTIME_READ_OPEN: &str = "function setRuntimeState(";
/// Where a function of the presentation module closes.
const FUNCTION_CLOSE: &str = "\n  }\n";
/// The extension, from the orchestrator crate. Its packaged files are read, never edited, so
/// these pins hold the frozen package to the service without changing it.
const EXTENSION: &str = "../../extension";
/// The extension module that names stop reasons as literals: the offscreen document loads it
/// without `protocol.js`, so it cannot take them from there.
const RECORDING_JS: &str = "lib/recording.js";
/// What precedes each stop-reason literal in `recording.js`.
const STOP_REASON_LITERALS: [&str; 3] = ["freeze(state, \"", "reason === \"", "reason = \""];
/// The receipt fixture of a dialog status with no dialog open, which names the unknown type.
const NO_DIALOG_RECEIPT: &str = "receipts/dialog.ok_absent.json";
/// The file name suffix of every fixture.
const JSON_SUFFIX: &str = ".json";
/// The case-name prefix of a receipt fixture that carries an outcome.
const OK_CASE: &str = "ok";
/// The primitive name of the receipt fixture for a primitive the extension does not know.
const UNKNOWN_PRIMITIVE: &str = "unknown_primitive";

/// Frame kinds whose fixtures have their own sections: requests, receipts and events.
const SECTIONED_KINDS: [&str; 3] = ["request", "receipt", "event"];

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
    let name = PrimitiveName::ALL
        .into_iter()
        .find(|name| name.as_str() == primitive)
        .unwrap_or_else(|| panic!("no primitive {primitive}"));
    match name {
        PrimitiveName::ListTabs => through::<<wire::ListTabs as Spec>::Outcome>(primitive, ok),
        PrimitiveName::OpenTab => through::<<wire::OpenTab as Spec>::Outcome>(primitive, ok),
        PrimitiveName::Navigate => through::<<wire::Navigate as Spec>::Outcome>(primitive, ok),
        PrimitiveName::FocusTab => through::<<FocusTab as Spec>::Outcome>(primitive, ok),
        PrimitiveName::CloseTab => through::<<wire::CloseTab as Spec>::Outcome>(primitive, ok),
        PrimitiveName::SetZoom => through::<<wire::SetZoom as Spec>::Outcome>(primitive, ok),
        PrimitiveName::ResizeWindow => {
            through::<<wire::ResizeWindow as Spec>::Outcome>(primitive, ok)
        }
        PrimitiveName::Frames => through::<<Frames as Spec>::Outcome>(primitive, ok),
        PrimitiveName::Call => through::<<WorldCalls as Spec>::Outcome>(primitive, ok),
        PrimitiveName::EvaluateMain => {
            through::<<wire::EvaluateMain as Spec>::Outcome>(primitive, ok)
        }
        PrimitiveName::View => through::<<View as Spec>::Outcome>(primitive, ok),
        PrimitiveName::Input => through::<<wire::Input as Spec>::Outcome>(primitive, ok),
        PrimitiveName::Capture => through::<<wire::Capture as Spec>::Outcome>(primitive, ok),
        PrimitiveName::Dialog => through::<<wire::Dialog as Spec>::Outcome>(primitive, ok),
        PrimitiveName::Diagnostics => {
            through::<<wire::DiagnosticsOp as Spec>::Outcome>(primitive, ok)
        }
        PrimitiveName::Recording => through::<<wire::RecordingOp as Spec>::Outcome>(primitive, ok),
        PrimitiveName::AccessibilitySnapshot => {
            through::<<wire::AccessibilitySnapshot as Spec>::Outcome>(primitive, ok)
        }
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
        set(PrimitiveName::ALL.map(PrimitiveName::as_str)),
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
                let (_, ok) = typed_receipt(PrimitiveName::Call.as_str(), frame);
                let called: wire::Called = decode(&stem, &ok.expect("ok"));
                assert_eq!(called.answers, vec![CallAnswer::Unknown]);
            }
            "input.unknown_stop_reason" => {
                let (_, ok) = typed_receipt(PrimitiveName::Input.as_str(), frame);
                let done: wire::InputDone = decode(&stem, &ok.expect("ok"));
                assert_eq!(done.stopped, Some(InputStop::Other));
            }
            "tab.unknown_readiness" => {
                let (_, ok) = typed_receipt(PrimitiveName::Navigate.as_str(), frame);
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
                    typed_receipt(PrimitiveName::Navigate.as_str(), frame),
                    typed_receipt(PrimitiveName::Navigate.as_str(), &target)
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
    check(
        "primitives",
        set(PrimitiveName::ALL.map(PrimitiveName::as_str)),
    );
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
    check(
        "world_message_kinds",
        set(WorldKind::ALL.map(WorldKind::as_str)),
    );

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
    assert_eq!(
        vocabulary["group_title_prefix"],
        crate::language::provenance::GROUP_TITLE_PREFIX
    );
    assert_eq!(limits["direct_frame_bytes"], wire::DIRECT_FRAME_BYTES);
    assert_eq!(limits["max_world_calls"], wire::MAX_WORLD_CALLS);
    assert_eq!(limits["max_input_steps"], wire::MAX_INPUT_STEPS);
    assert_eq!(limits["max_unseen"], wire::MAX_UNSEEN);
}

/// The extension refuses a longer group title, and the agent it names could open no tab.
#[test]
fn the_longest_group_title_fits_the_extensions_limit() {
    let longest = crate::language::provenance::group_title(&crate::agent::Agent::of(
        Some(&"\u{1F600}".repeat(200)),
        crate::intake::session::IntakeChannel::Mcp,
    ));
    let limit = vocabulary()["group_title_chars"]
        .as_u64()
        .and_then(|limit| usize::try_from(limit).ok())
        .expect("the group title limit");
    let characters = longest.chars().count();
    assert!(characters <= limit, "{characters} > {limit}");
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

/// The quoted word that follows each occurrence of `marker`.
fn quoted_after(source: &str, marker: &str) -> BTreeSet<String> {
    source
        .split(marker)
        .skip(1)
        .map(|rest| rest.split('"').next().expect("a word").to_owned())
        .collect()
}

#[test]
fn content_js_dispatches_exactly_the_kinds_the_service_builds() {
    let dispatched = quoted_after(CONTENT_JS, DISPATCH_TEST);
    assert_eq!(dispatched, set(WorldKind::ALL.map(WorldKind::as_str)));
}

#[test]
fn every_presentation_name_the_service_sends_is_one_the_page_draws() {
    let kinds = set(names!(PresentationKind: Start, Target, Progress, Completion, Denial));
    assert_eq!(
        quoted_between(PRESENTATION_JS, SIGNALS_OPEN, TABLE_CLOSE),
        kinds
    );
    let activities = set(
        names!(PresentationActivity: Quiet, Navigate, Click, Hover, Drag, Type, Key, Scroll,
        Read, Find, Screenshot, Zoom, Fill, Upload, Script, Wait, Dialog),
    );
    assert_eq!(
        quoted_between(PRESENTATION_JS, ACTIVITIES_OPEN, TABLE_CLOSE),
        activities
    );
}

#[test]
fn every_area_and_tier_the_page_names_is_one_the_service_decodes() {
    assert_eq!(
        quoted_between(CONTENT_JS, AREAS_OPEN, LIST_CLOSE),
        set(PageArea::ALL.map(PageArea::word))
    );
    assert_eq!(
        quoted_between(CONTENT_JS, LEFT_OUT_OPEN, TABLE_CLOSE),
        set(LeftOutKey::ALL.map(LeftOutKey::word))
    );
}

/// One packaged extension source.
fn extension(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(EXTENSION)
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"))
}

#[test]
fn every_stop_reason_the_extension_sends_is_one_the_service_decodes() {
    let unknown = name(&RecordingStopReason::Unknown);
    let mut known = set(
        names!(RecordingStopReason: DocumentBoundary, Explicit, HardTimeout, MemoryLimit,
        BrowserDetached, RuntimeHeld, ServiceDisconnected, FrameTooLarge, Unknown),
    );
    known.remove(&unknown);
    let listed = listed(&vocabulary(), "recording_stop_reasons");
    assert_eq!(listed, known, "vocabulary.recording_stop_reasons");
    let recording = extension(RECORDING_JS);
    let mut literals = BTreeSet::new();
    for marker in STOP_REASON_LITERALS {
        literals.extend(quoted_after(&recording, marker));
    }
    assert!(!literals.is_empty(), "recording.js names its stop reasons");
    assert!(
        literals.is_subset(&known),
        "{literals:?} is not within {known:?}"
    );
}

#[test]
fn the_runtime_states_the_extension_reports_are_the_services_and_disconnected() {
    let vocabulary = vocabulary();
    let states = listed(&vocabulary, "runtime_states");
    let controlled = set(names!(RuntimeControlState: Active, Held, Ended));
    assert_eq!(controlled, listed(&vocabulary, "control_states"));
    let lost: BTreeSet<String> = states.difference(&controlled).cloned().collect();
    assert!(controlled.is_subset(&states), "{states:?}");
    assert_eq!(lost.len(), 1, "one state beyond the service's: {lost:?}");
    let read = quoted_between(PRESENTATION_JS, RUNTIME_READ_OPEN, FUNCTION_CLOSE);
    assert!(
        read.is_superset(&lost) && read.is_subset(&states),
        "{read:?}"
    );
}

#[test]
fn the_extensions_chunk_limits_equal_the_services() {
    let limits = &vocabulary()["chunk_limits"];
    assert_eq!(limits["chunk_payload_bytes"], wire::CHUNK_PAYLOAD_BYTES);
    assert_eq!(limits["transfer_max_bytes"], wire::TRANSFER_MAX_BYTES);
    assert_eq!(limits["transfer_max_chunks"], wire::TRANSFER_MAX_CHUNKS);
}

#[test]
fn the_unknown_dialog_type_is_the_one_the_receipts_carry() {
    assert_eq!(
        vocabulary()["unknown_dialog_type"],
        read(NO_DIALOG_RECEIPT)["ok"]["dialog_type"]
    );
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
    let kind = WorldKind::ALL
        .into_iter()
        .find(|kind| kind.as_str() == kind_name)
        .unwrap_or_else(|| panic!("no world kind {kind_name}"));
    let message = match kind {
        WorldKind::ReadText => {
            world::read_text(optional("locator"), optional("mode"), size("max_chars"))
        }
        WorldKind::Inspect => world::inspect(text("inspect_kind"), size("max_items")),
        WorldKind::InspectTree => {
            world::inspect_tree(optional("locator"), size("max_depth"), size("max_nodes"))
        }
        WorldKind::Find => world::find(text("text"), text("find_kind"), size("max_results")),
        WorldKind::QuerySemantic => world::query_semantic(
            text("name"),
            optional("role"),
            flag("exact"),
            flag("form_scope"),
            size("max_results"),
        ),
        WorldKind::Describe => world::describe(&decode::<Vec<String>>("locators", &f["locators"])),
        WorldKind::DescribeFocused => world::describe_focused(),
        WorldKind::VerifyFocusedText => world::verify_focused_text(credentials()),
        WorldKind::Box => world::box_(text("locator")),
        WorldKind::PrepareFill => world::prepare_fill(
            &fields(&f["fields"]),
            credentials(),
            optional("submit_locator"),
        ),
        WorldKind::VerifyFillRetained => {
            world::verify_fill_retained(&fields(&f["fields"]), credentials(), count("limit_ms"))
        }
        WorldKind::Observe => world::observe(
            text("condition"),
            optional("value"),
            optional("locator"),
            count("timeout_ms"),
            flag("until_satisfied"),
        ),
        WorldKind::Activate => world::activate(text("locator")),
        WorldKind::Hover => world::hover(text("locator")),
        WorldKind::Focus => world::focus(text("locator")),
        WorldKind::ScrollPoint => world::scroll_point(number("x"), number("y")),
        WorldKind::ViewportPoint => world::viewport_point(number("x"), number("y")),
        WorldKind::ScrollOffset => world::scroll_offset(),
        WorldKind::HitPoint => world::hit_point(number("x"), number("y"), flag("page")),
        WorldKind::OwnerBox => world::owner_box(),
        WorldKind::OwnerProbe => world::owner_probe(number("x"), number("y")),
        WorldKind::FocusProbe => world::focus_probe(),
        WorldKind::RequireFocused => world::require_focused(),
        WorldKind::FillLocal => world::fill_local(&field(), credentials()),
        WorldKind::FocusTextField => world::focus_text_field(&field(), credentials()),
        WorldKind::SubmitFill => {
            world::submit_fill(&fields(&f["fields"]), text("submit_locator"), credentials())
        }
        WorldKind::TypeText => world::type_text(
            text("locator"),
            text("text"),
            flag("clear_first"),
            credentials(),
        ),
        WorldKind::ClearFocused => world::clear_focused(credentials()),
        WorldKind::Scroll => world::scroll(
            optional("locator"),
            optional("direction"),
            optional("amount"),
        ),
        WorldKind::UploadFiles => world::upload_files(text("locator"), &files(&f["files"])),
        WorldKind::DropFiles => world::drop_files(number("x"), number("y"), &files(&f["files"])),
        WorldKind::AttachFile => world::attach_file(text("locator"), text("file_name")),
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
        assert_eq!(message.kind().as_str(), stem);
        assert_eq!(
            normalized(&message.to_json()),
            normalized(&fixture),
            "{stem}"
        );
        seen.insert(stem);
    }
    assert_eq!(
        seen,
        set(WorldKind::ALL.map(WorldKind::as_str)),
        "every world kind has a fixture"
    );
}
