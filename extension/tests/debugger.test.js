"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const debuggerApi = require("../lib/debugger.js");

function fakeDebugger() {
  const calls = [];
  return {
    calls,
    async attach(target, version) { calls.push(["attach", target.tabId, version]); },
    async detach(target) { calls.push(["detach", target.tabId]); },
    async sendCommand(target, method) { calls.push(["command", target.tabId, method]); }
  };
}

test("concurrent debugger users share one attachment", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);

  await Promise.all([lifecycle.acquire(7), lifecycle.acquire(7)]);
  assert.deepEqual(chromeDebugger.calls, [
    ["attach", 7, "1.3"],
    ["command", 7, "Page.enable"]
  ]);
  assert.equal(lifecycle.attachedCount(), 1);

  await lifecycle.release(7);
  assert.equal(lifecycle.attachedCount(), 1);
  await lifecycle.release(7);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(chromeDebugger.calls.at(-1), ["detach", 7]);
});

test("a retained controlled tab stays attached between sequential operations", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);

  await lifecycle.retain(8);
  await lifecycle.acquire(8);
  await lifecycle.release(8);
  await lifecycle.acquire(8);
  await lifecycle.release(8);

  assert.equal(lifecycle.attachedCount(), 1);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "attach"), [["attach", 8, "1.3"]]);
  assert.equal(chromeDebugger.calls.filter(([kind]) => kind === "detach").length, 0);

  lifecycle.forget(8);
  assert.equal(lifecycle.attachedCount(), 0);
});

test("terminal shutdown detaches retained controlled tabs", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);

  await Promise.all([lifecycle.retain(17), lifecycle.retain(18)]);
  await lifecycle.detachAll();

  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "detach").sort(), [
    ["detach", 17],
    ["detach", 18]
  ]);
});

test("an open JavaScript dialog retains its debugger session until handled", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);

  await lifecycle.acquire(9);
  lifecycle.openDialog(9, "prompt");
  await lifecycle.release(9);

  assert.deepEqual(lifecycle.currentDialog(9), { type: "prompt" });
  assert.equal(lifecycle.attachedCount(), 1);
  assert.equal(chromeDebugger.calls.filter(([kind]) => kind === "detach").length, 0);

  await lifecycle.acquire(9);
  await lifecycle.closeDialog(9);
  assert.equal(lifecycle.attachedCount(), 1);
  await lifecycle.release(9);

  assert.equal(lifecycle.currentDialog(9), null);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "attach"), [["attach", 9, "1.3"]]);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "detach"), [["detach", 9]]);
});

test("an external detach preserves a known dialog for a later handling lease", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);

  await lifecycle.acquire(11);
  lifecycle.openDialog(11, "confirm");
  lifecycle.detached(11);
  await lifecycle.release(11);
  assert.deepEqual(lifecycle.currentDialog(11), { type: "confirm" });

  await lifecycle.acquire(11);
  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "attach"), [
    ["attach", 11, "1.3"],
    ["attach", 11, "1.3"]
  ]);
  await lifecycle.closeDialog(11);
  await lifecycle.release(11);
});

test("a new lease waits for an in-flight detach and then reattaches", async () => {
  const calls = [];
  let finishFirstDetach;
  let detachCount = 0;
  const lifecycle = debuggerApi.create({
    async attach(target, version) { calls.push(["attach", target.tabId, version]); },
    async detach(target) {
      calls.push(["detach", target.tabId]);
      detachCount += 1;
      if (detachCount === 1) await new Promise((resolve) => { finishFirstDetach = resolve; });
    },
    async sendCommand(target, method) { calls.push(["command", target.tabId, method]); }
  });

  await lifecycle.acquire(12);
  const releasing = lifecycle.release(12);
  await new Promise((resolve) => setImmediate(resolve));
  const acquiring = lifecycle.acquire(12);
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(calls.filter(([kind]) => kind === "attach").length, 1);

  finishFirstDetach();
  await Promise.all([releasing, acquiring]);
  assert.equal(calls.filter(([kind]) => kind === "attach").length, 2);
  assert.equal(lifecycle.attachedCount(), 1);
  await lifecycle.release(12);
});

test("failed Page enablement leaves no attached lifecycle state", async () => {
  const calls = [];
  const lifecycle = debuggerApi.create({
    async attach(target, version) { calls.push(["attach", target.tabId, version]); },
    async detach(target) { calls.push(["detach", target.tabId]); },
    async sendCommand() { throw new Error("Page domain unavailable"); }
  });

  await assert.rejects(lifecycle.acquire(13), /Page domain unavailable/);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.deepEqual(calls, [["attach", 13, "1.3"], ["detach", 13]]);
});

test("optional CDP domains share the existing attachment", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);

  await lifecycle.retain(14);
  assert.equal(await lifecycle.enableDomain(14, "Runtime"), true);
  assert.equal(await lifecycle.enableDomain(14, "Runtime"), false);
  assert.equal(await lifecycle.enableDomain(14, "Network"), true);
  await lifecycle.disableDomain(14, "Network");

  assert.deepEqual(chromeDebugger.calls.filter(([kind]) => kind === "attach"), [["attach", 14, "1.3"]]);
  assert.deepEqual(chromeDebugger.calls.filter(([, , method]) => method === "Runtime.enable"), [
    ["command", 14, "Runtime.enable"]
  ]);
  assert.deepEqual(chromeDebugger.calls.filter(([, , method]) => method === "Network.enable"), [
    ["command", 14, "Network.enable"]
  ]);
  assert.deepEqual(chromeDebugger.calls.filter(([, , method]) => method === "Network.disable"), [
    ["command", 14, "Network.disable"]
  ]);
});
