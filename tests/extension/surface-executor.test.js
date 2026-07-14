// SPDX-License-Identifier: Apache-2.0 OR MIT

const { test } = require("node:test");
const assert = require("node:assert");
const { createSurfaceExecutor } = require("../../extension/lib/surface-executor.js");

function deferred() {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
}

function settle() {
  return new Promise((resolve) => setImmediate(resolve));
}

function harness(overrides = {}) {
  let sequence = 0;
  let nextTimer = 1;
  const timers = new Map();
  const started = [];
  const accepted = [];
  const rejected = [];
  const terminal = [];
  const gates = new Map();
  const executor = createSurfaceExecutor({
    execute: (item) => {
      started.push(item.commandId);
      const gate = deferred();
      gates.set(item.commandId, gate);
      return gate.promise;
    },
    onAccepted: (item, duplicate) => accepted.push([item.commandId, duplicate]),
    onRejected: (item, reason) => rejected.push([item && item.commandId, reason]),
    onTerminal: (item) => terminal.push(item.commandId),
    setTimer: (callback) => {
      const id = nextTimer++;
      timers.set(id, callback);
      return id;
    },
    clearTimer: (id) => timers.delete(id),
    ...overrides,
  });
  const submit = (key, bypass = false, bytes = 10, commandId = String(++sequence)) => {
    const item = { commandId, key, bypass, bytes, request: { secret: commandId } };
    return { accepted: executor.submit(item), item };
  };
  return { executor, submit, started, accepted, rejected, terminal, gates, timers };
}

test("same-surface commands execute in FIFO order", async () => {
  const h = harness();
  h.submit("surface:1");
  h.submit("surface:1");
  h.submit("surface:1");
  await settle();
  assert.deepStrictEqual(h.started, ["1"]);
  h.gates.get("1").resolve();
  await settle();
  assert.deepStrictEqual(h.started, ["1", "2"]);
  h.gates.get("2").resolve();
  await settle();
  assert.deepStrictEqual(h.started, ["1", "2", "3"]);
});

test("different surfaces and presentation run concurrently", async () => {
  const h = harness();
  h.submit("surface:1");
  h.submit("surface:2");
  h.submit("presentation", true);
  await settle();
  assert.deepStrictEqual(new Set(h.started), new Set(["1", "2", "3"]));
});

test("queue bounds reject before retaining a payload", async () => {
  const h = harness({ maxHeld: 2, maxHeldBytes: 25 });
  h.submit("surface:1", false, 10);
  h.submit("surface:1", false, 10);
  const third = h.submit("surface:1", false, 10);
  await settle();
  assert.strictEqual(third.accepted, false);
  assert.deepStrictEqual(h.rejected, [["3", "queue_overloaded"]]);
  assert.deepStrictEqual(h.executor.stats(), { held: 2, bytes: 20, resources: 1 });
});

test("duplicate commands are not executed twice", async () => {
  const h = harness();
  h.submit("surface:1", false, 10, "same");
  h.submit("surface:1", false, 10, "same");
  await settle();
  assert.deepStrictEqual(h.started, ["same"]);
  assert.deepStrictEqual(h.accepted, [["same", false], ["same", true]]);
  h.gates.get("same").resolve();
  await settle();
  h.submit("surface:1", false, 10, "same");
  assert.deepStrictEqual(h.terminal, ["same", "same"]);
  assert.deepStrictEqual(h.started, ["same"]);
});

test("queued expiry rejects and erases the retained payload", async () => {
  const h = harness();
  h.submit("surface:1");
  const queued = h.submit("surface:1");
  await settle();
  const expiry = Array.from(h.timers.values())[0];
  expiry();
  assert.deepStrictEqual(h.rejected, [["2", "queue_expired"]]);
  assert.strictEqual(queued.item.request, null);
  assert.deepStrictEqual(h.executor.stats(), { held: 1, bytes: 10, resources: 1 });
});

test("destroying a surface rejects its queued commands only", async () => {
  const h = harness();
  h.submit("surface:1");
  h.submit("surface:1");
  h.submit("surface:2");
  await settle();
  h.executor.destroyKey("surface:1");
  assert.deepStrictEqual(h.rejected, [["2", "resource_destroyed"]]);
  assert.deepStrictEqual(new Set(h.started), new Set(["1", "3"]));
});

test("a new executor generation may accept the same wire command identity", async () => {
  const first = harness();
  first.submit("surface:1", false, 10, "7");
  await settle();
  first.gates.get("7").resolve();
  await settle();

  const next = harness();
  next.submit("surface:1", false, 10, "7");
  await settle();
  assert.deepStrictEqual(next.started, ["7"]);
});
