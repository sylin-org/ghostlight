"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");
const formDiagnosticsApi = require("../lib/form-diagnostics.js");

const EXTENSION_ID = "fixture-extension";
const DOCUMENT_ID = "01234567-89ab-cdef-0123-456789abcdef";
const OPTIONS_URL = `chrome-extension://${EXTENSION_ID}/options.html`;
const sender = (overrides = {}) => ({ id: EXTENSION_ID, tab: { id: 7 }, frameId: 0,
  documentId: DOCUMENT_ID, ...overrides });
const row = { event: "checkpoint", trigger: "focusin", control_count: 3, nonempty_count: 0 };

function section(source, first, next) {
  const start = source.indexOf(first), end = source.indexOf(next, start);
  assert.ok(start >= 0 && end > start, `source section ${first} exists`);
  return source.slice(start, end);
}

// Run the shipped worker's routing and its real bounded log. No native connection is
// present: passive observations must remain local even during unrelated browser work.
async function fixture({ enabled = true, control = "active", owned = [7], failWrites = false } = {}) {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  const saved = { debug: enabled }, messages = [], work = [], owners = new Set(owned);
  let listener;
  const storage = {
    get: async () => structuredClone(saved),
    set: async value => {
      if (failWrites) throw new Error("fixture storage failure");
      Object.assign(saved, structuredClone(value));
    }
  };
  const formLog = formDiagnosticsApi.createLog({ storage, debugKey: "debug", now: () => 1234 });
  await formLog.snapshot();
  const forbidden = name => () => { work.push(name); throw new Error(`unexpected ${name}`); };
  const sandbox = {
    formDiagnosticsApi, formLog,
    preferences: { diagnostics: enabled }, liveState: { control_state: control },
    topology: { workspaceFor: tab => owners.has(tab) ? "workspace" : null },
    frames: { TOP_FRAME_ID: 0 },
    shared: { bounded: value => String(value) },
    stateApi: { preferences: value => ({ ...value }), preferencesForStorage: value => ({ debug: value.diagnostics }) },
    connectionEvents: { PREFERENCES_CHANGED: "preferences_changed" },
    connectionLog: { snapshot: async () => ({ schema: "connection-fixture", entries: [] }),
      setEnabled: async () => {}, record: () => {} },
    send: forbidden("native send"), requestRuntimeControl: forbidden("runtime control"),
    requestDiagnosticsToggle: forbidden("process diagnostics toggle"),
    chrome: {
      runtime: { id: EXTENSION_ID, getURL: path => `chrome-extension://${EXTENSION_ID}/${path}`,
        onMessage: { addListener: callback => { listener = callback; } } },
      storage: { local: storage },
      tabs: { query: async () => [{ id: 7 }, { id: 11 }, { id: 19 }, {}],
        sendMessage: async (tab, message, options) => { messages.push({ tab, message, options }); } }
    }
  };
  vm.createContext(sandbox);
  vm.runInContext(section(source, "function formDiagnosticsEnabled(", "\nconnectNative();"), sandbox);
  async function message(payload, sourceSender = sender()) {
    return new Promise(resolve => assert.equal(listener(payload, sourceSender, resolve), true));
  }
  return { sandbox, saved, messages, work, owners, formLog, message };
}

test("passive form rows use Chromium tab and document identity and never create native work", async () => {
  const state = await fixture();
  const reply = await state.message({ kind: formDiagnosticsApi.MESSAGE_KIND,
    row: { ...row, tab_id: 99, document_id: "page-supplied-identity", value: "PRIVATE_VALUE", name: "PRIVATE_NAME" } });
  assert.equal(reply.ok, true);
  assert.equal(reply.value, null);
  const snapshot = await state.formLog.snapshot();
  assert.equal(snapshot.entries.length, 1);
  assert.equal(snapshot.entries[0].tab_id, 7);
  assert.equal(snapshot.entries[0].document_id, DOCUMENT_ID);
  assert.equal(snapshot.entries[0].time_ms, 1234);
  assert.equal(JSON.stringify(snapshot).includes("PRIVATE_"), false);
  assert.equal(JSON.stringify(snapshot).includes("page-supplied"), false);
  assert.deepEqual(state.work, []);
  assert.deepEqual(state.messages, []);
});

