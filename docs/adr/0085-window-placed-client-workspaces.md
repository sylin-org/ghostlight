# ADR-0085: Window-placed client workspaces

Status: Accepted

Date: 2026-07-16

Amends: ADR-0066 Decision 1 (the presentation key gains a browser-window dimension)

Builds on: ADR-0047, ADR-0058, ADR-0061, ADR-0080, and ADR-0084

## Context

Ghostlight used to create a new Chrome window whenever a client needed its first tab group. That
made the automation area obvious, but it also filled the desktop with windows and defeated a
simple user workflow: place a browser window for a recording, then ask Ghostlight to work there.

A tab group is enough visible organization. Users do not read a group as a security boundary, and
Ghostlight must not present it as one. The service's per-session tab ownership remains the
authoritative isolation boundary. The extension's managed-group and managed-tab checks remain a
load-bearing mechanism guard that prevents access to arbitrary user tabs.

ADR-0084 defines the complete v2 multi-vendor routing model. Chromium can deliver the immediate
ergonomic improvement without adding the complete global window-attention protocol: inside one
connected profile, `chrome.windows.getLastFocused()` is a direct, pull-based answer at the moment
the first workspace is needed.

## Decision

### D1. A client workspace follows user-owned window placement

The first unaddressed topology operation in an MCP session selects the most recently focused
eligible normal window in the chosen Chromium profile. It reuses that window. Ghostlight creates a
new normal window only when no eligible normal window exists. This invariant is identical on every
supported Chromium platform, including Windows and Linux.

Window inventory order is never interpreted as z-order. Resolution uses the browser's
last-focused result, a live inventory entry explicitly marked focused, a bounded focus MRU built
from browser-native events and validated against live inventory, then one sole eligible window.
Several eligible windows without those facts are an error with no side effect. Zero eligible
windows permits creation only when live inventory completed successfully. Inventory failure is
unknown, never an empty inventory, and never authorizes another window.

Some Linux window managers report `WINDOW_ID_NONE` immediately before the next focused Chrome
window. The local focus ledger ignores that absence marker and records the following real window
ID in receipt order; it never treats the transient marker as proof that no windows exist.

Ordinary windows are eligible. Incognito, popup, application, DevTools, and disappearing windows
are not silently substituted.

### D2. Selection is pinned once per MCP session

The native tool envelope gains a private additive `workspace` instruction. Before a session is
pinned, the service sends:

```json
{ "workspace": { "select": "last_focused_normal" } }
```

The extension returns the chosen native window ID in private response metadata. The service
removes that metadata before returning the tool result and pins:

```text
session GUID -> browser slot + native window ID
```

Later unaddressed topology calls name that exact native window. An addressed `tabId` remains
stronger than workspace selection. A pinned window that disappears fails truthfully and never
falls into another authenticated window. An automatic selection that disappears before its first
tab is created may be resolved and attempted once more, before mutation.

Before the first pin, client-topology calls serialize through one browser-neutral bootstrap queue.
This prevents simultaneous first calls from racing through different connected browser profiles.
After the pin, they use the ordinary browser-local client-topology queue.

The pin survives native-port and service-worker reconnects. It is cleared when a changed browser
process generation proves native window IDs stale. Like the existing client-key map, session state
is service-memory-only and may survive an MCP transport reconnect that retains the session GUID.

### D3. Visible groups are scoped by browser window and client

ADR-0066's extension presentation key becomes:

```text
browser instance + native window ID + client key -> Chrome tab-group ID
```

The browser instance is implicit in each extension. Sessions from the same client share a group
when they select the same window. The same client working in another window gets another group.
Stored client-only keys are migrated from each live group's actual window.

Ghostlight never moves an existing tab or group to satisfy a workspace pin. If the user moves a
group, the extension rekeys it to its new window and leaves the former window free for a new group.
Out-of-band group requests filter tab IDs to the pinned window so a delayed presentation request
cannot pull a moved tab back.

### D4. Context-establishing operations share one placement path

`tabs_context_mcp`, `tabs_create_mcp`, and unaddressed `navigate` use the same workspace resolver.
Unaddressed navigation reuses the session's own group in the pinned window instead of selecting a
tab from another client's managed group. A newly created browser window reuses Chrome's initial
blank tab so the fallback does not leave litter behind.

The tool schemas and public structured results do not change. Workspace metadata exists only on
the native service-to-extension boundary.

### D5. Window placement is ergonomics, not authority

Recent focus is a placement hint. It is not consent, authentication, a user gesture, policy
approval, or evidence for audit. Governance still evaluates the resolved action in the Rust
service. The visible group is presentation. Per-session tab ownership in the service and the
extension's managed-surface predicate continue to enforce reachability.

### D6. The existing browser-profile selector remains until v2

This change deliberately does not implement ADR-0084's global vendor-neutral window queue,
browser directory, explicit browser selector, or browser provenance. The existing coarse
browser-profile focus chain still chooses among connected Chromium profiles. Within that chosen
profile, this ADR uses a pull-based last-focused-window query and adds no window-focus event or
service-side window MRU state. The extension may retain browser-native focus events in bounded
`storage.session` state as a local recovery fact; it does not send the native window ID to the
service focus chain.

An optional future adapter may add OS window-order evidence when browser-native facts are
insufficient. A process ID can narrow the browser instance but cannot identify or rank several
windows in the same process. Such an adapter must correlate an OS window handle with an
adapter-native window ID, declare platform availability, and fail explicitly where a compositor
such as Wayland does not expose global z-order.

The complete v2 implementation may replace this bootstrap seam with the ADR-0084 resolver. The
session pin and window-scoped presentation key remain valid inputs to that design.

## Consequences

- Repeated demo and ordinary client runs stop opening a new browser window when a usable one is
  already present.
- A user can position a browser for recording before starting work, and Ghostlight uses it.
- One session stays in one window unless it addresses an already-owned tab directly.
- Same-client groups can exist in separate windows without collision or forced movement.
- Window closure is a visible failure rather than an invisible context switch.
- The trained MCP schemas and model token cost are unchanged.
- Full cross-browser focus, selection, and capability-aware routing remain v2 work.

## Rejected alternatives

- **Always create a dedicated window.** This litters the desktop and overrides deliberate user
  placement.
- **Use `windows.getAll()` order as z-order.** Chrome does not promise that meaning.
- **Treat failed inventory as an empty inventory.** Unknown state cannot authorize creating more
  user-visible state.
- **Use browser PID as the window selector.** One browser process can own several top-level
  windows. PID is a candidate filter for a future OS adapter, not a window identity or order.
- **Move an old group into the selected window.** This reverses a user's placement and can drag
  unrelated in-progress tabs across windows.
- **Treat the tab group as the security boundary.** The group is visible organization. Service tab
  ownership is authoritative, with the extension's managed-surface check as a mechanism guard.
- **Send every focus event to the service now.** That is the larger ADR-0084 protocol. A pull at
  the one decision point is smaller and sufficient for the v1 Chromium behavior.
- **Silently fail over after a pin breaks.** Browser windows carry different authenticated state
  and user intent.

## Related decisions

- ADR-0007: sacred trained tool surface.
- ADR-0047: unified session tab identity and ownership.
- ADR-0058 and ADR-0061: browser routing and composite tab IDs.
- ADR-0066: client-scoped tab-group presentation, amended here.
- ADR-0080: client-topology scheduling.
- ADR-0084: complete v2 browser-window attention routing.
