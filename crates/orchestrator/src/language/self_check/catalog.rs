//! Every advertised input schema accepts its own examples and ordinary calls, exactly once.
//!
//! A strict client validates arguments against the advertised `inputSchema` before Ghostlight
//! sees them, and some client adapters convert `oneOf` strictly. So each example and each
//! known-good call below must satisfy the schema, must not match any `oneOf` twice, and must
//! decode through the real decoder.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::schema::{is_valid, violations};
use crate::intake::session::ToolDefinition;
use crate::language::{catalog, decode};

/// An absolute path the decoder accepts on the platform the tests run on.
#[cfg(windows)]
const ABSOLUTE_PATH: &str = "C:\\Users\\me\\report.pdf";
/// An absolute path the decoder accepts on the platform the tests run on.
#[cfg(not(windows))]
const ABSOLUTE_PATH: &str = "/home/me/report.pdf";

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

#[test]
fn every_example_satisfies_its_own_schema_and_decodes() {
    for tool in catalog() {
        let examples = tool.input_schema["examples"]
            .as_array()
            .unwrap_or_else(|| panic!("{} advertises no examples", tool.name));
        assert!(!examples.is_empty(), "{} advertises no examples", tool.name);
        for example in examples {
            assert_accepted(&tool.name, example);
        }
    }
}

#[test]
fn known_good_calls_satisfy_the_schema_and_decode() {
    let calls = known_good_calls();
    for tool in catalog() {
        let count = calls.iter().filter(|(name, _)| *name == tool.name).count();
        let takes_input = tool.input_schema["properties"]
            .as_object()
            .is_some_and(|properties| !properties.is_empty());
        let least = if takes_input { 2 } else { 1 };
        assert!(count >= least, "{} has {count} known-good calls", tool.name);
    }
    for (tool, call) in &calls {
        assert_accepted(tool, call);
    }
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

#[test]
fn no_one_of_matches_a_known_good_value_more_than_once() {
    let mut calls: Vec<(String, Value)> = known_good_calls()
        .into_iter()
        .map(|(tool, call)| (tool.to_owned(), call))
        .collect();
    for tool in catalog() {
        for example in tool.input_schema["examples"]
            .as_array()
            .into_iter()
            .flatten()
        {
            calls.push((tool.name.clone(), example.clone()));
        }
    }
    let mut exercised = BTreeSet::new();
    for (tool, call) in &calls {
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

#[test]
fn any_of_only_separates_different_types_and_defaults_are_valid() {
    for tool in catalog() {
        each_subschema(&tool.input_schema, &mut |schema| {
            if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
                let types: Vec<&Value> = branches.iter().filter_map(|b| b.get("type")).collect();
                let distinct: BTreeSet<String> = types.iter().map(ToString::to_string).collect();
                assert!(
                    types.len() == branches.len() && distinct.len() == types.len(),
                    "{} uses anyOf for same-typed alternatives: {schema}",
                    tool.name
                );
            }
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

/// Negative control: the same checker, against the real catalog, rejects calls a strict client
/// would reject, so the passing tests above are not vacuous.
#[test]
fn the_checker_rejects_invalid_calls_against_the_real_catalog() {
    let invalid = [
        ("browser_dialog", json!({"accept":true})),
        ("browser_tabs", json!({"action":"open"})),
        ("browser_wait", json!({"condition":"never"})),
        ("browser_type_text", json!({"target":"target_1"})),
        ("browser_scroll", json!({"direction":"sideways"})),
        ("browser_navigate", json!({"url":"ftp://example.com"})),
        ("browser_press_key", json!({"strokes":["Control","a"]})),
        ("browser_click", json!({"target":"view_1"})),
        ("browser_wait", json!({"condition":"layout_stable"})),
        (
            "browser_record",
            json!({"action":"stop","recording":"recording_1"}),
        ),
        ("browser_read", json!({"timeout_ms":99})),
    ];
    for (tool, call) in invalid {
        assert!(
            !violations(&advertised(tool), &call).is_empty(),
            "{tool} schema accepts invalid {call}"
        );
        assert!(decode(tool, call.clone()).is_err(), "{tool} decodes {call}");
    }
}

/// The tools `tools/list` returns for the complete catalog: the output schema stays out of the
/// listing, so it stays out of the measure.
fn listed_tools() -> Vec<ToolDefinition> {
    catalog()
        .into_iter()
        .map(|tool| ToolDefinition {
            output_schema: None,
            ..tool
        })
        .collect()
}

/// The most characters the `tools` array of `tools/list` may serialize to (ADR-0198 decision 26).
///
/// The budget ratchets down. A later change raises it only by the measured size of a clause its
/// new behavior needs, and the commit that raises it says so.
const CATALOG_BUDGET_CHARS: usize = 31_600;

/// Tools that carry the most guidance get their own cap on the serialized tool object, so one
/// tool cannot grow while the whole stays under budget.
const TOOL_BUDGET_CHARS: [(&str, usize); 5] = [
    ("browser_inspect", 1_250),
    ("browser_find", 950),
    ("browser_click", 2_200),
    ("browser_fill_form", 2_700),
    ("browser_read", 1_050),
];

/// Governance phrases the trim must keep. Each one says something no name, type or bound does.
const PINNED_PHRASES: [&str; 6] = [
    "for this request",
    "admitted before the effect",
    "may cause an external effect",
    "needs write even for unsent drafts",
    "screenshot image",
    "Omit for load_ready, else required",
];

#[test]
fn the_listed_catalog_stays_under_its_size_budget() {
    let tools = listed_tools();
    let size = serde_json::to_string(&tools)
        .expect("catalog serializes")
        .len();
    assert!(
        size <= CATALOG_BUDGET_CHARS,
        "tools/list serializes to {size} characters; the budget is {CATALOG_BUDGET_CHARS}"
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
}

#[test]
fn the_trim_keeps_the_governance_phrases() {
    let listed = serde_json::to_string(&listed_tools()).expect("catalog serializes");
    for phrase in PINNED_PHRASES {
        assert!(listed.contains(phrase), "the catalog lost \"{phrase}\"");
    }
}

/// These tools lead with the shortest call LANGUAGE.md documents for them: `{}` where every
/// field has a default, `{"key":"Enter"}` for `browser_press_key`.
#[test]
fn the_documented_shortest_calls_lead_their_examples() {
    let first = |name: &str| {
        listed_tools()
            .into_iter()
            .find(|tool| tool.name == name)
            .map(|tool| tool.input_schema["examples"][0].clone())
            .unwrap_or_else(|| panic!("{name} is not in the catalog"))
    };
    assert_eq!(first("browser_screenshot"), json!({}));
    assert_eq!(first("browser_scroll"), json!({}));
    assert_eq!(first("browser_press_key"), json!({"key":"Enter"}));
}
