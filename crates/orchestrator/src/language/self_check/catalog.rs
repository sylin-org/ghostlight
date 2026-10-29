//! Every advertised input schema accepts its own examples and ordinary calls, exactly once.
//!
//! A strict client validates arguments against the advertised `inputSchema` before Ghostlight
//! sees them, and some client adapters convert `oneOf` strictly. So each example and each
//! known-good call below must satisfy the schema, must not match any `oneOf` twice, and must
//! decode through the real decoder.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::golden::{check_golden, golden_text, CATALOG_GOLDEN};
use super::schema::{is_valid, violations};
use crate::language::bounds::{
    FIELD_CHOICES, FIELD_VALUE_CHARS, HANDLE_CHARS, MAX_POSTCONDITION_VALUE_CHARS,
};
use crate::language::catalog::EXAMPLE_UPLOAD_PATH;
use crate::language::wait::EXPECT_REFUSAL;
use crate::language::{catalog, decode, HandleKind, WaitCondition, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS};

/// An absolute path the decoder accepts on the platform the tests run on.
#[cfg(windows)]
const ABSOLUTE_PATH: &str = "C:\\Users\\me\\report.pdf";
/// An absolute path the decoder accepts on the platform the tests run on.
#[cfg(not(windows))]
const ABSOLUTE_PATH: &str = "/home/me/report.pdf";

/// The upload example path as `docs/1.0/catalog.json` spells it, on every platform.
///
/// The catalog advertises a path the service's own platform accepts (`EXAMPLE_UPLOAD_PATH`), so
/// its text differs by operating system. The golden is one file, written on Windows, so the
/// comparison and the update both use this form.
const CANONICAL_UPLOAD_PATH: &str = "C:\\Users\\me\\notes.txt";

