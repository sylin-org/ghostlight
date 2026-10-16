// Live acceptance for a handle that follows its control through a re-render (ADR-0198 Decision 17).
//
// It serves tests/fixtures/agent-ux/rerender-list.html from a disposable local server and drives
// it through the installed `ghostlight mcp` edge, the service and the installed adapter. It starts
// nothing itself: install and connect a Ghostlight browser first, then run it. Journeys read
// .target-ghostlight-1.0/debug unless GHOSTLIGHT_BIN_DIR says otherwise.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const binDir = process.env.GHOSTLIGHT_BIN_DIR || join(root, ".target-ghostlight-1.0", "debug");
const edge = join(binDir, `ghostlight${process.platform === "win32" ? ".exe" : ""}`);
const page = readFileSync(join(root, "tests", "fixtures", "agent-ux", "rerender-list.html"));

const server = createServer((request, response) => {
  response.setHeader("Content-Type", "text/html; charset=utf-8");
  response.end(page);
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const url = `http://127.0.0.1:${server.address().port}/`;

const child = spawn(edge, ["mcp"], { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
let stderr = "";
child.stderr.on("data", (chunk) => { stderr = (stderr + chunk).slice(-2000); });
const pending = new Map();
let next = 0;
createInterface({ input: child.stdout }).on("line", (line) => {
  const message = JSON.parse(line);
  const waiter = pending.get(message.id);
  if (!waiter) return;
  pending.delete(message.id);
  clearTimeout(waiter.timer);
  waiter.resolve(message);
});
const request = (method, params) => new Promise((resolveCall, reject) => {
  const id = ++next;
  const timer = setTimeout(() => {
    pending.delete(id);
    reject(new Error(`MCP timeout: ${method} ${stderr}`));
  }, 30000);
  pending.set(id, { resolve: resolveCall, timer });
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
});

/** One tool call; the structured result, whatever its status. */
async function call(tool, args) {
  const response = await request("tools/call", { name: tool, arguments: args });
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  return response.result.structuredContent;
}

let tab;

/** Every handle named `name` on the page, in document order. */
async function handles(name) {
  const found = await call("browser_find", { tab, text: name, visual_settle: false });
  return found.facts.matches.filter((match) => match.name === name).map((match) => match.target);
}

/** Click the one page control named `name`. */
async function press(name) {
  const [target] = await handles(name);
  const clicked = await call("browser_click", { tab, target, visual_settle: false });
  assert.equal(clicked.status, "succeeded", JSON.stringify(clicked));
}

/** The song ids of the "More options" buttons clicked so far. */
async function clicks() {
  const read = await call("browser_execute", { tab, visual_settle: false, script: "return window.optionsClicks;" });
  return read.facts.value;
}

/** Click a "More options" handle and report the result. */
const options = (target) => call("browser_click", { tab, target, visual_settle: false });

const results = [];
try {
  await request("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "rerender-journey", version: "1" } });
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`);
  tab = (await call("browser_navigate", { url, new_tab: true })).facts.tab;

  // A re-render the page proves: the second Neon Rain take follows its own link.
  let menus = await handles("More options");
  assert.equal(menus.length, 4);
  await press("Re-render all rows");
  let clicked = await options(menus[1]);
  assert.equal(clicked.status, "succeeded", JSON.stringify(clicked));
  assert.equal(clicked.facts.rebound, true);
  assert.deepEqual((await clicks()).slice(-1), [12], "the click landed on the second take, not on its place in the list");
  results.push("re-render followed");

  // An in-app tab switch at the same address and title, then back.
  menus = await handles("More options");
  await press("Playlists");
  await press("Songs");
  clicked = await options(menus[2]);
  assert.equal(clicked.status, "succeeded", JSON.stringify(clicked));
  assert.equal(clicked.facts.rebound, true);
  assert.deepEqual((await clicks()).slice(-1), [21]);
  results.push("tab switch followed");

  // A find after a re-render returns the same handles.
  await press("Re-render all rows");
  assert.deepEqual(await handles("More options"), menus);
  results.push("find returned the same handles");

  // A deleted take is not replaced by the other take of the same title.
  await press("Delete the first take");
  clicked = await options(menus[0]);
  assert.equal(clicked.status, "failed", JSON.stringify(clicked));
  assert.equal(clicked.facts.stale, "removed");
  assert.equal(clicked.facts.rebound, undefined);
  const before = await clicks();
  assert.deepEqual(before.slice(-1), [21], "nothing was clicked for the deleted take");
  results.push("deleted take refused");

  // A recycled row node, and a same-address view switch that renames the region, are not followed.
  await press("Restore the library");
  menus = await handles("More options");
  await press("Recycle the first row");
  clicked = await options(menus[0]);
  assert.equal(clicked.status, "failed", JSON.stringify(clicked));
  assert.equal(clicked.facts.stale, "removed");
  await press("Restore the library");
  menus = await handles("More options");
  await press("Rename the region");
  clicked = await options(menus[0]);
  assert.equal(clicked.status, "failed", JSON.stringify(clicked));
  assert.equal(clicked.facts.stale, "removed");
  assert.deepEqual((await clicks()).length, before.length, "no refused click reached the page");
  results.push("recycled row and renamed region refused");

  console.log(JSON.stringify({ passed: true, results }));
} catch (error) {
  console.error(error.stack);
  process.exitCode = 1;
} finally {
  child.stdin.end();
  child.kill();
  server.closeAllConnections();
  await new Promise((done) => server.close(done));
  console.log(`Owned fixture tab is retained for inspection: ${tab ?? "not opened"}`);
}
