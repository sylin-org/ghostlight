"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const diagnosticsApi = require("../lib/diagnostics.js");

function clock() {
  let value = 1000;
  return () => {
    value += 1;
    return value;
  };
}

function consoleCall(text, type = "log") {
  return { type, args: [{ type: "string", value: text }] };
}

function request(requestId, url, method = "GET", type = "Fetch") {
  return { requestId, request: { url, method }, type };
}

function fakeTimers() {
  let nextId = 0;
  const active = new Map();
  return {
    setTimer(callback) {
      nextId += 1;
      active.set(nextId, callback);
      return nextId;
    },
    clearTimer(id) { active.delete(id); },
    fire(id) {
      const callback = active.get(id);
      if (!callback) return false;
      active.delete(id);
      callback();
      return true;
    },
    ids() { return Array.from(active.keys()); }
  };
}

test("capture is opt-in, defaults to problems from both sources, and never persists", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "test" });
  assert.equal(diagnostics.consoleAPICalled(7, consoleCall("before enable", "error")), false);

  assert.equal(diagnostics.enable(7), true);
  assert.equal(diagnostics.enable(7), false);
  diagnostics.consoleAPICalled(7, consoleCall("ordinary"));
  diagnostics.consoleAPICalled(7, consoleCall("watch this", "warning"));
  diagnostics.requestWillBeSent(7, request("one", "https://example.test/ok?secret=yes"));
  diagnostics.responseReceived(7, {
    requestId: "one",
    type: "Fetch",
    response: { url: "https://example.test/ok?secret=yes", status: 200 }
  });

  const first = diagnostics.read(7);
  assert.equal(first.capture_started, true);
  assert.deepEqual(first.entries.map((entry) => [entry.entry, entry.level]), [
    ["console", "warning"]
  ]);
  assert.equal(first.truncated, false);
  assert.equal(first.evicted, false);
  assert.equal(first.omitted_count, 0);
  assert.equal(diagnostics.read(7).capture_started, false);

  const restartedWorker = diagnosticsApi.create({ now: clock(), cursorSecret: "other" });
  const restarted = restartedWorker.read(7, { detail: "all" });
  assert.equal(restarted.capture_started, true);
  assert.deepEqual(restarted.entries, []);
  assert.equal(restarted.cursor, null);
});

test("literal matching is case-insensitive and does not interpret regular expressions", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "test" });
  diagnostics.enable(1);
  diagnostics.consoleAPICalled(1, consoleCall("Literal .* marker", "error"));
  diagnostics.consoleAPICalled(1, consoleCall("literal abc marker", "error"));

  const result = diagnostics.read(1, { match_text: ".*" });
  assert.deepEqual(result.entries.map((entry) => entry.text), ["Literal .* marker"]);
});

test("console evidence carries sanitized fail-closed source provenance", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "provenance" });
  diagnostics.enable(13);
  assert.equal(diagnostics.executionContextCreated(13, {
    context: { id: 41, origin: "https://context.test:8443" }
  }), true);
  assert.equal(diagnostics.executionContextCreated(13, {
    context: { id: 42, origin: "https://exception.test" }
  }), true);

  diagnostics.consoleAPICalled(13, {
    ...consoleCall("from context", "warning"),
    executionContextId: 41
  });
  diagnostics.consoleAPICalled(13, {
    ...consoleCall("from stack", "error"),
    executionContextId: 41,
    stackTrace: {
      callFrames: [{ url: "https://user:password@context.test:8443/app.js?token=secret#fragment" }]
    }
  });
  diagnostics.consoleAPICalled(13, {
    ...consoleCall("spoofed stack", "error"),
    executionContextId: 41,
    stackTrace: {
      callFrames: [{ url: "https://allowed-spoof.test/app.js" }]
    }
  });
  diagnostics.exceptionThrown(13, {
    exceptionDetails: {
      text: "from exception URL",
      url: "https://exception.test/source.js?private=yes#detail",
      executionContextId: 42
    }
  });
  diagnostics.consoleAPICalled(13, {
    ...consoleCall("unknown source", "error"),
    executionContextId: 999
  });

  const result = diagnostics.read(13, { source: "console", detail: "all" });
  assert.deepEqual(result.entries.map((entry) => entry.url), [
    "https://context.test:8443/",
    "https://context.test:8443/app.js",
    "https://context.test:8443/",
    "https://exception.test/source.js",
    "invalid:"
  ]);
  assert.doesNotMatch(JSON.stringify(result), /password|token|secret|private|fragment|detail|allowed-spoof/);
});

