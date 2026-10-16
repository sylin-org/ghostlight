// Repeatable H1-H8, C1 reporting, and editor regression gates. Each journey retains its own
// isolation and cleanup. This runner builds the exact binaries it passes to every process test.
// It is the only list of journeys and lanes: CI runs `--lane=process` and `--lane=browser`.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createWriteStream, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";

const root = resolve(import.meta.dirname, "..");
const args = process.argv.slice(2);
assert.ok(args.length <= 1 && (!args.length || /^--lane=(all|process|browser)$/.test(args[0])),
  "Usage: node tests/hardening-suite.mjs [--lane=all|process|browser]");
const lane = args[0]?.split("=")[1] || "all";
const target = resolve(process.env.CARGO_TARGET_DIR || join(root, ".target-ghostlight-1.0"));
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || join(root, ".tmp/chrome-testing/chrome-win64/chrome.exe");
const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const output = join(root, ".tmp", "hardening-suite", `${stamp}-${process.pid}`);
mkdirSync(output, { recursive: true });
const environment = { ...process.env, CARGO_TARGET_DIR: target,
  GHOSTLIGHT_TEST_BROWSER: browser };
delete environment.GHOSTLIGHT_BIN_DIR;
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
function source() {
  // Scope explicitly to public source and tests; never inspect machine-local or personal files.
  const paths = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
    "crates", "extension", "tests", ".cargo", "packaging", "examples", "scripts", "docs/legal", "docs/guides",
    ".github/workflows/ci.yml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"],
  { cwd: root, encoding: "utf8", windowsHide: true }).split("\0").filter(Boolean).sort();
  return hash(paths.map(path => `${path}\0${existsSync(join(root, path)) ? hash(readFileSync(join(root, path))) : "deleted"}\n`).join(""));
}
const report = { lane, platform: process.platform, node: process.version,
  revision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8", windowsHide: true }).trim(),
  source_sha256: source(), binaries: {}, started_at: new Date().toISOString(), results: [] };
