# ADR-0172: Cross-workspace tab discovery and informative ownership refusals

Date: 2026-09-14. Status: Accepted by owner direction.

Builds on ADR-0010, ADR-0050, ADR-0157, ADR-0161, and ADR-0170.

## Context

Ghostlight enforces strict workspace authority boundaries (ADR-0010). Each client connection
or admitted session operates within an isolated workspace aggregate owning its own opaque tab
handles (`tab_...`), target handles (`target_...`), and view handles (`view_...`).

When a client or LLM agent attempts to interact with a tab handle that belongs to a different
workspace:

1. `WorkspaceLease::select_tab` detects that the requested handle is not present in the current
   workspace's tab map, but is present in another admitted workspace's tab map.
2. The orchestrator returned a generic `WorkspaceError::NotOwnedTab`, which translated to a generic
   refusal:
   - Summary: `"That handle belongs to a different Ghostlight session."`
   - Next steps: `["Collect fresh handles from this session, then continue with those."]`
   - Facts: `{"reason": "ownership_mismatch"}`
3. While correct and preventing cross-workspace authority confusion, this response lacked
   actionable discovery: in multi-agent or multi-workspace environments, an LLM agent could not
   tell *which* workspace owned the tab, nor whether it should switch workspaces or refresh
   handles within its current workspace.

## Decision

1. **Informative Ownership Refusals**:
   When a tab handle exists in another admitted workspace, the orchestrator identifies the owning
   workspace by its descriptive client label (or workspace identifier if no label is set).

2. **Strict Privacy Boundary**:
   Only the owning workspace's identity/label is surfaced. Page content, URLs, document structure,
   and cookies from the owning workspace remain strictly partitioned and inaccessible to the
   requesting workspace.

3. **Contextual Summary and Guidance**:
   - Structured facts include `"owner_workspace": "<owner_label_or_id>"`.
   - The authored summary explicitly states:
     `"That tab handle belongs to <owner>."`
   - The contextual `next_steps` offer direct actionable guidance:
     `"Switch workspace or call browser_tabs with action list to see tabs in the current workspace."`

4. **Preserved Generic Fallback**:
   When the owning workspace is unknown or if the resource cannot be attributed to a specific
   active workspace, the existing generic ownership mismatch summary and next steps remain the
   truthful fallback.
