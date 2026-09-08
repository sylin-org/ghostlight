// Actual desktop authority and CLI intake, with preserved on-disk history across restarts.
// Optional predecessor is an already verified published executable; this runner never downloads.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const suffix = process.platform === "win32" ? ".exe" : "";
const current = join(resolve(process.env.GHOSTLIGHT_BIN_DIR || ".target-ghostlight-1.0/debug"), `ghostlight${suffix}`);
const previous = process.env.GHOSTLIGHT_PREVIOUS_EXECUTABLE && resolve(process.env.GHOSTLIGHT_PREVIOUS_EXECUTABLE);
mkdirSync(join(root, ".tmp/history-compatibility"), { recursive: true });
const area = mkdtempSync(join(root, ".tmp/history-compatibility/run-"));
const audit = join(area, "audit.jsonl");
const policy = join(area, "policy.json");
const controls = join(area, "controls.txt");
writeFileSync(controls, "active");
writeFileSync(policy, JSON.stringify({ schema: 3, name: "History compatibility", version: "1",
  grants: [{ id: "ordinary-work", hosts: { allow: ["*"] }, allowed: ["read", "action", "write", "execute"] }],
  config: [{ key: "browser.startup", value: "manual", level: "mandatory" }] }));
const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const report = { passed: false, release_ready: false, binaries: [], phases: [],
  scope: "Real authority/CLI persistence; no browser or installed-package acceptance",
  predecessor_exercised: Boolean(previous), source_test_sha256: digest(readFileSync(new URL(import.meta.url))) };
const save = () => writeFileSync(join(area, "results.json"), JSON.stringify(report, null, 2) + "\n");
const sleep = ms => new Promise(done => setTimeout(done, ms));
let active;
function run(executable, args, environment) {
  const result = spawnSync(executable, args, { env: environment, windowsHide: true, encoding: "utf8", timeout: 20000 });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return result.stdout;
}
async function stop() {
  if (!active) return;
  const { child, runtime } = active;
  if (child.pid && child.exitCode === null && child.signalCode === null) {
    const exited = new Promise(done => child.once("exit", done));
    child.kill();
    let timer;
    try { await Promise.race([exited, new Promise(done => { timer = setTimeout(done, 10000); })]); }
    finally { clearTimeout(timer); }
    assert.ok(child.exitCode !== null || child.signalCode !== null, "Test authority did not exit");
  }
  // Exact uniquely-created files only; original audit and logs remain as evidence.
  rmSync(runtime, { force: true });
  rmSync(runtime.replace(/\.json$/, ".lock"), { force: true });
  active = undefined;
}
async function exercise(executable, name, expectedUnreadable) {
  await stop();
  const before = existsSync(audit) ? readFileSync(audit) : Buffer.alloc(0);
  const runtime = join(dirname(executable), `.history-compatibility-${process.pid}-${name}.json`);
  assert.equal(existsSync(runtime), false, "Refusing existing runtime file");
  assert.equal(existsSync(runtime.replace(/\.json$/, ".lock")), false, "Refusing existing lease");
  const environment = { ...process.env, GHOSTLIGHT_RUNTIME_FILE: runtime, GHOSTLIGHT_AUDIT_FILE: audit,
    GHOSTLIGHT_NATIVE_HOST_DIR: join(area, "native"), GHOSTLIGHT_POLICY_FILE: policy,
    GHOSTLIGHT_RUNTIME_CONTROL_FILE: controls };
  const log = [];
  const child = spawn(executable, [], { env: environment, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  active = { child, runtime };
  let failure;
  child.on("error", error => { failure = error; });
  child.stdout.on("data", bytes => log.push(bytes));
  child.stderr.on("data", bytes => log.push(bytes));
  try {
    const deadline = Date.now() + 20000;
    while (!existsSync(runtime) && !failure && child.exitCode === null && Date.now() < deadline) await sleep(50);
    assert.equal(failure, undefined);
    assert.equal(existsSync(runtime), true, `Authority failed to start: ${name}`);
    const outcome = JSON.parse(run(executable, ["call", "policy_explain", "{}", "--json"], environment));
    assert.equal(outcome.status, "succeeded");
    if (expectedUnreadable !== undefined) {
      assert.equal(outcome.facts.audit_health.unreadable_entries, expectedUnreadable);
      assert.equal(outcome.facts.audit_health.history_unavailable, false);
      assert.equal(outcome.facts.audit_health.failure, null);
      assert.equal(outcome.history_storage, "saved");
    }
    const after = readFileSync(audit);
    assert.deepEqual(after.subarray(0, before.length), before, "Reading or saving changed original history");
    const appended = after.subarray(before.length).toString("utf8").split("\n").filter(line => line.trim()).map(JSON.parse);
    assert.ok(appended.some(record => record.invocation === outcome.invocation), "New work was not saved");
    assert.ok(!JSON.stringify(outcome).includes("PRIVATE_FUTURE_FIELD"));
    report.phases.push({ name, passed: true, preserved_bytes: before.length, new_receipts: appended.length });
    save();
    console.log(`PASS history compatibility: ${name}`);
    return appended.find(record => record.invocation === outcome.invocation);
  } finally {
    await stop();
    writeFileSync(join(area, `${name}.log`), Buffer.concat(log));
  }
}
try {
  for (const executable of new Set([current, previous].filter(Boolean))) {
    report.binaries.push({ executable, sha256: digest(readFileSync(executable)),
      version: run(executable, ["--version"], process.env).trim() });
  }
  save();
  if (previous) await exercise(previous, "published-predecessor");
  const receipt = await exercise(current, previous ? "upgrade-with-predecessor-history" : "current-writer", 0);
  await exercise(current, "current-restart", 0);
  // No future binary exists: deliberately changed records test format tolerance separately.
  const extended = { ...receipt, invocation: "future_additive", future_optional: { text: "PRIVATE_FUTURE_FIELD" } };
  const unsupported = { ...receipt, invocation: "future_required", reason: "future_decision" };
  const original = readFileSync(audit);
  writeFileSync(audit, Buffer.concat([original, Buffer.from([
    JSON.stringify(extended), JSON.stringify(unsupported), JSON.stringify(receipt), ""
  ].join("\n"))]));
  await exercise(current, "future-records-preserve-startup-and-saving", 1);
  await exercise(current, "repeated-restart-keeps-original-history", 1);
  report.passed = true;
} catch (error) {
  report.failure = error.message;
  process.exitCode = 1;
  console.error(error);
} finally {
  await stop();
  save();
  console.log(`Evidence: ${area}`);
}
