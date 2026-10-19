// The workbench cache, exercised for real.
//
// The store is the copy of the orchestrator the window draws from. A refresh asks the service for
// a snapshot; when that snapshot is at the sequence the cache already reached, the feed the cache
// built from streamed changes is current and must be kept. Every change the service publishes is
// folded in, and the band's working word latches past each quick action. These tests run the real
// store with a recording announcer and, where time matters, a clock and timers the test moves. No
// page, no transport and no Ghostlight process is involved.
//
// Run with: node --test tests/workbench-store.test.mjs
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { test } from "node:test";
import { join, resolve } from "node:path";

const require = createRequire(import.meta.url);
const ui = join(resolve(import.meta.dirname, ".."), "crates", "orchestrator", "ui", "lib");
const Words = require(join(ui, "words.js"));
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

/* ------------------------------ the changes it folds in ------------------------------ */

const operation = (phase = "running") => ({
  invocation: "two", workspace: "w", tool: "browser_click", activity: "Clicking", capability: "action",
  started_at_ms: 1, phase
});
const record = { ...snapshotAt(0).history[0], invocation: "two", tool: "browser_click", capability: "action" };
const status = {
  service: { version: "2.0.0", started_at_ms: 1, runtime_state: "held" },
  readiness: { state: "ready", word: "Ready" },
  audit_health: { state: "saved" },
  audit_notice: "",
  configuration: { policy: { tone: "applied" } }
};

// Every kind of change the service publishes (WorkbenchChange in workbench/mod.rs), each with the
// payload it carries. A kind the store does not fold in reads as ignored, and the window would
// draw nothing for it. The service's copy of these kinds is pinned by
// every_change_the_orchestrator_publishes_carries_the_kind_the_window_reads in workbench/tests.rs.
const PUBLISHED = [
  { kind: "audit_health_changed", health: { state: "saved" } },
  { kind: "document_coverage_changed", details: { invocation: "one", summary: "Read the page." } },
  { kind: "operation_started", operation: operation() },
  { kind: "operation_changed", operation: operation("waiting") },
  { kind: "operation_settled", record },
  { kind: "composition_changed", record },
  { kind: "runtime_changed", runtime_state: "held" },
  { kind: "status_changed", status },
  { kind: "connections_changed", connections: { sessions: [{ id: "w" }], browsers: [] } },
  { kind: "controls_changed", controls: { you: { context: "control" } } },
  { kind: "part_changed", part: "integrations", etag: "e1" }
];

/** A store on a clock the test moves, with timers it fires by hand. */
function latched(extra = {}) {
  const clock = { at: 100_000 };
  const timers = new Map();
  let next = 0;
  const kinds = [];
  const store = Store.create({
    announce: (kind) => kinds.push(kind),
    now: () => clock.at,
    setTimer: (work, ms) => {
      next += 1;
      timers.set(next, { work, ms });
      return next;
    },
    clearTimer: (handle) => timers.delete(handle)
  });
  store.applySnapshot({ ...snapshotAt(0), ...extra }, true);
  return { store, clock, timers, kinds };
}

test("every change the service publishes is folded in, and a kind it never publishes is ignored", () => {
  const { store } = latched();
  PUBLISHED.forEach((change, index) => {
    assert.notEqual(store.applyChange({ seq: index + 1, change }), "ignored", change.kind);
  });
  assert.equal(store.seq(), PUBLISHED.length);
  assert.equal(store.applyChange({ seq: PUBLISHED.length + 1, change: { kind: "made_up" } }), "ignored");
});

test("each published change moves the cache it names", () => {
  const { store } = latched({ browsers: [], configuration: {}, overview: {} });
  const apply = (seq, change) => store.applyChange({ seq, change });
  assert.equal(apply(1, PUBLISHED[2]), "applied");
  assert.equal(store.hero().invocation, "two");
  assert.equal(store.hero().settled, false);
  assert.equal(apply(2, PUBLISHED[4]), "applied");
  assert.equal(store.hero().settled, true);
  assert.equal(apply(3, PUBLISHED[6]), "applied");
  assert.equal(store.runtime(), "held");
  assert.equal(apply(4, PUBLISHED[8]), "applied");
  assert.deepEqual(store.snapshot().sessions, [{ id: "w" }]);
  assert.equal(apply(5, PUBLISHED[9]), "applied");
  assert.deepEqual(store.snapshot().controls, { you: { context: "control" } });
  assert.equal(apply(6, PUBLISHED[0]), "refresh");
  assert.equal(apply(7, PUBLISHED[10]), "hinted");
});

test("the no-browser line follows the connections part, and goes when a browser connects", () => {
  const { store } = latched({ browsers: [], configuration: {}, overview: {} });
  const line = "Starts Google Chrome on request";
  store.applyChange({ seq: 1, change: { kind: "connections_changed", connections: { sessions: [], browsers: [], no_browser: line } } });
  assert.equal(store.snapshot().no_browser, line);
  store.applyChange({ seq: 2, change: { kind: "connections_changed", connections: { sessions: [], browsers: [{ id: "b" }] } } });
  assert.equal("no_browser" in store.snapshot(), false);
});

