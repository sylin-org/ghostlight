// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/geometry.js (screenshot sizing and coordinate rescaling).

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const {
  targetDims,
  zoomScale,
  rescaleCtxCoord,
  clampRegionToViewport,
} = require("../../extension/lib/geometry.js");

test("targetDims passes small viewports through", () => {
  assert.deepStrictEqual(targetDims(1280, 720), { w: 1280, h: 720 });
});

test("targetDims shrinks to the token budget", () => {
  assert.deepStrictEqual(targetDims(1920, 1080), { w: 1466, h: 824 });
});

test("targetDims clamps the longest side", () => {
  assert.deepStrictEqual(targetDims(4000, 100), { w: 1568, h: 39 });
});

test("targetDims never returns zero", () => {
  assert.deepStrictEqual(targetDims(1, 1), { w: 1, h: 1 });
});

test("zoomScale magnifies a small region within budget", () => {
  const s = zoomScale(100, 100);
  assert.ok(10.8 < s && s < 10.9, `s = ${s}`);
  assert.ok(Math.ceil(Math.round(100 * s) / 28) ** 2 <= 1568);
});

test("zoomScale shrinks a large region to the budget edge", () => {
  const s = zoomScale(2000, 1000);
  assert.strictEqual(Math.round(2000 * s), 1568);
  assert.strictEqual(Math.round(1000 * s), 784);
});

test("rescaleCtxCoord passthrough without context", () => {
  assert.deepStrictEqual(rescaleCtxCoord(null, 10.4, 20.6), [10, 21]);
});

test("rescaleCtxCoord maps screenshot px to viewport px", () => {
  assert.deepStrictEqual(
    rescaleCtxCoord({ vpW: 1280, vpH: 720, shotW: 1024, shotH: 576 }, 512, 288),
    [640, 360]
  );
});

test("rescaleCtxCoord adds zoom region offsets", () => {
  assert.deepStrictEqual(
    rescaleCtxCoord(
      { vpW: 1280, vpH: 720, shotW: 800, shotH: 600, offX: 100, offY: 50, regionW: 400, regionH: 300 },
      400,
      300
    ),
    [300, 200]
  );
});

test("clampRegionToViewport returns only non-empty capturable rectangles", () => {
  assert.deepStrictEqual(clampRegionToViewport([10, 20, 70, 80], 100, 100), {
    x0: 10, y0: 20, x1: 70, y1: 80, w: 60, h: 60, clamped: false,
  });
  assert.deepStrictEqual(clampRegionToViewport([-10, -20, 70, 120], 100, 100), {
    x0: 0, y0: 0, x1: 70, y1: 100, w: 70, h: 100, clamped: true,
  });
  assert.strictEqual(clampRegionToViewport([110, 10, 120, 20], 100, 100), null);
  assert.strictEqual(clampRegionToViewport([-20, 10, -10, 20], 100, 100), null);
  assert.strictEqual(clampRegionToViewport([0, 0, 0, 20], 100, 100), null);
});

test("worker reports a clamp-empty zoom as a tool failure before capture", () => {
  const worker = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  const start = worker.indexOf("async function zoomScreenshot");
  const end = worker.indexOf("// --- Input helpers ---", start);
  const zoom = worker.slice(start, end);
  const guard = zoom.indexOf("if (!clipped)");
  const capture = zoom.indexOf('Page.captureScreenshot');
  assert.ok(start >= 0 && end > start && guard >= 0 && capture > guard);
  assert.match(zoom, /if \(!clipped\) \{\s*throw hopError\("page", "zoom region is empty or entirely outside the visible viewport"\);/);
  assert.doesNotMatch(zoom, /return \{ error: "zoom region is empty/);
  assert.doesNotMatch(worker, /if \(z\.error\) return text/);
});