test("form observation state and persistence require the enabled owned top document", async () => {
  const cases = [
    ["off", { enabled: false }, sender()],
    ["ended", { control: "ended" }, sender()],
    ["unowned", { owned: [] }, sender()],
    ["subframe", {}, sender({ frameId: 1 })],
    ["external sender", {}, sender({ id: "other-extension" })],
    ["missing tab", {}, sender({ tab: undefined })],
    ["missing document identity", {}, sender({ documentId: undefined })],
    ["empty document identity", {}, sender({ documentId: "" })],
    ["malformed document identity", {}, sender({ documentId: "page-supplied-identity" })],
    ["invalid tab identity", {}, sender({ tab: { id: -1 } })]
  ];
  for (const [label, options, sourceSender] of cases) {
    const state = await fixture(options);
    const response = await state.message({ kind: formDiagnosticsApi.STATE_MESSAGE_KIND }, sourceSender);
    assert.equal(response.ok, true, label);
    assert.equal(response.value.enabled, false, label);
    assert.equal((await state.message({ kind: formDiagnosticsApi.MESSAGE_KIND, row }, sourceSender)).ok, true, label);
    assert.equal((await state.formLog.snapshot()).entries.length, 0, label);
    assert.equal(state.saved[formDiagnosticsApi.KEY], undefined, label);
    assert.deepEqual(state.work, [], label);
  }
  const permitted = await fixture();
  assert.equal((await permitted.message({ kind: formDiagnosticsApi.STATE_MESSAGE_KIND })).value.enabled, true);
});

test("form diagnostic export is restricted to this extension's options page", async () => {
  const state = await fixture();
  await state.message({ kind: formDiagnosticsApi.MESSAGE_KIND, row });
  for (const sourceSender of [sender(), sender({ url: "https://example.test/options.html" }),
    sender({ url: OPTIONS_URL, id: "other-extension" })]) {
    const response = await state.message({ kind: "connection_diagnostics" }, sourceSender);
    assert.equal(response.ok, false);
    assert.equal(response.value, undefined);
  }
  const response = await state.message({ kind: "connection_diagnostics" }, { id: EXTENSION_ID, url: OPTIONS_URL });
  assert.equal(response.ok, true);
  assert.equal(response.value.form_diagnostics.entries.length, 1);
  assert.deepEqual(state.work, []);
});

test("preference changes enable owned tabs and stop released or unrelated tabs without adopting them", async () => {
  const state = await fixture({ enabled: false, owned: [7, 11] });
  state.owners.delete(11);
  const response = await state.message({ kind: "set_preferences", preferences: { diagnostics: true } },
    { id: EXTENSION_ID, url: OPTIONS_URL });
  assert.equal(response.ok, true);
  await new Promise(setImmediate);
  assert.deepEqual(state.messages.map(item => [item.tab, item.message.enabled]), [[7, true], [11, false], [19, false]]);
  for (const item of state.messages) {
    assert.equal(item.message.kind, formDiagnosticsApi.STATE_MESSAGE_KIND);
    assert.equal(item.options.frameId, 0);
  }
  state.messages.length = 0;
  state.owners.delete(7);
  await state.sandbox.syncFormDiagnostics(7);
  assert.equal(state.messages[0].message.enabled, false);
  assert.equal(state.owners.size, 0);
  assert.deepEqual(state.work, []);
});

test("diagnostic storage and missing receivers cannot fail passive routing", async () => {
  const state = await fixture({ failWrites: true });
  assert.equal((await state.message({ kind: formDiagnosticsApi.MESSAGE_KIND, row })).ok, true);
  assert.equal((await state.formLog.snapshot()).write_failures, 1);
  assert.equal((await state.message({ kind: formDiagnosticsApi.STATE_MESSAGE_KIND })).value.enabled, true);
  state.sandbox.chrome.tabs.sendMessage = async () => { throw new Error("document unloaded"); };
  await state.sandbox.syncFormDiagnostics(7);
  await state.sandbox.refreshFormDiagnostics();
  assert.deepEqual(state.work, []);
});

test("content state survives delayed startup responses and embedded documents stay disabled", async () => {
  const source = readFileSync(join(__dirname, "../content.js"), "utf8");
  for (const isTop of [true, false]) for (const enabled of [true, false]) {
    const states = [];
    let resolveStartup, listener;
    const sandbox = {
      document: {}, window: {}, IS_TOP: isTop,
      credentialClass: () => false, queryAll: () => [],
      GhostlightFormDiagnostics: { ...formDiagnosticsApi, createObserver: () => ({ setEnabled: value => states.push(value) }) },
      chrome: { runtime: { sendMessage: () => new Promise(resolve => { resolveStartup = resolve; }),
        onMessage: { addListener: callback => { listener = callback; } } } }
    };
    vm.createContext(sandbox);
    const startup = section(source, "  const formDiagnosticsApi =", "  function clearCaptureMask(");
    const receiver = section(source, "  chrome.runtime.onMessage.addListener(", "    // Resolve and scroll inside");
    vm.runInContext(startup + receiver + "return false;\n});", sandbox);
    assert.equal(Boolean(resolveStartup), isTop, "only the top document requests observation state");
    listener({ kind: formDiagnosticsApi.STATE_MESSAGE_KIND, enabled }, {}, () => {});
    resolveStartup?.({ ok: true, value: { enabled: !enabled } });
    await new Promise(setImmediate);
    assert.deepEqual(states, [isTop && enabled]);
  }
});
