// H6 acceptance with the shipped MV3 worker, content scripts, Chrome document APIs and CDP.
// Only native-port discovery is replaced by a loopback test pipe to the real browser connector.
// No existing profile, native-host registration, or installed extension is changed.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { readDevToolsPort, removeBrowserScratch, waitForChromiumExit } from "./lib/chromium.mjs";

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
  const begin = (method, params = {}, sessionId) => {
    const id = ++next;
    const promise = new Promise((done, reject) => {
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 45000);
      pending.set(id, { done, reject, timer });
      write({ id, method, params, ...(sessionId ? { sessionId } : {}) });
    });
    return { id, promise };
  };
  return {
    begin,
    send(method, params = {}, sessionId) { return begin(method, params, sessionId).promise; },
    notify(method, params = {}) { write({ method, params }); },
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
    grants: [{ id: "parent", hosts: { allow: child === "unrestricted" ? ["*"] : ["sylin.org", "localhost"] }, allowed: ["read", "action", "write", "execute"] },
      ...(["denied", "unrestricted"].includes(child) ? [] : [{ id: "child", hosts: { allow: ["127.0.0.1"] }, allowed: child === "read" ? ["read"] : ["read", "action", "write", "execute"] }])],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" },
      { key: "content.frames.handling", value: mode, level: "mandatory" },
      { key: "content.frames.notice", value: notice, level: "mandatory" }]
  }));
}
const sourceUrls = ["https://sylin.org/ghostlight/demo/iframe/", "https://sylin.org/ghostlight/demo/iframe/form/"];
const liveSylin = process.env.GHOSTLIGHT_TEST_LIVE_SYLIN === "1";
const snapshot = JSON.parse(readFileSync(join(repository, "tests/fixtures/sylin-iframe.json"), "utf8"));
const source = liveSylin
  ? await Promise.all(sourceUrls.map(async (url) => { const response = await fetch(url, { signal: AbortSignal.timeout(20000) }); assert.equal(response.status, 200); return response.text(); }))
  : snapshot.pages.map((page, index) => {
    assert.equal(page.url, sourceUrls[index]);
    assert.equal(createHash("sha256").update(page.body).digest("hex"), page.sha256);
    return page.body;
  });
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
  const asset = snapshot.assets.find((item) => item.path.split("?")[0] === request.url.split("?")[0]);
  if (asset) {
    const bytes = Buffer.from(asset.data_base64, "base64");
    assert.equal(createHash("sha256").update(bytes).digest("hex"), asset.sha256);
    response.setHeader("content-type", asset.content_type); response.end(bytes); return;
  }
  if (request.url === "/editor") {
    response.end(readFileSync(join(repository, "tests/fixtures/contenteditable.html"), "utf8"));
    return;
  }
  const child = request.url.startsWith("/form");
  let html = source[child ? 1 : 0].replace("<head>", `<head><base href="http://localhost:${port}/">`);
  if (child && request.url.includes("readonly-fixture")) html = html.replace("</body>", '<label>Embedded read only draft<input readonly aria-label="Embedded read only draft" value="Protected embedded input"></label></body>');
  if (!child) html = html.replace('src="./form/"', `src="http://${request.url === "/same-origin" ? "localhost" : "127.0.0.1"}:${port}/form"`)
    .replace("</body>", '<label>Parent note<input aria-label="Parent note" id="h6-parent"></label></body>');
  response.end(html);
});
let socket, cdp, chromium;
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
  chromium = start(browser, ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
    ...(process.env.GHOSTLIGHT_TEST_NO_SANDBOX === "1" ? ["--no-sandbox"] : []),
    ...(!liveSylin ? ["--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE localhost, EXCLUDE 127.0.0.1"] : []),
    `--load-extension=${extension}`, "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
    "--disable-component-update", "--disable-sync", "--window-size=1280,900", "about:blank"]);
  const [debugPort, endpoint] = await readDevToolsPort(profile, chromium);
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
  const dispatchedSince = (index) => commands.slice(index).map((command) =>
    command.command === "in_documents" ? command.primitive.command : command.command);
  const live = await call("browser_navigate", { url: liveSylin ? sourceUrls[0] : `http://localhost:${port}/same-origin`, new_tab: true });
  assert.equal(live.status, "succeeded", JSON.stringify(live));
  let result = await call("browser_read", { tab: live.facts.tab, mode: "visible", max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.ok(result.facts.coverage.inspected_documents >= 2, JSON.stringify(result));
  assert.match(JSON.stringify(result), /Project name/); check(`${liveSylin ? "live" : "snapshot"} Sylin composed document read`);
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
  const beforeBlockedScript = commands.length;
  result = await call("browser_execute", { tab, script: "document.title = 'must not execute'" });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.ok(!dispatchedSince(beforeBlockedScript).includes("evaluate_script"));
  assert.notEqual(await rawPage("document.title"), "must not execute"); check("script refused before execution");
  result = await call("browser_screenshot", { tab, full_page: true });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage.masked_regions, 1, JSON.stringify(result));
  const screenshot = lastResponse.content.find((item) => item.type === "image"); assert.ok(screenshot);
  writeFileSync(join(scratchRoot, "h6-masked-sylin.jpg"), Buffer.from(screenshot.data, "base64"));
  // Inspect the delivered JPEG itself. A coverage counter cannot prove that excluded pixels
  // were actually hidden; deliberately removing the mask makes these samples fail.
  const maskPixels = await rawPage(`(async () => {
    const image = new Image(); image.src = ${JSON.stringify(`data:${screenshot.mimeType};base64,${screenshot.data}`)};
    await image.decode(); const canvas = document.createElement('canvas');
    canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext('2d'); context.drawImage(image,0,0);
    const rectangle = document.querySelector('iframe').getBoundingClientRect();
    const scale = image.width / document.documentElement.scrollWidth;
    return [0.2,0.8].flatMap(x => [0.2,0.8].map(y => [...context.getImageData(
      Math.round((rectangle.left + scrollX + rectangle.width*x)*scale),
      Math.round((rectangle.top + scrollY + rectangle.height*y)*scale),1,1).data]));
  })()`);
  for (const pixel of maskPixels) for (const [index, expected] of [32, 36, 43, 255].entries()) {
    assert.ok(Math.abs(pixel[index] - expected) <= 5, `Excluded region pixel: ${pixel}`);
  }
  assert.equal(await rawPage("getComputedStyle(document.querySelector('iframe')).visibility"), "visible");
  check("excluded Sylin form masked and original styles restored");
  policy("permitted_content", "all");
  result = await call("browser_read", { tab, mode: "visible", max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.match(JSON.stringify(result), /Project name/);
  check("same Sylin content fully available when allowed");
  for (const mode of ["permitted_content", "complete_operation", "complete_page"]) {
    for (const notice of ["on_demand", "when_affected", "when_excluded"]) {
      policy(mode, "all", notice);
      result = await call("browser_read", { tab, mode: "visible", max_chars: 20000 });
      assert.equal(result.status, "succeeded", JSON.stringify(result));
      assert.equal(result.facts.coverage.excluded_documents, 0);
      assert.equal(result.facts.coverage.unavailable_documents, 0);
      assert.ok(result.facts.coverage.inspected_documents >= 2);
      assert.match(JSON.stringify(result), /Project name/);
      check(`${mode}/${notice}: fully permitted content remains available`);
    }
  }
  result = await call("browser_execute", { tab, script: "document.body.dataset.executionCount=String(Number(document.body.dataset.executionCount||0)+1); await Promise.resolve(); return Number(document.body.dataset.executionCount);" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.value, 1);
  assert.equal(await rawPage("document.body.dataset.executionCount"), "1");
  check("permitted embedded page executes awaited bare return once through the shipped MV3 worker");
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
  policy("permitted_content", "all");
  await rawPage(`document.querySelector('iframe').src='http://127.0.0.1:${port}/form?readonly-fixture'; true`);
  await delay(200);
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 100 });
  const embeddedReadonly = result.facts.items.find((item) => item.name === "Embedded read only draft");
  assert.ok(embeddedReadonly, JSON.stringify(result));
  result = await call("browser_fill_form", { tab, fields: [
    { target: parent.target, value: "MUST_NOT_CHANGE_PARENT" },
    { target: embeddedReadonly.target, value: "MUST_NOT_CHANGE_EMBED" }
  ] });
  assert.equal(await rawPage("document.getElementById('h6-parent').value"), "");
  assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.effect, "none");
  check("later embedded readonly field refuses before editing an allowed parent document");
  policy();
  for (const mode of ["permitted_content", "complete_operation"]) {
    policy(mode);
    result = await call("browser_fill_form", { tab, fields: [{ target: parent.target, value: mode }] });
    assert.equal(result.status, "succeeded", JSON.stringify(result));
    assert.equal(result.facts.coverage.excluded_documents, 0); check(`${mode}: unrelated parent target remains usable`);
  }
  policy();
  result = await call("browser_fill_form", { tab, fields: [
    { selector: { name: "Parent note", role: "textbox", exact: true }, value: "Standalone semantic draft" }
  ] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.submitted, false);
  assert.equal(await rawPage("document.getElementById('h6-parent').value"), "Standalone semantic draft");
  result = await call("browser_fill_form", { tab, fields: [{ target: parent.target, value: "Standalone handle draft" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.submitted, false);
  assert.equal(await rawPage("document.getElementById('h6-parent').value"), "Standalone handle draft");
  check("standalone permitted controls support equivalent semantic-selector and handle fills");
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
  const oldDocument = (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`)).find((frame) => frame.frameId !== 0);
  await rawPage(`document.querySelector('iframe').src = 'http://127.0.0.1:${port}/form?new-document'; true`);
  const newDocument = await until(async () => (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`))
    .find((frame) => frame.frameId === oldDocument.frameId && frame.documentId !== oldDocument.documentId), "replacement embedded document");
  assert.equal(newDocument.frameId, oldDocument.frameId);
  assert.notEqual(newDocument.documentId, oldDocument.documentId);
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
  policy("permitted_content", "unrestricted");
  result = await call("browser_execute", { tab, script: "document.body.dataset.allOpen='AVAILABLE'; return document.body.dataset.allOpen;" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.value, "AVAILABLE");
  assert.equal(await rawPage("document.body.dataset.allOpen"), "AVAILABLE");
  check("unrestricted scripts remain usable when an opaque embed is present");
  result = await call("browser_record", { action: "start", tab });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); const unrestrictedRecording = result.facts.recording;
  await rawPage(`document.querySelector('iframe').src = 'http://127.0.0.1:${port}/form?unrestricted-recording'; true`);
  await delay(300);
  result = await call("browser_record", { action: "status", recording: unrestrictedRecording });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.state, "recording", JSON.stringify(result));
  assert.equal(result.facts.stop_reason, null);
  result = await call("browser_record", { action: "stop", recording: unrestrictedRecording });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_record", { action: "save", recording: unrestrictedRecording });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.ok(lastResponse.content.some((item) => item.type === "image"));
  check("unrestricted recording survives document navigation and exports its retained replay");
  policy("permitted_content", "all");
  result = await call("browser_record", { action: "save", recording: unrestrictedRecording });
  assert.equal(result.status, "failed", JSON.stringify(result));
  assert.equal(result.facts.reason, "document_unavailable");
  assert.equal(result.facts.coverage.unavailable_documents, 1);
  assert.ok(!lastResponse.content.some((item) => item.type === "image"));
  check("incomplete recorded source evidence cannot later be exported under host restrictions");
  policy(); await freshFixture();
  const beforeBlockedRecording = commands.length;
  result = await call("browser_record", { action: "start", tab });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.equal(result.effect, "none");
  assert.ok(!dispatchedSince(beforeBlockedRecording).includes("start_recording"));
  assert.ok(!lastResponse.content.some((item) => item.type === "image"));
  check("recording cannot start across an excluded document boundary");
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
  const beforeSemanticShadow = await rawPage("editorEvidence()");
  result = await call("browser_fill_form", { tab, restrict_capabilities: ["read", "write"], fields: [
    { selector: { name: "Shadow reply", role: "textbox", exact: true }, value: "Standalone semantic shadow draft" }
  ] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.submitted, false);
  const afterSemanticShadow = await rawPage("editorEvidence()");
  assert.equal(afterSemanticShadow.shadow.value, "Standalone semantic shadow draft");
  assert.equal(afterSemanticShadow.shadow.rendered, afterSemanticShadow.shadow.value);
  assert.equal(afterSemanticShadow.reply.rendered, beforeSemanticShadow.reply.rendered);
  assert.equal(afterSemanticShadow.submissions, 0);
  result = await call("browser_fill_form", { tab, fields: [{ target: shadowTarget, value: "Standalone handle shadow draft" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.submitted, false);
  const afterHandleShadow = await rawPage("editorEvidence()");
  assert.equal(afterHandleShadow.shadow.value, "Standalone handle shadow draft");
  assert.equal(afterHandleShadow.shadow.rendered, afterHandleShadow.shadow.value);
  assert.equal(afterHandleShadow.submissions, 0);
  check("open-shadow rich editors outside forms support equivalent semantic-selector and handle fills");
  await rawPage("(()=>{const input=document.createElement('input');input.type='file';input.id='standalone-upload';input.setAttribute('aria-label','Standalone upload');document.body.append(input);return true;})()");
  const uploadPath = join(scratch, "standalone-upload.txt");
  const uploadedFile = () => rawPage("(async()=>{const files=document.getElementById('standalone-upload').files;return {count:files.length,name:files[0]?.name,text:await files[0]?.text()};})()");
  writeFileSync(uploadPath, "SYNTHETIC_UPLOAD_BY_SELECTOR");
  result = await call("browser_upload", { tab, restrict_capabilities: ["read", "write"],
    selector: { name: "Standalone upload", exact: true }, paths: [uploadPath] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.uploaded_count, 1);
  assert.deepEqual(await uploadedFile(), { count: 1, name: "standalone-upload.txt", text: "SYNTHETIC_UPLOAD_BY_SELECTOR" });
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 30 });
  const uploadTarget = result.facts.items.find(item => item.name === "Standalone upload")?.target;
  assert.ok(uploadTarget, JSON.stringify(result));
  writeFileSync(uploadPath, "SYNTHETIC_UPLOAD_BY_HANDLE");
  result = await call("browser_upload", { tab, restrict_capabilities: ["write"], target: uploadTarget, paths: [uploadPath] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.uploaded_count, 1);
  assert.deepEqual(await uploadedFile(), { count: 1, name: "standalone-upload.txt", text: "SYNTHETIC_UPLOAD_BY_HANDLE" });
  assert.equal((await rawPage("editorEvidence()")).submissions, 0);
  check("standalone file inputs support equivalent selector and handle attachment with verified file contents");
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
  for (const [target, name] of [[replyTarget, "reply"], [shadowTarget, "shadow"]]) {
    for (const focused of [false, true]) {
      result = await call("browser_fill_form", { tab, fields: [{ target, value: "Existing draft" }] });
      assert.equal(result.status, "succeeded", JSON.stringify(result));
      const before = await rawPage("editorEvidence()");
      const replacement = `${name} ${focused ? "focused" : "targeted"} action draft`;
      result = await call("browser_type_text", { tab, ...(focused ? { focused: true } : { target }),
        text: replacement, clear_first: true, restrict_capabilities: ["action"] });
      assert.equal(result.status, "succeeded", JSON.stringify(result));
      assert.equal(result.effect, "applied");
      await delay(50);
      const after = await rawPage("editorEvidence()");
      assert.equal(after[name].value, replacement);
      assert.equal(after[name].rendered, replacement);
      assert.equal(after[name].synthetic, before[name].synthetic);
      assert.equal(after[name === "reply" ? "shadow" : "reply"].rendered, before[name === "reply" ? "shadow" : "reply"].rendered);
      assert.equal(after.submissions, 0);
      check(`${name}/${focused ? "focused" : "targeted"}: Action-only typing retains its draft and succeeds`);
    }
  }
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 20 });
  const editorTargets = new Map(result.facts.items.map((item) => [item.name, item.target]));
  result = await call("browser_fill_form", { tab, restrict_capabilities: ["read", "write"], fields: [
    { target: editorTargets.get("Ordinary draft"), value: "Ordinary replacement" },
    { target: editorTargets.get("Multiline draft"), value: "Ordinary first line\nOrdinary second line" }
  ] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.filled_count, 2); assert.equal(result.facts.submitted, false);
  const ordinaryFilled = await rawPage("editorEvidence()");
  assert.equal(ordinaryFilled.ordinary, "Ordinary replacement");
  assert.equal(ordinaryFilled.multiline, "Ordinary first line\nOrdinary second line");
  assert.equal(ordinaryFilled.submissions, 0);
  check("Read+Write fills ordinary input and textarea without submitting");
  for (const name of ["Read only draft", "Disabled draft", "Hidden draft helper"]) {
    const before = await rawPage("editorEvidence()");
    result = await call("browser_fill_form", { tab, fields: [
      { target: replyTarget, value: "MUST_NOT_CHANGE_EARLIER_EDITOR" },
      { target: editorTargets.get(name), value: "MUST_NOT_CHANGE_PROTECTED_FIELD" }
    ] });
    assert.deepEqual(await rawPage("editorEvidence()"), before);
    assert.equal(result.status, "failed", JSON.stringify(result));
    assert.equal(result.effect, "none"); assert.equal(result.repeat_safe, true);
    check(`${name}: batch refusal preserves every existing draft without claiming success`);
  }
  for (const invalid of ["option", "submit"]) {
    const before = await rawPage("editorEvidence()");
    result = await call("browser_fill_form", { tab, fields: [
      { target: replyTarget, value: "MUST_NOT_CHANGE_BEFORE_VALIDATION" },
      ...(invalid === "option" ? [{ target: editorTargets.get("Draft category"), value: "Missing category" }] : [])
    ], ...(invalid === "submit" ? { submit_target: editorTargets.get("Other form send") } : {}) });
    assert.deepEqual(await rawPage("editorEvidence()"), before);
    assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.effect, "none");
    check(`invalid ${invalid}: form preparation refuses before the first edit or submission`);
  }
  // Page code can invalidate a later field after preparation. Preserve the first edit and
  // uncertainty in that case; preflight is not a transaction or permission to replay a batch.
  const beforeChangedField = await rawPage("editorEvidence()");
  await rawPage("document.querySelector('#ordinary').addEventListener('input',()=>{document.querySelector('#multiline').readOnly=true;},{once:true}); true");
  result = await call("browser_fill_form", { tab, fields: [
    { target: editorTargets.get("Ordinary draft"), value: "PARTIAL_DRAFT_EFFECT" },
    { target: editorTargets.get("Multiline draft"), value: "MUST_NOT_CHANGE_LATER_FIELD" }
  ] });
  const afterChangedField = await rawPage("editorEvidence()");
  assert.equal(afterChangedField.ordinary, "PARTIAL_DRAFT_EFFECT");
  assert.equal(afterChangedField.multiline, beforeChangedField.multiline);
  assert.equal(afterChangedField.submissions, 0);
  assert.equal(result.status, "unknown", JSON.stringify(result));
  assert.equal(result.effect, "unknown"); assert.equal(result.repeat_safe, false);
  await rawPage("document.querySelector('#multiline').readOnly=false; true");
  check("a page change after the first edit preserves the partial draft and refuses replay");
  for (const capabilities of [["read"], ["write"], ["read", "write"]]) {
    const before = await rawPage("editorEvidence()");
    const beforeDispatch = commands.length;
    result = await call("browser_fill_form", { tab, restrict_capabilities: capabilities,
      fields: [{ target: replyTarget, value: "Complete allowlist draft" }] });
    if (capabilities.length === 1) {
      assert.equal(result.status, "blocked", JSON.stringify(result)); assert.equal(result.effect, "none");
      assert.equal(result.facts.restriction, "restrict_capabilities");
      assert.deepEqual(result.facts.required_capabilities, ["read", "write"]);
      assert.match(result.summary, /restrict_capabilities.*read \+ write/);
      assert.ok(!dispatchedSince(beforeDispatch).some((command) => ["fill_form", "type_text"].includes(command)));
      assert.deepEqual(await rawPage("editorEvidence()"), before);
    } else {
      assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.submitted, false);
      await delay(50);
      const after = await rawPage("editorEvidence()");
      assert.equal(after.reply.rendered, "Complete allowlist draft");
      assert.equal(after.reply.value, "Complete allowlist draft"); assert.equal(after.submissions, 0);
    }
    check(`${capabilities.join("+")}: draft fill uses the complete per-call capability allowlist`);
  }
  // Exercise the shipped renderer in its closed shadow tree. Holding an observation or script
  // gives us time to inspect the actual live elements while another controlled tab is active.
  policy("permitted_content", "all"); await newSession();
  const openVisualTab = async name => {
    const url = `http://localhost:${port}/editor#${name}`;
    const opened = await call("browser_navigate", { url, new_tab: true });
    assert.equal(opened.status, "succeeded", JSON.stringify(opened));
    const physical = await rawWorker(`(await chrome.tabs.query({})).find(tab => tab.url === ${JSON.stringify(url)}).id`);
    return { handle: opened.facts.tab, physical };
  };
  const targetVisual = await openVisualTab("visual-target");
  const unrelatedVisual = await openVisualTab("visual-unrelated");
  const firstVisualPeer = mcp;
  await rawWorker(`chrome.tabs.update(${unrelatedVisual.physical}, {active:true})`);
  const visualState = async tabId => rawWorker(`(async () => {
    const {root} = await chrome.debugger.sendCommand({tabId:${tabId}}, 'DOM.getDocument', {depth:-1,pierce:true});
    const pending = [root], counts = {wheel:0,read:0,signature:0};
    while(pending.length) {
      const node = pending.pop(), attributes = node.attributes || [];
      const classes = String(attributes[attributes.indexOf('class')+1] || '').split(/\\s+/);
      if(classes.includes('workwheel')) counts.wheel++;
      if(classes.includes('read-scan')) counts.read++;
      if(classes.includes('signature')) counts.signature++;
      pending.push(...(node.children||[]),...(node.shadowRoots||[]),...(node.pseudoElements||[]));
      if(node.contentDocument) pending.push(node.contentDocument);
    }
    return counts;
  })()`);
  const visualPage = (target, expression) => rawWorker(`(await chrome.debugger.sendCommand({tabId:${target.physical}},
    'Runtime.evaluate', {expression:${JSON.stringify(expression)},returnByValue:true,awaitPromise:true})).result.value`);
  const captureVisual = async (target, name) => {
    // The product screenshot path intentionally hides presentation. This test observes the
    // actual Chrome surface directly, retaining the wheel for human comparison with cleanup.
    const data = await rawWorker(`(await chrome.debugger.sendCommand({tabId:${target.physical}},
      'Page.captureScreenshot', {format:'png'})).data`);
    writeFileSync(join(scratchRoot, `h6-visual-${name}.png`), Buffer.from(data, "base64"));
  };
  await rawWorker(`(() => {
    globalThis.visualOriginalSendMessage=chrome.tabs.sendMessage.bind(chrome.tabs);
    globalThis.visualPresentations=[]; globalThis.visualReadTarget=${targetVisual.physical};
    chrome.tabs.sendMessage=async function(tabId,message,...rest) {
      if(message.kind==='present') visualPresentations.push({tabId,signal:message.signal});
      if(message.kind==='read_text' && tabId===visualReadTarget) await new Promise(resolve=>{globalThis.visualReleaseRead=resolve;});
      return visualOriginalSendMessage(tabId,message,...rest);
    }; return true;
  })()`);
  const visualRequests = [];
  const beginVisual = (peer, name, args) => {
    const request = peer.begin("tools/call", { name, arguments: args });
    request.promise.catch(() => {}); // The main assertion or cleanup awaits this same promise.
    visualRequests.push({ peer, ...request });
    return request;
  };
  try {
    const reading = beginVisual(firstVisualPeer, "browser_read", { tab: targetVisual.handle });
    await until(async () => (await visualState(targetVisual.physical)).read > 0, "read scan on its background target", 5000);
    assert.equal((await visualState(unrelatedVisual.physical)).read, 0);
    await rawWorker("(visualReadTarget=null,visualReleaseRead(),true)");
    assert.equal((await reading.promise).structuredContent.status, "succeeded");
    const readDeliveries = await rawWorker("visualPresentations.filter(item=>item.signal.activity==='read' && item.signal.signal==='start').map(item=>item.tabId)");
    assert.ok(readDeliveries.length > 0); assert.ok(readDeliveries.every(id => id === targetVisual.physical));
    check("read animation renders only on the requested background tab");

    const heldScript = (peer, target, finish) => beginVisual(peer, "browser_execute", {
      tab: target.handle, timeout_ms: 15000,
      script: `await new Promise(resolve=>{globalThis.__visualRelease=resolve;}); ${finish}`
    });
    const firstScript = heldScript(firstVisualPeer, targetVisual, "return 'visual success';");
    await until(async () => (await visualState(targetVisual.physical)).wheel === 1, "script wheel on its background target", 5000);
    assert.equal((await visualState(unrelatedVisual.physical)).wheel, 0);
    await until(() => visualPage(targetVisual, "typeof globalThis.__visualRelease==='function'"), "first held script");
    await captureVisual(targetVisual, "script-running");
    await newSession();
    const secondVisual = await openVisualTab("visual-overlap");
    const secondScript = heldScript(mcp, secondVisual, "throw new Error('visual failure');");
    await until(async () => (await visualState(secondVisual.physical)).wheel === 1, "independent overlapping script wheel", 5000);
    await until(() => visualPage(secondVisual, "typeof globalThis.__visualRelease==='function'"), "second held script");
    assert.equal((await visualState(targetVisual.physical)).wheel, 1);
    await visualPage(targetVisual, "(__visualRelease(),true)");
    assert.equal((await firstScript.promise).structuredContent.status, "succeeded");
    await until(async () => (await visualState(targetVisual.physical)).wheel === 0, "successful script wheel cleanup", 3000);
    await captureVisual(targetVisual, "script-complete");
    assert.equal((await visualState(secondVisual.physical)).wheel, 1);
    assert.equal((await visualState(unrelatedVisual.physical)).wheel, 0);
    check("overlapping script wheels stay on their own tabs and success cleans only its invocation");
    await visualPage(secondVisual, "(__visualRelease(),true)");
    const failedScript = (await secondScript.promise).structuredContent;
    assert.equal(failedScript.status, "unknown"); assert.equal(failedScript.repeat_safe, false);
    await until(async () => (await visualState(secondVisual.physical)).wheel === 0, "failed script wheel cleanup", 3000);
    check("runtime script failure clears the actual work wheel");

    await rawWorker(`chrome.tabs.update(${unrelatedVisual.physical},{active:true})`);
    await visualPage(targetVisual, "(delete globalThis.__visualRelease,true)");
    const cancelledScript = heldScript(firstVisualPeer, targetVisual, "return 'cancelled script completed physically';");
    await until(async () => (await visualState(targetVisual.physical)).wheel === 1, "cancelled script starts its wheel", 5000);
    await until(() => visualPage(targetVisual, "typeof globalThis.__visualRelease==='function'"), "script ready to cancel");
    firstVisualPeer.notify("notifications/cancelled", { requestId: cancelledScript.id, reason: "visual acceptance cancellation" });
    const cancelled = (await cancelledScript.promise).structuredContent;
    assert.equal(cancelled.status, "unknown"); assert.equal(cancelled.repeat_safe, false);
    await until(async () => (await visualState(targetVisual.physical)).wheel === 0, "cancelled script wheel cleanup", 3000);
    assert.equal((await visualState(unrelatedVisual.physical)).wheel, 0);
    await visualPage(targetVisual, "(__visualRelease(),true)");
    check("cancellation clears its work wheel without leaving a spinner on another tab");

    // Hold document discovery after the real targeted start. Its denied child is known before
    // any script dispatch; the hidden-tab notice may wait, but its live wheel must stop now.
    await visualPage(targetVisual, `(()=>{const frame=document.createElement('iframe');frame.src='http://127.0.0.1:${port}/form?visual-denial';document.body.append(frame);return true;})()`);
    await until(async () => (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${targetVisual.physical}})`))
      .some(frame => frame.url.includes("visual-denial")), "denied visual fixture document");
    policy();
    await rawWorker(`(() => {
      globalThis.visualOriginalFrames=chrome.webNavigation.getAllFrames.bind(chrome.webNavigation);
      globalThis.visualInventoryTarget=${targetVisual.physical};
      chrome.webNavigation.getAllFrames=async function(details) {
        if(details.tabId===visualInventoryTarget) await new Promise(resolve=>{globalThis.visualReleaseInventory=resolve;});
        return visualOriginalFrames(details);
      }; return true;
    })()`);
    const beforeDeniedVisual = commands.length;
    const deniedVisual = beginVisual(firstVisualPeer, "browser_execute", {
      tab: targetVisual.handle, script: "document.title='MUST_NOT_EXECUTE_VISUAL_DENIED';"
    });
    await until(async () => (await visualState(targetVisual.physical)).wheel === 1, "script wheel before document admission", 5000);
    assert.equal((await visualState(unrelatedVisual.physical)).wheel, 0);
    await until(() => rawWorker("typeof visualReleaseInventory==='function'"), "held document discovery");
    await rawWorker("(visualInventoryTarget=null,visualReleaseInventory(),true)");
    const deniedResult = (await deniedVisual.promise).structuredContent;
    assert.equal(deniedResult.status, "blocked", JSON.stringify(deniedResult)); assert.equal(deniedResult.effect, "none");
    assert.ok(!dispatchedSince(beforeDeniedVisual).includes("evaluate_script"));
    await until(async () => (await visualState(targetVisual.physical)).wheel === 0, "hidden denied script wheel cleanup", 3000);
    const queuedDenials = await rawWorker("(await chrome.storage.session.get('ghostlight.presentations'))['ghostlight.presentations'] || []");
    assert.ok(queuedDenials.some(item => item.tabId === targetVisual.physical && item.signal.invocation === deniedResult.invocation && item.signal.signal === "denial"));
    assert.equal((await visualState(unrelatedVisual.physical)).wheel, 0);
    check("hidden denied script clears its wheel while preserving its queued human notice");
  } finally {
    await rawWorker("(()=>{chrome.tabs.sendMessage=visualOriginalSendMessage;globalThis.visualReleaseRead?.();if(globalThis.visualOriginalFrames)chrome.webNavigation.getAllFrames=visualOriginalFrames;globalThis.visualReleaseInventory?.();return true;})()");
    for (const request of visualRequests) request.peer.notify("notifications/cancelled", { requestId: request.id, reason: "visual fixture cleanup" });
    await Promise.allSettled(visualRequests.map(request => request.promise));
  }
  const audit = readFileSync(environment.GHOSTLIGHT_AUDIT_FILE, "utf8");
  assert.doesNotMatch(audit, /127\.0\.0\.1|h6@example\.invalid|Ghostlight H6 fixture|ifxf-project|Draft line one|Shadow draft|Replacement draft|action draft|Complete allowlist draft|Ordinary replacement|MUST_NOT_CHANGE_|PARTIAL_DRAFT_EFFECT|Standalone semantic|Standalone handle|standalone-upload\.txt|SYNTHETIC_UPLOAD_/);
  check("durable audit excludes embedded origins, values, and selectors");
  writeFileSync(join(scratchRoot, "h6-browser-evidence.json"), JSON.stringify({ browser: version.product,
    source_kind: liveSylin ? "live_html" : "checked_in_snapshot", source: sourceEvidence, passed,
    transport: "MV3 native-port shim -> real browser connector -> orchestrator -> real MCP connector" }, null, 2));
  console.log(`H6 browser journey: ${passed.length} checks passed with ${version.product}.`);
} finally {
  if (cdp && socket?.readyState === WebSocket.OPEN) { try { await cdp.send("Browser.close"); } catch {} }
  socket?.close(); poll?.end("[]");
  await waitForChromiumExit(chromium);
  for (const child of children.toReversed()) if (child.exitCode === null && child.signalCode === null) child.kill();
  await until(() => children.every((child) => child.exitCode !== null || child.signalCode !== null || child.startError), "owned children exit");
  server.closeAllConnections(); await new Promise((done) => server.close(done));
  await removeBrowserScratch(scratch, scratchRoot, "frame-browser-");
  for (const path of [runtimeFile, runtimeFile.replace(/\.json$/, ".lock")]) { assert.equal(dirname(resolve(path)), binDir); rmSync(path, { force: true }); }
}
