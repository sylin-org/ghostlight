"use strict";

// The protocol 4 worker entry, run in a vm context over the real lib modules, a scripted browser
// (tabs, windows, storage, alarms), a scripted debugger and a scripted native port. The tests drive
// the worker only through the port, the Chrome listeners and the extension message channel.

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");
const { webcrypto } = require("node:crypto");
const World = require("../lib/world.js");
const { golden, goldenArgs } = require("./p4-chrome.js");
const { fakeBrowser } = require("./p4-chrome.js");
const { fakeChrome, assets } = require("./p4-fakes.js");

const ROOT = join(__dirname, "..");
const WORKER_SOURCE = readFileSync(join(ROOT, "worker.js"), "utf8");
const TAB = 12;
const OTHER_TAB = 13;
const WINDOW = 1;
const EXTENSION_ID = "ghostlight-extension";
const VERSION = "2.0.0";
const CONTROLLED_KEY = "ghostlight.controlled";
const PRESENTATIONS_KEY = "ghostlight.presentations";
const OUTDATED = "Update Ghostlight on this computer.";
const ENGINE = "131.0.6778.86";
const BRANDS = [{ brand: "Not A;Brand", version: "99" }, { brand: "Chromium", version: "131" }, { brand: "Chrome", version: "131" }];
const ACCOUNT = { id: TAB, windowId: WINDOW, url: "https://example.com/account", title: "Account", active: true, status: "complete" };
const OTHER = { id: OTHER_TAB, windowId: WINDOW, url: "https://example.com/other", title: "Other", active: false, status: "complete" };

function event() {
  const listeners = [];
  return { listeners, addListener(listener) { listeners.push(listener); }, emit(...args) { return listeners.map((listener) => listener(...args)); } };
}

function makePort(name) {
  const port = {
    name,
    posted: [],
    disconnected: false,
    onMessage: event(),
    onDisconnect: event(),
    postMessage(frame) { port.posted.push(JSON.parse(JSON.stringify(frame))); },
    disconnect() { port.disconnected = true; }
  };
  return port;
}

// setTimeout that runs for real but is also recorded, so a test can fire a timer by its delay.
function makeTimers() {
  const records = [];
  return {
    records,
    setTimeout(callback, ms, ...args) {
      const record = { callback, ms, cleared: false, handle: null };
      record.handle = setTimeout(() => { if (!record.cleared) callback(...args); }, ms);
      record.handle.unref?.();
      records.push(record);
      return record.handle;
    },
    clearTimeout(handle) {
      for (const record of records) if (record.handle === handle) record.cleared = true;
      clearTimeout(handle);
    },
    pending(ms) { return records.filter((record) => record.ms === ms && !record.cleared); },
    fire(ms) {
      const record = this.pending(ms)[0];
      assert.ok(record, `a pending ${ms} ms timer`);
      record.cleared = true;
      clearTimeout(record.handle);
      record.callback();
    },
    dispose() { for (const record of records) { record.cleared = true; clearTimeout(record.handle); } }
  };
}

/** The controlled set as the worker persisted it, in this realm. */
const controlledStore = (worker) => JSON.parse(JSON.stringify(worker.browser.store.session.get(CONTROLLED_KEY) ?? null));
// Lets the worker finish what it started. SHA-256 runs on the thread pool, so timer turns are mixed in.
const flush = async () => {
  for (let round = 0; round < 2; round += 1) {
    await new Promise((resolve) => setTimeout(resolve, 3));
    for (let turn = 0; turn < 10; turn += 1) await new Promise((resolve) => setImmediate(resolve));
  }
};
const wireFrame = (frame) => JSON.parse(JSON.stringify(frame));
const gate = () => { let open; const promise = new Promise((resolve) => { open = resolve; }); return { promise, open }; };

/**
 * Loads worker.js into a fresh vm context and lets it connect. Options: `tabs`, `focused` (the window
 * has focus), `session` (seeded storage.session), `local` (seeded storage.local), `brands`,
 * `userAgent`, `connect` (false leaves the native host unreachable).
 */