/// Ordinary calls a model makes, at least two per tool (one for a tool that takes no input).
///
/// The overlapping cases from MSURF-1 are here on purpose: typing ordinary and empty text,
/// `clear_first`, text and target waits, page and wheel scroll directions, and `full_page:false`.
fn known_good_calls() -> Vec<(&'static str, Value)> {
    vec![
        ("browser_tabs", json!({"action":"list"})),
        ("browser_tabs", json!({"action":"focus","tab":"tab_1"})),
        ("browser_tabs", json!({"action":"close","tab":"tab_1"})),
        ("browser_navigate", json!({"url":"https://example.com"})),
        (
            "browser_navigate",
            json!({"url":"https://example.com/a","tab":"tab_1"}),
        ),
        (
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true}),
        ),
        (
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":false,"reuse":"never"}),
        ),
        (
            "browser_navigate",
            json!({"url":"https://example.com","new_tab":true,"browser":"browser_1","timeout_ms":5000}),
        ),
        (
            "browser_navigate",
            json!({"url":"http://example.com","beforeunload":"discard"}),
        ),
        ("browser_history", json!({"action":"back"})),
        ("browser_history", json!({"action":"forward","tab":"tab_1"})),
        (
            "browser_history",
            json!({"action":"reload","bypass_cache":true,"timeout_ms":3000}),
        ),
        ("browser_window", json!({"action":"zoom","percent":150})),
        (
            "browser_window",
            json!({"action":"resize","width":1280,"height":800,"tab":"tab_1"}),
        ),
        ("browser_read", json!({})),
        (
            "browser_read",
            json!({"max_chars":2000,"visual_settle":false}),
        ),
        ("browser_read", json!({"target":"target_1"})),
        ("browser_read", json!({"mode":"visible","tab":"tab_1"})),
        ("browser_inspect", json!({})),
        (
            "browser_inspect",
            json!({"scope":"controls","max_items":20}),
        ),
        (
            "browser_inspect",
            json!({"scope":"document","root":"target_1","max_depth":3}),
        ),
        ("browser_find", json!({"text":"Sign in"})),
        (
            "browser_find",
            json!({"text":"Next","scope":"control","max_results":5}),
        ),
        ("browser_screenshot", json!({})),
        ("browser_screenshot", json!({"full_page":false})),
        (
            "browser_screenshot",
            json!({"full_page":true,"visual_settle":false}),
        ),
        ("browser_screenshot", json!({"target":"target_1"})),
        (
            "browser_screenshot",
            json!({"view":"view_1","x":0,"y":0,"width":100,"height":50.5}),
        ),
        ("browser_click", json!({"target":"target_1"})),
        (
            "browser_click",
            json!({"selector":{"name":"Save","role":"button","exact":true}}),
        ),
        (
            "browser_click",
            json!({"view":"view_1","x":10,"y":20.5,"button":"secondary","click_count":2,"modifiers":["Shift"]}),
        ),
        (
            "browser_click",
            json!({"target":"target_1","expect":{"condition":"url_contains","value":"/done"}}),
        ),
        ("browser_scroll", json!({})),
        ("browser_scroll", json!({"direction":"up"})),
        (
            "browser_scroll",
            json!({"direction":"down","amount":"page"}),
        ),
        (
            "browser_scroll",
            json!({"direction":"left","amount":"small"}),
        ),
        ("browser_scroll", json!({"target":"target_1"})),
        (
            "browser_scroll",
            json!({"view":"view_1","x":5,"y":5,"direction":"up","ticks":2}),
        ),
        ("browser_hover", json!({"target":"target_1"})),
        ("browser_hover", json!({"view":"view_1","x":1.5,"y":2})),
        (
            "browser_fill_form",
            json!({"fields":[{"target":"target_1","value":"Ada"}]}),
        ),
        (
            "browser_fill_form",
            json!({"fields":[{"target":"target_1","value":""}]}),
        ),
        (
            "browser_fill_form",
            json!({"fields":[
            {"selector":{"name":"Email"},"value":"ada@example.com"},
            {"target":"target_2","value":true},
            {"target":"target_3","value":3}
        ],"submit_target":"target_9","user_authorized_credentials":true}),
        ),
        (
            "browser_type_text",
            json!({"target":"target_1","text":"hello"}),
        ),
        (
            "browser_type_text",
            json!({"target":"target_1","text":"","clear_first":true}),
        ),
        (
            "browser_type_text",
            json!({"target":"target_1","text":"hello","clear_first":true}),
        ),
        (
            "browser_type_text",
            json!({"target":"target_1","text":"hello","clear_first":false}),
        ),
        ("browser_type_text", json!({"focused":true,"text":"hello"})),
        (
            "browser_type_text",
            json!({"focused":true,"text":"","clear_first":true}),
        ),
        (
            "browser_type_text",
            json!({"selector":{"name":"Search"},"text":"query"}),
        ),
        ("browser_press_key", json!({"key":"Enter"})),
        (
            "browser_press_key",
            json!({"key":"a","modifiers":["Control"]}),
        ),
        (
            "browser_press_key",
            json!({"key":"Escape","target":"target_1"}),
        ),
        (
            "browser_press_key",
            json!({"strokes":["Tab","Tab","Enter"]}),
        ),
        (
            "browser_press_key",
            json!({"strokes":["a","c"],"modifiers":["Control"]}),
        ),
        (
            "browser_press_key",
            json!({"strokes":["ArrowDown"],"repeat":3}),
        ),
        (
            "browser_drag",
            json!({"source_target":"target_1","destination_target":"target_2"}),
        ),
        (
            "browser_drag",
            json!({"view":"view_1","start_x":1,"start_y":2,"end_x":30,"end_y":40}),
        ),
        ("browser_wait", json!({"condition":"load_ready"})),
        (
            "browser_wait",
            json!({"condition":"load_ready","visual_settle":false,"timeout_ms":2000}),
        ),
        (
            "browser_wait",
            json!({"condition":"url_contains","value":"/done"}),
        ),
        (
            "browser_wait",
            json!({"condition":"text_present","value":"Ready"}),
        ),
        (
            "browser_wait",
            json!({"condition":"text_present","value":"Ready","visual_settle":true}),
        ),
        (
            "browser_wait",
            json!({"condition":"text_absent","value":"Loading"}),
        ),
        (
            "browser_wait",
            json!({"condition":"target_present","target":"target_1"}),
        ),
        (
            "browser_wait",
            json!({"condition":"target_absent","target":"target_1"}),
        ),
        ("browser_wait", json!({"condition":"visual_settle"})),
        (
            "browser_wait",
            json!({"condition":"visual_settle","target":"target_1"}),
        ),
        (
            "browser_wait",
            json!({"condition":"selector_present","selector":{"name":"Done"}}),
        ),
        (
            "browser_wait",
            json!({"condition":"duration","value":"1200"}),
        ),
        ("browser_dialog", json!({"action":"status"})),
        ("browser_dialog", json!({"action":"accept"})),
        ("browser_dialog", json!({"action":"dismiss","tab":"tab_1"})),
        ("browser_dialog", json!({"action":"respond","text":""})),
        ("browser_dialog", json!({"action":"respond","text":"yes"})),
        (
            "browser_upload",
            json!({"target":"target_1","paths":[ABSOLUTE_PATH]}),
        ),
        (
            "browser_upload",
            json!({"selector":{"name":"Attach"},"files":[{"name":"a.txt","data_base64":"SGk="}]}),
        ),
        (
            "browser_upload",
            json!({"source_image":"image_1","target":"target_1"}),
        ),
        (
            "browser_upload",
            json!({"source_image":"image_1","view":"view_1","x":1,"y":2}),
        ),
        ("browser_execute", json!({"script":"document.title"})),
        (
            "browser_execute",
            json!({"script":"return 1","max_result_chars":200,"timeout_ms":1000}),
        ),
        ("browser_flow", json!({"steps":[{"tool":"browser_read"}]})),
        (
            "browser_flow",
            json!({"steps":[
            {"id":"find","tool":"browser_find","arguments":{"text":"Next"}},
            {"tool":"browser_click","arguments":{"target":{"flow_ref":{"step":"find","pointer":"/facts/matches/0/target"}}}}
        ],"on_error":"continue","tab":"tab_1"}),
        ),
        ("browser_record", json!({"action":"start"})),
        ("browser_record", json!({"action":"status"})),
        ("browser_record", json!({"action":"stop"})),
        ("browser_record", json!({"action":"save","download":true})),
        (
            "browser_record",
            json!({"action":"save","target":"target_1"}),
        ),
        ("browser_record", json!({"action":"discard"})),
        (
            "browser_record",
            json!({"action":"status","timeout_ms":5000}),
        ),
        ("browser_diagnose", json!({})),
        (
            "browser_diagnose",
            json!({"source":"network","detail":"all","match":"api","after":"diag_1","limit":10}),
        ),
        ("policy_explain", json!({})),
        ("policy_explain", json!({"timeout_ms":1000})),
        ("browser_tabs", json!({"action":"list","timeout_ms":2000})),
        ("browser_read", json!({"timeout_ms":15000})),
        ("browser_find", json!({"text":"Save","timeout_ms":15000})),
        (
            "browser_dialog",
            json!({"action":"status","timeout_ms":500}),
        ),
    ]
}

