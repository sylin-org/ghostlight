// Exercise the production keyboard replacement function against real Chromium editing.
// This component test starts no Ghostlight service and touches no installed browser profile.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { basename, dirname, join, resolve } from "node:path";
import vm from "node:vm";
import { readDevToolsPort, waitForChromiumExit } from "./lib/chromium.mjs";

const root = resolve(import.meta.dirname, "..");
const scratchRoot = join(root, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "keyboard-browser-"));
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || join(root, ".tmp/chrome-testing/chrome-win64/chrome.exe");
const child = spawn(browser, ["--headless=new", "--remote-debugging-port=0",
  `--user-data-dir=${scratch}`, "--no-first-run", "--no-default-browser-check", "about:blank"],
{ windowsHide: true, stdio: "ignore" });
child.on("error", error => { child.startError = error; });
let socket;
let send;
try {
  const [port, endpoint] = await readDevToolsPort(scratch, child);
  socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
  await new Promise((resolveOpen, reject) => {
    socket.addEventListener("open", resolveOpen, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  let next = 0;
  const pending = new Map();
  socket.addEventListener("message", event => {
    const response = JSON.parse(event.data);
    const request = pending.get(response.id);
    if (!request) return;
    pending.delete(response.id);
    clearTimeout(request.timer);
    if (response.error) request.reject(new Error(JSON.stringify(response.error)));
    else request.resolve(response.result);
  });
  send = (method, params = {}, sessionId) => new Promise((resolveCall, reject) => {
    const id = ++next;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timeout: ${method}`)); }, 10000);
    pending.set(id, { resolve: resolveCall, reject, timer });
    socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
  });
  const { targetInfos } = await send("Target.getTargets");
  const target = targetInfos.find(info => info.type === "page");
  const { sessionId } = await send("Target.attachToTarget", { targetId: target.targetId, flatten: true });
  const evaluate = async expression => {
    const response = await send("Runtime.evaluate", { expression, returnByValue: true }, sessionId);
    assert.equal(response.exceptionDetails, undefined);
    return response.result.value;
  };
  await evaluate(`document.body.innerHTML = '<form><textarea id="draft"></textarea><input id="next"><button>Submit</button></form>';
    window.submits = 0; window.inputs = []; window.model = '';
    document.querySelector('form').addEventListener('submit', e => { e.preventDefault(); window.submits++; });
    draft.addEventListener('input', e => { window.model = draft.value; window.inputs.push({trusted:e.isTrusted,type:e.inputType}); });`);
  const require = createRequire(import.meta.url);
  const sandbox = { shared: require("../extension/lib/shared.js"),
    sendDebugger: (_target, method, params) => send(method, params, sessionId) };
  vm.createContext(sandbox);
  const worker = readFileSync(join(root, "extension/service-worker.js"), "utf8");
  const start = worker.indexOf("async function replaceFocusedText(");
  const end = worker.indexOf("async function fill(", start);
  assert.ok(start >= 0 && end > start);
  vm.runInContext(worker.slice(start, end), sandbox);
  const content = readFileSync(join(root, "extension/content.js"), "utf8");
  const expectedStart = content.indexOf("  function expectedFillValue(");
  const expectedEnd = content.indexOf("  async function verifyStableFillValue(", expectedStart);
  assert.ok(expectedStart >= 0 && expectedEnd > expectedStart);
  await evaluate(content.slice(expectedStart, expectedEnd));
  const cases = ["First job\nSecond job", "\nFirst\n\nLast\n", "First\r\nSecond\r\n", "First\rSecond\r", "Single line", ""];
  for (const value of cases) {
    await evaluate("draft.value = 'old draft'; draft.focus(); window.inputs = []; window.model = 'old draft';");
    await sandbox.replaceFocusedText(1, 0, value);
    const observed = await evaluate("({value:draft.value, model:window.model, inputs:window.inputs, submits:window.submits, focused:document.activeElement.id})");
    const expected = value.replace(/\r\n?/g, "\n");
    assert.equal(observed.value, expected, `DOM value for ${JSON.stringify(value)}`);
    assert.equal(observed.model, expected, "input listener model matches the requested text");
    assert.equal(await evaluate(`expectedFillValue(draft, ${JSON.stringify(value)})`), observed.value,
      "production readback accepts Chromium's normalized textarea value");
    assert.ok(observed.inputs.length > 0 && observed.inputs.every(event => event.trusted));
    assert.equal(observed.submits, 0);
    assert.equal(observed.focused, "next");
  }
  console.log(`PASS: ${cases.length} Chromium text replacements preserve DOM/model values, trusted input, blur, and no submission.`);
} finally {
  if (send && socket?.readyState === WebSocket.OPEN) await send("Browser.close").catch(() => {});
  socket?.close();
  if (!await waitForChromiumExit(child)) { child.kill(); await waitForChromiumExit(child); }
  assert.equal(dirname(resolve(scratch)), scratchRoot);
  assert.ok(basename(scratch).startsWith("keyboard-browser-"));
  rmSync(scratch, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}
