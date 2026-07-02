//! MCP protocol layer -- hand-rolled **JSON-RPC 2.0 over stdio**.
//!
//! We deliberately do NOT use an MCP SDK crate (per `CLAUDE.md`): the protocol is simple and we
//! must preserve an exact, byte-identical tool surface. Handles `initialize`, `tools/list`, and
//! `tools/call`. Implemented in Phase 1.

pub mod server;
pub mod tools;
pub mod types;
