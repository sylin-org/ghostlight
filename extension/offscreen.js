// Ghostlight -- the document that decodes recording frames and encodes the animated GIF.
//
// This runs off the service worker deliberately. Chrome may evict an MV3 worker in the middle of
// a long encode, and an offscreen document is the sanctioned place for canvas and blob work
// (ADR-0109). It holds no recording state: the service worker hands it frames, it hands back one
// finished GIF, and it forgets.
"use strict";

const OBJECT_URLS = new Set();

/** Decode one base64 JPEG frame into raw RGBA pixels. */
async function decodeFrame(data) {
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
  const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/jpeg" }));
  try {
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) throw new Error("recording frame could not be rasterized");
    context.drawImage(bitmap, 0, 0);
    const image = context.getImageData(0, 0, bitmap.width, bitmap.height);
    return { data: image.data, width: image.width, height: image.height };
  } finally {
    bitmap.close();
  }
}

const composer = globalThis.GhostlightGif.create({ decode: decodeFrame });

function encodeBase64(bytes) {
  let binary = "";
  // Chunked so a multi-megabyte GIF does not blow the argument limit of String.fromCharCode.
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode.apply(null, bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

// The GIF materializes in exactly one form per destination: an object URL the browser can
// download without anyone reading the bytes, or base64 for a caller that must receive them.
function deliverable(encoded, transfer) {
  if (transfer === "object_url") {
    const url = URL.createObjectURL(new Blob([encoded.bytes], { type: encoded.mime_type }));
    OBJECT_URLS.add(url);
    return { object_url: url };
  }
  return { data: encodeBase64(encoded.bytes) };
}

function release(url) {
  if (!OBJECT_URLS.delete(url)) return false;
  URL.revokeObjectURL(url);
  return true;
}

chrome.runtime.onMessage.addListener((message, _sender, respond) => {
  if (message?.target !== "ghostlight-offscreen") return false;
  if (message.kind === "release_recording") {
    respond({ ok: true, released: release(message.object_url) });
    return false;
  }
  if (message.kind !== "encode_recording") {
    respond({ ok: false, reason: `unsupported offscreen request ${message.kind}` });
    return false;
  }
  composer
    .encode(message.frames, { maxBytes: message.max_bytes })
    .then((encoded) => {
      respond({
        ok: true,
        measurements: {
          frame_count: encoded.frame_count,
          captured_frame_count: encoded.captured_frame_count,
          duration_ms: encoded.duration_ms,
          width: encoded.width,
          height: encoded.height,
          byte_count: encoded.byte_count
        },
        mime_type: encoded.mime_type,
        ...deliverable(encoded, message.transfer)
      });
    })
    .catch((error) => respond({ ok: false, reason: String(error?.message ?? error) }));
  return true;
});
