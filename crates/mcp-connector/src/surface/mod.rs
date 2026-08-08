// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Edge-owned model-facing tool surfaces.
//!
//! A surface owns declarations, external validation and normalization, and result rendering. It
//! never owns capability requirements, routing, scheduling, policy, or browser mechanisms.

pub(crate) mod ghostlight_legacy;
mod schema;
