// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/presentation-placement.js (ADRs 0072/0083).

const { test } = require("node:test");
const assert = require("node:assert");
const {
  POINTER_FRESH_MS,
  TOUCHED_FRESH_MS,
  FOCUSED_FRESH_MS,
  SCROLL_FRESH_MS,
  chooseNarrationPosition,
  chooseSignaturePosition,
} = require("../../extension/lib/presentation-placement.js");

const NOW = 20000;

test("explicit narration edges are deterministic", () => {
  const busyTop = {
    viewportHeight: 1000,
    touched: { y: 100, at: NOW },
    pointer: { y: 120, at: NOW },
  };
  assert.strictEqual(chooseNarrationPosition("top", busyTop, NOW), "top");
  assert.strictEqual(chooseNarrationPosition("bottom", busyTop, NOW), "bottom");
});

test("auto narration avoids touched controls, signatures, and recent pointer activity", () => {
  assert.strictEqual(chooseNarrationPosition("auto", {
    viewportHeight: 1000,
    touched: { y: 850, at: NOW },
    pointer: { y: 100, at: NOW },
    scroll: { direction: "up", at: NOW },
  }, NOW), "top");
  assert.strictEqual(chooseNarrationPosition("auto", {
    viewportHeight: 900,
    signaturePosition: "bottom-right",
  }, NOW), "top");
  assert.strictEqual(chooseNarrationPosition("auto", {
    viewportHeight: 900,
    pointer: { y: 80, at: NOW },
  }, NOW), "bottom");
});

test("auto narration puts content on the quiet edge while scrolling", () => {
  assert.strictEqual(chooseNarrationPosition("auto", {
    viewportHeight: 900,
    scroll: { direction: "down", at: NOW },
  }, NOW), "top");
  assert.strictEqual(chooseNarrationPosition("auto", {
    viewportHeight: 900,
    scroll: { direction: "up", at: NOW },
  }, NOW), "bottom");
});

test("signature placement defaults bottom-right and avoids narration", () => {
  const viewport = { viewportWidth: 1000, viewportHeight: 800 };
  assert.strictEqual(chooseSignaturePosition(viewport, NOW), "bottom-right");
  assert.strictEqual(chooseSignaturePosition({
    ...viewport,
    narrationPosition: "bottom",
  }, NOW), "top-right");
});

test("signature placement avoids recent touched, focused, and pointer quadrants", () => {
  const viewport = { viewportWidth: 1000, viewportHeight: 800 };
  assert.strictEqual(chooseSignaturePosition({
    ...viewport,
    touched: { x: 900, y: 700, at: NOW },
  }, NOW), "top-left");
  assert.strictEqual(chooseSignaturePosition({
    ...viewport,
    focused: { x: 100, y: 100, at: NOW },
  }, NOW), "bottom-right");
  assert.strictEqual(chooseSignaturePosition({
    ...viewport,
    pointer: { x: 900, y: 100, at: NOW },
  }, NOW), "bottom-left");
});

test("stale placement signals are ignored", () => {
  const context = {
    viewportWidth: 1000,
    viewportHeight: 900,
    touched: { x: 900, y: 800, at: NOW - TOUCHED_FRESH_MS - 1 },
    focused: { x: 900, y: 800, at: NOW - FOCUSED_FRESH_MS - 1 },
    pointer: { x: 900, y: 800, at: NOW - POINTER_FRESH_MS - 1 },
    scroll: { direction: "up", at: NOW - SCROLL_FRESH_MS - 1 },
  };
  assert.strictEqual(chooseNarrationPosition("auto", context, NOW), "bottom");
  assert.strictEqual(chooseSignaturePosition(context, NOW), "bottom-right");
});
