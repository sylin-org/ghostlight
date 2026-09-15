# ADR-0175: First-Class Workspace Discovery and Switching

Date: 2026-09-14. Status: Accepted by owner direction.

Builds on ADR-0005, ADR-0007, ADR-0008, ADR-0105, and ADR-0172.

## Context

Under ADR-0105, Ghostlight establishes strict workspace boundaries: each admitted MCP connection
or session operates within an isolated workspace holding its own controlled tabs, target handles,
view handles, and audit logs. Under ADR-0172, Ghostlight introduced cross-workspace tab discovery,
allowing agents to observe tabs held by other workspaces.

When an agent attempts to manipulate a tab owned by another workspace, Ghostlight produces an
`ownership_mismatch` refusal with the contextual recovery advice:
"Switch workspace or call browser_tabs with action list to see tabs in the current workspace."

However, prior to this ADR, an agent had no tool to actually perform that switch. Tearing down
the MCP stdio connection and reconnecting was impossible from inside an autonomous agent loop.
This friction blocked multi-agent collaboration, workspace handoffs, and corrective tab recovery.

## Decision

1. **First-Class Tool: `browser_workspace`**:
   Introduce `browser_workspace` into the advertised catalog with two operations:
   - `action: "list"`: Returns a structured inventory of all currently admitted workspaces,
     including workspace IDs, client labels, tab counts, and their observable tabs.
   - `action: "switch", workspace: "<workspace_id>"`: Switches the active session binding of the
     calling connection to the designated workspace.

2. **Autonomous Session Rebinding**:
   - The service worker loop maintains an atomic reference to the session's admitted workspace.
   - When `browser_workspace` with `action: "switch"` succeeds, the active workspace reference is
     rebound to the target workspace.
   - Subsequent tool invocations (`browser_tabs`, `browser_read`, `browser_click`, etc.) execute
     against the new workspace, granting immediate authority over its tabs and targets.
   - Switching requires read capability on the session and does not alter the underlying browser
     tabs or mutate workspace state.

3. **Validation and Error Recovery**:
   - If the caller requests a workspace that does not exist or has already closed, the tool fails
     with `unknown_workspace` and `Refusal::WorkspaceClosed`, leaving the session's active workspace
     unaltered.
   - The terminal outcome returns `Outcome::WorkspaceSwitched`, providing the safe next step:
     "Call browser_tabs with action list to inspect tabs in the switched workspace."
