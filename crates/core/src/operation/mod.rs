// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Canonical, protocol-neutral browser-operation semantics.
//!
//! Surface declarations translate into the closed operation vocabulary before this module is
//! entered. The registry is the single authority for validation, RAWX classification, workspace
//! use, resource resolution, scheduling, dispatch, result provenance, and success disposition.

pub(crate) mod preparation;
pub mod registry;
pub mod result;
