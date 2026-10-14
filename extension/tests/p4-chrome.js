"use strict";

// A scripted browser for the protocol 4 tab, capture and notice tests: chrome.tabs, windows,
// tabGroups and storage with just enough Chromium behaviour (group removal when empty, windows
// that hold tabs) and a call log for assertions. Also loads the golden fixtures and drives a
// handler the way the dispatcher does.

const fs = require("node:fs");
const path = require("node:path");

const GOLDEN = path.join(__dirname, "..", "..", "tests", "golden", "protocol4");

/** Reads one golden fixture, for example `golden("receipts/navigate.ok")`. */
function golden(relative) {
  return JSON.parse(fs.readFileSync(path.join(GOLDEN, `${relative}.json`), "utf8"));
}

/** The args of a golden request fixture. */
function goldenArgs(name) {
  return golden(`requests/${name}`).primitive.args;
}

/** An effect watch like the dispatcher's. */
function effects() {
  return { watched: [], dispatched: false, watch(tabId) { this.watched.push(tabId); } };
}

/**
 * Runs a handler as the dispatcher does: validate, then run with a fresh effect watch. Resolves
 * `{ ok, dispatched, watched }` or `{ failure, message, dispatched, watched }`.
 */
async function execute(spec, args, correlation = "p4_test") {
  const fx = effects();
  try {
    spec.validate(args);
    const ok = await spec.run(args, fx, correlation);
    return { ok, dispatched: fx.dispatched, watched: fx.watched };
  } catch (error) {
    return { failure: error.code ?? "other", message: error.message, dispatched: fx.dispatched, watched: fx.watched };
  }
}

function missing(id) {
  return new Error(`No tab with id: ${id}.`);
}

/**
 * Builds the browser. `tabs` are seeded Chrome tab objects (`id`, `windowId`, `url`, `title`,
 * `status`, `active`, `groupId`, `openerTabId`); `groups` are `{ id, windowId, title }`;
 * `focusedWindows` lists the window ids that have focus. `readyAfter` is how many reads of a
 * newly created or navigated tab report `loading` before `complete` (default 0).
 */
