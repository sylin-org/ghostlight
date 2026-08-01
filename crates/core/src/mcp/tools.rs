// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `tools/list` advertisement and the agent onboarding guide (ADR-0031 + ADR-0034 Decision 4).
//!
//! The tool advertisements live in code as `browser::directory::REGISTRY` entries (each
//! `ToolDescriptor` carries its own `advertised_description`, `input_schema`, and `example`).
//! This module renders them into the JSON shapes MCP expects. There is no separate fixture
//! file; the registry IS the single source.

use crate::browser::directory;

/// The `tools/list` advertisement: the complete `tools` array with each tool's name,
/// description, inputSchema, standard annotations, and example (when present), in registry order.
/// Rendered from the code-declared registry -- no fixture file.
pub fn advertised_tools_json() -> serde_json::Value {
    directory::advertised_tools_json()
}

/// Render the agent onboarding guide (ADR-0031 Decision 1) into the single string MCP's
/// `initialize.instructions` field expects.
pub fn agent_guide_text() -> String {
    directory::agent_guide_text()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-0031 Decision 1: the guide renders all four fields, in order, separated cleanly.
    #[test]
    fn agent_guide_text_renders_all_four_fields_in_order() {
        let text = agent_guide_text();
        assert!(!text.is_empty(), "the guide is non-empty");
        assert!(text.contains(directory::AGENT_GUIDE.summary));
        assert!(text.contains(directory::AGENT_GUIDE.workflow));
        assert!(text.contains(directory::AGENT_GUIDE.flow));
        assert!(text.contains(directory::AGENT_GUIDE.denials));
        assert!(
            text.contains("tabId"),
            "the workflow rule about tabId is present"
        );
    }

    /// The flow line is labeled so a reader recognizes the spine.
    #[test]
    fn agent_guide_text_labels_the_flow_line() {
        let text = agent_guide_text();
        assert!(
            text.contains("Typical flow:"),
            "the flow field is labeled so a reader recognizes the spine"
        );
    }

    /// The advertised tools JSON is well-formed and carries every registered tool.
    #[test]
    fn advertised_tools_json_carries_every_registered_tool() {
        let v = advertised_tools_json();
        let tools = v["tools"].as_array().expect("tools array");
        assert_eq!(tools.len(), directory::REGISTRY.len());
        for (entry, desc) in tools.iter().zip(directory::REGISTRY.iter()) {
            assert_eq!(entry["name"], desc.tool);
            assert!(entry.get("description").is_some());
            assert!(entry.get("inputSchema").is_some());
            assert!(entry.get("annotations").is_some());
        }
    }
}
