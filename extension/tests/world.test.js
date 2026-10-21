"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const World = require("../lib/world.js");
const protocol = require("../lib/protocol.js");
const {
  TOP, TOP_LOADER, CHILD, CHILD_LOADER, node, fakeChrome, assets, wire, effects, methods, worldCalls
} = require("./p4-fakes.js");

const TAB = 12;
const top = { frame_id: TOP, loader_id: TOP_LOADER };
const child = { frame_id: CHILD, loader_id: CHILD_LOADER };

async function ready(options = {}) {
  const chrome = fakeChrome(options.chrome);
  const parts = wire(chrome, options);
  assert.equal(await parts.world.setAssets(assets()), true);
  return { chrome, ...parts };
}

function callArgs(calls, extra = {}) {
  return { tab_id: TAB, mode: "sequence", calls, settle_ms: 0, timeout_ms: 2000, ...extra };
}

const box = { frame: top, message: { kind: "box", locator: "locator_a_1" } };
const activate = { frame: top, message: { kind: "activate", locator: "locator_a_2" } };

test("registration removes superseded identifiers, then registers in every session with the id-wrapped source", async () => {
  const chrome = fakeChrome();
  const { lifecycle, world } = wire(chrome);
  const first = assets();
  assert.equal(await world.setAssets(first), true);
  await lifecycle.retain(TAB);
  chrome.emit({ tabId: TAB }, "Target.attachedToTarget",
    { sessionId: "s1", targetInfo: { type: "iframe", targetId: "OOP" }, waitingForDebugger: false });
  await lifecycle.acquire(TAB);
  await lifecycle.release(TAB);
  const adds = chrome.calls.filter((call) => call.method === "Page.addScriptToEvaluateOnNewDocument");
  assert.deepEqual(adds.map((call) => call.target), [{ tabId: TAB }, { tabId: TAB, sessionId: "s1" }]);
  for (const call of adds) {
    assert.equal(call.params.worldName, protocol.WORLD_NAME);
    assert.equal(call.params.runImmediately, true);
    assert.equal(call.params.source, `(${World.wrap(first.source)})(${JSON.stringify(first.id)});`);
  }

  chrome.calls.length = 0;
  const second = assets("\"use strict\";\n// next");
  assert.equal(await world.setAssets(second), true);
  assert.deepEqual(chrome.calls.map((call) => [call.method, call.target.sessionId ?? "root", call.params.identifier ?? null]), [
    ["Page.removeScriptToEvaluateOnNewDocument", "root", "script-1"],
    ["Page.addScriptToEvaluateOnNewDocument", "root", null],
    ["Page.removeScriptToEvaluateOnNewDocument", "s1", "script-2"],
    ["Page.addScriptToEvaluateOnNewDocument", "s1", null]
  ]);
  assert.match(chrome.calls[1].params.source, /\/\/ next/);

  chrome.calls.length = 0;
  assert.equal(await world.setAssets(second), true);
  assert.deepEqual(chrome.calls, []);
  await lifecycle.detachAll();
});

test("a checksum mismatch is refused and keeps the previous assets", async () => {
  const chrome = fakeChrome();
  const { world } = wire(chrome);
  const good = assets();
  assert.equal(await world.setAssets({ id: "0".repeat(64), source: good.source }), false);
  assert.equal(await world.setAssets({ id: "not hex", source: good.source }), false);
  assert.equal(await world.setAssets({ id: good.id }), false);
  assert.equal(world.assetsId(), null);
  assert.equal(await world.setAssets(good), true);
  assert.equal(await world.setAssets({ id: good.id.replace(/^./, (c) => (c === "a" ? "b" : "a")), source: good.source }), false);
  assert.equal(world.assetsId(), good.id);
});

