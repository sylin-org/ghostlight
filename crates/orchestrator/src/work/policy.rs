//! Model-facing policy explanation execution (ADR-0136).
//!
//! The one operation that crosses no browser seam: the authority in force is compiled by the
//! governance facade's single projection -- the same one the workbench renders -- and handed to
//! the model with machine-local document texts and paths withheld.

use serde_json::{json, Value};

use crate::governance::{CapabilitySet, Decision};
use crate::language::outcome::Outcome;

use super::{ApplicationExecutor, Effect, InvocationContext, Readiness, Terminal};

impl ApplicationExecutor {
    /// Explain current authority from the orchestrator-owned projection.
    pub(super) fn explain_policy(&self, context: &InvocationContext<'_>) -> Terminal {
        // Explaining authority requires no browser permission and remains available while that
        // authority stops browser work. Keep the ordinary completion/audit path, without clearing
        // attention, changing human controls, or inventing a grant evaluation (ADR-0136).
        let decision = Decision::permitted();
        self.retain_permission(
            context,
            context
                .snapshot
                .decision_evidence(CapabilitySet::EMPTY, None, decision),
        );
        let mut authority = self.governance.effective_authority();
        // Machine-local reading aids stay out of model results (ADR-0136 Decision 2). The
        // person's workbench destination keeps rendering them.
        for layer in &mut authority.layers {
            layer.path = None;
            layer.document = None;
        }
        let capabilities = authority.capabilities.len();
        let layers = authority.layers.len();
        let facts = json!({
            "situation": authority.headline,
            "organization": authority.organization.as_ref().map(|org| json!({
                "name": org.name,
                "statement": org.statement,
            })),
            "capabilities": authority.capabilities,
            "layers": authority.layers.iter().map(|layer| json!({
                "kind": layer.kind,
                "title": layer.title,
                "policy_name": layer.policy_name,
                "version": layer.version,
                "mode": layer.mode,
                "rules": layer.rules,
                "settings": layer.settings,
            })).collect::<Vec<Value>>(),
            "ceilings": authority.ceilings,
            "browser_startup": authority.browser_startup,
            "audit": authority.audit,
            "audit_health": self.audit.health(),
            "documents": authority.documents,
        });
        self.succeeded(
            context,
            decision,
            None,
            Effect::None,
            Readiness::NotApplicable,
            true,
            Outcome::PolicyExplained {
                capabilities,
                layers,
            },
            facts,
        )
    }
}
