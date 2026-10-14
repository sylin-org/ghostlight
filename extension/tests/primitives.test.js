"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Primitives = require("../lib/primitives.js");
const World = require("../lib/world.js");
const protocol = require("../lib/protocol.js");
const sessions = require("../lib/sessions.js");
const { TOP, TOP_LOADER, assets } = require("./p4-fakes.js");
const { golden } = require("./p4-chrome.js");
const { TAB, URL, ACCOUNT_TAB, names, wireFrame, gate, request, focus, rig } = require("./p4-rig.js");

// ---- the table ------------------------------------------------------------------------------

test("the table has a handler for every primitive of the vocabulary and nothing else", () => {
  const { primitives } = rig();
  assert.deepEqual(Object.keys(primitives.handlers).sort(), [...protocol.vocabulary.primitives].sort());
  assert.ok(Object.isFrozen(primitives.handlers));
  for (const [name, spec] of Object.entries(primitives.handlers)) {
    assert.equal(typeof spec.validate, "function", name);
    assert.equal(typeof spec.run, "function", name);
    assert.equal(typeof spec.addresses, "function", name);
    assert.equal(typeof spec.tabOf, "function", name);
  }
});

test("a dispatcher without its collaborators is refused", () => {
  assert.throws(() => Primitives.create({}), TypeError);
  const parts = rig();
  assert.throws(() => Primitives.create({ send() {}, controlled: parts.hook, cancellations: parts.cancellations }), TypeError);
});

test("which primitives address a tab, and which tab", () => {
  const { primitives } = rig();
  const addressing = (args = {}) => Object.keys(primitives.handlers).filter((name) => primitives.handlers[name].addresses(args));
  assert.deepEqual(addressing({ op: { op: "status" } }).sort(), [
    "accessibility_snapshot", "call", "capture", "dialog", "evaluate_main", "focus_tab", "frames", "input", "navigate", "resize_window",
    "set_zoom", "view"
  ]);
  assert.equal(primitives.handlers.diagnostics.addresses({ op: "read" }), true);
  assert.equal(primitives.handlers.recording.addresses({ op: "start" }), true);
  assert.equal(primitives.handlers.call.tabOf({ tab_id: 9 }), 9);
});

// ---- validation -----------------------------------------------------------------------------

test("every golden request fixture passes its primitive's validator", () => {
  const { primitives } = rig();
  const stems = names("requests");
  assert.equal(stems.length, 40);
  for (const stem of stems) {
    const { primitive } = golden(`requests/${stem}`);
    assert.doesNotThrow(() => primitives.handlers[primitive.name].validate(primitive.args), stem);
  }
});

test("bad arguments to the primitives validated here are invalid_request", async () => {
  const fixture = (name) => golden(`requests/${name}`).primitive.args;
  const call = fixture("call.sequence");
  const input = fixture("input.mouse");
  const accessibility = fixture("accessibility_snapshot");
  const cases = [
    ["frames", { tab_id: "12" }],
    ["frames", null],
    ["call", { ...call, mode: "swarm" }],
    ["call", { ...call, calls: [] }],
    ["call", { ...call, calls: Array.from({ length: 257 }, () => call.calls[0]) }],
    ["call", { ...call, calls: [{ message: {} }] }],
    ["call", { ...call, settle_ms: -1 }],
    ["call", { ...call, timeout_ms: 0 }],
    ["call", { ...call, guard: { documents: [], exact: 1 } }],
    ["input", { ...input, steps: [] }],
    ["input", { ...input, steps: Array.from({ length: 4097 }, () => input.steps[0]) }],
    ["input", { ...input, steps: [{ step: "teleport" }] }],
    ["input", { ...input, steps: [{ step: "mouse", action: "hover", x: 1, y: 1 }] }],
    ["input", { ...input, steps: [{ step: "mouse", action: "moved", x: "1", y: 1 }] }],
    ["input", { ...input, steps: [{ step: "mouse", action: "moved", x: 1, y: 1, button: "thumb" }] }],
    ["input", { ...input, steps: [{ step: "key", action: "tap", key: "a" }] }],
    ["input", { ...input, steps: [{ step: "key", action: "down" }] }],
    ["input", { ...input, steps: [{ step: "insert_text" }] }],
    ["input", { ...input, steps: [{ step: "drag", path: [{ x: 1, y: 1 }] }] }],
    ["input", { ...input, steps: [{ step: "pause", ms: 1001 }] }],
    ["input", { ...input, precondition: { frame: {}, message: { kind: "focus" } } }],
    ["dialog", { tab_id: TAB, op: { op: "dismiss" } }],
    ["dialog", { tab_id: TAB, op: { op: "handle" } }],
    ["dialog", { tab_id: TAB, op: { op: "handle", accept: true, text: 5 } }],
    ["accessibility_snapshot", { ...accessibility, max_nodes: 5001 }],
    ["accessibility_snapshot", { ...accessibility, max_nodes: 0 }],
    ["accessibility_snapshot", { ...accessibility, frame: { frame_id: TOP } }],
    ["accessibility_snapshot", { ...accessibility, max_depth: -1 }],
    ["accessibility_snapshot", { ...accessibility, query: "button" }],
    ["accessibility_snapshot", { ...accessibility, query: { role: 7 } }]
  ];
  for (const [name, args] of cases) {
    const parts = rig();
    await parts.primitives.run("p4_bad", request(name, args));
    const [frame] = parts.frames;
    assert.equal(frame.failure, "invalid_request", `${name} ${JSON.stringify(args).slice(0, 80)}`);
    assert.equal(frame.dispatched, false);
    assert.deepEqual(parts.hook.calls, [], "an invalid request never marks a tab managed");
    assert.deepEqual(parts.chrome.calls, [], "and never touches the browser");
  }
});

