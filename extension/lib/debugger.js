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
    let pageRuntimeScript = null;
    const iframeAutoAttach = Object.freeze({
      autoAttach: true,
      waitForDebuggerOnStart: true,
      flatten: true,
      filter: Object.freeze([{ type: "iframe", exclude: false }])
    });
    // The worker supplies the negotiated runtime state. Restored tabs start with
    // ordinary focus until the service confirms that browser work is active.
    let focusEmulationEnabled = false;

    function tabState(tabId) {
      let state = tabs.get(tabId);
      if (!state) {
        state = {
          leases: 0,
          retained: false,
          attached: false,
          pending: Promise.resolve(),
          pendingCount: 0,
          closing: false,
          generation: 0,
          focusEmulated: false,
          focusAttempted: false,
          dialog: null,
          domains: new Set(),
          runtimeScript: null,
          runtimeError: null,
          runtimeTasks: new Set()
        };
        tabs.set(tabId, state);
      }
      return state;
    }

    function prune(tabId, state) {
      if (tabs.get(tabId) === state && !state.retained && !state.attached && state.pendingCount === 0 && state.leases === 0 && !state.dialog) {
        tabs.delete(tabId);
      }
    }

    // Serialize Chrome lifecycle commands for one tab. Ownership and control flags
    // change immediately, so a queued enable cannot overtake a later pause/release.
    function enqueue(tabId, state, action) {
      state.pendingCount += 1;
      const result = state.pending.then(action).finally(() => {
        state.pendingCount -= 1;
        prune(tabId, state);
      });
      state.pending = result.catch(() => {});
      return result;
    }

    function clearAttachment(state) {
      state.attached = false;
      state.focusEmulated = false;
      state.focusAttempted = false;
      state.generation += 1;
      state.domains.clear();
      state.runtimeScript = null;
      state.runtimeError = null;
      state.runtimeTasks.clear();
    }

    function trackRuntimeTask(state, generation, work) {
      let guarded;
      guarded = Promise.resolve(work)
        .catch((error) => {
          if (state.generation === generation) state.runtimeError = error;
        })
        .finally(() => state.runtimeTasks.delete(guarded));
      state.runtimeTasks.add(guarded);
      return guarded;
    }

    async function waitForRuntimeTasks(state) {
      while (state.runtimeTasks.size) {
        await Promise.all(Array.from(state.runtimeTasks));
      }
      if (state.runtimeError) throw state.runtimeError;
    }

    async function installRuntimeInSession(target, enablePage) {
      if (enablePage) await debuggerApi.sendCommand(target, "Page.enable");
      await debuggerApi.sendCommand(target, "Page.addScriptToEvaluateOnNewDocument", {
        source: pageRuntimeScript,
        runImmediately: true
      });
      await debuggerApi.sendCommand(target, "Target.setAutoAttach", iframeAutoAttach);
    }

    async function installRuntimeInTab(tabId, state) {
      if (!pageRuntimeScript || state.runtimeScript === pageRuntimeScript) {
        await waitForRuntimeTasks(state);
        return;
      }
      state.runtimeError = null;
      await installRuntimeInSession({ tabId }, false);
      await waitForRuntimeTasks(state);
      state.runtimeScript = pageRuntimeScript;
    }

    debuggerApi.onEvent?.addListener((source, method, params) => {
      if (method !== "Target.attachedToTarget" || !source.tabId || !params?.sessionId) return;
      const state = tabs.get(source.tabId);
      if (!state?.attached || !pageRuntimeScript || params.targetInfo?.type !== "iframe") return;
      const generation = state.generation;
      const child = { ...source, sessionId: params.sessionId };
      const work = (async () => {
        try {
          await installRuntimeInSession(child, true);
        } finally {
          if (params.waitingForDebugger) {
            await debuggerApi.sendCommand(child, "Runtime.runIfWaitingForDebugger");
          }
        }
      })();
      trackRuntimeTask(state, generation, work);
    });

    async function detachSession(tabId, state) {
      if (!state.attached) return;
      if (state.focusEmulated || state.focusAttempted) {
        try {
          await debuggerApi.sendCommand({ tabId }, "Emulation.setFocusEmulationEnabled", { enabled: false });
          state.focusEmulated = false;
          state.focusAttempted = false;
        } catch (_error) { /* detachment also removes the override */ }
      }
      try {
        await debuggerApi.detach({ tabId });
      } catch (error) {
        // An external onDetach may already have confirmed removal. Otherwise keep
        // the state for cleanup retry instead of claiming that an override is gone.
        if (state.attached) throw error;
      }
      clearAttachment(state);
    }

    async function syncFocus(tabId, state) {
      while (state.attached) {
        const enabled = focusEmulationEnabled && state.retained && !state.closing;
        if (state.focusEmulated === enabled) return;
        const generation = state.generation;
        state.focusAttempted = true;
        try {
          await debuggerApi.sendCommand({ tabId }, "Emulation.setFocusEmulationEnabled", { enabled });
        } catch (error) {
          // A failed command may have taken effect. Release the session rather than
          // leave an override whose state we cannot confirm.
          await detachSession(tabId, state);
          throw error;
        }
        if (state.generation !== generation) return;
        state.focusEmulated = enabled;
        state.focusAttempted = false;
      }
    }

    async function ensureAttached(tabId, state) {
      if (state.closing) throw new Error("The debugger session was released.");
      try {
        if (!state.attached) {
          await debuggerApi.attach({ tabId }, protocolVersion);
          state.attached = true;
          await debuggerApi.sendCommand({ tabId }, "Page.enable");
          state.domains.add("Page");
        }
        await installRuntimeInTab(tabId, state);
        await syncFocus(tabId, state);
        if (!state.attached || state.closing) throw new Error("The debugger session was released during setup.");
      } catch (error) {
        await detachSession(tabId, state);
        throw error;
      }
    }

    async function settle(tabId, state) {
      if (!state.retained && state.leases === 0 && !state.dialog) await detachSession(tabId, state);
    }

    async function acquire(tabId) {
      const state = tabState(tabId);
      state.leases += 1;
      try {
        await enqueue(tabId, state, () => ensureAttached(tabId, state));
      } catch (error) {
        state.leases = Math.max(0, state.leases - 1);
        prune(tabId, state);
        throw error;
      }
    }

    async function retain(tabId) {
      const state = tabState(tabId);
      state.retained = true;
      await enqueue(tabId, state, () => ensureAttached(tabId, state));
    }

    async function unretain(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      state.retained = false;
      await enqueue(tabId, state, async () => {
        await syncFocus(tabId, state);
        await settle(tabId, state);
      });
    }

    async function setFocusEmulationEnabled(enabled) {
      focusEmulationEnabled = Boolean(enabled);
      const results = await Promise.allSettled(Array.from(tabs, ([tabId, state]) =>
        enqueue(tabId, state, () => syncFocus(tabId, state))));
      const errors = results.filter((result) => result.status === "rejected").map((result) => result.reason);
      if (errors.length) throw new AggregateError(errors, "Could not update controlled-tab focus.");
    }

    async function release(tabId) {
      const state = tabs.get(tabId);
      if (!state || state.leases === 0) return;
      state.leases -= 1;
      await enqueue(tabId, state, () => settle(tabId, state));
    }

    function openDialog(tabId, type) {
      tabState(tabId).dialog = { type: type || "unknown" };
    }

    async function closeDialog(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      state.dialog = null;
      await enqueue(tabId, state, () => settle(tabId, state));
    }

    function currentDialog(tabId) {
      const dialog = tabs.get(tabId)?.dialog;
      return dialog ? { ...dialog } : null;
    }

    async function enableDomain(tabId, domain) {
      const state = tabState(tabId);
      return enqueue(tabId, state, async () => {
        await ensureAttached(tabId, state);
        if (state.domains.has(domain)) return false;
        await debuggerApi.sendCommand({ tabId }, `${domain}.enable`);
        state.domains.add(domain);
        return true;
      });
    }

    async function disableDomain(tabId, domain) {
      const state = tabs.get(tabId);
      if (!state || domain === "Page") return;
      await enqueue(tabId, state, async () => {
        if (!state.attached || !state.domains.has(domain)) return;
        try {
          await debuggerApi.sendCommand({ tabId }, `${domain}.disable`);
        } finally {
          state.domains.delete(domain);
        }
      });
    }

    function detached(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      clearAttachment(state);
      prune(tabId, state);
    }

    function forget(tabId) {
      const state = tabs.get(tabId);
      if (state) {
        state.closing = true;
        clearAttachment(state);
      }
      tabs.delete(tabId);
    }

    async function detachAll() {
      const results = await Promise.allSettled(Array.from(tabs, async ([tabId, state]) => {
        state.retained = false;
        state.closing = true;
        state.dialog = null;
        state.leases = 0;
        try {
          await enqueue(tabId, state, () => detachSession(tabId, state));
        } finally {
          if (!state.attached && tabs.get(tabId) === state) tabs.delete(tabId);
        }
      }));
      const errors = results.filter((result) => result.status === "rejected").map((result) => result.reason);
      if (errors.length) throw new AggregateError(errors, "Could not release every debugger session.");
    }

    function attachedCount() {
      return Array.from(tabs.values()).filter((state) => state.attached).length;
    }

    async function installPageRuntime(script) {
      if (typeof script !== "string" || script.length === 0) {
        throw new TypeError("page runtime script must be a non-empty string");
      }
      pageRuntimeScript = script;
      const results = await Promise.allSettled(Array.from(tabs, ([tabId, state]) =>
        state.attached ? enqueue(tabId, state, () => installRuntimeInTab(tabId, state)) : Promise.resolve()));
      const errors = results.filter((result) => result.status === "rejected").map((result) => result.reason);
      if (errors.length) throw new AggregateError(errors, "Could not install the page runtime in every attached tab.");
    }

    return Object.freeze({
      acquire,
      retain,
      unretain,
      setFocusEmulationEnabled,
      release,
      openDialog,
      closeDialog,
      currentDialog,
      enableDomain,
      disableDomain,
      detached,
      forget,
      detachAll,
      attachedCount,
      installPageRuntime
    });
  }

  return Object.freeze({ create });
});
