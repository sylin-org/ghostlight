"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Tabs = require("../lib/tabs.js");
const state = require("../lib/state.js");
const { fakeChrome, wire } = require("./p4-fakes.js");
const { golden, goldenArgs, execute, fakeBrowser } = require("./p4-chrome.js");

const WINDOW = 1;
const OTHER_WINDOW = 2;
const TITLE = "Ghostlight - Research";
const KEY = "workspace_9a2c";
const HANDLERS = ["listTabs", "openTab", "navigate", "focusTab", "closeTab", "setZoom", "resizeWindow"];

function tab(overrides) {
  return { windowId: WINDOW, url: "https://example.com/", title: "Example Domain", ...overrides };
}

function setup({ browser = {}, cancelled = [] } = {}) {
  const chrome = fakeBrowser(browser);
  const debuggerChrome = fakeChrome();
  const { lifecycle } = wire(debuggerChrome);
  let clock = 0;
  const hook = {
    calls: [],
    async manage(id) { this.calls.push(["manage", id]); },
    async release(id) { this.calls.push(["release", id]); }
  };
  const tabs = Tabs.create({
    chromeApi: chrome,
    lifecycle,
    controlled: hook,
    isCancelled: (correlation) => cancelled.includes(correlation),
    now: () => clock,
    sleep: async (ms) => { clock += ms; }
  });
  return { chrome, debuggerChrome, lifecycle, hook, tabs, advance: (ms) => { clock += ms; } };
}

test("every handler carries validate, run, addresses and tabOf", () => {
  const { tabs } = setup();
  for (const name of HANDLERS) {
    const handler = tabs[name];
    assert.equal(typeof handler.validate, "function", name);
    assert.equal(typeof handler.run, "function", name);
    assert.equal(typeof handler.addresses, "function", name);
    assert.equal(handler.tabOf({ tab_id: 7 }), 7, name);
  }
  assert.deepEqual(
    HANDLERS.filter((name) => tabs[name].addresses({})),
    ["navigate", "focusTab", "setZoom", "resizeWindow"]
  );
});

test("every golden request for a tab primitive passes validation", () => {
  const { tabs } = setup();
  const requests = {
    listTabs: ["list_tabs"],
    openTab: ["open_tab", "open_tab.adopt"],
    navigate: ["navigate.url", "navigate.back", "navigate.forward", "navigate.reload"],
    focusTab: ["focus_tab"],
    closeTab: ["close_tab"],
    setZoom: ["set_zoom"],
    resizeWindow: ["resize_window"]
  };
  for (const [handler, names] of Object.entries(requests)) {
    for (const name of names) assert.doesNotThrow(() => tabs[handler].validate(goldenArgs(name)), name);
  }
});

test("bad arguments are invalid_request before anything happens", async () => {
  const { tabs, chrome } = setup();
  const open = goldenArgs("open_tab");
  const cases = [
    [tabs.listTabs, { tab_ids: "12" }],
    [tabs.listTabs, { tab_ids: [0] }],
    [tabs.openTab, { ...open, group_key: "" }],
    [tabs.openTab, { ...open, focus_window: "yes" }],
    [tabs.openTab, { ...open, ready_timeout_ms: -1 }],
    [tabs.openTab, { ...open, adopt_tab_id: 0 }],
    [tabs.navigate, { ...goldenArgs("navigate.url"), to: { to: "sideways" } }],
    [tabs.navigate, { ...goldenArgs("navigate.url"), to: { to: "url" } }],
    [tabs.navigate, { ...goldenArgs("navigate.url"), tab_id: "12" }],
    [tabs.focusTab, {}],
    [tabs.closeTab, { tab_id: 12, released: "no" }],
    [tabs.setZoom, { tab_id: 12, zoom: Number.NaN }],
    [tabs.setZoom, { tab_id: 12, zoom: -1 }],
    [tabs.resizeWindow, { tab_id: 12, width: 100, height: 800 }],
    [tabs.resizeWindow, { tab_id: 12, width: 1280, height: 4321 }],
    [tabs.resizeWindow, { tab_id: 12, width: 1280.5, height: 800 }]
  ];
  for (const [handler, args] of cases) {
    const result = await execute(handler, args);
    assert.equal(result.failure, "invalid_request", JSON.stringify(args));
    assert.equal(result.dispatched, false);
  }
  assert.equal(chrome.calls.length, 0);
});

