// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- bounded browser-tab transition observations.
//
// Chrome owns tab lifecycle. This policy-free journal records only browser-native identifiers and
// correlates transitions with one already-owned opener tab while a tool request is in flight. It
// never infers that the request caused the transition, never decides workspace authority, and
// never moves a tab or group.
(function () {
"use strict";

const TAB_DELTA_V1 = "tabDeltaV1";
const DEFAULT_MAX_EVENTS = 64;
const DEFAULT_MAX_ITEMS = 16;

function positiveInteger(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function createTabEffectJournal(options = {}) {
  const maxEvents = positiveInteger(options.maxEvents) && options.maxEvents > 0
    ? options.maxEvents
    : DEFAULT_MAX_EVENTS;
  const maxItems = positiveInteger(options.maxItems) && options.maxItems > 0
    ? options.maxItems
    : DEFAULT_MAX_ITEMS;
  const events = [];
  const openerByTab = new Map();
  let sequence = 0;

  function append(event) {
    events.push(Object.assign({ sequence: ++sequence }, event));
    if (events.length > maxEvents) events.splice(0, events.length - maxEvents);
  }

  function opened(workspaceId, sourceTabId, tab) {
    if (typeof workspaceId !== "string" || !workspaceId ||
        !positiveInteger(sourceTabId) || !tab || !positiveInteger(tab.id)) {
      return false;
    }
    openerByTab.set(tab.id, { workspaceId, sourceTabId });
    append({
      kind: "opened",
      workspaceId,
      sourceTabId,
      tabId: tab.id,
      active: tab.active === true,
    });
    return true;
  }

  function closed(workspaceId, tabId) {
    if (typeof workspaceId !== "string" || !workspaceId || !positiveInteger(tabId)) return false;
    const opener = openerByTab.get(tabId);
    openerByTab.delete(tabId);
    append({
      kind: "closed",
      workspaceId,
      sourceTabId: opener && opener.workspaceId === workspaceId ? opener.sourceTabId : tabId,
      tabId,
    });
    return true;
  }

  function cursor() {
    return sequence;
  }

  function deltaSince(after, workspaceId, sourceTabId) {
    if (!Number.isSafeInteger(after) || typeof workspaceId !== "string" || !workspaceId ||
        !positiveInteger(sourceTabId)) {
      return null;
    }
    const matching = events.filter((event) =>
      event.sequence > after && event.workspaceId === workspaceId &&
      event.sourceTabId === sourceTabId
    );
    if (matching.length === 0) return null;

    const selected = matching.slice(0, maxItems);
    const openedTabs = selected
      .filter((event) => event.kind === "opened")
      .map((event) => ({ tabId: event.tabId, active: event.active }));
    const closedTabs = selected
      .filter((event) => event.kind === "closed")
      .map((event) => event.tabId);
    const closedSet = new Set(closedTabs);
    const active = openedTabs.filter((tab) => tab.active && !closedSet.has(tab.tabId)).at(-1);
    const delta = {
      opened: openedTabs,
      closed: closedTabs,
      more: matching.length > selected.length,
    };
    if (active) delta.activeTabId = active.tabId;
    return delta;
  }

  return { cursor, opened, closed, deltaSince };
}

function requestsTabDelta(request) {
  return !!request && Array.isArray(request.resultFeatures) &&
    request.resultFeatures.includes(TAB_DELTA_V1);
}

function attachTabDelta(result, delta) {
  if (!result || typeof result !== "object" || !delta) return result;
  result.structuredContent = Object.assign({}, result.structuredContent || {}, {
    tabDelta: delta,
  });
  return result;
}

const GhostlightTabEffects = {
  TAB_DELTA_V1,
  createTabEffectJournal,
  requestsTabDelta,
  attachTabDelta,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightTabEffects;
} else {
  self.GhostlightTabEffects = GhostlightTabEffects;
}
})();