const save = () => writeFileSync(join(output, "results.json"), JSON.stringify(report, null, 2) + "\n");
save();
async function run(name, command, commandArgs, cwd = root, onLine) {
  const started = Date.now();
  const logPath = join(output, `${name}.log`);
  const log = createWriteStream(logPath);
  console.log(`RUN ${name}`);
  const result = await new Promise(done => {
    const child = spawn(command, commandArgs, { cwd, env: environment, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    child.stdout.on("data", bytes => log.write(bytes));
    child.stderr.on("data", bytes => log.write(bytes));
    if (onLine) createInterface({ input: child.stdout }).on("line", onLine);
    child.once("error", error => done({ status: "failed", error: error.message }));
    child.once("close", (code, signal) => done({ status: code === 0 ? "passed" : "failed", code, signal }));
  });
  await new Promise(done => log.end(done));
  report.results.push({ name, ...result, duration_ms: Date.now() - started, log: logPath });
  save();
  console.log(`${result.status.toUpperCase()} ${name} (${Date.now() - started} ms)`);
  if (result.status !== "passed") console.error(readFileSync(logPath, "utf8").slice(-6000));
  return result.status === "passed";
}
function blocked(name, reason) { report.results.push({ name, status: "blocked", reason }); save(); }
const node = (name, path) => run(name, process.execPath, [path]);

// The journeys, by lane. `file` is under tests/. A journey runs with `node` unless its kind says
// otherwise, only on its `platform` when it names one, and needs the fresh build unless
// `needsBuild` is false. A `test` entry runs the node test runner over its `files`.
const PROCESS_JOURNEYS = [
  { name: "journey-helpers", kind: "test", files: ["lib/home.test.mjs", "lib/installed.test.mjs", "lib/browser.test.mjs"], needsBuild: false },
  { name: "shell-installer", file: "installer-shell.mjs", platform: "linux" },
  { name: "edge-parent", file: "edge-parent-lifecycle.mjs" },
  { name: "fixed-installation", file: "fixed-installation-journey.mjs", platform: "linux" },
  { name: "process", file: "process-journey.mjs" },
  { name: "continuity", file: "local-resilience-journey.mjs" },
  { name: "history-compatibility", file: "history-compatibility.mjs" },
  { name: "provenance", file: "provenance-journey.mjs" },
  { name: "cli", file: "cli-journey.mjs" },
  { name: "cli-powershell", file: "cli-powershell-journey.mjs" },
  { name: "workbench", file: "workbench-surface.mjs" },
  { name: "powershell-installer", kind: "pwsh", file: "installer-powershell.ps1", platform: "win32" },
  { name: "desktop-windows", kind: "pwsh", file: "windows-desktop-journey.ps1", platform: "win32" }
];
const BROWSER_JOURNEYS = [
  { name: "browser-harness", kind: "test", files: ["chromium-harness.test.mjs", "linux/installed-reporting.test.mjs"], needsBuild: false, needsBrowser: false },
  { name: "script-browser", file: "script-browser-journey.mjs", needsBrowser: true },
  { name: "frame-browser", file: "frame-browser-journey.mjs", needsBrowser: true },
  { name: "keyboard-browser", file: "keyboard-browser-journey.mjs", needsBrowser: true },
  { name: "history-browser", file: "workbench-history-browser.mjs", needsBrowser: true }
];
function launch({ name, file, kind = "node", files = [] }) {
  if (kind === "pwsh") return run(name, "pwsh", ["-NoProfile", "-File", `tests/${file}`]);
  if (kind === "test") return run(name, process.execPath, ["--test", ...files.map((path) => `tests/${path}`)]);
  return node(name, `tests/${file}`);
}
async function runJourneys(journeys) {
  for (const journey of journeys) {
    if (journey.platform && journey.platform !== process.platform) continue;
    if (journey.needsBuild !== false && !built) blocked(journey.name, "Current-source build failed.");
    else if (journey.needsBrowser && !existsSync(browser)) blocked(journey.name, "Set GHOSTLIGHT_TEST_BROWSER to Chrome for Testing with unpacked-extension support.");
    else await launch(journey);
  }
}

if (lane === "all") {
  await run("format", "cargo", ["fmt", "--all", "--", "--check"]);
  await run("clippy", "cargo", ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]);
  await run("rust", "cargo", ["test", "--workspace", "--locked", "--no-fail-fast"]);
  const extensionTests = readdirSync(join(root, "extension/tests")).filter(path => path.endsWith(".test.js")).sort();
  await run("extension", process.execPath, ["--test", ...extensionTests.map(path => `tests/${path}`)], join(root, "extension"));
  await node("npm-launcher", "packaging/npm/test/launcher.test.js");
  await node("mcpb-launcher", "packaging/mcpb/test/launcher.test.js");
  await run("winget-archive", "pwsh", ["-NoProfile", "-File", "tests/winget-archive.ps1"]);
  await node("policy-grammar", "tests/policy-grammar.mjs");
}
const artifacts = new Map();
const executables = ["ghostlight", "ghostlight-service", "ghostlight-desktop"];
let built = await run("build", "cargo", ["build", "--workspace", "--locked", "--message-format=json-render-diagnostics"], root, line => {
  const message = JSON.parse(line);
  if (message.reason === "compiler-artifact" && message.executable && executables.includes(message.target.name)
    && message.target.kind.includes("bin")) artifacts.set(message.target.name, resolve(message.executable));
});
if (built) {
  const directories = new Set([...artifacts.values()].map(dirname));
  if (executables.some(name => !artifacts.has(name)) || directories.size !== 1) {
    blocked("build-artifacts", "Cargo must identify the edge, the service and the shell in one directory.");
    built = false;
  } else {
    environment.GHOSTLIGHT_BIN_DIR = [...directories][0];
    for (const [name, path] of artifacts) report.binaries[name] = { path, sha256: hash(readFileSync(path)) };
  }
  save();
}
if (lane !== "browser") await runJourneys(PROCESS_JOURNEYS);
if (lane !== "process") await runJourneys(BROWSER_JOURNEYS);
report.finished_at = new Date().toISOString();
report.source_sha256_end = source();
report.source_unchanged = report.source_sha256 === report.source_sha256_end;
report.passed = report.results.every((result) => result.status === "passed");
save();
console.log(`Evidence: ${join(output, "results.json")}`);
if (!report.source_unchanged) console.warn("Note: source changed during the run; results.json records both fingerprints. Do not cite this run as evidence for one revision.");
process.exitCode = report.passed ? 0 : 1;
