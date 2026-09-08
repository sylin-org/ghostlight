// Exercise the shipped shell installer with deterministic release/CDN replies and a fresh home.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

assert.equal(process.platform, "linux", "The shell installer supports Linux only.");
const root = resolve(import.meta.dirname, "..");
mkdirSync(join(root, ".tmp/installer-shell"), { recursive: true });
const area = mkdtempSync(join(root, ".tmp/installer-shell/run-"));
const shims = join(area, "shims");
mkdirSync(shims);
const names = ["ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"];
const bodies = Object.fromEntries(names.map(name => [name, `fixture executable: ${name}\n`]));
const sums = names.map(name => `${createHash("sha256").update(bodies[name]).digest("hex")}  ${name}-x86_64-unknown-linux-gnu`).join("\n") + "\n";
writeFileSync(join(shims, "curl"), `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2), url = args.at(-1);
const fixture = JSON.parse(fs.readFileSync(process.env.INSTALLER_FIXTURE));
fs.appendFileSync(process.env.INSTALLER_REQUESTS, url + '\\n');
let body = '', effective = url;
if (url.endsWith('/releases/latest')) {
  effective = fixture.latest;
} else if (url.endsWith('/SHA256SUMS')) {
  body = fixture.sums;
  effective = 'https://release-assets.githubusercontent.com/fixture/SHA256SUMS';
} else {
  const name = url.split('/').at(-1).replace('-x86_64-unknown-linux-gnu', '');
  if (!(name in fixture.bodies)) process.exit(22);
  body = fixture.bodies[name];
  effective = 'https://release-assets.githubusercontent.com/fixture/' + name;
}
if (args.includes('-o')) fs.writeFileSync(args[args.indexOf('-o') + 1], body);
if (args.includes('-w')) process.stdout.write(effective);
`, { mode: 0o755 });
// Provenance verification is optional. Keep this fixture offline even on machines with gh.
writeFileSync(join(shims, "gh"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
function run(label, changes = {}, version) {
  const directory = join(area, label), userDirectory = join(directory, "user");
  mkdirSync(userDirectory, { recursive: true });
  const fixture = join(directory, "fixture.json"), requests = join(directory, "requests.txt");
  writeFileSync(fixture, JSON.stringify({ latest: "https://github.com/sylin-org/ghostlight/releases/tag/v1.2.3", sums, bodies, ...changes }));
  const environment = { ...process.env, HOME: userDirectory, PATH: `${shims}:${process.env.PATH}`,
    GHOSTLIGHT_NO_REGISTER: "1", INSTALLER_FIXTURE: fixture, INSTALLER_REQUESTS: requests };
  delete environment.GHOSTLIGHT_VERSION;
  if (version) environment.GHOSTLIGHT_VERSION = version;
  const result = spawnSync("sh", [join(root, "scripts/get.sh")], { env: environment, encoding: "utf8", timeout: 15000 });
  assert.equal(result.error, undefined);
  writeFileSync(join(directory, "output.log"), result.stdout + result.stderr);
  return { ...result, userDirectory, requests: readFileSync(requests, "utf8").trim().split("\n") };
}
const success = run("cdn-downloads");
assert.equal(success.status, 0, success.stderr);
assert.deepEqual(success.requests, [
  "https://github.com/sylin-org/ghostlight/releases/latest",
  ...["SHA256SUMS", ...names.map(name => `${name}-x86_64-unknown-linux-gnu`)]
    .map(name => `https://github.com/sylin-org/ghostlight/releases/download/v1.2.3/${name}`)
]);
const installed = join(success.userDirectory, ".ghostlight/bin/v1.2.3");
assert.deepEqual(readdirSync(installed).sort(), [...names].sort());
for (const name of names) {
  assert.equal(readFileSync(join(installed, name), "utf8"), bodies[name]);
  assert.equal(statSync(join(installed, name)).mode & 0o777, 0o755);
}
for (const latest of ["https://example.com/releases/tag/v1.2.3", "https://github.com/sylin-org/ghostlight/releases/tag/v1.2.3-rc1"]) {
  const result = run(`invalid-release-${latest.includes("example") ? "host" : "tag"}`, { latest });
  assert.notEqual(result.status, 0);
  assert.equal(result.requests.length, 1);
  assert.equal(existsSync(join(result.userDirectory, ".ghostlight")), false);
}
const mismatch = run("requested-version", {}, "9.9.9");
assert.notEqual(mismatch.status, 0);
assert.equal(mismatch.requests.length, 1);
const corrupt = run("corrupt-download", { bodies: { ...bodies, ghostlight: "wrong bytes" } });
assert.notEqual(corrupt.status, 0);
assert.equal(existsSync(join(corrupt.userDirectory, ".ghostlight/bin/v1.2.3/ghostlight")), false);
console.log(`PASS shell installer: pinned release, CDN delivery, exact siblings, modes, invalid release/version, checksum refusal\nEvidence: ${area}`);