test("invalid_request messages match the golden receipts", async () => {
  const { tabs } = setup();
  const { url: _url, ...withoutUrl } = goldenArgs("open_tab");
  const open = await execute(tabs.openTab, withoutUrl);
  assert.equal(open.failure, golden("receipts/open_tab.invalid_request").failure);
  assert.equal(open.message, golden("receipts/open_tab.invalid_request").message);
  const resize = await execute(tabs.resizeWindow, { tab_id: 12, width: 100, height: 800 });
  assert.equal(resize.message, golden("receipts/resize_window.invalid_request").message);
});

// ---- list_tabs --------------------------------------------------------------------------------

test("list_tabs reports each tab's facts, matching the golden receipt", async () => {
  const { tabs } = setup({
    browser: {
      tabs: [
        { id: 3, windowId: WINDOW, url: "https://example.com/start", title: "Start", active: false },
        { id: 12, windowId: WINDOW, url: "https://example.com/account", title: "Account", active: true, openerTabId: 3 },
        { id: 13, windowId: WINDOW, url: "https://docs.example.com/", title: "Docs", active: false, status: "loading" }
      ]
    }
  });
  const result = await execute(tabs.listTabs, goldenArgs("list_tabs"));
  assert.deepEqual(result.ok, golden("receipts/list_tabs.ok").ok);
  assert.equal(result.dispatched, golden("receipts/list_tabs.ok").dispatched);
});

test("list_tabs without tab_ids lists every tab and never reports an opener", async () => {
  const { tabs } = setup({
    browser: {
      tabs: [
        { id: 3, windowId: WINDOW, url: "https://a.example/", title: "A" },
        { id: 4, windowId: WINDOW, url: "", pendingUrl: "https://b.example/", title: "B", openerTabId: 3, status: "loading" },
        { id: 5, windowId: WINDOW, url: "", title: "" }
      ]
    }
  });
  const { ok } = await execute(tabs.listTabs, {});
  assert.deepEqual(ok.tabs.map((entry) => entry.tab_id), [3, 4, 5]);
  assert.equal("opener_tab_id" in ok.tabs[1], false);
  assert.equal(ok.tabs[1].url, "https://b.example/");
  assert.equal(ok.tabs[1].readiness, "loading");
  assert.equal(ok.tabs[2].url, "about:blank");
});

test("list_tabs with tab_ids includes the children of a named tab", async () => {
  const { tabs } = setup({
    browser: {
      tabs: [
        { id: 3, windowId: WINDOW, url: "https://a.example/", title: "A" },
        { id: 4, windowId: WINDOW, url: "https://b.example/", title: "B", openerTabId: 3 },
        { id: 5, windowId: WINDOW, url: "https://c.example/", title: "C" }
      ]
    }
  });
  const { ok } = await execute(tabs.listTabs, { tab_ids: [3] });
  assert.deepEqual(ok.tabs.map((entry) => entry.tab_id), [3, 4]);
  assert.equal(ok.tabs.some((entry) => "opener_tab_id" in entry), false);
});

// ---- open_tab ---------------------------------------------------------------------------------

test("open_tab creates a window and a group when there is neither, matching the golden receipt", async () => {
  const { tabs, chrome, hook } = setup({ browser: { tabs: [tab({ id: 1, active: true })] } });
  const result = await execute(tabs.openTab, goldenArgs("open_tab"));
  const created = chrome.named("windows.create");
  assert.equal(created.length, 1);
  assert.deepEqual(created[0].args[0], { url: "https://example.com/", focused: false, type: "normal" });
  assert.equal(result.ok.adopted, golden("receipts/open_tab.ok").ok.adopted);
  assert.deepEqual(Object.keys(result.ok.tab), Object.keys(golden("receipts/open_tab.ok").ok.tab));
  assert.equal(result.dispatched, true);
  assert.deepEqual(result.watched, [result.ok.tab.tab_id]);
  assert.deepEqual(hook.calls, [["manage", result.ok.tab.tab_id]]);
  assert.deepEqual(chrome.groupList.map((group) => [group.title, group.color, group.collapsed]), [[TITLE, "blue", false]]);
  assert.deepEqual(chrome.store.session.get(Tabs.GROUPS_KEY), [[KEY, { groupId: chrome.groupList[0].id, title: TITLE }]]);
});