test("a validator that fails without a code is still invalid_request, and one that names a code keeps it", async () => {
  const handler = (validate) => ({ validate, run: async () => ({}), addresses: () => true, tabOf: (args) => args.tab_id });
  const parts = rig({ tabHandlers: {
    focusTab: handler(() => { throw new TypeError("Cannot read properties of null"); }),
    setZoom: handler(() => { throw sessions.failure("tab_missing", "Named by the validator."); })
  } });
  await parts.primitives.run("a", request("focus_tab", focus));
  await parts.primitives.run("b", request("set_zoom", focus));
  assert.deepEqual(parts.frames.map((frame) => [frame.failure, frame.message]), [
    ["invalid_request", "Cannot read properties of null"],
    ["tab_missing", "Named by the validator."]
  ]);
  assert.deepEqual(parts.hook.calls, []);
});

// ---- the dispatcher -------------------------------------------------------------------------

test("an unknown primitive is unsupported with nothing dispatched, and so is a name that only exists on objects", async () => {
  const parts = rig();
  for (const name of ["install_assets", "constructor", "__proto__", "toString", "", undefined, null, 7]) {
    parts.frames.length = 0;
    await parts.primitives.run("p4_9e01", { name, args: {} });
    assert.deepEqual(wireFrame(parts.frames), [{ kind: "receipt", correlation: "p4_9e01", failure: "unsupported", dispatched: false, committed_urls: [] }], String(name));
  }
  await parts.primitives.run("p4_9e02", null);
  await parts.primitives.run("p4_9e03", undefined);
  assert.equal(parts.frames.length, 3);
  assert.equal(parts.hook.calls.length, 0);
  assert.deepEqual(parts.chrome.calls, []);
});

test("every request gets exactly one receipt: success, failure, invalid, unsupported and cancelled alike", async () => {
  const stubs = new Map([
    ["focus_tab", async (_args, fx) => { fx.dispatched = true; return { tab_id: TAB, active: true, window_focused: true }; }],
    ["set_zoom", async () => { throw sessions.failure("tab_missing", "No tab with id: 12."); }]
  ]);
  const parts = rig({ stubs });
  await parts.primitives.run("a", request("focus_tab", focus));
  await parts.primitives.run("b", request("set_zoom", { tab_id: TAB, zoom: 1 }));
  await parts.primitives.run("c", request("set_zoom", { tab_id: TAB }));
  await parts.primitives.run("d", request("warp", {}));
  parts.primitives.cancel("e");
  await parts.primitives.run("e", request("focus_tab", focus));
  assert.deepEqual(parts.frames.map((frame) => [frame.correlation, frame.ok ? "ok" : frame.failure]), [
    ["a", "ok"], ["b", "tab_missing"], ["c", "invalid_request"], ["d", "unsupported"], ["e", "cancelled"]
  ]);
});

test("the prelude marks the tab managed before the handler runs, for addressing primitives only", async () => {
  const order = [];
  const stubs = new Map([
    ["focus_tab", async () => { order.push("run"); return {}; }],
    ["list_tabs", async () => { order.push("list"); return { tabs: [] }; }]
  ]);
  const parts = rig({ stubs });
  parts.hook.manage = async (id) => { order.push(`manage ${id}`); };
  await parts.primitives.run("a", request("focus_tab", focus));
  await parts.primitives.run("b", request("list_tabs", {}));
  assert.deepEqual(order, ["manage 12", "run", "list"]);
});

