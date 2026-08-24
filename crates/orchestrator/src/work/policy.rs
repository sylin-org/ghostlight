//! Model-facing policy explanation execution (ADR-0136).
//!
//! The one operation that crosses no browser seam: the authority in force is compiled by the
//! governance facade's single projection -- the same one the workbench renders -- and handed to
//! the model with machine-local document texts and paths withheld.

use serde_json::{json, Value};

use crate::governance::Capability;
use crate::language::outcome::Outcome;

use super::{ApplicationExecutor, Effect, InvocationContext, Readiness, Terminal};

impl ApplicationExecutor {
    /// Explain current authority from the orchestrator-owned projection.
    pub(super) fn explain_policy(&self, context: &InvocationContext<'_>) -> Terminal {
        let decision = self.authorize(context, Capability::Read, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
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
