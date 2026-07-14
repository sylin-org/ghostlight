// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- pure, policy-free narration placement (ADR-0072).
//
// The visual layer supplies recent interaction signals. This module only scores the two allowed
// viewport edges and returns one stable placement for the lifetime of a narration.
(function () {

const POINTER_FRESH_MS = 5000;
const TOUCHED_FRESH_MS = 15000;
const SCROLL_FRESH_MS = 6000;

function isFresh(signal, now, lifetimeMs) {
  return !!signal && Number.isFinite(signal.at) && now - signal.at <= lifetimeMs;
}

function edgeForY(y, viewportHeight) {
  if (!Number.isFinite(y) || !Number.isFinite(viewportHeight) || viewportHeight <= 0) return null;
  return y < viewportHeight / 2 ? "top" : "bottom";
}

function addPenalty(scores, edge, amount) {
  if (edge === "top" || edge === "bottom") scores[edge] += amount;
}

function chooseNarrationPosition(requested, context, now) {
  if (requested === "top" || requested === "bottom") return requested;

  const currentTime = Number.isFinite(now) ? now : Date.now();
  const state = context || {};
  const viewportHeight = state.viewportHeight;
  const scores = { top: 0, bottom: 0 };

  if (isFresh(state.touched, currentTime, TOUCHED_FRESH_MS)) {
    addPenalty(scores, edgeForY(state.touched.y, viewportHeight), 6);
  }
  if (isFresh(state.pointer, currentTime, POINTER_FRESH_MS)) {
    addPenalty(scores, edgeForY(state.pointer.y, viewportHeight), 3);
  }
  if (isFresh(state.scroll, currentTime, SCROLL_FRESH_MS)) {
    if (state.scroll.direction === "down") scores.bottom += 2;
    if (state.scroll.direction === "up") scores.top += 2;
  }

  return scores.top < scores.bottom ? "top" : "bottom";
}

const GhostlightNarrationPlacement = {
  POINTER_FRESH_MS,
  TOUCHED_FRESH_MS,
  SCROLL_FRESH_MS,
  chooseNarrationPosition,
};

if (typeof self !== "undefined") self.GhostlightNarrationPlacement = GhostlightNarrationPlacement;
if (typeof module !== "undefined" && module.exports) module.exports = GhostlightNarrationPlacement;

})();
