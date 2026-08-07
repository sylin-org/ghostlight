// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- browser-shore workspace topology.
//
// The service supplies an opaque WorkspaceId and a presentation title. This helper owns only
// Chrome mechanism: live tabs, groups, windows, and initial placement. It never knows policy,
// grants, page content, composite tab ids, or model-facing browser identity.
(function () {
"use strict";

const FOCUS_MRU_KEY = "ghostlight_workspace_focus_mru_v1";
const MAX_FOCUS_MRU = 32;
let focusMutationTail = Promise.resolve();

function eligibleNormalWindow(win) {
  return !!win && Number.isSafeInteger(win.id) && win.type === "normal" && win.incognito !== true;
}

function normalizedFocusMru(value) {
  if (!Array.isArray(value)) return [];
  return Array.from(new Set(value.filter(Number.isSafeInteger))).slice(0, MAX_FOCUS_MRU);
}

async function readStoredFocusMru(chrome) {
  try {
    const stored = await chrome.storage.session.get(FOCUS_MRU_KEY);
    return normalizedFocusMru(stored && stored[FOCUS_MRU_KEY]);
  } catch {
    return [];
  }
}

async function readFocusMru(chrome) {
  await focusMutationTail;
  return readStoredFocusMru(chrome);
}

// Record a browser-native focus fact locally. This never reaches policy or audit; it only lets a
// later pull recover when getLastFocused is temporarily unavailable.
async function rememberFocusedWindow(chrome, windowId) {
  if (!Number.isSafeInteger(windowId) || windowId === chrome.windows.WINDOW_ID_NONE) return false;
  const mutation = focusMutationTail.then(async () => {
    let win;
    try { win = await chrome.windows.get(windowId); } catch { return false; }
    if (!eligibleNormalWindow(win)) return false;
    const mru = await readStoredFocusMru(chrome);
    const next = [windowId, ...mru.filter((id) => id !== windowId)].slice(0, MAX_FOCUS_MRU);
    try {
      await chrome.storage.session.set({ [FOCUS_MRU_KEY]: next });
      return true;
    } catch {
      return false;
    }
  });
  focusMutationTail = mutation.then(() => {}, () => {});
  return mutation;
}

async function forgetWorkspaceWindow(chrome, windowId) {
  const mutation = focusMutationTail.then(async () => {
    const mru = await readStoredFocusMru(chrome);
    const next = mru.filter((id) => id !== windowId);
    if (next.length === mru.length) return false;
    try {
      await chrome.storage.session.set({ [FOCUS_MRU_KEY]: next });
      return true;
    } catch {
      return false;
    }
  });
  focusMutationTail = mutation.then(() => {}, () => {});
  return mutation;
}

// Select an initial Chrome window. Once a workspace has live tabs, their live placement supersedes
// this bootstrap choice.
async function resolveWorkspaceWindow(chrome) {
  try {
    const last = await chrome.windows.getLastFocused({ windowTypes: ["normal"] });
    if (eligibleNormalWindow(last)) return { window: last, created: false };
  } catch { /* fall through to the bounded inventory check */ }

  let windows;
  try {
    windows = (await chrome.windows.getAll({ windowTypes: ["normal"] }))
      .filter(eligibleNormalWindow);
  } catch {
    throw new Error("Chrome could not inspect existing normal windows; Ghostlight will not create another one");
  }

  const focused = windows.find((win) => win.focused === true);
  if (focused) return { window: focused, created: false };

  const byId = new Map(windows.map((win) => [win.id, win]));
  for (const windowId of await readFocusMru(chrome)) {
    if (byId.has(windowId)) return { window: byId.get(windowId), created: false };
  }

  if (windows.length === 1) return { window: windows[0], created: false };
  if (windows.length > 1) {
    throw new Error("Several normal browser windows exist, but Chrome reported no most-recently-focused window");
  }

  const created = await chrome.windows.create({ focused: true, type: "normal" });
  if (!eligibleNormalWindow(created)) {
    throw new Error("Chrome could not create an eligible normal window for Ghostlight");
  }
  return { window: created, created: true };
}

function workspaceRecord(index, key) {
  if (typeof key !== "string" || !key) return null;
  let record = index.get(key);
  if (!record) {
    record = { tabIds: new Set(), groupId: null };
    index.set(key, record);
  }
  return record;
}

function replaceWorkspaceTabs(index, key, tabIds, groupId) {
  const record = workspaceRecord(index, key);
  if (!record) return false;
  record.tabIds = new Set((Array.isArray(tabIds) ? tabIds : []).filter(Number.isSafeInteger));
  if (Number.isSafeInteger(groupId)) record.groupId = groupId;
  return true;
}

function addWorkspaceTab(index, key, tabId, groupId) {
  const record = workspaceRecord(index, key);
  if (!record || !Number.isSafeInteger(tabId)) return false;
  record.tabIds.add(tabId);
  if (Number.isSafeInteger(groupId)) record.groupId = groupId;
  return true;
}

function removeWorkspaceTab(index, tabId) {
  for (const record of index.values()) {
    if (record.tabIds.delete(tabId) && record.tabIds.size === 0) record.groupId = null;
  }
}

// Return the one workspace that owns this native tab id. Ambiguous topology is never resolved by
// guessing: the service owns real authority, while this extension helper only supplies a safe
// browser-shore correlation fact.
function workspaceIdForTab(index, tabId) {
  if (!Number.isSafeInteger(tabId)) return null;
  let found = null;
  for (const [workspaceId, record] of index.entries()) {
    if (!record.tabIds.has(tabId)) continue;
    if (found !== null) return null;
    found = workspaceId;
  }
  return found;
}

function workspaceGroupIds(index) {
  return new Set(Array.from(index.values())
    .map((record) => record.groupId)
    .filter(Number.isSafeInteger));
}

function isWorkspaceGroupId(index, groupId) {
  return Number.isSafeInteger(groupId) && workspaceGroupIds(index).has(groupId);
}

async function liveWorkspaceTabs(chrome, index, key) {
  const record = index.get(key);
  if (!record) return [];
  const tabs = [];
  for (const tabId of Array.from(record.tabIds)) {
    try {
      tabs.push(await chrome.tabs.get(tabId));
    } catch {
      record.tabIds.delete(tabId);
    }
  }
  return tabs;
}

function byMostRecent(left, right) {
  const recency = (right.lastAccessed || 0) - (left.lastAccessed || 0);
  return recency || left.id - right.id;
}

// Derive the workspace's presentation group from its own live tabs. A stored group id alone is
// never enough: the user may have detached every workspace tab while that shared group stayed live.
async function liveWorkspaceGroup(chrome, index, key, liveTabs) {
  const record = workspaceRecord(index, key);
  if (!record) return null;
  const tabs = Array.isArray(liveTabs) ? liveTabs : await liveWorkspaceTabs(chrome, index, key);
  for (const tab of tabs.slice().sort(byMostRecent)) {
    if (!Number.isSafeInteger(tab.groupId) || tab.groupId < 0) continue;
    try {
      const group = await chrome.tabGroups.get(tab.groupId);
      record.groupId = group.id;
      return group;
    } catch { /* inspect the next owned tab */ }
  }
  record.groupId = null;
  return null;
}

async function preferredNamedGroup(chrome, index, windowId, title) {
  if (!Number.isSafeInteger(windowId) || typeof title !== "string" || !title) return null;
  let candidates;
  try {
    candidates = await chrome.tabGroups.query({ title, windowId });
  } catch {
    return null;
  }
  candidates = (Array.isArray(candidates) ? candidates : [])
    .filter((group) => group && Number.isSafeInteger(group.id) && group.windowId === windowId);
  const managed = workspaceGroupIds(index);
  const managedCandidates = candidates.filter((group) => managed.has(group.id));
  const pool = managedCandidates.length > 0 ? managedCandidates : candidates;
  pool.sort((left, right) => left.id - right.id);
  return pool.length > 0 ? pool[0] : null;
}

// Find where a new workspace tab belongs without moving any existing tab. Live workspace tabs win
// over focus; focus is only the bootstrap when the workspace has no live browser artifacts.
async function resolveWorkspacePlacement(chrome, index, key, title) {
  const tabs = await liveWorkspaceTabs(chrome, index, key);
  const group = await liveWorkspaceGroup(chrome, index, key, tabs);
  if (group) {
    return { tabs, group, windowId: group.windowId, createdWindow: false, initialTab: null };
  }

  const ordered = tabs.slice().sort(byMostRecent);
  if (ordered.length > 0) {
    const windowId = ordered[0].windowId;
    const named = await preferredNamedGroup(chrome, index, windowId, title);
    return { tabs, group: named, windowId, createdWindow: false, initialTab: null };
  }

  const resolved = await resolveWorkspaceWindow(chrome);
  const windowId = resolved.window.id;
  const named = await preferredNamedGroup(chrome, index, windowId, title);
  const initialTab = resolved.created && resolved.window.tabs && resolved.window.tabs[0]
    ? resolved.window.tabs[0]
    : null;
  return {
    tabs,
    group: named,
    windowId,
    createdWindow: resolved.created,
    initialTab,
  };
}

async function pruneWorkspaceGroups(chrome, index) {
  let changed = false;
  for (const record of index.values()) {
    if (!Number.isSafeInteger(record.groupId)) continue;
    try {
      await chrome.tabGroups.get(record.groupId);
    } catch {
      record.groupId = null;
      changed = true;
    }
  }
  return changed;
}

function serializeWorkspaceTopology(index) {
  return Array.from(index.entries(), ([key, record]) => [key, {
    tabIds: Array.from(record.tabIds),
    groupId: Number.isSafeInteger(record.groupId) ? record.groupId : null,
  }]);
}

function restoreWorkspaceTopology(index, serialized) {
  if (!Array.isArray(serialized)) return false;
  let restored = false;
  for (const entry of serialized) {
    if (!Array.isArray(entry) || entry.length !== 2 || typeof entry[0] !== "string") continue;
    const value = entry[1];
    if (!value || typeof value !== "object") continue;
    replaceWorkspaceTabs(index, entry[0], value.tabIds, value.groupId);
    restored = true;
  }
  return restored;
}

function legacyWorkspaceId(key) {
  if (typeof key !== "string" || !key) return null;
  try {
    const parsed = JSON.parse(key);
    if (Array.isArray(parsed) && parsed.length === 3 && parsed[0] === "v1" &&
        typeof parsed[2] === "string" && parsed[2]) {
      return parsed[2];
    }
  } catch { /* a direct legacy key is already usable */ }
  return key;
}

function migrateLegacyWorkspaceTopology(index, stored) {
  let migrated = false;
  if (Array.isArray(stored && stored.clientGroupsState)) {
    for (const entry of stored.clientGroupsState) {
      if (!Array.isArray(entry) || entry.length !== 2 || !Number.isSafeInteger(entry[1])) continue;
      const key = legacyWorkspaceId(entry[0]);
      if (!key) continue;
      replaceWorkspaceTabs(index, key, [], entry[1]);
      migrated = true;
    }
  }
  if (Array.isArray(stored && stored.workspaceTabsState)) {
    for (const entry of stored.workspaceTabsState) {
      if (!Array.isArray(entry) || entry.length !== 2) continue;
      const key = legacyWorkspaceId(entry[0]);
      if (!key || !Array.isArray(entry[1])) continue;
      for (const tabId of entry[1]) {
        if (Number.isSafeInteger(tabId)) addWorkspaceTab(index, key, tabId);
      }
      migrated = true;
    }
  }
  return migrated;
}

const GhostlightWorkspace = {
  FOCUS_MRU_KEY,
  eligibleNormalWindow,
  resolveWorkspaceWindow,
  rememberFocusedWindow,
  forgetWorkspaceWindow,
  workspaceRecord,
  replaceWorkspaceTabs,
  addWorkspaceTab,
  removeWorkspaceTab,
  workspaceIdForTab,
  workspaceGroupIds,
  isWorkspaceGroupId,
  liveWorkspaceTabs,
  liveWorkspaceGroup,
  preferredNamedGroup,
  resolveWorkspacePlacement,
  pruneWorkspaceGroups,
  serializeWorkspaceTopology,
  restoreWorkspaceTopology,
  migrateLegacyWorkspaceTopology,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightWorkspace;
} else {
  self.GhostlightWorkspace = GhostlightWorkspace;
}
})();
