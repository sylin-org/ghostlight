import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  assertAllowedDownload,
  assetNames,
  compareVersions,
  edgeArguments,
  ensureBinary,
  executableNames,
  neverInstalls,
  publishedAssetNames,
  releaseTarget,
  validateChecksums,
} from "../bin/ghostlight.js";

test("a real offline npm install invokes the launcher through the platform bin entry", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ghostlight-npm-bin-"));
  try {
    const packageRoot = fileURLToPath(new URL("../", import.meta.url));
    const stage = join(directory, "package");
    await mkdir(stage);
    await cp(join(packageRoot, "bin"), join(stage, "bin"), { recursive: true });
    await cp(join(packageRoot, "package.json"), join(stage, "package.json"));
    const metadata = JSON.parse(await readFile(join(stage, "package.json"), "utf8"));
    // A deliberately incomplete checksum set stops the actual launcher before any download.
    // This proves npm's installed entry point, not candidate binary delivery or startup.
    await writeFile(join(stage, "checksums.json"), JSON.stringify({ version: metadata.version,
      algorithm: "sha256", binaries: {} }));
    // npm exposes its exact CLI when it runs this test. A direct Node invocation on Linux
    // resolves npm through PATH; its installation need not live beside the Node executable.
    const npmCommand = process.env.npm_execpath
      ? [process.execPath, process.env.npm_execpath]
      : process.platform === "win32"
        ? [process.execPath, join(dirname(process.execPath), "node_modules/npm/bin/npm-cli.js")]
        : ["npm"];
    const environment = { ...process.env, npm_config_cache: join(directory, "cache"),
      GHOSTLIGHT_TEST_HOME: join(directory, "ghostlight-home") };
    const run = (command, args) => spawnSync(command, args, { cwd: directory, env: environment,
      windowsHide: true, encoding: "utf8", timeout: 30000 });
    const runNpm = args => run(npmCommand[0], [...npmCommand.slice(1), ...args]);
    const packed = runNpm(["pack", stage, "--json", "--ignore-scripts", "--offline", "--pack-destination", directory]);
    assert.equal(packed.status, 0, packed.stderr || packed.error?.message);
    const archive = join(directory, JSON.parse(packed.stdout)[0].filename);
    const installed = runNpm(["install", archive, "--prefix", directory,
      "--offline", "--ignore-scripts", "--no-audit", "--no-fund"]);
    assert.equal(installed.status, 0, installed.stderr || installed.error?.message);
    const bin = join(directory, "node_modules/.bin/ghostlight");
    const launched = process.platform === "win32"
      ? run("pwsh", ["-NoProfile", "-File", `${bin}.ps1`, "--version"])
      : run(bin, ["--version"]);
    assert.equal(launched.status, 1, launched.error?.message);
    assert.match(launched.stderr, /complete binary checksum set/);
    assert.equal(launched.stdout, "", "Launcher failures must not pollute MCP stdout");
  } finally {
    assert.equal(resolve(dirname(directory)), resolve(tmpdir()));
    await rm(directory, { recursive: true, force: true });
  }
});

test("the published platforms resolve to exact release targets", () => {
  assert.equal(releaseTarget("win32", "x64"), "x86_64-pc-windows-msvc");
  assert.equal(releaseTarget("linux", "x64"), "x86_64-unknown-linux-gnu");
  assert.throws(() => releaseTarget("linux", "arm64"), /does not publish/);
});

test("every platform downloads the complete unit of edge, service and shell", () => {
  assert.deepEqual(assetNames("linux", "x64"), [
    "ghostlight-x86_64-unknown-linux-gnu",
    "ghostlight-service-x86_64-unknown-linux-gnu",
    "ghostlight-desktop-x86_64-unknown-linux-gnu",
  ]);
  assert.ok(assetNames("win32", "x64").every((name) => name.endsWith(".exe")));
});

test("verified assets are cached under exact trusted sibling names", () => {
  assert.deepEqual(executableNames("linux", "x64"), [
    "ghostlight",
    "ghostlight-service",
    "ghostlight-desktop",
  ]);
  assert.deepEqual(executableNames("win32", "x64"), [
    "ghostlight.exe",
    "ghostlight-service.exe",
    "ghostlight-desktop.exe",
  ]);
});

