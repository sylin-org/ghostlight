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

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function delayedPersistence() {
  const store = persistence();
  const pending = [];
  let delayed = false;
  let active = 0;
  let maximumConcurrent = 0;
  return {
    ...store,
    pending,
    delay() { delayed = true; },
    maximumConcurrent() { return maximumConcurrent; },
    async save(value) {
      active += 1;
      maximumConcurrent = Math.max(maximumConcurrent, active);
      try {
        if (delayed) {
          const barrier = deferred();
          pending.push({ ...barrier, value: structuredClone(value) });
          await barrier.promise;
        }
        await store.save(value);
      } finally {
        active -= 1;
      }
    }
  };
}

function nextTurn() {
  return new Promise((resolve) => setImmediate(resolve));
}

test("the sole pre-handler save includes a fresh operation before its handler runs", async () => {
  const store = delayedPersistence();
  const engine = engineApi.create(store);
  await engine.activate("service_one");
  store.delay();
  let calls = 0;
  const attempted = {
    epoch: "service_one",
    records: [{ id: "physical_one", phase: "dispatched" }]
  };
  const operation = engine.execute("physical_one", async () => {
    calls += 1;
    assert.deepEqual(store.value(), attempted, "the saved attempt precedes the handler");
    assert.equal(store.writes.length, 2, "activation plus one pre-handler save");
    return { outcome: "tab_closed", secret: "memory only" };
  });

  await nextTurn();
  assert.equal(calls, 0, "a pending save cannot enter the handler");
  assert.equal(store.pending.length, 1);
  assert.deepEqual(store.pending[0].value, attempted);
  store.pending[0].resolve();
  await nextTurn();
  assert.equal(calls, 1);
  assert.equal(store.pending.length, 2, "only terminal persistence follows the handler");
  store.pending[1].resolve();
  assert.deepEqual(await operation, { outcome: "tab_closed", secret: "memory only" });
  assert.doesNotMatch(JSON.stringify(store.writes), /memory only|tab_closed|secret/);
});

test("a failed pre-handler save forbids the handler and joined duplicates can retry safely", async () => {
  const store = delayedPersistence();
  const engine = engineApi.create(store);
  await engine.activate("service_one");
  store.delay();
  let calls = 0;
  const effect = async () => {
    calls += 1;
    return { outcome: "cancelled" };
  };
  const first = engine.execute("physical_one", effect);
  const duplicate = engine.execute("physical_one", effect);
  const failure = new Error("storage unavailable before effect");
  const rejected = Promise.all([
    assert.rejects(first, (error) => error === failure),
    assert.rejects(duplicate, (error) => error === failure)
  ]);

  await nextTurn();
  assert.equal(store.pending.length, 1, "duplicates share the pre-handler save");
  store.pending[0].reject(failure);
  await rejected;
  assert.equal(calls, 0);
  assert.deepEqual(engine.snapshot(), { epoch: "service_one", records: [] });

  const retried = engine.execute("physical_one", effect);
  await nextTurn();
  assert.equal(store.pending.length, 2, "a rejected save does not block later persistence");
  assert.equal(calls, 0);
  store.pending[1].resolve();
  await nextTurn();
  assert.equal(calls, 1);
  assert.equal(store.pending.length, 3);
  store.pending[2].resolve();
  assert.deepEqual(await retried, { outcome: "cancelled" });
});

test("same-epoch restart at the fresh handler boundary refuses re-entry", async () => {
  const store = persistence();
  const engine = engineApi.create(store);
  await engine.activate("service_one");
  const finish = deferred();
  let calls = 0;
  let boundary;
  const first = engine.execute("physical_one", () => {
    calls += 1;
    boundary = structuredClone(store.value());
    return finish.promise;
  });
  await nextTurn();
  assert.equal(calls, 1);
  assert.deepEqual(boundary, {
    epoch: "service_one",
    records: [{ id: "physical_one", phase: "dispatched" }]
  });

  const restored = engineApi.create(persistence(boundary));
  assert.equal(await restored.activate("service_one"), false);
  await assert.rejects(restored.execute("physical_one", async () => { calls += 1; }),
    (error) => error.code === "operation_result_unavailable" && error.effectUnknown === true);
  assert.equal(calls, 1, "the restored request never enters another handler");
  finish.resolve({ outcome: "cancelled" });
  await first;
});

