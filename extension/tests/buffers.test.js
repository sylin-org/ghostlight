"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Buffers = require("../lib/buffers.js");
const Diagnostics = require("../lib/diagnostics.js");
const Recording = require("../lib/recording.js");
const { TOP, TOP_LOADER, CHILD, CHILD_LOADER, ROOT, fakeChrome, assets, wire, methods } = require("./p4-fakes.js");
const { golden, goldenArgs, execute, fakeBrowser } = require("./p4-chrome.js");

const TAB = 12;
const URL = "https://example.com/account";
const EMBED = "https://example.com/embed";
const SHOT = { width: 1000, height: 600 };
const METRICS = {
  cssVisualViewport: { pageX: 0, pageY: 0, clientWidth: SHOT.width, clientHeight: SHOT.height },
  cssContentSize: { width: SHOT.width, height: 2000 }
};
const GIF = { frame_count: 3, captured_frame_count: 3, duration_ms: 900, width: 1000, height: 600, byte_count: 77 };
const watchBoth = [
  { frame_id: TOP, loader_id: TOP_LOADER, url: URL },
  { frame_id: CHILD, loader_id: CHILD_LOADER, url: EMBED }
];
const guardOf = (documents, exact = false) => ({ documents, exact, unseen: 0 });

function setup({ withAssets = false, encoder = null } = {}) {
  const chrome = fakeChrome();
  let shots = 0;
  Object.assign(chrome.handlers, {
    "Page.getLayoutMetrics": async () => METRICS,
    "Page.captureScreenshot": async () => { shots += 1; return { data: Buffer.from([shots]).toString("base64") }; },
    "Page.startScreencast": async () => ({}),
    "Page.stopScreencast": async () => ({}),
    "Page.screencastFrameAck": async () => ({})
  });
  const box = {};
  const parts = wire(chrome, { local: { recording: (tabId) => Boolean(box.recording?.activeForTab(tabId)) } });
  const browser = fakeBrowser({ tabs: [{ id: TAB, windowId: 1, url: URL, title: "Account" }] });
  const log = { sent: [], offscreen: [], downloads: [], changed: 0 };
  let hasDocument = false;
  const downloadListeners = new Set();
  const chromeApi = Object.assign(browser, {
    runtime: {
      async sendMessage(message) {
        log.sent.push(message);
        if (message.kind === "encode_recording") return encoder ? encoder(message) : { ok: true, measurements: GIF, mime_type: "image/gif", data: "R0lGODlhAQABAAAAACw=", object_url: "blob:one" };
        return { ok: true };
      }
    },
    offscreen: {
      async hasDocument() { return hasDocument; },
      async createDocument(options) { log.offscreen.push(["create", options]); hasDocument = true; },
      async closeDocument() { log.offscreen.push(["close"]); hasDocument = false; }
    },
    downloads: {
      onChanged: { addListener: (listener) => downloadListeners.add(listener), removeListener: (listener) => downloadListeners.delete(listener) },
      async download(options) { log.downloads.push(options); return 7; },
      async search() { return [{ state: "complete" }]; },
      async cancel() {}
    }
  });
  let ids = 0;
  let time = 1_000_000;
  const clock = () => time;
  box.diagnostics = Diagnostics.create({ cursorSecret: "test-secret" });
  box.recording = Recording.create({
    now: clock,
    newId: () => `recording_${++ids}`,
    onStop: (...stop) => box.buffers.recordingEnded(...stop)
  });
  box.buffers = Buffers.create({
    chromeApi,
    lifecycle: parts.lifecycle,
    inventory: parts.inventory,
    worldCalls: parts.world,
    diagnostics: box.diagnostics,
    recording: box.recording,
    imageSize: async () => SHOT,
    now: clock,
    changed: () => { log.changed += 1; }
  });
  return { chrome, browser, log, ...parts, ...box, advance: (ms) => { time += ms; }, ready: withAssets ? parts.world.setAssets(assets()) : Promise.resolve(true) };
}

// A managed tab is retained, as the dispatcher's prelude makes it, so its domains and screencast survive a lease.
async function managed(options) {
  const rig = setup(options);
  await rig.ready;
  await rig.lifecycle.retain(TAB);
  rig.chrome.calls.length = 0;
  return rig;
}