async function boot({
  tabs = [ACCOUNT, OTHER],
  focused = true,
  session = {},
  local = {},
  brands = BRANDS,
  userAgent = `Mozilla/5.0 Chrome/${ENGINE} Safari/537.36`,
  highEntropy = { fullVersionList: BRANDS.map((entry) => ({ brand: entry.brand, version: entry.brand === "Chromium" ? ENGINE : "131.0.0.0" })) },
  connect = true
} = {}) {
  const browser = fakeBrowser({ tabs, focusedWindows: focused ? [WINDOW] : [] });
  for (const [key, value] of Object.entries(session)) browser.store.session.set(key, value);
  for (const [key, value] of Object.entries(local)) browser.store.local.set(key, value);
  const debuggerApi = fakeChrome();
  const attached = new Set();
  const detachCalls = [];
  debuggerApi.attach = async (target) => { attached.add(target.tabId); };
  debuggerApi.detach = async (target) => { attached.delete(target.tabId); detachCalls.push(target.tabId); };
  debuggerApi.onDetach = event();
  const ports = [];
  const runtimeMessages = [];
  const alarms = { created: [], cleared: [], onAlarm: event() };
  const badge = [];
  const timers = makeTimers();
  const chrome = {
    ...browser,
    runtime: {
      id: EXTENSION_ID,
      lastError: undefined,
      getManifest: () => ({ name: "Ghostlight in Browser", version: VERSION }),
      getURL: (path) => `chrome-extension://${EXTENSION_ID}/${path}`,
      connectNative(name) {
        if (!connect) throw new Error("Specified native messaging host not found.");
        const port = makePort(name);
        ports.push(port);
        return port;
      },
      async sendMessage(message) { runtimeMessages.push(message); return { ok: true }; },
      onInstalled: event(),
      onStartup: event(),
      onMessage: event()
    },
    alarms: {
      async create(name, info) { alarms.created.push({ name, info }); },
      async clear(name) { alarms.cleared.push(name); return true; },
      async get() { return { scheduledTime: 1 }; },
      onAlarm: alarms.onAlarm
    },
    action: {
      setBadgeText(value) { badge.push(wireFrame(value)); },
      setBadgeBackgroundColor() {},
      setTitle() {}
    },
    commands: { onCommand: event() },
    debugger: debuggerApi,
    webNavigation: { onCommitted: event(), onBeforeNavigate: event(), getAllFrames: debuggerApi.webNavigation.getAllFrames },
    offscreen: { async hasDocument() { return false; }, async createDocument() {}, async closeDocument() {} },
    downloads: { onChanged: event(), async download() { return 7; }, async search() { return [{ state: "complete" }]; }, async cancel() {} }
  };
  Object.assign(chrome.tabs, { onUpdated: event(), onActivated: event(), onRemoved: event() });
  Object.assign(chrome.windows, {
    WINDOW_ID_NONE: -1,
    onFocusChanged: event(),
    async getLastFocused() { return [...browser.windowList.values()].find((window) => window.focused) ?? { focused: false }; }
  });
  const sandbox = {
    chrome,
    crypto: webcrypto,
    navigator: { userAgent, userAgentData: { brands, getHighEntropyValues: async () => highEntropy } },
    atob,
    TextDecoder,
    TextEncoder,
    URL,
    structuredClone,
    console,
    setTimeout: timers.setTimeout.bind(timers),
    clearTimeout: timers.clearTimeout.bind(timers)
  };
  sandbox.importScripts = (...paths) => {
    for (const path of paths) vm.runInContext(readFileSync(join(ROOT, path), "utf8"), context, { filename: path });
  };
  const context = vm.createContext(sandbox);
  vm.runInContext(WORKER_SOURCE, context, { filename: "worker.js" });
  await flush();

  const harness = {
    chrome, browser, timers, ports, attached, detachCalls, runtimeMessages, alarms, badge,
    get port() { return ports[ports.length - 1]; },
    /** Receipts sent on one port (default: the latest). */
    receipts(port = harness.port) { return port.posted.filter((frame) => frame.kind === "receipt"); },
    /** Event frames sent on one port, as the event objects. */
    events(port = harness.port) { return port.posted.filter((frame) => frame.kind === "event").map((frame) => frame.event); },
    /** Delivers a service frame on the latest port. */
    async deliver(frame, port = harness.port) {
      port.onMessage.emit(wireFrame(frame));
      await flush();
    },
    async accept(overrides = {}) {
      await harness.deliver({ ...golden("frames/hello_accepted"), ...overrides });
    },
    async request(correlation, name, args) {
      await harness.deliver({ kind: "request", correlation, primitive: { name, args } });
    },
    /** The port closes from the other side. */
    async close(message, port = harness.port) {
      chrome.runtime.lastError = message ? { message } : undefined;
      port.onDisconnect.emit();
      chrome.runtime.lastError = undefined;
      await flush();
    },
    /** Sends an extension message the way a popup or options page does. */
    async ask(message, sender = { id: EXTENSION_ID, url: chrome.runtime.getURL("popup.html") }) {
      return new Promise((resolve) => {
        const kept = chrome.runtime.onMessage.emit(wireFrame(message), sender, (response) => resolve(wireFrame(response)));
        assert.ok(kept.some((value) => value === true) || message.target, "the listener keeps the channel open");
      });
    },
    /** The `local` facts every AMBIENT call carried, in order. */
    ambient() {
      return debuggerApi.calls
        .filter((call) => call.method === "Runtime.callFunctionOn" && call.params.functionDeclaration === World.SHIMS.AMBIENT)
        .map((call) => wireFrame(call.params.arguments[1].value));
    },
    /** The signals the page was asked to show, in order. */
    presented() {
      return debuggerApi.calls
        .filter((call) => call.method === "Runtime.callFunctionOn" && call.params.functionDeclaration === World.SHIMS.PRESENT)
        .map((call) => wireFrame(call.params.arguments[1].value));
    },
    focusCalls() {
      return debuggerApi.calls.filter((call) => call.method === "Emulation.setFocusEmulationEnabled").map((call) => call.params.enabled);
    },
    dispose() { timers.dispose(); }
  };
  return harness;
}

