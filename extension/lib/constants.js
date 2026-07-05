// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- operational tunables for the extension: the one place for the numeric knobs that
// shape screenshot sizing, JPEG encoding, and timing. Pure data, no logic.
//
// IIFE-wrapped and exposed as a single namespace so it is idempotent under MV3 worker re-evaluation:
// importScripts shares the worker's global scope, so declaring these at top level would collide with
// a re-import or with a consumer's own binding (see lib/geometry.js for the failure this avoids).
// Only the export assignment touches the global, and reassigning it is harmless.
(function () {
const GhostlightConstants = {
  // Screenshot token/side budget (ADR-0010): a capture is downscaled so
  // ceil(w/PX_PER_TOKEN)*ceil(h/PX_PER_TOKEN) <= MAX_TOKENS and the longest side <= MAX_SIDE px.
  PX_PER_TOKEN: 28,
  MAX_TOKENS: 1568,
  MAX_SIDE: 1568,
  // Hard cap on a returned base64 JPEG; over this the pipeline re-encodes at the fallback quality.
  MAX_SCREENSHOT_B64: 1100000,

  // JPEG quality on the CDP Page.captureScreenshot scale (0-100). The canvas re-encode path uses the
  // same numbers divided by 100 (its scale is 0-1). Default 55, dropping to 30 above the size budget;
  // 80 for zoom / full-detail captures.
  JPEG_QUALITY: 55,
  JPEG_QUALITY_FALLBACK: 30,
  JPEG_QUALITY_FULL: 80,

  // Timing.
  KEEPALIVE_PERIOD_MINUTES: 0.4, // MV3 keepalive alarm period.
  RECONNECT_DELAY_MS: 2000,      // native-port reconnect backoff.
  HOLD_REQUEST_TIMEOUT_MS: 1500, // take-the-wheel hold query timeout.
  CAPTURE_SETTLE_MS: 40,         // wait after hiding the on-page indicator before a screenshot.
  CLICK_GAP_MS: 40,              // press/release and inter-click spacing.
  NAV_SETTLE_TIMEOUT_MS: 10000,  // max wait for a navigation to report complete.
};
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightConstants;
} else {
  self.GhostlightConstants = GhostlightConstants;
}
})();