const readArgs = (overrides = {}) => ({ op: "read", tab_id: TAB, source: "both", detail: "all", limit: 50, ...overrides });

// ---- diagnostics ----------------------------------------------------------------------------

test("the handlers carry validate, run, addresses and tabOf, and addressing follows the operation", () => {
  const { buffers } = setup();
  for (const handler of [buffers.diagnostics, buffers.recording]) {
    assert.equal(typeof handler.validate, "function");
    assert.equal(typeof handler.run, "function");
  }
  assert.equal(buffers.diagnostics.addresses({ op: "read" }), true);
  assert.equal(buffers.diagnostics.addresses({ op: "clear" }), false);
  assert.equal(buffers.diagnostics.tabOf({ op: "read", tab_id: 4 }), 4);
  const exported = (destination) => ({ op: "export", destination });
  assert.deepEqual(
    [{ op: "start", tab_id: 3 }, { op: "status" }, { op: "stop" }, { op: "discard" },
      exported({ destination: "client" }), exported({ destination: "download" }), exported({ destination: "page", tab_id: 9 })]
      .map((request) => buffers.recording.addresses(request)),
    [true, false, false, false, false, false, true]
  );
  assert.equal(buffers.recording.tabOf({ op: "start", tab_id: 3 }), 3);
  assert.equal(buffers.recording.tabOf(exported({ destination: "page", tab_id: 9 })), 9);
});

test("every golden diagnostics and recording request passes validation", () => {
  const { buffers } = setup();
  for (const name of ["diagnostics.read", "diagnostics.clear"]) assert.doesNotThrow(() => buffers.diagnostics.validate(goldenArgs(name)), name);
  for (const name of ["recording.start", "recording.status", "recording.stop", "recording.discard", "recording.export.download",
    "recording.export.client", "recording.export.page"]) {
    assert.doesNotThrow(() => buffers.recording.validate(goldenArgs(name)), name);
  }
});

test("bad diagnostics and recording arguments are invalid_request", async () => {
  const { buffers } = setup();
  const read = goldenArgs("diagnostics.read");
  const exported = goldenArgs("recording.export.page");
  const cases = [
    [buffers.diagnostics, null],
    [buffers.diagnostics, { op: "peek" }],
    [buffers.diagnostics, { ...read, limit: 0 }],
    [buffers.diagnostics, { ...read, limit: 201 }],
    [buffers.diagnostics, { ...read, source: "stderr" }],
    [buffers.diagnostics, { ...read, detail: "most" }],
    [buffers.diagnostics, { ...read, match_text: "" }],
    [buffers.diagnostics, { ...read, tab_id: 0 }],
    [buffers.diagnostics, { ...read, guard: { documents: "none", exact: false } }],
    [buffers.diagnostics, { op: "clear", tab_ids: [] }],
    [buffers.diagnostics, { op: "clear", tab_ids: [1, 1] }],
    [buffers.diagnostics, { op: "clear", tab_ids: [0] }],
    [buffers.recording, { op: "rewind" }],
    [buffers.recording, { op: "start", tab_id: -1 }],
    [buffers.recording, { op: "start", tab_id: TAB, watch: [{ frame_id: TOP }] }],
    [buffers.recording, { op: "start", tab_id: TAB, watch: "everything" }],
    [buffers.recording, { op: "status" }],
    [buffers.recording, { op: "stop", recording_id: "" }],
    [buffers.recording, { ...exported, max_output_bytes: 0 }],
    [buffers.recording, { ...exported, destination: { destination: "ftp" } }],
    [buffers.recording, { ...exported, destination: { destination: "download" } }],
    [buffers.recording, { ...exported, destination: { ...exported.destination, call: { message: {} } } }],
    [buffers.recording, { ...exported, destination: { ...exported.destination, tab_id: "12" } }]
  ];
  for (const [handler, args] of cases) {
    const result = await execute(handler, args);
    assert.equal(result.failure, "invalid_request", JSON.stringify(args));
    assert.equal(result.dispatched, false);
  }
});

