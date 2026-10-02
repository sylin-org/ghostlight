// Owned draft journeys through real CLI/MCP, the authority and the shipped MV3 worker.
// Native-port discovery uses the existing loopback shim; installed/native acceptance is separate.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fixtureRenderingArguments, readDevToolsPort, removeBrowserScratch, waitForChromiumExit } from "./lib/chromium.mjs";

const repository = resolve(import.meta.dirname, "..");
const binDir = resolve(process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0/debug"));
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || join(repository, ".tmp/chrome-testing/chrome-win64/chrome.exe");
const candidate = process.argv.includes("--candidate");
assert.ok(process.argv.slice(2).every(arg => arg === "--candidate"));
const scratchRoot = join(repository, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "quiet-browser-delight-"));
const runtimeFile = join(binDir, `.ghostlight-delight-${process.pid}.json`);
const environment = { ...process.env, GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
  GHOSTLIGHT_AUDIT_FILE: join(scratch, "audit.jsonl"), GHOSTLIGHT_POLICY_FILE: join(scratch, "policy.json"),
  GHOSTLIGHT_DIAGNOSTICS_DIR: join(scratch, "diagnostics"), GHOSTLIGHT_NATIVE_HOST_DIR: join(scratch, "native-host") };
const evidencePath = resolve(process.env.GHOSTLIGHT_DELIGHT_EVIDENCE || join(scratchRoot,
  `delight-${candidate ? "candidate" : "baseline"}-${process.pid}.json`));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const report = { candidate, passed: false, started_at: new Date().toISOString(), binaries: {},
  revision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, windowsHide: true, encoding: "utf8" }).trim(),
  limitations: ["Isolated Chromium/MV3 component journey; native-port discovery is a shim.",
    "No physical keyboard, installed-client rendering, or installed native UX acceptance."],
  journeys: [], receipts: [], commands: [], observations: [], cleanup: {} };
