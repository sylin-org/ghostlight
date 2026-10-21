// The workbench transport, exercised for real.
//
// The transport is the only thing the page uses to talk to the service, and it has one road: JSON
// over loopback, in the Ghostlight window and in a browser tab. These tests run the real module
// with a fake fetch that records what it was asked. No page, no document and no Ghostlight
// process is involved.
//
// Run with: node --test tests/workbench-transport.test.mjs
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { test } from "node:test";
import { join, resolve } from "node:path";

const require = createRequire(import.meta.url);
const ui = join(resolve(import.meta.dirname, ".."), "crates", "orchestrator", "ui", "lib");
const Transport = require(join(ui, "transport.js"));
const Words = require(join(ui, "words.js"));

const PARTS = "status,connections,activity,integrations,diagnostics,controls";

/** A response the fake service gives. */
function answer(status, body, headers = {}) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: (name) => headers[name.toLowerCase()] ?? null },
    text: async () => (body === undefined ? "" : JSON.stringify(body))
  };
}

/** A fake fetch that records each call and answers from `respond`. */
function fakeFetch(respond = () => answer(200, {})) {
  const calls = [];
  const send = async (path, init = {}) => {
    calls.push({ path, ...init });
    return respond(path, init);
  };
  send.calls = calls;
  return send;
}

const parts = {
  status: {
    service: { version: "2.0.0", started_at_ms: 1, runtime_state: "active" },
    readiness: { state: "ready", word: "Ready" },
    audit_health: { state: "saved" },
    audit_notice: "",
    configuration: { policy: { tone: "applied" }, managed_policy: { configured: false } },
    runtime_note: "Paused from the tray."
  },
  connections: {
    sessions: [{ id: "w1" }], browsers: [{ id: "b1" }, { id: "b2" }], harnesses_in_use: ["codex"],
    counts: { agents: 1, mcp_clients: 1, command_line: 0, browsers: 2 }
  },
  activity: {
    operations: [{ invocation: "a" }],
    history: [{ invocation: "b", allowed: true }, { invocation: "c", allowed: false }],
    document_coverage: [{ invocation: "b" }]
  },
  integrations: { harnesses: [{ id: "codex" }] },
  diagnostics: { diagnostics: [{ label: "Ghostlight" }] },
  controls: { viewing: { watch: true }, you: { context: "control" } }
};

test("a snapshot is one GET naming the six parts, put back into today's flat shape", async () => {
  const send = fakeFetch(() => answer(200, { seq: 9, instance: "i1", parts }));
  const transport = Transport.overHttp({ fetch: send });
  const snapshot = await transport.snapshot();
  assert.equal(send.calls.length, 1);
  assert.equal(send.calls[0].method, "GET");
  assert.equal(send.calls[0].path, `/api/snapshot?parts=${PARTS}`);
  assert.equal(send.calls[0].body, undefined);
  assert.deepEqual(Object.keys(snapshot).sort(), [
    "audit_health", "audit_notice", "browsers", "configuration", "controls", "counts", "diagnostics",
    "document_coverage", "harnesses", "harnesses_in_use", "history", "instance", "operations",
    "readiness", "runtime_note", "seq", "service", "sessions"
  ]);
  assert.deepEqual(snapshot.controls, parts.controls);
  assert.deepEqual(snapshot.harnesses_in_use, ["codex"], "the rows the service says are in use");
  assert.deepEqual(snapshot.counts, parts.connections.counts, "what the service counted, as the tray says it");
  assert.equal(snapshot.seq, 9);
  assert.equal(snapshot.service.runtime_state, "active");
  assert.equal(snapshot.runtime_note, "Paused from the tray.");
});

test("a snapshot of a running service has no pause note", () => {
  const { runtime_note: _, ...status } = parts.status;
  const flat = Transport.flatten({ seq: 1, parts: { ...parts, status } });
  assert.equal("runtime_note" in flat, false);
});

