"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const api = require("../lib/form-diagnostics.js");

function surface() {
  const listeners = new Map();
  return {
    addEventListener(name, listener) {
      if (!listeners.has(name)) listeners.set(name, new Set());
      listeners.get(name).add(listener);
    },
    removeEventListener(name, listener) { listeners.get(name)?.delete(listener); },
    fire(name, event = {}) {
      for (const listener of [...listeners.get(name) ?? []]) listener({ target: this, ...event });
    },
    listenerCount() { return [...listeners.values()].reduce((count, items) => count + items.size, 0); }
  };
}

function observerFixture(extra = {}) {
  let time = 10_000, nextTimer = 0, queries = 0, valueReads = 0;
  const document = { ...surface(), visibilityState: "visible", hasFocus: () => true };
  const window = { ...surface(), getComputedStyle: element => ({ display: element.hidden ? "none" : "block", visibility: "visible", opacity: "1" }) };
  const intervals = new Map(), timeouts = new Map(), rows = [];
  const controls = [];
  function control(overrides = {}) {
    const element = { tagName: "INPUT", type: "text", isConnected: true, raw: "PRIVATE_VALUE_SENTINEL",
      getAttribute: name => name === "aria-label" ? "PRIVATE_LABEL_SENTINEL" : null,
      getBoundingClientRect: () => ({ width: 200, height: 30 }), ...overrides };
    Object.defineProperty(element, "value", { get() { valueReads++; return this.raw; } });
    Object.defineProperty(element, "textContent", { get() { valueReads++; return this.raw; } });
    controls.push(element);
    return element;
  }
  const observer = api.createObserver({ document, window, queryControls: () => { queries++; return controls; },
    credentialClass: element => element.credential === true, emit: row => rows.push(row), now: () => time,
    setInterval: (callback, delay) => { const id = ++nextTimer; intervals.set(id, { callback, delay }); return id; },
    clearInterval: id => intervals.delete(id),
    setTimeout: (callback, delay) => { const id = ++nextTimer; timeouts.set(id, { callback, due: time + delay }); return id; },
    clearTimeout: id => timeouts.delete(id), ...extra });
  function advance(ms = api.INTERVAL_MS) {
    time += ms;
    for (const [id, timer] of [...timeouts]) if (timer.due <= time) { timeouts.delete(id); timer.callback(); }
    for (const timer of [...intervals.values()]) timer.callback();
  }
  return { observer, document, window, rows, controls, control, advance, intervals, timeouts,
    reads: () => ({ queries, valueReads }) };
}

test("form observation is inert while off and disabling removes listeners, timers, and reads", () => {
  const fixture = observerFixture();
  fixture.control();
  const result = {};
  assert.equal(fixture.observer.run("fill", () => result), result);
  fixture.advance();
  assert.deepEqual(fixture.reads(), { queries: 0, valueReads: 0 });
  assert.equal(fixture.document.listenerCount(), 0);
  fixture.observer.setEnabled(true);
  assert.equal(fixture.rows[0].event, "trace_started");
  assert.equal(fixture.rows[0].nonempty_count, 1);
  const enabledReads = fixture.reads();
  fixture.observer.setEnabled(false);
  assert.deepEqual(fixture.reads(), enabledReads, "turning off cannot take another sample");
  assert.equal(fixture.rows.at(-1).event, "trace_stopped");
  assert.equal(fixture.document.listenerCount() + fixture.window.listenerCount(), 0);
  assert.equal(fixture.intervals.size + fixture.timeouts.size, 0);
  fixture.document.fire("input"); fixture.window.fire("focus"); fixture.advance(1000);
  assert.deepEqual(fixture.reads(), enabledReads);
});

test("silent empty transitions and node replacement remain distinguishable without values", () => {
  const fixture = observerFixture();
  const element = fixture.control();
  fixture.observer.setEnabled(true);
  element.raw = "";
  fixture.advance();
  assert.equal(fixture.rows.at(-1).became_empty, 1);
  assert.equal(fixture.rows.at(-1).input_seen, false);
  assert.equal(fixture.rows.at(-1).trigger, "poll");
  assert.equal(fixture.rows.at(-1).elapsed_ms, 250);
  fixture.controls.splice(0, 1);
  fixture.control({ raw: "" });
  fixture.advance();
  assert.equal(fixture.rows.at(-1).added_count, 1);
  assert.equal(fixture.rows.at(-1).removed_count, 1);
  assert.equal(fixture.rows.at(-1).became_empty, 0);
  assert.equal(JSON.stringify(fixture.rows).includes("PRIVATE_"), false);
});

