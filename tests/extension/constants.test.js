// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/constants.js (the extension's operational tunables). These pin
// the values and sanity-check their ranges, so a stray edit to a tunable is caught in CI.

const { test } = require("node:test");
const assert = require("node:assert");
const C = require("../../extension/lib/constants.js");

test("screenshot budget matches ADR-0010", () => {
  assert.strictEqual(C.PX_PER_TOKEN, 28);
  assert.strictEqual(C.MAX_TOKENS, 1568);
  assert.strictEqual(C.MAX_SIDE, 1568);
  assert.strictEqual(C.MAX_SCREENSHOT_B64, 1100000);
});

test("jpeg qualities are valid 0-100 integers, fallback below primary below full", () => {
  for (const q of [C.JPEG_QUALITY, C.JPEG_QUALITY_FALLBACK, C.JPEG_QUALITY_FULL]) {
    assert.ok(Number.isInteger(q) && q > 0 && q <= 100, `quality out of range: ${q}`);
  }
  assert.ok(C.JPEG_QUALITY_FALLBACK < C.JPEG_QUALITY, "fallback quality must be below primary");
  assert.ok(C.JPEG_QUALITY < C.JPEG_QUALITY_FULL, "primary quality must be below full-detail");
});

test("timing tunables are positive numbers", () => {
  for (const [name, v] of Object.entries({
    KEEPALIVE_PERIOD_MINUTES: C.KEEPALIVE_PERIOD_MINUTES,
    RECONNECT_DELAY_MS: C.RECONNECT_DELAY_MS,
    HOLD_REQUEST_TIMEOUT_MS: C.HOLD_REQUEST_TIMEOUT_MS,
    CAPTURE_SETTLE_MS: C.CAPTURE_SETTLE_MS,
    CLICK_GAP_MS: C.CLICK_GAP_MS,
    DRAG_INTERCEPT_GRACE_MS: C.DRAG_INTERCEPT_GRACE_MS,
    DRAG_INTERCEPT_WAIT_MS: C.DRAG_INTERCEPT_WAIT_MS,
    NAV_SETTLE_TIMEOUT_MS: C.NAV_SETTLE_TIMEOUT_MS,
  })) {
    assert.ok(typeof v === "number" && v > 0, `${name} must be a positive number, got ${v}`);
  }
  assert.ok(C.DRAG_INTERCEPT_GRACE_MS < C.DRAG_INTERCEPT_WAIT_MS);
});
