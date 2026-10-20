// Exercise the shipped shell installer with deterministic release/CDN replies and a fresh home.
// The release carries three images: the edge `ghostlight`, the service `ghostlight-service` and the
// shell `ghostlight-desktop`. The installer verifies all three, runs the downloaded service's
// `deployment install`, then the same downloaded service's `install` in the person's own console.
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
const muslTarget = "x86_64-unknown-linux-musl";
const bodies = {
  // The installer never runs the edge.
  ghostlight: `#!/bin/sh
echo "edge: $*" >> "$INSTALLER_COMMANDS"
exit 1
`,
  // The service installs every image from the directory it runs in. It records which copy ran each
  // command, and its install and doctor say so in the console they run in. Its install fails when
  // INSTALLER_INSTALL_FAILS is set, and its doctor always reports a problem.
  "ghostlight-service": `#!/bin/sh
copy=downloaded
[ "$(dirname "$0")" = "$HOME/.ghostlight/bin" ] && copy=installed
if [ "$1" = deployment ] && [ "$2" = install ]; then
  echo "$copy service: $*" >> "$INSTALLER_COMMANDS"
  mkdir -p "$HOME/.ghostlight/bin"
  for name in ghostlight ghostlight-service ghostlight-desktop; do
    cp "$(dirname "$0")/$name" "$HOME/.ghostlight/bin/$name"
  done
elif [ "$1" = install ]; then
  echo "fixture service: $*"
  echo "$copy service: $*" >> "$INSTALLER_COMMANDS"
  [ -z "\${INSTALLER_INSTALL_FAILS:-}" ] || exit 3
elif [ "$1" = doctor ]; then
  echo "fixture service: $*"
  echo "$copy service: $*" >> "$INSTALLER_COMMANDS"
  exit 1
else
  exit 1
fi
`,
  // The shell is only ever installed, never run, by the installer.
  "ghostlight-desktop": `#!/bin/sh
exit 1
`
};
// SHA256SUMS as a release carries it: sha256sum's layout with one LF per line, listing the other
// system's images too. GitHub serves it as application/octet-stream; get.sh saves the bytes with
// curl -o and reads the saved file, so the content type never matters.
const sums = names.flatMap(name => [
  `${createHash("sha256").update(bodies[name]).digest("hex")}  ${name}-${target}`,
  `${createHash("sha256").update(bodies[name]).digest("hex")}  ${name}-${muslTarget}`,
  `${"0".repeat(64)}  ${name}-x86_64-pc-windows-msvc.exe`
]).join("\n") + "\n";
writeFileSync(join(shims, "curl"), `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2), url = args.at(-1);
const fixture = JSON.parse(fs.readFileSync(process.env.INSTALLER_FIXTURE));
fs.appendFileSync(process.env.INSTALLER_REQUESTS, url + '\\n');
let body = '', effective = url, status = 200;
if (url.endsWith('/releases/latest')) {
  effective = fixture.latest;
} else if (url.includes('/releases/tags/')) {
  // The releases API answers a tag it has with the release, any other tag with 404, and may answer
  // anything else (a rate limit, a server error). Without -f curl exits 0 and reports the status.
  const tag = url.split('/').at(-1);
  if (fixture.transportFailure) {
    process.stderr.write('curl: (7) Failed to connect to api.github.com port 443\\n');
    process.exit(7);
  }
  status = fixture.tagStatus ?? (fixture.tags.includes(tag) ? 200 : 404);
  if (status === 200) body = JSON.stringify({ tag_name: fixture.wrongTag ?? tag, prerelease: tag.includes('-') });
  else body = JSON.stringify({ message: 'fixture refusal' });
  effective = url;
} else if (url.endsWith('/SHA256SUMS')) {
  body = fixture.sums;
  effective = 'https://release-assets.githubusercontent.com/fixture/SHA256SUMS';
} else {
  const name = url.split('/').at(-1).replace(/-x86_64-unknown-linux-(gnu|musl)$/, '');
  if (!(name in fixture.bodies)) process.exit(22);
  body = fixture.bodies[name];
  effective = 'https://release-assets.githubusercontent.com/fixture/' + name;
}
if (args.includes('-o')) fs.writeFileSync(args[args.indexOf('-o') + 1], body);
if (args.includes('-w')) {
  const format = args[args.indexOf('-w') + 1];
  process.stdout.write(format.includes('http_code') ? String(status) : effective);
}
`, { mode: 0o755 });
// A shim that does not parse would fail every request and hide what the cases mean to prove.
const shimCheck = spawnSync(process.execPath, ["--check", join(shims, "curl")], { encoding: "utf8" });
assert.equal(shimCheck.status, 0, `curl shim does not parse: ${shimCheck.stderr}`);
// Provenance verification is optional. Keep this fixture offline even on machines with gh.
writeFileSync(join(shims, "gh"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
// get.sh asks ldd, which belongs to the system's C library, which images to install, and whether
// the window's libraries load. This one answers as glibc's does, or as musl's does when a case sets
// INSTALLER_LIBC=musl, whatever system the test runs on; with INSTALLER_WINDOW_LIBRARIES=missing
// it reports WebKitGTK missing the way each C library's ldd does.
writeFileSync(join(shims, "ldd"), `#!/bin/sh
libc=\${INSTALLER_LIBC:-glibc}
if [ "$1" = --version ]; then
  if [ "$libc" = musl ]; then
    echo "musl libc (x86_64)" >&2
    echo "Version 1.2.6" >&2
    exit 1
  fi
  echo "ldd (GNU libc) 2.39"
  exit 0
fi
if [ "\${INSTALLER_WINDOW_LIBRARIES:-}" = missing ]; then
  if [ "$libc" = musl ]; then
    echo "Error loading shared library libwebkit2gtk-4.1.so.0: No such file or directory (needed by $1)" >&2
  else
    echo "\tlibwebkit2gtk-4.1.so.0 => not found"
  fi
  exit 1
fi
echo "\tlibwebkit2gtk-4.1.so.0 => /usr/lib/libwebkit2gtk-4.1.so.0"
`, { mode: 0o755 });
function run(label, changes = {}, version, arguments_ = [], settings = {}) {
  const directory = join(area, label), userDirectory = join(directory, "user");
  mkdirSync(userDirectory, { recursive: true });
  const fixture = join(directory, "fixture.json"), requests = join(directory, "requests.txt"), commands = join(directory, "commands.txt");
  writeFileSync(fixture, JSON.stringify({ latest: "https://github.com/sylin-org/ghostlight/releases/tag/v1.2.3", tags: ["v1.2.3", "v2.0.0-rc.1"], sums, bodies, ...changes }));
  const environment = { ...process.env, HOME: userDirectory, PATH: `${shims}:${process.env.PATH}`,
    INSTALLER_FIXTURE: fixture, INSTALLER_REQUESTS: requests, INSTALLER_COMMANDS: commands };
  delete environment.INSTALLER_INSTALL_FAILS;
  delete environment.INSTALLER_LIBC;
  delete environment.INSTALLER_WINDOW_LIBRARIES;
  delete environment.GHOSTLIGHT_VERSION;
  delete environment.GHOSTLIGHT_NO_REGISTER;
  delete environment.GHOSTLIGHT_FROM;
  if (version) environment.GHOSTLIGHT_VERSION = version;
  Object.assign(environment, settings);
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
assert.deepEqual(success.commands, ["downloaded service: deployment install", "downloaded service: install"],
  "the downloaded service installs, then runs install itself");
assert.match(success.stdout, /fixture service: install/, "the output of ghostlight install reaches the person's console");
// A system whose C library is musl (Alpine and its kind) downloads the musl images instead.
const musl = run("musl-downloads", {}, undefined, [], { INSTALLER_LIBC: "musl" });
assert.equal(musl.status, 0, musl.stderr);
assert.deepEqual(musl.requests, [
  "https://github.com/sylin-org/ghostlight/releases/latest",
  ...["SHA256SUMS", ...names.map(name => `${name}-${muslTarget}`)]
    .map(name => `https://github.com/sylin-org/ghostlight/releases/download/v1.2.3/${name}`)
]);
assert.deepEqual(musl.commands, ["downloaded service: deployment install", "downloaded service: install"]);
// With the window's libraries loadable nothing more is said. Without them the install still
// succeeds, and the person is told what the window needs, with Alpine's command on musl.
assert.equal(success.stdout.includes("WebKitGTK"), false, success.stdout);
for (const [label, libc, hint] of [
  ["window-libraries-glibc", "glibc", "ghostlight: Install your system's WebKitGTK 4.1 package to have the window."],
  ["window-libraries-musl", "musl", "ghostlight: On Alpine, add it with: doas apk add webkit2gtk-4.1 libayatana-appindicator"]
]) {
  const windowless = run(label, {}, undefined, [], { INSTALLER_LIBC: libc, INSTALLER_WINDOW_LIBRARIES: "missing" });
  assert.equal(windowless.status, 0, windowless.stderr);
  assert.ok(windowless.stdout.includes("ghostlight: the Ghostlight window needs WebKitGTK 4.1, which is not installed. Browsers and agents work without it."), windowless.stdout);
  assert.ok(windowless.stdout.includes(hint), windowless.stdout);
}
// A failed install leaves no ghostlight command to name, so the downloaded service's doctor reports
// what it found and the script says only that installation did not complete.
const unfinished = run("install-fails", {}, undefined, [], { INSTALLER_INSTALL_FAILS: "1" });
assert.notEqual(unfinished.status, 0);
assert.deepEqual(unfinished.commands,
  ["downloaded service: deployment install", "downloaded service: install", "downloaded service: doctor"],
  "a failed install runs the downloaded service's doctor");
assert.match(unfinished.stdout, /fixture service: doctor/, "the doctor's report reaches the person's console");
assert.equal(unfinished.stderr.trim(), "ghostlight: installation did not complete");
const restored = run("restore", {}, undefined, ["--restore"]);
assert.equal(restored.status, 0, restored.stderr);
assert.deepEqual(restored.commands, ["downloaded service: deployment install --restore", "downloaded service: install"]);
for (const latest of ["https://example.com/releases/tag/v1.2.3", "https://github.com/sylin-org/ghostlight/releases/tag/v1.2.3-rc1"]) {
  const result = run(`invalid-release-${latest.includes("example") ? "host" : "tag"}`, { latest });
  assert.notEqual(result.status, 0);
  assert.equal(result.requests.length, 1);
  assert.equal(existsSync(join(result.userDirectory, ".ghostlight")), false);
}
// A requested version is found by its own tag, never by what is latest, and never touches the
// latest page. A prerelease installs this way.
const pinnedAssets = tag => ["SHA256SUMS", ...names.map(name => `${name}-${target}`)]
  .map(name => `https://github.com/sylin-org/ghostlight/releases/download/${tag}/${name}`);
for (const [label, version] of [["pinned-stable", "1.2.3"], ["pinned-prerelease", "2.0.0-rc.1"]]) {
  const pinned = run(label, {}, version);
  assert.equal(pinned.status, 0, pinned.stderr);
  assert.deepEqual(pinned.requests, [`https://api.github.com/repos/sylin-org/ghostlight/releases/tags/v${version}`, ...pinnedAssets(`v${version}`)]);
  assert.deepEqual(pinned.commands, ["downloaded service: deployment install", "downloaded service: install"]);
  assert.ok(pinned.stdout.includes(`Ghostlight ${version} downloaded`), pinned.stdout);
}
const pinnedOlder = run("pinned-older-than-latest", { latest: "https://github.com/sylin-org/ghostlight/releases/tag/v3.0.0" }, "1.2.3");
assert.equal(pinnedOlder.status, 0, pinnedOlder.stderr);
assert.equal(pinnedOlder.requests.some(request => request.endsWith("/releases/latest")), false, "a pinned install never resolves latest");
// A version that does not exist, or a reply for another tag, installs nothing and says so.
for (const [label, version, changes] of [["absent", "9.9.9", {}], ["absent-prerelease", "2.0.0-rc.2", {}], ["wrong-tag", "1.2.3", { wrongTag: "v1.2.4" }]]) {
  const missing = run(`requested-${label}`, changes, version);
  assert.notEqual(missing.status, 0);
  assert.equal(missing.stderr.trim(), `ghostlight: release v${version} does not exist`);
  assert.equal(missing.requests.length, 1);
  assert.equal(existsSync(join(missing.userDirectory, ".ghostlight")), false);
}
// Any other answer is a failed lookup, never an absent release: a rate limit or a server error for
// a release that exists must not tell the person it does not.
for (const [label, changes, said] of [
  ["rate-limited", { tagStatus: 403 }, "ghostlight: could not look up release v1.2.3 (HTTP 403)"],
  ["server-error", { tagStatus: 503 }, "ghostlight: could not look up release v1.2.3 (HTTP 503)"],
  ["transport-failure", { transportFailure: true }, "ghostlight: could not look up release v1.2.3"]
]) {
  const failed = run(`requested-${label}`, changes, "1.2.3");
  assert.notEqual(failed.status, 0);
  assert.equal(failed.stderr.includes("does not exist"), false, failed.stderr);
  assert.ok(failed.stderr.trim().endsWith(said), failed.stderr);
  assert.equal(failed.requests.length, 1);
  assert.equal(existsSync(join(failed.userDirectory, ".ghostlight")), false);
}
const malformed = run("requested-malformed", {}, "latest");
assert.notEqual(malformed.status, 0);
assert.equal(malformed.requests.length, 0);
for (const damaged of names) {
  const corrupt = run(`corrupt-${damaged}`, { bodies: { ...bodies, [damaged]: "wrong bytes" } });
  assert.notEqual(corrupt.status, 0);
  assert.equal(existsSync(join(corrupt.userDirectory, ".ghostlight/bin")), false, `a corrupt ${damaged} installs nothing`);
  assert.deepEqual(corrupt.commands, [], "a corrupt download runs nothing");
}
// A release bundle on this machine installs without asking the network for anything. Its folder is
// the release workflow's artifact (SHA256SUMS beside assets/, with a manifest naming the version) or
// a folder of release downloads (SHA256SUMS beside the images). The folder's name has a space.
function bundle(label, { layout = "candidate", images = bodies, checksums = sums, system = target } = {}) {
  const folder = join(area, label, "release candidate");
  const assets = layout === "candidate" ? join(folder, "assets") : folder;
  mkdirSync(assets, { recursive: true });
  if (checksums !== null) writeFileSync(join(folder, "SHA256SUMS"), checksums);
  for (const [name, body] of Object.entries(images)) writeFileSync(join(assets, `${name}-${system}`), body);
  writeFileSync(join(assets, "ghostlight-x86_64-pc-windows-msvc.exe"), "the other system's image");
  if (layout === "candidate") {
    writeFileSync(join(folder, "release-candidate.json"),
      JSON.stringify({ schemaVersion: 1, version: "2.0.0", status: "release-candidate", artifacts: [] }, null, 2) + "\n");
  }
  return folder;
}
const candidate = bundle("local-bundle");
const fromBundle = run("local-bundle", {}, undefined, [], { GHOSTLIGHT_FROM: candidate });
assert.equal(fromBundle.status, 0, fromBundle.stderr);
assert.deepEqual(fromBundle.requests, [], "a local bundle asks the network for nothing");
assert.deepEqual(fromBundle.commands, ["downloaded service: deployment install", "downloaded service: install"]);
assert.ok(fromBundle.stdout.includes(`Ghostlight 2.0.0 copied from ${candidate}. Checksums verified.`), fromBundle.stdout);
for (const name of names) {
  assert.equal(readFileSync(join(fromBundle.userDirectory, ".ghostlight/bin", name), "utf8"), bodies[name]);
}
const downloads = bundle("local-downloads", { layout: "flat" });
const fromDownloads = run("local-downloads", {}, undefined, ["--restore"], { GHOSTLIGHT_FROM: downloads });
assert.equal(fromDownloads.status, 0, fromDownloads.stderr);
assert.deepEqual(fromDownloads.requests, []);
assert.deepEqual(fromDownloads.commands, ["downloaded service: deployment install --restore", "downloaded service: install"]);
assert.ok(fromDownloads.stdout.includes(`Ghostlight copied from ${downloads}. Checksums verified.`), fromDownloads.stdout);
// A musl system takes the musl images from a bundle. A bundle holding only those cannot serve a
// glibc system, which says which image is missing.
const muslBundle = run("local-bundle-musl", {}, undefined, [],
  { GHOSTLIGHT_FROM: bundle("local-bundle-musl", { system: muslTarget }), INSTALLER_LIBC: "musl" });
assert.equal(muslBundle.status, 0, muslBundle.stderr);
assert.deepEqual(muslBundle.commands, ["downloaded service: deployment install", "downloaded service: install"]);
const muslOnly = run("local-bundle-musl-on-glibc", {}, undefined, [],
  { GHOSTLIGHT_FROM: bundle("local-bundle-musl-on-glibc", { system: muslTarget }) });
assert.notEqual(muslOnly.status, 0);
assert.ok(muslOnly.stderr.trim().endsWith(` has no ghostlight-${target}`), muslOnly.stderr);
assert.deepEqual(muslOnly.commands, []);
const unregistered = run("local-no-register", {}, undefined, [], { GHOSTLIGHT_FROM: bundle("local-no-register"), GHOSTLIGHT_NO_REGISTER: "1" });
assert.equal(unregistered.status, 0, unregistered.stderr);
assert.deepEqual(unregistered.commands, ["downloaded service: deployment install"]);
assert.ok(unregistered.stdout.includes("Ghostlight 2.0.0 is installed. Run 'ghostlight install'"), unregistered.stdout);
// Whatever SHA256SUMS does not bind, a bundle installs nothing and runs nothing.
const withoutLine = asset => sums.split("\n").filter(line => !line.endsWith(`  ${asset}`)).join("\n");
for (const [label, options, said] of [
  ["local-corrupt", { images: { ...bodies, "ghostlight-service": "wrong bytes" } }, `ghostlight: checksum verification failed for ghostlight-service-${target}`],
  ["local-missing-image", { images: { ghostlight: bodies.ghostlight, "ghostlight-service": bodies["ghostlight-service"] } }, ` has no ghostlight-desktop-${target}`],
  ["local-unbound", { checksums: withoutLine(`ghostlight-desktop-${target}`) }, `ghostlight: invalid checksum for ghostlight-desktop-${target}`],
  ["local-no-sums", { checksums: null }, "release candidate has no SHA256SUMS"]
]) {
  const refused = run(label, {}, undefined, [], { GHOSTLIGHT_FROM: bundle(label, options) });
  assert.notEqual(refused.status, 0);
  assert.ok(refused.stderr.trim().endsWith(said), refused.stderr);
  assert.deepEqual(refused.requests, []);
  assert.deepEqual(refused.commands, [], `${label} runs nothing`);
  assert.equal(existsSync(join(refused.userDirectory, ".ghostlight")), false, `${label} installs nothing`);
}
const combined = run("local-and-version", {}, "2.0.0", [], { GHOSTLIGHT_FROM: bundle("local-and-version") });
assert.notEqual(combined.status, 0);
assert.equal(combined.stderr.trim(), "ghostlight: GHOSTLIGHT_FROM and GHOSTLIGHT_VERSION cannot be combined");
assert.deepEqual(combined.requests, []);
const nowhere = join(area, "no such folder");
const absentFolder = run("local-absent", {}, undefined, [], { GHOSTLIGHT_FROM: nowhere });
assert.notEqual(absentFolder.status, 0);
assert.equal(absentFolder.stderr.trim(), `ghostlight: GHOSTLIGHT_FROM is not a folder: ${nowhere}`);
assert.deepEqual(absentFolder.requests, []);
console.log(`PASS shell installer: requested version by tag (prerelease too), latest release, CDN delivery, all three images, the musl images on a musl system, the window's missing libraries named, install in the console, a failed install reported by the downloaded doctor, restore, invalid release, absent, unreachable or malformed requested version, checksum refusal, a local bundle in both layouts with nothing asked of the network, and its refusals\nEvidence: ${area}`);
