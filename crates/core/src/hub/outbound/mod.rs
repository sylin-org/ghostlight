// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The outbound zone -- backend executors for canonical browser operations.
//!
//! The canonical operation registry owns product semantics and enforcement metadata. These
//! executors own only physical delivery and backend state. Model-facing declarations and
//! translations stay at the MCP edge under ADR-0101.

pub mod browser;
pub mod diagnostics;
mod legacy_mechanism;
mod workspace;
