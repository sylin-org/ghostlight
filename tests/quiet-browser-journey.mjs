// Quiet coexistence through the shipped MV3 worker, real Chromium APIs, both intake edges,
// and the service executor. Only native-port discovery uses the existing loopback test seam.
// This is isolated component evidence, not the installed browser or physical keyboard journey.
// Actual old custody opt-in: GHOSTLIGHT_TEST_QUIET_COMPATIBILITY=1 enables --legacy-custody
// in the sequential hardening suite; the checkout must contain the pinned baseline Git object.
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
const legacy = process.argv.includes("--legacy");
const legacyCustody = process.argv.includes("--legacy-custody");
const historicalRevision = "de1a686761af5430afc50763d1a282efaa80f615";
assert.ok(process.argv.length <= 3 && process.argv.slice(2).every(arg => ["--legacy", "--legacy-custody"].includes(arg)),
  "Usage: node tests/quiet-browser-journey.mjs [--legacy|--legacy-custody]");
assert.ok(existsSync(browser), "Set GHOSTLIGHT_TEST_BROWSER to Chrome for Testing with unpacked extensions.");
const scratchRoot = join(repository, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "quiet-browser-"));
const runtimeFile = join(binDir, `.ghostlight-quiet-runtime-${process.pid}.json`);
const environment = { ...process.env, GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
  GHOSTLIGHT_AUDIT_FILE: join(scratch, "audit.jsonl"), GHOSTLIGHT_POLICY_FILE: join(scratch, "policy.json"),
  GHOSTLIGHT_DIAGNOSTICS_DIR: join(scratch, "diagnostics"), GHOSTLIGHT_NATIVE_HOST_DIR: join(scratch, "native-host") };
const evidencePath = resolve(process.env.GHOSTLIGHT_QUIET_EVIDENCE || join(scratchRoot,
  `quiet-browser-${legacy ? "legacy-" : legacyCustody ? "legacy-custody-" : ""}${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}.json`));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const desktopForeground = () => process.platform === "win32" ? JSON.parse(execFileSync("pwsh", ["-NoProfile", "-NonInteractive", "-Command",
  `Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class QuietForeground { [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p); }'; $window=[QuietForeground]::GetForegroundWindow(); $owner=0; [QuietForeground]::GetWindowThreadProcessId($window,[ref]$owner)|Out-Null; $process=Get-CimInstance Win32_Process -Filter ('ProcessId = '+$owner); @{window=$window.ToInt64();process=$owner;name=$process.Name;path=$process.ExecutablePath}|ConvertTo-Json -Compress`],
  { windowsHide: true, encoding: "utf8" })) : null;
const report = { started_at: new Date().toISOString(), passed: false, legacy_adapter: legacy || legacyCustody,
  transport: "isolated MV3 native-port shim -> browser connector -> service -> MCP and CLI",
  limitations: ["Not installed-stack acceptance", "Typing uses trusted Chromium input, not a physical keyboard",
    "Chrome retains document.hasFocus after native input even without emulation; cleanup uses an inactive never-native sentinel and requires every controlled debugger to detach on Stop"],
  browser_fixture_arguments: fixtureRenderingArguments(),
  revision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, windowsHide: true, encoding: "utf8" }).trim(),
  binaries: {}, checks: [], receipts: [], observations: [], dispatched_commands: [], native_trace: [], native_attention_receipts: [], failure: "run_incomplete" };
