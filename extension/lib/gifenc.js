// SPDX-License-Identifier: MIT
// Ghostlight -- vendored minimal animated-GIF89a encoder (ADR-0050 Decision 5, gif_creator).
//
// Self-contained ASCII JS: MV3 forbids remote code, so the encoder ships IN the extension package
// (never fetched at runtime). It reduces each RGBA frame with a FIXED 3-3-2 (RRRGGGBB) uniform
// 256-color palette -- deterministic, always <= 256 colors, no per-frame quantization -- then LZW-
// compresses the index stream per the GIF89a variable-width LZW discipline. This is a coarse but
// always-valid FLOOR (Phase 1); richer color quantization and overlays (click cues, labels,
// watermark, progress bar) are DEFERRED (see the T4 LEDGER entry). The LZW code-size / clear
// discipline follows the GIF89a spec (and the omggif MIT approach): codes start at minCodeSize+1
// bits, grow when the next code index reaches 2^codeSize, and a Clear code resets the table at 4096.
//
// IIFE-wrapped and exposed as a namespace per lib/constants.js's pattern (idempotent under MV3
// worker re-evaluation; loadable as a content-script/worker global and under node --test).
(function () {
  "use strict";

  var MIN_CODE_SIZE = 8; // 256-entry global color table

  // The fixed 3-3-2 palette: 256 RGB triples; index = (r>>5)<<5 | (g>>5)<<2 | (b>>6).
  function palette332() {
    var p = new Uint8Array(256 * 3);
    for (var i = 0; i < 256; i++) {
      var r3 = (i >> 5) & 7, g3 = (i >> 2) & 7, b2 = i & 3;
      p[i * 3] = Math.round((r3 * 255) / 7);
      p[i * 3 + 1] = Math.round((g3 * 255) / 7);
      p[i * 3 + 2] = Math.round((b2 * 255) / 3);
    }
    return p;
  }

  // Map a whole RGBA frame (Uint8Array/Uint8ClampedArray, length w*h*4) to 3-3-2 palette indices.
  function frameToIndices(rgba, width, height) {
    var n = width * height;
    var out = new Uint8Array(n);
    for (var i = 0; i < n; i++) {
      var r = rgba[i * 4], g = rgba[i * 4 + 1], b = rgba[i * 4 + 2];
      out[i] = ((r >> 5) << 5) | ((g >> 5) << 2) | (b >> 6);
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

  // encodeGif(frames, {width, height, delayMs}) -> Uint8Array.
  //   frames: array of RGBA byte arrays, each of length width*height*4.
  //   delayMs: per-frame delay in milliseconds (GIF stores centiseconds; min 2cs like most encoders).
  // Returns a complete animated GIF89a (looping forever) as a Uint8Array.
  function encodeGif(frames, opts) {
    var width = opts.width, height = opts.height;
    var delayCs = Math.max(2, Math.round((opts.delayMs || 100) / 10));
    var bytes = [];

    // Header + Logical Screen Descriptor (global color table present, 256 entries).
    pushStr(bytes, "GIF89a");
    pushU16LE(bytes, width);
    pushU16LE(bytes, height);
    bytes.push(0xf7); // GCT flag=1, color res=7, sort=0, GCT size=7 (2^8 = 256)
    bytes.push(0x00); // background color index
    bytes.push(0x00); // pixel aspect ratio

    // Global Color Table.
    var pal = palette332();
    for (var p = 0; p < pal.length; p++) bytes.push(pal[p]);

    // NETSCAPE2.0 application extension: loop forever.
    bytes.push(0x21, 0xff, 0x0b);
    pushStr(bytes, "NETSCAPE2.0");
    bytes.push(0x03, 0x01, 0x00, 0x00, 0x00); // sub-block: loop count 0 (infinite), terminator

    for (var f = 0; f < frames.length; f++) {
      // Graphic Control Extension: disposal=1 (leave in place), no transparency, delay.
      bytes.push(0x21, 0xf9, 0x04, 0x04);
      pushU16LE(bytes, delayCs);
      bytes.push(0x00, 0x00); // transparent color index (unused), block terminator

      // Image Descriptor: full frame, no local color table.
      bytes.push(0x2c);
      pushU16LE(bytes, 0);
      pushU16LE(bytes, 0);
      pushU16LE(bytes, width);
      pushU16LE(bytes, height);
      bytes.push(0x00);

      // Image data: LZW minimum code size byte, then LZW sub-blocks.
      var indices = frameToIndices(frames[f], width, height);
      bytes.push(MIN_CODE_SIZE);
      pushSubBlocks(bytes, lzwEncode(indices));
    }

    bytes.push(0x3b); // trailer
    return new Uint8Array(bytes);
  }

  var GhostlightGifenc = { encodeGif: encodeGif, frameToIndices: frameToIndices, palette332: palette332 };
  if (typeof module !== "undefined" && module.exports) {
    module.exports = GhostlightGifenc;
  } else {
    self.GhostlightGifenc = GhostlightGifenc;
  }
})();