test("a snapshot with no browser carries the line the service wrote for it", () => {
  const connections = { ...parts.connections, browsers: [], no_browser: "Starts Google Chrome on request" };
  const flat = Transport.flatten({ seq: 1, parts: { ...parts, connections } });
  assert.equal(flat.no_browser, "Starts Google Chrome on request");
  assert.equal("no_browser" in Transport.flatten({ seq: 1, parts }), false);
});

test("the table of parts is the one the service lists", () => {
  assert.deepEqual([...Transport.SNAPSHOT_PARTS], PARTS.split(","));
});

test("a search is a POST whose body holds the words and whose address holds none", async () => {
  const send = fakeFetch(() => answer(200, []));
  const transport = Transport.overHttp({ fetch: send });
  await transport.search("login page");
  const [call] = send.calls;
  assert.equal(call.method, "POST");
  assert.equal(call.path, "/api/search");
  assert.equal(call.body, '{"query":"login page"}');
  assert.equal(call.headers["Content-Type"], "application/json");
  assert.equal(call.credentials, "omit");
  assert.equal(call.cache, "no-store");
});

test("an image is asked for with the key and handed back as a source the page can draw", async () => {
  const bytes = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x7a]);
  const respond = () => ({
    ok: true,
    status: 200,
    headers: { get: (name) => (name.toLowerCase() === "content-type" ? "image/png" : null) },
    arrayBuffer: async () => bytes.buffer
  });
  const keyed = fakeFetch(respond);
  const source = await Transport.overHttp({ fetch: keyed, bearer: "secret-1" }).image("agent-icons/zed?v=0123456789ab");
  assert.equal(keyed.calls[0].method, "GET");
  assert.equal(keyed.calls[0].path, "/agent-icons/zed?v=0123456789ab");
  assert.equal(keyed.calls[0].headers.Authorization, "Bearer secret-1");
  assert.equal(keyed.calls[0].credentials, "omit");
  assert.equal(source, `data:image/png;base64,${Buffer.from(bytes).toString("base64")}`);

  const open = fakeFetch(respond);
  await Transport.overHttp({ fetch: open }).image("agent-icons/zed");
  assert.equal("Authorization" in open.calls[0].headers, false, "a tab sends no key");

  const large = new Uint8Array(100000).fill(7);
  const big = await Transport.overHttp({
    fetch: fakeFetch(() => ({ ok: true, status: 200, headers: { get: () => "image/png" }, arrayBuffer: async () => large.buffer }))
  }).image("agent-icons/zed");
  assert.equal(big, `data:image/png;base64,${Buffer.from(large).toString("base64")}`, "an image larger than one chunk");
});

test("a monogram the service serves at an agent's address is drawn the same way as an app's icon", async () => {
  const svg = new TextEncoder().encode('<svg xmlns="http://www.w3.org/2000/svg"/>');
  const send = fakeFetch(() => ({
    ok: true, status: 200, headers: { get: () => "image/svg+xml" }, arrayBuffer: async () => svg.buffer
  }));
  const source = await Transport.overHttp({ fetch: send, bearer: "secret-1" }).image("agent-icons/codex?v=fedcba987654");
  assert.equal(send.calls[0].path, "/agent-icons/codex?v=fedcba987654");
  assert.equal(send.calls[0].headers.Authorization, "Bearer secret-1");
  assert.equal(source, `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`);
});

test("an image the service refuses rejects with its sentence", async () => {
  const send = fakeFetch(() => answer(403, { error: { code: "watching_off", message: "Watching is off." } }));
  await assert.rejects(
    Transport.overHttp({ fetch: send }).image("agent-icons/zed"),
    (refusal) => refusal.status === 403 && refusal.code === "watching_off" && String(refusal) === "Watching is off."
  );
});

