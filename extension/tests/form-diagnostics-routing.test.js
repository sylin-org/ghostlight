"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const { readFileSync } = require("node:fs"), { join } = require("node:path"), vm = require("node:vm");
const api = require("../lib/form-diagnostics.js");
const EXTENSION_ID = "fixture-extension", DOCUMENT_ID = "01234567-89ab-cdef-0123-456789abcdef";
const OPTIONS_URL = `chrome-extension://${EXTENSION_ID}/options.html`;
const sender = (extra = {}) => ({ id: EXTENSION_ID, tab: { id: 7 }, frameId: 0, documentId: DOCUMENT_ID, ...extra });
const row = { event: "checkpoint", trigger: "focusin", control_count: 3, nonempty_count: 0 };
function section(source, first, next) {
  const start = source.indexOf(first), end = source.indexOf(next, start);
  assert.ok(start >= 0 && end > start, first); return source.slice(start, end);
}
function surface() {
  const listeners = new Map();
  return {
    addEventListener(name, callback) { if (!listeners.has(name)) listeners.set(name, new Set()); listeners.get(name).add(callback); },
    removeEventListener(name, callback) { listeners.get(name)?.delete(callback); },
    dispatchEvent(event) { event.target = this; for (const callback of [...listeners.get(event.type) ?? []]) callback(event); return true; },
    listenerCount(name) { return listeners.get(name)?.size ?? 0; }
  };
}
// Execute worker and relay in separate worlds sharing a document. Only the isolated
// world has extension messaging identity. Passive rows never use a native service.
async function fixture({ enabled = true, control = "active", owned = [7], failWrites = false,
  sessionSaved = {}, existingPages = new Map(), failSessionWrites = false } = {}) {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  const content = readFileSync(join(__dirname, "../../crates/orchestrator/src/page_runtime/content.js"), "utf8");
  const saved = { debug: enabled }, injections = [], work = [], owners = new Set(owned), pages = existingPages;
  let listener;
  const storage = { get: async () => structuredClone(saved), set: async value => {
    if (failWrites) throw new Error("storage failure"); Object.assign(saved, structuredClone(value));
  } };
  const formLog = api.createLog({ storage, debugKey: "debug", now: () => 1234 }); await formLog.snapshot();
  const forbidden = name => () => { work.push(name); throw new Error(`unexpected ${name}`); };
  function message(payload, identity = sender()) { return new Promise(resolve => assert.equal(listener(payload, identity, resolve), true)); }
  function page(tabId, isTop = true) {
    const window = { ...surface(), getComputedStyle: () => ({ display: "block", visibility: "visible", opacity: "1" }) };
    window.self = window; window.top = isTop ? window : {};
    const document = { ...surface(), visibilityState: "visible", hasFocus: () => false };
    const controls = [{ tagName: "INPUT", type: "text", value: "PRIVATE_VALUE", isConnected: true }], timers = new Map(); let timerId = 0;
    const main = { window, document, IS_TOP: isTop, GhostlightFormDiagnostics: api, queryAll: () => controls, credentialClass: () => false,
      CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
      setInterval: callback => { const id = ++timerId; timers.set(id, callback); return id; }, clearInterval: id => timers.delete(id),
      setTimeout: () => ++timerId, clearTimeout() {}, chrome: { runtime: { sendMessage: forbidden("page messaging") } } };
    vm.createContext(main);
    vm.runInContext(readFileSync(join(__dirname, "../lib/form-diagnostics.js"), "utf8"), main);
    vm.runInContext(section(content, "  const formDiagnosticsApi =", "  function finishDragObservation(")
      + section(content, "  window.__ghostlight_dispatch__ = function(message) {", "    // Resolve and scroll inside") + "return false;\n});\n};", main);
    const isolated = { window, chrome: { runtime: { sendMessage: payload => message(payload, sender({ tab: { id: tabId } })) } } };
    vm.createContext(isolated);
    return { main, isolated, window, document, controls, timers, id: DOCUMENT_ID };
  }
  const sandbox = { formDiagnosticsApi: api, formLog, preferences: { diagnostics: enabled }, liveState: { control_state: control },
    topology: { workspaceFor: tab => owners.has(tab) ? "workspace" : null }, frames: { TOP_FRAME_ID: 0 }, shared: { bounded: value => String(value) },
    stateApi: { preferences: value => ({ ...value }), preferencesForStorage: value => ({ debug: value.diagnostics }) },
    connectionEvents: { PREFERENCES_CHANGED: "preferences_changed" }, connectionLog: { snapshot: async () => ({ entries: [] }), setEnabled: async () => {}, record() {} },
    send: forbidden("native send"), requestRuntimeControl: forbidden("runtime control"), requestDiagnosticsToggle: forbidden("process diagnostics"),
    chrome: { runtime: { id: EXTENSION_ID, getURL: path => `chrome-extension://${EXTENSION_ID}/${path}`, onMessage: { addListener: callback => { listener = callback; } } },
      storage: { local: storage, session: {
        get: async () => structuredClone(sessionSaved),
        set: async value => { if (failSessionWrites) throw new Error("session storage failure"); Object.assign(sessionSaved, structuredClone(value)); }
      } }, tabs: { query: async () => [{ id: 7 }, { id: 11 }, { id: 19 }, {}], sendMessage: forbidden("tabs messaging") },
      scripting: { executeScript: async options => {
        injections.push(options); const tabId = options.target.tabId;
        if (!pages.has(tabId)) pages.set(tabId, page(tabId)); const current = pages.get(tabId);
        if (options.target.documentIds && !options.target.documentIds.includes(current.id)) throw new Error("document unloaded");
        const context = options.world === "ISOLATED" ? current.isolated : current.main; let result;
        if (options.files) for (const path of options.files) vm.runInContext(readFileSync(join(__dirname, "..", path), "utf8"), context);
        else { context.injectionArgs = options.args ?? []; result = await vm.runInContext(`(${options.func.toString()})(...injectionArgs)`, context); }
        return [{ frameId: 0, documentId: current.id, result }];
      } } }
  };
  vm.createContext(sandbox);
  vm.runInContext(section(source, "async function injectedContentPrimitive(", "\nasync function contentIn("), sandbox);
  vm.runInContext(section(source, "const formDiagnosticDocuments =", "\nconnectNative();"), sandbox);
  for (const [tabId, current] of pages) current.isolated.chrome.runtime.sendMessage = payload => message(payload, sender({ tab: { id: tabId } }));
  return { sandbox, saved, sessionSaved, injections, work, owners, pages, formLog, message, page };
}
test("passive rows use Chromium identity, project private fields, and create no native work", async () => {
  const state = await fixture();
  assert.equal((await state.message({ kind: api.MESSAGE_KIND, row: { ...row, tab_id: 99, document_id: "page-supplied", value: "PRIVATE_VALUE" } })).ok, true);
  const entries = (await state.formLog.snapshot()).entries;
  assert.equal(entries.length, 1); assert.equal(entries[0].tab_id, 7); assert.equal(entries[0].document_id, DOCUMENT_ID);
  assert.equal(entries[0].time_ms, 1234); assert.equal(JSON.stringify(entries).includes("PRIVATE"), false);
  assert.deepEqual(state.work, []); assert.deepEqual(state.injections, []);
});
test("persistence requires an enabled owned Chromium top-document sender", async () => {
  for (const [options, identity] of [[{ enabled: false }, sender()], [{ control: "ended" }, sender()], [{ owned: [] }, sender()],
    [{}, sender({ frameId: 1 })], [{}, sender({ id: "other" })], [{}, sender({ tab: undefined })],
    [{}, sender({ documentId: undefined })], [{}, sender({ documentId: "" })], [{}, sender({ documentId: "page-supplied" })], [{}, sender({ tab: { id: -1 } })]]) {
    const state = await fixture(options);
    assert.equal((await state.message({ kind: api.STATE_MESSAGE_KIND }, identity)).value.enabled, false);
    assert.equal((await state.message({ kind: api.MESSAGE_KIND, row }, identity)).ok, true);
    assert.equal((await state.formLog.snapshot()).entries.length, 0); assert.deepEqual(state.work, []);
  }
});
test("owned opt-in preserves start, sample, stop, and release without adopting tabs", async () => {
  const state = await fixture({ owned: [7, 11] }); await state.sandbox.refreshFormDiagnostics();
  assert.equal(state.pages.has(19), false);
  assert.equal((await state.formLog.snapshot()).entries.filter(item => item.event === "trace_started").length, 2);
  const page = state.pages.get(7); await state.sandbox.syncFormDiagnostics(7);
  assert.equal(page.window.listenerCount(api.ROW_EVENT_KIND), 1, "relay is idempotent");
  page.controls[0].value = ""; for (const tick of page.timers.values()) tick(); await page.isolated.GhostlightFormDiagnosticsRelay.flush();
  assert.equal((await state.formLog.snapshot()).entries.at(-1).became_empty, 1);
  vm.runInContext('formDiagnostics.run("fill", () => {})', page.main);
  await page.isolated.GhostlightFormDiagnosticsRelay.flush();
  assert.deepEqual((await state.formLog.snapshot()).entries.slice(-2).map(item => item.event),
    ["operation_started", "operation_finished"]);
  state.owners.delete(7); await state.sandbox.syncFormDiagnostics(7);
  assert.equal((await state.formLog.snapshot()).entries.at(-1).event, "trace_stopped"); assert.equal(page.timers.size, 0);
  const count = (await state.formLog.snapshot()).entries.length;
  page.window.dispatchEvent({ type: api.ROW_EVENT_KIND, detail: JSON.stringify(row) }); await page.isolated.GhostlightFormDiagnosticsRelay.flush();
  assert.equal((await state.formLog.snapshot()).entries.length, count); assert.deepEqual([...state.owners], [11]); assert.deepEqual(state.work, []);
});
test("release stop acceptance is bound to the exact installed Chromium document", async () => {
  const state = await fixture();
  await state.sandbox.syncFormDiagnostics(7);
  state.owners.delete(7);
  const before = (await state.formLog.snapshot()).entries.length;
  await state.message({ kind: api.MESSAGE_KIND, row: { event: "trace_stopped", trigger: "disabled" } },
    sender({ documentId: "ffffffff-ffff-ffff-ffff-ffffffffffff" }));
  assert.equal((await state.formLog.snapshot()).entries.length, before);
  await state.sandbox.syncFormDiagnostics(7);
  assert.equal((await state.formLog.snapshot()).entries.at(-1).document_id, DOCUMENT_ID);
});
test("disabling keeps the final stop record and stops page sampling", async () => {
  const state = await fixture(); await state.sandbox.syncFormDiagnostics(7);
  assert.equal((await state.message({ kind: "set_preferences", preferences: { diagnostics: false } }, { id: EXTENSION_ID, url: OPTIONS_URL })).ok, true);
  const snapshot = await state.formLog.snapshot(); assert.equal(snapshot.enabled, false); assert.equal(snapshot.entries.at(-1).event, "trace_stopped");
  assert.equal(state.pages.get(7).timers.size, 0); assert.equal(state.pages.has(19), false); assert.deepEqual(state.work, []);
});
test("worker restart restores exact document custody for off and release", async () => {
  for (const release of [false, true]) {
    const first = await fixture();
    await first.sandbox.syncFormDiagnostics(7);
    assert.equal(first.pages.get(7).timers.size, 1);
    const restarted = await fixture({ sessionSaved: first.sessionSaved, existingPages: first.pages,
      owned: release ? [] : [7] });
    if (release) await restarted.sandbox.syncFormDiagnostics(7);
    else await restarted.message({ kind: "set_preferences", preferences: { diagnostics: false } },
      { id: EXTENSION_ID, url: OPTIONS_URL });
    assert.equal(restarted.pages.get(7).timers.size, 0);
    assert.equal((await restarted.formLog.snapshot()).entries.at(-1).event, "trace_stopped");
    assert.equal((await restarted.formLog.snapshot()).entries.at(-1).document_id, DOCUMENT_ID);
    assert.deepEqual(restarted.sessionSaved[api.DOCUMENTS_KEY], []);
    assert.equal(restarted.pages.has(19), false);
    assert.deepEqual(restarted.work, []);
  }
});
test("failed document-custody persistence cannot enable an observer", async () => {
  const state = await fixture({ failSessionWrites: true });
  await state.sandbox.syncFormDiagnostics(7);
  assert.equal(state.pages.get(7).timers.size, 0);
  assert.equal((await state.formLog.snapshot()).entries.length, 0);
  assert.equal(state.sessionSaved[api.DOCUMENTS_KEY], undefined);
  assert.equal(state.injections.filter(item => item.world === "MAIN").length, 0);
  assert.deepEqual(state.work, []);
});
test("unrelated tabs cannot consume synchronization capacity before an owned observer", async () => {
  const state = await fixture();
  const unrelated = Array.from({ length: api.DOCUMENT_LIMIT + 20 }, (_, index) => ({ id: index + 20 }));
  state.sandbox.chrome.tabs.query = async () => [...unrelated, { id: 7 }];
  await state.sandbox.refreshFormDiagnostics();
  assert.equal(state.pages.size, 1);
  assert.equal(state.pages.get(7).timers.size, 1);
  assert.equal((await state.formLog.snapshot()).entries.at(-1).event, "trace_started");
  state.sandbox.preferences.diagnostics = false;
  await state.sandbox.refreshFormDiagnostics();
  assert.equal(state.pages.get(7).timers.size, 0);
  assert.deepEqual(state.sessionSaved[api.DOCUMENTS_KEY], []);
  assert.equal((await state.formLog.snapshot()).entries.at(-1).event, "trace_stopped");
  assert.equal(state.injections.every(options => options.target.tabId === 7), true);
  assert.deepEqual(state.work, []);
});
test("failed or invalid stop receipts preserve retry custody across worker restart", async () => {
  for (const failure of ["rejected", "error", "missing", "wrong-state", "wrong-document"]) {
    const state = await fixture();
    await state.sandbox.syncFormDiagnostics(7);
    const execute = state.sandbox.chrome.scripting.executeScript;
    state.sandbox.preferences.diagnostics = false;
    state.sandbox.chrome.scripting.executeScript = async options => {
      if (options.world !== "MAIN") return execute(options);
      if (failure === "rejected") throw new Error("exact document remains live but injection failed");
      if (failure === "missing") return [];
      return [{ frameId: 0, documentId: failure === "wrong-document" ? "ffffffff-ffff-ffff-ffff-ffffffffffff" : DOCUMENT_ID,
        result: failure === "error" ? { error: "stop failed" } : { enabled: failure === "wrong-state" } }];
    };
    await state.sandbox.syncFormDiagnostics(7);
    assert.equal(state.pages.get(7).timers.size, 1, failure);
    assert.deepEqual(state.sessionSaved[api.DOCUMENTS_KEY], [{ tab_id: 7, document_id: DOCUMENT_ID }], failure);
    state.sandbox.chrome.scripting.executeScript = execute;
    const restarted = await fixture({ sessionSaved: state.sessionSaved, existingPages: state.pages });
    restarted.sandbox.preferences.diagnostics = false;
    await restarted.sandbox.syncFormDiagnostics(7);
    assert.equal(restarted.pages.get(7).timers.size, 0, failure);
    assert.deepEqual(restarted.sessionSaved[api.DOCUMENTS_KEY], [], failure);
    assert.equal((await restarted.formLog.snapshot()).entries.at(-1).event, "trace_stopped", failure);
  }
});
test("an older disable cannot clear a newer enable's document custody", async () => {
  const state = await fixture();
  await state.sandbox.syncFormDiagnostics(7);
  const execute = state.sandbox.chrome.scripting.executeScript;
  let entered, release;
  const paused = new Promise(resolve => { entered = resolve; });
  const released = new Promise(resolve => { release = resolve; });
  let gated = false;
  state.sandbox.chrome.scripting.executeScript = async options => {
    if (!gated && options.world === "ISOLATED" && options.func?.name === "flushFormDiagnosticsRelay") {
      gated = true;
      entered();
      await released;
    }
    return execute(options);
  };
  state.sandbox.preferences.diagnostics = false;
  const olderOff = state.sandbox.syncFormDiagnostics(7);
  await paused;
  state.sandbox.preferences.diagnostics = true;
  const newerOn = state.sandbox.syncFormDiagnostics(7);
  await new Promise(setImmediate);
  const newerWaited = state.pages.get(7).timers.size === 0;
  release();
  await Promise.all([olderOff, newerOn]);
  assert.equal(newerWaited, true, "same-tab state effects serialize through the stop acknowledgement and flush");
  assert.equal(state.pages.get(7).timers.size, 1);
  assert.deepEqual(state.sessionSaved[api.DOCUMENTS_KEY], [{ tab_id: 7, document_id: DOCUMENT_ID }]);
  const restarted = await fixture({ sessionSaved: state.sessionSaved, existingPages: state.pages });
  restarted.sandbox.preferences.diagnostics = false;
  await restarted.sandbox.syncFormDiagnostics(7);
  assert.equal(restarted.pages.get(7).timers.size, 0);
  assert.deepEqual(restarted.sessionSaved[api.DOCUMENTS_KEY], []);
  assert.equal((await restarted.formLog.snapshot()).entries.at(-1).event, "trace_stopped");
});
test("relay projects rows, isolates malformed events and synchronous failures, and bounds pending delivery", async () => {
  const state = await fixture(); await state.sandbox.syncFormDiagnostics(7); const page = state.pages.get(7);
  for (const detail of ["invalid JSON", {}, "x".repeat(api.ROW_MAX_CHARS + 1), JSON.stringify({ event: "unknown" })]) page.window.dispatchEvent({ type: api.ROW_EVENT_KIND, detail });
  const beforeForeign = (await state.formLog.snapshot()).entries.length;
  const foreign = { target: {}, type: api.ROW_EVENT_KIND, detail: JSON.stringify(row) };
  page.window.dispatchEvent.call({ target: "foreign" }, foreign);
  await page.isolated.GhostlightFormDiagnosticsRelay.flush();
  assert.equal((await state.formLog.snapshot()).entries.length, beforeForeign, "foreign event targets are rejected");
  page.window.dispatchEvent({ type: api.ROW_EVENT_KIND, detail: JSON.stringify({ ...row, value: "PRIVATE_VALUE", tab_id: 99 }) });
  await page.isolated.GhostlightFormDiagnosticsRelay.flush(); const delivered = (await state.formLog.snapshot()).entries.at(-1);
  assert.equal(delivered.document_id, DOCUMENT_ID); assert.equal(delivered.tab_id, 7); assert.equal(JSON.stringify(delivered).includes("PRIVATE"), false);
  let calls = 0; page.isolated.chrome.runtime.sendMessage = () => { calls++; throw new Error("extension unavailable"); };
  for (let index = 0; index < api.LIMIT + 5; index++) page.window.dispatchEvent({ type: api.ROW_EVENT_KIND, detail: JSON.stringify(row) });
  await page.isolated.GhostlightFormDiagnosticsRelay.flush(); assert.equal(calls, api.LIMIT); assert.deepEqual(state.work, []);
});
test("export requires extension options and storage or unloaded-document failures stay isolated", async () => {
  const state = await fixture({ failWrites: true }); assert.equal((await state.message({ kind: api.MESSAGE_KIND, row })).ok, true);
  assert.equal((await state.formLog.snapshot()).write_failures, 1);
  for (const identity of [sender(), sender({ url: "https://example.test/options.html" }), sender({ url: OPTIONS_URL, id: "other" })]) assert.equal((await state.message({ kind: "connection_diagnostics" }, identity)).ok, false);
  assert.equal((await state.message({ kind: "connection_diagnostics" }, { id: EXTENSION_ID, url: OPTIONS_URL })).ok, true);
  state.sandbox.chrome.scripting.executeScript = async () => { throw new Error("document unloaded"); };
  await state.sandbox.syncFormDiagnostics(7); await state.sandbox.refreshFormDiagnostics(); assert.deepEqual(state.work, []);
});
test("embedded runtimes stay disabled despite pushed state", async () => {
  const state = await fixture(), page = state.page(7, false);
  assert.equal((await page.window.__ghostlight_dispatch__({ kind: api.STATE_MESSAGE_KIND, enabled: true })).enabled, false);
  assert.equal(page.timers.size, 0);
});