const save = () => writeFileSync(evidencePath, JSON.stringify(report, null, 2) + "\n");
save();
const children = [], queue = [], controls = [], serverState = new Map();
const token = randomUUID();
let poll, relay, nativeReady = false, socket, cdp, chromium, passed = false;
const delay = ms => new Promise(done => setTimeout(done, ms));
async function until(check, label, timeout = 20000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = await check(); if (value) return value; await delay(40); }
  throw new Error(`Timed out: ${label}`);
}
function start(path, args = [], env = environment) {
  const child = spawn(path, args, { env, windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
  child.logs = "";
  child.stderr.on("data", bytes => { child.logs = (child.logs + bytes).slice(-8000); });
  child.on("error", error => { child.startError = error; });
  child.on("close", () => { child.closed = true; });
  children.push(child); return child;
}
const executable = name => join(binDir, name + (process.platform === "win32" ? ".exe" : ""));
function channel(write) {
  let next = 0;
  const pending = new Map();
  return {
    send(method, params = {}, sessionId) {
      const id = ++next;
      const promise = new Promise((done, reject) => {
        const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 40000);
        pending.set(id, { done, reject, timer });
        write({ id, method, params, ...(sessionId ? { sessionId } : {}) });
      });
      return { id, promise };
    },
    receive(message) {
      const item = pending.get(message.id); if (!item) return;
      pending.delete(message.id); clearTimeout(item.timer);
      if (message.error) item.reject(new Error(JSON.stringify(message.error))); else item.done(message.result);
    }
  };
}
function nativeSend(frame) {
  const bytes = Buffer.from(JSON.stringify(frame)), header = Buffer.alloc(4); header.writeUInt32LE(bytes.length);
  relay.stdin.write(Buffer.concat([header, bytes]));
}
function flush() { if (poll && queue.length) { poll.end(JSON.stringify(queue.splice(0))); poll = null; } }
const server = createServer(async (request, response) => {
  response.setHeader("access-control-allow-origin", "*");
  const url = new URL(request.url, "http://localhost");
  if (url.pathname === `/${token}`) {
    if (request.method === "POST") {
      let body = ""; for await (const bytes of request) body += bytes;
      nativeSend(JSON.parse(body)); response.end("ok");
    } else { poll = response; flush(); }
    return;
  }
  const id = url.searchParams.get("id");
  if (url.pathname === "/state") {
    response.setHeader("content-type", "application/json"); response.end(JSON.stringify(serverState.get(id) || { saves: 0 })); return;
  }
  if (url.pathname === "/save") {
    let body = ""; for await (const bytes of request) body += bytes;
    const current = serverState.get(id) || { saves: 0 };
    serverState.set(id, { saves: current.saves + 1, fields: JSON.parse(body) });
    response.end("saved"); return;
  }
  response.setHeader("content-type", "text/html; charset=utf-8");
  const slow = Number(url.searchParams.get("slow") || 0);
  const ambiguous = url.searchParams.has("ambiguous");
  const late = Number(url.searchParams.get("late") || 0);
  const confirmation = url.searchParams.has("unique") ? `Draft saved ${id}` : "Draft saved";
  response.end(`<!doctype html><title>Owned draft fixture</title><h1>Launch note</h1>
    <label>Title<input id="title" aria-label="Title"></label>
    <label>Draft<textarea id="draft" aria-label="Draft"></textarea></label>
    <button id="save" ${late ? "hidden" : ""}>Save draft</button>
    ${ambiguous ? '<button id="other">Save draft</button>' : ""}
    <p id="confirmation">${url.searchParams.has("misleading") ? "Draft saved" : "Unsaved"}</p><p id="details">Details loading</p>
    <script>window.model={title:'',draft:''};window.lateEffects=0;window.clicks=0;
      for(const field of [title,draft])field.addEventListener('input',event=>{
        if(event.isTrusted){model[field.id]=field.value;setTimeout(()=>{field.value=model[field.id]},150)}
      });
      save.onclick=async()=>{clicks++;await new Promise(done=>setTimeout(done,${slow}));
        await fetch('/save?id='+${JSON.stringify(id)},{method:'POST',body:JSON.stringify(model)});
        confirmation.textContent=${JSON.stringify(confirmation)}};
      setTimeout(()=>{save.hidden=false;details.textContent='Details ready'},${late || 1800});
      ${url.searchParams.has("unstable") ? "details.animate([{transform:'translateX(0)'},{transform:'translateX(20px)'}],{duration:60000});" : ""}
    </script>`);
});

try {
  for (const name of ["ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"]) {
    assert.ok(existsSync(executable(name))); report.binaries[name] = { path: executable(name), sha256: hash(readFileSync(executable(name))) };
  }
  report.guardian_art_sha256 = hash(readFileSync(join(repository, "crates/orchestrator/ui/ghostlight.png")));
  writeFileSync(environment.GHOSTLIGHT_POLICY_FILE, JSON.stringify({ schema: 3, name: "Owned delight fixture", version: "1",
    grants: [{ id: "local", hosts: { allow: ["localhost", "127.0.0.1"] }, allowed: ["read", "action", "write", "execute"] }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" },
      { key: "browser.attention", value: "background", level: "mandatory" }] }));
  await new Promise(done => server.listen(0, "127.0.0.1", done));
  const port = server.address().port, origin = `http://localhost:${port}`;
  const authority = start(executable("ghostlight"));
  await until(() => { if (authority.startError) throw authority.startError; assert.equal(authority.exitCode, null, authority.logs); return existsSync(runtimeFile); }, "authority startup");
  relay = start(executable("ghostlight-browser-connector"));
  let buffer = Buffer.alloc(0);
  relay.stdout.on("data", chunk => {
    buffer = Buffer.concat([buffer, chunk]);
    while (buffer.length >= 4) {
      const size = buffer.readUInt32LE(); if (buffer.length < size + 4) break;
      const frame = JSON.parse(buffer.subarray(4, size + 4)); buffer = buffer.subarray(size + 4);
      if (frame.kind === "hello_accepted") nativeReady = true;
      if (frame.kind === "control_state") controls.push(frame.state);
      if (frame.kind === "request") report.commands.push({ at: Date.now(), correlation: frame.request.correlation,
        command: frame.request.command.command, primitive: frame.request.command.primitive?.command ?? null,
        condition: frame.request.command.primitive?.condition ?? frame.request.command.condition ?? null,
        attention: frame.request.attention });
      queue.push(frame); flush();
    }
  });
  const extension = join(scratch, "extension");
  cpSync(resolve(process.env.GHOSTLIGHT_TEST_EXTENSION || join(repository, "extension")), extension, { recursive: true });
  const worker = join(extension, "service-worker.js");
  writeFileSync(worker, `chrome.runtime.connectNative=()=>{
    const listeners=[];let live=true;const endpoint=${JSON.stringify(`http://127.0.0.1:${port}/${token}`)};
    const port={onMessage:{addListener(fn){listeners.push(fn)}},onDisconnect:{addListener(){}},
      postMessage(frame){fetch(endpoint,{method:'POST',body:JSON.stringify(frame)}).catch(()=>{})},disconnect(){live=false}};
    (async()=>{while(live){try{for(const frame of await(await fetch(endpoint)).json())for(const fn of listeners)fn(frame)}
      catch(_){await new Promise(done=>setTimeout(done,100))}}})();return port};\n` + readFileSync(worker, "utf8"));
  chromium = start(browser, ["--remote-debugging-port=0", `--user-data-dir=${join(scratch, "profile")}`,
    ...fixtureRenderingArguments(), "--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE localhost, EXCLUDE 127.0.0.1",
    `--load-extension=${extension}`, "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
    "--disable-component-update", "--disable-sync", "--window-size=1280,900", "about:blank"]);
  const [debugPort, endpoint] = await readDevToolsPort(join(scratch, "profile"), chromium);
  socket = new WebSocket(`ws://127.0.0.1:${debugPort}${endpoint}`);
  await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
  cdp = channel(message => socket.send(JSON.stringify(message)));
  socket.onmessage = ({ data }) => cdp.receive(JSON.parse(data));
  const devtools = (method, params, session) => cdp.send(method, params, session).promise;
  const workerTarget = await until(async () => (await devtools("Target.getTargets")).targetInfos.find(target =>
    target.type === "service_worker" && target.url.includes("service-worker.js")), "MV3 worker");
  const { sessionId: workerSession } = await devtools("Target.attachToTarget", { targetId: workerTarget.targetId, flatten: true });
  const rawWorker = async expression => {
    const result = await devtools("Runtime.evaluate", { expression: `await (${expression})`, returnByValue: true,
      awaitPromise: true, replMode: true }, workerSession);
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails)); return result.result.value;
  };
  await until(() => nativeReady, "connector negotiation");
  await until(() => rawWorker("Boolean(globalThis.ghostlightPageRuntime?.sha256)"), "page runtime");
  const human = await rawWorker(`chrome.tabs.create({url:${JSON.stringify(`${origin}/human?id=human`)},active:true})`);
  await until(async () => (await rawWorker(`chrome.tabs.get(${human.id})`)).status === "complete", "human fixture");
  const humanState = () => rawWorker(`Promise.all([chrome.tabs.get(${human.id}),chrome.windows.get(${human.windowId}),
    chrome.scripting.executeScript({target:{tabId:${human.id}},world:'MAIN',func:()=>({draft:draft.value,focused:document.activeElement.id})})])
    .then(([tab,window,values])=>({id:tab.id,url:tab.url,active:tab.active,window:tab.windowId,window_focused:window.focused,
      owned:topology.workspaceFor(tab.id)||null,...values[0].result}))`);
  await rawWorker(`chrome.scripting.executeScript({target:{tabId:${human.id}},world:'MAIN',func:()=>{draft.value='HUMAN_KEEPS_WORKING';draft.focus()}})`);
  let humanBaseline = await humanState(); report.observations.push({ human_baseline: humanBaseline });
  const humanTarget = await until(async () => (await devtools("Target.getTargets")).targetInfos.find(target =>
    target.url === humanBaseline.url && target.type === "page"), "committed human fixture target");
  const { sessionId: humanSession } = await devtools("Target.attachToTarget", { targetId: humanTarget.targetId, flatten: true });
  const mcpChild = start(executable("ghostlight-mcp-connector"));
  const mcp = channel(message => mcpChild.stdin.write(JSON.stringify({ jsonrpc: "2.0", ...message }) + "\n"));
  createInterface({ input: mcpChild.stdout }).on("line", line => mcp.receive(JSON.parse(line)));
  await mcp.send("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "delight MCP", version: "1" } }).promise;
  mcpChild.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
  const record = (edge, tool, result, extra = {}) => { report.receipts.push({ edge, tool, result, ...extra }); save(); return result; };
  const mcpCall = async (name, args) => record("mcp", name, (await mcp.send("tools/call", { name, arguments: args }).promise).structuredContent);
  const cliCall = async (name, args, session = "draft") => {
    const child = start(executable("ghostlight"), ["call", name, JSON.stringify(args), "--json"],
      { ...environment, GHOSTLIGHT_SESSION: `delight-${process.pid}-${session}` });
    let output = ""; child.stdout.on("data", bytes => { output += bytes; });
    await until(() => child.closed || child.startError, "CLI output and process completion");
    if (child.startError) throw child.startError;
    assert.ok(output.trim(), `CLI returned no receipt (exit ${child.exitCode}): ${child.logs}`);
    return record("cli", name, JSON.parse(output), { exit_code: child.exitCode });
  };
  const onPage = (physical, expression) => rawWorker(`chrome.scripting.executeScript({target:{tabId:${physical}},world:'MAIN',
    func:()=>(${expression})}).then(items=>items[0].result)`);
  const physicalFor = url => rawWorker(`(await chrome.tabs.query({})).find(tab=>tab.url===${JSON.stringify(url)}).id`);
  const expected = { title: "Synthetic launch note", draft: "Keep this draft. Do not publish." };
  const persisted = async id => (await fetch(`http://127.0.0.1:${port}/state?id=${id}`)).json();
  const auditRecords = () => readFileSync(environment.GHOSTLIGHT_AUDIT_FILE, "utf8").trim().split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line));
  const custody = physical => rawWorker(`(()=>{const resources=typeof cancellableOperations==='undefined'?scriptEvaluations:cancellableOperations;return({
    evaluations:[...resources.values()].filter(record=>record.acquisition!==undefined).length,
    observers:[...resources.values()].filter(record=>record.observations!==undefined).length,
    scoped:Boolean(documents.context(${physical})),watchers:navigationWatchers.size,
    active_requests:activity.size,attached_tabs:debuggerLifecycle.attachedCount()})})()`);
  for (const edge of ["mcp", "cli"]) {
    const call = edge === "mcp" ? mcpCall : cliCall;
    for (const slow of [0, 3500]) {
      const id = `${edge}-${slow}`, url = `${origin}/draft?id=${id}&slow=${slow}`;
      const tabRef = { flow_ref: { step: "open", pointer: "/facts/tab" } };
      const steps = [
        { id: "open", tool: "browser_navigate", arguments: { url, new_tab: true, reuse: "never" } },
        { tool: "browser_fill_form", arguments: { tab: tabRef, fields: [
          { selector: { name: "Title", role: "textbox", exact: true }, value: expected.title },
          { selector: { name: "Draft", role: "textbox", exact: true }, value: expected.draft } ] } },
        { tool: "browser_click", arguments: { tab: tabRef, selector: { name: "Save draft", role: "button", exact: true },
          expect: { condition: "text_present", value: "Draft saved" } } }
      ];
      const started = Date.now(), commandStart = report.commands.length;
      const result = await call("browser_flow", { steps, timeout_ms: 15000 });
      const firstAttempt = result.status === "succeeded";
      assert.equal(firstAttempt, slow === 0, JSON.stringify(result));
      const tab = result.facts.steps[0].result.facts.tab;
      let recoveryCalls = 0;
      if (!firstAttempt) {
        assert.equal(result.effect, "partial"); assert.equal(result.repeat_safe, false);
        const recovered = await call("browser_wait", { tab, condition: "text_present", value: "Draft saved" });
        assert.equal(recovered.status, "succeeded"); recoveryCalls++;
      }
      const physical = await physicalFor(url);
      const state = await (await fetch(`http://127.0.0.1:${port}/state?id=${id}`)).json();
      const page = await onPage(physical, "({model,values:{title:title.value,draft:draft.value},confirmation:confirmation.textContent,clicks})");
      assert.equal(state.saves, 1); assert.deepEqual(state.fields, expected); assert.deepEqual(page.values, expected);
      assert.equal(page.confirmation, "Draft saved"); assert.equal(page.clicks, 1);
      assert.deepEqual(await humanState(), humanBaseline);
      report.journeys.push({ edge, pattern: "short_check", slow_ms: slow, first_attempt: firstAttempt, client_calls: 1 + recoveryCalls,
        discovery_calls: 0, replayed_mutations: 0, human_interruptions: 0, elapsed_ms: Date.now() - started,
        independent_server: state, retained_page: page, commands: report.commands.slice(commandStart) }); save();
    }
  }
  // Recommended caller path: bounded discovery, then one fill/save/condition/read flow.
  // Discovery produces exact current handles once. A short click check is not a persistence gate.
  for (const edge of ["mcp", "cli"]) {
    const call = edge === "mcp" ? mcpCall : cliCall;
    for (const scenario of [
      { slow: 0, late: 0 }, { slow: 3500, late: 0 },
      { slow: 3500, late: 2400 }, { slow: 3500, late: 0, misleading: true }
    ]) {
      const id = `intent-${edge}-${scenario.slow}-${scenario.late}-${Boolean(scenario.misleading)}`;
      const url = `${origin}/draft?id=${id}&slow=${scenario.slow}&late=${scenario.late}&unique${scenario.misleading ? "&misleading" : ""}`;
      const tabRef = { flow_ref: { step: "open", pointer: "/facts/tab" } };
      const discoverySteps = [{ id: "open", tool: "browser_navigate", arguments: { url, new_tab: true, reuse: "never" } }];
      if (scenario.late) discoverySteps.push({ tool: "browser_wait", arguments: { tab: tabRef,
        condition: "selector_present", selector: { name: "Save draft", role: "button", exact: true }, visual_settle: false } });
      discoverySteps.push({ tool: "browser_inspect", arguments: { tab: tabRef } });
      const started = Date.now(), commandStart = report.commands.length;
      const discovery = await call("browser_flow", { steps: discoverySteps, timeout_ms: 12000 });
      assert.equal(discovery.status, "succeeded", JSON.stringify(discovery));
      const tab = discovery.facts.steps[0].result.facts.tab;
      const items = discovery.facts.steps.at(-1).result.facts.items;
      const unique = (name, role) => {
        const matches = items.filter(item => item.name === name && item.role === role);
        assert.equal(matches.length, 1, `ambiguous discovery: ${name}`); return matches[0].target;
      };
      const saving = call("browser_flow", { timeout_ms: 15000, steps: [
        { tool: "browser_fill_form", arguments: { tab, fields: [
          { target: unique("Title", "textbox"), value: expected.title },
          { target: unique("Draft", "textbox"), value: expected.draft } ] } },
        { tool: "browser_click", arguments: { tab, target: unique("Save draft", "button") } },
        { tool: "browser_wait", arguments: { tab, condition: "text_present", value: `Draft saved ${id}`, timeout_ms: 10000 } },
        { tool: "browser_read", arguments: { tab } }
      ] });
      let concurrentHumanInput = false;
      if (scenario.slow === 3500 && !scenario.late && !scenario.misleading) {
        await delay(500);
        const before = await humanState(); assert.deepEqual(before, humanBaseline);
        await devtools("Input.insertText", { text: ` HUMAN_CONTINUES_${edge}` }, humanSession);
        const after = await humanState();
        assert.ok(after.draft.includes(`HUMAN_CONTINUES_${edge}`));
        assert.deepEqual({ ...after, draft: before.draft }, before);
        assert.equal(await onPage(human.id, "model.draft"), after.draft, "continuing input was not trusted by the page");
        humanBaseline = after; concurrentHumanInput = true;
      }
      const result = await saving;
      assert.equal(result.status, "succeeded", JSON.stringify(result));
      const physical = await physicalFor(url), state = await persisted(id);
      const page = await onPage(physical, "({model,values:{title:title.value,draft:draft.value},confirmation:confirmation.textContent,clicks})");
      assert.equal(state.saves, 1); assert.deepEqual(state.fields, expected);
      assert.deepEqual(page.values, expected); assert.equal(page.clicks, 1);
      assert.equal(page.confirmation, `Draft saved ${id}`);
      assert.deepEqual(await humanState(), humanBaseline);
      const resources = await custody(physical);
      assert.equal(resources.evaluations, 0); assert.equal(resources.scoped, false);
      report.journeys.push({ edge, pattern: "discover_then_save_wait_read", ...scenario,
        first_attempt: true, client_calls: 2, discovery_calls: 1, replayed_mutations: 0,
        human_interruptions: 0, concurrent_trusted_fixture_input: concurrentHumanInput,
        elapsed_ms: Date.now() - started, independent_server: state,
        retained_page: page, custody: resources, commands: report.commands.slice(commandStart) }); save();
    }
  }
  const misleadingId = "misleading-short-check", misleadingUrl = `${origin}/draft?id=${misleadingId}&slow=3500&unique&misleading`;
  const misleadingOpened = await mcpCall("browser_navigate", { url: misleadingUrl, new_tab: true, reuse: "never" });
  const misleadingTab = misleadingOpened.facts.tab;
  const dispatched = await mcpCall("browser_click", { tab: misleadingTab,
    selector: { name: "Save draft", role: "button", exact: true }, expect: { condition: "text_present", value: "Draft saved" } });
  assert.equal(dispatched.status, "succeeded");
  const beforePersistence = await persisted(misleadingId);
  assert.equal(beforePersistence.saves, 0, "a pre-existing generic marker must not stand in for server persistence");
  assert.equal((await mcpCall("browser_wait", { tab: misleadingTab, condition: "text_present", value: `Draft saved ${misleadingId}` })).status, "succeeded");
  assert.equal((await persisted(misleadingId)).saves, 1);
  report.observations.push({ name: "preexisting_marker_does_not_prove_persistence", short_check: dispatched,
    independent_server_at_short_check: beforePersistence, independent_server_after_fresh_condition: await persisted(misleadingId) }); save();
  const opened = await mcpCall("browser_navigate", { url: `${origin}/challenge?id=challenge&ambiguous`, new_tab: true, reuse: "never" });
  const tab = opened.facts.tab, physical = await physicalFor(`${origin}/challenge?id=challenge&ambiguous`);
  const ambiguous = await mcpCall("browser_click", { tab, selector: { name: "Save draft", role: "button", exact: true } });
  assert.equal(ambiguous.status, "failed"); assert.equal(ambiguous.effect, "none"); assert.equal(ambiguous.facts.selector_matched, 2);
  assert.equal(await onPage(physical, "clicks"), 0);
  if (candidate) {
    assert.ok(!ambiguous.next_steps.join(" ").includes("with role and exact"));
  }
  // Cancel only after the read-only settlement reached the browser, not during a local delay.
  const observationId = "cancel-observation", observationUrl = `${origin}/draft?id=${observationId}&unstable`;
  const observationOpened = await mcpCall("browser_navigate", { url: observationUrl, new_tab: true, reuse: "never" });
  const observationTab = observationOpened.facts.tab, observationPhysical = await physicalFor(observationUrl);
  assert.equal(await onPage(observationPhysical, "document.getAnimations().some(animation=>animation.playState==='running')"), true);
  // Track only the sensor's own resources in this owned page. The finite page animation
  // remains outside these hooks and must keep running after observation cancellation.
  await onPage(observationPhysical, `(() => {
    const original = GhostlightSensor.settleVisual;
    const trace = globalThis.ownedObservationTrace = { started:0, finished:0, timers:new Set(), rafs:new Set(), listeners:new Set() };
    GhostlightSensor.settleVisual = async (target, options) => {
      trace.started++;
      const signal = options.signal;
      const trackedSignal = signal && {
        get aborted() { return signal.aborted; }, get reason() { return signal.reason; },
        addEventListener(type, listener, settings) { trace.listeners.add(listener); signal.addEventListener(type, listener, settings); },
        removeEventListener(type, listener) { trace.listeners.delete(listener); signal.removeEventListener(type, listener); }
      };
      try { return await original(target, { ...options, signal:trackedSignal,
        setTimeout(callback, milliseconds) { const id=setTimeout(() => { trace.timers.delete(id); callback(); },milliseconds); trace.timers.add(id); return id; },
        clearTimeout(id) { trace.timers.delete(id); clearTimeout(id); },
        requestAnimationFrame(callback) { const id=requestAnimationFrame(time => { trace.rafs.delete(id); callback(time); }); trace.rafs.add(id); return id; },
        cancelAnimationFrame(id) { trace.rafs.delete(id); cancelAnimationFrame(id); }
      }); } finally { trace.finished++; }
    };
    return true;
  })()`);
  for (const settlement of [false, true]) {
    const observationCommandStart = report.commands.length;
    const observationRequest = mcp.send("tools/call", { name: "browser_wait", arguments: {
      tab: observationTab, condition: settlement ? "text_present" : "visual_settle",
      ...(settlement ? { value: "Unsaved" } : {}), visual_settle: true, timeout_ms: 20000 } });
    let observationResolved = false;
    observationRequest.promise.then(() => { observationResolved = true; }, () => { observationResolved = true; });
    await until(() => report.commands.slice(observationCommandStart).some(command =>
      command.primitive === "observe" && command.condition === "visual_settle"), "visual observation dispatched");
    await until(async () => (await custody(observationPhysical)).scoped, "visual observation owns document scope");
    await delay(60);
    assert.equal(observationResolved, false, "observation must still be in flight when cancelled");
    const cancelStarted = Date.now();
    mcpChild.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/cancelled",
      params: { requestId: observationRequest.id } }) + "\n");
    const observationCancelled = record("mcp", "browser_wait", (await observationRequest.promise).structuredContent);
    const cancellationReceiptMs = Date.now() - cancelStarted;
    const immediateStarted = Date.now(), immediatelyAfterCancellation = await custody(observationPhysical);
    const immediateRead = await mcpCall("browser_read", { tab: observationTab });
    const immediateReadMs = Date.now() - immediateStarted;
    if (candidate) {
      assert.equal(observationCancelled.status, "cancelled"); assert.equal(observationCancelled.effect, "none");
      assert.equal(observationCancelled.readiness, "unknown"); assert.ok(observationCancelled.summary.includes("observation"));
      if (settlement) assert.equal(observationCancelled.facts.condition_satisfied, true);
      assert.equal(immediateRead.status, "succeeded", JSON.stringify(immediateRead));
      assert.ok(Date.now() - cancelStarted < 1500, "cancellation and immediate next read must not wait for the 20-second observation budget");
    }
    await until(async () => !(await custody(observationPhysical)).scoped, "cancelled observation releases scope", 25000);
    const releaseMs = Date.now() - cancelStarted;
    if (immediateRead.status !== "succeeded") assert.equal((await mcpCall("browser_read", { tab: observationTab })).status, "succeeded");
    assert.equal((await persisted(observationId)).saves, 0); assert.equal(await onPage(observationPhysical, "clicks"), 0);
    const sensorResources = await onPage(observationPhysical, `({ started:ownedObservationTrace.started,
      finished:ownedObservationTrace.finished, timers:ownedObservationTrace.timers.size,
      rafs:ownedObservationTrace.rafs.size, listeners:ownedObservationTrace.listeners.size,
      page_animation_running:document.getAnimations().some(animation=>animation.playState==='running') })`);
    assert.equal(sensorResources.started, sensorResources.finished);
    assert.equal(sensorResources.timers, 0); assert.equal(sensorResources.rafs, 0); assert.equal(sensorResources.listeners, 0);
    assert.equal(sensorResources.page_animation_running, true);
    assert.deepEqual(await humanState(), humanBaseline);
    report.observations.push({ name: settlement ? "cancelled_dispatched_settlement" : "cancelled_dispatched_observation",
      result: observationCancelled, commands: report.commands.slice(observationCommandStart),
      cancellation_receipt_ms: cancellationReceiptMs, immediate_custody: immediatelyAfterCancellation,
      immediate_next_read: immediateRead, immediate_next_read_ms: immediateReadMs, release_elapsed_ms: releaseMs,
      server: await persisted(observationId), clicks: await onPage(observationPhysical, "clicks"),
      custody: await custody(observationPhysical), sensor_resources: sensorResources }); save();
  }
  const waitRequest = mcp.send("tools/call", { name: "browser_wait", arguments: { tab, condition: "duration", value: "4000", visual_settle: false } });
  await delay(350);
  mcpChild.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/cancelled", params: { requestId: waitRequest.id, reason: "owned cancellation" } }) + "\n");
  const cancelled = record("mcp", "browser_wait", (await waitRequest.promise).structuredContent);
  assert.equal(cancelled.status, "cancelled"); assert.equal(cancelled.effect, "none");
  if (candidate) assert.ok(!cancelled.summary.includes("disconnected"));
  assert.equal((await mcpCall("browser_read", { tab })).status, "succeeded");
  // Cancellation before Save keeps the acknowledged fill prefix, without replay or rollback.
  const prefixId = "cancel-prefix", prefixUrl = `${origin}/draft?id=${prefixId}`;
  const prefixOpened = await mcpCall("browser_navigate", { url: prefixUrl, new_tab: true, reuse: "never" });
  const prefixTab = prefixOpened.facts.tab, prefixPhysical = await physicalFor(prefixUrl);
  const prefixAuditStart = auditRecords().length;
  const prefixCommandStart = report.commands.length;
  const prefixRequest = mcp.send("tools/call", { name: "browser_flow", arguments: { steps: [
    { tool: "browser_fill_form", arguments: { tab: prefixTab, fields: [
      { selector: { name: "Draft", role: "textbox", exact: true }, value: "ACKNOWLEDGED_PREFIX" } ] } },
    { tool: "browser_wait", arguments: { tab: prefixTab, condition: "text_present", value: "WAIT_WITH_ACKNOWLEDGED_PREFIX", timeout_ms: 20000, visual_settle: false } },
    { tool: "browser_click", arguments: { tab: prefixTab, selector: { name: "Save draft", role: "button", exact: true } } }
  ] } });
  await until(async () => await onPage(prefixPhysical, "model.draft") === "ACKNOWLEDGED_PREFIX", "acknowledged fill prefix");
  await until(() => auditRecords().slice(prefixAuditStart).some(record => record.tool === "browser_fill_form" && record.effect === "applied"), "authority acknowledged fill prefix");
  await until(() => report.commands.slice(prefixCommandStart).some(command => command.primitive === "observe" && command.condition === "text_present"), "acknowledged prefix then dispatched observation");
  await until(async () => candidate ? (await custody(prefixPhysical)).observers === 1 : (await custody(prefixPhysical)).scoped, "prefix observer owns resources");
  mcpChild.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/cancelled", params: { requestId: prefixRequest.id } }) + "\n");
  const prefix = record("mcp", "browser_flow", (await prefixRequest.promise).structuredContent);
  assert.equal(prefix.status, "cancelled"); assert.equal(prefix.effect, "partial"); assert.equal(prefix.repeat_safe, false);
  assert.equal((await persisted(prefixId)).saves, 0); assert.equal(await onPage(prefixPhysical, "clicks"), 0);
  assert.equal((await mcpCall("browser_read", { tab: prefixTab })).status, "succeeded");
  assert.equal(await onPage(prefixPhysical, "draft.value"), "ACKNOWLEDGED_PREFIX");
  report.observations.push({ name: "cancelled_acknowledged_prefix", result: prefix, server: await persisted(prefixId),
    retained_draft: await onPage(prefixPhysical, "draft.value"), custody: await custody(prefixPhysical) }); save();
  // The existing human Pause blocks the next effect in a flow, and Resume never replays it.
  const pauseId = "pause-prefix", pauseUrl = `${origin}/draft?id=${pauseId}`;
  const pauseOpened = await mcpCall("browser_navigate", { url: pauseUrl, new_tab: true, reuse: "never" });
  const pauseTab = pauseOpened.facts.tab, pausePhysical = await physicalFor(pauseUrl);
  const pauseAuditStart = auditRecords().length;
  const pauseRequest = mcp.send("tools/call", { name: "browser_flow", arguments: { steps: [
    { tool: "browser_fill_form", arguments: { tab: pauseTab, fields: [
      { selector: { name: "Draft", role: "textbox", exact: true }, value: "PAUSE_PRESERVES_DRAFT" } ] } },
    { tool: "browser_wait", arguments: { tab: pauseTab, condition: "duration", value: "1000", visual_settle: false } },
    { tool: "browser_click", arguments: { tab: pauseTab, selector: { name: "Save draft", role: "button", exact: true } } }
  ] } });
  await until(async () => await onPage(pausePhysical, "model.draft") === "PAUSE_PRESERVES_DRAFT", "pause fill prefix");
  await until(() => auditRecords().slice(pauseAuditStart).some(record => record.tool === "browser_fill_form" && record.effect === "applied"), "authority acknowledged prefix before Pause");
  nativeSend({ kind: "event", event: { event: "runtime_control_requested", intent: "hold" } });
  await until(() => controls.at(-1) === "held", "Pause reaches isolated adapter");
  const paused = record("mcp", "browser_flow", (await pauseRequest.promise).structuredContent);
  assert.notEqual(paused.status, "succeeded"); assert.equal(paused.effect, "partial"); assert.equal(paused.repeat_safe, false);
  assert.equal((await persisted(pauseId)).saves, 0); assert.equal(await onPage(pausePhysical, "clicks"), 0);
  nativeSend({ kind: "event", event: { event: "runtime_control_requested", intent: "resume" } });
  await until(() => controls.at(-1) === "active", "Resume reaches isolated adapter");
  assert.equal((await mcpCall("browser_read", { tab: pauseTab })).status, "succeeded");
  assert.equal(await onPage(pausePhysical, "draft.value"), "PAUSE_PRESERVES_DRAFT");
  assert.equal(await onPage(pausePhysical, "clicks"), 0);
  report.observations.push({ name: "pause_before_next_effect", result: paused, server: await persisted(pauseId),
    retained_draft: await onPage(pausePhysical, "draft.value"), custody: await custody(pausePhysical) }); save();
  const deadline = await mcpCall("browser_execute", { tab, script: "lateEffects++; await new Promise(done=>setTimeout(done,2000)); lateEffects++;", timeout_ms: 500 });
  assert.equal(deadline.effect, "unknown"); assert.equal(deadline.repeat_safe, false);
  assert.equal((await mcpCall("browser_read", { tab })).status, "succeeded");
  await until(async () => await onPage(physical, "lateEffects") === 2, "late page continuation");
  const uncertainRequest = mcp.send("tools/call", { name: "browser_execute", arguments: { tab,
    script: "lateEffects=0;lateEffects++;await new Promise(done=>setTimeout(done,2000));lateEffects++;" } });
  await until(async () => await onPage(physical, "lateEffects") === 1, "uncertain mutation dispatched");
  mcpChild.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/cancelled", params: { requestId: uncertainRequest.id } }) + "\n");
  const uncertain = record("mcp", "browser_execute", (await uncertainRequest.promise).structuredContent);
  assert.equal(uncertain.effect, "unknown"); assert.equal(uncertain.repeat_safe, false);
  assert.equal((await mcpCall("browser_read", { tab })).status, "succeeded");
  const cleaned = await custody(physical);
  assert.equal(cleaned.evaluations, 0); assert.equal(cleaned.scoped, false); assert.equal(cleaned.watchers, 0);
  await until(async () => await onPage(physical, "lateEffects") === 2, "page continuation after cancellation");
  assert.deepEqual(await humanState(), humanBaseline);
  report.observations.push({ name: "cancelled_uncertain_mutation", result: uncertain, custody: cleaned,
    late_effects: await onPage(physical, "lateEffects"), stopped_background_js_claimed: false }); save();
  const preserved = await mcpCall("browser_tabs", { tab, action: "close" });
  assert.equal(preserved.status, "blocked"); assert.equal(preserved.effect, "none");
  assert.equal((await rawWorker(`chrome.tabs.get(${physical})`)).id, physical);
  assert.deepEqual(await humanState(), humanBaseline);
  report.observations.push({ ambiguity_zero_clicks: true, cancellation: cancelled,
    continuation_after_custody_release: await onPage(physical, "lateEffects"), preserved });
  report.audit = auditRecords();
  save(); passed = true;
} catch (error) {
  report.failure = String(error.stack || error); save(); throw error;
} finally {
  try {
    if (cdp && socket?.readyState === WebSocket.OPEN) { try { await cdp.send("Browser.close").promise; } catch {} }
    socket?.close(); poll?.end("[]");
    if (chromium) await waitForChromiumExit(chromium);
    for (const child of children.toReversed()) if (child.exitCode === null && child.signalCode === null) child.kill();
    await until(() => children.every(child => child.exitCode !== null || child.signalCode !== null || child.startError), "owned children exit");
    server.closeAllConnections(); await new Promise(done => server.close(done));
    await removeBrowserScratch(scratch, scratchRoot, "quiet-browser-");
    for (const path of [runtimeFile, runtimeFile.replace(/\.json$/, ".lock")]) {
      assert.equal(dirname(resolve(path)), binDir); rmSync(path, { force: true });
    }
    report.cleanup = { spawned_processes_exited: children.length, profile_removed: !existsSync(scratch),
      isolated_runtime_removed: !existsSync(runtimeFile), installed_runtime_replaced: false, persistent_settings_changed: false };
    report.passed = passed;
  } catch (error) { report.cleanup_failure = String(error.stack || error); report.passed = false; throw error; }
  finally { report.finished_at = new Date().toISOString(); save(); console.log(`Evidence: ${evidencePath}`); }
}
