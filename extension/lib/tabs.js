// Ghostlight -- the tab primitives of adapter protocol 4.
(function installGhostlightTabs(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), sessions: require("./sessions.js"), shared: require("./shared.js"), state: require("./state.js"),
      args: require("./arguments.js") }
    : { protocol: root.GhostlightProtocol, sessions: root.GhostlightSessions, shared: root.GhostlightShared, state: root.GhostlightState,
      args: root.GhostlightArguments });
  root.GhostlightTabs = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightTabsApi(deps) {
  "use strict";

  const { protocol, sessions, shared, state, args: check } = deps;
  const { CDP } = sessions;
  const { invalid, isObject, requireTabId, requireText, requireInteger, requireArgs } = check;

  // Tabs, windows and zoom: list_tabs, open_tab, navigate, focus_tab, close_tab, set_zoom and
  // resize_window. The service chooses which tab to touch and which tab to adopt; this module only
  // does what a request names. A tab opened or adopted for the service joins a tab group found by the
  // service's opaque group key (a title match repairs the mapping after an extension reload), a tab
  // the person keeps after its session ends leaves that group, and every handler is a
  // `{ validate, run, addresses, tabOf }` entry for the primitive dispatcher.

  const GROUPS_KEY = "ghostlight.groups";
  const GROUP_COLOR = "blue";
  const NO_GROUP = -1;
  const GROUP_KEY_CHARS = 200;
  const MESSAGE_CHARS = 500;
  const READY_POLL_MS = 100;
  const HISTORY_SETTLE_MS = 75;
  const BLANK_URL = "about:blank";
  const STATUS_LOADING = "loading";
  const STATUS_COMPLETE = "complete";
  const BEFORE_UNLOAD = "beforeunload";
  const TARGET = Object.freeze({ URL: "url", BACK: "back", FORWARD: "forward", RELOAD: "reload" });
  const FAILURE = protocol.RECEIPT_FAILURE;
  // What back and forward say when the tab's history has no entry that way. Advisory, as every
  // receipt message is: the service words the refusal itself.
  const NO_ENTRY_MESSAGE = Object.freeze({
    back: "The tab has no earlier page in its history.",
    forward: "The tab has no later page in its history."
  });
  const INTERLOCK_MESSAGE = "Ghostlight is preserving controlled tabs by local browser choice.";
  const WIDTH_LIMITS = protocol.vocabulary.limits.resize_width;
  const HEIGHT_LIMITS = protocol.vocabulary.limits.resize_height;

  function requireOptionalBoolean(value, name) {
    if (value !== undefined && typeof value !== "boolean") throw invalid(`${name} must be true or false.`);
  }

  /** The `Tab` outcome object of a Chrome tab. */
  function tabFact(tab) {
    return {
      tab_id: tab.id,
      title: shared.bounded(tab.title, protocol.TITLE_CHARS),
      url: shared.bounded(tab.url || tab.pendingUrl || BLANK_URL, protocol.URL_CHARS),
      active: Boolean(tab.active),
      readiness: shared.readinessForStatus(tab.status)
    };
  }

  function sortedUnique(ids) {
    return Array.from(new Set(ids)).sort((left, right) => left - right);
  }

  function byGroupId(left, right) {
    return left.id - right.id;
  }

  /**
   * Creates the tab primitives over the Chrome tabs, windows, tabGroups and storage APIs.
   *
   * `lifecycle` is a GhostlightSessions instance, used for the debugger lease of a navigation that
   * discards beforeunload dialogs. `controlled` is the worker's controlled-tab hook:
   * `manage(tabId)` and `release(tabId)`. `isCancelled(correlation)` reports a
   * cancel frame; `now` and `sleep` are the clock.
   */
  function create({
    chromeApi = globalThis.chrome,
    lifecycle,
    controlled,
    isCancelled = () => false,
    now = () => Date.now(),
    sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  } = {}) {
    if (!chromeApi?.tabs || !chromeApi?.windows || !chromeApi?.tabGroups || !chromeApi?.storage) {
      throw new TypeError("the tab primitives require the Chrome tabs, windows, tabGroups and storage APIs");
    }
    if (!lifecycle?.cdp || !controlled?.manage || !controlled?.release) {
      throw new TypeError("the tab primitives require debugger sessions and the controlled-tab hook");
    }

    const groups = new Map();
    const beforeUnloadTabs = new Map();
    let loaded = null;
    let queue = Promise.resolve();

    // ---- groups -------------------------------------------------------------------------------

    // A request the service cancelled ends before its next effect.
    function refuseCancelled(correlation) {
      if (isCancelled(correlation)) throw protocol.failure(FAILURE.CANCELLED, "The request was cancelled.");
    }

    // Group changes run one at a time, so two opens for one key never create two groups. A request
    // the service cancelled while it waited for its turn ends when the turn comes, before it touches
    // the browser.
    function serialized(task, correlation) {
      const turn = () => {
        refuseCancelled(correlation);
        return task();
      };
      const result = queue.then(turn, turn);
      queue = result.catch(() => {});
      return result;
    }

    function loadGroups() {
      loaded ??= (async () => {
        try {
          const stored = (await chromeApi.storage.session.get(GROUPS_KEY))[GROUPS_KEY];
          for (const [key, entry] of Array.isArray(stored) ? stored : []) {
            if (typeof key === "string" && Number.isSafeInteger(entry?.groupId)) groups.set(key, { groupId: entry.groupId, title: String(entry.title ?? "") });
          }
        } catch (_error) { /* without stored groups the title search finds them again */ }
      })();
      return loaded;
    }

    async function persistGroups() {
      await chromeApi.storage.session.set({ [GROUPS_KEY]: Array.from(groups) });
    }

    // One group per title is the invariant, and history can leave duplicates behind (a restart
    // between creating and titling a group). Chromium removes a group when its last tab leaves, so
    // merging is: move every tab of each same-title duplicate into the canonical group. A group an
    // earlier release titled for the same agent is a duplicate too.
    async function mergeDuplicates(title, canonical) {
      const titles = [title, ...protocol.legacyGroupTitles(title)];
      const duplicates = (await chromeApi.tabGroups.query({}))
        .filter((group) => titles.includes(group.title) && group.id !== canonical.id)
        .sort(byGroupId);
      for (const duplicate of duplicates) {
        try {
          const strays = await chromeApi.tabs.query({ groupId: duplicate.id });
          const tabIds = strays.map((tab) => tab.id).filter((id) => id !== undefined);
          if (tabIds.length > 0) await chromeApi.tabs.group({ groupId: canonical.id, tabIds });
        } catch (_error) { /* left for the next open; cleanup never fails the caller */ }
      }
    }

    // The lowest-id group an earlier release titled for the agent the current title `title` names,
    // retitled to `title`, so the person keeps one group per agent across the update.
    async function renamedLegacyGroup(live, title) {
      const legacy = protocol.legacyGroupTitles(title);
      const group = live.filter((candidate) => legacy.includes(candidate.title)).sort(byGroupId)[0];
      if (!group) return undefined;
      await chromeApi.tabGroups.update(group.id, { title });
      return { ...group, title };
    }

    // The group for a key: the stored group when it still carries the title, else the lowest-id group
    // with exactly that title (which repairs the mapping after an extension reload), else a group an
    // earlier release titled for the same agent, retitled.
    async function canonicalGroup(key, title) {
      const entry = groups.get(key);
      if (entry) {
        try {
          const stored = await chromeApi.tabGroups.get(entry.groupId);
          if (stored.title === title) {
            await mergeDuplicates(title, stored);
            return stored;
          }
        } catch (_error) { /* a stale id falls through to the title search */ }
        groups.delete(key);
      }
      const live = await chromeApi.tabGroups.query({});
      const found = live.filter((group) => group.title === title).sort(byGroupId)[0] ?? await renamedLegacyGroup(live, title);
      if (!found) return undefined;
      groups.set(key, { groupId: found.id, title });
      await mergeDuplicates(title, found);
      return found;
    }

    // The window of any group this adapter keeps, so a new group lands beside the others.
    async function ownGroupWindow() {
      for (const [key, entry] of Array.from(groups).sort((left, right) => left[1].groupId - right[1].groupId)) {
        try {
          return (await chromeApi.tabGroups.get(entry.groupId)).windowId;
        } catch (_error) {
          groups.delete(key);
        }
      }
      return undefined;
    }

    // Moves the tab into the group's window when it is elsewhere, groups it (a new group when there
    // is none), and titles the group. Returns the tab as it is afterwards.
    async function placeInGroup(tabId, key, title, group) {
      let tab = await chromeApi.tabs.get(tabId);
      if (group && tab.windowId !== group.windowId) {
        await chromeApi.tabs.move(tabId, { windowId: group.windowId, index: -1 });
        tab = await chromeApi.tabs.get(tabId);
      }
      const groupId = await chromeApi.tabs.group(group ? { groupId: group.id, tabIds: [tabId] } : { tabIds: [tabId] });
      await chromeApi.tabGroups.update(groupId, { title, color: GROUP_COLOR, collapsed: false });
      groups.set(key, { groupId, title });
      await persistGroups();
      return tab;
    }

    function requireCreated(tab) {
      if (tab?.id === undefined) throw protocol.failure(FAILURE.OTHER, "Ghostlight could not observe the tab created by Chromium.");
      return tab;
    }

    async function createGroupedTab(args, fx, correlation) {
      return serialized(async () => {
        await loadGroups();
        const group = await canonicalGroup(args.group_key, args.group_title);
        const windowId = group?.windowId ?? await ownGroupWindow();
        fx.dispatched = true;
        let tab;
        if (windowId !== undefined) {
          tab = requireCreated(await chromeApi.tabs.create({ url: args.url, active: true, windowId }));
        } else {
          const created = await chromeApi.windows.create({ url: args.url, focused: args.focus_window, type: "normal" });
          const opened = created?.tabs ?? (created?.id === undefined ? [] : await chromeApi.tabs.query({ windowId: created.id }));
          tab = requireCreated(opened.find((candidate) => candidate.id !== undefined));
        }
        fx.watch(tab.id);
        await placeInGroup(tab.id, args.group_key, args.group_title, group);
        if (args.focus_window && windowId !== undefined) await chromeApi.windows.update(tab.windowId, { focused: true });
        return tab;
      }, correlation);
    }

    function inOwnGroup(tab) {
      return Array.from(groups.values()).some((entry) => entry.groupId === tab.groupId);
    }

    // A tab the person put in a group of their own stays where they put it; any other tab joins the
    // key's group.
    async function adoptGroupedTab(tabId, args, fx, correlation) {
      return serialized(async () => {
        fx.dispatched = true;
        await loadGroups();
        const group = await canonicalGroup(args.group_key, args.group_title);
        const tab = await chromeApi.tabs.get(tabId);
        if (Number.isInteger(tab.groupId) && tab.groupId !== NO_GROUP && tab.groupId !== group?.id && !inOwnGroup(tab)) return tab;
        return placeInGroup(tabId, args.group_key, args.group_title, group);
      }, correlation);
    }

    // Takes a kept tab out of the adapter's group; a tab the person put in a group of their own stays.
    // Chromium deletes a group emptied this way but saves a group that is still open when its window
    // closes or the browser exits, and no extension API can find, reopen or delete a saved group. So
    // a group with no live session's tab is gone before Chromium can save it, and the next session's
    // group is the only one with its title. Adopting the tab again puts it back.
    async function leaveOwnGroup(tabId) {
      return serialized(async () => {
        try {
          await loadGroups();
          if (inOwnGroup(await chromeApi.tabs.get(tabId))) await chromeApi.tabs.ungroup(tabId);
        } catch (_error) { /* a tab that cannot leave keeps its group; the close answers as it would */ }
      });
    }

    // ---- shared helpers -----------------------------------------------------------------------

    function requireTab(tabId) {
      return check.requireTab(chromeApi.tabs, tabId);
    }

    // Polls the tab until it is complete, or settled with a url, or the timeout passes. A cancel
    // frame ends the wait; the effect already happened, so the caller's dispatched flag stays true.
    async function waitForReady(tabId, timeoutMs, correlation) {
      const deadline = now() + timeoutMs;
      for (;;) {
        refuseCancelled(correlation);
        const tab = await requireTab(tabId);
        if (tab.status === STATUS_COMPLETE) return tab;
        if (tab.status !== STATUS_LOADING && (tab.url || tab.pendingUrl)) return tab;
        if (now() >= deadline) throw protocol.failure(FAILURE.TIMEOUT, `The tab did not become ready in ${timeoutMs} ms.`);
        await sleep(READY_POLL_MS);
      }
    }

    async function preserveTabsEnabled() {
      const stored = await chromeApi.storage.local.get(state.PRESERVE_TABS_KEY);
      return stored[state.PRESERVE_TABS_KEY] !== false;
    }

    function handler({ validate, run, addresses }) {
      return Object.freeze({ validate, run, addresses: () => addresses, tabOf: (args) => args.tab_id });
    }

    // ---- primitives ---------------------------------------------------------------------------

    const listTabs = handler({
      addresses: false,
      validate(args) {
        requireArgs(args);
        if (args.tab_ids === undefined || args.tab_ids === null) return;
        if (!Array.isArray(args.tab_ids)) throw invalid("tab_ids must be a list of tab ids.");
        for (const id of args.tab_ids) requireTabId(id, "tab_ids entry");
      },
      /** Resolves `Tabs`: the named tabs and the tabs they opened, or every tab. Reads only: no debugger, no custody. */
      async run(args) {
        const named = Array.isArray(args.tab_ids) ? new Set(args.tab_ids) : null;
        const all = await chromeApi.tabs.query({});
        return {
          tabs: all
            .filter((tab) => !named || named.has(tab.id) || named.has(tab.openerTabId))
            .map(tabFact)
        };
      }
    });

    const openTab = handler({
      addresses: false,
      validate(args) {
        requireArgs(args);
        if (args.adopt_tab_id !== undefined) requireTabId(args.adopt_tab_id, "adopt_tab_id");
        if (args.adopt_tab_id === undefined && args.url === undefined) throw invalid("open_tab requires a url unless it adopts a tab.");
        if (args.url !== undefined) requireText(args.url, "url", protocol.URL_CHARS);
        requireText(args.group_key, "group_key", GROUP_KEY_CHARS);
        requireText(args.group_title, "group_title", protocol.GROUP_TITLE_CHARS);
        if (typeof args.focus_window !== "boolean") throw invalid("focus_window must be true or false.");
        requireInteger(args.ready_timeout_ms, "ready_timeout_ms");
      },
      /**
       * Resolves `TabOpened`. Adopts the named tab (moving it into the group, unless the person put it
       * in a group of their own) or creates a tab in the group's window, then marks the tab
       * controlled and waits for it to be ready.
       */
      async run(args, fx, correlation) {
        let tab;
        if (args.adopt_tab_id !== undefined) {
          const existing = await requireTab(args.adopt_tab_id);
          fx.watch(existing.id);
          const placed = await adoptGroupedTab(existing.id, args, fx, correlation);
          await chromeApi.tabs.update(existing.id, args.url ? { url: args.url, active: true } : { active: true });
          if (args.focus_window) await chromeApi.windows.update(placed.windowId, { focused: true });
          tab = existing;
        } else {
          tab = await createGroupedTab(args, fx, correlation);
        }
        await controlled.manage(tab.id);
        const ready = await waitForReady(tab.id, args.ready_timeout_ms, correlation);
        return { tab: tabFact(ready), adopted: args.adopt_tab_id !== undefined };
      }
    });

    /**
     * Goes to the tab's previous or next history entry exactly, as the page's own history.back()
     * and history.forward() do. The browser's back button, and tabs.goBack with it, skip an entry
     * whose page the person never interacted with (Chrome's history manipulation intervention),
     * which stranded an agent that moved between pages with navigate alone. With no entry that way
     * nothing is sent, and the receipt says no_history_entry.
     */
    async function traverseHistory(tabId, direction, fx) {
      await lifecycle.withLease(tabId, async () => {
        const history = await lifecycle.cdp({ tabId }, CDP.PAGE_GET_NAVIGATION_HISTORY);
        const step = direction === TARGET.BACK ? -1 : 1;
        const entry = Array.isArray(history?.entries) && Number.isSafeInteger(history.currentIndex)
          ? history.entries[history.currentIndex + step]
          : undefined;
        if (!Number.isSafeInteger(entry?.id)) throw protocol.failure(FAILURE.NO_HISTORY_ENTRY, NO_ENTRY_MESSAGE[direction]);
        fx.dispatched = true;
        await lifecycle.cdp({ tabId }, CDP.PAGE_NAVIGATE_TO_HISTORY_ENTRY, { entryId: entry.id });
      });
    }

    const navigate = handler({
      addresses: true,
      validate(args) {
        requireArgs(args);
        requireTabId(args.tab_id);
        if (!isObject(args.to) || !protocol.isKnown("navigation_targets", args.to.to)) throw invalid("to must name a navigation target.");
        if (args.to.to === TARGET.URL) requireText(args.to.url, "to.url", protocol.URL_CHARS);
        requireOptionalBoolean(args.to.bypass_cache, "to.bypass_cache");
        requireOptionalBoolean(args.discard_beforeunload, "discard_beforeunload");
        requireInteger(args.ready_timeout_ms, "ready_timeout_ms");
      },
      /**
       * Resolves `Navigated`. Loads a url, or goes back, forward or reloads, then waits for the tab.
       * With `discard_beforeunload` the tab is attached for the duration so its beforeunload dialog
       * is accepted instead of blocking.
       */
      async run(args, fx, correlation) {
        await requireTab(args.tab_id);
        fx.watch(args.tab_id);
        const discard = args.discard_beforeunload === true;
        if (discard) {
          await lifecycle.acquire(args.tab_id);
          beforeUnloadTabs.set(args.tab_id, (beforeUnloadTabs.get(args.tab_id) ?? 0) + 1);
        }
        try {
          if (args.to.to === TARGET.URL) {
            fx.dispatched = true;
            await chromeApi.tabs.update(args.tab_id, { url: args.to.url, active: true });
          } else if (args.to.to === TARGET.BACK || args.to.to === TARGET.FORWARD) {
            await traverseHistory(args.tab_id, args.to.to, fx);
            await sleep(HISTORY_SETTLE_MS);
          } else {
            fx.dispatched = true;
            await chromeApi.tabs.reload(args.tab_id, { bypassCache: Boolean(args.to.bypass_cache) });
            await sleep(HISTORY_SETTLE_MS);
          }
          return { tab: tabFact(await waitForReady(args.tab_id, args.ready_timeout_ms, correlation)) };
        } finally {
          if (discard) {
            const remaining = (beforeUnloadTabs.get(args.tab_id) ?? 1) - 1;
            if (remaining > 0) beforeUnloadTabs.set(args.tab_id, remaining);
            else beforeUnloadTabs.delete(args.tab_id);
            await lifecycle.release(args.tab_id);
          }
        }
      }
    });

    const focusTab = handler({
      addresses: true,
      validate(args) {
        requireArgs(args);
        requireTabId(args.tab_id);
      },
      /** Resolves `TabFocused`: makes the tab active in its window and focuses the window. */
      async run(args, fx) {
        await requireTab(args.tab_id);
        fx.watch(args.tab_id);
        fx.dispatched = true;
        const tab = await chromeApi.tabs.update(args.tab_id, { active: true });
        const window = await chromeApi.windows.update(tab.windowId, { focused: true });
        return { tab_id: args.tab_id, active: Boolean(tab.active), window_focused: Boolean(window.focused) };
      }
    });

    const closeTab = handler({
      addresses: false,
      validate(args) {
        requireArgs(args);
        requireTabId(args.tab_id);
        requireOptionalBoolean(args.released, "released");
      },
      /**
       * Resolves `TabClosed`. The person's preserve-tabs choice vetoes the close with
       * `local_interlock`; a released close that is vetoed still ends Ghostlight's control of the tab
       * and takes it out of Ghostlight's group.
       */
      async run(args, fx) {
        if (await preserveTabsEnabled()) {
          if (args.released === true) {
            await controlled.release(args.tab_id);
            await leaveOwnGroup(args.tab_id);
          }
          throw protocol.failure(FAILURE.LOCAL_INTERLOCK, INTERLOCK_MESSAGE);
        }
        await requireTab(args.tab_id);
        fx.dispatched = true;
        await chromeApi.tabs.remove(args.tab_id);
        return { tab_id: args.tab_id };
      }
    });

    const setZoom = handler({
      addresses: true,
      validate(args) {
        requireArgs(args);
        requireTabId(args.tab_id);
        if (typeof args.zoom !== "number" || !Number.isFinite(args.zoom) || args.zoom < 0) throw invalid("zoom must be a number of at least 0.");
      },
      /** Resolves `Zoomed` with the zoom the browser settled on. Chrome's own range check is `browser_refused`. */
      async run(args, fx) {
        await requireTab(args.tab_id);
        fx.watch(args.tab_id);
        fx.dispatched = true;
        try {
          await chromeApi.tabs.setZoom(args.tab_id, args.zoom);
        } catch (error) {
          throw protocol.failure(FAILURE.BROWSER_REFUSED, shared.bounded(error?.message ?? "The browser refused the zoom level.", MESSAGE_CHARS));
        }
        return { tab_id: args.tab_id, zoom: await chromeApi.tabs.getZoom(args.tab_id) };
      }
    });

    const resizeWindow = handler({
      addresses: true,
      validate(args) {
        requireArgs(args);
        requireTabId(args.tab_id);
        if (!Number.isSafeInteger(args.width) || args.width < WIDTH_LIMITS.min || args.width > WIDTH_LIMITS.max) {
          throw invalid(`Window width must be between ${WIDTH_LIMITS.min} and ${WIDTH_LIMITS.max}.`);
        }
        if (!Number.isSafeInteger(args.height) || args.height < HEIGHT_LIMITS.min || args.height > HEIGHT_LIMITS.max) {
          throw invalid(`Window height must be between ${HEIGHT_LIMITS.min} and ${HEIGHT_LIMITS.max}.`);
        }
      },
      /** Resolves `WindowResized`: the window's size afterwards and every tab that shares it. */
      async run(args, fx) {
        const tab = await requireTab(args.tab_id);
        fx.watch(args.tab_id);
        fx.dispatched = true;
        const resized = await chromeApi.windows.update(tab.windowId, { width: args.width, height: args.height });
        const inWindow = await chromeApi.tabs.query({ windowId: tab.windowId });
        return {
          tab_id: args.tab_id,
          width: Number.isSafeInteger(resized?.width) ? resized.width : args.width,
          height: Number.isSafeInteger(resized?.height) ? resized.height : args.height,
          affected_tab_ids: sortedUnique([...inWindow.map((item) => item.id).filter(Number.isSafeInteger), args.tab_id])
        };
      }
    });

    /**
     * Accepts a `beforeunload` dialog on a tab whose navigation asked to discard them. The worker calls
     * this for every opened dialog (`dialog` is `{ type, sessionId }`). Resolves true when it accepted one.
     */
    async function acceptBeforeUnload(tabId, dialog) {
      if (!beforeUnloadTabs.has(tabId) || dialog?.type !== BEFORE_UNLOAD) return false;
      const target = dialog.sessionId ? { tabId, sessionId: dialog.sessionId } : { tabId };
      await lifecycle.cdp(target, CDP.PAGE_HANDLE_JAVASCRIPT_DIALOG, { accept: true });
      return true;
    }

    return Object.freeze({ listTabs, openTab, navigate, focusTab, closeTab, setZoom, resizeWindow, acceptBeforeUnload });
  }

  return Object.freeze({ create, tabFact, GROUPS_KEY });
});
