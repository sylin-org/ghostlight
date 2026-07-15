// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- pure, policy-free placement for narration and action signatures (ADRs 0072/0083).
//
// The visual layer supplies recent interaction and occupied-presentation signals. This module only
// scores fixed viewport anchors and returns one stable placement for a presentation's lifetime.
(function () {

const POINTER_FRESH_MS = 5000;
const TOUCHED_FRESH_MS = 15000;
const FOCUSED_FRESH_MS = 15000;
const SCROLL_FRESH_MS = 6000;
const SIGNATURE_POSITIONS = ["bottom-right", "bottom-left", "top-right", "top-left"];

function isFresh(signal, now, lifetimeMs) {
  return !!signal && Number.isFinite(signal.at) && now - signal.at <= lifetimeMs;
}

function edgeForY(y, viewportHeight) {
  if (!Number.isFinite(y) || !Number.isFinite(viewportHeight) || viewportHeight <= 0) return null;
  return y < viewportHeight / 2 ? "top" : "bottom";
}

function edgeForPosition(position) {
  if (typeof position !== "string") return null;
  if (position.startsWith("top")) return "top";
  if (position.startsWith("bottom")) return "bottom";
  return null;
}

function addEdgePenalty(scores, edge, amount) {
  if (edge === "top" || edge === "bottom") scores[edge] += amount;
}

function chooseNarrationPosition(requested, context, now) {
  if (requested === "top" || requested === "bottom") return requested;

  const currentTime = Number.isFinite(now) ? now : Date.now();
  const state = context || {};
  const viewportHeight = state.viewportHeight;
  const scores = { top: 0, bottom: 0 };

  if (isFresh(state.touched, currentTime, TOUCHED_FRESH_MS)) {
    addEdgePenalty(scores, edgeForY(state.touched.y, viewportHeight), 6);
  }
  if (isFresh(state.focused, currentTime, FOCUSED_FRESH_MS)) {
    addEdgePenalty(scores, edgeForY(state.focused.y, viewportHeight), 5);
  }
  if (isFresh(state.pointer, currentTime, POINTER_FRESH_MS)) {
    addEdgePenalty(scores, edgeForY(state.pointer.y, viewportHeight), 3);
  }
  if (isFresh(state.scroll, currentTime, SCROLL_FRESH_MS)) {
    if (state.scroll.direction === "down") scores.bottom += 2;
    if (state.scroll.direction === "up") scores.top += 2;
  }
  addEdgePenalty(scores, edgeForPosition(state.signaturePosition), 5);

  return scores.top < scores.bottom ? "top" : "bottom";
}

function quadrantForPoint(point, viewportWidth, viewportHeight) {
  if (!point || !Number.isFinite(point.x) || !Number.isFinite(point.y) ||
      !Number.isFinite(viewportWidth) || viewportWidth <= 0 ||
      !Number.isFinite(viewportHeight) || viewportHeight <= 0) return null;
  const vertical = point.y < viewportHeight / 2 ? "top" : "bottom";
  const horizontal = point.x < viewportWidth / 2 ? "left" : "right";
  return `${vertical}-${horizontal}`;
}

function addPointPenalty(scores, point, amount, viewportWidth, viewportHeight) {
  const occupied = quadrantForPoint(point, viewportWidth, viewportHeight);
  if (!occupied) return;
  const [vertical, horizontal] = occupied.split("-");
  for (const position of SIGNATURE_POSITIONS) {
    const [candidateVertical, candidateHorizontal] = position.split("-");
    if (candidateVertical === vertical) scores[position] += amount * 0.35;
    if (candidateHorizontal === horizontal) scores[position] += amount * 0.2;
    if (position === occupied) scores[position] += amount;
  }
}

function chooseSignaturePosition(context, now) {
  const currentTime = Number.isFinite(now) ? now : Date.now();
  const state = context || {};
  const scores = Object.fromEntries(SIGNATURE_POSITIONS.map((position) => [position, 0]));

  if (isFresh(state.touched, currentTime, TOUCHED_FRESH_MS)) {
    addPointPenalty(scores, state.touched, 8, state.viewportWidth, state.viewportHeight);
  }
  if (isFresh(state.focused, currentTime, FOCUSED_FRESH_MS)) {
    addPointPenalty(scores, state.focused, 7, state.viewportWidth, state.viewportHeight);
  }
  if (isFresh(state.pointer, currentTime, POINTER_FRESH_MS)) {
    addPointPenalty(scores, state.pointer, 4, state.viewportWidth, state.viewportHeight);
  }
  const narrationEdge = edgeForPosition(state.narrationPosition);
  if (narrationEdge) {
    for (const position of SIGNATURE_POSITIONS) {
      if (edgeForPosition(position) === narrationEdge) scores[position] += 6;
    }
  }
  if (isFresh(state.scroll, currentTime, SCROLL_FRESH_MS)) {
    const busyEdge = state.scroll.direction === "down" ? "bottom"
      : state.scroll.direction === "up" ? "top" : null;
    if (busyEdge) {
      for (const position of SIGNATURE_POSITIONS) {
        if (edgeForPosition(position) === busyEdge) scores[position] += 2;
      }
    }
  }

  let best = SIGNATURE_POSITIONS[0];
  for (const position of SIGNATURE_POSITIONS.slice(1)) {
    if (scores[position] < scores[best]) best = position;
  }
  return best;
}

const GhostlightPresentationPlacement = {
  POINTER_FRESH_MS,
  TOUCHED_FRESH_MS,
  FOCUSED_FRESH_MS,
  SCROLL_FRESH_MS,
  SIGNATURE_POSITIONS,
  chooseNarrationPosition,
  chooseSignaturePosition,
};

if (typeof self !== "undefined") self.GhostlightPresentationPlacement = GhostlightPresentationPlacement;
if (typeof module !== "undefined" && module.exports) module.exports = GhostlightPresentationPlacement;

})();
