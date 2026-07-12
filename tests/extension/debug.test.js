// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/debug.js (ADR-0059 developer diagnostics forwarder). Pure
// module: an injected fake storage area and a `post` callback stand in for
// chrome.storage.local / nativePort.postMessage, matching lib/grouping.js's injected-chrome
// precedent -- no mocked extension globals needed.

const { test } = require("node:test");
const assert = require("node:assert");
const { createDebugForwarder } = require("../../extension/lib/debug.js");

function fakeStorage(initial) {
  let value = { ...initial };
  return {
    async get(key) {
      return { [key]: value[key] };
    },
    set(next) {
      value = { ...value, ...next };
    },
  };
}

test("send is a no-op when the debug flag is off", async () => {
  const storage = fakeStorage({ ghostlight_debug: false });
  const forwarder = createDebugForwarder(storage);
  const posted = [];
  await forwarder.send((msg) => posted.push(msg), "connect_attempt");
  assert.deepStrictEqual(posted, []);
});

test("send posts immediately when a port is available and the flag is on", async () => {
  const storage = fakeStorage({ ghostlight_debug: true });
  const forwarder = createDebugForwarder(storage);
  const posted = [];
  await forwarder.send((msg) => posted.push(msg), "connect_attempt", { note: "hello" });
  assert.strictEqual(posted.length, 1);
  assert.deepStrictEqual(posted[0], {
    type: "debug_event",
    event: "connect_attempt",
    detail: { note: "hello" },
  });
});

test("a missing detail defaults to null, never undefined (must survive JSON framing)", async () => {
  const storage = fakeStorage({ ghostlight_debug: true });
  const forwarder = createDebugForwarder(storage);
  const posted = [];
  await forwarder.send((msg) => posted.push(msg), "connect_attempt");
  assert.strictEqual(posted[0].detail, null);
});

test("a note raised with no port buffers, then flushes in order on the next post", async () => {
  const storage = fakeStorage({ ghostlight_debug: true });
  const forwarder = createDebugForwarder(storage);
  await forwarder.send(null, "connect_attempt");
  await forwarder.send(null, "connect_disconnect", "boom");

  const posted = [];
  forwarder.flush((msg) => posted.push(msg));
  assert.strictEqual(posted.length, 2);
  assert.strictEqual(posted[0].event, "connect_attempt");
  assert.strictEqual(posted[1].event, "connect_disconnect");
  assert.strictEqual(posted[1].detail, "boom");
});

test("flushing drains the buffer -- a second flush with no new notes posts nothing", async () => {
  const storage = fakeStorage({ ghostlight_debug: true });
  const forwarder = createDebugForwarder(storage);
  await forwarder.send(null, "connect_attempt");

  const first = [];
  forwarder.flush((msg) => first.push(msg));
  assert.strictEqual(first.length, 1);

  const second = [];
  forwarder.flush((msg) => second.push(msg));
  assert.deepStrictEqual(second, []);
});

test("a post that throws stops the flush but does not lose the ordering of what posted first", async () => {
  const storage = fakeStorage({ ghostlight_debug: true });
  const forwarder = createDebugForwarder(storage);
  await forwarder.send(null, "first");
  await forwarder.send(null, "second");

  const posted = [];
  forwarder.flush((msg) => {
    posted.push(msg);
    throw new Error("port died mid-flush");
  });
  assert.strictEqual(posted.length, 1, "only the first message was attempted before the throw");
  assert.strictEqual(posted[0].event, "first");
});

test("the pending buffer is bounded: overflowing it drops the OLDEST notes first", async () => {
  const storage = fakeStorage({ ghostlight_debug: true });
  const forwarder = createDebugForwarder(storage);
  for (let i = 0; i < 25; i++) {
    await forwarder.send(null, `event-${i}`);
  }
  const posted = [];
  forwarder.flush((msg) => posted.push(msg));
  assert.strictEqual(posted.length, 20, "bounded to MAX_PENDING_DEBUG_EVENTS");
  assert.strictEqual(posted[0].event, "event-5", "the oldest 5 were dropped to stay bounded");
  assert.strictEqual(posted[19].event, "event-24");
});

test("a post that throws falls back to buffering rather than losing the note", async () => {
  const storage = fakeStorage({ ghostlight_debug: true });
  const forwarder = createDebugForwarder(storage);
  await forwarder.send(() => {
    throw new Error("port not actually open");
  }, "connect_attempt");

  const posted = [];
  forwarder.flush((msg) => posted.push(msg));
  assert.strictEqual(posted.length, 1);
  assert.strictEqual(posted[0].event, "connect_attempt");
});
