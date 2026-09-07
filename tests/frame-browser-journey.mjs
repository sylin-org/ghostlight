// H6 acceptance with the shipped MV3 worker, content scripts, Chrome document APIs and CDP.
// Only native-port discovery is replaced by a loopback test pipe to the real browser connector.
// No existing profile, native-host registration, or installed extension is changed.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { basename, dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";

const repository = resolve(import.meta.dirname, "..");
const binDir = resolve(process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0/debug"));
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || join(repository, ".tmp/chrome-testing/chrome-win64/chrome.exe");
assert.ok(existsSync(browser), "Set GHOSTLIGHT_TEST_BROWSER to Chrome for Testing (unpacked extensions required).");
const scratchRoot = join(repository, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "frame-browser-"));
const runtimeFile = join(binDir, `.ghostlight-frame-runtime-${process.pid}.json`);
const environment = { ...process.env, GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
  GHOSTLIGHT_AUDIT_FILE: join(scratch, "audit.jsonl"), GHOSTLIGHT_POLICY_FILE: join(scratch, "policy.json"),
  GHOSTLIGHT_DIAGNOSTICS_DIR: join(scratch, "diagnostics"), GHOSTLIGHT_NATIVE_HOST_DIR: join(scratch, "native-host") };
const children = [];
const delay = (ms) => new Promise((done) => setTimeout(done, ms));
async function until(check, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = await check(); if (value) return value; await delay(50); }
  throw new Error(`Timed out: ${label}`);
}
function start(path, args = []) {
  const child = spawn(path, args, { env: environment, windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
  child.stderr.on("data", (data) => { child.logs = (child.logs || "").slice(-4000) + data; });
  child.on("error", (error) => { child.startError = error; });
  children.push(child);
  return child;
}
function executable(name) { return join(binDir, name + (process.platform === "win32" ? ".exe" : "")); }
function channel(write) {
  let next = 0;
  const pending = new Map();
  return {
    send(method, params = {}, sessionId) {
      const id = ++next;
      return new Promise((done, reject) => {
        const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 45000);
        pending.set(id, { done, reject, timer });
        write({ id, method, params, ...(sessionId ? { sessionId } : {}) });
      });
    },
    receive(message) {
      const item = pending.get(message.id);
      if (!item) return;
      pending.delete(message.id); clearTimeout(item.timer);
      if (message.error) item.reject(new Error(JSON.stringify(message.error))); else item.done(message.result);
    }
  };
}
function policy(mode = "permitted_content", child = "denied", notice = "when_affected") {
  writeFileSync(environment.GHOSTLIGHT_POLICY_FILE, JSON.stringify({ schema: 3, name: "H6 isolated Sylin journey", version: randomUUID(),
    grants: [{ id: "parent", hosts: { allow: ["sylin.org", "localhost"] }, allowed: ["read", "action", "write", "execute"] },
      ...(child === "denied" ? [] : [{ id: "child", hosts: { allow: ["127.0.0.1"] }, allowed: child === "read" ? ["read"] : ["read", "action", "write", "execute"] }])],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" },
      { key: "content.frames.handling", value: mode, level: "mandatory" },
      { key: "content.frames.notice", value: notice, level: "mandatory" }]
  }));
}
const sourceUrls = ["https://sylin.org/ghostlight/demo/iframe/", "https://sylin.org/ghostlight/demo/iframe/form/"];
const source = await Promise.all(sourceUrls.map(async (url) => { const response = await fetch(url); assert.equal(response.status, 200); return response.text(); }));
const sourceEvidence = source.map((html, index) => ({ url: sourceUrls[index], sha256: createHash("sha256").update(html).digest("hex") }));
const token = randomUUID();
const queue = [];
const commands = [];
let poll, nativeReady = false, relay, port;
function nativeSend(frame) {
  const data = Buffer.from(JSON.stringify(frame)); const header = Buffer.alloc(4); header.writeUInt32LE(data.length);
  relay.stdin.write(Buffer.concat([header, data]));
}
function flush() { if (poll && queue.length) { poll.end(JSON.stringify(queue.splice(0))); poll = null; } }
const server = createServer(async (request, response) => {
  response.setHeader("access-control-allow-origin", "*");
  if (request.url === `/${token}`) {
    if (request.method === "POST") {
      let body = ""; for await (const chunk of request) body += chunk;
      nativeSend(JSON.parse(body)); response.end("ok");
    } else { poll = response; flush(); }
    return;
  }
  response.setHeader("content-type", "text/html");
  if (request.url === "/editor") {
    response.end(readFileSync(join(repository, "tests/fixtures/contenteditable.html"), "utf8"));
    return;
  }
  const child = request.url.startsWith("/form");
  let html = source[child ? 1 : 0].replace("<head>", '<head><base href="https://sylin.org/">');
  if (!child) html = html.replace('src="./form/"', `src="http://127.0.0.1:${port}/form"`)
    .replace("</body>", '<label>Parent note<input aria-label="Parent note" id="h6-parent"></label></body>');
  response.end(html);
});
let socket, cdp;
try {
  await new Promise((done) => server.listen(0, "127.0.0.1", done)); port = server.address().port;
  policy(); start(executable("ghostlight"));
  await until(() => existsSync(runtimeFile), "service startup");
  relay = start(executable("ghostlight-browser-connector"));
  let buffer = Buffer.alloc(0);
  relay.stdout.on("data", (chunk) => {
    buffer = Buffer.concat([buffer, chunk]);
    while (buffer.length >= 4) {
      const size = buffer.readUInt32LE(); if (buffer.length < size + 4) break;
      const frame = JSON.parse(buffer.subarray(4, size + 4)); buffer = buffer.subarray(size + 4);
      if (frame.kind === "hello_accepted") nativeReady = true;
      if (frame.kind === "request") commands.push(frame.request.command);
      queue.push(frame); flush();
    }
  });
  const extension = join(scratch, "extension"); cpSync(join(repository, "extension"), extension, { recursive: true });
  const worker = join(extension, "service-worker.js");
  // The substitute only carries native port messages. Every browser mechanism remains shipped code.
  const shim = `chrome.runtime.connectNative = () => {
    const listeners = [], closed = []; let live = true;
    const endpoint = ${JSON.stringify(`http://127.0.0.1:${port}/${token}`)};
    const port = { onMessage: { addListener(fn) { listeners.push(fn); } }, onDisconnect: { addListener(fn) { closed.push(fn); } },
      postMessage(frame) { fetch(endpoint, { method: "POST", body: JSON.stringify(frame) }).catch(() => {}); },
      disconnect() { live = false; } };
    (async () => { while (live) { try { for (const frame of await (await fetch(endpoint)).json()) for (const fn of listeners) fn(frame); }
      catch (_) { await new Promise(done => setTimeout(done, 100)); } } })(); return port;
  };\n`;
  writeFileSync(worker, shim + readFileSync(worker, "utf8"));
  const profile = join(scratch, "profile");
  const chromium = start(browser, ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
    `--load-extension=${extension}`, "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
    "--disable-component-update", "--disable-sync", "--window-size=1280,900", "about:blank"]);
  const portFile = join(profile, "DevToolsActivePort");
  await until(() => { if (chromium.startError) throw chromium.startError; return existsSync(portFile); }, "Chrome startup");
  const [debugPort, endpoint] = readFileSync(portFile, "utf8").trim().split(/\r?\n/);
  socket = new WebSocket(`ws://127.0.0.1:${debugPort}${endpoint}`);
  await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
  cdp = channel((message) => socket.send(JSON.stringify(message)));
  socket.onmessage = ({ data }) => cdp.receive(JSON.parse(data));
  const version = await cdp.send("Browser.getVersion");
  const workerTarget = await until(async () => (await cdp.send("Target.getTargets")).targetInfos.find((target) => target.type === "service_worker" && target.url.includes("service-worker.js")), "MV3 worker");
  const { sessionId } = await cdp.send("Target.attachToTarget", { targetId: workerTarget.targetId, flatten: true });
  const rawWorker = async (expression) => {
    const result = await cdp.send("Runtime.evaluate", { expression: `await (${expression})`, returnByValue: true, awaitPromise: true, replMode: true }, sessionId);
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails)); return result.result.value;
  };
  await until(() => nativeReady, "real connector negotiation");
  const connector = start(executable("ghostlight-mcp-connector"));
  let mcp = channel((message) => connector.stdin.write(JSON.stringify({ jsonrpc: "2.0", ...message }) + "\n"));
  const initialMcp = mcp;
  createInterface({ input: connector.stdout }).on("line", (line) => initialMcp.receive(JSON.parse(line)));
  await mcp.send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "h6-browser-journey", version: "1" } });
  connector.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
  let lastResponse;
  const call = async (name, args) => {
    lastResponse = await mcp.send("tools/call", { name, arguments: args });
    assert.ok(lastResponse.structuredContent, JSON.stringify(lastResponse)); return lastResponse.structuredContent;
  };
  const newSession = async () => {
    const next = start(executable("ghostlight-mcp-connector"));
    const peer = channel((message) => next.stdin.write(JSON.stringify({ jsonrpc: "2.0", ...message }) + "\n"));
    createInterface({ input: next.stdout }).on("line", (line) => peer.receive(JSON.parse(line)));
    await peer.send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "h6-case", version: "1" } });
    next.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
    mcp = peer;
  };
  const passed = [];
  const check = (name) => { passed.push(name); console.log(`PASS ${name}`); };
  const live = await call("browser_navigate", { url: sourceUrls[0], new_tab: true });
  assert.equal(live.status, "succeeded", JSON.stringify(live));
  let result = await call("browser_read", { tab: live.facts.tab, mode: "visible", max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.ok(result.facts.coverage.inspected_documents >= 2, JSON.stringify(result));
  assert.match(JSON.stringify(result), /Project name/); check("live Sylin composed document read");
  const opened = await call("browser_navigate", { url: `http://localhost:${port}/`, new_tab: true });
  assert.equal(opened.status, "succeeded", JSON.stringify(opened)); let tab = opened.facts.tab;
  let physical = await rawWorker(`(await chrome.tabs.query({})).find(tab => tab.url === 'http://localhost:${port}/').id`);
  const freshFixture = async () => {
    await newSession();
    const next = await call("browser_navigate", { url: `http://localhost:${port}/`, new_tab: true });
    assert.equal(next.status, "succeeded", JSON.stringify(next)); tab = next.facts.tab;
    physical = await rawWorker(`(await chrome.tabs.query({})).filter(tab => tab.url === 'http://localhost:${port}/').at(-1).id`);
  };
  const rawPage = async (expression) => rawWorker(`(await chrome.debugger.sendCommand({tabId:${physical}}, 'Runtime.evaluate', {expression:${JSON.stringify(expression)}, returnByValue:true, awaitPromise:true})).result.value`);
  await until(async () => (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`)).length >= 2, "fixture frame")
    .catch(async (error) => { console.error(await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`));
      console.error(await rawWorker(`chrome.debugger.sendCommand({tabId:${physical}}, 'Page.getFrameTree')`)); throw error; });
  for (const mode of ["permitted_content", "complete_operation", "complete_page"]) {
    if (mode !== "permitted_content") await freshFixture();
    for (const notice of ["on_demand", "when_affected", "when_excluded"]) {
      policy(mode, "denied", notice);
      result = await call("browser_read", { tab, mode: "visible", max_chars: 20000 });
      assert.equal(result.status, mode === "permitted_content" ? "succeeded" : "blocked", JSON.stringify(result));
      assert.equal(result.facts.coverage.excluded_documents, 1);
      assert.doesNotMatch(JSON.stringify(result), /127\.0\.0\.1|Project name|ifxf-project/);
      check(`${mode}/${notice}: excluded child never extracted`);
    }
  }
  policy();
  await freshFixture();
  result = await call("browser_execute", { tab, script: "document.title = 'must not execute'" });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.notEqual(await rawPage("document.title"), "must not execute"); check("script refused before execution");
  result = await call("browser_screenshot", { tab, full_page: true });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage.masked_regions, 1, JSON.stringify(result));
  const screenshot = lastResponse.content.find((item) => item.type === "image"); assert.ok(screenshot);
  writeFileSync(join(scratchRoot, "h6-masked-sylin.jpg"), Buffer.from(screenshot.data, "base64"));
  assert.equal(await rawPage("getComputedStyle(document.querySelector('iframe')).visibility"), "visible");
  check("excluded Sylin form masked and original styles restored");
  policy("permitted_content", "all");
  result = await call("browser_read", { tab, mode: "visible", max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.match(JSON.stringify(result), /Project name/);
  check("same Sylin content fully available when allowed");
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 100 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  const project = result.facts.items.find((item) => item.name === "Project name");
  const parent = result.facts.items.find((item) => item.name === "Parent note");
  assert.ok(project && parent, JSON.stringify(result));
  policy("permitted_content", "read");
  result = await call("browser_fill_form", { tab, fields: [{ target: parent.target, value: "MUST_NOT_FILL" }, { target: project.target, value: "MUST_NOT_FILL" }] });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.equal(await rawPage("document.getElementById('h6-parent').value"), "");
  check("readable child cannot be written; known batch denial precedes parent write");
  policy();
  for (const mode of ["permitted_content", "complete_operation"]) {
    policy(mode);
    result = await call("browser_fill_form", { tab, fields: [{ target: parent.target, value: mode }] });
    assert.equal(result.status, "succeeded", JSON.stringify(result));
    assert.equal(result.facts.coverage.excluded_documents, 0); check(`${mode}: unrelated parent target remains usable`);
  }
  policy(); await freshFixture();
  result = await call("browser_find", { tab, text: "Project name", scope: "control" });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.matches.length, 0, JSON.stringify(result));
  assert.equal(result.facts.coverage.excluded_documents, 1); check("denied-only match yields scoped empty search");
  result = await call("browser_wait", { tab, condition: "text_absent", value: "Project name", timeout_ms: 100 });
  assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.facts.coverage?.excluded_documents, 1, JSON.stringify(result));
  check("absence across excluded content remains unproven");
  result = await call("browser_read", { tab, max_chars: 500 });
  assert.equal(result.facts.coverage.limited_by_size, true); assert.equal(result.facts.coverage.excluded_documents, 1);
  check("size ceiling and policy exclusion remain separate");
  result = await call("browser_screenshot", { tab });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.coverage.masked_regions, 1);
  const view = result.facts.view;
  result = await call("browser_screenshot", { view, x: 300, y: 350, width: 400, height: 250 });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.coverage.masked_regions, 1);
  check("viewport and magnified captures preserve exclusion");
  policy("permitted_content", "all");
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 100 });
  const oldProject = result.facts.items.find((item) => item.name === "Project name"); assert.ok(oldProject);
  await rawPage(`document.querySelector('iframe').src = 'http://127.0.0.1:${port}/form?new-document'; true`);
  await delay(200);
  result = await call("browser_fill_form", { tab, fields: [{ target: oldProject.target, value: "STALE" }] });
  assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.effect, "none");
  check("navigation cannot revive a stale embedded target");
  result = await call("browser_record", { action: "start", tab });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); const recording = result.facts.recording;
  assert.ok(result.facts.frame_count > 0, JSON.stringify(result));
  await rawPage(`document.querySelector('iframe').src = 'http://127.0.0.1:${port}/form?recording-boundary'; true`);
  await delay(300);
  result = await call("browser_record", { action: "status", recording });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.stop_reason, "document_boundary", JSON.stringify(result));
  assert.ok(result.facts.frame_count > 0); const frozen = result.facts.frame_count;
  await delay(200);
  result = await call("browser_record", { action: "status", recording }); assert.equal(result.facts.frame_count, frozen);
  result = await call("browser_record", { action: "save", recording });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.ok(lastResponse.content.some((item) => item.type === "image"));
  check("recording stops at its admitted document boundary and prior frames remain exportable");
  policy();
  result = await call("browser_record", { action: "save", recording });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.ok(!lastResponse.content.some((item) => item.type === "image"));
  check("replay export rechecks its captured embedded sources against current policy");
  policy("permitted_content", "all");
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 100 });
  const field = (name) => result.facts.items.find((item) => item.name === name)?.target;
  const values = [["Project name", "Ghostlight H6 fixture"], ["Contact email", "h6@example.invalid"],
    ["Repository URL", "https://example.invalid/fixture"], ["Maintainer type", "Individual"],
    ["Build system", "GitHub Actions"], ["Notes", "Isolated browser acceptance"]];
  const fields = values.map(([name, value]) => { const target = field(name); assert.ok(target, name); return { target, value }; });
  const submit = field("Submit application"); assert.ok(submit);
  result = await call("browser_fill_form", { tab, fields, submit_target: submit });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_read", { tab, max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.match(JSON.stringify(result), /The frame confirmed receipt locally/);
  check("permitted Sylin embedded form fills and submits its local simulation");
  policy();
  // Deliberately change the embedding element during capture, while preserving the browser's
  // document inventory. Verification must discard the image, then restore the original styles.
  await rawWorker(`(() => { globalThis.h6OriginalCapture = chrome.debugger.sendCommand;
    chrome.debugger.sendCommand = async function(target, method, params) {
      if (method === 'Page.captureScreenshot') await h6OriginalCapture(target, 'Runtime.evaluate', { expression: "document.querySelector('iframe').style.opacity='1'", returnByValue:true });
      return h6OriginalCapture(target, method, params);
    }; return true; })()`);
  result = await call("browser_screenshot", { tab });
  assert.equal(result.status, "failed", JSON.stringify(result));
  assert.ok(!lastResponse.content.some((item) => item.type === "image"));
  await rawWorker("(chrome.debugger.sendCommand = h6OriginalCapture, true)");
  assert.equal(await rawPage("getComputedStyle(document.querySelector('iframe')).visibility"), "visible");
  check("mask mutation discards the capture and cleanup restores the embed");
  result = await call("browser_inspect", { tab, scope: "structure", max_items: 100 });
  const heading = result.facts.items.find((item) => item.name === "Apply to the Sylin Foundry"); assert.ok(heading);
  result = await call("browser_screenshot", { tab, target: heading.target });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.coverage.masked_regions, 1);
  check("target captures use the same exclusion mechanism");
  await rawPage("window.scrollTo(0,0); true");
  result = await call("browser_screenshot", { tab }); const pointView = result.facts.view;
  const point = await rawPage("(()=>{const box=document.querySelector('iframe').getBoundingClientRect();return {x:box.left+60,y:box.top+60};})()");
  result = await call("browser_click", { view: pointView, x: point.x, y: point.y });
  assert.equal(result.status, "blocked", JSON.stringify(result)); assert.equal(result.effect, "none");
  check("coordinates from a masked image cannot act inside the excluded document");
  await rawPage("document.querySelector('iframe').src='about:blank'; true"); await delay(200);
  result = await call("browser_read", { tab });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage.excluded_documents, 0); assert.equal(result.facts.coverage.unavailable_documents, 1);
  result = await call("browser_execute", { tab, script: "document.title='UNAVAILABLE_SCRIPT'" });
  assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.effect, "none");
  check("unavailable embedded documents are separate and bounded scripts refuse");
  await newSession();
  const editorPage = await call("browser_navigate", { url: `http://localhost:${port}/editor`, new_tab: true });
  assert.equal(editorPage.status, "succeeded", JSON.stringify(editorPage)); tab = editorPage.facts.tab;
  physical = await rawWorker(`(await chrome.tabs.query({})).find(tab => tab.url === 'http://localhost:${port}/editor').id`);
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 20 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  const replyTarget = result.facts.items.find((item) => item.name === "Reply")?.target;
  const shadowTarget = result.facts.items.find((item) => item.name === "Shadow reply")?.target;
  assert.ok(replyTarget); assert.ok(shadowTarget);
  assert.ok(result.facts.items.find((item) => item.name === "Hidden draft helper")?.state.includes("hidden"));
  assert.ok(!result.facts.items.find((item) => item.name === "Reply").state.includes("hidden"));
  // Prove this fixture catches the old DOM-assignment plus synthetic-event failure.
  await rawPage("document.querySelector('#reply').textContent='UNACCEPTED'; document.querySelector('#reply').dispatchEvent(new Event('input',{bubbles:true})); true");
  await delay(50);
  assert.equal((await rawPage("editorEvidence()")).reply.rendered, "Initial draft");
  for (const [target, name, value] of [[replyTarget, "reply", "Draft line one\nDraft line two"],
    [shadowTarget, "shadow", "Shadow draft\nSecond line"], [replyTarget, "reply", "Replacement draft"],
    [replyTarget, "reply", ""], [replyTarget, "reply", ""], [shadowTarget, "shadow", ""]]) {
    const before = await rawPage("editorEvidence()");
    result = await call("browser_fill_form", { tab, fields: [{ target, value }] });
    assert.equal(result.status, "succeeded", JSON.stringify(result));
    assert.equal(result.facts.filled_count, 1); assert.equal(result.facts.submitted, false);
    await delay(50);
    const after = await rawPage("editorEvidence()");
    assert.equal(after[name].value, value);
    assert.equal(after[name].rendered, value);
    assert.equal(after[name].synthetic, before[name].synthetic);
    assert.equal(after[name === "reply" ? "shadow" : "reply"].rendered, before[name === "reply" ? "shadow" : "reply"].rendered);
    assert.equal(after.submissions, 0);
  }
  check("controlled rich editors retain native multiline fills and clears without synthetic events or submission");
  result = await call("browser_fill_form", { tab, fields: [{ target: replyTarget, value: "Old typed draft" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_type_text", { tab, target: replyTarget, text: "Typed replacement", clear_first: true });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  await delay(50);
  assert.equal((await rawPage("editorEvidence()")).reply.rendered, "Typed replacement");
  result = await call("browser_fill_form", { tab, fields: [{ target: shadowTarget, value: "Focused draft" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_type_text", { tab, focused: true, text: "", clear_first: true });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  await delay(50);
  const clearedEditor = await rawPage("editorEvidence()");
  assert.equal(clearedEditor.shadow.rendered, "");
  assert.equal(clearedEditor.reply.rendered, "Typed replacement");
  assert.equal(clearedEditor.shadow.synthetic, 0); assert.equal(clearedEditor.submissions, 0);
  check("targeted typing replacement and focused shadow-editor clearing preserve native editor state");
  const audit = readFileSync(environment.GHOSTLIGHT_AUDIT_FILE, "utf8");
  assert.doesNotMatch(audit, /127\.0\.0\.1|h6@example\.invalid|Ghostlight H6 fixture|ifxf-project|Draft line one|Shadow draft|Replacement draft/);
  check("durable audit excludes embedded origins, values, and selectors");
  writeFileSync(join(scratchRoot, "h6-browser-evidence.json"), JSON.stringify({ browser: version.product, source: sourceEvidence, passed,
    transport: "MV3 native-port shim -> real browser connector -> orchestrator -> real MCP connector" }, null, 2));
  console.log(`H6 browser journey: ${passed.length} checks passed with ${version.product}.`);
} finally {
  if (cdp && socket?.readyState === WebSocket.OPEN) { try { await cdp.send("Browser.close"); } catch {} }
  socket?.close(); poll?.end("[]");
  for (const child of children.toReversed()) if (child.exitCode === null && child.signalCode === null) child.kill();
  await until(() => children.every((child) => child.exitCode !== null || child.signalCode !== null || child.startError), "owned children exit");
  server.closeAllConnections(); await new Promise((done) => server.close(done));
  assert.equal(dirname(resolve(scratch)), resolve(scratchRoot)); assert.ok(basename(scratch).startsWith("frame-browser-"));
  rmSync(scratch, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  for (const path of [runtimeFile, runtimeFile.replace(/\.json$/, ".lock")]) { assert.equal(dirname(resolve(path)), binDir); rmSync(path, { force: true }); }
}
