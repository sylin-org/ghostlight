//! Fidelity guard for the sacred `tools/list` surface (`src/mcp/schemas/tools.json`).
//!
//! Ensures the embedded schema fixture stays intact: exactly the 13 trained tools, byte-identical
//! and in order, each with a non-empty description and an object inputSchema, PLUS exactly one
//! sanctioned addition: `explain` (ADR-0022 Decision 7), positioned last. This file was amended
//! ONCE, in stage-3 task s07, to pin that 13-plus-1 invariant; ADR-0022 Decision 7 explicitly
//! relaxes ADR-0007's byte-parity story from "byte-identical to the official extension" to "the
//! 13 trained tool schemas are byte-identical; exactly one additive, argument-less governance
//! tool is sanctioned on top." Any further change to this file, or to
//! `src/transport/mcp/schemas/tools.json`, is UNSANCTIONED -- s07 is the only task ever
//! authorized to touch either.

use browser_mcp::mcp::tools::TOOLS_JSON;
use serde_json::{json, Value};

/// The 13 trained tools, in order. Changing this array is changing the sacred contract.
const EXPECTED_TRAINED: [&str; 13] = [
    "tabs_context_mcp",
    "tabs_create_mcp",
    "navigate",
    "computer",
    "find",
    "form_input",
    "get_page_text",
    "javascript_tool",
    "read_console_messages",
    "read_network_requests",
    "read_page",
    "resize_window",
    "update_plan",
];

/// The `explain` tool's exact, pinned description string (ADR-0022 Decision 7).
const EXPLAIN_DESCRIPTION: &str = "Returns this server's action directory: every available \
action, the capability it requires (read, action, write, or execute; some require none), and a \
short description of what it does, plus definitions of the capability vocabulary. Use it to \
learn what you are allowed to do in this session. It does not read, summarize, or explain web \
pages.";

fn tools() -> Vec<Value> {
    let v: Value = serde_json::from_str(TOOLS_JSON).expect("tools.json must be valid JSON");
    v["tools"]
        .as_array()
        .expect("`tools` must be an array")
        .clone()
}

#[test]
fn advertises_exactly_the_thirteen_trained_tools_plus_explain_positioned_last() {
    let names: Vec<String> = tools()
        .iter()
        .map(|t| {
            t["name"]
                .as_str()
                .expect("name must be a string")
                .to_string()
        })
        .collect();
    assert_eq!(
        names.len(),
        14,
        "13 trained tools plus exactly one addition"
    );
    assert_eq!(
        names[..13],
        EXPECTED_TRAINED,
        "the 13 trained tools must stay byte-identical and in order"
    );
    assert_eq!(
        names[13], "explain",
        "the 14th (and only sanctioned addition) must be named explain, positioned last"
    );
}

/// The `explain` tool's own object matches ADR-0022 Decision 7 exactly: name, the pinned
/// description string, and the no-argument inputSchema shape (byte-for-byte the same shape as
/// `tabs_create_mcp`'s, the house style for a no-argument tool). No other tool was added: this
/// is checked separately from position/count above so a future stray addition fails loudly here
/// too, not just via the length assertion.
#[test]
fn explain_tool_object_matches_the_pinned_adr_0022_decision_7_shape() {
    let all = tools();
    let explain = all
        .iter()
        .find(|t| t["name"] == "explain")
        .expect("explain tool present");
    assert_eq!(explain["description"], EXPLAIN_DESCRIPTION);
    assert_eq!(
        explain["inputSchema"],
        json!({
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        }),
        "explain's inputSchema must match tabs_create_mcp's no-argument shape exactly"
    );

    let tabs_create_mcp = all
        .iter()
        .find(|t| t["name"] == "tabs_create_mcp")
        .expect("tabs_create_mcp tool present");
    assert_eq!(
        explain["inputSchema"], tabs_create_mcp["inputSchema"],
        "explain's inputSchema shape must be byte-for-byte identical to tabs_create_mcp's"
    );

    assert_eq!(
        all.len(),
        14,
        "no tool other than explain was added to the sacred fixture"
    );
}

#[test]
fn every_tool_is_well_formed() {
    for t in tools() {
        let name = t["name"].as_str().expect("name");
        assert!(!name.is_empty(), "tool name must be non-empty");
        assert!(
            t["description"].as_str().is_some_and(|d| !d.is_empty()),
            "{name}: description must be a non-empty string"
        );
        assert_eq!(
            t["inputSchema"]["type"].as_str(),
            Some("object"),
            "{name}: inputSchema.type must be \"object\""
        );
    }
}

