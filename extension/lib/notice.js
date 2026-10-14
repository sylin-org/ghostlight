// Ghostlight -- the notice delivery of adapter protocol 4.
(function installGhostlightNotice(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { shared: require("./shared.js") }
    : { shared: root.GhostlightShared });
  root.GhostlightNotice = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightNoticeApi(deps) {
  "use strict";

  const { shared } = deps;

  // A notice is the service's one-way presentation of what Ghostlight is doing: a spinner, a
  // target ring, a completion mark, a block. It is rendered by the page code in the tab's own world;
  // this module only delivers it. It never attaches a debugger and never installs page code, so a
  // notice for a tab the adapter is not attached to is simply not rendered. A denial notice may ask to
  // wait until its page is visible: it is queued (with the frame it names), the invocation's
  // spinner is retired at once, and the queued notice is shown when the person looks at the tab.

  // The queue is keyed by workspace; this adapter has one namespace, because the service decides
  // which tab a notice is for.
  const NAMESPACE = "ghostlight";
  const SIGNAL = Object.freeze({ COMPLETION: "completion", DENIAL: "denial", ATTENTION: "attention" });
  const QUIET_ACTIVITY = "quiet";
  const GROUP_PREFIX = /^Ghostlight - /;
  const DEFAULT_CLIENT_LABEL = "MCP client";
  const PHASE_CHARS = 80;
  const ACTIVITY_LIMIT = 8;
  const NO_GROUP = -1;

  function isTerminal(signal) {
    return signal === SIGNAL.COMPLETION || signal === SIGNAL.DENIAL || signal === SIGNAL.ATTENTION;
  }

  /**
   * Creates the notice delivery.
   *
   * `worldCalls` is a GhostlightWorld, `inventory` a GhostlightInventory and `queue` a
   * GhostlightPresentationQueue instance. `changed()` is called after the queue or the activity list
   * changes, so the worker can persist the queue and refresh the popup and badge.
   */
  function create({
    chromeApi = globalThis.chrome,
    worldCalls,
    inventory,
    queue,
    changed = () => {}
  } = {}) {
    if (!worldCalls?.present || !inventory?.snapshot || !queue?.defer) {
      throw new TypeError("notices require the world, the inventory and the presentation queue");
    }
    if (!chromeApi?.tabs || !chromeApi?.windows || !chromeApi?.tabGroups) {
      throw new TypeError("notices require the Chrome tabs, windows and tabGroups APIs");
    }
    const activity = new Map();

    async function groupTitleOf(tabId) {
      try {
        const tab = await chromeApi.tabs.get(tabId);
        if (!Number.isInteger(tab.groupId) || tab.groupId === NO_GROUP) return "";
        return (await chromeApi.tabGroups.get(tab.groupId)).title ?? "";
      } catch (_error) {
        return "";
      }
    }

    // The tab is the active one of a focused window.
    async function tabIsVisible(tabId) {
      try {
        const tab = await chromeApi.tabs.get(tabId);
        if (!tab.active || !Number.isInteger(tab.windowId)) return false;
        return Boolean((await chromeApi.windows.get(tab.windowId)).focused);
      } catch (_error) {
        return false;
      }
    }

    // The popup's activity list: one line per running invocation, labelled with the tab's group title.
    function updateActivity(signal, groupTitle) {
      if (isTerminal(signal.signal)) {
        activity.delete(signal.invocation);
      } else {
        activity.set(signal.invocation, {
          invocation: signal.invocation,
          client_label: groupTitle.replace(GROUP_PREFIX, "") || DEFAULT_CLIENT_LABEL,
          label: shared.activityLabel(signal.activity),
          phase: shared.bounded(signal.phase, PHASE_CHARS)
        });
      }
      changed();
    }

    function forget(tabId) {
      if (queue.forget(tabId)) changed();
    }

    // The queue keeps the frame inside the stored signal; reading splits it back out. The queue
    // accepts a detail only as text, so an absent one is left out.
    function defer(tabId, frame, signal) {
      const stored = frame ? { ...signal, frame } : { ...signal };
      if (typeof stored.detail !== "string") delete stored.detail;
      if (queue.defer(NAMESPACE, tabId, stored)) changed();
    }

    function queuedNotice(tabId) {
      const entry = queue.get(tabId);
      if (!entry) return null;
      const { frame = null, tab_id: _tabId, ...signal } = entry.signal;
      return { frame, signal };
    }

    async function frameIsCurrent(tabId, frame) {
      try {
        const snap = await inventory.snapshot(tabId);
        return snap.owner.get(frame.frame_id)?.loaderId === frame.loader_id;
      } catch (_error) {
        return false;
      }
    }

    /**
     * Delivers a `notice` frame. Renders it in the world of its frame (the top document when the
     * frame is absent). A deferrable notice for a tab nobody is looking at first retires the
     * invocation's decoration with a quiet completion, then waits in the queue. Resolves true when
     * the notice was rendered.
     */
    async function deliver({ tab_id: tabId, frame = null, signal, defer_until_visible: deferUntilVisible = false } = {}) {
      if (!Number.isSafeInteger(tabId) || !signal || typeof signal !== "object") return false;
      updateActivity(signal, await groupTitleOf(tabId));
      if (deferUntilVisible && !(await tabIsVisible(tabId))) {
        await worldCalls.present(tabId, frame, {
          ...signal, signal: SIGNAL.COMPLETION, activity: QUIET_ACTIVITY, phase: "", detail: null, locator: null, click: null
        });
        defer(tabId, frame, signal);
        return false;
      }
      const rendered = await worldCalls.present(tabId, frame, signal);
      if (rendered) forget(tabId);
      else if (deferUntilVisible) defer(tabId, frame, signal);
      return rendered;
    }

    /**
     * Shows a tab's queued notice once the tab is visible. A frame that no longer holds the document
     * the notice named falls back to the top document without its locator. Resolves true when it was
     * rendered. Called on tab activation, window focus and tab load.
     */
    async function flush(tabId) {
      const pending = queuedNotice(tabId);
      if (!pending || !(await tabIsVisible(tabId))) return false;
      const current = pending.frame ? await frameIsCurrent(tabId, pending.frame) : false;
      const signal = current ? pending.signal : { ...pending.signal, locator: null };
      const rendered = await worldCalls.present(tabId, current ? pending.frame : null, signal);
      if (rendered) forget(tabId);
      return rendered;
    }

    /** Flushes the active tab of a window that took focus. A focus change to no window does nothing. */
    async function flushWindow(windowId) {
      if (!Number.isInteger(windowId) || windowId < 0) return;
      const active = await chromeApi.tabs.query({ active: true, windowId });
      await Promise.all(active.map((tab) => flush(tab.id)));
    }

    /** The invocations currently shown in the popup, oldest first, at most eight. */
    function activityList() {
      return Array.from(activity.values()).slice(-ACTIVITY_LIMIT);
    }

    return Object.freeze({ deliver, flush, flushWindow, activityList });
  }

  return Object.freeze({ create, NAMESPACE });
});
