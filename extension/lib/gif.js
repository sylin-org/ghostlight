// Ghostlight -- animated GIF composition for extension-owned recordings.
//
// The browser records the pixels, so the browser encodes them (ADR-0109). Frames never cross a
// process boundary: this module turns retained JPEGs into one finished GIF inside the browser,
// and only that GIF can be handed anywhere.
//
// Decoding a JPEG needs a document, so it arrives injected. Everything else here is pure, which
// is what lets the fit loop and the duration arithmetic be tested without a browser.
(function installGhostlightGif(root, factory) {
  const api = factory();
  root.GhostlightGif = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightGifApi() {
  "use strict";

  // GIF stores a frame delay in hundredths of a second, and browsers treat anything under two
  // of them as "as fast as possible". Clamping keeps playback honest at both ends.
  const MIN_FRAME_DELAY_MS = 20;
  const MAX_FRAME_DELAY_MS = 0xffff * 10;
  const MAX_PIXELS = 8_000_000;
  const PALETTE_COLORS = 256;
  const PALETTE_FORMAT = "rgb565";
  const MIME_TYPE = "image/gif";
  const FRAME_MIME_TYPE = "image/jpeg";
  // Below three frames there is no intermediate frame to drop, so the fit loop cannot make
  // progress and must say so rather than spin.
  const MIN_THINNABLE_FRAMES = 3;

  function failure(reason) {
    return Object.assign(new Error(reason), { code: "recording_export_failed" });
  }

  function requireDimensions(width, height) {
    if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height)
      || width < 1 || height < 1 || width > 0xffff || height > 0xffff
      || width * height > MAX_PIXELS) {
      throw failure("recording dimensions are unsupported");
    }
  }

  function delayMs(frame) {
    const duration = Number(frame.duration_ms);
    if (!Number.isFinite(duration)) return MIN_FRAME_DELAY_MS;
    return Math.min(MAX_FRAME_DELAY_MS, Math.max(MIN_FRAME_DELAY_MS, Math.round(duration)));
  }

  // A recording can span a window resize, so later frames may not match the first one's shape.
  // Nearest-neighbour is enough: the alternative is refusing a recording the caller already made.
  function resized(image, width, height) {
    if (image.width === width && image.height === height) return image;
    const source = image.data;
    const data = new Uint8ClampedArray(width * height * 4);
    for (let y = 0; y < height; y += 1) {
      const sourceY = Math.floor(y * image.height / height);
      for (let x = 0; x < width; x += 1) {
        const sourceX = Math.floor(x * image.width / width);
        const from = (sourceY * image.width + sourceX) * 4;
        const to = (y * width + x) * 4;
        data[to] = source[from];
        data[to + 1] = source[from + 1];
        data[to + 2] = source[from + 2];
        data[to + 3] = source[from + 3];
      }
    }
    return { data, width, height };
  }

  function create({
    encoder = globalThis.gifenc,
    thinFrames = globalThis.GhostlightRecording?.thinFrames,
    decode
  } = {}) {
    if (!encoder?.GIFEncoder) throw new TypeError("gif composition requires the GIF encoder");
    if (typeof thinFrames !== "function") throw new TypeError("gif composition requires the recording fidelity policy");
    if (typeof decode !== "function") throw new TypeError("gif composition requires a frame decoder");

    function compose(frames, width, height) {
      const stream = encoder.GIFEncoder();
      for (const [index, frame] of frames.entries()) {
        const pixels = frame.image.data;
        const palette = encoder.quantize(pixels, PALETTE_COLORS, { format: PALETTE_FORMAT });
        const indexed = encoder.applyPalette(pixels, palette, PALETTE_FORMAT);
        stream.writeFrame(indexed, width, height, {
          palette,
          delay: delayMs(frame),
          repeat: index === 0 ? 0 : undefined
        });
      }
      stream.finish();
      return stream.bytes();
    }

    /**
     * Encode retained recording frames into one animated GIF that fits `maxBytes`.
     *
     * Over budget, fidelity is traded rather than coverage: intermediate frames are dropped and
     * their time folded into the frame before them, so the replay still spans the whole recording
     * and still plays for as long as the work took.
     */
    async function encode(frames, { maxBytes }) {
      if (!Array.isArray(frames) || frames.length === 0) throw failure("recording has no frames");
      if (!Number.isSafeInteger(maxBytes) || maxBytes < 1) throw new TypeError("gif composition requires a byte budget");

      const decoded = [];
      for (const frame of frames) {
        if (frame.mime_type !== FRAME_MIME_TYPE) {
          throw failure(`unsupported recording frame type ${frame.mime_type}`);
        }
        decoded.push({ duration_ms: frame.duration_ms, image: await decode(frame.data) });
      }
      const { width, height } = decoded[0].image;
      requireDimensions(width, height);

      let kept = decoded.map((frame) => ({ ...frame, image: resized(frame.image, width, height) }));
      for (;;) {
        const bytes = compose(kept, width, height);
        if (bytes.length <= maxBytes) {
          return {
            bytes,
            mime_type: MIME_TYPE,
            frame_count: kept.length,
            captured_frame_count: frames.length,
            duration_ms: kept.reduce((total, frame) => total + delayMs(frame), 0),
            width,
            height,
            byte_count: bytes.length
          };
        }
        if (kept.length < MIN_THINNABLE_FRAMES) {
          throw failure(`recording GIF exceeds ${maxBytes} bytes at its lowest fidelity`);
        }
        kept = thinFrames(kept).kept;
      }
    }

    return Object.freeze({ encode });
  }

  return Object.freeze({
    MIME_TYPE, MIN_FRAME_DELAY_MS, MAX_FRAME_DELAY_MS, MAX_PIXELS, MIN_THINNABLE_FRAMES, create
  });
});
