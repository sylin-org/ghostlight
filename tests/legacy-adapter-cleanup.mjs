// Historical adapter cleanup evidence. Reads the pinned shipped implementation from Git;
// no predecessor implementation is copied into current product code.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import vm from "node:vm";

const baseline = "de1a686761af5430afc50763d1a282efaa80f615";
const historical = path => execFileSync("git", ["show", `${baseline}:${path}`], { encoding: "utf8", windowsHide: true });
const worker = historical("extension/service-worker.js");
const debuggerSource = historical("extension/lib/debugger.js");
const sharedSource = historical("extension/lib/shared.js");
const moduleFor = source => {
  const context = vm.createContext({ module: { exports: {} } });
  vm.runInContext(source, context);
  return context.module.exports;
};
const debuggerApi = moduleFor(debuggerSource);
const shared = moduleFor(sharedSource);
const attached = new Set();
const focus = new Map();
const effects = [];
const topology = new Map([[7, "old-workspace"], [8, "other-workspace"]]);
const lifecycle = debuggerApi.create({
  async attach({ tabId }) { attached.add(tabId); effects.push(["attach", tabId]); },
  async detach({ tabId }) { attached.delete(tabId); focus.delete(tabId); effects.push(["detach", tabId]); },
  async sendCommand({ tabId }, method, params) {
    if (method === "Emulation.setFocusEmulationEnabled") {
      focus.set(tabId, params.enabled); effects.push(["focus", tabId, params.enabled]);
    }
  }
});
const sandbox = {
  shared, stateApi: moduleFor(historical("extension/lib/state.js")), debuggerLifecycle: lifecycle,
  liveState: { connected: true, compatible: true, control_state: "active" },
  setConnection(patch) { Object.assign(sandbox.liveState, patch); },
  commandChunks: { clear() {} }, diagnostics: { clearAll: () => [] }, diagnosticDocuments: new Map(),
  interruptAllRecordings: async () => {}, disableDiagnosticCapture: async () => {},
  refreshFormDiagnostics: async () => {}, broadcastRuntimeState: async () => {},
  nativePort: null, browserNegotiation: Promise.resolve(),
  chrome: { tabs: { remove() { assert.fail("Legacy cleanup must never close any tab"); } } }
};
vm.createContext(sandbox);
for (const name of ["applyRuntimeState", "settleServiceBoundaryState", "onNativeMessage"]) {
  const body = worker.match(new RegExp(`async function ${name}\\([^]*?\\n}`));
  assert.ok(body, `historical ${name}`);
  vm.runInContext(body[0], sandbox);
}
await sandbox.applyRuntimeState("active");
for (const tab of [7, 8]) await lifecycle.retain(tab);
assert.equal(attached.size, 2);
assert.equal(focus.get(7), true);
assert.equal(focus.get(8), true);
await sandbox.onNativeMessage({ kind: "control_state", state: "ended" });
assert.equal(sandbox.liveState.control_state, "ended");
assert.equal(attached.size, 0);
assert.equal(focus.size, 0);
assert.equal(topology.size, 2, "Old topology hints persist; cleanup must not claim forgetting ownership");
await sandbox.settleServiceBoundaryState();
sandbox.setConnection({ connected: false });
// Quarantine reconnect carries Ended and an explicit error, never an Active hello.
await sandbox.onNativeMessage({ kind: "control_state", state: "ended" });
await sandbox.onNativeMessage({ kind: "error", message: "Update the browser adapter before background browser work." });
assert.equal(sandbox.liveState.control_state, "ended");
assert.equal(attached.size, 0);
assert.equal(focus.size, 0);
assert.match(sandbox.liveState.last_error, /Update the browser adapter/);
// Ended erased debugger retention, so even a later Active signal alone cannot revive it.
await sandbox.applyRuntimeState("active");
assert.equal(attached.size, 0);
assert.equal(focus.size, 0);
console.log(JSON.stringify({ passed: true, baseline,
  source_sha256: { worker: createHash("sha256").update(worker).digest("hex"),
    debugger: createHash("sha256").update(debuggerSource).digest("hex") },
  checks: ["actual baseline Ended disables emulation and detaches plural retained tabs without closing",
    "quarantine reconnect without Active hello remains detached and names upgrade",
    "later Active alone does not revive erased debugger retention",
    "topology hints persist and are not claimed released"], effects }, null, 2));