/// The advertised input schema of one catalog tool.
fn advertised(name: &str) -> Value {
    catalog()
        .into_iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("{name} is not in the catalog"))
        .input_schema
}

/// Assert one call satisfies its tool's advertised schema and decodes.
fn assert_accepted(tool: &str, call: &Value) {
    let found = violations(&advertised(tool), call);
    assert!(found.is_empty(), "{tool} schema rejects {call}: {found:?}");
    if let Err(error) = decode(tool, call.clone()) {
        panic!("{tool} decoder rejects {call}: {error}");
    }
}

/// The first example a tool advertises, the call the bound rows change one field of.
fn first_example(tool: &str) -> Value {
    advertised(tool)["examples"][0].clone()
}

/// `call` with `field` set to `value`.
fn with(mut call: Value, field: &str, value: Value) -> Value {
    call[field] = value;
    call
}

/// The tools that advertise an action's `expect`.
fn expecting_tools() -> Vec<String> {
    catalog()
        .into_iter()
        .filter(|tool| tool.input_schema["properties"].get("expect").is_some())
        .map(|tool| tool.name)
        .collect()
}

/// A fill of one target field with `value`.
fn fill(value: Value) -> Value {
    json!({"fields":[{"target":"target_1","value":value}]})
}

/// A target handle `chars` characters long.
fn target_handle(chars: usize) -> String {
    let prefix = HandleKind::Target.prefix();
    format!("{prefix}{}", "x".repeat(chars - prefix.len()))
}

/// Calls at an advertised bound, built from the bounds the decoder enforces: every tool's
/// shortest and longest `timeout_ms`, the longest `expect` value, a fill list of the most
/// choices and its longest item, and the longest handle. Each must be accepted, so a schema
/// bound tighter than the decoder's fails here, and one looser fails the invalid-call rows.
fn boundary_calls() -> Vec<(String, Value)> {
    let mut calls = Vec::new();
    for tool in catalog() {
        for ms in [MIN_TIMEOUT_MS, MAX_TIMEOUT_MS] {
            calls.push((
                tool.name.clone(),
                with(first_example(&tool.name), "timeout_ms", json!(ms)),
            ));
        }
    }
    for tool in expecting_tools() {
        let expect =
            json!({"condition":"text_present","value":"x".repeat(MAX_POSTCONDITION_VALUE_CHARS)});
        calls.push((tool.clone(), with(first_example(&tool), "expect", expect)));
    }
    calls.push((
        "browser_fill_form".into(),
        fill(json!(vec!["x"; FIELD_CHOICES])),
    ));
    calls.push((
        "browser_fill_form".into(),
        fill(json!(["x".repeat(FIELD_VALUE_CHARS)])),
    ));
    calls.push((
        "browser_read".into(),
        json!({"target": target_handle(HANDLE_CHARS)}),
    ));
    calls
}

/// Every call the gate-2 checks read: the known-good calls, the calls at an advertised bound and
/// every example each tool advertises.
fn every_call() -> Vec<(String, Value)> {
    let mut calls: Vec<(String, Value)> = known_good_calls()
        .into_iter()
        .map(|(tool, call)| (tool.to_owned(), call))
        .collect();
    calls.extend(boundary_calls());
    for tool in catalog() {
        for example in tool.input_schema["examples"]
            .as_array()
            .into_iter()
            .flatten()
        {
            calls.push((tool.name.clone(), example.clone()));
        }
    }
    calls
}

/// `call` with every advertised default filled in for the fields it leaves out, as a client that
/// applies schema defaults sends it.
fn with_defaults(schema: &Value, call: &Value) -> Value {
    let mut filled = call.clone();
    if let (Some(properties), Some(object)) =
        (schema["properties"].as_object(), filled.as_object_mut())
    {
        for (name, property) in properties {
            if let Some(default) = property.get("default") {
                object
                    .entry(name.clone())
                    .or_insert_with(|| default.clone());
            }
        }
    }
    filled
}

