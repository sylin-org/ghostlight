// Ghostlight -- the debugger sessions of adapter protocol 4, the only path to CDP.
(function installGhostlightSessions(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node ? { protocol: require("./protocol.js") } : { protocol: root.GhostlightProtocol });
  root.GhostlightSessions = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightSessionsApi(deps) {
  "use strict";

  const { protocol } = deps;

  // Debugger sessions for adapter protocol 4. This module owns every chrome.debugger attachment:
  // shared attach leases per tab, retained (controlled) tabs, focus emulation, the open JavaScript
  // dialog, opt-in CDP domains, and the registry of root and child (iframe) sessions with the
  // script registration identifiers recorded against each. It is the only path to CDP: every
  // internal call goes through cdp(), which refuses any method outside CDP_ALLOW. It knows nothing
  // about page code. The installer the worker supplies registers that code in each session.
  //
  // It also owns whether a child session is alive. The registry is a tree: each child remembers
  // the session that announced it, and endSession is the one place a child leaves the registry.
  // Chrome announces the end of a session attached through the root, but says nothing when a
  // session attached through a child ends with that child, so a child also ends with the session
  // that announced it and when Chrome answers a command to it with SESSION_UNKNOWN_TEXT.

  // Every CDP method the adapter may send. Runtime.enable is listed because enableDomain
  // (opt-in diagnostics) sends it; attaching a session never does.
  // Every CDP command Ghostlight sends, by name. Its values are the allow list: cdp() refuses
  // any other method, so a module names a command through this table or not at all.
  const CDP = Object.freeze({
    PAGE_ENABLE: "Page.enable",
    PAGE_GET_FRAME_TREE: "Page.getFrameTree",
    PAGE_ADD_SCRIPT_TO_EVALUATE_ON_NEW_DOCUMENT: "Page.addScriptToEvaluateOnNewDocument",
    PAGE_REMOVE_SCRIPT_TO_EVALUATE_ON_NEW_DOCUMENT: "Page.removeScriptToEvaluateOnNewDocument",
    PAGE_CREATE_ISOLATED_WORLD: "Page.createIsolatedWorld",
    PAGE_GET_LAYOUT_METRICS: "Page.getLayoutMetrics",
    PAGE_CAPTURE_SCREENSHOT: "Page.captureScreenshot",
    PAGE_HANDLE_JAVASCRIPT_DIALOG: "Page.handleJavaScriptDialog",
    PAGE_START_SCREENCAST: "Page.startScreencast",
    PAGE_STOP_SCREENCAST: "Page.stopScreencast",
    PAGE_SCREENCAST_FRAME_ACK: "Page.screencastFrameAck",
    PAGE_GET_NAVIGATION_HISTORY: "Page.getNavigationHistory",
    PAGE_NAVIGATE_TO_HISTORY_ENTRY: "Page.navigateToHistoryEntry",
    RUNTIME_EVALUATE: "Runtime.evaluate",
    RUNTIME_CALL_FUNCTION_ON: "Runtime.callFunctionOn",
    RUNTIME_RELEASE_OBJECT: "Runtime.releaseObject",
    RUNTIME_RELEASE_OBJECT_GROUP: "Runtime.releaseObjectGroup",
    RUNTIME_RUN_IF_WAITING_FOR_DEBUGGER: "Runtime.runIfWaitingForDebugger",
    RUNTIME_TERMINATE_EXECUTION: "Runtime.terminateExecution",
    RUNTIME_ENABLE: "Runtime.enable",
    RUNTIME_DISABLE: "Runtime.disable",
    NETWORK_ENABLE: "Network.enable",
    NETWORK_DISABLE: "Network.disable",
    TARGET_SET_AUTO_ATTACH: "Target.setAutoAttach",
    DOM_GET_FRAME_OWNER: "DOM.getFrameOwner",
    DOM_RESOLVE_NODE: "DOM.resolveNode",
    INPUT_DISPATCH_MOUSE_EVENT: "Input.dispatchMouseEvent",
    INPUT_DISPATCH_KEY_EVENT: "Input.dispatchKeyEvent",
    INPUT_INSERT_TEXT: "Input.insertText",
    INPUT_DISPATCH_DRAG_EVENT: "Input.dispatchDragEvent",
    INPUT_SET_INTERCEPT_DRAGS: "Input.setInterceptDrags",
    INPUT_CANCEL_DRAGGING: "Input.cancelDragging",
    EMULATION_SET_FOCUS_EMULATION_ENABLED: "Emulation.setFocusEmulationEnabled",
    ACCESSIBILITY_GET_FULL_AX_TREE: "Accessibility.getFullAXTree",
    ACCESSIBILITY_QUERY_AX_TREE: "Accessibility.queryAXTree"
  });
  const CDP_ALLOW = Object.freeze(Object.values(CDP));
  // Chrome's answer to a command for a child session it no longer knows. This text alone ends a
  // child at cdp(): it was measured on a session whose announcing session had ended (Chrome 157).
  // The other SESSION_LOST_TEXT alternates were not measured on a child, and a false end hides a
  // live frame for the rest of the attachment.
  const SESSION_UNKNOWN = "Session with given id not found";
  const SESSION_UNKNOWN_TEXT = new RegExp(SESSION_UNKNOWN, "i");
  // What a command pending on a child session is rejected with when that session ends first.
  const SESSION_ENDED = "frame's debugger session ended";
  const SESSION_ENDED_MESSAGE = `The ${SESSION_ENDED}.`;
  // The two texts that say one child session ended while the tab stayed attached. A world call
  // reads them as a changed document, not as lost control of the tab.
  const SESSION_ENDED_TEXT = new RegExp(`${SESSION_UNKNOWN}|${SESSION_ENDED}`, "i");
  // The texts in which Chrome or this module says a debugger session is gone, the two above
  // included. A receipt reads them all as `session_lost`. A world call drops its handle on any of
  // them, and reads all but those two as `session_lost`.
  const SESSION_LOST_TEXT = new RegExp(`Debugger is not attached|Detached while handling command|Target closed|No session with given id|debugger session was released|${SESSION_ENDED_TEXT.source}`, "i");
  const allowed = new Set(CDP_ALLOW);

  const EVENT_ATTACHED = "Target.attachedToTarget";
  const EVENT_DETACHED = "Target.detachedFromTarget";
  const TARGET_IFRAME = "iframe";

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
   * voluntary detach, unless the tab's page shows a JavaScript dialog and could not answer.
   * `onSessionGone(tabId, sessionId, dialogEnded)` fires once for every child session that ends
   * while the tab stays attached: on Chrome's detach event, with the session that announced it, or
   * on Chrome's answer that it no longer knows the session. `dialogEnded` is true for the session
   * whose open dialog was cleared with it. The end of the whole attachment does not fire it.
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
     * with a `browser_refused` failure and nothing reaches Chrome. A command to a registered child
     * session settles at once, rejected, when that session ends before Chrome answers. When Chrome
     * answers that it no longer knows the child, the child is ended here and the same error is
     * passed on. Only the session the command was sent to is ended: a late answer to a command of
     * a session that already ended ends nothing.
     */
    function cdp(target, method, params) {
      if (!allowed.has(method)) {
        return Promise.reject(protocol.failure(protocol.RECEIPT_FAILURE.BROWSER_REFUSED, `CDP method ${method} is not allowed`));
      }
      const record = target?.sessionId ? tabs.get(target.tabId)?.children.get(target.sessionId) : undefined;
      if (!record) return debuggerApi.sendCommand(target, method, params);
      let sent;
      try {
        sent = Promise.resolve(debuggerApi.sendCommand(target, method, params));
      } catch (error) {
        sent = Promise.reject(error);
      }
      return untilEnded(record, sent, (error) => {
        const current = tabs.get(target.tabId)?.children.get(target.sessionId);
        if (current === record && SESSION_UNKNOWN_TEXT.test(String(error?.message ?? error))) endSession(target.tabId, target.sessionId);
      });
    }

    // Settles as `work` does, or rejects at once when the child of `record` ends first. A rejection
    // of `work` is passed on unchanged, and `afterRejection` has run by the time the caller reads it.
    function untilEnded(record, work, afterRejection) {
      return new Promise((resolve, reject) => {
        const ended = () => reject(new Error(SESSION_ENDED_MESSAGE));
        record.waiters.add(ended);
        work.then(
          (value) => { record.waiters.delete(ended); resolve(value); },
          (error) => { record.waiters.delete(ended); reject(error); afterRejection?.(error); }
        );
      });
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
          dialogWaiters: new Set(),
          domains: new Set(),
          root: { target: Object.freeze({ tabId }), targetId: null, identifiers: [] },
          // Child session id -> { session, parent, waiters, setup, settled, failure }. `parent` is
          // the id of the session that announced the child, null for the root. `waiters` reject the
          // work pending on the child when it ends. `failure` is the error of its own setup.
          children: new Map()
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

    // Whether `sent`, a command the tab's page must answer, was answered before a JavaScript dialog
    // showed there. A page that shows a dialog answers nothing until the dialog closes, so lifecycle
    // work never waits on it: this resolves false at once when a dialog is open, or as soon as one
    // opens, and leaves the command to take effect when the dialog closes. A failed command rejects.
    function answeredBeforeDialog(state, sent) {
      let wake;
      const shown = state.dialog
        ? Promise.resolve()
        : new Promise((resolve) => { wake = resolve; state.dialogWaiters.add(wake); });
      return Promise.race([sent.then(() => true), shown.then(() => false)])
        .finally(() => state.dialogWaiters.delete(wake));
    }

    /**
     * Starts watching a tab for a JavaScript dialog. `opened` resolves with the dialog's type as
     * soon as one shows, or at once when one is already open; `forget()` ends the watch. A dialog
     * blocks the renderer, so in-flight work learns of it here instead of waiting for a reply
     * that cannot come.
     */
    function watchDialog(tabId) {
      const state = tabState(tabId);
      let wake;
      const opened = state.dialog
        ? Promise.resolve(state.dialog.type)
        : new Promise((resolve) => { wake = resolve; state.dialogWaiters.add(wake); });
      return Object.freeze({ opened, forget: () => state.dialogWaiters.delete(wake) });
    }

    /** Throws `dialog_open` when a JavaScript dialog blocks the tab's renderer. */
    function requireNoDialog(tabId) {
      if (tabs.get(tabId)?.dialog) throw protocol.dialogOpen();
    }

    // Forgets everything that belonged to one debugger attachment. Registrations and child
    // sessions die with the attachment, so their records go with it, and work pending on a child
    // settles. An open dialog is kept, since it can still be answered after a new attach, but it no
    // longer names a session: none of this attachment's sessions exists any more.
    function clearAttachment(state) {
      state.attached = false;
      state.prepared = false;
      state.focusEmulated = false;
      state.focusAttempted = false;
      state.generation += 1;
      state.domains.clear();
      state.root.identifiers.length = 0;
      const records = Array.from(state.children.values());
      state.children.clear();
      for (const record of records) settlePending(record);
      if (state.dialog) state.dialog.sessionId = null;
    }

    // Rejects everything pending on a child that has left the registry.
    function settlePending(record) {
      for (const ended of Array.from(record.waiters)) ended();
    }

    /**
     * Ends a child session and every session announced through it, while the tab stays attached.
     * This is the only way a child leaves the registry short of the whole attachment ending. Work
     * pending on an ended session is rejected, a dialog one of them raised is cleared, and
     * onSessionGone is called once per ended session. An id with no registered session, and none
     * announced through it, is ignored, and so is no id at all: the root is not a child, and the
     * sessions announced through it end with the attachment. Sends nothing to Chrome.
     */
    function endSession(tabId, sessionId) {
      const state = tabs.get(tabId);
      if (!state || !sessionId) return;
      const ending = new Set([sessionId]);
      // A fixpoint, so the order the sessions were announced in does not matter.
      for (let grew = true; grew;) {
        grew = false;
        for (const [id, record] of state.children) {
          if (!ending.has(id) && ending.has(record.parent)) {
            ending.add(id);
            grew = true;
          }
        }
      }
      const ended = Array.from(state.children).filter(([id]) => ending.has(id));
      if (ended.length === 0) return;
      for (const [id] of ended) state.children.delete(id);
      const dialogSession = state.dialog && ending.has(state.dialog.sessionId) ? state.dialog.sessionId : null;
      // Nothing can answer a dialog whose session is gone, and while it is recorded every read of
      // the tab refuses and the session never detaches.
      if (dialogSession !== null) dropDialog(tabId, state).catch(() => {});
      for (const [id, record] of ended) {
        settlePending(record);
        try {
          onSessionGone?.(tabId, id, id === dialogSession);
        } catch (_error) { /* a failing hook must not keep the other sessions registered */ }
      }
    }

    // Waits until every registered child is set up, then throws the setup failure of a child that
    // is still registered. A failure belongs to its child and leaves with it. A setup the page
    // cannot answer because it shows a dialog is not waited on: the tab's queue stays free for the
    // call that answers the dialog, and the next attach waits for the setup again.
    async function waitForChildren(state) {
      for (;;) {
        const pending = Array.from(state.children.values()).filter((record) => !record.settled);
        if (pending.length === 0) break;
        if (!await answeredBeforeDialog(state, Promise.all(pending.map((record) => record.setup)))) return;
      }
      for (const record of state.children.values()) {
        if (record.failure) throw record.failure;
      }
    }

    // A child frame attaches paused. It is given its own Page domain and registrations, and set to
    // auto-attach its own iframes (their attach events arrive here again). It resumes last, even
    // when setup failed, so a page is never left paused. A child that ended has nothing to resume.
    async function setUpChild(state, record, waitingForDebugger) {
      const child = record.session;
      try {
        await cdp(child.target, CDP.PAGE_ENABLE);
        await installer(child);
        await cdp(child.target, CDP.TARGET_SET_AUTO_ATTACH, iframeAutoAttach);
      } finally {
        if (waitingForDebugger && state.children.get(child.target.sessionId) === record) {
          await cdp(child.target, CDP.RUNTIME_RUN_IF_WAITING_FOR_DEBUGGER);
        }
      }
    }

    // Registers a child before its setup starts, so its own commands and the sessions it announces
    // find it. A child whose announcing session is not registered is still set up, so a paused
    // frame is resumed; if it is dead, Chrome's answer to its first command ends it.
    function recordChild(tabId, state, parent, params) {
      const sessionId = params.sessionId;
      endSession(tabId, sessionId);
      const record = {
        session: {
          target: Object.freeze({ tabId, sessionId }),
          targetId: params.targetInfo.targetId ?? null,
          identifiers: []
        },
        parent,
        waiters: new Set(),
        setup: null,
        settled: false,
        failure: null
      };
      state.children.set(sessionId, record);
      record.setup = untilEnded(record, setUpChild(state, record, Boolean(params.waitingForDebugger)))
        .catch((error) => {
          if (state.children.get(sessionId) === record) record.failure = error;
        })
        .then(() => { record.settled = true; });
    }

    debuggerApi.onEvent.addListener((source, method, params) => {
      const state = source?.tabId === undefined ? undefined : tabs.get(source.tabId);
      if (!state?.attached || !params?.sessionId) return;
      if (method === EVENT_ATTACHED && params.targetInfo?.type === TARGET_IFRAME) {
        recordChild(source.tabId, state, source.sessionId ?? null, params);
      } else if (method === EVENT_DETACHED) {
        endSession(source.tabId, params.sessionId);
      }
    });

    // A page that shows a dialog is not waited on here: the detach that follows ends what its page
    // code and its focus override would have been told.
    async function detachSession(tabId, state) {
      if (!state.attached) return;
      try {
        await answeredBeforeDialog(state, Promise.resolve(onRelease?.(tabId)));
      } catch (_error) { /* releasing page code is best effort; detachment ends the session anyway */ }
      if (state.focusEmulated || state.focusAttempted) {
        try {
          if (await answeredBeforeDialog(state, cdp({ tabId }, CDP.EMULATION_SET_FOCUS_EMULATION_ENABLED, { enabled: false }))) {
            state.focusEmulated = false;
            state.focusAttempted = false;
          }
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

    // Brings the tab's focus override to what control asks for. A tab that shows a dialog is left as
    // it is until closeDialog runs this again: its page runs nothing meanwhile, so nothing on it can
    // tell. A command a dialog left unanswered may still take effect, so it never counts as done.
    async function syncFocus(tabId, state) {
      while (state.attached) {
        const enabled = focusEmulationEnabled && state.retained && !state.closing;
        if (state.focusEmulated === enabled && !state.focusAttempted) return;
        if (state.dialog) return;
        const generation = state.generation;
        state.focusAttempted = true;
        let answered;
        try {
          answered = await answeredBeforeDialog(state, cdp({ tabId }, CDP.EMULATION_SET_FOCUS_EMULATION_ENABLED, { enabled }));
        } catch (error) {
          // A failed command may have taken effect. Release the session rather than
          // leave an override whose state we cannot confirm.
          await detachSession(tabId, state);
          throw error;
        }
        if (!answered || state.generation !== generation) return;
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
          await cdp(state.root.target, CDP.PAGE_ENABLE);
          state.domains.add("Page");
        }
        if (!state.prepared) {
          const generation = state.generation;
          await installer(state.root);
          await cdp(state.root.target, CDP.TARGET_SET_AUTO_ATTACH, iframeAutoAttach);
          if (state.generation !== generation) throw new Error("The debugger session was released during setup.");
          state.prepared = true;
        }
        await waitForChildren(state);
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
     * Runs `work()` inside one lease on a tab, and gives the lease back however `work` ends.
     * Resolves or rejects as `work` does. A failed acquire runs nothing and owes no release.
     */
    async function withLease(tabId, work) {
      await acquire(tabId);
      try {
        return await work();
      } finally {
        await release(tabId);
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
     * A tab that shows a JavaScript dialog takes the change when the dialog closes, so nothing waits
     * on its page (ADR-0168 amendment of 2026-10-20). Rejects with an AggregateError of every tab
     * that could not be updated; such a tab has released its session rather than keep an override
     * whose state is unknown.
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
     * closeDialog, so the dialog can still be answered. Lifecycle work waiting on the tab's page
     * stops waiting. `sessionId` is null for the root or names a registered child: a dialog from a
     * child that is not registered is not recorded, since its session has ended and nothing could
     * answer it. Returns whether the dialog was recorded.
     */
    function openDialog(tabId, { type, message = "", defaultPrompt = "", url = "", sessionId = null } = {}) {
      if (sessionId !== null && !tabs.get(tabId)?.children.has(sessionId)) return false;
      const state = tabState(tabId);
      state.dialog = { type: type || protocol.UNKNOWN_DIALOG_TYPE, message, defaultPrompt, url, sessionId };
      for (const wake of state.dialogWaiters) wake(state.dialog.type);
      return true;
    }

    // Forgets the open dialog, brings the focus override to what control asks for now, and detaches
    // the session if nothing else needs it.
    function dropDialog(tabId, state) {
      state.dialog = null;
      return enqueue(tabId, state, async () => {
        // A focus command that fails has already released the session; the dialog is gone either way.
        await syncFocus(tabId, state).catch(() => {});
        await settle(tabId, state);
      });
    }

    /**
     * Clears the open dialog of a tab, brings its focus override to what control asks for now, and
     * detaches the session if nothing else needs it. Does nothing for a tab that is not tracked.
     */
    async function closeDialog(tabId) {
      const state = tabs.get(tabId);
      if (!state) return;
      await dropDialog(tabId, state);
    }

    /**
     * Returns a copy of the open dialog of a tab as `{ type, message, defaultPrompt, url, sessionId }`,
     * or null when none is open. Changing the copy does not change the recorded dialog. `sessionId`
     * is null or names a child session that is registered now.
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
     * or the domain is not enabled. A page that shows a dialog takes the command when it closes,
     * and nothing waits for that.
     */
    async function disableDomain(tabId, domain) {
      const state = tabs.get(tabId);
      if (!state || domain === "Page") return;
      await enqueue(tabId, state, async () => {
        if (!state.attached || !state.domains.has(domain)) return;
        try {
          await answeredBeforeDialog(state, cdp(state.root.target, `${domain}.disable`));
        } finally {
          state.domains.delete(domain);
        }
      });
    }

    /**
     * Records that Chrome ended the attachment from outside (chrome.debugger.onDetach). It only
     * forgets the attachment's state: it sends nothing to Chrome and does not call onRelease. Leases,
     * retention and the open dialog are kept, so the next acquire attaches again. The dialog no
     * longer names a child session, so it is answered on the root of the next attachment.
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
     * A tab that shows a dialog is detached without waiting on its page. Rejects with an
     * AggregateError when any session could not be detached; those tabs stay tracked for a retry.
     */
    async function detachAll() {
      const results = await Promise.allSettled(Array.from(tabs, async ([tabId, state]) => {
        state.retained = false;
        state.closing = true;
        state.leases = 0;
        try {
          await enqueue(tabId, state, () => detachSession(tabId, state));
        } finally {
          // Cleared only now, so the detach knows not to wait on a page that shows a dialog.
          state.dialog = null;
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
      return state?.attached ? Array.from(state.children.values(), (record) => record.session) : [];
    }

    /**
     * Visits the root and child sessions of every attached tab, a tab at a time and behind that
     * tab's pending lifecycle work. A tab that shows a JavaScript dialog, or comes to show one
     * during its visit, is passed over from then on, since its page would answer nothing until the
     * dialog closes. The visitor may use cdp() but must not take or give back a lease. A child that
     * ends during the visits, as a visitor's own command can show, is passed over. Every other
     * session is visited even when one visit fails; failures are then rejected together as one
     * AggregateError.
     */
    async function forEachSession(visit) {
      const results = await Promise.allSettled(Array.from(tabs, ([tabId, state]) =>
        state.attached ? enqueue(tabId, state, async () => {
          const errors = [];
          for (const record of [null, ...state.children.values()]) {
            if (state.dialog) break;
            if (record && state.children.get(record.session.target.sessionId) !== record) continue;
            const session = record ? record.session : state.root;
            try {
              await answeredBeforeDialog(state, Promise.resolve().then(() => visit(session)));
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
      withLease,
      retain,
      unretain,
      setFocusEmulationEnabled,
      release,
      openDialog,
      closeDialog,
      currentDialog,
      watchDialog,
      requireNoDialog,
      enableDomain,
      disableDomain,
      detached,
      endSession,
      forget,
      detachAll,
      attachedCount,
      rootSession,
      childSessions,
      forEachSession
    });
  }

  return Object.freeze({ create, CDP, CDP_ALLOW, SESSION_LOST_TEXT, SESSION_ENDED_TEXT });
});