test("a prelude that cannot take custody fails the request before anything runs", async () => {
  let ran = false;
  const parts = rig({ stubs: new Map([["focus_tab", async () => { ran = true; return {}; }]]) });
  parts.hook.manage = async () => { throw new Error("Cannot attach to this target."); };
  await parts.primitives.run("a", request("focus_tab", focus));
  assert.equal(ran, false);
  assert.equal(parts.frames[0].failure, "debugger_unavailable");
  assert.equal(parts.frames[0].dispatched, false);
});

test("the receipt carries the ok outcome, the effect facts, and the tab as it is afterwards", async () => {
  const stubs = new Map([["navigate", async (_args, fx) => {
    fx.watch(TAB);
    fx.dispatched = true;
    parts.primitives.committed(TAB, "https://example.com/next");
    return { tab: { tab_id: TAB } };
  }]]);
  const parts = rig({ stubs, tabs: [{ ...ACCOUNT_TAB, url: "https://example.com/next", title: "Next" }] });
  await parts.primitives.run("n1", request("navigate", { tab_id: TAB, to: { to: "url", url: "https://example.com/next" }, discard_beforeunload: false, ready_timeout_ms: 1000 }));
  assert.deepEqual(wireFrame(parts.frames[0]), {
    kind: "receipt",
    correlation: "n1",
    ok: { tab: { tab_id: TAB } },
    dispatched: true,
    committed_urls: ["https://example.com/next"],
    tab: { tab_id: TAB, title: "Next", url: "https://example.com/next", active: true, readiness: "complete" }
  });
});

test("no tab is read when nothing was dispatched, or when no tab was watched, or when the tab is gone", async () => {
  const run = async (script, tabs) => {
    const parts = rig({ stubs: new Map([["focus_tab", script]]), tabs });
    await parts.primitives.run("x", request("focus_tab", focus));
    return parts.frames[0];
  };
  assert.equal((await run(async (_a, fx) => { fx.watch(TAB); return {}; })).tab, undefined, "not dispatched");
  assert.equal((await run(async (_a, fx) => { fx.dispatched = true; return {}; })).tab, undefined, "no tab watched");
  const gone = await run(async (_a, fx) => { fx.watch(TAB); fx.dispatched = true; return {}; }, []);
  assert.equal(gone.tab, undefined, "the tab closed");
  assert.equal(gone.dispatched, true);
});

test("a failure after dispatch keeps its facts: the commits and the tab", async () => {
  const parts = rig({ stubs: new Map([["navigate", async (_args, fx) => {
    fx.watch(TAB);
    fx.dispatched = true;
    parts.primitives.committed(TAB, "https://example.com/");
    throw sessions.failure("timeout", "The tab did not become ready in 8000 ms.");
  }]]) });
  await parts.primitives.run("n", request("navigate", { tab_id: TAB, to: { to: "url", url: "https://example.com/" }, discard_beforeunload: false, ready_timeout_ms: 8000 }));
  const [frame] = parts.frames;
  assert.equal(frame.failure, "timeout");
  assert.equal(frame.message, "The tab did not become ready in 8000 ms.");
  assert.equal(frame.dispatched, true);
  assert.deepEqual(frame.committed_urls, ["https://example.com/"]);
  assert.equal(frame.tab.tab_id, TAB);
});

test("two primitives running on one tab both see a commit", async () => {
  const hold = gate();
  const stubs = new Map([
    ["focus_tab", async (_args, fx) => { fx.watch(TAB); fx.dispatched = true; await hold.promise; return {}; }],
    ["set_zoom", async (_args, fx) => { fx.watch(TAB); fx.dispatched = true; await hold.promise; return {}; }]
  ]);
  const parts = rig({ stubs });
  const first = parts.primitives.run("first", request("focus_tab", focus));
  const second = parts.primitives.run("second", request("set_zoom", { tab_id: TAB, zoom: 1 }));
  await new Promise((resolve) => setTimeout(resolve, 5));
  parts.primitives.committed(TAB, "https://example.com/one");
  parts.primitives.committed(99, "https://example.com/elsewhere");
  parts.primitives.committed(TAB, "https://example.com/two");
  hold.open();
  await Promise.all([first, second]);
  for (const frame of parts.frames) assert.deepEqual(frame.committed_urls, ["https://example.com/one", "https://example.com/two"], frame.correlation);
});

