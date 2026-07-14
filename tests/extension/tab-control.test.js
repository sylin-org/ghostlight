// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/tab-control.js (ADR-0078 D7).

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const { makeReceipt } = require("../../extension/lib/tab-control.js");

test("tab control receipts use content-free observed categories", () => {
  const page = { tabId: 7 };
  assert.deepStrictEqual(makeReceipt("focus", page).observedAfter, { tabFocused: true });
  assert.deepStrictEqual(makeReceipt("reload", page).observedAfter, { tabReloaded: true });
  assert.deepStrictEqual(makeReceipt("close", page).observedAfter, { tabClosed: true });
  assert.throws(() => makeReceipt("unknown", page), /unsupported/);
});

test("worker controls exactly one tab and never deletes a tab group", () => {
  const source = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  const start = source.indexOf("async tab_control(a)");
  const end = source.indexOf("async computer(a)", start);
  const handler = source.slice(start, end);
  assert.match(handler, /effectiveTabId\(a\.tabId\)/);
  assert.match(handler, /chrome\.tabs\.update\(tabId, \{ active: true \}\)/);
  assert.match(handler, /chrome\.tabs\.reload\(tabId\)/);
  assert.match(handler, /chrome\.tabs\.remove\(tabId\)/);
  assert.doesNotMatch(handler, /tabGroups\.(remove|ungroup)/);
  assert.doesNotMatch(handler, /chrome\.windows\.remove/);
});

test("tab close and browser close share idempotent transient cleanup", () => {
  const source = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  assert.match(source, /function clearTabState\(tabId\)/);
  assert.match(source, /chrome\.tabs\.onRemoved\.addListener\(\(tabId\) => \{\s*clearTabState\(tabId\)/);
  assert.match(source, /chrome\.tabs\.remove\(tabId\);\s*clearTabState\(tabId\)/);
});
