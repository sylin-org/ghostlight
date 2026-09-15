//! Model-facing workspace discovery and switching execution (ADR-0175).

use serde_json::{json, Value};

use crate::governance::Capability;
use crate::language::outcome::Outcome;
use crate::language::ManageWorkspace;
use crate::workspace::{WorkspaceError, WorkspaceId};

use super::{ApplicationExecutor, Effect, InvocationContext, Readiness, Terminal};

impl ApplicationExecutor {
    /// List admitted browser workspaces or switch the active session workspace.
    pub(super) fn manage_workspace(
        &self,
        context: &InvocationContext<'_>,
        value: &ManageWorkspace,
    ) -> Terminal {
        let decision = self.authorize(context, Capability::Read, None);
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                None,
                Effect::None,
                true,
                json!({"reason": decision.reason.as_str()}),
            );
        }
        match value.action.as_str() {
            "list" => {
                let summaries = self.workspaces.summaries();
                let count = summaries.len();
                let workspace_list: Vec<Value> = summaries
                    .into_iter()
                    .map(|summary| {
                        let is_active = summary.id == context.workspace.as_str();
                        json!({
                            "workspace": summary.id,
                            "client_label": summary.client_label,
                            "tab_count": summary.tab_count,
                            "active": is_active,
                            "leased": summary.leased,
                            "held_tab_count": summary.held_tab_count,
                        })
                    })
                    .collect();
                let facts = json!({
                    "workspaces": workspace_list,
                    "count": count,
                    "active_workspace": context.workspace.as_str(),
                });
                let outcome = Outcome::WorkspacesListed { count };
                self.succeeded(
                    context,
                    decision,
                    None,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    outcome,
                    facts,
                )
            }
            "switch" => {
                let target = value
                    .workspace
                    .as_deref()
                    .expect("validated workspace handle");
                let target_id = WorkspaceId::from(target);
                if !self.workspaces.exists(&target_id) {
                    return self.workspace_failure(context, WorkspaceError::UnknownWorkspace);
                }
                if let Some(conn) = self.workspaces.single_connection(context.workspace) {
                    if let Some(conn_id) = conn.attribution().connection_id.as_deref() {
                        let _ = self.workspaces.switch_connection(
                            context.workspace,
                            &target_id,
                            conn_id,
                        );
                    }
                }
                let facts = json!({
                    "workspace": target,
                    "switched": true,
                    "switched_workspace": target,
                    "previous_workspace": context.workspace.as_str(),
                });
                let outcome = Outcome::WorkspaceSwitched {
                    workspace: target.to_owned(),
                };
                self.succeeded(
                    context,
                    decision,
                    None,
                    Effect::None,
                    Readiness::NotApplicable,
                    true,
                    outcome,
                    facts,
                )
            }
            _ => unreachable!("validated action"),
        }
    }
}