test("a finished primitive stops collecting, and a later one starts with no commits", async () => {
  const parts = rig({ stubs: new Map([["focus_tab", async (_args, fx) => { fx.watch(TAB); fx.dispatched = true; return {}; }]]) });
  await parts.primitives.run("first", request("focus_tab", focus));
  parts.primitives.committed(TAB, "https://example.com/late");
  await parts.primitives.run("second", request("focus_tab", focus));
  assert.deepEqual(parts.frames.map((frame) => frame.committed_urls), [[], []]);
});

test("a watch follows one tab, and failing to say so is loud", async () => {
  const parts = rig({ stubs: new Map([["focus_tab", async (_args, fx) => { fx.watch(TAB); fx.watch(TAB); fx.watch(13); return {}; }]]) });
  await parts.primitives.run("x", request("focus_tab", focus));
  assert.equal(parts.frames[0].failure, "other");
  assert.match(parts.frames[0].message, /follows one tab/);
});

// ---- cancel ---------------------------------------------------------------------------------

test("a cancel that beats the request still cancels it, once", async () => {
  let ran = 0;
  const parts = rig({ stubs: new Map([["focus_tab", async () => { ran += 1; return {}; }]]) });
  parts.primitives.cancel("chunky");
  assert.equal(parts.primitives.isCancelled("chunky"), true);
  await parts.primitives.run("chunky", request("focus_tab", focus));
  assert.equal(ran, 0);
  assert.deepEqual(wireFrame(parts.frames[0]), { kind: "receipt", correlation: "chunky", failure: "cancelled", message: "The request was cancelled.", dispatched: false, committed_urls: [] });
  assert.equal(parts.primitives.isCancelled("chunky"), false, "the cancel is spent");
  assert.equal(parts.hook.calls.length, 0, "a cancelled request takes no custody");
});

test("a cancel is remembered for a minute and then forgotten", async () => {
  let ran = 0;
  const parts = rig({ stubs: new Map([["focus_tab", async () => { ran += 1; return {}; }]]) });
  parts.primitives.cancel("old");
  parts.advance(59_999);
  assert.equal(parts.primitives.isCancelled("old"), true);
  parts.advance(1);
  assert.equal(parts.primitives.isCancelled("old"), false);
  await parts.primitives.run("old", request("focus_tab", focus));
  assert.equal(ran, 1);
});

test("a cancel that arrives while the primitive runs reaches the loops and the world calls in flight", async () => {
  const hold = gate();
  const seen = [];
  const parts = rig({ stubs: new Map([["focus_tab", async (_args, _fx, correlation) => {
    await hold.promise;
    seen.push(parts.primitives.isCancelled(correlation));
    return {};
  }]]) });
  parts.worldLog.inflight.set("busy", [{ token: "busy:0" }, { token: "busy:1" }]);
  const running = parts.primitives.run("busy", request("focus_tab", focus));
  await new Promise((resolve) => setTimeout(resolve, 5));
  parts.primitives.cancel("busy");
  hold.open();
  await running;
  assert.deepEqual(seen, [true]);
  assert.deepEqual(parts.worldLog.cancelled.map((entry) => entry.token), ["busy:0", "busy:1"]);
  assert.equal(parts.frames[0].ok !== undefined, true, "the handler decides how a cancel ends it");
  assert.equal(parts.primitives.isCancelled("busy"), false);
});

test("losing the port cancels what is in flight and drops its receipt", async () => {
  const hold = gate();
  const seen = [];
  const parts = rig({ stubs: new Map([["focus_tab", async (_args, _fx, correlation) => {
    await hold.promise;
    seen.push(parts.primitives.isCancelled(correlation));
    return {};
  }]]) });
  const running = parts.primitives.run("lost", request("focus_tab", focus));
  await new Promise((resolve) => setTimeout(resolve, 5));
  parts.primitives.disconnect();
  hold.open();
  await running;
  assert.deepEqual(seen, [true]);
  assert.deepEqual(parts.frames, [], "the port it would answer on is gone");
  await parts.primitives.run("fresh", request("focus_tab", focus));
  assert.equal(parts.frames.length, 1, "a request on the next port is answered");
});

test("a rejected chunk transfer is answered with an invalid_request receipt", () => {
  const parts = rig();
  parts.primitives.cancel("chunked");
  parts.primitives.reject("chunked", "invalid_request", "The transfer was incomplete.");
  assert.deepEqual(wireFrame(parts.frames), [{
    kind: "receipt", correlation: "chunked", failure: "invalid_request", message: "The transfer was incomplete.", dispatched: false, committed_urls: []
  }]);
  assert.equal(parts.primitives.isCancelled("chunked"), false);
});

// ---- failure codes --------------------------------------------------------------------------

