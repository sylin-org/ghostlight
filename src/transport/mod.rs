// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The protocol-plumbing layer: the MCP JSON-RPC session ([`mcp`]), the native-messaging wire
//! and inter-instance IPC ([`native`]), and the parent-death [`watchdog`].
//!
//! This is the wire level -- framed bytes and protocol state -- distinct from the role zones in
//! [`crate::hub::inbound`] (per-channel ingestors that converge on the pipeline) and
//! [`crate::hub::outbound`] (per-capability executors). The browser-executor handle that used to
//! live here moved to [`crate::hub::outbound::browser`]; this module keeps the framing and the
//! protocol state it speaks over. It depends on [`crate::governance`] and [`crate::browser`];
//! neither depends back.

pub mod mcp;
pub mod native;
pub mod watchdog;
