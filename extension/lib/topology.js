(function installGhostlightTopology(root, factory) {
  const api = factory();
  root.GhostlightTopology = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightTopologyApi() {
  "use strict";

  const GROUP_PREFIX = "Ghostlight - ";
  const GROUP_COLOR = "blue";

  function validTitle(value) {
    return typeof value === "string" && value.startsWith(GROUP_PREFIX) && value.length <= 120;
  }

  function create(chromeApi, storageKey) {
    const tabWorkspaces = new Map();
    const groups = new Map();
    const titles = new Map();
    let topologyQueue = Promise.resolve();

    function serialized(task) {
      const result = topologyQueue.then(task, task);
      topologyQueue = result.catch(() => {});
      return result;
    }

    function resolvedTitle(workspace, requestedTitle) {
      const title = validTitle(requestedTitle)
        ? requestedTitle
        : titles.get(workspace) || `${GROUP_PREFIX}MCP client`;
      titles.set(workspace, title);
      return title;
    }

    function requireCreatedTab(tab) {
      if (tab?.id !== undefined) return tab;
      throw Object.assign(
        new Error("Ghostlight could not observe the tab created by Chromium."),
        { effectUnknown: true }
      );
    }

    async function restore() {
      const stored = (await chromeApi.storage.session.get(storageKey))[storageKey];
      for (const [tabId, workspace] of stored?.tabs ?? []) {
        try {
          await chromeApi.tabs.get(Number(tabId));
          tabWorkspaces.set(Number(tabId), workspace);
        } catch (_error) {
          // Closed tabs are deliberately forgotten.
        }
      }
      for (const [title, groupId] of stored?.groups ?? []) {
        if (!validTitle(title)) continue;
        try {
          const group = await chromeApi.tabGroups.get(Number(groupId));
          if (group.title === title) groups.set(title, Number(groupId));
        } catch (_error) {
          // Group ids are browser-session hints and may be stale.
        }
      }
      for (const [workspace, title] of stored?.titles ?? []) {
        if (typeof workspace === "string" && validTitle(title)) titles.set(workspace, title);
      }
      await persist();
    }

    async function persist() {
      await chromeApi.storage.session.set({
        [storageKey]: {
          tabs: Array.from(tabWorkspaces.entries()),
          groups: Array.from(groups.entries()),
          titles: Array.from(titles.entries())
        }
      });
    }

    async function canonicalGroup(title) {
      const storedId = groups.get(title);
      if (storedId !== undefined) {
        try {
          const stored = await chromeApi.tabGroups.get(storedId);
          if (stored.title === title) return stored;
        } catch (_error) {
          // Exact-title discovery below repairs stale group ids.
        }
        groups.delete(title);
      }
      const exact = (await chromeApi.tabGroups.query({}))
        .filter((group) => group.title === title)
        .sort((left, right) => left.id - right.id)[0];
      if (exact) groups.set(title, exact.id);
      return exact;
    }

    async function ghostlightWindow() {
      return (await chromeApi.tabGroups.query({}))
        .filter((group) => validTitle(group.title))
        .sort((left, right) => left.id - right.id)[0]?.windowId;
    }

    async function groupTab(tabId, workspace, title, group) {
      tabWorkspaces.set(tabId, workspace);
      let tab = await chromeApi.tabs.get(tabId);
      if (group && tab.windowId !== group.windowId) {
        await chromeApi.tabs.move(tabId, { windowId: group.windowId, index: -1 });
        tab = await chromeApi.tabs.get(tabId);
      }
      const groupId = await chromeApi.tabs.group(
        group ? { groupId: group.id, tabIds: [tabId] } : { tabIds: [tabId] }
      );
      groups.set(title, groupId);
      await chromeApi.tabGroups.update(groupId, {
        title,
        color: GROUP_COLOR,
        collapsed: false
      });
      await persist();
      return tab;
    }

    async function assignInternal(tabId, workspace, requestedTitle) {
      const title = resolvedTitle(workspace, requestedTitle);
      const group = await canonicalGroup(title);
      await groupTab(tabId, workspace, title, group);
      return workspace;
    }

    async function assign(tabId, workspace, requestedTitle) {
      return serialized(() => assignInternal(tabId, workspace, requestedTitle));
    }

    async function open(url, workspace, requestedTitle, onCreated) {
      return serialized(async () => {
        const title = resolvedTitle(workspace, requestedTitle);
        const firstWorkspaceTab = !Array.from(tabWorkspaces.values()).includes(workspace);
        const group = await canonicalGroup(title);
        let tab;

        if (group) {
          tab = requireCreatedTab(
            await chromeApi.tabs.create({ url, active: true, windowId: group.windowId })
          );
          onCreated?.(tab);
          await groupTab(tab.id, workspace, title, group);
          if (firstWorkspaceTab) await chromeApi.windows.update(group.windowId, { focused: true });
          return tab;
        }

        const windowId = await ghostlightWindow();
        if (windowId !== undefined) {
          tab = requireCreatedTab(await chromeApi.tabs.create({ url, active: true, windowId }));
          onCreated?.(tab);
          await groupTab(tab.id, workspace, title, null);
          if (firstWorkspaceTab) await chromeApi.windows.update(windowId, { focused: true });
          return tab;
        }

        const createdWindow = await chromeApi.windows.create({
          url,
          focused: firstWorkspaceTab,
          type: "normal"
        });
        const createdTabs = createdWindow?.tabs ?? (
          createdWindow?.id === undefined ? [] : await chromeApi.tabs.query({ windowId: createdWindow.id })
        );
        tab = requireCreatedTab(createdTabs.find((candidate) => candidate.id !== undefined));
        onCreated?.(tab);
        await groupTab(tab.id, workspace, title, null);
        return tab;
      });
    }

    async function adopt(openerTabId, tabId) {
      const workspace = tabWorkspaces.get(openerTabId);
      if (!workspace) return null;
      await assign(tabId, workspace, titles.get(workspace));
      return workspace;
    }

    async function reattach(tabId) {
      const workspace = tabWorkspaces.get(tabId);
      if (workspace) await assign(tabId, workspace, titles.get(workspace));
    }

    async function forget(tabId) {
      return serialized(async () => {
        tabWorkspaces.delete(tabId);
        await persist();
      });
    }

    function workspaceFor(tabId) {
      return tabWorkspaces.get(tabId) ?? null;
    }

    function titleFor(workspace) {
      return titles.get(workspace) ?? null;
    }

    function tabsFor(workspace) {
      return Array.from(tabWorkspaces.entries())
        .filter(([, owner]) => owner === workspace)
        .map(([tabId]) => tabId);
    }

    return Object.freeze({ restore, open, assign, adopt, reattach, forget, workspaceFor, titleFor, tabsFor });
  }

  return Object.freeze({ GROUP_PREFIX, GROUP_COLOR, validTitle, create });
});