// Holds the next Page.getFrameTree answer, which a `frames` request is waiting on, until the gate opens.
function blockNextFrameTree(worker) {
  const held = gate();
  let armed = true;
  worker.chrome.debugger.handlers["Page.getFrameTree"] = async () => {
    if (armed) {
      armed = false;
      await held.promise;
    }
    return { frameTree: worker.chrome.debugger.trees.root };
  };
  return held;
}

// A harness that has finished the handshake and holds tab 12 as a managed, attached tab.
async function connected(options) {
  const worker = await boot(options);
  await worker.accept();
  await worker.request("p4_manage", "frames", goldenArgs("frames"));
  return worker;
}

test("the hello is the first frame and matches the golden hello", async () => {
  const worker = await boot();
  try {
    assert.equal(worker.ports.length, 1);
    assert.equal(worker.port.name, "org.sylin.ghostlight");
    assert.equal(worker.port.posted.length, 1);
    const hello = worker.port.posted[0];
    const fixture = golden("frames/hello");
    assert.deepEqual(hello, { ...fixture, browser_id: hello.browser_id });
    assert.match(hello.browser_id, /^browser_[0-9a-f]{32}$/);
  } finally { worker.dispose(); }
});

test("the hello reads the engine from the user agent when brands are unavailable and omits the unknown", async () => {
  const worker = await boot({ highEntropy: {}, brands: [] });
  try {
    const hello = worker.port.posted[0];
    assert.equal(hello.engine_version, ENGINE);
    assert.equal("browser_name" in hello, false);
    assert.equal(hello.attended, true);
  } finally { worker.dispose(); }
  const bare = await boot({ highEntropy: {}, userAgent: "Mozilla/5.0", focused: false });
  try {
    assert.equal("engine_version" in bare.port.posted[0], false);
    assert.equal(bare.port.posted[0].attended, false);
  } finally { bare.dispose(); }
});

test("a browser id is created once and kept", async () => {
  const first = await boot();
  const second = await boot({ local: { "ghostlight.browser_id": first.port.posted[0].browser_id } });
  try {
    assert.equal(second.port.posted[0].browser_id, first.port.posted[0].browser_id);
  } finally { first.dispose(); second.dispose(); }
});

test("simultaneous connect triggers open one port", async () => {
  const worker = await boot();
  try {
    worker.chrome.runtime.onStartup.emit();
    worker.chrome.runtime.onInstalled.emit({ reason: "update" });
    worker.chrome.alarms.onAlarm.emit({ name: "ghostlight-reconnect" });
    await flush();
    assert.equal(worker.ports.length, 1);
  } finally { worker.dispose(); }
});

test("installing the extension opens the setup page", async () => {
  const worker = await boot();
  try {
    worker.chrome.runtime.onInstalled.emit({ reason: "install" });
    await flush();
    assert.deepEqual(worker.browser.named("tabs.create").map((call) => call.args[0].url), ["https://sylin.org/ghostlight/chromium-extension/post-install/"]);
    worker.chrome.runtime.onInstalled.emit({ reason: "update" });
    await flush();
    assert.equal(worker.browser.named("tabs.create").length, 1);
  } finally { worker.dispose(); }
});

test("hello_accepted connects the link and applies the control state", async () => {
  const worker = await boot();
  try {
    await worker.accept({ control_state: "held" });
    const snapshot = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(snapshot.connected, true);
    assert.equal(snapshot.compatible, true);
    assert.equal(snapshot.control_state, "held");
    assert.equal(snapshot.service_version, "2.0.0");
    assert.equal(snapshot.link_state, "connected");
    assert.equal(snapshot.adapter_version, VERSION);
    assert.equal(snapshot.last_error, null);
    await worker.deliver({ kind: "control_state", state: "active" });
    assert.equal((await worker.ask({ kind: "ui_snapshot" })).value.control_state, "active");
  } finally { worker.dispose(); }
});

test("an unknown control state is ignored", async () => {
  const worker = await boot();
  try {
    await worker.accept();
    await worker.deliver({ kind: "control_state", state: "held" });
    await worker.deliver({ kind: "control_state", state: "attention" });
    assert.equal((await worker.ask({ kind: "ui_snapshot" })).value.control_state, "held");
  } finally { worker.dispose(); }
});

test("a heartbeat is acknowledged with its sequence", async () => {
  const worker = await boot();
  try {
    await worker.deliver(golden("frames/heartbeat"));
    assert.deepEqual(worker.port.posted.at(-1), golden("frames/heartbeat_ack"));
  } finally { worker.dispose(); }
});

