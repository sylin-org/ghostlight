"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");
const documentsApi = require("../lib/documents.js");
const frames = require("../lib/frames.js");

function fixture({ scoped, missing = false, preflight = false }) {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  const effects = [], injections = [];
  const fields = { first: { value: "first draft", readOnly: false }, second: { value: "second draft", readOnly: preflight } };
  let focused = null;
  const dispatch = async message => {
    await Promise.resolve();
    if (message.kind === "prepare_fill") {
      if (Object.values(fields).some(field => field.readOnly)) throw new Error("target is read-only");
      return { field_kinds: message.fields.map(() => "browser_text") };
    }
    if (message.kind === "prepare_text_fill") {
      const name = message.field.locator.replace("locator_", "");
      if (fields[name].readOnly) throw new Error("target is read-only");
      focused = name;
      return { focused: true };
    }
    if (message.kind === "verify_fill_values") return { retained: true };
    return { focused: true };
  };
  const sandbox = {
    window: { __ghostlight_dispatch__: dispatch }, frames,
    navigationWatchers: new Map(), cancelled: new Set(), shared: require("../lib/shared.js"),
    ensureDebugger: async () => {}, detachDebugger: async () => {}, physicalTab: tab => tab,
    sendDebugger: async (_target, method, params) => {
      effects.push({ method, text: params.text });
      if (method === "Input.insertText") { fields[focused].value = params.text; fields.second.readOnly = true; }
    },
    FILL_RETAINED_STABLE_MS: 0, FILL_RETAINED_LIMIT_MS: 100, FILL_RETAINED_POLL_MS: 0, setTimeout,
    chrome: { tabs: { get: async id => ({ id, status: "complete" }) }, scripting: {
      async executeScript(options) {
        const message = options.args[0]; injections.push({ target: options.target, kind: message.kind });
        if (missing && message.kind === "prepare_text_fill" && message.field.locator === "locator_second") {
          // Chrome can resolve an injected async rejection with an InjectionResult lacking result.
          return [{ frameId: 0, documentId: "document" }];
        }
        try { return [{ frameId: 0, documentId: "document", result: await options.func(...options.args) }]; }
        catch (_) { return [{ frameId: 0, documentId: "document" }]; }
      }
    } }
  };
  vm.createContext(sandbox);
  for (const name of ["injectedContentPrimitive", "contentInjectionResult", "sendDocumentPrimitive", "contentIn"]) {
    const body = source.match(new RegExp(`(?:async )?function ${name}\\([^]*?\\n}`));
    assert.ok(body, name); vm.runInContext(body[0], sandbox);
  }
  const raw = [{ frameId: 0, parentFrameId: -1, documentId: "document", url: "https://fixture.test/" }];
  sandbox.documents = documentsApi.create({ frames, getFrames: async () => raw,
    sendDocument: (tab, document, message) => sandbox.sendDocumentPrimitive(tab, document, message) });
  const fillStart = source.indexOf("function requireFillBudget(");
  vm.runInContext(source.slice(fillStart, source.indexOf("async function typeText(", fillStart)), sandbox);
  const call = () => sandbox.fill("fill", { tab_id: 7, timeout_ms: 1_000,
    fields: [{ locator: frames.scopedLocator(0, "locator_first", "document"), value: "PARTIAL_DRAFT_EFFECT" }, { locator: frames.scopedLocator(0, "locator_second", "document"), value: "MUST_NOT_CHANGE_LATER_FIELD" }] }, { background: true });
  const run = () => scoped ? sandbox.documents.run(7,
    { documents: documentsApi.inventory(raw), allowed: ["document"], watch_changes: true }, call) : call();
  return { sandbox, fields, effects, injections, run };
}

for (const scoped of [false, true]) {
  for (const missing of [false, true]) {
    test(`${scoped ? "document-scoped" : "frame fallback"} injection ${missing ? "missing response" : "async rejection"} stops the second input with known partial uncertainty`, async () => {
      const fixtureState = fixture({ scoped, missing });
      await assert.rejects(fixtureState.run(), error => error.effectUnknown === true
        && (missing ? /no result/ : /read-only/).test(error.message));
      assert.equal(fixtureState.fields.first.value, "PARTIAL_DRAFT_EFFECT");
      assert.equal(fixtureState.fields.second.value, "second draft");
      assert.deepEqual(fixtureState.effects, [{ method: "Input.insertText", text: "PARTIAL_DRAFT_EFFECT" }]);
      assert.equal(fixtureState.injections.filter(item => item.kind === "prepare_text_fill").length, 2);
      assert.ok(fixtureState.injections.every(item => scoped ? item.target.documentIds?.[0] === "document" : item.target.frameIds?.[0] === 0));
    });
  }
  test(`${scoped ? "document-scoped" : "frame fallback"} rejected preparation remains no effect`, async () => {
    const fixtureState = fixture({ scoped, preflight: true });
    await assert.rejects(fixtureState.run(), error => error.effectUnknown === false && /read-only/.test(error.message));
    assert.deepEqual(fixtureState.effects, []);
    assert.equal(fixtureState.fields.first.value, "first draft");
    assert.equal(fixtureState.fields.second.value, "second draft");
  });
}

test("the injection boundary rejects empty, missing and error responses while optional presentation stays passive", async () => {
  const { sandbox } = fixture({ scoped: false });
  for (const results of [[], [{}], [{ result: undefined }], [{ result: null }], [{ result: { error: "read-only" } }], [{ result: { error: "" } }]]) {
    assert.throws(() => sandbox.contentInjectionResult(results));
  }
  assert.equal(sandbox.contentInjectionResult([{ result: { focused: true } }]).focused, true);
  sandbox.chrome.scripting.executeScript = async () => [{}];
  assert.equal((await sandbox.contentIn(7, 0, { kind: "present" }, true)).presented, false);
});
