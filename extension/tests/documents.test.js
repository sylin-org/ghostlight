"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const api = require("../lib/documents.js");
const frames = require("../lib/frames.js");

function fixture() {
  const state = { raw: [
    { frameId: 0, parentFrameId: -1, documentId: "top-1", url: "https://allowed.test/" },
    { frameId: 2, parentFrameId: 0, documentId: "child-1", url: "https://excluded.test/" }
  ], calls: [], answer: () => ({ text: "permitted" }) };
  const documents = api.create({ frames, getFrames: async () => state.raw,
    sendDocument: async (tab, id, message) => { state.calls.push({ tab, id, message }); return state.answer(id, message); } });
  const scope = { documents: api.inventory(state.raw), allowed: ["top-1"], subjects: [], mask: null, watch_changes: true };
  return { state, documents, scope };
}

test("excluded documents receive no extraction, and locators bind to document identity", async () => {
  const { state, documents, scope } = fixture();
  const receipt = await documents.run(7, scope, async () => {
    assert.deepEqual(documents.frameIds(7), [0]);
    const locator = documents.locator(7, 0, "locator_3");
    assert.equal(frames.documentOf(locator), "top-1");
    assert.equal(frames.localOf(locator), "locator_3");
    await assert.rejects(documents.route(7, 2, { kind: "read_text" }), { code: "document_scope_changed" });
    await documents.route(7, 0, { kind: "read_text" });
    return { outcome: "text", truncated: true };
  });
  assert.deepEqual(state.calls.map((item) => item.id), ["top-1"]);
  assert.deepEqual(receipt.observation.visited, ["top-1"]);
  assert.equal(receipt.observation.limited_by_size, true);
  assert.equal(documents.context(7), undefined);
});

test("reused frame ids cannot revive stale target handles", async () => {
  const { state, documents } = fixture();
  const locator = frames.scopedLocator(2, "locator_1", "child-1");
  state.raw[1].documentId = "child-2";
  const result = await documents.describe({ tab_id: 7, locators: [locator], points: [], focused: false });
  assert.equal(result.unresolved, true);
  assert.deepEqual(result.subjects, []);
  assert.deepEqual(state.calls, []);
});

test("a changed graph refuses before invoking the primitive", async () => {
  const { state, documents, scope } = fixture();
  state.raw[1].url = "https://different.test/";
  let effects = 0;
  await assert.rejects(documents.run(7, scope, async () => { effects++; }), { code: "document_scope_changed", effectUnknown: false });
  assert.equal(effects, 0);
});

test("targeted work ignores unrelated active-frame replacement even with restricted allowed content", async () => {
  const { state, documents, scope } = fixture();
  scope.strict_tree = false;
  scope.subjects = ["top-1"];
  state.raw[1].documentId = "child-2";
  let effects = 0;
  await documents.run(7, scope, async () => {
    await documents.input(7, "Input.dispatchMouseEvent", { x: 10, y: 10 }); effects++;
    state.raw[1].documentId = "child-3";
    await documents.input(7, "Input.dispatchMouseEvent", { x: 10, y: 10 }); effects++;
    return {};
  });
  assert.equal(effects, 2);
});

test("relaxed scope still stops replacement of the actual input document and its ancestry", async () => {
  const { state, documents, scope } = fixture();
  scope.strict_tree = false;
  scope.subjects = ["top-1"];
  let effects = 0;
  await assert.rejects(documents.run(7, scope, async () => {
    await documents.input(7, "Input.dispatchMouseEvent", { x: 10, y: 10 }); effects++;
    state.raw[0].documentId = "top-2";
    state.raw[1].parentDocumentId = "top-2";
    await documents.input(7, "Input.dispatchMouseEvent", { x: 10, y: 10 }); effects++;
  }), { code: "document_scope_changed", effectUnknown: true });
  assert.equal(effects, 1);
});

test("same-document URL changes do not invalidate a permitted target", async () => {
  const { state, documents, scope } = fixture();
  scope.strict_tree = false;
  state.raw[0].url = "https://allowed.test/new-route?updated=1";
  await documents.run(7, scope, async () => { await documents.verify(7); return {}; });
});

test("relaxed pointer checks re-route the current point when an excluded frame moves over it", async () => {
  const { state, documents, scope } = fixture();
  scope.strict_tree = false;
  scope.subjects = ["top-1"];
  state.answer = () => ({ x: 10, y: 10 });
  await assert.rejects(documents.run(7, scope, async () => {
    await documents.input(7, "Input.dispatchMouseEvent", { x: 10, y: 10 });
    state.answer = id => id === "top-1"
      ? { x: 10, y: 10, embed: { src: "https://excluded.test/", left: 0, top: 0 } }
      : { x: 10, y: 10 };
    await documents.input(7, "Input.dispatchMouseEvent", { x: 10, y: 10 });
  }), { code: "document_scope_changed", effectUnknown: true });
});