test("page code that fails its checksum drops the connection", async () => {
  const worker = await boot();
  try {
    const frame = golden("frames/hello_accepted");
    await worker.accept({ assets: { ...frame.assets, source: `${frame.assets.source}tampered` } });
    assert.equal(worker.port.disconnected, true);
    const snapshot = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(snapshot.connected, false);
    assert.equal(snapshot.compatible, false);
    assert.equal(worker.alarms.created.length, 1);
  } finally { worker.dispose(); }
});

test("a second hello_accepted re-registers assets and settles state only on a new epoch", async () => {
  const worker = await connected();
  try {
    const registrations = () => worker.chrome.debugger.calls.filter((call) => call.method === "Page.addScriptToEvaluateOnNewDocument");
    assert.deepEqual(worker.focusCalls(), [true]);
    const first = registrations().length;
    assert.ok(first >= 1);

    const next = assets("\"use strict\";\nglobalThis.__ghostlightRuntime = { id: __ghostlightAssetId, second: true };");
    await worker.accept({ assets: next });
    assert.equal(registrations().length, first + 1, "new assets are registered on the live session");
    assert.ok(registrations().at(-1).params.source.includes("second: true"));
    assert.deepEqual(worker.focusCalls(), [true], "the same epoch settles nothing");

    await worker.accept({ assets: next });
    assert.equal(registrations().length, first + 1, "the same assets register once");

    await worker.accept({ assets: next, service_epoch: "service_other" });
    assert.deepEqual(worker.focusCalls(), [true, false, true], "a new epoch settles the boundary state");
  } finally { worker.dispose(); }
});

test("each request is answered exactly once", async () => {
  const worker = await boot();
  try {
    await worker.accept();
    await worker.request("p4_a", "list_tabs", goldenArgs("list_tabs"));
    await worker.request("p4_b", "no_such_primitive", {});
    await worker.request("p4_c", "set_zoom", { tab_id: "twelve" });
    await worker.request("p4_d", "focus_tab", { tab_id: 999 });
    const receipts = worker.receipts();
    assert.deepEqual(receipts.map((receipt) => receipt.correlation).sort(), ["p4_a", "p4_b", "p4_c", "p4_d"]);
    const byId = Object.fromEntries(receipts.map((receipt) => [receipt.correlation, receipt]));
    assert.ok(byId.p4_a.ok);
    assert.equal(byId.p4_b.failure, "unsupported");
    assert.equal(byId.p4_c.failure, "invalid_request");
    assert.equal(byId.p4_d.failure, "tab_missing");
    await flush();
    assert.equal(worker.receipts().length, 4);
  } finally { worker.dispose(); }
});

test("a request waits for the hello_accepted work and then runs once", async () => {
  const worker = await boot();
  try {
    worker.port.onMessage.emit(wireFrame(golden("frames/hello_accepted")));
    worker.port.onMessage.emit(wireFrame({ kind: "request", correlation: "p4_early", primitive: { name: "frames", args: goldenArgs("frames") } }));
    await flush();
    assert.equal(worker.receipts().length, 1);
    assert.ok(worker.receipts()[0].ok, "the world had its page code before the request ran");
    const order = worker.chrome.debugger.calls.map((call) => call.method);
    assert.ok(order.indexOf("Page.addScriptToEvaluateOnNewDocument") >= 0);
    assert.ok(order.indexOf("Page.addScriptToEvaluateOnNewDocument") < order.indexOf("Page.getFrameTree"), "page code is registered before the request reads the tab");
  } finally { worker.dispose(); }
});

test("a chunked request reassembles and is answered once", async () => {
  const worker = await boot();
  try {
    await worker.accept();
    await worker.deliver(golden("frames/command_chunk"));
    const receipts = worker.receipts();
    assert.equal(receipts.length, 1);
    assert.equal(receipts[0].correlation, "p4_c0ffee01");
    assert.ok(receipts[0].ok.tabs);
  } finally { worker.dispose(); }
});

test("a rejected chunk transfer is answered with one invalid_request receipt", async () => {
  const worker = await boot();
  try {
    await worker.accept();
    await worker.deliver({ ...golden("frames/command_chunk"), sha256: "0".repeat(64) });
    const receipts = worker.receipts();
    assert.equal(receipts.length, 1);
    assert.equal(receipts[0].correlation, "p4_c0ffee01");
    assert.equal(receipts[0].failure, "invalid_request");
    assert.equal(receipts[0].dispatched, false);
    assert.deepEqual(receipts[0].committed_urls, []);
  } finally { worker.dispose(); }
});

test("unknown frame kinds are ignored", async () => {
  const worker = await boot();
  try {
    await worker.accept();
    const before = worker.port.posted.length;
    const tolerance = golden("tolerance/frame.unknown_kind");
    assert.equal(tolerance.expect.ignored, true);
    await worker.deliver(tolerance.frame);
    await worker.deliver({ kind: "from_the_future", anything: [1, 2, 3] });
    await worker.deliver({ not: "a frame" });
    assert.equal(worker.port.posted.length, before);
    assert.equal((await worker.ask({ kind: "ui_snapshot" })).value.connected, true);
  } finally { worker.dispose(); }
});

