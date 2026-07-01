//! Fidelity guard for the sacred `tools/list` surface (`src/mcp/schemas/tools.json`).
//!
//! Ensures the embedded schema fixture stays intact: exactly the 13 preserved tools, in order,
//! each with a non-empty description and an object inputSchema. Once `tools/list` is implemented
//! (Phase 1), this is extended to byte-compare the emitted output against the fixture.

use browser_mcp::mcp::tools::TOOLS_JSON;
use serde_json::Value;

/// The exact advertised surface, in order. Changing this array is changing the sacred contract.
const EXPECTED: [&str; 13] = [
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

fn tools() -> Vec<Value> {
    let v: Value = serde_json::from_str(TOOLS_JSON).expect("tools.json must be valid JSON");
    v["tools"]
        .as_array()
        .expect("`tools` must be an array")
        .clone()
}

#[test]
fn advertises_exactly_the_thirteen_preserved_tools_in_order() {
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
        names, EXPECTED,
        "the advertised tool set/order must match the sacred surface"
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
    assert_eq!(tool("tabs_context_mcp")["name"].as_str(), Some("tabs_context_mcp"));
    assert_eq!(tool("tabs_create_mcp")["name"].as_str(), Some("tabs_create_mcp"));
}