test("input signals coalesce without per-key events or counts and survive unchanged polls", () => {
  const fixture = observerFixture();
  const element = fixture.control({ raw: "" });
  fixture.observer.setEnabled(true);
  for (let index = 0; index < 500; index++) {
    fixture.document.fire("beforeinput", { target: element, data: "PRIVATE_EVENT_DATA", isTrusted: true });
    fixture.document.fire("input", { target: element, data: "PRIVATE_EVENT_DATA", isTrusted: index % 2 === 0 });
    fixture.document.fire("change", { target: element });
  }
  fixture.advance();
  assert.equal(fixture.rows.length, 1);
  element.raw = "PRIVATE_NEW_VALUE";
  fixture.advance();
  assert.equal(fixture.rows.length, 2);
  const row = fixture.rows.at(-1);
  for (const key of ["beforeinput_seen", "input_seen", "change_seen", "trusted_input_seen", "synthetic_input_seen"]) assert.equal(row[key], true);
  assert.equal(row.became_nonempty, 1);
  element.raw = "PRIVATE_DIFFERENT_NONEMPTY_VALUE";
  fixture.advance();
  assert.equal(fixture.rows.length, 2, "nonempty string replacement is deliberately not observed");
  fixture.document.fire("focusout", { target: element });
  assert.equal(fixture.rows.at(-1).input_seen, false);
  assert.equal(JSON.stringify(fixture.rows).includes("PRIVATE_"), false);
  assert.ok(!Object.keys(row).some(key => /length|input_count|key|data|value/.test(key)));
});

test("credential, hidden, file, and toggle controls are excluded before any value read", () => {
  const fixture = observerFixture();
  const protectedControls = [
    fixture.control({ credential: true }), fixture.control({ type: "password" }),
    fixture.control({ type: "hidden" }), fixture.control({ type: "file" }),
    fixture.control({ type: "checkbox" }), fixture.control({ type: "radio" }),
    fixture.control({ hidden: true })
  ];
  fixture.observer.setEnabled(true);
  assert.equal(fixture.reads().valueReads, 0);
  for (const target of protectedControls) fixture.document.fire("input", { target, isTrusted: true });
  fixture.document.fire("focusout");
  assert.equal(fixture.rows.at(-1).control_count, 0);
  assert.equal(fixture.rows.at(-1).input_seen, false);
});

test("ordinary textarea, select, and rich controls are capped by node identity", () => {
  const fixture = observerFixture();
  fixture.control({ tagName: "TEXTAREA" });
  fixture.control({ tagName: "SELECT", raw: "" });
  fixture.control({ tagName: "DIV", isContentEditable: true });
  for (let index = 0; index < 101; index++) fixture.control();
  fixture.observer.setEnabled(true);
  assert.equal(fixture.rows[0].control_count, api.CONTROL_LIMIT);
  assert.equal(fixture.rows[0].nonempty_count, api.CONTROL_LIMIT - 1);
  assert.equal(fixture.rows[0].limited, true);
  assert.equal(fixture.reads().valueReads, api.CONTROL_LIMIT);
});

test("lifecycle and reset checkpoints expose reload clues without URLs or event payloads", () => {
  const fixture = observerFixture();
  const target = fixture.control();
  fixture.observer.setEnabled(true);
  fixture.window.fire("focus", { target });
  assert.equal(fixture.rows.length, 1, "window capture ignores bubbled field focus");
  fixture.window.fire("blur");
  assert.equal(fixture.rows.at(-1).trigger, "window_blur");
  fixture.document.visibilityState = "hidden";
  fixture.document.fire("visibilitychange", { url: "PRIVATE_URL" });
  assert.equal(fixture.rows.at(-1).visibility, "hidden");
  fixture.window.fire("pagehide", { persisted: true });
  assert.equal(fixture.rows.at(-1).persisted, true);
  fixture.window.fire("pageshow", { persisted: false });
  assert.equal(fixture.rows.at(-1).persisted, false);
  fixture.document.fire("reset", { target, data: "PRIVATE_RESET" });
  assert.equal(fixture.rows.at(-1).trigger, "reset");
  assert.equal(JSON.stringify(fixture.rows).includes("PRIVATE_"), false);
});

