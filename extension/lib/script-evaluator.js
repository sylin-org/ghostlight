// Browser-local script form selection and CDP execution. Syntax is parsed before any page effect;
// browser exceptions carry uncertainty, never authority to retry the supplied program.
(function installGhostlightScriptEvaluator(root, factory) {
  const parser = typeof module !== "undefined" && module.exports ? require("../vendor/acorn.js") : root.acorn;
  const api = factory(parser);
  root.GhostlightScriptEvaluator = api;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
})(globalThis, function createGhostlightScriptEvaluator(parser) {
  "use strict";

  const UNPARSED_FAILURE = "page script failed";
  const FUNCTION_NODES = new Set(["FunctionDeclaration", "FunctionExpression", "ArrowFunctionExpression"]);
  const PARSE_OPTIONS = Object.freeze({ ecmaVersion: "latest", allowAwaitOutsideFunction: true });
  const WRAPPER_PREFIX = "await (async () => {\n";
  const WRAPPER_SUFFIX = "\n})()";

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

  function prepareExpression(script, maximum) {
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
      const error = new Error(description.slice(0, maximum));
      error.code = "invalid_script";
      error.effectUnknown = false; // The adapter has not sent an evaluation at all.
      throw error;
    }
  }

  function failureDescription(details, maximum) {
    const raw = String(details?.exception?.description || details?.text || UNPARSED_FAILURE);
    return raw.slice(0, maximum);
  }

  function failureError(details, maximum) {
    const error = new Error(failureDescription(details, maximum));
    error.code = "primitive_failed";
    error.effectUnknown = true;
    return error;
  }

  async function evaluate(send, script, maximum) {
    const expression = prepareExpression(script, maximum);
    let evaluated;
    try {
      evaluated = await send("Runtime.evaluate", evaluationRequest(expression));
    } catch (error) {
      error.effectUnknown = true;
      throw error;
    }
    if (!evaluated.exceptionDetails) {
      return evaluated.result?.value ?? null;
    }
    throw failureError(evaluated.exceptionDetails, maximum);
  }

  return { evaluate };
});
