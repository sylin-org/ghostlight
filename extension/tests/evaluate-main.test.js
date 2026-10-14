"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Evaluate = require("../lib/evaluate.js");
const { TOP, TOP_LOADER, fakeChrome, wire, methods } = require("./p4-fakes.js");
const { golden, goldenArgs, execute } = require("./p4-chrome.js");

const TAB = 12;
const URL = "https://example.com/account";

function setup(handlers = {}) {
  const chrome = fakeChrome();
  Object.assign(chrome.handlers, handlers);
  const parts = wire(chrome);
  const slept = [];
  const { evaluateMain } = Evaluate.create({
    lifecycle: parts.lifecycle,
    inventory: parts.inventory,
    sleep: async (ms) => { slept.push(ms); }
  });
  return { chrome, ...parts, evaluateMain, slept };
}

const request = (overrides = {}) => ({ tab_id: TAB, script: "document.title", max_result_chars: 4000, settle_ms: 0, timeout_ms: 2000, ...overrides });
const answering = (value) => ({ "Runtime.evaluate": async () => ({ result: { value } }) });

test("the handler carries validate, run, addresses and tabOf", () => {
  const { evaluateMain } = setup();
  assert.equal(evaluateMain.addresses({}), true);
  assert.equal(evaluateMain.tabOf({ tab_id: 7 }), 7);
  assert.doesNotThrow(() => evaluateMain.validate(goldenArgs("evaluate_main")));
});

test("bad arguments are invalid_request before anything is sent", async () => {
  const { chrome, evaluateMain } = setup();
  const bad = [
    null,
    request({ tab_id: "12" }),
    request({ script: 7 }),
    request({ max_result_chars: 0 }),
    request({ settle_ms: -1 }),
    request({ timeout_ms: 0 }),
    request({ guard: { documents: [{ frame_id: TOP }], exact: false } }),
    request({ guard: { documents: [], exact: "no" } })
  ];
  for (const args of bad) {
    const result = await execute(evaluateMain, args);
    assert.equal(result.failure, "invalid_request", JSON.stringify(args));
    assert.equal(result.dispatched, false);
  }
  assert.deepEqual(chrome.calls, []);
});

test("a script runs in the main world of the top frame and answers JSON text", async () => {
  const { chrome, evaluateMain, lifecycle } = setup(answering("Account"));
  const guard = { documents: [{ frame_id: TOP, loader_id: TOP_LOADER, url: URL }], exact: false, unseen: 0 };
  const result = await execute(evaluateMain, { ...goldenArgs("evaluate_main"), guard });
  assert.deepEqual(result.ok, golden("receipts/evaluate_main.ok").ok);
  assert.equal(result.dispatched, true);
  assert.deepEqual(result.watched, [TAB]);
  const sent = chrome.calls.find((call) => call.method === "Runtime.evaluate");
  assert.deepEqual(sent.target, { tabId: TAB });
  assert.equal(sent.params.awaitPromise, true);
  assert.equal(sent.params.returnByValue, true);
  assert.equal(sent.params.userGesture, true);
  assert.equal(sent.params.replMode, true);
  assert.equal("contextId" in sent.params, false);
  assert.equal(sent.params.expression, "document.title");
  assert.equal(lifecycle.attachedCount(), 0, "the lease is given back");
});

