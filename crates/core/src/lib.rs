// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight core: the churny brain (governance, tools, browser protocol, hub composition,
//! installer, CLI support). Depends on ghostlight-transport; the `ghostlight-mcp-connector` and
//! browser-only `ghostlight-browser-connector` shore executables must NEVER depend on this crate.

pub(crate) mod armor;
pub(crate) mod b64;
pub mod browser;
pub mod constants;
pub mod gif;
pub mod governance;
pub mod hub;
pub mod install;
pub mod messages;
pub mod operation;
pub mod origin;
pub(crate) mod recording;
pub mod tool;
pub mod work;

pub use ghostlight_transport::error::{Error, Result, ToolError};
