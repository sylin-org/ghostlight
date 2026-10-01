"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const debuggerApi = require("../lib/debugger.js");
const documentApi = require("../lib/documents.js");
const engineApi = require("../lib/engine.js");
const evaluatorApi = require("../lib/script-evaluator.js");
const frames = require("../lib/frames.js");
const shared = require("../lib/shared.js");
const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");

function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const turn = () => new Promise(resolve => setImmediate(resolve));
async function until(condition) {
  for (let remaining = 100; remaining > 0; remaining--) { if (condition()) return; await turn(); }
  assert.fail("fixture did not reach its expected physical boundary");
}

async function fixture({ full = false, failDetach = false, ignoreDetach = false, setup = null } = {}) {
  const physical = [], replies = [], pending = new Map(), owners = new Map();
  const captures = [], navigations = new Map();
  const pages = new Map([7, 8].map(id => [id, vm.createContext({ effects: 0, draft: `retained-${id}` })]));
  const gates = new Map();
  for (const [id, page] of pages) { const gate = deferred(); gates.set(id, gate); page.gate = gate.promise; }
  const attachment = new Map();
  const options = { failDetach, ignoreDetach, capture: null, failPresentation: null };
  let lifecycle;
  const chrome = { debugger: {
    async attach({ tabId }) { attachment.set(tabId, true); physical.push(["attach", tabId]); if (setup) await setup.promise; },
    async detach({ tabId }) {
      physical.push(["detach", tabId]);
      if (options.failDetach) throw new Error("synthetic detach failure");
      attachment.delete(tabId);
      lifecycle.detached(tabId);
      if (!options.ignoreDetach) pending.get(tabId)?.reject(new Error("exact CDP session released"));
    },
    async sendCommand({ tabId }, method, params) {
      physical.push([method, tabId]);
      if (method === "Page.getLayoutMetrics") return { cssVisualViewport: { pageX: 0, pageY: 0, clientWidth: 100, clientHeight: 100 } };
      if (method === "Page.captureScreenshot") return options.capture ? await options.capture.promise : { data: "synthetic_frame" };
      if (method !== "Runtime.evaluate") return {};
      const response = deferred(); pending.set(tabId, response);
      const expression = params.expression.startsWith("await ") ? params.expression.slice(6) : params.expression;
      Promise.resolve(vm.runInContext(expression, pages.get(tabId))).then(
        value => response.resolve({ result: { value } }),
        error => response.resolve({ exceptionDetails: { text: error.message } }));
      try { return await response.promise; }
      finally { if (pending.get(tabId) === response) pending.delete(tabId); }
    }
  }, tabs: {
    async get(id) { return { id, url: "https://fixture.invalid/", title: "fixture", status: "complete" }; },
    async update(id) { physical.push(["navigate", id]); return this.get(id); }
  } };
  lifecycle = debuggerApi.create(chrome.debugger);
  const raw = id => [{ frameId: 0, parentFrameId: -1, documentId: `document-${id}`, url: "https://fixture.invalid/" }];
  const documents = documentApi.create({ frames, getFrames: async id => raw(id),
    sendDocument: async (id) => { physical.push(["read", id]); return { text: pages.get(id).draft, truncated: false }; } });
  const engine = engineApi.create({ maximumRecords: full ? 1 : 256, load: async () => null, save: async () => {} });
  await engine.activate("service_one");
  const sandbox = { chrome, documents, shared, scriptEvaluator: evaluatorApi, operationEngine: engine,
    debuggerLifecycle: lifecycle, cancelled: new Set(), scriptEvaluations: new Map(), navigationWatchers: new Map(),
    beforeUnloadAcceptors: new Map(),
    browserServiceEpoch: "service_one", CANCEL_MARKER_LIMIT: 256, SCRIPT_SETTLE_MS: 1, SCRIPT_CLEANUP_TIMEOUT_MS: 40,
    INPUT_DISPATCH_METHODS: new Set(), NATIVE_SURFACE_COMMANDS: new Set(), BACKGROUND_EDIT_COMMANDS: new Set(),
    COMMAND_HANDLERS: { in_documents: {}, read_text: {}, evaluate_script: {}, cancel: {}, describe_documents: {} },
    topology: { workspaceFor: id => owners.get(id), async remember(id, workspace) { owners.set(id, workspace); } },
    browserNegotiation: Promise.resolve(), nativePort: {}, setTimeout, clearTimeout,
    send: frame => replies.push(frame), physicalTab: tab => ({ id: tab.id, url: tab.url, title: tab.title, status: tab.status }),
    retainManagedDebugger: id => lifecycle.retain(id),
    recordingScopeCurrent: async () => true,
    recording: { append: (...frame) => { captures.push(frame); return true; } },
    imageDimensions: async () => ({ width: 100, height: 100 }),
    contentAll: async (_id, message) => {
      if (options.failPresentation === (message.hidden ? "hide" : "show")) throw new Error("synthetic presentation failure");
    },
    waitForReady: (id, correlation) => {
      const ready = deferred(); navigations.set(correlation, ready); return ready.promise;
    },
    content: (id, message) => documents.route(id, 0, message, () => assert.fail("read escaped document scope")) };
  sandbox.globalThis = sandbox;
  sandbox.GhostlightDocuments = documentApi;
  sandbox.GhostlightRecording = { MAX_WIDTH: 500, MAX_HEIGHT: 500, JPEG_QUALITY: 80 };
  vm.createContext(sandbox);
  for (const name of ["sendDebugger", "scriptCancellation", "assertScriptEvaluation", "teardownScriptEvaluation",
    "cancelBrowserOperation", "evaluateScript", "ensureDebugger", "detachDebugger",
    "captureRecordingFrame", "navigateDiscardingBeforeUnload", "dispatch", "onNativeMessage"]) {
    const body = source.match(new RegExp(`(?:async )?function ${name}\\([^]*?\\n}`));
    assert.ok(body, `production ${name} seam exists`); vm.runInContext(body[0], sandbox);
  }
  const request = (correlation, command, tab = 7) => ({ kind: "request", request: {
    correlation, workspace: `workspace_${tab}`, attention: "background", command } });
  const scoped = (correlation, primitive, tab = 7) => request(correlation, { command: "in_documents",
    scope: { documents: documentApi.inventory(raw(tab)), allowed: [`document-${tab}`], subjects: [], mask: null, watch_changes: true }, primitive }, tab);
  const execute = (id, script, tab = 7) => sandbox.onNativeMessage(scoped(id,
    { command: "evaluate_script", tab_id: tab, script, max_result_chars: 1000 }, tab));
  const read = (id, tab = 7) => sandbox.onNativeMessage(scoped(id, { command: "read_text", tab_id: tab, max_chars: 1000 }, tab));
  const cancel = id => sandbox.onNativeMessage(request(`cancel_${id}`, { command: "cancel", correlation: id }));
  return { sandbox, physical, replies, pending, pages, gates, documents, lifecycle, engine, options, captures, navigations, execute, read, cancel };
}
const pendingScript = "effects += 1; await gate; return effects;";