test("an intent is a POST with the key, and without a key no Authorization is sent", async () => {
  const keyed = fakeFetch(() => answer(200, { accepted: true }));
  await Transport.overHttp({ fetch: keyed, bearer: "secret-1" }).applyIntent("hold");
  assert.equal(keyed.calls[0].method, "POST");
  assert.equal(keyed.calls[0].path, "/api/runtime");
  assert.equal(keyed.calls[0].body, '{"intent":"hold"}');
  assert.equal(keyed.calls[0].headers.Authorization, "Bearer secret-1");

  const open = fakeFetch(() => answer(200, { accepted: true }));
  await Transport.overHttp({ fetch: open }).applyIntent("hold");
  assert.equal("Authorization" in open.calls[0].headers, false);
});

test("a read carries the key too", async () => {
  const send = fakeFetch(() => answer(200, { context: "control" }));
  await Transport.overHttp({ fetch: send, bearer: "secret-1" }).context();
  assert.equal(send.calls[0].method, "GET");
  assert.equal(send.calls[0].path, "/api/context");
  assert.equal(send.calls[0].headers.Authorization, "Bearer secret-1");
  assert.equal("Content-Type" in send.calls[0].headers, false);
});

test("a policy write quotes the tag of the rules it was drawn from", async () => {
  const send = fakeFetch(() => answer(200, { accepted: true, message: "Applied." }));
  const transport = Transport.overHttp({ fetch: send, bearer: "k" });
  await transport.applyPolicy("{}", "tag-1");
  await transport.removePolicy("tag-2");
  const [put, remove] = send.calls;
  assert.equal(put.method, "PUT");
  assert.equal(put.path, "/api/policy/user");
  assert.equal(put.body, '{"document":"{}"}');
  assert.equal(put.headers["If-Match"], '"tag-1"');
  assert.equal(remove.method, "DELETE");
  assert.equal(remove.path, "/api/policy/user");
  assert.equal(remove.body, undefined);
  assert.equal(remove.headers["If-Match"], '"tag-2"');
});

test("a client act carries the state the client was drawn in", async () => {
  const send = fakeFetch(() => answer(200, { changed: true }));
  await Transport.overHttp({ fetch: send, bearer: "k" }).manageHarness("codex", "install", "available");
  assert.equal(send.calls[0].path, "/api/integrations/act");
  assert.equal(send.calls[0].body, '{"id":"codex","action":"install","expected_state":"available"}');
});

test("the other acts go where the table sends them", async () => {
  const send = fakeFetch(() => answer(200, {}));
  const transport = Transport.overHttp({ fetch: send, bearer: "k" });
  await transport.previewPolicy("{}");
  await transport.setDiagnostics(true);
  await transport.setWatching(false);
  await transport.refreshHarnesses();
  await transport.setupDetectedHarnesses();
  await transport.locateHarness("codex");
  await transport.testNotification();
  await transport.revealDiagnostics();
  await transport.continueInWindow();
  await transport.continueInWindow("settings/diagnostics");
  await transport.policy();
  await transport.quit();
  const seen = send.calls.map((call) => `${call.method} ${call.path} ${call.body ?? ""}`.trim());
  assert.deepEqual(seen, [
    'POST /api/policy/preview {"document":"{}"}',
    'PUT /api/controls/diagnostics {"on":true}',
    'PUT /api/controls/viewing {"watch":false}',
    "POST /api/integrations/rescan {}",
    "POST /api/integrations/setup-all {}",
    'POST /api/integrations/locate {"id":"codex"}',
    "POST /api/notifications/test {}",
    "POST /api/diagnostics/reveal {}",
    'POST /api/window {"view":"workbench"}',
    'POST /api/window {"view":"workbench","address":"settings/diagnostics"}',
    "GET /api/policy",
    "POST /api/quit {}"
  ]);
});

