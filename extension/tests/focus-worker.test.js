"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");
const debuggerApi = require("../lib/debugger.js");

function fixture() {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  const attached = new Set();
  const focus = new Map();
  const owners = new Map([[7, "workspace"], [8, "other-workspace"]]);
  const lifecycle = debuggerApi.create({
    async attach({ tabId }) { attached.add(tabId); },
    async detach({ tabId }) { attached.delete(tabId); focus.delete(tabId); },
    async sendCommand({ tabId }, method, params) {
      if (method === "Emulation.setFocusEmulationEnabled") focus.set(tabId, params.enabled);
    }
  });
  const sandbox = {
    liveState: { connected: true, compatible: true },
    setConnection(patch) { Object.assign(sandbox.liveState, patch); },
    debuggerLifecycle: lifecycle,
    topology: { workspaceFor: id => owners.get(id), forget: async id => owners.delete(id),
      remember: async (id, workspace) => owners.set(id, workspace) },
    shared: { bounded: value => String(value) },
    content: async () => ({ text: "fixture page" }),
    commandChunks: { clear() {} },
    diagnostics: { clearAll: () => [] },
    diagnosticDocuments: new Map(),
    cancelled: new Set(),
    interruptAllRecordings: async () => {},
    disableDiagnosticCapture: async () => {},
    refreshFormDiagnostics: async () => {},
    syncFormDiagnostics: async () => {},
    broadcastRuntimeState: async () => {},
    tabPreservationEnabled: async () => true,
    chrome: { tabs: { remove() { assert.fail("preserved tabs must remain open"); } } }
  };
  vm.createContext(sandbox);
  for (const name of ["applyRuntimeState", "settleServiceBoundaryState", "ensureDebugger", "retainManagedDebugger", "dispatch"]) {
    const body = source.match(new RegExp(`async function ${name}\\([^]*?\\n}`));
    assert.ok(body, name);
    vm.runInContext(body[0], sandbox);
  }
  return { sandbox, lifecycle, attached, focus, owners };
}

test("runtime pause, attention, stop, and disconnect restore focus without affecting unowned tabs", async () => {
  for (const boundary of ["held", "attention", "ended", "disconnected"]) {
    const { sandbox, lifecycle, attached, focus } = fixture();
    await sandbox.applyRuntimeState("active");
    await sandbox.ensureDebugger(7);
    await lifecycle.release(7);
    await sandbox.ensureDebugger(99);
    assert.equal(focus.get(7), true);
    assert.equal(focus.has(99), false);
    if (boundary === "disconnected") await sandbox.settleServiceBoundaryState();
    else await sandbox.applyRuntimeState(boundary);
    assert.notEqual(focus.get(7), true, boundary);
    if (boundary === "ended") assert.equal(attached.size, 0);
    await lifecycle.detachAll();
  }
});

test("an incompatible or disconnected service cannot enable focus", async () => {
  for (const flag of ["compatible", "connected"]) {
    const { sandbox, lifecycle, focus } = fixture();
    sandbox.liveState[flag] = false;
    await sandbox.applyRuntimeState("active");
    await sandbox.ensureDebugger(7);
    assert.equal(focus.has(7), false);
    await lifecycle.detachAll();
  }
});

test("a preserved tab from a released workspace loses both ownership and debugger custody", async () => {
  const { sandbox, lifecycle, attached, focus, owners } = fixture();
  await sandbox.applyRuntimeState("active");
  for (const id of [7, 8]) {
    await sandbox.ensureDebugger(id);
    await lifecycle.release(id);
  }
  const close = released => sandbox.dispatch({ correlation: "close", command: { command: "close_tab", tab_id: 7, released } });
  await assert.rejects(close(false), { code: "local_interlock" });
  assert.equal(focus.get(7), true);
  assert.equal(owners.has(7), true);
  await assert.rejects(close(true), { code: "local_interlock" });
  assert.equal(owners.has(7), false);
  assert.equal(attached.has(7), false);
  assert.equal(focus.has(7), false);
  assert.equal(focus.get(8), true);
  await lifecycle.detachAll();
});

test("only new explicit debugger work restores focus after a local release", async () => {
  const { sandbox, lifecycle, attached, focus } = fixture();
  await sandbox.applyRuntimeState("active");
  await sandbox.ensureDebugger(7);
  await lifecycle.release(7);
  await lifecycle.detachAll();
  await sandbox.applyRuntimeState("active");
  assert.equal(attached.size, 0);
  await sandbox.ensureDebugger(7);
  assert.equal(focus.get(7), true);
  await lifecycle.release(7);
  await lifecycle.detachAll();
});

test("explicit service work restores custody after reload erased the adapter's session cache", async () => {
  const { sandbox, lifecycle, attached, focus, owners } = fixture();
  owners.clear();
  await sandbox.applyRuntimeState("active");
  assert.equal(attached.size, 0);
  const result = await sandbox.dispatch({ correlation: "read-after-reload", workspace: "service-owned-workspace",
    command: { command: "read_text", tab_id: 7, max_chars: 100 } });
  assert.equal(result.outcome, "text");
  assert.equal(owners.get(7), "service-owned-workspace");
  assert.equal(focus.get(7), true);
  assert.equal(attached.has(8), false);
  await lifecycle.detachAll();
});

test("released-tab cleanup cannot reacquire a lost workspace association", async () => {
  const { sandbox, lifecycle, attached, owners } = fixture();
  owners.clear();
  await sandbox.applyRuntimeState("active");
  await assert.rejects(sandbox.dispatch({ correlation: "release-after-reload", workspace: "released-workspace",
    command: { command: "close_tab", tab_id: 7, released: true } }), { code: "local_interlock" });
  assert.equal(owners.size, 0);
  assert.equal(attached.size, 0);
  await lifecycle.detachAll();
});

test("remembering service ownership only repairs opaque storage and never moves browser tabs", async () => {
  const topologyApi = require("../lib/topology.js");
  const saved = {};
  const topology = topologyApi.create({ storage: { session: {
    get: async () => saved,
    set: async patch => Object.assign(saved, structuredClone(patch))
  } } }, "topology");
  await topology.restore();
  await topology.remember(7, "workspace-a");
  await topology.remember(8, "workspace-b");
  const restored = topologyApi.create({ storage: { session: {
    get: async () => saved, set: async () => {}
  } }, tabs: { get: async id => ({ id }) } }, "topology");
  await restored.restore();
  assert.equal(restored.workspaceFor(7), "workspace-a");
  assert.equal(restored.workspaceFor(8), "workspace-b");
  assert.equal(restored.titleFor("workspace-a"), null);
  assert.equal(saved.topology.groups.length, 0);
});
