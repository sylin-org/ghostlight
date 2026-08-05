// SPDX-License-Identifier: Apache-2.0 OR MIT
const { test } = require("node:test");
const assert = require("node:assert");
const {
  TAB_DELTA_V1,
  createTabEffectJournal,
  requestsTabDelta,
  attachTabDelta,
} = require("../../extension/lib/tab-effects.js");

test("a child opened during one source-tab action produces a bounded delta", () => {
  const journal = createTabEffectJournal();
  const cursor = journal.cursor();
  assert.strictEqual(journal.opened("workspace", 10, { id: 11, active: true }), true);
  assert.deepStrictEqual(journal.deltaSince(cursor, "workspace", 10), {
    opened: [{ tabId: 11, active: true }],
    closed: [],
    activeTabId: 11,
    more: false,
  });
});

test("workspace and opener correlation never consume unrelated transitions", () => {
  const journal = createTabEffectJournal();
  const cursor = journal.cursor();
  journal.opened("workspace-a", 10, { id: 11, active: false });
  journal.opened("workspace-b", 20, { id: 21, active: true });
  assert.deepStrictEqual(journal.deltaSince(cursor, "workspace-a", 10), {
    opened: [{ tabId: 11, active: false }],
    closed: [],
    more: false,
  });
  assert.strictEqual(journal.deltaSince(cursor, "workspace-a", 20), null);
});

test("a child that closes during the same action retains its opener correlation", () => {
  const journal = createTabEffectJournal();
  const cursor = journal.cursor();
  journal.opened("workspace", 10, { id: 11, active: true });
  journal.closed("workspace", 11);
  assert.deepStrictEqual(journal.deltaSince(cursor, "workspace", 10), {
    opened: [{ tabId: 11, active: true }],
    closed: [11],
    more: false,
  });
});

test("journal reports truncation instead of growing one result without bound", () => {
  const journal = createTabEffectJournal({ maxEvents: 8, maxItems: 2 });
  const cursor = journal.cursor();
  journal.opened("workspace", 10, { id: 11 });
  journal.opened("workspace", 10, { id: 12 });
  journal.opened("workspace", 10, { id: 13 });
  assert.deepStrictEqual(journal.deltaSince(cursor, "workspace", 10), {
    opened: [
      { tabId: 11, active: false },
      { tabId: 12, active: false },
    ],
    closed: [],
    more: true,
  });
});

test("tab delta is attached only through the explicit compatibility feature", () => {
  assert.strictEqual(requestsTabDelta({ resultFeatures: [TAB_DELTA_V1] }), true);
  assert.strictEqual(requestsTabDelta({}), false);
  const result = { content: [{ type: "text", text: "Clicked." }] };
  attachTabDelta(result, { opened: [], closed: [10], more: false });
  assert.deepStrictEqual(result.structuredContent.tabDelta, {
    opened: [], closed: [10], more: false,
  });
});