test("a read opts the tab in, reads the ring after the cursor, and matches the golden receipt shape", async () => {
  const rig = await managed();
  const first = await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  assert.equal(first.dispatched, false);
  assert.deepEqual(first.ok.entries, []);
  assert.equal(first.ok.capture_started, true);
  assert.equal(first.ok.cleared_count, 0);
  assert.equal(methods(rig.chrome, "Runtime.enable").length + methods(rig.chrome, "Network.enable").length, 2);
  rig.diagnostics.consoleAPICalled(TAB, { type: "error", args: [{ type: "string", value: "Uncaught TypeError" }] });
  const second = await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a", after: first.ok.cursor ?? undefined }));
  assert.equal(second.ok.entries.length, 1);
  assert.deepEqual(Object.keys(second.ok.entries[0]).sort(), ["cursor", "entry", "level", "text", "timestamp_ms", "url"].sort());
  assert.equal(second.ok.entries[0].text, "Uncaught TypeError");
  assert.equal(second.ok.capture_started, false);
  const expected = golden("receipts/diagnostics.ok").ok;
  assert.deepEqual(Object.keys(second.ok).sort(), Object.keys(expected).sort());
  assert.equal(methods(rig.chrome, "Runtime.enable").length, 1, "a domain is enabled once");
});

test("the source decides which domains a read enables", async () => {
  const consoleOnly = await managed();
  await execute(consoleOnly.buffers.diagnostics, readArgs({ source: "console" }));
  assert.deepEqual([methods(consoleOnly.chrome, "Runtime.enable").length, methods(consoleOnly.chrome, "Network.enable").length], [1, 0]);
  const networkOnly = await managed();
  await execute(networkOnly.buffers.diagnostics, readArgs({ source: "network" }));
  assert.deepEqual([methods(networkOnly.chrome, "Runtime.enable").length, methods(networkOnly.chrome, "Network.enable").length], [0, 1]);
});

test("a different binding resets the ring and turns capture off and on again; the same binding keeps it", async () => {
  const rig = await managed();
  await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  rig.diagnostics.consoleAPICalled(TAB, { type: "log", args: [{ type: "string", value: "kept" }] });
  const same = await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  assert.equal(same.ok.entries.length, 1);
  assert.equal(methods(rig.chrome, "Runtime.disable").length, 0);
  const reset = await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_b" }));
  assert.deepEqual(reset.ok.entries, []);
  assert.equal(reset.ok.capture_started, true);
  assert.equal(methods(rig.chrome, "Runtime.disable").length, 1);
  assert.equal(methods(rig.chrome, "Network.disable").length, 1);
  assert.equal(methods(rig.chrome, "Runtime.enable").length, 2);
});

test("the first binding of a tab keeps the ring that began at the navigation", async () => {
  const rig = await managed();
  rig.diagnostics.enable(TAB);
  rig.diagnostics.consoleAPICalled(TAB, { type: "log", args: [{ type: "string", value: "during load" }] });
  const result = await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  assert.equal(result.ok.entries.length, 1);
  assert.equal(methods(rig.chrome, "Runtime.disable").length, 0);
});

test("a navigation on a bound tab starts a fresh ring and keeps capture on", async () => {
  const rig = await managed();
  await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  rig.diagnostics.consoleAPICalled(TAB, { type: "log", args: [{ type: "string", value: "old page" }] });
  rig.buffers.beforeNavigate(TAB);
  assert.equal(rig.diagnostics.isEnabled(TAB), true);
  rig.diagnostics.consoleAPICalled(TAB, { type: "log", args: [{ type: "string", value: "new page" }] });
  const result = await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_b" }));
  assert.deepEqual(result.ok.entries.map((entry) => entry.text), ["new page"], "the boundary already reset it, so the new binding adds no reset");
  assert.equal(methods(rig.chrome, "Runtime.disable").length, 0);
  rig.buffers.beforeNavigate(99);
  assert.equal(rig.diagnostics.isEnabled(TAB), true);
});

test("a read checks its guard and answers document_changed without reading", async () => {
  const rig = await managed();
  const stale = guardOf([{ frame_id: TOP, loader_id: "another", url: URL }]);
  const result = await execute(rig.buffers.diagnostics, readArgs({ guard: stale }));
  assert.equal(result.failure, "document_changed");
  assert.equal(result.dispatched, false);
  const fine = guardOf([{ frame_id: TOP, loader_id: TOP_LOADER, url: URL }]);
  assert.ok((await execute(rig.buffers.diagnostics, readArgs({ guard: fine }))).ok);
});