test("the setup part is read alone, and a screen the service leaves it out for reads null", async () => {
  const setup = { stage: "browser", rows: [] };
  const send = fakeFetch(() => answer(200, { seq: 3, instance: "i1", parts: { setup } }));
  const transport = Transport.overHttp({ fetch: send, bearer: "k" });
  assert.deepEqual(await transport.setupPart(), setup);
  assert.equal(send.calls.length, 1);
  assert.equal(send.calls[0].method, "GET");
  assert.equal(send.calls[0].path, "/api/snapshot?parts=setup");
  assert.equal(send.calls[0].body, undefined);
  assert.equal(send.calls[0].headers.Authorization, "Bearer k");
  // The window's snapshot does not ask for it: an unfinished setup reads this computer.
  assert.ok(!Transport.SNAPSHOT_PARTS.includes("setup"));

  const watcher = fakeFetch(() => answer(200, { seq: 3, instance: "i1", parts: {} }));
  assert.equal(await Transport.overHttp({ fetch: watcher }).setupPart(), null);
});

test("a setup step is one POST of the step itself, and the answer is the setup part", async () => {
  const step = { kind: "open_browser", browser: "edge", profile: "Profile 1" };
  const send = fakeFetch(() => answer(200, { stage: "browser" }));
  const transport = Transport.overHttp({ fetch: send, bearer: "k" });
  assert.deepEqual(await transport.setupAct(step), { stage: "browser" });
  assert.equal(send.calls[0].method, "POST");
  assert.equal(send.calls[0].path, "/api/setup/act");
  assert.deepEqual(JSON.parse(send.calls[0].body), step);
  assert.equal(send.calls[0].headers["Content-Type"], "application/json");
  assert.equal(Transport.ACT.setupAct, "setup_act");

  const refused = Transport.overHttp({ fetch: fakeFetch(() => answer(422, { error: { code: "refused", message: "Edge is no longer offered." } })) });
  await assert.rejects(refused.setupAct(step), (error) => error.message === "Edge is no longer offered." && error.status === 422);
});

test("quitting is one POST with an empty body, and the sentence the service answers is handed back", async () => {
  const send = fakeFetch(() => answer(202, { message: "Ghostlight is quitting." }));
  const transport = Transport.overHttp({ fetch: send, bearer: "k" });
  const result = await transport.quit();
  assert.equal(send.calls.length, 1);
  assert.equal(send.calls[0].method, "POST");
  assert.equal(send.calls[0].path, "/api/quit");
  assert.equal(send.calls[0].body, "{}");
  assert.equal(send.calls[0].headers.Authorization, "Bearer k");
  assert.equal(send.calls[0].headers["Content-Type"], "application/json");
  assert.deepEqual(result, { message: "Ghostlight is quitting." });

  const refused = Transport.overHttp({ fetch: fakeFetch(() => answer(401, { error: { code: "needs_window", message: "Change this in the Ghostlight window." } })) });
  await assert.rejects(refused.quit(), (error) => error.code === "needs_window" && error.status === 401);
});

test("opening a destination over HTTP is one POST naming it", async () => {
  const send = fakeFetch(() => answer(200, {}));
  const transport = Transport.overHttp({ fetch: send, bearer: "k" });
  await transport.openDestination("home");
  await transport.openHarnessDownload("cline-vscode");
  assert.equal(send.calls.length, 2);
  assert.equal(send.calls[0].method, "POST");
  assert.equal(send.calls[0].path, "/api/open");
  assert.equal(send.calls[0].body, '{"destination":"home"}');
  assert.equal(send.calls[1].body, '{"harness":"cline-vscode"}');
});

