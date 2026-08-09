// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The single operation-completion chokepoint.
//!
//! The operation executor has already reduced private mechanism evidence into one closed typed
//! result. This module performs the only remaining concern: bind workspace-owned opaque handles.
//! It cannot see adapter JSON and cannot rebuild result identity from later mutable browser state.

use crate::hub::workspace::WorkspaceRegistry;
use crate::tool::outcome::OperationCompletion;
use ghostlight_transport::operation::{BrowserResult, Operation};
use ghostlight_transport::workspace_id::WorkspaceId;

/// Bind workspace-owned handles to one already-closed operation result.
pub(crate) fn bind_operation_completion(
    operation: &Operation,
    workspace: Option<WorkspaceId>,
    workspaces: Option<&WorkspaceRegistry>,
    completion: OperationCompletion,
) -> BrowserResult {
    let mut result = completion.result;
    if let (Some(workspaces), Some(workspace)) = (workspaces, workspace.as_ref()) {
        workspaces.enrich_canonical_result_tabs(
            workspace,
            operation,
            &mut result,
            &completion.topology,
        );
    }
    result
}
