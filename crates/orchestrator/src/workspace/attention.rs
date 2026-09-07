//! Session-local human review state, erased with the owning workspace.

use super::{WorkspaceError, WorkspaceId, WorkspaceStore};
use serde::Serialize;

/// Closed causes that require explicit human recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionReason {
    RepeatedDenials,
    CredentialHandoff,
}

/// The first outstanding incident; later rejected requests cannot replace its evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SessionAttention {
    /// Opaque incident identity, separate from its triggering composition.
    pub id: String,
    /// Invocation whose retained history explains this incident.
    pub invocation: String,
    /// Closed recovery cause.
    pub reason: AttentionReason,
}

impl WorkspaceStore {
    /// Read the current review requirement without acquiring the operation lease.
    pub fn attention(&self, workspace: &WorkspaceId) -> Option<SessionAttention> {
        self.lock()
            .workspaces
            .get(workspace)
            .and_then(|state| state.attention.clone())
    }

    /// Enter attention once; report whether this call created a new incident.
    pub fn require_attention(&self, workspace: &WorkspaceId, attention: SessionAttention) -> bool {
        let mut state = self.lock();
        let Some(workspace) = state.workspaces.get_mut(workspace) else {
            return false;
        };
        if workspace.attention.is_some() {
            return false;
        }
        workspace.attention = Some(attention);
        true
    }

    /// Clear only the incident a human reviewed; stale UI cannot clear a newer incident.
    pub fn resume_attention(
        &self,
        workspace: &str,
        incident: &str,
    ) -> Result<bool, WorkspaceError> {
        let mut state = self.lock();
        let workspace = state
            .workspaces
            .get_mut(&WorkspaceId(workspace.into()))
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        if workspace
            .attention
            .as_ref()
            .is_some_and(|attention| attention.id == incident)
        {
            workspace.attention = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
