// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- policy-free, memory-only attention presentation state (ADR-0079).
(function () {

function createAttentionStore() {
  const records = new Map();

  function show(record) {
    if (!record || typeof record.guid !== "string" || !record.guid) return null;
    const normalized = {
      guid: record.guid,
      tabId: Number.isSafeInteger(record.tabId) ? record.tabId : null,
      label: String(record.label || "MCP client").slice(0, 80),
      category: record.category === "sacred" ? "sacred" : "policy",
      origin: typeof record.origin === "string" ? record.origin : null,
      threshold: record.threshold === "session" ? "session" : "matching",
      count: Number.isInteger(record.count) ? record.count : 0,
      title: String(record.title || "Agent browsing paused"),
      description: String(record.description || "Repeated blocked actions need your attention."),
      controls: Array.isArray(record.controls) ? record.controls.slice() : [],
    };
    records.set(normalized.guid, normalized);
    return { ...normalized };
  }

  function remove(guid) {
    const prior = records.get(guid) || null;
    records.delete(guid);
    return prior ? { ...prior } : null;
  }

  function list() {
    return Array.from(records.values(), (record) => ({ ...record }));
  }

  function forTab(tabId) {
    return list().filter((record) => record.tabId === tabId);
  }

  function replace(recordsFromService) {
    records.clear();
    for (const record of recordsFromService || []) show(record);
    return list();
  }

  function clear() {
    const prior = list();
    records.clear();
    return prior;
  }

  return { show, remove, list, forTab, replace, clear };
}

const GhostlightAttention = { createAttentionStore };
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightAttention;
} else {
  self.GhostlightAttention = GhostlightAttention;
}
})();
