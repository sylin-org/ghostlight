// SPDX-License-Identifier: Apache-2.0 OR MIT
const { test } = require("node:test");
const assert = require("node:assert");
const {
  LAST_FOCUSED_NORMAL,
  FOCUS_MRU_KEY,
  workspaceGroupKey,
  parseWorkspaceGroupKey,
  resolveWorkspaceWindow,
  rememberFocusedWindow,
  forgetWorkspaceWindow,
  resolveWorkspaceGroup,
  reconcileWorkspaceGroups,
  tabsInWindow,
} = require("../../extension/lib/workspace.js");

function normal(id, extra = {}) {
  return { id, type: "normal", incognito: false, ...extra };
}

test("workspace group keys preserve client and window identity", () => {
  const key = workspaceGroupKey("ghostlight-demo", 42);
  assert.deepStrictEqual(parseWorkspaceGroupKey(key), {
    windowId: 42,
    clientKey: "ghostlight-demo",
  });
  assert.strictEqual(parseWorkspaceGroupKey("ghostlight-demo"), null);
});

test("automatic selection pulls the most recently focused normal window", async () => {
  let created = 0;
  const chrome = {
    windows: {
      async getLastFocused(options) {
        assert.deepStrictEqual(options, { windowTypes: ["normal"] });
        return normal(7, { focused: false });
      },
      async getAll() { throw new Error("inventory should not be needed"); },
      async create() { created += 1; return normal(9); },
    },
  };
  const resolved = await resolveWorkspaceWindow(chrome, { select: LAST_FOCUSED_NORMAL });
  assert.strictEqual(resolved.window.id, 7);
  assert.strictEqual(resolved.created, false);
  assert.strictEqual(created, 0);
});

test("live inventory focused state recovers when getLastFocused fails", async () => {
  const chrome = {
    windows: {
      async getLastFocused() { throw new Error("temporarily unavailable"); },
      async getAll() { return [normal(1), normal(2, { focused: true })]; },
      async create() { throw new Error("must not create"); },
    },
    storage: { session: { async get() { return {}; } } },
  };
  const resolved = await resolveWorkspaceWindow(chrome, { select: LAST_FOCUSED_NORMAL });
  assert.strictEqual(resolved.window.id, 2);
  assert.strictEqual(resolved.created, false);
});

test("validated local focus MRU recovers without inventory ordering", async () => {
  const chrome = {
    windows: {
      WINDOW_ID_NONE: -1,
      async getLastFocused() { throw new Error("temporarily unavailable"); },
      async getAll() { return [normal(1), normal(2)]; },
      async get(id) { return normal(id); },
      async create() { throw new Error("must not create"); },
    },
    storage: {
      session: {
        value: {},
        async get(key) { return { [key]: this.value[key] }; },
        async set(update) { Object.assign(this.value, update); },
      },
    },
  };
  assert.strictEqual(await rememberFocusedWindow(chrome, 2), true);
  const resolved = await resolveWorkspaceWindow(chrome, { select: LAST_FOCUSED_NORMAL });
  assert.strictEqual(resolved.window.id, 2);
  assert.deepStrictEqual(chrome.storage.session.value[FOCUS_MRU_KEY], [2]);
  assert.strictEqual(await forgetWorkspaceWindow(chrome, 2), true);
  assert.deepStrictEqual(chrome.storage.session.value[FOCUS_MRU_KEY], []);
});

test("focus events retain receipt order across asynchronous storage", async () => {
  const chrome = {
    windows: {
      WINDOW_ID_NONE: -1,
      async get(id) {
        if (id === 1) await new Promise((resolve) => setTimeout(resolve, 5));
        return normal(id);
      },
    },
    storage: {
      session: {
        value: {},
        async get(key) { return { [key]: this.value[key] }; },
        async set(update) { Object.assign(this.value, update); },
      },
    },
  };
  const first = rememberFocusedWindow(chrome, 1);
  const second = rememberFocusedWindow(chrome, 2);
  await Promise.all([first, second]);
  assert.deepStrictEqual(chrome.storage.session.value[FOCUS_MRU_KEY], [2, 1]);
});