test("open_tab with focus_window focuses the new window or the group's window", async () => {
  const fresh = setup();
  await execute(fresh.tabs.openTab, { ...goldenArgs("open_tab"), focus_window: true });
  assert.equal(fresh.chrome.named("windows.create")[0].args[0].focused, true);
  assert.equal(fresh.chrome.named("windows.update").length, 0);

  const existing = setup({
    browser: {
      tabs: [tab({ id: 1, windowId: OTHER_WINDOW, groupId: 9 })],
      groups: [{ id: 9, windowId: OTHER_WINDOW, title: TITLE }]
    }
  });
  await execute(existing.tabs.openTab, { ...goldenArgs("open_tab"), focus_window: true });
  assert.deepEqual(existing.chrome.named("windows.update").map((call) => call.args), [[OTHER_WINDOW, { focused: true }]]);
  const quiet = setup({
    browser: {
      tabs: [tab({ id: 1, windowId: OTHER_WINDOW, groupId: 9 })],
      groups: [{ id: 9, windowId: OTHER_WINDOW, title: TITLE }]
    }
  });
  await execute(quiet.tabs.openTab, goldenArgs("open_tab"));
  assert.equal(quiet.chrome.named("windows.update").length, 0);
});

test("a second open_tab for the same key joins the stored group in its window", async () => {
  const { tabs, chrome } = setup({ browser: { tabs: [tab({ id: 1, active: true })] } });
  const first = await execute(tabs.openTab, goldenArgs("open_tab"));
  const second = await execute(tabs.openTab, { ...goldenArgs("open_tab"), url: "https://example.com/two" });
  assert.equal(chrome.named("windows.create").length, 1);
  const groupId = chrome.groupList[0].id;
  assert.equal(chrome.groupList.length, 1);
  const created = chrome.named("tabs.create");
  assert.equal(created.length, 1);
  assert.equal(created[0].args[0].windowId, chrome.tabList.find((item) => item.id === first.ok.tab.tab_id).windowId);
  assert.deepEqual(chrome.tabList.filter((item) => item.groupId === groupId).map((item) => item.id), [first.ok.tab.tab_id, second.ok.tab.tab_id]);
});

test("after an extension reload the group is found again by its title", async () => {
  const { tabs, chrome } = setup({
    browser: {
      tabs: [tab({ id: 1, windowId: OTHER_WINDOW, groupId: 9 })],
      groups: [{ id: 9, windowId: OTHER_WINDOW, title: TITLE }]
    }
  });
  assert.equal(chrome.store.session.size, 0);
  const result = await execute(tabs.openTab, goldenArgs("open_tab"));
  assert.equal(chrome.named("windows.create").length, 0);
  assert.equal(chrome.named("tabs.create")[0].args[0].windowId, OTHER_WINDOW);
  assert.equal(chrome.tabList.find((item) => item.id === result.ok.tab.tab_id).groupId, 9);
  assert.equal(chrome.groupList.length, 1);
  assert.deepEqual(chrome.store.session.get(Tabs.GROUPS_KEY), [[KEY, { groupId: 9, title: TITLE }]]);
});

test("a stored group that lost its title is replaced by the group that has it", async () => {
  const { tabs, chrome } = setup({
    browser: {
      tabs: [tab({ id: 1, groupId: 8 }), tab({ id: 2, groupId: 9 })],
      groups: [{ id: 8, windowId: WINDOW, title: "Renamed by hand" }, { id: 9, windowId: WINDOW, title: TITLE }]
    }
  });
  chrome.store.session.set(Tabs.GROUPS_KEY, [[KEY, { groupId: 8, title: TITLE }]]);
  const result = await execute(tabs.openTab, goldenArgs("open_tab"));
  assert.equal(chrome.tabList.find((item) => item.id === result.ok.tab.tab_id).groupId, 9);
});