test("correlated Cancel retires the exact pending evaluation and admits a fresh scoped read without End/Start", async () => {
  const f = await fixture();
  const work = f.execute("physical_one", pendingScript);
  await until(() => f.pending.has(7));
  const cleanup = f.cancel("physical_one");
  const observation = f.read("fresh_read");
  await Promise.all([work, cleanup, observation]);
  assert.equal(f.pages.get(7).effects, 1);
  assert.equal(f.pages.get(7).draft, "retained-7");
  assert.equal(f.physical.filter(([method]) => method === "Runtime.evaluate").length, 1);
  assert.ok(f.replies.some(frame => frame.correlation === "physical_one" && frame.effect_unknown === true));
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "fresh_read" && frame.receipt.result.result.text === "retained-7"));
  assert.equal(f.documents.context(7), undefined);
  assert.equal(f.sandbox.scriptEvaluations.size, 0);
  assert.equal(f.sandbox.navigationWatchers.size, 0);
});

test("Cancel bypasses a saturated journal and does not spend another operation slot", async () => {
  const f = await fixture({ full: true });
  const work = f.execute("physical_one", pendingScript);
  await until(() => f.pending.has(7));
  assert.equal(f.engine.snapshot().records.length, 1);
  await Promise.all([f.cancel("physical_one"), work]);
  assert.equal(f.engine.snapshot().records.length, 1);
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "cancel_physical_one"));
  await f.engine.acknowledge("physical_one");
  await f.read("fresh_read");
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "fresh_read"));
  assert.equal(f.pages.get(7).effects, 1);
});

test("failed cleanup refuses observations without effects and leaves another workspace usable", async () => {
  const f = await fixture({ failDetach: true });
  const work = f.execute("physical_one", pendingScript);
  await until(() => f.pending.has(7));
  await f.cancel("physical_one");
  await Promise.all([f.read("refused_read"), f.read("other_read", 8)]);
  assert.ok(f.replies.some(frame => frame.correlation === "refused_read" && frame.code === "operation_cleanup_required" && !frame.effect_unknown));
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "other_read"));
  assert.equal(f.physical.filter(([method, id]) => method === "read" && id === 7).length, 0);
  assert.equal(f.pages.get(7).effects, 1);
  f.options.failDetach = false;
  await f.lifecycle.detachAll();
  await work;
  await f.read("after_explicit_release");
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "after_explicit_release"));
});

