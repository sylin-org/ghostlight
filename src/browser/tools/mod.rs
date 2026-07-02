//! Tool implementations -- thin wrappers that translate an MCP `tools/call` into an extension
//! command (built here in the binary; executed by the extension).
//!
//! The **13 tools** of the all-open v1.0 engine. Read/write classification is an engine-intrinsic
//! property consumed by the v1.5 overlay; in v1.0 every tool is available (all-open). The five
//! excluded reference stubs (`gif_creator`, `shortcuts_list`, `shortcuts_execute`,
//! `switch_browser`, `upload_image`) are not implemented.
//!
//! | Module | Tool(s) | Class |
//! |---|---|---|
//! | [`tabs`] | `tabs_context_mcp`, `tabs_create_mcp` | Observe, Mutate |
//! | [`navigate`] | `navigate` | Observe (overlay domain-enforcement point) |
//! | [`computer`] | `computer` (13 actions) | per-action Observe/Mutate |
//! | [`read_page`] | `read_page` | Observe |
//! | [`page_text`] | `get_page_text` | Observe |
//! | [`find`] | `find` | Observe |
//! | [`form_input`] | `form_input` | Mutate |
//! | [`javascript`] | `javascript_tool` | Mutate |
//! | [`network`] | `read_console_messages`, `read_network_requests` | Observe |
//! | [`manage`] | `resize_window`, `update_plan` | Manage |

pub mod computer;
pub mod find;
pub mod form_input;
pub mod javascript;
pub mod manage;
pub mod navigate;
pub mod network;
pub mod page_text;
pub mod read_page;
pub mod tabs;
