// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- browser-window placement mechanism for managed tab workspaces.
//
// This adapter helper knows Chrome window ids and API calls. It does not know policy, grants, page
// content, or model-facing browser identity. The service chooses the selection mode and pins the
// returned mechanism fact for the MCP session.
(function () {
"use strict";

const KEY_VERSION = "v1";
const LAST_FOCUSED_NORMAL = "last_focused_normal";
const FOCUS_MRU_KEY = "ghostlight_workspace_focus_mru_v1";
const MAX_FOCUS_MRU = 32;
let focusMutationTail = Promise.resolve();

function eligibleNormalWindow(win) {
  return !!win && Number.isSafeInteger(win.id) && win.type === "normal" && win.incognito !== true;
}

function workspaceGroupKey(clientKey, windowId) {
  if (typeof clientKey !== "string" || !clientKey || !Number.isSafeInteger(windowId)) return null;
  return JSON.stringify([KEY_VERSION, windowId, clientKey]);
}

function parseWorkspaceGroupKey(key) {
  if (typeof key !== "string") return null;
  try {
    const parsed = JSON.parse(key);
    if (!Array.isArray(parsed) || parsed.length !== 3 || parsed[0] !== KEY_VERSION) return null;
    if (!Number.isSafeInteger(parsed[1]) || typeof parsed[2] !== "string" || !parsed[2]) return null;
    return { windowId: parsed[1], clientKey: parsed[2] };
  } catch {
    return null;
  }
}

async function getEligibleWindow(chrome, windowId) {
  let win;
  try { win = await chrome.windows.get(windowId); } catch { win = null; }
  if (!eligibleNormalWindow(win)) {
    throw new Error("The selected Ghostlight workspace window is no longer eligible");
  }
  return win;
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

async function resolveAutomaticWindow(chrome) {
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

async function resolveWorkspaceWindow(chrome, request) {
  if (request && Number.isSafeInteger(request.windowId)) {
    return { window: await getEligibleWindow(chrome, request.windowId), created: false, pinned: true };
  }
  const selector = request && request.select;
  if (selector !== undefined && selector !== LAST_FOCUSED_NORMAL) {
    throw new Error(`Unknown Ghostlight workspace selector: ${selector}`);
  }
  const resolved = await resolveAutomaticWindow(chrome);
  return { ...resolved, pinned: false };
}

// Resolve a client's group in one window without moving it. If the user moved the group, re-key
// that live group to its actual window and leave the requested workspace empty.
async function resolveWorkspaceGroup(chrome, groups, clientKey, windowId) {
  const key = workspaceGroupKey(clientKey, windowId);
  if (!key || !groups.has(key)) return { key, groupId: null, changed: false };
  const groupId = groups.get(key);
  try {
    const group = await chrome.tabGroups.get(groupId);
    if (group.windowId === windowId) return { key, groupId, changed: false };
    groups.delete(key);
    const movedKey = workspaceGroupKey(clientKey, group.windowId);
    if (movedKey && !groups.has(movedKey)) groups.set(movedKey, groupId);
  } catch {
    groups.delete(key);
  }
  return { key, groupId: null, changed: true };
}

// Upgrade old `clientKey -> groupId` entries and repair window keys after a group was moved. The
// group itself is authoritative for its current Chrome window. A collision is left first-wins;
// both group ids remain managed through `managedTabs`, while the canonical workspace mapping stays
// deterministic.
async function reconcileWorkspaceGroups(chrome, groups) {
  let changed = false;
  for (const [storedKey, groupId] of Array.from(groups.entries())) {
    let group;
    try { group = await chrome.tabGroups.get(groupId); } catch { continue; }
    if (!Number.isSafeInteger(group.windowId)) continue;
    const parsed = parseWorkspaceGroupKey(storedKey);
    const clientKey = parsed ? parsed.clientKey : storedKey;
    if (typeof clientKey !== "string" || !clientKey) continue;
    const currentKey = workspaceGroupKey(clientKey, group.windowId);
    if (!currentKey || currentKey === storedKey) continue;
    if (!groups.has(currentKey)) groups.set(currentKey, groupId);
    groups.delete(storedKey);
    changed = true;
  }
  return changed;
}

async function tabsInWindow(chrome, tabIds, windowId) {
  const live = [];
  for (const tabId of tabIds) {
    try {
      const tab = await chrome.tabs.get(tabId);
      if (tab.windowId === windowId) live.push(tabId);
    } catch { /* a vanished tab contributes nothing */ }
  }
  return live;
}

const GhostlightWorkspace = {
  LAST_FOCUSED_NORMAL,
  FOCUS_MRU_KEY,
  eligibleNormalWindow,
  workspaceGroupKey,
  parseWorkspaceGroupKey,
  resolveWorkspaceWindow,
  rememberFocusedWindow,
  forgetWorkspaceWindow,
  resolveWorkspaceGroup,
  reconcileWorkspaceGroups,
  tabsInWindow,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightWorkspace;
} else {
  self.GhostlightWorkspace = GhostlightWorkspace;
}
})();
