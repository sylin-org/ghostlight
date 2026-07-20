// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- managed tab-group presentation: the PURE grouping DECISION (ADR-0030
// Decision 6/7; docs/tasks/hub/H7-tab-group-per-session.md; docs/tasks/hub/PINS.md SS6).
//
// Given an injected `chrome`-like object, an adapter-chosen workspace-key -> Chrome tab-group-id
// map, and a `group_request`'s named tabIds/title, groups EXACTLY those tabIds into that workspace's
// Chrome
// tab group -- creating one on first use, reusing it (idempotent) on every later request for the
// SAME workspace key, and titling it every call. Makes NO policy decision: it never reads a tab's
// url/host/domain/grant to decide membership -- it groups the tabIds the caller named, full stop
// (ADR-0030 Decision 6: "The extension's per-group checks remain defense-in-depth only"; Migration
// H7: "groups on request only"). Since ADR-0047 D1 this module is no longer merely additive: the
// single-group access-control gate in `service-worker.js` (`groupTabs`/`inGroup`/`effectiveTabId`)
// now CONSULTS this module's `managedGroupIds`/`isManagedGroupId` predicate so a tab counts as
// in-surface when it sits in ANY Ghostlight-managed group -- the legacy global group OR any
// per-session group. The earlier "does not touch or call" claim was false at the Chrome API level:
// a tab can be in exactly one group, so a per-session group evicts a tab from the global one
// (ADR-0047 Context records the resulting F4 desync).
//
// IIFE-wrapped so its bindings stay function-scoped under importScripts' shared global (see
// lib/geometry.js's header for why); this file is ASCII-only source (the ghost glyph the caller
// embeds in `title` is produced elsewhere as a `\u{1F47B}` escape, never written here).
(function () {

// Group EXACTLY `tabIds` (a plain array of Chrome tab ids) into the Chrome tab group belonging to
// `key` in `sessionGroups` (a `Map<string, number>`, workspace key -> chrome tab-group id -- mutated in
// place so the caller's map stays the single source of truth for reuse/persistence). `title` is
// applied every call (idempotent: `chrome.tabGroups.update` on an unchanged title is a no-op from
// the caller's point of view). Returns the group id used, or `null` if every named tab was gone.
//
// A named tab that no longer exists is a best-effort, silent no-op -- a liveness fact, not a
// policy decision (mirrors the existing `chrome.tabs.get` failure posture elsewhere in this
// worker): this function probes each tabId with `chrome.tabs.get` ONLY to check it still exists,
// never reading any field (`.url` included) off the result.
async function groupSessionTabs(chrome, sessionGroups, key, tabIds, title) {
  const liveTabIds = [];
  for (const tabId of tabIds) {
    try {
      await chrome.tabs.get(tabId);
      liveTabIds.push(tabId);
    } catch {
      // the tab no longer exists: a liveness fact, not a policy decision -- drop it silently.
    }
  }
  if (liveTabIds.length === 0) return sessionGroups.has(key) ? sessionGroups.get(key) : null;

  let groupId = null;
  if (sessionGroups.has(key)) {
    const existingGroupId = sessionGroups.get(key);
    try {
      await chrome.tabGroups.get(existingGroupId);
      groupId = existingGroupId; // still live: reuse it (idempotent)
    } catch {
      groupId = null; // the group vanished; a fresh one is created below
    }
  }

  if (groupId === null) {
    groupId = await chrome.tabs.group({ tabIds: liveTabIds });
  } else {
    await chrome.tabs.group({ tabIds: liveTabIds, groupId });
  }
  await chrome.tabGroups.update(groupId, { title, color: "blue" });
  sessionGroups.set(key, groupId);
  return groupId;
}

// The managed surface (ADR-0047 D1): every Chrome tab-group id this extension manages -- the
// legacy global group (when set) plus every workspace group it created on service request.
function managedGroupIds(globalGroupId, sessionGroups) {
  const ids = new Set();
  if (globalGroupId !== null && globalGroupId !== undefined) ids.add(globalGroupId);
  for (const gid of sessionGroups.values()) ids.add(gid);
  return ids;
}

// True iff `groupId` (a chrome tab's .groupId; -1 means ungrouped) is a managed group.
function isManagedGroupId(groupId, globalGroupId, sessionGroups) {
  if (groupId === -1 || groupId === null || groupId === undefined) return false;
  return managedGroupIds(globalGroupId, sessionGroups).has(groupId);
}

// ADR-0047 D5 hygiene: drop sessionGroups entries whose Chrome group no longer exists. Returns
// true when anything was removed (the caller persists). Probes group liveness only; reads no
// tab or group content.
async function pruneDeadGroups(chrome, sessionGroups) {
  let changed = false;
  for (const [guid, gid] of Array.from(sessionGroups.entries())) {
    try {
      await chrome.tabGroups.get(gid);
    } catch {
      sessionGroups.delete(guid);
      changed = true;
    }
  }
  return changed;
}

// ADR-0066 D4: re-attach forgotten client groups after a full browser restart. The persisted
// key->groupId map lives in chrome.storage.session, which a browser restart clears, so on startup
// this scans the tab groups Chrome RESTORED and re-maps any whose title carries the managed
// `prefix` (the ghost glyph + a space, `\u{1F47B} `) back to its clientKey -- `title` minus the
// prefix, the exact contract `session_title` writes on the service side. It reads ONLY live state,
// never a persisted (possibly Chrome-renumbered) id, so it can never resurrect a stale id that
// might now alias one of the user's own tabs. A key already mapped to a live group, or a group id
// already claimed, is left untouched (first title wins), so legacy litter titled `... (2)` maps to
// its own distinct key and is never re-created for a fresh session. Returns true when it added
// anything (the caller persists). Best-effort: no tabGroups access, or none, yields no change.
// `keyFor` optionally combines that client key with the live group window. This is how the current
// adapter reclaims identical client titles in several user-arranged windows without relying on a
// persisted native window id. Legacy callers omit it and retain the old client-only key.
async function reclaimGroupsByTitle(chrome, sessionGroups, prefix, keyFor) {
  let groups;
  try {
    groups = await chrome.tabGroups.query({});
  } catch {
    return false;
  }
  const mapped = new Set(sessionGroups.values());
  let changed = false;
  for (const g of groups) {
    const title = (g && g.title) || "";
    if (!title.startsWith(prefix)) continue;
    const clientKey = title.slice(prefix.length);
    const key = keyFor ? keyFor(clientKey, g.windowId) : clientKey;
    if (!key || sessionGroups.has(key) || mapped.has(g.id)) continue;
    sessionGroups.set(key, g.id);
    mapped.add(g.id);
    changed = true;
  }
  return changed;
}

const GhostlightGrouping = {
  groupSessionTabs,
  managedGroupIds,
  isManagedGroupId,
  pruneDeadGroups,
  reclaimGroupsByTitle,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightGrouping;
} else {
  self.GhostlightGrouping = GhostlightGrouping;
}
})();
