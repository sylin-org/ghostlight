// SPDX-License-Identifier: Apache-2.0 OR MIT
// Tests for the gif_creator recording buffer (ADR-0050 Decision 5, extension/lib/recbuffer.js): the
// bounded per-tab frame state machine (start -> 1 frame; captures grow it; clear empties it; the
// bound drops the oldest).

const test = require("node:test");
const assert = require("node:assert");
const rb = require("../../extension/lib/recbuffer.js");

test("start seeds one frame; captures grow it; clear empties it", () => {
  const store = rb.createStore();
  assert.strictEqual(rb.start(store, 1, "f0"), 1, "start with a first frame -> 1 frame");
  assert.strictEqual(rb.capture(store, 1, "f1"), 2);
  assert.strictEqual(rb.capture(store, 1, "f2"), 3, "two captures -> 3 frames total");
  assert.deepStrictEqual(rb.frames(store, 1), ["f0", "f1", "f2"]);
  rb.clear(store, 1);
  assert.deepStrictEqual(rb.frames(store, 1), [], "clear discards the recording");
});

test("capture is a no-op unless the tab is actively recording", () => {
  const store = rb.createStore();
  assert.strictEqual(rb.capture(store, 7, "x"), -1, "no recording -> no capture");
  rb.start(store, 7);
  assert.strictEqual(rb.capture(store, 7, "a"), 1);
  rb.stop(store, 7);
  assert.strictEqual(rb.capture(store, 7, "b"), -1, "stopped -> no more captures");
  assert.deepStrictEqual(rb.frames(store, 7), ["a"], "stop keeps the captured frames");
});

test("the bound drops the oldest frame beyond max", () => {
  const store = rb.createStore(3); // small bound for the test
  rb.start(store, 2, "0");
  rb.capture(store, 2, "1");
  rb.capture(store, 2, "2"); // now [0,1,2], at the bound
  rb.capture(store, 2, "3"); // pushes the 4th -> drops "0"
  assert.deepStrictEqual(rb.frames(store, 2), ["1", "2", "3"], "oldest ('0') evicted, count stays 3");
});

test("recordings are independent per tab", () => {
  const store = rb.createStore();
  rb.start(store, 1, "a");
  rb.start(store, 2, "b");
  rb.capture(store, 1, "a2");
  assert.deepStrictEqual(rb.frames(store, 1), ["a", "a2"]);
  assert.deepStrictEqual(rb.frames(store, 2), ["b"]);
});
