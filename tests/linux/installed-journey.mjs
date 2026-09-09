// Opt-in recovery acceptance through Linux's real registered native host. The caller supplies
// an idle development/test installation and a dedicated running browser; no native pipe is mocked.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, realpathSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

const options = new Map();
for (let index = 2; index < process.argv.length; index++) {
  const name = process.argv[index];
  assert.ok(["--bin-dir", "--browser", "--devtools-port-file", "--exercise-installed-stack"].includes(name), `Unknown option: ${name}`);
  assert.ok(!options.has(name), `Repeated option: ${name}`);
  const value = name === "--exercise-installed-stack" ? true : process.argv[++index];
  assert.ok(value && !String(value).startsWith("--"), `Missing value: ${name}`);
  options.set(name, value);
}
assert.ok(process.platform === "linux" && options.get("--exercise-installed-stack") &&
  options.get("--bin-dir") && options.get("--browser"),
"Usage: node tests/linux/installed-journey.mjs --bin-dir <installed siblings> --browser <running executable> --exercise-installed-stack [--devtools-port-file <dedicated browser file>]");
for (const name of ["GHOSTLIGHT_RUNTIME_FILE", "GHOSTLIGHT_NATIVE_HOST_DIR", "GHOSTLIGHT_PROFILE_DIR", "GHOSTLIGHT_POLICY_FILE", "GHOSTLIGHT_RUNTIME_CONTROL_FILE"]) {
  assert.ok(!process.env[name], `Remove ${name}: this journey requires the installation's ordinary paths.`);
}
const root = resolve(import.meta.dirname, "../..");
const bin = realpathSync(options.get("--bin-dir"));
const browserImage = realpathSync(options.get("--browser"));
const authorityImage = join(bin, "ghostlight");
const nativeImage = join(bin, "ghostlight-browser-connector");
const mcpImage = join(bin, "ghostlight-mcp-connector");
const configRoot = process.env.XDG_CONFIG_HOME || join(homedir(), ".config");
const output = join(process.env.GHOSTLIGHT_LINUX_INSTALLED_AREA || join(root, ".tmp/linux-installed"),
  "recovery", `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`);
mkdirSync(output, { recursive: true });
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const delay = ms => new Promise(done => setTimeout(done, ms));
const report = { started_at: new Date().toISOString(), passed: false, release_ready: false,
  scope: "Linux development installation; actual native messaging; dedicated browser context",
  revision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim(),
  test_sha256: hash(readFileSync(new URL(import.meta.url))),
  browser_version: execFileSync(browserImage, ["--version"], { encoding: "utf8", timeout: 10000 }).trim(),
  config_root: configRoot, binaries: {}, identities: {}, phases: [], invocations: [],
  remaining_release_gates: ["clean-install-both-orders", "candidate-package-lifecycle", "store-adapter",
    "second-browser", "three-real-MCP-clients", "governed-installed-browser", "visible-desktop-lifecycle"] };
