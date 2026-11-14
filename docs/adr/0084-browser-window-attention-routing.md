# ADR-0084: Browser-window attention routing and instance ergonomics

Status: Accepted

Date: 2026-07-15

Implementation target: v2. The routing contract is accepted, but no production implementation is
part of the v1 release line.

Amends: ADR-0058 Decision 3 (focus routing and ambiguity fallback)

Amendment, 2026-11-13: D4 is amended by ADR-0196's amendment of the same day, for one case. A
session whose browser is not connected moves to the only connected browser, which is no choice,
and the move is logged, not announced. Choosing among several browsers is still never silent.

Builds on: ADR-0061 (extension-owned browser identity and service-assigned slots)

## Context

Ghostlight already admits several browser profiles at once. Each extension profile mints a stable
browser ID, the service assigns it a numeric slot, and composite tab IDs route every follow-up call
to the browser that owns that tab. When a call has no tab yet, ADR-0058 uses a browser-level
focus chain as its bootstrap target.

That chain is close to the user experience we want, but it loses information and treats transport
lifecycle as attention:

- `chrome.windows.onFocusChanged` supplies a `windowId`, but the extension discards it and sends
  only `{ "type": "focus" }`;
- the service stores recency by browser slot, so it cannot choose among several windows in the same
  browser profile;
- attach and reconnect move a browser to the front before its cold-start focus check arrives, even
  though connection order says nothing about user intent;
- if several browsers are connected without a focus report, the old design permits a deterministic
  arbitrary fallback;
- `BrowserInfo.focused` means "front of the focus chain," not necessarily the window holding OS
  focus when the diagnostic is read;
- tab-creation and tab-context results do not identify the browser instance that supplied them;
- the model has no compact way to list or select connected browser targets.

The multi-vendor research in `docs/research/19-firefox-browser-adapter-dossier-2026-07.md` makes
these gaps more important. A user may have Chrome and Firefox, two profiles of either, or one
Firefox profile with several windows and containers. The desirable default remains simple: new
work should begin in the browser window the person most recently attended. Once a tab or workflow
exists, its context must not drift.

Full OS z-order is neither necessary nor portable. Browser-native window focus events already give
the one fact needed for an ergonomic default: which eligible browser window most recently gained
attention. This is an attention hint, not authentication and not proof of a physical user gesture.

## Decision

### D1. Automatic bootstrap routing uses a global browser-window attention queue

The service owns one move-to-front queue of attention targets. A target identifies both the stable
browser instance and one adapter-native window:

```text
AttentionTarget {
    browser_id,
    browser_slot,
    connection_generation,
    native_window_id,
    window_kind,
    private_context,
    active_context,
    eligibility,
    recency,
}
```

This is a domain shape, not a pinned Rust type or public wire schema. Adapter-native window IDs stay
behind the browser boundary. The model receives an opaque browser reference and, only where useful,
a service-local window reference.

The queue may contain several windows from one browser instance:

```text
front -> Firefox Personal / window 42
         Chrome Work / window 8
         Firefox Personal / window 17
```

A live focus event moves its target to the front without duplicates. A second window in the same
profile therefore changes the exact tab-creation target, not just the chosen browser name.

The queue is service-owned state. Extensions and future adapters report mechanism facts; they do
not decide routing.

### D2. Connection is not attention

Admitting or replacing a browser connection does not move it to the front. This explicitly repeals
ADR-0058's attach-time focus seeding.

On connection, the adapter sends an initial attention snapshot containing its current window
inventory and the best local focus facts it can truthfully provide. A newly connected instance is
not eligible for automatic bootstrap selection until that snapshot arrives or a bounded snapshot
deadline expires. Explicitly addressed work may still use the connected instance during this
period when its target is otherwise valid.

The snapshot distinguishes:

- a window that is focused now;
- previously observed per-window focus recency, when the adapter retained it;
- a window with no known attention history.

A currently focused eligible window moves to the front. Historical recency reconstructs order
after a service restart when the MCP client, rather than a browser, currently owns OS focus. A
connection with no focus evidence is admitted without disturbing established queue order.

If a snapshot deadline expires:

- one connected eligible browser may be used as the sole unambiguous target;
- several connected candidates produce a browser-selection response with no side effect;
- connection or slot order is never treated as attention.

Live focus-event receipt order is authoritative while the service runs. Bootstrap timestamps are
only restart hints and cannot overwrite a newer live event.

### D3. Focus and window lifecycle messages carry enough mechanism truth

The extension focus message gains the native window ID. The browser wire gains additive window
snapshot, creation, removal, and attention messages. Exact message names and serialization are an
implementation detail, but every accepted event is bound to the already authenticated browser
connection and its generation.

The service applies these invariants:

- a duplicate focus event is an idempotent move-to-front;
- an event from a replaced connection generation is ignored;
- a late bootstrap snapshot cannot override a later live event;
- window removal deletes that live target;
- browser detach removes all its live window targets;
- window creation alone does not imply attention;
- a reconnect does not imply attention;
- losing focus is not an event the resolver needs; recency order is sufficient.

