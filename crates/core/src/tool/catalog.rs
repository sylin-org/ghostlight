// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Protocol-neutral projection of the canonical operation registry.

use crate::governance::dispatch::Governance;
use crate::governance::overlay::SessionOverlay;
use ghostlight_transport::bridge::CatalogProjection;

/// Project the ordered service catalog under current authority and an optional restriction.
pub fn project_catalog(
    governance: &Governance,
    restriction: Option<&SessionOverlay>,
    generation: u64,
) -> CatalogProjection {
    crate::operation::registry::project_availability(governance, restriction, generation)
}
