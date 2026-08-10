(function installGhostlightDebuggerLifecycle(root, factory) {
  const api = factory();
  root.GhostlightDebuggerLifecycle = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightDebuggerLifecycleApi() {
  "use strict";

  function create(debuggerApi, protocolVersion = "1.3") {
    if (!debuggerApi?.attach || !debuggerApi?.detach || !debuggerApi?.sendCommand) {
      throw new TypeError("debugger lifecycle requires the Chrome debugger API");
    }

    const tabs = new Map();

    function tabState(tabId) {
      let state = tabs.get(tabId);
      if (!state) {
        state = {
          leases: 0,
          retained: false,
          attached: false,
          attaching: null,
          detaching: null,
          dialog: null
        };
        tabs.set(tabId, state);
      }
      return state;
    }

    function prune(tabId, state) {
      if (!state.retained && !state.attached && !state.attaching && !state.detaching && state.leases === 0 && !state.dialog) {
        tabs.delete(tabId);
      }
    }

    async function ensureAttached(tabId, state) {
      if (state.detaching) await state.detaching;
      if (state.attached) return;
      if (!state.attaching) {
        state.attaching = (async () => {
          await debuggerApi.attach({ tabId }, protocolVersion);
          state.attached = true;
          try {
            await debuggerApi.sendCommand({ tabId }, "Page.enable");
          } catch (error) {
            try { await debuggerApi.detach({ tabId }); } catch (_detachError) { /* already detached */ }
            state.attached = false;
            throw error;
          }
        })();
      }
      try {
        await state.attaching;
      } finally {
        state.attaching = null;
        prune(tabId, state);
      }
    }

    async function settle(tabId, state) {
      if (state.retained || state.leases > 0 || state.dialog || !state.attached || state.attaching) return;
      if (!state.detaching) {
        state.detaching = (async () => {
          try { await debuggerApi.detach({ tabId }); } catch (_error) { /* already detached */ }
          state.attached = false;
        })();
      }
      try {
        await state.detaching;
      } finally {
        state.detaching = null;
        prune(tabId, state);
      }
    }

    async function acquire(tabId) {
      const state = tabState(tabId);
      state.leases += 1;
      try {
        await ensureAttached(tabId, state);
      } catch (error) {
        state.leases -= 1;
        prune(tabId, state);
        throw error;
      }
    }

    async function retain(tabId) {
      const state = tabState(tabId);
      state.retained = true;
      await ensureAttached(tabId, state);
    }

    async function release(tabId) {
      const state = tabs.get(tabId);
      if (!state || state.leases === 0) return;
      state.leases -= 1;
      await settle(tabId, state);
    }

    function openDialog(tabId, type) {
      tabState(tabId).dialog = { type: type || "unknown" };
    }

    async function closeDialog(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      state.dialog = null;
      await settle(tabId, state);
    }

    function currentDialog(tabId) {
      const dialog = tabs.get(tabId)?.dialog;
      return dialog ? { ...dialog } : null;
    }

    function detached(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      state.attached = false;
      prune(tabId, state);
    }

    function forget(tabId) {
      tabs.delete(tabId);
    }

    async function detachAll() {
      await Promise.all(Array.from(tabs, async ([tabId, state]) => {
        state.retained = false;
        state.dialog = null;
        state.leases = 0;
        if (state.attaching) {
          try { await state.attaching; } catch (_error) { /* attachment already failed */ }
        }
        if (state.detaching) await state.detaching;
        if (state.attached) {
          try { await debuggerApi.detach({ tabId }); } catch (_error) { /* already detached */ }
          state.attached = false;
        }
      }));
      tabs.clear();
    }

    function attachedCount() {
      return Array.from(tabs.values()).filter((state) => state.attached).length;
    }

    return Object.freeze({
      acquire,
      retain,
      release,
      openDialog,
      closeDialog,
      currentDialog,
      detached,
      forget,
      detachAll,
      attachedCount
    });
  }

  return Object.freeze({ create });
});
