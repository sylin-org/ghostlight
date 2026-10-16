// The PowerShell deliverable, against the real edge and service and a scripted browser.
//
// scripts/browser-journey.ps1 is meant to run on a real machine with a real Chromium. This harness
// stands up the same process topology in a test home with a synthetic protocol 4 browser in the
// browser's place, so the script itself -- its session handling, its handle passing, its
// screenshot file, the exit code a script sees -- is proved rather than assumed.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { spawn } from "node:child_process";
import { createHome, openPolicy, repository, startReason } from "./lib/home.mjs";
import { ONE_PIXEL_JPEG, SyntheticBrowser, World } from "./lib/peers.mjs";

const DENIED_HOST = "blocked.localhost";
const PAGE_TEXT = "Example Domain. This domain is for use in documentation.";
const home = createHome({ policy: openPolicy([DENIED_HOST]), label: "ps" });
const shotFile = join(home.area, "shot.jpg");
const shell = process.platform === "win32" ? "pwsh.exe" : "pwsh";

/** A synthetic browser whose pages hold a longer text, so the audit can be checked for page content. */
class PageBrowser extends SyntheticBrowser {
  callAnswers(args, tab) {
    return super.callAnswers(args, tab).map((answer) => {
      const value = answer.envelope?.value;
      return typeof value?.text === "string" && value.text !== ""
        ? { ...answer, envelope: { ok: true, value: { ...value, text: PAGE_TEXT } } }
        : answer;
    });
  }
}

/**
 * Run a script under pwsh in the test home. It is never spawnSync: the synthetic browser answers on
 * this event loop, and a synchronous child would block it, so every browser call would dispatch and
 * then time out as unknown.
 */
function powershell(script, args) {
  return new Promise((done) => {
    const child = spawn(shell, ["-NoProfile", "-File", join(repository, "scripts", script), ...args], {
      env: home.environment,
      windowsHide: true
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("close", (status) => done({ status, stdout, stderr }));
  });
}

try {
  home.start(home.service, ["serve", "--reason", startReason.agent], { stdio: ["ignore", "ignore", "pipe"] });
  await home.serviceUp();
  const world = new World();
  const browser = PageBrowser.viaEdge(home, world);
  await browser.connect();

  const journey = await powershell("browser-journey.ps1", ["-Ghostlight", home.edge, "-OutputPath", shotFile]);
  process.stdout.write(journey.stdout);
  if (journey.status !== 0) process.stderr.write(journey.stderr);
  assert.equal(journey.status, 0, "the PowerShell journey must exit zero when every step succeeds");

  for (const expected of [
    "Opened example.com.",
    "Read 9 words from example.com.",
    "Closed the controlled tab."
  ]) {
    assert.ok(journey.stdout.includes(expected), `the journey never reported: ${expected}`);
  }
  assert.match(journey.stdout, /Captured the viewport at \d+x\d+\./);

  assert.equal(existsSync(shotFile), true, "the journey wrote no screenshot");
  const image = readFileSync(shotFile);
  assert.equal(image[0], 0xff, "the screenshot is not JPEG bytes");
  assert.equal(image[1], 0xd8, "the screenshot is not JPEG bytes");
  assert.deepEqual(image, Buffer.from(ONE_PIXEL_JPEG, "base64"), "the CLI saves the exact delivered screenshot bytes");
  const reads = browser.requested("call").filter((request) => request.primitive.args.calls.some((entry) => entry.message.kind === "read_text"));
  assert.equal(reads.length, 1, "the read physically executes once");
  assert.equal(browser.requested("capture").length, 1, "the capture physically executes once");

  const records = home.auditRecords();
  const tools = records.map((record) => record.tool);
  for (const tool of ["browser_navigate", "browser_tabs", "browser_read", "browser_screenshot"]) {
    assert.ok(tools.includes(tool), `${tool} never reached the executor`);
  }
  for (const record of records) {
    assert.equal(record.channel, "cli", "a scripted step was not attributed to the cli channel");
    assert.equal(record.allowed, true, `${record.tool} was refused: ${record.reason}`);
    assert.equal(record.status, "succeeded", `${record.tool} did not complete: ${record.summary}`);
  }
  // The audit stays payload-free even when a script drove the work.
  const encoded = JSON.stringify(records);
  assert.equal(encoded.includes("This domain is for use"), false, "page text reached the audit");
  assert.ok(encoded.includes('"host":"example.com"'), "the landing host was not recorded");
  // Every step was its own process, so one workspace across all of them is the session marker
  // working (ADR-0106).
  assert.equal(new Set(records.map((record) => record.workspace)).size, 1, "steps from one caller must share a workspace, whatever process each ran in");
  console.log("PASS the script opens, lists, reads, captures and closes as the cli channel in one workspace");

  // A refused step is the exit code a script sees: the script stops and reports it.
  const opens = browser.requested("open_tab").length;
  const refused = await powershell("browser-journey.ps1", ["-Ghostlight", home.edge, "-OutputPath", shotFile, "-Url", `http://${DENIED_HOST}:3000/`]);
  assert.notEqual(refused.status, 0, "a refused step must end the script with a nonzero exit code");
  assert.match(`${refused.stdout}\n${refused.stderr}`, /blocked/i);
  assert.equal(browser.requested("open_tab").length, opens, "a refused navigation never reaches the browser");
  console.log("PASS a refused step ends the script with a nonzero exit code");

  const demoFailure = await powershell("demo-foundry.ps1", [
    // Node treats Ghostlight's first `call` argument as a missing script, exits nonzero, and writes
    // no JSON. That gives the demo a portable real-process transport failure.
    "-Ghostlight", process.execPath,
    "-Beat", "0"
  ]);
  const failureOutput = `${demoFailure.stdout}\n${demoFailure.stderr}`;
  assert.notEqual(demoFailure.status, 0, "the demo must fail when Ghostlight returns no result");
  assert.ok(/open did not return a JSON result \(exit [1-9][0-9]*\)/.test(failureOutput), `the demo hid its transport failure: ${failureOutput}`);
  assert.equal(failureOutput.includes("The property 'status' cannot be found"), false, "the demo regressed to dereferencing an absent result");

  browser.close();
  console.log("\npowershell journey ok: separate processes, one session, open/list/read/capture/close, refusal exit code");
} finally {
  await home.cleanup();
}
