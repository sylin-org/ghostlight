"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Notice = require("../lib/notice.js");
const protocol = require("../lib/protocol.js");
const Queue = require("../lib/presentation-queue.js");
const World = require("../lib/world.js");
const { TOP, TOP_LOADER, fakeChrome, assets, wire } = require("./p4-fakes.js");
const { golden, fakeBrowser } = require("./p4-chrome.js");

const TAB = 12;
const WINDOW = 1;
const GROUP = 5;
const TITLE = `${protocol.vocabulary.group_title_prefix}Research`;
const LEGACY_TITLE = `${protocol.vocabulary.legacy_group_title_prefixes[0]}Research`;

function denial(overrides = {}) {
  return { invocation: "inv_4e9a", signal: "denial", activity: "click", phase: "Blocked", detail: "Not allowed", locator: "locator_a1b2c3d4_8", ...overrides };
}

/**
 * Wires notices over a scripted debugger and browser. The tab is the active tab of a focused
 * window by default; `visible: false` makes it a background tab. `present` decides what the page answers.
 */
async function setup({ visible = true, attach = true, present = () => true } = {}) {
  const chrome = fakeChrome();
  const presented = [];
  chrome.handlers["Runtime.callFunctionOn"] = (_target, params) => {
    if (params.functionDeclaration === World.SHIMS.PRESENT) {
      presented.push({ on: params.objectId, signal: params.arguments[1].value });
      return { result: { value: present(params.arguments[1].value) } };
    }
    return { result: { value: true } };
  };
  let attaches = 0;
  const realAttach = chrome.attach;
  chrome.attach = async (...args) => {
    attaches += 1;
    return realAttach(...args);
  };
  const parts = wire(chrome);
  await parts.world.setAssets(assets());
  if (attach) await parts.lifecycle.retain(TAB);
  attaches = 0;
  const browser = fakeBrowser({
    tabs: [{ id: TAB, windowId: WINDOW, url: "https://example.com/", active: visible, groupId: GROUP }],
    groups: [{ id: GROUP, windowId: WINDOW, title: TITLE }],
    focusedWindows: visible ? [WINDOW] : []
  });
  const queue = Queue.create();
  let changes = 0;
  const notice = Notice.create({ chromeApi: browser, worldCalls: parts.world, inventory: parts.inventory, queue, changed: () => { changes += 1; } });
  return {
    ...parts,
    chrome,
    browser,
    queue,
    notice,
    presented,
    attaches: () => attaches,
    changes: () => changes,
    show() {
      browser.tabList[0].active = true;
      browser.windowList.get(WINDOW).focused = true;
    }
  };
}

test("a notice is rendered in the world of its frame and never attaches", async () => {
  const frame = golden("frames/notice");
  const { notice, presented, queue, attaches } = await setup();
  const rendered = await notice.deliver({ tab_id: TAB, frame: { frame_id: TOP, loader_id: TOP_LOADER }, signal: frame.signal, defer_until_visible: false });
  assert.equal(rendered, true);
  assert.deepEqual(presented, [{ on: `global:${TOP}`, signal: frame.signal }]);
  assert.equal(queue.size(), 0);
  assert.equal(attaches(), 0);
});

test("a notice for a tab that is not attached is not rendered and nothing attaches", async () => {
  const { notice, presented, attaches, chrome } = await setup({ attach: false });
  const rendered = await notice.deliver({ tab_id: TAB, signal: denial(), defer_until_visible: false });
  assert.equal(rendered, false);
  assert.deepEqual(presented, []);
  assert.equal(attaches(), 0);
  assert.equal(chrome.calls.length, 0);
});

test("a notice with no frame goes to the top document", async () => {
  const { notice, presented } = await setup();
  await notice.deliver({ tab_id: TAB, signal: denial({ signal: "start" }) });
  assert.deepEqual(presented.map((entry) => entry.on), [`global:${TOP}`]);
});

test("a deferrable notice for a background tab retires the spinner at once and waits in the queue", async () => {
  const { notice, presented, queue, changes } = await setup({ visible: false });
  const rendered = await notice.deliver({ tab_id: TAB, frame: { frame_id: TOP, loader_id: TOP_LOADER }, signal: denial(), defer_until_visible: true });
  assert.equal(rendered, false);
  assert.equal(presented.length, 1);
  assert.deepEqual(presented[0].signal, {
    invocation: "inv_4e9a", signal: "completion", activity: "quiet", phase: "", detail: null, locator: null, click: null
  });
  assert.equal(queue.size(), 1);
  assert.ok(changes() > 0);
  const queued = queue.get(TAB);
  assert.equal(queued.tabId, TAB);
  assert.equal(queued.signal.signal, "denial");
});

test("a deferred notice is shown when its tab becomes visible, then leaves the queue", async () => {
  const { notice, presented, queue, show } = await setup({ visible: false });
  await notice.deliver({ tab_id: TAB, frame: { frame_id: TOP, loader_id: TOP_LOADER }, signal: denial(), defer_until_visible: true });
  presented.length = 0;
  assert.equal(await notice.flush(TAB), false);
  assert.deepEqual(presented, []);
  assert.equal(queue.size(), 1);

  show();
  assert.equal(await notice.flush(TAB), true);
  assert.equal(presented.length, 1);
  assert.deepEqual(presented[0], { on: `global:${TOP}`, signal: denial() });
  assert.equal(queue.size(), 0);
  assert.equal(await notice.flush(TAB), false);
});