test("handles verify the loader and cache an objectId; calls never carry an executionContextId", async () => {
  const { chrome, world } = await ready();
  const fx = effects();
  const result = await world.callPrimitive(callArgs([box, activate]), fx, "p4_1");
  assert.deepEqual(result.answers.map((answer) => answer.status), ["returned", "returned"]);
  assert.equal(result.answers[0].envelope.value.on, `global:${TOP}`);
  assert.equal(fx.dispatched, true);
  assert.deepEqual(fx.watched, [TAB]);
  assert.equal(methods(chrome, "Page.createIsolatedWorld").length, 1);
  assert.equal(methods(chrome, "Runtime.evaluate").length, 1);
  const evaluate = chrome.calls.find((call) => call.method === "Runtime.evaluate");
  assert.equal(evaluate.params.expression, "globalThis");
  assert.equal(evaluate.params.objectGroup, protocol.WORLD_NAME);
  const createIndex = chrome.calls.findIndex((call) => call.method === "Page.createIsolatedWorld");
  const firstCall = chrome.calls.findIndex((call) => call.method === "Runtime.callFunctionOn");
  assert.ok(chrome.calls.slice(createIndex, firstCall).some((call) => call.method === "Page.getFrameTree"), "loader re-check");
  for (const call of chrome.calls.filter((entry) => entry.method === "Runtime.callFunctionOn")) {
    assert.equal("executionContextId" in call.params, false);
    assert.ok(call.params.objectId);
  }
  assert.equal(methods(chrome, "Runtime.enable").length, 0);
});

test("a loader that changes while the world is created fails the call before it starts", async () => {
  const { chrome, world } = await ready();
  let reads = 0;
  chrome.handlers["Page.getFrameTree"] = () => {
    reads += 1;
    return { frameTree: reads <= 1 ? chrome.trees.root : node(TOP, "L-new", "https://example.com/account") };
  };
  const result = await world.callPrimitive(callArgs([box]), effects(), "p4_2");
  assert.deepEqual(result.answers[0], { status: "failed", code: "document_changed",
    message: "The frame changed document while its world was created.", started: false });
  assert.equal(worldCalls(chrome).length, 0);
});

test("runtime_missing triggers exactly one lazy install and one retry", async () => {
  const { chrome, world } = await ready();
  const current = assets();
  let replies = 0;
  chrome.handlers["Runtime.callFunctionOn"] = (target, params) => {
    if (params.functionDeclaration === World.SHIMS.CALL) {
      replies += 1;
      if (replies === 1) return { result: { value: { ok: false, error: { code: "runtime_missing", message: "x" } } } };
      return { result: { value: { ok: true, value: 1 } } };
    }
    return { result: { value: true } };
  };
  const result = await world.callPrimitive(callArgs([box]), effects(), "p4_3");
  assert.deepEqual(result.answers, [{ status: "returned", envelope: { ok: true, value: 1 } }]);
  const installs = chrome.calls.filter((call) => call.method === "Runtime.callFunctionOn" && call.params.functionDeclaration === World.wrap(current.source));
  assert.equal(installs.length, 1);
  assert.deepEqual(installs[0].params.arguments, [{ value: current.id }]);
  assert.equal(worldCalls(chrome).length, 2);

  replies = 0;
  chrome.handlers["Runtime.callFunctionOn"] = (target, params) => (params.functionDeclaration === World.SHIMS.CALL
    ? { result: { value: { ok: false, error: { code: "runtime_missing", message: "x" } } } }
    : { result: { value: true } });
  chrome.calls.length = 0;
  const again = await world.callPrimitive(callArgs([box]), effects(), "p4_4");
  assert.equal(again.answers[0].envelope.error.code, "runtime_missing");
  assert.equal(worldCalls(chrome).length, 2);
});

test("exceptionDetails maps to page_exception, started", async () => {
  const { chrome, world } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = () => ({ exceptionDetails: { exception: { description: "TypeError: x is not a function" } } });
  const fx = effects();
  const result = await world.callPrimitive(callArgs([box]), fx, "p4_5");
  assert.deepEqual(result.answers[0], { status: "failed", code: "page_exception", message: "TypeError: x is not a function", started: true });
  assert.equal(fx.dispatched, true);
});

test("Cannot find context is document_changed, not started, and drops the cached handle", async () => {
  const { chrome, lifecycle, world } = await ready();
  await lifecycle.retain(TAB);
  await world.callPrimitive(callArgs([box]), effects(), "p4_6a");
  chrome.handlers["Runtime.callFunctionOn"] = () => { throw new Error("Cannot find context with specified id"); };
  const fx = effects();
  const result = await world.callPrimitive(callArgs([box]), fx, "p4_6");
  assert.deepEqual(result.answers[0], { status: "failed", code: "document_changed", message: "Cannot find context with specified id", started: false });
  assert.equal(fx.dispatched, false);
  delete chrome.handlers["Runtime.callFunctionOn"];
  await world.callPrimitive(callArgs([box]), effects(), "p4_7");
  await world.callPrimitive(callArgs([box]), effects(), "p4_7a");
  assert.equal(methods(chrome, "Page.createIsolatedWorld").length, 2);
  await lifecycle.detachAll();
});