test("a cursor that is not this ring's is invalid_request", async () => {
  const rig = await managed();
  await execute(rig.buffers.diagnostics, readArgs());
  const result = await execute(rig.buffers.diagnostics, readArgs({ after: "diag_1_00000000" }));
  assert.equal(result.failure, "invalid_request");
});

test("a read that cannot enable a domain forgets the ring it started", async () => {
  const rig = await managed();
  rig.chrome.handlers["Network.enable"] = async () => { throw new Error("Network domain unavailable"); };
  const result = await execute(rig.buffers.diagnostics, readArgs());
  assert.equal(result.message, "Network domain unavailable");
  assert.equal(rig.diagnostics.isEnabled(TAB), false);
});

test("a read gives its lease back, so an unmanaged tab detaches", async () => {
  const rig = setup();
  await execute(rig.buffers.diagnostics, readArgs());
  assert.equal(rig.lifecycle.attachedCount(), 0);
});

test("a clear forgets the named rings, turns their capture off and counts them", async () => {
  const rig = await managed();
  await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  const cleared = await execute(rig.buffers.diagnostics, { op: "clear", tab_ids: [TAB, 40] });
  assert.deepEqual(cleared.ok, { cleared_count: 1 }, "only the tab that had a ring is counted");
  assert.deepEqual(Object.keys(cleared.ok), Object.keys(golden("receipts/diagnostics.ok_cleared").ok));
  assert.equal(cleared.dispatched, false);
  assert.equal(rig.diagnostics.isEnabled(TAB), false);
  assert.equal(methods(rig.chrome, "Runtime.disable").length, 1);
  const again = await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  assert.equal(again.ok.capture_started, true, "the cleared binding is forgotten");
  assert.equal(methods(rig.chrome, "Runtime.disable").length, 1, "and so no reset was needed");
});

test("a detach forgets the tab's diagnostics and its binding", async () => {
  const rig = await managed();
  await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  rig.buffers.tabDetached(TAB);
  assert.equal(rig.diagnostics.isEnabled(TAB), false);
  const reread = await execute(rig.buffers.diagnostics, readArgs({ binding: "binding_b" }));
  assert.equal(methods(rig.chrome, "Runtime.disable").length, 0, "a forgotten binding needs no reset");
  assert.ok(reread.ok);
});

// ---- recording ------------------------------------------------------------------------------

async function startRecording(rig, watch) {
  const result = await execute(rig.buffers.recording, { op: "start", tab_id: TAB, ...(watch ? { watch } : {}) });
  assert.equal(result.ok?.result, "started", JSON.stringify(result));
  return result.ok.summary.recording_id;
}

const summaryOf = (rig, id) => rig.recording.status("protocol4", id).summary;

test("start takes a seed frame, starts the screencast and reports the recording, matching the golden shape", async () => {
  const rig = await managed({ withAssets: true });
  const result = await execute(rig.buffers.recording, { ...goldenArgs("recording.start"), watch: watchBoth });
  assert.equal(result.ok.result, "started");
  assert.equal(result.ok.existing, false);
  assert.equal(result.dispatched, false);
  assert.deepEqual(Object.keys(JSON.parse(JSON.stringify(result.ok.summary))).sort(), Object.keys(golden("receipts/recording.ok").ok.summary).sort());
  assert.equal(result.ok.summary.state, "recording");
  assert.equal(result.ok.summary.frame_count, 1, "the seed frame");
  assert.deepEqual(result.ok.summary.source_urls, [URL, EMBED], "the seed frame notes every watched document");
  const order = methods(rig.chrome).filter((method) => ["Page.captureScreenshot", "Page.startScreencast"].includes(method));
  assert.deepEqual(order, ["Page.captureScreenshot", "Page.startScreencast"]);
  const screencast = rig.chrome.calls.find((call) => call.method === "Page.startScreencast").params;
  assert.deepEqual(screencast, {
    format: "jpeg", quality: Recording.JPEG_QUALITY, maxWidth: Recording.MAX_WIDTH, maxHeight: Recording.MAX_HEIGHT, everyNthFrame: 1
  });
  assert.equal(rig.log.changed, 1);
  assert.equal(rig.lifecycle.attachedCount(), 1, "the retained tab stays attached");
});

