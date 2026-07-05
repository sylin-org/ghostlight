// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The ADAPTER/CONTROL endpoint's session-hello (ADR-0030 Decision 1, the 2026-07-04 two-endpoint
//! amendment; PINS.md SS1).
//!
//! Carried ON TOP OF the existing 4-byte-LE `transport::native::host` framing (never a change to
//! that framing) as one JSON object: `{ "hub": 1, "role": "<role>", "guid": "<uuid>"? }`. This
//! endpoint is the ONLY place a hello is ever sent: the EXTENSION endpoint keeps its exact
//! server-speaks-first contract and carries no hello frame at all, so there is NO `ROLE_EXT` --
//! the extension is identified by the endpoint it arrives at, not by a role string.

/// The session-hello protocol major version (PINS.md SS1).
pub const HUB_PROTO: u32 = 1;

/// An MCP stdio adapter session (PINS.md SS1): the role `hub::run_mcp_server`'s loser branch
/// (`ipc::relay_adapter`) sends, and the role dispatched to
/// [`crate::transport::mcp::server::serve_session`] on the service side.
pub const ROLE_ADAPTER: &str = "adapter";

/// The reserved control-plane role (doctor/console; not used before H8). Cleanly refused by the
/// service until then (PINS.md SS1).
pub const ROLE_CONTROL: &str = "control";
