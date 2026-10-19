(function installGhostlightSessions(root, factory) {
  const api = factory();
  root.GhostlightSessions = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightSessionsApi() {
  "use strict";

  // Debugger sessions for adapter protocol 4. This module owns every chrome.debugger attachment:
  // shared attach leases per tab, retained (controlled) tabs, focus emulation, the open JavaScript
  // dialog, opt-in CDP domains, and the registry of root and child (iframe) sessions with the
  // script registration identifiers recorded against each. It is the only path to CDP: every
  // internal call goes through cdp(), which refuses any method outside CDP_ALLOW. It knows nothing
  // about page code. The installer the worker supplies registers that code in each session.

  const CODE_BROWSER_REFUSED = "browser_refused";

  // Every CDP method the adapter may send. Runtime.enable is listed because enableDomain
  // (opt-in diagnostics) sends it; attaching a session never does.
  const CDP_ALLOW = Object.freeze([
    "Page.enable", "Page.getFrameTree", "Page.addScriptToEvaluateOnNewDocument",
    "Page.removeScriptToEvaluateOnNewDocument", "Page.createIsolatedWorld", "Page.getLayoutMetrics",
    "Page.captureScreenshot", "Page.handleJavaScriptDialog", "Page.startScreencast", "Page.stopScreencast",
    "Page.screencastFrameAck",
    "Runtime.evaluate", "Runtime.callFunctionOn", "Runtime.releaseObject", "Runtime.releaseObjectGroup",
    "Runtime.runIfWaitingForDebugger", "Runtime.terminateExecution", "Runtime.enable", "Runtime.disable",
    "Network.enable", "Network.disable", "Target.setAutoAttach", "DOM.getFrameOwner", "DOM.resolveNode",
    "Input.dispatchMouseEvent", "Input.dispatchKeyEvent", "Input.insertText", "Input.dispatchDragEvent",
    "Input.setInterceptDrags", "Input.cancelDragging", "Emulation.setFocusEmulationEnabled",
    "Accessibility.getFullAXTree", "Accessibility.queryAXTree"
  ]);
  const allowed = new Set(CDP_ALLOW);

  const EVENT_ATTACHED = "Target.attachedToTarget";
  const EVENT_DETACHED = "Target.detachedFromTarget";
  const TARGET_IFRAME = "iframe";
  const DIALOG_UNKNOWN = "unknown";
  const DIALOG_OPEN = "dialog_open";
  const DIALOG_OPEN_MESSAGE = "A JavaScript dialog blocks the page.";

  /**
   * Builds a coded failure: an Error whose `code` is a protocol 4 receipt failure code.
   * Callers throw or reject with it so the worker can report the code unchanged.
   */
  function failure(code, message) {
    return Object.assign(new Error(message), { code });
  }

  /**
   * The failure for work that needs a renderer a JavaScript dialog is blocking: the receipt code
   * `dialog_open` with its one message. Anything that would wait on the page throws it at once.
   */
  function dialogOpen() {
    return failure(DIALOG_OPEN, DIALOG_OPEN_MESSAGE);
  }

  const iframeAutoAttach = Object.freeze({
    autoAttach: true,
    waitForDebuggerOnStart: true,
    flatten: true,
    filter: Object.freeze([{ type: TARGET_IFRAME, exclude: false }])
  });

  /**
   * Creates the session registry over the Chrome debugger API.
   *
   * `installer(session)` is required: it registers page code in a root or child session and is
   * awaited before a child resumes. `onRelease(tabId)` is awaited, best effort, before every
   * voluntary detach. `onSessionGone(tabId, sessionId)` fires when a child session ends.
   * A session is `{ target, targetId, identifiers }`: `target` is the CDP target for cdp(),
   * `targetId` is null for a root session, and `identifiers` is the live list of script
   * registration identifiers that belong to it.
   */
  function create(debuggerApi, { installer, onRelease = null, onSessionGone = null, protocolVersion = "1.3" } = {}) {
    if (!debuggerApi?.attach || !debuggerApi?.detach || !debuggerApi?.sendCommand || !debuggerApi?.onEvent?.addListener) {
      throw new TypeError("debugger sessions require the Chrome debugger API");
    }
    if (typeof installer !== "function") {
      throw new TypeError("debugger sessions require an installer");
    }

    const tabs = new Map();
    // The worker supplies the negotiated runtime state. Restored tabs start with
    // ordinary focus until the service confirms that browser work is active.
    let focusEmulationEnabled = false;

    /**
     * Sends one CDP command to `target` and returns its result. A method outside CDP_ALLOW rejects
     * with a `browser_refused` failure and nothing reaches Chrome.
     */
    function cdp(target, method, params) {
      if (!allowed.has(method)) {
        return Promise.reject(failure(CODE_BROWSER_REFUSED, `CDP method ${method} is not allowed`));
      }
      return debuggerApi.sendCommand(target, method, params);
    }

    function tabState(tabId) {
      let state = tabs.get(tabId);
      if (!state) {
        state = {
          leases: 0,
          retained: false,
          attached: false,
          prepared: false,
          pending: Promise.resolve(),
          pendingCount: 0,
          closing: false,
          generation: 0,
          focusEmulated: false,
          focusAttempted: false,
          dialog: null,
          domains: new Set(),
          root: { target: Object.freeze({ tabId }), targetId: null, identifiers: [] },
          children: new Map(),
          childError: null,
          childTasks: new Set()
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

    // Forgets everything that belonged to one debugger attachment. Registrations and child
    // sessions die with the attachment, so their records go with it.
    function clearAttachment(state) {
      state.attached = false;
      state.prepared = false;
      state.focusEmulated = false;
      state.focusAttempted = false;
      state.generation += 1;
      state.domains.clear();
      state.root.identifiers.length = 0;
      state.children.clear();
      state.childError = null;
      state.childTasks.clear();
    }

    function trackChildTask(state, generation, sessionId, work) {
      const guarded = Promise.resolve(work)
        .catch((error) => {
          // A child that already ended has nothing left to install.
          if (state.generation === generation && state.children.has(sessionId)) state.childError = error;
        })
        .finally(() => state.childTasks.delete(guarded));
      state.childTasks.add(guarded);
      return guarded;
    }

    async function waitForChildTasks(state) {
      while (state.childTasks.size) {
        await Promise.all(Array.from(state.childTasks));
      }
      if (state.childError) throw state.childError;
    }

    // A child frame attaches paused. It is recorded, given its own Page domain and registrations,
    // and set to auto-attach its own iframes (their attach events arrive here again). It resumes
    // last, even when setup failed, so a page is never left paused.
    async function setUpChild(child, waitingForDebugger) {
      try {
        await cdp(child.target, "Page.enable");
        await installer(child);
        await cdp(child.target, "Target.setAutoAttach", iframeAutoAttach);
      } finally {
        if (waitingForDebugger) await cdp(child.target, "Runtime.runIfWaitingForDebugger");
      }
    }

    debuggerApi.onEvent.addListener((source, method, params) => {
      const state = source?.tabId === undefined ? undefined : tabs.get(source.tabId);
      if (!state?.attached || !params?.sessionId) return;
      if (method === EVENT_ATTACHED && params.targetInfo?.type === TARGET_IFRAME) {
        const child = {
          target: Object.freeze({ tabId: source.tabId, sessionId: params.sessionId }),
          targetId: params.targetInfo.targetId ?? null,
          identifiers: []
        };
        state.children.set(params.sessionId, child);
        trackChildTask(state, state.generation, params.sessionId, setUpChild(child, Boolean(params.waitingForDebugger)));
      } else if (method === EVENT_DETACHED) {
        state.children.delete(params.sessionId);
        onSessionGone?.(source.tabId, params.sessionId);
      }
    });

    async function detachSession(tabId, state) {
      if (!state.attached) return;
      try {
        await onRelease?.(tabId);
      } catch (_error) { /* releasing page code is best effort; detachment ends the session anyway */ }
      if (state.focusEmulated || state.focusAttempted) {
        try {
          await cdp({ tabId }, "Emulation.setFocusEmulationEnabled", { enabled: false });
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
          await cdp({ tabId }, "Emulation.setFocusEmulationEnabled", { enabled });
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

    // Attaches once per attachment: Page domain, the root registration, then iframe auto-attach so
    // existing and future child frames arrive as sessions. Runtime.enable is never sent here.
    async function ensureAttached(tabId, state) {
      if (state.closing) throw new Error("The debugger session was released.");
      try {
        if (!state.attached) {
          await debuggerApi.attach({ tabId }, protocolVersion);
          state.attached = true;
          await cdp(state.root.target, "Page.enable");
          state.domains.add("Page");
        }
        if (!state.prepared) {
          const generation = state.generation;
          await installer(state.root);
          await cdp(state.root.target, "Target.setAutoAttach", iframeAutoAttach);
          if (state.generation !== generation) throw new Error("The debugger session was released during setup.");
          state.prepared = true;
        }
        await waitForChildTasks(state);
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

    /**
     * Takes one lease on a tab, attaching and preparing the debugger session if it is not attached.
     * Every successful acquire must be paired with exactly one release. A failed acquire gives its
     * lease back and rejects, so the caller owes no release.
     */
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

    /**
     * Marks a tab as controlled and attaches it. A retained tab keeps its session, and its focus
     * emulation when enabled, until unretain, forget or detachAll, independent of leases.
     */
    async function retain(tabId) {
      const state = tabState(tabId);
      state.retained = true;
      await enqueue(tabId, state, () => ensureAttached(tabId, state));
    }

    /**
     * Ends control of a tab. Focus emulation is turned off, and the session detaches once no lease
     * and no open dialog remain. Does nothing for a tab that is not tracked.
     */
    async function unretain(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      state.retained = false;
      await enqueue(tabId, state, async () => {
        await syncFocus(tabId, state);
        await settle(tabId, state);
      });
    }

    /**
     * Sets whether controlled (retained) tabs emulate focus, and applies it to every attached tab now.
     * Rejects with an AggregateError of every tab that could not be updated; such a tab has released
     * its session rather than keep an override whose state is unknown.
     */
    async function setFocusEmulationEnabled(enabled) {
      focusEmulationEnabled = Boolean(enabled);
      const results = await Promise.allSettled(Array.from(tabs, ([tabId, state]) =>
        enqueue(tabId, state, () => syncFocus(tabId, state))));
      const errors = results.filter((result) => result.status === "rejected").map((result) => result.reason);
      if (errors.length) throw new AggregateError(errors, "Could not update controlled-tab focus.");
    }

    /**
     * Gives back one lease taken by acquire. When it was the last use (no lease, no retention, no
     * open dialog) the session detaches, awaiting onRelease first. Does nothing without a lease.
     */
    async function release(tabId) {
      const state = tabs.get(tabId);
      if (!state || state.leases === 0) return;
      state.leases -= 1;
      await enqueue(tabId, state, () => settle(tabId, state));
    }

    /**
     * Records that a JavaScript dialog is open on a tab, as
     * `{ type, message, defaultPrompt, url, sessionId }`. `defaultPrompt` is the text a prompt's
     * field starts with, empty for every other dialog. `url` is the url of the frame that raised
     * it, or empty when the browser did not say. An open dialog keeps the session attached until
     * closeDialog, so the dialog can still be answered.
     */
    function openDialog(tabId, { type, message = "", defaultPrompt = "", url = "", sessionId = null } = {}) {
      tabState(tabId).dialog = { type: type || DIALOG_UNKNOWN, message, defaultPrompt, url, sessionId };
    }

    /**
     * Clears the open dialog of a tab and detaches the session if nothing else needs it.
     * Does nothing for a tab that is not tracked.
     */
    async function closeDialog(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      state.dialog = null;
      await enqueue(tabId, state, () => settle(tabId, state));
    }

    /**
     * Returns a copy of the open dialog of a tab as `{ type, message, defaultPrompt, url, sessionId }`,
     * or null when none is open. Changing the copy does not change the recorded dialog.
     */
    function currentDialog(tabId) {
      const dialog = tabs.get(tabId)?.dialog;
      return dialog ? { ...dialog } : null;
    }

    /**
     * Enables an opt-in CDP domain on the root session, attaching first if needed. This is the only
     * path that sends `<domain>.enable`, Runtime.enable included. Resolves true when it enabled the
     * domain and false when the domain was already enabled.
     */
    async function enableDomain(tabId, domain) {
      const state = tabState(tabId);
      return enqueue(tabId, state, async () => {
        await ensureAttached(tabId, state);
        if (state.domains.has(domain)) return false;
        await cdp(state.root.target, `${domain}.enable`);
        state.domains.add(domain);
        return true;
      });
    }

    /**
     * Disables a domain enabled by enableDomain and forgets it, even when the command fails. The Page
     * domain stays enabled for the life of the attachment. Does nothing when the tab is not attached
     * or the domain is not enabled.
     */
    async function disableDomain(tabId, domain) {
      const state = tabs.get(tabId);
      if (!state || domain === "Page") return;
      await enqueue(tabId, state, async () => {
        if (!state.attached || !state.domains.has(domain)) return;
        try {
          await cdp(state.root.target, `${domain}.disable`);
        } finally {
          state.domains.delete(domain);
        }
      });
    }

    /**
     * Records that Chrome ended the attachment from outside (chrome.debugger.onDetach). It only
     * forgets the attachment's state: it sends nothing to Chrome and does not call onRelease. Leases,
     * retention and the open dialog are kept, so the next acquire attaches again.
     */
    function detached(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      clearAttachment(state);
      prune(tabId, state);
    }

    /**
     * Drops every record of a tab that closed. Like detached it sends nothing and does not call
     * onRelease, and it also discards leases, retention and the open dialog. Work already queued for
     * the tab fails rather than attach again.
     */
    function forget(tabId) {
      const state = tabs.get(tabId);
      if (state) {
        state.closing = true;
        clearAttachment(state);
      }
      tabs.delete(tabId);
    }

    /**
     * Releases every debugger session, awaiting onRelease for each and turning focus emulation off.
     * This is terminal: it clears leases, retention and open dialogs, and refuses queued attaches.
     * Rejects with an AggregateError when any session could not be detached; those tabs stay tracked
     * for a retry.
     */
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

    /**
     * Returns how many tabs currently have an attached debugger session.
     */
    function attachedCount() {
      return Array.from(tabs.values()).filter((state) => state.attached).length;
    }

    /**
     * Returns the root session of an attached tab, or null when the tab is not attached.
     */
    function rootSession(tabId) {
      const state = tabs.get(tabId);
      return state?.attached ? state.root : null;
    }

    /**
     * Returns the child (iframe) sessions currently attached under a tab, or an empty list.
     */
    function childSessions(tabId) {
      const state = tabs.get(tabId);
      return state?.attached ? Array.from(state.children.values()) : [];
    }

    /**
     * Visits the root and child sessions of every attached tab, a tab at a time and behind that
     * tab's pending lifecycle work. The visitor may use cdp() but must not call back into the
     * lifecycle. Every session is visited even when one visit fails; failures are then rejected
     * together as one AggregateError.
     */
    async function forEachSession(visit) {
      const results = await Promise.allSettled(Array.from(tabs, ([tabId, state]) =>
        state.attached ? enqueue(tabId, state, async () => {
          const errors = [];
          for (const session of [state.root, ...state.children.values()]) {
            try {
              await visit(session);
            } catch (error) {
              errors.push(error);
            }
          }
          if (errors.length) throw new AggregateError(errors, "Could not visit every debugger session.");
        }) : Promise.resolve()));
      const errors = results.filter((result) => result.status === "rejected").flatMap((result) => result.reason.errors ?? [result.reason]);
      if (errors.length) throw new AggregateError(errors, "Could not visit every debugger session.");
    }

    return Object.freeze({
      cdp,
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
      rootSession,
      childSessions,
      forEachSession
    });
  }

  return Object.freeze({ create, CDP_ALLOW, failure, dialogOpen, DIALOG_OPEN, DIALOG_OPEN_MESSAGE });
});