test("start sends the recording flag to the page worlds", async () => {
  const rig = await managed({ withAssets: true });
  await startRecording(rig);
  const ambient = rig.chrome.calls.filter((call) => call.method === "Runtime.callFunctionOn" && call.params.functionDeclaration.includes("rt.ambient"));
  assert.ok(ambient.length >= 1);
  assert.equal(ambient.at(-1).params.arguments[1].value.recording, true);
});

test("start again on a recording tab reports the existing recording and does nothing else", async () => {
  const rig = await managed();
  const id = await startRecording(rig);
  rig.chrome.calls.length = 0;
  const again = await execute(rig.buffers.recording, { op: "start", tab_id: TAB });
  assert.equal(again.ok.existing, true);
  assert.equal(again.ok.summary.recording_id, id);
  assert.equal(methods(rig.chrome, "Page.startScreencast").length, 0);
  assert.equal(methods(rig.chrome, "Page.captureScreenshot").length, 0);
});

test("start on a tab that is gone is tab_missing and records nothing", async () => {
  const rig = await managed();
  const result = await execute(rig.buffers.recording, { op: "start", tab_id: 99 });
  assert.equal(result.failure, "tab_missing");
  assert.equal(result.dispatched, false);
  assert.equal(rig.recording.count(), 0);
});

test("a start that cannot start the screencast discards the recording", async () => {
  const rig = await managed();
  rig.chrome.handlers["Page.startScreencast"] = async () => { throw new Error("screencast unavailable"); };
  const result = await execute(rig.buffers.recording, { op: "start", tab_id: TAB });
  assert.equal(result.message, "screencast unavailable");
  assert.equal(rig.recording.count(), 0);
  assert.ok(methods(rig.chrome, "Page.stopScreencast").length >= 1);
  assert.equal(rig.log.changed, 1);
});

test("a screencast frame is acknowledged and kept while the watched documents are the tab's documents", async () => {
  const rig = await managed();
  const id = await startRecording(rig, watchBoth);
  rig.advance(200);
  await rig.buffers.screencastFrame(TAB, { sessionId: 5, data: "Ag==" });
  assert.deepEqual(rig.chrome.calls.filter((call) => call.method === "Page.screencastFrameAck").map((call) => call.params), [{ sessionId: 5 }]);
  assert.equal(summaryOf(rig, id).frame_count, 2);
  assert.ok(summaryOf(rig, id).source_urls.includes(EMBED), "every document url of the inventory is noted");
});

test("a screencast frame ends a watched recording when the documents are no longer the watched set", async () => {
  const rig = await managed();
  const id = await startRecording(rig, watchBoth);
  rig.chrome.trees[ROOT].childFrames[0].frame.loaderId = "L-child-2";
  await rig.buffers.screencastFrame(TAB, { sessionId: 6, data: "Aw==" });
  const summary = summaryOf(rig, id);
  assert.equal(summary.state, "interrupted");
  assert.equal(summary.stop_reason, "document_boundary");
  assert.equal(summary.frame_count, 1, "the frame that showed the change is not kept");
  assert.ok(methods(rig.chrome, "Page.stopScreencast").length >= 1, "the screencast is stopped");
});

test("a watched recording also ends when a frame is added, removed or cannot be seen", async () => {
  const added = await managed();
  const addedId = await startRecording(added, watchBoth);
  added.chrome.trees[ROOT].childFrames.push({ frame: { id: "EXTRA", loaderId: "L-extra", url: "https://example.com/ad", parentId: TOP }, childFrames: [] });
  await added.buffers.screencastFrame(TAB, { sessionId: 1, data: "BA==" });
  assert.equal(summaryOf(added, addedId).stop_reason, "document_boundary");

  const unseen = await managed();
  const unseenId = await startRecording(unseen, watchBoth);
  unseen.chrome.frames.push({ frameId: 9, documentLifecycle: "active", url: "https://other.example/frame" });
  await unseen.buffers.screencastFrame(TAB, { sessionId: 1, data: "BA==" });
  assert.equal(summaryOf(unseen, unseenId).stop_reason, "document_boundary");

  const blind = await managed();
  const blindId = await startRecording(blind, watchBoth);
  blind.chrome.handlers["Page.getFrameTree"] = async () => { throw new Error("Debugger is not attached"); };
  await blind.buffers.screencastFrame(TAB, { sessionId: 1, data: "BA==" });
  assert.equal(summaryOf(blind, blindId).stop_reason, "document_boundary");
});

