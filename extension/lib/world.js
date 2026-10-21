// Ghostlight -- the isolated world of adapter protocol 4, extension side.
(function installGhostlightWorld(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), sessions: require("./sessions.js"), shared: require("./shared.js") }
    : { protocol: root.GhostlightProtocol, sessions: root.GhostlightSessions, shared: root.GhostlightShared });
  root.GhostlightWorld = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightWorldApi(deps) {
  "use strict";

  const { protocol, sessions, shared } = deps;

  // The service's page assets run in the `ghostlight` isolated world of every document the
  // adapter touches. This module registers them in every debugger session, reaches each world
  // through an objectId handle of its global, and calls it with fixed shims. It never builds or
  // reads a world message: the message and the envelope that answers it are opaque here. Every CDP
  // call goes through the sessions cdp(). Runtime.enable is never sent, and Runtime.callFunctionOn
  // never carries an executionContextId: numeric context ids can be reused by another renderer
  // process, while an objectId fails closed in a foreign isolate.

  const WORLD = protocol.WORLD_NAME;
  const MESSAGE_CHARS = 500;
  const ASSET_ID = /^[0-9a-f]{64}$/;
  const RUNTIME_MISSING = "runtime_missing";

  // Fixed strings, compiled by the inspector and called on an objectId: no eval, no page CSP.
  const SHIMS = Object.freeze({
    CALL: `async function(id, message, local, token, attachment, owned) {
    const rt = globalThis.__ghostlightRuntime;
    if (!rt || rt.id !== id) return { ok: false, error: { code: "runtime_missing", message: "Ghostlight page code is not installed in this document." } };
    return await rt.call(message, { local, token, attachment, owner: owned ? this : null }); }`,
    CANCEL: "function(token) { return globalThis.__ghostlightRuntime?.cancel(token) ?? false; }",
    AMBIENT: "function(id, local) { const rt = globalThis.__ghostlightRuntime; return rt?.id === id ? rt.ambient(local) : false; }",
    PRESENT: "function(id, signal, local) { const rt = globalThis.__ghostlightRuntime; return rt?.id === id ? rt.present(signal, local) : false; }",
    DISPOSE: "function() { globalThis.__ghostlightRuntime?.dispose(); return true; }",
    DPR: "function() { return globalThis.devicePixelRatio; }",
    DOC: "function() { return document; }"
  });

  const METHOD = Object.freeze({
    ADD_SCRIPT: "Page.addScriptToEvaluateOnNewDocument",
    REMOVE_SCRIPT: "Page.removeScriptToEvaluateOnNewDocument",
    CREATE_WORLD: "Page.createIsolatedWorld",
    EVALUATE: "Runtime.evaluate",
    CALL_FUNCTION: "Runtime.callFunctionOn",
    RELEASE_OBJECT: "Runtime.releaseObject",
    RELEASE_GROUP: "Runtime.releaseObjectGroup",
    FRAME_OWNER: "DOM.getFrameOwner",
    RESOLVE_NODE: "DOM.resolveNode"
  });

  const CALL = Object.freeze({
    DOCUMENT_CHANGED: "document_changed",
    FRAME_MISSING: "frame_missing",
    OWNER_MISSING: "owner_missing",
    DIALOG_OPEN: sessions.DIALOG_OPEN,
    TIMEOUT: "timeout",
    CANCELLED: "cancelled",
    SESSION_LOST: "session_lost",
    PAGE_EXCEPTION: "page_exception",
    UNSERIALIZABLE: "unserializable",
    OTHER: "other"
  });

  const ANSWER = Object.freeze({ RETURNED: "returned", FAILED: "failed", SKIPPED: "skipped" });
  const MODE = Object.freeze({ SEQUENCE: "sequence", PARALLEL: "parallel", RACE: "race" });
  const SKIPPED = Object.freeze({ status: ANSWER.SKIPPED });
  const DIALOG_UNKNOWN = "unknown";
  const RUNTIME_DISCONNECTED = "disconnected";

  // The closed mapping from CDP error text to a call failure. `started` says whether the page may
  // have run the message; `dropHandle` says the cached world handle is dead.
  const CALL_ERRORS = Object.freeze([
    { pattern: /Cannot find context with specified id|Could not find object with given id|No frame with given id found/i,
      code: CALL.DOCUMENT_CHANGED, started: false, dropHandle: true },
    { pattern: /Execution context was destroyed|Inspected target navigated or closed/i,
      code: CALL.DOCUMENT_CHANGED, started: true, dropHandle: true },
    { pattern: /Debugger is not attached|Detached while handling command|Target closed|No session with given id/i,
      code: CALL.SESSION_LOST, started: true, dropHandle: true },
    { pattern: /Object couldn't be returned by value/i, code: CALL.UNSERIALIZABLE, started: true, dropHandle: false }
  ]);

  /** Wraps the asset source as the function the registration and the lazy installer call with the asset id. */
  function wrap(source) {
    return `function(__ghostlightAssetId) {\n${source}\n}`;
  }

  /** Builds the registration script: the wrapped source applied to its own id. */
  function registration(assets) {
    return `(${wrap(assets.source)})(${JSON.stringify(assets.id)});`;
  }

  /** Builds a call failure: an Error carrying a protocol 4 call failure code and whether the call started. */
  function callFailure(code, started, message) {
    return Object.assign(new Error(message ?? code), { callCode: code, started });
  }

  const CALL_CODES = new Set(Object.values(CALL));

  /**
   * Maps an error raised while calling a world to `{ code, started, dropHandle }` through the closed
   * CALL_ERRORS table. A call failure keeps its own code and start. A coded failure from
   * `sessions.failure` (such as the `dialog_open` a frame check throws) keeps its code when that
   * code is also a call failure, and counts as started like any failure that does not say;
   * anything else is `other`, started.
   */
  function mapCallError(error) {
    if (error?.callCode) return { code: error.callCode, started: Boolean(error.started), dropHandle: false };
    if (CALL_CODES.has(error?.code)) return { code: error.code, started: true, dropHandle: false };
    const text = String(error?.message ?? error);
    const known = CALL_ERRORS.find((entry) => entry.pattern.test(text));
    if (known) return { code: known.code, started: known.started, dropHandle: known.dropHandle };
    return { code: CALL.OTHER, started: true, dropHandle: false };
  }

  /** True when a call answer is a returned envelope that says `ok: true`. */
  function okAnswer(answer) {
    return answer?.status === ANSWER.RETURNED && answer.envelope?.ok === true;
  }

  /** True when a call answer may have run page code: it returned, or it failed after it started. */
  function started(answer) {
    return answer?.status === ANSWER.RETURNED || (answer?.status === ANSWER.FAILED && answer.started === true);
  }

  function failed(code, startedFlag, message) {
    return { status: ANSWER.FAILED, code, message: shared.bounded(message ?? "", MESSAGE_CHARS), started: startedFlag };
  }

  function describe(exceptionDetails) {
    return exceptionDetails?.exception?.description ?? exceptionDetails?.text ?? "The page threw an exception.";
  }

  async function sha256Hex(subtle, text) {
    const digest = await subtle.digest("SHA-256", new TextEncoder().encode(text));
    return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
  }

  /**
   * Creates the world registry.
   *
   * `lifecycle` is a GhostlightSessions instance and `inventory` a GhostlightInventory instance over
   * it. Pass `register` as the lifecycle installer, `disposeTab` as its onRelease and `sessionGone`
   * as its onSessionGone. `acquire` and `release` take and give back a debugger lease (default: the
   * lifecycle's). `isCancelled(correlation)` reports a cancel frame. `local` supplies the
   * extension-owned facts: `preferences()`, `managed(tabId)`, `recording(tabId)` and `runtime()`.
   */
  function create({
    lifecycle,
    inventory,
    subtle = globalThis.crypto?.subtle,
    acquire = (tabId) => lifecycle.acquire(tabId),
    release = (tabId) => lifecycle.release(tabId),
    isCancelled = () => false,
    local = {},
    now = () => Date.now(),
    sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  } = {}) {
    if (!lifecycle?.cdp || !inventory?.snapshot) throw new TypeError("the world requires debugger sessions and an inventory");
    if (!subtle?.digest) throw new TypeError("the world requires crypto.subtle");
    const cdp = lifecycle.cdp;

    let assets = null;
    const handles = new Map();
    const inflight = new Map();
    const cancelledTokens = new Set();
    const hiddenHolds = new Map();
    const dialogWaiters = new Map();

    const handleKey = (tabId, ref) => `${tabId}|${ref.frame_id}|${ref.loader_id}`;
    const tabPrefix = (tabId) => `${tabId}|`;

    function tabHandles(tabId) {
      const prefix = tabPrefix(tabId);
      return Array.from(handles).filter(([key]) => key.startsWith(prefix));
    }

    function dropHandle(tabId, ref) {
      if (ref) handles.delete(handleKey(tabId, ref));
    }

    /**
     * Stores the page assets from `hello_accepted` when `sha256(source)` equals `id`, then registers
     * them on every live session. Resolves false, and keeps the previous assets, on a checksum or
     * shape mismatch. The same id again is a no-op.
     */
    async function setAssets(next) {
      if (typeof next?.source !== "string" || typeof next?.id !== "string" || !ASSET_ID.test(next.id)) return false;
      if (await sha256Hex(subtle, next.source) !== next.id) return false;
      if (assets?.id === next.id) return true;
      assets = Object.freeze({ id: next.id, source: next.source });
      handles.clear();
      try {
        await lifecycle.forEachSession(register);
      } catch (_error) { /* a document this missed installs lazily on its first call */ }
      return true;
    }

    /** Returns the id of the stored assets, or null before the first `hello_accepted`. */
    function assetsId() {
      return assets?.id ?? null;
    }

    /**
     * The sessions installer: removes the session's superseded registrations, then registers the
     * current assets in the `ghostlight` world, running them at once in existing documents.
     * Does nothing before assets arrive.
     */
    async function register(session) {
      if (!assets) return;
      for (const identifier of session.identifiers.splice(0)) {
        await cdp(session.target, METHOD.REMOVE_SCRIPT, { identifier }).catch(() => {});
      }
      const { identifier } = await cdp(session.target, METHOD.ADD_SCRIPT,
        { source: registration(assets), worldName: WORLD, runImmediately: true });
      session.identifiers.push(identifier);
    }

    // The objectId of a document's world global. The numeric context id lives only between world
    // creation and the loader re-check that proves the document is still the one asked for.
    async function worldHandle(tabId, ref, snap) {
      const target = ref ?? snap.top;
      if (!target) throw callFailure(CALL.FRAME_MISSING, false, "The tab has no top document.");
      const key = handleKey(tabId, target);
      const known = handles.get(key);
      if (known) return known;
      const owner = snap.owner.get(target.frame_id);
      if (!owner) throw callFailure(CALL.FRAME_MISSING, false, "The frame is not in the tab.");
      if (owner.loaderId !== target.loader_id) throw callFailure(CALL.DOCUMENT_CHANGED, false, "The frame holds another document.");
      const { executionContextId } = await cdp(owner.session, METHOD.CREATE_WORLD, { frameId: target.frame_id, worldName: WORLD });
      const { result } = await cdp(owner.session, METHOD.EVALUATE,
        { expression: "globalThis", contextId: executionContextId, objectGroup: WORLD });
      if (await inventory.currentLoader(owner.session, target.frame_id) !== target.loader_id) {
        throw callFailure(CALL.DOCUMENT_CHANGED, false, "The frame changed document while its world was created.");
      }
      const handle = Object.freeze({ session: owner.session, global: result.objectId, ref: target });
      handles.set(key, handle);
      return handle;
    }

    // The owner element of a child frame, resolved in the parent's world by exact frame identity.
    async function ownerObject(tabId, ref, childFrameId, snap) {
      const handle = await worldHandle(tabId, ref, snap);
      const { executionContextId } = await cdp(handle.session, METHOD.CREATE_WORLD, { frameId: handle.ref.frame_id, worldName: WORLD });
      let owner;
      try {
        owner = await cdp(handle.session, METHOD.FRAME_OWNER, { frameId: childFrameId });
      } catch (_error) {
        throw callFailure(CALL.OWNER_MISSING, false, "The frame has no owner element in this document.");
      }
      const { object } = await cdp(handle.session, METHOD.RESOLVE_NODE,
        { backendNodeId: owner.backendNodeId, executionContextId, objectGroup: WORLD });
      if (await inventory.currentLoader(handle.session, handle.ref.frame_id) !== handle.ref.loader_id) {
        cdp(handle.session, METHOD.RELEASE_OBJECT, { objectId: object.objectId }).catch(() => {});
        throw callFailure(CALL.DOCUMENT_CHANGED, false, "The frame changed document while its owner was resolved.");
      }
      return { handle, objectId: object.objectId };
    }

    function track(correlation, entry) {
      if (!inflight.has(correlation)) inflight.set(correlation, new Set());
      inflight.get(correlation).add(entry);
    }

    function untrack(correlation, entry) {
      const entries = inflight.get(correlation);
      if (!entries) return;
      entries.delete(entry);
      if (entries.size === 0) inflight.delete(correlation);
    }

    /** Returns the in-flight world calls of a primitive as `{ session, global, token }` entries. */
    function inflightOf(correlation) {
      return Array.from(inflight.get(correlation) ?? []);
    }

    /**
     * Marks one in-flight entry's token cancelled, so a call that has not reached the page yet never
     * starts, and sends CANCEL best effort without waiting: a busy renderer may never answer it.
     */
    function cancelToken(entry) {
      cancelledTokens.add(entry.token);
      cdp(entry.session, METHOD.CALL_FUNCTION,
        { objectId: entry.global, functionDeclaration: SHIMS.CANCEL, arguments: [{ value: entry.token }] }).catch(() => {});
    }

    /**
     * Starts watching a tab for a JavaScript dialog. `opened` resolves with the dialog type when
     * announceDialog reports one; `forget()` ends the watch. A dialog blocks the renderer, so an
     * in-flight command learns of it here instead of waiting for a reply that cannot come.
     */
    function watchForDialog(tabId) {
      let announce;
      const opened = new Promise((resolve) => { announce = resolve; });
      if (!dialogWaiters.has(tabId)) dialogWaiters.set(tabId, new Set());
      dialogWaiters.get(tabId).add(announce);
      return {
        opened,
        forget() {
          const waiting = dialogWaiters.get(tabId);
          if (!waiting) return;
          waiting.delete(announce);
          if (waiting.size === 0) dialogWaiters.delete(tabId);
        }
      };
    }

    /** Reports a dialog that opened on a tab to every watch of that tab. */
    function announceDialog(tabId, type) {
      const waiting = dialogWaiters.get(tabId);
      if (!waiting) return;
      dialogWaiters.delete(tabId);
      for (const resolve of waiting) resolve(type || DIALOG_UNKNOWN);
    }

    // Races a started CDP call with the tab's dialog watch and the deadline. On the deadline the
    // cancel thunk fires and the call answers timeout at once; both outcomes count as started.
    async function raceDialogAndDeadline(tabId, deadline, call, cancel) {
      call.catch(() => {});
      const dialog = watchForDialog(tabId);
      let timer = null;
      const expired = new Promise((resolve) => {
        if (Number.isFinite(deadline)) timer = setTimeout(resolve, Math.max(0, deadline - now()));
      });
      try {
        const winner = await Promise.race([
          call.then((reply) => ({ reply })),
          dialog.opened.then(() => ({ dialog: true })),
          expired.then(() => ({ expired: true }))
        ]);
        if (winner.dialog) throw callFailure(CALL.DIALOG_OPEN, true, "A JavaScript dialog opened during the call.");
        if (winner.expired) {
          cancel();
          throw callFailure(CALL.TIMEOUT, true, "The call did not finish before its deadline.");
        }
        return winner.reply;
      } finally {
        clearTimeout(timer);
        dialog.forget();
      }
    }

    /**
     * Returns the extension-owned facts every world call and AMBIENT carries: preferences, whether
     * the tab is managed, whether it is recording, the runtime control state, and whether a capture
     * hold hides Ghostlight's own presentation.
     */
    function localFacts(tabId) {
      const preferences = local.preferences?.() ?? {};
      return {
        preferences: { effects: preferences.effects !== false, captions: preferences.captions !== false },
        managed: Boolean(local.managed?.(tabId)),
        recording: Boolean(local.recording?.(tabId)),
        runtime: local.runtime?.() ?? RUNTIME_DISCONNECTED,
        hidden: (hiddenHolds.get(tabId) ?? 0) > 0
      };
    }

    /** Takes one capture hold on a tab: localFacts reports `hidden` while any hold exists. */
    function holdHidden(tabId) {
      hiddenHolds.set(tabId, (hiddenHolds.get(tabId) ?? 0) + 1);
    }

    /** Gives back one capture hold taken by holdHidden. */
    function releaseHidden(tabId) {
      const count = (hiddenHolds.get(tabId) ?? 0) - 1;
      if (count > 0) hiddenHolds.set(tabId, count);
      else hiddenHolds.delete(tabId);
    }

    /**
     * Runs one world call and answers it as a call answer: `returned` with the unparsed envelope, or
     * `failed` with a call failure code and whether the page may have run it. A world without the
     * current runtime is installed lazily, exactly once, and the call is retried once.
     * `ctx` is `{ correlation, token, deadline, snap, attachment }`.
     */
    async function callOne(tabId, call, ctx) {
      try {
        return await attemptCall(tabId, call, ctx);
      } finally {
        cancelledTokens.delete(ctx.token);
      }
    }

    async function attemptCall(tabId, call, ctx) {
      if (!assets) return failed(CALL.OTHER, false, "No page assets have been received.");
      if (lifecycle.currentDialog(tabId)) return failed(CALL.DIALOG_OPEN, false, sessions.DIALOG_OPEN_MESSAGE);
      let reinstalled = false;
      for (;;) {
        let handle;
        let target;
        try {
          if (call.owner_of) ({ handle, objectId: target } = await ownerObject(tabId, call.frame ?? null, call.owner_of, ctx.snap));
          else {
            handle = await worldHandle(tabId, call.frame ?? null, ctx.snap);
            target = handle.global;
          }
        } catch (error) {
          const mapped = mapCallError(error);
          if (mapped.dropHandle) dropHandle(tabId, call.frame ?? ctx.snap.top);
          return failed(mapped.code, false, error?.message);
        }
        const entry = { session: handle.session, global: handle.global, token: ctx.token };
        track(ctx.correlation, entry);
        try {
          if (cancelledTokens.has(ctx.token) || isCancelled(ctx.correlation)) {
            return failed(CALL.CANCELLED, false, "The call was cancelled before it started.");
          }
          const sent = cdp(handle.session, METHOD.CALL_FUNCTION, {
            objectId: target,
            functionDeclaration: SHIMS.CALL,
            awaitPromise: true,
            returnByValue: true,
            arguments: [assets.id, call.message, localFacts(tabId), ctx.token, ctx.attachment ?? null, Boolean(call.owner_of)]
              .map((value) => ({ value }))
          });
          const reply = await raceDialogAndDeadline(tabId, ctx.deadline, sent, () => cancelToken(entry));
          if (reply?.exceptionDetails) return failed(CALL.PAGE_EXCEPTION, true, describe(reply.exceptionDetails));
          const envelope = reply?.result?.value ?? null;
          if (envelope?.error?.code === RUNTIME_MISSING && !reinstalled) {
            reinstalled = true;
            try {
              await cdp(handle.session, METHOD.CALL_FUNCTION,
                { objectId: handle.global, functionDeclaration: wrap(assets.source), arguments: [{ value: assets.id }] });
            } catch (error) {
              const mapped = mapCallError(error);
              if (mapped.dropHandle) dropHandle(tabId, handle.ref);
              return failed(mapped.code, false, error?.message);
            }
            continue;
          }
          return { status: ANSWER.RETURNED, envelope };
        } catch (error) {
          const mapped = mapCallError(error);
          if (mapped.dropHandle) dropHandle(tabId, handle.ref);
          return failed(mapped.code, mapped.started, error?.message);
        } finally {
          untrack(ctx.correlation, entry);
          if (call.owner_of) cdp(handle.session, METHOD.RELEASE_OBJECT, { objectId: target }).catch(() => {});
        }
      }
    }

    // Runs every call at once and completes at the first ok answer. Calls still pending then are
    // cancelled by token and reported skipped; their late answers are discarded.
    async function race(tabId, calls, contexts) {
      const answers = new Array(calls.length).fill(undefined);
      if (calls.length === 0) return answers;
      let finish;
      const done = new Promise((resolve) => { finish = resolve; });
      let remaining = calls.length;
      calls.forEach((call, index) => {
        callOne(tabId, call, contexts[index])
          .catch((error) => failed(CALL.OTHER, true, error?.message))
          .then((answer) => {
            if (answers[index] !== undefined) return;
            answers[index] = answer;
            remaining -= 1;
            if (okAnswer(answer) || remaining === 0) finish();
          });
      });
      await done;
      const losers = new Set();
      answers.forEach((answer, index) => {
        if (answer !== undefined) return;
        answers[index] = SKIPPED;
        losers.add(contexts[index].token);
      });
      for (const token of losers) cancelledTokens.add(token);
      const pending = Array.from(inflight.values(), (entries) => Array.from(entries)).flat()
        .filter((entry) => losers.has(entry.token));
      pending.forEach(cancelToken);
      return answers;
    }

    function pruneHandles(tabId, snap) {
      for (const [key, handle] of tabHandles(tabId)) {
        const now = snap?.owner.get(handle.ref.frame_id);
        if (!now || now.loaderId !== handle.ref.loader_id) handles.delete(key);
      }
    }

    /**
     * The `call` primitive: snapshot the tab, check the guard, then run the calls in `sequence`
     * (in order, skipping after the first answer that is not ok), `parallel` (all at once), or
     * `race` (first ok wins). `fx.dispatched` is true when any call started. A cancelled primitive
     * throws `cancelled` with those facts kept.
     */
    async function callPrimitive(args, fx, correlation) {
      await acquire(args.tab_id);
      let snap = null;
      try {
        snap = await inventory.snapshot(args.tab_id);
        inventory.checkGuard(args.guard, snap);
        fx.watch(args.tab_id);
        const deadline = now() + args.timeout_ms;
        const contexts = args.calls.map((_call, index) =>
          ({ correlation, token: `${correlation}:${index}`, deadline, snap, attachment: null }));
        let answers;
        if (args.mode === MODE.SEQUENCE) {
          answers = [];
          for (const [index, call] of args.calls.entries()) {
            const skip = answers.some((answer) => !okAnswer(answer)) || isCancelled(correlation);
            answers.push(skip ? SKIPPED : await callOne(args.tab_id, call, contexts[index]));
          }
        } else if (args.mode === MODE.PARALLEL) {
          answers = await Promise.all(args.calls.map((call, index) => callOne(args.tab_id, call, contexts[index])));
        } else {
          answers = await race(args.tab_id, args.calls, contexts);
        }
        fx.dispatched = answers.some(started);
        if (isCancelled(correlation)) throw sessions.failure(CALL.CANCELLED, "The request was cancelled.");
        if (args.settle_ms) await sleep(args.settle_ms);
        return { answers };
      } finally {
        pruneHandles(args.tab_id, snap);
        await release(args.tab_id);
      }
    }

    // The top world of an attached tab, or null. Never attaches.
    async function topHandle(tabId) {
      if (!lifecycle.rootSession(tabId)) return null;
      return worldHandle(tabId, null, await inventory.topSnapshot(tabId));
    }

    async function callShim(handle, functionDeclaration, args, options = {}) {
      return cdp(handle.session, METHOD.CALL_FUNCTION, {
        objectId: handle.global,
        functionDeclaration,
        arguments: args.map((value) => ({ value })),
        returnByValue: true,
        ...options
      });
    }

    /**
     * Sends AMBIENT (the local facts, overlaid with `patch`) to the top world and every cached world
     * of a tab, best effort. Never attaches and never installs. Resolves true when any world took it.
     */
    async function ambientAll(tabId, patch = {}) {
      // A dialog blocks every world of the tab, so none can be told until it is handled.
      if (!assets || lifecycle.currentDialog(tabId)) return false;
      const facts = { ...localFacts(tabId), ...patch };
      const targets = new Map(tabHandles(tabId));
      try {
        const top = await topHandle(tabId);
        if (top) targets.set(handleKey(tabId, top.ref), top);
      } catch (_error) { /* the top world is best effort like the rest */ }
      const results = await Promise.all(Array.from(targets.values(), (handle) =>
        callShim(handle, SHIMS.AMBIENT, [assets.id, facts]).then((reply) => reply?.result?.value === true, () => false)));
      return results.some(Boolean);
    }

    /**
     * Sends PRESENT with `signal` to the world of `ref`, or of the top document when `ref` is null.
     * Never attaches and never installs: resolves false when the tab is not attached, the frame is
     * gone, or the world has no current runtime.
     */
    async function present(tabId, ref, signal) {
      if (!assets || !lifecycle.rootSession(tabId)) return false;
      try {
        const snap = ref ? await inventory.snapshot(tabId) : await inventory.topSnapshot(tabId);
        const handle = await worldHandle(tabId, ref, snap);
        const reply = await callShim(handle, SHIMS.PRESENT, [assets.id, signal, localFacts(tabId)]);
        return reply?.result?.value === true;
      } catch (_error) {
        return false;
      }
    }

    /**
     * Releases a tab's page code before its debugger detaches (the sessions onRelease hook): DISPOSE
     * in every cached world, the `ghostlight` object group and the registrations of every session.
     * Best effort throughout.
     */
    async function disposeTab(tabId) {
      for (const [key, handle] of tabHandles(tabId)) {
        await callShim(handle, SHIMS.DISPOSE, []).catch(() => {});
        handles.delete(key);
      }
      const rootSession = lifecycle.rootSession(tabId);
      for (const session of rootSession ? [rootSession, ...lifecycle.childSessions(tabId)] : []) {
        await cdp(session.target, METHOD.RELEASE_GROUP, { objectGroup: WORLD }).catch(() => {});
        for (const identifier of session.identifiers.splice(0)) {
          await cdp(session.target, METHOD.REMOVE_SCRIPT, { identifier }).catch(() => {});
        }
      }
    }

    /** Forgets every world handle of a tab whose debugger attachment ended. Sends nothing. */
    function dropTab(tabId) {
      for (const [key] of tabHandles(tabId)) handles.delete(key);
      hiddenHolds.delete(tabId);
    }

    /** Forgets the world handles that lived in a child session that ended (the sessions onSessionGone hook). */
    function sessionGone(tabId, sessionId) {
      for (const [key, handle] of tabHandles(tabId)) {
        if (handle.session.sessionId === sessionId) handles.delete(key);
      }
    }

    /**
     * Reads devicePixelRatio in the top isolated world, where the page cannot override it.
     * The caller holds a debugger lease.
     */
    async function devicePixelRatio(tabId) {
      const handle = await topHandle(tabId);
      if (!handle) throw sessions.failure(CALL.SESSION_LOST, "The debugger is not attached to this tab.");
      const reply = await callShim(handle, SHIMS.DPR, []);
      return reply?.result?.value;
    }

    /**
     * Returns a remote objectId for the `document` of `ref` (in the `ghostlight` object group), for
     * CDP methods that take a node. The caller holds a debugger lease.
     */
    async function documentObject(tabId, ref, snap) {
      const handle = await worldHandle(tabId, ref, snap);
      const reply = await callShim(handle, SHIMS.DOC, [], { returnByValue: false, objectGroup: WORLD });
      return reply?.result?.objectId;
    }

    return Object.freeze({
      setAssets,
      assetsId,
      register,
      callOne,
      callPrimitive,
      inflight: inflightOf,
      cancelToken,
      watchForDialog,
      announceDialog,
      ambientAll,
      present,
      disposeTab,
      dropTab,
      sessionGone,
      devicePixelRatio,
      documentObject,
      localFacts,
      holdHidden,
      releaseHidden
    });
  }

  return Object.freeze({ create, SHIMS, wrap, registration, mapCallError, okAnswer, started, SKIPPED });
});