test("same-title duplicate groups are merged into the lowest-id group", async () => {
  const { tabs, chrome } = setup({
    browser: {
      tabs: [tab({ id: 1, groupId: 9 }), tab({ id: 2, groupId: 12 }), tab({ id: 3, groupId: 12 }), tab({ id: 4, groupId: 15 })],
      groups: [
        { id: 15, windowId: WINDOW, title: TITLE },
        { id: 9, windowId: WINDOW, title: TITLE },
        { id: 12, windowId: WINDOW, title: TITLE }
      ]
    }
  });
  await execute(tabs.openTab, goldenArgs("open_tab"));
  assert.deepEqual(chrome.groupList.map((group) => group.id), [9]);
  assert.deepEqual(chrome.tabList.filter((item) => item.groupId === 9).map((item) => item.id).sort((a, b) => a - b), [1, 2, 3, 4, 101]);
});

test("open_tab adopts the named tab: it moves into the group's window and is navigated, matching the golden receipt", async () => {
  const { tabs, chrome, hook } = setup({
    browser: {
      tabs: [
        tab({ id: 14, windowId: WINDOW, url: "https://example.com/old", title: "Inbox", active: false }),
        tab({ id: 20, windowId: OTHER_WINDOW, groupId: 9, active: true })
      ],
      groups: [{ id: 9, windowId: OTHER_WINDOW, title: TITLE }]
    }
  });
  const result = await execute(tabs.openTab, goldenArgs("open_tab.adopt"));
  const expected = golden("receipts/open_tab.ok_adopted");
  assert.equal(result.ok.adopted, true);
  assert.equal(result.ok.tab.tab_id, expected.ok.tab.tab_id);
  assert.equal(result.dispatched, expected.dispatched);
  assert.deepEqual(result.watched, [14]);
  assert.equal(chrome.named("tabs.create").length, 0);
  assert.equal(chrome.named("windows.create").length, 0);
  const adopted = chrome.tabList.find((item) => item.id === 14);
  assert.equal(adopted.windowId, OTHER_WINDOW);
  assert.equal(adopted.groupId, 9);
  assert.equal(adopted.url, "https://example.com/inbox");
  assert.equal(adopted.active, true);
  assert.deepEqual(hook.calls, [["manage", 14]]);
  assert.deepEqual(chrome.named("windows.update").map((call) => call.args), [[OTHER_WINDOW, { focused: true }]]);
});

test("an adopted tab in a group of the person's stays in it and is navigated there", async () => {
  const { tabs, chrome } = setup({
    browser: {
      tabs: [
        tab({ id: 14, windowId: WINDOW, groupId: 7, url: "https://example.com/old" }),
        tab({ id: 20, windowId: OTHER_WINDOW, groupId: 9, active: true })
      ],
      groups: [{ id: 7, windowId: WINDOW, title: "Reading" }, { id: 9, windowId: OTHER_WINDOW, title: TITLE }]
    }
  });
  const result = await execute(tabs.openTab, { ...goldenArgs("open_tab.adopt"), focus_window: false });
  assert.equal(result.ok.adopted, true);
  const adopted = chrome.tabList.find((item) => item.id === 14);
  assert.deepEqual([adopted.windowId, adopted.groupId, adopted.url], [WINDOW, 7, "https://example.com/inbox"]);
  assert.equal(chrome.named("tabs.move").length, 0);
  assert.equal(chrome.named("tabs.group").length, 0);
});

test("open_tab adopting without a url only activates the tab, and a missing tab is tab_missing with nothing dispatched", async () => {
  const { tabs, chrome } = setup({ browser: { tabs: [tab({ id: 14, active: false })] } });
  const { url: _url, ...withoutUrl } = goldenArgs("open_tab.adopt");
  const adopted = await execute(tabs.openTab, { ...withoutUrl, focus_window: false });
  assert.equal(adopted.ok.adopted, true);
  assert.deepEqual(chrome.named("tabs.update").map((call) => call.args), [[14, { active: true }]]);
  assert.equal(chrome.groupList.length, 1);

  const lost = await execute(tabs.openTab, { ...goldenArgs("open_tab.adopt"), adopt_tab_id: 99 });
  assert.equal(lost.failure, "tab_missing");
  assert.equal(lost.message, "No tab with id: 99.");
  assert.equal(lost.dispatched, false);
});

