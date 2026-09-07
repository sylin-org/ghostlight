// H4: real bundled UI in isolated Chromium, with synthetic workbench projection events.
// This proves rendering and interaction, not an installed Tauri/native-host deployment.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

const repository = resolve(import.meta.dirname, "..");
const scratchRoot = join(repository, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "history-browser-"));
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || [
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
].find(existsSync);
assert.ok(browser, "Set GHOSTLIGHT_TEST_BROWSER to a Chromium executable.");
const children = [];
const delay = (ms) => new Promise((done) => setTimeout(done, ms));
async function until(check, label) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    const value = await check();
    if (value) return value;
    await delay(25);
  }
  throw new Error(`Timed out: ${label}`);
}
function start(executable, args, env = process.env) {
  const child = spawn(executable, args, { cwd: repository, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  child.stderr.on("data", () => {});
  child.on("error", (error) => { child.startError = error; });
  children.push(child);
  return child;
}
let socket;
let send;
try {
  const server = start(process.execPath, ["tests/workbench-preview-server.mjs"], {
    ...process.env, GHOSTLIGHT_PREVIEW_SCENARIO: "h5", GHOSTLIGHT_PREVIEW_PORT: "0"
  });
  let address = "";
  server.stdout.on("data", (data) => { address += data; });
  await until(() => address.includes("http://"), "preview server");
  const profile = join(scratch, "profile");
  start(browser, ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
    "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
    "--disable-component-update", "--disable-sync", "about:blank"]);
  const portFile = join(profile, "DevToolsActivePort");
  await until(() => existsSync(portFile), "Chromium startup");
  const [port, endpoint] = readFileSync(portFile, "utf8").trim().split(/\r?\n/);
  socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
  await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
  let nextId = 0;
  const pending = new Map();
  send = (method, params = {}, sessionId) => new Promise((done, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 10000);
    pending.set(id, { done, reject, timer });
    socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
  });
  socket.onmessage = ({ data }) => {
    const message = JSON.parse(data);
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    clearTimeout(entry.timer);
    if (message.error) entry.reject(new Error(JSON.stringify(message.error)));
    else entry.done(message.result);
  };
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  const page = (method, params) => send(method, params, sessionId);
  const evaluate = async (expression) => {
    const result = await page("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const resize = (width, height) => page("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
  const capture = async (name) => {
    const { data } = await page("Page.captureScreenshot", { format: "png" });
    writeFileSync(join(scratchRoot, `h4-${name}.png`), Buffer.from(data, "base64"));
  };
  await resize(1280, 900);
  await page("Page.navigate", { url: address.trim() });
  await until(() => evaluate("!!document.querySelector('.composition-details')"), "grouped history");
  assert.equal(await evaluate("document.querySelector('.composition-details').open"), false);
  await capture("collapsed");
  await evaluate("document.querySelector('.composition-details > summary').click()");
  await until(() => evaluate("document.querySelector('.composition-details').open"), "expanded history");
  assert.equal(await evaluate("document.querySelectorAll('.history-step').length"), 4);
  await evaluate("document.querySelector('.history-step .permission-details > summary').click()");
  await capture("expanded");
  // Exercise an overflowing group; follow-up receipts must preserve both focus and scroll.
  await evaluate("window.__GHOSTLIGHT_HISTORY_FIXTURE__ = structuredClone(window.__GHOSTLIGHT_PREVIEW__.history[0])");
  await evaluate(`(() => {
    const record = structuredClone(window.__GHOSTLIGHT_HISTORY_FIXTURE__);
    record.steps = Array.from({ length: 20 }, (_, index) => ({ ...record.steps[index === 16 ? 2 : 0], position: index + 1 }));
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'composition_changed', record });
  })()`);
  await delay(80);
  await evaluate("document.querySelector('.composition-details > summary').click()");
  await delay(30);
  await evaluate("document.querySelector('.composition-details > summary').click()");
  await delay(30);
  assert.equal(await evaluate(`(() => {
    const list = document.querySelector('.history-steps').getBoundingClientRect();
    const problem = document.querySelector('[data-step-problem="true"]').getBoundingClientRect();
    return problem.top >= list.top && problem.bottom <= list.bottom;
  })()`), true);
  const scroll = await evaluate(`(() => {
    const list = document.querySelector('.history-steps'); list.scrollTop = 220;
    document.querySelector('.composition-details > summary').focus({ preventScroll: true });
    return list.scrollTop;
  })()`);
  assert.ok(scroll > 0);
  await evaluate(`(() => {
    const record = structuredClone(window.__GHOSTLIGHT_HISTORY_FIXTURE__);
    record.steps = Array.from({ length: 20 }, (_, index) => ({ ...record.steps[index === 16 ? 2 : 0], position: index + 1 }));
    record.summary = 'Completed 3 of 20 steps.';
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'composition_changed', record });
  })()`);
  await delay(80);
  assert.equal(await evaluate("document.querySelector('.history-steps').scrollTop"), scroll);
  assert.equal(await evaluate("document.activeElement.matches('.composition-details > summary')"), true);
  assert.equal(await evaluate("document.querySelector('.history-step .permission-details').open"), true);
  await resize(720, 900);
  await delay(80);
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  await capture("narrow");
  // Session review restores an explicitly cleared group, expands its problem, and recovers
  // only the incident handed to the UI. No browser job or global Resume is dispatched.
  await evaluate("document.querySelector('#clear-monitor').click()");
  await evaluate("document.querySelector('[data-review-session]').click()");
  await until(() => evaluate("!!document.querySelector('.composition-details')?.open"), "review restored group");
  assert.equal(await evaluate("document.querySelectorAll('.history-step').length"), 20);
  assert.equal(await evaluate(`(() => {
    const list = document.querySelector('.history-steps').getBoundingClientRect();
    const problem = document.querySelector('[data-step-problem="true"]').getBoundingClientRect();
    return problem.top >= list.top && problem.bottom <= list.bottom;
  })()`), true);
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  await capture("session-review");
  await evaluate("document.querySelector('[data-resume-session]').click()");
  await until(() => evaluate("!document.querySelector('[data-resume-session]')"), "scoped recovery");
  assert.deepEqual(await evaluate("window.__GHOSTLIGHT_RESUMED__"), { workspace: "workspace_codex", incident: "attention_test" });
  assert.equal(await evaluate("window.__GHOSTLIGHT_PREVIEW__.service.runtime_state"), "active");

  // H7 uses the actual bundled UI with a synthetic storage-failure projection.
  await evaluate(`(() => {
    const snapshot = window.__GHOSTLIGHT_PREVIEW__;
    snapshot.audit_notice = 'History cannot be saved. Browser work can continue. Ghostlight checks automatically.';
    snapshot.audit_health = { failure: 'write', unconfirmed_receipts: 1, unreadable_entries: 0, history_unavailable: false };
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'audit_health_changed', health: snapshot.audit_health });
  })()`);
  await until(() => evaluate("!document.querySelector('#audit-health').hidden"), "audit health notice");
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  await evaluate(`(() => {
    const record = structuredClone(window.__GHOSTLIGHT_HISTORY_FIXTURE__);
    record.storage = 'saved';
    record.storage_detail = 'Some step history could not be saved.';
    record.steps[0].record.storage = 'unconfirmed';
    record.steps[0].record.storage_detail = 'History could not be saved. Storage was not confirmed.';
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'composition_changed', record });
  })()`);
  await until(() => evaluate("document.querySelector('.history-step')?.textContent.includes('Storage was not confirmed')"), "child storage notice");
  assert.equal(await evaluate("document.querySelector('.composition-details').open"), true);
  await capture("audit-unavailable");
  await evaluate(`(() => {
    const snapshot = window.__GHOSTLIGHT_PREVIEW__;
    snapshot.audit_notice = 'History is saving. 1 earlier receipt was not confirmed saved.';
    snapshot.audit_health.failure = null;
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'audit_health_changed', health: snapshot.audit_health });
  })()`);
  await until(() => evaluate("document.querySelector('#audit-health').textContent.includes('1 earlier receipt')"), "recovery gap notice");
  assert.equal(await evaluate("document.querySelector('.history-step').textContent.includes('Storage was not confirmed')"), true);
  await capture("audit-recovered");
  console.log("H7 browser history: persistent health, independent child storage, recovery gap, preserved expansion, and narrow layout passed.");
  console.log("H4/H5 browser history: expansion, incremental scroll/focus, cleared-history review, scoped resume, and narrow layout passed.");
} finally {
  if (send && socket?.readyState === WebSocket.OPEN) {
    try { await send("Browser.close"); } catch { /* shutdown can close the reply channel */ }
  }
  socket?.close();
  for (const child of children.toReversed()) {
    if (child.exitCode === null && child.signalCode === null) child.kill();
  }
  await until(() => children.every((child) => child.exitCode !== null || child.signalCode !== null || child.startError), "owned child exit");
  assert.equal(dirname(resolve(scratch)), resolve(scratchRoot));
  assert.ok(basename(scratch).startsWith("history-browser-"));
  rmSync(scratch, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}
