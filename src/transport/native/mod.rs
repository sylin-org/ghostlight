//! Native messaging and inter-instance IPC.
//!
//! The **same executable** runs in two roles, selected at startup by launch context:
//!
//! - **native-host role** -- launched by Chrome via `chrome.runtime.connectNative`. Speaks the
//!   Chrome native-messaging protocol (4-byte little-endian length prefix + UTF-8 JSON) on
//!   stdin/stdout. See [`host`].
//! - **mcp-server role** -- launched by the MCP client (Claude Code, etc.) over stdio. It reaches
//!   the browser through the native-host instance via the local IPC. See [`ipc`].
//!
//! The first instance to acquire the IPC endpoint owns the browser; a second is rejected with
//! [`crate::Error::SessionBusy`] (single active session in v1.0; multi-session sharing deferred).

pub mod host;
pub mod ipc;
pub mod messages;