test("open_tab waits for the tab and times out with the golden message", async () => {
  const { tabs, chrome } = setup({ browser: { tabs: [tab({ id: 1 })] } });
  chrome.tabs.create = async (properties) => {
    chrome.tabList.push({ id: 77, windowId: properties.windowId, url: properties.url, title: "", status: "loading", active: true, groupId: -1 });
    return { ...chrome.tabList.at(-1) };
  };
  chrome.groupList.push({ id: 9, windowId: WINDOW, title: TITLE, collapsed: false, color: "blue" });
  chrome.tabList[0].groupId = 9;
  const result = await execute(tabs.openTab, goldenArgs("open_tab"));
  assert.equal(result.failure, "timeout");
  assert.equal(result.message, golden("receipts/open_tab.timeout").message);
  assert.equal(result.dispatched, true);
});

test("open_tab polls until the tab is complete", async () => {
  const { tabs, chrome } = setup({ browser: { readyAfter: 4, tabs: [tab({ id: 1 })] } });
  const result = await execute(tabs.openTab, goldenArgs("open_tab"));
  assert.equal(result.ok.tab.readiness, "complete");
  assert.ok(chrome.named("tabs.get").length >= 4);
});

test("a cancel during the ready wait ends it with cancelled and dispatched stays true", async () => {
  const { tabs, chrome } = setup({ cancelled: ["c1"], browser: { tabs: [tab({ id: 1 })] } });
  chrome.tabs.create = async (properties) => {
    chrome.tabList.push({ id: 77, windowId: properties.windowId, url: properties.url, title: "", status: "loading", active: true, groupId: -1 });
    return { ...chrome.tabList.at(-1) };
  };
  chrome.groupList.push({ id: 9, windowId: WINDOW, title: TITLE, collapsed: false, color: "blue" });
  chrome.tabList[0].groupId = 9;
  const result = await execute(tabs.openTab, goldenArgs("open_tab"), "c1");
  assert.equal(result.failure, "cancelled");
  assert.equal(result.dispatched, true);
});

// ---- navigate ---------------------------------------------------------------------------------

test("navigate to a url matches the golden receipt", async () => {
  const { tabs, chrome } = setup({ browser: { tabs: [tab({ id: 12, active: true, title: "Example Domain" })] } });
  const result = await execute(tabs.navigate, goldenArgs("navigate.url"));
  const expected = golden("receipts/navigate.ok");
  assert.deepEqual(result.ok, expected.ok);
  assert.equal(result.dispatched, expected.dispatched);
  assert.deepEqual(result.watched, [12]);
  assert.deepEqual(chrome.named("tabs.update").map((call) => call.args), [[12, { url: "https://example.com/", active: true }]]);
});

test("navigate back and forward go to the exact history entry, and reload calls the tab method", async () => {
  const { tabs, chrome, lifecycle, debuggerChrome } = setup({ browser: { tabs: [tab({ id: 12 })] } });
  debuggerChrome.handlers["Page.getNavigationHistory"] = () => ({ currentIndex: 1, entries: [{ id: 10 }, { id: 11 }, { id: 12 }] });
  for (const name of ["navigate.back", "navigate.forward", "navigate.reload"]) {
    const result = await execute(tabs.navigate, goldenArgs(name));
    assert.equal(result.ok.tab.tab_id, 12, name);
    assert.equal(result.dispatched, true, name);
  }
  const sent = debuggerChrome.calls.filter((call) => call.method === "Page.navigateToHistoryEntry");
  assert.deepEqual(sent.map((call) => [call.target, call.params]), [[{ tabId: 12 }, { entryId: 10 }], [{ tabId: 12 }, { entryId: 12 }]]);
  // The browser's back button skips a page the person never touched; the exact entry does not.
  assert.equal(chrome.named("tabs.goBack").length, 0);
  assert.equal(chrome.named("tabs.goForward").length, 0);
  assert.deepEqual(chrome.named("tabs.reload").map((call) => call.args), [[12, { bypassCache: true }]]);
  assert.equal(lifecycle.attachedCount(), 0, "the debugger lease is given back");
});

