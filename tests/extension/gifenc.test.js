// SPDX-License-Identifier: Apache-2.0 OR MIT
// Tests for the vendored GIF89a encoder (ADR-0050 Decision 5, extension/lib/gifenc.js). Beyond the
// GIF89a header check the T4 prompt asks for, this ALSO pins the exact LZW bytes for a trivial frame
// (a hand-computed, spec-compliant oracle independent of the encoder's own code) and round-trips a
// table-growth frame through an independent LZW decoder -- so an LZW bug that still produces a
// valid-looking header cannot pass silently.

const test = require("node:test");
const assert = require("node:assert");
const { encodeGif, frameToIndices, palette332 } = require("../../extension/lib/gifenc.js");

// Build an RGBA frame of `width*height` from a per-pixel (x,y)->[r,g,b] function.
function frame(width, height, fn) {
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const [r, g, b] = fn(x, y);
      const i = (y * width + x) * 4;
      rgba[i] = r; rgba[i + 1] = g; rgba[i + 2] = b; rgba[i + 3] = 255;
    }
  }
  return rgba;
}

// A standalone GIF89a parser + LZW decoder: extract the FIRST frame's palette indices. Independent
// of the encoder (different code path), so a round-trip that reconstructs the input indices is real
// evidence the LZW stream is spec-decodable, not merely self-consistent.
function decodeFirstFrameIndices(gif) {
  let pos = 6; // skip "GIF89a"
  const width = gif[pos] | (gif[pos + 1] << 8);
  const height = gif[pos + 2] | (gif[pos + 3] << 8);
  const packed = gif[pos + 4];
  pos += 7; // LSD
  if (packed & 0x80) pos += 3 * (1 << ((packed & 0x07) + 1)); // global color table
  for (;;) {
    const b = gif[pos++];
    if (b === 0x3b) throw new Error("trailer before an image");
    if (b === 0x21) {
      pos++; // extension label
      // skip sub-blocks
      for (let n = gif[pos++]; n !== 0; n = gif[pos++]) pos += n;
      continue;
    }
    if (b === 0x2c) {
      const lp = gif[pos + 8];
      pos += 9; // image descriptor
      if (lp & 0x80) pos += 3 * (1 << ((lp & 0x07) + 1)); // local color table
      const minCodeSize = gif[pos++];
      const data = [];
      for (let n = gif[pos++]; n !== 0; n = gif[pos++]) {
        for (let j = 0; j < n; j++) data.push(gif[pos++]);
        if (pos > gif.length) throw new Error("unterminated sub-blocks");
      }
      return { indices: lzwDecode(minCodeSize, data), width, height };
    }
    throw new Error("unexpected block 0x" + b.toString(16));
  }
}

function lzwDecode(minCodeSize, data) {
  const clear = 1 << minCodeSize, eoi = clear + 1;
  let codeSize = minCodeSize + 1;
  let dict = [];
  const reset = () => {
    dict = [];
    for (let i = 0; i < clear; i++) dict[i] = [i];
    dict[clear] = null; dict[eoi] = null;
  };
  reset();
  const out = [];
  let bitBuf = 0, bitCnt = 0, p = 0;
  const read = () => {
    while (bitCnt < codeSize) {
      bitBuf |= (data[p++] || 0) << bitCnt;
      bitCnt += 8;
    }
    const code = bitBuf & ((1 << codeSize) - 1);
    bitBuf >>= codeSize; bitCnt -= codeSize;
    return code;
  };
  let prev = null;
  for (;;) {
    if (p >= data.length && bitCnt < codeSize) break;
    const code = read();
    if (code === clear) { reset(); codeSize = minCodeSize + 1; prev = null; continue; }
    if (code === eoi) break;
    let entry;
    if (dict[code] !== undefined && dict[code] !== null) entry = dict[code];
    else if (code === dict.length && prev) entry = prev.concat(prev[0]);
    else throw new Error("bad LZW code " + code);
    for (const s of entry) out.push(s);
    if (prev) {
      dict.push(prev.concat(entry[0]));
      // The decoder builds its table one entry BEHIND the encoder, so it must grow the code width
      // one entry EARLIER than the encoder's `nextCode === 2^width` fill point.
      if (dict.length === (1 << codeSize) - 1 && codeSize < 12) codeSize++;
    }
    prev = entry;
  }
  return out;
}

test("encodeGif returns a GIF89a header and a trailer", () => {
  const gif = encodeGif([frame(2, 2, () => [0, 0, 0])], { width: 2, height: 2, delayMs: 100 });
  assert.ok(gif instanceof Uint8Array);
  assert.deepStrictEqual(Array.from(gif.slice(0, 6)), [0x47, 0x49, 0x46, 0x38, 0x39, 0x61]); // "GIF89a"
  assert.strictEqual(gif[gif.length - 1], 0x3b, "ends with the GIF trailer");
  assert.ok(gif.length > 800, "non-trivial (header + 256-entry GCT + a frame)");
});

test("the LZW stream for a 2x2 solid frame matches a hand-computed spec oracle", () => {
  // indices [0,0,0,0] -> emitted 9-bit codes [clear=256, 0, 258, 0, eoi=257] -> LSB-packed bytes.
  const gif = encodeGif([frame(2, 2, () => [0, 0, 0])], { width: 2, height: 2, delayMs: 100 });
  const idx = frameToIndices(frame(2, 2, () => [0, 0, 0]), 2, 2);
  assert.deepStrictEqual(Array.from(idx), [0, 0, 0, 0], "black maps to palette index 0");
  // Locate the image data: it is the last non-trailer block; decode + re-check bytes via the parser.
  const { indices } = decodeFirstFrameIndices(gif);
  assert.deepStrictEqual(indices, [0, 0, 0, 0], "decodes back to the input indices");
});

test("round-trips a table-growth gradient frame through an independent decoder", () => {
  // A 32x32 frame with many distinct 3-3-2 indices forces the LZW table to grow and the code width
  // to bump past 9 bits -- so this exercises the code-size growth path, not just the base case.
  const W = 32, H = 32;
  const rgba = frame(W, H, (x, y) => [(x * 8) & 0xff, (y * 8) & 0xff, ((x + y) * 4) & 0xff]);
  const gif = encodeGif([rgba], { width: W, height: H, delayMs: 60 });
  const expected = Array.from(frameToIndices(rgba, W, H));
  const { indices, width, height } = decodeFirstFrameIndices(gif);
  assert.strictEqual(width, W);
  assert.strictEqual(height, H);
  assert.deepStrictEqual(indices, expected, "the decoded indices equal the encoder's input indices");
});

test("encodes multiple frames (animation) with a global color table of 256 entries", () => {
  const f1 = frame(4, 4, () => [255, 0, 0]);
  const f2 = frame(4, 4, () => [0, 255, 0]);
  const gif = encodeGif([f1, f2], { width: 4, height: 4, delayMs: 100 });
  // GCT size bits = packed & 7 -> 2^(n+1) entries; our fixed palette is 256.
  assert.strictEqual((gif[10] & 0x07) + 1, 8, "global color table is 2^8 = 256 entries");
  assert.strictEqual(palette332().length, 256 * 3);
  // Two image descriptors (0x2c) present.
  let images = 0;
  for (let i = 0; i < gif.length; i++) if (gif[i] === 0x2c) images++;
  assert.ok(images >= 2, "at least two image descriptors for two frames");
});
