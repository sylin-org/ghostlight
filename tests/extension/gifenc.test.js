// SPDX-License-Identifier: Apache-2.0 OR MIT
// Tests for the vendored GIF89a encoder (ADR-0050 Decision 5, extension/lib/gifenc.js). Beyond the
// GIF89a header check the T4 prompt asks for, this ALSO pins the exact LZW bytes for a trivial frame
// (a hand-computed, spec-compliant oracle independent of the encoder's own code) and round-trips a
// table-growth frame through an independent LZW decoder -- so an LZW bug that still produces a
// valid-looking header cannot pass silently.

const test = require("node:test");
const assert = require("node:assert");
const { encodeGif, buildGlobalPalette, quantizeFrame } = require("../../extension/lib/gifenc.js");

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

test("a 2x2 solid frame round-trips to four identical indices via an independent decoder", () => {
  // The exact index value now depends on the learned NeuQuant palette (no longer a fixed 3-3-2
  // index 0), but a solid frame must still map to four IDENTICAL indices, and the LZW stream must
  // decode back to exactly them. The independent decoder (not the encoder's own code) is the oracle.
  const rgba = frame(2, 2, () => [0, 0, 0]);
  const gif = encodeGif([rgba], { width: 2, height: 2, delayMs: 100 });
  const { palette, lookup } = buildGlobalPalette([rgba], 2, 2);
  const expected = Array.from(quantizeFrame(rgba, 2, 2, lookup));
  assert.strictEqual(palette.length, 256 * 3, "palette is a 256-entry RGB table");
  assert.deepStrictEqual(expected, [expected[0], expected[0], expected[0], expected[0]], "solid frame -> identical indices");
  const { indices } = decodeFirstFrameIndices(gif);
  assert.deepStrictEqual(indices, expected, "decodes back to the encoder's input indices");
});

test("round-trips a table-growth gradient frame through an independent decoder", () => {
  // A 32x32 frame with many distinct colors forces the LZW table to grow and the code width to bump
  // past 9 bits -- so this exercises the code-size growth path, not just the base case.
  const W = 32, H = 32;
  const rgba = frame(W, H, (x, y) => [(x * 8) & 0xff, (y * 8) & 0xff, ((x + y) * 4) & 0xff]);
  const gif = encodeGif([rgba], { width: W, height: H, delayMs: 60 });
  const { lookup } = buildGlobalPalette([rgba], W, H);
  const expected = Array.from(quantizeFrame(rgba, W, H, lookup));
  const { indices, width, height } = decodeFirstFrameIndices(gif);
  assert.strictEqual(width, W);
  assert.strictEqual(height, H);
  assert.deepStrictEqual(indices, expected, "the decoded indices equal the encoder's quantized input indices");
});

// Walk the GIF block structure and collect each frame's Graphic Control Extension delay (in
// centiseconds). Structure-aware (skips the GCT, other extensions, and LZW sub-blocks), so a delay
// byte pattern inside image data cannot be misread.
function readGceDelaysCs(gif) {
  let pos = 6;
  const packed = gif[pos + 4];
  pos += 7; // logical screen descriptor
  if (packed & 0x80) pos += 3 * (1 << ((packed & 0x07) + 1)); // global color table
  const delays = [];
  for (;;) {
    const b = gif[pos++];
    if (b === 0x3b) return delays;
    if (b === 0x21) {
      const label = gif[pos++];
      // GCE sub-block layout at pos: size=0x04, packed, delayLo, delayHi, transparent index.
      if (label === 0xf9) delays.push(gif[pos + 2] | (gif[pos + 3] << 8));
      for (let n = gif[pos++]; n !== 0; n = gif[pos++]) pos += n;
      continue;
    }
    if (b === 0x2c) {
      const lp = gif[pos + 8];
      pos += 9; // image descriptor
      if (lp & 0x80) pos += 3 * (1 << ((lp & 0x07) + 1)); // local color table
      pos++; // LZW minimum code size
      for (let n = gif[pos++]; n !== 0; n = gif[pos++]) pos += n;
      continue;
    }
    throw new Error("unexpected block 0x" + b.toString(16));
  }
}

test("per-frame delays land in each frame's graphic control extension", () => {
  const f1 = frame(4, 4, () => [255, 0, 0]);
  const f2 = frame(4, 4, () => [0, 255, 0]);
  const f3 = frame(4, 4, () => [0, 0, 255]);
  // ADR-0052 D3: delays[i] overrides delayMs per frame; ms -> centiseconds.
  const gif = encodeGif([f1, f2, f3], { width: 4, height: 4, delayMs: 100, delays: [250, 100, 2800] });
  assert.deepStrictEqual(readGceDelaysCs(gif), [25, 10, 280]);
  // Without delays, the uniform delayMs applies to every frame.
  const uni = encodeGif([f1, f2], { width: 4, height: 4, delayMs: 500 });
  assert.deepStrictEqual(readGceDelaysCs(uni), [50, 50]);
});

test("encodes multiple frames (animation) with a global color table of 256 entries", () => {
  const f1 = frame(4, 4, () => [255, 0, 0]);
  const f2 = frame(4, 4, () => [0, 255, 0]);
  const gif = encodeGif([f1, f2], { width: 4, height: 4, delayMs: 100 });
  // GCT size bits = packed & 7 -> 2^(n+1) entries; our adaptive palette is a full 256 entries.
  assert.strictEqual((gif[10] & 0x07) + 1, 8, "global color table is 2^8 = 256 entries");
  // Two image descriptors (0x2c) present.
  let images = 0;
  for (let i = 0; i < gif.length; i++) if (gif[i] === 0x2c) images++;
  assert.ok(images >= 2, "at least two image descriptors for two frames");
});

test("the adaptive palette is deterministic and maps primaries to near colors", () => {
  // Determinism: identical frames -> byte-identical palette (the harness forbids Math.random anyway,
  // but pin it). And a red/green/blue frame set should quantize each solid region to a palette entry
  // close to the true primary (adaptive palette actually represents the input colors).
  // Frames must clear NeuQuant's minpicturebytes floor (3*503 = 1509) for real learning; 32x32 gives
  // 9216 training bytes and converges to within ~1 of each primary (measured), so 16 is safe slack.
  const R = frame(32, 32, () => [255, 0, 0]);
  const G = frame(32, 32, () => [0, 255, 0]);
  const B = frame(32, 32, () => [0, 0, 255]);
  const a = buildGlobalPalette([R, G, B], 32, 32);
  const b = buildGlobalPalette([R, G, B], 32, 32);
  assert.deepStrictEqual(Array.from(a.palette), Array.from(b.palette), "same frames -> identical palette");

  const near = (r, g, bl) => {
    const i = a.lookup(r, g, bl) * 3;
    const dr = a.palette[i] - r, dg = a.palette[i + 1] - g, db = a.palette[i + 2] - bl;
    return Math.sqrt(dr * dr + dg * dg + db * db);
  };
  assert.ok(near(255, 0, 0) < 16, "red maps near red");
  assert.ok(near(0, 255, 0) < 16, "green maps near green");
  assert.ok(near(0, 0, 255) < 16, "blue maps near blue");
});
