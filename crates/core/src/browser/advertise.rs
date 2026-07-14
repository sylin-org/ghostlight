// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Tool advertisement filtering (browser plugin; ADR-0022 Decision 8).
//!
//! `tools/list` membership is a domain-independent visibility optimization: with no manifest,
//! the full surface is advertised verbatim (all-open stays byte-identical); with a
//! manifest, a tool is kept only when at least one of its action-directory variants (ADR-0022
//! Decision 2) could ever be permitted -- either its `requires` is empty (unconditionally
//! allowed) or it is a subset of some single grant's `allowed` set. No tab exists at
//! `tools/list` time, so this can never be a per-domain decision. Per-call enforcement
//! (`governance::enforcement`) remains the sole authoritative check regardless of what this
//! module returns; hiding a tool here is not denying it, and nothing in this module may claim
//! otherwise. Schema TEXT is never altered -- a kept tool object is the advertisement object, cloned
//! unchanged; only which tools appear in the array changes.
//!
//! [`advertised_tools`] computes the permitted set from a manifest snapshot; it is a pure
//! function and holds no notification logic. Dynamic re-advertisement -- emitting MCP
//! `notifications/tools/list_changed` when a manifest hot-reload changes the permitted set --
//! IS implemented, in the MCP server: on reload it recomputes the advertised set and, when it
//! differs, emits the notification through the single-writer stdout task (ADR-0025 Decision 4;
//! see `mcp::server` and its `advertised_set_diff_gates_the_notification` test). This module
//! stays the pure filter; the server owns the live diff-and-notify.

use crate::browser::directory;
use crate::governance::manifest::document::Grant;
use crate::governance::ports::capability_subset;
use serde_json::Value;

/// Compute the advertised `{ "tools": [...] }` object. `advertisement` is the parsed
/// tool-schema advertisement (the registry-rendered advertisement (`advertised_tools_json`), so this
/// browser-plugin module never depends on the transport layer). `grants` is `None` for no
/// manifest (all-open): `advertisement` is returned verbatim, byte-identical, no tool ever dropped,
/// reordered, or edited. `Some(grants)` (including an empty slice) filters to the tools with at
/// least one directory variant a grant could ever permit (see [`tool_has_a_reachable_variant`]).
/// An empty `grants` slice still advertises every tool with a `requires: []` variant (ADR-0022
/// Decision 5 step 2: those actions need no grant at all).
pub fn advertised_tools(advertisement: &Value, grants: Option<&[Grant]>) -> Value {
    let Some(grants) = grants else {
        return advertisement.clone();
    };
    let tools = advertisement["tools"]
        .as_array()
        .expect("the advertisement has a top-level 'tools' array");
    let kept: Vec<Value> = tools
        .iter()
        .filter(|tool| {
            let name = tool["name"]
                .as_str()
                .expect("every advertisement tool object has a string 'name'");
            tool_has_a_reachable_variant(name, grants)
        })
        .cloned()
        .collect();
    serde_json::json!({ "tools": kept })
}

/// Whether `tool_name` has at least one action-directory variant (for `computer`, any of its 13
/// action rows; for every other tool its single row) whose `requires` is empty OR is a subset
/// of ANY single grant's `allowed` (ADR-0022 Decision 8).
fn tool_has_a_reachable_variant(tool_name: &str, grants: &[Grant]) -> bool {
    directory::REGISTRY
        .iter()
        .filter(|row| row.tool == tool_name)
        .flat_map(|row| row.variants.iter())
        .any(|variant| {
            variant.requires.is_empty()
                || grants
                    .iter()
                    .any(|g| capability_subset(variant.requires, &g.allowed))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::manifest::document::HostRules;
    use crate::governance::ports::Capability;
    use crate::mcp::tools::advertised_tools_json;

    fn advertisement() -> Value {
        advertised_tools_json()
    }

    fn names_of(result: &Value) -> Vec<String> {
        result["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|t| t["name"].as_str().expect("name").to_string())
            .collect()
    }

    fn grant(allowed: &[Capability]) -> Grant {
        Grant {
            id: "g".to_string(),
            hosts: HostRules {
                allow: vec!["example.com".to_string()],
                deny: Vec::new(),
            },
            allowed: allowed.to_vec(),
            description: None,
            mode: None,
        }
    }

    #[test]
    fn no_manifest_returns_the_advertisement_verbatim() {
        let fx = advertisement();
        assert_eq!(
            advertised_tools(&fx, None),
            fx,
            "byte-identical, not just same names"
        );
    }

    /// ADR-0022 Decision 8 consequence: a read-only grant advertises everything except
    /// `form_input` (requires write) and `javascript_tool` (requires execute); `navigate` and
    /// every requires-empty tool join the set.
    #[test]
    fn read_only_grant_advertises_everything_except_write_and_execute_tools() {
        let fx = advertisement();
        let grants = vec![grant(&[Capability::Read])];
        let result = advertised_tools(&fx, Some(&grants));
        assert_eq!(
            names_of(&result),
            vec![
                "tabs_context_mcp",
                "tabs_create_mcp",
                "navigate",
                "computer",
                "find",
                "get_page_text",
                "read_console_messages",
                "read_network_requests",
                "read_page",
                "resize_window",
                "update_plan",
                "narrate",
                "wait_for",
                "script",
                "browser_batch",
                "gif_creator",
                "explain",
            ]
        );
    }

    /// ADR-0022 Decision 8 consequence: an empty-grants manifest advertises exactly the
    /// requires-empty set (`tabs_create_mcp`, `resize_window`, `update_plan`, `narrate`, `script`,
    /// `browser_batch`, `explain`, and `computer` via its `wait` row), in advertisement order -- not
    /// an empty list.
    #[test]
    fn empty_grants_manifest_advertises_exactly_the_requires_empty_set() {
        let fx = advertisement();
        let result = advertised_tools(&fx, Some(&[]));
        assert_eq!(
            names_of(&result),
            vec![
                "tabs_create_mcp",
                "computer",
                "resize_window",
                "update_plan",
                "narrate",
                "script",
                "browser_batch",
                "gif_creator",
                "explain",
            ]
        );
    }

    #[test]
    fn a_grant_permitting_write_advertises_form_input() {
        let fx = advertisement();
        let grants = vec![grant(&[Capability::Read, Capability::Write])];
        assert!(names_of(&advertised_tools(&fx, Some(&grants))).contains(&"form_input".to_string()));
    }

    #[test]
    fn a_grant_permitting_execute_advertises_javascript_tool() {
        let fx = advertisement();
        let grants = vec![grant(&[Capability::Execute])];
        assert!(names_of(&advertised_tools(&fx, Some(&grants)))
            .contains(&"javascript_tool".to_string()));
    }

    #[test]
    fn computer_is_advertised_under_every_nonempty_grant_since_wait_requires_nothing() {
        let fx = advertisement();
        for caps in [
            vec![Capability::Read],
            vec![Capability::Action],
            vec![Capability::Write],
            vec![Capability::Execute],
        ] {
            let grants = vec![grant(&caps)];
            assert!(
                names_of(&advertised_tools(&fx, Some(&grants))).contains(&"computer".to_string()),
                "{caps:?}"
            );
        }
        // Even zero grants: computer's `wait` action requires nothing.
        assert!(names_of(&advertised_tools(&fx, Some(&[]))).contains(&"computer".to_string()));
    }
}
