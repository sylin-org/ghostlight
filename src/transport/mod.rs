// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Transport infra -- the composition-root I/O and protocol layer.
//!
//! This bounded context (see docs/design/ghostlight-service-architecture.md section 3)
//! wires the outside world to the domain: the MCP JSON-RPC session ([`mcp`]), native
//! messaging and inter-instance IPC ([`native`]), and the [`executor`] handle the
//! mcp-server uses to call tools on the connected extension. It depends on both the
//! [`crate::governance`] core and the [`crate::browser`] domain plugin; neither of those
//! may depend back on this module.

pub mod executor;
pub mod mcp;
pub mod native;
pub mod watchdog;