test("a thrown error becomes a receipt failure through a closed table", () => {
  const code = (error) => Primitives.failureCode(error);
  for (const known of protocol.vocabulary.receipt_failures) assert.equal(code(sessions.failure(known, "x")), known);
  assert.equal(code(new Error("No tab with id: 99.")), "tab_missing");
  assert.equal(code(new Error("No tab with given id 99.")), "tab_missing");
  assert.equal(code(new Error("Cannot attach to this target.")), "debugger_unavailable");
  assert.equal(code(new Error("Cannot access a chrome:// URL")), "debugger_unavailable");
  assert.equal(code(new Error("Another debugger is already attached to the tab with id: 12.")), "debugger_unavailable");
  assert.equal(code(new Error("Debugger is not attached to the tab with id: 12.")), "session_lost");
  assert.equal(code(new Error("Detached while handling command.")), "session_lost");
  assert.equal(code(new Error("The debugger session was released.")), "session_lost");
  assert.equal(code(new Error("{\"code\":-32602,\"message\":\"Invalid parameters\"}")), "browser_refused");
  assert.equal(code(new Error("{\"code\":-32001,\"message\":\"Session with given id not found.\"}")), "session_lost");
  assert.equal(code(Object.assign(new Error("odd"), { code: -32000 })), "browser_refused");
  assert.equal(code(sessions.failure("frame_missing", "x")), "other", "a call failure is not a receipt code");
  assert.equal(code(new Error("boom")), "other");
  assert.equal(code("plain text"), "other");
  assert.equal(code(undefined), "other");
});

test("the message of a failure is bounded, and left out when empty", async () => {
  const parts = rig({ stubs: new Map([["focus_tab", async () => { throw new Error("x".repeat(900)); }], ["set_zoom", async () => { throw new Error(""); }]]) });
  await parts.primitives.run("a", request("focus_tab", focus));
  await parts.primitives.run("b", request("set_zoom", { tab_id: TAB, zoom: 1 }));
  assert.equal(parts.frames[0].message.length, 500);
  assert.equal("message" in parts.frames[1], false);
});

// ---- the receipt envelope -------------------------------------------------------------------

// The handlers are stubbed here, so this pins only the envelope: how the dispatcher turns an
// outcome, a thrown failure and the effect facts into receipt frames. The receipts the real modules
// build are pinned in golden-receipts.test.js.
const REAL = new Set(["dialog", "accessibility_snapshot"]);

// The golden request that stands for a primitive: the plain file, else the first variant.
function requestFor(name) {
  const stem = names("requests").find((file) => file === name || file.startsWith(`${name}.`));
  return golden(`requests/${stem}`);
}

test("the dispatcher frames any handler outcome and its effect facts in the golden receipt envelope", async () => {
  let checked = 0;
  for (const stem of names("receipts")) {
    const [name] = stem.split(".");
    if (REAL.has(name) || name === "unknown_primitive") continue;
    const receipt = golden(`receipts/${stem}`);
    const { primitive } = requestFor(name);
    const tabs = receipt.tab ? [{ ...ACCOUNT_TAB, id: receipt.tab.tab_id, title: receipt.tab.title, url: receipt.tab.url, active: receipt.tab.active, status: receipt.tab.readiness }] : [ACCOUNT_TAB];
    const watched = receipt.tab?.tab_id;
    const stubs = new Map([[name, async (_args, fx) => {
      if (watched !== undefined) fx.watch(watched);
      for (const url of receipt.committed_urls) parts.primitives.committed(watched, url);
      fx.dispatched = receipt.dispatched;
      if ("failure" in receipt) throw sessions.failure(receipt.failure, receipt.message);
      return receipt.ok;
    }]]);
    const parts = rig({ stubs, tabs });
    await parts.primitives.run(receipt.correlation, primitive);
    assert.deepEqual(wireFrame(parts.frames), [receipt], stem);
    checked += 1;
  }
  assert.equal(checked, 80 - 5 - 3 - 1, "every envelope except the 5 dialog, 3 snapshot and 1 unknown-primitive ones");
});

test("the unknown-primitive tolerance fixture is answered with its golden receipt", async () => {
  const entry = names("tolerance").map((stem) => golden(`tolerance/${stem}`)).find((fixture) => fixture.expect.answered);
  const parts = rig();
  await parts.primitives.run(entry.frame.correlation, entry.frame.primitive);
  assert.deepEqual(wireFrame(parts.frames), [golden(entry.expect.answered.replace(/\.json$/, ""))]);
});

// ---- real modules end to end ----------------------------------------------------------------

