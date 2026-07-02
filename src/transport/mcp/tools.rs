//! Tool registry and `tools/list` advertisement.
//!
//! The tool **schemas are sacred**: they are byte-identical to the reference's advertised surface
//! (captured in `src/mcp/schemas/`, guarded by `tests/tool_schema_fidelity.rs`). In all-open v1.0
//! the full surface is advertised unconditionally -- the 13 preserved Claude-in-Chrome tools
//! (`tabs_context_mcp`, `tabs_create_mcp`, `navigate`, `computer`, `find`, `form_input`,
//! `get_page_text`, `javascript_tool`, `read_console_messages`, `read_network_requests`,
//! `read_page`, `resize_window`, `update_plan`). The excluded stubs (`gif_creator`,
//! `shortcuts_list`, `shortcuts_execute`, `switch_browser`, `upload_image`) are not advertised.
//! Implemented in Phase 1.

/// The sacred `tools/list` surface: the 13 preserved tool schemas, embedded verbatim as raw JSON
/// (a const literal, per CLAUDE.md, to prevent accidental drift). Provenance and fidelity notes
/// are in `schemas/README.md`; `tests/tool_schema_fidelity.rs` guards it.
pub const TOOLS_JSON: &str = include_str!("schemas/tools.json");

/// True when `name` is one of the advertised tool names in the sacred fixture. Read-only use of
/// [`TOOLS_JSON`]: the fixture itself is never edited (see `tests/tool_schema_fidelity.rs`).
pub fn is_known_tool(name: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(TOOLS_JSON) else {
        return false;
    };
    parsed["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .any(|t| t.get("name").and_then(serde_json::Value::as_str) == Some(name))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_known_tool_recognizes_advertised_names() {
        assert!(is_known_tool("navigate"));
    }

    #[test]
    fn is_known_tool_rejects_unknown_names() {
        assert!(!is_known_tool("bogus_tool"));
    }
}
