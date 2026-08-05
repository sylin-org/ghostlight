# ADR-0047: Unified session and tab-surface identity

Date: 2026-07-08
Status: Accepted
Supersedes: the per-(re)connect guid-minting posture inside ADR-0045's relay (its Decision 1
"a reconnect is a NEW session" framing); the PINS.md SS6 per-session group-title format from the
hub batch (`docs/tasks/hub/PINS.md`). Amends: ADR-0030 (Decisions 6/7 presentation mechanics),
ADR-0045 (relay classification + session identity across reconnects).

## Amendment by ADR-0096 (2026-08-04)

ADR-0096 replaces session identity with `WorkspaceId` and supersedes every first-touch ownership
rule in D3, D5, and D7. An explicit input `tabId` is verification-only: the current live
workspace must already own it. Unknown and cross-workspace ids fail identically before any
browser frame. Only a successful, correlated `tabs_context_mcp` or `tabs_create_mcp` result may
atomically adopt the exact root `structuredContent.tabId` and
`structuredContent.tabs[].tabId` values it declares. A browser-process or service restart purges
the corresponding in-memory ownership; recovery then goes through a creator tool, never through
presenting an old id. D1's managed-surface defense and the no-auto-close user-artifact rule remain.

## Context

The first complete end-to-end test of the three-executable chain (ADR-0046, 2026-07-08) drove a
real MCP client through `ghostlight-adapter-agent`, the `ghostlight` service, and
`ghostlight-adapter-browser` into a live Chrome. The chain itself worked; the tab-handling layer
failed deterministically and an architecture review found the failures were symptoms of an
identity model that was never unified:

1. **Two tab-group systems fight over one tab.** The extension's tool gate (`inGroup` /
   `groupTabs` / `effectiveTabId` in `extension/service-worker.js`) recognizes exactly one
   process-global group titled `\u{1F47B}Ghostlight`. The H7 feature (ADR-0030 Decisions 6/7)
   moves a session's tabs into a per-session group titled `\u{1F47B} Ghostlight <guid8>`. A
   Chrome tab belongs to exactly ONE group, so the per-session move EVICTS the tab from the
   group the tool gate requires; the next tool call on that tab fails with "the group has no
   tabs". The H7 executor flagged this exact interaction in its ledger as real and unsolved
   (`docs/tasks/hub/LEDGER.md`, H7 entry); the H7 module header's claim that per-session groups
   are "ADDITIVE to (never a replacement of)" the global mechanism is false at the Chrome API
   level.

2. **Session identity dies on reconnect.** The resilient adapter (ADR-0045) replays the MCP
   handshake so the client rides through a service restart -- but the relay mints a FRESH
   `SessionGuid` per (re)connect (`crates/transport/src/ipc.rs`, `try_connect_once`), and its
   own comment declares that "exactly right". It is not: the service keys `owned_tabs` and the
   extension keys its persisted `sessionGroups` map on the guid, so every reconnect orphans the
   session's tab ownership and its Chrome group and mints a new group. The machinery for stable
   re-presentation already exists and is unused: the guid is adapter-minted (ADR-0030 Decision
   4) and `SessionRegistry::admit` sanctions same-user re-presentation.

3. **Born-global, adopted-later churn.** `tabs_create_mcp` places new tabs in the global group;
   the first explicit touch then migrates them to the session group. Group membership flaps by
   design, and the bootstrap path (`ensureGroup(true)`) creates a whole new window holding an
   `about:blank` tab that stays behind as litter.

4. **No lifecycle.** Sessions end; their groups, `sessionGroups` entries, and `owned_tabs`
   entries persist forever. Orphaned `Ghostlight be46ac..`-style groups and stray tabs
   accumulate in the user's real browser, and a dead session's ownership can lock a long-lived
   tab away from every future session (`claim_tab` refuses any different guid, live or dead).

5. **Identity leaks into presentation.** Group identity is recovered by title-string matching,
   and the title shows a truncated guid that means nothing to a human.

Two constraining facts, verified in the tree, shape the solution space:

- Every tab-scoped tool's sacred schema REQUIRES `tabId` (`crates/core/src/browser/directory.rs`
  `"required"` arrays; only `tabs_context_mcp` and `tabs_create_mcp` omit it, and both are free
  actions). Argument validation (`crates/core/src/mcp/validation.rs`) rejects a missing required
  field BEFORE dispatch, so a tab tool can never reach the pipeline without a `tabId` via MCP.
  The extension's implicit "group's active tab" fallback is therefore unreachable from the MCP
  path, and `Governance::authorize`'s fail-open arm 4 (`resource == None` with non-empty
  requires -> `Gate::Proceed`) is not reachable for tab tools through this server. No implicit-
  resolution back-channel is needed.
- ADR-0030 Decision 6 places cross-session isolation authoritatively in the SERVICE
  (`check_tab_ownership` runs before dispatch); the extension's group checks are defense-in-
  depth that scope tools to Ghostlight-managed tabs, never the security boundary. Widening the
  extension's membership predicate therefore removes no real isolation.

## Decision

### D1. The managed surface (extension)

The extension has ONE membership concept: a tab is in-surface iff its `groupId` is one of the
Chrome tab-group ids the extension itself manages -- the legacy global group (`groupId`) plus
every value in its own `sessionGroups` map (which it already persists and rehydrates). The tool
gate (`inGroup`, `groupTabs`, and through them `effectiveTabId`) consults this set; the
predicate is a pure, unit-tested function in `extension/lib/grouping.js`. Title matching remains
only as the legacy recovery path for the global group. This is defense-in-depth per ADR-0030
Decision 6; the service's ownership gate remains the isolation boundary.

### D2. Stable per-process session identity (transport)

`ghostlight-adapter-agent` mints its `SessionGuid` ONCE per process, before the reconnect loop,
and re-presents the SAME guid on every reconnect. `SessionRegistry::admit`'s same-user reuse
path admits it; `serve_session` binds the same guid; the shared `owned_tabs` map and the
extension's persisted `sessionGroups` map therefore keep working across the gap. Effects:

- Within one service lifetime: ownership and the Chrome group survive a reconnect.
- Across a service restart: the new service's registry and `owned_tabs` start empty, but the
  guid is stable and the extension's `sessionGroups` map is persisted, so re-adoption reuses
  the SAME visible Chrome group (idempotent `groupSessionTabs`). The user sees one stable group
  per editor session, not one per reconnect.

This supersedes the "a reconnect is a NEW session ... exactly right" posture written into the
relay. A NEW adapter process (editor restart) is still a new session with a fresh guid.

### D3. Session-scoped tab operations (wire + both sides)

The native `tool_request` envelope gains an additive `guid` field:
`{ "id", "type": "tool_request", "tool", "args", "guid" }`. The native envelope is NOT the
sacred surface (only the MCP tool schemas are; the envelope changed additively before, e.g.
`group_request`). With the calling session known at dispatch:

- `tabs_create_mcp` births the new tab DIRECTLY into the calling session's group. First tab of
  a session: create one focused window and group its single fresh tab as the session's group
  (no `about:blank` litter, no second tab); later tabs: create a tab in that group's window and
  group it immediately. No born-global-then-migrate churn. The handler grouping a tab it just
  created for the guid stamped on the request IS grouping on service instruction; the H7
  "groups on request only" oracle is reinterpreted accordingly (its test, which asserts
  `groupSessionTabs` is never called spontaneously, still holds verbatim).
- `tabs_context_mcp` reports the CALLING session's group (its Chrome group id as `mcpGroupId`,
  its tabs as `tabs`); `createIfEmpty: true` births the session's group the same way
  `tabs_create_mcp` does. It does not list other sessions' tabs or unmanaged tabs.
- The SERVICE claims a session-created tab the moment the `tabs_create_mcp` response reports it
  (`structuredContent.tabId`), so another session can never first-touch-steal a freshly created
  tab, and the claim emits the ordinary group request (which retitles the extension's
  placeholder title, below).
- A `tool_request` arriving WITHOUT `guid` (a legacy or hand-rolled native caller) falls back
  to today's global-group behavior in the extension.

Explicit-`tabId` calls keep the existing service-side ownership gate unchanged. The
first-adoption `group_request` race against the adopting call's own dispatch becomes harmless
under D1 (the tab is in SOME managed group at every instant), so grouping stays fire-and-forget.

### D4. Presentation follows client identity (service)

