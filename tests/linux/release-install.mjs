// The release candidate installed the way a person on Linux installs it (linux-2 L2-16): the real
// get.sh from the bundle, the browser's own connection through the registration the install wrote,
// doctor reading Ready, one browser task from an MCP client through the installed edge, and
// uninstall. The release workflow runs it on fresh runners, as the runner user, under xvfb.
//
// It installs into and uninstalls from the home it runs in, so it refuses to run outside CI.
//
//   node tests/linux/release-install.mjs --bundle <unpacked release-candidate> --browser <Chrome for Testing>
import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, readlinkSync, realpathSync } from "node:fs";
import { createServer } from "node:http";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { McpEdge, structured } from "../lib/peers.mjs";

assert.ok(process.env.CI, "This check installs into and uninstalls from this home; it runs only in CI.");
const option = (name) => {
  const index = process.argv.indexOf(name);
  assert.ok(index > 0 && process.argv[index + 1], `Pass ${name} <path>.`);
  return resolve(process.argv[index + 1]);
};
const bundle = option("--bundle");
const browserProgram = option("--browser");
const repository = resolve(import.meta.dirname, "..", "..");
const home = homedir();
const bin = join(home, ".ghostlight", "bin");
const command = join(home, ".local", "bin", "ghostlight");
// Chrome looks for native hosts in its user data directory, so the browser runs in the directory a
// person's Chrome uses and meets the registration the install wrote there.
const chromeData = join(home, ".config", "google-chrome");
const registration = join(chromeData, "NativeMessagingHosts", "org.sylin.ghostlight.json");
const WAIT_MS = 60_000;

const until = async (what, check) => {
  const deadline = Date.now() + WAIT_MS;
  while (Date.now() < deadline) {
    if (await check()) return;
    await new Promise((done) => setTimeout(done, 500));
  }
  assert.fail(`Timed out waiting for ${what}.`);
};
const json = (program, args) => JSON.parse(execFileSync(program, args, { encoding: "utf8" }));
// A process's executable reads as a resolved path, so every comparison is between resolved paths.
const ghostlightServices = () => {
  const service = realpathSync(join(bin, "ghostlight-service"));
  return readdirSync("/proc").filter((pid) => /^\d+$/.test(pid)).filter((pid) => {
    try { return readlinkSync(`/proc/${pid}/exe`) === service; } catch { return false; }
  });
};

// 1. The one-line installer, from the bundle.
const installed = spawnSync("sh", [join(repository, "scripts/get.sh")],
  { env: { ...process.env, GHOSTLIGHT_FROM: bundle }, encoding: "utf8", timeout: 300_000 });
process.stdout.write(installed.stdout + installed.stderr);
assert.equal(installed.status, 0, "get.sh failed");
assert.match(installed.stdout, /copied from .*Checksums/);
assert.ok(existsSync(command), "the install put no ghostlight command in ~/.local/bin");
const host = JSON.parse(readFileSync(registration, "utf8"));
assert.equal(realpathSync(host.path), realpathSync(join(bin, "ghostlight")), "Chrome's registration names another program");

// 2. The browser, with the extension from the bundle's revision; its connection starts Ghostlight.
const page = createServer((request, response) => {
  response.writeHead(200, { "content-type": "text/html" });
  response.end(`<!doctype html><title>Bell</title><button onclick="document.getElementById('said').textContent = 'The bell rang.'">Ring the bell</button><p id="said"></p>`);
});
await new Promise((done) => page.listen(0, "127.0.0.1", done));
// A runner has no keyring, and a browser that looks for one can hold its first requests while it
// waits, so the browser keeps its passwords in its profile.
const browser = spawn(browserProgram, [`--user-data-dir=${chromeData}`, `--load-extension=${join(repository, "extension")}`,
  "--password-store=basic", "--no-first-run", "--no-default-browser-check",
  ...(process.env.GHOSTLIGHT_TEST_NO_SANDBOX === "1" ? ["--no-sandbox"] : []), "about:blank"], { stdio: "ignore" });
try {
  await until("the browser to connect", () => {
    try { return json(command, ["status", "--json"]).connections.browsers >= 1; } catch { return false; }
  });
  const readiness = json(command, ["doctor", "--json"]).readiness;
  assert.equal(readiness.state, "ready", `doctor reads ${readiness.word}: ${readiness.detail}`);

  // 3. One task, from an MCP client through the installed edge that agents' entries name.
  const edge = spawn(join(bin, "ghostlight"), ["mcp"], { stdio: ["pipe", "pipe", "inherit"] });
  try {
    const mcp = new McpEdge(edge);
    await mcp.initialize("release-check");
    const step = async (tool, args) => {
      const result = structured(await mcp.call(tool, args));
      assert.equal(result.status, "succeeded", `${tool}: ${JSON.stringify(result)}`);
      return result.facts;
    };
    await step("browser_navigate", { url: `http://127.0.0.1:${page.address().port}/` });
    const found = await step("browser_find", { text: "Ring the bell" });
    await step("browser_click", { target: found.matches[0].target });
    assert.match((await step("browser_read", {})).text, /The bell rang\./);
  } finally {
    edge.kill();
  }
} finally {
  browser.kill();
  page.close();
}

// 4. Uninstall: the registrations and the command go, and the running Ghostlight stops.
const removed = spawnSync(command, ["uninstall"], { encoding: "utf8", timeout: 120_000 });
process.stdout.write(removed.stdout + removed.stderr);
assert.equal(removed.status, 0, "uninstall failed");
assert.match(removed.stdout, /^Ghostlight is uninstalled\./m);
assert.equal(existsSync(registration), false, "Chrome's registration is still there");
assert.equal(existsSync(command), false, "the ghostlight command is still there");
await until("the service to stop", () => ghostlightServices().length === 0);
console.log("PASS release install: get.sh from the bundle, the browser's connection, doctor Ready, a browser task through the installed edge, uninstall");