Focus events may result from the user, the browser, a page, or Ghostlight's own explicit focus
operation. Browser APIs do not reliably expose that cause. The system therefore calls this
**recent attention**, never **human focus**, and uses it only as an ergonomic bootstrap hint.

### D4. The resolver chooses context once and never silently fails over

For any operation that needs a browser or window target, resolution order is:

1. An explicit composite tab ID routes to its encoded browser owner and mapped surface.
2. A running script, browser batch, recording, or other multi-step operation uses its pinned target.
3. An explicit browser or window selection uses that target.
4. Automatic bootstrap uses the most recently attended live, ready, eligible target.
5. If there is no attention evidence and exactly one eligible target exists, use it.
6. Otherwise return compact candidates and perform no mutation.

An explicit or pinned target that lacks an operation does not cause a search for another browser.
An established task must never move between authenticated browser contexts because another adapter
has more capabilities. The service reports the limitation and any available alternatives; the
caller chooses whether to change context.

If a selected browser disconnects, its session selection remains a reconnectable identity for the
existing grace behavior. The service does not promote another browser as failover. Browser profiles
are different user contexts, not redundant servers.

### D5. Eligibility is separate from attention

The service records reported attention but selects only a window that is valid for the requested
operation. For ordinary new-tab creation, an eligible target is a live, ready, ordinary browser
window in which the adapter can create a normal tab.

DevTools, extension popups, application windows, windows being destroyed, and other non-tab-bearing
surfaces are not ordinary creation targets. Adapters report window kind and capability rather than
hard-coding Chromium names in the resolver.

Consequential context boundaries never degrade silently:

- a private window is eligible only when Ghostlight is explicitly permitted there;
- lack of private access does not silently redirect private-context work to an ordinary window;
- a Firefox container or future vendor context may be inherited only when the adapter reports it,
  Ghostlight can preserve it, and the result discloses it;
- an ineligible most-recent target produces an explanation or bounded choice when using another
  context would change authentication or privacy semantics.

The first implementation may support only ordinary windows. The domain retains `active_context`
so Firefox containers and similar user-owned contexts can be added without replacing the routing
model.

### D6. Model-facing identity separates routing, reasoning, and presentation

`brand` is not the public vocabulary. Model-facing browser descriptors use distinct fields:

| Field | Meaning |
|---|---|
| `browserRef` | Compact opaque handle the model passes back to Ghostlight. |
| `browserName` | Browser product, such as `firefox`, `chrome`, `edge`, or `brave`. |
| `engine` | Underlying engine family, such as `gecko`, `chromium`, or `webkit`. |
| `displayName` | Safe human-facing instance label, such as `Firefox - Personal`. |
| `adapterMode` | Control mode, such as `cdp`, `extension`, or `hybrid`. |
| `state` | Current readiness, such as `ready`, `degraded`, or `disconnected`. |

Models route with `browserRef`, reason about compatibility with `browserName`, `engine`, and
`adapterMode`, and present `displayName` to the user. Persistent extension UUIDs, profile paths, and
adapter-native window IDs are not model-facing identifiers.

Default display names do not inspect private profile contents. The service may derive a generic
product-plus-ordinal label or use a user-assigned label.

### D7. Context-establishing results carry compact browser provenance

Tab creation, tab context, browser selection, workflow start, capability mismatch, and disconnect
results identify the involved browser. Stable structured results carry browser provenance even in
the single-browser case so models do not depend on a conditional result shape. Human-readable text
may omit redundant browser wording when only one target exists.

The intended compact shape is:

```json
{
  "tabId": 4294967303,
  "browser": {
    "browserRef": "b1",
    "browserName": "firefox",
    "displayName": "Firefox - Personal"
  },
  "selectedBy": "recent_attention"
}
```

`selectedBy` explains the resolver path with a small fixed vocabulary such as explicit tab, pinned
workflow, explicit selection, recent attention, or sole eligible target. Exact JSON field
placement, enum spellings, and output-schema amendments are fixed during implementation review;
the semantic facts above are decided here.

Ordinary calls against an already addressed `tabId` do not repeat the full browser descriptor.
The tab already carries ownership, and repeated metadata would tax model context without improving
the next decision.

### D8. Ghostlight exposes a compact connected-browser directory

The model can list browser instances that are connected to Ghostlight and inspect which one
automatic routing would currently choose. The directory is optimized for choosing a target, not
for dumping every adapter capability:

```json
{
  "selectionMode": "auto",
  "currentBrowser": "b1",
  "browsers": [
    {
      "browserRef": "b1",
      "browserName": "firefox",
      "engine": "gecko",
      "displayName": "Firefox - Personal",
      "adapterMode": "hybrid",
      "attention": "most_recent",
      "state": "ready"
    }
  ]
}
```

The directory lists connected instances, including a concise degraded state where useful. It does
not present an installed-but-disconnected browser as targetable. Browser installation discovery and
repair guidance belong in `ghostlight doctor`.

The exact additive tool name, action enum, and optional selector placement on bootstrap tools are
deferred to implementation review. That review must preserve the trained fields of the sacred
schemas. This ADR requires the directory and explicit-selection behavior, not a particular schema
spelling invented before its token and fidelity tests exist.

