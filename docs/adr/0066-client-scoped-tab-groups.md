# ADR-0066: Client-scoped tab-group presentation

Date: 2026-07-12
Status: Accepted
Amends: ADR-0047 (unified session and tab-surface identity) -- specifically its D2/D4 posture
that the visible Chrome tab group is keyed on the per-process session GUID and its title deduped
per live session. Builds on ADR-0061 (extension-owned browser identity) and ADR-0030 Decision 6
(the SERVICE is the isolation boundary; the extension's group checks are defense-in-depth).

## Context

ADR-0047 set out to stop the browser accumulating orphan tab groups, and did -- within a single
adapter process's lifetime. Its reuse key is the session GUID, which ADR-0047 D2 mints ONCE PER
ADAPTER PROCESS. Every new client process (a new editor window, a fresh `claude` invocation, a
restart) is a new GUID, so:

1. The extension's `Map<guid, chromeGroupId>` has no entry for the new GUID and creates a BRAND-NEW
   Chrome group (`extension/service-worker.js` `createTabInSessionGroup`, `lib/grouping.js`
   `groupSessionTabs`).
2. `session_title` (`crates/core/src/hub/session.rs`) dedupes against every title cached in the
   service lifetime, live OR dead, so sequential sessions become "Claude Code", "Claude Code (2)",
   "(3)" ... each its own group.
3. The map lives in `chrome.storage.session`, wiped on a full browser restart, so after reopening
   Chrome the extension has forgotten its groups entirely -- pure orphans it can neither reuse nor
   prune.

The user reported doing manual "maintenance" to clean up abandoned groups. The gap is between
processes: nothing reuses a prior session's group, and the title even bumps its name.

Two facts shape the fix:

- The client's own identity is already known and already the group's display name: ADR-0047 D4
  titles the group from `clientInfo.name`. What is missing is keying REUSE on that identity instead
  of the ephemeral GUID.
- Ownership isolation is the SERVICE's job, by tabId (`owned_tabs: Map<tabId, guid>`,
  `claim_tab_live`), independent of which Chrome group a tab sits in. The extension's "is this tab
  in a managed group?" check is defense-in-depth (ADR-0030 Decision 6) -- but it is ALSO the only
  thing that keeps the agent inside Ghostlight's tabs and out of the user's own tabs, because the
  service first-touch-adopts ANY unowned tabId. So the group check cannot simply be relaxed; it has
  to distinguish "our tab, currently ungrouped" from "the user's own tab".

## Decision

### D1. Presentation keys on client identity, not the session GUID

The visible Chrome tab group is reused per CLIENT, not per session. The extension maps
`clientKey -> chromeGroupId`, where `clientKey` is a stable string derived from the MCP
`clientInfo.name` (fallback `Ghostlight`). Every session of the same client -- sequential OR
concurrent -- presents into the SAME group. The GUID remains the ownership/isolation key in the
service, unchanged: co-grouped tabs from two live sessions are still each touchable only by their
owning session (`claim_tab_live` refuses a different LIVE owner). Co-grouping is presentation only.

This is the deliberate, owner-chosen behavior: "N instances of the same client = same tab group
with different clients controlling each." One durable group per client; different clients
("Claude Code", "Cursor") still get different groups.

### D2. Stable title (supersedes ADR-0047 D4 dedup)

The group title is `"\u{1F47B} <clientKey>"` with NO ` (2)`/` (3)` dedup suffix: concurrent
same-client sessions share the group, so they share the title. `session_title` stops deduping;
the title is a pure function of the client name. This makes the title round-trippable back to the
clientKey (D4 reclaim below) -- the glyph-plus-name prefix is the only structure needed.

### D3. Wire: an additive `clientKey`

The native `tool_request` and `group_request` envelopes gain an additive `clientKey` string,
exactly the additive posture ADR-0047 D3 used to add `guid` (the native envelope is NOT the sacred
surface; only the MCP tool schemas are). The service stamps it: `Browser` holds a `guid ->
clientKey` map populated at `initialize` (when `clientInfo` is captured), and `Browser::call` /
`Browser::request_group` write it onto the wire. The extension resolves its presentation key as
`clientKey || guid`: a frame with a `clientKey` keys on the client (the D1 behavior); a frame with
only a `guid` (a legacy/hand-rolled native caller) keeps the ADR-0047 per-session behavior; a frame
with neither falls back to the legacy global group. Nothing about ownership, routing, or the sacred
tool schemas changes.

