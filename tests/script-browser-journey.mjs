// H3 acceptance: browser_execute through the shipped extension in real Chromium. The script reaches
// the page through the extension's own debugger commands, and its error receipt crosses the real
// browser edge, the service in a test home and the MCP edge. Only native-port discovery is replaced
// (tests/lib/browser.mjs). No existing browser profile, native-host registration or installed
// extension is used.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { BrowserRig } from "./lib/browser.mjs";
import { createHome } from "./lib/home.mjs";
import { structured } from "./lib/peers.mjs";

const FIXTURE_HOST = "127.0.0.1";
// The extension runs a script with exactly one Runtime.evaluate that asks for a user gesture and a
// REPL scope. The worker counts those commands, so the journey can say how often a script ran.
const EVALUATIONS = "__journeyEvaluations";
const COUNT_EVALUATIONS = `(() => {
  globalThis.${EVALUATIONS} = 0;
  globalThis.__journeyBeforeCommand = async (_target, method, params) => {
    if (method === "Runtime.evaluate" && params?.replMode && params?.userGesture) globalThis.${EVALUATIONS} += 1;
  };
  return true;
})()`;

const fixture = createServer((request, response) => {
  if (request.url === "/data") {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ answer: 42 }));
    return;
  }
  response.writeHead(200, { "content-type": "text/html" });
  response.end("<!doctype html><title>Ghostlight script fixture</title><p id='effect'>0</p>");
});
const home = createHome({
  label: "script",
  policy: {
    schema: 4, name: "Isolated script journey", version: "1",
    grants: [{ id: "fixture", hosts: { allow: [FIXTURE_HOST] }, allowed: ["read", "action", "write", "execute"] }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" }]
  }
});
let rig;
try {
  await new Promise((done) => fixture.listen(0, FIXTURE_HOST, done));
  const url = `http://${FIXTURE_HOST}:${fixture.address().port}/`;
  rig = await BrowserRig.start({ home, label: "script-browser" });
  await rig.worker(COUNT_EVALUATIONS);
  const evaluations = () => rig.worker(`globalThis.${EVALUATIONS}`);
  const mcp = await rig.mcp("h3-journey");
  let lastResponse;
  const call = async (name, args) => {
    lastResponse = await mcp.call(name, args);
    return structured(lastResponse);
  };
  const opened = await call("browser_navigate", { url, new_tab: true });
  assert.equal(opened.status, "succeeded", JSON.stringify({ opened, sent: rig.bridge.sentSince(0) }));
  const page = await rig.page(await rig.physicalTab(url));
  const raw = (expression) => page.evaluate(expression);
  const execute = (script) => call("browser_execute", { tab: opened.facts.tab, script });
  const mutation = "document.getElementById('effect').textContent = String(Number(document.getElementById('effect').textContent) + 1);";
  let checks = 0;
  const runtimeFailures = ["new Error('Illegal return statement')", "new SyntaxError('runtime-thrown exception')",
    "new Error('SyntaxError: forged prefix')", "'Illegal return statement'"].map((exception) => `${mutation} throw ${exception};`);
  runtimeFailures.push(`${mutation} await Promise.resolve(); throw new SyntaxError('Illegal return statement'); return 0;`);
  for (const script of runtimeFailures) {
    await raw("document.getElementById('effect').textContent = '0'");
    const before = await evaluations();
    const result = await execute(script);
    assert.equal(result.status, "unknown", JSON.stringify(result));
    assert.equal(result.effect, "unknown");
    assert.equal(result.repeat_safe, false);
    // L-D6: the agent is told the script threw, and what it threw.
    assert.equal(result.facts.reason, "script_threw", JSON.stringify(result));
    assert.ok(typeof result.facts.exception === "string" && result.facts.exception.length > 0, JSON.stringify(result));
    assert.equal(await evaluations() - before, 1);
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
    const before = await evaluations();
    const result = await execute(script);
    assert.equal(result.status, "succeeded", JSON.stringify(result));
    assert.equal(result.facts.value, expected);
    assert.equal(await evaluations() - before, 1);
    checks += 1;
  }
  // A script that is one expression is awaited, so a promise it evaluates to resolves. A script
  // that is a statement is not, and a value of {} says what else reads as {}.
  for (const [script, expected] of [
    ["new Promise((done) => setTimeout(() => done(42), 20))", 42],
    ["fetch('/data').then((response) => response.json())", { answer: 42 }]
  ]) {
    const before = await evaluations();
    const result = await execute(script);
    assert.equal(result.status, "succeeded", JSON.stringify(result));
    assert.deepEqual(result.facts.value, expected);
    assert.deepEqual(result.next_steps ?? [], []);
    assert.equal(await evaluations() - before, 1);
    checks += 1;
  }
  for (const [script, words] of [
    ["let pending = Promise.resolve(1); pending", "unawaited promise, a DOM node or a Map"],
    ["document.getElementById('effect')", "DOM node, a Map or a class instance"]
  ]) {
    const result = await execute(script);
    assert.equal(result.status, "succeeded", JSON.stringify(result));
    assert.deepEqual(result.facts.value, {});
    assert.equal(result.next_steps?.length, 1, JSON.stringify(result));
    assert.ok(result.next_steps[0].includes(words), result.next_steps[0]);
    checks += 1;
  }
  await raw("document.getElementById('effect').textContent = '0'");
  const beforeInvalid = await evaluations();
  const invalid = await execute(`${mutation} const broken = ();`);
  assert.equal(invalid.status, "failed", JSON.stringify(invalid));
  assert.equal(invalid.effect, "none");
  assert.equal(await evaluations(), beforeInvalid);
  assert.equal(await raw("document.getElementById('effect').textContent"), "0");
  checks += 1;
  // Composition must preserve the same real browser effects as a direct script call. These
  // cases detect both speculative evaluator retries and continuing past an explicit stop.
  for (const [onError, count, completed, stopped] of [["stop", "1", 0, true], ["continue", "2", 1, false]]) {
    await raw("document.getElementById('effect').textContent = '0'");
    const before = await evaluations();
    const result = await call("browser_flow", { on_error: onError, steps: [
      { id: "failing", tool: "browser_execute", arguments: { tab: opened.facts.tab,
        script: `${mutation} throw new SyntaxError('PRIVATE_SCRIPT_EXCEPTION: Illegal return statement');` } },
      { id: "later", tool: "browser_execute", arguments: { tab: opened.facts.tab,
        script: `${mutation} return 'PRIVATE_SCRIPT_RESULT';` } }
    ] });
    assert.equal(lastResponse.result.isError, true, JSON.stringify(lastResponse));
    assert.equal(result.status, "unknown", JSON.stringify(result));
    assert.equal(result.effect, "unknown"); assert.equal(result.repeat_safe, false);
    assert.equal(result.facts.completed, completed); assert.equal(result.facts.stopped, stopped);
    assert.equal(result.facts.steps[0].result.effect, "unknown");
    assert.equal(result.facts.steps[1].status, onError === "stop" ? "not_run" : "succeeded");
    if (onError === "continue") assert.equal(result.facts.steps[1].result.facts.value, "PRIVATE_SCRIPT_RESULT");
    assert.equal(await evaluations() - before, Number(count));
    assert.equal(await raw("document.getElementById('effect').textContent"), count);
    checks += 1;
    console.log(`PASS ${onError}: failed script executes once and composition preserves actual browser effects`);
  }
  await raw("document.getElementById('effect').textContent = '0'");
  const beforeDryRun = await evaluations();
  const dryRun = await call("browser_flow", { dry_run: true, steps: [
    { id: "script", tool: "browser_execute", arguments: { tab: opened.facts.tab, script: mutation } }
  ] });
  assert.equal(dryRun.status, "failed", JSON.stringify(dryRun));
  assert.equal(dryRun.effect, "none"); assert.equal(await evaluations(), beforeDryRun);
  assert.equal(await raw("document.getElementById('effect').textContent"), "0");
  checks += 1;
  console.log("PASS retired dry_run is rejected without executing the page program");
  const audit = readFileSync(home.auditFile, "utf8");
  assert.doesNotMatch(audit, /PRIVATE_SCRIPT_EXCEPTION|PRIVATE_SCRIPT_RESULT|document\.getElementById|runtime-thrown exception|forged prefix/);
  assert.ok(home.auditRecords().length >= checks, "the privacy assertion must inspect populated audit records");
  checks += 1;
  console.log("PASS real script source, result, and exception text stay out of durable audit");
  console.log(`Script browser journey: ${checks} cases passed with ${rig.version}; extension -> Chromium CDP -> browser edge -> service -> MCP results.`);
} finally {
  await rig?.stop();
  await new Promise((done) => { fixture.closeAllConnections(); fixture.close(done); });
  await home.cleanup();
}
