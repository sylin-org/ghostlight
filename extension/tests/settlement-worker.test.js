"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const api = require("../lib/documents.js");
const frames = require("../lib/frames.js");

function fixture({ quiet = false, cancel = false, churn = false } = {}) {
  const raw = [{ frameId: 0, parentFrameId: -1, documentId: "top", url: "https://fixture.test/" }];
  const trace = [];
  if (churn) raw.push({ frameId: 2, parentFrameId: 0, documentId: "child-old", url: "https://child.test/" });
  const documents = api.create({ frames, getFrames: async () => raw, sendDocument: async (_tab, id) =>
    churn && id === "top" ? { x: 10, y: 10, embed: { src: "https://child.test/", left: 0, top: 0 } } : {} });
  const sandbox = {
    shared: { browserAttention: () => "foreground", BROWSER_ATTENTION: { BACKGROUND: "background" } },
    GhostlightDocuments: api, frames, documents, cancelled: new Set(),
    async observeOperation(request, command) {
      trace.push({ kind: "settle", budget: command.timeout_ms });
      assert.equal(documents.context(7), request.documentScope);
      if (cancel) throw Object.assign(new Error("cancelled"), { code: "operation_cancelled", effectUnknown: false });
      if (!quiet) await new Promise(resolve => setTimeout(resolve, command.timeout_ms));
      if (churn) raw[1].documentId = "child-current";
      return { outcome: "observed", satisfied: quiet };
    },
    async activate() { trace.push({ kind: "click" }); return { outcome: "activated" }; },
    async fill() { trace.push({ kind: "fill" }); return { outcome: "filled" }; },
    async hoverLocator() { trace.push({ kind: "hover" }); return { outcome: "hovered" }; },
    async typeText() { trace.push({ kind: "type" }); return { outcome: "typed" }; },
    async pressKey() { trace.push({ kind: "key" }); return { outcome: "key_pressed" }; },
    async evaluateScript() { trace.push({ kind: "script" }); return { outcome: "evaluated" }; }
  };
  sandbox.globalThis = sandbox;
  vm.createContext(sandbox);
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  vm.runInContext(source.match(/async function prepareDocumentDiscovery\([^]*?\n}/)[0], sandbox);
  vm.runInContext(source.match(/async function dispatch\([^]*?\n}/)[0], sandbox);
  const run = async (primitive, budget = 30) => {
    const discovery = await sandbox.dispatch({ correlation: 'prepare', attention: 'foreground', command: {
      command: 'describe_documents', tab_id: 7, locators: [], points: [], focused: false, settle_ms: budget } });
    return sandbox.dispatch({ correlation: 'one', attention: 'foreground', command: {
      command: 'in_documents', scope: { documents: discovery.inventory.documents, allowed: ['top'],
        subjects: ['top'], watch_changes: false, strict_tree: false, settle_ms: 0 },
      primitive: { command: primitive, tab_id: 7 } } });
  };
  return { run, trace, documents, dispatch: sandbox.dispatch };
}

test("busy pages reach each input family once after the bounded settlement budget", async () => {
  for (const command of ["activate", "fill", "hover", "type_text", "press_key", "evaluate_script"]) {
    const { run, trace, documents } = fixture();
    await run(command);
    assert.equal(trace.length, 2, command);
    assert.equal(trace[0].kind, "settle");
    assert.ok(trace[0].budget <= 30);
    assert.equal(documents.context(7), undefined);
  }
});

test("quiet pages take the early completion path and opt-out allocates no observer", async () => {
  const quiet = fixture({ quiet: true });
  await quiet.run("activate", 1000);
  assert.deepEqual(quiet.trace.map(item => item.kind), ["settle", "click"]);
  const immediate = fixture();
  await immediate.run("activate", 0);
  assert.deepEqual(immediate.trace.map(item => item.kind), ["click"]);
});

test("cancellation during optional settlement never enters the action handler", async () => {
  const { run, trace, documents } = fixture({ cancel: true });
  await assert.rejects(run("activate"), { code: "operation_cancelled", effectUnknown: false });
  assert.deepEqual(trace.map(item => item.kind), ["settle"]);
  assert.equal(documents.context(7), undefined);
});

test("coordinate subjects bind to the current child after bounded preparation", async () => {
  const { dispatch } = fixture({ churn: true });
  const result = await dispatch({ correlation: "coordinate", attention: "foreground", command: {
    command: "describe_documents", tab_id: 7, locators: [], points: [{ x: 10, y: 10 }], focused: false, settle_ms: 25 } });
  assert.deepEqual(result.inventory.subjects, ["child-current"]);
  assert.equal(result.inventory.unresolved, false);
});
