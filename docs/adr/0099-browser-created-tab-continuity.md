# ADR-0099: Browser-created tab continuity

Status: Accepted

Date: 2026-08-05

Builds on: ADR-0078's observation language, ADR-0080's per-surface ordering, and ADR-0098's
extension-owned browser topology

Amends: ADR-0098 Decision 4

## Context

Ghostlight adopted tabs created through its explicit topology tools. A page could also open a new
tab or popup during an ordinary browser action. Chrome knew that tab's opener and placement, but
Ghostlight did not add it to the logical workspace until a later inventory call, and the action
result did not tell the agent which tab to use next.

This was especially confusing in sign-in and account-switch flows. A click can navigate the same
tab, open a child tab in the same window, or open a popup in another window. Page instrumentation
is unavailable on protected Chrome pages, so page evidence alone cannot distinguish those cases.

The fix must preserve the existing boundary: Chrome lifecycle and placement belong to the
extension, while workspace authority and public composite tab ids belong to the service. It must
also tolerate independent adapter and service versions.

## Decision

### 1. Exact managed openers establish browser-shore continuity

When Chrome reports a new tab with `openerTabId`, the extension may add it to its logical workspace
only when that opener belongs to exactly one extension topology record. Missing or ambiguous
openers are ignored. A title, URL, group name, active window, timing, or nearby tab is never enough
to infer ownership.

The extension marks an accepted child as managed and persists its workspace membership. It does
not move, group, focus, or otherwise alter the placement Chrome chose. Account popups therefore
stay popups, and tabs opened in a separate window stay in that window.

### 2. One bounded journal correlates transitions with in-flight work

The extension keeps one memory-only tab-transition journal. It records at most 64 recent opened or
closed observations and returns at most 16 observations in one result. An entry carries only the
opaque workspace id internally, the source tab id, the affected tab id, and whether an opened tab
was initially active. It carries no URL, title, page content, credentials, or policy fact.

Per-surface FIFO execution from ADR-0080 gives each request an exact before cursor. A transition is
eligible for that request only when it occurred after the cursor and matches both the request's
opaque workspace id and native source tab id. The result reports what Ghostlight observed while
the call ran. It never claims that the call caused the transition.

### 3. Tab deltas are an explicit adapter result feature

The service opts in on each browser tool frame:

```json
{ "resultFeatures": ["tabDeltaV1"] }
```

A supporting extension may then add:

```json
{
  "tabDelta": {
    "opened": [{ "tabId": 42, "active": true }],
    "closed": [],
    "activeTabId": 42,
    "more": false
  }
}
```

The field is optional. An older extension ignores the request member. A newer extension never
returns a delta to an older service that did not opt in. Passive extension-side adoption still
helps older services on their next explicit topology inventory.

### 4. The service adopts before it exposes

The service strictly validates the bounded delta and extracts only still-open `opened` ids. It
atomically claims their composite ids for the correlated `WorkspaceId` before completing the tool
reply. A malformed delta or cross-workspace collision fails the result; no unusable or unauthorized
id reaches the MCP client.

After adoption, the existing generic result encoder converts opened, closed, and active native ids
to their public composite form. The tool pipeline then adds concise service-authored routing
guidance after page provenance is applied. The guidance says a new tab was observed while the call
ran and names the id to use next. It does not describe page content or claim causality.

### 5. Debug topology is metadata-only and opt-in

When the existing local extension debug flag is enabled, the extension emits bounded lifecycle
breadcrumbs for managed tab open, close, URL-change, attach, detach, and activation events. These
carry tab, window, group, position, status, and opener identifiers only. Full URLs, titles, page
content, workspace ids, and user identity are excluded.

## Consequences

- An agent can continue directly in an observed child tab without polling `tabs_context_mcp`.
- The child is already authorized for that workspace when its id becomes visible.
- User-controlled popup, window, and group placement remains untouched.
- Same-tab account switches correctly return no opened tab while debug evidence identifies the
  existing tab's URL-change lifecycle.
- Missing opener evidence degrades to later explicit inventory instead of a guessed adoption.
- No MCP input schema, process count, transport, permission, or local-only boundary changes.
- This is a real browser-adapter change, so the manifest version advances independently under
  ADR-0093.
