"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const engineApi = require("../lib/engine.js");

function persistence(initial = null) {
  let value = initial;
  const writes = [];
  return {
    writes,
    async load() { return value; },
    async save(next) {
      value = structuredClone(next);
      writes.push(value);
    },
    value() { return value; }
  };
}

test("concurrent and completed duplicates execute a browser effect once", async () => {
  const store = persistence();
  const engine = engineApi.create(store);
  await engine.activate("service_one");
  let calls = 0;
  let release;
  const effect = () => {
    calls += 1;
    return new Promise((resolve) => { release = resolve; });
  };

  const first = engine.execute("physical_one", effect);
  const duplicate = engine.execute("physical_one", effect);
  await new Promise((resolve) => setImmediate(resolve));
  release({ outcome: "tab_closed", tab_id: 7, secret: "memory only" });
  assert.deepEqual(await first, { outcome: "tab_closed", tab_id: 7, secret: "memory only" });
  assert.deepEqual(await duplicate, { outcome: "tab_closed", tab_id: 7, secret: "memory only" });
  assert.deepEqual(await engine.execute("physical_one", effect), {
    outcome: "tab_closed",
    tab_id: 7,
    secret: "memory only"
  });
  assert.equal(calls, 1);

  const serialized = JSON.stringify(store.value());
  assert.doesNotMatch(serialized, /memory only|tab_closed|secret/);
  assert.deepEqual(store.value(), {
    epoch: "service_one",
    records: [{ id: "physical_one", phase: "completed" }]
  });
});

test("acknowledgement releases a terminal operation record", async () => {
  const store = persistence();
  const engine = engineApi.create(store);
  await engine.activate("service_one");
  await engine.execute("physical_one", async () => ({ outcome: "cancelled" }));
  await engine.acknowledge("physical_one");
  assert.deepEqual(engine.snapshot(), { epoch: "service_one", records: [] });
});

test("restart resumes only phases that prove no browser effect was dispatched", async () => {
  for (const phase of ["accepted", "failed"]) {
    const store = persistence({
      epoch: "service_one",
      records: [{ id: `physical_${phase}`, phase }]
    });
    const engine = engineApi.create(store);
    await engine.activate("service_one");
    let calls = 0;
    const result = await engine.execute(`physical_${phase}`, async () => {
      calls += 1;
      return { outcome: "cancelled" };
    });
    assert.deepEqual(result, { outcome: "cancelled" });
    assert.equal(calls, 1);
  }

  for (const phase of ["dispatched", "completed", "uncertain"]) {
    const store = persistence({
      epoch: "service_one",
      records: [{ id: `physical_${phase}`, phase }]
    });
    const engine = engineApi.create(store);
    await engine.activate("service_one");
    let calls = 0;
    await assert.rejects(
      engine.execute(`physical_${phase}`, async () => { calls += 1; }),
      (error) => error.code === "operation_result_unavailable" && error.effectUnknown === true
    );
    assert.equal(calls, 0);
  }
});

test("a new service epoch clears stale operation recovery state", async () => {
  const store = persistence({
    epoch: "service_old",
    records: [{ id: "physical_old", phase: "dispatched" }]
  });
  const engine = engineApi.create(store);
  await engine.activate("service_new");
  assert.deepEqual(store.value(), { epoch: "service_new", records: [] });
});

test("the recovery ledger is bounded without evicting unacknowledged effects", async () => {
  const store = persistence();
  const engine = engineApi.create({ ...store, maximumRecords: 1 });
  await engine.activate("service_one");
  await engine.execute("physical_one", async () => ({ outcome: "cancelled" }));
  let calls = 0;
  await assert.rejects(
    engine.execute("physical_two", async () => { calls += 1; }),
    (error) => error.code === "operation_result_unavailable" && error.effectUnknown === true
  );
  assert.equal(calls, 0);
});

test("terminal persistence failure never turns a completed effect into failure", async () => {
  let writes = 0;
  const engine = engineApi.create({
    async load() { return null; },
    async save() {
      writes += 1;
      if (writes === 4) throw new Error("storage unavailable after effect");
    }
  });
  await engine.activate("service_one");
  let calls = 0;
  const result = await engine.execute("physical_one", async () => {
    calls += 1;
    return { outcome: "cancelled" };
  });
  assert.deepEqual(result, { outcome: "cancelled" });
  assert.deepEqual(await engine.execute("physical_one", async () => { calls += 1; }), result);
  assert.equal(calls, 1);
  assert.deepEqual(engine.snapshot(), {
    epoch: "service_one",
    records: [{ id: "physical_one", phase: "completed" }]
  });
});
