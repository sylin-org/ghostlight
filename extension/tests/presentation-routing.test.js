"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

// Exercise the worker's actual routing functions with independent tab/activity observations.
// The real MV3 journey separately verifies their delivered renderer effects in Chromium.
function fixture() {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  function section(first, next) {
    const start = source.indexOf(first), end = source.indexOf(next, start);
    assert.ok(start >= 0 && end > start, `worker function ${first} exists`);
    return source.slice(start, end);
  }
  const active = new Map();
  const delivered = [], deferred = [];
  let queries = 0;
  const sandbox = {
    activity: active, publishUiState() {},
    shared: { activityLabel: value => value, bounded: value => value },
    topology: { tabsFor: () => [7, 11], titleFor: () => "Ghostlight - fixture" },
    chrome: { tabs: { query: async () => { queries++; return [{ id: 7, active: false }, { id: 11, active: true }]; } } },
    preferences: { effects: true, captions: true },
    frames: { frameOf: () => null, TOP_FRAME_ID: 0 },
    tabIsVisible: async tab => tab === 11,
    deferPresentation: async (workspace, tab, signal) => deferred.push({ workspace, tab, signal }),
    contentIn: async (tab, frame, message) => { delivered.push({ tab, frame, message }); return { presented: true }; },
    presentationQueue: { forget: () => false }
  };
  vm.createContext(sandbox);
  vm.runInContext(section("async function presentationTab(", "async function tabIsVisible(")
    + section("function presentMessage(", "async function flushPendingPresentation(")
    + section("function updateActivity(", "function physicalTab("), sandbox);
  return { ...sandbox, active, delivered, deferred, queries: () => queries };
}

test("tabless operation feedback never guesses the workspace's active page", async () => {
  const state = fixture();
  for (const signal of ["start", "progress", "target", "completion"]) {
    assert.equal(await state.presentationTab("workspace", { signal, activity: "read", invocation: "read-a" }), undefined);
  }
  assert.equal(state.queries(), 0, "page routing does not inspect unrelated tabs");
  for (const tab of [7, 11]) for (const signal of ["start", "completion"]) {
    assert.equal(await state.presentationTab("workspace", { signal, tab_id: tab, invocation: `read-${tab}` }), tab);
  }
  assert.equal(state.queries(), 0);
  assert.equal(await state.presentationTab("workspace", { signal: "denial", invocation: "denied-before-target" }), 11,
    "an untargeted human guardrail notice retains its existing visible-workspace route");
});

test("an unseen denial finishes its own page activity before waiting for a human notice", async () => {
  const state = fixture();
  const signal = { signal: "denial", activity: "script", invocation: "script-a", tab_id: 7,
    phase: "Ghostlight held this action", detail: "A runtime guardrail paused browser work." };
  assert.equal(await state.deliverPresentation("workspace", signal), false);
  assert.equal(state.delivered.length, 1);
  const terminal = state.delivered[0];
  assert.equal(terminal.tab, 7);
  assert.equal(terminal.frame, 0);
  assert.equal(terminal.message.signal.invocation, signal.invocation);
  assert.equal(terminal.message.signal.signal, "completion");
  assert.equal(terminal.message.signal.activity, "quiet");
  assert.equal(terminal.message.signal.detail, null);
  assert.deepEqual(state.deferred, [{ workspace: "workspace", tab: 7, signal }],
    "the original notice remains queued for its own page");
  assert.equal(await state.deliverPresentation("workspace", { ...signal, tab_id: 11 }), true);
  assert.equal(state.delivered.length, 2);
  assert.equal(state.delivered[1].tab, 11);
  assert.equal(state.delivered[1].message.signal.signal, "denial");
  assert.equal(state.deferred.length, 1, "a visible denial does not need to be deferred");
});

test("each terminal outcome removes only its invocation from toolbar activity", () => {
  for (const terminal of ["completion", "denial", "attention"]) {
    const state = fixture();
    const signal = { signal: "start", activity: "read", phase: "Reading page" };
    state.updateActivity({ ...signal, invocation: "read-a", tab_id: 7 }, "workspace");
    state.updateActivity({ ...signal, invocation: "read-b", tab_id: 11 }, "workspace");
    state.updateActivity({ ...signal, invocation: "read-a", signal: terminal, tab_id: 7 }, "workspace");
    assert.equal(state.active.has("read-a"), false);
    assert.equal(state.active.has("read-b"), true);
    state.updateActivity({ ...signal, invocation: "read-b", signal: "completion", tab_id: 11 }, "workspace");
    assert.equal(state.active.size, 0);
  }
});
