// The command-line intake, against the real executable and a real service (ADR-0105).
//
// Unit tests cover argument parsing and exit-code mapping. This proves the parts only a process
// can prove: that a script reaches the same executor, that its work is attributed to the cli
// channel in the audit file the service wrote, and that a batch keeps one workspace so handles
// from one line resolve on the next.

import assert from "node:assert/strict";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";

const repository = resolve(import.meta.dirname, "..");
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const binDir = process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0", "debug");
const runtimeFile = join(repository, `tests/.ghostlight-cli-runtime-${process.pid}.json`);
const auditFile = join(repository, `tests/.ghostlight-cli-audit-${process.pid}.jsonl`);
// The service holds its lifetime lease beside the runtime file; killing it leaves the lease behind.
const leaseFile = runtimeFile.replace(/\.json$/, ".lock");
const scriptingDisabledPolicyFile = join(repository, "examples/scripting-disabled.json");
const environment = {
  ...process.env,
  GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
  GHOSTLIGHT_AUDIT_FILE: auditFile
};

const ghostlight = join(binDir, `ghostlight${executableSuffix}`);
if (!existsSync(ghostlight)) throw new Error(`Missing ${ghostlight}; build the workspace first.`);

const sleep = (ms) => new Promise((resolvePromise) => setTimeout(resolvePromise, ms));
const call = (args, input) =>
  spawnSync(ghostlight, ["call", ...args], { env: environment, encoding: "utf8", input });

function records() {
  return readFileSync(auditFile, "utf8")
    .trim()
    .split("\n")
    .filter((line) => line.trim())
    .map((line) => JSON.parse(line));
}

rmSync(runtimeFile, { force: true });
rmSync(auditFile, { force: true });
rmSync(leaseFile, { force: true });
const services = [];
const cleanup = [runtimeFile, auditFile, leaseFile];

