// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/narration-placement.js (ADR-0072).

const { test } = require("node:test");
const assert = require("node:assert");
const {
  POINTER_FRESH_MS,
  TOUCHED_FRESH_MS,
  SCROLL_FRESH_MS,
  chooseNarrationPosition,
} = require("../../extension/lib/narration-placement.js");

const NOW = 20000;

test("explicit_edges_are_deterministic", () => {
  const busyTop = {
    viewportHeight: 1000,
    touched: { y: 100, at: NOW },
    pointer: { y: 120, at: NOW },
  };
  assert.strictEqual(chooseNarrationPosition("top", busyTop, NOW), "top");
  assert.strictEqual(chooseNarrationPosition("bottom", busyTop, NOW), "bottom");
});

test("auto_avoids_the_last_touched_control_before_other_signals", () => {
  const context = {
    viewportHeight: 1000,
    touched: { y: 850, at: NOW },
    pointer: { y: 100, at: NOW },
    scroll: { direction: "up", at: NOW },
  };
  assert.strictEqual(chooseNarrationPosition("auto", context, NOW), "top");
});

test("auto_avoids_a_recent_pointer_when_no_control_was_touched", () => {
  assert.strictEqual(chooseNarrationPosition("auto", {
    viewportHeight: 900,
    pointer: { y: 80, at: NOW },
  }, NOW), "bottom");
});

test("auto_puts_new_content_on_the_uncovered_edge_while_scrolling", () => {
  assert.strictEqual(chooseNarrationPosition("auto", {
    viewportHeight: 900,
    scroll: { direction: "down", at: NOW },
  }, NOW), "top");
  assert.strictEqual(chooseNarrationPosition("auto", {
    viewportHeight: 900,
    scroll: { direction: "up", at: NOW },
  }, NOW), "bottom");
});

test("stale_signals_are_ignored_and_bottom_is_the_cinematic_fallback", () => {
  const context = {
    viewportHeight: 900,
    touched: { y: 800, at: NOW - TOUCHED_FRESH_MS - 1 },
    pointer: { y: 800, at: NOW - POINTER_FRESH_MS - 1 },
    scroll: { direction: "up", at: NOW - SCROLL_FRESH_MS - 1 },
  };
  assert.strictEqual(chooseNarrationPosition("auto", context, NOW), "bottom");
  assert.strictEqual(chooseNarrationPosition(undefined, {}, NOW), "bottom");
});