test("back from the first entry and forward from the last send nothing and say no_history_entry", async () => {
  const { tabs, lifecycle, debuggerChrome } = setup({ browser: { tabs: [tab({ id: 12 })] } });
  const cases = [
    [{ currentIndex: 0, entries: [{ id: 5 }, { id: 6 }] }, "navigate.back", "The tab has no earlier page in its history."],
    [{ currentIndex: 1, entries: [{ id: 5 }, { id: 6 }] }, "navigate.forward", "The tab has no later page in its history."],
    [{}, "navigate.back", "The tab has no earlier page in its history."]
  ];
  for (const [history, name, message] of cases) {
    debuggerChrome.handlers["Page.getNavigationHistory"] = () => history;
    const result = await execute(tabs.navigate, goldenArgs(name));
    assert.equal(result.failure, "no_history_entry", name);
    assert.equal(result.message, message, name);
    assert.equal(result.dispatched, false, name);
  }
  assert.equal(debuggerChrome.calls.filter((call) => call.method === "Page.navigateToHistoryEntry").length, 0);
  assert.equal(lifecycle.attachedCount(), 0, "the debugger lease is given back");
});

test("navigate on a missing tab is tab_missing before any effect", async () => {
  const { tabs, chrome } = setup();
  const result = await execute(tabs.navigate, goldenArgs("navigate.url"));
  const expected = golden("receipts/navigate.tab_missing");
  assert.equal(result.failure, expected.failure);
  assert.equal(result.message, expected.message);
  assert.equal(result.dispatched, expected.dispatched);
  assert.equal(chrome.named("tabs.update").length, 0);
});

test("navigate that never becomes ready times out with the golden message", async () => {
  const { tabs } = setup({ browser: { tabs: [tab({ id: 12, status: "loading", url: "" })] } });
  const result = await execute(tabs.navigate, { ...goldenArgs("navigate.url"), ready_timeout_ms: 8000 });
  assert.equal(result.failure, "timeout");
  assert.equal(result.message, golden("receipts/navigate.timeout").message);
  assert.equal(result.dispatched, true);
});

test("a cancel frame ends a navigation wait with the golden cancelled message", async () => {
  const { tabs } = setup({ cancelled: ["c1"], browser: { tabs: [tab({ id: 12, status: "loading", url: "" })] } });
  const result = await execute(tabs.navigate, goldenArgs("navigate.url"), "c1");
  assert.equal(result.failure, "cancelled");
  assert.equal(result.message, golden("receipts/navigate.cancelled").message);
  assert.equal(result.dispatched, true);
});

test("discard_beforeunload holds a debugger lease and accepts only beforeunload dialogs while navigating", async () => {
  const { tabs, chrome, lifecycle, debuggerChrome } = setup({ browser: { tabs: [tab({ id: 12 })] } });
  const answers = [];
  const update = chrome.tabs.update;
  chrome.tabs.update = async (id, properties) => {
    assert.equal(lifecycle.attachedCount(), 1);
    answers.push(await tabs.acceptBeforeUnload(id, { type: "beforeunload", sessionId: "s1" }));
    answers.push(await tabs.acceptBeforeUnload(id, { type: "prompt", sessionId: null }));
    answers.push(await tabs.acceptBeforeUnload(id + 1, { type: "beforeunload", sessionId: null }));
    return update(id, properties);
  };
  const result = await execute(tabs.navigate, { ...goldenArgs("navigate.url"), discard_beforeunload: true });
  assert.equal(result.ok.tab.tab_id, 12);
  assert.deepEqual(answers, [true, false, false]);
  const handled = debuggerChrome.calls.filter((call) => call.method === "Page.handleJavaScriptDialog");
  assert.deepEqual(handled.map((call) => [call.target, call.params]), [[{ tabId: 12, sessionId: "s1" }, { accept: true }]]);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(await tabs.acceptBeforeUnload(12, { type: "beforeunload", sessionId: null }), false);
});

test("a navigation that does not discard never attaches the debugger", async () => {
  const { tabs, lifecycle, debuggerChrome } = setup({ browser: { tabs: [tab({ id: 12 })] } });
  await execute(tabs.navigate, { ...goldenArgs("navigate.url"), discard_beforeunload: false });
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(debuggerChrome.calls.length, 0);
});