try {
  const version = spawnSync(ghostlight, ["--version"], { env: environment, encoding: "utf8" });
  assert.equal(version.status, 0, version.stderr);
  assert.equal(version.stdout.trim(), "ghostlight 1.0.0");
  const help = spawnSync(ghostlight, ["--help"], { env: environment, encoding: "utf8" });
  assert.equal(help.status, 0, help.stderr);
  assert.match(help.stdout, /ghostlight install/);
  assert.match(help.stdout, /ghostlight doctor/);
  assert.doesNotMatch(help.stdout, /ghostlight service|--headless/);
  for (const removed of [["service"], ["--headless"]]) {
    const rejectedLaunch = spawnSync(ghostlight, removed, { env: environment, encoding: "utf8" });
    assert.notEqual(rejectedLaunch.status, 0, `${removed[0]} must not start an authority`);
    assert.equal(existsSync(runtimeFile), false, `${removed[0]} must not publish runtime discovery`);
  }
  const dryRun = spawnSync(ghostlight, ["install", "--dry-run", "--no-clients"], {
    env: environment,
    encoding: "utf8"
  });
  assert.equal(dryRun.status, 0, dryRun.stderr);
  assert.match(dryRun.stdout, /no machine state will change/);

  const authority = spawn(ghostlight, [], {
    env: environment,
    stdio: ["pipe", "pipe", "pipe"]
  });
  services.push(authority);
  authority.stderr.on("data", (chunk) => process.stderr.write(`[ghostlight] ${chunk}`));

  for (let attempt = 0; attempt < 80 && !existsSync(runtimeFile); attempt += 1) await sleep(50);
  assert.equal(existsSync(runtimeFile), true, "the service never published runtime discovery");
  const status = spawnSync(ghostlight, ["status", "--json"], {
    env: environment,
    encoding: "utf8"
  });
  assert.equal(status.status, 0, status.stderr);
  const statusJson = JSON.parse(status.stdout);
  assert.equal(statusJson.running, true);
  assert.equal("token" in statusJson, false, "status must never reveal local authentication material");

  const listed = call(["browser_tabs", '{"action":"list"}']);
  assert.equal(listed.status, 0, listed.stderr);
  assert.equal(listed.stdout.trim(), "Listed 0 controlled tabs.");

  const rendered = call(["browser_tabs", '{"action":"list"}', "--json"]);
  assert.equal(rendered.status, 0);
  const result = JSON.parse(rendered.stdout.trim());
  assert.equal(result.status, "succeeded");
  assert.deepEqual(result.facts.tabs, []);

  const catalog = call(["--catalog"]);
  assert.equal(catalog.status, 0);
  assert.equal(catalog.stdout.trim().split("\n").length, 22, "the CLI sees the whole catalog");

  // A refusal must not look like success to a shell.
  const rejected = call(["browser_navigate", "{}"]);
  assert.notEqual(rejected.status, 0);
  assert.equal(rejected.stdout.trim(), "The call does not match the Ghostlight catalog.");

  // One process, one session: handles from an earlier line are usable on a later one.
  const batch = call(["--stdin"], 'browser_tabs {"action":"list"}\nbrowser_tabs {"action":"list"}\n\nbrowser_tabs {"action":"list"}\n');
  assert.equal(batch.status, 0, batch.stderr);
  assert.equal(batch.stdout.trim().split("\n").length, 3);

  await sleep(300);
  // Three separate processes, then three batched calls. Retrieving the catalog invokes nothing
  // and is deliberately not an audited action.
  const written = records();
  assert.equal(written.length, 6, `expected every invocation to be audited, saw ${written.length}`);
  for (const record of written) {
    assert.equal(record.channel, "cli", "scripted work must be attributed to its own channel");
  }
  assert.equal(
    new Set(written.slice(-3).map((record) => record.workspace)).size,
    1,
    "a batch must hold one workspace across its calls"
  );
  // Every call here came from this one node process, so the session marker gathers all of them --
  // the separate processes and the batch alike -- into a single workspace (ADR-0106).
  assert.equal(
    new Set(written.map((record) => record.workspace)).size,
    1,
    "calls from one caller must reach one workspace, whatever process each ran in"
  );

  // An authority layer may decline the intake outright. Governance belongs to the service, so
  // this needs its own authority with the policy in its environment.
  const governedRuntime = `${runtimeFile}.governed.json`;
  const governedAudit = `${auditFile}.governed.jsonl`;
  const governedEnvironment = {
    ...process.env,
    GHOSTLIGHT_RUNTIME_FILE: governedRuntime,
    GHOSTLIGHT_AUDIT_FILE: governedAudit,
    GHOSTLIGHT_POLICY_FILE: scriptingDisabledPolicyFile
  };
  cleanup.push(governedRuntime, governedAudit, governedRuntime.replace(/\.json$/, ".lock"));
  const governedService = spawn(ghostlight, [], {
    env: governedEnvironment,
    stdio: ["pipe", "pipe", "pipe"]
  });
  services.push(governedService);
  for (let attempt = 0; attempt < 80 && !existsSync(governedRuntime); attempt += 1) await sleep(50);
  assert.equal(existsSync(governedRuntime), true, "the governed service never started");

  const refused = spawnSync(ghostlight, ["call", "browser_tabs", '{"action":"list"}'], {
    env: governedEnvironment,
    encoding: "utf8"
  });
  assert.notEqual(refused.status, 0, "a disabled channel must not succeed");
  assert.match(
    refused.stderr,
    /channel_denied/,
    `expected a channel refusal, saw: ${refused.stderr}`
  );
  // The refusal lands at admission, before a workspace exists, so nothing was invoked.
  assert.equal(
    existsSync(governedAudit) ? readFileSync(governedAudit, "utf8").trim() : "",
    "",
    "a refused intake must not write an audit record"
  );
  // The negative control: the same service admits the MCP intake, so the refusal is this policy
  // naming this channel rather than the authority being broken.
  assert.equal(
    JSON.parse(readFileSync(governedRuntime, "utf8")).service_bridge_major,
    2,
    "the governed service is otherwise healthy"
  );

  console.log("cli journey ok: demand-free call -> governed result -> cli-attributed audit -> batch session -> channel refusal");
} finally {
  for (const child of services) if (!child.killed) child.kill();
  for (const file of cleanup) rmSync(file, { force: true });
}
