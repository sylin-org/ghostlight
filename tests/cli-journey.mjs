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
// ADR-0150: the runtime override elects the authority directory, so it points inside the
// build under test rather than at a directory that holds no authority.
const runtimeFile = join(binDir, `.ghostlight-cli-journey-runtime-${process.pid}.json`);
const auditFile = join(repository, `tests/.ghostlight-cli-audit-${process.pid}.jsonl`);
// The service holds its lifetime lease beside the runtime file; killing it leaves the lease behind.
const leaseFile = runtimeFile.replace(/\.json$/, ".lock");
const scriptingDisabledPolicyFile = join(repository, "examples/scripting-disabled.json");
// The whole native-host registration surface (manifests and Windows registry keys) is isolated
// below this directory. Recovery silently repairs Ghostlight-owned registrations toward the
// running tree (ADR-0149), so an un-isolated journey would adopt the machine's real browser
// registration into whatever build is under test -- exactly the 2026-08-30 preflight leak.
const nativeHostDir = join(repository, `tests/.ghostlight-cli-native-host-${process.pid}`);
const environment = {
  ...process.env,
  GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
  GHOSTLIGHT_AUDIT_FILE: auditFile,
  GHOSTLIGHT_NATIVE_HOST_DIR: nativeHostDir
};

// The machine's persistent native-host registration, as one comparable string. The journey
// snapshots it before its first process and asserts it is byte-identical after its last, so no
// journey beat can ever mutate the real registration unnoticed.
const REGISTRY_VENDORS = [
  ["Google", "Chrome"],
  ["Microsoft", "Edge"],
  ["BraveSoftware", "Brave-Browser"],
  ["Chromium"]
];
const LINUX_MANIFEST_DIRECTORIES = [
  "google-chrome",
  "microsoft-edge",
  "BraveSoftware/Brave-Browser",
  "chromium"
];
function machineRegistration() {
  const parts = [];
  const readManifest = (path) =>
    existsSync(path) ? readFileSync(path, "utf8") : "<absent>";
  if (process.platform === "win32") {
    parts.push(
      readManifest(
        join(
          process.env.LOCALAPPDATA || "",
          "Ghostlight",
          "NativeMessagingHosts",
          "org.sylin.ghostlight.json"
        )
      )
    );
    for (const vendor of REGISTRY_VENDORS) {
      const key = `HKCU\\Software\\${vendor.join("\\")}\\NativeMessagingHosts\\org.sylin.ghostlight`;
      const query = spawnSync("reg", ["query", key, "/ve"], { encoding: "utf8" });
      parts.push(query.status === 0 ? (query.stdout || "").trim() : "<absent>");
    }
  } else {
    const configHome = process.env.XDG_CONFIG_HOME || join(process.env.HOME || "", ".config");
    for (const directory of LINUX_MANIFEST_DIRECTORIES) {
      parts.push(
        readManifest(join(configHome, directory, "NativeMessagingHosts", "org.sylin.ghostlight.json"))
      );
    }
  }
  return parts.join("\n---\n");
}
const registrationBefore = machineRegistration();

const ghostlight = join(binDir, `ghostlight${executableSuffix}`);
if (!existsSync(ghostlight)) throw new Error(`Missing ${ghostlight}; build the workspace first.`);
// The expected banner derives from the workspace version so a version bump cannot silently
// invalidate this journey; the pin is "banner reflects source truth", not a literal.
const sourceVersion = readFileSync(join(repository, "Cargo.toml"), "utf8")
  .match(/\[workspace\.package\][\s\S]*?^version = "([^"]+)"/m)?.[1];
