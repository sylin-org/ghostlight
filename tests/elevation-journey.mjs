// The refusal to run elevated, proved on a runner whose token is elevated.
//   - without the debug-build waiver, `serve` and an edge command that starts a service both
//     refuse with the administrator-rights sentence and leave no service behind;
//   - with GHOSTLIGHT_TEST_ALLOW_ELEVATED=1 the same debug build serves.
// GitHub's Windows runners run jobs elevated; a developer's regular terminal is not, and the
// journey then skips quietly.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHome, startReason } from "./lib/home.mjs";

/** The environment variable a debug build reads to let an elevated token run. */
const WAIVER = "GHOSTLIGHT_TEST_ALLOW_ELEVATED";
/** The sentence the refusal carries, in part. */
const REFUSAL_WORDS = "administrator rights";
/** The mandatory-level SIDs of an elevated token: High and System. */
const ELEVATED_LEVELS = ["S-1-16-12288", "S-1-16-16384"];

/** Whether this process runs with an elevated Windows token. */
function elevated() {
  if (process.platform !== "win32") return false;
  const groups = spawnSync("whoami", ["/groups"], { encoding: "utf8", windowsHide: true, timeout: 30_000 });
  return groups.status === 0 && ELEVATED_LEVELS.some((level) => groups.stdout.includes(level));
}

if (!elevated()) {
  console.log("SKIP not elevated");
  process.exit(0);
}

const home = createHome({ label: "elevation" });
const refusedEnvironment = { [WAIVER]: "" };

try {
  const served = home.run(home.service, ["serve", "--reason", startReason.agent], { env: refusedEnvironment });
  assert.notEqual(served.status, 0, "an elevated serve is refused");
  assert.ok(served.stderr.includes(REFUSAL_WORDS), `the refusal says why: ${served.stderr}`);
  assert.equal(await home.status(), null, "a refused serve leaves no service");
  console.log("PASS an elevated serve is refused and starts nothing");

  const called = home.run(home.edge, ["call", "--catalog"], { env: refusedEnvironment, timeout: 120_000 });
  assert.notEqual(called.status, 0, "an elevated command that would start a service is refused");
  assert.ok(called.stderr.includes(REFUSAL_WORDS), `the refusal says why: ${called.stderr}`);
  assert.equal(await home.status(), null, "a refused start leaves no service");
  console.log("PASS an elevated command that would start a service is refused");

  const service = home.start(home.service, ["serve", "--reason", startReason.agent], { env: { [WAIVER]: "1" } });
  const status = await home.serviceUp();
  assert.ok(status.pid > 0, "the waiver lets a debug build serve");
  assert.equal(service.exitCode, null, "the service stays up");
  console.log("PASS the debug-build waiver lets the journeys serve");
  console.log("elevation journey ok");
} finally {
  await home.cleanup();
}