test("relaxed keyboard checks re-resolve current focus before the next packet", async () => {
  const { state, documents, scope } = fixture();
  scope.strict_tree = false;
  scope.subjects = ["top-1"];
  let focused = "top-1";
  state.answer = id => ({ focused: id === focused });
  await assert.rejects(documents.run(7, scope, async () => {
    await documents.input(7, "Input.dispatchKeyEvent", { type: "keyDown" });
    focused = "child-1";
    await documents.input(7, "Input.dispatchKeyEvent", { type: "keyDown" });
  }), { code: "document_scope_changed", effectUnknown: true });
});

test("implicit collectors share the remaining settlement budget and opt-out skips it", async () => {
  for (const budget of [0, 1000]) {
    const { state, documents, scope } = fixture();
    scope.settle_ms = budget;
    await documents.run(7, scope, async context => {
      if (budget) context.settleDeadline = Date.now() + 40;
      await documents.route(7, 0, { kind: "read_text" });
      context.settleDeadline = Date.now() - 1;
      await documents.route(7, 0, { kind: "query_semantic" });
      return {};
    });
    assert.equal(state.calls[0].message.visual_settle, budget > 0);
    assert.ok(state.calls[0].message.settle_ms <= 40);
    assert.equal(state.calls[1].message.visual_settle, false);
    assert.equal(state.calls[1].message.settle_ms, 0);
  }
});

test("unavailable content is separate from empty content and size ceilings", async () => {
  const { state, documents, scope } = fixture();
  state.answer = () => { throw new Error("document gone"); };
  const receipt = await documents.run(7, scope, async () => {
    await documents.route(7, 0, { kind: "inspect" }).catch(() => {});
    return { targets: [] };
  });
  assert.deepEqual(receipt.observation.visited, []);
  assert.deepEqual(receipt.observation.unavailable, ["top-1"]);
  assert.equal(receipt.observation.limited_by_size, false);
});

test("point input cannot fall through into excluded content", async () => {
  const { state, documents, scope } = fixture();
  state.answer = (id) => id === "top-1" ? { x: 10, y: 10, embed: { src: "https://excluded.test/", left: 0, top: 0 } } : { x: 10, y: 10 };
  await documents.run(7, scope, async () => {
    await assert.rejects(documents.input(7, "Input.dispatchMouseEvent", { x: 10, y: 10 }), { code: "document_scope_changed" });
    assert.equal(documents.context(7).dispatched, false);
    return {};
  });
  assert.ok(state.calls.every((item) => item.message.kind === "document_route"));
});

test("later scope failure preserves already dispatched effects", async () => {
  const { state, documents, scope } = fixture();
  await assert.rejects(documents.run(7, scope, async () => {
    await documents.route(7, 0, { kind: "fill" });
    state.raw[1].documentId = "child-2";
    await documents.verify(7);
  }), { code: "document_scope_changed", effectUnknown: true });
  assert.equal(documents.context(7), undefined);
});

test("focused input preflight refuses absent or excluded focus without marking dispatch", async () => {
  for (const focusedId of [null, "child-1"]) {
    const { state, documents, scope } = fixture();
    state.answer = (id) => ({ focused: id === focusedId });
    await assert.rejects(documents.run(7, scope, async () => {
      await documents.verifyInput(7, "Input.insertText", { text: "replacement" });
    }), { code: "document_scope_changed", effectUnknown: false });
    assert.ok(state.calls.every(({ message }) => message.kind === "document_route"));
  }
});

test("targeted typing dispatch remains bound to the admitted document", async () => {
  const { state, documents, scope } = fixture();
  await assert.rejects(documents.run(7, scope, async () => {
    await assert.rejects(documents.route(7, 2, { kind: "type_text", text: "excluded" }),
      { code: "document_scope_changed" });
    assert.equal(documents.context(7).dispatched, false);
    await documents.route(7, 0, { kind: "type_text", text: "permitted" });
    state.raw[0].documentId = "top-2";
    await documents.verify(7);
  }), { code: "document_scope_changed", effectUnknown: true });
  assert.deepEqual(state.calls.map(({ id }) => id), ["top-1"]);
});

test("malformed and oversized document inventories refuse", () => {
  for (const raw of [[], [{}], Array(257).fill({}), [
    { frameId: 0, parentFrameId: -1, documentId: "same", url: "https://allowed.test/" },
    { frameId: 1, parentFrameId: 0, documentId: "same", url: "https://allowed.test/" }
  ]]) assert.throws(() => api.inventory(raw), { code: "document_scope_changed" });
});
