"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

test("native hello is first even when browser events arrive during the focus query", async () => {
  let resolveFocus, opened = 0, failure;
  const focus = new Promise(resolve => { resolveFocus = resolve; });
  const frames = [];
  const port = { onMessage: { addListener() {} }, onDisconnect: { addListener() {} } };
  const sandbox = {
    browserId: "browser_test", adapterEpoch: "adapter_test", nativePort: null,
    connectionAttemptNumber: 0, HOST_NAME: "org.sylin.ghostlight", ADAPTER_CAPABILITIES: [],
    connectionEvents: {}, connectionLog: { record() {} },
    holdsFocusedWindow: () => focus, initializeLocalState: async () => {},
    navigator: { userAgentData: { brands: [] } },
    shared: { ADAPTER_PROTOCOL_MAJOR: 3, browserName: () => null, bounded: value => String(value) },
    chrome: { runtime: { connectNative() { opened++; return port; }, getManifest: () => ({ version: "1.3.13" }) },
      alarms: { clear: async () => {} } },
    onNativeMessage: async () => {}, clearNativeRetryTimer() {}, scheduleNativeRetry() {},
    setConnection: value => { failure = value.last_error; }
  };
  sandbox.send = frame => { if (!sandbox.nativePort) return false; frames.push(frame); return true; };
  vm.createContext(sandbox);
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  vm.runInContext(source.match(/async function establishNativeConnection\([^]*?\n}/)[0], sandbox);
  const pending = sandbox.establishNativeConnection();
  const concurrent = sandbox.establishNativeConnection();
  assert.equal(opened, 0);
  sandbox.send({ kind: "event", event: { event: "attended" } });
  assert.equal(frames.length, 0);
  resolveFocus(true); await Promise.all([pending, concurrent]);
  assert.equal(opened, 1);
  assert.equal(failure, undefined);
  assert.equal(frames.length, 1); assert.equal(frames[0].kind, "hello");
  assert.equal(frames[0].attended, true);
});
