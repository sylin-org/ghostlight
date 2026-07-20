// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for complete CDP pointer-event descriptors.

const { test } = require("node:test");
const assert = require("node:assert");
const {
  BUTTON_BITS,
  mouseMoveEvent,
  mouseButtonEvent,
  mouseWheelEvent,
  dragEvent,
} = require("../../extension/lib/input-events.js");

test("mouse move distinguishes idle and held-button state", () => {
  assert.deepStrictEqual(mouseMoveEvent(10, 20, 2), {
    type: "mouseMoved",
    x: 10,
    y: 20,
    button: "none",
    modifiers: 2,
    buttons: 0,
    force: 0,
  });
  assert.deepStrictEqual(mouseMoveEvent(11, 21, 0, BUTTON_BITS.left), {
    type: "mouseMoved",
    x: 11,
    y: 21,
    button: "left",
    modifiers: 0,
    buttons: 1,
    force: 0.5,
  });
});

test("mouse button packets carry native button state and click count", () => {
  assert.deepStrictEqual(mouseButtonEvent("mousePressed", 1, 2, "right", 8, 2), {
    type: "mousePressed",
    x: 1,
    y: 2,
    button: "right",
    clickCount: 2,
    modifiers: 8,
    buttons: 2,
    force: 0.5,
  });
  const released = mouseButtonEvent("mouseReleased", 1, 2, "left", 0, 0);
  assert.equal(released.clickCount, 1);
  assert.equal(released.buttons, 0);
  assert.equal(released.force, 0);
});

test("wheel packet carries explicit neutral button state", () => {
  assert.deepStrictEqual(mouseWheelEvent(4, 5, -100, 300, 1), {
    type: "mouseWheel",
    x: 4,
    y: 5,
    button: "none",
    modifiers: 1,
    buttons: 0,
    force: 0,
    deltaX: -100,
    deltaY: 300,
  });
});

test("native drag packet preserves opaque CDP data", () => {
  const data = { items: [{ mimeType: "text/plain", data: "opaque" }], dragOperationsMask: 1 };
  assert.deepStrictEqual(dragEvent("dragEnter", 7, 8, data, 2), {
    type: "dragEnter",
    x: 7,
    y: 8,
    data,
    modifiers: 2,
  });
  assert.throws(() => dragEvent("dragStart", 0, 0, data, 0), /Unknown drag event type/);
});
