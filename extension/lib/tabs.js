// Ghostlight -- the tab primitives of adapter protocol 4.
(function installGhostlightTabs(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), sessions: require("./sessions.js"), shared: require("./shared.js"), state: require("./state.js") }
    : { protocol: root.GhostlightProtocol, sessions: root.GhostlightSessions, shared: root.GhostlightShared, state: root.GhostlightState });
  root.GhostlightTabs = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightTabsApi(deps) {
  "use strict";

  const { protocol, sessions, shared, state } = deps;

  // Tabs, windows and zoom: list_tabs, open_tab, navigate, focus_tab, close_tab, set_zoom and
  // resize_window. The service chooses which tab to touch and which tab to adopt; this module only
  // does what a request names. A tab opened or adopted for the service joins a tab group found by the
  // service's opaque group key (a title match repairs the mapping after an extension reload), and
  // every handler is a `{ validate, run, addresses, tabOf }` entry for the primitive dispatcher.

  const GROUPS_KEY = "ghostlight.groups";
  const GROUP_COLOR = "blue";
  const GROUP_KEY_CHARS = 200;
  const GROUP_TITLE_CHARS = 120;
  const MESSAGE_CHARS = 500;
  const READY_POLL_MS = 100;
  const HISTORY_SETTLE_MS = 75;
  const BLANK_URL = "about:blank";
  const STATUS_LOADING = "loading";
  const STATUS_COMPLETE = "complete";
  const BEFORE_UNLOAD = "beforeunload";
  const METHOD = Object.freeze({ HANDLE_DIALOG: "Page.handleJavaScriptDialog" });
  const TARGET = Object.freeze({ URL: "url", BACK: "back", FORWARD: "forward", RELOAD: "reload" });
  const FAILURE = Object.freeze({
    INVALID_REQUEST: "invalid_request",
    TAB_MISSING: "tab_missing",
    TIMEOUT: "timeout",
    CANCELLED: "cancelled",
    LOCAL_INTERLOCK: "local_interlock",
    BROWSER_REFUSED: "browser_refused",
    OTHER: "other"
  });
  const INTERLOCK_MESSAGE = "Ghostlight is preserving controlled tabs by local browser choice.";
  const WIDTH_LIMITS = protocol.vocabulary.limits.resize_width;
  const HEIGHT_LIMITS = protocol.vocabulary.limits.resize_height;

  function invalid(message) {
    return sessions.failure(FAILURE.INVALID_REQUEST, message);
  }

  function isObject(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }

  function requireTabId(value, name = "tab_id") {
    if (!Number.isSafeInteger(value) || value <= 0) throw invalid(`${name} must be a positive integer.`);
    return value;
  }

  function requireText(value, name, maximum, { empty = false } = {}) {
    if (typeof value !== "string" || (!empty && value.length === 0) || value.length > maximum) {
      throw invalid(`${name} must be text of at most ${maximum} characters.`);
    }
    return value;
  }

  function requireOptionalBoolean(value, name) {
    if (value !== undefined && typeof value !== "boolean") throw invalid(`${name} must be true or false.`);
  }

  function requireDuration(value, name) {
    if (!Number.isSafeInteger(value) || value < 0) throw invalid(`${name} must be a whole number of milliseconds.`);
  }

  function requireArgs(args) {
    if (!isObject(args)) throw invalid("The request arguments must be an object.");
  }

  /** The `Tab` outcome object of a Chrome tab. `opener` is the opener tab id to report, when there is one. */
  function tabFact(tab, opener) {
    const fact = {
      tab_id: tab.id,
      title: shared.bounded(tab.title, protocol.TITLE_CHARS),
      url: shared.bounded(tab.url || tab.pendingUrl || BLANK_URL, protocol.URL_CHARS),
      active: Boolean(tab.active),
      readiness: shared.readinessForStatus(tab.status)
    };
    if (opener !== undefined) fact.opener_tab_id = opener;
    return fact;
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
   * `manage(tabId)`, `release(tabId)` and `managedIds()`. `isCancelled(correlation)` reports a
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
    if (!lifecycle?.cdp || !controlled?.manage || !controlled?.release || !controlled?.managedIds) {
      throw new TypeError("the tab primitives require debugger sessions and the controlled-tab hook");
    }

    const groups = new Map();
    const beforeUnloadTabs = new Map();
    let loaded = null;
    let queue = Promise.resolve();

    // ---- groups -------------------------------------------------------------------------------

    // Group changes run one at a time, so two opens for one key never create two groups.
    function serialized(task) {
      const result = queue.then(task, task);
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
    // merging is: move every tab of each same-title duplicate into the canonical group.
    async function mergeDuplicates(title, canonical) {
      const duplicates = (await chromeApi.tabGroups.query({}))
        .filter((group) => group.title === title && group.id !== canonical.id)
        .sort(byGroupId);
      for (const duplicate of duplicates) {
        try {
          const strays = await chromeApi.tabs.query({ groupId: duplicate.id });
          const tabIds = strays.map((tab) => tab.id).filter((id) => id !== undefined);
          if (tabIds.length > 0) await chromeApi.tabs.group({ groupId: canonical.id, tabIds });
        } catch (_error) { /* left for the next open; cleanup never fails the caller */ }
      }
    }

    // The group for a key: the stored group when it still carries the title, else the lowest-id group
    // with exactly that title (which repairs the mapping after an extension reload).
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
      const exact = (await chromeApi.tabGroups.query({})).filter((group) => group.title === title).sort(byGroupId)[0];
      if (!exact) return undefined;
      groups.set(key, { groupId: exact.id, title });
      await mergeDuplicates(title, exact);
      return exact;
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
      if (tab?.id === undefined) throw sessions.failure(FAILURE.OTHER, "Ghostlight could not observe the tab created by Chromium.");
      return tab;
    }

    async function createGroupedTab(args, fx) {
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
      });
    }

    async function adoptGroupedTab(tabId, args) {
      return serialized(async () => {
        await loadGroups();
        const group = await canonicalGroup(args.group_key, args.group_title);
        return placeInGroup(tabId, args.group_key, args.group_title, group);
      });
    }

    // ---- shared helpers -----------------------------------------------------------------------

    async function requireTab(tabId) {
      try {
        return await chromeApi.tabs.get(tabId);
      } catch (error) {
        throw sessions.failure(FAILURE.TAB_MISSING, shared.bounded(error?.message ?? `No tab with id: ${tabId}.`, MESSAGE_CHARS));
      }
    }

    // Polls the tab until it is complete, or settled with a url, or the timeout passes. A cancel
    // frame ends the wait; the effect already happened, so the caller's dispatched flag stays true.
    async function waitForReady(tabId, timeoutMs, correlation) {
      const deadline = now() + timeoutMs;
      for (;;) {
        if (isCancelled(correlation)) throw sessions.failure(FAILURE.CANCELLED, "The request was cancelled.");
        const tab = await requireTab(tabId);
        if (tab.status === STATUS_COMPLETE) return tab;
        if (tab.status !== STATUS_LOADING && (tab.url || tab.pendingUrl)) return tab;
        if (now() >= deadline) throw sessions.failure(FAILURE.TIMEOUT, `The tab did not become ready in ${timeoutMs} ms.`);
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
      /** Resolves `Tabs`. Reads only: no debugger, no custody. An opener is reported only when the service manages it or named it. */
      async run(args) {
        const named = Array.isArray(args.tab_ids) ? new Set(args.tab_ids) : null;
        const openers = new Set([...controlled.managedIds(), ...(named ?? [])]);
        const all = await chromeApi.tabs.query({});
        return {
          tabs: all
            .filter((tab) => !named || named.has(tab.id) || named.has(tab.openerTabId))
            .map((tab) => tabFact(tab, openers.has(tab.openerTabId) ? tab.openerTabId : undefined))
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
        requireText(args.group_title, "group_title", GROUP_TITLE_CHARS);
        if (typeof args.focus_window !== "boolean") throw invalid("focus_window must be true or false.");
        requireDuration(args.ready_timeout_ms, "ready_timeout_ms");
      },
      /**
       * Resolves `TabOpened`. Adopts the named tab (moving it into the group) or creates a tab in the
       * group's window, then marks the tab controlled and waits for it to be ready.
       */
      async run(args, fx, correlation) {
        let tab;
        if (args.adopt_tab_id !== undefined) {
          const existing = await requireTab(args.adopt_tab_id);
          fx.watch(existing.id);
          fx.dispatched = true;
          const placed = await adoptGroupedTab(existing.id, args);
          await chromeApi.tabs.update(existing.id, args.url ? { url: args.url, active: true } : { active: true });
          if (args.focus_window) await chromeApi.windows.update(placed.windowId, { focused: true });
          tab = existing;
        } else {
          tab = await createGroupedTab(args, fx);
        }
        await controlled.manage(tab.id);
        const ready = await waitForReady(tab.id, args.ready_timeout_ms, correlation);
        return { tab: tabFact(ready), adopted: args.adopt_tab_id !== undefined };
      }
    });

    const navigate = handler({
      addresses: true,
      validate(args) {
        requireArgs(args);
        requireTabId(args.tab_id);
        if (!isObject(args.to) || !protocol.isKnown("navigation_targets", args.to.to)) throw invalid("to must name a navigation target.");
        if (args.to.to === TARGET.URL) requireText(args.to.url, "to.url", protocol.URL_CHARS);
        requireOptionalBoolean(args.to.bypass_cache, "to.bypass_cache");
        requireOptionalBoolean(args.discard_beforeunload, "discard_beforeunload");
        requireDuration(args.ready_timeout_ms, "ready_timeout_ms");
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
          fx.dispatched = true;
          if (args.to.to === TARGET.URL) {
            await chromeApi.tabs.update(args.tab_id, { url: args.to.url, active: true });
          } else if (args.to.to === TARGET.BACK) {
            await chromeApi.tabs.goBack(args.tab_id);
            await sleep(HISTORY_SETTLE_MS);
          } else if (args.to.to === TARGET.FORWARD) {
            await chromeApi.tabs.goForward(args.tab_id);
            await sleep(HISTORY_SETTLE_MS);
          } else {
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
       * `local_interlock`; a released close that is vetoed still ends Ghostlight's control of the tab.
       */
      async run(args, fx) {
        if (await preserveTabsEnabled()) {
          if (args.released === true) await controlled.release(args.tab_id);
          throw sessions.failure(FAILURE.LOCAL_INTERLOCK, INTERLOCK_MESSAGE);
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
          throw sessions.failure(FAILURE.BROWSER_REFUSED, shared.bounded(error?.message ?? "The browser refused the zoom level.", MESSAGE_CHARS));
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
      await lifecycle.cdp(target, METHOD.HANDLE_DIALOG, { accept: true });
      return true;
    }

    return Object.freeze({ listTabs, openTab, navigate, focusTab, closeTab, setZoom, resizeWindow, acceptBeforeUnload });
  }

  return Object.freeze({ create, tabFact, GROUPS_KEY });
});