/// Walk a value alongside its schema and count, for every `oneOf` the value reaches, how many
/// branches it matches.
fn one_of_matches(schema: &Value, value: &Value, at: &str, seen: &mut Vec<(String, usize)>) {
    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        let matched = branches
            .iter()
            .filter(|branch| is_valid(branch, value))
            .count();
        seen.push((at.to_owned(), matched));
    }
    if let (Some(properties), Some(object)) = (
        schema.get("properties").and_then(Value::as_object),
        value.as_object(),
    ) {
        for (name, field) in object {
            if let Some(field_schema) = properties.get(name) {
                one_of_matches(field_schema, field, &format!("{at}/{name}"), seen);
            }
        }
    }
    if let (Some(items), Some(values)) = (schema.get("items"), value.as_array()) {
        for item in values {
            one_of_matches(items, item, &format!("{at}/items"), seen);
        }
    }
}

/// Every place a schema uses `oneOf`, as a path through properties and items.
fn one_of_locations(schema: &Value, at: &str, found: &mut BTreeSet<String>) {
    if schema.get("oneOf").is_some() {
        found.insert(at.to_owned());
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (name, field_schema) in properties {
            one_of_locations(field_schema, &format!("{at}/{name}"), found);
        }
    }
    if let Some(items) = schema.get("items") {
        one_of_locations(items, &format!("{at}/items"), found);
    }
}

/// Call `visit` on every subschema of `schema`, including itself.
fn each_subschema(schema: &Value, visit: &mut impl FnMut(&Value)) {
    visit(schema);
    if let Some(fields) = schema.get("properties").and_then(Value::as_object) {
        for field_schema in fields.values() {
            each_subschema(field_schema, visit);
        }
    }
    if let Some(items) = schema.get("items") {
        each_subschema(items, visit);
    }
    for keyword in ["oneOf", "anyOf"] {
        for branch in schema
            .get(keyword)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            each_subschema(branch, visit);
        }
    }
}

/// ACCEPTANCE contract gate 2 (MSURF-1, 0ddb34af): under a strict JSON Schema check, every
/// advertised example, at least two ordinary calls per tool that takes input, and a call at each
/// advertised bound satisfy the tool's own schema, match each `oneOf` they reach exactly once, and
/// decode. Every `oneOf` in the
/// catalog is reached by some call, and the checker flags the ambiguous schema MSURF-1 shipped, so
/// the check cannot pass vacuously.
#[test]
fn every_example_and_ordinary_call_is_accepted_exactly_once() {
    let ordinary = known_good_calls();
    for tool in catalog() {
        let examples = tool.input_schema["examples"]
            .as_array()
            .unwrap_or_else(|| panic!("{} advertises no examples", tool.name));
        assert!(!examples.is_empty(), "{} advertises no examples", tool.name);
        let count = ordinary
            .iter()
            .filter(|(name, _)| *name == tool.name)
            .count();
        let takes_input = tool.input_schema["properties"]
            .as_object()
            .is_some_and(|properties| !properties.is_empty());
        let least = if takes_input { 2 } else { 1 };
        assert!(count >= least, "{} has {count} known-good calls", tool.name);
    }

    let mut exercised = BTreeSet::new();
    for (tool, call) in &every_call() {
        assert_accepted(tool, call);
        let mut seen = Vec::new();
        one_of_matches(&advertised(tool), call, tool, &mut seen);
        for (at, matched) in seen {
            assert_eq!(
                matched, 1,
                "{call} matches {matched} oneOf branches at {at}"
            );
            exercised.insert(at);
        }
    }
    for tool in catalog() {
        let mut locations = BTreeSet::new();
        one_of_locations(&tool.input_schema, &tool.name, &mut locations);
        for location in locations {
            assert!(
                exercised.contains(&location),
                "no known-good call reaches the oneOf at {location}"
            );
        }
    }

    // The flattened `text` schema browser_type_text shipped before ADR-0196: every ordinary value
    // matched two branches, so a strict validator rejected ordinary typing.
    let shipped = json!({"oneOf":[
        {"type":"string","minLength":1,"maxLength":8000},
        {"type":"string","const":""},
        {"type":"string","minLength":0,"maxLength":8000}
    ]});
    for typed in [json!("hello"), json!("")] {
        let found = violations(&shipped, &typed);
        assert!(
            found
                .iter()
                .any(|violation| violation.contains("ambiguous oneOf")),
            "the MSURF-1 schema must be ambiguous for {typed}; got {found:?}"
        );
    }
}

/// Advertised defaults the decoder keeps unset, because a later step applies them: settlement
/// reads `visual_settle`, and the scroll executor applies its direction and amount.
const APPLIED_AFTER_DECODE: [&str; 3] = ["visual_settle", "direction", "amount"];

