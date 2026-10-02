// Real package replacement at fixed paths with isolated user state and synthetic native framing.
// This proves executable custody and connector continuity, not an installed Chrome journey.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";

const root = resolve(import.meta.dirname, "..");
const binaries = resolve(process.env.GHOSTLIGHT_BIN_DIR || join(root, ".target-ghostlight-1.0/debug"));
const suffix = process.platform === "win32" ? ".exe" : "";
const names = ["ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"];
mkdirSync(join(root, ".tmp"), { recursive: true });
const area = mkdtempSync(join(root, ".tmp/fixed-installation-"));
const user = join(area, "user"), control = join(user, ".ghostlight"), destination = join(control, "bin");
const stage = join(area, "download");
mkdirSync(stage);
const environment = { ...process.env, USERPROFILE: user, HOME: user,
  APPDATA: join(user, "roaming"), LOCALAPPDATA: join(user, "local"), CODEX_HOME: join(user, "codex"),
  XDG_CONFIG_HOME: join(user, "config"), XDG_STATE_HOME: join(user, "state"),
  WEBVIEW2_USER_DATA_FOLDER: join(area, "webview"),
  GHOSTLIGHT_NATIVE_HOST_DIR: join(area, "native-host"), GHOSTLIGHT_POLICY_FILE: join(area, "policy.json"),
  GHOSTLIGHT_AUDIT_FILE: join(area, "audit.jsonl"), GHOSTLIGHT_DIAGNOSTICS_DIR: join(area, "diagnostics") };
delete environment.GHOSTLIGHT_RUNTIME_FILE;
writeFileSync(environment.GHOSTLIGHT_POLICY_FILE, JSON.stringify({ schema: 3, name: "Isolated package test", version: "1",
  grants: [{ id: "all", hosts: { allow: ["*"] }, allowed: ["read", "action", "write", "execute"] }],
  config: [{ key: "browser.startup", value: "manual", level: "mandatory" }] }));
