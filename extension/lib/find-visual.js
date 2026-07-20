// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- fixed, content-free find-presentation vocabulary (ADR-0086).
//
// The broker carries only lifecycle and aggregate result state. Query text, matched text, DOM
// references, and geometry stay inside the page's isolated world.
(function () {

const TYPE = "AGENT_FIND_VISUAL";
const CHANNEL = "find-visual";
const MAX_RESULTS = 20;
const PHASES = Object.freeze({
  START: "start",
  FOUND: "found",
  EMPTY: "empty",
  CANCEL: "cancel",
});
const PHASE_VALUES = Object.freeze(Object.values(PHASES));

function message(phase, count, more) {
  if (!PHASE_VALUES.includes(phase)) throw new Error(`unknown find visual phase: ${phase}`);
  if (phase === PHASES.FOUND) {
    if (!Number.isInteger(count) || count < 1 || count > MAX_RESULTS) {
      throw new Error(`find visual count must be between 1 and ${MAX_RESULTS}`);
    }
    return { type: TYPE, phase, count, more: !!more };
  }
  if (phase === PHASES.EMPTY) {
    return { type: TYPE, phase, count: 0, more: false };
  }
  return { type: TYPE, phase };
}

function isMessage(value) {
  if (!value || value.type !== TYPE || !PHASE_VALUES.includes(value.phase)) return false;
  if (value.phase === PHASES.FOUND) {
    return Number.isInteger(value.count) && value.count >= 1 && value.count <= MAX_RESULTS &&
      typeof value.more === "boolean";
  }
  if (value.phase === PHASES.EMPTY) return value.count === 0 && value.more === false;
  return true;
}

const GhostlightFindVisual = Object.freeze({
  TYPE,
  CHANNEL,
  MAX_RESULTS,
  PHASES,
  message,
  isMessage,
});

if (typeof self !== "undefined") self.GhostlightFindVisual = GhostlightFindVisual;
if (typeof module !== "undefined" && module.exports) module.exports = GhostlightFindVisual;

})();