test("trace expires after ten minutes, releases state, and only explicit enable restarts it", () => {
  const fixture = observerFixture();
  fixture.control(); fixture.observer.setEnabled(true);
  fixture.advance(api.DURATION_MS);
  assert.equal(fixture.rows.at(-1).event, "trace_expired");
  assert.equal(fixture.rows.at(-1).elapsed_ms, api.DURATION_MS);
  assert.equal(fixture.document.listenerCount() + fixture.window.listenerCount(), 0);
  assert.equal(fixture.intervals.size + fixture.timeouts.size, 0);
  const reads = fixture.reads(); fixture.advance();
  assert.deepEqual(fixture.reads(), reads);
  fixture.observer.setEnabled(true);
  assert.equal(fixture.rows.at(-1).event, "trace_expired", "scope synchronization cannot reset expiry");
  fixture.observer.setEnabled(false); fixture.observer.setEnabled(true);
  assert.equal(fixture.rows.at(-1).event, "trace_started");
  assert.equal(fixture.rows.at(-1).elapsed_ms, 0);
});

test("a previously ordinary control becoming credential-class contributes no input signal", () => {
  const fixture = observerFixture();
  const element = fixture.control(); fixture.observer.setEnabled(true);
  element.credential = true;
  const reads = fixture.reads().valueReads;
  fixture.document.fire("input", { target: element, isTrusted: true });
  fixture.document.fire("focusout", { target: element });
  assert.equal(fixture.reads().valueReads, reads);
  assert.equal(fixture.rows.at(-1).input_seen, false);
});

test("operation boundaries preserve exact callback results and errors despite logging failures", () => {
  const fixture = observerFixture();
  const element = fixture.control(); fixture.observer.setEnabled(true);
  const result = {};
  assert.equal(fixture.observer.run("clear", () => { element.raw = ""; return result; }), result);
  assert.equal(fixture.rows.at(-2).event, "operation_started");
  assert.equal(fixture.rows.at(-1).event, "operation_finished");
  assert.equal(fixture.rows.at(-1).succeeded, true);
  assert.equal(fixture.rows.at(-1).became_empty, 1);
  const failure = new Error("PRIVATE_EXCEPTION");
  assert.throws(() => fixture.observer.run("fill", () => { throw failure; }), error => error === failure);
  assert.equal(fixture.rows.at(-1).succeeded, false);
  const rowCount = fixture.rows.length;
  assert.equal(fixture.observer.run("PRIVATE_OPERATION", () => result), result);
  assert.equal(fixture.rows.length, rowCount);
  const broken = observerFixture({ emit: () => { throw new Error("sink failed"); } });
  broken.control(); broken.observer.setEnabled(true);
  assert.equal(broken.observer.run("type", () => result), result);
  assert.throws(() => broken.observer.run("type", () => { throw failure; }), error => error === failure);
  assert.equal(JSON.stringify(fixture.rows).includes("PRIVATE_"), false);
});

test("unavailable page sampling cannot hide operation boundaries or invent removed controls", () => {
  const fixture = observerFixture({ queryControls: () => { throw new Error("PRIVATE_PAGE_FAILURE"); } });
  fixture.observer.setEnabled(true);
  assert.equal(fixture.rows[0].event, "trace_started");
  assert.equal(fixture.rows[0].limited, true);
  const result = {};
  assert.equal(fixture.observer.run("fill", () => result), result);
  assert.equal(fixture.rows.at(-1).event, "operation_finished");
  assert.equal(fixture.rows.at(-1).succeeded, true);
  assert.equal(fixture.rows.at(-1).removed_count, 0);
  assert.equal(JSON.stringify(fixture.rows).includes("PRIVATE_"), false);
});