test("a context destroyed mid-await is document_changed, started", async () => {
  const { chrome, world } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = async () => { throw new Error("Execution context was destroyed."); };
  const fx = effects();
  const result = await world.callPrimitive(callArgs([box]), fx, "p4_8");
  assert.equal(result.answers[0].code, "document_changed");
  assert.equal(result.answers[0].started, true);
  assert.equal(fx.dispatched, true);
});

test("mapCallError covers the closed table", () => {
  assert.deepEqual(World.mapCallError(new Error("Could not find object with given id")), { code: "document_changed", started: false, dropHandle: true });
  assert.deepEqual(World.mapCallError(new Error("Inspected target navigated or closed")), { code: "document_changed", started: true, dropHandle: true });
  assert.equal(World.mapCallError(new Error("Debugger is not attached to the tab with id: 12.")).code, "session_lost");
  assert.equal(World.mapCallError(new Error("Object couldn't be returned by value")).code, "unserializable");
  assert.deepEqual(World.mapCallError(new Error("Something odd")), { code: "other", started: true, dropHandle: false });
  for (const text of ["Session with given id not found.", "debugger session was released"]) {
    assert.deepEqual(World.mapCallError(new Error(text)), { code: "session_lost", started: true, dropHandle: true }, text);
  }
  for (const code of ["document_changed", "session_lost", "unserializable", "other"]) assert.ok(protocol.isKnown("call_failures", code));
});

test("the deadline sends CANCEL, then answers timeout", async () => {
  const { chrome, world } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = (target, params) => (params.functionDeclaration === World.SHIMS.CALL
    ? new Promise(() => {})
    : { result: { value: true } });
  const result = await world.callPrimitive(callArgs([box], { timeout_ms: 30 }), effects(), "p4_9");
  assert.equal(result.answers[0].code, "timeout");
  assert.equal(result.answers[0].started, true);
  const cancel = chrome.calls.find((call) => call.params.functionDeclaration === World.SHIMS.CANCEL);
  assert.deepEqual(cancel.params.arguments, [{ value: "p4_9:0" }]);
  assert.equal(cancel.params.objectId, `global:${TOP}`);
});

test("a dialog that opens during a call answers dialog_open, started; a known dialog refuses the whole call before any renderer work", async () => {
  const { chrome, lifecycle, world } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = (target, params) => {
    if (params.functionDeclaration === World.SHIMS.CALL) {
      setTimeout(() => lifecycle.openDialog(TAB, { type: "confirm" }), 5);
      return new Promise(() => {});
    }
    return { result: { value: true } };
  };
  const result = await world.callPrimitive(callArgs([box]), effects(), "p4_10");
  assert.equal(result.answers[0].code, "dialog_open");
  assert.equal(result.answers[0].started, true);

  await lifecycle.retain(TAB);
  lifecycle.openDialog(TAB, { type: "alert" });
  chrome.calls.length = 0;
  await assert.rejects(world.callPrimitive(callArgs([box]), effects(), "p4_11"),
    { code: "dialog_open", message: "A JavaScript dialog blocks the page." });
  assert.equal(chrome.calls.length, 0, "nothing is sent to a renderer a dialog blocks");
  await lifecycle.detachAll();
});

test("a dialog that opens while a world is set up answers dialog_open, not started", async () => {
  const { chrome, lifecycle, world } = await ready();
  chrome.handlers["Runtime.evaluate"] = () => {
    lifecycle.openDialog(TAB, { type: "alert" });
    return { result: { objectId: `global:${TOP}` } };
  };
  const result = await world.callPrimitive(callArgs([box]), effects(), "p4_12");
  assert.equal(result.answers[0].code, "dialog_open");
  assert.equal(result.answers[0].started, false);
  assert.equal(chrome.calls.filter((call) => call.params.functionDeclaration === World.SHIMS.CALL).length, 0,
    "the message never reached the page");
  await lifecycle.detachAll();
});

