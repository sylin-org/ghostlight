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
  assert.equal(snap.complete, true);

  // The top document is not part of the comparison on either side, so a frame nobody read that
  // shows the top's own url is still unseen.
  chrome.frames.push({ frameId: 8, url: `${TOP_URL}#again`, documentLifecycle: "active" });
  assert.deepEqual((await inventory.snapshot(TAB)).unseen, ["https://example.com/embed", "https://ads.example.org/slot", `${TOP_URL}#again`]);

  // Without the browser's own list the comparison did not run, whether the browser failed or
  // answered with no list.
  for (const answer of [async () => null, async () => { throw new Error("No tab with id: 12."); }]) {
    chrome.webNavigation.getAllFrames = answer;
    const unchecked = await inventory.snapshot(TAB);
    assert.deepEqual([unchecked.complete, unchecked.unseen], [false, []]);
  }
  await lifecycle.detachAll();
});

test("unseen urls are bounded to MAX_UNSEEN", async () => {
  const frames = Array.from({ length: protocol.MAX_UNSEEN + 5 }, (_unused, index) =>
    ({ frameId: index + 1, url: `https://x.example/${index}`, documentLifecycle: "active" }));
  const { inventory, lifecycle } = await attached(fakeChrome({ frames }));
  assert.equal((await inventory.snapshot(TAB)).unseen.length, protocol.MAX_UNSEEN);
  await lifecycle.detachAll();
});

test("an attached session that cannot be read leaves the snapshot incomplete, and one Chrome no longer knows does not", async () => {
  const chrome = oopifChrome();
  const { inventory, lifecycle } = await attached(chrome);
  chrome.emit({ tabId: TAB }, "Target.attachedToTarget", { sessionId: "s1", targetInfo: { type: "iframe", targetId: "OOP" } });
  chrome.emit({ tabId: TAB, sessionId: "s1" }, "Target.attachedToTarget", { sessionId: "s2", targetInfo: { type: "iframe", targetId: "DEEP" } });
  await lifecycle.acquire(TAB);
  const asked = () => chrome.calls.filter((call) => call.method === "Page.getFrameTree").map((call) => call.target.sessionId ?? "root");

  // The frame's session is still attached and did not answer: it may hold documents nobody listed.
  chrome.handlers["Page.getFrameTree"] = (target) => {
    if (target.sessionId === "s1") throw new Error("{\"code\":-32000,\"message\":\"Internal error\"}");
    return { frameTree: chrome.trees[target.sessionId ?? "root"] };
  };
  let snap = await inventory.snapshot(TAB);
  assert.equal(snap.complete, false);
  assert.equal(snap.owner.get("OOP").loaderId, "L-stub", "the root's own entry for the frame is what remains");
  assert.equal(lifecycle.childSessions(TAB).length, 2);
  delete chrome.handlers["Page.getFrameTree"];

  // Chrome ended the session and said nothing (Chrome 157 does this for a session attached through
  // a child). It holds no document, so the snapshot is whole without it, and it is not asked again:
  // neither it nor the session announced through it.
  chrome.end("s1");
  chrome.end("s2");
  chrome.calls.length = 0;
  snap = await inventory.snapshot(TAB);
  assert.equal(snap.complete, true);
  assert.equal(snap.owner.has("INNER"), false);
  assert.deepEqual(asked(), ["root", "s1"]);
  assert.deepEqual(lifecycle.childSessions(TAB), []);
  chrome.calls.length = 0;
  assert.equal((await inventory.snapshot(TAB)).complete, true);
  assert.deepEqual(asked(), ["root"]);

  // The whole attachment ends under a frame's read. No session is registered then, which is no
  // inventory at all, never a complete one.
  chrome.emit({ tabId: TAB }, "Target.attachedToTarget", { sessionId: "s3", targetInfo: { type: "iframe", targetId: "OOP" } });
  await lifecycle.acquire(TAB);
  let reading;
  const read = new Promise((resolve) => { reading = resolve; });
  chrome.handlers["Page.getFrameTree"] = (target) => {
    if (target.sessionId !== "s3") return { frameTree: chrome.trees.root };
    reading();
    return new Promise(() => {});
  };
  const pending = inventory.snapshot(TAB);
  await read;
  lifecycle.detached(TAB);
  await assert.rejects(pending, { code: "session_lost" });
  await lifecycle.detachAll();
});

test("the root must be read: a tab with no session is session_lost, and a root that cannot answer fails the snapshot", async () => {
  const chrome = fakeChrome();
  const { inventory, lifecycle } = wire(chrome);
  await assert.rejects(inventory.snapshot(TAB), { code: "session_lost" });
  await lifecycle.retain(TAB);
  chrome.handlers["Page.getFrameTree"] = () => { throw new Error("Debugger is not attached to the tab with id: 12."); };
  await assert.rejects(inventory.snapshot(TAB), /Debugger is not attached/);
  await assert.rejects(inventory.framesPrimitive({ tab_id: TAB }), /Debugger is not attached/);
  // A root that names no top document is no inventory either, never an empty and complete one.
  chrome.handlers["Page.getFrameTree"] = () => ({});
  await assert.rejects(inventory.snapshot(TAB), { code: "document_changed" });
  await lifecycle.detachAll();
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

test("no document is read while a dialog is open: every read refuses with dialog_open and sends nothing", async () => {
  const chrome = fakeChrome();
  const { inventory, lifecycle } = await attached(chrome);
  lifecycle.openDialog(TAB, { type: "prompt", message: "Name?", url: TOP_URL });
  chrome.calls.length = 0;
  const refused = { code: "dialog_open", message: "A JavaScript dialog blocks the page." };
  await assert.rejects(inventory.snapshot(TAB), refused);
  await assert.rejects(inventory.topSnapshot(TAB), refused);
  await assert.rejects(inventory.currentLoader({ tabId: TAB }, TOP), refused);
  await assert.rejects(inventory.framesPrimitive({ tab_id: TAB }), refused);
  assert.equal(methods(chrome, "Page.getFrameTree").length, 0, "the frame tree is never asked for");
  await lifecycle.closeDialog(TAB);
  assert.equal((await inventory.snapshot(TAB)).owner.size > 0, true, "reads resume once the dialog is handled");
  await lifecycle.detachAll();
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