test("overlapping commands and terminal saves preserve every attempted operation in order", async () => {
  const store = delayedPersistence();
  const engine = engineApi.create(store);
  await engine.activate("service_one");
  store.delay();
  const finishOne = deferred();
  const finishTwo = deferred();
  const calls = [0, 0];
  const first = engine.execute("physical_one", () => {
    calls[0] += 1;
    assert.deepEqual(store.value().records, [{ id: "physical_one", phase: "dispatched" }]);
    return finishOne.promise;
  });
  const second = engine.execute("physical_two", () => {
    calls[1] += 1;
    assert.deepEqual(store.value().records, [
      { id: "physical_one", phase: "dispatched" },
      { id: "physical_two", phase: "dispatched" }
    ]);
    return finishTwo.promise;
  });
  await nextTurn();
  assert.deepEqual(calls, [0, 0]);
  assert.equal(store.pending.length, 1, "the second save cannot overtake the first");
  assert.deepEqual(store.pending[0].value.records, [{ id: "physical_one", phase: "dispatched" }]);

  store.pending[0].resolve();
  await nextTurn();
  assert.deepEqual(calls, [1, 0]);
  assert.equal(store.pending.length, 2);
  assert.deepEqual(store.pending[1].value.records, [
    { id: "physical_one", phase: "dispatched" },
    { id: "physical_two", phase: "dispatched" }
  ]);
  store.pending[1].resolve();
  await nextTurn();
  assert.deepEqual(calls, [1, 1]);

  finishTwo.resolve({ outcome: "cancelled" });
  await nextTurn();
  finishOne.resolve({ outcome: "cancelled" });
  await nextTurn();
  assert.equal(store.pending.length, 3, "terminal saves use the same persistence order");
  assert.deepEqual(store.value().records, [
    { id: "physical_one", phase: "dispatched" },
    { id: "physical_two", phase: "dispatched" }
  ]);
  assert.deepEqual(store.pending[2].value.records, [
    { id: "physical_one", phase: "dispatched" },
    { id: "physical_two", phase: "completed" }
  ]);
  store.pending[2].resolve();
  await second;
  await nextTurn();
  assert.equal(store.pending.length, 4);
  assert.deepEqual(store.pending[3].value.records, [
    { id: "physical_one", phase: "completed" },
    { id: "physical_two", phase: "completed" }
  ]);
  store.pending[3].resolve();
  await first;
  assert.deepEqual(store.value(), engine.snapshot());
  assert.equal(store.maximumConcurrent(), 1);
  assert.deepEqual(calls, [1, 1]);
});

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
  await nextTurn();
  await engine.acknowledge("physical_one");
  assert.deepEqual(engine.snapshot(), {
    epoch: "service_one",
    records: [{ id: "physical_one", phase: "dispatched" }]
  }, "an in-flight acknowledgement cannot discard the attempt");
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

test("overlapping acknowledgements cannot restore an already removed operation record", async () => {
  const store = delayedPersistence();
  const engine = engineApi.create(store);
  await engine.activate("service_one");
  let calls = 0;
  const effect = async () => {
    calls += 1;
    return { outcome: "cancelled" };
  };
  await engine.execute("physical_one", effect);
  await engine.execute("physical_two", effect);
  store.delay();

  const first = engine.acknowledge("physical_one");
  const second = engine.acknowledge("physical_two");
  await nextTurn();
  assert.equal(store.pending.length, 1);
  assert.deepEqual(store.pending[0].value.records, [{ id: "physical_two", phase: "completed" }]);
  store.pending[0].resolve();
  await first;
  await nextTurn();
  assert.equal(store.pending.length, 2);
  assert.deepEqual(store.pending[1].value.records, []);
  store.pending[1].resolve();
  await second;
  assert.deepEqual(store.value(), { epoch: "service_one", records: [] });
  assert.equal(store.maximumConcurrent(), 1);
  assert.equal(calls, 2);
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
      assert.deepEqual(store.value().records, [{ id: `physical_${phase}`, phase: "dispatched" }]);
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
  assert.equal(await engine.activate("service_new"), true);
  assert.deepEqual(store.value(), { epoch: "service_new", records: [] });
  assert.equal(await engine.activate("service_new"), false);
});

test("the recovery ledger is bounded without evicting unacknowledged effects", async () => {
  const store = persistence();
  const engine = engineApi.create({ ...store, maximumRecords: 1 });
  await engine.activate("service_one");
  await engine.execute("physical_one", async () => ({ outcome: "cancelled" }));
  let calls = 0;
  await assert.rejects(
    engine.execute("physical_two", async () => { calls += 1; }),
    (error) => error.code === "operation_ledger_full" && error.effectUnknown === false
  );
  assert.equal(calls, 0);
  assert.deepEqual(engine.snapshot(), {
    epoch: "service_one",
    records: [{ id: "physical_one", phase: "completed" }]
  });
  await engine.acknowledge("physical_one");
  await engine.execute("physical_two", async () => {
    calls += 1;
    return { outcome: "cancelled" };
  });
  assert.equal(calls, 1, "acknowledgement makes bounded capacity available again");
});

test("same-engine terminal failures retain their error without entering the handler again", async () => {
  const store = persistence();
  const engine = engineApi.create(store);
  await engine.activate("service_one");
  let calls = 0;
  const failure = Object.assign(new Error("no browser effect"), { effectUnknown: false });
  const effect = async () => {
    calls += 1;
    throw failure;
  };
  await assert.rejects(engine.execute("physical_one", effect), (error) => error === failure);
  await assert.rejects(engine.execute("physical_one", effect), (error) => error === failure);
  assert.equal(calls, 1);
  assert.deepEqual(store.value(), {
    epoch: "service_one",
    records: [{ id: "physical_one", phase: "failed" }]
  });
  assert.doesNotMatch(JSON.stringify(store.value()), /no browser effect|effectUnknown/);
  await engine.acknowledge("physical_one");
  assert.deepEqual(store.value(), { epoch: "service_one", records: [] });
});

test("terminal persistence failure never turns a completed effect into failure", async () => {
  let writes = 0;
  const engine = engineApi.create({
    async load() { return null; },
    async save() {
      writes += 1;
      if (writes === 3) throw new Error("storage unavailable after effect");
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
