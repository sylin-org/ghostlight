(function installGhostlightScriptEvaluator(root, factory) {
  const api = factory();
  root.GhostlightScriptEvaluator = api;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
})(globalThis, function createGhostlightScriptEvaluator() {
  "use strict";

  const BARE_RETURN_MARKER = "Illegal return statement";
  const SYNTAX_ERROR_CLASS = "SyntaxError";
  const UNPARSED_FAILURE = "page script failed";

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
    return `(async () => {\n${script}\n})()`;
  }

  function isBareReturnFailure(details) {
    return String(details?.exception?.description ?? "").includes(BARE_RETURN_MARKER);
  }

  function isParseFailure(details) {
    const className = String(details?.exception?.className ?? "");
    const description = String(details?.exception?.description ?? "");
    return className === SYNTAX_ERROR_CLASS || description.startsWith(`${SYNTAX_ERROR_CLASS}:`);
  }

  function failureDescription(details, maximum) {
    const raw = String(details?.exception?.description || details?.text || UNPARSED_FAILURE);
    return raw.slice(0, maximum);
  }

  function failureError(details, maximum) {
    const parseFailure = isParseFailure(details);
    const error = new Error(failureDescription(details, maximum));
    error.code = parseFailure ? "invalid_script" : "primitive_failed";
    error.effectUnknown = !parseFailure;
    return error;
  }

  async function evaluate(send, script, maximum) {
    const evaluated = await send("Runtime.evaluate", evaluationRequest(script));
    if (!evaluated.exceptionDetails) {
      return evaluated.result?.value ?? null;
    }
    if (isBareReturnFailure(evaluated.exceptionDetails)) {
      const retried = await send("Runtime.evaluate", evaluationRequest(wrappedExpression(script)));
      if (!retried.exceptionDetails) {
        return retried.result?.value ?? null;
      }
      throw failureError(retried.exceptionDetails, maximum);
    }
    throw failureError(evaluated.exceptionDetails, maximum);
  }

  return { evaluate };
});