test("execution-context provenance is bounded and erased with its diagnostic state", () => {
  const diagnostics = diagnosticsApi.create({
    now: clock(),
    cursorSecret: "contexts",
    maximumExecutionContexts: 2
  });
  diagnostics.enable(14);
  for (const id of [1, 2, 3]) {
    diagnostics.executionContextCreated(14, {
      context: { id, origin: `https://context-${id}.test` }
    });
  }

  diagnostics.consoleAPICalled(14, { ...consoleCall("evicted", "error"), executionContextId: 1 });
  diagnostics.consoleAPICalled(14, { ...consoleCall("retained", "error"), executionContextId: 2 });
  assert.equal(diagnostics.executionContextDestroyed(14, { executionContextId: 2 }), true);
  diagnostics.consoleAPICalled(14, { ...consoleCall("destroyed", "error"), executionContextId: 2 });
  assert.equal(diagnostics.executionContextsCleared(14), true);
  diagnostics.consoleAPICalled(14, { ...consoleCall("cleared", "error"), executionContextId: 3 });

  assert.deepEqual(diagnostics.read(14, { detail: "all" }).entries.map((entry) => entry.url), [
    "invalid:",
    "https://context-2.test/",
    "invalid:",
    "invalid:"
  ]);
  assert.equal(diagnosticsApi.DEFAULT_MAX_EXECUTION_CONTEXTS, 256);

  diagnostics.executionContextCreated(14, { context: { id: 4, origin: "https://cleared.test" } });
  diagnostics.clear(14);
  diagnostics.consoleAPICalled(14, { ...consoleCall("after clear", "error"), executionContextId: 4 });
  assert.equal(diagnostics.read(14).entries[0].url, "invalid:");
});

test("reads are non-destructive and opaque cursors paginate in global event order", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "test" });
  diagnostics.enable(2);
  diagnostics.consoleAPICalled(2, consoleCall("first", "warning"));
  diagnostics.requestWillBeSent(2, request("two", "https://example.test/failure"));
  diagnostics.loadingFailed(2, { requestId: "two", type: "Fetch", errorText: "net::ERR_FAILED" });
  diagnostics.exceptionThrown(2, {
    exceptionDetails: { exception: { type: "object", description: "TypeError: third" } }
  });

  const firstPage = diagnostics.read(2, { limit: 1 });
  assert.equal(firstPage.entries[0].text, "first");
  assert.equal(firstPage.truncated, true);
  assert.match(firstPage.cursor, /^diag_[0-9a-z]+_[0-9a-f]{8}$/);
  assert.ok(firstPage.cursor.length <= diagnosticsApi.MAX_CURSOR_CHARS);

  const secondPage = diagnostics.read(2, { after: firstPage.cursor, limit: 2 });
  assert.deepEqual(secondPage.entries.map((entry) => entry.entry), ["network", "console"]);
  assert.equal(secondPage.truncated, false);
  assert.equal(diagnostics.read(2).entries.length, 3);

  const tampered = `${firstPage.cursor.slice(0, -1)}0`;
  assert.throws(() => diagnostics.read(2, { after: tampered }), /valid diagnostic cursor/);
  assert.throws(() => diagnostics.read(3, { after: firstPage.cursor }), /valid diagnostic cursor/);
});

test("a request that becomes a problem advances past an already-read pending cursor", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "test" });
  diagnostics.enable(3);
  diagnostics.requestWillBeSent(3, request("late", "https://example.test/late"));
  const pending = diagnostics.read(3, { source: "network", detail: "all" });
  assert.equal(pending.entries[0].status, null);

  diagnostics.responseReceived(3, {
    requestId: "late",
    type: "Fetch",
    response: { url: "https://example.test/late", status: 500 }
  });
  const completed = diagnostics.read(3, {
    source: "network",
    after: pending.cursor
  });
  assert.equal(completed.entries[0].status, 500);
  assert.notEqual(completed.cursor, pending.cursor);
});

