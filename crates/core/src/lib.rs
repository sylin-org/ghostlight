// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight core: the churny brain (governance, tools, browser protocol, hub composition,
//! installer, CLI support). Depends on ghostlight-transport; the adapter executables must
//! NEVER depend on this crate (ADR-0046 Decision 2).

pub(crate) mod b64;
pub mod browser;
pub mod gif;
pub mod governance;
pub mod hub;
pub mod install;
pub mod mcp;
pub mod messages;
pub mod origin;

pub use ghostlight_transport::error::{Error, Result, ToolError};
