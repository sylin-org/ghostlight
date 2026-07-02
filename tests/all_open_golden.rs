//! All-open golden guard for the A1 module reorg. The regroup into governance/ browser/
//! transport/ must change NOTHING observable. Two invariants, reached through the NEW
//! module locations:
//!   1. tools/list byte-stability -- the advertised tool surface is the same 13 tools in
//!      the same order, and `is_known_tool` still resolves them.
//!   2. dispatch round-trip -- the moved `governance::dispatch` seam resolves every call
//!      to `Allow` (all-open), and `audit` is a no-op that does not panic.

use browser_mcp::governance::dispatch::{self, PolicyDecision};
use browser_mcp::transport::mcp::tools::{is_known_tool, TOOLS_JSON};
use serde_json::Value;

/// The 13 tool names in advertised order, copied from the parsed `TOOLS_JSON` fixture (the
/// sacred fixture is the source of truth for the exact order).
const GOLDEN_TOOL_NAMES: [&str; 13] = [
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

#[test]
fn tools_list_is_byte_stable_through_the_move() {
    let v: Value = serde_json::from_str(TOOLS_JSON).expect("TOOLS_JSON parses");
    let tools = v["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        GOLDEN_TOOL_NAMES.len(),
        "all 13 tools advertised"
    );
    for (i, name) in GOLDEN_TOOL_NAMES.iter().enumerate() {
        assert_eq!(
            tools[i]["name"], *name,
            "tool #{i} name and order preserved"
        );
        assert!(is_known_tool(name), "{name} must be a known tool");
    }
    assert!(!is_known_tool("bogus_tool"), "unknown tools stay unknown");
}

#[test]
fn dispatch_seam_is_all_open_after_the_move() {
    for name in GOLDEN_TOOL_NAMES {
        assert_eq!(
            dispatch::policy_check(name),
            PolicyDecision::Allow,
            "{name} must be allowed in the all-open engine"
        );
        dispatch::audit(name); // no-op seam; must not panic
    }
}
