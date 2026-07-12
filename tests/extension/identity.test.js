// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/identity.js (ADR-0061 extension-owned browser identity). Pure
// module: an injected fake storage area and a deterministic generator stand in for
// chrome.storage.local / crypto.randomUUID, matching lib/debug.js's injected-dependency precedent.

const { test } = require("node:test");
const assert = require("node:assert");
const { createBrowserIdentity, STORAGE_KEY } = require("../../extension/lib/identity.js");

function fakeStorage(initial) {
  const value = { ...initial };
  return {
    async get(key) {
      return { [key]: value[key] };
    },
    async set(next) {
      Object.assign(value, next);
    },
    peek() {
      return { ...value };
    },
  };
}

test("mints a fresh id when storage has none, and persists it", async () => {
  const storage = fakeStorage({});
  const identity = createBrowserIdentity(storage, () => "uuid-1");
  const id = await identity.get();
  assert.strictEqual(id, "uuid-1");
  assert.strictEqual(storage.peek()[STORAGE_KEY], "uuid-1", "the minted id is persisted");
});

test("reads back an already-persisted id rather than minting a new one", async () => {
  const storage = fakeStorage({ [STORAGE_KEY]: "existing-uuid" });
  let generated = 0;
  const identity = createBrowserIdentity(storage, () => {
    generated += 1;
    return "should-not-be-used";
  });
  const id = await identity.get();
  assert.strictEqual(id, "existing-uuid");
  assert.strictEqual(generated, 0, "an existing id is never regenerated");
});

test("is stable within a worker lifetime: repeated get() returns the same id and mints once", async () => {
  const storage = fakeStorage({});
  let generated = 0;
  const identity = createBrowserIdentity(storage, () => `uuid-${++generated}`);
  const first = await identity.get();
  const second = await identity.get();
  assert.strictEqual(first, second);
  assert.strictEqual(generated, 1, "the generator runs exactly once, then the value is cached");
});

test("a fresh instance (new worker) reads the persisted id, keeping identity stable across restarts", async () => {
  const storage = fakeStorage({});
  const first = await createBrowserIdentity(storage, () => "uuid-persisted").get();
  // Simulate a service-worker restart: a brand-new instance over the SAME (local) storage.
  const second = await createBrowserIdentity(storage, () => "uuid-different").get();
  assert.strictEqual(second, first, "the persisted id survives the restart");
});

test("still returns a usable id when storage.get throws (degraded, in-memory only)", async () => {
  const brokenStorage = {
    async get() {
      throw new Error("storage unavailable");
    },
    async set() {
      /* no-op */
    },
  };
  const id = await createBrowserIdentity(brokenStorage, () => "uuid-fallback").get();
  assert.strictEqual(id, "uuid-fallback");
});

test("returns a usable id even when persistence fails, and does not re-mint within the session", async () => {
  let generated = 0;
  const writeOnlyBroken = {
    async get() {
      return {};
    },
    async set() {
      throw new Error("cannot persist");
    },
  };
  const identity = createBrowserIdentity(writeOnlyBroken, () => `uuid-${++generated}`);
  const first = await identity.get();
  const second = await identity.get();
  assert.strictEqual(first, "uuid-1");
  assert.strictEqual(second, "uuid-1", "cached in memory despite the failed persist");
  assert.strictEqual(generated, 1);
});
