"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const chunksApi = require("../lib/chunks.js");

function digest(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function harness(overrides = {}) {
  const delivered = [];
  const rejected = [];
  const timers = new Map();
  let timerId = 0;
  const chunks = chunksApi.create({
    decodeBase64: (value) => new Uint8Array(Buffer.from(value, "base64")),
    decodeUtf8: (bytes) => new TextDecoder("utf-8", { fatal: true }).decode(bytes),
    sha256Hex: async (bytes) => digest(bytes),
    setTimer(callback) { const id = ++timerId; timers.set(id, callback); return id; },
    clearTimer(id) { timers.delete(id); },
    ...overrides
  });
  return {
    chunks,
    delivered,
    rejected,
    timers,
    accept(frame) { chunks.accept(frame, (value) => delivered.push(value), (correlation, reason) => rejected.push({ correlation, reason })); }
  };
}

function framesFor(request, chunkSize = 8, transferId = "chunk_1") {
  const bytes = Buffer.from(JSON.stringify(request));
  const count = Math.ceil(bytes.length / chunkSize);
  return Array.from({ length: count }, (_value, index) => ({
    kind: "command_chunk",
    transfer_id: transferId,
    correlation: request.request.correlation,
    index,
    count,
    total_bytes: bytes.length,
    sha256: digest(bytes),
    data: bytes.subarray(index * chunkSize, (index + 1) * chunkSize).toString("base64")
  }));
}

function settle() { return new Promise((resolve) => setImmediate(resolve)); }

test("verified chunks deliver the ordinary request exactly once", async () => {
  const h = harness();
  const request = { kind: "request", request: { correlation: "physical_1", command: { command: "upload_files" } } };
  for (const frame of framesFor(request)) h.accept(frame);
  await settle();
  assert.deepEqual(h.delivered, [request]);
  assert.deepEqual(h.rejected, []);
  assert.deepEqual(h.chunks.stats(), { active: 0, bytes: 0, completed: 1 });

  for (const frame of framesFor(request)) h.accept(frame);
  await settle();
  assert.deepEqual(h.delivered, [request]);
  assert.deepEqual(h.rejected, []);
});

test("digest and correlation mismatches never dispatch", async () => {
  const request = { kind: "request", request: { correlation: "physical_2", command: {} } };
  const digestHarness = harness();
  const badDigest = framesFor(request, 1000)[0];
  badDigest.sha256 = "0".repeat(64);
  digestHarness.accept(badDigest);
  await settle();
  assert.equal(digestHarness.delivered.length, 0);
  assert.match(digestHarness.rejected[0].reason, /digest mismatch/);

  const correlationHarness = harness();
  const badCorrelation = framesFor(request, 1000)[0];
  badCorrelation.correlation = "physical_other";
  correlationHarness.accept(badCorrelation);
  await settle();
  assert.equal(correlationHarness.delivered.length, 0);
  assert.match(correlationHarness.rejected[0].reason, /correlation mismatch/);
});

test("duplicates, expiry, and all hard bounds erase partial bytes", () => {
  const request = { kind: "request", request: { correlation: "physical_3", command: { payload: "long enough" } } };
  const duplicate = harness();
  const frames = framesFor(request, 5);
  duplicate.accept(frames[0]);
  duplicate.accept(frames[0]);
  assert.match(duplicate.rejected[0].reason, /duplicate/);
  assert.deepEqual(duplicate.chunks.stats(), { active: 0, bytes: 0, completed: 0 });

  const inconsistent = harness();
  inconsistent.accept(frames[0]);
  inconsistent.accept({ ...frames[1], correlation: "physical_wrong" });
  assert.equal(inconsistent.rejected[0].correlation, "physical_3");
  assert.deepEqual(inconsistent.chunks.stats(), { active: 0, bytes: 0, completed: 0 });

  const expired = harness();
  expired.accept(frames[0]);
  Array.from(expired.timers.values())[0]();
  assert.match(expired.rejected[0].reason, /expired/);
  assert.deepEqual(expired.chunks.stats(), { active: 0, bytes: 0, completed: 0 });

  const memory = harness({ maxBytes: 100, maxTotalBytes: 4 });
  memory.accept(framesFor(request, 1000)[0]);
  assert.match(memory.rejected[0].reason, /memory bound/);
  assert.deepEqual(memory.chunks.stats(), { active: 0, bytes: 0, completed: 0 });

  const chunk = harness({ maxChunkBytes: 4 });
  chunk.accept(framesFor(request, 1000)[0]);
  assert.match(chunk.rejected[0].reason, /invalid chunk metadata|memory bound/);
  assert.deepEqual(chunk.chunks.stats(), { active: 0, bytes: 0, completed: 0 });

  const count = harness({ maxChunks: 1 });
  count.accept(frames[0]);
  assert.match(count.rejected[0].reason, /invalid chunk metadata/);
});

test("disconnect clearing forgets every partial transfer", () => {
  const h = harness();
  const request = { kind: "request", request: { correlation: "physical_4", command: { payload: "long enough" } } };
  h.accept(framesFor(request, 5)[0]);
  assert.equal(h.chunks.stats().active, 1);
  h.chunks.clear();
  assert.deepEqual(h.chunks.stats(), { active: 0, bytes: 0, completed: 0 });
});

test("disconnect during digest verification cannot dispatch into a later connection", async () => {
  let finishDigest;
  const h = harness({
    sha256Hex: () => new Promise((resolve) => { finishDigest = resolve; })
  });
  const request = { kind: "request", request: { correlation: "physical_5", command: {} } };
  h.accept(framesFor(request, 1000)[0]);
  await settle();
  h.chunks.clear();
  finishDigest(digest(Buffer.from(JSON.stringify(request))));
  await settle();
  assert.deepEqual(h.delivered, []);
  assert.deepEqual(h.chunks.stats(), { active: 0, bytes: 0, completed: 0 });
});

test("logical expiry rejects late chunks even when a worker timer is delayed", () => {
  let time = 0;
  const h = harness({ now: () => time });
  const request = { kind: "request", request: { correlation: "physical_6", command: { payload: "long enough" } } };
  const frames = framesFor(request, 5);
  h.accept(frames[0]);
  time = 15_000;
  h.accept(frames[1]);
  assert.equal(h.delivered.length, 0);
  assert.match(h.rejected[0].reason, /expired/);
  assert.deepEqual(h.chunks.stats(), { active: 0, bytes: 0, completed: 0 });
});

test("completed transfer tombstones preserve exact-once behavior at their count bound", async () => {
  let time = 0;
  const h = harness({ maxCompleted: 2, now: () => time, ttlMs: 100 });
  const requests = ["one", "two", "three"].map((name) => ({
    kind: "request",
    request: { correlation: `physical_${name}`, command: {} }
  }));

  for (const [index, request] of requests.entries()) {
    for (const frame of framesFor(request, 1000, `chunk_${index}`)) h.accept(frame);
    await settle();
  }
  assert.equal(h.delivered.length, 2);
  assert.match(h.rejected[0].reason, /completion ledger is full/);
  assert.deepEqual(h.chunks.stats(), { active: 0, bytes: 0, completed: 2 });

  for (const frame of framesFor(requests[1], 1000, "chunk_1")) h.accept(frame);
  await settle();
  assert.equal(h.delivered.length, 2);

  time = 100;
  for (const frame of framesFor(requests[2], 1000, "chunk_2")) h.accept(frame);
  await settle();
  assert.equal(h.delivered.length, 3);
  assert.deepEqual(h.chunks.stats(), { active: 0, bytes: 0, completed: 1 });
});

test("clearing a connection erases completed transfer tombstones", async () => {
  const h = harness();
  const request = { kind: "request", request: { correlation: "physical_clear", command: {} } };
  const frames = framesFor(request, 1000);
  for (const frame of frames) h.accept(frame);
  await settle();
  assert.equal(h.delivered.length, 1);
  assert.equal(h.chunks.stats().completed, 1);

  h.chunks.clear();
  assert.deepEqual(h.chunks.stats(), { active: 0, bytes: 0, completed: 0 });
  for (const frame of frames) h.accept(frame);
  await settle();
  assert.equal(h.delivered.length, 2);
});

test("logical expiry also fences delayed digest completion", async () => {
  let time = 0;
  let finishDigest;
  const h = harness({
    now: () => time,
    sha256Hex: () => new Promise((resolve) => { finishDigest = resolve; })
  });
  const request = { kind: "request", request: { correlation: "physical_7", command: {} } };
  h.accept(framesFor(request, 1000)[0]);
  await settle();
  time = 15_000;
  finishDigest(digest(Buffer.from(JSON.stringify(request))));
  await settle();
  assert.equal(h.delivered.length, 0);
  assert.match(h.rejected[0].reason, /expired/);
});

test("a cancelled verifier cannot erase a later transfer with the same id", async () => {
  let rejectOldDigest;
  let digestCalls = 0;
  const h = harness({
    sha256Hex: (bytes) => {
      digestCalls += 1;
      if (digestCalls === 1) {
        return new Promise((_resolve, reject) => { rejectOldDigest = reject; });
      }
      return Promise.resolve(digest(bytes));
    }
  });
  const oldRequest = { kind: "request", request: { correlation: "physical_old", command: {} } };
  h.accept(framesFor(oldRequest, 1000)[0]);
  await settle();
  h.chunks.clear();

  const newRequest = { kind: "request", request: { correlation: "physical_new", command: {} } };
  h.accept(framesFor(newRequest, 1000)[0]);
  rejectOldDigest(new Error("old verifier ended"));
  await settle();
  assert.deepEqual(h.delivered, [newRequest]);
  assert.deepEqual(h.rejected, []);
});
