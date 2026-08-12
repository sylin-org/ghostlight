// The Agent Plugins declaration, against an installer-shaped copy of the real native stack.
//
// The contract test owns static manifest validation. This journey proves the part a schema cannot:
// a client can resolve the declared bare command, reach the separately installed Ghostlight
// authority, and receive the exact orchestrator-owned catalog without a browser connection.

import assert from "node:assert/strict";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createInterface } from "node:readline";

const EXPECTED_TOOL_NAMES = [
  "browser_tabs",
  "browser_navigate",
  "browser_history",
  "browser_window",
  "browser_read",
  "browser_inspect",
  "browser_find",
  "browser_screenshot",
  "browser_click",
  "browser_scroll",
  "browser_hover",
  "browser_fill_form",
  "browser_type_text",
  "browser_press_key",
  "browser_drag",
  "browser_wait",
  "browser_dialog",
  "browser_upload",
  "browser_execute",
  "browser_sequence",
  "browser_record",
  "browser_diagnose"
];
const JOURNEY_PREFIX = "ghostlight-agent-plugin-";
const repository = resolve(import.meta.dirname, "..");
const pluginRoot = realpathSync.native(repository);
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const binDir = process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0", "debug");
const manifest = JSON.parse(readFileSync(join(repository, "mcp.json"), "utf8"));

assert.deepEqual(Object.keys(manifest.mcpServers), ["ghostlight"]);
const declaration = manifest.mcpServers.ghostlight;
assert.equal(declaration.type, "stdio");
assert.equal(declaration.command, "ghostlight-mcp-connector");
assert.deepEqual(declaration.args || [], []);

const journeyDirectory = mkdtempSync(join(tmpdir(), JOURNEY_PREFIX));
const installedBinDir = join(journeyDirectory, "installed");
const pluginDataDir = join(journeyDirectory, "plugin-data");
const runtimeFile = join(journeyDirectory, "ghostlight-runtime.json");
const auditFile = join(journeyDirectory, "ghostlight-audit.jsonl");
const processes = [];

function installedExecutable(name) {
  return join(installedBinDir, `${name}${executableSuffix}`);
}

function copyNativeStack() {
  mkdirSync(installedBinDir);
  for (const name of ["ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"]) {
    const source = join(binDir, `${name}${executableSuffix}`);
    if (!existsSync(source)) throw new Error(`Missing ${source}; build the workspace first.`);
    const destination = installedExecutable(name);
    copyFileSync(source, destination);
    if (process.platform !== "win32") chmodSync(destination, 0o755);
  }
}

function isolatedEnvironment() {
  const environment = {
    ...process.env,
    GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
    GHOSTLIGHT_AUDIT_FILE: auditFile
  };
  for (const name of Object.keys(environment)) {
    if (["path", "plugin_root", "plugin_data"].includes(name.toLowerCase())) delete environment[name];
  }
  environment.PATH = installedBinDir;
  environment.PLUGIN_ROOT = pluginRoot;
  environment.PLUGIN_DATA = realpathSync.native(pluginDataDir);
  return environment;
}

function start(command, args, environment, label) {
  const child = spawn(command, args, {
    cwd: pluginRoot,
    env: environment,
    stdio: ["pipe", "pipe", "pipe"],
    windowsHide: true
  });
  child.stderr.on("data", (chunk) => process.stderr.write(`[${label}] ${chunk}`));
  processes.push(child);
  return child;
}

function waitForExit(child, timeoutMs = 5000) {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((resolvePromise, reject) => {
    const timer = setTimeout(() => reject(new Error("Timed out waiting for child process exit")), timeoutMs);
    child.once("exit", () => {
      clearTimeout(timer);
      resolvePromise();
    });
  });
}

async function waitForFile(path, timeoutMs = 10000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (existsSync(path)) return;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 25));
  }
  throw new Error(`Timed out waiting for ${path}`);
}

function removeJourneyDirectory() {
  const resolvedJourney = realpathSync.native(journeyDirectory);
  const resolvedTemp = realpathSync.native(tmpdir());
  const samePath = (left, right) =>
    process.platform === "win32" ? left.toLowerCase() === right.toLowerCase() : left === right;
  assert.equal(
    samePath(dirname(resolvedJourney), resolvedTemp),
    true,
    `Refusing to remove journey directory outside ${resolvedTemp}`
  );
  assert.equal(
    basename(resolvedJourney).startsWith(JOURNEY_PREFIX),
    true,
    `Refusing to remove unexpected journey directory ${resolvedJourney}`
  );
  rmSync(resolvedJourney, { recursive: true, force: true });
}