function fakeBrowser({ tabs = [], groups = [], focusedWindows = [], readyAfter = 0 } = {}) {
  const calls = [];
  const tabList = tabs.map((tab) => ({ status: "complete", title: "", active: false, groupId: -1, ...tab }));
  const groupList = groups.map((group) => ({ collapsed: false, color: "grey", ...group }));
  const windowList = new Map();
  const zoom = new Map();
  const loading = new Map();
  const store = { session: new Map(), local: new Map() };
  let nextTab = 100;
  let nextWindow = 50;
  let nextGroup = 500;

  for (const tab of tabList) windowList.set(tab.windowId, { id: tab.windowId, focused: focusedWindows.includes(tab.windowId) });
  for (const id of focusedWindows) windowList.set(id, { id, focused: true });
  for (const group of groupList) if (!windowList.has(group.windowId)) windowList.set(group.windowId, { id: group.windowId, focused: false });

  const log = (name, ...args) => calls.push({ name, args });
  const find = (id) => tabList.find((tab) => tab.id === id);
  const copy = (value) => (value === undefined ? value : { ...value });

  function startLoading(tab) {
    if (readyAfter > 0) {
      tab.status = "loading";
      loading.set(tab.id, readyAfter);
    }
  }

  function pruneGroups() {
    for (let index = groupList.length - 1; index >= 0; index -= 1) {
      if (!tabList.some((tab) => tab.groupId === groupList[index].id)) groupList.splice(index, 1);
    }
  }

  function newTab(url, windowId, active) {
    nextTab += 1;
    const tab = { id: nextTab, windowId, url, title: `Title ${nextTab}`, status: "complete", active, groupId: -1 };
    tabList.push(tab);
    startLoading(tab);
    return tab;
  }

  function storageArea(area) {
    return {
      async get(key) {
        const keys = typeof key === "string" ? [key] : key;
        return Object.fromEntries(keys.filter((name) => area.has(name)).map((name) => [name, area.get(name)]));
      },
      async set(values) { for (const [name, value] of Object.entries(values)) area.set(name, value); }
    };
  }

  const browser = {
    calls,
    tabList,
    groupList,
    windowList,
    store,
    named(name) { return calls.filter((call) => call.name === name); },
    tabs: {
      async get(id) {
        log("tabs.get", id);
        const tab = find(id);
        if (!tab) throw missing(id);
        if (loading.has(id)) {
          const left = loading.get(id) - 1;
          if (left <= 0) {
            loading.delete(id);
            tab.status = "complete";
          } else {
            loading.set(id, left);
          }
        }
        return copy(tab);
      },
      async query(filter = {}) {
        log("tabs.query", filter);
        return tabList
          .filter((tab) => (filter.windowId === undefined || tab.windowId === filter.windowId)
            && (filter.groupId === undefined || tab.groupId === filter.groupId)
            && (filter.active === undefined || tab.active === filter.active))
          .map(copy);
      },
      async create(properties) {
        log("tabs.create", properties);
        return copy(newTab(properties.url, properties.windowId, properties.active !== false));
      },
      async update(id, properties) {
        log("tabs.update", id, properties);
        const tab = find(id);
        if (!tab) throw missing(id);
        if (properties.url !== undefined) {
          tab.url = properties.url;
          startLoading(tab);
        }
        if (properties.active !== undefined) {
          tab.active = properties.active;
          if (properties.active) for (const other of tabList) if (other !== tab && other.windowId === tab.windowId) other.active = false;
        }
        return copy(tab);
      },
      async move(id, properties) {
        log("tabs.move", id, properties);
        const tab = find(id);
        if (!tab) throw missing(id);
        tab.windowId = properties.windowId;
        tab.groupId = -1;
        pruneGroups();
        return copy(tab);
      },
      async group({ groupId, tabIds }) {
        log("tabs.group", { groupId, tabIds });
        let id = groupId;
        if (id === undefined) {
          nextGroup += 1;
          id = nextGroup;
          groupList.push({ id, windowId: find(tabIds[0]).windowId, title: "", collapsed: false, color: "grey" });
        }
        for (const tabId of tabIds) find(tabId).groupId = id;
        pruneGroups();
        return id;
      },
      async remove(id) {
        log("tabs.remove", id);
        const index = tabList.findIndex((tab) => tab.id === id);
        if (index < 0) throw missing(id);
        tabList.splice(index, 1);
        pruneGroups();
      },
      async goBack(id) { log("tabs.goBack", id); if (!find(id)) throw missing(id); startLoading(find(id)); },
      async goForward(id) { log("tabs.goForward", id); if (!find(id)) throw missing(id); startLoading(find(id)); },
      async reload(id, properties) { log("tabs.reload", id, properties); if (!find(id)) throw missing(id); startLoading(find(id)); },
      async setZoom(id, level) {
        log("tabs.setZoom", id, level);
        if (level !== 0 && (level < 0.25 || level > 5)) throw new Error("Zoom level is outside the range the browser allows.");
        zoom.set(id, level === 0 ? 1 : level);
      },
      async getZoom(id) { return zoom.get(id) ?? 1; }
    },
    tabGroups: {
      async get(id) {
        const group = groupList.find((candidate) => candidate.id === id);
        if (!group) throw new Error(`No group with id: ${id}.`);
        return { ...group };
      },
      async query() { return groupList.map(copy); },
      async update(id, properties) {
        log("tabGroups.update", id, properties);
        Object.assign(groupList.find((group) => group.id === id), properties);
      }
    },
    windows: {
      async create(properties) {
        log("windows.create", properties);
        nextWindow += 1;
        windowList.set(nextWindow, { id: nextWindow, focused: properties.focused !== false });
        const tab = newTab(properties.url, nextWindow, true);
        return { id: nextWindow, tabs: [copy(tab)] };
      },
      async update(id, properties) {
        log("windows.update", id, properties);
        const window = windowList.get(id) ?? { id };
        windowList.set(id, window);
        Object.assign(window, properties);
        return { ...window };
      },
      async get(id) {
        const window = windowList.get(id);
        if (!window) throw new Error(`No window with id: ${id}.`);
        return { ...window };
      }
    },
    storage: { session: storageArea(store.session), local: storageArea(store.local) }
  };
  return browser;
}

module.exports = { golden, goldenArgs, effects, execute, fakeBrowser };
