// SPDX-License-Identifier: Apache-2.0 OR MIT
const { test } = require("node:test");
const assert = require("node:assert");
const {
  FOCUS_MRU_KEY,
  resolveWorkspaceWindow,
  rememberFocusedWindow,
  forgetWorkspaceWindow,
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
} = require("../../extension/lib/workspace.js");

function normal(id, extra = {}) {
  return { id, type: "normal", incognito: false, ...extra };
}

test("initial placement pulls the most recently focused normal window", async () => {
  let created = 0;
  const chrome = {
    windows: {
      async getLastFocused(options) {
        assert.deepStrictEqual(options, { windowTypes: ["normal"] });
        return normal(7);
      },
      async getAll() { throw new Error("inventory should not be needed"); },
      async create() { created += 1; return normal(9); },
    },
  };
  const resolved = await resolveWorkspaceWindow(chrome);
  assert.strictEqual(resolved.window.id, 7);
  assert.strictEqual(resolved.created, false);
  assert.strictEqual(created, 0);
});

test("validated focus MRU recovers without inventory ordering", async () => {
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
  const resolved = await resolveWorkspaceWindow(chrome);
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
  await Promise.all([rememberFocusedWindow(chrome, 1), rememberFocusedWindow(chrome, 2)]);
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
  await assert.rejects(resolveWorkspaceWindow(chrome), /will not create another one/);
  assert.strictEqual(creates, 0);
});

test("a new window is created only when no eligible normal window exists", async () => {
  const chrome = {
    windows: {
      async getLastFocused() { throw new Error("none"); },
      async getAll() { return []; },
      async create(options) {
        assert.deepStrictEqual(options, { focused: true, type: "normal" });
        return normal(23, { tabs: [{ id: 101, windowId: 23 }] });
      },
    },
  };
  const resolved = await resolveWorkspaceWindow(chrome);
  assert.strictEqual(resolved.window.id, 23);
  assert.strictEqual(resolved.created, true);
});

test("a new workspace adopts an exact-title group only in its selected window", async () => {
  const index = new Map();
  const chrome = {
    windows: {
      async getLastFocused() { return normal(8); },
    },
    tabGroups: {
      async query(query) {
        assert.deepStrictEqual(query, { title: "Ghostlight - Codex", windowId: 8 });
        return [
          { id: 55, windowId: 8, title: query.title },
          { id: 66, windowId: 9, title: query.title },
        ];
      },
    },
  };
  const placement = await resolveWorkspacePlacement(
    chrome,
    index,
    "workspace-new",
    "Ghostlight - Codex"
  );
  assert.strictEqual(placement.windowId, 8);
  assert.strictEqual(placement.group.id, 55);
});

test("a moved whole group becomes the workspace placement without consulting focus", async () => {
  const index = new Map();
  addWorkspaceTab(index, "workspace", 1, 55);
  const chrome = {
    tabs: {
      async get(id) { return { id, windowId: 12, groupId: 55, lastAccessed: 7 }; },
    },
    tabGroups: {
      async get(id) { return { id, windowId: 12, title: "Ghostlight - Codex" }; },
      async query() { throw new Error("the live group already decides placement"); },
    },
    windows: {
      async getLastFocused() { throw new Error("focus must not override live placement"); },
    },
  };
  const placement = await resolveWorkspacePlacement(
    chrome,
    index,
    "workspace",
    "Ghostlight - Codex"
  );
  assert.strictEqual(placement.windowId, 12);
  assert.strictEqual(placement.group.id, 55);
  assert.deepStrictEqual(placement.tabs.map((tab) => tab.id), [1]);
});

test("a detached owned tab stays put and anchors later placement in its new window", async () => {
  const index = new Map();
  addWorkspaceTab(index, "workspace", 1, 55);
  const chrome = {
    tabs: {
      async get(id) { return { id, windowId: 12, groupId: -1, lastAccessed: 9 }; },
    },
    tabGroups: {
      async query(query) {
        assert.deepStrictEqual(query, { title: "Ghostlight - Codex", windowId: 12 });
        return [{ id: 77, windowId: 12, title: query.title }];
      },
    },
  };
  const placement = await resolveWorkspacePlacement(
    chrome,
    index,
    "workspace",
    "Ghostlight - Codex"
  );
  assert.strictEqual(placement.windowId, 12);
  assert.strictEqual(placement.group.id, 77);
  assert.strictEqual(index.get("workspace").groupId, null);
});

