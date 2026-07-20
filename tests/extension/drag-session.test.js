// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for the per-tab native-drag coordinator.

const { test } = require("node:test");
const assert = require("node:assert");
const {
  DRAG_SESSION_PHASES,
  DRAG_OBSERVATION_MESSAGES,
  createDragCoordinator,
} = require("../../extension/lib/drag-session.js");

test("drag observation messages are a stable shared vocabulary", () => {
  assert.deepStrictEqual(DRAG_OBSERVATION_MESSAGES, {
    BEGIN: "dragObservationBegin",
    FINISH: "dragObservationFinish",
  });
});

test("intercepted drag data remains the same opaque object", async () => {
  const coordinator = createDragCoordinator(20);
  const session = coordinator.begin(7);
  const data = { items: [{ mimeType: "application/x-test", data: "secret" }] };

  assert.equal(session.phase, DRAG_SESSION_PHASES.ARMED);
  assert.equal(coordinator.intercepted(7, data), true);
  const result = await coordinator.finish(session, 20);

  assert.equal(session.phase, DRAG_SESSION_PHASES.NATIVE);
  assert.equal(result.mode, DRAG_SESSION_PHASES.NATIVE);
  assert.equal(result.data, data);
});

test("timeout selects the pointer lane", async () => {
  const coordinator = createDragCoordinator(1);
  const session = coordinator.begin(8);
  const result = await coordinator.finish(session, 0);

  assert.equal(result.mode, DRAG_SESSION_PHASES.POINTER);
  assert.equal(session.phase, DRAG_SESSION_PHASES.POINTER);
  assert.equal(coordinator.intercepted(8, {}), false);
});

test("new and explicit cancellation settle old sessions", async () => {
  const coordinator = createDragCoordinator(20);
  const replaced = coordinator.begin(9);
  const current = coordinator.begin(9);

  assert.equal((await replaced.promise).mode, DRAG_SESSION_PHASES.CANCELLED);
  assert.equal(replaced.phase, DRAG_SESSION_PHASES.CANCELLED);
  assert.equal(coordinator.cancel(9), true);
  assert.equal((await current.promise).mode, DRAG_SESSION_PHASES.CANCELLED);
  assert.equal(coordinator.cancel(9), false);
});

test("events are correlated by tab and clear cancels every session", async () => {
  const coordinator = createDragCoordinator(20);
  const one = coordinator.begin(1);
  const two = coordinator.begin(2);

  assert.equal(coordinator.intercepted(3, {}), false);
  coordinator.clear();
  assert.equal((await one.promise).mode, DRAG_SESSION_PHASES.CANCELLED);
  assert.equal((await two.promise).mode, DRAG_SESSION_PHASES.CANCELLED);
});