const save = () => writeFileSync(join(output, "results.json"), JSON.stringify(report, null, 2) + "\n");
save();
function identity(pid) {
  try {
    const fields = readFileSync(`/proc/${pid}/stat`, "utf8").split(/\) /).at(-1).split(" ");
    return { pid: Number(pid), image: readlinkSync(`/proc/${pid}/exe`), start_ticks: fields[19], parent: Number(fields[1]) };
  } catch (error) { if (["ENOENT", "EACCES", "EPERM", "ESRCH"].includes(error.code)) return null; throw error; }
}
function processes(image) {
  return readdirSync("/proc").filter(name => /^\d+$/.test(name)).map(identity).filter(item => item?.image === image);
}
function only(image) { const matches = processes(image); assert.equal(matches.length, 1, `Requires one exact process: ${image}`); return matches[0]; }
function unchanged(observed) { const current = identity(observed.pid); assert.equal(current?.image, observed.image, "Process executable changed"); assert.equal(current?.start_ticks, observed.start_ticks, "Process lifetime changed"); }
function stop(observed) { unchanged(observed); process.kill(observed.pid, "SIGKILL"); }
function nativePipes(observed) { unchanged(observed); return [0, 1].map(fd => readlinkSync(`/proc/${observed.pid}/fd/${fd}`)); }
function descendsFrom(observed, ancestor) {
  let current = observed;
  for (let depth = 0; current && depth < 16; depth++) {
    if (current.pid === ancestor.pid) return current.start_ticks === ancestor.start_ticks;
    current = identity(current.parent);
  }
  return false;
}
function product(args) { return execFileSync(authorityImage, args, { encoding: "utf8", timeout: 15000, stdio: ["ignore", "pipe", "pipe"] }); }
function doctor() { return JSON.parse(product(["doctor", "--json"])); }
async function until(check, label, timeout = 45000) {
  const deadline = Date.now() + timeout;
  do { const result = await check(); if (result) return result; await delay(100); } while (Date.now() < deadline);
  throw new Error(`Timed out: ${label}`);
}
async function ready() { return until(() => doctor().readiness?.state === "ready", "installed readiness"); }
let browser, restoreOwed = false, mcp, socket, heldResponse, effectCount = 0, activePhase = "preflight";
const registrations = [];
const pending = new Map();
let nextId = 0;
function receive(message) {
  const waiter = pending.get(message.id);
  if (waiter) { pending.delete(message.id); clearTimeout(waiter.timer); waiter.resolve(message); }
}
function request(method, params = {}) {
  return new Promise((resolveReply, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`MCP timed out: ${method}`)); }, 45000);
    pending.set(id, { resolve: resolveReply, reject, timer });
    mcp.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
}
async function call(name, args) {
  const reply = await request("tools/call", { name, arguments: args });
  const result = reply.result?.structuredContent;
  const reason = result?.facts?.reason;
  report.invocations.push({ tool: name, status: result?.status, effect: result?.effect,
    reason: typeof reason === "string" && /^[a-z_]+$/.test(reason) ? reason : null,
    ...(Number.isInteger(reply.error?.code) ? { protocol_error_code: reply.error.code } : {}) });
  save();
  assert.equal(reply.error, undefined, "Unexpected MCP protocol failure");
  assert.equal(reply.result?.structuredContent?.status, "succeeded", `${name} did not succeed`);
  return reply.result.structuredContent;
}
function phase(name, started, facts = {}) {
  unchanged(browser);
  report.phases.push({ name, status: "passed", elapsed_ms: Date.now() - started, ...facts });
  save(); console.log(`PASS installed Linux: ${name}`);
}
const fixture = createServer((incoming, response) => {
  if (incoming.url === "/effect" && incoming.method === "POST") {
    effectCount++; heldResponse = response; return;
  }
  response.writeHead(200, { "Content-Type": "text/html" });
  response.end("<!doctype html><title>Ghostlight recovery fixture</title><h1>Recovery fixture ready</h1>");
});
try {
  for (const image of [authorityImage, nativeImage, mcpImage]) report.binaries[image] = hash(readFileSync(image));
  const roots = processes(browserImage).filter(item => !readFileSync(`/proc/${item.pid}/cmdline`, "utf8").includes("--type="));
  assert.equal(roots.length, 1, "Requires one running browser root at the selected executable");
  browser = roots[0]; report.identities.browser = browser;
  let authority = only(authorityImage), native = only(nativeImage);
  assert.ok(descendsFrom(native, browser), "Native host must descend from the selected real browser");
  assert.equal(doctor().readiness?.state, "ready", "Requires an idle Ready installation");
  report.identities.initial_authority = authority; report.identities.initial_native = native;
  // native-host install/uninstall owns this fixed roster. Refuse partial, foreign, symlinked,
  // or cross-installation state before allowing that aggregate command to remove anything.
  for (const directory of ["google-chrome", "microsoft-edge", "BraveSoftware/Brave-Browser", "chromium"]) {
    const path = join(configRoot, directory, "NativeMessagingHosts/org.sylin.ghostlight.json");
    assert.ok(lstatSync(path).isFile() && !lstatSync(path).isSymbolicLink(), "Requires ordinary native manifests");
    const bytes = readFileSync(path), manifest = JSON.parse(bytes);
    assert.equal(manifest.name, "org.sylin.ghostlight");
    assert.equal(manifest.path, nativeImage, "Every registration must belong to this installation");
    registrations.push({ path, bytes, sha256: hash(bytes) });
  }
  save();
  activePhase = "registration-reinstalled-with-browser-running";
  restoreOwed = true;
  product(["native-host", "uninstall"]);
  assert.ok(registrations.every(item => !existsSync(item.path)), "Native registrations were not removed");
  stop(native);
  await until(() => doctor().readiness?.state === "not_connected", "native-host absence");
  await delay(1500);
  let started = Date.now();
  product(["native-host", "install"]);
  await ready();
  for (const item of registrations) assert.equal(hash(readFileSync(item.path)), item.sha256, "Restored manifest bytes differ");
  restoreOwed = false;
  unchanged(authority);
  native = only(nativeImage);
  assert.ok(descendsFrom(native, browser));
  phase("registration-reinstalled-with-browser-running", started);

  const priorNative = native;
  activePhase = "native-connector-crash-recovers";
  started = Date.now(); stop(priorNative);
  native = await until(() => { const matches = processes(nativeImage); return matches.length === 1 && matches[0].pid !== priorNative.pid && matches[0]; }, "native connector replacement");
  await ready(); unchanged(authority); assert.ok(descendsFrom(native, browser));
  phase("native-connector-crash-recovers", started);

  mcp = spawn(mcpImage, [], { stdio: ["pipe", "pipe", "pipe"] });
  mcp.stderr.on("data", () => {});
  mcp.on("error", error => { for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(error); } pending.clear(); });
  mcp.on("exit", () => { for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(new Error("MCP exited")); } pending.clear(); });
  createInterface({ input: mcp.stdout }).on("line", line => receive(JSON.parse(line)));
  const initialized = await request("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "Linux installed recovery", version: "1" } });
  assert.equal(initialized.result?.serverInfo.name, "ghostlight");
  mcp.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
  await call("browser_tabs", { action: "list" });
  report.identities.retained_mcp = identity(mcp.pid);
  const pipes = nativePipes(native);
  report.identities.retained_native = { ...native, pipes };
  activePhase = "authority-crash-retains-native-pipes-and-initialized-MCP";
  started = Date.now(); stop(authority);
  await ready(); unchanged(native); assert.deepEqual(nativePipes(native), pipes);
  authority = only(authorityImage);
  assert.notEqual(authority.pid, report.identities.initial_authority.pid);
  await call("browser_tabs", { action: "list" });
  unchanged(report.identities.retained_mcp);
  phase("authority-crash-retains-native-pipes-and-initialized-MCP", started);

  await new Promise(done => fixture.listen(0, "127.0.0.1", done));
  const opened = await call("browser_navigate", { url: `http://127.0.0.1:${fixture.address().port}/`, new_tab: true });
  await call("browser_wait", { tab: opened.facts.tab, condition: "load_ready" });
  const read = await call("browser_read", { tab: opened.facts.tab });
  assert.match(read.facts.text, /Recovery fixture ready/);
  started = Date.now();
  activePhase = "in-flight-browser-effect-is-not-replayed";
  const interrupted = request("tools/call", { name: "browser_execute", arguments: { tab: opened.facts.tab,
    script: "await fetch('/effect', {method:'POST'}); 'effect acknowledged'", timeout_ms: 30000 } });
  // Observe the side effect independently at the fixture server, before dropping its receipt.
  await until(() => effectCount === 1, "independently observed browser effect", 15000);
  stop(authority);
  const interruptedReply = await interrupted;
  assert.match(interruptedReply.error?.message || "", /outcome is unavailable/);
  heldResponse.end("recorded"); heldResponse = null;
  await ready(); unchanged(native); assert.deepEqual(nativePipes(native), pipes);
  await call("browser_tabs", { action: "list" });
  await delay(1500);
  assert.equal(effectCount, 1, "Uncertain browser effect must never replay");
  phase("in-flight-browser-effect-is-not-replayed", started, { effect_count: effectCount, result: "outcome_unavailable" });
  report.preserved_tab = opened.facts.tab;

  if (options.has("--devtools-port-file")) {
    activePhase = "stopped-worker-preserves-binding-and-recovers-on-browser-activity";
    const [port, endpoint] = readFileSync(options.get("--devtools-port-file"), "utf8").trim().split(/\r?\n/);
    assert.match(port, /^\d+$/); assert.match(endpoint, /^\/devtools\/browser\/[a-zA-Z0-9-]+$/);
    socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
    await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
    let cdpId = 0;
    const waiting = new Map(), versions = new Map();
    socket.onmessage = ({ data }) => {
      const message = JSON.parse(data);
      if (message.method === "ServiceWorker.workerVersionUpdated") for (const version of message.params.versions) versions.set(version.versionId, version);
      const waiter = waiting.get(message.id);
      if (waiter) { waiting.delete(message.id); clearTimeout(waiter.timer); message.error ? waiter.reject(new Error("CDP worker control failed")) : waiter.done(message.result); }
    };
    const send = (method, params = {}, sessionId) => new Promise((done, reject) => {
      const id = ++cdpId, timer = setTimeout(() => { waiting.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 10000);
      waiting.set(id, { done, reject, timer });
      socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
    const { processInfo } = await send("SystemInfo.getProcessInfo");
    assert.ok(processInfo.some(item => item.type === "browser" && item.id === browser.pid), "CDP must belong to the selected dedicated browser");
    const { targetInfos } = await send("Target.getTargets");
    const worker = targetInfos.find(item => item.type === "service_worker" && /^chrome-extension:\/\/(cjcmhepmagomefjggkcohdbfemacojoa|lejccfmoeogmhemakeknjjdhkfkgncdl)\/service-worker\.js$/.test(item.url));
    assert.ok(worker, "Requires the actual Ghostlight worker");
    const page = targetInfos.find(item => item.type === "page" && item.url === `http://127.0.0.1:${fixture.address().port}/`);
    assert.ok(page, "Requires this journey's own fixture tab");
    const { sessionId } = await send("Target.attachToTarget", { targetId: page.targetId, flatten: true });
    await send("ServiceWorker.enable", {}, sessionId);
    const version = await until(() => [...versions.values()].find(item => item.scriptURL === worker.url && item.runningStatus === "running"), "running adapter worker version", 10000);
    started = Date.now();
    await send("ServiceWorker.stopWorker", { versionId: version.versionId }, sessionId);
    await until(() => versions.get(version.versionId)?.runningStatus === "stopped", "worker actually stopped", 10000);
    await until(() => doctor().readiness?.state === "not_connected", "service observed stopped worker");
    // This initialized workspace is pinned to its original browser identity. ADR-0114 and
    // recovery::decide deliberately refuse to select a replacement profile while it is absent.
    // Prove that refusal, then wake this same browser with an ordinary new-tab event. No
    // extension reload, explicit worker start, or browser restart is used.
    const recovery = await request("tools/call", { name: "browser_tabs", arguments: { action: "list" } });
    assert.equal(recovery.error, undefined);
    const result = recovery.result.structuredContent;
    assert.equal(result.status, "failed"); assert.equal(result.effect, "none");
    assert.equal(result.facts.reason, "browser_wrong_profile"); assert.equal(result.repeat_safe, true);
    report.invocations.push({ tool: "browser_tabs", status: result.status, effect: result.effect, reason: result.facts.reason });
    const wakeTab = (await send("Target.createTarget", { url: "about:blank" })).targetId;
    await until(async () => (await send("Target.getTargets")).targetInfos.some(item => item.type === "service_worker" && item.url === worker.url && item.targetId !== worker.targetId), "worker recovery on browser activity");
    await ready();
    await call("browser_tabs", { action: "list" });
    assert.equal(effectCount, 1);
    phase(activePhase, started, { effect_count: effectCount, recovery_trigger: "ordinary_browser_tab" });
    await send("Target.closeTarget", { targetId: wakeTab });
    await send("Target.detachFromTarget", { sessionId });
  } else {
    report.phases.push({ name: "stopped-worker-preserves-binding-and-recovers-on-browser-activity", status: "blocked", reason: "Dedicated browser DevTools endpoint was not supplied." });
  }
  for (const [path, digest] of Object.entries(report.binaries)) assert.equal(hash(readFileSync(path)), digest, "Installed binary changed during the run");
  unchanged(browser);
  report.passed = report.phases.every(item => item.status === "passed");
} catch (error) {
  report.failure = error.message;
  report.phases.push({ name: activePhase, status: "failed", reason: error.message });
  process.exitCode = 1;
  console.error(error);
} finally {
  try {
    if (restoreOwed) {
      // Re-check after interruption. Never overwrite a registration installed by another actor.
      for (const item of registrations) {
        if (existsSync(item.path)) assert.equal(JSON.parse(readFileSync(item.path)).path, nativeImage, "Cleanup found a different installation");
        writeFileSync(item.path, item.bytes);
        assert.equal(hash(readFileSync(item.path)), item.sha256);
      }
    }
  } catch (error) { report.restoration_failure = error.message; report.passed = false; }
  heldResponse?.end("recorded");
  fixture.closeAllConnections(); fixture.close();
  socket?.close();
  mcp?.stdin.end();
  if (mcp && mcp.exitCode === null) { await delay(500); if (mcp.exitCode === null) mcp.kill(); }
  report.finished_at = new Date().toISOString(); save();
  console.log(`Evidence: ${join(output, "results.json")}`);
  if (!report.passed) process.exitCode = 1;
}
