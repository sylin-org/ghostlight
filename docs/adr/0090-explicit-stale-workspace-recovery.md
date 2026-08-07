# ADR-0090: Explicit stale-workspace recovery

Status: Accepted

Date: 2026-07-31

Amends: ADR-0085 Decision 2

## Amendment by ADR-0096 (2026-08-04)

A "known Ghostlight `tabId`" now means a tab owned by the current live `WorkspaceId`, not an id
that merely existed in an older service or browser process. Input ids are verification-only and
cannot recreate ownership. After ownership is lost, recovery goes through a successful
`tabs_context_mcp` or `tabs_create_mcp` result, which establishes fresh membership at the browser
shore. The explicit stale-window retry below remains valid inside that live workspace.

## Superseded by ADR-0098 (2026-08-05)

The service no longer owns a native Chrome window pin, so there is no service-side stale-window
condition or retry. The extension resolves current placement from the workspace's live owned tabs
and groups. Moving them is ordinary browser state, not a recovery event. Exact service-side tab
authority and creator-result admission remain unchanged.

## Context

When a session's pinned Chrome window disappeared, Ghostlight returned a truthful error but gave
the agent no working repair. `tabs_create_mcp` was sent back to the same stale window and failed
again.

## Decision

A known Ghostlight `tabId` remains the strongest target and keeps working directly.

When the extension conclusively reports that the pinned window is unavailable, only an explicit
`tabs_create_mcp` call may recover. Ghostlight retries that blank-tab creation once against the
most recently focused eligible normal window in the same browser profile. A successful result
replaces the stale session pin. No other tool retries or changes workspace.

The error is short and corrective:

```text
That Ghostlight workspace is no longer available. Next step: call tabs_create_mcp to open a fresh tab, or keep using a known Ghostlight tabId.
```

The extension sends a private typed error code so recovery never depends on matching prose. The
trained tool schemas and public result shapes do not change.

## Consequences

- Agents can recover without asking the user to reconnect the extension.
- Navigation and context reads never jump to another window on their own.
- Timeouts, disconnects, inventory failures, and unknown outcomes are never retried.
- Older engines and extensions ignore the additive private error code safely.
