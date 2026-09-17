"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");

test("Firefox extension tab operations map cleanly to PhysicalTab and outcomes", async () => {
  const tabsList = [
    { id: 101, windowId: 1, url: "https://example.com", title: "Example Domain", active: true, status: "complete" },
    { id: 102, windowId: 1, url: "https://ghostlight.org", title: "Ghostlight", active: false, status: "loading" }
  ];

  let activeTabId = 101;
  let closedTabId = null;
  let navigated = null;

  globalThis.browser = {
    tabs: {
      async query() {
        return tabsList;
      },
      async get(tabId) {
        return tabsList.find((t) => t.id === tabId) || { id: tabId, windowId: 1 };
      },
      async update(tabId, updateInfo) {
        if (updateInfo.active) activeTabId = tabId;
        if (updateInfo.url) navigated = { tabId, url: updateInfo.url };
        const found = tabsList.find((t) => t.id === tabId);
        if (found) Object.assign(found, updateInfo);
        return { id: tabId, windowId: 1, ...updateInfo };
      },
      async create(createProperties) {
        const newTab = { id: 103, windowId: 1, ...createProperties };
        tabsList.push(newTab);
        return newTab;
      },
      async remove(tabId) {
        closedTabId = tabId;
      }
    },
    windows: {
      async getCurrent() {
        return { id: 1, focused: true };
      },
      async update() {
        return { id: 1, focused: true };
      },
      onFocusChanged: {
        addListener() {}
      }
    }
  };

  const bg = require("../background.js");

  // Test listing tabs
  const listResult = await bg.handleListTabs();
  assert.equal(listResult.outcome, "tabs");
  assert.equal(listResult.tabs.length, 2);
  assert.deepEqual(listResult.tabs[0], {
    tab_id: 101,
    title: "Example Domain",
    url: "https://example.com",
    active: true,
    readiness: "complete"
  });
  assert.equal(listResult.tabs[1].readiness, "loading");

  // Test opening a tab
  const openResult = await bg.handleOpenTab("https://newsite.org");
  assert.equal(openResult.outcome, "tab_opened");
  assert.equal(openResult.tab.tab_id, 103);
  assert.equal(openResult.tab.url, "https://newsite.org");
  assert.equal(openResult.reused, false);

  // Test focusing a tab
  const focusResult = await bg.handleFocusTab(102);
  assert.equal(focusResult.outcome, "tab_focused");
  assert.equal(focusResult.tab_id, 102);
  assert.equal(focusResult.active, true);
  assert.equal(focusResult.window_focused, true);
  assert.equal(activeTabId, 102);

  // Test navigating a tab
  const navResult = await bg.handleNavigateTab(101, "https://updated.org");
  assert.equal(navResult.outcome, "navigated");
  assert.equal(navResult.tab.tab_id, 101);
  assert.equal(navResult.tab.url, "https://updated.org");
  assert.deepEqual(navigated, { tabId: 101, url: "https://updated.org" });

  // Test closing a tab
  const closeResult = await bg.handleCloseTab(102);
  assert.equal(closeResult.outcome, "tab_closed");
  assert.equal(closeResult.tab_id, 102);
  assert.equal(closedTabId, 102);
});