### D9. Auto-follow is the default; explicit selection and workflow pinning are reversible

The user-facing vocabulary is:

- **Auto**: use recent attention for new, unaddressed work;
- **Selected**: use an explicitly chosen browser for this MCP session;
- **Pinned**: keep an active workflow on the context where it began.

Explicit selection is reversible by returning to Auto. It does not rewrite existing tab ownership.
A future Console control and the model-facing browser directory use the same service-owned
selection state.

Changing browser selection is locally auditable but requires no page capability by itself. It
changes future routing; it does not authorize future operations, weaken tab ownership, or bypass
the ordinary RAWX decision for the eventual call.

Automatic tab creation may activate the new tab inside the chosen browser window but does not force
that browser application to steal OS keyboard focus. User attention is an input to routing, not a
license to interrupt the person.

### D10. Attention data is local, minimized, and never governance evidence

Attention messages contain identifiers and bounded context classifications only. They do not carry
window titles, URLs, page content, form values, or screenshots.

Diagnostics and audit may record:

- stable internal browser identity;
- `browserName`, `engine`, and `adapterMode`;
- safe display label;
- selected-by reason;
- ordinary/private/container context classification when relevant;
- native/composed/degraded operation fidelity when adapter support exists.

They do not treat attention recency as user consent, authentication, policy approval, or proof of a
physical gesture. Governance evaluates the resolved operation and authoritative page context in the
service before adapter dispatch, exactly as before.

## Consequences

- A new tab opened without an explicit target follows the user's most recently attended eligible
  browser window, including the correct window within one profile.
- Browser reconnect order can no longer redirect new work.
- Ambiguity produces a bounded choice without an accidental tab.
- Models learn the browser involved without an extra discovery call in the common path.
- Users can reason in Firefox/Chrome and friendly labels while adapters retain precise engine and
  mode facts.
- Existing tab ownership, workflow pinning, governance, and audit remain authoritative over focus.
- Future Firefox containers and other vendor contexts have a place in the model without making them
  implicit or weakening privacy boundaries.
- The current implementation is intentionally non-conformant until follow-up work removes
  attach-time promotion, carries window IDs, introduces the window queue, enriches results, and
  adds browser directory/selection behavior.
- `BrowserInfo.focused` must be replaced or clarified as recent attention; a diagnostic must not
  claim current OS focus from an MRU queue.
- More lifecycle state is retained per connected browser, but it remains small, local, and
  content-free.

## Rejected alternatives

- **Full OS z-order enumeration.** Rejected as unnecessary, platform-specific, and unreliable under
  Wayland and other compositor boundaries.
- **Connection order as a fallback.** Rejected because reconnect timing is transport behavior, not
  user intent.
- **A browser-only queue.** Rejected because it cannot choose among several windows in one profile.
- **A window-only public identity.** Rejected because models need a stable browser instance for
  reasoning and explicit reuse; native window IDs remain adapter details.
- **`brand` as the browser field.** Rejected because it conflates product, vendor, engine, routing
  identity, and presentation.
- **Arbitrary deterministic selection when several targets are unknown.** Rejected because a stable
  wrong side effect is still wrong.
- **Capability-driven silent browser switching.** Rejected because browser profiles carry different
  accounts, tabs, and user intent.
- **Returning the full browser descriptor on every action.** Rejected as repetitive model-context
  cost once a composite tab ID already establishes ownership.
- **Listing installed browsers as targets.** Rejected because installation does not mean a live,
  authenticated Ghostlight connection exists.
- **Treating focus as user approval.** Rejected because focus cause is not reliably knowable and
  attention is not authorization.

## Explicitly deferred

- Production implementation of this ADR's attention queue, browser directory, explicit selection,
  and browser provenance behavior. These are one coherent v2 workstream rather than incremental v1
  surface changes.
- The vendor-neutral typed `BrowserOperation` and `BrowserAdapter` interfaces.
- Universal service-minted surface handles for adapters whose native context IDs are strings.
- Firefox extension-only versus hybrid product support.
- Firefox extension-to-Marionette/BiDi pairing.
- The exact additive browser-directory tool schema and selector parameters.
- The first supported private/container context inheritance rules beyond the invariants above.
- Adapter capability negotiation and dynamic `initialize`/`explain` rendering.

Those decisions build on this routing contract but require the Firefox proof of concept and their
own implementation evidence.

## Related decisions

- ADR-0007: sacred trained tool surface.
- ADR-0034: capability registry and additive tool growth.
- ADR-0047: unified session and tab-surface identity.
- ADR-0058: per-browser identity and focus-chain routing, amended here.
- ADR-0059: developer instrumentation and fake-browser focus controls.
- ADR-0061: extension-owned browser identity and service-assigned tab slots.
- ADR-0066: client-scoped tab groups and owned surfaces.
- ADR-0078: closed-loop browser core and explicit tab controls.
- ADR-0080: resource-scoped browser command scheduling and workflow pinning.
- Research 19: Firefox and browser-adapter capability dossier.
