// SPDX-License-Identifier: Apache-2.0 OR MIT
//! MCP protocol layer -- hand-rolled **JSON-RPC 2.0 over stdio**.
//!
//! We deliberately do NOT use an MCP SDK crate (per `CLAUDE.md`): the protocol is simple and we
//! must preserve an exact, byte-identical tool surface. Handles `initialize`, `tools/list`, and
//! `tools/call`. Implemented in Phase 1.

pub mod browser_batch;
pub mod form_fill;
pub mod gif_creator;
pub mod outcome;
pub mod pipeline;
pub mod refs;
pub mod script;
pub mod server;
pub mod tools;
pub mod types;
pub mod upload_image;
pub mod validation;
