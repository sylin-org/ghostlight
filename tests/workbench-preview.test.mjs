// The design-review preview's own script, run as the page runs it.
//
// The preview server composes a fixture script that plays a synthetic service inside the page.
// Code in that script runs in the page, not in the server, so a name it borrows from the server
// fails only once the page reaches it. This test serves the working page's fixture, runs it in a
// sandbox with the page's globals, and lets scripted work start and settle. Nothing in it starts
// a browser or a Ghostlight process.
//
// Run with: node --test tests/workbench-preview.test.mjs
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { resolve } from "node:path";
import { test } from "node:test";
import vm from "node:vm";

const repository = resolve(import.meta.dirname, "..");
const SETTLE_DEADLINE_MS = 15000;

/** Start the preview server on a free port and resolve to the address it prints. */
async function startPreview() {
  const server = spawn(process.execPath, ["tests/workbench-preview-server.mjs"], {
    cwd: repository, env: { ...process.env, GHOSTLIGHT_PREVIEW_PORT: "0" },
    windowsHide: true, stdio: ["ignore", "pipe", "pipe"]
  });
  let printed = "";
  server.stderr.on("data", () => {});
  for await (const data of server.stdout) {
    printed += data;
    if (printed.includes("\n")) break;
  }
  return { server, address: printed.trim() };
}

/**
 * Run `source` with the globals the fixture script reads in a page. Its timers are the host's,
 * held so the test can stop them, and an exception thrown in one is kept in `errors`.
 */
function runFixture(source, address) {
  const timers = new Set();
  const errors = [];
  const sandbox = {
    location: { href: `${address}/` },
    fetch: async () => { throw new Error("the fixture asked the network"); },
    setTimeout: (callback, ms) => {
      const timer = setTimeout(() => {
        timers.delete(timer);
        try { callback(); } catch (error) { errors.push(error); }
      }, ms);
      timers.add(timer);
      return timer;
    },
    clearTimeout: (timer) => { timers.delete(timer); clearTimeout(timer); },
    TextEncoder, ReadableStream, Response, URL
  };
  sandbox.window = sandbox;
  vm.runInNewContext(source, sandbox);
  const stop = () => { for (const timer of timers) clearTimeout(timer); timers.clear(); };
  return { window: sandbox, errors, stop };
}

/** Read the page's event stream until a settled record arrives or a fixture timer throws. */
async function firstSettled(window, errors) {
  const response = await window.fetch("/api/events");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  const deadline = Date.now() + SETTLE_DEADLINE_MS;
  let buffered = "";
  // One read stays pending across the polls, so no chunk is read into a promise nobody awaits.
  let pending = reader.read();
  try {
    while (Date.now() < deadline && errors.length === 0) {
      const next = await Promise.race([pending, new Promise((done) => setTimeout(() => done(null), 100))]);
      if (next === null) continue;
      if (next.done) break;
      pending = reader.read();
      buffered += decoder.decode(next.value, { stream: true });
      for (const [, data] of buffered.matchAll(/^data: (.+)$/gm)) {
        const { change } = JSON.parse(data);
        if (change?.kind === "operation_settled") return change.record;
      }
    }
    return null;
  } finally {
    await reader.cancel();
  }
}

test("scripted work on the working page settles with its session's evidence", async () => {
  const { server, address } = await startPreview();
  try {
    const answer = await fetch(`${address}/fixture.js?readiness=working`);
    assert.equal(answer.status, 200);
    const page = runFixture(await answer.text(), address);
    try {
      const record = await firstSettled(page.window, page.errors);
      assert.deepEqual(page.errors.map(String), [], "a fixture timer threw");
      assert.ok(record, "no scripted work settled");
      const session = page.window.__GHOSTLIGHT_PREVIEW__.sessions.find((each) => each.id === record.workspace);
      assert.ok(session, `the record names no preview session: ${record.workspace}`);
      // The sandbox's objects have its own prototypes, so they are compared as the page reads them.
      assert.equal(JSON.stringify(record.agent), JSON.stringify(session.agent), "the record names its session's agent");
      assert.equal(record.provenance.channel, session.channel);
      assert.equal(record.provenance.connection_id, `connection_${session.id}`);
    } finally {
      page.stop();
    }
  } finally {
    server.kill();
    await once(server, "exit");
  }
});
