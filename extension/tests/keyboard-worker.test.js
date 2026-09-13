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
    shared: {
      modifierMask: modifiers => modifiers.includes("Control") ? 2 : 0,
      keyDescriptor: key => ({
        key,
        code: `Key${key.toUpperCase()}`,
        windowsVirtualKeyCode: key.toUpperCase().charCodeAt(0),
        nativeVirtualKeyCode: key.toUpperCase().charCodeAt(0),
        text: key
      })
    },
    content: async () => ({ subject: "field" }),
    ensureDebugger: async () => {},
    detachDebugger: async () => {},
    sendDebugger: async (_target, method, params) => calls.push({ method, params }),
    chrome: { tabs: { get: async id => ({ id, status: "complete" }) } },
    physicalTab: tab => tab
  };
  vm.createContext(sandbox);
  const start = source.indexOf("async function pressKey(");
  const end = source.indexOf("async function dropImageAt(", start);
  assert.ok(start >= 0 && end > start);
  vm.runInContext(source.slice(start, end), sandbox);
  return { calls, sandbox };
}

test("modified printable keys dispatch physical shortcuts without literal text", async () => {
  const { calls, sandbox } = fixture();
  await sandbox.pressKey("press", {
    tab_id: 7,
    key: "a",
    modifiers: ["Control"]
  });

  assert.deepEqual(JSON.parse(JSON.stringify(calls[0])), {
    method: "Input.dispatchKeyEvent",
    params: {
      type: "keyDown",
      key: "a",
      code: "KeyA",
      windowsVirtualKeyCode: 65,
      nativeVirtualKeyCode: 65,
      modifiers: 2
    }
  });
  assert.equal(calls[1].params.type, "keyUp");
  assert.equal(Object.hasOwn(calls[1].params, "text"), false);
});

test("plain printable keys keep their text packet", async () => {
  const { calls, sandbox } = fixture();
  await sandbox.pressKey("press", {
    tab_id: 7,
    key: "x",
    modifiers: []
  });

  assert.equal(calls[0].params.text, "x");
});
