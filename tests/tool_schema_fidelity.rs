//! Regression snapshot for the `tools/list` surface, now code-declared in
//! `browser::directory::REGISTRY` (ADR-0034 Decision 4: tool declarations in code, not JSON).
//!
//! Pins the structural invariants: the 13 trained tools plus sanctioned additive tools, in order, each with
//! a non-empty description and an object inputSchema. The computer tool carries all 13 actions.
//! This is a regression snapshot (visibility), not a drift-prevention contract between two files.

use ghostlight::mcp::tools::advertised_tools_json;
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

const DIALOG_DESCRIPTION: &str = "Inspect or explicitly resolve the JavaScript dialog blocking \
one owned tab. Use status when the dialog state is unknown. Never accept, dismiss, or respond \
without intent from the current task.";

const TAB_CONTROL_DESCRIPTION: &str = "Focus, reload, or close one tab owned by this Ghostlight \
session. Close is always explicit and never affects a user-owned tab or automatically deletes the \
containing tab group.";

fn tools() -> Vec<Value> {
    let v = advertised_tools_json();
    v["tools"]
        .as_array()
        .expect("`tools` must be an array")
        .clone()
}

#[test]
fn advertises_the_thirteen_trained_tools_plus_sanctioned_additions_with_explain_last() {
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
        25,
        "13 trained tools plus narrate, wait_for, script, form_fill, act_on, dialog, tab_control, file_upload, browser_batch, upload_image, gif_creator, and explain"
    );
    assert_eq!(
        names[..13],
        EXPECTED_TRAINED,
        "the 13 trained tools must stay byte-identical and in order"
    );
    assert_eq!(names[13], "narrate", "the 14th tool is narrate");
    assert_eq!(names[14], "wait_for", "the 15th tool is wait_for");
    assert_eq!(names[15], "script", "the 16th tool is script");
    assert_eq!(names[16], "form_fill", "the 17th tool is form_fill");
    assert_eq!(names[17], "act_on", "the 18th tool is act_on");
    assert_eq!(names[18], "dialog", "the 19th tool is dialog");
    assert_eq!(names[19], "tab_control", "the 20th tool is tab_control");
    assert_eq!(names[20], "file_upload", "the 21st tool is file_upload");
    assert_eq!(names[21], "browser_batch", "the 22nd tool is browser_batch");
    assert_eq!(names[22], "upload_image", "the 23rd tool is upload_image");
    assert_eq!(
        names[23], "gif_creator",
        "the 24th tool is gif_creator, immediately before explain"
    );
    assert_eq!(names[24], "explain", "explain stays positioned last");
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
        25,
        "no tool other than narrate, wait_for, script, form_fill, act_on, dialog, tab_control, file_upload, browser_batch, upload_image, gif_creator, and explain was added to the sacred fixture"
    );
}

#[test]
fn dialog_tool_matches_the_pinned_adr_0078_shape() {
    let dialog = tool("dialog");
    assert_eq!(dialog["description"], DIALOG_DESCRIPTION);
    assert_eq!(
        dialog["inputSchema"],
        json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to inspect or resolve. The tab must belong to this Ghostlight session."
                },
                "action": {
                    "type": "string",
                    "enum": ["status", "accept", "dismiss", "respond"],
                    "description": "Inspect the current dialog or explicitly resolve it."
                },
                "text": {
                    "type": "string",
                    "description": "Prompt response text. Required only for respond."
                }
            },
            "required": ["tabId", "action"],
            "allOf": [{
                "if": {
                    "properties": { "action": { "const": "respond" } },
                    "required": ["action"]
                },
                "then": { "required": ["text"] },
                "else": { "not": { "required": ["text"] } }
            }],
            "additionalProperties": false
        })
    );
}