test("a bare launch is the MCP relay and every other request reaches the edge unchanged", () => {
  assert.deepEqual(edgeArguments([]), ["mcp"]);
  assert.deepEqual(edgeArguments(["install"]), ["install"]);
  assert.deepEqual(edgeArguments(["mcp"]), ["mcp"]);
});

test("the checksum manifest must bind the exact version and file set", () => {
  const names = publishedAssetNames();
  const binaries = Object.fromEntries(names.map((name) => [name, "a".repeat(64)]));
  assert.doesNotThrow(() =>
    validateChecksums({ version: "1.0.0", algorithm: "sha256", binaries }, "1.0.0", names),
  );
  assert.throws(
    () => validateChecksums({ version: "0.8.0", algorithm: "sha256", binaries }, "1.0.0", names),
    /does not match/,
  );
  assert.throws(
    () => validateChecksums({ version: "1.0.0", algorithm: "sha256", binaries: {} }, "1.0.0", names),
    /complete binary checksum set/,
  );
});

test("downloads accept only the fixed HTTPS release host chain", () => {
  for (const host of [
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
  ]) {
    assert.doesNotThrow(() => assertAllowedDownload(`https://${host}/asset`));
  }
  assert.throws(() => assertAllowedDownload("http://github.com/asset"), /untrusted/);
  assert.throws(() => assertAllowedDownload("https://example.com/asset"), /untrusted/);
});

