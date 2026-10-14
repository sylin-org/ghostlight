"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const World = require("../lib/world.js");
const { TOP, TOP_LOADER, fakeChrome, assets, wire, effects, methods, worldCalls } = require("./p4-fakes.js");

const TAB = 12;
const top = { frame_id: TOP, loader_id: TOP_LOADER };
const guard = { exact: false, unseen: 0, documents: [{ frame_id: TOP, loader_id: TOP_LOADER, url: "https://example.com/account" }] };

async function ready(options = {}) {
  const chrome = fakeChrome();
  const parts = wire(chrome, options);
  await parts.world.setAssets(assets());
  return { chrome, ...parts };
}

function inputArgs(steps, extra = {}) {
  return { tab_id: TAB, steps, guard, settle_ms: 0, timeout_ms: 2000, ...extra };
}

const click = [
  { step: "mouse", action: "moved", x: 10, y: 20 },
  { step: "mouse", action: "pressed", x: 10, y: 20, button: "left", buttons: 1, click_count: 1 },
  { step: "mouse", action: "released", x: 10, y: 20, button: "left", click_count: 1 }
];

function inputEvents(chrome) {
  return chrome.calls.filter((call) => call.method.startsWith("Input."));
}

test("mouse, key, insert_text and pause steps reach the root session in order", async () => {
  const { chrome, input } = await ready();
  const fx = effects();
  const result = await input.run(inputArgs([
    ...click,
    { step: "key", action: "down", key: "Enter", code: "Enter", key_code: 13, text: "\r", modifiers: 0 },
    { step: "insert_text", text: "Ada Lovelace" },
    { step: "pause", ms: 5 }
  ]), fx, "p4_1");
  assert.deepEqual(result, { completed_steps: 6, native_drag: false });
  assert.equal(fx.dispatched, true);
  assert.deepEqual(fx.watched, [TAB]);
  const events = inputEvents(chrome);
  assert.deepEqual(events.map((call) => call.params.type ?? call.method), ["mouseMoved", "mousePressed", "mouseReleased", "keyDown", "Input.insertText"]);
  assert.deepEqual(events[1].params, { type: "mousePressed", x: 10, y: 20, button: "left", buttons: 1, clickCount: 1, modifiers: 0, deltaX: 0, deltaY: 0, force: 0 });
  assert.equal(events[3].params.windowsVirtualKeyCode, 13);
  assert.equal(events[3].params.text, "\r");
  assert.ok(events.every((call) => call.target.sessionId === undefined));
  assert.equal(methods(chrome).includes("Runtime.enable"), false);
});

test("the guard runs before any input", async () => {
  const { chrome, input } = await ready();
  const fx = effects();
  const stale = { ...guard, documents: [{ ...guard.documents[0], loader_id: "old" }] };
  await assert.rejects(input.run(inputArgs(click, { guard: stale }), fx, "p4_2"), { code: "document_changed" });
  assert.equal(inputEvents(chrome).length, 0);
  assert.equal(fx.dispatched, false);
});

test("a non-ok precondition sends no input", async () => {
  const { chrome, input } = await ready();
  chrome.handlers["Runtime.callFunctionOn"] = () => ({ result: { value: { ok: false, error: { code: "focus_lost", message: "focus moved before typing" } } } });
  const fx = effects();
  const result = await input.run(inputArgs([{ step: "insert_text", text: "x" }], {
    precondition: { frame: top, message: { kind: "require_focused" } }
  }), fx, "p4_3");
  assert.deepEqual(result, {
    completed_steps: 0,
    native_drag: false,
    precondition: { status: "returned", envelope: { ok: false, error: { code: "focus_lost", message: "focus moved before typing" } } }
  });
  assert.equal(inputEvents(chrome).length, 0);
  assert.equal(fx.dispatched, true);
});

test("the focus precondition runs right before key input and the guard is re-checked after it", async () => {
  const { chrome, input } = await ready();
  const result = await input.run(inputArgs([{ step: "insert_text", text: "x" }], {
    precondition: { frame: top, message: { kind: "require_focused" } }
  }), effects(), "p4_4");
  assert.equal(result.precondition.status, "returned");
  assert.equal(result.completed_steps, 1);
  const shim = chrome.calls.findIndex((call) => call.params.functionDeclaration === World.SHIMS.CALL);
  const insert = chrome.calls.findIndex((call) => call.method === "Input.insertText");
  assert.ok(shim < insert);
  assert.ok(chrome.calls.slice(shim, insert).some((call) => call.method === "Page.getFrameTree"));
  assert.equal(worldCalls(chrome)[0].params.arguments[3].value, "p4_4:p");
});

test("the input batch stops at a dialog and reports the completed steps", async () => {
  const { chrome, world, input } = await ready();
  chrome.handlers["Input.dispatchMouseEvent"] = (target, params) => {
    if (params.type !== "mouseReleased") return {};
    setTimeout(() => world.announceDialog(TAB, "alert"), 1);
    return new Promise(() => {});
  };
  const result = await input.run(inputArgs([...click, { step: "insert_text", text: "never" }]), effects(), "p4_5");
  assert.deepEqual(result, { completed_steps: 2, native_drag: false, stopped: { reason: "dialog", dialog_type: "alert" } });
  assert.equal(methods(chrome, "Input.insertText").length, 0);
});

test("a dialog already open refuses the batch before any input", async () => {
  const { chrome, lifecycle, input } = await ready();
  await lifecycle.retain(TAB);
  lifecycle.openDialog(TAB, { type: "alert" });
  await assert.rejects(input.run(inputArgs(click), effects(), "p4_6"), { code: "dialog_open" });
  assert.equal(inputEvents(chrome).length, 0);
  await lifecycle.detachAll();
});