test("a success with no body resolves null, and an act hands over the change it caused", async () => {
  const nothing = Transport.overHttp({ fetch: fakeFetch(() => answer(200, undefined)) });
  assert.equal(await nothing.testNotification(), null);

  const caused = Transport.overHttp({ fetch: fakeFetch(() => answer(200, { accepted: true }, { "ghostlight-seq": "42" })) });
  const result = await caused.applyIntent("hold");
  assert.equal(Transport.seqOf(result), 42);
  assert.deepEqual(result, { accepted: true });
  assert.equal(JSON.stringify(result), '{"accepted":true}');

  const roster = Transport.overHttp({ fetch: fakeFetch(() => answer(200, [{ id: "codex" }], { "ghostlight-seq": "7" })) });
  assert.equal(Transport.seqOf(await roster.refreshHarnesses()), 7);

  const plain = Transport.overHttp({ fetch: fakeFetch(() => answer(200, { accepted: true })) });
  assert.equal(Transport.seqOf(await plain.applyIntent("hold")), undefined);
});

test("an error envelope rejects with exactly its message, and carries its code", async () => {
  const message = "Your rules changed in another window; your draft is kept.";
  const send = fakeFetch(() => answer(412, { error: { code: "stale", message } }));
  const transport = Transport.overHttp({ fetch: send });
  await assert.rejects(transport.applyPolicy("{}", "t"), (error) => {
    assert.equal(String(error), message);
    assert.equal(error.message, message);
    assert.equal(error.code, Transport.STALE);
    assert.equal(error.status, 412);
    return true;
  });
});

test("a failure with no envelope still rejects, and a network failure rejects with the fetch error", async () => {
  const bare = Transport.overHttp({ fetch: fakeFetch(() => answer(500, undefined)) });
  await assert.rejects(bare.search("x"), (error) => String(error) === "HTTP 500");
  const broken = Transport.overHttp({ fetch: async () => { throw new TypeError("Failed to fetch"); } });
  await assert.rejects(broken.search("x"), (error) => error instanceof TypeError && error.message === "Failed to fetch");
});

test("without a fetch there is no orchestrator", async () => {
  const transport = Transport.overHttp({});
  assert.equal(transport.available, false);
  await assert.rejects(transport.snapshot(), /no orchestrator/);
});

test("copying writes to the clipboard handed in", async () => {
  const written = [];
  const transport = Transport.overHttp({ fetch: fakeFetch(), clipboard: { writeText: async (text) => written.push(text) } });
  await transport.copyText("claude mcp add");
  assert.deepEqual(written, ["claude mcp add"]);
  await assert.rejects(Transport.overHttp({ fetch: fakeFetch() }).copyText("x"), /no clipboard/);
});

/* ------------------------------- the live stream ------------------------------ */

/** A stream the test writes into, chunk by chunk. */
function fakeStream() {
  const encoder = new TextEncoder();
  const queued = [];
  let waiting = null;
  let aborted = null;
  const settle = (result) => {
    const resolve = waiting;
    waiting = null;
    resolve?.(result);
  };
  return {
    write(text) {
      const value = encoder.encode(text);
      if (waiting) settle({ done: false, value });
      else queued.push(value);
    },
    end: () => settle({ done: true }),
    reader: {
      read: () => {
        if (queued.length) return Promise.resolve({ done: false, value: queued.shift() });
        return new Promise((resolve, reject) => {
          waiting = resolve;
          aborted = reject;
        });
      }
    },
    abort: (error) => aborted?.(error)
  };
}

/** A fetch that opens one fake stream per call and a scheduler whose timers the test fires. */
function streamingWorld() {
  const streams = [];
  const calls = [];
  const timers = [];
  const send = async (path, init) => {
    calls.push({ path, ...init });
    const stream = fakeStream();
    init.signal.addEventListener("abort", () => stream.abort(new Error("aborted")));
    streams.push(stream);
    return { ok: true, status: 200, headers: { get: () => null }, body: { getReader: () => stream.reader } };
  };
  const schedule = (work, ms) => {
    const timer = { work, ms, cancelled: false };
    timers.push(timer);
    return timer;
  };
  const cancel = (timer) => { if (timer) timer.cancelled = true; };
  return { send, streams, calls, timers, schedule, cancel };
}

const ticks = async () => { for (let i = 0; i < 20; i += 1) await new Promise((resolve) => setImmediate(resolve)); };

