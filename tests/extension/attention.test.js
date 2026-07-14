// SPDX-License-Identifier: Apache-2.0 OR MIT
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { createAttentionStore } = require("../../extension/lib/attention.js");

const root = path.join(__dirname, "../..");
const indicator = fs.readFileSync(path.join(root, "extension/agent-visual-indicator.js"), "utf8");
const worker = fs.readFileSync(path.join(root, "extension/service-worker.js"), "utf8");
const popup = fs.readFileSync(path.join(root, "extension/popup.js"), "utf8");

test("attention presentation records replace by session and remain independent", () => {
  const store = createAttentionStore();
  store.show({ guid: "a", tabId: 1, label: "Cline", category: "policy", count: 3 });
  store.show({ guid: "b", tabId: 2, label: "Codex", category: "sacred", count: 5 });
  store.show({ guid: "a", tabId: 3, label: "Cline", category: "policy", count: 4 });
  assert.equal(store.list().length, 2);
  assert.equal(store.forTab(3)[0].count, 4);
  assert.equal(store.forTab(1).length, 0);
});

test("remove and service replacement clear stale presentation", () => {
  const store = createAttentionStore();
  store.show({ guid: "old", tabId: 1 });
  assert.equal(store.remove("old").guid, "old");
  assert.equal(store.remove("old"), null);
  store.replace([{ guid: "fresh", tabId: 9, label: "Kilo" }]);
  assert.deepEqual(store.list().map((record) => record.guid), ["fresh"]);
});

test("isolated denial stickers replace, expire, and stay non-modal", () => {
  assert.match(indicator, /dismissNotification\(\); \/\/ replace, never stack/);
  assert.match(indicator, /setTimeout\(dismissNotification, Math\.max\(500, Number\(durationMs\) \|\| 3000\)\)/);
  assert.match(indicator, /ghostlight-notification-layer/);
  assert.match(indicator, /pointer-events:none/);
  assert.match(worker, /durationMs: msg\.durationMs/);
});

test("attention recovery is replayed and available from the popup", () => {
  assert.match(worker, /attentionRequest\(\{ type: "get_attention" \}\)/);
  assert.match(worker, /renderAttention\(record\.tabId, record\)/);
  assert.match(worker, /AGENT_ATTENTION_REQUIRED/);
  assert.match(popup, /GET_ATTENTION_STATE/);
  assert.match(popup, /ATTENTION_ACTION/);
});

test("all four dispositions relay exact service vocabulary", () => {
  for (const disposition of ["keep_paused", "resume", "resume_quiet", "end_session"]) {
    assert.match(indicator, new RegExp('"' + disposition + '"'));
    assert.match(popup, new RegExp('"' + disposition + '"'));
  }
  assert.match(indicator, /attachShadow\(\{ mode: "closed" \}\)/);
  assert.match(indicator, /style\.textContent = attentionCss\(\)/);
  assert.match(indicator, /prefers-reduced-motion:reduce/);
});