test("the most recently accessed grouped owned tab chooses among split groups", async () => {
  const index = new Map();
  replaceWorkspaceTabs(index, "workspace", [1, 2], 55);
  const chrome = {
    tabs: {
      async get(id) {
        return id === 1
          ? { id, windowId: 8, groupId: 55, lastAccessed: 4 }
          : { id, windowId: 12, groupId: 77, lastAccessed: 9 };
      },
    },
    tabGroups: {
      async get(id) { return { id, windowId: id === 55 ? 8 : 12 }; },
    },
  };
  const group = await liveWorkspaceGroup(
    chrome,
    index,
    "workspace",
    await liveWorkspaceTabs(chrome, index, "workspace")
  );
  assert.strictEqual(group.id, 77);
  assert.strictEqual(index.get("workspace").groupId, 77);
});

test("managed exact-title groups win deterministic adoption", async () => {
  const index = new Map();
  replaceWorkspaceTabs(index, "old", [1], 90);
  const chrome = {
    tabGroups: {
      async query() {
        return [
          { id: 40, windowId: 8 },
          { id: 90, windowId: 8 },
        ];
      },
    },
  };
  const selected = await preferredNamedGroup(chrome, index, 8, "Ghostlight - Codex");
  assert.strictEqual(selected.id, 90);
});

test("shared presentation keeps workspace tab inventories separate", async () => {
  const index = new Map();
  replaceWorkspaceTabs(index, "workspace-a", [1, 2], 55);
  replaceWorkspaceTabs(index, "workspace-b", [3], 55);
  addWorkspaceTab(index, "workspace-a", 4, 55);
  const chrome = {
    tabs: {
      async get(id) {
        if (id === 2) throw new Error("closed");
        return { id, groupId: 55, title: `tab-${id}` };
      },
    },
  };
  assert.deepStrictEqual(
    (await liveWorkspaceTabs(chrome, index, "workspace-a")).map((tab) => tab.id),
    [1, 4]
  );
  assert.deepStrictEqual(
    (await liveWorkspaceTabs(chrome, index, "workspace-b")).map((tab) => tab.id),
    [3]
  );
  removeWorkspaceTab(index, 1);
  assert.deepStrictEqual(Array.from(index.get("workspace-a").tabIds), [4]);
  assert.deepStrictEqual(Array.from(index.get("workspace-b").tabIds), [3]);
  assert.deepStrictEqual(Array.from(workspaceGroupIds(index)), [55]);
  assert.strictEqual(isWorkspaceGroupId(index, 55), true);
});

test("reverse lookup returns only one unambiguous workspace owner", () => {
  const index = new Map();
  replaceWorkspaceTabs(index, "workspace-a", [1, 2], 55);
  replaceWorkspaceTabs(index, "workspace-b", [3], 55);
  assert.strictEqual(workspaceIdForTab(index, 2), "workspace-a");
  assert.strictEqual(workspaceIdForTab(index, 99), null);
  addWorkspaceTab(index, "workspace-b", 2, 55);
  assert.strictEqual(workspaceIdForTab(index, 2), null);
});

test("topology serialization round-trips tab sets and shared groups", () => {
  const source = new Map();
  replaceWorkspaceTabs(source, "a", [1, 2], 55);
  replaceWorkspaceTabs(source, "b", [3], 55);
  const restored = new Map();
  assert.strictEqual(
    restoreWorkspaceTopology(restored, serializeWorkspaceTopology(source)),
    true
  );
  assert.deepStrictEqual(Array.from(restored.get("a").tabIds), [1, 2]);
  assert.strictEqual(restored.get("a").groupId, 55);
  assert.deepStrictEqual(Array.from(restored.get("b").tabIds), [3]);
});

test("legacy window-qualified maps migrate into one workspace record", () => {
  const oldWindow = JSON.stringify(["v1", 8, "workspace"]);
  const movedWindow = JSON.stringify(["v1", 12, "workspace"]);
  const index = new Map();
  assert.strictEqual(migrateLegacyWorkspaceTopology(index, {
    clientGroupsState: [[movedWindow, 55]],
    workspaceTabsState: [[oldWindow, [1, 2]]],
  }), true);
  assert.deepStrictEqual(Array.from(index.get("workspace").tabIds), [1, 2]);
  assert.strictEqual(index.get("workspace").groupId, 55);
});

test("dead groups are pruned without losing workspace tab ownership", async () => {
  const index = new Map();
  replaceWorkspaceTabs(index, "workspace", [1], 55);
  const chrome = {
    tabGroups: { async get() { throw new Error("gone"); } },
  };
  assert.strictEqual(await pruneWorkspaceGroups(chrome, index), true);
  assert.strictEqual(index.get("workspace").groupId, null);
  assert.deepStrictEqual(Array.from(index.get("workspace").tabIds), [1]);
});
