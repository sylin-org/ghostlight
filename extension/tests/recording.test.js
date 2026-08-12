"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const recordingApi = require("../lib/recording.js");

function harness() {
  let time = 1_000;
  let nextTimer = 0;
  let nextId = 0;
  const timers = new Map();
  const stops = [];
  const recording = recordingApi.create({
    now: () => time,
    setTimer(callback, delay) {
      const id = ++nextTimer;
      timers.set(id, { callback, delay });
      return id;
    },
    clearTimer(id) { timers.delete(id); },
    newId: () => `recording_${++nextId}`,
    onStop(tabId, recordingId, reason) { stops.push({ tabId, recordingId, reason }); }
  });
  return { recording, timers, stops, setTime(value) { time = value; } };
}

const FRAME = "AA==";

test("the registry owns plural recording identities and tab-local capture", () => {
  const h = harness();
  const first = h.recording.start("workspace_a", 7, "https://example.com/path?secret=1").started;
  const second = h.recording.start("workspace_a", 8, "https://example.org/").started;
  assert.notEqual(first.recording_id, second.recording_id);
  assert.equal(h.recording.count(), 2);
  assert.equal(h.recording.append(7, FRAME, "seed", 1_000), true);
  assert.equal(h.recording.append(8, FRAME, "seed", 1_000), true);
  assert.equal(h.recording.read("workspace_a", first.recording_id).frames.length, 1);
  assert.equal(h.recording.read("workspace_a", second.recording_id).frames.length, 1);
});

test("start is idempotent only inside the same opaque workspace", () => {
  const h = harness();
  const started = h.recording.start("workspace_a", 7, "https://example.com/").started;
  assert.equal(h.recording.start("workspace_a", 7).existing.recording_id, started.recording_id);
  assert.throws(
    () => h.recording.start("workspace_b", 7),
    (error) => error.code === "recording_active"
  );
  assert.deepEqual(h.recording.status("workspace_b", started.recording_id), { notFound: true });
});

test("omitted identities are convenient when unique and corrective when ambiguous", () => {
  const h = harness();
  const first = h.recording.start("workspace_a", 7).started.recording_id;
  assert.equal(h.recording.status("workspace_a").summary.recording_id, first);
  const second = h.recording.start("workspace_a", 8).started.recording_id;
  assert.deepEqual(h.recording.status("workspace_a"), { ambiguous: [first, second] });
});

test("the extension stops autonomously at its hard deadline and later flushes bytes", () => {
  const h = harness();
  const id = h.recording.start("workspace_a", 7).started.recording_id;
  h.recording.append(7, FRAME, "seed", 1_000);
  h.setTime(1_000 + recordingApi.HARD_DURATION_MS);
  Array.from(h.timers.values())[0].callback();
  const frozen = h.recording.status("workspace_a", id).summary;
  assert.equal(frozen.state, "interrupted");
  assert.equal(frozen.stop_reason, "hard_timeout");
  assert.deepEqual(h.stops, [{ tabId: 7, recordingId: id, reason: "hard_timeout" }]);
  h.setTime(frozen.retention_expires_unix_ms);
  Array.from(h.timers.values())[0].callback();
  assert.deepEqual(h.recording.status("workspace_a", id), { notFound: true });
});

test("stop freezes, read is non-consuming, and discard reclaims exact bytes", () => {
  const h = harness();
  const id = h.recording.start("workspace_a", 7).started.recording_id;
  h.recording.append(7, FRAME, "seed", 1_000);
  const plan = h.recording.beginStop("workspace_a", id);
  const stopped = h.recording.finishStop(plan.state);
  assert.equal(stopped.state, "frozen");
  assert.equal(stopped.stop_reason, "explicit");
  assert.equal(h.recording.read("workspace_a", id).frames.length, 1);
  assert.equal(h.recording.read("workspace_a", id).frames.length, 1);
  assert.deepEqual(h.recording.discard("workspace_a", id), {
    recordingId: id, releasedBytes: 1, active: false, tabId: 7
  });
  assert.deepEqual(h.recording.status("workspace_a", id), { notFound: true });
});

test("frame size, recording size, cadence, and finalization are extension-owned", () => {
  const h = harness();
  const id = h.recording.start("workspace_a", 7).started.recording_id;
  assert.equal(recordingApi.MAX_FRAME_BYTES, 2 * 1024 * 1024);
  assert.equal(recordingApi.MAX_RECORDING_BYTES, 5 * 1024 * 1024);
  assert.equal(h.recording.append(7, FRAME, "screencast", 1_000), true);
  assert.equal(h.recording.append(7, FRAME, "screencast", 1_050), false);
  assert.equal(h.recording.append(7, FRAME, "screencast", 1_100), true);
  h.recording.beginStop("workspace_a", id);
  assert.equal(h.recording.append(7, FRAME, "screencast", 1_200), false);
  assert.equal(h.recording.append(7, FRAME, "final", 1_200), true);
});

test("invalid and oversized frames interrupt capture before transport", () => {
  const empty = harness();
  const emptyId = empty.recording.start("workspace_a", 7).started.recording_id;
  assert.equal(empty.recording.append(7, "", "seed", 1_000), false);
  assert.equal(empty.recording.status("workspace_a", emptyId).summary.stop_reason, "frame_too_large");

  const large = harness();
  const largeId = large.recording.start("workspace_a", 8).started.recording_id;
  assert.equal(large.recording.append(8, "A".repeat(recordingApi.MAX_FRAME_BASE64_CHARS + 1), "seed", 1_000), false);
  assert.equal(large.recording.status("workspace_a", largeId).summary.stop_reason, "frame_too_large");
});

test("browser and service loss interrupt all active recordings but retain frozen bytes", () => {
  const h = harness();
  const first = h.recording.start("workspace_a", 7).started.recording_id;
  const second = h.recording.start("workspace_b", 8).started.recording_id;
  h.recording.append(7, FRAME, "seed", 1_000);
  h.recording.append(8, FRAME, "seed", 1_000);
  assert.equal(h.recording.interruptTab(7, "browser_detached").stop_reason, "browser_detached");
  assert.equal(h.recording.interruptAll("service_disconnected")[0].recording_id, second);
  assert.equal(h.recording.count(), 0);
  assert.equal(h.recording.read("workspace_a", first).frames.length, 1);
  assert.equal(h.recording.read("workspace_b", second).frames.length, 1);
});