test("the debugger lease and beforeunload mark are given back when a discarding navigation fails", async () => {
  const { tabs, lifecycle } = setup({ browser: { tabs: [tab({ id: 12, status: "loading", url: "" })] } });
  const result = await execute(tabs.navigate, { ...goldenArgs("navigate.url"), discard_beforeunload: true });
  assert.equal(result.failure, "timeout");
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(await tabs.acceptBeforeUnload(12, { type: "beforeunload" }), false);
});

// ---- focus_tab, set_zoom, resize_window -------------------------------------------------------

test("focus_tab activates the tab and focuses its window, matching the golden receipt", async () => {
  const { tabs, chrome } = setup({ browser: { tabs: [tab({ id: 12, active: false }), tab({ id: 13, active: true })] } });
  const result = await execute(tabs.focusTab, goldenArgs("focus_tab"));
  const expected = golden("receipts/focus_tab.ok");
  assert.deepEqual(result.ok, expected.ok);
  assert.equal(result.dispatched, expected.dispatched);
  assert.deepEqual(result.watched, [12]);
  assert.deepEqual(chrome.named("windows.update").map((call) => call.args), [[WINDOW, { focused: true }]]);
});

test("focus_tab on a missing tab is tab_missing and dispatches nothing", async () => {
  const { tabs, chrome } = setup();
  const result = await execute(tabs.focusTab, goldenArgs("focus_tab"));
  const expected = golden("receipts/focus_tab.tab_missing");
  assert.equal(result.failure, expected.failure);
  assert.equal(result.message, expected.message);
  assert.equal(result.dispatched, false);
  assert.equal(chrome.named("tabs.update").length, 0);
});

test("set_zoom returns the zoom the browser holds, matching the golden receipt", async () => {
  const { tabs } = setup({ browser: { tabs: [tab({ id: 12 })] } });
  const result = await execute(tabs.setZoom, goldenArgs("set_zoom"));
  const expected = golden("receipts/set_zoom.ok");
  assert.deepEqual(result.ok, expected.ok);
  assert.equal(result.dispatched, expected.dispatched);
});

test("set_zoom outside the browser's range is browser_refused after dispatch", async () => {
  const { tabs } = setup({ browser: { tabs: [tab({ id: 12 })] } });
  const result = await execute(tabs.setZoom, { tab_id: 12, zoom: 9 });
  const expected = golden("receipts/set_zoom.browser_refused");
  assert.equal(result.failure, expected.failure);
  assert.equal(result.message, expected.message);
  assert.equal(result.dispatched, expected.dispatched);
  const lost = await execute(tabs.setZoom, { tab_id: 99, zoom: 1 });
  assert.equal(lost.failure, "tab_missing");
  assert.equal(lost.dispatched, golden("receipts/set_zoom.tab_missing").dispatched);
});

test("resize_window resizes the window and lists every tab that shares it, matching the golden receipt", async () => {
  const { tabs, chrome } = setup({
    browser: { tabs: [tab({ id: 13, active: false }), tab({ id: 12, active: true }), tab({ id: 40, windowId: OTHER_WINDOW })] }
  });
  const result = await execute(tabs.resizeWindow, goldenArgs("resize_window"));
  const expected = golden("receipts/resize_window.ok");
  assert.deepEqual(result.ok, expected.ok);
  assert.equal(result.dispatched, expected.dispatched);
  assert.deepEqual(chrome.named("windows.update").map((call) => call.args), [[WINDOW, { width: 1280, height: 800 }]]);
});

// ---- close_tab --------------------------------------------------------------------------------

test("close_tab closes the tab when the person does not preserve tabs, matching the golden receipt", async () => {
  const { tabs, chrome } = setup({ browser: { tabs: [tab({ id: 12 })] } });
  chrome.store.local.set(state.PRESERVE_TABS_KEY, false);
  const result = await execute(tabs.closeTab, goldenArgs("close_tab"));
  const expected = golden("receipts/close_tab.ok");
  assert.deepEqual(result.ok, expected.ok);
  assert.equal(result.dispatched, expected.dispatched);
  assert.equal(chrome.tabList.length, 0);
});

