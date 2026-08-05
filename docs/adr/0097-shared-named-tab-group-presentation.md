# ADR-0097: Shared named tab-group presentation

Status: Accepted; implementation superseded by ADR-0098

Date: 2026-08-05

Amends: ADR-0066 Decisions 1-4, ADR-0085 Decision 3, and ADR-0096's rule that two
same-named workspaces remain distinct at the browser shore

Builds on: ADR-0096's `WorkspaceId` authority boundary

## Superseded in implementation by ADR-0098 (2026-08-05)

The exact-title shared-presentation and separate per-workspace tab-inventory goals remain. The
window-qualified maps, stable pinned-group rule, and service-authored `group_request` refresh path
are superseded. One extension-owned workspace topology record now follows live owned tabs and
groups across Chrome windows. The service sends only the desired group title and owns no Chrome
placement state.

## Context

ADR-0096 correctly made `WorkspaceId` the sole routing, ownership, scheduling, and authority key.
It also keyed each visible Chrome group by that opaque workspace. A restarted MCP edge mints a new
workspace, so the extension cannot find the prior group even when the new work has the same human
label in the same Chrome window. It creates another group with the same title. Repeated recovery
attempts can therefore fill one window with visually indistinguishable Ghostlight groups.

A Chrome tab group is presentation, not authority. Several workspaces may safely place their tabs
in one visible group only if Chrome group membership is no longer used as the tab inventory for a
particular workspace.

## Decision

### 1. Exact title reuses one group inside one Chrome window

The service includes the workspace's desired group title in the existing private workspace
placement instruction. The title is presentation only. It never replaces `WorkspaceId` in
`guid`, selects browser authority, or enters an MCP schema or result.

Before creating a group for an unmapped workspace, the extension looks for an exact-title group in
the selected native window. An already managed exact-title group wins, then the lowest live
exact-title group id for deterministic recovery. The workspace maps to that group and new tabs
join it. If no match exists, the extension creates one group with the desired title. Once a
workspace has a live group in its pinned window, that mapping is stable. A later asynchronous
presentation request may refresh its title and tabs but cannot replace the group id returned by
the creator call.

Chrome groups cannot span windows. The same title may exist in two windows because reusing it
globally would require moving user-placed tabs. ADR-0085's no-move rule remains intact.

### 2. Workspace tab inventory is independent of presentation groups

The extension keeps a browser-session-only `WorkspaceId -> tab ids` index. Successful tab
creation adds the new tab. Each service-authored `group_request` replaces that workspace's entry
with the service's exact owned-tab list. Tab closure removes the id from every entry.

`tabs_context_mcp`, `tabs_create_mcp`, and unaddressed navigation read this index rather than every
tab in the shared Chrome group. This prevents one workspace from observing or claiming another
workspace's tabs merely because their human labels match.

The index uses `chrome.storage.session`, like the existing group and managed-tab maps. A
service-worker restart restores it while Chrome tab ids remain valid. A full browser restart
clears it; the service's existing browser-generation handling also purges stale tab ownership.

### 3. Existing authority and safety rules do not change

`WorkspaceId` remains the only browser routing and ownership identity. The service registry still
authoritatively admits creator results and rejects unknown or cross-workspace input handles. The
extension's managed-tab predicate remains a defense-in-depth reachability check. A group title
grants no access.

Existing duplicate groups are not deleted automatically. Ghostlight does not close or move user
artifacts as cleanup. Future work adopts a deterministic existing match and stops creating new
duplicates.

## Consequences

- Reconnecting the same named client in the same window adds tabs to the existing visible group.
- Same-name workspaces share presentation without sharing routing, tab inventory, or authority.
- The first topology call can reuse the group immediately; no temporary duplicate title is needed.
- The change adds one meaningful browser-shore state map and no process, service, protocol handler,
  registry, or public tool field.
