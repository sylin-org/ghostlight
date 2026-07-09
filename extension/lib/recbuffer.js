// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- gif_creator recording buffer (ADR-0050 Decision 5). A pure, bounded per-tab frame
// store: `start` opens a recording (active) and clears prior frames, `capture` appends a frame while
// active (dropping the oldest beyond MAX_FRAMES), `stop` freezes it (frames kept, no more capture),
// `clear` discards it. No DOM, no chrome.*, no timers -- so the state machine is unit-testable under
// node --test; the service worker owns the actual screenshot capture and GIF encoding.
//
// IIFE-wrapped and exposed as a namespace per lib/constants.js's pattern (idempotent under MV3
// worker re-evaluation; loadable as a worker global via importScripts and under node --test).
(function () {
  "use strict";

  var MAX_FRAMES = 100;

  // A recording store keyed by tabId. Each entry: { frames: [...], active: bool }.
  function createStore(maxFrames) {
    return { byTab: new Map(), max: maxFrames || MAX_FRAMES };
  }

  // Begin (or restart) recording for `tabId`: mark active, drop any prior frames, seed with
  // `firstFrame` when one is supplied. Returns the new frame count.
  function start(store, tabId, firstFrame) {
    var frames = firstFrame === undefined ? [] : [firstFrame];
    store.byTab.set(tabId, { frames: frames, active: true });
    return frames.length;
  }

  // Append a frame while `tabId` is actively recording; a no-op (returns -1) otherwise. Bounds the
  // buffer to `store.max`, dropping the oldest frame beyond it. Returns the frame count.
  function capture(store, tabId, frame) {
    var rec = store.byTab.get(tabId);
    if (!rec || !rec.active) return -1;
    rec.frames.push(frame);
    while (rec.frames.length > store.max) rec.frames.shift();
    return rec.frames.length;
  }

  // Stop recording `tabId` (keep the frames, stop capturing). Returns the frame count, or -1 if none.
  function stop(store, tabId) {
    var rec = store.byTab.get(tabId);
    if (!rec) return -1;
    rec.active = false;
    return rec.frames.length;
  }

  // Discard `tabId`'s recording entirely.
  function clear(store, tabId) {
    store.byTab.delete(tabId);
  }

  // The captured frames for `tabId` (empty array if none).
  function frames(store, tabId) {
    var rec = store.byTab.get(tabId);
    return rec ? rec.frames : [];
  }

  var GhostlightRecbuffer = { createStore: createStore, start: start, capture: capture, stop: stop, clear: clear, frames: frames, MAX_FRAMES: MAX_FRAMES };
  if (typeof module !== "undefined" && module.exports) {
    module.exports = GhostlightRecbuffer;
  } else {
    self.GhostlightRecbuffer = GhostlightRecbuffer;
  }
})();