test("the rows in use follow the connections part, as the service publishes them with the sessions", () => {
  const { store } = latched({ browsers: [], configuration: {}, overview: {}, harnesses_in_use: [] });
  const session = { id: "w", agent: { id: "codex" } };
  store.applyChange({ seq: 1, change: { kind: "connections_changed",
    connections: { sessions: [session], browsers: [], harnesses_in_use: ["codex"] } } });
  assert.deepEqual(store.snapshot().harnesses_in_use, ["codex"]);
  store.applyChange({ seq: 2, change: { kind: "connections_changed", connections: { sessions: [], browsers: [], harnesses_in_use: [] } } });
  assert.deepEqual(store.snapshot().harnesses_in_use, []);
});

/* ------------------------------------ the latch ------------------------------------ */

test("the band stays working for ten seconds after the last action, not for each operation", () => {
  const { store, clock } = latched();
  assert.equal(store.band().working, false, "nothing has happened yet");
  store.applyChange({ seq: 1, change: PUBLISHED[2] });
  assert.equal(store.band().working, true);
  clock.at += 300;
  store.applyChange({ seq: 2, change: PUBLISHED[4] });
  assert.equal(store.band().running, 0, "the operation settled");
  clock.at += Words.WORKING_LATCH_MS - 1;
  assert.equal(store.band().working, true, "the word holds through a quick settle");
  clock.at += 1;
  assert.equal(store.band().working, false, "the latch ends ten seconds after the last action");
  assert.equal(Words.WORKING_LATCH_MS, 10_000);
});

test("the latch schedules its own end, and each new action pushes it back", () => {
  const { store, clock, timers, kinds } = latched();
  store.applyChange({ seq: 1, change: PUBLISHED[2] });
  assert.equal(timers.size, 1);
  const [first] = timers.values();
  assert.ok(first.ms >= Words.WORKING_LATCH_MS, "the band is woken after the latch has run out");
  clock.at += 5_000;
  store.applyChange({ seq: 2, change: PUBLISHED[3] });
  assert.equal(timers.size, 1, "the earlier deadline is replaced, not kept beside the new one");
  clock.at += 6_000;
  assert.equal(store.band().working, true, "the deadline counts from the later action");
  kinds.length = 0;
  const [last] = timers.values();
  last.work();
  assert.deepEqual(kinds, [Store.CHANGE.Band], "nothing else wakes the band once the work settles");
});

test("only an action refreshes the latch: a runtime or status change does not", () => {
  const { store, clock } = latched();
  store.applyChange({ seq: 1, change: PUBLISHED[5] });
  clock.at += 9_000;
  store.applyChange({ seq: 2, change: PUBLISHED[6] });
  store.applyChange({ seq: 3, change: PUBLISHED[7] });
  clock.at += 1_000;
  assert.equal(store.band().working, false);
});

/* ---------------------------------- attribution ---------------------------------- */

test("a workspace resolves to its own session, even when several share one client", () => {
  const { store } = recording();
  const codex = { id: "codex", name: "Codex", symbol: { icon: "agent-icons/codex?v=fedcba987654" } };
  const sessions = [
    { id: "w1", agent: codex, channel: "mcp" },
    { id: "w2", agent: codex, channel: "mcp" },
    { id: "w3", agent: { id: "zed", name: "Zed", symbol: { icon: "agent-icons/zed?v=fedcba987654" } }, channel: "mcp" }
  ];
  store.applySnapshot({ ...snapshotAt(0), sessions }, true);
  assert.equal(store.sessionFor("w2"), sessions[1]);
  assert.equal(store.sessionFor("w3").agent.name, "Zed");
  assert.equal(store.sessionFor("gone"), null);
});

test("a settled action adds to the recorded count past the list's length, up to what the service holds", () => {
  const { store } = recording();
  const held = (count) => Array.from({ length: count }, (_, index) => ({
    invocation: `old-${index}`, workspace: "w", tool: "browser_read", status: "succeeded", allowed: true, complete: true,
    timestamp_ms: 1, summary: "Read a page."
  }));
  const snapshot = { ...snapshotAt(1), history: held(410) };
  store.applySnapshot(snapshot, true);
  const settled = { ...held(1)[0], invocation: "new", timestamp_ms: 2 };
  assert.equal(store.applyChange({ seq: 2, change: { kind: "operation_settled", record: settled } }), "applied");
  assert.equal(store.snapshot().history.length, 411, "the band says what the service records, not what the list shows");
  assert.ok(Words.HISTORY_HELD > Words.FEED_LIMIT);

  store.applySnapshot({ ...snapshotAt(3), history: held(Words.HISTORY_HELD) }, true);
  store.applyChange({ seq: 4, change: { kind: "operation_settled", record: settled } });
  assert.equal(store.snapshot().history.length, Words.HISTORY_HELD, "no more than the service holds");
});