test("a deferred notice whose frame was replaced falls back to the top document without its locator", async () => {
  const { notice, presented, queue, show } = await setup({ visible: false });
  await notice.deliver({ tab_id: TAB, frame: { frame_id: "GONE", loader_id: "L-old" }, signal: denial(), defer_until_visible: true });
  presented.length = 0;
  show();
  assert.equal(await notice.flush(TAB), true);
  assert.deepEqual(presented, [{ on: `global:${TOP}`, signal: { ...denial(), locator: null } }]);
  assert.equal(queue.size(), 0);
});

test("a deferred notice stays queued while the tab is not attached", async () => {
  const { notice, queue, show, attaches } = await setup({ visible: false, attach: false });
  await notice.deliver({ tab_id: TAB, signal: denial(), defer_until_visible: true });
  show();
  assert.equal(await notice.flush(TAB), false);
  assert.equal(queue.size(), 1);
  assert.equal(attaches(), 0);
});

test("a deferrable notice for a visible tab that refuses it is queued", async () => {
  const { notice, queue } = await setup({ present: () => false });
  const rendered = await notice.deliver({ tab_id: TAB, signal: denial(), defer_until_visible: true });
  assert.equal(rendered, false);
  assert.equal(queue.size(), 1);
});

test("a notice that renders clears an older queued notice for its tab", async () => {
  const { notice, queue, show } = await setup({ visible: false });
  await notice.deliver({ tab_id: TAB, signal: denial(), defer_until_visible: true });
  assert.equal(queue.size(), 1);
  show();
  await notice.deliver({ tab_id: TAB, signal: denial({ invocation: "inv_2" }), defer_until_visible: false });
  assert.equal(queue.size(), 0);
});

test("a deferred notice without detail is still queued", async () => {
  const { notice, queue } = await setup({ visible: false });
  await notice.deliver({ tab_id: TAB, signal: denial({ detail: null }), defer_until_visible: true });
  assert.equal(queue.size(), 1);
});

test("only a denial can wait: another signal for a background tab is retired and dropped", async () => {
  const { notice, queue, presented } = await setup({ visible: false });
  await notice.deliver({ tab_id: TAB, signal: denial({ signal: "progress" }), defer_until_visible: true });
  assert.equal(presented.length, 1);
  assert.equal(queue.size(), 0);
});

test("window focus flushes the active tab of that window", async () => {
  const { notice, presented, queue, show } = await setup({ visible: false });
  await notice.deliver({ tab_id: TAB, signal: denial(), defer_until_visible: true });
  presented.length = 0;
  await notice.flushWindow(-1);
  assert.equal(queue.size(), 1);
  await notice.flushWindow(WINDOW);
  assert.equal(queue.size(), 1);
  show();
  await notice.flushWindow(WINDOW);
  assert.equal(queue.size(), 0);
  assert.equal(presented.length, 1);
});

test("the activity list follows the invocations, labelled by the tab's group title", async () => {
  const { notice } = await setup();
  await notice.deliver({ tab_id: TAB, signal: { invocation: "inv_1", signal: "progress", activity: "click", phase: "Clicking Save" } });
  assert.deepEqual(notice.activityList(), [{ invocation: "inv_1", client_label: "Research", label: "Clicking", phase: "Clicking Save" }]);
  await notice.deliver({ tab_id: TAB, signal: { invocation: "inv_1", signal: "completion", activity: "quiet", phase: "" } });
  assert.deepEqual(notice.activityList(), []);
});

test("the activity list uses the frame's label and falls back to the table without one", async () => {
  const { notice } = await setup();
  await notice.deliver({ tab_id: TAB, signal: { invocation: "inv_1", signal: "progress", activity: "click", phase: "p", label: "Pressing" } });
  assert.equal(notice.activityList()[0].label, "Pressing");
  await notice.deliver({ tab_id: TAB, signal: { invocation: "inv_1", signal: "progress", activity: "click", phase: "p", label: "" } });
  assert.equal(notice.activityList()[0].label, "Clicking");
});

test("the activity list names an ungrouped tab's client generically and keeps only the last eight", async () => {
  const { notice, browser } = await setup();
  browser.tabList[0].groupId = -1;
  for (let index = 0; index < 10; index += 1) {
    await notice.deliver({ tab_id: TAB, signal: { invocation: `inv_${index}`, signal: "progress", activity: "read", phase: "p" } });
  }
  const list = notice.activityList();
  assert.equal(list.length, 8);
  assert.equal(list[0].invocation, "inv_2");
  assert.equal(list[0].client_label, "MCP client");
});

test("a notice with no usable tab or signal is ignored", async () => {
  const { notice, presented } = await setup();
  assert.equal(await notice.deliver({ signal: denial() }), false);
  assert.equal(await notice.deliver({ tab_id: TAB }), false);
  assert.equal(await notice.deliver(), false);
  assert.deepEqual(presented, []);
});

test("the activity list names the agent of a group an earlier release titled", async () => {
  const { notice, browser } = await setup();
  browser.groupList[0].title = LEGACY_TITLE;
  await notice.deliver({ tab_id: TAB, signal: { invocation: "inv_1", signal: "progress", activity: "click", phase: "p" } });
  assert.equal(notice.activityList()[0].client_label, "Research");
});