test("detach confirmation alone cannot clear a scope whose handler still has not settled", async () => {
  const f = await fixture({ ignoreDetach: true });
  const work = f.execute("physical_one", pendingScript);
  await until(() => f.pending.has(7));
  await f.cancel("physical_one");
  await f.read("refused_read");
  assert.ok(f.replies.some(frame => frame.correlation === "refused_read" && frame.code === "operation_cleanup_required"));
  assert.ok(f.documents.context(7));
  f.pending.get(7).reject(new Error("delayed session retirement"));
  await work;
  await f.read("fresh_read");
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "fresh_read"));
  assert.equal(f.pages.get(7).effects, 1);
});

test("delayed page continuation can still have effects after custody release without replay or a stopped claim", async () => {
  const f = await fixture();
  const work = f.execute("physical_one", "effects += 1; await gate; effects += 1; return effects;");
  await until(() => f.pending.has(7));
  await Promise.all([f.cancel("physical_one"), work]);
  assert.equal(f.pages.get(7).effects, 1);
  f.gates.get(7).resolve(); await turn();
  assert.equal(f.pages.get(7).effects, 2);
  assert.equal(f.physical.filter(([method]) => method === "Runtime.evaluate").length, 1);
  assert.ok(f.replies.some(frame => frame.correlation === "physical_one" && frame.effect_unknown));
  assert.ok(!JSON.stringify(f.replies).includes("page activity stopped"));
});

test("Cancel during setup prevents evaluation and releases only the acquired generation", async () => {
  const setup = deferred();
  const f = await fixture({ setup });
  const work = f.execute("physical_one", pendingScript);
  await until(() => f.physical.some(([method]) => method === "attach"));
  const cancel = f.cancel("physical_one");
  setup.resolve();
  await Promise.all([work, cancel]);
  assert.equal(f.pages.get(7).effects, 0);
  assert.equal(f.physical.filter(([method]) => method === "Runtime.evaluate").length, 0);
  await f.read("fresh_read");
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "fresh_read"));
});

test("late Cancel and old completion cannot delete a newer epoch's scope, watcher or lease", async () => {
  const f = await fixture();
  const old = f.execute("physical_old", pendingScript);
  await until(() => f.pending.has(7));
  await Promise.all([old, f.cancel("physical_old")]);
  f.sandbox.browserServiceEpoch = "service_two";
  await f.engine.activate("service_two");
  const current = f.execute("physical_old", pendingScript);
  await until(() => f.pending.has(7));
  const context = f.documents.context(7), watcher = f.sandbox.navigationWatchers.get(7);
  const before = f.physical.filter(([method]) => method === "detach").length;
  await f.sandbox.cancelBrowserOperation("service_one", "physical_old");
  f.gates.get(7).resolve(); await turn();
  assert.equal(f.documents.context(7), context);
  assert.equal(f.sandbox.navigationWatchers.get(7), watcher);
  assert.equal(f.physical.filter(([method]) => method === "detach").length, before);
  await Promise.all([f.cancel("physical_old"), current]);
});

test("a Cancel waiting on old negotiation cannot enter a newer service epoch", async () => {
  const f = await fixture();
  const negotiation = deferred(); f.sandbox.browserNegotiation = negotiation.promise;
  const stale = f.cancel("reused_id");
  f.sandbox.browserServiceEpoch = "service_two";
  await f.engine.activate("service_two");
  f.sandbox.browserNegotiation = Promise.resolve();
  const current = f.execute("reused_id", pendingScript);
  await until(() => f.pending.has(7));
  negotiation.resolve(); await stale;
  assert.equal(f.sandbox.scriptEvaluations.get("reused_id").cancelled, false);
  assert.equal(f.physical.filter(([method]) => method === "detach").length, 0);
  await Promise.all([f.cancel("reused_id"), current]);
});

test("an ordinary settled script adds no cancellation detach and leaves the next observation available", async () => {
  const f = await fixture();
  await f.execute("physical_one", "effects += 1; return false;");
  await f.read("fresh_read");
  assert.equal(f.pages.get(7).effects, 1);
  assert.equal(f.physical.filter(([method]) => method === "detach").length, 0);
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "fresh_read"));
});

