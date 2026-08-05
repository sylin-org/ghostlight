# ADR-0098: Extension-owned browser topology

Status: Accepted

Date: 2026-08-05

Supersedes: ADR-0085 Decisions 2-4, ADR-0090's stale native-window recovery, and
ADR-0097's window-qualified workspace presentation

Builds on: ADR-0096's `WorkspaceId` authority boundary

## Context

Ghostlight split one browser workspace across two topology authorities. The service pinned a
`WorkspaceId` to a browser slot plus native Chrome window id. The extension separately tracked
the workspace's Chrome group and repaired that group when the user moved it to another window.

Those rules conflict. After the user moves a whole managed group, the extension sees the group in
its new window while the service continues sending the old window id. A later topology call then
reports an empty workspace even though its tabs and group are live. Retrying, rekeying, or adding
another recovery state would preserve the false premise that the service can own user-controlled
Chrome placement.

The service must remain authoritative for workspace and tab ownership. That does not require it to
know where Chrome currently presents those tabs.

## Decision

### 1. The service owns whose work; the extension owns where it appears

The service remains authoritative for:

- `WorkspaceId` lifecycle and bearer authority;
- exact tab ownership and cross-workspace rejection;
- governance, audit, and scheduling;
- selection of the connected browser profile; and
- composite tab-id encoding and addressed-call routing.

The extension is authoritative for Chrome-native mechanism:

- normal-window selection;
- tab creation and live tab discovery;
- tab-group creation, reuse, naming, and placement;
- following user-moved groups and tabs; and
- browser-session persistence of native tab and group ids.

Native window ids never enter service state. A Chrome group or window is presentation, not
authority.

### 2. The private tool envelope carries identity and presentation, not placement

Workspace topology requests keep `guid` as the `WorkspaceId` and carry one private presentation
field:

```json
{
  "guid": "<WorkspaceId>",
  "workspace": { "groupTitle": "Ghostlight - <client label>" }
}
```

The service no longer sends `workspace.windowId` or `workspace.select`. The extension no longer
returns `_ghostlightWorkspace` metadata. There is no stale-window error or replacement pin.

The service may bind a workspace to a connected browser profile when required for unaddressed
routing. That binding contains only the service-local browser slot. It never identifies a Chrome
window.

### 3. One extension record owns each workspace's browser topology

The extension keeps one browser-session record per `WorkspaceId`:

```text
WorkspaceId -> { tabIds, groupId? }
```

Several records may reference the same visible exact-title group. Their tab sets remain separate.
The record is persisted in `chrome.storage.session`; closed tabs and dead groups are pruned against
live Chrome state.

On every topology call, the extension reads the workspace's tabs from Chrome. It never filters
them through a service-supplied historical window. If the group moved, its live group id and
window win. If a tab was detached or moved separately, it remains reachable without being moved
back.

When creating a tab, the extension chooses placement in this order:

1. the live group containing the workspace's most recently accessed grouped tab;
2. the window of the workspace's most recently accessed live tab; or
3. the most recently focused eligible normal window, creating one only when none exists.

For a workspace with no live tabs or group, an exact-title group in the selected window is reused.
Otherwise a group is created there. Existing tabs are never moved to satisfy a placement choice.

### 4. Creator completion is the only membership synchronization

`tabs_context_mcp`, `tabs_create_mcp`, and unaddressed `navigate` update extension topology
atomically while handling the correlated tool request. The service validates creator results and
adopts their composite tab ids into its authoritative registry as before.

The fire-and-forget `group_request` path is removed. It duplicated extension state after the
creator result, introduced an asynchronous group-id race, and became unnecessary once the browser
shore owns placement.

### 5. User movement is ordinary state, not recovery

Moving a tab or whole group is supported browser behavior. It does not invalidate a workspace,
trigger a service recovery path, or authorize another tab. The extension follows live Chrome
objects; the service continues enforcing the same logical ownership.

## Consequences

- Moving a whole Ghostlight group to another window preserves the next context result.
- Moving one owned tab out of its group preserves access without pulling it back.
- Same-title workspaces can share one visible group without sharing tab ownership.
- The service loses its native-window registry, private result metadata, and stale-pin retry.
- The extension loses window-qualified workspace keys and asynchronous grouping reconciliation.
- The public MCP schemas and process count do not change.
