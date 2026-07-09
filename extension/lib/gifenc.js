// SPDX-License-Identifier: MIT
// Ghostlight -- vendored minimal animated-GIF89a encoder (ADR-0050 Decision 5, gif_creator).
//
// Self-contained ASCII JS: MV3 forbids remote code, so the encoder ships IN the extension package
// (never fetched at runtime). It reduces the frames with an ADAPTIVE 256-color GLOBAL palette learned
// from the actual pixels via NeuQuant (lib/neuquant.js -- the same quantizer the official
// Claude-in-Chrome extension ships in gif.js), then LZW-compresses each frame's index stream per the
// GIF89a variable-width LZW discipline. NeuQuant is deterministic (no Math.random) and reference-
// standard; it replaces the coarse fixed 3-3-2 uniform palette the Phase-1 encoder used, which
// banded photographic screenshots badly. One global color table is built from all frames (bounded
// training sample) so the animation shares a single GCT, and every pixel is mapped to its nearest
// palette entry via the network's own search. The LZW code-size / clear discipline follows the GIF89a
// spec (and the omggif MIT approach): codes start at minCodeSize+1 bits, grow when the next code index
// reaches 2^codeSize, and a Clear code resets the table at 4096.
//
// IIFE-wrapped and exposed as a namespace per lib/constants.js's pattern (idempotent under MV3
// worker re-evaluation; loadable as a content-script/worker global and under node --test).
(function () {
  "use strict";

  var MIN_CODE_SIZE = 8; // 256-entry global color table

  // NeuQuant quantizer -- required in node, a worker global under importScripts (see lib/neuquant.js).
  var Neuquant =
    typeof module !== "undefined" && module.exports
      ? require("./neuquant.js")
      : self.GhostlightNeuquant;

  var DEFAULT_SAMPLE_FAC = 10; // gif.js's default quality; 1=best/slow .. 30=coarse/fast
  var TRAIN_PIXEL_BUDGET = 500000; // cap pixels fed to NeuQuant (bounds memory + time; deterministic)

  // Build ONE adaptive 256-color palette from all frames. `frames` is an array of RGBA byte buffers
  // (each length width*height*4). Returns { palette: Uint8Array(768) RGB triples, lookup: (r,g,b)->idx }.
  // Training pixels are sub-sampled with a deterministic stride so a long recording stays bounded;
  // NeuQuant then sub-samples that buffer again per `sampleFac`. With no frames, returns NeuQuant's
  // default gray-ramp palette (still a valid GCT).
  function buildGlobalPalette(frames, width, height, sampleFac) {
    var fac = sampleFac || DEFAULT_SAMPLE_FAC;
    var perFrame = width * height;
    var totalPixels = frames.length * perFrame;
    var stride = totalPixels > TRAIN_PIXEL_BUDGET ? Math.floor(totalPixels / TRAIN_PIXEL_BUDGET) : 1;
    if (stride < 1) stride = 1;

    // Pack a strided RGB (alpha-stripped) training buffer across every frame.
    var sampled = stride > 1 ? Math.ceil(totalPixels / stride) : totalPixels;
    var train = new Uint8Array(sampled * 3);
    var w = 0; // write cursor into train (byte index)
    var g = 0; // global pixel counter across all frames
    for (var f = 0; f < frames.length; f++) {
      var rgba = frames[f];
      for (var p = 0; p < perFrame; p++, g++) {
        if (g % stride !== 0) continue;
        var s = p * 4;
        train[w++] = rgba[s];
        train[w++] = rgba[s + 1];
        train[w++] = rgba[s + 2];
      }
    }
    // train may be slightly over-allocated by rounding; trim so NeuQuant's length math is exact.
    if (w < train.length) train = train.subarray(0, w);

    var nq = new Neuquant.NeuQuant(train, fac);
    nq.buildColormap();
    var map = nq.getColormap(); // flat [r,g,b,...] * 256, values 0..255
    var palette = new Uint8Array(256 * 3);
    for (var i = 0; i < palette.length; i++) palette[i] = map[i] & 0xff;
    return { palette: palette, lookup: nq.lookupRGB };
  }

  // Map a whole RGBA frame (Uint8Array/Uint8ClampedArray, length w*h*4) to palette indices via the
  // NeuQuant nearest-color search built by buildGlobalPalette. Lookups are memoized per distinct
  // color: screenshots hold thousands of distinct colors, not millions, so the cache converts the
  // dominant per-pixel network search into a map hit -- measured ~34x faster with byte-identical
  // output, the difference between a service worker that stalls mid-export and one that breathes
  // (ADR-0052 Decision 3).
  function quantizeFrame(rgba, width, height, lookup) {
    var n = width * height;
    var out = new Uint8Array(n);
    var cache = new Map();
    for (var i = 0; i < n; i++) {
      var s = i * 4;
      var key = (rgba[s] << 16) | (rgba[s + 1] << 8) | rgba[s + 2];
      var v = cache.get(key);
      if (v === undefined) {
        v = lookup(rgba[s], rgba[s + 1], rgba[s + 2]);
        cache.set(key, v);
      }
      out[i] = v;
    }
    return out;
  }

  // GIF variable-width LZW encode of an index array (each 0..255). Returns an array of bytes.
  function lzwEncode(indices) {
    var clearCode = 1 << MIN_CODE_SIZE;
    var eoiCode = clearCode + 1;
    var codeSize = MIN_CODE_SIZE + 1;
    var nextCode = eoiCode + 1;
    var table = new Map();

    var out = [];
    var bitBuf = 0, bitCnt = 0;
    function emit(code) {
      bitBuf |= code << bitCnt;
      bitCnt += codeSize;
      while (bitCnt >= 8) {
        out.push(bitBuf & 0xff);
        bitBuf >>= 8;
        bitCnt -= 8;
      }
    }

    emit(clearCode);
    if (indices.length === 0) {
      emit(eoiCode);
      if (bitCnt > 0) out.push(bitBuf & 0xff);
      return out;
    }
    var prefix = indices[0];
    for (var i = 1; i < indices.length; i++) {
      var k = indices[i];
      var mapKey = prefix * 4096 + k; // prefix is a CODE (<=4095), k a symbol (<=255): unique key
      if (table.has(mapKey)) {
        prefix = table.get(mapKey);
      } else {
        emit(prefix);
        table.set(mapKey, nextCode);
        nextCode++;
        if (nextCode === (1 << codeSize) && codeSize < 12) {
          codeSize++;
        }
        if (nextCode > 4095) {
          emit(clearCode);
          table = new Map();
          codeSize = MIN_CODE_SIZE + 1;
          nextCode = eoiCode + 1;
        }
        prefix = k;
      }
    }
    emit(prefix);
    emit(eoiCode);
    if (bitCnt > 0) out.push(bitBuf & 0xff);
    return out;
  }

  function pushU16LE(arr, v) {
    arr.push(v & 0xff, (v >> 8) & 0xff);
  }
  function pushStr(arr, s) {
    for (var i = 0; i < s.length; i++) arr.push(s.charCodeAt(i) & 0xff);
  }
  // Split LZW bytes into GIF sub-blocks (<=255 each, length-prefixed), terminated by a 0x00 block.
  function pushSubBlocks(arr, bytes) {
    var i = 0;
    while (i < bytes.length) {
      var n = Math.min(255, bytes.length - i);
      arr.push(n);
      for (var j = 0; j < n; j++) arr.push(bytes[i + j]);
      i += n;
    }
    arr.push(0x00);
  }

  // encodeGif(frames, {width, height, delayMs, delays, sampleFac}) -> Uint8Array.
  //   frames: array of RGBA byte arrays, each of length width*height*4.
  //   delayMs: uniform per-frame delay in milliseconds (GIF stores centiseconds; min 2cs like most
  //     encoders).
  //   delays: optional per-frame delay array in ms (ADR-0052 D3: real capture timing); each index
  //     overrides delayMs for that frame.
  //   sampleFac: optional NeuQuant quality/sampling factor (default 10; 1=best/slow, 30=coarse/fast).
  // Returns a complete animated GIF89a (looping forever) as a Uint8Array.
  function encodeGif(frames, opts) {
    var width = opts.width, height = opts.height;
    function delayCsFor(f) {
      var ms = opts.delays && opts.delays[f] !== undefined ? opts.delays[f] : opts.delayMs || 100;
      return Math.max(2, Math.round(ms / 10));
    }
    var bytes = [];

    // Learn one adaptive 256-color global palette from all frames' pixels.
    var quant = buildGlobalPalette(frames, width, height, opts.sampleFac);

    // Header + Logical Screen Descriptor (global color table present, 256 entries).
    pushStr(bytes, "GIF89a");
    pushU16LE(bytes, width);
    pushU16LE(bytes, height);
    bytes.push(0xf7); // GCT flag=1, color res=7, sort=0, GCT size=7 (2^8 = 256)
    bytes.push(0x00); // background color index
    bytes.push(0x00); // pixel aspect ratio

    // Global Color Table (the adaptive NeuQuant palette).
    for (var p = 0; p < quant.palette.length; p++) bytes.push(quant.palette[p]);

    // NETSCAPE2.0 application extension: loop forever.
    bytes.push(0x21, 0xff, 0x0b);
    pushStr(bytes, "NETSCAPE2.0");
    bytes.push(0x03, 0x01, 0x00, 0x00, 0x00); // sub-block: loop count 0 (infinite), terminator

    for (var f = 0; f < frames.length; f++) {
      // Graphic Control Extension: disposal=1 (leave in place), no transparency, per-frame delay.
      bytes.push(0x21, 0xf9, 0x04, 0x04);
      pushU16LE(bytes, delayCsFor(f));
      bytes.push(0x00, 0x00); // transparent color index (unused), block terminator

      // Image Descriptor: full frame, no local color table.
      bytes.push(0x2c);
      pushU16LE(bytes, 0);
      pushU16LE(bytes, 0);
      pushU16LE(bytes, width);
      pushU16LE(bytes, height);
      bytes.push(0x00);

      // Image data: LZW minimum code size byte, then LZW sub-blocks.
      var indices = quantizeFrame(frames[f], width, height, quant.lookup);
      bytes.push(MIN_CODE_SIZE);
      pushSubBlocks(bytes, lzwEncode(indices));
    }

    bytes.push(0x3b); // trailer
    return new Uint8Array(bytes);
  }

  var GhostlightGifenc = {
    encodeGif: encodeGif,
    buildGlobalPalette: buildGlobalPalette,
    quantizeFrame: quantizeFrame,
  };
  if (typeof module !== "undefined" && module.exports) {
    module.exports = GhostlightGifenc;
  } else {
    self.GhostlightGifenc = GhostlightGifenc;
  }
})();