test("a recording with no watch keeps every frame and checks nothing", async () => {
  const rig = await managed();
  const id = await startRecording(rig);
  rig.chrome.calls.length = 0;
  rig.advance(200);
  await rig.buffers.screencastFrame(TAB, { sessionId: 1, data: "BQ==" });
  assert.equal(summaryOf(rig, id).frame_count, 2);
  assert.equal(methods(rig.chrome, "Page.getFrameTree").length, 0);
});

test("a screencast frame for a tab with no recording is acknowledged and dropped", async () => {
  const rig = await managed();
  await rig.buffers.screencastFrame(TAB, { sessionId: 1, data: "BQ==" });
  assert.equal(methods(rig.chrome, "Page.screencastFrameAck").length, 1);
  assert.equal(rig.recording.count(), 0);
});

test("a commit in any frame interrupts a watched recording; a top commit notes the url for any recording", async () => {
  const watched = await managed();
  const watchedId = await startRecording(watched, watchBoth);
  watched.buffers.committed({ tabId: TAB, frameId: 4, url: "https://example.com/inner" });
  assert.equal(summaryOf(watched, watchedId).stop_reason, "document_boundary");

  const open = await managed();
  const openId = await startRecording(open);
  open.buffers.committed({ tabId: TAB, frameId: 4, url: "https://example.com/inner" });
  assert.equal(summaryOf(open, openId).state, "recording", "no watch, no boundary");
  open.buffers.committed({ tabId: TAB, frameId: 0, url: "https://example.com/next?q=1#x" });
  assert.deepEqual(summaryOf(open, openId).source_urls, [URL, "https://example.com/next"]);
  open.buffers.committed({ tabId: 77, frameId: 0, url: "https://example.com/other-tab" });
});

test("status answers the summary, and not_found for a recording that is not there", async () => {
  const rig = await managed();
  const id = await startRecording(rig);
  const status = await execute(rig.buffers.recording, { op: "status", recording_id: id });
  assert.equal(status.ok.result, "status");
  assert.equal(status.ok.summary.recording_id, id);
  assert.equal(status.dispatched, false);
  assert.deepEqual((await execute(rig.buffers.recording, { op: "status", recording_id: "recording_zzz" })).ok, golden("receipts/recording.ok_not_found").ok);
});

test("stop takes a final frame, stops the screencast and freezes the recording; stopping again changes nothing", async () => {
  const rig = await managed();
  const id = await startRecording(rig);
  rig.chrome.calls.length = 0;
  const stopped = await execute(rig.buffers.recording, { op: "stop", recording_id: id });
  assert.equal(stopped.ok.result, "stopped");
  assert.equal(stopped.ok.changed, true);
  assert.equal(stopped.ok.summary.state, "frozen");
  assert.equal(stopped.ok.summary.stop_reason, "explicit");
  assert.equal(stopped.ok.summary.frame_count, 2, "seed and final");
  assert.equal(stopped.dispatched, false);
  assert.deepEqual(methods(rig.chrome).filter((method) => method.startsWith("Page.") && method !== "Page.getLayoutMetrics"),
    ["Page.captureScreenshot", "Page.stopScreencast"], "stopped once, by the stop itself");
  const again = await execute(rig.buffers.recording, { op: "stop", recording_id: id });
  assert.equal(again.ok.changed, false);
  assert.equal(again.ok.summary.state, "frozen");
  assert.equal((await execute(rig.buffers.recording, { op: "stop", recording_id: "recording_zzz" })).ok.result, "not_found");
});

test("a stop that cannot stop the screencast interrupts the recording and fails", async () => {
  const rig = await managed();
  const id = await startRecording(rig);
  rig.chrome.handlers["Page.stopScreencast"] = async () => { throw new Error("Detached while handling command"); };
  const result = await execute(rig.buffers.recording, { op: "stop", recording_id: id });
  assert.equal(result.message, "Detached while handling command");
  assert.equal(summaryOf(rig, id).state, "interrupted");
});