/// A default means the same as leaving its field out (b79f4024: Claude Code fills in advertised
/// defaults, and browser_navigate new_tab:true with reuse:"domain" was refused; f54e2469). Every
/// example and ordinary call, with its advertised defaults filled in, satisfies the schema and
/// decodes to the same operation; the defaults applied after decoding agree with the step that
/// applies them; and every default in the catalog satisfies its own schema.
#[test]
fn an_advertised_default_means_the_same_as_leaving_the_field_out() {
    let mut refused = Vec::new();
    let mut drifted = Vec::new();
    for (tool, call) in &every_call() {
        let schema = advertised(tool);
        let mut filled = with_defaults(&schema, call);
        let full = match decode(tool, filled.clone()) {
            Ok(full) if violations(&schema, &filled).is_empty() => full,
            _ => {
                refused.push(format!("{tool} {filled}"));
                continue;
            }
        };
        let bare = decode(tool, call.clone()).expect("a known-good call decodes");
        for field in APPLIED_AFTER_DECODE {
            if call.get(field).is_none() {
                filled
                    .as_object_mut()
                    .expect("calls are objects")
                    .remove(field);
            }
        }
        if crate::language::settlement::enabled(&bare)
            != crate::language::settlement::enabled(&full)
        {
            drifted.push(format!("{tool} {call}: visual_settle"));
        }
        let partial = decode(tool, filled).expect("a call with some defaults decodes");
        if format!("{bare:?}") != format!("{partial:?}") {
            drifted.push(format!(
                "{tool} {call}:\n  omitted {bare:?}\n  filled  {partial:?}"
            ));
        }
    }
    assert!(
        refused.is_empty(),
        "refused with defaults filled in: {refused:#?}"
    );
    assert!(drifted.is_empty(), "defaults that drifted: {drifted:#?}");

    // A page scroll with no direction lets the page choose (ADR-0205 Decision 3), so the
    // catalog advertises no direction default.
    let scroll = advertised("browser_scroll");
    assert!(scroll["properties"]["direction"].get("default").is_none());
    assert_eq!(
        scroll["properties"]["amount"]["default"],
        crate::language::SCROLL_AMOUNT_DEFAULT.as_str()
    );

    for tool in catalog() {
        each_subschema(&tool.input_schema, &mut |schema| {
            if let Some(default) = schema.get("default") {
                let found = violations(schema, default);
                assert!(
                    found.is_empty(),
                    "{} default {default} breaks its own schema: {found:?}",
                    tool.name
                );
            }
        });
    }
}

/// What the decoder's refusal of an invalid call says.
enum Says {
    /// A refusal; its words are not checked.
    Refused,
    /// Exactly this refusal.
    Exactly(String),
    /// A refusal that names this word.
    Naming(String),
}

/// One invalid call: the tool, the call, the keyword the advertised schema reports for it, and
/// what the decoder's refusal says. The keyword is none for a call the one wide envelope a tool
/// advertises admits, which only the decoder refuses.
struct Invalid {
    tool: String,
    call: Value,
    schema: Option<&'static str>,
    says: Says,
}

fn invalid(tool: &str, call: Value, schema: Option<&'static str>, says: Says) -> Invalid {
    Invalid {
        tool: tool.to_owned(),
        call,
        schema,
        says,
    }
}