test("a service error frame is shown without ending the connection", async () => {
  const worker = await boot();
  try {
    await worker.accept();
    await worker.deliver(golden("frames/error"));
    const snapshot = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(snapshot.connected, true);
    await worker.ask({ kind: "set_preferences", preferences: { diagnostics: true } });
    assert.match((await worker.ask({ kind: "ui_snapshot" })).value.last_error, /adapter protocol 4/);
  } finally { worker.dispose(); }
});

test("events go only for controlled tabs", async () => {
  const worker = await connected();
  try {
    const { chrome } = worker;
    chrome.webNavigation.onCommitted.emit({ tabId: TAB, frameId: 0, url: "https://example.com/account" });
    chrome.webNavigation.onCommitted.emit({ tabId: TAB, frameId: 3, url: "https://example.com/embed" });
    chrome.webNavigation.onCommitted.emit({ tabId: OTHER_TAB, frameId: 0, url: "https://example.com/other" });
    chrome.tabs.onUpdated.emit(TAB, { status: "complete" });
    chrome.tabs.onUpdated.emit(OTHER_TAB, { status: "complete" });
    chrome.tabs.onUpdated.emit(TAB, { title: "no status" });
    chrome.debugger.emit({ tabId: TAB }, "Page.javascriptDialogOpening", { type: "confirm", message: "Sure?" });
    chrome.debugger.emit({ tabId: OTHER_TAB }, "Page.javascriptDialogOpening", { type: "alert", message: "Hi" });
    await flush();
    const events = worker.events();
    assert.deepEqual(events, [
      golden("events/document_committed").event,
      { event: "readiness_changed", tab_id: TAB, readiness: "complete" },
      golden("events/dialog_changed").event
    ]);
    chrome.debugger.emit({ tabId: TAB }, "Page.javascriptDialogClosed", {});
    chrome.debugger.emit({ tabId: OTHER_TAB }, "Page.javascriptDialogClosed", {});
    chrome.tabs.onRemoved.emit(OTHER_TAB);
    chrome.tabs.onRemoved.emit(TAB);
    await flush();
    assert.deepEqual(worker.events().slice(3), [
      { event: "dialog_changed", tab_id: TAB, present: false, dialog_type: "unknown" },
      golden("events/tab_closed").event
    ]);
  } finally { worker.dispose(); }
});

test("a dialog is recorded with its message and session", async () => {
  const worker = await connected();
  try {
    worker.chrome.debugger.emit({ tabId: TAB, sessionId: "CHILD" }, "Page.javascriptDialogOpening", { type: "prompt", message: "Name?" });
    await flush();
    await worker.request("p4_dialog", "dialog", { tab_id: TAB, op: { op: "status" } });
    const receipt = worker.receipts().find((entry) => entry.correlation === "p4_dialog");
    assert.deepEqual(receipt.ok, { present: true, dialog_type: "prompt", message: "Name?" });
  } finally { worker.dispose(); }
});

test("a released tab reports one commit and then stops", async () => {
  const worker = await connected();
  try {
    const { chrome } = worker;
    await worker.request("p4_release", "close_tab", { tab_id: TAB, released: true });
    assert.equal(worker.receipts().find((entry) => entry.correlation === "p4_release").failure, "local_interlock");
    assert.equal(worker.attached.has(TAB), false, "release gives the debugger back");
    assert.deepEqual(controlledStore(worker), [[TAB, "released"]]);

    chrome.tabs.onUpdated.emit(TAB, { status: "loading" });
    chrome.webNavigation.onCommitted.emit({ tabId: TAB, frameId: 0, url: "https://example.com/next" });
    chrome.webNavigation.onCommitted.emit({ tabId: TAB, frameId: 0, url: "https://example.com/after" });
    chrome.tabs.onUpdated.emit(TAB, { status: "complete" });
    await flush();
    assert.deepEqual(worker.events(), [
      { event: "readiness_changed", tab_id: TAB, readiness: "loading" },
      { event: "document_committed", tab_id: TAB, url: "https://example.com/next" }
    ]);
    assert.deepEqual(controlledStore(worker), []);
  } finally { worker.dispose(); }
});

test("a removed tab drops out of the controlled set", async () => {
  const worker = await connected();
  try {
    worker.chrome.tabs.onRemoved.emit(TAB);
    await flush();
    assert.deepEqual(controlledStore(worker), []);
    worker.chrome.tabs.onRemoved.emit(TAB);
    await flush();
    assert.equal(worker.events().filter((entry) => entry.event === "tab_closed").length, 1);
  } finally { worker.dispose(); }
});

test("a request that addresses a tab marks it managed, retains it and tells its page", async () => {
  const worker = await connected();
  try {
    assert.equal(worker.attached.has(TAB), true);
    assert.deepEqual(controlledStore(worker), [[TAB, "managed"]]);
    assert.ok(worker.ambient().some((facts) => facts.managed === true && facts.runtime === "active"));
    assert.equal(worker.attached.has(OTHER_TAB), false, "an untouched tab is never attached");
  } finally { worker.dispose(); }
});