test("mapCallError keeps the call code of a coded failure, and reads any other code as text", () => {
  const coded = (code) => Object.assign(new Error("coded"), { code });
  assert.deepEqual(World.mapCallError(coded("dialog_open")), { code: "dialog_open", started: true, dropHandle: false });
  assert.deepEqual(World.mapCallError(coded("session_lost")), { code: "session_lost", started: true, dropHandle: false });
  assert.deepEqual(World.mapCallError(coded("tab_missing")), { code: "other", started: true, dropHandle: false });
  assert.deepEqual(World.mapCallError(coded(-32000)), { code: "other", started: true, dropHandle: false });
});

test("ambient facts are not sent to the worlds of a tab a dialog blocks", async () => {
  const { chrome, lifecycle, world } = await ready();
  await lifecycle.retain(TAB);
  lifecycle.openDialog(TAB, { type: "alert", url: "https://example.com/" });
  chrome.calls.length = 0;
  assert.equal(await world.ambientAll(TAB), false);
  assert.equal(chrome.calls.length, 0, "nothing is sent to a renderer a dialog blocks");
  await lifecycle.detachAll();
});

test("race cancels pending losers and reports them skipped", async () => {
  const { chrome, world } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = (target, params) => {
    if (params.functionDeclaration !== World.SHIMS.CALL) return { result: { value: true } };
    if (params.objectId === `global:${TOP}`) return new Promise(() => {});
    return new Promise((resolve) => setTimeout(() => resolve({ result: { value: { ok: true, value: "child" } } }), 5));
  };
  const calls = [
    { frame: top, message: { kind: "observe", until_satisfied: true } },
    { frame: child, message: { kind: "observe", until_satisfied: true } }
  ];
  const fx = effects();
  const result = await world.callPrimitive(callArgs(calls, { mode: "race" }), fx, "p4_12");
  assert.deepEqual(result.answers, [{ status: "skipped" }, { status: "returned", envelope: { ok: true, value: "child" } }]);
  const cancels = chrome.calls.filter((call) => call.params.functionDeclaration === World.SHIMS.CANCEL);
  assert.deepEqual(cancels.map((call) => call.params.arguments[0].value), ["p4_12:0"]);
  assert.equal(fx.dispatched, true);
});

test("a renderer that answers neither CALL nor CANCEL still times out within the deadline", async () => {
  const { chrome, world } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = (target, params) => (params.functionDeclaration === World.SHIMS.CALL
    || params.functionDeclaration === World.SHIMS.CANCEL
    ? new Promise(() => {})
    : { result: { value: true } });
  const started = Date.now();
  const result = await world.callPrimitive(callArgs([box], { timeout_ms: 50 }), effects(), "p4_hung");
  assert.ok(Date.now() - started < 500);
  assert.equal(result.answers[0].code, "timeout");
  assert.equal(result.answers[0].started, true);
  assert.ok(chrome.calls.some((call) => call.params.functionDeclaration === World.SHIMS.CANCEL));
});

test("race returns the winner while a hung loser never answers CANCEL", async () => {
  const { chrome, world } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = (target, params) => {
    if (params.functionDeclaration === World.SHIMS.CANCEL) return new Promise(() => {});
    if (params.functionDeclaration !== World.SHIMS.CALL) return { result: { value: true } };
    if (params.objectId === `global:${TOP}`) return new Promise(() => {});
    return new Promise((resolve) => setTimeout(() => resolve({ result: { value: { ok: true, value: "child" } } }), 5));
  };
  const calls = [
    { frame: top, message: { kind: "observe", until_satisfied: true } },
    { frame: child, message: { kind: "observe", until_satisfied: true } }
  ];
  const started = Date.now();
  const result = await world.callPrimitive(callArgs(calls, { mode: "race" }), effects(), "p4_hung_race");
  assert.ok(Date.now() - started < 500);
  assert.deepEqual(result.answers, [{ status: "skipped" }, { status: "returned", envelope: { ok: true, value: "child" } }]);
});

test("sequence skips every call after the first answer that is not ok", async () => {
  const { chrome, world } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = () => ({ result: { value: { ok: false, error: { code: "stale_locator", message: "stale" } } } });
  const result = await world.callPrimitive(callArgs([box, activate, box]), effects(), "p4_13");
  assert.deepEqual(result.answers.map((answer) => answer.status), ["returned", "skipped", "skipped"]);
  assert.equal(worldCalls(chrome).length, 1);
});

