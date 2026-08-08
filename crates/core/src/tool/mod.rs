// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Protocol-neutral tool catalog, execution pipeline, and local compositions.
//!
//! Wire lifecycle and response envelopes live in `ghostlight-mcp-connector`. This module consumes only
//! normalized product operations and returns semantic outcomes.

pub mod act_on;
pub mod catalog;
pub mod flow;
pub mod form_fill;
pub mod gif_creator;
pub mod outcome;
pub mod pipeline;
pub mod provenance;
pub mod refs;
pub mod result;
pub mod tools;
pub mod update_plan;
pub mod upload_image;
pub mod validation;
