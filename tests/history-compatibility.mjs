// Saved history across restarts, through the real service and the command line in a test home.
// The history is 2.0 lines only: lines written before the 2.0 break are archived by the break
// migration and never shown, so this journey writes its own and reads them back. It also appends
// lines from a later release and checks they neither hide readable history nor stop new work.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createHome } from "./lib/home.mjs";

const FUTURE_FIELD = "PRIVATE_FUTURE_FIELD";
const home = createHome({ label: "history" });

/** One `ghostlight call policy_explain` through the edge; it starts the service when none runs. */
function explain() {
  const result = home.run(home.edge, ["call", "policy_explain", "{}", "--json"], { timeout: 60_000 });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

/** One phase: a service starts, does work over the saved history, and quits. */
async function exercise(name, expectedUnreadable) {
  const before = existsSync(home.auditFile) ? readFileSync(home.auditFile) : Buffer.alloc(0);
  const outcome = explain();
  try {
    assert.equal(outcome.status, "succeeded");
    if (expectedUnreadable !== undefined) {
      assert.equal(outcome.facts.audit_health.unreadable_entries, expectedUnreadable);
      assert.equal(outcome.facts.audit_health.history_unavailable, false);
      assert.equal(outcome.facts.audit_health.failure, null);
    }
    const after = readFileSync(home.auditFile);
    assert.deepEqual(after.subarray(0, before.length), before, "reading or saving changed the original history");
    const appended = after.subarray(before.length).toString("utf8").split("\n").filter((line) => line.trim()).map(JSON.parse);
    assert.ok(appended.some((record) => record.invocation === outcome.invocation), "new work was not saved");
    assert.ok(!JSON.stringify(outcome).includes(FUTURE_FIELD));
    console.log(`PASS history compatibility: ${name}`);
    return appended.find((record) => record.invocation === outcome.invocation);
  } finally {
    await home.quit();
  }
}

try {
  const receipt = await exercise("current-writer", 0);
  await exercise("current-restart", 0);
  const extended = { ...receipt, invocation: "future_additive", future_optional: { text: FUTURE_FIELD } };
  const unsupported = { ...receipt, invocation: "future_required", reason: "future_decision" };
  const original = readFileSync(home.auditFile);
  writeFileSync(home.auditFile, Buffer.concat([original, Buffer.from([
    JSON.stringify(extended), JSON.stringify(unsupported), JSON.stringify(receipt), ""
  ].join("\n"))]));
  await exercise("future-records-preserve-startup-and-saving", 1);
  await exercise("repeated-restart-keeps-original-history", 1);
  console.log("history compatibility ok: 2.0 history survives restarts and later-release lines");
} finally {
  await home.cleanup();
}
