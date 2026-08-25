"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const frames = require("../lib/frames.js");

test("scoped locators round-trip through frameOf and localOf", () => {
  const handle = frames.scopedLocator(3, "locator_12");
  assert.equal(handle, "3:locator_12");
  assert.equal(frames.frameOf(handle), 3);
  assert.equal(frames.localOf(handle), "locator_12");
});

test("the top frame is frame zero, not an unscoped locator", () => {
  const handle = frames.scopedLocator(frames.TOP_FRAME_ID, "locator_1");
  assert.equal(handle, "0:locator_1");
  assert.equal(frames.frameOf(handle), 0);
});

test("unscoped or foreign handles report no owning frame", () => {
  assert.equal(frames.frameOf("locator_4"), null);
  assert.equal(frames.frameOf(""), null);
  assert.equal(frames.frameOf(null), null);
  assert.equal(frames.frameOf(undefined), null);
});

test("localOf passes unscoped handles through unchanged", () => {
  assert.equal(frames.localOf("locator_9"), "locator_9");
});

test("scopeTargets stamps every observed target with its minting frame", () => {
  const stamped = frames.scopeTargets(2, [
    { locator: "locator_1", role: "button", name: "Save" },
    { locator: "locator_2", role: "textbox", name: "" }
  ]);
  assert.deepEqual(stamped.map((target) => target.locator), ["2:locator_1", "2:locator_2"]);
  assert.equal(stamped[0].role, "button");
  assert.deepEqual(frames.scopeTargets(0, undefined), []);
});

test("mergeTargets orders by frame id with the top document first under one ceiling", () => {
  const merged = frames.mergeTargets({
    "5": [{ locator: frames.scopedLocator(5, "locator_a") }],
    "0": [{ locator: frames.scopedLocator(0, "locator_top") }, { locator: frames.scopedLocator(0, "locator_top2") }],
    "12": [{ locator: frames.scopedLocator(12, "locator_deep") }]
  }, 10);
  assert.deepEqual(merged.map((target) => target.locator), ["0:locator_top", "0:locator_top2", "5:locator_a", "12:locator_deep"]);
});

test("mergeTargets caps the total across frames, top frame winning its share first", () => {
  const merged = frames.mergeTargets({
    "1": [{ locator: "1:locator_b" }, { locator: "1:locator_c" }, { locator: "1:locator_d" }],
    "0": [{ locator: "0:locator_a" }]
  }, 3);
  assert.deepEqual(merged.map((target) => target.locator), ["0:locator_a", "1:locator_b", "1:locator_c"]);
  assert.deepEqual(frames.mergeTargets({}, 3), []);
});

test("groupLocators preserves first-appearance frame order and in-group field order", () => {
  const groups = frames.groupLocators([
    "7:locator_s3",
    "0:locator_f1",
    "7:locator_s1",
    "2:locator_i1",
    "0:locator_f2"
  ]);
  assert.deepEqual(Array.from(groups.keys()), [7, 0, 2]);
  assert.deepEqual(
    Array.from(groups.entries()).map(([frameId, entries]) => [frameId, entries.map((entry) => entry.handle)]),
    [[7, ["7:locator_s3", "7:locator_s1"]], [0, ["0:locator_f1", "0:locator_f2"]], [2, ["2:locator_i1"]]]
  );
  const first = groups.get(7)[0];
  assert.equal(first.local, "locator_s3");
});

test("groupLocators refuses an unscoped handle instead of silently routing it to the top frame", () => {
  assert.throws(() => frames.groupLocators(["0:locator_ok", "locator_legacy"]), /not frame-scoped/);
});

test("embed identity is origin plus path, tolerant of query and fragment drift", () => {
  assert.equal(frames.embedMatches("https://js-eu1.hsforms.net/embed/form", "https://js-eu1.hsforms.net/embed/form?__hstc=1"), true);
  assert.equal(frames.embedMatches("https://sylin.org/ghostlight/demo/iframe/form/", "https://sylin.org/ghostlight/demo/iframe/form/"), true);
  assert.equal(frames.embedMatches("https://sylin.org/other/form/", "https://sylin.org/ghostlight/demo/iframe/form/"), false);
  assert.equal(frames.embedMatches("https://evil.test/ghostlight/demo/iframe/form/", "https://sylin.org/ghostlight/demo/iframe/form/"), false);
  assert.equal(frames.embedMatches("", "https://sylin.org/x/"), false);
  assert.equal(frames.embedMatches("not a url", "https://sylin.org/x/"), false);
});