test("discard releases the bytes, stops an active screencast and forgets the recording", async () => {
  const rig = await managed();
  const id = await startRecording(rig, watchBoth);
  const held = summaryOf(rig, id).bytes_held;
  rig.chrome.calls.length = 0;
  const discarded = await execute(rig.buffers.recording, { op: "discard", recording_id: id });
  assert.deepEqual(discarded.ok, { result: "discarded", recording_id: id, released_bytes: held });
  assert.equal(methods(rig.chrome, "Page.stopScreencast").length, 1);
  assert.equal(rig.recording.count(), 0);
  assert.equal((await execute(rig.buffers.recording, { op: "discard", recording_id: id })).ok.result, "not_found");
  rig.buffers.committed({ tabId: TAB, frameId: 4, url: "https://example.com/inner" });
});

test("a frozen recording is discarded without a screencast to stop", async () => {
  const rig = await managed();
  const id = await startRecording(rig);
  await execute(rig.buffers.recording, { op: "stop", recording_id: id });
  rig.chrome.calls.length = 0;
  assert.equal((await execute(rig.buffers.recording, { op: "discard", recording_id: id })).ok.result, "discarded");
  assert.equal(methods(rig.chrome, "Page.stopScreencast").length, 0);
});

test("a detach interrupts the recording; settling the boundary interrupts every recording and drops every ring", async () => {
  const detached = await managed();
  const detachedId = await startRecording(detached);
  detached.buffers.tabDetached(TAB);
  assert.equal(summaryOf(detached, detachedId).stop_reason, "browser_detached");

  const settled = await managed();
  const settledId = await startRecording(settled);
  await execute(settled.buffers.diagnostics, readArgs({ binding: "binding_a" }));
  settled.chrome.calls.length = 0;
  await settled.buffers.settleBoundaryState();
  assert.equal(summaryOf(settled, settledId).stop_reason, "service_disconnected");
  assert.equal(settled.diagnostics.isEnabled(TAB), false);
  assert.equal(methods(settled.chrome, "Runtime.disable").length, 1);
  assert.equal(methods(settled.chrome, "Network.disable").length, 1);
});

// ---- export ---------------------------------------------------------------------------------

async function frozen(rig) {
  const id = await startRecording(rig);
  await execute(rig.buffers.recording, { op: "stop", recording_id: id });
  return id;
}

const exportArgs = (id, destination, max = 5_000_000) => ({ op: "export", recording_id: id, destination, max_output_bytes: max });

test("export to the client encodes in the offscreen document and returns the GIF, then closes the encoder", async () => {
  const rig = await managed();
  const id = await frozen(rig);
  const result = await execute(rig.buffers.recording, exportArgs(id, { destination: "client" }));
  assert.equal(result.ok.result, "exported");
  assert.deepEqual(result.ok.encoded, GIF);
  assert.deepEqual(result.ok.delivery, { delivery: "returned", mime_type: "image/gif", data: "R0lGODlhAQABAAAAACw=" });
  assert.equal(result.ok.summary.recording_id, id);
  assert.equal(result.dispatched, false);
  const encode = rig.log.sent.find((message) => message.kind === "encode_recording");
  assert.equal(encode.target, "ghostlight-offscreen");
  assert.equal(encode.transfer, "base64");
  assert.equal(encode.max_bytes, 5_000_000);
  assert.equal(encode.frames.length, 2);
  assert.deepEqual(rig.log.offscreen.map((entry) => entry[0]), ["create", "close"]);
  assert.equal(rig.log.offscreen[0][1].url, "offscreen.html");
  assert.deepEqual(Object.keys(result.ok).sort(), Object.keys(golden("receipts/recording.ok_exported_client").ok).sort());
});

test("export to a download hands the browser an object URL, waits for it, and releases the URL", async () => {
  const rig = await managed();
  const id = await frozen(rig);
  const result = await execute(rig.buffers.recording, exportArgs(id, { destination: "download", file_name: "demo.gif" }));
  assert.deepEqual(result.ok.delivery, golden("receipts/recording.ok_exported_download").ok.delivery);
  assert.equal(result.dispatched, false);
  assert.deepEqual(rig.log.downloads, [{ url: "blob:one", filename: "demo.gif", saveAs: false }]);
  assert.equal(rig.log.sent.find((message) => message.kind === "encode_recording").transfer, "object_url");
  assert.ok(rig.log.sent.some((message) => message.kind === "release_recording" && message.object_url === "blob:one"));
  assert.deepEqual(rig.log.offscreen.map((entry) => entry[0]), ["create", "close"]);
});