test("Cancel preserves an in-flight recording capture sharing the evaluator attachment", async () => {
  const f = await fixture();
  const work = f.execute("physical_one", pendingScript);
  await until(() => f.pending.has(7));
  f.options.capture = deferred();
  const capture = f.sandbox.captureRecordingFrame({ tabId: 7 }, "sample");
  await until(() => f.physical.some(([method]) => method === "Page.captureScreenshot"));
  await f.cancel("physical_one");
  await f.read("refused_read");
  assert.equal(f.physical.filter(([method]) => method === "detach").length, 0);
  assert.ok(f.replies.some(frame => frame.correlation === "refused_read" && frame.code === "operation_cleanup_required" && !frame.effect_unknown));
  assert.equal(f.pending.has(7), true);
  f.options.capture.resolve({ data: "synthetic_frame" });
  assert.equal(await capture, true);
  assert.equal(f.captures.length, 1);
  assert.equal(f.lifecycle.owns(f.sandbox.scriptEvaluations.get("physical_one").lease), true);
  assert.equal(f.pages.get(7).effects, 1);
  assert.equal(f.pages.get(7).draft, "retained-7");
  await f.lifecycle.detachAll(); await work;
  await f.read("after_explicit_release");
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "after_explicit_release"));
});

test("Cancel preserves beforeunload navigation sharing the evaluator attachment", async () => {
  const f = await fixture();
  const work = f.execute("physical_one", pendingScript);
  await until(() => f.pending.has(7));
  const navigation = f.sandbox.navigateDiscardingBeforeUnload("ordinary_navigation", { tab_id: 7, url: "https://fixture.invalid/next" }, { background: true });
  await until(() => f.navigations.has("ordinary_navigation"));
  const watcher = f.sandbox.navigationWatchers.get(7);
  await f.cancel("physical_one");
  await f.read("refused_read");
  assert.equal(f.physical.filter(([method]) => method === "detach").length, 0);
  assert.equal(f.sandbox.navigationWatchers.get(7), watcher);
  assert.equal(f.sandbox.beforeUnloadAcceptors.get(7), watcher);
  assert.ok(f.replies.some(frame => frame.correlation === "refused_read" && frame.code === "operation_cleanup_required" && !frame.effect_unknown));
  f.navigations.get("ordinary_navigation").resolve(await f.sandbox.chrome.tabs.get(7));
  assert.equal((await navigation).outcome, "navigated");
  assert.equal(f.lifecycle.owns(f.sandbox.scriptEvaluations.get("physical_one").lease), true);
  assert.equal(f.physical.filter(([method]) => method === "navigate").length, 1);
  await f.lifecycle.detachAll(); await work;
  await f.read("after_explicit_release");
  assert.ok(f.replies.some(frame => frame.receipt?.correlation === "after_explicit_release"));
  assert.equal(f.pages.get(7).effects, 1);
});

test("an old ordinary navigation finally preserves the replacement generation's lease and watchers", async () => {
  const f = await fixture();
  await f.sandbox.topology.remember(7, "workspace_7");
  const command = { tab_id: 7, url: "https://fixture.invalid/next" };
  const old = f.sandbox.navigateDiscardingBeforeUnload("old_navigation", command, { background: true });
  await until(() => f.navigations.has("old_navigation"));
  await f.lifecycle.detachAll();
  const current = f.sandbox.navigateDiscardingBeforeUnload("current_navigation", command, { background: true });
  await until(() => f.navigations.has("current_navigation"));
  const watcher = f.sandbox.navigationWatchers.get(7);
  f.navigations.get("old_navigation").resolve(await f.sandbox.chrome.tabs.get(7));
  await old;
  assert.equal(f.sandbox.navigationWatchers.get(7), watcher);
  assert.equal(f.sandbox.beforeUnloadAcceptors.get(7), watcher);
  await f.lifecycle.unretain(7);
  assert.equal(f.lifecycle.attachedCount(), 1);
  f.navigations.get("current_navigation").resolve(await f.sandbox.chrome.tabs.get(7));
  await current;
  assert.equal(f.lifecycle.attachedCount(), 0);
  assert.equal(f.physical.filter(([method]) => method === "detach").length, 2);
  assert.equal(f.sandbox.navigationWatchers.has(7), false);
  assert.equal(f.sandbox.beforeUnloadAcceptors.has(7), false);
});

test("recording presentation failures still release their exact ordinary lease", async () => {
  for (const failure of ["hide", "show"]) {
    const f = await fixture();
    const work = f.execute("physical_one", pendingScript);
    await until(() => f.pending.has(7));
    f.options.failPresentation = failure;
    await assert.rejects(f.sandbox.captureRecordingFrame({ tabId: 7 }, "sample"), /presentation failure/);
    await Promise.all([f.cancel("physical_one"), work]);
    await f.read("fresh_read");
    assert.ok(f.replies.some(frame => frame.receipt?.correlation === "fresh_read"));
    assert.equal(f.pages.get(7).effects, 1);
    assert.equal(f.physical.filter(([method]) => method === "Runtime.evaluate").length, 1);
  }
});
