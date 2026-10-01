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

    // Foreground grouping repairs one exact-title group per client label. Quiet requests
    // leave duplicates and human placement untouched. History can leave duplicates behind (service-worker restarts between creation and titling, pre-repair
    // releases). Chromium removes a group the moment its last tab leaves, so merging is: move
    // every stray tab of every same-title group into the canonical one and the duplicates
    // cease to exist. Best-effort per duplicate; the next assignment retries.
    async function mergeDuplicates(title, canonical) {
      const duplicates = (await chromeApi.tabGroups.query({}))
        .filter((group) => group.title === title && group.id !== canonical.id)
        .sort((left, right) => left.id - right.id);
      for (const duplicate of duplicates) {
        try {
          const strays = await chromeApi.tabs.query({ groupId: duplicate.id });
          const tabIds = strays.map((tab) => tab.id).filter((id) => id !== undefined);
          if (tabIds.length > 0) {
            await chromeApi.tabs.group({ groupId: canonical.id, tabIds });
          }
        } catch (_error) {
          // Left for the next assignment; never fail the caller over cleanup.
        }
      }
    }

    async function ownedWindow(windowId) {
      const [tabs, window] = await Promise.all([
        chromeApi.tabs.query({ windowId }), chromeApi.windows.get(windowId)
      ]);
      return !window.focused && tabs.length > 0 && tabs.every(tab => tabWorkspaces.has(tab.id));
    }

    async function canonicalGroup(title, { background = false } = {}) {
      const storedId = groups.get(title);
      if (storedId !== undefined) {
        try {
          const stored = await chromeApi.tabGroups.get(storedId);
          if (stored.title === title && (!background || await ownedWindow(stored.windowId))) {
            if (!background) await mergeDuplicates(title, stored);
            return stored;
          }
        } catch (_error) {
          // Exact-title discovery below repairs stale group ids.
        }
        groups.delete(title);
      }
      const candidates = (await chromeApi.tabGroups.query({}))
        .filter(group => group.title === title)
        .sort((left, right) => left.id - right.id);
      for (const exact of candidates) {
        if (background && !await ownedWindow(exact.windowId)) continue;
        groups.set(title, exact.id);
        if (!background) await mergeDuplicates(title, exact);
        return exact;
      }
      return undefined;
    }

    async function ghostlightWindow({ background = false } = {}) {
      const candidates = (await chromeApi.tabGroups.query({}))
        .filter(group => validTitle(group.title))
        .sort((left, right) => left.id - right.id);
      for (const group of candidates) {
        if (!background || await ownedWindow(group.windowId)) return group.windowId;
      }
      return undefined;
    }

    async function groupTab(tabId, workspace, title, group, { background = false } = {}) {
      let tab = await chromeApi.tabs.get(tabId);
      // Quiet grouping never moves a tab to satisfy a historical presentation hint.
      if (background && group && tab.windowId !== group.windowId) group = null;
      tabWorkspaces.set(tabId, workspace);
      if (group && tab.windowId !== group.windowId) {
        await chromeApi.tabs.move(tabId, { windowId: group.windowId, index: -1 });
        tab = await chromeApi.tabs.get(tabId);
      }
      let revision = 0;
      const moved = id => { if (id === tabId) revision++; };
      const focused = id => { if (id === tab.windowId) revision++; };
      if (background) {
        chromeApi.tabs.onDetached?.addListener(moved);
        chromeApi.tabs.onAttached?.addListener(moved);
        chromeApi.windows.onFocusChanged?.addListener(focused);
      }
      try {
        if (background) {
          const observedRevision = revision;
          const eligible = await ownedWindow(tab.windowId);
          const current = await chromeApi.tabs.get(tabId);
          if (!eligible || revision !== observedRevision || current.windowId !== tab.windowId) {
            // The person changed placement or took this window. Keep custody, skip grouping.
            await persist();
            return current;
          }
          tab = current;
        }
        const observedRevision = revision;
        const groupId = await chromeApi.tabs.group(group
          ? { groupId: group.id, tabIds: [tabId] }
          // Chromium otherwise creates the group in the CURRENT window, moving this tab.
          : { tabIds: [tabId], createProperties: { windowId: tab.windowId } });
        const current = await chromeApi.tabs.get(tabId);
        if (background && (revision !== observedRevision || current.windowId !== tab.windowId)) {
          throw Object.assign(new Error("The tab moved while Chromium was grouping it."), { effectUnknown: true });
        }
        groups.set(title, groupId);
        await chromeApi.tabGroups.update(groupId, {
          title,
          color: GROUP_COLOR,
          ...(background ? {} : { collapsed: false })
        });
        await persist();
        return await chromeApi.tabs.get(tabId);
      } finally {
        if (background) {
          chromeApi.tabs.onDetached?.removeListener(moved);
          chromeApi.tabs.onAttached?.removeListener(moved);
          chromeApi.windows.onFocusChanged?.removeListener(focused);
        }
      }
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

    // A trusted service request can restore an opaque association lost on extension
    // reload. This is cache repair only: no grouping, movement, or ownership inference.
    async function remember(tabId, workspace) {
      return serialized(async () => {
        if (tabWorkspaces.get(tabId) === workspace) return;
        tabWorkspaces.set(tabId, workspace);
        await persist();
      });
    }

    async function open(url, workspace, requestedTitle, onCreated, { background = false } = {}) {
      return serialized(async () => {
        const title = resolvedTitle(workspace, requestedTitle);
        const firstWorkspaceTab = !Array.from(tabWorkspaces.values()).includes(workspace);
        const group = await canonicalGroup(title, { background });
        let tab;

        if (group) {
          tab = requireCreatedTab(
            await chromeApi.tabs.create({ url, active: !background, windowId: group.windowId })
          );
          onCreated?.(tab);
          tab = await groupTab(tab.id, workspace, title, group, { background });
          if (!background && firstWorkspaceTab) await chromeApi.windows.update(group.windowId, { focused: true });
          return tab;
        }

        const windowId = await ghostlightWindow({ background });
        if (windowId !== undefined) {
          tab = requireCreatedTab(await chromeApi.tabs.create({ url, active: !background, windowId }));
          onCreated?.(tab);
          tab = await groupTab(tab.id, workspace, title, null, { background });
          if (!background && firstWorkspaceTab) await chromeApi.windows.update(windowId, { focused: true });
          return tab;
        }

        const createdWindow = await chromeApi.windows.create({
          url,
          focused: !background && firstWorkspaceTab,
          type: "normal"
        });
        const createdTabs = createdWindow?.tabs ?? (
          createdWindow?.id === undefined ? [] : await chromeApi.tabs.query({ windowId: createdWindow.id })
        );
        tab = requireCreatedTab(createdTabs.find((candidate) => candidate.id !== undefined));
        onCreated?.(tab);
        tab = await groupTab(tab.id, workspace, title, null, { background });
        return tab;
      });
    }

    async function forget(tabId) {
      return serialized(async () => {
        tabWorkspaces.delete(tabId);
        await persist();
      });
    }

    // A new authority epoch invalidates opaque custody, not physical presentation hints.
    async function forgetAll() {
      return serialized(async () => {
        tabWorkspaces.clear();
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

    // The reuse ladder (ADR-0137): an unbound tab is one no workspace owns -- released by a
    // dead workspace or opened by the person. An exact-URL match wins, then the lowest tab id
    // of the same host. Only ordinary web pages are adoption candidates.
    async function findReusable(url) {
      let target;
      try {
        target = new URL(url);
      } catch (_error) {
        return null;
      }
      if (target.protocol !== "https:" && target.protocol !== "http:") return null;
      const unbound = (await chromeApi.tabs.query({})).filter(
        (tab) => tab.id !== undefined && !tabWorkspaces.has(tab.id)
      );
      const sameHost = unbound.filter((tab) => {
        try {
          const candidate = new URL(tab.url ?? "");
          return (
            (candidate.protocol === "https:" || candidate.protocol === "http:") &&
            candidate.host === target.host
          );
        } catch (_error) {
          return false;
        }
      });
      return (
        sameHost.sort((left, right) => {
          const leftExact = left.url === url ? 0 : 1;
          const rightExact = right.url === url ? 0 : 1;
          return leftExact - rightExact || left.id - right.id;
        })[0] ?? null
      );
    }

    return Object.freeze({ restore, open, assign, remember, forget, forgetAll, findReusable, workspaceFor, titleFor, tabsFor });
  }

  return Object.freeze({ GROUP_PREFIX, GROUP_COLOR, validTitle, create });
});