test("managed tabs are retained again with their state when the worker starts", async () => {
  const worker = await boot({ session: { [CONTROLLED_KEY]: [[TAB, "managed"], [OTHER_TAB, "released"], [99, "managed"], [14, "other"]] } });
  try {
    assert.deepEqual([...worker.attached], [TAB]);
    await worker.accept();
    assert.ok(worker.ambient().some((facts) => facts.managed === true));
    worker.chrome.webNavigation.onCommitted.emit({ tabId: OTHER_TAB, frameId: 0, url: "https://example.com/other" });
    worker.chrome.webNavigation.onCommitted.emit({ tabId: 99, frameId: 0, url: "https://example.com/gone" });
    await flush();
    assert.deepEqual(worker.events().map((entry) => entry.tab_id), [OTHER_TAB]);
  } finally { worker.dispose(); }
});

test("a notice queued for a restored managed tab is shown after the first hello_accepted", async () => {
  const queued = {
    workspace: "ws_1",
    tabId: TAB,
    signal: { invocation: "inv_1", signal: "denial", activity: "click", phase: "blocked", tab_id: TAB },
    expiresAt: Date.now() + 60_000
  };
  const worker = await boot({ session: { [CONTROLLED_KEY]: [[TAB, "managed"]], [PRESENTATIONS_KEY]: [queued] } });
  try {
    assert.equal(worker.presented().length, 0, "no page code exists before the hello is accepted");
    assert.equal(worker.browser.store.session.get(PRESENTATIONS_KEY).length, 1);
    await worker.accept();
    assert.equal(worker.presented().length, 1);
    assert.equal(worker.presented()[0].invocation, "inv_1");
    assert.equal(worker.browser.store.session.get(PRESENTATIONS_KEY).length, 0);
    assert.equal((await worker.ask({ kind: "ui_snapshot" })).value.unseen_denials, 0);
  } finally { worker.dispose(); }
});

test("a notice for a tab nobody is looking at waits in the persisted queue", async () => {
  const worker = await boot({ focused: false });
  try {
    await worker.accept();
    await worker.deliver({
      ...golden("frames/notice"),
      signal: { invocation: "inv_1", signal: "denial", activity: "click", phase: "blocked" },
      frame: undefined
    });
    const stored = worker.browser.store.session.get(PRESENTATIONS_KEY);
    assert.equal(stored.length, 1);
    assert.equal(stored[0].tabId, TAB);
    assert.equal((await worker.ask({ kind: "ui_snapshot" })).value.unseen_denials, 1);
    assert.deepEqual(worker.badge.at(-1), { text: "!" });
  } finally { worker.dispose(); }
});

test("browser-level events: attended and runtime control", async () => {
  const worker = await boot();
  try {
    worker.chrome.windows.onFocusChanged.emit(WINDOW);
    worker.chrome.windows.onFocusChanged.emit(-1);
    worker.chrome.commands.onCommand.emit("toggle-hold");
    worker.chrome.commands.onCommand.emit("something-else");
    await flush();
    assert.deepEqual(worker.events(), [golden("events/attended").event, golden("events/runtime_control_requested").event]);
    assert.deepEqual((await worker.ask({ kind: "runtime_control", intent: "end_session" })).value, { queued: true });
    assert.equal(worker.events().at(-1).intent, "end_session");
    const refused = await worker.ask({ kind: "runtime_control", intent: "format_disk" });
    assert.equal(refused.ok, false);
    assert.match(refused.error, /Unknown runtime control intent/);
  } finally { worker.dispose(); }
});

test("with no hello_accepted in 10 s the outdated-service state shows", async () => {
  const worker = await boot();
  try {
    assert.equal(worker.timers.pending(10_000).length, 1);
    worker.timers.fire(10_000);
    const snapshot = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(snapshot.link_state, "service_outdated");
    assert.equal(snapshot.last_error, OUTDATED);
    assert.equal(snapshot.connected, false);
    assert.equal(snapshot.compatible, false);

    await worker.accept();
    const healed = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(healed.link_state, "connected");
    assert.equal(healed.last_error, null);
    assert.equal(healed.compatible, true);
  } finally { worker.dispose(); }
});

test("an answered hello never shows the outdated-service state", async () => {
  const worker = await boot();
  try {
    await worker.accept();
    assert.equal(worker.timers.pending(10_000).length, 0);
  } finally { worker.dispose(); }
});

test("a service that closes on the hello and reports itself unavailable still shows the outdated-service state", async () => {
  const worker = await boot();
  try {
    await worker.deliver(golden("frames/backend_unavailable"));
    assert.equal(worker.ports.length, 1, "the connector keeps the port while it retries");
    assert.equal(worker.timers.pending(10_000).length, 1, "the hello wait survives the unavailable report");
    const waiting = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(waiting.link_state, "unreachable");

    worker.timers.fire(10_000);
    const snapshot = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(snapshot.link_state, "service_outdated");
    assert.equal(snapshot.last_error, OUTDATED);
  } finally { worker.dispose(); }
});

