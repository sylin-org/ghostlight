// The command line through the real edge and a real service, in a test home (ADR-0105, ADR-0199).
//
// Unit tests cover argument parsing and exit-code mapping. This proves the parts only a process
// can prove: `status` and `doctor` print and return their exit codes without starting a service,
// `call` starts one with the start reason `command` when none runs, a script reaches the same
// executor, its work is attributed to the cli channel in the audit file, and a batch keeps one
// workspace so handles from one line resolve on the next. Every process runs in a fresh test home;
// nothing touches the real one.
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { buildVersion, createHome, repository, startReason, until } from "./lib/home.mjs";

const catalogNames = JSON.parse(readFileSync(join(repository, "docs", "1.0", "catalog.json"), "utf8")).map((tool) => tool.name);
const LIST = '{"action":"list"}';
const RECORDING_STATUS = '{"action":"status"}';
const EXIT_FAILED = 4;

// The machine's persistent native-host registration, as one comparable string. The journey
// snapshots it before its first process and asserts it is byte-identical after its last, so no
// beat can mutate the real registration unnoticed.
const REGISTRY_VENDORS = [["Google", "Chrome"], ["Microsoft", "Edge"], ["BraveSoftware", "Brave-Browser"], ["Chromium"]];
const LINUX_MANIFEST_DIRECTORIES = ["google-chrome", "microsoft-edge", "BraveSoftware/Brave-Browser", "chromium"];
function machineRegistration() {
  const parts = [];
  const readManifest = (path) => (existsSync(path) ? readFileSync(path, "utf8") : "<absent>");
  if (process.platform === "win32") {
    parts.push(readManifest(join(process.env.USERPROFILE || "", ".ghostlight", "manifests", "org.sylin.ghostlight.json")));
    for (const vendor of REGISTRY_VENDORS) {
      const key = `HKCU\\Software\\${vendor.join("\\")}\\NativeMessagingHosts\\org.sylin.ghostlight`;
      const query = spawnSync("reg", ["query", key, "/ve"], { encoding: "utf8" });
      parts.push(query.status === 0 ? (query.stdout || "").trim() : "<absent>");
    }
  } else {
    const configHome = process.env.XDG_CONFIG_HOME || join(process.env.HOME || "", ".config");
    for (const directory of LINUX_MANIFEST_DIRECTORIES) {
      parts.push(readManifest(join(configHome, directory, "NativeMessagingHosts", "org.sylin.ghostlight.json")));
    }
  }
  return parts.join("\n---\n");
}
const registrationBefore = machineRegistration();