for (const name of names) copyFileSync(join(binaries, name + suffix), join(stage, name + suffix));
const children = [];
const delay = ms => new Promise(done => setTimeout(done, ms));
async function until(predicate, label, timeout = 35000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = predicate(); if (value) return value; await delay(25); }
  throw new Error(`Timed out: ${label}\n${JSON.stringify(children.map(child => ({ executable: child.spawnfile,
    pid: child.pid, exit: child.exitCode, signal: child.signalCode, error: child.failure?.message, log: child.output })))}`);
}
function start(path, args = []) {
  const child = spawn(path, args, { env: environment, windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
  child.output = "";
  child.stderr.on("data", data => { child.output += data; });
  child.on("error", error => { child.failure = error; });
  children.push(child); return child;
}
async function command(path, args) {
  const child = start(path, args);
  child.stdin.end();
  await until(() => child.exitCode !== null || child.signalCode !== null || child.failure, args.join(" "));
  assert.equal(child.exitCode, 0, child.output || child.failure?.message);
}
const installed = name => join(destination, name + suffix);
const candidate = join(stage, "ghostlight" + suffix);
const runtimeFile = join(control, "ghostlight-runtime.json");
function runtime() { try { return JSON.parse(readFileSync(runtimeFile, "utf8")); } catch { return null; } }
function nativeFrame(value) {
  const body = Buffer.from(JSON.stringify(value)), header = Buffer.alloc(4);
  header.writeUInt32LE(body.length); return Buffer.concat([header, body]);
}
let native, mcp;
const epochs = [], hashes = [];
try {
  // Begin with the old package layout and owned routes, then migrate exactly once.
  const legacy = join(destination, "v1.3.11");
  mkdirSync(legacy, { recursive: true });
  for (const name of names) copyFileSync(join(stage, name + suffix), join(legacy, name + suffix));
  const legacyService = join(legacy, "ghostlight" + suffix);
  await command(legacyService, ["deployment", "release"]);
  await command(legacyService, ["native-host", "install"]);
  const retained = Buffer.from('retained installation history\n');
  writeFileSync(join(control, "audit.jsonl"), retained);
  mkdirSync(environment.CODEX_HOME, { recursive: true });
  const codexConfig = join(environment.CODEX_HOME, "config.toml");
  writeFileSync(codexConfig, `# preserve caller notes\n[mcp_servers.ghostlight]\ncommand = ${JSON.stringify(join(legacy, "ghostlight-mcp-connector" + suffix))}\nargs = []\n`);
  await command(candidate, ["deployment", "install", destination]);
  assert.equal(existsSync(legacy), false, "the obsolete release directory is retired");
  assert.deepEqual(readFileSync(join(control, "audit.jsonl")), retained);
  assert.match(readFileSync(codexConfig, "utf8"), /preserve caller notes/);
  const config = readFileSync(codexConfig, "utf8");
  assert.equal(config.includes("v1.3.11"), false, config);
  assert.match(config, /ghostlight-mcp-connector/);
  await command(installed("ghostlight"), ["native-host", "install"]);
  const registration = () => execFileSync(installed("ghostlight"), ["native-host", "check"],
    { env: environment, windowsHide: true, encoding: "utf8" });
  const registrationBefore = registration();
  const service = start(installed("ghostlight"));
  const firstRuntime = await until(runtime, "initial authority");
  native = start(installed("ghostlight-browser-connector"));
  let buffered = Buffer.alloc(0);
  native.stdout.on("data", chunk => {
    buffered = Buffer.concat([buffered, chunk]);
    while (buffered.length >= 4 && buffered.length >= buffered.readUInt32LE(0) + 4) {
      const length = buffered.readUInt32LE(0), value = JSON.parse(buffered.subarray(4, length + 4));
      buffered = buffered.subarray(length + 4);
      if (value.kind === "hello_accepted") epochs.push(value.service_epoch);
      if (value.kind === "heartbeat") native.stdin.write(nativeFrame({ kind: "heartbeat_ack", sequence: value.sequence }));
      if (value.kind === "request" && value.request.command.command === "install_page_runtime") {
        const { revision, sha256, script } = value.request.command;
        assert.equal(createHash("sha256").update(script).digest("hex"), sha256);
        hashes.push(sha256);
        native.stdin.write(nativeFrame({ kind: "receipt", receipt: { correlation: value.request.correlation,
          result: { outcome: "page_runtime_installed", revision, sha256 } } }));
      }
    }
  });
  native.stdin.write(nativeFrame({ kind: "hello", major: 3, adapter_version: "unchanged injector",
    browser_id: "browser_fixed_install", adapter_epoch: "adapter_fixed_install",
    capabilities: ["page_runtime", "adapter_liveness", "browser_attention"].map(name => ({ name, revision: 1 })) }));
  await until(() => hashes.length === 1, "first runtime acknowledgement");
  mcp = start(installed("ghostlight-mcp-connector"));
  const replies = new Map();
  createInterface({ input: mcp.stdout }).on("line", line => {
    const reply = JSON.parse(line); if (reply.id !== undefined) replies.set(reply.id, reply);
  });
  mcp.stdin.write(JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: {
    protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "fixed install", version: "1" } } }) + "\n");
  assert.ok((await until(() => replies.get(1), "MCP initialization")).result);
  mcp.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
  const peerIds = [native.pid, mcp.pid];
  // PE/ELF allow trailing data. Change only the service image without rebuilding the shores.
  appendFileSync(candidate, Buffer.from("\nfixed-installation-service-update\n"));
  await command(candidate, ["deployment", "install", destination]);
  await until(() => service.exitCode !== null || service.signalCode !== null, "old exact authority exits");
  await until(() => runtime()?.token !== firstRuntime.token && hashes.length >= 2, "same connector installs runtime after replacement");
  assert.equal(native.exitCode, null, native.output);
  assert.equal(mcp.exitCode, null, mcp.output);
  assert.deepEqual([native.pid, mcp.pid], peerIds);
  assert.notEqual(epochs.at(-1), epochs[0]);
  assert.equal(registration(), registrationBefore, "native registration remains fixed");
  mcp.stdin.write('{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n');
  assert.equal((await until(() => replies.get(2), "same MCP stream after update")).result.tools.length, 24);
  assert.deepEqual(readdirSync(destination).sort(), names.map(name => name + suffix).sort());
  assert.equal(existsSync(join(control, "deploy.lock")), false);
  console.log("PASS fixed installation: old owned routes migrated; history retained; real locked service replacement; same MCP/native processes; runtime reacknowledged; registration unchanged; no release directory or temporary leftovers");
} finally {
  if (existsSync(installed("ghostlight"))) await command(installed("ghostlight"), ["native-host", "uninstall"]);
  // Keep demand-start quiesced while retiring only this fixture's exact installed images.
  mkdirSync(control, { recursive: true });
  writeFileSync(join(control, "deploy.lock"), "fixture cleanup");
  for (const child of children) if (child.exitCode === null && child.signalCode === null) child.kill();
  if (process.platform === "win32") {
    const cleanup = join(area, "cleanup.ps1");
    writeFileSync(cleanup, `param([string]$Directory)
$images = @('ghostlight.exe','ghostlight-mcp-connector.exe','ghostlight-browser-connector.exe') | ForEach-Object { [IO.Path]::GetFullPath((Join-Path $Directory $_)) }
Get-Process | ForEach-Object { $candidateProcess = $_; try { if ($images -contains $candidateProcess.Path) { Stop-Process -Id $candidateProcess.Id -ErrorAction SilentlyContinue; Wait-Process -Id $candidateProcess.Id -Timeout 10 -ErrorAction SilentlyContinue } } catch {} }
`);
    execFileSync("pwsh", ["-NoProfile", "-File", cleanup, destination], { windowsHide: true });
  } else {
    for (const pid of readdirSync("/proc").filter(name => /^\d+$/.test(name))) {
      try {
        const { readlinkSync } = await import("node:fs");
        if (names.map(installed).includes(readlinkSync(`/proc/${pid}/exe`))) process.kill(Number(pid));
      } catch { /* already exited or foreign process */ }
    }
  }
  await delay(500);
  assert.equal(dirname(area), join(root, ".tmp"));
  rmSync(area, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}
