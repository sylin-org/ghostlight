// Ghostlight -- bounded screenshot geometry shared by the service worker and tests.
(function installGhostlightScreenshot(root, factory) {
  const api = factory();
  root.GhostlightScreenshot = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightScreenshotApi() {
  "use strict";

  const MAX_SIDE = 2400;
  const MAX_PIXELS = 4_000_000;
  const MIN_SCALE = 0.05;
  const JPEG_QUALITY = 55;
  const FALLBACK_JPEG_QUALITY = 30;
  const MAX_BASE64_CHARS = 6_000_000;

  function requireExtent(value, name) {
    if (!Number.isFinite(value) || value <= 0) throw new TypeError(`${name} must be positive`);
    return value;
  }

  function requireCoordinate(value, name) {
    if (!Number.isFinite(value) || value < 0) throw new TypeError(`${name} must be non-negative`);
    return value;
  }

  function outputScale(width, height, magnify) {
    requireExtent(width, "width");
    requireExtent(height, "height");
    const budget = Math.min(
      MAX_SIDE / width,
      MAX_SIDE / height,
      Math.sqrt(MAX_PIXELS / (width * height))
    );
    return Math.max(MIN_SCALE, magnify ? budget : Math.min(1, budget));
  }

  function ordinaryClip(x, y, width, height) {
    return {
      x: requireCoordinate(x, "x"),
      y: requireCoordinate(y, "y"),
      width: requireExtent(width, "width"),
      height: requireExtent(height, "height"),
      scale: outputScale(width, height, false)
    };
  }

  function regionClip(region) {
    if (!region || typeof region !== "object") throw new TypeError("region is required");
    const x = requireCoordinate(region.x, "x");
    const y = requireCoordinate(region.y, "y");
    const width = requireExtent(region.width, "width");
    const height = requireExtent(region.height, "height");
    return { x, y, width, height, scale: outputScale(width, height, true) };
  }

  return Object.freeze({
    MAX_SIDE,
    MAX_PIXELS,
    MIN_SCALE,
    JPEG_QUALITY,
    FALLBACK_JPEG_QUALITY,
    MAX_BASE64_CHARS,
    outputScale,
    ordinaryClip,
    regionClip
  });
});