#[test]
fn computer_advertises_all_thirteen_actions() {
    let computer = tools()
        .into_iter()
        .find(|t| t["name"] == "computer")
        .expect("computer tool must exist");
    let actions = computer["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .expect("computer.action must have an enum");
    assert_eq!(
        actions.len(),
        13,
        "computer must advertise all 13 actions (was {})",
        actions.len()
    );
}

/// Helper: fetch a tool by name (panics if absent).
fn tool(name: &str) -> Value {
    tools()
        .into_iter()
        .find(|t| t["name"] == name)
        .unwrap_or_else(|| panic!("{name} tool must exist"))
}

/// The `computer.action` enum order is part of the trained surface. The official v1.0.78 order
/// is NOT sorted and NOT the same order as its own description bullets -- it must be reproduced
/// verbatim.
#[test]
fn computer_action_enum_matches_official_order() {
    let computer = tool("computer");
    let actions: Vec<&str> = computer["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .expect("computer.action must have an enum")
        .iter()
        .map(|v| v.as_str().expect("enum entry must be a string"))
        .collect();
    assert_eq!(
        actions,
        [
            "left_click",
            "right_click",
            "type",
            "screenshot",
            "wait",
            "scroll",
            "key",
            "left_click_drag",
            "double_click",
            "triple_click",
            "zoom",
            "scroll_to",
            "hover",
        ],
        "computer.action enum order must match official v1.0.78 verbatim"
    );
}

/// The parity corrections harvested from official v1.0.78 (docs/research/12 section A). Each
/// assertion guards one correction against future drift.
#[test]
fn official_v1_0_78_schema_corrections_present() {
    // A1: navigate advertises the `force` boolean.
    assert_eq!(
        tool("navigate")["inputSchema"]["properties"]["force"]["type"].as_str(),
        Some("boolean"),
        "navigate must advertise the `force` boolean (official v1.0.78)"
    );

    // A2: get_page_text advertises `max_chars`.
    assert_eq!(
        tool("get_page_text")["inputSchema"]["properties"]["max_chars"]["type"].as_str(),
        Some("number"),
        "get_page_text must advertise the `max_chars` number param"
    );

    // A3: computer.duration is capped at 10 seconds (was 30).
    assert_eq!(
        tool("computer")["inputSchema"]["properties"]["duration"]["maximum"].as_i64(),
        Some(10),
        "computer.duration.maximum must be 10 (official v1.0.78)"
    );

    // A4: javascript_tool.action must NOT declare a `const` (official omits it).
    assert!(
        tool("javascript_tool")["inputSchema"]["properties"]["action"]
            .get("const")
            .is_none(),
        "javascript_tool.action must not declare `const` (official v1.0.78 omits it)"
    );
}

/// A7: the official references the tab tools by their BARE names (`tabs_context`, `tabs_create`)
/// in every description string -- the `_mcp` suffix appears only on the tool `name` fields. This
/// reproduces the trained tokens exactly. No description (tool-level or param-level) may contain
/// the `_mcp`-suffixed names.
#[test]
fn descriptions_reference_bare_tab_tool_names() {
    for t in tools() {
        let name = t["name"].as_str().unwrap_or("<unknown>");

        let tool_desc = t["description"].as_str().unwrap_or("");
        assert!(
            !tool_desc.contains("tabs_context_mcp"),
            "{name}: description must use bare `tabs_context`, not `tabs_context_mcp`"
        );
        assert!(
            !tool_desc.contains("tabs_create_mcp"),
            "{name}: description must use bare `tabs_create`, not `tabs_create_mcp`"
        );

        if let Some(props) = t["inputSchema"]["properties"].as_object() {
            for (pname, p) in props {
                let d = p["description"].as_str().unwrap_or("");
                assert!(
                    !d.contains("tabs_context_mcp"),
                    "{name}.{pname}: param description must use bare `tabs_context`"
                );
                assert!(
                    !d.contains("tabs_create_mcp"),
                    "{name}.{pname}: param description must use bare `tabs_create`"
                );
            }
        }
    }

    // The two renamed tab tools must still carry the `_mcp` suffix on their `name` field.
    assert_eq!(
        tool("tabs_context_mcp")["name"].as_str(),
        Some("tabs_context_mcp")
    );
    assert_eq!(
        tool("tabs_create_mcp")["name"].as_str(),
        Some("tabs_create_mcp")
    );
}
