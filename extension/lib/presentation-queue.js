// Ghostlight -- bounded browser-local delivery state for unseen denial signals.
(function installGhostlightPresentationQueue(root, factory) {
  const api = factory();
  if (typeof module === "object" && module.exports) module.exports = api;
  root.GhostlightPresentationQueue = api;
})(globalThis, function createGhostlightPresentationQueueApi() {
  "use strict";

  const DEFAULT_LIMIT = 16;
  const DEFAULT_TTL_MS = 10 * 60 * 1000;

  function create({ limit = DEFAULT_LIMIT, ttlMs = DEFAULT_TTL_MS, now = Date.now } = {}) {
    const entries = new Map();

    function valid(value) {
      return value
        && Number.isInteger(value.tabId)
        && value.tabId > 0
        && typeof value.workspace === "string"
        && value.workspace.length > 0
        && value.workspace.length <= 200
        && Number.isFinite(value.expiresAt)
        && value.expiresAt > now()
        && value.signal
        && value.signal.signal === "denial"
        && value.signal.tab_id === value.tabId
        && typeof value.signal.invocation === "string"
        && value.signal.invocation.length <= 100
        && typeof value.signal.phase === "string"
        && value.signal.phase.length <= 100
        && (value.signal.detail === undefined
          || (typeof value.signal.detail === "string" && value.signal.detail.length <= 240));
    }

    function prune() {
      for (const [tabId, entry] of entries) {
        if (entry.expiresAt <= now()) entries.delete(tabId);
      }
      while (entries.size > limit) entries.delete(entries.keys().next().value);
    }

    function restore(value) {
      entries.clear();
      for (const entry of Array.isArray(value) ? value : []) {
        if (valid(entry)) entries.set(entry.tabId, entry);
      }
      prune();
    }

    function defer(workspace, tabId, signal) {
      const entry = { workspace, tabId, signal: { ...signal, tab_id: tabId }, expiresAt: now() + ttlMs };
      if (!valid(entry)) return false;
      entries.delete(tabId);
      entries.set(tabId, entry);
      prune();
      return true;
    }

    function get(tabId) {
      prune();
      return entries.get(tabId) || null;
    }

    function forget(tabId) {
      return entries.delete(tabId);
    }

    function size() {
      prune();
      return entries.size;
    }

    function snapshot() {
      prune();
      return Array.from(entries.values());
    }

    return Object.freeze({ restore, defer, get, forget, size, snapshot });
  }

  return Object.freeze({ DEFAULT_LIMIT, DEFAULT_TTL_MS, create });
});
