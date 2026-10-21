// Ghostlight -- main-world script evaluation of adapter protocol 4.
(function installGhostlightEvaluate(root, factory) {
  const node = typeof module !== "undefined" && module.exports;
  const api = factory(node
    ? {
      args: require("./arguments.js"),
      parser: require("../vendor/acorn.js"),
      sessions: require("./sessions.js")
    }
    : {
      args: root.GhostlightArguments,
      parser: root.acorn,
      sessions: root.GhostlightSessions
    });
  root.GhostlightEvaluate = api;
  if (node) module.exports = api;
})(globalThis, function createGhostlightEvaluateApi(deps) {
  "use strict";

  const { args: check, parser, sessions } = deps;
  const { CDP } = sessions;

  // The `evaluate_main` primitive runs a script in the main world of the top frame, where the page's
  // own code lives. The script's form (a plain expression, or a function body with `return`) is
  // chosen by parsing it before anything is sent: a script that does not parse is `invalid_request`
  // and the browser is never touched. Once the evaluation is sent, the receipt says it was
  // dispatched, so a script the page threw on, or a reply that was lost, leaves its effect
  // uncertain and is never sent again. This module also checks the guard and the dialog, and keeps
  // a deadline. At the deadline it asks the page to stop with `Runtime.terminateExecution` and
  // answers `timeout`. Terminating stops a running script; it cannot stop an awaited promise, so
  // the receipt is `timeout` with the effect marked dispatched either way.

  const FAILURE = Object.freeze({ TIMEOUT: "timeout" });
  const UNPARSED_FAILURE = "page script failed";
  const FUNCTION_NODES = new Set(["FunctionDeclaration", "FunctionExpression", "ArrowFunctionExpression"]);
  const PARSE_OPTIONS = Object.freeze({ ecmaVersion: "latest", allowAwaitOutsideFunction: true });
  const WRAPPER_PREFIX = "await (async () => {\n";
  const WRAPPER_SUFFIX = "\n})()";

  /** The `Runtime.evaluate` parameters for one expression, with REPL-grade flags. */
  function evaluationRequest(expression) {
    return {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
      replMode: true
    };
  }

  function wrappedExpression(script) {
    // REPL evaluation awaits its own result envelope, not a promise stored in that envelope.
    // A leading hashbang is a comment only at the start of a program, so preserve it as a comment
    // inside the function as well. Unwrapped REPL execution still receives the original source.
    const body = script.startsWith("#!") ? `//${script.slice(2)}` : script;
    return `${WRAPPER_PREFIX}${body}${WRAPPER_SUFFIX}`;
  }

  function hasTopLevelReturn(program) {
    const pending = [program];
    while (pending.length > 0) {
      const node = pending.pop();
      if (node.type === "ReturnStatement") return true;
      if (FUNCTION_NODES.has(node.type)) continue;
      for (const value of Object.values(node)) {
        if (Array.isArray(value)) {
          for (const child of value) if (child && typeof child.type === "string") pending.push(child);
        } else if (value && typeof value.type === "string") {
          pending.push(value);
        }
      }
    }
    return false;
  }

  /**
   * The expression `script` is sent as: the script itself, or the script wrapped as an async
   * function body when it returns at top level. A script that does not parse throws
   * `invalid_request`, its description cut to `maximum`, before anything is sent.
   */
  function scriptExpression(script, maximum) {
    try {
      const expression = wrappedExpression(script);
      // An async body permits REPL syntax such as top-level await and resource declarations.
      // Parse it without running it, then keep the original REPL scope unless it contains return.
      const program = parser.parse(expression, PARSE_OPTIONS);
      const call = program.body[0]?.expression?.argument;
      const body = call?.callee?.body;
      if (program.body.length !== 1 || program.body[0].expression?.type !== "AwaitExpression"
          || call?.type !== "CallExpression" || call.callee?.type !== "ArrowFunctionExpression"
          || body?.start !== WRAPPER_PREFIX.indexOf("{")
          || body?.end !== expression.length - WRAPPER_SUFFIX.length + WRAPPER_SUFFIX.indexOf("}") + 1) {
        throw new Error("script must form one function body");
      }
      return hasTopLevelReturn(body) ? expression : script;
    } catch (cause) {
      let description = String(cause?.message || UNPARSED_FAILURE);
      if (cause?.loc) {
        // The parser's synthetic prefix adds one line; report coordinates in the supplied source.
        description = description.replace(/\(\d+:\d+\)$/, `(${Math.max(1, cause.loc.line - 1)}:${cause.loc.column})`);
      }
      throw check.invalid(description.slice(0, maximum));
    }
  }

  /** The bounded description of an exception the page threw. */
  function exceptionText(details, maximum) {
    return String(details?.exception?.description || details?.text || UNPARSED_FAILURE).slice(0, maximum);
  }

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
   * GhostlightInventory. `sleep(ms)` is the settle wait.
   */
  function create({
    lifecycle,
    inventory,
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
        const expression = scriptExpression(request.script, request.max_result_chars);
        return lifecycle.withLease(tabId, async () => {
          if (lifecycle.currentDialog(tabId)) throw sessions.dialogOpen();
          inventory.checkGuard(request.guard, await inventory.snapshot(tabId));
          fx.watch(tabId);
          fx.dispatched = true;
          const evaluated = await withDeadline(lifecycle.cdp(target, CDP.RUNTIME_EVALUATE, evaluationRequest(expression)),
            request.timeout_ms, () => { lifecycle.cdp(target, CDP.RUNTIME_TERMINATE_EXECUTION).catch(() => {}); });
          if (evaluated.exceptionDetails) {
            return { value: "", truncated: false, exception: exceptionText(evaluated.exceptionDetails, request.max_result_chars) };
          }
          const serialized = JSON.stringify(evaluated.result?.value ?? null);
          const text = serialized.slice(0, request.max_result_chars);
          if (request.settle_ms) await sleep(request.settle_ms);
          return { value: text, truncated: serialized.length > text.length };
        });
      }
    });

    return Object.freeze({ evaluateMain });
  }

  return Object.freeze({ create });
});
