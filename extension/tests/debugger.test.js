"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const debuggerApi = require("../lib/debugger.js");

function fakeDebugger() {
  const calls = [];
  const focus = new Map();
  return {
    calls,
    focus,
    async attach(target, version) { calls.push(["attach", target.tabId, version]); },
    async detach(target) { calls.push(["detach", target.tabId]); focus.delete(target.tabId); },
    async sendCommand(target, method, params) {
      calls.push(["command", target.tabId, method]);
      if (method === "Emulation.setFocusEmulationEnabled") focus.set(target.tabId, params.enabled);
    }
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

test("runtime installation reaches every execution context already present at attachment", async () => {
  const commands = [];
  const lifecycle = debuggerApi.create({
    async attach() {},
    async detach() {},
    async sendCommand(target, method, params) {
      commands.push({ tabId: target.tabId, method, params });
    }
  });
  await lifecycle.installPageRuntime("globalThis.runtimeInstalled = true;");
  await lifecycle.retain(81);

  const installation = commands.find((command) => command.method === "Page.addScriptToEvaluateOnNewDocument");
  assert.deepEqual(installation, {
    tabId: 81,
    method: "Page.addScriptToEvaluateOnNewDocument",
    params: {
      source: "globalThis.runtimeInstalled = true;",
      runImmediately: true
    }
  });
  assert.equal(commands.some((command) => command.method === "Runtime.evaluate"), false);
  await lifecycle.detachAll();
});

test("runtime installation recursively covers an existing out-of-process iframe", async () => {
  const commands = [];
  let onEvent;
  const lifecycle = debuggerApi.create({
    onEvent: { addListener(listener) { onEvent = listener; } },
    async attach() {},
    async detach() {},
    async sendCommand(target, method, params) {
      commands.push({ target, method, params });
      if (method === "Target.setAutoAttach" && !target.sessionId) {
        onEvent(
          { tabId: target.tabId },
          "Target.attachedToTarget",
          { sessionId: "child-1", targetInfo: { type: "iframe" }, waitingForDebugger: true }
        );
      }
    }
  });

  await lifecycle.installPageRuntime("globalThis.runtimeInstalled = true;");
  await lifecycle.retain(82);

  const childCommands = commands.filter((command) => command.target.sessionId === "child-1");
  assert.deepEqual(childCommands.map((command) => command.method), [
    "Page.enable",
    "Page.addScriptToEvaluateOnNewDocument",
    "Target.setAutoAttach",
    "Runtime.runIfWaitingForDebugger"
  ]);
  assert.equal(childCommands[1].params.runImmediately, true);
  await lifecycle.detachAll();
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

test("only retained tabs emulate focus, and only after active runtime negotiation", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.retain(21);
  await lifecycle.acquire(22);
  assert.equal(chromeDebugger.focus.size, 0);

  await lifecycle.setFocusEmulationEnabled(true);
  assert.equal(chromeDebugger.focus.get(21), true);
  assert.equal(chromeDebugger.focus.has(22), false);
  await lifecycle.acquire(21);
  await lifecycle.release(21);
  assert.equal(chromeDebugger.focus.get(21), true);
  await lifecycle.retain(23);
  assert.equal(chromeDebugger.focus.get(23), true);
  await lifecycle.detachAll();
});

test("pause restores focus on every retained tab and resume preserves their attachments", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  await Promise.all([lifecycle.retain(24), lifecycle.retain(25)]);
  await lifecycle.setFocusEmulationEnabled(false);
  assert.deepEqual([...chromeDebugger.focus.values()], [false, false]);
  assert.equal(lifecycle.attachedCount(), 2);
  await lifecycle.setFocusEmulationEnabled(true);
  assert.deepEqual([...chromeDebugger.focus.values()], [true, true]);
  assert.equal(chromeDebugger.calls.filter(([kind]) => kind === "attach").length, 2);
  await lifecycle.detachAll();
});

test("releasing ownership restores focus immediately while a command still holds its lease", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.retain(26);
  await lifecycle.acquire(26);
  await lifecycle.unretain(26);
  assert.equal(chromeDebugger.focus.get(26), false);
  assert.equal(lifecycle.attachedCount(), 1);
  await lifecycle.release(26);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.has(26), false);
});

test("shutdown and local release clear focus before detaching every tab", async () => {
  const chromeDebugger = fakeDebugger();
  const transitions = [];
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (method === "Emulation.setFocusEmulationEnabled") transitions.push([target.tabId, params.enabled]);
    return sendCommand(target, method, params);
  };
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  await Promise.all([lifecycle.retain(27), lifecycle.retain(28)]);
  await lifecycle.detachAll();
  assert.deepEqual(transitions.sort((a, b) => a[0] - b[0]), [[27, true], [27, false], [28, true], [28, false]]);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
});

