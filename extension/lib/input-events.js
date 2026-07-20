// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- pure CDP pointer-event descriptors.
//
// IIFE-wrapped because importScripts shares the service-worker global lexical scope. The worker
// orchestrates timing and dispatch; this module owns the complete packet invariants.
(function () {
const BUTTON_BITS = Object.freeze({ left: 1, right: 2, middle: 4 });

function pointerForce(buttons) {
  return buttons === 0 ? 0 : 0.5;
}

function activeButton(buttons) {
  if ((buttons & BUTTON_BITS.left) !== 0) return "left";
  if ((buttons & BUTTON_BITS.right) !== 0) return "right";
  if ((buttons & BUTTON_BITS.middle) !== 0) return "middle";
  return "none";
}

function mouseMoveEvent(x, y, modifiers, buttons) {
  const held = Number.isInteger(buttons) ? buttons : 0;
  return {
    type: "mouseMoved",
    x,
    y,
    button: activeButton(held),
    modifiers: modifiers || 0,
    buttons: held,
    force: pointerForce(held),
  };
}

function mouseButtonEvent(type, x, y, button, modifiers, clickCount) {
  const pressed = type === "mousePressed";
  const buttons = pressed ? (BUTTON_BITS[button] || 0) : 0;
  return {
    type,
    x,
    y,
    button,
    clickCount: Math.max(1, Number.isInteger(clickCount) ? clickCount : 1),
    modifiers: modifiers || 0,
    buttons,
    force: pointerForce(buttons),
  };
}

function mouseWheelEvent(x, y, deltaX, deltaY, modifiers) {
  return {
    type: "mouseWheel",
    x,
    y,
    button: "none",
    modifiers: modifiers || 0,
    buttons: 0,
    force: 0,
    deltaX,
    deltaY,
  };
}

function dragEvent(type, x, y, data, modifiers) {
  if (!["dragEnter", "dragOver", "drop", "dragCancel"].includes(type)) {
    throw new Error(`Unknown drag event type: ${type}`);
  }
  return {
    type,
    x,
    y,
    data,
    modifiers: modifiers || 0,
  };
}

const GhostlightInputEvents = {
  BUTTON_BITS,
  mouseMoveEvent,
  mouseButtonEvent,
  mouseWheelEvent,
  dragEvent,
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightInputEvents;
} else {
  self.GhostlightInputEvents = GhostlightInputEvents;
}
})();