#[test]
fn tab_control_tool_matches_the_pinned_adr_0078_shape() {
    let tab_control = tool("tab_control");
    assert_eq!(tab_control["description"], TAB_CONTROL_DESCRIPTION);
    assert_eq!(
        tab_control["inputSchema"],
        json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID to control. The tab must belong to this Ghostlight session."
                },
                "action": {
                    "type": "string",
                    "enum": ["focus", "reload", "close"],
                    "description": "Focus the tab, reload its page, or explicitly close that one tab."
                }
            },
            "required": ["tabId", "action"],
            "additionalProperties": false
        })
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
fn narrate_is_additive_and_pins_its_bounded_schema() {
    let narrate = tools()
        .into_iter()
        .find(|t| t["name"] == "narrate")
        .expect("narrate tool present");
    assert_eq!(
        narrate["inputSchema"],
        json!({
            "type": "object",
            "properties": {
                "tabId": {
                    "type": "number",
                    "description": "Tab ID in which to show the narration. Must be a tab owned by this session."
                },
                "text": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 240,
                    "description": "One short, user-visible sentence describing the current workflow phase."
                },
                "position": {
                    "type": "string",
                    "enum": ["auto", "top", "bottom"],
                    "default": "auto",
                    "description": "Which viewport edge holds the narration ribbon. Auto avoids recent interaction and scroll activity; defaults to auto."
                },
                "duration_ms": {
                    "type": "integer",
                    "minimum": 1000,
                    "maximum": 30000,
                    "default": 5000,
                    "description": "How long to show the narration, in milliseconds. Defaults to 5000."
                }
            },
            "required": ["tabId", "text"],
            "additionalProperties": false
        })
    );
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

/// ADR-0031 Decision 5: the agentGuide section is present and well-formed. The four fields are
/// the agent's workflow contract at handshake; a missing/empty one breaks the onboarding payload.
#[test]
fn agent_guide_is_present_with_all_four_non_empty_fields() {
    let guide = ghostlight::browser::directory::AGENT_GUIDE;
    for (key, val) in [
        ("summary", guide.summary),
        ("workflow", guide.workflow),
        ("flow", guide.flow),
        ("denials", guide.denials),
    ] {
        assert!(!val.is_empty(), "agentGuide.{key} must be non-empty");
    }
    // The load-bearing workflow rule must be present (this is the fact that, when missing, an
    // untrained model gets wrong on the first call).
    assert!(
        guide.workflow.contains("tabId"),
        "agentGuide.workflow must state the tabId-first rule"
    );
}

/// ADR-0031 Decision 5: every trained tool's `example.call` VALIDATES against its own
/// `inputSchema` (run through the same validator o04 wires into the pipeline). This is the
/// "trimmed-for-readability examples are mechanically uncommittable" guardrail -- an example
/// missing a required field, carrying an unknown property, or a wrong-typed value fails CI.
#[test]
fn every_trained_tools_example_call_validates_against_its_own_input_schema() {
    use ghostlight::transport::mcp::validation::{validate_arguments, ToolSchema};

    for t in tools() {
        let name = t["name"].as_str().expect("name");
        // `explain` carries no example (argument-less, self-describing) -- skip it.
        let Some(example) = t.get("example") else {
            assert_eq!(
                name, "explain",
                "only explain is permitted to omit an example; {name} must carry one"
            );
            continue;
        };
        let call = example
            .get("call")
            .unwrap_or_else(|| panic!("{name}: example must carry a call object"));
        // Build the schema the same way the pipeline does (o04) and validate.
        let schema = ToolSchema {
            input_schema: t["inputSchema"].clone(),
            example_call: Some(call.clone()),
        };
        validate_arguments(&schema, call).unwrap_or_else(|e| {
            panic!("{name}: example.call must validate against its own inputSchema, but: {e}")
        });
    }
}

/// C3 (ADR-0038 Decision 3, PINS.md SS5): `outputSchema` is advertised for exactly the v1
/// structured-result vocabulary tools declared so far, in advertised order, and nowhere else;
/// each is a JSON-Schema object. ADR-0078 C1 adds targeted `read_page`; C2 adds interaction
/// receipts to the low-level mutating tools.
#[test]
fn output_schemas_present_exactly_where_declared() {
    let with_schema: Vec<String> = tools()
        .iter()
        .filter(|t| t.get("outputSchema").is_some())
        .map(|t| t["name"].as_str().expect("name").to_string())
        .collect();
    assert_eq!(
        with_schema,
        vec![
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
            "narrate",
            "wait_for",
            "script",
            "form_fill",
            "act_on",
            "dialog",
            "tab_control",
            "file_upload",
            "upload_image",
            "gif_creator"
        ],
        "outputSchema must be advertised for exactly these tools, in this order"
    );
    for name in &with_schema {
        let schema = &tool(name)["outputSchema"];
        assert_eq!(
            schema["type"].as_str(),
            Some("object"),
            "{name}: outputSchema.type must be \"object\""
        );
    }
    for name in [
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
        "wait_for",
        "form_fill",
        "act_on",
        "dialog",
        "file_upload",
        "upload_image",
        "gif_creator",
    ] {
        assert!(
            tool(name)["outputSchema"]["properties"]
                .get("provenance")
                .is_some(),
            "{name}: page-sourced output declares provenance"
        );
    }
}
