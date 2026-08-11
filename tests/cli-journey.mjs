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
const service = spawn(ghostlight, ["--headless"], { env: environment, stdio: ["pipe", "pipe", "pipe"] });
service.stderr.on("data", (chunk) => process.stderr.write(`[ghostlight] ${chunk}`));

try {
  for (let attempt = 0; attempt < 80 && !existsSync(runtimeFile); attempt += 1) await sleep(50);
  assert.equal(existsSync(runtimeFile), true, "the service never published runtime discovery");

  const listed = call(["browser_list_tabs"]);
  assert.equal(listed.status, 0, listed.stderr);
  assert.equal(listed.stdout.trim(), "Listed 0 controlled tabs.");

  const rendered = call(["browser_list_tabs", "{}", "--json"]);
  assert.equal(rendered.status, 0);
  const result = JSON.parse(rendered.stdout.trim());
  assert.equal(result.status, "succeeded");
  assert.deepEqual(result.facts.tabs, []);

  const catalog = call(["--catalog"]);
  assert.equal(catalog.status, 0);
  assert.equal(catalog.stdout.trim().split("\n").length, 24, "the CLI sees the whole catalog");

  // A refusal must not look like success to a shell.
  const rejected = call(["browser_open_page", "{}"]);
  assert.notEqual(rejected.status, 0);
  assert.equal(rejected.stdout.trim(), "The call does not match the Ghostlight catalog.");

  // One process, one session: handles from an earlier line are usable on a later one.
  const batch = call(["--stdin"], "browser_list_tabs\nbrowser_list_tabs {}\n\nbrowser_list_tabs\n");
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
  assert.equal(
    new Set(written.slice(0, 3).map((record) => record.workspace)).size,
    3,
    "separate processes are separate workspaces, which is why batch mode exists"
  );

  console.log("cli journey ok: demand-free call -> governed result -> cli-attributed audit -> batch session");
} finally {
  if (!service.killed) service.kill();
  rmSync(runtimeFile, { force: true });
  rmSync(auditFile, { force: true });
  rmSync(leaseFile, { force: true });
}