test("parallel answers every call and a cancelled primitive throws cancelled with true facts", async () => {
  let cancelled = false;
  const { world } = await ready({ isCancelled: () => cancelled });
  const fx = effects();
  const result = await world.callPrimitive(callArgs([box, { frame: child, message: { kind: "box" } }], { mode: "parallel" }), fx, "p4_14");
  assert.deepEqual(result.answers.map((answer) => answer.envelope.value.on), [`global:${TOP}`, `global:${CHILD}`]);

  cancelled = true;
  const cancelledFx = effects();
  await assert.rejects(world.callPrimitive(callArgs([box]), cancelledFx, "p4_15"), { code: "cancelled" });
  assert.equal(cancelledFx.dispatched, false);
});

test("owner_of uses getFrameOwner and resolveNode in the parent session, re-checks the loader and releases the owner", async () => {
  const { chrome, world } = await ready();
  const result = await world.callPrimitive(callArgs([{ frame: top, owner_of: CHILD, message: { kind: "owner_box" } }]), effects(), "p4_16");
  assert.equal(result.answers[0].envelope.value.on, "owner:77");
  const owner = chrome.calls.find((call) => call.method === "DOM.getFrameOwner");
  assert.deepEqual(owner.params, { frameId: CHILD });
  const resolve = chrome.calls.findIndex((call) => call.method === "DOM.resolveNode");
  assert.equal(chrome.calls[resolve].params.objectGroup, protocol.WORLD_NAME);
  assert.ok(Number.isInteger(chrome.calls[resolve].params.executionContextId));
  assert.equal(chrome.calls[resolve + 1].method, "Page.getFrameTree");
  const shim = worldCalls(chrome)[0];
  assert.equal(shim.params.objectId, "owner:77");
  assert.equal(shim.params.arguments[5].value, true);
  assert.ok(chrome.calls.some((call) => call.method === "Runtime.releaseObject" && call.params.objectId === "owner:77"));

  chrome.handlers["DOM.getFrameOwner"] = () => { throw new Error("Frame with the given id was not found."); };
  const missing = await world.callPrimitive(callArgs([{ frame: top, owner_of: "GONE", message: { kind: "owner_box" } }]), effects(), "p4_17");
  assert.equal(missing.answers[0].code, "owner_missing");
  assert.equal(missing.answers[0].started, false);
});

test("an unknown frame is frame_missing and a replaced loader is document_changed", async () => {
  const { world } = await ready();
  const result = await world.callPrimitive(callArgs([
    { frame: { frame_id: "NOPE", loader_id: "x" }, message: { kind: "box" } }
  ], { mode: "parallel" }), effects(), "p4_18");
  assert.equal(result.answers[0].code, "frame_missing");
  const changed = await world.callPrimitive(callArgs([{ frame: { frame_id: TOP, loader_id: "old" }, message: { kind: "box" } }]), effects(), "p4_19");
  assert.equal(changed.answers[0].code, "document_changed");
  assert.equal(changed.answers[0].started, false);
});

test("a call carries the local facts, the token and the attachment", async () => {
  const { chrome, world } = await ready({
    local: { preferences: () => ({ effects: false, captions: true }), managed: () => true, recording: () => false, runtime: () => "active" }
  });
  await world.withHidden(TAB, () => world.callPrimitive(callArgs([box]), effects(), "p4_20"));
  const args = worldCalls(chrome)[0].params.arguments.map((entry) => entry.value);
  assert.equal(args[0], assets().id);
  assert.deepEqual(args[1], box.message);
  assert.deepEqual(args[2], { preferences: { effects: false, captions: true }, managed: true, recording: false, runtime: "active", hidden: true });
  assert.equal(args[3], "p4_20:0");
  assert.equal(args[4], null);
  assert.equal(args[5], false);
  assert.equal(world.localFacts(TAB).hidden, false, "the hold ends with the work");
});

test("withHidden shows the presentation again when its work throws", async () => {
  const { world } = await ready();
  await assert.rejects(world.withHidden(TAB, async () => {
    assert.equal(world.localFacts(TAB).hidden, true);
    throw new Error("capture failed");
  }), /capture failed/);
  assert.equal(world.localFacts(TAB).hidden, false);
});

