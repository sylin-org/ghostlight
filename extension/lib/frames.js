// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- frame-scoped locator identity and cross-frame result merging.
//
// The extension observes every frame (ADR-0138), but each frame instance owns a private
// element registry keyed by plain `locator_N`. This module is the one place that binds a
// locator to the frame that mints it and the one place that folds per-frame results back
// into the single document a model sees. The scoped form never leaves the extension: the
// bridge and orchestrator treat locators as opaque strings behind TargetHandle.
(function installGhostlightFrames(root, factory) {
  const api = factory();
  root.GhostlightFrames = api;
  if (typeof module !== "undefined" && module.exports) {
    module.exports = api;
  }
})(globalThis, function createGhostlightFrames() {
  "use strict";

  const TOP_FRAME_ID = 0;
  const SCOPED_LOCATOR = /^(0|[1-9][0-9]*):(locator_.+)$/;

  function scopedLocator(frameId, local) {
    return `${frameId}:${local}`;
  }

  // Returns the owning frame id, or null when the handle predates frame scoping or is not
  // a locator at all. Callers treat null as the top frame only where a legacy handle is
  // still legal; every fresh mint is scoped.
  function frameOf(handle) {
    const match = SCOPED_LOCATOR.exec(String(handle ?? ""));
    return match ? Number(match[1]) : null;
  }

  function localOf(handle) {
    const match = SCOPED_LOCATOR.exec(String(handle ?? ""));
    return match ? match[2] : String(handle ?? "");
  }

  // Stamps every observed target in a fulfilled per-frame result with its minting frame.
  function scopeTargets(frameId, targets) {
    return (targets ?? []).map((target) => ({ ...target, locator: scopedLocator(frameId, target.locator) }));
  }

  // Merges per-frame target lists in stable frame order, top frame first, under one total
  // ceiling. Frame keys arrive as strings from object enumeration; sort numerically.
  function mergeTargets(perFrame, maximum) {
    const merged = [];
    const frameIds = Object.keys(perFrame).map(Number).sort((left, right) => left - right);
    for (const frameId of frameIds) {
      for (const target of perFrame[frameId]) {
        if (merged.length >= maximum) return merged;
        merged.push(target);
      }
    }
    return merged;
  }

  // Groups locator-bearing request fields by owning frame, preserving first-appearance
  // order of the frames and, inside a group, the caller's field order. Throws on an
  // unscoped handle rather than silently routing it to the top frame.
  function groupLocators(handles) {
    const groups = new Map();
    for (const handle of handles) {
      const frameId = frameOf(handle);
      if (frameId === null) throw new Error("target handle is not frame-scoped");
      if (!groups.has(frameId)) groups.set(frameId, []);
      groups.get(frameId).push({ handle, local: localOf(handle) });
    }
    return groups;
  }

  return Object.freeze({
    TOP_FRAME_ID,
    scopedLocator,
    frameOf,
    localOf,
    scopeTargets,
    mergeTargets,
    groupLocators
  });
});