/// The words a closed field advertises at `pointer` in `tool`'s schema, as a refusal lists them.
fn advertised_words(tool: &str, pointer: &str) -> Vec<String> {
    let schema = advertised(tool);
    let field = schema
        .pointer(pointer)
        .unwrap_or_else(|| panic!("{tool} advertises nothing at {pointer}"));
    field["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("{tool} {pointer} is not a closed field"))
        .iter()
        .map(|word| word.as_str().expect("a closed word is text").to_owned())
        .collect()
}

/// The refusal of `bad` for the closed field `field`, listing the words the catalog advertises
/// for it at `pointer`.
fn refused_word(tool: &str, pointer: &str, field: &str, bad: &str) -> Says {
    Says::Exactly(format!(
        "invalid input: {field} must be one of {}: got `{bad}`",
        advertised_words(tool, pointer).join(", ")
    ))
}

/// The refusal of `value` for the whole-number field `field`, naming the bounds the catalog
/// advertises for it at `pointer`.
fn refused_number(tool: &str, pointer: &str, field: &str, value: u64) -> Says {
    let schema = advertised(tool);
    let bounds = schema
        .pointer(pointer)
        .unwrap_or_else(|| panic!("{tool} advertises nothing at {pointer}"));
    Says::Exactly(format!(
        "invalid input: {field} must be between {} and {}; got {value}",
        bounds["minimum"], bounds["maximum"]
    ))
}

/// The refusal of a field the tool's schema does not advertise, at the top level or inside a
/// nested object (L-D7: a nested unknown field is refused, never dropped). A top-level one also
/// names the fields the tool takes (ADR-0205 Decision 6).
fn unknown_field(field: &str) -> Says {
    Says::Naming(format!("unknown field `{field}`"))
}

/// Every invalid call the gate-2 refusal check sends.
fn invalid_calls() -> Vec<Invalid> {
    let mut calls = vec![
        // L-D7 (4b1cdea0): a nested object the schema closes is closed in the decoder too, and
        // a field it does not name is refused in the top level's words rather than dropped (a
        // dropped `role` widened what a click could land on).
        invalid(
            "browser_click",
            json!({"selector":{"name":"Save","rol":"button"}}),
            Some("unknown field `rol`"),
            unknown_field("rol"),
        ),
        invalid(
            "browser_click",
            json!({"target":"target_1","expect":{"condition":"load_ready","valu":"x"}}),
            Some("unknown field `valu`"),
            unknown_field("valu"),
        ),
        invalid(
            "browser_upload",
            json!({"target":"target_1","files":[{"name":"a.txt","data_base64":"SGk=","mime":"x"}]}),
            Some("unknown field `mime`"),
            unknown_field("mime"),
        ),
        invalid(
            "browser_fill_form",
            json!({"fields":[{"selector":{"name":"Email","exactly":true},"value":"a"}]}),
            Some("unknown field `exactly`"),
            unknown_field("exactly"),
        ),
        invalid(
            "browser_dialog",
            json!({"accept":true}),
            Some("unknown field `accept`"),
            unknown_field("accept"),
        ),
        invalid(
            "browser_record",
            json!({"action":"stop","recording":"recording_1"}),
            Some("unknown field `recording`"),
            unknown_field("recording"),
        ),
        // A closed word is refused with the words the catalog advertises for that field.
        invalid(
            "browser_tabs",
            json!({"action":"open"}),
            Some("is not one of"),
            refused_word("browser_tabs", "/properties/action", "action", "open"),
        ),
        invalid(
            "browser_history",
            json!({"action":"up"}),
            Some("is not one of"),
            refused_word("browser_history", "/properties/action", "action", "up"),
        ),
        invalid(
            "browser_window",
            json!({"action":"maximize"}),
            Some("is not one of"),
            refused_word("browser_window", "/properties/action", "action", "maximize"),
        ),
        invalid(
            "browser_navigate",
            json!({"url":"https://example.com","reuse":"sometimes"}),
            Some("is not one of"),
            refused_word(
                "browser_navigate",
                "/properties/reuse",
                "reuse",
                "sometimes",
            ),
        ),
        invalid(
            "browser_navigate",
            json!({"url":"https://example.com","beforeunload":"keep"}),
            Some("is not one of"),
            refused_word(
                "browser_navigate",
                "/properties/beforeunload",
                "beforeunload",
                "keep",
            ),
        ),
        invalid(
            "browser_inspect",
            json!({"scope":"links"}),
            Some("is not one of"),
            refused_word("browser_inspect", "/properties/scope", "scope", "links"),
        ),
        invalid(
            "browser_find",
            json!({"text":"Save","scope":"links"}),
            Some("is not one of"),
            refused_word("browser_find", "/properties/scope", "scope", "links"),
        ),
        invalid(
            "browser_click",
            json!({"target":"target_1","modifiers":["Hyper"]}),
            Some("is not one of"),
            refused_word(
                "browser_click",
                "/properties/modifiers/items",
                "modifiers",
                "Hyper",
            ),
        ),
        invalid(
            "browser_scroll",
            json!({"direction":"sideways"}),
            Some("is not one of"),
            refused_word(
                "browser_scroll",
                "/properties/direction",
                "direction",
                "sideways",
            ),
        ),
        invalid(
            "browser_scroll",
            json!({"amount":"huge"}),
            None,
            Says::Naming("amount must be".into()),
        ),
        invalid(
            "browser_click",
            json!({"target":"target_1","button":"back"}),
            Some("is not one of"),
            refused_word("browser_click", "/properties/button", "button", "back"),
        ),
        invalid(
            "browser_dialog",
            json!({"action":"close"}),
            Some("is not one of"),
            refused_word("browser_dialog", "/properties/action", "action", "close"),
        ),
        invalid(
            "browser_record",
            json!({"action":"pause"}),
            Some("is not one of"),
            refused_word("browser_record", "/properties/action", "action", "pause"),
        ),
        invalid(
            "browser_flow",
            json!({"steps":[{"tool":"browser_read"}],"on_error":"retry"}),
            Some("is not one of"),
            refused_word("browser_flow", "/properties/on_error", "on_error", "retry"),
        ),
        invalid(
            "browser_diagnose",
            json!({"source":"dom"}),
            Some("is not one of"),
            refused_word("browser_diagnose", "/properties/source", "source", "dom"),
        ),
        invalid(
            "browser_diagnose",
            json!({"detail":"some"}),
            Some("is not one of"),
            refused_word("browser_diagnose", "/properties/detail", "detail", "some"),
        ),
        // A key is free text the decoder reads (ADR-0205); one that names no key is refused with
        // the keys it takes.
        invalid(
            "browser_press_key",
            json!({"key":"PageLeft"}),
            None,
            Says::Naming("a supported named key: Enter".into()),
        ),
        // LANGUAGE.md browser_wait (C17a): an unknown condition is refused with the curated list,
        // which has never named selector_present, a condition a wait names with its selector; a
        // retired layout_stable points to visual_settle; and an expect that names a condition it
        // does not take lists the four it does.
        invalid(
            "browser_wait",
            json!({"condition":"never"}),
            Some("is not one of"),
            Says::Exactly(format!(
                "invalid input: condition must be one of {}: got `never`",
                advertised_words("browser_wait", "/properties/condition")
                    .into_iter()
                    .filter(|word| word != WaitCondition::SelectorPresent.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ),
        invalid(
            "browser_wait",
            json!({"condition":"layout_stable"}),
            Some("is not one of"),
            Says::Naming(WaitCondition::VisualSettle.as_str().into()),
        ),
        invalid(
            "browser_click",
            json!({"target":"target_1","expect":{"condition":"duration"}}),
            Some("is not one of"),
            Says::Exactly(format!("invalid input: {EXPECT_REFUSAL}")),
        ),
        // A refused number names the advertised bounds; a refused handle names its prefix.
        invalid(
            "browser_window",
            json!({"action":"zoom","percent":900}),
            Some("breaks maximum"),
            refused_number("browser_window", "/properties/percent", "percent", 900),
        ),
        invalid(
            "browser_tabs",
            json!({"action":"close","tab":"nope"}),
            Some("does not match"),
            Says::Naming(HandleKind::Tab.prefix().into()),
        ),
        invalid(
            "browser_type_text",
            json!({"target":"target_1"}),
            Some("missing required field `text`"),
            Says::Refused,
        ),
        invalid(
            "browser_navigate",
            json!({"url":"ftp://example.com"}),
            Some("does not match"),
            Says::Refused,
        ),
        invalid(
            "browser_press_key",
            json!({"strokes":["Control","a"]}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_click",
            json!({"target":"view_1"}),
            Some("does not match"),
            Says::Naming(HandleKind::Target.prefix().into()),
        ),
        invalid(
            "browser_press_key",
            json!({"key":"Enter","repeat":1.5}),
            Some("is not of type integer"),
            Says::Refused,
        ),
        invalid(
            "browser_find",
            json!({"text":""}),
            Some("breaks minLength"),
            Says::Refused,
        ),
        invalid(
            "browser_find",
            json!({"text":"a".repeat(501)}),
            Some("breaks maxLength"),
            Says::Refused,
        ),
        invalid(
            "browser_click",
            json!({"target":"target_1","click_count":4}),
            Some("breaks maximum"),
            Says::Refused,
        ),
        invalid(
            "browser_screenshot",
            json!({"view":"view_1","x":0,"y":0,"width":0,"height":10}),
            Some("breaks exclusiveMinimum"),
            Says::Refused,
        ),
        invalid(
            "browser_fill_form",
            json!({"fields":[]}),
            Some("break minItems"),
            Says::Refused,
        ),
        invalid(
            "browser_press_key",
            json!({"strokes":vec!["a"; 21]}),
            Some("break maxItems"),
            Says::Refused,
        ),
        invalid(
            "browser_fill_form",
            json!({"fields":[{"target":"target_1","value":{}}]}),
            Some("matches no anyOf branch"),
            Says::Refused,
        ),
        // One step past each bound the boundary calls sit on.
        invalid(
            "browser_fill_form",
            fill(json!(vec!["x"; FIELD_CHOICES + 1])),
            Some("matches no anyOf branch"),
            Says::Refused,
        ),
        invalid(
            "browser_fill_form",
            fill(json!(["x".repeat(FIELD_VALUE_CHARS + 1)])),
            Some("matches no anyOf branch"),
            Says::Refused,
        ),
        invalid(
            "browser_read",
            json!({"target": target_handle(HANDLE_CHARS + 1)}),
            Some("breaks maxLength"),
            Says::Refused,
        ),
        invalid(
            "browser_read",
            json!({"target":""}),
            Some("does not match"),
            Says::Refused,
        ),
        invalid(
            "browser_read",
            json!({"target": HandleKind::Target.prefix()}),
            Some("does not match"),
            Says::Refused,
        ),
        // The wide envelope admits these; the decoder refuses each one outside its branch.
        invalid(
            "browser_wait",
            json!({"condition":"text_present"}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_wait",
            json!({"condition":"target_present","value":"x"}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_screenshot",
            json!({"target":"target_1","full_page":true}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_screenshot",
            json!({"x":0,"y":0,"width":300,"height":200}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_screenshot",
            json!({"view":"view_1","x":0,"y":0,"width":300}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_screenshot",
            json!({"view":"view_1","x":0,"y":0,"width":300,"height":200,"full_page":true}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_screenshot",
            json!({"view":"view_1","x":0,"y":0,"width":300,"height":200,"target":"target_1"}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_click",
            json!({"target":"target_1","view":"view_1","x":1,"y":1}),
            None,
            Says::Refused,
        ),
        invalid("browser_click", json!({"x":1,"y":1}), None, Says::Refused),
        invalid(
            "browser_upload",
            json!({"target":"target_1","paths":["relative.txt"]}),
            None,
            Says::Refused,
        ),
        invalid(
            "browser_window",
            json!({"action":"zoom","width":800,"height":600}),
            None,
            Says::Refused,
        ),
    ];
    for tool in expecting_tools() {
        let expect = json!({"condition":"text_present","value":"x".repeat(MAX_POSTCONDITION_VALUE_CHARS + 1)});
        calls.push(invalid(
            &tool,
            with(first_example(&tool), "expect", expect),
            Some("breaks maxLength"),
            Says::Refused,
        ));
    }
    calls
}

/// What a strict client refuses is what Ghostlight refuses, and the refusal says what the field
/// takes in the catalog's own words (ACCEPTANCE gate 2: the schemas carry every decoder
/// requirement and conditional branch; LANGUAGE.md's contract rules). Each invalid call breaks one
/// keyword the catalog uses, or one branch the wide envelope cannot express: the advertised
/// schema reports that keyword, and the decoder refuses the call before dispatch. A refused word
/// lists the enum the catalog advertises for the field, a refused number its advertised bounds, a
/// refused handle its prefix, and an unknown field, nested ones included (L-D7, 4b1cdea0), is
/// named; every refusal carries guidance that is itself without its `invalid input: ` prefix, the
/// words a next step quotes. So the checks above are not vacuous.
#[test]
fn an_invalid_call_is_refused_before_dispatch_by_both_sides_in_the_catalogs_words() {
    let mut disagreements = Vec::new();
    for Invalid {
        tool,
        call,
        schema,
        says,
    } in invalid_calls()
    {
        if let Some(fragment) = schema {
            let found = violations(&advertised(&tool), &call);
            if !found.iter().any(|violation| violation.contains(fragment)) {
                disagreements.push(format!(
                    "{tool} {call}: schema says {found:?}, not {fragment:?}"
                ));
            }
        }
        let error = match decode(&tool, call.clone()) {
            Ok(operation) => {
                disagreements.push(format!("{tool} {call}: decodes to {operation:?}"));
                continue;
            }
            Err(error) => error,
        };
        let said = error.to_string();
        if said != format!("invalid input: {}", error.guidance()) {
            disagreements.push(format!("{tool} {call}: guidance {:?}", error.guidance()));
        }
        let expected = match &says {
            Says::Refused => true,
            Says::Exactly(refusal) => said == *refusal,
            Says::Naming(word) => said.contains(word.as_str()),
        };
        if !expected {
            let wanted = match says {
                Says::Refused => String::new(),
                Says::Exactly(refusal) => refusal,
                Says::Naming(word) => format!("naming {word}"),
            };
            disagreements.push(format!("{tool} {call}: refused {said:?}, not {wanted:?}"));
        }
    }
    assert!(disagreements.is_empty(), "{disagreements:#?}");
}

/// The most characters the `tools` array of `tools/list` may serialize to (ADR-0198 decision 26).
///
/// A commit that adds a behavior needing words raises this ceiling, and says so.
const CATALOG_BUDGET_CHARS: usize = 31_800;

/// Tools that carry the most guidance get their own cap on the serialized tool object, so one
/// tool cannot grow while the whole stays under budget.
const TOOL_BUDGET_CHARS: [(&str, usize); 5] = [
    ("browser_inspect", 1_300),
    ("browser_find", 950),
    ("browser_click", 2_300),
    ("browser_fill_form", 3_000),
    ("browser_read", 1_050),
];

/// Governance phrases the trim must keep. Each one says something no name, type or bound does.
const PINNED_PHRASES: [&str; 7] = [
    "for this request",
    "admitted before the effect",
    "may cause an external effect",
    "needs write even for unsent drafts",
    "screenshot image",
    "Omit for load_ready, else required",
    "refused before any field changes",
];

/// ADR-0198 Decision 26 and ACCEPTANCE gate 2: the catalog that tools/list serves stays under its
/// size budget, the tools that carry the most guidance stay under their own caps, and the trim
/// keeps the governance phrases the decision pins.
#[test]
fn the_catalog_trim_stays_under_budget_and_keeps_the_governance_phrases() {
    let tools = catalog();
    let listed = serde_json::to_string(&tools).expect("catalog serializes");
    assert!(
        listed.len() <= CATALOG_BUDGET_CHARS,
        "tools/list serializes to {} characters; the budget is {CATALOG_BUDGET_CHARS}",
        listed.len()
    );
    for (name, cap) in TOOL_BUDGET_CHARS {
        let tool = tools
            .iter()
            .find(|tool| tool.name == name)
            .unwrap_or_else(|| panic!("{name} is not in the catalog"));
        let size = serde_json::to_string(tool).expect("tool serializes").len();
        assert!(
            size <= cap,
            "{name} serializes to {size} characters; its cap is {cap}"
        );
    }
    for phrase in PINNED_PHRASES {
        assert!(listed.contains(phrase), "the catalog lost \"{phrase}\"");
    }
}

/// Contract: the catalog tools/list serves is docs/1.0/catalog.json, the golden LANGUAGE.md,
/// ACCEPTANCE, the integrity script and the process journey read (GHOSTLIGHT_CATALOG_UPDATE=1
/// rewrites it).
#[test]
fn the_catalog_matches_docs_1_0_catalog_json() {
    let platform = serde_json::to_string(EXAMPLE_UPLOAD_PATH).expect("a path encodes");
    let canonical = serde_json::to_string(CANONICAL_UPLOAD_PATH).expect("a path encodes");
    check_golden(
        CATALOG_GOLDEN,
        &golden_text(&catalog()).replace(&platform, &canonical),
    );
}
