// Ghostlight -- the primitive dispatcher of adapter protocol 4.
(function installGhostlightPrimitives(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? {
      args: require("./arguments.js"),
      protocol: require("./protocol.js"),
      sessions: require("./sessions.js"),
      shared: require("./shared.js"),
      tabsApi: require("./tabs.js")
    }
    : {
      args: root.GhostlightArguments,
      protocol: root.GhostlightProtocol,
      sessions: root.GhostlightSessions,
      shared: root.GhostlightShared,
      tabsApi: root.GhostlightTabs
    });
  root.GhostlightPrimitives = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightPrimitivesApi(deps) {
  "use strict";

  const { args: check, protocol, sessions, shared, tabsApi } = deps;
  const { CDP } = sessions;

  // Every request frame names one primitive and gets exactly one receipt. This module is the only
  // place that decides how: it looks the primitive up in a closed table (an unknown name is
  // `unsupported` and touches nothing), validates the arguments (a violation is `invalid_request`
  // and touches nothing), marks the tab managed for primitives that address one, runs the handler,
  // and builds the receipt from the handler's outcome and the effect facts it gathered.
  //
  // Effect facts are per primitive. Each running primitive owns a watch, kept in a per-tab set, so
  // two primitives on one tab both see every top-frame commit and neither clobbers the other. The
  // facts are `dispatched` (did the primitive send anything that can change the browser or the
  // page), `committed_urls`, and the tab as it stands afterwards when something was dispatched
  // against a tab. The handlers decide what counts as dispatched; this module only reports it.
  //
  // A cancel can arrive before its request has finished reassembling from chunks, so cancels are
  // remembered for a minute and checked when the request starts. The dialog and accessibility
  // snapshot primitives are small enough to live here.

  const CODE = Object.freeze({
    UNSUPPORTED: "unsupported",
    INVALID_REQUEST: "invalid_request",
    TAB_MISSING: "tab_missing",
    DEBUGGER_UNAVAILABLE: "debugger_unavailable",
    DOCUMENT_CHANGED: "document_changed",
    CANCELLED: "cancelled",
    BROWSER_REFUSED: "browser_refused",
    SESSION_LOST: "session_lost",
    OTHER: "other"
  });
  const CANCEL_TTL_MS = 60_000;
  const MESSAGE_CHARS = 500;
  const DIALOG_SETTLE_MS = 50;
  const DIALOG_UNKNOWN = "unknown";
  const DIALOG_PROMPT = "prompt";
  const NO_DIALOG = /No dialog is showing/;
  const AX_ROLE_CHARS = 100;
  const AX_TEXT_CHARS = 500;
  const AX_PROPERTIES = Object.freeze(new Set(["checked", "disabled", "expanded", "focused", "required", "selected", "level"]));
  const DIALOG_OP = Object.freeze({ STATUS: "status", HANDLE: "handle" });
  const STEP = Object.freeze({ MOUSE: "mouse", KEY: "key", INSERT_TEXT: "insert_text", DRAG: "drag", PAUSE: "pause" });

  // How a thrown error becomes a receipt failure when it does not carry a protocol code itself. The
  // patterns are closed and ordered; a CDP error (Chrome reports it as JSON with a numeric code)
  // that none of them claims is the browser refusing the call.
  const ERROR_PATTERNS = Object.freeze([
    { pattern: /No tab with (given )?id/i, code: CODE.TAB_MISSING },
    { pattern: sessions.SESSION_LOST_TEXT, code: CODE.SESSION_LOST },
    { pattern: /Cannot (attach|access)|(Another debugger|debugger) is already attached/i, code: CODE.DEBUGGER_UNAVAILABLE },
    { pattern: /^\s*\{\s*"code"\s*:\s*-?\d+/, code: CODE.BROWSER_REFUSED }
  ]);

  /**
   * The receipt failure code for a thrown error: its own code when it is a protocol receipt code,
   * else the first matching pattern, else `other`.
   */
  function failureCode(error) {
    if (protocol.isKnown("receipt_failures", error?.code)) return error.code;
    const text = String(error?.message ?? error);
    const known = ERROR_PATTERNS.find((entry) => entry.pattern.test(text));
    if (known) return known.code;
    return Number.isInteger(error?.code) ? CODE.BROWSER_REFUSED : CODE.OTHER;
  }

  /**
   * Creates the cancel registry. A cancel is remembered for 60 seconds, whether or not the request it
   * names has started, so a cancel that beats a chunked request still counts. `now` is the clock.
   * Give `isCancelled` to the modules whose loops check for cancel, and the registry to `create`.
   */
  function createCancellations({ now = () => Date.now() } = {}) {
    const expiry = new Map();

    function prune() {
      const current = now();
      for (const [correlation, until] of expiry) if (until <= current) expiry.delete(correlation);
    }

    return Object.freeze({
      /** Remembers a cancel for `correlation`. */
      cancel(correlation) {
        prune();
        expiry.set(correlation, now() + CANCEL_TTL_MS);
      },
      /** True when a cancel for `correlation` was seen and has not expired. */
      isCancelled(correlation) {
        const until = expiry.get(correlation);
        return until !== undefined && until > now();
      },
      /** Forgets the cancel for `correlation`: its primitive has ended. */
      forget(correlation) {
        expiry.delete(correlation);
      }
    });
  }

  function handler({ validate, run, addresses }) {
    return Object.freeze({ validate, run, addresses: () => addresses, tabOf: (request) => request.tab_id });
  }

  function validateTab(request) {
    check.requireArgs(request);
    check.requireTabId(request.tab_id);
  }

  function validateCall(request) {
    validateTab(request);
    if (!protocol.isKnown("call_modes", request.mode)) throw check.invalid("mode must name a call mode.");
    if (!Array.isArray(request.calls) || request.calls.length < 1 || request.calls.length > protocol.MAX_WORLD_CALLS) {
      throw check.invalid(`calls must list from 1 to ${protocol.MAX_WORLD_CALLS} calls.`);
    }
    request.calls.forEach((call, index) => check.requireWorldCall(call, `calls[${index}]`));
    check.optionalGuard(request.guard);
    check.requireInteger(request.settle_ms, "settle_ms");
    check.requireInteger(request.timeout_ms, "timeout_ms", { minimum: 1 });
  }

  function validatePoint(point, name) {
    if (!check.isObject(point)) throw check.invalid(`${name} must be a point.`);
    check.requireFinite(point.x, `${name}.x`);
    check.requireFinite(point.y, `${name}.y`);
  }

  function validateStep(step, name) {
    if (!check.isObject(step) || !protocol.isKnown("input_steps", step.step)) throw check.invalid(`${name} must name an input step.`);
    if (step.step === STEP.MOUSE) {
      if (!protocol.isKnown("mouse_actions", step.action)) throw check.invalid(`${name}.action must name a mouse action.`);
      validatePoint(step, name);
      if (!check.absent(step.button) && !protocol.isKnown("mouse_buttons", step.button)) throw check.invalid(`${name}.button must name a mouse button.`);
    } else if (step.step === STEP.KEY) {
      if (!protocol.isKnown("key_actions", step.action)) throw check.invalid(`${name}.action must name a key action.`);
      check.requireText(step.key, `${name}.key`, 64);
    } else if (step.step === STEP.INSERT_TEXT) {
      if (typeof step.text !== "string") throw check.invalid(`${name}.text must be text.`);
    } else if (step.step === STEP.DRAG) {
      if (!Array.isArray(step.path) || step.path.length < 2) throw check.invalid(`${name}.path must list at least two points.`);
      step.path.forEach((point, index) => validatePoint(point, `${name}.path[${index}]`));
    } else {
      check.requireInteger(step.ms, `${name}.ms`, { maximum: protocol.MAX_PAUSE_MS });
    }
  }

  function validateInput(request) {
    validateTab(request);
    if (!Array.isArray(request.steps) || request.steps.length < 1 || request.steps.length > protocol.MAX_INPUT_STEPS) {
      throw check.invalid(`steps must list from 1 to ${protocol.MAX_INPUT_STEPS} steps.`);
    }
    request.steps.forEach((step, index) => validateStep(step, `steps[${index}]`));
    if (!check.absent(request.precondition)) check.requireWorldCall(request.precondition, "precondition");
    check.optionalGuard(request.guard);
    check.requireInteger(request.settle_ms, "settle_ms");
    check.requireInteger(request.timeout_ms, "timeout_ms", { minimum: 1 });
  }

  // The url fact of a dialog: the bounded url of the frame that raised it, and nothing when the
  // browser did not say.
  function dialogUrl(dialog) {
    return dialog?.url ? { url: shared.bounded(dialog.url, protocol.URL_CHARS) } : {};
  }

  function validateDialog(request) {
    validateTab(request);
    if (!check.isObject(request.op) || !protocol.isKnown("dialog_ops", request.op.op)) throw check.invalid("op must name a dialog operation.");
    if (request.op.op === DIALOG_OP.HANDLE) {
      check.requireBoolean(request.op.accept, "op.accept");
      if (!check.absent(request.op.text) && typeof request.op.text !== "string") throw check.invalid("op.text must be text.");
    }
  }

  function validateAccessibility(request) {
    validateTab(request);
    check.requireFrameRef(request.frame);
    check.requireInteger(request.max_nodes, "max_nodes", { minimum: 1, maximum: protocol.MAX_AX_NODES });
    if (!check.absent(request.max_depth)) check.requireInteger(request.max_depth, "max_depth");
    if (!check.absent(request.query)) {
      if (!check.isObject(request.query)) throw check.invalid("query must be an object.");
      for (const field of ["role", "name"]) {
        if (!check.absent(request.query[field])) check.requireText(request.query[field], `query.${field}`, AX_TEXT_CHARS, { empty: true });
      }
    }
    check.optionalGuard(request.guard);
  }

  // One accessibility node as the protocol reports it: bounded text, and only the properties the
  // service reads.
  function axFact(node) {
    const fact = {
      node_id: String(node.nodeId),
      role: shared.bounded(node.role?.value, AX_ROLE_CHARS),
      name: shared.bounded(node.name?.value, AX_TEXT_CHARS)
    };
    if (node.parentId !== undefined) fact.parent_id = String(node.parentId);
    if (node.value?.value !== undefined) fact.value = shared.bounded(node.value.value, AX_TEXT_CHARS);
    const properties = (node.properties ?? [])
      .filter((property) => AX_PROPERTIES.has(property.name) && property.value?.value !== undefined)
      .map((property) => [property.name, String(property.value.value)]);
    if (properties.length > 0) fact.properties = properties;
    return fact;
  }

  /**
   * Creates the dispatcher.
   *
   * `send(frame)` writes a frame to the native port. `controlled.manage(tabId)` is the worker's
   * controlled-tab hook, called before any primitive that addresses a tab. `cancellations` is a
   * `createCancellations()` registry. `lifecycle`, `inventory` and `world` are the sessions,
   * inventory and world instances. `tabs`, `capture`, `input`, `buffers` and `evaluate` are the
   * modules whose handlers fill the table. `chromeApi` supplies `tabs.get` for the tab facts.
   * `sleep` defaults to a timer.
   */
  function create({
    chromeApi = globalThis.chrome,
    send,
    controlled,
    cancellations,
    lifecycle,
    inventory,
    world,
    tabs,
    capture,
    input,
    buffers,
    evaluate,
    sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  } = {}) {
    if (typeof send !== "function" || !controlled?.manage || !cancellations?.isCancelled) {
      throw new TypeError("the dispatcher requires a sender, the controlled-tab hook and the cancel registry");
    }
    if (!lifecycle?.cdp || !inventory?.snapshot || !world?.callPrimitive) {
      throw new TypeError("the dispatcher requires debugger sessions, an inventory and the world");
    }
    if (!chromeApi?.tabs?.get) throw new TypeError("the dispatcher requires chrome.tabs");
    const cdp = lifecycle.cdp;
    const inflight = new Map();
    // tab id -> the effect watches of the primitives running on that tab
    const watching = new Map();

    // ---- effect facts -------------------------------------------------------------------------

    // Opens one primitive's effect watch: the object handlers receive as `fx`.
    function openEffects() {
      let tabId = null;
      const commits = [];
      const entry = { record: (url) => commits.push(shared.bounded(url, protocol.URL_CHARS)) };
      const fx = {
        dispatched: false,
        /** Starts collecting the top-frame commits of a tab. A primitive follows one tab. */
        watch(id) {
          if (tabId === id) return;
          if (tabId !== null) throw new TypeError("an effect watch follows one tab");
          tabId = id;
          if (!watching.has(id)) watching.set(id, new Set());
          watching.get(id).add(entry);
        },
        /** Resolves `{ dispatched, committed_urls, tab? }`. The tab is read when something was dispatched against it. */
        async facts() {
          const facts = { dispatched: fx.dispatched, committed_urls: commits.slice() };
          if (fx.dispatched && tabId !== null) {
            try {
              facts.tab = tabsApi.tabFact(await chromeApi.tabs.get(tabId));
            } catch (_error) { /* a tab that is gone has no state to report */ }
          }
          return facts;
        },
        /** Ends the watch. */
        close() {
          if (tabId === null) return;
          const entries = watching.get(tabId);
          entries?.delete(entry);
          if (entries?.size === 0) watching.delete(tabId);
        }
      };
      return fx;
    }

    // ---- dialog and accessibility snapshot ----------------------------------------------------

    async function dialogStatus(request) {
      const tabId = request.tab_id;
      let dialog = lifecycle.currentDialog(tabId);
      if (!dialog) {
        // A dialog that was already open when the debugger attaches is announced just after.
        dialog = await lifecycle.withLease(tabId, async () => {
          await sleep(DIALOG_SETTLE_MS);
          return lifecycle.currentDialog(tabId);
        });
      }
      return {
        present: Boolean(dialog),
        dialog_type: dialog?.type ?? DIALOG_UNKNOWN,
        message: dialog ? shared.bounded(dialog.message, MESSAGE_CHARS) : undefined,
        // A prompt also says what its field holds: the text accept answers with.
        default_prompt: dialog?.type === DIALOG_PROMPT ? shared.bounded(dialog.defaultPrompt, MESSAGE_CHARS) : undefined,
        ...dialogUrl(dialog)
      };
    }

    // The text a prompt is answered with. Given text is sent as it is, empty included. Accepting a
    // prompt without text answers with the text its field starts with, as OK does: the browser
    // would otherwise answer with an empty string and drop the page's default.
    function promptText(op, dialog) {
      return op.text ?? (op.accept && dialog?.type === DIALOG_PROMPT ? dialog.defaultPrompt : undefined);
    }

    // The dialog is answered on the session that raised it, so a dialog of an out-of-process frame
    // is answered too.
    async function dialogHandle(request, fx) {
      const tabId = request.tab_id;
      return lifecycle.withLease(tabId, async () => {
      const dialog = lifecycle.currentDialog(tabId);
      try {
        fx.watch(tabId);
        fx.dispatched = true;
        const target = dialog?.sessionId ? { tabId, sessionId: dialog.sessionId } : { tabId };
        await cdp(target, CDP.PAGE_HANDLE_JAVASCRIPT_DIALOG, { accept: request.op.accept, promptText: promptText(request.op, dialog) });
        await lifecycle.closeDialog(tabId);
        return { present: true, dialog_type: dialog?.type ?? DIALOG_UNKNOWN, handled: true, accepted: request.op.accept, ...dialogUrl(dialog) };
      } catch (error) {
        if (!NO_DIALOG.test(String(error?.message))) throw error;
        // The dialog was already gone: nothing was handled, so nothing was dispatched.
        fx.dispatched = false;
        await lifecycle.closeDialog(tabId);
        return { present: false, dialog_type: DIALOG_UNKNOWN, handled: false, accepted: false };
      }
      });
    }

    const dialogPrimitive = handler({
      addresses: true,
      validate: validateDialog,
      /**
       * Resolves `DialogFacts`. `status` reports the open dialog with its bounded message and, for a
       * prompt, the bounded text its field holds. `handle` accepts or dismisses it, with the prompt
       * text when given.
       */
      run(request, fx) {
        return request.op.op === DIALOG_OP.STATUS ? dialogStatus(request) : dialogHandle(request, fx);
      }
    });

    const accessibilityPrimitive = handler({
      addresses: true,
      validate: validateAccessibility,
      /**
       * Resolves `AxSnapshot`: the accessibility nodes of one frame, or the nodes matching a role
       * and name, at most `max_nodes`. The frame must still hold the loader the request named,
       * before and after the read.
       */
      async run(request) {
        const tabId = request.tab_id;
        return lifecycle.withLease(tabId, async () => {
          const snap = await inventory.snapshot(tabId);
          inventory.checkGuard(request.guard, snap);
          const owner = snap.owner.get(request.frame.frame_id);
          if (!owner || owner.loaderId !== request.frame.loader_id) {
            throw sessions.failure(CODE.DOCUMENT_CHANGED, "The frame changed since it was read.");
          }
          const raw = request.query
            ? await cdp(owner.session, CDP.ACCESSIBILITY_QUERY_AX_TREE, {
              objectId: await world.documentObject(tabId, request.frame, snap),
              role: request.query.role ?? undefined,
              accessibleName: request.query.name ?? undefined
            })
            : await cdp(owner.session, CDP.ACCESSIBILITY_GET_FULL_AX_TREE, {
              frameId: request.frame.frame_id,
              depth: request.max_depth ?? undefined
            });
          if (await inventory.currentLoader(owner.session, request.frame.frame_id) !== request.frame.loader_id) {
            throw sessions.failure(CODE.DOCUMENT_CHANGED, "The frame changed while it was read.");
          }
          const nodes = (raw.nodes ?? []).filter((candidate) => !candidate.ignored);
          return { nodes: nodes.slice(0, request.max_nodes).map(axFact), truncated: nodes.length > request.max_nodes };
        });
      }
    });

    // ---- the table ----------------------------------------------------------------------------

    const handlers = Object.freeze({
      list_tabs: tabs.listTabs,
      open_tab: tabs.openTab,
      navigate: tabs.navigate,
      focus_tab: tabs.focusTab,
      close_tab: tabs.closeTab,
      set_zoom: tabs.setZoom,
      resize_window: tabs.resizeWindow,
      frames: handler({ addresses: true, validate: validateTab, run: (request, fx, correlation) => inventory.framesPrimitive(request, fx, correlation) }),
      call: handler({ addresses: true, validate: validateCall, run: (request, fx, correlation) => world.callPrimitive(request, fx, correlation) }),
      evaluate_main: evaluate.evaluateMain,
      view: capture.view,
      input: handler({ addresses: true, validate: validateInput, run: (request, fx, correlation) => input.run(request, fx, correlation) }),
      capture: capture.capture,
      dialog: dialogPrimitive,
      diagnostics: buffers.diagnostics,
      recording: buffers.recording,
      accessibility_snapshot: accessibilityPrimitive
    });
    for (const name of protocol.vocabulary.primitives) {
      if (!handlers[name]) throw new TypeError(`the primitive table has no handler for ${name}`);
    }

    // ---- dispatch -----------------------------------------------------------------------------

    function reply(correlation, receipt) {
      if (inflight.get(correlation)?.dropped) return;
      send({ kind: "receipt", correlation, ...receipt });
    }

    function validated(spec, request) {
      try {
        spec.validate(request);
      } catch (error) {
        // A validator that fails on a malformed shape without naming a code is still the request's fault.
        throw protocol.isKnown("receipt_failures", error?.code) ? error : check.invalid(String(error?.message ?? error));
      }
    }

    async function execute(correlation, spec, request) {
      const fx = openEffects();
      try {
        validated(spec, request);
        if (cancellations.isCancelled(correlation)) throw sessions.failure(CODE.CANCELLED, "The request was cancelled.");
        if (spec.addresses(request)) await controlled.manage(spec.tabOf(request));
        const ok = await spec.run(request, fx, correlation);
        return { ok, ...(await fx.facts()) };
      } catch (error) {
        const message = shared.bounded(error?.message ?? error, MESSAGE_CHARS);
        return { failure: failureCode(error), ...(message ? { message } : {}), ...(await fx.facts()) };
      } finally {
        fx.close();
      }
    }

    /**
     * Runs one request and sends its one receipt. An unknown primitive name is `unsupported` with
     * nothing dispatched. The receipt is dropped when `disconnect()` ran while the primitive was in flight.
     */
    async function run(correlation, primitive) {
      inflight.set(correlation, { dropped: false });
      try {
        const spec = Object.hasOwn(handlers, primitive?.name) ? handlers[primitive.name] : null;
        const receipt = spec
          ? await execute(correlation, spec, primitive.args ?? {})
          : { failure: CODE.UNSUPPORTED, dispatched: false, committed_urls: [] };
        reply(correlation, receipt);
      } finally {
        cancellations.forget(correlation);
        inflight.delete(correlation);
      }
    }

    /**
     * Answers a request that never reached a handler (a rejected chunk transfer) with a failure
     * receipt and nothing dispatched.
     */
    function reject(correlation, code, reason) {
      cancellations.forget(correlation);
      reply(correlation, { failure: code, message: shared.bounded(reason, MESSAGE_CHARS), dispatched: false, committed_urls: [] });
    }

    /**
     * Takes a `cancel` frame. The cancel is remembered, so a request that has not started or
     * finished reassembling is cancelled when it does, and the world calls in flight for the
     * request are told to stop.
     */
    function cancel(correlation) {
      cancellations.cancel(correlation);
      for (const entry of world.inflight(correlation)) world.cancelToken(entry);
    }

    /**
     * Takes the loss of the native port: every primitive in flight is cancelled and its receipt is
     * dropped, because the port it would answer on is gone.
     */
    function disconnect() {
      for (const [correlation, flight] of inflight) {
        flight.dropped = true;
        cancel(correlation);
      }
    }

    /**
     * Takes a top-frame commit (`webNavigation.onCommitted` with frame 0): every primitive watching
     * the tab records the url.
     */
    function committed(tabId, url) {
      for (const entry of watching.get(tabId) ?? []) entry.record(url);
    }

    return Object.freeze({
      run,
      reject,
      cancel,
      disconnect,
      committed,
      isCancelled: cancellations.isCancelled,
      handlers
    });
  }

  return Object.freeze({ create, createCancellations, failureCode, CODE });
});