test("the stream delivers each change as a payload, whatever the chunks cut through", async () => {
  const world = streamingWorld();
  const seen = [];
  const transport = Transport.overHttp({ fetch: world.send, bearer: "k", schedule: world.schedule, cancel: world.cancel });
  const stop = transport.subscribe({ change: (message) => seen.push(message) });
  await ticks();
  assert.equal(world.calls[0].path, "/api/events");
  assert.equal(world.calls[0].method, "GET");
  assert.equal(world.calls[0].headers.Authorization, "Bearer k");
  const [stream] = world.streams;
  stream.write('id: i-6\ndata: {"seq":6,"cha');
  stream.write('nge":{"kind":"runtime_changed"}}\n\n: keep\n\nid: i-7\ndata: {"seq":7,"change":{"kind":"x"}}');
  stream.write("\n\n");
  await ticks();
  assert.deepEqual(seen, [
    { payload: { seq: 6, change: { kind: "runtime_changed" } } },
    { payload: { seq: 7, change: { kind: "x" } } }
  ]);
  stop();
});

test("the stream hands hello, closed and reset to their own handlers and ignores comments", async () => {
  const world = streamingWorld();
  const seen = [];
  const transport = Transport.overHttp({ fetch: world.send, schedule: world.schedule, cancel: world.cancel });
  const stop = transport.subscribe({
    hello: (data) => seen.push(["hello", data]),
    closed: (data) => seen.push(["closed", data]),
    reset: (data) => seen.push(["reset", data]),
    change: () => seen.push(["change"])
  });
  await ticks();
  world.streams[0].write('event: hello\ndata: {"seq":5}\n\n: keep\n\n');
  world.streams[0].write('event: reset\ndata: {"reason":"behind"}\r\n\r\nevent: closed\ndata: {"sentence":"Off."}\n\n');
  await ticks();
  assert.deepEqual(seen, [["hello", { seq: 5 }], ["reset", { reason: "behind" }], ["closed", { sentence: "Off." }]]);
  stop();
});

test("a stream that ends is lost at once, opened again after 1, 2, 5 and 10 seconds, and found again by its hello", async () => {
  const world = streamingWorld();
  const seen = [];
  const transport = Transport.overHttp({ fetch: world.send, schedule: world.schedule, cancel: world.cancel });
  const stop = transport.subscribe({
    hello: (data) => seen.push(["hello", data.seq]),
    reconnecting: (on) => seen.push(["reconnecting", on])
  });
  await ticks();
  const waits = [];
  const waitFor = () => world.timers.filter((timer) => !timer.cancelled && [1000, 2000, 5000, 10000].includes(timer.ms)).at(-1);
  for (let attempt = 0; attempt < 5; attempt += 1) {
    world.streams.at(-1).end();
    await ticks();
    assert.deepEqual(seen.at(-1), ["reconnecting", true]);
    const timer = waitFor();
    waits.push(timer.ms);
    timer.cancelled = true;
    timer.work();
    await ticks();
    assert.equal(world.streams.length, attempt + 2);
  }
  assert.deepEqual(waits, [1000, 2000, 5000, 10000, 10000]);
  world.streams.at(-1).write('event: hello\ndata: {"seq":9}\n\n');
  await ticks();
  assert.deepEqual(seen.slice(-2), [["reconnecting", false], ["hello", 9]]);
  // Lost once, however many times it was tried again before it was found.
  assert.equal(seen.filter(([kind, value]) => kind === "reconnecting" && value === true).length, 1);
  stop();
});

test("a good read starts the waits over", async () => {
  const world = streamingWorld();
  const transport = Transport.overHttp({ fetch: world.send, schedule: world.schedule, cancel: world.cancel });
  const stop = transport.subscribe({ reconnecting() {} });
  await ticks();
  world.streams[0].end();
  await ticks();
  let timer = world.timers.filter((entry) => !entry.cancelled && entry.ms === 1000).at(-1);
  timer.work();
  await ticks();
  world.streams[1].write("event: hello\ndata: {}\n\n");
  await ticks();
  world.streams[1].end();
  await ticks();
  timer = world.timers.filter((entry) => !entry.cancelled && [1000, 2000].includes(entry.ms)).at(-1);
  assert.equal(timer.ms, 1000);
  stop();
});