// A manual startup policy lets one call ask for a browser without starting one, so the journey can
// read the no-browser language on the real machine's inventory.
const home = createHome({
  label: "cli",
  policy: {
    schema: 4,
    name: "cli fixture",
    version: "1",
    grants: [{ id: "sylin", hosts: { allow: ["sylin.org"] }, allowed: ["read", "action", "write", "execute"] }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" }]
  }
});
const version = buildVersion();
const edge = (args, input) => home.run(home.edge, args, { input, timeout: 120_000 });
const call = (args, input) => edge(["call", ...args], input);

try {
  // Nothing here may start a service.
  const reported = edge(["--version"]);
  assert.equal(reported.status, 0, reported.stderr);
  assert.equal(reported.stdout.trim(), `ghostlight ${version}`);
  const help = edge(["--help"]);
  assert.equal(help.status, 0, help.stderr);
  assert.match(help.stdout, /ghostlight install/);
  assert.match(help.stdout, /ghostlight doctor/);
  assert.doesNotMatch(help.stdout, /ghostlight service|--headless/);
  for (const removed of [["service"], ["--headless"]]) {
    const rejected = edge(removed);
    assert.notEqual(rejected.status, 0, `${removed[0]} must not start a service`);
  }
  const dryRun = edge(["install", "--dry-run", "--no-clients"]);
  assert.equal(dryRun.status, 0, dryRun.stderr);
  assert.match(dryRun.stdout, /no machine state will change/);
  // A client id that names nothing is refused before the install changes anything.
  const mistyped = edge(["install", "--client", "nope", "--no-open"]);
  assert.notEqual(mistyped.status, 0, "a mistyped client id is refused");
  assert.match(mistyped.stderr, /unknown MCP client 'nope'/);
  assert.ok(!existsSync(home.nativeHostDir) || readdirSync(home.nativeHostDir).length === 0,
    "no browser registration was written before the refusal");
  assert.ok(!existsSync(join(home.area, "user", ".local")),
    "no command link, completions or Applications entry was written before the refusal");

  const absent = edge(["status"]);
  assert.equal(absent.status, 0, absent.stderr);
  assert.match(absent.stdout, new RegExp(`^Ghostlight ${version.replaceAll(".", "\\.")}$`, "m"));
  assert.match(absent.stdout, /Ghostlight is not running\./);
  const absentJson = edge(["status", "--json"]);
  assert.equal(absentJson.status, 0, absentJson.stderr);
  assert.equal(JSON.parse(absentJson.stdout).running, false);
  const absentDoctor = edge(["doctor"]);
  assert.equal(absentDoctor.status, 0, absentDoctor.stderr);
  assert.match(absentDoctor.stdout, new RegExp(`Ghostlight ${version.replaceAll(".", "\\.")} diagnostics`));
  assert.match(absentDoctor.stdout, /Ghostlight is not running\./);
  const absentDiagnosis = edge(["doctor", "--json"]);
  assert.equal(absentDiagnosis.status, 0, absentDiagnosis.stderr);
  const unreached = JSON.parse(absentDiagnosis.stdout);
  assert.equal(unreached.service.running, false);
  assert.equal(unreached.readiness, null, "no service means no readiness to report");
  assert.equal(await home.status(), null, "status, doctor, help, version and a dry run start no service");
  console.log("PASS status and doctor print and return without starting a service");

  // A call starts the service when none runs, as a command start.
  const catalog = call(["--catalog"]);
  assert.equal(catalog.status, 0, catalog.stderr);
  assert.deepEqual(catalog.stdout.trim().split("\n").sort(), [...catalogNames].sort(), "the CLI sees the whole catalog");
  const started = await home.serviceUp();
  assert.equal(started.started_by, startReason.command, "a call that finds no service starts one for the command line");
  assert.equal(started.service, version);
  console.log("PASS call starts one service with the start reason command");

  // With a service running, status and doctor report it and still return zero.
  const status = edge(["status", "--json"]);
  assert.equal(status.status, 0, status.stderr);
  const statusJson = JSON.parse(status.stdout);
  assert.equal(statusJson.running, true);
  assert.equal(statusJson.instance, started.instance);
  assert.equal("token" in statusJson, false, "status must never reveal local authentication material");
  const doctor = edge(["doctor"]);
  assert.equal(doctor.status, 0, doctor.stderr);
  assert.match(
    doctor.stdout,
    /Readiness: Not connected -- No browser is connected\. Open a supported Chromium browser with the Ghostlight extension installed\./
  );
  const doctorJson = edge(["doctor", "--json"]);
  assert.equal(doctorJson.status, 0, doctorJson.stderr);
  const diagnosis = JSON.parse(doctorJson.stdout);
  assert.equal(diagnosis.readiness.state, "not_connected");
  assert.equal(diagnosis.readiness.word, "Not connected");
  assert.equal(diagnosis.service.running, true);
  console.log("PASS status and doctor report the running service");

  // A session that holds no tab already knows its tabs, so its list answers at once with none,
  // asks no browser and binds none.
  const listed = call(["browser_tabs", LIST]);
  assert.equal(listed.status, 0, listed.stderr);
  assert.equal(listed.stdout.trim(), "Listed 0 tabs in this session.");
  const rendered = call(["browser_tabs", LIST, "--json"]);
  assert.equal(rendered.status, 0, rendered.stderr);
  const answered = JSON.parse(rendered.stdout.trim());
  assert.equal(answered.status, "succeeded");
  assert.deepEqual(answered.facts.tabs, []);

  // A session also knows which recording it holds (ADR-0203 Decision 6.7). One that holds none
  // answers at once that it has none, asks no browser and binds none; only a session that holds a
  // recording asks its browser about it.
  const recording = call(["browser_record", RECORDING_STATUS]);
  assert.equal(recording.status, EXIT_FAILED, recording.stderr);
  assert.equal(recording.stdout.trim().split("\n")[0], "This session has no recording.");
  const refused = call(["browser_record", RECORDING_STATUS, "--json"]);
  assert.equal(refused.status, EXIT_FAILED);
  const result = JSON.parse(refused.stdout.trim());
  assert.equal(result.status, "failed");
  assert.equal(result.facts.reason, "not_found");

  // With no browser connected and startup manual, a navigate refuses instead of starting one.
  // Which honest answer arrives depends on this machine's installed browsers, so the journey pins
  // the closed language contract, not the inventory: plural evidence asks the person to open a
  // named browser, unusable registrations name the choice-free remedy, and no refusal ever says
  // Ghostlight declined to choose.
  const NAVIGATE = '{"url":"https://sylin.org/","new_tab":true}';
  const away = call(["browser_navigate", NAVIGATE]);
  assert.equal(away.status, EXIT_FAILED, away.stderr);
  const sentence = away.stdout.trim().split(/\r?\n/)[0];
  const awayResult = JSON.parse(call(["browser_navigate", NAVIGATE, "--json"]).stdout.trim());
  if (awayResult.facts.reason === "browser_startup_manual") {
    assert.match(
      sentence,
      /^No browser is connected\. Ask the user to open .+ browser window with the Ghostlight extension installed, then repeat the call\.$/
    );
    assert.ok(awayResult.facts.browsers.length >= 1, away.stdout);
    if (awayResult.facts.browsers.length === 1) assert.equal(awayResult.facts.browser, awayResult.facts.browsers[0]);
  } else if (awayResult.facts.reason === "native_host_unavailable") {
    assert.equal(sentence, "The browser cannot use Ghostlight's native messaging registration.");
    assert.ok(awayResult.facts.details.length >= 1, away.stdout);
  } else if (awayResult.facts.reason === "browser_absent") {
    assert.equal(sentence, "No supported Chromium browser is installed.");
  } else {
    assert.fail(`unexpected no-browser reason: ${awayResult.facts.reason}`);
  }

  // A refusal must not look like success to a shell.
  const rejected = call(["browser_navigate", "{}"]);
  assert.notEqual(rejected.status, 0);
  assert.equal(rejected.stdout.trim(), "The call does not match the Ghostlight catalog.");

  // One process, one session: an earlier line shares the workspace with later ones.
  const batch = call(["--stdin"], `browser_tabs ${LIST}\nbrowser_tabs ${LIST}\n\nbrowser_tabs ${LIST}\n`);
  assert.equal(batch.status, 0, batch.stderr);
  const lines = batch.stdout.trim().split("\n").filter((line) => line.trim());
  assert.equal(lines.length, 3, "every line of the batch is answered");
  for (const line of lines) assert.equal(line, "Listed 0 tabs in this session.");

  // Retrieving the catalog invokes nothing and is deliberately not an audited action.
  const tabRecords = await until(() => {
    const found = home.auditRecords().filter((record) => record.tool === "browser_tabs");
    return found.length >= 5 && found;
  }, "the five browser_tabs calls are audited");
  assert.equal(tabRecords.length, 5, "two separate calls and a batch of three are audited once each");
  const written = home.auditRecords();
  for (const record of written) {
    assert.equal(record.channel, "cli", "scripted work must be attributed to its own channel");
  }
  assert.equal(
    new Set(tabRecords.slice(-3).map((record) => record.workspace)).size,
    1,
    "a batch must hold one workspace across its calls"
  );
  // Every call came from this one node process, so the session marker gathers all of them -- the
  // separate processes and the batch alike -- into a single workspace (ADR-0106).
  assert.equal(
    new Set(written.map((record) => record.workspace)).size,
    1,
    "calls from one caller must reach one workspace, whatever process each ran in"
  );
  console.log("PASS a call reaches the executor as the cli channel and a batch keeps one workspace");

  // After a quit the next call starts a service again, as a command start.
  const quit = await home.quit();
  assert.ok(quit, "the service quits");
  assert.equal(await home.status(), null);
  const again = call(["--catalog"]);
  assert.equal(again.status, 0, again.stderr);
  const second = await home.serviceUp();
  assert.notEqual(second.instance, started.instance, "the second service is a new instance");
  assert.equal(second.started_by, startReason.command);
  console.log("PASS a call after a quit starts a new service with the start reason command");

  // The strongest guarantee this journey owns: after every beat, the machine's real native-host
  // registration is byte-identical to the snapshot taken before the first process.
  assert.equal(machineRegistration(), registrationBefore, "the journey changed the machine's native-host registration");

  console.log("cli journey ok: status and doctor without a service -> call starts one -> governed result -> cli-attributed audit -> batch session");
} finally {
  await home.cleanup();
}
