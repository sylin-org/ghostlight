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

test("concurrent scope admission never overlaps unresolved handlers after inventory awaits", async () => {
  const { documents, scope } = fixture();
  let finish;
  let handlers = 0;
  const first = documents.run(7, scope, async () => {
    handlers++;
    await new Promise(resolve => { finish = resolve; });
    return {};
  });
  const second = assert.rejects(documents.run(7, scope, async () => { handlers++; return {}; }),
    { code: "operation_cleanup_required", effectUnknown: false });
  await second;
  assert.equal(handlers, 1);
  finish(); await first;
  assert.equal(documents.context(7), undefined);
});

test("a stale cleanup promise cannot replace the current scope's bounded wait", async () => {
  const { documents, scope } = fixture();
  let finish;
  const work = documents.run(7, scope, async context => {
    const stale = { tabId: 7 };
    documents.cleaning(stale, new Promise(() => {}));
    assert.equal(context.cleanup, null);
    await new Promise(resolve => { finish = resolve; }); return {};
  });
  await new Promise(resolve => setImmediate(resolve));
  await assert.rejects(documents.run(7, scope, async () => ({})), { code: "operation_cleanup_required" });
  finish(); await work;
});

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

test("explicit active snapshots preserve the legacy current-tree inventory", async () => {
  const { state, documents } = fixture();
  const expected = api.inventory(state.raw);
  state.raw = state.raw.map(frame => ({ ...frame, documentLifecycle: "active" }));
  assert.deepEqual(api.inventory(state.raw), expected);
  assert.deepEqual((await documents.current(7)).documents, expected);
});

test("prerendered, cached, and pending-deletion roots and descendants are outside current coverage", async () => {
  for (const lifecycle of ["prerender", "cached", "pending_deletion"]) {
    const { state, documents, scope } = fixture();
    state.raw = state.raw.map(frame => ({ ...frame, documentLifecycle: "active" }));
    state.raw.push(
      { frameId: 42, parentFrameId: -1, documentId: "inactive-root", url: "https://inactive.test/", documentLifecycle: lifecycle },
      { frameId: 43, parentFrameId: 42, parentDocumentId: "inactive-root", documentId: "inactive-child", url: "https://inactive-child.test/", documentLifecycle: lifecycle }
    );
    const current = await documents.current(7);
    assert.deepEqual(current.raw.map(frame => frame.frameId), [0, 2]);
    assert.deepEqual(current.documents, scope.documents);
    state.answer = id => ({ focused: id === "inactive-root" });
    const focused = await documents.describe({ tab_id: 7, locators: [], points: [], focused: true });
    assert.equal(focused.unresolved, true);
    assert.deepEqual(focused.subjects, []);
    assert.deepEqual(state.calls.map(call => call.id), ["top-1", "child-1"]);
    state.calls.length = 0;
    const receipt = await documents.run(7, scope, async () => {
      await assert.rejects(documents.route(7, 42, { kind: "read_text" }), { code: "document_scope_changed" });
      await documents.route(7, 0, { kind: "read_text" });
      return { text: "current" };
    });
    assert.deepEqual(receipt.observation.visited, ["top-1"]);
    assert.deepEqual(receipt.observation.unavailable, []);
    assert.deepEqual(state.calls.map(call => call.id), ["top-1"]);
  }
});

test("inactive trees may change without invalidating the exact current scope", async () => {
  const { state, documents, scope } = fixture();
  state.raw = state.raw.map(frame => ({ ...frame, documentLifecycle: "active" }));
  await documents.run(7, scope, async () => {
    state.raw.push({ frameId: 42, parentFrameId: -1, documentId: "prerender-1", url: "https://other.test/", documentLifecycle: "prerender" });
    await documents.verify(7);
    state.raw[2].documentLifecycle = "cached";
    state.raw[2].documentId = "cached-2";
    await documents.verify(7);
    state.raw.pop();
    await documents.verify(7);
    await documents.route(7, 0, { kind: "read_text" });
    return {};
  });
  assert.deepEqual(state.calls.map(call => call.id), ["top-1"]);
});

test("observed active and prerender roots permit consecutive scoped reads and fills", async () => {
  const { state, documents } = fixture();
  state.raw = [
    { frameId: 0, parentFrameId: -1, documentId: "live-top", url: "http://127.0.0.1/fixture", documentLifecycle: "active", errorOccurred: false },
    { frameId: 42, parentFrameId: -1, documentId: "prerender-top", url: "https://inactive.test/", documentLifecycle: "prerender", errorOccurred: false }
  ];
  let value = "original";
  state.answer = (id, message) => {
    assert.equal(id, "live-top");
    if (message.kind === "fill") value = message.value;
    return { text: value };
  };
  for (const [kind, expected] of [["read_text", "original"], ["fill", "replacement"], ["read_text", "replacement"]]) {
    const description = await documents.describe({ tab_id: 7, locators: [], points: [], focused: false });
    assert.deepEqual(description.documents.map(item => item.id), ["live-top"]);
    const scope = { documents: description.documents, allowed: ["live-top"], subjects: [], mask: null, watch_changes: true };
    const receipt = await documents.run(7, scope, async () => {
      await documents.verify(7);
      return documents.route(7, 0, { kind, value: "replacement" });
    });
    assert.equal(receipt.result.text, expected);
    assert.deepEqual(receipt.observation.visited, ["live-top"]);
    assert.deepEqual(receipt.observation.unavailable, []);
  }
  assert.deepEqual(state.calls.map(call => call.message.kind), ["read_text", "fill", "read_text"]);
});