test("the outdated-service wait spans replaced ports and a later hello_accepted heals it", async () => {
  const worker = await boot();
  try {
    await worker.deliver(golden("frames/backend_unavailable"));
    await worker.close("Native host has exited.");
    assert.equal(worker.timers.pending(10_000).length, 1, "closing the port does not end the wait");
    worker.chrome.alarms.onAlarm.emit({ name: "ghostlight-reconnect", scheduledTime: 5 });
    await flush();
    assert.equal(worker.ports.length, 2);
    assert.equal(worker.port.posted[0].kind, "hello");
    assert.equal(worker.timers.pending(10_000).length, 1, "a new hello does not restart the wait");

    worker.timers.fire(10_000);
    await worker.close("Native host has exited.");
    const outdated = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(outdated.link_state, "service_outdated");
    assert.equal(outdated.last_error, OUTDATED);

    worker.chrome.alarms.onAlarm.emit({ name: "ghostlight-reconnect", scheduledTime: 6 });
    await flush();
    assert.equal(worker.ports.length, 3);
    assert.equal(worker.timers.pending(10_000).length, 0, "an elapsed wait is not armed again");
    await worker.accept();
    const healed = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(healed.link_state, "connected");
    assert.equal(healed.last_error, null);
  } finally { worker.dispose(); }
});

test("a disconnect cancels in-flight work, tells pages, and reconnects after 3 s", async () => {
  const worker = await connected();
  try {
    const held = blockNextFrameTree(worker);
    const first = worker.port;
    await worker.request("p4_stuck", "frames", goldenArgs("frames"));
    const before = worker.ambient().length;

    await worker.close("Native host has exited.");
    assert.ok(worker.ambient().length > before);
    assert.equal(worker.ambient().at(-1).runtime, "disconnected");
    const snapshot = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(snapshot.connected, false);
    assert.equal(worker.ports.length, 1, "no immediate reconnect after an abrupt exit");
    assert.deepEqual(worker.alarms.created.map((entry) => entry.name), ["ghostlight-reconnect"]);

    held.open();
    await flush();
    assert.equal(worker.receipts(first).some((entry) => entry.correlation === "p4_stuck"), false, "the dropped receipt is never sent");

    worker.timers.fire(3_000);
    await flush();
    assert.equal(worker.ports.length, 2);
    assert.equal(worker.port.posted[0].kind, "hello");
    assert.equal(worker.port.posted.length, 1);
  } finally { worker.dispose(); }
});

test("the reconnect alarm opens a new port", async () => {
  const worker = await boot();
  try {
    await worker.close("Native host has exited.");
    worker.chrome.alarms.onAlarm.emit({ name: "ghostlight-reconnect", scheduledTime: 5 });
    await flush();
    assert.equal(worker.ports.length, 2);
    assert.equal(worker.port.posted[0].kind, "hello");
  } finally { worker.dispose(); }
});

test("backend_unavailable cancels in-flight work, tells pages, and reconnects at once on the orderly exit", async () => {
  const worker = await connected();
  try {
    const held = blockNextFrameTree(worker);
    const first = worker.port;
    await worker.request("p4_stuck", "frames", goldenArgs("frames"));

    await worker.deliver(golden("frames/backend_unavailable"));
    assert.equal(worker.ambient().at(-1).runtime, "disconnected");
    const snapshot = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(snapshot.connected, false);
    assert.equal(snapshot.last_error, "The local Ghostlight service is unavailable.");
    assert.equal(worker.ports.length, 1, "the port stays until the connector exits");

    held.open();
    await flush();
    assert.equal(worker.receipts(first).some((entry) => entry.correlation === "p4_stuck"), false);

    await worker.close("Native host has exited.", first);
    assert.equal(worker.ports.length, 2, "an orderly exit after backend_unavailable reconnects at once");
    assert.equal(worker.port.posted[0].kind, "hello");
    assert.deepEqual(worker.alarms.created, []);
  } finally { worker.dispose(); }
});

test("a cancel for a request nobody sent answers nothing", async () => {
  const worker = await connected();
  try {
    const before = worker.port.posted.length;
    await worker.deliver(golden("frames/cancel"));
    await worker.deliver({ kind: "cancel" });
    assert.equal(worker.port.posted.length, before);
  } finally { worker.dispose(); }
});

test("a cancel that arrives before its request cancels it", async () => {
  const worker = await connected();
  try {
    await worker.deliver({ kind: "cancel", correlation: "p4_ahead" });
    await worker.request("p4_ahead", "frames", goldenArgs("frames"));
    const receipt = worker.receipts().find((entry) => entry.correlation === "p4_ahead");
    assert.equal(receipt.failure, "cancelled");
  } finally { worker.dispose(); }
});

