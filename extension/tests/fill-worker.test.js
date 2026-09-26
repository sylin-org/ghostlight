"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

function fixture() {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  const calls = [];
  const sandbox = {
    navigationWatchers: new Map(),
    cancelled: new Set(),
    frames: {
      frameOf: () => 0,
      localOf: locator => locator,
      groupLocators: () => new Map([[0, []]])
    },
    shared: require("../lib/shared.js"),
    contentIn: async (_tab, _frame, message) => {
      calls.push({ kind: message.kind });
      if (message.kind === "prepare_fill") return { field_kinds: ["browser_text"] };
      if (message.kind === "prepare_text_fill") return { subject: { role: "textbox", name: "Draft" } };
      if (message.kind === "verify_fill_values") return { retained: true };
      return { focused: true };
    },
    sendDebugger: async (_target, method, params) => { calls.push({ kind: method, params }); },
    ensureDebugger: async () => { calls.push({ kind: "attach" }); },
    detachDebugger: async () => { calls.push({ kind: "detach" }); },
    chrome: { tabs: { get: async id => ({ id, status: "complete" }) } },
    physicalTab: tab => tab,
    FILL_RETAINED_STABLE_MS: 0,
    FILL_RETAINED_LIMIT_MS: 1,
    FILL_RETAINED_POLL_MS: 0,
    setTimeout
  };
  vm.createContext(sandbox);
  const start = source.indexOf("function requireFillBudget(");
  const end = source.indexOf("async function typeText(", start);
  assert.ok(start >= 0 && end > start);
  vm.runInContext(source.slice(start, end), sandbox);
  return { calls, sandbox };
}

test("form text fill uses document-local focus and complete keyboard packets before verification", async () => {
  const { calls, sandbox } = fixture();
  const result = await sandbox.fill("fill", {
    tab_id: 7,
    timeout_ms: 1_000,
    fields: [{ locator: "locator_1", value: "Ab" }]
  });

  assert.equal(result.outcome, "filled");
  assert.equal(result.filled_count, 1);
  assert.equal(result.submitted, false);
  assert.deepEqual(calls.slice(0, 5).map(call => call.kind), [
    "prepare_fill",
    "attach",
    "prepare_text_fill",
    "verify_text_fill_focus",
    "Input.dispatchKeyEvent"
  ]);
  const insertedKeys = calls.filter(call => call.kind === "Input.dispatchKeyEvent"
    && call.params.type === "keyDown" && ["A", "b"].includes(call.params.key));
  assert.deepEqual(insertedKeys.map(call => call.params.key), ["A", "b"]);
  assert.ok(insertedKeys.every(call => call.params.unmodifiedText));
  assert.deepEqual(JSON.parse(JSON.stringify(insertedKeys.map(call => call.params))), [
    { type: "keyDown", key: "A", code: "KeyA", windowsVirtualKeyCode: 65, nativeVirtualKeyCode: 65,
      modifiers: 8, text: "A", unmodifiedText: "a" },
    { type: "keyDown", key: "b", code: "KeyB", windowsVirtualKeyCode: 66, nativeVirtualKeyCode: 66,
      text: "b", unmodifiedText: "b" }
  ]);
  const tabDown = calls.find(call => call.kind === "Input.dispatchKeyEvent"
    && call.params.type === "keyDown" && call.params.key === "Tab");
  assert.ok(tabDown);
  assert.ok(calls.findIndex(call => call.kind === "detach")
    > calls.findIndex(call => call === tabDown));
  assert.equal(calls.filter(call => call.kind === "verify_fill_values").length, 1);
  assert.ok(calls.findIndex(call => call.kind === "verify_fill_values")
    > calls.findIndex(call => call.kind === "detach"));
});

test("final batch verification catches a page rollback after browser input", async () => {
  const { calls, sandbox } = fixture();
  const contentIn = sandbox.contentIn;
  sandbox.contentIn = async (...args) => {
    if (args[2].kind === "verify_fill_values") {
      calls.push({ kind: args[2].kind });
      return { retained: false };
    }
    return contentIn(...args);
  };

  await assert.rejects(
    sandbox.fill("fill", { tab_id: 7, timeout_ms: 1_000, fields: [{ locator: "locator_1", value: "Ab" }] }),
    /did not retain/
  );
  assert.ok(calls.some(call => call.kind === "Input.dispatchKeyEvent"));
  assert.ok(calls.some(call => call.kind === "detach"));
});

test("multiline fill preserves Enter character payload only on key down", async () => {
  const { calls, sandbox } = fixture();
  await sandbox.fill("fill", { tab_id: 7, timeout_ms: 1_000, fields: [{ locator: "locator_1", value: "A\r\n\rB\n" }] });
  const enter = calls.filter(call => call.kind === "Input.dispatchKeyEvent" && call.params.key === "Enter");
  assert.equal(enter.length, 6);
  assert.ok(enter.filter(call => call.params.type === "keyDown")
    .every(call => call.params.text === "\r" && call.params.unmodifiedText === "\r"));
  assert.ok(enter.filter(call => call.params.type === "keyUp")
    .every(call => !Object.hasOwn(call.params, "text") && !Object.hasOwn(call.params, "unmodifiedText")));
});

test("failed text focus stops keyboard input and releases the debugger", async () => {
  const { calls, sandbox } = fixture();
  const contentIn = sandbox.contentIn;
  sandbox.contentIn = async (...args) => {
    if (args[2].kind === "prepare_text_fill") throw new Error("target did not retain browser input focus");
    return contentIn(...args);
  };
  await assert.rejects(sandbox.fill("fill", { tab_id: 7, timeout_ms: 1_000, fields: [{ locator: "locator_1", value: "Ab" }] }), /input focus/);
  assert.equal(calls.some(call => call.kind === "Input.dispatchKeyEvent"), false);
  assert.equal(calls.at(-1).kind, "detach");
});

test("an exhausted physical budget refuses before form observation or input", async () => {
  const { calls, sandbox } = fixture();
  await assert.rejects(
    sandbox.fill("fill", { tab_id: 7, timeout_ms: 0, fields: [{ locator: "locator_1", value: "Ab" }] }),
    /execution budget/
  );
  assert.deepEqual(calls, []);
});