test("a function body with return is wrapped, and the result is the returned value", async () => {
  const { chrome, evaluateMain } = setup(answering({ n: 1 }));
  const result = await execute(evaluateMain, request({ script: "const n = 1; return { n };" }));
  assert.deepEqual(result.ok, { value: "{\"n\":1}", truncated: false });
  assert.match(chrome.calls.find((call) => call.method === "Runtime.evaluate").params.expression, /^await \(async \(\) => \{/);
});

test("a result longer than max_result_chars is cut and marked truncated", async () => {
  const { evaluateMain } = setup(answering("a".repeat(40)));
  const result = await execute(evaluateMain, request({ max_result_chars: 17 }));
  assert.deepEqual(result.ok, { value: `"${"a".repeat(16)}`, truncated: true });
  assert.equal(result.ok.value.length, 17);
});

test("a result with no value is null", async () => {
  const { evaluateMain } = setup({ "Runtime.evaluate": async () => ({ result: { type: "undefined" } }) });
  assert.deepEqual((await execute(evaluateMain, request())).ok, { value: "null", truncated: false });
});

test("a page exception is a result with the bounded description, matching the golden receipt", async () => {
  const { evaluateMain } = setup({
    "Runtime.evaluate": async () => ({ exceptionDetails: { text: "Uncaught", exception: { description: "ReferenceError: nope is not defined" } } })
  });
  const result = await execute(evaluateMain, request({ script: "nope" }));
  assert.deepEqual(result.ok, golden("receipts/evaluate_main.ok_exception").ok);
  assert.equal(result.dispatched, true);
  const short = setup({
    "Runtime.evaluate": async () => ({ exceptionDetails: { exception: { description: "ReferenceError: nope is not defined" } } })
  });
  assert.equal((await execute(short.evaluateMain, request({ max_result_chars: 14 }))).ok.exception, "ReferenceError");
});

test("a script that does not parse is invalid_request and the browser is never touched", async () => {
  const { chrome, evaluateMain } = setup(answering("never"));
  const result = await execute(evaluateMain, request({ script: "let x = ;" }));
  assert.equal(result.failure, "invalid_request");
  assert.equal(result.dispatched, false);
  assert.deepEqual(result.watched, []);
  assert.deepEqual(chrome.calls, []);
});

test("a guard that fails stops the script before it is sent", async () => {
  const { chrome, evaluateMain, lifecycle } = setup(answering("never"));
  const guard = { documents: [{ frame_id: TOP, loader_id: "other-loader", url: URL }], exact: false };
  const result = await execute(evaluateMain, request({ guard }));
  assert.equal(result.failure, "document_changed");
  assert.equal(result.dispatched, false);
  assert.equal(methods(chrome, "Runtime.evaluate").length, 0);
  assert.equal(lifecycle.attachedCount(), 0);
  const current = { documents: [{ frame_id: TOP, loader_id: TOP_LOADER, url: URL }], exact: false };
  assert.equal((await execute(evaluateMain, request({ guard: current }))).dispatched, true);
});

test("an open dialog stops the script before it is sent", async () => {
  const { chrome, evaluateMain, lifecycle } = setup(answering("never"));
  lifecycle.openDialog(TAB, { type: "alert", message: "hi" });
  const result = await execute(evaluateMain, request());
  assert.equal(result.failure, "dialog_open");
  assert.equal(result.dispatched, false);
  assert.equal(methods(chrome, "Runtime.evaluate").length, 0);
});

test("at the deadline the page is told to stop and the primitive times out having dispatched", async () => {
  const { chrome, evaluateMain, lifecycle } = setup({ "Runtime.evaluate": () => new Promise(() => {}) });
  const result = await execute(evaluateMain, request({ timeout_ms: 20 }));
  assert.equal(result.failure, "timeout");
  assert.equal(result.message, "The script did not finish in 20 ms.");
  assert.equal(result.dispatched, true);
  const terminate = chrome.calls.filter((call) => call.method === "Runtime.terminateExecution");
  assert.equal(terminate.length, 1);
  assert.deepEqual(terminate[0].target, { tabId: TAB });
  assert.equal(lifecycle.attachedCount(), 0);
});

test("a script that finishes before the deadline never terminates anything", async () => {
  const { chrome, evaluateMain } = setup(answering(1));
  await execute(evaluateMain, request({ timeout_ms: 20 }));
  await new Promise((resolve) => setTimeout(resolve, 40));
  assert.equal(methods(chrome, "Runtime.terminateExecution").length, 0);
});

test("a late answer after the deadline is ignored", async () => {
  let finish;
  const { evaluateMain } = setup({ "Runtime.evaluate": () => new Promise((resolve) => { finish = resolve; }) });
  const result = await execute(evaluateMain, request({ timeout_ms: 10 }));
  assert.equal(result.failure, "timeout");
  finish({ result: { value: 1 } });
  await new Promise((resolve) => setTimeout(resolve, 5));
});

test("settle_ms waits after a result, and a refused evaluation still gives the lease back", async () => {
  const settled = setup(answering(1));
  await execute(settled.evaluateMain, request({ settle_ms: 150 }));
  assert.deepEqual(settled.slept, [150]);
  const refused = setup({ "Runtime.evaluate": async () => { throw new Error("Cannot find context"); } });
  const result = await execute(refused.evaluateMain, request());
  assert.equal(result.message, "Cannot find context");
  assert.equal(result.dispatched, true);
  assert.equal(refused.lifecycle.attachedCount(), 0);
});