test("forty seconds without a byte is a dead stream: it is aborted and opened again", async () => {
  const world = streamingWorld();
  const seen = [];
  const transport = Transport.overHttp({ fetch: world.send, schedule: world.schedule, cancel: world.cancel });
  const stop = transport.subscribe({ reconnecting: (on) => seen.push(on) });
  await ticks();
  const dead = world.timers.filter((timer) => !timer.cancelled && timer.ms === 40000).at(-1);
  assert.ok(dead, "a dead-stream timer is armed");
  dead.work();
  await ticks();
  assert.equal(world.calls[0].signal.aborted, true);
  assert.deepEqual(seen, [true]);
  stop();
});

test("a stream refused because watching is off reports the service's sentence and stops", async () => {
  const sentence = "Watching is off on this computer. Turn it on in the Ghostlight window.";
  const seen = [];
  const send = async () => answer(403, { error: { code: "watching_off", message: sentence } });
  const transport = Transport.overHttp({ fetch: send, schedule: () => 0, cancel: () => {} });
  transport.subscribe({ closed: (data) => seen.push(data), reconnecting: () => seen.push("lost") });
  await ticks();
  assert.deepEqual(seen, [{ sentence }]);
});

test("stopping ends the stream and nothing opens again", async () => {
  const world = streamingWorld();
  const transport = Transport.overHttp({ fetch: world.send, schedule: world.schedule, cancel: world.cancel });
  const stop = transport.subscribe({});
  await ticks();
  stop();
  await ticks();
  assert.equal(world.calls[0].signal.aborted, true);
  assert.equal(world.calls.length, 1);
});

test("with nothing to fetch with, every call rejects", async () => {
  const transport = Transport.overHttp({});
  assert.equal(transport.available, false);
  await assert.rejects(transport.snapshot(), /no orchestrator/);
});

test("the transport has one road, over loopback", () => {
  assert.equal(Transport.create, undefined);
  assert.equal(typeof Transport.overHttp, "function");
  const transport = Transport.overHttp({});
  assert.equal(transport.copyHarnessText, undefined);
  assert.equal(Words.CHANGE_EVENT, undefined);
});

test("the page's own words for copying and for waiting are the ones the transport's callers use", () => {
  assert.equal(Words.COPIED_SETUP, "Manual setup copied.");
  assert.equal(Words.COPIED_COMMAND, "MCP command copied.");
  assert.equal(Words.RECONNECTING, "Reconnecting");
  assert.equal(Words.CONTINUE_LABEL, "Continue in the Ghostlight window");
});

test("every act the page names is a key of the context a window is given", () => {
  const control = {
    context: "control",
    opener: Transport.OPENER_SERVICE,
    acts: Object.fromEntries([
      "search", "check_policy", "rescan", "continue_in_window", "hold", "resume", "start_session",
      "end_session", "apply_policy", "remove_policy", "set_diagnostics", "set_watching", "setup_all",
      "harness_act", "test_notification", "open", "reveal_diagnostics", "locate", "setup_act", "quit"
    ].map((act) => [act, { available: true, why_not: null }]))
  };
  const named = Object.values(Transport.ACT);
  assert.equal(new Set(named).size, named.length);
  assert.deepEqual([...named].sort(), Object.keys(control.acts).sort());
  assert.equal(Transport.CONTEXT.closed, "closed");
  assert.deepEqual({ ...Transport.COPY_KIND }, { command: "command", setup: "setup" });
  assert.throws(() => { "use strict"; Transport.ACT.rescan = "other"; }, TypeError);
});
