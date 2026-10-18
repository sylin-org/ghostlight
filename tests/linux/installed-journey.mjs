// Opt-in recovery acceptance through Linux's real registered native host. The caller supplies
// the idle installation and the running system browser; no native pipe is mocked.
// The installation holds two images: the edge `ghostlight`, which is the MCP relay and the browser
// relay, and the service `ghostlight-service`. A lost service takes its browser edge with it: once
// a service answers again the browser edge exits and the browser starts a new one, while an MCP
// edge keeps its session and starts the next service when a request needs it.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, readlinkSync, realpathSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { RelayMcp } from "../lib/peers.mjs";
import { templateKind } from "../lib/home.mjs";
import { invoker } from "./lib/reporting.mjs";

const options = new Map();
for (let index = 2; index < process.argv.length; index++) {
  const name = process.argv[index];
  assert.ok(["--bin-dir", "--browser", "--exercise-installed-stack"].includes(name), `Unknown option: ${name}`);
  assert.ok(!options.has(name), `Repeated option: ${name}`);
  const value = name === "--exercise-installed-stack" ? true : process.argv[++index];
  assert.ok(value && !String(value).startsWith("--"), `Missing value: ${name}`);
  options.set(name, value);
}
assert.ok(process.platform === "linux" && options.get("--exercise-installed-stack") &&
  options.get("--bin-dir") && options.get("--browser"),
"Usage: node tests/linux/installed-journey.mjs --bin-dir <installed siblings> --browser <running executable> --exercise-installed-stack");
for (const name of ["GHOSTLIGHT_TEST_HOME", "GHOSTLIGHT_NATIVE_HOST_DIR", "GHOSTLIGHT_PROFILE_DIR", "GHOSTLIGHT_POLICY_FILE"]) {
  assert.ok(!process.env[name], `Remove ${name}: this journey requires the installation's ordinary paths.`);
}
const root = resolve(import.meta.dirname, "../..");
const bin = realpathSync(options.get("--bin-dir"));
const browserImage = realpathSync(options.get("--browser"));
const serviceImage = join(bin, "ghostlight-service");
const edgeImage = join(bin, "ghostlight");
const installedHome = join(homedir(), ".ghostlight");
const configRoot = process.env.XDG_CONFIG_HOME || join(homedir(), ".config");
const output = join(process.env.GHOSTLIGHT_LINUX_INSTALLED_AREA || join(root, ".tmp/linux-installed"),
  "recovery", `${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}`);
mkdirSync(output, { recursive: true });
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const delay = ms => new Promise(done => setTimeout(done, ms));
const report = { started_at: new Date().toISOString(), passed: false,
  scope: "Linux development installation; actual native messaging; the running system browser",
  revision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim(),
  test_sha256: hash(readFileSync(new URL(import.meta.url))),
  browser_version: execFileSync(browserImage, ["--version"], { encoding: "utf8", timeout: 10000 }).trim(),
  config_root: configRoot, binaries: {}, identities: {}, phases: [], invocations: [] };
