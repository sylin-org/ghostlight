// SPDX-License-Identifier: Apache-2.0 OR MIT
// Connection-scoped tool response delivery for the extension worker. Mechanism only.
(function initExecutionResponse(root) {
  "use strict";

  function createResponseScope(requestId, port, commandId) {
    const scope = { requestId, port };
    if (commandId !== undefined && commandId !== null) {
      scope.commandId = String(commandId);
    }
    return Object.freeze(scope);
  }

  function createConnectionResponder(port) {
    return Object.freeze({
      post(message) {
        try { port.postMessage(message); } catch { /* connection generation is gone */ }
      },
    });
  }

  function createToolResponder(executorGeneration) {
    function post(scope, message) {
      if (!scope || !scope.port) return;
      if (scope.commandId !== undefined) {
        message.commandId = scope.commandId;
        message.executorGeneration = executorGeneration;
      }
      try { scope.port.postMessage(message); } catch { /* connection generation is gone */ }
    }

    function reply(scope, result) {
      post(scope, {
        id: scope.requestId,
        type: "tool_response",
        result,
      });
    }

    function fail(scope, error) {
      const message = {
        id: scope.requestId,
        type: "tool_error",
        error: (error && error.message) || String(error),
      };
      if (error && error.hop) message.hop = error.hop;
      if (error && error.detail) message.detail = error.detail;
      if (error && error.code) message.code = error.code;
      post(scope, message);
    }

    return { fail, reply };
  }

  const GhostlightExecutionResponse = {
    createConnectionResponder,
    createResponseScope,
    createToolResponder,
  };
  if (typeof module !== "undefined" && module.exports) {
    module.exports = GhostlightExecutionResponse;
  } else {
    root.GhostlightExecutionResponse = GhostlightExecutionResponse;
  }
})(typeof self !== "undefined" ? self : globalThis);
