// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- policy-free owned-tab control result vocabulary (ADR-0078 D7).
(function () {

const OBSERVED_KEYS = {
  focus: "tabFocused",
  reload: "tabReloaded",
  close: "tabClosed",
};

function makeReceipt(action, page) {
  const key = OBSERVED_KEYS[action];
  if (!key) throw new Error(`unsupported tab action: ${action}`);
  return {
    targetAssurance: "none",
    action,
    observedAfter: { [key]: true },
    blockers: [],
    page: { ...(page || {}) },
    more: false,
  };
}

const GhostlightTabControl = { OBSERVED_KEYS, makeReceipt };
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightTabControl;
} else {
  self.GhostlightTabControl = GhostlightTabControl;
}
})();
