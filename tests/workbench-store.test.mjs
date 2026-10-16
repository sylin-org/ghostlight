// The workbench cache, exercised for real.
//
// The store is the copy of the orchestrator the window draws from. A refresh asks the service for
// a snapshot; when that snapshot is at the sequence the cache already reached, the feed the cache
// built from streamed changes is current and must be kept. These tests run the real store with a
// recording announcer. No page, no transport and no Ghostlight process is involved.
//
// Run with: node --test tests/workbench-store.test.mjs
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { test } from "node:test";
import { join, resolve } from "node:path";

const require = createRequire(import.meta.url);
const ui = join(resolve(import.meta.dirname, ".."), "crates", "orchestrator", "ui", "lib");
require(join(ui, "words.js"));
require(join(ui, "entries.js"));
const Store = require(join(ui, "store.js"));

/** A snapshot at `seq` holding one settled record. */
function snapshotAt(seq) {
  return {
    seq,
    service: { runtime_state: "active" },
    operations: [],
    history: [{ invocation: "one", workspace: "w", tool: "browser_read", status: "succeeded", allowed: true, complete: true, timestamp_ms: 1, summary: "Read a page." }],
    sessions: []
  };
}

/** A store that records every kind it announces. */
function recording() {
  const kinds = [];
  const store = Store.create({ announce: (kind) => kinds.push(kind) });
  return { store, kinds };
}

test("a snapshot at the cache's own sequence does not need a rebuild", () => {
  const { store, kinds } = recording();
  store.applySnapshot(snapshotAt(4), store.needsRebuild(snapshotAt(4)));
  assert.equal(store.applyChange({ seq: 5, change: { kind: "audit_health_changed", health: {} } }), "refresh");
  assert.equal(store.seq(), 5);
  kinds.length = 0;

  const refreshed = snapshotAt(5);
  assert.equal(store.needsRebuild(refreshed), false);
  store.applySnapshot(refreshed, store.needsRebuild(refreshed));
  assert.ok(!kinds.includes(Store.CHANGE.Feed), "the feed is kept");
});

test("a snapshot at another sequence rebuilds the feed", () => {
  const { store, kinds } = recording();
  store.applySnapshot(snapshotAt(4), store.needsRebuild(snapshotAt(4)));
  store.applyChange({ seq: 5, change: { kind: "audit_health_changed", health: {} } });
  kinds.length = 0;

  const ahead = snapshotAt(7);
  assert.equal(store.needsRebuild(ahead), true);
  store.applySnapshot(ahead, store.needsRebuild(ahead));
  assert.ok(kinds.includes(Store.CHANGE.Feed), "the feed is rebuilt");
});

test("the first snapshot always builds the feed", () => {
  const { store, kinds } = recording();
  const first = snapshotAt(0);
  assert.equal(store.needsRebuild(first), true);
  store.applySnapshot(first, store.needsRebuild(first));
  assert.ok(kinds.includes(Store.CHANGE.Feed));
});