test("control held pauses focus emulation and tells pages; active resumes", async () => {
  const worker = await connected();
  try {
    await worker.deliver({ kind: "control_state", state: "held" });
    assert.deepEqual(worker.focusCalls(), [true, false]);
    assert.equal(worker.ambient().at(-1).runtime, "held");
    assert.equal(worker.attached.has(TAB), true, "held keeps the sessions");
    await worker.deliver({ kind: "control_state", state: "active" });
    assert.deepEqual(worker.focusCalls(), [true, false, true]);
    assert.equal(worker.ambient().at(-1).runtime, "active");
  } finally { worker.dispose(); }
});

test("control ended detaches everything", async () => {
  const worker = await connected();
  try {
    await worker.request("p4_second", "frames", { tab_id: OTHER_TAB });
    assert.deepEqual([...worker.attached].sort(), [TAB, OTHER_TAB]);
    await worker.deliver({ kind: "control_state", state: "ended" });
    assert.deepEqual([...worker.attached], []);
    assert.deepEqual(worker.detachCalls.sort(), [TAB, OTHER_TAB]);
    assert.equal(worker.ambient().at(-1).runtime, "ended");
    assert.equal((await worker.ask({ kind: "ui_snapshot" })).value.attached_tabs, 0);
    await worker.deliver({ kind: "control_state", state: "active" });
    await worker.request("p4_again", "frames", goldenArgs("frames"));
    assert.ok(worker.receipts().find((entry) => entry.correlation === "p4_again").ok, "a new session can start");
    assert.equal(worker.attached.has(TAB), true);
  } finally { worker.dispose(); }
});

test("the extension pages' messages", async () => {
  const worker = await connected();
  try {
    assert.deepEqual((await worker.ask({ kind: "get_preferences" })).value,
      { effects: true, captions: false, diagnostics: false, preserveTabs: true });
    const before = worker.ambient().length;
    const saved = await worker.ask({ kind: "set_preferences", preferences: { effects: false, captions: true, diagnostics: true, preserveTabs: false } });
    assert.deepEqual(saved.value, { effects: false, captions: true, diagnostics: true, preserveTabs: false });
    await flush();
    assert.ok(worker.ambient().length > before, "a preference change reaches the pages");
    assert.deepEqual(worker.ambient().at(-1).preferences, { effects: false, captions: true });
    assert.equal(worker.browser.store.local.get("ghostlight_captions"), true);

    const toggle = await worker.ask({ kind: "diagnostics_toggle" });
    assert.equal(toggle.ok, false);
    assert.match(toggle.error, /workbench owns diagnostics/);

    const unknown = await worker.ask({ kind: "form_diagnostics_state" });
    assert.equal(unknown.ok, false);
    assert.equal(unknown.error, "Unknown extension message.");

    const encoderTraffic = worker.chrome.runtime.onMessage.emit({ target: "ghostlight-offscreen", kind: "encode_recording" }, {}, () => {});
    assert.deepEqual(encoderTraffic, [false], "the offscreen document answers its own messages");
  } finally { worker.dispose(); }
});

test("connection diagnostics are for the options page only", async () => {
  const worker = await boot();
  try {
    const refused = await worker.ask({ kind: "connection_diagnostics" }, { id: EXTENSION_ID, url: "https://example.com/" });
    assert.equal(refused.ok, false);
    assert.match(refused.error, /Open extension options/);
    const foreign = await worker.ask({ kind: "connection_diagnostics" }, { id: "other", url: worker.chrome.runtime.getURL("options.html") });
    assert.equal(foreign.ok, false);
    const allowed = await worker.ask({ kind: "connection_diagnostics" }, { id: EXTENSION_ID, url: worker.chrome.runtime.getURL("options.html") });
    assert.equal(allowed.ok, true);
    assert.equal(allowed.value.schema, "ghostlight-connection-log-1");
    assert.equal("form_diagnostics" in allowed.value, false);
  } finally { worker.dispose(); }
});

test("releasing debugger sessions is local and works without a service", async () => {
  const worker = await connected();
  try {
    await worker.close("Native host has exited.");
    assert.equal(worker.attached.has(TAB), true);
    const released = await worker.ask({ kind: "release_debugger_sessions" });
    assert.deepEqual(released.value, { released: true });
    assert.equal(worker.attached.has(TAB), false);
  } finally { worker.dispose(); }
});

test("the popup snapshot reports activity, tabs and the link before any service", async () => {
  const worker = await boot({ connect: false });
  try {
    assert.equal(worker.ports.length, 0);
    const snapshot = (await worker.ask({ kind: "ui_snapshot" })).value;
    assert.equal(snapshot.connected, false);
    assert.equal(snapshot.link_state, "host_absent");
    assert.equal(snapshot.attached_tabs, 0);
    assert.equal(snapshot.recording_tabs, 0);
    assert.deepEqual(snapshot.activity, []);
    assert.equal(worker.alarms.created.length, 1, "an unreachable host retries");
    assert.equal(worker.timers.pending(3_000).length, 1);
  } finally { worker.dispose(); }
});