test("the preserve-tabs choice vetoes close_tab with local_interlock and the tab stays", async () => {
  for (const stored of [undefined, true]) {
    const { tabs, chrome, hook } = setup({ browser: { tabs: [tab({ id: 12 })] } });
    if (stored !== undefined) chrome.store.local.set(state.PRESERVE_TABS_KEY, stored);
    const result = await execute(tabs.closeTab, goldenArgs("close_tab"));
    const expected = golden("receipts/close_tab.local_interlock");
    assert.equal(result.failure, expected.failure);
    assert.equal(result.message, expected.message);
    assert.equal(result.dispatched, expected.dispatched);
    assert.equal(chrome.tabList.length, 1);
    assert.deepEqual(hook.calls, []);
  }
});

test("a vetoed released close still releases the tab from Ghostlight's control", async () => {
  const { tabs, chrome, hook } = setup({ browser: { tabs: [tab({ id: 12 })] } });
  const result = await execute(tabs.closeTab, { tab_id: 12, released: true });
  assert.equal(result.failure, "local_interlock");
  assert.equal(result.dispatched, false);
  assert.deepEqual(hook.calls, [["release", 12]]);
  assert.equal(chrome.tabList.length, 1);
});

test("a tab kept after its session leaves Ghostlight's group, which goes with its last tab", async () => {
  const { tabs, chrome } = setup({
    browser: {
      tabs: [tab({ id: 12, groupId: 9 }), tab({ id: 13, groupId: 9 })],
      groups: [{ id: 9, windowId: WINDOW, title: TITLE }]
    }
  });
  chrome.store.session.set(Tabs.GROUPS_KEY, [[KEY, { groupId: 9, title: TITLE }]]);
  assert.equal((await execute(tabs.closeTab, { tab_id: 12, released: true })).failure, "local_interlock");
  assert.deepEqual(chrome.tabList.map((item) => [item.id, item.groupId]), [[12, -1], [13, 9]]);
  assert.equal((await execute(tabs.closeTab, { tab_id: 13, released: true })).failure, "local_interlock");
  assert.deepEqual(chrome.tabList.map((item) => [item.id, item.groupId]), [[12, -1], [13, -1]]);
  assert.deepEqual(chrome.groupList, []);
});

test("a kept tab in a group of the person's stays there, and a close that does not release keeps its group", async () => {
  const { tabs, chrome } = setup({
    browser: {
      tabs: [tab({ id: 12, groupId: 7 }), tab({ id: 13, groupId: 9 })],
      groups: [{ id: 7, windowId: WINDOW, title: "Reading" }, { id: 9, windowId: WINDOW, title: TITLE }]
    }
  });
  chrome.store.session.set(Tabs.GROUPS_KEY, [[KEY, { groupId: 9, title: TITLE }]]);
  await execute(tabs.closeTab, { tab_id: 12, released: true });
  await execute(tabs.closeTab, { tab_id: 13 });
  assert.deepEqual(chrome.tabList.map((item) => [item.id, item.groupId]), [[12, 7], [13, 9]]);
  assert.equal(chrome.named("tabs.ungroup").length, 0);
});

test("a kept tab the next session adopts goes back into Ghostlight's group", async () => {
  const { tabs, chrome } = setup({ browser: { tabs: [tab({ id: 1, active: true })] } });
  const opened = await execute(tabs.openTab, goldenArgs("open_tab"));
  const id = opened.ok.tab.tab_id;
  await execute(tabs.closeTab, { tab_id: id, released: true });
  assert.equal(chrome.tabList.find((item) => item.id === id).groupId, -1);
  assert.deepEqual(chrome.groupList, []);
  await execute(tabs.openTab, { ...goldenArgs("open_tab"), adopt_tab_id: id });
  const group = chrome.tabList.find((item) => item.id === id).groupId;
  assert.deepEqual(chrome.groupList.map((item) => [item.id, item.title]), [[group, TITLE]]);
});

test("close_tab on a missing tab is tab_missing with nothing dispatched", async () => {
  const { tabs, chrome } = setup();
  chrome.store.local.set(state.PRESERVE_TABS_KEY, false);
  const result = await execute(tabs.closeTab, goldenArgs("close_tab"));
  const expected = golden("receipts/close_tab.tab_missing");
  assert.equal(result.failure, expected.failure);
  assert.equal(result.message, expected.message);
  assert.equal(result.dispatched, expected.dispatched);
});
