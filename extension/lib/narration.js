// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- policy-free, memory-only narration state (ADR-0072).
//
// The service worker owns tab routing and calls this pure store. The store knows nothing about
// domains, grants, capabilities, or page content. It only gives replacement and expiry semantics
// to one short-lived narration record per tab.
(function () {

const DEFAULT_DURATION_MS = 5000;
const MIN_DURATION_MS = 1000;
const MAX_DURATION_MS = 30000;
const POSITIONS = new Set(["auto", "top", "bottom"]);

function normalizePosition(position) {
  return POSITIONS.has(position) ? position : "auto";
}

function normalizeDuration(durationMs) {
  const value = Number.isInteger(durationMs) ? durationMs : DEFAULT_DURATION_MS;
  return Math.max(MIN_DURATION_MS, Math.min(MAX_DURATION_MS, value));
}

function createNarrationStore(now) {
  const clock = typeof now === "function" ? now : Date.now;
  const records = new Map();
  let nextGeneration = 1;

  function current(tabId) {
    const record = records.get(tabId);
    if (!record) return null;
    const remainingMs = record.deadline - clock();
    if (remainingMs <= 0) {
      records.delete(tabId);
      return null;
    }
    return { ...record, remainingMs };
  }

  function show(tabId, text, position, durationMs) {
    const replaced = current(tabId) !== null;
    const effectiveDuration = normalizeDuration(durationMs);
    const record = {
      generation: nextGeneration++,
      text: String(text),
      position: normalizePosition(position),
      durationMs: effectiveDuration,
      deadline: clock() + effectiveDuration,
    };
    records.set(tabId, record);
    return { record: { ...record }, replaced };
  }

  function remove(tabId, generation) {
    const record = records.get(tabId);
    if (!record) return false;
    if (generation !== undefined && record.generation !== generation) return false;
    records.delete(tabId);
    return true;
  }

  function clear() {
    const tabIds = Array.from(records.keys());
    records.clear();
    return tabIds;
  }

  return { current, show, remove, clear };
}

const GhostlightNarration = {
  DEFAULT_DURATION_MS,
  MIN_DURATION_MS,
  MAX_DURATION_MS,
  normalizePosition,
  normalizeDuration,
  createNarrationStore,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightNarration;
} else {
  self.GhostlightNarration = GhostlightNarration;
}
})();
