// SPDX-License-Identifier: Apache-2.0 OR MIT
// Tests for the pure gif_creator overlay geometry + routing (extension/lib/gifoverlay.js). The canvas
// draws themselves are live-verified in the service worker; here we pin the reference-derived math and
// the options gating that decide WHAT gets drawn where.

const test = require("node:test");
const assert = require("node:assert");
const O = require("../../extension/lib/gifoverlay.js");

test("resolveOverlayOptions defaults every switch to true, disables only on explicit false", () => {
  assert.deepStrictEqual(O.resolveOverlayOptions(undefined), {
    showClickIndicators: true, showDragPaths: true, showActionLabels: true,
    showProgressBar: true, showWatermark: true,
  });
  const r = O.resolveOverlayOptions({ showWatermark: false, showProgressBar: false });
  assert.strictEqual(r.showWatermark, false);
  assert.strictEqual(r.showProgressBar, false);
  assert.strictEqual(r.showClickIndicators, true, "unspecified stays true");
  // A non-boolean truthy value is not a literal false, so it stays enabled.
  assert.strictEqual(O.resolveOverlayOptions({ showClickIndicators: 0 }).showClickIndicators, true);
});

test("describeAction builds metadata for computer clicks, drags, typing, and navigation", () => {
  const click = O.describeAction("computer", { action: "left_click", coordinate: [100, 200] });
  assert.strictEqual(click.type, "left_click");
  assert.deepStrictEqual(click.coordinate, [100, 200]);
  assert.strictEqual(click.description, "left_click");

  const drag = O.describeAction("computer", { action: "left_click_drag", start_coordinate: [10, 20], coordinate: [30, 40] });
  assert.deepStrictEqual(drag.start_coordinate, [10, 20]);
  assert.deepStrictEqual(drag.coordinate, [30, 40]);

  const typed = O.describeAction("computer", { action: "type", text: "hello world" });
  assert.strictEqual(typed.description, "type: hello world");
  assert.ok(!typed.coordinate, "typing has no coordinate");

  const longType = O.describeAction("computer", { action: "type", text: "x".repeat(50) });
  assert.ok(longType.description.endsWith("..."), "long text is truncated");

  const key = O.describeAction("computer", { action: "key", text: "Enter" });
  assert.strictEqual(key.description, "key: Enter");

  const nav = O.describeAction("navigate", { url: "https://example.com/path?q=1" });
  assert.strictEqual(nav.type, "navigate");
  assert.strictEqual(nav.description, "navigate: example.com");

  assert.strictEqual(O.describeAction("read_page", {}), null, "un-annotated tools return null");
});

test("scaleFactorFor divides canvas width by viewport width, falling back to 1", () => {
  assert.strictEqual(O.scaleFactorFor(1200, 600), 2);
  assert.strictEqual(O.scaleFactorFor(800, 0), 1, "no viewport -> 1");
  assert.strictEqual(O.scaleFactorFor(0, 600), 1, "no canvas -> 1");
});

test("labelBox offsets up-right, then flips left when it would overflow the right edge", () => {
  // Comfortably inside: label goes to the right of the anchor (x + 20*sf).
  const inside = O.labelBox(100, 100, 50, 1000, 1);
  assert.strictEqual(inside.bgX, 120);
  assert.strictEqual(inside.bgW, 50 + 16, "text + 2*padding");
  assert.strictEqual(inside.textX, 120 + 8);

  // Near the right edge: bgX flips to the left of the anchor.
  const edge = O.labelBox(980, 100, 50, 1000, 1);
  assert.ok(edge.bgX < 980, "flipped left of the anchor");

  // Near the top edge: bgY drops below the anchor instead of going negative.
  const top = O.labelBox(100, 5, 50, 1000, 1);
  assert.ok(top.bgY >= 0, "label stays on-canvas vertically");
});

test("progressBarRect spans the width at the bottom, filled to the progress fraction", () => {
  const bar = O.progressBarRect(400, 300, 0.25, 1);
  assert.strictEqual(bar.x, 0);
  assert.strictEqual(bar.width, 400);
  assert.strictEqual(bar.height, 4);
  assert.strictEqual(bar.y, 300 - 4, "anchored to the bottom edge");
  assert.strictEqual(bar.fillWidth, 100, "25% of 400");
});

test("clickRadii scales the reference radii", () => {
  assert.deepStrictEqual(O.clickRadii(1), { outer: 15, inner: 11, border: 11, lineWidth: 2 });
  assert.deepStrictEqual(O.clickRadii(2), { outer: 30, inner: 22, border: 22, lineWidth: 4 });
});

test("computeFrameDelays clamps real deltas and holds the last frame", () => {
  // Deltas: 250 kept as-is; 50 clamps up to 100; 7700 clamps down to 4000; the last frame always
  // plays 800 + 2000 = 2800 ms (the official extension's end-of-animation hold).
  assert.deepStrictEqual(O.computeFrameDelays([1000, 1250, 1300, 9000]), [250, 100, 4000, 2800]);
  assert.deepStrictEqual(O.computeFrameDelays([5000]), [2800], "single frame gets the hold");
  assert.deepStrictEqual(O.computeFrameDelays([]), []);
  // A non-monotonic clock (negative delta) clamps up to the minimum instead of going backwards.
  assert.deepStrictEqual(O.computeFrameDelays([2000, 1500]), [100, 2800]);
});

test("overlayPlan routes each action type to the right overlays", () => {
  // Click -> ring + label near it.
  const click = O.overlayPlan({ type: "left_click", coordinate: [50, 60], description: "left_click" }, {});
  assert.deepStrictEqual(click.clickRing, { x: 50, y: 60 });
  assert.strictEqual(click.dragPath, null);
  assert.deepStrictEqual(click.label, { text: "left_click", x: 50, y: 60, topLeft: false });

  // Drag -> path (+ ring, since the reference click branch also fires for a "click"-containing type).
  const drag = O.overlayPlan({ type: "left_click_drag", start_coordinate: [1, 2], coordinate: [3, 4], description: "drag" }, {});
  assert.deepStrictEqual(drag.dragPath, { sx: 1, sy: 2, ex: 3, ey: 4 });
  assert.deepStrictEqual(drag.clickRing, { x: 3, y: 4 });

  // Typing -> top-left label only.
  const typed = O.overlayPlan({ type: "type", description: "type: hi" }, {});
  assert.strictEqual(typed.clickRing, null);
  assert.deepStrictEqual(typed.label, { text: "type: hi", x: 20, y: 20, topLeft: true });

  // Options gate: click indicators off -> no ring (and no label, since the label rides the ring).
  const gated = O.overlayPlan({ type: "left_click", coordinate: [5, 5], description: "x" }, { showClickIndicators: false });
  assert.strictEqual(gated.clickRing, null);
  assert.strictEqual(gated.label, null);

  // Labels off -> ring stays, label drops.
  const noLabel = O.overlayPlan({ type: "left_click", coordinate: [5, 5], description: "x" }, { showActionLabels: false });
  assert.deepStrictEqual(noLabel.clickRing, { x: 5, y: 5 });
  assert.strictEqual(noLabel.label, null);

  // No metadata -> empty plan.
  assert.deepStrictEqual(O.overlayPlan(null, {}), { clickRing: null, dragPath: null, label: null });
});
