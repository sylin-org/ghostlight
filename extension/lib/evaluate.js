// Ghostlight -- main-world script evaluation of adapter protocol 4.
(function installGhostlightEvaluate(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? {
      args: require("./arguments.js"),
      scriptEvaluator: require("./script-evaluator.js"),
      sessions: require("./sessions.js")
    }
    : {
      args: root.GhostlightArguments,
      scriptEvaluator: root.GhostlightScriptEvaluator,
      sessions: root.GhostlightSessions
    });
  root.GhostlightEvaluate = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightEvaluateApi(deps) {
  "use strict";

  const { args: check, scriptEvaluator, sessions } = deps;

  // The `evaluate_main` primitive runs a script in the main world of the top frame, where the page's
  // own code lives. The script form (a plain expression, or a function body with `return`) is chosen
  // by the unchanged script evaluator, which parses before anything is sent: a script that does not
  // parse is `invalid_request` and the browser is never touched. This module adds the guard check,
  // the dialog check, and a deadline. At the deadline it asks the page to stop with
  // `Runtime.terminateExecution` and answers `timeout`. Terminating stops a running script; it
  // cannot stop an awaited promise, so the receipt is `timeout` with the effect marked dispatched
  // either way.

  const METHOD = Object.freeze({ TERMINATE: "Runtime.terminateExecution" });
  const FAILURE = Object.freeze({ TIMEOUT: "timeout" });
  const SCRIPT_CODE = Object.freeze({ INVALID: "invalid_script", FAILED: "primitive_failed" });

  /**
   * Resolves or rejects like `work`, unless `timeoutMs` passes first: then `onTimeout()` runs and the
   * result rejects with a `timeout` failure. A settled `work` that lost the race is ignored.
   */
  function withDeadline(work, timeoutMs, onTimeout) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        onTimeout();
        reject(sessions.failure(FAILURE.TIMEOUT, `The script did not finish in ${timeoutMs} ms.`));
      }, timeoutMs);
      work.then(
        (value) => { clearTimeout(timer); resolve(value); },
        (error) => { clearTimeout(timer); reject(error); }
      );
    });
  }

  /**
   * Creates the evaluate handler. `lifecycle` is a GhostlightSessions instance and `inventory` a
   * GhostlightInventory. `acquire` and `release` take and give back a debugger lease (default: the
   * lifecycle's). `sleep(ms)` is the settle wait.
   */
  function create({
    lifecycle,
    inventory,
    acquire = (tabId) => lifecycle.acquire(tabId),
    release = (tabId) => lifecycle.release(tabId),
    sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  } = {}) {
    if (!lifecycle?.cdp || !inventory?.snapshot) throw new TypeError("evaluation requires debugger sessions and an inventory");

    const evaluateMain = Object.freeze({
      addresses: () => true,
      tabOf: (request) => request.tab_id,
      validate(request) {
        check.requireArgs(request);
        check.requireTabId(request.tab_id);
        if (typeof request.script !== "string") throw check.invalid("script must be text.");
        check.requireInteger(request.max_result_chars, "max_result_chars", { minimum: 1 });
        check.optionalGuard(request.guard);
        check.requireInteger(request.settle_ms, "settle_ms");
        check.requireInteger(request.timeout_ms, "timeout_ms", { minimum: 1 });
      },
      /**
       * Resolves `Evaluated`: the result as JSON text cut to `max_result_chars`, and whether it was
       * cut. A script the page threw on resolves with the bounded `exception` description instead.
       * The debugger is attached only once the script has parsed, and the guard is checked right
       * before the evaluation is sent.
       */
      async run(request, fx) {
        const tabId = request.tab_id;
        const target = { tabId };
        let leased = false;
        const send = async (method, params) => {
          await acquire(tabId);
          leased = true;
          if (lifecycle.currentDialog(tabId)) throw sessions.dialogOpen();
          inventory.checkGuard(request.guard, await inventory.snapshot(tabId));
          fx.watch(tabId);
          fx.dispatched = true;
          return withDeadline(lifecycle.cdp(target, method, params), request.timeout_ms,
            () => { lifecycle.cdp(target, METHOD.TERMINATE).catch(() => {}); });
        };
        try {
          let value;
          try {
            value = await scriptEvaluator.evaluate(send, request.script, request.max_result_chars);
          } catch (error) {
            if (error?.code === SCRIPT_CODE.INVALID) throw check.invalid(error.message);
            if (error?.code === SCRIPT_CODE.FAILED) return { value: "", truncated: false, exception: error.message };
            throw error;
          }
          const serialized = JSON.stringify(value ?? null);
          const text = serialized.slice(0, request.max_result_chars);
          if (request.settle_ms) await sleep(request.settle_ms);
          return { value: text, truncated: serialized.length > text.length };
        } finally {
          if (leased) await release(tabId);
        }
      }
    });

    return Object.freeze({ evaluateMain });
  }

  return Object.freeze({ create });
});