function logFixture(enabled = true) {
  const saved = { debug: enabled };
  return { saved, storage: { get: async () => structuredClone(saved),
    set: async values => Object.assign(saved, structuredClone(values)) } };
}
const identity = { tab_id: 7, document_id: "01234567-89ab-cdef-0123-456789abcdef" };
const row = { event: "checkpoint", trigger: "poll", control_count: 2, became_empty: 1, input_seen: false, elapsed_ms: 250 };

test("diagnostic identity accepts only Chrome document UUIDs and positive safe tab IDs", () => {
  assert.equal(api.validIdentity(identity), true);
  assert.equal(api.validIdentity({ tab_id: 1, document_id: "0123456789ABCDEF0123456789ABCDEF" }), true);
  for (const invalid of [undefined, {}, { ...identity, tab_id: 0 }, { ...identity, tab_id: 1.5 },
    { ...identity, tab_id: Number.MAX_SAFE_INTEGER + 1 }, { ...identity, document_id: "PRIVATE_DOCUMENT" }]) {
    assert.equal(api.validIdentity(invalid), false);
  }
});

test("form log survives reload, caps rows, reprojects stored data, and rejects invented identity", async () => {
  const { saved, storage } = logFixture();
  const log = api.createLog({ storage, debugKey: "debug", now: () => 12_345 });
  for (let index = 0; index <= api.LIMIT; index++) await log.record({ ...row,
    value: "PRIVATE_VALUE", name: "PRIVATE_NAME", length: 987654321, data: "PRIVATE_EVENT_DATA",
    operation: "PRIVATE_OPERATION", visibility: "PRIVATE_VISIBILITY", error: "PRIVATE_ERROR" }, identity);
  assert.equal((await log.snapshot()).entries.length, api.LIMIT);
  saved[api.KEY].entries[0].data = "PRIVATE_STORED_PAYLOAD";
  saved[api.KEY].entries[1].tab_id = "PRIVATE_TAB_ID";
  saved[api.KEY].entries[2].document_id = "PRIVATE_DOCUMENT_ID";
  saved[api.KEY].entries[3].event = "PRIVATE_EVENT";
  saved[api.KEY].entries[4].control_count = "PRIVATE_COUNT";
  saved[api.KEY].entries[5].control_count = 101;
  const restored = api.createLog({ storage, debugKey: "debug" });
  const snapshot = await restored.snapshot();
  assert.equal(snapshot.entries.length, api.LIMIT - 3);
  assert.equal(snapshot.entries[0].time_ms, 12_345);
  assert.equal(snapshot.entries[0].document_id, identity.document_id);
  assert.equal(JSON.stringify(snapshot).includes("PRIVATE_"), false);
  assert.ok(snapshot.entries.every(entry => entry.control_count === undefined || entry.control_count <= 100));
  for (const invalid of [{ ...identity, document_id: "not-a-chrome-document" }, { ...identity, tab_id: NaN }]) await restored.record(row, invalid);
  await restored.record({ ...row, event: "PRIVATE_EVENT" }, identity);
  assert.equal((await restored.snapshot()).entries.length, snapshot.entries.length);
  snapshot.entries[0].event = "PRIVATE_MUTATION";
  assert.equal((await restored.snapshot()).entries[0].event, "checkpoint");
});

test("logging off performs no writes, preserves readable snapshots, and swallows storage failure", async () => {
  const { saved, storage } = logFixture(false);
  const log = api.createLog({ storage, debugKey: "debug", now: () => 1000 });
  await log.record(row, identity);
  assert.equal(saved[api.KEY], undefined);
  await log.setEnabled(true); await log.record(row, identity);
  await log.setEnabled(false); await log.record(row, identity);
  assert.equal((await log.snapshot()).entries.length, 1);
  assert.equal((await log.snapshot()).enabled, false);
  await log.setEnabled(true);
  storage.set = async () => { throw new Error("PRIVATE_STORAGE_FAILURE"); };
  await log.record(row, identity);
  assert.equal((await log.snapshot()).write_failures, 1);
  assert.equal(JSON.stringify(await log.snapshot()).includes("PRIVATE_"), false);
  const unreadable = api.createLog({ storage: { get: async () => { throw new Error("read failed"); } }, debugKey: "debug" });
  assert.equal((await unreadable.snapshot()).write_failures, 1);
});
