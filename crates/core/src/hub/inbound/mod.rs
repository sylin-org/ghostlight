// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Owner-only local ingress for the persistent service.
//!
//! The composition root claims one named-pipe/UDS listener before startup continues. [`pipe`]
//! applies the `inbound.pipe.enabled` gate and runs the accept loop. The endpoint then admits
//! same-user MCP-edge or control peers, proves service identity, and routes MCP-edge peers into
//! the protocol-neutral typed bridge. MCP JSON-RPC and revision state terminate in
//! `ghostlight-mcp-connector`; they never enter this module or the service work pipeline.

pub mod pipe;