test("present and ambient never attach and never install", async () => {
  const { chrome, lifecycle, world } = await ready();
  assert.equal(await world.present(TAB, null, { signal: "progress" }), false);
  assert.equal(await world.ambientAll(TAB), false);
  assert.deepEqual(chrome.calls, []);

  await lifecycle.retain(TAB);
  chrome.calls.length = 0;
  assert.equal(await world.present(TAB, null, { signal: "progress" }), true);
  const present = chrome.calls.find((call) => call.params.functionDeclaration === World.SHIMS.PRESENT);
  assert.equal(present.params.arguments[0].value, assets().id);
  assert.equal(await world.ambientAll(TAB, { runtime: "disconnected" }), true);
  const ambient = chrome.calls.find((call) => call.params.functionDeclaration === World.SHIMS.AMBIENT);
  assert.equal(ambient.params.arguments[1].value.runtime, "disconnected");

  chrome.handlers["Runtime.callFunctionOn"] = () => ({ result: { value: false } });
  assert.equal(await world.present(TAB, null, { signal: "progress" }), false);
  assert.equal(chrome.calls.some((call) => call.params.functionDeclaration === World.wrap(assets().source)), false);
  await lifecycle.detachAll();
});

test("dispose releases the cached worlds, the object group and the registrations before detach", async () => {
  const { chrome, lifecycle, world } = await ready();
  await lifecycle.retain(TAB);
  await world.callPrimitive(callArgs([box]), effects(), "p4_21");
  chrome.calls.length = 0;
  await lifecycle.unretain(TAB);
  const sequence = chrome.calls.map((call) => call.method === "Runtime.callFunctionOn" ? call.params.functionDeclaration : call.method);
  assert.deepEqual(sequence, [World.SHIMS.DISPOSE, "Runtime.releaseObjectGroup", "Page.removeScriptToEvaluateOnNewDocument"]);
  assert.equal(chrome.calls[1].params.objectGroup, protocol.WORLD_NAME);
});

test("devicePixelRatio is read in the top isolated world and documentObject keeps a remote object", async () => {
  const { chrome, lifecycle, world, inventory } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = (target, params) => (params.functionDeclaration === World.SHIMS.DPR
    ? { result: { value: 2 } }
    : { result: { objectId: "document:1" } });
  await lifecycle.retain(TAB);
  assert.equal(await world.devicePixelRatio(TAB), 2);
  const snap = await inventory.snapshot(TAB);
  assert.equal(await world.documentObject(TAB, top, snap), "document:1");
  const doc = chrome.calls.find((call) => call.params.functionDeclaration === World.SHIMS.DOC);
  assert.equal(doc.params.returnByValue, false);
  await lifecycle.detachAll();
});

test("a child session that ends drops its handles", async () => {
  const chrome = fakeChrome({ trees: { root: node(TOP, TOP_LOADER, "https://example.com/"), s9: node("OOP", "L-oop", "https://other.example/", TOP) } });
  const { lifecycle, world } = wire(chrome);
  await world.setAssets(assets());
  await lifecycle.retain(TAB);
  chrome.emit({ tabId: TAB }, "Target.attachedToTarget", { sessionId: "s9", targetInfo: { type: "iframe", targetId: "OOP" } });
  const oop = { frame: { frame_id: "OOP", loader_id: "L-oop" }, message: { kind: "box" } };
  await world.callPrimitive(callArgs([oop]), effects(), "p4_22");
  const created = chrome.calls.filter((call) => call.method === "Page.createIsolatedWorld");
  assert.deepEqual(created.at(-1).target, { tabId: TAB, sessionId: "s9" });
  chrome.emit({ tabId: TAB }, "Target.detachedFromTarget", { sessionId: "s9" });
  chrome.emit({ tabId: TAB }, "Target.attachedToTarget", { sessionId: "s9", targetInfo: { type: "iframe", targetId: "OOP" } });
  await world.callPrimitive(callArgs([oop]), effects(), "p4_23");
  assert.equal(chrome.calls.filter((call) => call.method === "Page.createIsolatedWorld").length, created.length + 1);
  await lifecycle.detachAll();
});

test("Runtime.enable is never sent by world calls", async () => {
  const { chrome, world } = await ready();
  await world.callPrimitive(callArgs([box, { frame: child, owner_of: undefined, message: { kind: "box" } }], { mode: "parallel" }), effects(), "p4_24");
  await world.present(TAB, null, {});
  assert.equal(methods(chrome).includes("Runtime.enable"), false);
});
