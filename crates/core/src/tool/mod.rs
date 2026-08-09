// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight operation execution, results, and local compositions.
//!
//! Wire lifecycle and response envelopes live in `ghostlight-mcp-connector`. This module consumes only
//! normalized product operations and returns semantic outcomes.

pub mod act_on;
pub(crate) mod drag;
pub mod flow;
pub mod form_fill;
pub(crate) mod navigation_readiness;
pub mod outcome;
pub(crate) mod page_read;
pub mod pipeline;
pub mod provenance;
pub mod result;
pub(crate) mod tab_navigation;
pub(crate) mod target_screenshot;
pub(crate) mod wait;
