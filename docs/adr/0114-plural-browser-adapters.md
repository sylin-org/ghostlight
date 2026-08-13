# ADR-0114: Browsers are plural, and every crossing names the one it means

Status: Accepted

Date: 2026-08-13

Amends: ADR-0113 (liveness is per browser, not per service)

Implements a 1.0-scoped subset of: ADR-0084 (browser-window attention routing)

Builds on: ADR-0061 (extension-owned browser identity), ADR-0096 (`WorkspaceId` as the browser
routing identity), ADR-0098 (extension-owned browser topology)

## Context

The 1.0 service held exactly one browser adapter, in one slot:

```rust
connection: Arc<Mutex<Option<Connection>>>
```

Every new connection replaced whatever was there, failed that connection's pending work, and left
the replaced socket open. That singleton contradicted a standing invariant -- browser instances are
a plural domain collection -- and it produced a live failure that took a full investigation to
explain.

The observed failure, 2026-08-13: a Chromium service worker raced itself at browser startup and
opened two native-messaging ports. Both relays connected. The service kept whichever attached last
and forgot the other. The extension honors messages only from its own most recent port, so when the
service kept the other one, every request was written into a socket whose far end discarded it in
silence. No error, no receipt: work sat until its deadline and returned "Sent, but the browser never
confirmed what happened." The extension-side race is fixed separately, but the race was only the
trigger. The singleton is what turned a duplicate connection into an unrecoverable one, because the
healthy connection had already been forgotten and had no reason to reconnect.

The same singleton makes an ordinary setup impossible to serve. A person running Chrome and Edge at
once -- or two profiles of one browser -- is realistic and desirable. Under one slot, only the last
one to attach is reachable at all.

Physical tab ids make this sharper. They are unique inside one browser and meaningless outside it.
The event path resolved them globally, so Chrome's tab 5 and Edge's tab 5 were one lookup: an Edge
navigation could be governed, applied, and audited against a Chrome tab.

ADR-0084 already decided the full routing contract, including a per-window attention queue and
model-facing browser descriptors. It is explicitly marked "Implementation target: v2". This ADR
takes the part 1.0 needs and leaves the rest where ADR-0084 put it.

## Decision

### D1. One connection per browser identity, and a replaced connection is closed

The service holds a registry keyed by the persistent `browser_id` the adapter mints and stores
(ADR-0061). Two browsers are two entries. A hello carrying an identity that is already registered is
the same adapter arriving twice: it replaces that entry, fails the replaced connection's pending
work, and **shuts the replaced stream down**.

Closing it is the load-bearing half. An abandoned but open socket keeps its relay process alive and
the browser's stale native port alive with it, so neither shore ever learns the connection is
finished. Closing it makes the duplicate collapse on its own: the relay reads end-of-stream and
exits, and the browser observes its port disconnect. Any future duplicate, from any cause, heals
without a person noticing it happened.

### D2. A workspace works in one browser for its whole life

The workspace aggregate owns the binding. The first crossing binds it; every later crossing reads it
back. Tabs, targets, and views are physical things inside one browser, so a workspace that could span
two could not name its own tabs unambiguously.

Physical tab resolution is keyed by `(browser, physical_id)` throughout. Every browser event carries
the identity of the connection that produced it.

When a bound browser disconnects, the binding stays. Work waits for the browser it belongs to and
says so. Browser profiles are different user contexts, not redundant servers (ADR-0084 D4).

### D3. Resolution order

For any crossing that needs a browser:

1. an explicit selection the call named;
2. the browser this workspace is already bound to;
3. the most recently attended connected browser;
4. the only connected browser;
5. otherwise refuse, naming the candidates, with no physical effect.

An explicit selection outranks the automatic default but never an established binding: a workspace
with tabs open in one browser cannot be told to continue in another, because the tabs it owns would
stay where they are.

Resolution happens at the browser seam, not at admission. A call that never reaches a browser must
not need one -- listing a workspace's own tabs answers truthfully with no browser connected at all.

### D4. Attention is reported, never inferred from connection order

Adapters that declare the `adapter_attention` capability report a browser-level attention event when
a window gains focus, and report truthfully at hello whether they already hold a focused window.
Attaching never moves a browser to the front by itself (ADR-0084 D2).

Attention order outlives connections, so a browser that reconnects keeps the place its last reported
attention earned, and it routes only to browsers that are currently connected.

Only the gain is reported. Losing focus tells the resolver nothing that recency order does not
already say. No adapter can prove a focus change came from a person rather than from the browser or
a page, so this is an ergonomic hint and never an authorization fact.

Browser granularity only. ADR-0084's per-window queue, window kinds, and private-context eligibility
remain v2.

### D5. The browser identity is the model-facing handle

`browser_id` is already opaque, content-free, and stable, so it is handed to the model as-is. A
second mapping table would add a lookup without adding a guarantee.

- `browser_tabs` with action `list` gains a `browsers` inventory: handle, reported product name, and
  which one is currently attended. It is the read a caller already makes to learn what is there.
- `browser_navigate` gains an optional `browser` on its new-tab branch. That is the only call that
  can be a workspace's first work; every other call arrives holding a handle that already names its
  browser.

The catalog stays at 22 tools (ADR-0107). The product name is reported by the adapter, bounded, and
never used for routing; it exists so a person or a model can tell two connected browsers apart.
ADR-0084 D6's fuller descriptors (`engine`, `displayName`) remain v2.

### D6. Runtime control reaches every browser

Runtime control is a property of Ghostlight, not of one browser. Publishing it writes to every
connected adapter; one unreachable browser does not hide the state from the rest.

## Consequences

- The duplicate-connection failure class is closed at its root, and closed for causes nobody has
  found yet.
- Chrome and Edge, or two profiles, can be connected and worked in at once, each keeping its own
  tabs, sessions, and governance.
- A person with two browsers open and no attention reported gets a refusal naming both rather than a
  coin flip. This is a deliberate cost: the alternative silently puts work in the wrong signed-in
  context.
- An adapter that predates this ADR still negotiates. The hello fields are additive with defaults,
  attention is capability-gated, and an adapter that reports no attention is routed by binding or by
  being the only browser present.
- `BrowserPort::call` names its browser, and `BrowserEventSink::on_event` names the browser that
  produced the event. Both are compile-enforced, so a future crossing cannot forget to say where it
  is going.
