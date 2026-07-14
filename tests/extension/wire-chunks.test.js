// SPDX-License-Identifier: Apache-2.0 OR MIT
// Pure tests for bounded native-message reassembly (ADR-0074).

const { test } = require("node:test");
const assert = require("node:assert");
const crypto = require("node:crypto");
const { createWireChunkStore } = require("../../extension/lib/wire-chunks.js");

function digest(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function harness(overrides = {}) {
  let nextTimer = 1;
  const timers = new Map();
  const rejected = [];
  const delivered = [];
  const store = createWireChunkStore({
    decodeBase64: (value) => new Uint8Array(Buffer.from(value, "base64")),
    decodeUtf8: (bytes) => new TextDecoder("utf-8", { fatal: true }).decode(bytes),
    sha256Hex: async (bytes) => digest(bytes),
    setTimer: (callback) => {
      const id = nextTimer++;
      timers.set(id, callback);
      return id;
    },
    clearTimer: (id) => timers.delete(id),
    ...overrides,
  });
  const accept = (chunk) => store.accept(
    chunk,
    (request) => delivered.push(request),
    (requestId, reason) => rejected.push({ requestId, reason })
  );
  return { store, accept, delivered, rejected, timers };
}

function chunksFor(request, size = 8) {
  const bytes = Buffer.from(JSON.stringify(request));
  const pieces = [];
  const count = Math.ceil(bytes.length / size);
  for (let index = 0; index < count; index += 1) {
    pieces.push({
      type: "wire_chunk",
      transferId: "wire_1",
      requestId: request.id,
      index,
      count,
      totalBytes: bytes.length,
      sha256: digest(bytes),
      data: bytes.subarray(index * size, (index + 1) * size).toString("base64"),
    });
  }
  return pieces;
}

function settle() {
  return new Promise((resolve) => setImmediate(resolve));
}

test("complete verified chunks dispatch the ordinary request exactly once", async () => {
  const h = harness();
  const request = { id: "42", type: "tool_request", args: { data: "private pixels" } };
  for (const chunk of chunksFor(request)) h.accept(chunk);
  await settle();
  assert.deepStrictEqual(h.delivered, [request]);
  assert.deepStrictEqual(h.rejected, []);
  assert.deepStrictEqual(h.store.stats(), { active: 0, bytes: 0 });
});

test("a duplicate chunk rejects and erases the partial transfer", () => {
  const h = harness();
  const chunks = chunksFor({ id: "7", type: "tool_request", value: "long enough" });
  h.accept(chunks[0]);
  h.accept(chunks[0]);
  assert.strictEqual(h.rejected[0].reason, "duplicate or inconsistent chunk");
  assert.deepStrictEqual(h.store.stats(), { active: 0, bytes: 0 });
});

test("a digest mismatch never dispatches", async () => {
  const h = harness();
  const chunks = chunksFor({ id: "8", type: "tool_request" }, 1024);
  chunks[0].sha256 = "0".repeat(64);
  h.accept(chunks[0]);
  await settle();
  assert.strictEqual(h.delivered.length, 0);
  assert.strictEqual(h.rejected[0].reason, "digest mismatch");
});

test("the aggregate memory ceiling rejects before retaining excess bytes", () => {
  const h = harness({ maxTotalBytes: 4, maxBytes: 100 });
  const chunks = chunksFor({ id: "9", x: "payload" }, 1024);
  h.accept(chunks[0]);
  assert.strictEqual(h.rejected[0].reason, "memory bound exceeded");
  assert.deepStrictEqual(h.store.stats(), { active: 0, bytes: 0 });
});

test("expiry erases a partial transfer", () => {
  const h = harness();
  const chunks = chunksFor({ id: "10", value: "several chunks required" }, 5);
  h.accept(chunks[0]);
  const callback = Array.from(h.timers.values())[0];
  callback();
  assert.strictEqual(h.rejected[0].reason, "transfer expired");
  assert.deepStrictEqual(h.store.stats(), { active: 0, bytes: 0 });
});
