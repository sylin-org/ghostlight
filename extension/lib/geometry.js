// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- screenshot geometry: token/side budget sizing and coordinate rescaling.
//
// IIFE-wrapped so its internal const/function bindings stay function-scoped, not global lexical
// bindings in the service worker (importScripts shares the worker's global scope; a top-level const
// here would collide with a re-import or a consumer's binding and fail worker registration). Only
// the export assignment is global, and reassigning it is idempotent. The budget tunables come from
// lib/constants.js (the single source), read here via importScripts order (worker) or require (Node).
(function () {
const C = (typeof module !== "undefined" && module.exports) ? require("./constants.js") : self.GhostlightConstants;
const { PX_PER_TOKEN, MAX_TOKENS, MAX_SIDE } = C;

// Target screenshot dimensions (derived from the CSS viewport) under the token + longest-side budget.
function targetDims(vpW, vpH) {
  let w = vpW, h = vpH;
  const tokens = Math.ceil(w / PX_PER_TOKEN) * Math.ceil(h / PX_PER_TOKEN);
  if (tokens > MAX_TOKENS) { const s = Math.sqrt(MAX_TOKENS / tokens); w = Math.round(w * s); h = Math.round(h * s); }
  const longest = Math.max(w, h);
  if (longest > MAX_SIDE) { const s = MAX_SIDE / longest; w = Math.round(w * s); h = Math.round(h * s); }
  return { w: Math.max(1, w), h: Math.max(1, h) };
}
// Largest capture scale for a region of CSS size w x h that keeps the output inside the token +
// longest-side budget; magnifies a small region, shrinks a large one.
function zoomScale(w, h) {
  let s = Math.min(MAX_SIDE / Math.max(w, h), Math.sqrt((MAX_TOKENS * PX_PER_TOKEN * PX_PER_TOKEN) / (w * h)));
  while (s > 0 && Math.ceil(Math.round(w * s) / PX_PER_TOKEN) * Math.ceil(Math.round(h * s) / PX_PER_TOKEN) > MAX_TOKENS) s *= 0.98;
  return s;
}
// Map a model-provided coordinate (read off the downscaled screenshot) back to CSS viewport px,
// given the context record `c` (or null/incomplete, in which case this is a passthrough round).
// A zoomed capture carries a region offset (offX, offY) that the mapped point is added back onto.
function rescaleCtxCoord(c, x, y) {
  if (!c || !c.shotW || !c.shotH) return [Math.round(x), Math.round(y)];
  const rw = c.regionW || c.vpW, rh = c.regionH || c.vpH;
  return [Math.round((c.offX || 0) + (x * rw) / c.shotW), Math.round((c.offY || 0) + (y * rh) / c.shotH)];
}

const GhostlightGeometry = { targetDims, zoomScale, rescaleCtxCoord };
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightGeometry;
} else {
  self.GhostlightGeometry = GhostlightGeometry;
}
})();
