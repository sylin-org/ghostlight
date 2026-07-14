// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The outbound zone -- per-capability EXECUTORS that translate a native tool-call into the
//! backend's commands and await its reply.
//!
//! Each capability (the browser today; desktop, shell, filesystem, network later) gets its own
//! executor module here, symmetric with the per-channel ingestors in [`crate::hub::inbound`].
//! The pair forms the matrix: inbound ingestors converge on the governance pipeline, which
//! dispatches a native tool-call to the matching outbound executor. The pipeline knows neither
//! end; the executors know no policy.
//!
//! Every capability implements [`ICapability`] and is registered in the composition root's
//! [`Registry`]. The registry aggregates each capability's tool directory + agent guide into
//! the single source that `tools/list`, `explain`, the enforcement `requires` lookup, and the
//! schema validator consume. Adding a capability is "implement the trait, register at the
//! composition root," not "edit four files."

pub mod browser;
pub mod diagnostics;

use crate::browser::directory::{AgentGuide, ToolDescriptor};
use std::sync::Arc;

/// A capability executor: owns a backend connection, declares its tool surface, and dispatches
/// native tool-calls to the backend.
///
/// `Send + Sync` so it can be shared across the tokio runtime and held by an `Arc` on the
/// [`Registry`]. The trait's methods take serializable inputs and return serializable outputs
/// (no `&self` lending of internal state) so a future out-of-process capability impl is a
/// redesign, not a rewrite.
///
/// Today only the browser capability exists; shell, filesystem, desktop, and network are
/// future. The trait is the contract that makes adding one "implement + register," not "edit
/// the pipeline, the composition root, the directory, and the explain renderer."
pub trait ICapability: Send + Sync {
    /// The stable identifier (`"browser"`). Used as the audit `capability_origin` field and
    /// the manifest entry key.
    fn code(&self) -> &'static str;

    /// A one-line human description for the capability manifest at handshake.
    fn descriptor(&self) -> &'static str;

    /// The tool declarations this capability owns: name, advertised description, inputSchema,
    /// RAWX requirements, example, dispatch kind, resource shape. The registry composes every
    /// capability's slice into the aggregated directory.
    fn directory(&self) -> &'static [ToolDescriptor];

    /// The agent-facing onboarding guide this capability contributes. The registry composes
    /// each capability's guide additively into `initialize.instructions`.
    fn agent_guide(&self) -> AgentGuide;
}

/// The composition root's registry of capability executors. Built once at startup
/// ([`Registry::new`]), held by `ServiceContext` as `Arc<[Arc<dyn ICapability>]>`, and consumed
/// by every surface that needs the aggregated tool directory or the per-capability manifest.
///
/// Routing (decision b): the registry builds a `tool_name -> capability_code` lookup at
/// construction. A duplicate claim (two capabilities declaring the same tool name) is a
/// fail-closed error at construction, never a silent misroute.
pub struct Registry {
    capabilities: Arc<[Arc<dyn ICapability>]>,
}

impl Registry {
    /// Build the registry from an ordered list of capabilities. Panics on a duplicate tool-name
    /// claim (a startup error -- the composition root catches this before serving).
    pub fn new(capabilities: Vec<Arc<dyn ICapability>>) -> Self {
        let mut seen = std::collections::HashSet::new();
        for cap in &capabilities {
            for desc in cap.directory() {
                if !seen.insert(desc.tool) {
                    panic!(
                        "duplicate tool name '{}' claimed by capability '{}'",
                        desc.tool,
                        cap.code()
                    );
                }
            }
        }
        Self {
            capabilities: capabilities.into(),
        }
    }

    /// The ordered list of capabilities (for manifest rendering, etc.).
    pub fn capabilities(&self) -> &[Arc<dyn ICapability>] {
        &self.capabilities
    }

    /// The aggregated tool directory: every capability's declarations, in capability-then-
    /// declaration order. This is the single source consumed by `tools/list`, `explain`,
    /// enforcement, and the validator.
    pub fn aggregated_directory(&self) -> Vec<&'static ToolDescriptor> {
        self.capabilities
            .iter()
            .flat_map(|cap| cap.directory().iter())
            .collect()
    }
}

impl Clone for Registry {
    fn clone(&self) -> Self {
        Self {
            capabilities: Arc::clone(&self.capabilities),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_capability_exposes_the_full_directory() {
        let cap = browser::BrowserCapability::new(browser::Browser::new());
        assert_eq!(cap.code(), "browser");
        assert!(!cap.descriptor().is_empty());
        // The browser's directory is the full REGISTRY (ADR-0051 Phase 1: derived from the one
        // advertised-surface oracle, so an additive tool does not re-bump this site).
        assert_eq!(
            cap.directory().len(),
            crate::browser::directory::advertised_tool_count()
        );
        // The agent guide carries all four fields.
        let guide = cap.agent_guide();
        assert!(!guide.summary.is_empty());
        assert!(!guide.workflow.is_empty());
        assert!(!guide.flow.is_empty());
        assert!(!guide.denials.is_empty());
    }

    #[test]
    fn registry_aggregates_the_browser_directory() {
        let cap: Arc<dyn ICapability> =
            Arc::new(browser::BrowserCapability::new(browser::Browser::new()));
        let reg = Registry::new(vec![cap]);
        assert_eq!(reg.capabilities().len(), 1);
        assert_eq!(
            reg.aggregated_directory().len(),
            crate::browser::directory::advertised_tool_count()
        );
    }

    #[test]
    #[should_panic(expected = "duplicate tool name")]
    fn registry_rejects_a_duplicate_tool_claim() {
        let cap: Arc<dyn ICapability> =
            Arc::new(browser::BrowserCapability::new(browser::Browser::new()));
        // Two browser capabilities both claim "navigate" -- a startup error.
        let _ = Registry::new(vec![cap.clone(), cap]);
    }
}