const save = () => writeFileSync(join(output, "results.json"), JSON.stringify(report, null, 2) + "\n");
save();
function identity(pid) {
  try {
    const fields = readFileSync(`/proc/${pid}/stat`, "utf8").split(/\) /).at(-1).split(" ");
    return { pid: Number(pid), image: readlinkSync(`/proc/${pid}/exe`), args: readFileSync(`/proc/${pid}/cmdline`, "utf8").split("\0").slice(1).filter(Boolean),
      start_ticks: fields[19], parent: Number(fields[1]) };
  } catch (error) { if (["ENOENT", "EACCES", "EPERM", "ESRCH"].includes(error.code)) return null; throw error; }
}
function processes(image, wanted = () => true) {
  return readdirSync("/proc").filter(name => /^\d+$/.test(name)).map(identity).filter(item => item?.image === image && wanted(item));
}
const isBrowserEdge = item => item.args[0]?.startsWith("chrome-extension://");
const nativeEdges = () => processes(edgeImage, isBrowserEdge);
function only(image, wanted) { const matches = processes(image, wanted); assert.equal(matches.length, 1, `Requires one exact process: ${image}`); return matches[0]; }
function unchanged(observed) { const current = identity(observed.pid); assert.equal(current?.image, observed.image, "Process executable changed"); assert.equal(current?.start_ticks, observed.start_ticks, "Process lifetime changed"); }
function stop(observed) { unchanged(observed); process.kill(observed.pid, "SIGKILL"); }
function descendsFrom(observed, ancestor) {
  let current = observed;
  for (let depth = 0; current && depth < 16; depth++) {
    if (current.pid === ancestor.pid) return current.start_ticks === ancestor.start_ticks;
    current = identity(current.parent);
  }
  return false;
}
function product(args) { return execFileSync(edgeImage, args, { encoding: "utf8", timeout: 15000, stdio: ["ignore", "pipe", "pipe"] }); }
function doctor() { return JSON.parse(product(["doctor", "--json"])); }
async function until(check, label, timeout = 45000) {
  const deadline = Date.now() + timeout;
  do { const result = await check(); if (result) return result; await delay(100); } while (Date.now() < deadline);
  throw new Error(`Timed out: ${label}`);
}
async function ready() { return until(() => doctor().readiness?.state === "ready", "installed readiness"); }
let browser, restoreOwed = false, mcp, heldResponse, effectCount = 0, activePhase = "preflight";
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
const call = invoker({ request, report, save });
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
  for (const image of [serviceImage, edgeImage]) report.binaries[image] = hash(readFileSync(image));
  const roots = processes(browserImage).filter(item => !readFileSync(`/proc/${item.pid}/cmdline`, "utf8").includes("--type="));
  assert.equal(roots.length, 1, "Requires one running browser root at the selected executable");
  browser = roots[0]; report.identities.browser = browser;
  let authority = only(serviceImage), native = only(edgeImage, isBrowserEdge);
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
    assert.equal(manifest.path, edgeImage, "Every registration must belong to this installation");
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
  native = only(edgeImage, isBrowserEdge);
  assert.ok(descendsFrom(native, browser));
  phase("registration-reinstalled-with-browser-running", started);

  const priorNative = native;
  activePhase = "native-connector-crash-recovers";
  started = Date.now(); stop(priorNative);
  native = await until(() => { const matches = nativeEdges(); return matches.length === 1 && matches[0].pid !== priorNative.pid && matches[0]; }, "browser edge replacement");
  await ready(); unchanged(authority); assert.ok(descendsFrom(native, browser));
  phase("native-connector-crash-recovers", started);

  mcp = spawn(edgeImage, ["mcp"], { stdio: ["pipe", "pipe", "pipe"] });
  mcp.stderr.on("data", () => {});
  mcp.on("error", error => { for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(error); } pending.clear(); });
  mcp.on("exit", () => { for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(new Error("MCP exited")); } pending.clear(); });
  createInterface({ input: mcp.stdout }).on("line", line => receive(JSON.parse(line)));
  const initialized = await request("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "Linux installed recovery", version: "1" } });
  assert.equal(initialized.result?.serverInfo.name, "ghostlight");
  mcp.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
  await call("browser_tabs", { action: "list" });
  report.identities.retained_mcp = identity(mcp.pid);
  // A service lost to a crash: the MCP edge keeps its stream and starts the next service when a
  // request needs it; the browser edge exits once that service answers and the browser starts a new
  // one. Both ends are the same stream and the same browser, with a new edge between them.
  async function recoverAfterServiceLoss(lost) {
    const relisted = await request("tools/list");
    assert.equal(relisted.error, undefined, "The initialized MCP stream must survive a lost service");
    assert.ok(relisted.result.tools.length > 0);
    await ready();
    // The MCP edge and the tray shell both start a service after a crash. The start that loses
    // the lock waits LEAVING_LOCK_WAIT (5 s) for a service that might be leaving, then gives way,
    // so one service remains once the race settles.
    const replacement = await until(() => { const matches = processes(serviceImage); return matches.length === 1 && matches[0]; }, "one service once the racing starts give way");
    assert.notEqual(replacement.pid, lost.pid, "A new service must answer");
    const edge = await until(() => { const matches = nativeEdges(); return matches.length === 1 && matches[0].pid !== native.pid && matches[0]; }, "browser edge replacement after the service returns");
    assert.ok(descendsFrom(edge, browser), "The new browser edge must descend from the selected browser");
    unchanged(report.identities.retained_mcp);
    return { replacement, edge };
  }
  activePhase = "service-crash-keeps-the-initialized-MCP-stream-and-the-browser-reconnects";
  started = Date.now(); stop(authority);
  ({ replacement: authority, edge: native } = await recoverAfterServiceLoss(authority));
  assert.notEqual(authority.pid, report.identities.initial_authority.pid);
  await call("browser_tabs", { action: "list" });
  phase("service-crash-keeps-the-initialized-MCP-stream-and-the-browser-reconnects", started);

  await new Promise(done => fixture.listen(0, "127.0.0.1", done));
  const opened = await call("browser_navigate", { url: `http://127.0.0.1:${fixture.address().port}/`, new_tab: true });
  await call("browser_wait", { tab: opened.facts.tab, condition: "load_ready" });
  const read = await call("browser_read", { tab: opened.facts.tab });
  assert.match(read.facts.text, /Recovery fixture ready/);
  const probe = await RelayMcp.open({ path: installedHome });
  const cutOff = probe.reply.templates[templateKind.cut_off]["tools/call"].result.content;
  probe.close();
  started = Date.now();
  activePhase = "in-flight-browser-effect-is-not-replayed";
  const interrupted = request("tools/call", { name: "browser_execute", arguments: { tab: opened.facts.tab,
    script: "await fetch('/effect', {method:'POST'}); 'effect acknowledged'", timeout_ms: 30000 } });
  // Observe the side effect independently at the fixture server, before dropping its receipt.
  await until(() => effectCount === 1, "independently observed browser effect", 15000);
  stop(authority);
  const interruptedReply = await interrupted;
  assert.equal(interruptedReply.result?.isError, true, "A call cut off by a lost service is an error result");
  assert.deepEqual(interruptedReply.result.content, cutOff, "The cut-off call answers with the service's own words");
  heldResponse.end("recorded"); heldResponse = null;
  ({ replacement: authority, edge: native } = await recoverAfterServiceLoss(authority));
  await call("browser_tabs", { action: "list" });
  await delay(1500);
  assert.equal(effectCount, 1, "Uncertain browser effect must never replay");
  phase("in-flight-browser-effect-is-not-replayed", started, { effect_count: effectCount, result: "outcome_unavailable" });
  report.preserved_tab = opened.facts.tab;

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
        if (existsSync(item.path)) assert.equal(JSON.parse(readFileSync(item.path)).path, edgeImage, "Cleanup found a different installation");
        writeFileSync(item.path, item.bytes);
        assert.equal(hash(readFileSync(item.path)), item.sha256);
      }
    }
  } catch (error) { report.restoration_failure = error.message; report.passed = false; }
  heldResponse?.end("recorded");
  fixture.closeAllConnections(); fixture.close();
  mcp?.stdin.end();
  if (mcp && mcp.exitCode === null) { await delay(500); if (mcp.exitCode === null) mcp.kill(); }
  report.finished_at = new Date().toISOString(); save();
  console.log(`Evidence: ${join(output, "results.json")}`);
  if (!report.passed) process.exitCode = 1;
}
