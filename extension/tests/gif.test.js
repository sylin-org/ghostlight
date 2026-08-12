"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const gifApi = require("../lib/gif.js");
const recordingApi = require("../lib/recording.js");
const gifenc = require("../vendor/gifenc.js");

const WIDTH = 8;
const HEIGHT = 8;

// A recording frame as the registry retains it. The data is a stand-in identity rather than real
// JPEG bytes: the decoder is injected, so what matters here is that each frame decodes to its own
// distinguishable pixels and carries its own visual duration.
function frame(shade, durationMs) {
  return { frame_kind: "screencast", duration_ms: durationMs, mime_type: "image/jpeg", data: `shade:${shade}` };
}

// Noise, not flat colour. A palette-based encoder compresses eight identical pixels to almost
// nothing, and then no budget is ever small enough to force thinning.
function pixels(shade, width = WIDTH, height = HEIGHT) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let index = 0; index < width * height; index += 1) {
    data[index * 4] = (shade * 37 + index * 11) % 256;
    data[index * 4 + 1] = (shade * 91 + index * 53) % 256;
    data[index * 4 + 2] = (shade * 17 + index * 29) % 256;
    data[index * 4 + 3] = 255;
  }
  return { data, width, height };
}

function composer(overrides = {}) {
  const decoded = [];
  const api = gifApi.create({
    encoder: gifenc,
    thinFrames: recordingApi.thinFrames,
    decode: async (data) => {
      decoded.push(data);
      const [, shade] = data.split(":");
      return pixels(Number(shade));
    },
    ...overrides
  });
  return { api, decoded };
}

const GENEROUS = 8 * 1024 * 1024;

test("an encoded recording is a real GIF89a animation", async () => {
  const { api } = composer();
  const encoded = await api.encode([frame(1, 100), frame(2, 250)], { maxBytes: GENEROUS });
  const header = Buffer.from(encoded.bytes.subarray(0, 6)).toString("latin1");
  assert.equal(header, "GIF89a");
  assert.equal(encoded.mime_type, "image/gif");
  assert.equal(encoded.frame_count, 2);
  assert.equal(encoded.captured_frame_count, 2);
  assert.equal(encoded.width, WIDTH);
  assert.equal(encoded.height, HEIGHT);
  assert.equal(encoded.byte_count, encoded.bytes.length);
});

test("playback time is the sum of what each frame was on screen for", async () => {
  const { api } = composer();
  const encoded = await api.encode([frame(1, 4_000), frame(2, 26_000)], { maxBytes: GENEROUS });
  assert.equal(encoded.duration_ms, 30_000);
});

test("a frame with no measured time still plays long enough to be seen", async () => {
  // GIF stores hundredths of a second and browsers treat zero as "as fast as possible", so a
  // final frame captured at stop would otherwise vanish from the replay.
  const { api } = composer();
  const encoded = await api.encode([frame(1, 1_000), frame(2, 0)], { maxBytes: GENEROUS });
  assert.equal(encoded.duration_ms, 1_020);
});

test("a tight budget trades fidelity and keeps the whole span", async () => {
  const { api } = composer();
  const captured = [frame(1, 100), frame(2, 100), frame(3, 100), frame(4, 100), frame(5, 700)];
  const generous = await api.encode(captured, { maxBytes: GENEROUS });

  const thinned = await api.encode(captured, { maxBytes: Math.floor(generous.byte_count / 2) });
  assert.ok(thinned.frame_count < generous.frame_count, "a tight budget must drop frames");
  assert.ok(thinned.byte_count <= Math.floor(generous.byte_count / 2), "thinning must reach the budget");
  assert.equal(thinned.captured_frame_count, captured.length, "the caller still learns what was captured");
  // Coverage survives: a thinned replay still plays for as long as the work took, because the
  // time of every dropped frame was folded into the frame before it.
  assert.equal(thinned.duration_ms, generous.duration_ms);
});

test("a budget nothing can meet is a decisive refusal, not a truncated replay", async () => {
  const { api } = composer();
  await assert.rejects(
    () => api.encode([frame(1, 100), frame(2, 100), frame(3, 100)], { maxBytes: 64 }),
    (error) => error.code === "recording_export_failed" && /lowest fidelity/.test(error.message)
  );
});

test("a recording with no frames refuses instead of encoding nothing", async () => {
  const { api } = composer();
  await assert.rejects(
    () => api.encode([], { maxBytes: GENEROUS }),
    (error) => error.code === "recording_export_failed" && /no frames/.test(error.message)
  );
});

test("only JPEG frames are accepted", async () => {
  const { api } = composer();
  await assert.rejects(
    () => api.encode([{ ...frame(1, 100), mime_type: "image/png" }], { maxBytes: GENEROUS }),
    (error) => error.code === "recording_export_failed"
  );
});

test("a recording that spans a window resize still encodes one animation", async () => {
  // Every frame after the first is scaled to the first frame's shape rather than refused. GIF has
  // one logical screen, and the caller has already done the work.
  const { api } = composer({
    decode: async (data) => {
      const shade = Number(data.split(":")[1]);
      return shade === 1 ? pixels(shade, 8, 8) : pixels(shade, 12, 6);
    }
  });
  const encoded = await api.encode([frame(1, 100), frame(2, 100)], { maxBytes: GENEROUS });
  assert.equal(encoded.width, 8);
  assert.equal(encoded.height, 8);
  assert.equal(encoded.frame_count, 2);
});

test("the encoder uses the recording registry's own fidelity policy", () => {
  // One implementation, in one place. When this stops being true, a thinned replay and a thinned
  // recording start disagreeing about how long the work took.
  assert.equal(typeof recordingApi.thinFrames, "function");
  const { kept } = recordingApi.thinFrames([
    { duration_ms: 10 }, { duration_ms: 20 }, { duration_ms: 30 }, { duration_ms: 40 }, { duration_ms: 50 }
  ]);
  assert.deepEqual(kept.map((entry) => entry.duration_ms), [30, 70, 50]);
  assert.equal(kept.reduce((total, entry) => total + entry.duration_ms, 0), 150);
});

test("fewer than three frames cannot be thinned, and the policy says so", () => {
  // The fit loop relies on this: a list that cannot shrink has to be reported, not retried.
  for (const frames of [[], [{ duration_ms: 1 }], [{ duration_ms: 1 }, { duration_ms: 2 }]]) {
    const { kept, dropped } = recordingApi.thinFrames(frames);
    assert.equal(kept.length, frames.length);
    assert.equal(dropped.length, 0);
  }
});