test("a pause that arrives during focus enablement wins before setup finishes", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  let completeEnable;
  chromeDebugger.sendCommand = async (target, method, params) => {
    await sendCommand(target, method, params);
    if (params?.enabled) await new Promise(resolve => { completeEnable = resolve; });
  };
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  const retaining = lifecycle.retain(29);
  await new Promise(resolve => setImmediate(resolve));
  const pausing = lifecycle.setFocusEmulationEnabled(false);
  completeEnable();
  await Promise.all([retaining, pausing]);
  assert.equal(chromeDebugger.focus.get(29), false);
  await lifecycle.detachAll();
});

test("a released tab cannot regain focus from setup already in flight", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  let completePageEnable;
  chromeDebugger.sendCommand = async (target, method, params) => {
    await sendCommand(target, method, params);
    if (method === "Page.enable") await new Promise(resolve => { completePageEnable = resolve; });
  };
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  const retaining = lifecycle.retain(30);
  await new Promise(resolve => setImmediate(resolve));
  const releasing = lifecycle.unretain(30);
  completePageEnable();
  await Promise.all([retaining, releasing]);
  assert.equal(chromeDebugger.focus.size, 0);
  assert.equal(lifecycle.attachedCount(), 0);
});

test("an external detach stays detached until an explicit operation reacquires the tab", async () => {
  const chromeDebugger = fakeDebugger();
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.retain(31);
  await chromeDebugger.detach({ tabId: 31 });
  lifecycle.detached(31);
  await lifecycle.setFocusEmulationEnabled(false);
  await lifecycle.setFocusEmulationEnabled(true);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
  await lifecycle.acquire(31);
  assert.equal(chromeDebugger.focus.get(31), true);
  await lifecycle.release(31);
  await lifecycle.detachAll();
});

test("failed focus enablement releases its possibly applied override and permits a later retry", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  let fail = true;
  chromeDebugger.sendCommand = async (target, method, params) => {
    await sendCommand(target, method, params);
    if (params?.enabled && fail) throw new Error("focus command failed after dispatch");
  };
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  await assert.rejects(lifecycle.retain(32), /focus command failed/);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
  fail = false;
  await lifecycle.acquire(32);
  assert.equal(chromeDebugger.focus.get(32), true);
  await lifecycle.release(32);
  await lifecycle.detachAll();
});

test("a failed pause on one tab still restores every other tab", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (target.tabId === 33 && params?.enabled === false) throw new Error("focus disable failed");
    await sendCommand(target, method, params);
  };
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  await Promise.all([lifecycle.retain(33), lifecycle.retain(34)]);
  await assert.rejects(lifecycle.setFocusEmulationEnabled(false), /Could not update controlled-tab focus/);
  assert.equal(chromeDebugger.focus.has(33), false);
  assert.equal(chromeDebugger.focus.get(34), false);
  await lifecycle.detachAll();
});

test("unconfirmed cleanup remains tracked and a later release retries it", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  const detach = chromeDebugger.detach;
  let fail = false;
  chromeDebugger.sendCommand = async (target, method, params) => {
    if (fail && params?.enabled === false) throw new Error("focus disable unavailable");
    return sendCommand(target, method, params);
  };
  chromeDebugger.detach = async target => {
    if (fail) throw new Error("debugger detach unavailable");
    return detach(target);
  };
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  await lifecycle.retain(35);
  fail = true;
  await assert.rejects(lifecycle.detachAll(), /Could not release every debugger session/);
  assert.equal(lifecycle.attachedCount(), 1);
  assert.equal(chromeDebugger.focus.get(35), true);
  fail = false;
  await lifecycle.detachAll();
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
});

test("stop during attachment rejects unfinished setup and leaves no session", async () => {
  const chromeDebugger = fakeDebugger();
  const attach = chromeDebugger.attach;
  let completeAttach;
  chromeDebugger.attach = async (target, version) => {
    await attach(target, version);
    await new Promise(resolve => { completeAttach = resolve; });
  };
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  const retaining = assert.rejects(lifecycle.retain(36), /released during setup/);
  await new Promise(resolve => setImmediate(resolve));
  const stopping = lifecycle.detachAll();
  completeAttach();
  await Promise.all([retaining, stopping]);
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
  assert.equal(chromeDebugger.calls.filter(([, , method]) => method === "Emulation.setFocusEmulationEnabled").length, 0);
});

test("a late focus acknowledgement cannot resurrect an externally detached session", async () => {
  const chromeDebugger = fakeDebugger();
  const sendCommand = chromeDebugger.sendCommand;
  let completeEnable;
  chromeDebugger.sendCommand = async (target, method, params) => {
    await sendCommand(target, method, params);
    if (params?.enabled) await new Promise(resolve => { completeEnable = resolve; });
  };
  const lifecycle = debuggerApi.create(chromeDebugger);
  await lifecycle.setFocusEmulationEnabled(true);
  const retaining = assert.rejects(lifecycle.retain(37), /released during setup/);
  await new Promise(resolve => setImmediate(resolve));
  await chromeDebugger.detach({ tabId: 37 });
  lifecycle.detached(37);
  completeEnable();
  await retaining;
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chromeDebugger.focus.size, 0);
  await lifecycle.detachAll();
});
