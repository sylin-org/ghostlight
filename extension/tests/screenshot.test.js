"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const screenshot = require("../lib/screenshot.js");

test("ordinary captures shrink when needed but never magnify", () => {
  assert.equal(screenshot.outputScale(400, 300, false), 1);
  assert.equal(screenshot.outputScale(5000, 5000, false), 0.4);
});

test("region captures spend the full bounded output budget on magnification", () => {
  const clip = screenshot.regionClip({ x: 10, y: 20, width: 400, height: 300 });
  assert.equal(clip.x, 10);
  assert.equal(clip.y, 20);
  assert.ok(clip.scale > 1);
  assert.ok(clip.width * clip.scale <= screenshot.MAX_SIDE);
  assert.ok(clip.height * clip.scale <= screenshot.MAX_SIDE);
  assert.ok(clip.width * clip.height * clip.scale * clip.scale <= screenshot.MAX_PIXELS + 1);
});

test("region geometry rejects missing, negative, and empty rectangles", () => {
  assert.throws(() => screenshot.regionClip(null), /region is required/);
  assert.throws(() => screenshot.regionClip({ x: -1, y: 0, width: 1, height: 1 }), /x/);
  assert.throws(() => screenshot.regionClip({ x: 0, y: 0, width: 0, height: 1 }), /width/);
  assert.throws(() => screenshot.regionClip({ x: 0, y: 0, width: 1, height: NaN }), /height/);
});