The per-session Chrome group title is derived from the MCP client's self-reported identity, not
from the guid: `\u{1F47B} <clientInfo.name>` (example: the ghost glyph + " Claude Code"), with a
` (2)` / ` (3)` ... suffix when another live session already holds the same base title, and the
literal name `Ghostlight` as the fallback when no clientInfo was captured. The title is computed
once per guid in a service-lifetime title registry and reused on every subsequent group request
for that guid. This SUPERSEDES the PINS.md SS6 pinned format `\u{1F47B} Ghostlight <guid8>` and
the `group_title` function + pinned test that implement it. Titles are presentation only;
identity never round-trips through them (D1's predicate is id-based).

### D5. Ownership liveness (service) and group-map hygiene (extension)

The service tracks which guids currently have a live session (a per-guid counterpart of the
existing `live_sessions` counter, maintained by the same RAII pattern in `serve_session`). The
ownership gate refuses a tab claim only when the owning guid has a LIVE session; a tab owned by
a dead session is reassigned to the claiming session (first-touch adoption from the dead). No
timers, no background GC: the check runs where the conflict surfaces. The extension prunes
`sessionGroups` entries whose Chrome group no longer exists when it rehydrates. Tabs themselves
are user artifacts: nothing ever auto-closes a tab or dissolves a group that still has tabs.

### D6. Relay classification correctness (amends ADR-0045)

The adapter's service-to-client relay direction distinguishes WHICH side failed: a read error
from the service pipe classifies as `ServiceClosed` (reconnect), exactly like a clean service
EOF; only a write error toward the client classifies as `ClientClosed` (exit). The previous
`tokio::io::copy`-based arm collapsed both error kinds into `ClientClosed`, which on Windows
(where an abrupt service death often surfaces as `ERROR_BROKEN_PIPE` on the read) would exit
the adapter -- forcing the very client reload ADR-0045 exists to prevent.

### D7. Explicit non-decisions

- No implicit-tab resolution machinery (resolvedTabId back-channels, pre-dispatch tab
  resolution): unreachable via MCP because the sacred schemas require `tabId` (Context).
  The extension's implicit fallback stays as inert legacy for non-MCP callers.
- No cross-service-restart persistence of `owned_tabs` or the title registry: group continuity
  across restarts comes from the stable guid + the extension's persisted map (D2), which is the
  user-visible part; ownership re-forms by first touch.
- No listing of unmanaged or other sessions' tabs in `tabs_context_mcp` (D3). A legacy global
  group inherited from a pre-0047 install remains reachable by explicit `tabId` (D1 accepts it;
  first touch adopts and regroups it into the session's group).
- `group_request` stays fire-and-forget (D3 rationale); `PER_PEER_GROUP_CAP` stays reserved.

## Consequences

- The e2e blocker (F4) is closed at its architectural root: there is one membership concept,
  and every grouping the system performs keeps the tab inside it.
- ADR-0045's promise becomes true at the session layer: a reconnect preserves what the client
  can see and touch, not just the transport conversation.
- The user's browser stops accumulating orphan groups, about:blank tabs, and extra windows; the
  groups that do exist carry meaningful names ("Claude Code"), which is the delight posture
  (one glance answers "whose tabs are these?").
- The trained tool surface is untouched: no schema, name, description, or enum changes; the
  `tabs_context_mcp` / `tabs_create_mcp` structured-result shapes keep their exact fields with
  session-scoped values.
- `docs/tasks/hub/PINS.md` SS6's title pin and the ipc.rs fresh-guid-per-reconnect comment are
  superseded by this ADR (the hub batch documents stay as history, unedited).
- The all-open single-session experience is behavior-compatible: one client, one group, same
  tools; the group's title upgrades from a guid suffix to the client's name.

## Provenance

- The e2e session of 2026-07-08 (dev @ 125c768 + F1/F2 fixes) produced the evidence: the
  deterministic read_page failure, the visible `Ghostlight be46ac..` group, the reconnect-storm
  log, and the clean-run counter-evidence that exonerated the reconnect path.
- Two investigation reports (F4 tab-group desync; F3 reconnect storm) traced the code paths and
  are reflected in Context; their factual findings were re-verified against the tree at
  authoring time.
- The owner set the direction on 2026-07-08: "No partially-solved problems ... fix this with
  proper architecture", plus the standing delight-first posture and the unpacked-extension
  auto-detection + green-indicator work (landed as `cd77bf5`).
- The executable plan for this ADR is the task batch in `docs/tasks/tab-identity/` (BOOTSTRAP,
  PINS, T1-T6, LEDGER). Task files cite this ADR as normative; semantics live here only.
