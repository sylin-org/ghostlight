// Real Linux executable boundaries: failed desktop startup, contention and later recovery.
// This isolated process lane never changes installed registration or a browser profile.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, readlinkSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";

assert.equal(process.platform, "linux", "This process-observation lane requires Linux");
const repository = resolve(import.meta.dirname, "../..");
const binDir = resolve(process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0/debug"));
const before = process.argv.includes("--record-before");
const output = join(repository, ".tmp/linux-startup-recovery", before ? "before" : "after");
mkdirSync(output, { recursive: true });
const runtime = join(binDir, `.startup-recovery-${process.pid}.json`);
const diagnostics = join(output, `diagnostics-${process.pid}`);
const deployLock = join(binDir, "deploy.lock");
assert.ok(!existsSync(deployLock), "Run this lane separately from a deployment or process journey");
const authority = realpathSync(join(binDir, "ghostlight"));
const connector = join(binDir, "ghostlight-mcp-connector");
const environment = {
  ...process.env,
  GHOSTLIGHT_RUNTIME_FILE: runtime,
  GHOSTLIGHT_AUDIT_FILE: join(output, `audit-${process.pid}.jsonl`),
  GHOSTLIGHT_NATIVE_HOST_DIR: join(output, `native-host-${process.pid}`),
  GHOSTLIGHT_DIAGNOSTICS_DIR: diagnostics,
};
// Precisely the SDK-style missing desktop context that previously failed GTK initialization.
const sanitized = Object.fromEntries(Object.entries(environment).filter(([key]) =>
  ["HOME", "PATH", "USER", "LOGNAME", "SHELL", "TERM"].includes(key) || key.startsWith("GHOSTLIGHT_")));
const children = [];
const authorities = new Map();
const report = {
  passed: false, record_before: before, bin_dir: binDir,
  hashes: Object.fromEntries(["ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"].map(name =>
    [name, createHash("sha256").update(readFileSync(join(binDir, name))).digest("hex")])),
  callers: 4, failed_phase_peak: 0, failed_phase_unique: 0,
  failed_phase_runtime_seen: false, failed_phase_initialized: 0, deployment_interference: false,
};
const sleep = ms => new Promise(resolvePromise => setTimeout(resolvePromise, ms));
const save = () => writeFileSync(join(output, "result.json"), JSON.stringify(report, null, 2) + "\n");

function processFacts(pid) {
  try {
    const path = `/proc/${pid}`;
    const stat = readFileSync(`${path}/stat`, "utf8");
    const fields = stat.slice(stat.lastIndexOf(")") + 2).split(" ");
    let image;
    try { image = readlinkSync(`${path}/exe`); } catch {
      const known = authorities.get(Number(pid));
      if (fields[0] !== "Z" || known?.started !== fields[19]) return null;
      image = known.image;
    }
    return { pid: Number(pid), parent: Number(fields[1]), state: fields[0], started: fields[19], image };
  } catch { return null; }
}

function sample() {
  const live = [];
  for (const pid of readdirSync("/proc").filter(value => /^\d+$/.test(value))) {
    const facts = processFacts(pid);
    if (!facts || facts.image !== authority) continue;
    if (!children.some(child => child.pid === facts.parent) && !authorities.has(facts.pid)) continue;
    authorities.set(facts.pid, facts);
    live.push(facts);
  }
  return live;
}

function client(env) {
  const child = spawn(connector, [], { env, stdio: ["pipe", "pipe", "pipe"] });
  children.push(child);
  child.initialized = false;
  child.stderr.on("data", () => {}); // Product diagnostics, not arbitrary child stderr, is retained.
  child.stdin.on("error", () => {});
  createInterface({ input: child.stdout }).on("line", line => {
    try { if (JSON.parse(line).id === 1 && JSON.parse(line).result) child.initialized = true; } catch { /* not a result */ }
  });
  child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: {
    protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "startup-recovery-proof", version: "1" },
  } }) + "\n");
  return child;
}

async function stopClient(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = new Promise(resolvePromise => child.once("exit", resolvePromise));
  child.kill("SIGTERM");
  await Promise.race([exited, sleep(2000)]);
  if (child.exitCode === null && child.signalCode === null) { child.kill("SIGKILL"); await exited; }
}

function diagnosticEvents() {
  if (!existsSync(diagnostics)) return [];
  return readdirSync(diagnostics).filter(name => name.endsWith(".jsonl")).flatMap(name =>
    readFileSync(join(diagnostics, name), "utf8").split("\n").flatMap(line => {
      try { const value = JSON.parse(line); return value.event ? [value.event] : []; } catch { return []; }
    }));
}

try {
  const failing = Array.from({ length: report.callers }, () => client(sanitized));
  const deadline = Date.now() + (before ? 4000 : 11500);
  while (Date.now() < deadline) {
    report.failed_phase_peak = Math.max(report.failed_phase_peak, sample().length);
    report.failed_phase_runtime_seen ||= existsSync(runtime);
    report.deployment_interference ||= existsSync(deployLock);
    await sleep(10);
  }
  report.failed_phase_unique = authorities.size;
  report.failed_phase_initialized = failing.filter(child => child.initialized).length;
  const events = diagnosticEvents();
  report.failed_phase_publications = events.filter(event => event === "runtime_published").length;
  report.failed_phase_diagnostic_failures = events.filter(event => event === "process_failed").length;
  await Promise.all(failing.map(stopClient));
  sample(); save();
  if (!before) {
    assert.equal(report.deployment_interference, false, "another journey quiesced startup during this run");
    assert.ok(report.failed_phase_peak <= 1, "failed concurrent launches must not accumulate authorities");
    assert.ok(report.failed_phase_unique >= 2 && report.failed_phase_unique <= 3, "shared cooldown bounds retries without disabling recovery");
    assert.equal(report.failed_phase_runtime_seen, false, "failed GUI startup must not publish discovery");
    assert.equal(report.failed_phase_publications, 0);
    assert.equal(report.failed_phase_initialized, 0, "no MCP metadata may be advertised before desktop readiness");
    assert.ok(report.failed_phase_diagnostic_failures >= 2, "native startup failures must reach persistent diagnostics");
    const recovered = Array.from({ length: report.callers }, () => client(environment));
    const start = Date.now();
    let live = [];
    while (Date.now() - start < 15000 && !recovered.every(child => child.initialized)) { live = sample(); await sleep(20); }
    report.recovery_ms = Date.now() - start;
    assert.ok(recovered.every(child => child.initialized), "corrected desktop context must recover within the bounded wait");
    live = sample();
    assert.equal(live.length, 1, "corrected concurrent clients converge on one authority");
    report.recovered_authority = live[0].pid;
    report.recovered_clients = recovered.length;
    await sleep(500);
    assert.equal(sample().length, 1, "ready authority survives native event-loop startup");
  }
  report.passed = !before; save();
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  report.error = error.message; save(); throw error;
} finally {
  sample();
  await Promise.all(children.map(stopClient));
  for (const facts of authorities.values()) {
    const current = processFacts(facts.pid);
    if (current?.image === facts.image && current.started === facts.started) {
      try { process.kill(facts.pid, "SIGTERM"); } catch { /* already exited */ }
    }
  }
  await sleep(300);
  for (const path of [runtime, runtime.replace(/\.json$/, ".lock"), runtime.replace(/\.json$/, ".startup")]) rmSync(path, { force: true });
}
