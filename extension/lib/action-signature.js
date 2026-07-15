// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- fixed, content-free action-signature vocabulary (ADR-0083).
//
// Both the service worker and the isolated-world renderer use this domain module. Messages carry
// only a fixed kind and phase; arbitrary tool arguments and page content cannot enter this lane.
(function () {

const TYPE = "AGENT_ACTION_SIGNATURE";
const CHANNEL = "action-signature";
const KINDS = Object.freeze({
  JAVASCRIPT: "javascript",
  TYPING: "typing",
  WAIT: "wait",
  SCREENSHOT: "screenshot",
});
const PHASES = Object.freeze({
  START: "start",
  FINISH: "finish",
  CONFIRM: "confirm",
});
const KIND_VALUES = Object.freeze(Object.values(KINDS));
const PHASE_VALUES = Object.freeze(Object.values(PHASES));

function message(kind, phase) {
  if (!KIND_VALUES.includes(kind)) throw new Error(`unknown action signature kind: ${kind}`);
  if (!PHASE_VALUES.includes(phase)) throw new Error(`unknown action signature phase: ${phase}`);
  return { type: TYPE, kind, phase };
}

function isMessage(value) {
  return !!value && value.type === TYPE && KIND_VALUES.includes(value.kind) &&
    PHASE_VALUES.includes(value.phase);
}

const GhostlightActionSignature = Object.freeze({
  TYPE,
  CHANNEL,
  KINDS,
  PHASES,
  message,
  isMessage,
});

if (typeof self !== "undefined") self.GhostlightActionSignature = GhostlightActionSignature;
if (typeof module !== "undefined" && module.exports) module.exports = GhostlightActionSignature;

})();
