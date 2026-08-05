// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Local peer identity captured before an owner-only stream is type-erased.
//!
//! The OS-user principal admits workspace access. The process id is diagnostic only and is never
//! retained as authority, routing, or application continuity.

/// Stable same-user principal supplied by the local operating system.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PeerUser(pub String);

/// Credentials captured from one accepted local stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerCred {
    /// Same-user principal used for workspace admission and quotas.
    pub user: PeerUser,
    /// Connecting process id, for connection-scoped diagnostics only.
    pub pid: u32,
}