test("a download the browser interrupts is export_failed with its reason", async () => {
  const rig = await managed();
  const id = await frozen(rig);
  rig.browser.downloads.search = async () => [{ state: "interrupted" }];
  const result = await execute(rig.buffers.recording, exportArgs(id, { destination: "download", file_name: "demo.gif" }));
  assert.deepEqual(result.ok, { result: "export_failed", reason: "the browser could not write the replay" });
  assert.ok(rig.log.sent.some((message) => message.kind === "release_recording"));
});

test("an encoder that fails is export_failed, the encoder is closed, and nothing is dispatched", async () => {
  const rig = await managed({ encoder: () => ({ ok: false, reason: "The encoded recording is larger than the output budget." }) });
  const id = await frozen(rig);
  const result = await execute(rig.buffers.recording, exportArgs(id, { destination: "client" }, 10));
  assert.deepEqual(result.ok, golden("receipts/recording.ok_export_failed").ok);
  assert.equal(result.dispatched, false);
  assert.deepEqual(rig.log.offscreen.map((entry) => entry[0]), ["create", "close"]);
});

test("export of a recording that is not there is not_found and starts no encoder", async () => {
  const rig = await managed();
  const result = await execute(rig.buffers.recording, exportArgs("recording_zzz", { destination: "client" }));
  assert.equal(result.ok.result, "not_found");
  assert.deepEqual(rig.log.offscreen, []);
});

const pageDestination = (guard) => ({
  destination: "page",
  tab_id: TAB,
  call: { frame: { frame_id: TOP, loader_id: TOP_LOADER }, message: { kind: "attach_file", locator: "locator_a_1", file_name: "demo.gif" } },
  ...(guard ? { guard } : {})
});

test("export into a page runs the world call with the GIF as its attachment and dispatches", async () => {
  const rig = await managed({ withAssets: true });
  const id = await frozen(rig);
  const guard = guardOf([{ frame_id: TOP, loader_id: TOP_LOADER, url: URL }]);
  rig.chrome.calls.length = 0;
  const result = await execute(rig.buffers.recording, exportArgs(id, pageDestination(guard)));
  assert.equal(result.ok.result, "exported");
  assert.deepEqual(result.ok.delivery, { delivery: "attached", tab_id: TAB });
  assert.equal(result.dispatched, true);
  assert.deepEqual(result.watched, [TAB]);
  const call = rig.chrome.calls.find((entry) => entry.method === "Runtime.callFunctionOn" && entry.params.arguments?.length === 6);
  assert.deepEqual(call.params.arguments[1].value, { kind: "attach_file", locator: "locator_a_1", file_name: "demo.gif" });
  assert.deepEqual(call.params.arguments[4].value, { media_type: "image/gif", data: "R0lGODlhAQABAAAAACw=", size: GIF.byte_count });
  assert.equal(rig.log.sent.find((message) => message.kind === "encode_recording").transfer, "base64");
});

test("a page that refuses the file is export_failed, having dispatched", async () => {
  const rig = await managed({ withAssets: true });
  const id = await frozen(rig);
  const { World } = { World: require("../lib/world.js") };
  rig.chrome.handlers["Runtime.callFunctionOn"] = async (_target, params) => (
    params.functionDeclaration === World.SHIMS.CALL
      ? { result: { value: { ok: false, error: { code: "not_file_input", message: "Not a file input." } } } }
      : { result: { value: true } });
  const result = await execute(rig.buffers.recording, exportArgs(id, pageDestination()));
  assert.deepEqual(result.ok, { result: "export_failed", reason: "the page did not accept the replay" });
  assert.equal(result.dispatched, true);
});

test("a guard that fails before a page delivery is document_changed, with nothing dispatched and the encoder closed", async () => {
  const rig = await managed({ withAssets: true });
  const id = await frozen(rig);
  const stale = guardOf([{ frame_id: TOP, loader_id: "another", url: URL }]);
  const result = await execute(rig.buffers.recording, exportArgs(id, pageDestination(stale)));
  assert.equal(result.failure, "document_changed");
  assert.equal(result.dispatched, false);
  assert.deepEqual(rig.log.offscreen.map((entry) => entry[0]), ["create", "close"]);
});
