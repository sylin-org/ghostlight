"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Inventory = require("../lib/inventory.js");
const protocol = require("../lib/protocol.js");
const { TOP, TOP_LOADER, CHILD, CHILD_LOADER, node, sameProcessTree, fakeChrome, wire, methods } = require("./p4-fakes.js");

const TAB = 12;
const TOP_URL = "https://example.com/account";

function oopifChrome(frames = []) {
  return fakeChrome({
    trees: {
      root: node(TOP, TOP_LOADER, TOP_URL, null, [node("OOP", "L-stub", "https://widgets.example.net/embed", TOP)]),
      s1: node("OOP", "L-oop", "https://widgets.example.net/embed", TOP, [node("INNER", "L-inner", "https://widgets.example.net/inner", "OOP")])
    },
    frames
  });
}

async function attached(chrome) {
  const parts = wire(chrome);
  await parts.lifecycle.retain(TAB);
  return parts;
}

test("the snapshot lists every document of the root session with its loader and parent", async () => {
  const chrome = fakeChrome();
  const { inventory, lifecycle } = await attached(chrome);
  const snap = await inventory.snapshot(TAB);
  assert.deepEqual(snap.top, { frame_id: TOP, loader_id: TOP_LOADER });
  assert.equal(snap.complete, true);
  assert.deepEqual(snap.unseen, []);
  assert.deepEqual(snap.owner.get(TOP).fact, { frame_id: TOP, loader_id: TOP_LOADER, url: TOP_URL, unreachable: false });
  assert.deepEqual(snap.owner.get(CHILD).fact,
    { frame_id: CHILD, loader_id: CHILD_LOADER, url: "https://example.com/embed", unreachable: false, parent_frame_id: TOP });
  assert.deepEqual(snap.owner.get(CHILD).session, { tabId: TAB });
  await lifecycle.detachAll();
});

test("an out-of-process frame belongs to its own session, with its own loader and children", async () => {
  const chrome = oopifChrome();
  const { inventory, lifecycle } = await attached(chrome);
  chrome.emit({ tabId: TAB }, "Target.attachedToTarget", { sessionId: "s1", targetInfo: { type: "iframe", targetId: "OOP" } });
  const snap = await inventory.snapshot(TAB);
  assert.deepEqual(snap.owner.get("OOP").session, { tabId: TAB, sessionId: "s1" });
  assert.equal(snap.owner.get("OOP").loaderId, "L-oop");
  assert.equal(snap.owner.get("OOP").fact.parent_frame_id, TOP);
  assert.deepEqual(snap.owner.get("INNER").session, { tabId: TAB, sessionId: "s1" });
  assert.deepEqual(snap.top, { frame_id: TOP, loader_id: TOP_LOADER });
  assert.equal(snap.owner.size, 3);
  await lifecycle.detachAll();
});

test("active frames no session reported are unseen urls, matched as a multiset without fragments", async () => {
  const chrome = fakeChrome({
    frames: [
      { frameId: 0, url: TOP_URL, documentLifecycle: "active" },
      { frameId: 4, url: "https://example.com/embed#top", documentLifecycle: "active" },
      { frameId: 5, url: "https://example.com/embed", documentLifecycle: "active" },
      { frameId: 6, url: "https://ads.example.org/slot", documentLifecycle: "active" },
      { frameId: 7, url: "https://cached.example.org/", documentLifecycle: "cached" }
    ]
  });
  const { inventory, lifecycle } = await attached(chrome);
  const snap = await inventory.snapshot(TAB);
  assert.deepEqual(snap.unseen, ["https://example.com/embed", "https://ads.example.org/slot"]);
  await lifecycle.detachAll();
});

test("unseen urls are bounded to MAX_UNSEEN", async () => {
  const frames = Array.from({ length: protocol.MAX_UNSEEN + 5 }, (_unused, index) =>
    ({ frameId: index + 1, url: `https://x.example/${index}`, documentLifecycle: "active" }));
  const { inventory, lifecycle } = await attached(fakeChrome({ frames }));
  assert.equal((await inventory.snapshot(TAB)).unseen.length, protocol.MAX_UNSEEN);
  await lifecycle.detachAll();
});

test("a session that cannot be read makes the snapshot incomplete", async () => {
  const chrome = oopifChrome();
  const { inventory, lifecycle } = await attached(chrome);
  chrome.emit({ tabId: TAB }, "Target.attachedToTarget", { sessionId: "s1", targetInfo: { type: "iframe", targetId: "OOP" } });
  chrome.handlers["Page.getFrameTree"] = (target) => {
    if (target.sessionId) throw new Error("No session with given id");
    return { frameTree: chrome.trees.root };
  };
  const snap = await inventory.snapshot(TAB);
  assert.equal(snap.complete, false);
  assert.equal(snap.owner.get("OOP").loaderId, "L-stub");
  await lifecycle.detachAll();
});