class McpPeer {
  constructor(child) {
    this.child = child;
    this.nextId = 1;
    this.pending = new Map();
    createInterface({ input: child.stdout }).on("line", (line) => {
      const message = JSON.parse(line);
      const key = JSON.stringify(message.id);
      const pending = this.pending.get(key);
      if (!pending) return;
      this.pending.delete(key);
      clearTimeout(pending.timer);
      pending.resolve(message);
    });
    const failPending = (error) => {
      for (const pending of this.pending.values()) {
        clearTimeout(pending.timer);
        pending.reject(error);
      }
      this.pending.clear();
    };
    child.once("error", failPending);
    child.once("exit", (code, signal) => {
      failPending(new Error(`MCP connector exited code=${code} signal=${signal}`));
    });
  }

  request(method, params = {}) {
    const id = this.nextId++;
    const key = JSON.stringify(id);
    const promise = new Promise((resolvePromise, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(key);
        reject(new Error(`Timed out waiting for MCP ${method}`));
      }, 10000);
      this.pending.set(key, { resolve: resolvePromise, reject, timer });
    });
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    return promise;
  }

  notify(method, params = {}) {
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
  }
}

try {
  copyNativeStack();
  mkdirSync(pluginDataDir);
  const environment = isolatedEnvironment();
  const service = start(installedExecutable("ghostlight"), ["--headless"], environment, "ghostlight");
  await waitForFile(runtimeFile);
  assert.equal(service.exitCode, null, "the installed authority must remain active");
  const initialEndpoint = JSON.parse(readFileSync(runtimeFile, "utf8"));
  assert.equal(initialEndpoint.service_version, "1.0.0");

  // Use the manifest token, not an absolute path. PATH contains only the simulated signed stack.
  const connector = start(declaration.command, declaration.args || [], environment, declaration.command);
  const mcp = new McpPeer(connector);
  const initialized = await mcp.request("initialize", {
    protocolVersion: "2025-11-25",
    capabilities: {},
    clientInfo: { name: "agent-plugin-journey", version: "1" }
  });
  assert.equal(initialized.error, undefined, JSON.stringify(initialized.error));
  assert.deepEqual(initialized.result.serverInfo, { name: "ghostlight", version: "1.0.0" });
  assert.equal(initialized.result.protocolVersion, "2025-11-25");
  assert.equal(initialized.result.capabilities.tools.listChanged, true);
  mcp.notify("notifications/initialized");

  const listed = await mcp.request("tools/list");
  assert.equal(listed.error, undefined, JSON.stringify(listed.error));
  assert.deepEqual(
    listed.result.tools.map((tool) => tool.name),
    EXPECTED_TOOL_NAMES,
    "the plugin edge must expose the one current orchestrator catalog"
  );
  assert.equal(
    listed.result.tools.every((tool) => tool.outputSchema && tool.annotations),
    true,
    "every advertised tool must retain the canonical output schema and MCP annotations"
  );

  // A second stack process cannot become another authority for the same runtime. The live
  // connector remains usable, and runtime discovery still names the original authority.
  const contender = spawnSync(installedExecutable("ghostlight"), ["--headless"], {
    cwd: pluginRoot,
    env: environment,
    encoding: "utf8",
    timeout: 5000,
    windowsHide: true
  });
  assert.equal(contender.error, undefined, contender.error?.message);
  assert.notEqual(contender.status, 0, "a second authority must be rejected");
  assert.match(contender.stderr, /another Ghostlight orchestrator already owns this runtime/);
  assert.deepEqual(JSON.parse(readFileSync(runtimeFile, "utf8")), initialEndpoint);
  const pinged = await mcp.request("ping");
  assert.deepEqual(pinged.result, {});

  console.log("agent plugin journey ok: bare manifest command -> one installed authority -> exact catalog");
} finally {
  for (const child of processes.reverse()) {
    if (child.exitCode === null && child.signalCode === null) child.kill();
  }
  await Promise.all(processes.map((child) => waitForExit(child).catch(() => {})));
  removeJourneyDirectory();
}
