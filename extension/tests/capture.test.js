"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

function fixture(failCapture = false) {
  const calls = [];
  const sandbox = {
    ensureDebugger: async () => "lease",
    detachDebugger: async (...args) => calls.push(["detach", ...args]),
    contentAll: async (...args) => calls.push(["content", ...args]),
    screenshotApi: require("../lib/screenshot.js"),
    imageDimensions: async () => ({ width: 800, height: 600, outputScale: 1 }),
    chrome: { tabs: { getZoom: async () => 1 } },
    sendDebugger: async (_target, method, params) => {
      calls.push([method, params]);
      if (method === "Page.getLayoutMetrics") return {
        cssVisualViewport: { pageX: 0, pageY: 0, clientWidth: 800, clientHeight: 600 }
      };
      if (method === "Page.captureScreenshot") {
        if (failCapture) throw new Error("capture failed");
        return { data: "original-image" };
      }
      if (method === "Runtime.evaluate") return { result: { value: 1 } };
      throw new Error(`Unexpected command: ${method}`);
    }
  };
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  vm.createContext(sandbox);
  vm.runInContext(source.slice(source.indexOf("async function screenshot("),
    source.indexOf("async function imageDimensions(")), sandbox);
  return { sandbox, calls };
}

test("capture returns original browser pixels without document masks or an inventory", async () => {
  const { sandbox, calls } = fixture();
  const result = await sandbox.screenshot({ tab_id: 7, visual_settle: false });
  assert.equal(result.data, "original-image");
  assert.deepEqual(calls.map(([kind]) => kind), ["content", "Page.getLayoutMetrics",
    "Page.captureScreenshot", "Runtime.evaluate", "content", "detach"]);
  assert.equal(calls[0][2].kind, "presentation_visibility");
  assert.equal(calls[0][2].hidden, true);
  assert.equal(calls.at(-2)[2].hidden, false);
});

test("failed capture restores presentation and releases its debugger lease", async () => {
  const { sandbox, calls } = fixture(true);
  await assert.rejects(sandbox.screenshot({ tab_id: 7, visual_settle: false }), /capture failed/);
  assert.equal(calls.at(-2)[2].hidden, false);
  assert.deepEqual(calls.at(-1), ["detach", 7, "lease"]);
});