test("an evicted cursor resumes at the oldest retained entry and reports the gap", () => {
  const diagnostics = diagnosticsApi.create({
    maximumEntries: 2,
    now: clock(),
    cursorSecret: "test"
  });
  diagnostics.enable(4);
  diagnostics.consoleAPICalled(4, consoleCall("one", "error"));
  const oldCursor = diagnostics.read(4).cursor;
  diagnostics.consoleAPICalled(4, consoleCall("two", "error"));
  diagnostics.consoleAPICalled(4, consoleCall("three", "error"));

  const result = diagnostics.read(4, { after: oldCursor });
  assert.equal(result.evicted, true);
  assert.deepEqual(result.entries.map((entry) => entry.text), ["two", "three"]);
});

test("entry, byte, text, cursor, and read bounds are enforced", () => {
  const diagnostics = diagnosticsApi.create({
    maximumEntries: 10,
    maximumBytes: 500,
    now: clock(),
    cursorSecret: "test"
  });
  diagnostics.enable(5);
  diagnostics.consoleAPICalled(5, consoleCall("x".repeat(2500), "error"));
  diagnostics.consoleAPICalled(5, consoleCall("kept", "error"));

  const result = diagnostics.read(5, { detail: "all" });
  assert.equal(result.evicted, true);
  assert.deepEqual(result.entries.map((entry) => entry.text), ["kept"]);
  assert.throws(() => diagnostics.read(5, { limit: 0 }), /limit/);
  assert.throws(() => diagnostics.read(5, { limit: 201 }), /limit/);
  assert.throws(() => diagnostics.read(5, { match_text: "x".repeat(501) }), /match_text/);
  assert.throws(() => diagnostics.read(5, { after: `diag_${"x".repeat(200)}` }), /160/);

  const textBound = diagnosticsApi.create({ now: clock(), cursorSecret: "bound" });
  textBound.enable(5);
  textBound.consoleAPICalled(5, consoleCall("x".repeat(2500), "error"));
  assert.equal(textBound.read(5).entries[0].text.length, diagnosticsApi.MAX_CONSOLE_CHARS);
});

test("network evidence contains only sanitized bounded facts and problems filter correctly", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "test" });
  diagnostics.enable(6);
  diagnostics.requestWillBeSent(6, request(
    "ok",
    "https://user:password@example.test/path/to?q=secret#fragment",
    "post",
    "XHR"
  ));
  diagnostics.responseReceived(6, {
    requestId: "ok",
    type: "XHR",
    response: {
      url: "https://user:password@example.test/path/to?q=secret#fragment",
      status: 204,
      headers: { authorization: "secret" }
    }
  });
  diagnostics.requestWillBeSent(6, request("bad", "https://example.test/bad?token=secret"));
  diagnostics.responseReceived(6, {
    requestId: "bad",
    type: "Fetch",
    response: { url: "https://example.test/bad?token=secret", status: 503 }
  });
  diagnostics.requestWillBeSent(6, request("failed", "https://example.test/offline#private"));
  diagnostics.loadingFailed(6, {
    requestId: "failed",
    type: "Fetch",
    errorText: "net::ERR_NAME_NOT_RESOLVED additional detail"
  });

  const problems = diagnostics.read(6);
  assert.deepEqual(problems.entries.map((entry) => [entry.url, entry.status, entry.failure]), [
    ["https://example.test/bad", 503, null],
    ["https://example.test/offline", null, "net::ERR_NAME_NOT_RESOLVED"]
  ]);
  assert.deepEqual(Object.keys(problems.entries[0]).sort(), [
    "cursor", "entry", "failure", "method", "resource_type", "status", "timestamp_ms", "url"
  ]);

  const all = diagnostics.read(6, { detail: "all" });
  assert.equal(all.entries[0].url, "https://example.test/path/to");
  assert.equal(all.entries[0].method, "POST");
  assert.doesNotMatch(JSON.stringify(all), /password|secret|authorization|fragment/);
});

