(function installNativeInput(root, factory) {
  const api = factory();
  root.GhostlightNativeInput = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createNativeInputApi() {
  "use strict";
  const SELECTION_EVENT_TIMEOUT_MS = 1000;

  // Browser-local serialization and current-window safety, never a policy preference.
  function create(chromeApi, workspaceFor, protectedOutcome) {
    const windows = new Map();
    const changed = windowId => { const entry = windows.get(windowId); if (entry) entry.revision++; };
    chromeApi.tabs.onActivated?.addListener(info => {
      changed(info.windowId);
      const selection = windows.get(info.windowId)?.selection;
      if (selection?.tabId === info.tabId) selection.resolve(true);
    });
    chromeApi.tabs.onCreated?.addListener(tab => changed(tab.windowId));
    chromeApi.tabs.onRemoved?.addListener((_id, info) => changed(info.windowId));
    chromeApi.tabs.onAttached?.addListener((_id, info) => changed(info.newWindowId));
    chromeApi.tabs.onDetached?.addListener((_id, info) => changed(info.oldWindowId));
    chromeApi.tabs.onMoved?.addListener((_id, info) => changed(info.windowId));
    chromeApi.windows.onFocusChanged?.addListener(id => changed(id));

    function refusal() { return Object.assign(new Error("The native input window changed or contains foreground work."), { nativeInputProtected: true }); }
    async function snapshot(tabId, windowId, entry, expectedRevision) {
      const revision = entry.revision;
      if (expectedRevision !== null && revision !== expectedRevision) throw refusal();
      const [tab, window, tabs] = await Promise.all([
        chromeApi.tabs.get(tabId), chromeApi.windows.get(windowId), chromeApi.tabs.query({ windowId })
      ]);
      if (entry.revision !== revision || tab.windowId !== windowId) throw refusal();
      return { tab, window, tabs, revision };
    }

    async function run(tabId, { surface }, operation) {
      const initial = await chromeApi.tabs.get(tabId);
      const windowId = initial.windowId;
      let entry = windows.get(windowId);
      if (!entry) { entry = { tail: Promise.resolve(), revision: 0, users: 0 }; windows.set(windowId, entry); }
      entry.users++;
      const previous = entry.tail;
      let unlock;
      entry.tail = new Promise(resolve => { unlock = resolve; });
      await previous;
      let started = false;
      const heldKeys = new Set();
      const heldButtons = new Set();
      let preparedRevision = null;
      const eligible = state => {
        if (workspaceFor(tabId) === null) throw refusal();
        if (surface) {
          if (state.window.focused || state.tabs.some(tab => workspaceFor(tab.id) === null)) throw refusal();
        } else if (state.tab.active && state.window.focused) throw refusal();
      };
      const context = {
        async prepare() {
          let state = await snapshot(tabId, windowId, entry, null);
          eligible(state);
          if (surface && !state.tab.active) {
            // The final snapshot fences movement/focus/addition before this selection.
            state = await snapshot(tabId, windowId, entry, state.revision);
            eligible(state);
            // Chrome can resolve update before delivering onActivated. Fence our own
            // selection event before freezing the revision used by native packets.
            let timer;
            const selected = new Promise(resolve => {
              entry.selection = { tabId, resolve };
              timer = setTimeout(() => resolve(false), SELECTION_EVENT_TIMEOUT_MS);
            });
            try {
              await chromeApi.tabs.update(tabId, { active: true });
              if (!await selected) throw refusal();
            } finally {
              clearTimeout(timer);
              delete entry.selection;
            }
            state = await snapshot(tabId, windowId, entry, null);
            eligible(state);
            if (!state.tab.active) throw refusal();
          }
          preparedRevision = state.revision;
        },
        async check() {
          const state = await snapshot(tabId, windowId, entry, preparedRevision);
          eligible(state);
          if (surface && !state.tab.active) throw refusal();
        },
        isCompensation(method, params) {
          // Complete releases even after takeover; never continue held movement or new input.
          return method === "Input.cancelDragging" && heldButtons.size > 0
            || method === "Input.dispatchKeyEvent" && params?.type === "keyUp" && heldKeys.has(params.code ?? params.key)
            || method === "Input.dispatchMouseEvent" && params?.type === "mouseReleased" && heldButtons.has(params.button ?? "left");
        },
        async beforePacket(method, params) {
          const compensation = context.isCompensation(method, params);
          if (!compensation) await context.check();
          started = true;
          if (method === "Input.dispatchKeyEvent" && ["keyDown", "rawKeyDown"].includes(params?.type)) heldKeys.add(params.code ?? params.key);
          if (method === "Input.dispatchMouseEvent" && params?.type === "mousePressed") heldButtons.add(params.button ?? "left");
          return compensation;
        },
        afterPacket(method, params) {
          if (method === "Input.dispatchKeyEvent" && params?.type === "keyUp") heldKeys.delete(params.code ?? params.key);
          if (method === "Input.dispatchMouseEvent" && params?.type === "mouseReleased") heldButtons.delete(params.button ?? "left");
          if (method === "Input.cancelDragging") heldButtons.clear();
        },
        markEffect() { started = true; }
      };
      try {
        await context.prepare();
        const result = await operation(context);
        await context.check();
        return result;
      } catch (error) {
        if (!error?.nativeInputProtected) throw error;
        if (!started) return protectedOutcome();
        error.effectUnknown = true;
        throw error;
      } finally {
        unlock();
        if (--entry.users === 0) windows.delete(windowId);
      }
    }
    return Object.freeze({ run, pendingWindows: () => windows.size });
  }
  return Object.freeze({ create });
});