test("focus_tab, frames and call run through the real modules and answer their receipts", async () => {
  const parts = rig({ tabs: [{ ...ACCOUNT_TAB, active: false }] });
  await parts.world.setAssets(assets());
  await parts.primitives.run("f1", request("focus_tab", focus));
  assert.equal(parts.frames[0].ok.window_focused, true);
  assert.equal(parts.frames[0].dispatched, true);
  assert.equal(parts.frames[0].tab.tab_id, TAB);
  assert.deepEqual(parts.hook.calls, [TAB]);

  await parts.primitives.run("f2", request("frames", focus));
  assert.equal(parts.frames[1].ok.frames.length, 2);
  assert.equal(parts.frames[1].dispatched, false);

  const box = { frame: { frame_id: TOP, loader_id: TOP_LOADER }, message: { kind: "box", locator: "locator_a_1" } };
  await parts.primitives.run("f3", request("call", { tab_id: TAB, mode: "sequence", calls: [box], settle_ms: 0, timeout_ms: 2000 }));
  assert.equal(parts.frames[2].ok.answers[0].status, "returned");
  assert.equal(parts.frames[2].dispatched, true);
});

test("an attach the browser refuses is debugger_unavailable with nothing dispatched", async () => {
  const parts = rig({ attach: async () => { throw new Error("Cannot attach to this target."); } });
  await parts.primitives.run("v1", request("frames", focus));
  const [frame] = parts.frames;
  assert.equal(frame.failure, "debugger_unavailable");
  assert.equal(frame.message, "Cannot attach to this target.");
  assert.equal(frame.dispatched, false);
});

// ---- dialog ---------------------------------------------------------------------------------

test("dialog status reports the open dialog with its bounded message, matching the golden receipt", async () => {
  const parts = rig();
  parts.lifecycle.openDialog(TAB, { type: "confirm", message: "Discard changes?" });
  await parts.primitives.run("p4_001d", request("dialog", { tab_id: TAB, op: { op: "status" } }));
  assert.deepEqual(wireFrame(parts.frames), [golden("receipts/dialog.ok")]);
  parts.frames.length = 0;
  parts.lifecycle.openDialog(TAB, { type: "alert", message: "m".repeat(900) });
  await parts.primitives.run("long", request("dialog", { tab_id: TAB, op: { op: "status" } }));
  assert.equal(parts.frames[0].ok.message.length, 500);
});

test("dialog status with no dialog attaches, waits for an announcement, and answers absent", async () => {
  const parts = rig();
  await parts.primitives.run("p4_001d", request("dialog", { tab_id: TAB, op: { op: "status" } }));
  assert.deepEqual(wireFrame(parts.frames), [golden("receipts/dialog.ok_absent")]);
  assert.deepEqual(parts.slept, [50]);
  assert.equal(parts.lifecycle.attachedCount(), 0, "the lease is given back");
});

test("dialog status finds a dialog that was already open when the debugger attached", async () => {
  const parts = rig({ onSleep: () => parts.lifecycle.openDialog(TAB, { type: "prompt", message: "Name?" }) });
  await parts.primitives.run("late", request("dialog", { tab_id: TAB, op: { op: "status" } }));
  assert.deepEqual(parts.frames[0].ok, { present: true, dialog_type: "prompt", message: "Name?" });
});

test("dialog handle answers on the session that raised the dialog and closes it, matching the golden receipt", async () => {
  const parts = rig();
  parts.lifecycle.openDialog(TAB, { type: "confirm", message: "Discard changes?", sessionId: "CHILD-SESSION" });
  await parts.primitives.run("p4_001e", request("dialog", { tab_id: TAB, op: { op: "handle", accept: true, text: "yes" } }));
  assert.deepEqual(wireFrame(parts.frames), [golden("receipts/dialog.ok_handled")]);
  const sent = parts.chrome.calls.find((call) => call.method === "Page.handleJavaScriptDialog");
  assert.deepEqual(sent.target, { tabId: TAB, sessionId: "CHILD-SESSION" });
  assert.deepEqual(sent.params, { accept: true, promptText: "yes" });
  assert.equal(parts.lifecycle.currentDialog(TAB), null);
  assert.equal(parts.lifecycle.attachedCount(), 0);
});

test("dialog handle on the root session when the dialog has none, and a dismissal is not accepted", async () => {
  const parts = rig();
  parts.lifecycle.openDialog(TAB, { type: "alert", message: "" });
  await parts.primitives.run("d", request("dialog", { tab_id: TAB, op: { op: "handle", accept: false } }));
  const sent = parts.chrome.calls.find((call) => call.method === "Page.handleJavaScriptDialog");
  assert.deepEqual(sent.target, { tabId: TAB });
  assert.equal(parts.frames[0].ok.accepted, false);
  assert.equal(parts.frames[0].ok.handled, true);
});

