// SPDX-License-Identifier: Apache-2.0 OR MIT

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const {
  createConnectionResponder,
  createResponseScope,
  createToolResponder,
} = require("../../extension/lib/execution-response.js");

function deferred() {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
}

test("responses stay on their original connection when request ids are reused", async () => {
  const oldMessages = [];
  const newMessages = [];
  const oldPort = { postMessage: (message) => oldMessages.push(message) };
  const newPort = { postMessage: (message) => newMessages.push(message) };
  const oldScope = createResponseScope("7", oldPort, "old-command");
  const newScope = createResponseScope("7", newPort, "new-command");
  const responder = createToolResponder("executor-generation");
  const oldGate = deferred();

  const oldCompletion = oldGate.promise.then(() => {
    responder.reply(oldScope, { source: "old" });
  });
  const newError = new Error("new failure");
  newError.hop = "page";
  newError.detail = "new detail";
  responder.fail(newScope, newError);
  oldGate.resolve();
  await oldCompletion;

  assert.strictEqual(Object.isFrozen(oldScope), true);
  assert.strictEqual(Object.isFrozen(newScope), true);
  assert.deepStrictEqual(oldMessages, [{
    id: "7",
    type: "tool_response",
    result: { source: "old" },
    commandId: "old-command",
    executorGeneration: "executor-generation",
  }]);
  assert.deepStrictEqual(newMessages, [{
    id: "7",
    type: "tool_error",
    error: "new failure",
    hop: "page",
    detail: "new detail",
    commandId: "new-command",
    executorGeneration: "executor-generation",
  }]);
});

test("pre-executor failures stay on the connection that received the request", () => {
  const oldMessages = [];
  const newMessages = [];
  const responder = createToolResponder("executor-generation");
  const oldScope = createResponseScope("same", {
    postMessage: (message) => oldMessages.push(message),
  });
  const newScope = createResponseScope("same", {
    postMessage: (message) => newMessages.push(message),
  });

  responder.fail(newScope, new Error("new request rejected"));
  responder.fail(oldScope, new Error("old request rejected"));

  assert.deepStrictEqual(oldMessages, [{
    id: "same",
    type: "tool_error",
    error: "old request rejected",
  }]);
  assert.deepStrictEqual(newMessages, [{
    id: "same",
    type: "tool_error",
    error: "new request rejected",
  }]);
});

test("a delayed tab URL response cannot cross into a newer connection", async () => {
  const oldMessages = [];
  const newMessages = [];
  const oldResponder = createConnectionResponder({
    postMessage: (message) => oldMessages.push(message),
  });
  const newResponder = createConnectionResponder({
    postMessage: (message) => newMessages.push(message),
  });
  const oldGate = deferred();

  const oldCompletion = oldGate.promise.then(() => {
    oldResponder.post({
      id: "reused",
      type: "tab_url_response",
      result: { url: "https://old.example/" },
    });
  });
  newResponder.post({
    id: "reused",
    type: "tab_url_response",
    result: { url: "https://new.example/" },
  });
  oldGate.resolve();
  await oldCompletion;

  assert.deepStrictEqual(oldMessages, [{
    id: "reused",
    type: "tab_url_response",
    result: { url: "https://old.example/" },
  }]);
  assert.deepStrictEqual(newMessages, [{
    id: "reused",
    type: "tab_url_response",
    result: { url: "https://new.example/" },
  }]);
});

test("worker carries response scopes instead of looking up raw request ids", () => {
  const source = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  assert.doesNotMatch(source, /executionByRequest/);
  assert.match(source, /execute: async \(item\) => \{\s*await dispatch\(item\)/);
  assert.match(source, /reply\(item\.response, await handler\(args, key, request\.workspace\)\)/);
  assert.match(
    source,
    /createResponseScope\(requestId, connectedPort\)/,
    "chunk failures must remain bound to the port that received the chunks"
  );
  assert.match(
    source,
    /createResponseScope\(msg\.id, connectedPort\)/,
    "kill-switch failures must remain bound to the port that received the request"
  );
  const auxiliaryStart = source.indexOf('if (msg && msg.type === "tab_url_request"');
  const auxiliaryEnd = source.indexOf("// On-screen notification", auxiliaryStart);
  assert.ok(auxiliaryStart >= 0 && auxiliaryEnd > auxiliaryStart);
  const auxiliaryResponses = source.slice(auxiliaryStart, auxiliaryEnd);
  assert.doesNotMatch(auxiliaryResponses, /\bnativePort\b/);
  assert.strictEqual(
    (auxiliaryResponses.match(/connectedResponder\.post/g) || []).length,
    4,
    "tab URL and group success/failure responses must use the connection responder"
  );
});
