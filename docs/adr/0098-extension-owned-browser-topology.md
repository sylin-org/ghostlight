# ADR-0098: Extension-owned browser topology

Status: Accepted

Amended by ADR-0186 for request-scoped background placement, no unowned adoption or duplicate
group movement, and explicit operator foreground opt-out. The original decision remains below.

Correction, 2026-10-13: that ADR-0186 amendment is an unpublished trial and is not in this tree.
Tab reuse is decided by the service from released Ghostlight tabs only (ADR-0196 Decision 9); the
extension never picks a tab to adopt.

Amended 2026-10-21: a group's title is the ghost emoji and the agent's name, and the extension
retitles a group 2.0.0 named `Ghostlight - <agent>` (amendment below).

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

Amended by ADR-0099: exact browser-created children may also establish continuity through their
managed opener, with service adoption required before their ids are exposed.

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

## Amendment (2026-10-21): the ghost names the group

The owner asked whether the ghost emoji should replace the `Ghostlight - ` prefix and approved it
after a UX review. A group's title is now the ghost emoji (U+1F47B), a space and the agent's name as
the Ghostlight window names it. Decision 2's `groupTitle` example shows the form 2.0.0 sent; nothing
else in the envelope changes.

The review:

- A group's name sits in a small chip in the tab strip. `Ghostlight - ` spent thirteen characters
  there on the product before the agent's name, which is what tells two groups apart. The ghost and
  its space take two, and the agent comes first.
- A tab group's only settable properties are its title, one of nine colors, and whether it is
  collapsed. There is no icon and no separate accessible name, so the mascot cannot be drawn there,
  and a screen reader announces the title. It reads the emoji as "ghost", which still says whose
  group it is.
- The ghost needs an emoji font. Most desktops ship one. A system without one shows a placeholder
  box before the agent's name, and the name still reads. The four test machines look at the tab
  strip before the release; the Alpine one is the likeliest to lack the font.
- The color stays blue.

The mechanics:

- The prefix is the protocol 4 vocabulary's `group_title_prefix`, pinned by the service's golden
  test and the extension's. The service composes the title in `language/provenance.rs`.
- The vocabulary's `legacy_group_title_prefixes` keeps `Ghostlight - `. When no group carries the
  current title, the extension retitles the lowest-id group that carries the agent's old title, and
  it merges any other such group into the current one. A person who updates keeps one group per
  agent. A group with any other title is never retitled.
- The popup names the agent after either prefix. An updated extension beside a 2.0.0 service, which
  still sends the old title, keeps the old groups and names their agents.
- A 2.0.0 extension beside an updated service opens the ghost group next to the old one and shows
  the whole title in its popup. When the extension updates, the agent's next open merges the two.
- The packaged `lib/notice.js`, `lib/protocol.js` and `lib/tabs.js` change, and the freeze golden is
  regenerated (ADR-0200 Decision 16). The owner withdrew the 2.0.1 store candidate before this
  change, so the next candidate carries it.