test("dialog handle with nothing to handle answers absent and dispatches nothing, matching the golden receipt", async () => {
  const parts = rig();
  parts.chrome.handlers["Page.handleJavaScriptDialog"] = async () => { throw new Error("No dialog is showing"); };
  await parts.primitives.run("p4_001e", request("dialog", { tab_id: TAB, op: { op: "handle", accept: true } }));
  assert.deepEqual(wireFrame(parts.frames), [golden("receipts/dialog.ok_nothing_to_handle")]);
});

test("dialog handle that fails for another reason fails having dispatched", async () => {
  const parts = rig();
  parts.lifecycle.openDialog(TAB, { type: "alert", message: "" });
  parts.chrome.handlers["Page.handleJavaScriptDialog"] = async () => { throw new Error("{\"code\":-32000,\"message\":\"Cannot handle\"}"); };
  await parts.primitives.run("d", request("dialog", { tab_id: TAB, op: { op: "handle", accept: true } }));
  assert.equal(parts.frames[0].failure, "browser_refused");
  assert.equal(parts.frames[0].dispatched, true);
  assert.notEqual(parts.lifecycle.currentDialog(TAB), null, "the dialog is still there");
});

test("dialog on a page the debugger cannot attach to is debugger_unavailable, matching the golden failure", async () => {
  const parts = rig({ attach: async () => { throw new Error("Cannot attach to this target."); } });
  await parts.primitives.run("p4_001d", request("dialog", { tab_id: TAB, op: { op: "handle", accept: true } }));
  const expected = golden("receipts/dialog.debugger_unavailable");
  assert.equal(parts.frames[0].failure, expected.failure);
  assert.equal(parts.frames[0].dispatched, expected.dispatched);
});

// ---- accessibility snapshot -----------------------------------------------------------------

const top = { frame_id: TOP, loader_id: TOP_LOADER };

function axRig({ nodes, query } = {}) {
  const parts = rig();
  parts.chrome.handlers["Accessibility.getFullAXTree"] = async () => ({ nodes });
  parts.chrome.handlers["Accessibility.queryAXTree"] = async () => ({ nodes });
  const callFunctionOn = parts.chrome.handlers["Runtime.callFunctionOn"];
  parts.chrome.handlers["Runtime.callFunctionOn"] = async (target, params) => (
    params.functionDeclaration === World.SHIMS.DOC ? { result: { objectId: "document-object" } } : (callFunctionOn ?? (async () => ({ result: { value: true } })))(target, params));
  return { ...parts, query };
}

const axNode = (id, extra = {}) => ({ nodeId: id, ignored: false, role: { type: "role", value: "generic" }, name: { type: "computedString", value: "" }, ...extra });
const goldenNodes = [
  axNode("ax_1", { role: { value: "RootWebArea" }, name: { value: "Account" } }),
  axNode("ax_2", {
    parentId: "ax_1",
    role: { value: "button" },
    name: { value: "Save" },
    value: { type: "string", value: "" },
    properties: [
      { name: "focused", value: { type: "booleanOrUndefined", value: true } },
      { name: "disabled", value: { type: "boolean", value: false } },
      { name: "busy", value: { type: "boolean", value: false } },
      { name: "describedby", value: { type: "idrefList", value: "x" } }
    ]
  })
];

test("an accessibility snapshot of a frame reports bounded nodes with only the properties the service reads, matching the golden receipt", async () => {
  const parts = axRig({ nodes: [...goldenNodes, axNode("ax_3", { ignored: true })] });
  await parts.primitives.run("p4_0028", request("accessibility_snapshot", { tab_id: TAB, frame: top, max_nodes: 2000, max_depth: 6 }));
  const expected = golden("receipts/accessibility_snapshot.ok");
  assert.deepEqual(wireFrame(parts.frames), [expected]);
  const sent = parts.chrome.calls.find((call) => call.method === "Accessibility.getFullAXTree");
  assert.deepEqual(sent.params, { frameId: TOP, depth: 6 });
  assert.equal(parts.chrome.calls.some((call) => call.method === "Accessibility.enable"), false, "nothing enables the domain");
  assert.equal(parts.lifecycle.attachedCount(), 0);
});

test("a snapshot with a query asks for the matching nodes under the frame's document", async () => {
  const parts = axRig({ nodes: goldenNodes });
  await parts.world.setAssets(assets());
  await parts.primitives.run("q", request("accessibility_snapshot", { tab_id: TAB, frame: top, max_nodes: 10, query: { role: "button", name: "Save" } }));
  const sent = parts.chrome.calls.find((call) => call.method === "Accessibility.queryAXTree");
  assert.deepEqual(sent.params, { objectId: "document-object", role: "button", accessibleName: "Save" });
  assert.equal(parts.frames[0].ok.nodes.length, 2);
});

