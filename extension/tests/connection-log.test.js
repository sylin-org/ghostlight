const { test } = require("node:test");
const assert = require("node:assert/strict");
const log = require("../lib/connection-log.js");
function fixture(enabled = true) {
  const saved = { debug: enabled };
  return { saved, storage: {
    get: async () => structuredClone(saved),
    set: async values => Object.assign(saved, structuredClone(values))
  } };
}
test("connection evidence survives new workers, is bounded, and drops unlisted payload fields", async () => {
  const { saved, storage } = fixture();
  const first = log.create({ storage, debugKey: "debug", context: { epoch: "first" } });
  await first.record(log.EVENTS.WORKER_STARTED);
  for (let attempt = 0; attempt <= log.LIMIT; attempt++) {
    first.record(log.EVENTS.CONNECT_FAILED, { attempt, error: "Native messaging host not found",
      payload: "PRIVATE_PAGE_CONTENT", token: "PRIVATE_TOKEN" });
  }
  const snapshot = await first.snapshot();
  assert.equal(snapshot.entries.length, log.LIMIT);
  assert.equal(snapshot.boots[0].epoch, "first");
  assert.equal(JSON.stringify(snapshot).includes("PRIVATE_"), false);
  saved[log.KEY].entries[0].payload = "PRIVATE_STORED_PAYLOAD";
  const second = log.create({ storage, debugKey: "debug", context: { epoch: "second" } });
  await second.record(log.EVENTS.WORKER_STARTED);
  const restored = await second.snapshot();
  assert.deepEqual(restored.boots.map(row => row.epoch), ["first", "second"]);
  assert.equal(JSON.stringify(restored).includes("PRIVATE_"), false);
  assert.equal(restored.entries.at(-2).attempt, log.LIMIT);
});
test("disabled logging stays off and persistence failures cannot reject connection work", async () => {
  const { saved, storage } = fixture(false);
  const writer = log.create({ storage, debugKey: "debug", context: {} });
  await writer.record(log.EVENTS.CONNECT_REQUESTED);
  assert.equal(saved[log.KEY], undefined);
  await writer.setEnabled(true);
  storage.set = async () => { throw new Error("disk unavailable"); };
  await writer.record(log.EVENTS.CONNECT_REQUESTED);
  assert.equal((await writer.snapshot()).write_failures, 1);
});
