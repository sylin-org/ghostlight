# ADR-0090: Explicit stale-workspace recovery

Status: Accepted

Date: 2026-07-31

Amends: ADR-0085 Decision 2

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