test("a snapshot is cut at max_nodes and says so; text is bounded", async () => {
  const many = Array.from({ length: 6 }, (_unused, index) => axNode(`n${index}`, { role: { value: "r".repeat(300) }, name: { value: "n".repeat(900) } }));
  const parts = axRig({ nodes: many });
  await parts.primitives.run("cut", request("accessibility_snapshot", { tab_id: TAB, frame: top, max_nodes: 4 }));
  const { ok } = parts.frames[0];
  assert.equal(ok.nodes.length, 4);
  assert.equal(ok.truncated, true);
  assert.equal(ok.nodes[0].role.length, 100);
  assert.equal(ok.nodes[0].name.length, 500);
  const exact = axRig({ nodes: many.slice(0, 4) });
  await exact.primitives.run("fit", request("accessibility_snapshot", { tab_id: TAB, frame: top, max_nodes: 4 }));
  assert.equal(exact.frames[0].ok.truncated, false);
});

test("a snapshot of a frame that changed document, or of a frame that is gone, is document_changed, matching the golden failure", async () => {
  const parts = axRig({ nodes: goldenNodes });
  await parts.primitives.run("p4_0028", request("accessibility_snapshot", { tab_id: TAB, frame: { frame_id: TOP, loader_id: "another" }, max_nodes: 10 }));
  assert.equal(parts.frames[0].failure, golden("receipts/accessibility_snapshot.document_changed").failure);
  assert.equal(parts.frames[0].dispatched, false);
  await parts.primitives.run("gone", request("accessibility_snapshot", { tab_id: TAB, frame: { frame_id: "NOPE", loader_id: "x" }, max_nodes: 10 }));
  assert.equal(parts.frames[1].failure, "document_changed");
  assert.equal(parts.chrome.calls.some((call) => call.method === "Accessibility.getFullAXTree"), false);
});

test("a frame that navigates while its tree is read is document_changed", async () => {
  const parts = axRig({ nodes: goldenNodes });
  parts.chrome.handlers["Accessibility.getFullAXTree"] = async () => {
    parts.chrome.trees.root.frame.loaderId = "L-next";
    return { nodes: goldenNodes };
  };
  await parts.primitives.run("moving", request("accessibility_snapshot", { tab_id: TAB, frame: top, max_nodes: 10 }));
  assert.equal(parts.frames[0].failure, "document_changed");
});

test("a snapshot checks its guard and reads a frame of another process in its own session", async () => {
  const parts = axRig({ nodes: goldenNodes });
  const stale = { documents: [{ frame_id: TOP, loader_id: "another", url: URL }], exact: false };
  await parts.primitives.run("g", request("accessibility_snapshot", { tab_id: TAB, frame: top, max_nodes: 10, guard: stale }));
  assert.equal(parts.frames[0].failure, "document_changed");

  const split = axRig({ nodes: goldenNodes });
  split.chrome.trees.OOP = { frame: { id: "REMOTE", loaderId: "L-remote", url: "https://widgets.example.net/", parentId: TOP }, childFrames: [] };
  await split.lifecycle.retain(TAB);
  split.chrome.emit({ tabId: TAB }, "Target.attachedToTarget", { sessionId: "OOP", targetInfo: { type: "iframe", targetId: "REMOTE" }, waitingForDebugger: false });
  await split.primitives.run("remote", request("accessibility_snapshot", { tab_id: TAB, frame: { frame_id: "REMOTE", loader_id: "L-remote" }, max_nodes: 10 }));
  const sent = split.chrome.calls.find((call) => call.method === "Accessibility.getFullAXTree");
  assert.equal(sent.target.sessionId, "OOP");
  assert.equal(split.frames[0].ok.nodes.length, 2);
});

// ---- cancellations --------------------------------------------------------------------------

test("the cancel registry is a plain remembered set with a sixty second memory", () => {
  let now = 1000;
  const registry = Primitives.createCancellations({ now: () => now });
  assert.equal(registry.isCancelled("a"), false);
  registry.cancel("a");
  registry.cancel("b");
  now += 30_000;
  registry.cancel("c");
  now += 30_000;
  assert.deepEqual(["a", "b", "c"].map((id) => registry.isCancelled(id)), [false, false, true]);
  registry.forget("c");
  assert.equal(registry.isCancelled("c"), false);
  assert.ok(Object.isFrozen(registry));
});
