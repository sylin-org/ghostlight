"use strict";

// The protocol 4 dispatcher over the real modules and a scripted browser, shared by the dispatcher
// tests and the golden receipt tests: sessions, inventory, world, input, tabs, capture, evaluate and
// buffers are the real modules, and chrome.debugger and chrome.tabs are the scripted fakes.

const fs = require("node:fs");
const path = require("node:path");
const Buffers = require("../lib/buffers.js");
const Capture = require("../lib/capture.js");
const Diagnostics = require("../lib/diagnostics.js");
const Evaluate = require("../lib/evaluate.js");
const Primitives = require("../lib/primitives.js");
const Recording = require("../lib/recording.js");
const Tabs = require("../lib/tabs.js");
const { fakeChrome, wire } = require("./p4-fakes.js");
const { fakeBrowser } = require("./p4-chrome.js");

const GOLDEN = path.join(__dirname, "..", "..", "tests", "golden", "protocol4");
const TAB = 12;
const URL = "https://example.com/account";
const ACCOUNT_TAB = { id: TAB, windowId: 1, url: URL, title: "Account", active: true, status: "complete" };

/** The file stems of one golden fixture directory, sorted. */
const names = (directory) => fs.readdirSync(path.join(GOLDEN, directory)).filter((file) => file.endsWith(".json")).map((file) => file.slice(0, -5)).sort();

/** A frame as it crosses the native port: JSON, so absent and undefined fields look the same. */
const wireFrame = (frame) => JSON.parse(JSON.stringify(frame));

/** A promise and the function that opens it. */
const gate = () => { let open; const promise = new Promise((resolve) => { open = resolve; }); return { promise, open }; };

/** A primitive request as `Primitives.run` takes it. */
const request = (name, args) => ({ name, args });

/** The smallest arguments of a primitive that addresses the tab. */
const focus = { tab_id: TAB };

/**
 * Builds the dispatcher over the real modules and a scripted browser. `chrome` is the scripted
 * debugger (default: one tab with a same-process child frame). The module clock is `advance(ms)`
 * unless `recordingOptions.now` supplies one;
 * module sleeps are recorded in `waited` and take no time. `recordingOptions` ({ now, newId })
 * shapes the recording store. With `stubs`, the primitives named in the map run the given
 * function instead of their module (their real validators still run).
 */
function rig({
  tabs = [ACCOUNT_TAB],
  stubs = new Map(),
  chrome = fakeChrome(),
  attach,
  tabHandlers = {},
  onSleep = () => {},
  recordingOptions = {}
} = {}) {
  if (attach) chrome.attach = attach;
  let clock = 0;
  const waited = [];
  const moduleSleep = async (ms) => { waited.push(ms); };
  const cancellations = Primitives.createCancellations({ now: () => clock });
  // The modules that compare deadlines share one clock: the recording store's when it has one.
  const moduleNow = recordingOptions.now ?? (() => clock);
  const parts = wire(chrome, { isCancelled: cancellations.isCancelled, now: moduleNow, sleep: moduleSleep });
  const browser = fakeBrowser({ tabs });
  const encoder = { messages: [], reply: null, hasDocument: false, downloads: [] };
  Object.assign(browser, {
    runtime: {
      async sendMessage(message) {
        encoder.messages.push(message);
        return encoder.reply ? encoder.reply(message) : { ok: true };
      }
    },
    offscreen: {
      async hasDocument() { return encoder.hasDocument; },
      async createDocument() { encoder.hasDocument = true; },
      async closeDocument() { encoder.hasDocument = false; }
    },
    downloads: {
      onChanged: { addListener() {}, removeListener() {} },
      async download(options) { encoder.downloads.push(options); return 7; },
      async search() { return [{ state: "complete" }]; },
      async cancel() {}
    }
  });
  const hook = {
    calls: [],
    async manage(id) { this.calls.push(id); },
    async release() {}
  };
  const box = {};
  const recording = Recording.create({ ...recordingOptions, onStop: (...stop) => box.buffers.recordingEnded(...stop) });
  const real = {
    tabs: Tabs.create({ chromeApi: browser, lifecycle: parts.lifecycle, controlled: hook, isCancelled: cancellations.isCancelled }),
    capture: Capture.create({ lifecycle: parts.lifecycle, worldCalls: parts.world, chromeApi: browser }),
    evaluate: Evaluate.create({ lifecycle: parts.lifecycle, inventory: parts.inventory, sleep: moduleSleep }),
    buffers: Buffers.create({
      chromeApi: browser,
      lifecycle: parts.lifecycle,
      inventory: parts.inventory,
      worldCalls: parts.world,
      diagnostics: Diagnostics.create({ cursorSecret: "secret" }),
      recording,
      now: moduleNow
    })
  };
  box.buffers = real.buffers;
  const via = (name, handler) => (stubs.has(name)
    ? Object.freeze({ ...handler, run: (args, fx, correlation) => stubs.get(name)(args, fx, correlation) })
    : handler);
  const frames = [];
  const worldLog = { cancelled: [], inflight: new Map() };
  const modules = {
    tabs: {
      listTabs: via("list_tabs", real.tabs.listTabs),
      openTab: via("open_tab", real.tabs.openTab),
      navigate: via("navigate", real.tabs.navigate),
      focusTab: via("focus_tab", real.tabs.focusTab),
      closeTab: via("close_tab", real.tabs.closeTab),
      setZoom: via("set_zoom", real.tabs.setZoom),
      resizeWindow: via("resize_window", real.tabs.resizeWindow),
      ...tabHandlers
    },
    capture: { view: via("view", real.capture.view), capture: via("capture", real.capture.capture) },
    evaluate: { evaluateMain: via("evaluate_main", real.evaluate.evaluateMain) },
    buffers: { diagnostics: via("diagnostics", real.buffers.diagnostics), recording: via("recording", real.buffers.recording) },
    input: { run: stubs.get("input") ?? parts.input.run },
    inventory: { ...parts.inventory, framesPrimitive: stubs.get("frames") ?? parts.inventory.framesPrimitive },
    world: {
      ...parts.world,
      callPrimitive: stubs.get("call") ?? parts.world.callPrimitive,
      inflight: (correlation) => worldLog.inflight.get(correlation) ?? [],
      cancelToken: (entry) => worldLog.cancelled.push(entry)
    }
  };
  const slept = [];
  const primitives = Primitives.create({
    chromeApi: browser,
    send: (frame) => frames.push(frame),
    controlled: hook,
    cancellations,
    lifecycle: parts.lifecycle,
    sleep: async (ms) => { slept.push(ms); onSleep(ms); },
    ...modules
  });
  return {
    chrome, browser, hook, frames, primitives, cancellations, slept, waited, worldLog, encoder, recording,
    advance: (ms) => { clock += ms; },
    ...parts
  };
}

module.exports = { GOLDEN, TAB, URL, ACCOUNT_TAB, names, wireFrame, gate, request, focus, rig };