test("a tab with no debugger session is session_lost", async () => {
  const { inventory } = wire(fakeChrome());
  await assert.rejects(inventory.snapshot(TAB), { code: "session_lost" });
});

function snapOf(entries, { complete = true, unseen = [] } = {}) {
  const owner = new Map(entries.map(([frameId, loaderId, url]) => [frameId, { loaderId, fact: { url } }]));
  return { owner, complete, unseen };
}

test("the exact guard compares the whole set, urls, completeness and the unseen count", () => {
  const guard = { exact: true, unseen: 0, documents: [
    { frame_id: TOP, loader_id: TOP_LOADER, url: TOP_URL },
    { frame_id: CHILD, loader_id: CHILD_LOADER, url: "https://example.com/embed" }
  ] };
  const current = [[TOP, TOP_LOADER, TOP_URL], [CHILD, CHILD_LOADER, "https://example.com/embed"]];
  assert.doesNotThrow(() => Inventory.checkGuard(guard, snapOf(current)));
  assert.doesNotThrow(() => Inventory.checkGuard(null, snapOf([])));
  const changed = { code: "document_changed" };
  assert.throws(() => Inventory.checkGuard(guard, snapOf([[TOP, TOP_LOADER, `${TOP_URL}?tab=2`], current[1]])), changed);
  assert.throws(() => Inventory.checkGuard(guard, snapOf([[TOP, "L-new", TOP_URL], current[1]])), changed);
  assert.throws(() => Inventory.checkGuard(guard, snapOf(current, { complete: false })), changed);
  assert.throws(() => Inventory.checkGuard(guard, snapOf([...current, ["MORE", "L", "https://example.com/more"]])), changed);
  assert.throws(() => Inventory.checkGuard(guard, snapOf(current, { unseen: ["https://ads.example.org/"] })), changed);
  assert.doesNotThrow(() => Inventory.checkGuard({ ...guard, unseen: 1 }, snapOf(current, { unseen: ["https://ads.example.org/"] })));
});

test("the required guard compares identity and origin of the listed documents only", () => {
  const guard = { exact: false, unseen: 0, documents: [{ frame_id: TOP, loader_id: TOP_LOADER, url: TOP_URL }] };
  const extra = ["OTHER", "L-o", "https://elsewhere.example/"];
  assert.doesNotThrow(() => Inventory.checkGuard(guard, snapOf([[TOP, TOP_LOADER, "https://example.com/other?x=1"], extra], { complete: false, unseen: ["u"] })));
  assert.throws(() => Inventory.checkGuard(guard, snapOf([[TOP, TOP_LOADER, "https://evil.example/account"]])), { code: "document_changed" });
  assert.throws(() => Inventory.checkGuard(guard, snapOf([[TOP, "L-new", TOP_URL]])), { code: "document_changed" });
  assert.throws(() => Inventory.checkGuard(guard, snapOf([])), { code: "document_changed" });
  assert.ok(protocol.isKnown("receipt_failures", "document_changed"));
});

test("currentLoader reads the frame tree of the session", async () => {
  const chrome = fakeChrome({ trees: { root: sameProcessTree() } });
  const { inventory } = wire(chrome);
  assert.equal(await inventory.currentLoader({ tabId: TAB }, CHILD), CHILD_LOADER);
  assert.equal(await inventory.currentLoader({ tabId: TAB }, "GONE"), null);
});

test("the frames primitive answers the fixture shape inside a debugger lease", async () => {
  const chrome = fakeChrome({
    trees: { root: node(TOP, TOP_LOADER, TOP_URL, null, [node(CHILD, CHILD_LOADER, "https://widgets.example.net/embed", TOP)]) },
    frames: [{ frameId: 3, url: "https://ads.example.org/slot", documentLifecycle: "active" },
      { frameId: 2, url: "https://widgets.example.net/embed", documentLifecycle: "active" }]
  });
  const { inventory, lifecycle } = wire(chrome);
  const result = await inventory.framesPrimitive({ tab_id: TAB });
  assert.deepEqual(result, {
    frames: [
      { frame_id: TOP, loader_id: TOP_LOADER, url: TOP_URL, unreachable: false },
      { frame_id: CHILD, loader_id: CHILD_LOADER, parent_frame_id: TOP, url: "https://widgets.example.net/embed", unreachable: false }
    ],
    complete: true,
    unseen: ["https://ads.example.org/slot"]
  });
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(methods(chrome).includes("Runtime.enable"), false);
});

test("the inventory requires debugger sessions and webNavigation", () => {
  assert.throws(() => Inventory.create({}), /requires/);
});