test("Linux WINDOW_ID_NONE before a window switch does not erase focus order", async () => {
  const chrome = {
    windows: {
      WINDOW_ID_NONE: -1,
      async get(id) { return normal(id); },
    },
    storage: {
      session: {
        value: { [FOCUS_MRU_KEY]: [1] },
        async get(key) { return { [key]: this.value[key] }; },
        async set(update) { Object.assign(this.value, update); },
      },
    },
  };
  assert.strictEqual(await rememberFocusedWindow(chrome, -1), false);
  assert.strictEqual(await rememberFocusedWindow(chrome, 2), true);
  assert.deepStrictEqual(chrome.storage.session.value[FOCUS_MRU_KEY], [2, 1]);
});

test("inventory failure never authorizes a new window", async () => {
  let creates = 0;
  const chrome = {
    windows: {
      async getLastFocused() { throw new Error("temporarily unavailable"); },
      async getAll() { throw new Error("inventory unavailable"); },
      async create() { creates += 1; },
    },
  };
  await assert.rejects(
    resolveWorkspaceWindow(chrome, { select: LAST_FOCUSED_NORMAL }),
    /will not create another one/
  );
  assert.strictEqual(creates, 0);
});

test("a pinned window is validated and never silently replaced", async () => {
  const chrome = {
    windows: {
      async get(id) {
        assert.strictEqual(id, 11);
        throw new Error("closed");
      },
    },
  };
  await assert.rejects(
    resolveWorkspaceWindow(chrome, { windowId: 11 }),
    /no longer eligible/
  );
});

test("a new window is created only when no eligible normal window exists", async () => {
  let created = 0;
  const chrome = {
    windows: {
      async getLastFocused() { throw new Error("none"); },
      async getAll() { return []; },
      async create(options) {
        created += 1;
        assert.deepStrictEqual(options, { focused: true, type: "normal" });
        return normal(23, { tabs: [{ id: 101, windowId: 23 }] });
      },
    },
  };
  const resolved = await resolveWorkspaceWindow(chrome, { select: LAST_FOCUSED_NORMAL });
  assert.strictEqual(resolved.window.id, 23);
  assert.strictEqual(resolved.created, true);
  assert.strictEqual(created, 1);
});

test("ambiguous inventory is not treated as an ordering signal", async () => {
  const chrome = {
    windows: {
      async getLastFocused() { throw new Error("unknown"); },
      async getAll() { return [normal(1), normal(2)]; },
      async create() { throw new Error("must not create"); },
    },
  };
  await assert.rejects(
    resolveWorkspaceWindow(chrome, { select: LAST_FOCUSED_NORMAL }),
    /Several normal browser windows/
  );
});

test("stored client groups migrate to window-scoped keys", async () => {
  const groups = new Map([["ghostlight-demo", 55]]);
  const chrome = {
    tabGroups: {
      async get(id) { return { id, windowId: 8 }; },
    },
  };
  assert.strictEqual(await reconcileWorkspaceGroups(chrome, groups), true);
  assert.deepStrictEqual(Array.from(groups.entries()), [
    [workspaceGroupKey("ghostlight-demo", 8), 55],
  ]);
});

test("a user-moved group is re-keyed and never pulled back", async () => {
  const original = workspaceGroupKey("ghostlight-demo", 8);
  const moved = workspaceGroupKey("ghostlight-demo", 12);
  const groups = new Map([[original, 55]]);
  const chrome = {
    tabGroups: {
      async get(id) { return { id, windowId: 12 }; },
    },
  };
  assert.deepStrictEqual(
    await resolveWorkspaceGroup(chrome, groups, "ghostlight-demo", 8),
    { key: original, groupId: null, changed: true }
  );
  assert.deepStrictEqual(Array.from(groups.entries()), [[moved, 55]]);
});

test("window filtering never moves a named tab from another window", async () => {
  const chrome = {
    tabs: {
      async get(id) {
        if (id === 3) throw new Error("closed");
        return { id, windowId: id === 1 ? 10 : 20 };
      },
    },
  };
  assert.deepStrictEqual(await tabsInWindow(chrome, [1, 2, 3], 10), [1]);
});