test("a cancel stops the batch between steps", async () => {
  let cancelled = false;
  const { chrome, input } = await ready({ isCancelled: () => cancelled });
  chrome.handlers["Input.dispatchMouseEvent"] = () => { cancelled = true; return {}; };
  const result = await input.run(inputArgs(click), effects(), "p4_7");
  assert.deepEqual(result, { completed_steps: 1, native_drag: false, stopped: { reason: "cancelled" } });
});

test("the deadline between steps is a timeout", async () => {
  const { chrome, input } = await ready();
  chrome.handlers["Input.dispatchMouseEvent"] = () => new Promise((resolve) => setTimeout(() => resolve({}), 30));
  const fx = effects();
  await assert.rejects(input.run(inputArgs(click, { timeout_ms: 10 }), fx, "p4_8"), { code: "timeout", message: "Input did not finish in 10 ms." });
  assert.equal(fx.dispatched, true);
});

const dragStep = { step: "drag", path: [{ x: 100, y: 120 }, { x: 180, y: 160 }, { x: 260, y: 200 }], modifiers: 0 };

test("an intercepted HTML5 drag is replayed as dragEnter, dragOver and drop at the last point", async () => {
  const { chrome, input } = await ready();
  const data = { items: [{ mimeType: "text/plain", data: "card" }], dragOperationsMask: 1 };
  chrome.handlers["Input.dispatchMouseEvent"] = (target, params) => {
    if (params.type === "mouseMoved" && params.x === 180) setTimeout(() => input.dragIntercepted(TAB, data), 0);
    return {};
  };
  const result = await input.run(inputArgs([dragStep]), effects(), "p4_9");
  assert.deepEqual(result, { completed_steps: 1, native_drag: true });
  const sequence = inputEvents(chrome).map((call) => call.params.type ?? `${call.method}:${call.params.enabled ?? ""}`);
  assert.deepEqual(sequence, [
    "Input.setInterceptDrags:true", "mouseMoved", "mousePressed", "mouseMoved",
    "Input.setInterceptDrags:false", "dragEnter", "dragOver", "drop", "mouseReleased"
  ]);
  const drop = inputEvents(chrome).find((call) => call.params.type === "drop");
  assert.deepEqual(drop.params, { type: "drop", x: 260, y: 200, data, modifiers: 0 });
  assert.equal(methods(chrome, "Input.cancelDragging").length, 0);
});

test("a drag that is not intercepted presses, moves and releases", async () => {
  const { chrome, input } = await ready();
  const result = await input.run(inputArgs([dragStep]), effects(), "p4_10");
  assert.deepEqual(result, { completed_steps: 1, native_drag: false });
  const sequence = inputEvents(chrome).map((call) => call.params.type ?? `${call.method}:${call.params.enabled}`);
  assert.deepEqual(sequence, [
    "Input.setInterceptDrags:true", "mouseMoved", "mousePressed", "mouseMoved", "mouseMoved",
    "mouseReleased", "Input.setInterceptDrags:false"
  ]);
  const moves = inputEvents(chrome).filter((call) => call.params.type === "mouseMoved");
  assert.deepEqual([moves[1].params.buttons, moves[1].params.force, moves[1].params.button], [1, 1, "left"]);
});

test("interception data within the grace is replayed; data after the drag ended is cancelled", async () => {
  const { chrome, input } = await ready();
  const data = { items: [], dragOperationsMask: 1 };
  chrome.handlers["Input.dispatchMouseEvent"] = (target, params) => {
    if (params.type === "mouseMoved" && params.x === 260) setTimeout(() => input.dragIntercepted(TAB, data), 40);
    return {};
  };
  const graced = await input.run(inputArgs([dragStep]), effects(), "p4_11");
  assert.equal(graced.native_drag, true);

  delete chrome.handlers["Input.dispatchMouseEvent"];
  chrome.calls.length = 0;
  const plain = await input.run(inputArgs([dragStep]), effects(), "p4_12");
  assert.equal(plain.native_drag, false);
  await input.dragIntercepted(TAB, data);
  assert.deepEqual(methods(chrome, "Input.cancelDragging"), ["Input.cancelDragging"]);
  assert.deepEqual(chrome.calls.at(-1).target, { tabId: TAB });
});

test("a drag that fails while the button is held releases it and cancels dragging", async () => {
  const { chrome, input } = await ready();
  chrome.handlers["Input.dispatchMouseEvent"] = (target, params) => {
    if (params.type === "mouseMoved" && params.x === 180) throw new Error("Input dispatch failed");
    return {};
  };
  await assert.rejects(input.run(inputArgs([dragStep]), effects(), "p4_13"), /Input dispatch failed/);
  const tail = inputEvents(chrome).slice(-3).map((call) => call.params.type ?? `${call.method}:${call.params.enabled ?? ""}`);
  assert.deepEqual(tail, ["Input.setInterceptDrags:false", "mouseReleased", "Input.cancelDragging:"]);
});

test("a drag without interception support still drags", async () => {
  const { chrome, input } = await ready();
  chrome.handlers["Input.setInterceptDrags"] = () => { throw new Error("not supported"); };
  const result = await input.run(inputArgs([dragStep]), effects(), "p4_14");
  assert.deepEqual(result, { completed_steps: 1, native_drag: false });
  assert.equal(methods(chrome, "Input.setInterceptDrags").length, 1);
});
