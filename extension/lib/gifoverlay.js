// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- gif_creator visual-overlay geometry + routing (ADR-0050 Decision 5, refinement).
//
// PURE, node-testable helpers for the click ring / drag path / action label / progress bar overlays
// composited onto GIF frames. The overlay VOCABULARY and GEOMETRY are harvested from the official
// Claude-in-Chrome v1.0.80 offscreen.js (drawClickIndicator/drawActionLabel/drawProgressBar/
// applyActionIndicators): same radii, label box, edge-clamping, and scaleFactor = canvasWidth /
// viewportWidth. Two deliberate divergences from the reference: the actual canvas drawing stays in
// the service worker on our lean inline OffscreenCanvas (the reference uses an offscreen document +
// gif.js), and the palette is recolored to Ghostlight sky-blue (#38BDF8) instead of Claude coral.
// This module holds only the math that can be verified without a canvas; the draw calls live in
// service-worker.js (encodeRecording) and are live-verified.
//
// IIFE-wrapped and exposed as a namespace per lib/constants.js's pattern (idempotent under MV3 worker
// re-evaluation; loadable as a worker global via importScripts and under node --test).
(function () {
  "use strict";

  // Ghostlight brand color (sky-blue), replacing the reference's Claude-coral overlays.
  var BRAND_RGB = [56, 189, 248]; // #38BDF8

  function truncate(s, n) {
    s = String(s);
    return s.length > n ? s.slice(0, n) + "..." : s;
  }

  function shortUrl(u) {
    try {
      var h = new URL(u).host;
      return h || truncate(u, 30);
    } catch (e) {
      return truncate(u, 30);
    }
  }

  // A human label for a computer action (used as the on-frame caption).
  function labelForComputer(args) {
    var a = args.action || "action";
    if (a === "type" && typeof args.text === "string") return "type: " + truncate(args.text, 30);
    if (a === "key" && typeof args.text === "string") return "key: " + args.text;
    return a;
  }

  // Build per-frame action METADATA from a dispatched tool call. Returns null for tools we do not
  // annotate. `coordinate`/`start_coordinate` are copied through as-is (in whatever space the caller
  // holds); the service worker rescales them to CSS viewport px before storing on the frame.
  function describeAction(tool, args) {
    args = args || {};
    if (tool === "navigate") {
      return { type: "navigate", description: "navigate" + (args.url ? ": " + shortUrl(args.url) : "") };
    }
    if (tool === "computer") {
      var m = { type: args.action || "action", description: labelForComputer(args) };
      if (Array.isArray(args.coordinate) && args.coordinate.length === 2) {
        m.coordinate = [args.coordinate[0], args.coordinate[1]];
      }
      if (Array.isArray(args.start_coordinate) && args.start_coordinate.length === 2) {
        m.start_coordinate = [args.start_coordinate[0], args.start_coordinate[1]];
      }
      return m;
    }
    return null;
  }

  // Resolve the open `options` object into concrete booleans. Every switch defaults to true (the
  // reference's `?? true`); only an explicit `false` disables an overlay.
  function resolveOverlayOptions(options) {
    var o = options || {};
    return {
      showClickIndicators: o.showClickIndicators !== false,
      showDragPaths: o.showDragPaths !== false,
      showActionLabels: o.showActionLabels !== false,
      showProgressBar: o.showProgressBar !== false,
      showWatermark: o.showWatermark !== false,
    };
  }

  // scaleFactor maps CSS viewport px -> canvas (screenshot) px. Falls back to 1 with no viewport info.
  function scaleFactorFor(canvasWidth, viewportWidth) {
    return viewportWidth && canvasWidth ? canvasWidth / viewportWidth : 1;
  }

  // Click-ring radii (reference geometry: outer glow r=15, filled inner r=11, border r=11, lw=2).
  function clickRadii(sf) {
    return { outer: 15 * sf, inner: 11 * sf, border: 11 * sf, lineWidth: 2 * sf };
  }

  // Action-label box geometry (reference drawActionLabel): 14px font, 8px padding, 20px text height,
  // 6px corner radius, offset up-right of the anchor, edge-clamped against the right/top canvas edges.
  // `textWidth` is measured by the caller (impure); everything else is pure.
  function labelBox(x, y, textWidth, canvasWidth, sf) {
    var fontSize = 14 * sf;
    var textHeight = 20 * sf;
    var padding = 8 * sf;
    var radius = 6 * sf;

    var labelX = x + 20 * sf;
    var labelY = y - 10 * sf;
    if (labelX + textWidth + padding * 2 > canvasWidth) {
      labelX = x - textWidth - padding * 2 - 20 * sf;
    }
    if (labelY < 0) {
      labelY = y + 20 * sf;
    }

    return {
      bgX: labelX,
      bgY: labelY,
      bgW: textWidth + padding * 2,
      bgH: textHeight + padding,
      radius: radius,
      textX: labelX + padding,
      textY: labelY + padding,
      fontSize: fontSize,
    };
  }

  // Progress-bar rect (reference drawProgressBar): full width, 4px tall, anchored to the bottom edge.
  function progressBarRect(canvasWidth, canvasHeight, progress, sf) {
    var height = 4 * sf;
    return {
      x: 0,
      y: canvasHeight - height,
      width: canvasWidth,
      height: height,
      fillWidth: canvasWidth * progress,
    };
  }

  // Per-frame GIF delays from real capture timestamps (ADR-0052 D3). Frame i plays for the time
  // that actually elapsed until frame i+1, clamped to [100, 4000] ms (a stuck clock or an hour-long
  // pause must not freeze the GIF); the last frame holds 800 + 2000 ms -- the official extension's
  // end-of-animation viewing pause.
  var MIN_FRAME_DELAY_MS = 100;
  var MAX_FRAME_DELAY_MS = 4000;
  var LAST_FRAME_DELAY_MS = 800 + 2000;
  function computeFrameDelays(timestamps) {
    var out = [];
    for (var i = 0; i < timestamps.length; i++) {
      if (i + 1 < timestamps.length) {
        var d = timestamps[i + 1] - timestamps[i];
        out.push(Math.min(MAX_FRAME_DELAY_MS, Math.max(MIN_FRAME_DELAY_MS, d)));
      } else {
        out.push(LAST_FRAME_DELAY_MS);
      }
    }
    return out;
  }

  // Decide WHICH overlays a frame gets, mirroring the reference applyActionIndicators routing:
  //   click/scroll with a coordinate -> ring (+ label near it);
  //   left_click_drag with both coords -> drag path (+ label near the end);
  //   type/key/wait with no coordinate -> a top-left label.
  // Coordinates are returned in the meta's own space (CSS viewport px); the draw layer scales them.
  function overlayPlan(meta, options) {
    var o = resolveOverlayOptions(options);
    var plan = { clickRing: null, dragPath: null, label: null };
    if (!meta) return plan;
    var type = meta.type || "";
    var isClick = type.indexOf("click") !== -1 || type === "scroll";

    if (o.showClickIndicators && meta.coordinate && isClick) {
      plan.clickRing = { x: meta.coordinate[0], y: meta.coordinate[1] };
      if (o.showActionLabels && meta.description) {
        plan.label = { text: meta.description, x: meta.coordinate[0], y: meta.coordinate[1], topLeft: false };
      }
    }

    if (o.showDragPaths && type === "left_click_drag" && meta.start_coordinate && meta.coordinate) {
      plan.dragPath = {
        sx: meta.start_coordinate[0], sy: meta.start_coordinate[1],
        ex: meta.coordinate[0], ey: meta.coordinate[1],
      };
      if (o.showActionLabels && meta.description) {
        plan.label = { text: meta.description, x: meta.coordinate[0], y: meta.coordinate[1], topLeft: false };
      }
    }

    if (o.showActionLabels && meta.description && !meta.coordinate &&
        (type === "type" || type === "key" || type === "wait")) {
      plan.label = { text: meta.description, x: 20, y: 20, topLeft: true };
    }

    return plan;
  }

  var GhostlightGifoverlay = {
    BRAND_RGB: BRAND_RGB,
    describeAction: describeAction,
    resolveOverlayOptions: resolveOverlayOptions,
    scaleFactorFor: scaleFactorFor,
    clickRadii: clickRadii,
    labelBox: labelBox,
    progressBarRect: progressBarRect,
    overlayPlan: overlayPlan,
    computeFrameDelays: computeFrameDelays,
  };
  if (typeof module !== "undefined" && module.exports) {
    module.exports = GhostlightGifoverlay;
  } else {
    self.GhostlightGifoverlay = GhostlightGifoverlay;
  }
})();
