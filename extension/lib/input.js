// Ghostlight -- the input batch of adapter protocol 4.
(function installGhostlightInput(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? { protocol: require("./protocol.js"), sessions: require("./sessions.js"), world: require("./world.js") }
    : { protocol: root.GhostlightProtocol, sessions: root.GhostlightSessions, world: root.GhostlightWorld });
  root.GhostlightInput = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightInputApi(deps) {
  "use strict";

  const { protocol, sessions, world } = deps;
  const { CDP } = sessions;

  // Trusted input for one tab: an optional world-call precondition (for example the focus check that
  // must pass right before key input), then mouse, key, text, pause and drag steps sent through the
  // root session. A JavaScript dialog blocks the renderer, so each step races the tab's dialog watch
  // and the batch stops at the first dialog. HTML5 drags are intercepted and replayed as drag events;
  // a pointer-library drag is a plain press, move and release. The service decides what to send.

  const STEP = Object.freeze({ MOUSE: "mouse", KEY: "key", INSERT_TEXT: "insert_text", DRAG: "drag", PAUSE: "pause" });
  const MOUSE = Object.freeze({ moved: "mouseMoved", pressed: "mousePressed", released: "mouseReleased", wheel: "mouseWheel" });
  const KEY = Object.freeze({ down: "keyDown", up: "keyUp", raw_down: "rawKeyDown", char: "char" });
  const DRAG_REPLAY = Object.freeze(["dragEnter", "dragOver", "drop"]);
  const BUTTON_NONE = "none";
  const BUTTON_LEFT = "left";
  const STOP = Object.freeze({ DIALOG: "dialog", CANCELLED: "cancelled" });
  const FAILURE = Object.freeze({ INVALID_REQUEST: "invalid_request", TIMEOUT: "timeout" });
  // How long a held drag move waits for interception data, and the grace for late data at the end.
  const INTERCEPT_STEP_MS = 10;
  const INTERCEPT_GRACE_MS = 100;

  /**
   * Creates the input runner. `lifecycle` is a GhostlightSessions instance, `inventory` a
   * GhostlightInventory and `worldCalls` a GhostlightWorld instance. `isCancelled(correlation)`
   * reports a cancel frame.
   */
  function create({
    lifecycle,
    inventory,
    worldCalls,
    isCancelled = () => false,
    now = () => Date.now(),
    sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  } = {}) {
    if (!lifecycle?.cdp || !inventory?.snapshot || !worldCalls?.callOne) {
      throw new TypeError("input requires debugger sessions, an inventory and the world");
    }
    const cdp = lifecycle.cdp;
    const interceptions = new Map();
    const rootTarget = (tabId) => ({ tabId });

    function mouse(tabId, type, point, { button = BUTTON_NONE, buttons = 0, clickCount = 0, modifiers = 0, force = 0 } = {}) {
      return cdp(rootTarget(tabId), CDP.INPUT_DISPATCH_MOUSE_EVENT, { type, x: point.x, y: point.y, button, buttons, clickCount, modifiers, force });
    }

    function sendStep(tabId, step) {
      if (step.step === STEP.MOUSE) {
        return cdp(rootTarget(tabId), CDP.INPUT_DISPATCH_MOUSE_EVENT, {
          type: MOUSE[step.action], x: step.x, y: step.y, button: step.button ?? BUTTON_NONE, buttons: step.buttons ?? 0,
          clickCount: step.click_count ?? 0, modifiers: step.modifiers ?? 0, deltaX: step.delta_x ?? 0, deltaY: step.delta_y ?? 0,
          force: step.force ?? 0
        });
      }
      if (step.step === STEP.KEY) {
        return cdp(rootTarget(tabId), CDP.INPUT_DISPATCH_KEY_EVENT, {
          type: KEY[step.action], key: step.key, code: step.code ?? undefined, windowsVirtualKeyCode: step.key_code ?? undefined,
          nativeVirtualKeyCode: step.key_code ?? undefined, text: step.text ?? undefined, unmodifiedText: step.unmodified_text ?? undefined,
          modifiers: step.modifiers ?? 0
        });
      }
      if (step.step === STEP.INSERT_TEXT) return cdp(rootTarget(tabId), CDP.INPUT_INSERT_TEXT, { text: step.text });
      if (step.step === STEP.PAUSE) return sleep(Math.min(protocol.MAX_PAUSE_MS, step.ms));
      return Promise.reject(sessions.failure(FAILURE.INVALID_REQUEST, `Unknown input step: ${step.step}`));
    }

    // One drag's interception watch. Data that arrives between waits is kept for the next wait.
    function beginDragInterception(tabId) {
      const entry = { data: null, waiters: new Set() };
      interceptions.set(tabId, entry);
      return {
        within(ms) {
          if (entry.data) return Promise.resolve(entry.data);
          if (interceptions.get(tabId) !== entry) return Promise.resolve(null);
          return new Promise((resolve) => {
            const waiter = (data) => {
              clearTimeout(timer);
              entry.waiters.delete(waiter);
              resolve(data);
            };
            const timer = setTimeout(() => waiter(null), ms);
            entry.waiters.add(waiter);
          });
        }
      };
    }

    /**
     * Ends a tab's drag interception watch, waking any wait with no data. Called when a drag ends and
     * when the top frame commits a new document.
     */
    function cancelDragInterception(tabId) {
      const entry = interceptions.get(tabId);
      if (!entry) return;
      interceptions.delete(tabId);
      for (const waiter of Array.from(entry.waiters)) waiter(null);
    }

    /**
     * Takes `Input.dragIntercepted` data from the worker. A drag that is watching receives it; data
     * that arrives after its drag ended starts a drag no one will finish, so it is cancelled.
     */
    async function dragIntercepted(tabId, data) {
      const entry = interceptions.get(tabId);
      if (entry) {
        entry.data = data;
        for (const waiter of Array.from(entry.waiters)) waiter(data);
        return;
      }
      await cdp(rootTarget(tabId), CDP.INPUT_CANCEL_DRAGGING).catch(() => {});
    }

    // Press at the first point, move through the path, release at the last. An intercepted HTML5
    // drag is replayed as dragEnter, dragOver and drop at the last point. Resolves `{ native }`.
    async function drag(tabId, step) {
      const [start, ...rest] = step.path;
      const end = step.path.at(-1);
      const modifiers = step.modifiers ?? 0;
      const interception = beginDragInterception(tabId);
      let intercepting = false;
      let pressed = false;
      let released = false;

      async function replay(data) {
        await cdp(rootTarget(tabId), CDP.INPUT_SET_INTERCEPT_DRAGS, { enabled: false });
        intercepting = false;
        for (const type of DRAG_REPLAY) {
          await cdp(rootTarget(tabId), CDP.INPUT_DISPATCH_DRAG_EVENT, { type, x: end.x, y: end.y, data, modifiers });
        }
        await mouse(tabId, MOUSE.released, end, { button: BUTTON_LEFT, clickCount: 1, modifiers });
        released = true;
        return { native: true };
      }

      try {
        try {
          await cdp(rootTarget(tabId), CDP.INPUT_SET_INTERCEPT_DRAGS, { enabled: true });
          intercepting = true;
        } catch (_error) {
          cancelDragInterception(tabId);
        }
        await mouse(tabId, MOUSE.moved, start, { modifiers });
        await mouse(tabId, MOUSE.pressed, start, { button: BUTTON_LEFT, clickCount: 1, modifiers });
        pressed = true;
        for (const point of rest) {
          await mouse(tabId, MOUSE.moved, point, { button: BUTTON_LEFT, buttons: 1, force: 1, modifiers });
          const data = intercepting ? await interception.within(INTERCEPT_STEP_MS) : null;
          if (data) return await replay(data);
        }
        const late = intercepting ? await interception.within(INTERCEPT_GRACE_MS) : null;
        if (late) return await replay(late);
        await mouse(tabId, MOUSE.released, end, { button: BUTTON_LEFT, clickCount: 1, modifiers });
        released = true;
        return { native: false };
      } finally {
        cancelDragInterception(tabId);
        if (intercepting) await cdp(rootTarget(tabId), CDP.INPUT_SET_INTERCEPT_DRAGS, { enabled: false }).catch(() => {});
        if (pressed && !released) {
          await mouse(tabId, MOUSE.released, end, { button: BUTTON_LEFT, clickCount: 1 }).catch(() => {});
          await cdp(rootTarget(tabId), CDP.INPUT_CANCEL_DRAGGING).catch(() => {});
        }
      }
    }

    /**
     * The `input` primitive. Checks the guard, runs the precondition world call (a non-ok answer
     * ends the batch with no input sent), re-checks the guard right before the first event, then
     * sends the steps in order. Resolves `InputDone`: the precondition answer, the completed step
     * count, why the batch stopped early (a dialog or a cancel), and whether a drag was native.
     */
    async function run(args, fx, correlation) {
      return lifecycle.withLease(args.tab_id, async () => {
        let snap = await inventory.snapshot(args.tab_id);
        inventory.checkGuard(args.guard, snap);
        fx.watch(args.tab_id);
        const deadline = now() + args.timeout_ms;
        const outcome = { completed_steps: 0, native_drag: false };
        if (args.precondition) {
          const precondition = await worldCalls.callOne(args.tab_id, args.precondition,
            { correlation, token: `${correlation}:p`, deadline, snap, attachment: null });
          outcome.precondition = precondition;
          fx.dispatched = world.started(precondition);
          if (!world.okAnswer(precondition)) return outcome;
          snap = await inventory.snapshot(args.tab_id);
          inventory.checkGuard(args.guard, snap);
        }
        if (lifecycle.currentDialog(args.tab_id)) throw sessions.dialogOpen();
        const dialog = worldCalls.watchForDialog(args.tab_id);
        try {
          for (const step of args.steps) {
            if (isCancelled(correlation)) return { ...outcome, stopped: { reason: STOP.CANCELLED } };
            if (now() >= deadline) throw sessions.failure(FAILURE.TIMEOUT, `Input did not finish in ${args.timeout_ms} ms.`);
            fx.dispatched = true;
            const sent = step.step === STEP.DRAG
              ? drag(args.tab_id, step).then((result) => { outcome.native_drag ||= result.native; })
              : sendStep(args.tab_id, step);
            sent.catch(() => {});
            const opened = await Promise.race([sent.then(() => null), dialog.opened]);
            if (opened) return { ...outcome, stopped: { reason: STOP.DIALOG, dialog_type: opened } };
            outcome.completed_steps += 1;
          }
        } finally {
          dialog.forget();
        }
        if (args.settle_ms) await sleep(args.settle_ms);
        return outcome;
      });
    }

    return Object.freeze({ run, dragIntercepted, cancelDragInterception });
  }

  return Object.freeze({ create });
});