test("cached document locators never regain authority through a reused frame id", async () => {
  const { state, documents } = fixture();
  state.raw = state.raw.map(frame => ({ ...frame, documentLifecycle: "active" }));
  state.raw.unshift(
    { frameId: 0, parentFrameId: -1, documentId: "old-top", url: "https://old.test/", documentLifecycle: "cached" },
    { frameId: 2, parentFrameId: 0, documentId: "old-child", url: "https://excluded.test/", documentLifecycle: "cached" }
  );
  const stale = await documents.describe({ tab_id: 7,
    locators: [frames.scopedLocator(0, "locator_1", "old-top"), frames.scopedLocator(2, "locator_2", "old-child")],
    points: [], focused: false });
  assert.equal(stale.unresolved, true);
  assert.deepEqual(stale.subjects, []);
  const current = await documents.describe({ tab_id: 7,
    locators: [frames.scopedLocator(2, "locator_3", "child-1")], points: [], focused: false });
  assert.equal(current.unresolved, false);
  assert.deepEqual(current.subjects, ["child-1"]);
  assert.deepEqual(state.calls, []);
});

test("point routing cannot match inactive frames that share an embed URL", async () => {
  const { state, documents } = fixture();
  state.raw = state.raw.map(frame => ({ ...frame, documentLifecycle: "active" }));
  state.raw.push({ frameId: 44, parentFrameId: 0, documentId: "cached-child", url: "https://excluded.test/", documentLifecycle: "cached" });
  state.answer = id => id === "top-1"
    ? { x: 10, y: 10, embed: { src: "https://excluded.test/", left: 0, top: 0 } }
    : { x: 10, y: 10 };
  const request = { tab_id: 7, locators: [], points: [{ x: 10, y: 10 }], focused: false };
  const current = await documents.describe(request);
  assert.equal(current.unresolved, false);
  assert.deepEqual(current.subjects, ["child-1"]);
  assert.deepEqual(state.calls.map(call => call.id), ["top-1", "child-1"]);
  state.raw = state.raw.filter(frame => frame.documentId !== "child-1");
  state.calls.length = 0;
  const unavailable = await documents.describe(request);
  assert.equal(unavailable.unresolved, true);
  assert.deepEqual(unavailable.subjects, []);
  assert.deepEqual(state.calls.map(call => call.id), ["top-1"]);
});

test("current HTTP and error support checks remain independent of inactive-tree exclusion", () => {
  const { state } = fixture();
  state.raw = state.raw.map(frame => ({ ...frame, documentLifecycle: "active" }));
  state.raw[1].errorOccurred = true;
  assert.equal(api.inventory(state.raw)[1].supported, false);
  state.raw[1].errorOccurred = false;
  state.raw[1].url = "about:blank";
  assert.equal(api.inventory(state.raw)[1].supported, false);
});

test("ambiguous lifecycle and inconsistent current trees fail closed", () => {
  const root = { frameId: 0, parentFrameId: -1, documentId: "top", url: "https://current.test/", documentLifecycle: "active" };
  const child = { frameId: 2, parentFrameId: 0, documentId: "child", url: "https://child.test/", documentLifecycle: "active" };
  const cases = [
    [root, { ...child, documentLifecycle: undefined }],
    [{ ...root, documentLifecycle: null }],
    [root, { ...child, documentLifecycle: "unknown" }],
    [{ ...root, documentLifecycle: "prerender" }, child],
    [root, { ...root, frameId: 42, documentId: "other-root" }],
    [{ ...root, frameId: 42 }],
    [root, { ...child, parentFrameId: 99 }],
    [root, { ...child, parentDocumentId: "old-top" }],
    [root, child, { ...child, documentId: "duplicate-frame" }],
    [root, { ...child, parentFrameId: 3 }, { ...child, frameId: 3, parentFrameId: 2, documentId: "cycle" }],
    [{ ...root, documentLifecycle: "cached" }],
    Array(api.DOCUMENT_LIMIT + 1).fill({ ...root, documentLifecycle: "prerender" })
  ];
  for (const raw of cases) assert.throws(() => api.inventory(raw), { code: "document_scope_changed" });
});