test("host authority callback omits network detail with only a content-free count", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "test" });
  diagnostics.enable(8);
  diagnostics.requestWillBeSent(8, request("allowed", "https://allowed.test/fail"));
  diagnostics.loadingFailed(8, { requestId: "allowed", errorText: "net::ERR_FAILED" });
  diagnostics.requestWillBeSent(8, request("denied", "https://denied.test/fail"));
  diagnostics.loadingFailed(8, { requestId: "denied", errorText: "net::ERR_FAILED" });

  const result = diagnostics.read(8, {
    allowNetworkUrl: (url) => url.startsWith("https://allowed.test/")
  });
  assert.deepEqual(result.entries.map((entry) => entry.url), ["https://allowed.test/fail"]);
  assert.equal(result.omitted_count, 1);
  assert.doesNotMatch(JSON.stringify(result), /denied/);
});

test("clear, tab forget, and clear-all release volatile capture state", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "test" });
  diagnostics.enable(9);
  diagnostics.consoleAPICalled(9, consoleCall("temporary", "error"));
  assert.equal(diagnostics.clear(9), true);
  assert.equal(diagnostics.isEnabled(9), true);
  assert.deepEqual(diagnostics.read(9).entries, []);

  assert.equal(diagnostics.forget(9), true);
  assert.equal(diagnostics.isEnabled(9), false);
  diagnostics.enable(9);
  diagnostics.enable(10);
  assert.deepEqual(diagnostics.clearAll(), [9, 10]);
  assert.equal(diagnostics.isEnabled(9), false);
  assert.equal(diagnostics.isEnabled(10), false);
});

test("bulk teardown validates every tab id before forgetting bounded capture state", () => {
  const diagnostics = diagnosticsApi.create({ now: clock(), cursorSecret: "test" });
  diagnostics.enable(9);
  diagnostics.enable(10);

  assert.throws(() => diagnostics.forgetMany([]), /1 to 256/);
  assert.throws(() => diagnostics.forgetMany(new Array(257).fill(1)), /1 to 256/);
  assert.throws(() => diagnostics.forgetMany([9, 9]), /duplicates/);
  assert.throws(() => diagnostics.forgetMany([9, 0]), /positive safe integers/);
  assert.equal(diagnostics.isEnabled(9), true);

  assert.equal(diagnostics.forgetMany([9, 10, 11]), 2);
  assert.equal(diagnostics.isEnabled(9), false);
  assert.equal(diagnostics.isEnabled(10), false);
  assert.equal(diagnosticsApi.MAX_CLEAR_TABS, 256);
});

test("accepted events and reads refresh idle expiry before volatile cleanup", () => {
  const timers = fakeTimers();
  const expired = [];
  const diagnostics = diagnosticsApi.create({
    idleMs: 300000,
    now: clock(),
    cursorSecret: "test",
    setTimer: timers.setTimer,
    clearTimer: timers.clearTimer,
    onExpired: (tabId) => expired.push(tabId)
  });

  diagnostics.enable(11);
  const enabledTimer = timers.ids()[0];
  diagnostics.consoleAPICalled(11, consoleCall("still active", "error"));
  assert.equal(timers.fire(enabledTimer), false);
  const ingestionTimer = timers.ids()[0];
  diagnostics.read(11);
  assert.equal(timers.fire(ingestionTimer), false);
  assert.equal(diagnostics.isEnabled(11), true);

  assert.equal(timers.fire(timers.ids()[0]), true);
  assert.equal(diagnostics.isEnabled(11), false);
  assert.deepEqual(expired, [11]);
  assert.equal(diagnostics.consoleAPICalled(11, consoleCall("after expiry", "error")), false);
});

test("a delayed worker timer cannot let new traffic revive expired evidence", () => {
  const timers = fakeTimers();
  const expired = [];
  let now = 1000;
  const diagnostics = diagnosticsApi.create({
    idleMs: 100,
    now: () => now,
    cursorSecret: "delayed",
    setTimer: timers.setTimer,
    clearTimer: timers.clearTimer,
    onExpired: (tabId) => expired.push(tabId)
  });

  diagnostics.enable(12);
  diagnostics.consoleAPICalled(12, consoleCall("must expire", "error"));
  now += 101;
  assert.equal(diagnostics.consoleAPICalled(12, consoleCall("too late", "error")), false);
  assert.deepEqual(expired, [12]);

  const restarted = diagnostics.read(12);
  assert.equal(restarted.capture_started, true);
  assert.deepEqual(restarted.entries, []);
});
