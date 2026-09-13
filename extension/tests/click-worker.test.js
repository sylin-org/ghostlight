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
    frames: { frameOf: () => 4, TOP_FRAME_ID: 0 },
    shared: { modifierMask: () => 10 },
    ensureDebugger: async tabId => { calls.push({ kind: "attach", tabId }); },
    detachDebugger: async tabId => { calls.push({ kind: "detach", tabId }); },
    content: async () => {
      calls.push({ kind: "prepare" });
      return {
        subject: { role: "textbox", name: "First name" },
        rectangle: { left: 10, top: 20, width: 100, height: 30 }
      };
    },
    frameViewportOffset: async () => ({ x: 5, y: 7 }),
    dispatchClick: async (_tabId, point, button, clickCount, modifiers) => {
      calls.push({ kind: "trusted_click", point, button, clickCount, modifiers });
    },
    chrome: { tabs: { get: async id => ({ id, status: "complete" }) } },
    physicalTab: tab => tab,
    setTimeout: callback => callback()
  };
  vm.createContext(sandbox);
  const start = source.indexOf("async function activate(");
  const end = source.indexOf("async function fill(", start);
  assert.ok(start >= 0 && end > start);
  vm.runInContext(source.slice(start, end), sandbox);
  return { calls, sandbox };
}

test("semantic target activation uses a trusted pointer at the live element center", async () => {
  const { calls, sandbox } = fixture();
  const result = await sandbox.activate("click", {
    tab_id: 7,
    locator: "frame:4:target",
    button: "primary",
    click_count: 1,
    modifiers: ["Control"]
  });

  assert.deepEqual(calls.map(call => call.kind), ["attach", "prepare", "trusted_click", "detach"]);
  const click = calls[2];
  assert.equal(click.point.x, 65);
  assert.equal(click.point.y, 42);
  assert.equal(click.button, "primary");
  assert.equal(click.clickCount, 1);
  assert.equal(click.modifiers, 10);
  assert.equal(result.subject.name, "First name");
});