test("a matching cached binary is reused without a network request", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ghostlight-npm-cache-"));
  const path = join(directory, "ghostlight");
  const bytes = Buffer.from("trusted-binary");
  const expectedHash = createHash("sha256").update(bytes).digest("hex");
  await writeFile(path, bytes);
  let fetched = false;
  let announced = false;
  try {
    const changed = await ensureBinary({
      path,
      url: "https://github.com/sylin-org/ghostlight/releases/download/v1.0.0/ghostlight",
      expectedHash,
      fetchImpl: async () => {
        fetched = true;
      },
      onDownload: () => {
        announced = true;
      },
    });
    assert.equal(changed, false);
    assert.equal(fetched, false);
    assert.equal(announced, false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("a corrupt cached binary is replaced only by verified bytes", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ghostlight-npm-repair-"));
  const path = join(directory, "ghostlight");
  const trusted = Buffer.from("fresh-trusted-binary");
  const expectedHash = createHash("sha256").update(trusted).digest("hex");
  await writeFile(path, "corrupt");
  let announced = false;
  try {
    const changed = await ensureBinary({
      path,
      url: "https://github.com/sylin-org/ghostlight/releases/download/v1.0.0/ghostlight",
      expectedHash,
      fetchImpl: async () => new Response(trusted, { status: 200 }),
      onDownload: () => {
        announced = true;
      },
    });
    assert.equal(changed, true);
    assert.equal(announced, true);
    assert.deepEqual(await readFile(path), trusted);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("unverified download bytes never replace the cache", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ghostlight-npm-reject-"));
  const path = join(directory, "ghostlight");
  await writeFile(path, "old-corrupt");
  try {
    await assert.rejects(
      ensureBinary({
        path,
        url: "https://github.com/sylin-org/ghostlight/releases/download/v1.0.0/ghostlight",
        expectedHash: "a".repeat(64),
        fetchImpl: async () => new Response("wrong", { status: 200 }),
      }),
      /failed checksum/,
    );
    assert.equal(await readFile(path, "utf8"), "old-corrupt");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});


test("only doctor, status, help and version never install", () => {
  for (const command of ["doctor", "status", "help", "--help", "-h", "--version", "-V"]) {
    assert.equal(neverInstalls([command, "--json"]), true, command);
  }
  for (const arguments_ of [["install", "--help"], ["install", "-h"], ["uninstall", "--help"], ["uninstall", "-h"]]) {
    assert.equal(neverInstalls(arguments_), true, arguments_.join(" "));
  }
  for (const arguments_ of [[], ["install"], ["open"], ["serve"], ["diagnostics", "path"],
    ["policy", "validate"], ["call", "--catalog"], ["uninstall"], ["install", "--force"], ["deployment", "install"], ["unknown"]]) {
    assert.equal(neverInstalls(arguments_), false, arguments_.join(" ") || "(no arguments)");
  }
});

test("versions order numerically and a pre-release sorts below its release", () => {
  assert.ok(compareVersions("2.0.0", "1.9.9") > 0);
  assert.ok(compareVersions("1.3.9", "1.3.15") < 0);
  assert.equal(compareVersions("2.0.0", "2.0.0"), 0);
  assert.ok(compareVersions("2.0.0-rc.1", "2.0.0") < 0);
  assert.equal(compareVersions("not-a-version", "2.0.0"), null);
  assert.equal(compareVersions(undefined, "2.0.0"), null);
});

const PACKAGE_VERSION = "2.0.0";

/** A copy of the launcher with a package version and checksums for fixture binaries. */
async function launcherFixture() {
  const area = await mkdtemp(join(tmpdir(), "ghostlight-launch-"));
  const packageRoot = join(area, "package");
  await mkdir(packageRoot);
  await cp(fileURLToPath(new URL("../bin", import.meta.url)), join(packageRoot, "bin"), { recursive: true });
  const bytes = Object.fromEntries(publishedAssetNames().map(name => [name, Buffer.from(`${PACKAGE_VERSION}:${name}`)]));
  await writeFile(join(packageRoot, "package.json"), JSON.stringify({ version: PACKAGE_VERSION, type: "module" }));
  await writeFile(join(packageRoot, "checksums.json"), JSON.stringify({ version: PACKAGE_VERSION, algorithm: "sha256",
    binaries: Object.fromEntries(Object.entries(bytes).map(([name, body]) => [name, createHash("sha256").update(body).digest("hex")])) }));
  const { prepareLaunch } = await import(pathToFileURL(join(packageRoot, "bin/ghostlight.js")).href);
  const home = join(area, "home");
  const bin = join(home, "bin");
  const names = executableNames("linux", "x64");
  const activity = { downloads: [], installs: [] };
  const options = {
    platform: "linux", architecture: "x64", cacheRoot: home,
    fetchImpl: async url => {
      activity.downloads.push(url.split("/").at(-1));
      return new Response(bytes[url.split("/").at(-1)]);
    },
    installImpl: async (candidate, destination) => {
      activity.installs.push({ candidate, destination });
      await mkdir(destination, { recursive: true });
      for (const name of names) await cp(join(dirname(candidate), name), join(destination, name));
      await writeFile(join(destination, "install.json"), JSON.stringify({ service_version: PACKAGE_VERSION }));
    },
  };
  const install = async (version, contents = "installed", channel = undefined) => {
    await mkdir(bin, { recursive: true });
    for (const name of names) await writeFile(join(bin, name), `${contents}:${name}`);
    if (version !== undefined) {
      await writeFile(join(bin, "install.json"), JSON.stringify({ service_version: version, ...(channel ? { channel } : {}) }));
    }
  };
  const cleanup = () => rm(area, { recursive: true, force: true });
  return { area, home, bin, names, options, activity, install, prepareLaunch, cleanup };
}

const NO_DOWNLOAD = async () => { throw new Error("unexpected download"); };

/** What a command that installs nothing downloads: the service, which answers the command itself. */
const STAGED_SERVICE = ["ghostlight-service-x86_64-unknown-linux-gnu"];

test("a launch with nothing installed downloads, verifies, installs from the stage, then runs the installed edge", async () => {
  const f = await launcherFixture();
  try {
    const launch = await f.prepareLaunch({ ...f.options, arguments_: [] });
    assert.equal(launch.executable, join(f.bin, "ghostlight"));
    assert.deepEqual(launch.arguments_, ["mcp"], "no arguments is the MCP relay");
    assert.equal(f.activity.downloads.length, 3);
    assert.equal(f.activity.installs.length, 1);
    assert.equal(f.activity.installs[0].destination, f.bin);
    assert.notEqual(dirname(f.activity.installs[0].candidate), f.bin);
    assert.equal(basename(f.activity.installs[0].candidate), "ghostlight-service", "the staged service installs");
    assert.deepEqual((await readdir(f.home)).sort(), ["bin"], "the stage is removed");
    const open = await f.prepareLaunch({ ...f.options, arguments_: ["open"], fetchImpl: NO_DOWNLOAD });
    assert.equal(open.executable, join(f.bin, "ghostlight"));
    assert.deepEqual(open.arguments_, ["open"]);
    assert.equal(f.activity.installs.length, 1, "the second launch finds the install current");
  } finally {
    await f.cleanup();
  }
});

test("a launch installs when the installed service is older", async () => {
  for (const record of ["1.3.15", "1.9.9", "2.0.0-rc.1"]) {
    const f = await launcherFixture();
    try {
      await f.install(record);
      const launch = await f.prepareLaunch({ ...f.options, arguments_: ["serve"] });
      assert.equal(launch.executable, join(f.bin, "ghostlight"));
      assert.equal(f.activity.installs.length, 1, record);
      assert.match(await readFile(join(f.bin, "ghostlight"), "utf8"), /^2\.0\.0:/);
    } finally {
      await f.cleanup();
    }
  }
});

test("a launch installs when the install record is absent or unreadable", async () => {
  for (const record of [undefined, "{ not json"]) {
    const f = await launcherFixture();
    try {
      await f.install(undefined);
      if (record) await writeFile(join(f.bin, "install.json"), record);
      await f.prepareLaunch({ ...f.options, arguments_: ["install"] });
      assert.equal(f.activity.installs.length, 1, String(record));
    } finally {
      await f.cleanup();
    }
  }
});

test("a launch runs the installed set untouched when it is the same or newer", async () => {
  for (const record of ["2.0.0", "2.0.1", "2.1.0", "10.0.0"]) {
    const f = await launcherFixture();
    try {
      await f.install(record);
      const launch = await f.prepareLaunch({ ...f.options, arguments_: [], fetchImpl: NO_DOWNLOAD });
      assert.equal(launch.executable, join(f.bin, "ghostlight"));
      assert.deepEqual(launch.arguments_, ["mcp"]);
      assert.equal(f.activity.installs.length, 0, record);
      assert.match(await readFile(join(f.bin, "ghostlight"), "utf8"), /^installed:/,
        "installed files are not hashed or replaced");
    } finally {
      await f.cleanup();
    }
  }
});

test("a launch installs when the edge or the service it would run is missing", async () => {
  for (const missing of ["ghostlight", "ghostlight-service"]) {
    const f = await launcherFixture();
    try {
      await f.install("2.0.0");
      await rm(join(f.bin, missing));
      await f.prepareLaunch({ ...f.options, arguments_: [] });
      assert.equal(f.activity.installs.length, 1, missing);
    } finally {
      await f.cleanup();
    }
  }
});

test("the development channel keeps the installed set whatever its version", async () => {
  const f = await launcherFixture();
  try {
    await f.install("1.0.0", "installed", "development");
    const launch = await f.prepareLaunch({ ...f.options, arguments_: [], fetchImpl: NO_DOWNLOAD });
    assert.equal(launch.executable, join(f.bin, "ghostlight"));
    assert.deepEqual(launch.arguments_, ["mcp"]);
    assert.equal(f.activity.installs.length, 0);
  } finally {
    await f.cleanup();
  }
});

test("a development installation whose edge is gone names the way back and installs nothing", async () => {
  const f = await launcherFixture();
  try {
    await f.install("1.0.0", "installed", "development");
    await rm(join(f.bin, "ghostlight"));
    await assert.rejects(
      f.prepareLaunch({ ...f.options, arguments_: [], fetchImpl: NO_DOWNLOAD }),
      error => error.message.includes(`development build, but ${join(f.bin, "ghostlight")} is missing`)
        && error.message.includes("npx ghostlight deployment install --restore")
        && error.message.includes("-Restore")
        && error.message.includes("get.sh | sh -s -- --restore"),
    );
    assert.equal(f.activity.installs.length, 0);
  } finally {
    await f.cleanup();
  }
});

test("a release channel record does not keep an older installed set", async () => {
  const f = await launcherFixture();
  try {
    await f.install("1.0.0", "installed", "release");
    await f.prepareLaunch({ ...f.options, arguments_: [] });
    assert.equal(f.activity.installs.length, 1);
  } finally {
    await f.cleanup();
  }
});

test("deployment runs the package's own verified service image, whatever is installed", async () => {
  for (const [version, channel] of [[undefined, undefined], ["1.0.0", "development"], ["9.0.0", "release"]]) {
    const f = await launcherFixture();
    try {
      if (version) await f.install(version, "installed", channel);
      const arguments_ = ["deployment", "install", "--restore"];
      const launch = await f.prepareLaunch({ ...f.options, arguments_ });
      assert.notEqual(dirname(launch.executable), f.bin, "the installed image is never the one that installs");
      assert.equal(basename(launch.executable), "ghostlight-service");
      assert.equal(dirname(launch.executable).startsWith(f.home), true);
      assert.deepEqual(launch.arguments_, arguments_);
      assert.match(await readFile(launch.executable, "utf8"), /^2\.0\.0:/, "the package's verified download");
      assert.equal(f.activity.installs.length, 0, "the launcher installs nothing itself");
      assert.equal(f.activity.downloads.length, 3, "the whole set is staged beside the service that installs it");
      if (version) {
        assert.match(await readFile(join(f.bin, "ghostlight"), "utf8"), /^installed:/, "the installed set is untouched");
      }
      await launch.cleanup();
      assert.deepEqual((await readdir(f.home)).filter(name => name !== "bin"), [], "the stage goes away");
    } finally {
      await f.cleanup();
    }
  }
});

test("a bad download never touches the installed service", async () => {
  const f = await launcherFixture();
  try {
    await f.install("1.3.15");
    await assert.rejects(f.prepareLaunch({ ...f.options, arguments_: [],
      fetchImpl: async url => new Response(url.endsWith("ghostlight-desktop-x86_64-unknown-linux-gnu")
        ? "wrong" : "right") }), /failed checksum/);
    assert.equal(f.activity.installs.length, 0);
    assert.match(await readFile(join(f.bin, "ghostlight"), "utf8"), /^installed:/);
    assert.deepEqual((await readdir(f.home)).sort(), ["bin"], "the failed stage is removed");
  } finally {
    await f.cleanup();
  }
});

test("never-install commands run the installed edge whatever its version and never install", async () => {
  for (const arguments_ of [["doctor"], ["status", "--json"], ["help"], ["--help"], ["-h"], ["--version"], ["-V"],
    ["install", "--help"], ["install", "-h"], ["uninstall", "--help"], ["uninstall", "-h"]]) {
    const f = await launcherFixture();
    try {
      await f.install("1.0.0");
      const launch = await f.prepareLaunch({ ...f.options, arguments_, fetchImpl: NO_DOWNLOAD });
      assert.equal(launch.executable, join(f.bin, "ghostlight"));
      assert.deepEqual(launch.arguments_, arguments_);
      assert.equal(f.activity.installs.length, 0, arguments_.join(" "));
    } finally {
      await f.cleanup();
    }
  }
});

test("subcommand help with nothing installed runs the staged service and installs nothing", async () => {
  for (const arguments_ of [["install", "--help"], ["uninstall", "-h"]]) {
    const f = await launcherFixture();
    try {
      const launch = await f.prepareLaunch({ ...f.options, arguments_ });
      assert.equal(f.activity.installs.length, 0, arguments_.join(" "));
      assert.deepEqual(f.activity.downloads, STAGED_SERVICE);
      assert.equal(basename(launch.executable), "ghostlight-service");
      assert.deepEqual(launch.arguments_, arguments_);
      await launch.cleanup();
      assert.deepEqual(await readdir(f.home), [], "nothing is installed");
    } finally {
      await f.cleanup();
    }
  }
});

test("every other command with nothing installed installs the complete set first", async () => {
  for (const arguments_ of [["call", "--catalog"], ["policy", "validate", "p.json"], ["diagnostics", "path"], ["unknown"]]) {
    const f = await launcherFixture();
    try {
      const launch = await f.prepareLaunch({ ...f.options, arguments_ });
      assert.equal(launch.executable, join(f.bin, "ghostlight"));
      assert.deepEqual(launch.arguments_, arguments_);
      assert.equal(f.activity.downloads.length, 3, arguments_[0]);
      assert.equal(f.activity.installs.length, 1, arguments_[0]);
      assert.deepEqual((await readdir(f.home)).sort(), ["bin"], "the stage is removed");
    } finally {
      await f.cleanup();
    }
  }
});

test("a never-install command with nothing installed runs the verified staged service and installs nothing", async () => {
  const f = await launcherFixture();
  try {
    const launch = await f.prepareLaunch({ ...f.options, arguments_: ["--version"] });
    assert.equal(f.activity.installs.length, 0);
    assert.deepEqual(f.activity.downloads, STAGED_SERVICE);
    assert.deepEqual(launch.arguments_, ["--version"]);
    assert.equal(basename(launch.executable), "ghostlight-service");
    assert.equal(dirname(launch.executable).startsWith(f.home), true);
    assert.match(await readFile(launch.executable, "utf8"), /^2\.0\.0:/);
    await launch.cleanup();
    assert.deepEqual(await readdir(f.home), [], "the stage goes away and nothing is installed");
  } finally {
    await f.cleanup();
  }
});