const save = () => writeFileSync(evidencePath, JSON.stringify(report, null, 2) + "\n");
save();
const children = [];
const delay = ms => new Promise(done => setTimeout(done, ms));
async function until(check, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = await check(); if (value) return value; await delay(50); }
  throw new Error(`Timed out: ${label}`);
}
function start(path, args = []) {
  const child = spawn(path, args, { env: environment, windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
  child.stderr.on("data", bytes => { child.logs = ((child.logs || "") + bytes).slice(-6000); });
  child.on("error", error => { child.startError = error; });
  children.push(child); return child;
}
const executable = name => join(binDir, name + (process.platform === "win32" ? ".exe" : ""));
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
      const item = pending.get(message.id); if (!item) return;
      pending.delete(message.id); clearTimeout(item.timer);
      if (message.error) item.reject(new Error(JSON.stringify(message.error))); else item.done(message.result);
    }
  };
}
const token = randomUUID();
const queue = [], requests = [], controls = [], nativeErrors = [];
let poll, nativeReady = false, relay;
function nativeSend(frame) {
  const bytes = Buffer.from(JSON.stringify(frame)), header = Buffer.alloc(4); header.writeUInt32LE(bytes.length);
  relay.stdin.write(Buffer.concat([header, bytes]));
}
function flush() { if (poll && queue.length) { poll.end(JSON.stringify(queue.splice(0))); poll = null; } }
const server = createServer(async (request, response) => {
  response.setHeader("access-control-allow-origin", "*");
  if (request.url === `/${token}`) {
    if (request.method === "POST") {
      let body = ""; for await (const bytes of request) body += bytes;
      const frame = JSON.parse(body);
      if (frame.kind === "receipt" && (frame.receipt.result.outcome === "attention_protected"
        || frame.receipt.result.result?.outcome === "attention_protected")) {
        report.native_attention_receipts.push(frame.receipt); save();
      }
      nativeSend(frame); response.end("ok");
    } else { poll = response; flush(); }
    return;
  }
  response.setHeader("content-type", "text/html; charset=utf-8");
  response.end(`<!doctype html><title>Quiet coexistence fixture</title>
    <h1>Quiet coexistence fixture</h1><label>Human draft<textarea id="human" aria-label="Human draft"></textarea></label>
    <label>Agent draft<input id="agent" aria-label="Agent draft"></label>
    <button id="effect" onclick="window.effects++">Count effect</button>
    <button id="race" onclick="window.raceEffects++">Native race effect</button>
    <script>window.effects=0;window.raceEffects=0;window.nativeKeys=[];window.nativeClicks=[];window.nativeTextInputs=[];window.textModel='';window.inputs=[];
      human.addEventListener('input',event=>inputs.push(event.isTrusted));
      agent.addEventListener('keydown',event=>nativeKeys.push({type:event.type,key:event.key,trusted:event.isTrusted}));
      agent.addEventListener('keyup',event=>nativeKeys.push({type:event.type,key:event.key,trusted:event.isTrusted}));
      agent.addEventListener('input',event=>{textModel=agent.value;nativeTextInputs.push({trusted:event.isTrusted,value:agent.value})});
      race.addEventListener('click',event=>nativeClicks.push({trusted:event.isTrusted}));</script>`);
});
let socket, cdp, chromium, observeFailure, journeyPassed = false, createdDeployLock = false;
const deployLock = join(binDir, "deploy.lock"), deployMarker = `quiet custody fixture ${randomUUID()}`;
const check = name => { report.checks.push(name); save(); console.log(`PASS quiet: ${name}`); };
function receipt(edge, tool, result) {
  report.receipts.push({ edge, tool, invocation: result.invocation, status: result.status, effect: result.effect,
    reason: result.facts?.reason ?? null }); save(); return result;
}
try {
  for (const name of ["ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"]) {
    assert.ok(existsSync(executable(name)), `Build ${name} into GHOSTLIGHT_BIN_DIR first.`);
    report.binaries[name] = { path: executable(name), sha256: hash(readFileSync(executable(name))) };
  }
  const paths = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "extension"],
    { cwd: repository, windowsHide: true, encoding: "utf8" }).split("\0").filter(Boolean).sort();
  report.adapter_source_sha256 = hash(paths.map(path => `${path}\0${hash(readFileSync(join(repository, path)))}\n`).join(""));
  const sourcePaths = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
    "crates", "extension", "tests", "Cargo.toml", "Cargo.lock"], { cwd: repository, windowsHide: true, encoding: "utf8" })
    .split("\0").filter(Boolean).sort();
  report.source_sha256 = hash(sourcePaths.map(path => `${path}\0${existsSync(join(repository, path))
    ? hash(readFileSync(join(repository, path))) : "deleted"}\n`).join(""));
  writeFileSync(environment.GHOSTLIGHT_POLICY_FILE, JSON.stringify({ schema: 3, name: "Quiet local fixture", version: "1",
    grants: [{ id: "local", hosts: { allow: ["localhost", "127.0.0.1"] }, allowed: ["read", "action", "write", "execute"] }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" },
      { key: "browser.attention", value: legacyCustody ? "foreground" : "background", level: "mandatory" }] }));
  await new Promise(done => server.listen(0, "127.0.0.1", done));
  const port = server.address().port, origin = `http://localhost:${port}`;
  let authority = start(executable("ghostlight"));
  await until(() => {
    if (authority.startError) throw authority.startError;
    assert.equal(authority.exitCode, null, authority.logs); return existsSync(runtimeFile);
  }, "isolated authority startup");
  relay = start(executable("ghostlight-browser-connector"));
  let buffer = Buffer.alloc(0);
  relay.stdout.on("data", chunk => {
    buffer = Buffer.concat([buffer, chunk]);
    while (buffer.length >= 4) {
      const size = buffer.readUInt32LE(); if (buffer.length < size + 4) break;
      const frame = JSON.parse(buffer.subarray(4, size + 4)); buffer = buffer.subarray(size + 4);
      if (frame.kind === "hello_accepted") nativeReady = true;
      if (["hello_accepted", "control_state", "error", "backend_unavailable"].includes(frame.kind)) {
        report.native_trace.push({ kind: frame.kind, ...(frame.state ? { state: frame.state } : {}),
          ...(frame.control_state ? { control_state: frame.control_state } : {}),
          ...(frame.code ? { code: frame.code, effect_unknown: frame.effect_unknown } : {}) });
        if (report.native_trace.length > 64) report.native_trace.shift();
      }
      if (frame.kind === "error") nativeErrors.push(frame);
      if (frame.kind === "request") {
        requests.push(frame.request);
        report.dispatched_commands.push({ command: frame.request.command.command, attention: frame.request.attention,
          primitive: frame.request.command.primitive?.command ?? null });
      }
      if (frame.kind === "control_state") controls.push(frame.state);
      queue.push(frame); flush();
    }
  });
  const extension = join(scratch, "extension");
  if (legacyCustody) {
    // Historical adapter code is an immutable compatibility fixture, never implementation input.
    const historicalPaths = execFileSync("git", ["ls-tree", "-r", "--name-only", historicalRevision, "--", "extension"],
      { cwd: repository, windowsHide: true, encoding: "utf8" }).trim().split(/\r?\n/).filter(Boolean).sort();
    assert.ok(historicalPaths.includes("extension/service-worker.js"));
    const fingerprints = [];
    for (const path of historicalPaths) {
      assert.ok(path.startsWith("extension/")); const destination = join(scratch, path);
      const bytes = execFileSync("git", ["show", `${historicalRevision}:${path}`], { cwd: repository, windowsHide: true, maxBuffer: 16 * 1024 * 1024 });
      mkdirSync(dirname(destination), { recursive: true }); writeFileSync(destination, bytes); fingerprints.push(`${path}\0${hash(bytes)}\n`);
    }
    report.historical_adapter = { revision: historicalRevision, source_sha256: hash(fingerprints.join("")), files: historicalPaths.length };
    report.current_adapter_source_sha256 = report.adapter_source_sha256;
    report.adapter_source_sha256 = report.historical_adapter.source_sha256;
  } else cpSync(join(repository, "extension"), extension, { recursive: true });
  const worker = join(extension, "service-worker.js");
  const shim = `chrome.runtime.connectNative=()=>{
    const listeners=[];let live=true;const endpoint=${JSON.stringify(`http://127.0.0.1:${port}/${token}`)};
    const port={onMessage:{addListener(fn){listeners.push(fn)}},onDisconnect:{addListener(){}},
      postMessage(frame){if(${legacy}&&frame.kind==='hello')frame.capabilities=frame.capabilities.filter(item=>item.name!=='browser_attention');
        fetch(endpoint,{method:'POST',body:JSON.stringify(frame)}).catch(()=>{})},disconnect(){live=false}};
    (async()=>{while(live){try{for(const frame of await(await fetch(endpoint)).json())for(const fn of listeners)fn(frame)}
      catch(_){await new Promise(done=>setTimeout(done,100))}}})();return port};\n`;
  writeFileSync(worker, shim + readFileSync(worker, "utf8"));
  report.loaded_adapter_sha256 = hash(readFileSync(worker)); save();
  const profile = join(scratch, "profile");
  chromium = start(browser, ["--remote-debugging-port=0", `--user-data-dir=${profile}`,
    ...fixtureRenderingArguments(),
    ...(process.env.GHOSTLIGHT_TEST_NO_SANDBOX === "1" ? ["--no-sandbox"] : []),
    "--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE localhost, EXCLUDE 127.0.0.1",
    `--load-extension=${extension}`, "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
    "--disable-component-update", "--disable-sync", "--window-size=1280,900", "about:blank"]);
  const [debugPort, endpoint] = await readDevToolsPort(profile, chromium);
  socket = new WebSocket(`ws://127.0.0.1:${debugPort}${endpoint}`);
  await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
  cdp = channel(message => socket.send(JSON.stringify(message)));
  socket.onmessage = ({ data }) => cdp.receive(JSON.parse(data));
  report.browser = (await cdp.send("Browser.getVersion")).product;
  const workerTarget = await until(async () => (await cdp.send("Target.getTargets")).targetInfos
    .find(target => target.type === "service_worker" && target.url.includes("service-worker.js")), "MV3 worker");
  const { sessionId: workerSession } = await cdp.send("Target.attachToTarget", { targetId: workerTarget.targetId, flatten: true });
  const rawWorker = async expression => {
    const result = await cdp.send("Runtime.evaluate", { expression: `await (${expression})`, returnByValue: true,
      awaitPromise: true, replMode: true }, workerSession);
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails)); return result.result.value;
  };
  observeFailure = () => rawWorker(`Promise.all([chrome.windows.getAll(),chrome.tabs.query({})]).then(([windows,tabs])=>({windows:windows.map(({id,focused,state})=>({id,focused,state})),tabs:tabs.map(({id,windowId,active})=>({id,windowId,active})),activations:globalThis.quietActivations||[],focuses:globalThis.quietWindowFocuses||[]}))`);
  if (legacy) {
    await until(() => controls.includes("ended") && nativeErrors.some(frame => frame.code === "browser_attention_upgrade_required"), "old adapter receives Ended and explicit upgrade error");
    await until(() => rawWorker("liveState.control_state==='ended' && Boolean(liveState.last_error?.includes('Update Ghostlight'))"), "legacy worker applies retirement state and displays upgrade detail");
    assert.equal(nativeReady, false, "old background adapter must never be accepted Active");
    assert.equal(await rawWorker("Boolean(globalThis.ghostlightPageRuntime?.sha256)"), false, "retired adapter must receive no runtime installation");
    report.observations.push({ name: "legacy_retired", state: await rawWorker("({control:liveState.control_state,error:liveState.last_error})") }); save();
  } else {
    await until(() => nativeReady, "real connector negotiation");
    await until(() => rawWorker("Boolean(globalThis.ghostlightPageRuntime?.sha256)"), "acknowledged page runtime installed");
    report.page_runtime = await rawWorker("globalThis.ghostlightPageRuntime"); save();
  }
  const attachMcp = async (name = "quiet-browser-journey") => {
    const child = start(executable("ghostlight-mcp-connector"));
    const peer = channel(message => child.stdin.write(JSON.stringify({ jsonrpc: "2.0", ...message }) + "\n"));
    createInterface({ input: child.stdout }).on("line", line => peer.receive(JSON.parse(line)));
    await peer.send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name, version: "1" } });
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n"); return { child, peer };
  };
  const { child: mcpConnector, peer: mcp } = await attachMcp();
  const call = async (name, args) => receipt("mcp", name, (await mcp.send("tools/call", { name, arguments: args })).structuredContent);
  const cli = async (name, args) => {
    const child = start(executable("ghostlight"), ["call", name, JSON.stringify(args), "--json"]);
    let stdout = ""; child.stdout.on("data", bytes => { stdout += bytes; });
    await until(() => child.exitCode !== null || child.startError, "CLI completion");
    if (child.startError) throw child.startError;
    return { code: child.exitCode, result: receipt("cli", name, JSON.parse(stdout)) };
  };
  if (legacyCustody) {
    const human = await rawWorker(`chrome.tabs.create({url:${JSON.stringify(`${origin}/human`)},active:true})`);
    await until(async () => (await rawWorker(`chrome.tabs.get(${human.id})`)).status === "complete", "historical human fixture");
    // Distinct hosts avoid exercising historical same-host adoption in the custody cleanup proof.
    const agentUrl = `http://127.0.0.1:${port}/historical-agent`;
    const opened = await call("browser_navigate", { url: agentUrl, new_tab: true });
    assert.equal(opened.status, "succeeded", JSON.stringify(opened)); assert.equal(opened.facts.browser_attention.value, "foreground");
    const tab = opened.facts.tab;
    const physical = await rawWorker(`(await chrome.tabs.query({})).find(item=>item.url===${JSON.stringify(agentUrl)}).id`);
    assert.notEqual(physical, human.id);
    // An inactive positive control distinguishes retained emulation from a normal active
    // document's focus after startup. The sibling is synthetic, unowned human work.
    const historicalWindow = (await rawWorker(`chrome.tabs.get(${physical})`)).windowId;
    const historicalSibling = await rawWorker(`chrome.tabs.create({url:${JSON.stringify(`${origin}/historical-human-sibling`)},windowId:${historicalWindow},active:true})`);
    await rawWorker(`chrome.tabs.update(${human.id},{active:true})`); await rawWorker(`chrome.windows.update(${human.windowId},{focused:true})`);
    const observed = () => rawWorker(`Promise.all([chrome.tabs.get(${physical}),chrome.debugger.getTargets(),chrome.scripting.executeScript({target:{tabId:${physical}},world:'MAIN',func:()=>({focus:document.hasFocus(),visibility:document.visibilityState,value:agent.value})}),chrome.windows.get((await chrome.tabs.get(${physical})).windowId),chrome.tabs.get(${human.id})]).then(async([tab,targets,items,window,human])=>({id:tab.id,url:tab.url,active:tab.active,window:tab.windowId,window_focused:window.focused,human:{active:human.active,window:human.windowId,url:human.url,window_focused:(await chrome.windows.get(human.windowId)).focused},attached:Boolean(targets.find(target=>target.tabId===tab.id)?.attached),...items[0].result}))`);
    await until(async () => { const state = await observed(); return state.attached && state.focus && !state.active && !state.window_focused; }, "historical inactive adapter tab retained debugger and focus emulation in an unattended window");
    const historicalCustody = await observed();
    report.observations.push({ name: "historical_foreground_custody", ...historicalCustody }); save();
    const beforeRetirement = requests.length;
    const policy = JSON.parse(readFileSync(environment.GHOSTLIGHT_POLICY_FILE, "utf8"));
    policy.version = "2"; policy.config.find(item => item.key === "browser.attention").value = "background";
    writeFileSync(environment.GHOSTLIGHT_POLICY_FILE, JSON.stringify(policy));
    mcpConnector.stdin.end(); await until(() => mcpConnector.exitCode !== null, "last historical MCP edge closes");
    await until(() => controls.includes("ended") && nativeErrors.some(frame=>frame.code === "browser_attention_upgrade_required"), "tightened rule retires historical custody");
    await until(async () => { const state = await observed(); return !state.attached && !state.focus; }, "actual historical Ended handler detaches debugger and removes emulation");
    const retired = await observed(); assert.equal(retired.id, physical); assert.equal(retired.url, agentUrl);
    assert.equal(retired.active, historicalCustody.active); assert.equal(retired.window, historicalCustody.window); assert.equal(retired.window_focused, false);
    assert.equal((await rawWorker(`chrome.tabs.get(${human.id})`)).url, `${origin}/human`);
    assert.equal((await rawWorker(`chrome.tabs.get(${historicalSibling.id})`)).url, `${origin}/historical-human-sibling`);
    assert.ok(!requests.slice(beforeRetirement).some(request=>["close_tab", "close_tabs"].includes(request.command.command)));
    report.observations.push({ name: "historical_retired_custody", ...retired }); save();
    check("actual historical worker loses controlled debugger and focus under Ended retirement while both physical tabs stay open");
    const newEdge = await attachMcp();
    const refused = receipt("mcp-reconnect", "browser_navigate", (await newEdge.peer.send("tools/call", { name: "browser_navigate", arguments: { url: agentUrl, new_tab: true } })).structuredContent);
    assert.equal(refused.effect, "none"); assert.equal(refused.facts.reason, "browser_contract_failed"); assert.equal(refused.facts.capability, "browser_attention");
    assert.equal((await observed()).attached, false); assert.ok(!controls.slice(controls.indexOf("ended")).includes("active"));
    newEdge.child.stdin.end(); await until(() => newEdge.child.exitCode !== null, "reconnected edge closes");
    // The existing deploy marker suppresses relay demand-start while this owned service restarts.
    assert.equal(existsSync(deployLock), false, "never replace another fixture's deployment marker");
    writeFileSync(deployLock, deployMarker, { flag: "wx" }); createdDeployLock = true;
    const errorsBeforeRestart = nativeErrors.length;
    authority.kill(); await until(() => authority.exitCode !== null || authority.signalCode !== null, "historical fixture authority exits");
    rmSync(runtimeFile, { force: true }); authority = start(executable("ghostlight"));
    await until(() => existsSync(runtimeFile), "fresh authority runtime after restart");
    await until(() => nativeErrors.length > errorsBeforeRestart && nativeErrors.at(-1).code === "browser_attention_upgrade_required", "fresh authority retires replayed historical Hello");
    report.observations.push({ name: "historical_actual_after_service_restart", ...(await observed()) }); save();
    assert.equal((await observed()).attached, false); assert.equal((await observed()).focus, false);
    assert.ok(!controls.slice(controls.indexOf("ended")).includes("active"));
    assert.equal(requests.length, beforeRetirement, "reconnect and restart must send no new browser primitive to quarantined historical adapter");
    report.observations.push({ name: "historical_custody_after_service_restart", ...(await observed()) }); save();
    check("historical reconnect and service restart remain quarantined without Active, reattachment, operation dispatch or physical close");
  } else if (legacy) {
    assert.equal(requests.length, 0, "retired legacy handshake must dispatch no browser operation");
    for (const edge of ["mcp", "cli"]) {
      const args = { url: `${origin}/legacy`, new_tab: true };
      const result = edge === "mcp" ? await call("browser_navigate", args) : (await cli("browser_navigate", args)).result;
      assert.notEqual(result.status, "succeeded", JSON.stringify(result));
      assert.equal(result.effect, "none"); assert.equal(result.facts.reason, "browser_contract_failed", JSON.stringify(result));
      assert.equal(result.facts.capability, "browser_attention");
      assert.equal(result.facts.required_revision, 1); assert.equal(result.facts.advertised_revision, 0);
    }
    assert.equal(requests.length, 0, "retired old adapter must receive no browser work under the background rule");
    assert.equal(nativeReady, false); assert.ok(controls.every(state => state === "ended"));
    check("legacy adapter receives Ended plus upgrade error before Active or runtime installation; both edges refuse with no operation dispatch");
  } else {
    // This tab is deliberately unowned and has the same host as the requested agent tab.
    const human = await rawWorker(`chrome.tabs.create({url:${JSON.stringify(`${origin}/human`)},active:true})`);
    await until(async () => (await rawWorker(`chrome.tabs.get(${human.id})`)).status === "complete", "human fixture load");
    const groupId = await rawWorker(`chrome.tabs.group({tabIds:[${human.id}]})`);
    await rawWorker(`chrome.tabGroups.update(${groupId},{title:'Human work',color:'green',collapsed:false})`);
    await rawWorker(`chrome.windows.update(${human.windowId},{focused:true})`);
    const pageTarget = await until(async () => (await cdp.send("Target.getTargets")).targetInfos
      .find(target => target.type === "page" && target.url === `${origin}/human`), "human page target");
    const { sessionId: humanSession } = await cdp.send("Target.attachToTarget", { targetId: pageTarget.targetId, flatten: true });
    const humanPage = async expression => {
      const result = await cdp.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, humanSession);
      assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails)); return result.result.value;
    };
    await humanPage("human.focus(); true");
    const state = () => rawWorker(`Promise.all([chrome.tabs.get(${human.id}),chrome.windows.getAll(),chrome.tabGroups.get(${groupId})])
      .then(([tab,windows,group])=>({tab:{id:tab.id,url:tab.url,active:tab.active,window:tab.windowId,group:tab.groupId},
        focused:windows.filter(window=>window.focused).map(window=>window.id),group:{id:group.id,title:group.title,collapsed:group.collapsed}}))`);
    const baseline = await state();
    report.desktop_foreground_before = desktopForeground();
    report.observations.push({ name: "human_before", ...baseline }); save();
    await rawWorker(`(()=>{globalThis.quietActivations=[];globalThis.quietWindowFocuses=[];
      chrome.tabs.onActivated.addListener(info=>quietActivations.push(info));
      chrome.windows.onFocusChanged.addListener(id=>quietWindowFocuses.push(id));return true})()`);
    const fragments = ["Human ", "keeps ", "typing ", "during ", "agent ", "work."];
    const typing = (async () => {
      for (const text of fragments) { await cdp.send("Input.insertText", { text }, humanSession); await delay(100); }
    })();
    const opened = await call("browser_navigate", { url: `${origin}/agent` });
    assert.equal(opened.status, "succeeded", JSON.stringify(opened));
    assert.equal(opened.facts.browser_attention.value, "background", JSON.stringify(opened));
    const tab = opened.facts.tab;
    const physical = await rawWorker(`(await chrome.tabs.query({})).find(tab=>tab.url===${JSON.stringify(`${origin}/agent`)}).id`);
    assert.notEqual(physical, human.id, "same-host open must not navigate or adopt the human tab");
    const repeated = await call("browser_navigate", { url: `${origin}/agent-next` });
    assert.equal(repeated.status, "succeeded", JSON.stringify(repeated)); assert.equal(repeated.facts.tab, tab);
    const filled = await call("browser_fill_form", { tab, fields: [{ selector: { name: "Agent draft", role: "textbox", exact: true }, value: "Agent draft retained" }] });
    if (filled.status !== "succeeded") {
      report.observations.push({ name: "failed_fill_actual_state",
        agent: await rawWorker(`chrome.scripting.executeScript({target:{tabId:${physical}},world:'MAIN',func:()=>({
          value:agent.value,active:document.activeElement.id,focus:document.hasFocus(),visibility:document.visibilityState})}).then(items=>items[0].result)`),
        human: await humanPage("({value:human.value,active:document.activeElement.id,focus:document.hasFocus(),visibility:document.visibilityState})") }); save();
    }
    assert.equal(filled.status, "succeeded", JSON.stringify(filled));
    await typing;
    const typed = await humanPage("({value:human.value,focused:document.activeElement.id,trusted:inputs.every(Boolean),events:inputs.length})");
    assert.equal(typed.value, fragments.join("")); assert.equal(typed.focused, "human"); assert.equal(typed.trusted, true); assert.ok(typed.events >= fragments.length);
    assert.deepEqual(await state(), baseline);
    assert.deepEqual(await rawWorker(`quietActivations.filter(info=>info.windowId===${human.windowId})`), [],
      "background work must never transiently select another tab in the human window");
    assert.deepEqual(await rawWorker("quietWindowFocuses"), [], "background work must never transiently foreground a window");
    report.observations.push({ name: "human_after", ...(await state()), typing_retained: true, trusted_input: true });
    const screenshot = await cdp.send("Page.captureScreenshot", { format: "png" }, humanSession);
    const screenshotPath = evidencePath.replace(/\.json$/, "-human.png");
    const screenshotBytes = Buffer.from(screenshot.data, "base64"); writeFileSync(screenshotPath, screenshotBytes);
    report.artifacts = [{ path: screenshotPath, sha256: hash(screenshotBytes), content: "synthetic human typing fixture" }]; save();
    check("MCP open, owned reuse, navigate and fill retain the unowned human page, placement, selection and trusted typing");
    const cliOpened = await cli("browser_navigate", { url: `${origin}/cli`, new_tab: true });
    assert.equal(cliOpened.code, 0); assert.equal(cliOpened.result.status, "succeeded");
    assert.equal(cliOpened.result.facts.browser_attention.value, "background", JSON.stringify(cliOpened.result));
    assert.deepEqual(await state(), baseline);
    check("CLI uses the same background rule without stealing the human tab or focus");
    const beforeFocus = requests.length;
    const focused = await call("browser_tabs", { action: "focus", tab });
    assert.equal(focused.status, "blocked", JSON.stringify(focused)); assert.equal(focused.effect, "none");
    assert.equal(focused.facts.reason, "browser_attention_background", JSON.stringify(focused));
    assert.equal(focused.facts.browser_attention.value, "background");
    const cliFocus = await cli("browser_tabs", { action: "focus", tab: cliOpened.result.facts.tab });
    assert.equal(cliFocus.code, 2, JSON.stringify(cliFocus)); assert.equal(cliFocus.result.status, "blocked");
    assert.equal(cliFocus.result.effect, "none"); assert.equal(cliFocus.result.facts.reason, "browser_attention_background");
    const focusFlow = await call("browser_flow", { steps: [
      { tool: "browser_tabs", arguments: { action: "focus", tab } },
      { tool: "browser_type_text", arguments: { tab, selector: { name: "Agent draft", role: "textbox" }, text: "MUST_NOT_FOLLOW_FOCUS" } }
    ] });
    assert.equal(focusFlow.status, "blocked", JSON.stringify(focusFlow)); assert.equal(focusFlow.effect, "none");
    assert.equal(focusFlow.facts.steps[1].status, "not_run");
    assert.ok(!requests.slice(beforeFocus).some(request => request.command.command === "focus_tab"));
    assert.deepEqual(await state(), baseline);
    check("MCP, CLI and flow foreground requests are blocked before dispatch with truthful receipts");
    const directFocus = await rawWorker(`dispatch({correlation:'quiet-direct-focus',workspace:topology.workspaceFor(${physical}),
      attention:'background',command:{command:'focus_tab',tab_id:${physical}}})`);
    assert.deepEqual(directFocus, { outcome: "attention_protected", reason: "focus" }); assert.deepEqual(await state(), baseline);
    check("adapter mechanically refuses a background focus request");
    report.desktop_foreground_flow_before = desktopForeground(); save();
    const flow = await call("browser_flow", { steps: [
      { tool: "browser_navigate", arguments: { tab, url: `${origin}/flow` } },
      { tool: "browser_fill_form", arguments: { tab, fields: [{ selector: { name: "Agent draft", role: "textbox", exact: true }, value: "Flow retained" }] } }
    ] });
    report.desktop_foreground_flow_after = desktopForeground(); save();
    assert.equal(flow.status, "succeeded", JSON.stringify(flow)); assert.deepEqual(await state(), baseline);
    check("flow steps retain background behavior through the shared executor");
    const typingOpened = await call("browser_navigate", { url: `${origin}/native-text`, new_tab: true });
    assert.equal(typingOpened.status, "succeeded", JSON.stringify(typingOpened)); const typingTab = typingOpened.facts.tab;
    const typingPhysical = await rawWorker(`(await chrome.tabs.query({})).find(item=>item.url===${JSON.stringify(`${origin}/native-text`)}).id`);
    const typingState = () => rawWorker(`chrome.scripting.executeScript({target:{tabId:${typingPhysical}},world:'MAIN',
      func:()=>({value:agent.value,model:textModel,events:nativeTextInputs})}).then(items=>items[0].result)`);
    assert.equal((await rawWorker(`chrome.tabs.get(${typingPhysical})`)).active, false, "new typing fixture starts never selected");
    let expectedText = "";
    for (const mode of ["inactive", "active_unfocused"]) {
      if (mode === "active_unfocused") await rawWorker(`chrome.tabs.update(${typingPhysical},{active:true})`);
      const typingBrowserTab = await rawWorker(`chrome.tabs.get(${typingPhysical})`);
      const typingBrowserWindow = await rawWorker(`chrome.windows.get(${typingBrowserTab.windowId})`);
      report.observations.push({ name: "typing_window_before_input", mode,
        tab: { id: typingBrowserTab.id, active: typingBrowserTab.active, window: typingBrowserTab.windowId },
        window: { id: typingBrowserWindow.id, focused: typingBrowserWindow.focused }, human: await state(),
        neighbors: await rawWorker(`(await chrome.tabs.query({windowId:${typingBrowserTab.windowId}})).map(tab=>({id:tab.id,url:tab.url,active:tab.active,owner:topology.workspaceFor(tab.id)}))`),
        activations: await rawWorker("quietActivations"), window_focuses: await rawWorker("quietWindowFocuses") }); save();
      assert.equal(typingBrowserWindow.focused, false);
      expectedText = `Targeted ${mode}`;
      const targetedText = await call("browser_type_text", { tab: typingTab,
        selector: { name: "Agent draft", role: "textbox", exact: true }, text: expectedText, clear_first: true });
      assert.equal(targetedText.status, "succeeded", JSON.stringify(targetedText)); assert.equal(targetedText.effect, "applied");
      let actual = await typingState(); assert.equal(actual.value, expectedText); assert.equal(actual.model, expectedText);
      assert.ok(actual.events.length > 0 && actual.events.every(event=>event.trusted));
      // Fixture setup chooses a focused DOM control without sending input or selecting a window.
      await rawWorker(`chrome.scripting.executeScript({target:{tabId:${typingPhysical}},world:'MAIN',func:()=>agent.focus()})`);
      const focusedText = await call("browser_type_text", { tab: typingTab, focused: true, text: " focused" });
      assert.equal(focusedText.status, "succeeded", JSON.stringify(focusedText)); assert.equal(focusedText.effect, "applied");
      expectedText += " focused"; actual = await typingState(); assert.equal(actual.value, expectedText); assert.equal(actual.model, expectedText);
      assert.ok(actual.events.every(event=>event.trusted));
      report.observations.push({ name: "targeted_and_focused_native_text", mode, actual }); save();
      assert.deepEqual(await state(), baseline);
    }
    check("targeted and focused browser_type_text retain trusted input and model values in never-selected inactive and active unfocused tabs");
    const other = await attachMcp("quiet-native-race-peer");
    const otherCall = async (name, args) => receipt("mcp-peer", name,
      (await other.peer.send("tools/call", { name, arguments: args })).structuredContent);
    const peerOpened = await otherCall("browser_navigate", { url: `${origin}/native-peer`, new_tab: true });
    assert.equal(peerOpened.status, "succeeded", JSON.stringify(peerOpened)); const peerTab = peerOpened.facts.tab;
    const peerPhysical = await rawWorker(`(await chrome.tabs.query({})).find(item=>item.url===${JSON.stringify(`${origin}/native-peer`)}).id`);
    const physicalTabs = await rawWorker(`Promise.all([chrome.tabs.get(${physical}),chrome.tabs.get(${peerPhysical})])`);
    assert.notEqual(peerPhysical, physical); assert.equal(physicalTabs[0].windowId, physicalTabs[1].windowId,
      "independent clients must exercise the same native input window");
    assert.equal((await rawWorker(`chrome.windows.get(${physicalTabs[0].windowId})`)).focused, false);
    assert.notEqual(physicalTabs[0].windowId, human.windowId);
    const targetsFor = async (invoke, handle) => {
      const inspected = await invoke("browser_inspect", { tab: handle }); assert.equal(inspected.status, "succeeded", JSON.stringify(inspected));
      const field = inspected.facts.items.find(item=>item.name === "Agent draft");
      const button = inspected.facts.items.find(item=>item.name === "Native race effect");
      assert.ok(field?.target && button?.target, JSON.stringify(inspected)); return { field: field.target, button: button.target };
    };
    const [mainTargets, peerTargets] = await Promise.all([targetsFor(call, tab), targetsFor(otherCall, peerTab)]);
    const firstRace = await Promise.all([
      call("browser_press_key", { tab, target: mainTargets.field, key: "x" }),
      otherCall("browser_click", { tab: peerTab, target: peerTargets.button })
    ]);
    const secondRace = await Promise.all([
      call("browser_click", { tab, target: mainTargets.button }),
      otherCall("browser_press_key", { tab: peerTab, target: peerTargets.field, key: "y" })
    ]);
    for (const result of [...firstRace, ...secondRace]) {
      assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.effect, "applied");
      assert.equal(result.facts.browser_attention.value, "background");
    }
    const nativeState = tabId => rawWorker(`chrome.scripting.executeScript({target:{tabId:${tabId}},world:'MAIN',
      func:()=>({value:agent.value,keys:nativeKeys,clicks:nativeClicks,effects:raceEffects,focus:document.hasFocus()})}).then(items=>items[0].result)`);
    const [mainNative, peerNative] = await Promise.all([nativeState(physical), nativeState(peerPhysical)]);
    for (const [actual, value, key] of [[mainNative, "Flow retainedx", "x"], [peerNative, "y", "y"]]) {
      assert.equal(actual.value, value); assert.equal(actual.effects, 1); assert.deepEqual(actual.clicks, [{ trusted: true }]);
      assert.deepEqual(actual.keys, [{ type: "keydown", key, trusted: true }, { type: "keyup", key, trusted: true }]);
    }
    assert.deepEqual(await state(), baseline);
    assert.deepEqual(await rawWorker(`quietActivations.filter(info=>info.windowId===${human.windowId})`), []);
    assert.deepEqual(await rawWorker("quietWindowFocuses"), []);
    report.observations.push({ name: "two_client_native_effect_race", window: physicalTabs[0].windowId,
      main: mainNative, peer: peerNative, human: await state() }); save();
    check("two MCP clients concurrently deliver exact trusted native key and click effects in one unfocused owned window without disturbing human work");
    // This is the explicit operator reveal mechanism. The installed lane separately clicks the
    // actual Workbench button. Runtime remains Active and model permission stays Background.
    const operatorReveal = await rawWorker(`dispatch({correlation:'quiet-active-operator-reveal',workspace:topology.workspaceFor(${physical}),
      attention:'foreground',command:{command:'focus_tab',tab_id:${physical}}})`);
    assert.equal(operatorReveal.outcome, "tab_focused"); assert.equal(await rawWorker("liveState.control_state"), "active");
    const takeoverState = await state();
    const refusedNative = await call("browser_press_key", { tab, target: mainTargets.field, key: "z" });
    assert.equal(refusedNative.status, "blocked", JSON.stringify(refusedNative)); assert.equal(refusedNative.effect, "none");
    assert.equal(refusedNative.facts.attention_refusal, "native_input"); assert.deepEqual(await nativeState(physical), mainNative);
    const foregroundTabBefore = await rawWorker(`chrome.tabs.get(${physical})`);
    const fresh = await call("browser_navigate", { url: `${origin}/native-recovery`, new_tab: true });
    assert.equal(fresh.status, "succeeded", JSON.stringify(fresh)); const freshTab = fresh.facts.tab;
    const freshPhysical = await rawWorker(`(await chrome.tabs.query({})).find(item=>item.url===${JSON.stringify(`${origin}/native-recovery`)}).id`);
    const freshBrowserTab = await rawWorker(`chrome.tabs.get(${freshPhysical})`);
    assert.notEqual(freshBrowserTab.windowId, foregroundTabBefore.windowId);
    assert.notEqual(freshBrowserTab.windowId, human.windowId);
    assert.equal((await rawWorker(`chrome.windows.get(${freshBrowserTab.windowId})`)).focused, false);
    const freshReused = await call("browser_navigate", { url: `${origin}/native-recovery-reused` });
    assert.equal(freshReused.status, "succeeded", JSON.stringify(freshReused)); assert.equal(freshReused.facts.tab, freshTab);
    const freshTargets = await targetsFor(call, freshTab);
    const recoveredNative = await call("browser_press_key", { tab: freshTab, target: freshTargets.field, key: "r" });
    assert.equal(recoveredNative.status, "succeeded", JSON.stringify(recoveredNative)); const recoveredActual = await nativeState(freshPhysical);
    assert.equal(recoveredActual.value, "r"); assert.deepEqual(recoveredActual.keys,
      [{ type: "keydown", key: "r", trusted: true }, { type: "keyup", key: "r", trusted: true }]);
    assert.deepEqual(await state(), takeoverState);
    const foregroundTabAfter = await rawWorker(`chrome.tabs.get(${physical})`);
    assert.equal(foregroundTabAfter.windowId, foregroundTabBefore.windowId); assert.equal(foregroundTabAfter.groupId, foregroundTabBefore.groupId);
    assert.equal(foregroundTabAfter.active, true); assert.equal((await rawWorker(`chrome.windows.get(${foregroundTabAfter.windowId})`)).focused, true);
    report.observations.push({ name: "active_operator_reveal_native_refusal_and_fresh_recovery", refused: refusedNative.status,
      old_window: foregroundTabBefore.windowId, fresh_window: freshBrowserTab.windowId, fresh: await nativeState(freshPhysical) }); save();
    await rawWorker(`chrome.tabs.update(${human.id},{active:true})`); await rawWorker(`chrome.windows.update(${human.windowId},{focused:true})`);
    assert.deepEqual(await state(), baseline);
    check("operator reveal leaves runtime Active but foreground native work refuses; fresh quiet open and owned reuse recover without moving human or revealed tabs");
    const agentPage = expression => rawWorker(`chrome.debugger.sendCommand({tabId:${physical}},'Runtime.evaluate',
      {expression:${JSON.stringify(expression)},returnByValue:true}).then(result=>result.result.value)`);
    assert.equal(await agentPage("agent.value"), "Flow retainedx");
    const uncertain = await call("browser_flow", { steps: [
      { tool: "browser_execute", arguments: { tab, script: "window.effects++; throw new SyntaxError('Illegal return statement');" } },
      { tool: "browser_execute", arguments: { tab, script: "window.effects++; return window.effects;" } }
    ] });
    assert.equal(uncertain.status, "unknown", JSON.stringify(uncertain)); assert.equal(uncertain.effect, "unknown");
    assert.equal(uncertain.repeat_safe, false); assert.equal(uncertain.facts.steps[1].status, "not_run");
    assert.equal(await agentPage("window.effects"), 1, "uncertain effect is neither replayed nor followed by a dependent default-flow write");
    assert.deepEqual(await state(), baseline);
    check("uncertain script effect occurs once, remains unknown and stops dependent default-flow work");
    // Seed a duplicate group containing unowned work. Quiet opens may add their own tab,
    // but must not merge a person's same-title group or expand an existing collapsed group.
    assert.equal((await call("browser_navigate", { url: `${origin}/sibling`, new_tab: true })).status, "succeeded");
    const agentBefore = await rawWorker(`chrome.tabs.get(${physical})`);
    const agentGroup = await rawWorker(`chrome.tabGroups.get(${agentBefore.groupId})`);
    const duplicateWindow = await rawWorker(`chrome.windows.create({url:${JSON.stringify(`${origin}/duplicate-human`)},focused:false})`);
    const duplicateTab = duplicateWindow.tabs[0];
    const duplicateGroup = await rawWorker(`chrome.tabs.group({tabIds:[${duplicateTab.id}],createProperties:{windowId:${duplicateWindow.id}}})`);
    await rawWorker(`chrome.tabGroups.update(${duplicateGroup},{title:${JSON.stringify(agentGroup.title)},color:'red',collapsed:false})`);
    await rawWorker(`chrome.tabGroups.update(${agentGroup.id},{collapsed:true})`);
    const duplicateState = () => rawWorker(`Promise.all([chrome.tabs.get(${duplicateTab.id}),chrome.tabGroups.get(${duplicateGroup})])
      .then(([tab,group])=>({tab:{id:tab.id,url:tab.url,window:tab.windowId,group:tab.groupId},group:{id:group.id,title:group.title,collapsed:group.collapsed}}))`);
    const duplicateBaseline = await duplicateState();
    assert.equal((await call("browser_navigate", { url: `${origin}/another`, new_tab: true })).status, "succeeded");
    assert.deepEqual(await duplicateState(), duplicateBaseline);
    assert.equal((await rawWorker(`chrome.tabGroups.get(${agentGroup.id})`)).collapsed, true);
    await rawWorker(`chrome.tabs.move(${physical},{windowId:${duplicateWindow.id},index:-1})`);
    const moved = await rawWorker(`chrome.tabs.get(${physical})`);
    assert.equal((await call("browser_navigate", { tab, url: `${origin}/moved` })).status, "succeeded");
    const afterMoved = await rawWorker(`chrome.tabs.get(${physical})`);
    assert.equal(afterMoved.windowId, moved.windowId); assert.equal(afterMoved.groupId, moved.groupId);
    assert.deepEqual(await duplicateState(), duplicateBaseline); assert.deepEqual(await state(), baseline);
    check("quiet opens preserve duplicate human groups and collapsed groups; navigation respects manually moved owned tabs");
    const retainedValue = "Control cleanup retained";
    assert.equal((await call("browser_fill_form", { tab, fields: [{ selector: { name: "Agent draft", role: "textbox" }, value: retainedValue }] })).status, "succeeded");
    assert.equal(await agentPage("agent.value"), retainedValue);
    // Native input leaves Chromium's ordinary hasFocus flag sticky even after false+detach
    // (also reproduced on the pinned baseline and a never-emulated tab). Use a separate
    // retained, inactive target with no native input as the cleanup positive control.
    const custodyOpened = await call("browser_navigate", { url: `${origin}/custody-sentinel`, new_tab: true });
    assert.equal(custodyOpened.status, "succeeded", JSON.stringify(custodyOpened));
    const custodyPhysical = await rawWorker(`(await chrome.tabs.query({})).find(item=>item.url===${JSON.stringify(`${origin}/custody-sentinel`)}).id`);
    const custodyTab = await rawWorker(`chrome.tabs.get(${custodyPhysical})`);
    const custodySibling = await rawWorker(`(await chrome.tabs.query({windowId:${custodyTab.windowId}})).find(item=>item.id!==${custodyPhysical})`);
    if (custodySibling) await rawWorker(`chrome.tabs.update(${custodySibling.id},{active:true})`);
    else await rawWorker(`chrome.tabs.create({url:${JSON.stringify(`${origin}/custody-human-sibling`)},windowId:${custodyTab.windowId},active:true})`);
    const custodyPage = () => rawWorker(`chrome.scripting.executeScript({target:{tabId:${custodyPhysical}},world:'MAIN',
      func:()=>({focus:document.hasFocus(),visibility:document.visibilityState})}).then(items=>items[0].result)`);
    assert.equal((await rawWorker(`chrome.tabs.get(${custodyPhysical})`)).active, false);
    await until(async () => (await custodyPage()).focus === true, "inactive never-native controlled sentinel keeps focus emulation");
    report.observations.push({ name: "cleanup_before_pause", native_focus: await agentPage("document.hasFocus()"), sentinel: await custodyPage() }); save();
    nativeSend({ kind: "event", event: { event: "runtime_control_requested", intent: "hold" } });
    await until(() => controls.at(-1) === "held", "pause reaches adapter");
    await until(async () => (await custodyPage()).focus === false, "pause removes inactive never-native focus emulation");
    report.observations.push({ name: "cleanup_after_pause", native_focus: await agentPage("document.hasFocus()"), sentinel: await custodyPage() }); save();
    const paused = await call("browser_type_text", { tab, selector: { name: "Agent draft", role: "textbox" }, text: "MUST_NOT_TYPE" });
    assert.equal(paused.status, "blocked"); assert.equal(paused.effect, "none"); assert.equal(await agentPage("agent.value"), retainedValue);
    const revealed = await rawWorker(`dispatch({correlation:'quiet-human-reveal',workspace:topology.workspaceFor(${physical}),
      attention:'foreground',command:{command:'focus_tab',tab_id:${physical}}})`);
    assert.equal(revealed.outcome, "tab_focused"); assert.equal(revealed.active, true); assert.equal(revealed.window_focused, true);
    assert.equal(await rawWorker("liveState.control_state"), "held");
    assert.equal((await call("browser_type_text", { tab, selector: { name: "Agent draft", role: "textbox" }, text: "REVEAL_IS_NOT_PERMISSION" })).effect, "none");
    assert.equal(await agentPage("agent.value"), retainedValue);
    await rawWorker(`chrome.tabs.update(${human.id},{active:true})`);
    await rawWorker(`chrome.windows.update(${human.windowId},{focused:true})`);
    check("foreground reveal mechanism shows the tab while Pause still blocks agent input");
    nativeSend({ kind: "event", event: { event: "runtime_control_requested", intent: "resume" } });
    await until(() => controls.at(-1) === "active", "resume reaches adapter");
    await until(async () => (await custodyPage()).focus === true, "resume restores inactive never-native focus emulation");
    assert.equal(await agentPage("agent.value"), retainedValue, "resume must not replay paused input");
    assert.equal((await call("browser_read", { tab })).status, "succeeded");
    check("pause removes controlled focus, blocks future effects, and resume reobserves without replay");
    const windowBeforeResize = await rawWorker(`chrome.windows.get(${duplicateWindow.id})`);
    const resized = await call("browser_window", { tab, action: "resize", width: 1100, height: 700 });
    assert.equal(resized.status, "blocked", JSON.stringify(resized)); assert.equal(resized.effect, "none");
    assert.equal(resized.facts.attention_refusal, "shared_window_resize");
    const windowAfterResize = await rawWorker(`chrome.windows.get(${duplicateWindow.id})`);
    assert.equal(windowAfterResize.width, windowBeforeResize.width); assert.equal(windowAfterResize.height, windowBeforeResize.height);
    // Change only the disposable adapter's existing preserve-tabs interlock, so attention
    // protection is tested independently rather than accidentally passing on another refusal.
    await rawWorker("chrome.storage.local.set({ghostlight_preserve_tabs:false})");
    const closed = await call("browser_tabs", { tab, action: "close" });
    assert.equal(closed.status, "blocked", JSON.stringify(closed)); assert.equal(closed.effect, "none");
    assert.equal(closed.facts.attention_refusal, "active_tab_close");
    assert.equal((await rawWorker(`chrome.tabs.get(${physical})`)).id, physical);
    await rawWorker("chrome.storage.local.set({ghostlight_preserve_tabs:true})");
    assert.deepEqual(await duplicateState(), duplicateBaseline);
    check("shared-window resize and active-tab close refuse before disturbing the unowned neighbor");
    // Closed passive feedback and runtime installation use BrowserPresentation/the adapter
    // install boundary. Every model browser operation must carry the effective background rule.
    assert.ok(requests.filter(request => !["install_page_runtime", "present"].includes(request.command.command))
      .every(request => request.attention === "background"));
    const controlledPhysicals = await rawWorker("(await chrome.tabs.query({})).filter(tab=>topology.workspaceFor(tab.id)).map(tab=>tab.id)");
    nativeSend({ kind: "event", event: { event: "runtime_control_requested", intent: "end_session" } });
    await until(() => controls.at(-1) === "ended", "stop reaches adapter");
    const cleanupState = () => rawWorker(`chrome.scripting.executeScript({target:{tabId:${physical}},world:'MAIN',
      func:()=>({focus:document.hasFocus(),value:agent.value})}).then(items=>items[0].result)`);
    await until(async () => (await custodyPage()).focus === false, "stop restores inactive never-native page focus");
    const targetsAfterStop = await rawWorker("chrome.debugger.getTargets()");
    for (const ownedPhysical of controlledPhysicals) assert.equal(Boolean(targetsAfterStop.find(target=>target.tabId===ownedPhysical)?.attached), false);
    report.observations.push({ name: "cleanup_after_stop", native_focus: (await cleanupState()).focus,
      sentinel: await custodyPage(), owned_debuggers_detached: controlledPhysicals.length }); save();
    const stopped = await call("browser_type_text", { tab, selector: { name: "Agent draft", role: "textbox" }, text: "MUST_NOT_REPLAY" });
    assert.equal(stopped.status, "blocked", JSON.stringify(stopped)); assert.equal(stopped.effect, "none");
    assert.equal(stopped.facts.reason, "session_ended");
    assert.equal((await cleanupState()).value, retainedValue); assert.deepEqual(await state(), baseline);
    check("stop restores normal focus, preserves the unsent effect and leaves human work untouched");
  }
  journeyPassed = true;
} catch (error) {
  if (observeFailure) { try { report.browser_at_failure = await observeFailure(); } catch (diagnosticError) { report.browser_diagnostic_failure = String(diagnosticError); } }
  try { report.desktop_foreground_at_failure = desktopForeground(); } catch (diagnosticError) { report.desktop_foreground_failure = String(diagnosticError); }
  report.failure = String(error.stack || error); report.finished_at = new Date().toISOString(); save(); throw error;
} finally {
  try {
    if (cdp && socket?.readyState === WebSocket.OPEN) { try { await cdp.send("Browser.close"); } catch {} }
    socket?.close(); poll?.end("[]");
    if (chromium) await waitForChromiumExit(chromium);
    for (const child of children.toReversed()) if (child.exitCode === null && child.signalCode === null) child.kill();
    await until(() => children.every(child => child.exitCode !== null || child.signalCode !== null || child.startError), "owned children exit");
    server.closeAllConnections(); await new Promise(done => server.close(done));
    await removeBrowserScratch(scratch, scratchRoot, "quiet-browser-");
    for (const path of [runtimeFile, runtimeFile.replace(/\.json$/, ".lock")]) {
      assert.equal(dirname(resolve(path)), binDir); rmSync(path, { force: true });
    }
    if (createdDeployLock) { assert.equal(readFileSync(deployLock, "utf8"), deployMarker); rmSync(deployLock); }
    report.passed = journeyPassed;
    if (journeyPassed) delete report.failure;
  } catch (error) {
    report.passed = false; report.cleanup_failure = String(error.stack || error); throw error;
  } finally {
    report.finished_at = new Date().toISOString(); save(); console.log(`Evidence: ${evidencePath}`);
  }
}
