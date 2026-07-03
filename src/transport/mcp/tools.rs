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