### D4. A self-healing group map (reclaim by title)

The `clientKey -> groupId` map is persisted in `chrome.storage.session` -- the SAME durability
window ADR-0047 used, which survives a service-worker restart (extension reload, crash) but not a
full browser restart. Browser-restart re-attachment does NOT rely on persisted ids surviving
(Chrome may renumber both tab-group ids and TAB ids across a restart); it comes from RECLAIM: on
startup the extension (a) prunes map entries whose Chrome group no longer exists, then (b) scans
the live Chrome tab groups Chrome restored and re-maps any whose title carries the managed prefix
`"\u{1F47B} "` back to its clientKey (`title` minus the prefix). So after a restart the extension
re-attaches to the restored group instead of orphaning it and minting a new one -- and if the user
did NOT restore tabs, there is no group to reclaim and nothing was orphaned. The legacy global
group keeps its existing title-query recovery.

Deliberately NOT `storage.local`: persisting the map (or the `managedTabs` set below) across a full
browser restart would carry stale tab/group ids into a session where Chrome has renumbered them,
and a stale tabId could alias one of the user's OWN new tabs. Reclaim-by-title reads only live
state, so it never resurrects a stale id. (Legacy pre-0066 litter -- groups already titled
"... (2)" -- is left in place; this ADR prevents FUTURE accumulation, it does not retroactively
merge old groups.)

### D5. Owned tabs stay reachable when ungrouped

The extension tracks `managedTabs`, the set of tabIds it has placed in a managed group (on tab
creation and on every `group_request`). The in-surface predicate becomes: a tab is in-surface iff
its current Chrome group is managed (the D1 map or the legacy global group) OR its tabId is in
`managedTabs`. So a tab the user drags out of the group -- detached, or moved to another window,
which ungroups it in Chrome -- stays drivable, while a tab Ghostlight never managed (the user's own
Gmail tab, named by a guessed id) is still refused. `managedTabs` is pruned on `tabs.onRemoved` and
persisted alongside the group map. This closes the detach gap ADR-0047 left, and does so without
moving the tab back (the user's placement is respected).

### D6. Non-decisions

- No auto-closing of groups or tabs (ADR-0047 D5 stands: tabs are user artifacts). Hygiene comes
  from REUSE (fewer groups created) and RECLAIM (re-attaching to existing ones), never from
  deletion.
- No per-workspace/per-window discriminator inside a client. `clientInfo` carries only name +
  version; keying on the name is exactly the owner-approved "one group per client". If a client
  ever reports a richer identity, `client_key` is the one place to refine it.
- The service's `owned_tabs` map and `claim_tab_live` isolation are untouched. Cross-session
  isolation and the "which browser" routing (ADR-0058/0061 composite tabIds) are unchanged.

## Consequences

- The browser stops accumulating "Claude Code (2)/(3)" orphans: sequential sessions of a client
  reuse one group, and a browser restart re-attaches to it rather than abandoning it.
- Concurrent same-client sessions co-group. This is safe (per-guid ownership still isolates who can
  touch each tab) and is the intended behavior; it does change the ADR-0047 posture that two live
  same-name sessions get separate groups.
- A detached / moved-to-another-window owned tab keeps working, closing a rough edge that existed
  since ADR-0047.
- The trained tool surface is untouched: no schema, name, description, enum, or structured-result
  shape changes. `clientKey` is additive native-envelope plumbing, invisible to the model.
- `lib/grouping.js`'s pure decision (`groupSessionTabs`, `managedGroupIds`, `pruneDeadGroups`) is
  key-agnostic already -- it groups whatever key string the caller passes -- so its H7/ADR-0047
  oracle tests hold verbatim; the behavior change lives entirely in which key the caller supplies
  and in the new reclaim/managedTabs hygiene.

## Provenance

- Owner request, 2026-07-12: "the service frequently leaves 'abandoned' tab groups ... better tab
  group hygiene ... I'm fine with different tab groups per client, but we should at least try to
  reuse the groups"; then, on the concurrent-session question, "N instances of the same client =
  same tab group with different clients controlling each"; and the reachability requirement,
  "implement in such a way as that ungrouped-but-in-use tabs can still be reachable and used".
- The investigation that grounded this ADR traced the reuse key to `session_title`'s dedup, the
  extension's guid-keyed `sessionGroups`, and the `storage.session` durability window, and verified
  that the extension group gate -- not just the service -- is what scopes tools away from the user's
  own tabs (the service first-touch-adopts any unowned tabId).