if (!sourceVersion) throw new Error("Could not read the workspace version from Cargo.toml");

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
  assert.equal(version.stdout.trim(), `ghostlight ${sourceVersion}`);
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
  const doctor = spawnSync(ghostlight, ["doctor"], {
    env: environment,
    encoding: "utf8"
  });
  assert.equal(doctor.status, 0, doctor.stderr);
  assert.match(
    doctor.stdout,
    /Readiness: Not connected -- No browser is connected\. Open a supported Chromium browser with the Ghostlight extension installed\./
  );
  const doctorJson = spawnSync(ghostlight, ["doctor", "--json"], {
    env: environment,
    encoding: "utf8"
  });
  assert.equal(doctorJson.status, 0, doctorJson.stderr);
  const diagnosis = JSON.parse(doctorJson.stdout);
  assert.equal(diagnosis.readiness.state, "not_connected");
  assert.equal(diagnosis.readiness.word, "Not connected");

  const listed = call(["browser_tabs", '{"action":"list"}']);
  // A live read has nothing to read without a connected browser: it refuses instead of
  // answering from remembered state. Which honest answer arrives depends on this machine's
  // installed browsers, so the journey pins the closed language contract, not the inventory:
  // plural evidence asks the person to open a named browser, unusable registrations name the
  // choice-free remedy, and no refusal ever says Ghostlight declined to choose.
  assert.notEqual(listed.status, 0, listed.stderr);
  const rendered = call(["browser_tabs", '{"action":"list"}', "--json"]);
  assert.notEqual(rendered.status, 0);
  const result = JSON.parse(rendered.stdout.trim());
  assert.equal(result.status, "failed");
  if (result.facts.reason === "browser_startup_manual") {
    assert.match(
      listed.stdout.trim(),
      /^No browser is connected\. Ask the user to open a .+ browser window with the Ghostlight extension installed, then repeat the call\.$/
    );
    assert.ok(result.facts.browsers.length >= 1, listed.stdout);
    if (result.facts.browsers.length === 1) {
      assert.equal(result.facts.browser, result.facts.browsers[0]);
    }
  } else if (result.facts.reason === "native_host_unavailable") {
    assert.equal(
      listed.stdout.trim(),
      "The browser cannot use Ghostlight's native messaging registration."
    );
    assert.ok(result.facts.details.length >= 1, listed.stdout);
  } else if (result.facts.reason === "browser_absent") {
    assert.equal(listed.stdout.trim(), "No supported Chromium browser is installed.");
  } else {
    assert.fail(`unexpected no-browser reason: ${result.facts.reason}`);
  }

  const catalog = call(["--catalog"]);
  assert.equal(catalog.status, 0);
  assert.equal(catalog.stdout.trim().split("\n").length, 24, "the CLI sees the whole catalog");

  // A refusal must not look like success to a shell.
  const rejected = call(["browser_navigate", "{}"]);
  assert.notEqual(rejected.status, 0);
  assert.equal(rejected.stdout.trim(), "The call does not match the Ghostlight catalog.");

  // One process, one session: even refusals from an earlier line share the workspace with
  // later ones.
  const batch = call(["--stdin"], 'browser_tabs {"action":"list"}\nbrowser_tabs {"action":"list"}\n\nbrowser_tabs {"action":"list"}\n');
  // Every line refuses for the same honest reason; the batch reports the failure.
  assert.notEqual(batch.status, 0, batch.stderr);
  for (const line of batch.stdout.trim().split("\n")) {
    if (line.trim()) {
      assert.match(line, /No browser is connected|native messaging registration/);
    }
  }

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

  // The strongest guarantee this journey owns: after every beat, the machine's real
  // native-host registration is byte-identical to the snapshot taken before the first process.
  assert.equal(
    machineRegistration(),
    registrationBefore,
    "the journey changed the machine's native-host registration"
  );

  console.log("cli journey ok: demand-free call -> governed result -> cli-attributed audit -> batch session -> channel refusal");
} finally {
  for (const child of services) if (!child.killed) child.kill();
  for (const file of cleanup) rmSync(file, { force: true });
  rmSync(nativeHostDir, { force: true, recursive: true });
}
