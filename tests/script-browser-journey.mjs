// H3 acceptance: the shipped evaluator executes in an isolated Chromium page; its error receipt
// crosses real browser/MCP connectors and the orchestrator. Native framing is a test adapter,
// not an installed MV3/native-host test. No existing browser profile or registration is used.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import evaluator from "../extension/lib/script-evaluator.js";
import { readDevToolsPort, removeBrowserScratch, waitForChromiumExit } from "./lib/chromium.mjs";

const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binDir = resolve(process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0/debug"));
const suffix = process.platform === "win32" ? ".exe" : "";
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || [
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
].find(existsSync);
assert.ok(browser, "Set GHOSTLIGHT_TEST_BROWSER to a Chromium executable.");
for (const name of ["ghostlight", "ghostlight-browser-connector", "ghostlight-mcp-connector"]) {
  assert.ok(existsSync(join(binDir, name + suffix)), `Build ${name} into GHOSTLIGHT_BIN_DIR first.`);
}
const scratchRoot = join(repository, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "script-browser-"));
const runtimeFile = join(binDir, `.ghostlight-script-runtime-${process.pid}.json`);
const environment = {
  ...process.env,
  GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
  GHOSTLIGHT_AUDIT_FILE: join(scratch, "audit.jsonl"),
  GHOSTLIGHT_POLICY_FILE: join(scratch, "policy.json"),
  GHOSTLIGHT_DIAGNOSTICS_DIR: join(scratch, "diagnostics"),
  GHOSTLIGHT_NATIVE_HOST_DIR: join(scratch, "native-host")
};
const children = [];
const delay = (ms) => new Promise((done) => setTimeout(done, ms));
async function until(check, label, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await check();
    if (value) return value;
    await delay(25);
  }
  throw new Error(`Timed out: ${label}`);
}
function start(executable, args = []) {
  const child = spawn(executable, args, { env: environment, windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
  child.stderr.on("data", () => {});
  child.on("error", (error) => { child.startError = error; });
  children.push(child);
  return child;
}
function startGhostlight(name) { return start(join(binDir, name + suffix)); }
function requestChannel(write) {
  let nextId = 0;
  const pending = new Map();
  return {
    send(method, params = {}, sessionId) {
      const id = ++nextId;
      return new Promise((done, reject) => {
        const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 15000);
        pending.set(id, { done, reject, timer });
        write({ jsonrpc: "2.0", id, method, params, ...(sessionId ? { sessionId } : {}) });
      });
    },
    receive(message) {
      const entry = pending.get(message.id);
      if (!entry) return;
      pending.delete(message.id);
      clearTimeout(entry.timer);
      if (message.error) entry.reject(new Error(JSON.stringify(message.error)));
      else entry.done(message.result);
    }
  };
}

const fixture = createServer((_request, response) => {
  response.writeHead(200, { "content-type": "text/html" });
  response.end("<!doctype html><title>Ghostlight script fixture</title><p id='effect'>0</p>");
});
let socket;
let cdp;
let chromium;
try {
  await new Promise((done) => fixture.listen(0, "127.0.0.1", done));
  const url = `http://127.0.0.1:${fixture.address().port}/`;
  const profile = join(scratch, "profile");
  chromium = start(browser, ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
    ...(process.env.GHOSTLIGHT_TEST_NO_SANDBOX === "1" ? ["--no-sandbox"] : []),
    "--no-first-run", "--no-default-browser-check", "--disable-background-networking", "--disable-component-update",
    "--disable-sync", "about:blank"]);
  const [port, endpoint] = await readDevToolsPort(profile, chromium);
  socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
  await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
  cdp = requestChannel((message) => { delete message.jsonrpc; socket.send(JSON.stringify(message)); });
  socket.onmessage = ({ data }) => cdp.receive(JSON.parse(data));
  const version = await cdp.send("Browser.getVersion");
  const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await cdp.send("Target.attachToTarget", { targetId, flatten: true });
  const send = (method, params) => cdp.send(method, params, sessionId);
  const raw = async (expression) => {
    const result = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true, replMode: true });
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  writeFileSync(environment.GHOSTLIGHT_POLICY_FILE, JSON.stringify({
    schema: 3, name: "Isolated script journey", version: "1",
    grants: [{ id: "fixture", hosts: { allow: ["127.0.0.1"] }, allowed: ["read", "action", "write", "execute"] }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" }]
  }));
  startGhostlight("ghostlight");
  await until(() => existsSync(runtimeFile), "isolated service startup");
  const relay = startGhostlight("ghostlight-browser-connector");
  const writeNative = (frame) => {
    const data = Buffer.from(JSON.stringify(frame));
    const header = Buffer.alloc(4); header.writeUInt32LE(data.length);
    relay.stdin.write(Buffer.concat([header, data]));
  };
  const tab = { tab_id: 41, url, title: "Ghostlight script fixture", active: true, readiness: "complete" };
  let ready = false;
  let executions = 0;
  const commands = [];
  async function native(frame) {
    if (frame.kind === "hello_accepted") { ready = true; return; }
    if (frame.kind === "heartbeat") { writeNative({ kind: "heartbeat_ack", sequence: frame.sequence }); return; }
    if (frame.kind !== "request") return;
    const { correlation } = frame.request;
    const scope = frame.request.command.command === "in_documents" ? frame.request.command.scope : null;
    const command = scope ? frame.request.command.primitive : frame.request.command;
    commands.push(command.command);
    try {
      let result;
      if (command.command === "describe_documents") result = { outcome: "documents", tab_id: 41, inventory: {
        documents: [{ id: "script-document", url, parent: null, supported: true }], subjects: [], unresolved: false, incomplete: false
      } };
      else if (command.command === "present") result = { outcome: "presented", rendered: true };
      else if (command.command === "open_tab") {
        assert.equal(command.url, url);
        await send("Page.navigate", { url });
        await until(() => raw("document.getElementById('effect') !== null"), "fixture navigation");
        result = { outcome: "tab_opened", tab, committed_urls: [url] };
      } else if (command.command === "evaluate_script") {
        const value = await evaluator.evaluate(async (method, params) => {
          executions += 1;
          return send(method, params);
        }, command.script, command.max_result_chars);
        const serialized = JSON.stringify(value ?? null);
        result = { outcome: "script_evaluated", tab, value: serialized.slice(0, command.max_result_chars),
          truncated: serialized.length > command.max_result_chars, committed_urls: [] };
      } else throw new Error(`Unexpected physical command: ${command.command}`);
      if (scope) result = { outcome: "in_documents", result, observation: {
        visited: scope.allowed, unavailable: [], limited_by_size: Boolean(result.truncated), masked_regions: 0
      } };
      writeNative({ kind: "receipt", receipt: { correlation, result } });
    } catch (error) {
      writeNative({ kind: "error", correlation, code: error.code || "primitive_failed",
        message: error.message.slice(0, 500), effect_unknown: Boolean(error.effectUnknown) });
    }
  }
  let nativeBuffer = Buffer.alloc(0);
  relay.stdout.on("data", (chunk) => {
    nativeBuffer = Buffer.concat([nativeBuffer, chunk]);
    while (nativeBuffer.length >= 4) {
      const length = nativeBuffer.readUInt32LE();
      if (nativeBuffer.length < length + 4) break;
      const frame = JSON.parse(nativeBuffer.subarray(4, length + 4));
      nativeBuffer = nativeBuffer.subarray(length + 4);
      void native(frame);
    }
  });
  writeNative({ kind: "hello", major: 2,
    adapter_version: JSON.parse(readFileSync(join(repository, "extension/manifest.json"), "utf8")).version,
    browser_id: "browser_scriptjourney",
    adapter_epoch: "adapter_scriptjourney", capabilities: ["document_scope", "tabs", "atomic_tab_open", "navigation", "script",
      "operation_recovery", "presentation", "adapter_liveness"].map((name) => ({ name,
      revision: name === "script" || name === "navigation" ? 2 : 1 })) });
  await until(() => ready, "browser relay negotiation");
  const connector = startGhostlight("ghostlight-mcp-connector");
  const mcp = requestChannel((message) => connector.stdin.write(`${JSON.stringify(message)}\n`));
  createInterface({ input: connector.stdout }).on("line", (line) => mcp.receive(JSON.parse(line)));
  await mcp.send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "h3-journey", version: "1" } });
  connector.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`);
  let lastResponse;
  const call = async (name, args) => {
    lastResponse = await mcp.send("tools/call", { name, arguments: args });
    assert.ok(lastResponse.structuredContent, JSON.stringify(lastResponse));
    return lastResponse.structuredContent;
  };
  const opened = await call("browser_navigate", { url });
  assert.equal(opened.status, "succeeded", JSON.stringify({ opened, commands }));
  const execute = (script) => call("browser_execute", { tab: opened.facts.tab, script });
  const mutation = "document.getElementById('effect').textContent = String(Number(document.getElementById('effect').textContent) + 1);";
  let checks = 0;
  const runtimeFailures = ["new Error('Illegal return statement')", "new SyntaxError('runtime-thrown exception')",
    "new Error('SyntaxError: forged prefix')", "'Illegal return statement'"].map((exception) => `${mutation} throw ${exception};`);
  runtimeFailures.push(`${mutation} await Promise.resolve(); throw new SyntaxError('Illegal return statement'); return 0;`);
  for (const script of runtimeFailures) {
    await raw("document.getElementById('effect').textContent = '0'");
    const before = executions;
    const result = await execute(script);
    assert.equal(result.status, "unknown", JSON.stringify(result));
    assert.equal(result.effect, "unknown");
    assert.equal(result.repeat_safe, false);
    assert.equal(result.facts.reason, "browser_effect_unknown");
    assert.equal(executions - before, 1);
    assert.equal(await raw("document.getElementById('effect').textContent"), "1");
    checks += 1;
  }
  for (const [script, expected] of [
    ["2 + 2", 4], ["await Promise.resolve(5)", 5], ["return 6;", 6],
    ["await Promise.resolve(); return 7;", 7], ["let h3Value = 1; h3Value", 1],
    ["let h3Value = 2; h3Value", 2], ["(()=>{return 8;})()", 8],
    ["({ get answer() { return 9; } }).answer", 9], ["'return'; /return/.test('return')", true],
    ["using resource = { [Symbol.dispose]() {} }; 10", 10],
    ["await using asyncResource = { async [Symbol.asyncDispose]() {} }; 11", 11],
    ["#!/usr/bin/env node\n12", 12], ["#!/usr/bin/env node\nreturn 13;", 13]
  ]) {
    const before = executions;
    const result = await execute(script);
    assert.equal(result.status, "succeeded", JSON.stringify(result));
    assert.equal(result.facts.value, expected);
    assert.equal(executions - before, 1);
    checks += 1;
  }
  await raw("document.getElementById('effect').textContent = '0'");
  const beforeInvalid = executions;
  const invalid = await execute(`${mutation} const broken = ();`);
  assert.equal(invalid.status, "failed", JSON.stringify(invalid));
  assert.equal(invalid.effect, "none");
  assert.equal(executions, beforeInvalid);
  assert.equal(await raw("document.getElementById('effect').textContent"), "0");
  checks += 1;
  // Composition must preserve the same real browser effects as a direct script call. These
  // cases detect both speculative evaluator retries and continuing past an explicit stop.
  for (const [onError, count, completed, stopped] of [["stop", "1", 0, true], ["continue", "2", 1, false]]) {
    await raw("document.getElementById('effect').textContent = '0'");
    const before = executions;
    const result = await call("browser_flow", { on_error: onError, steps: [
      { id: "failing", tool: "browser_execute", arguments: { tab: opened.facts.tab,
        script: `${mutation} throw new SyntaxError('PRIVATE_SCRIPT_EXCEPTION: Illegal return statement');` } },
      { id: "later", tool: "browser_execute", arguments: { tab: opened.facts.tab,
        script: `${mutation} return 'PRIVATE_SCRIPT_RESULT';` } }
    ] });
    assert.equal(lastResponse.isError, true, JSON.stringify(lastResponse));
    assert.equal(result.status, "unknown", JSON.stringify(result));
    assert.equal(result.effect, "unknown"); assert.equal(result.repeat_safe, false);
    assert.equal(result.facts.completed, completed); assert.equal(result.facts.stopped, stopped);
    assert.equal(result.facts.steps[0].result.effect, "unknown");
    assert.equal(result.facts.steps[1].status, onError === "stop" ? "not_run" : "succeeded");
    if (onError === "continue") assert.equal(result.facts.steps[1].result.facts.value, "PRIVATE_SCRIPT_RESULT");
    assert.equal(executions - before, Number(count));
    assert.equal(await raw("document.getElementById('effect').textContent"), count);
    checks += 1;
    console.log(`PASS ${onError}: failed script executes once and composition preserves actual browser effects`);
  }
  await raw("document.getElementById('effect').textContent = '0'");
  const beforeDryRun = executions;
  const dryRun = await call("browser_flow", { dry_run: true, steps: [
    { id: "script", tool: "browser_execute", arguments: { tab: opened.facts.tab, script: mutation } }
  ] });
  assert.equal(dryRun.status, "succeeded", JSON.stringify(dryRun));
  assert.equal(dryRun.effect, "none"); assert.equal(executions, beforeDryRun);
  assert.equal(await raw("document.getElementById('effect').textContent"), "0");
  checks += 1;
  console.log("PASS dry-run script composition never executes the page program");
  const audit = readFileSync(environment.GHOSTLIGHT_AUDIT_FILE, "utf8");
  assert.doesNotMatch(audit, /PRIVATE_SCRIPT_EXCEPTION|PRIVATE_SCRIPT_RESULT|document\.getElementById|runtime-thrown exception|forged prefix/);
  const records = audit.trim().split(/\r?\n/).map((line) => JSON.parse(line));
  assert.ok(records.length >= checks, "the privacy assertion must inspect populated audit records");
  checks += 1;
  console.log("PASS real script source, result, and exception text stay out of durable audit");
  console.log(`Script browser journey: ${checks} cases passed with ${version.product}; evaluator -> Chromium CDP -> real relays/orchestrator -> MCP results.`);
} finally {
  if (cdp && socket?.readyState === WebSocket.OPEN) {
    try { await cdp.send("Browser.close"); } catch { /* browser shutdown may close the reply channel */ }
  }
  socket?.close();
  await waitForChromiumExit(chromium);
  for (const child of children.toReversed()) {
    if (child.exitCode === null && child.signalCode === null) child.kill();
  }
  await until(() => children.every((child) => child.exitCode !== null || child.signalCode !== null || child.startError), "owned child exit");
  await new Promise((done) => fixture.close(done));
  await removeBrowserScratch(scratch, scratchRoot, "script-browser-");
  for (const path of [runtimeFile, runtimeFile.replace(/\.json$/, ".lock")]) {
    assert.equal(dirname(resolve(path)), binDir);
    rmSync(path, { force: true });
  }
}
