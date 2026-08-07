// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Protocol-neutral projection of the one canonical tool registry.

use crate::browser::{advertise, directory};
use crate::governance::dispatch::Governance;
use crate::governance::overlay::SessionOverlay;
use crate::tool::tools::{advertised_tools_json, agent_guide_text};
use ghostlight_transport::bridge::{CatalogProjection, CatalogTool};
use serde_json::Value;

/// Project the ordered service catalog under current authority and an optional restriction.
pub fn project_catalog(
    governance: &Governance,
    restriction: Option<&SessionOverlay>,
    generation: u64,
) -> CatalogProjection {
    let canonical = advertised_tools_json();
    let service_filtered = advertise::advertised_tools(&canonical, governance.grants());
    let filtered = match restriction {
        Some(restriction) => advertise::advertised_tools(&service_filtered, restriction.grants()),
        None => service_filtered,
    };

    let declarations = filtered
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let tools = declarations
        .into_iter()
        .filter_map(|declaration| {
            let name = declaration.get("name")?.as_str()?;
            let descriptor = directory::descriptor(name)?;
            Some(CatalogTool {
                declaration,
                workspace_use: match descriptor.workspace_use {
                    directory::WorkspaceUse::Independent => {
                        ghostlight_transport::bridge::WorkspaceUse::Independent
                    }
                    directory::WorkspaceUse::Creates => {
                        ghostlight_transport::bridge::WorkspaceUse::Creates
                    }
                    directory::WorkspaceUse::Uses => {
                        ghostlight_transport::bridge::WorkspaceUse::Uses
                    }
                },
            })
        })
        .collect();

    CatalogProjection {
        generation,
        instructions: agent_guide_text(),
        tools,
        restricted: restriction.is_some(),
    }
}
