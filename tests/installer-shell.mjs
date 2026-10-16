// Exercise the shipped shell installer with deterministic release/CDN replies and a fresh home.
// The release carries three images: the edge `ghostlight`, the service `ghostlight-service` and the
// shell `ghostlight-desktop`. The installer verifies all three, runs the downloaded service's
// `deployment install`, then runs the installed edge's `install` in the person's own console.
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
const names = ["ghostlight", "ghostlight-service", "ghostlight-desktop"];
const target = "x86_64-unknown-linux-gnu";
const bodies = {
  // The edge records that it ran and says so in the console it runs in.
  ghostlight: `#!/bin/sh
echo "fixture edge: $*"
echo "edge: $*" >> "$INSTALLER_COMMANDS"
`,
  // The service installs every image from the directory it runs in.
  "ghostlight-service": `#!/bin/sh
if [ "$1" = deployment ] && [ "$2" = install ]; then
  echo "service: $*" >> "$INSTALLER_COMMANDS"
  mkdir -p "$HOME/.ghostlight/bin"
  for name in ghostlight ghostlight-service ghostlight-desktop; do
    cp "$(dirname "$0")/$name" "$HOME/.ghostlight/bin/$name"
  done
else
  exit 1
fi
`,
  // The shell is only ever installed, never run, by the installer.
  "ghostlight-desktop": `#!/bin/sh
exit 1
`
};
const sums = names.map(name => `${createHash("sha256").update(bodies[name]).digest("hex")}  ${name}-${target}`).join("\n") + "\n";
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
  const name = url.split('/').at(-1).replace('-${target}', '');
  if (!(name in fixture.bodies)) process.exit(22);
  body = fixture.bodies[name];
  effective = 'https://release-assets.githubusercontent.com/fixture/' + name;
}
if (args.includes('-o')) fs.writeFileSync(args[args.indexOf('-o') + 1], body);
if (args.includes('-w')) process.stdout.write(effective);
`, { mode: 0o755 });
// A shim that does not parse would fail every request and hide what the cases mean to prove.
const shimCheck = spawnSync(process.execPath, ["--check", join(shims, "curl")], { encoding: "utf8" });
assert.equal(shimCheck.status, 0, `curl shim does not parse: ${shimCheck.stderr}`);
// Provenance verification is optional. Keep this fixture offline even on machines with gh.
writeFileSync(join(shims, "gh"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
function run(label, changes = {}, version, arguments_ = []) {
  const directory = join(area, label), userDirectory = join(directory, "user");
  mkdirSync(userDirectory, { recursive: true });
  const fixture = join(directory, "fixture.json"), requests = join(directory, "requests.txt"), commands = join(directory, "commands.txt");
  writeFileSync(fixture, JSON.stringify({ latest: "https://github.com/sylin-org/ghostlight/releases/tag/v1.2.3", sums, bodies, ...changes }));
  const environment = { ...process.env, HOME: userDirectory, PATH: `${shims}:${process.env.PATH}`,
    INSTALLER_FIXTURE: fixture, INSTALLER_REQUESTS: requests, INSTALLER_COMMANDS: commands };
  delete environment.GHOSTLIGHT_VERSION;
  delete environment.GHOSTLIGHT_NO_REGISTER;
  if (version) environment.GHOSTLIGHT_VERSION = version;
  const result = spawnSync("sh", [join(root, "scripts/get.sh"), ...arguments_], { env: environment, encoding: "utf8", timeout: 15000 });
  assert.equal(result.error, undefined);
  writeFileSync(join(directory, "output.log"), result.stdout + result.stderr);
  const lines = file => existsSync(file) ? readFileSync(file, "utf8").trim().split("\n") : [];
  return { ...result, userDirectory, requests: lines(requests), commands: lines(commands) };
}
const success = run("cdn-downloads");
assert.equal(success.status, 0, success.stderr);
assert.deepEqual(success.requests, [
  "https://github.com/sylin-org/ghostlight/releases/latest",
  ...["SHA256SUMS", ...names.map(name => `${name}-${target}`)]
    .map(name => `https://github.com/sylin-org/ghostlight/releases/download/v1.2.3/${name}`)
]);
const installed = join(success.userDirectory, ".ghostlight/bin");
assert.deepEqual(readdirSync(installed).sort(), [...names].sort(), "all three images and nothing else are installed");
for (const name of names) {
  assert.equal(readFileSync(join(installed, name), "utf8"), bodies[name]);
  assert.equal(statSync(join(installed, name)).mode & 0o777, 0o755);
}
assert.deepEqual(success.commands, ["service: deployment install", "edge: install"], "the service installs, then the installed edge runs install");
assert.match(success.stdout, /fixture edge: install/, "the output of ghostlight install reaches the person's console");
const restored = run("restore", {}, undefined, ["--restore"]);
assert.equal(restored.status, 0, restored.stderr);
assert.deepEqual(restored.commands, ["service: deployment install --restore", "edge: install"]);
for (const latest of ["https://example.com/releases/tag/v1.2.3", "https://github.com/sylin-org/ghostlight/releases/tag/v1.2.3-rc1"]) {
  const result = run(`invalid-release-${latest.includes("example") ? "host" : "tag"}`, { latest });
  assert.notEqual(result.status, 0);
  assert.equal(result.requests.length, 1);
  assert.equal(existsSync(join(result.userDirectory, ".ghostlight")), false);
}
const mismatch = run("requested-version", {}, "9.9.9");
assert.notEqual(mismatch.status, 0);
assert.equal(mismatch.requests.length, 1);
for (const damaged of names) {
  const corrupt = run(`corrupt-${damaged}`, { bodies: { ...bodies, [damaged]: "wrong bytes" } });
  assert.notEqual(corrupt.status, 0);
  assert.equal(existsSync(join(corrupt.userDirectory, ".ghostlight/bin")), false, `a corrupt ${damaged} installs nothing`);
  assert.deepEqual(corrupt.commands, [], "a corrupt download runs nothing");
}
console.log(`PASS shell installer: pinned release, CDN delivery, all three images, install in the console, restore, invalid release/version, checksum refusal\nEvidence: ${area}`);
