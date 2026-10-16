#!/usr/bin/env node

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, existsSync, realpathSync } from "node:fs";
import { chmod, mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const RELEASE_ROOT = "https://github.com/sylin-org/ghostlight/releases/download";
const ALLOWED_DOWNLOAD_HOSTS = new Set([
  "github.com",
  "objects.githubusercontent.com",
  "release-assets.githubusercontent.com",
  "github-releases.githubusercontent.com",
]);
/** The images of one release, in the order the launcher stages them: the edge, the service, the shell. */
const EXECUTABLES = [
  "ghostlight",
  "ghostlight-service",
  "ghostlight-desktop",
];
/** Positions in `EXECUTABLES` of the two images every launch needs. */
const EDGE = 0;
const SERVICE = 1;
/** The edge command that starts the MCP stdio relay; a launch with no arguments means it. */
const MCP_COMMAND = "mcp";

/** Environment variable that moves the Ghostlight home. It exists for journeys only. */
const TEST_HOME_VARIABLE = "GHOSTLIGHT_TEST_HOME";
/** Record the installer writes into the install directory once a service is running. */
const INSTALL_RECORD_NAME = "install.json";
/** The channel `install.json` records for a build the development loop installed. */
const DEVELOPMENT_CHANNEL = "development";
/** First argument of the installer command, which always runs this package's own verified service. */
const DEPLOYMENT_COMMAND = "deployment";
/** First arguments of the commands that never install (ADR-0197 Decision 13). */
const NEVER_INSTALL_COMMANDS = new Set(["doctor", "status", "help", "--help", "-h", "--version", "-V"]);
/** Commands whose own help the service prints when exactly one help flag follows them. */
const HELP_COMMANDS = new Set(["install", "uninstall"]);
/** The flags that ask a command for its help. */
const HELP_FLAGS = new Set(["--help", "-h"]);

export function releaseTarget(platform = process.platform, architecture = process.arch) {
  const key = `${platform}/${architecture}`;
  const targets = {
    "win32/x64": "x86_64-pc-windows-msvc",
    "linux/x64": "x86_64-unknown-linux-gnu",
  };
  const target = targets[key];
  if (!target) {
    throw new Error(`Ghostlight does not publish binaries for ${key}`);
  }
  return target;
}

export function assetNames(platform = process.platform, architecture = process.arch) {
  const target = releaseTarget(platform, architecture);
  const extension = platform === "win32" ? ".exe" : "";
  return EXECUTABLES.map((name) => `${name}-${target}${extension}`);
}

export function executableNames(platform = process.platform, architecture = process.arch) {
  releaseTarget(platform, architecture);
  const extension = platform === "win32" ? ".exe" : "";
  return EXECUTABLES.map((name) => `${name}${extension}`);
}

export function publishedAssetNames() {
  return [
    ...assetNames("win32", "x64"),
    ...assetNames("linux", "x64"),
  ];
}

/** The arguments the edge receives: a launch with none is the MCP relay, and every other launch passes through. */
export function edgeArguments(arguments_) {
  return arguments_.length === 0 ? [MCP_COMMAND] : arguments_;
}

/**
 * True for the commands that never install: `doctor`, `status`, `help`, `--version`, and the
 * help of `install` and `uninstall` (`install --help`). Every other command, including no
 * arguments (the MCP relay), needs the installed set.
 */
export function neverInstalls(arguments_) {
  if (arguments_.length === 2 && HELP_COMMANDS.has(arguments_[0]) && HELP_FLAGS.has(arguments_[1])) {
    return true;
  }
  return arguments_.length > 0 && NEVER_INSTALL_COMMANDS.has(arguments_[0]);
}

/**
 * Orders two release versions. Returns a negative, zero or positive number, or null when either
 * is not `major.minor.patch` with an optional pre-release suffix. A pre-release sorts below its
 * release.
 */
export function compareVersions(left, right) {
  const parse = (value) => {
    const match = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(String(value));
    return match ? { numbers: match.slice(1, 4).map(Number), pre: match[4] } : null;
  };
  const a = parse(left);
  const b = parse(right);
  if (!a || !b) {
    return null;
  }
  for (let index = 0; index < 3; index++) {
    if (a.numbers[index] !== b.numbers[index]) {
      return a.numbers[index] - b.numbers[index];
    }
  }
  if (a.pre === b.pre) {
    return 0;
  }
  if (a.pre === undefined) {
    return 1;
  }
  if (b.pre === undefined) {
    return -1;
  }
  return a.pre < b.pre ? -1 : 1;
}

/** Reads `install.json` from the install directory; absent or unreadable records are null. */
export async function readInstallRecord(directory) {
  try {
    return JSON.parse(await readFile(join(directory, INSTALL_RECORD_NAME), "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT" || error instanceof SyntaxError) {
      return null;
    }
    throw error;
  }
}

/**
 * True when the installed service is the same as or newer than the package. An absent,
 * unreadable or unordered record counts as older, so the next launch installs.
 */
export async function installedSetIsCurrent(directory, packageVersion) {
  const record = await readInstallRecord(directory);
  const order = compareVersions(record?.service_version, packageVersion);
  return order !== null && order >= 0;
}

export function validateChecksums(manifest, version, names) {
  if (manifest?.version !== version || manifest?.algorithm !== "sha256") {
    throw new Error("the npm package checksum manifest does not match its version");
  }
  const expected = new Set(names);
  const actual = new Set(Object.keys(manifest.binaries ?? {}));
  if (expected.size !== actual.size || [...expected].some((name) => !actual.has(name))) {
    throw new Error("the npm package was not prepared with the complete binary checksum set");
  }
  for (const [name, hash] of Object.entries(manifest.binaries)) {
    if (!expected.has(name) || !/^[0-9a-f]{64}$/.test(hash)) {
      throw new Error(`the npm package has an invalid checksum for ${name}`);
    }
  }
}

export function assertAllowedDownload(url) {
  const parsed = new URL(url);
  if (parsed.protocol !== "https:" || !ALLOWED_DOWNLOAD_HOSTS.has(parsed.hostname)) {
    throw new Error(`refusing an untrusted Ghostlight download location: ${parsed.origin}`);
  }
}

export async function sha256(path) {
  const digest = createHash("sha256");
  for await (const chunk of createReadStream(path)) {
    digest.update(chunk);
  }
  return digest.digest("hex");
}

async function download(url, fetchImpl = globalThis.fetch, redirects = 0) {
  assertAllowedDownload(url);
  if (redirects > 5) {
    throw new Error("too many redirects while downloading Ghostlight");
  }
  const response = await fetchImpl(url, { redirect: "manual" });
  if (response.status >= 300 && response.status < 400) {
    const location = response.headers.get("location");
    if (!location) {
      throw new Error(`Ghostlight download redirected without a location (${response.status})`);
    }
    return download(new URL(location, url).href, fetchImpl, redirects + 1);
  }
  if (!response.ok) {
    throw new Error(`Ghostlight download failed (${response.status})`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

export async function ensureBinary({
  path,
  url,
  expectedHash,
  fetchImpl = globalThis.fetch,
  onDownload = () => {},
}) {
  try {
    if ((await sha256(path)) === expectedHash) {
      return false;
    }
  } catch (error) {
    if (error?.code !== "ENOENT") {
      throw error;
    }
  }

  onDownload();
  const bytes = await download(url, fetchImpl);
  const observed = createHash("sha256").update(bytes).digest("hex");
  if (observed !== expectedHash) {
    throw new Error(`downloaded Ghostlight binary failed checksum verification: ${url}`);
  }
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.${Date.now()}.tmp`;
  try {
    await writeFile(temporary, bytes, { mode: 0o755, flag: "wx" });
    await rm(path, { force: true });
    await rename(temporary, path);
    if (process.platform !== "win32") {
      await chmod(path, 0o755);
    }
  } finally {
    await rm(temporary, { force: true });
  }
  return true;
}

/** Downloads and verifies the given executables into a fresh stage directory under `cacheRoot`. */
async function stageDownloads({ indexes, packageJson, manifest, platform, architecture, cacheRoot, fetchImpl, reporter }) {
  const assets = assetNames(platform, architecture);
  const executables = executableNames(platform, architecture);
  await mkdir(cacheRoot, { recursive: true });
  const stage = await mkdtemp(join(cacheRoot, ".download-"));
  try {
    for (const [position, index] of indexes.entries()) {
      const changed = await ensureBinary({
        path: join(stage, executables[index]),
        url: `${RELEASE_ROOT}/v${packageJson.version}/${assets[index]}`,
        expectedHash: manifest.binaries[assets[index]],
        fetchImpl,
        onDownload: () => reporter(
          `Downloading Ghostlight ${packageJson.version} for ${platform}/${architecture} (${position + 1}/${indexes.length})...`,
        ),
      });
      if (changed) {
        reporter(`Verified ${EXECUTABLES[index]}.`);
      }
    }
  } catch (error) {
    await rm(stage, { recursive: true, force: true });
    throw error;
  }
  return stage;
}

/**
 * Decides what to run and makes it ready. `deployment` (`deployment install`, also with
 * `--restore`) runs this package's own verified service from a staged copy, never the installed
 * one, because the installer installs the images it belongs to. Every other command except the
 * never-install ones (see `neverInstalls`) runs the installed edge when `install.json` is the
 * same as or newer than this package, or records the development channel; otherwise the verified
 * download installs itself first. The edge hands every command but `mcp` to the service beside
 * it, and a launch with no arguments is `mcp`. The never-install commands run the installed edge,
 * or a verified staged edge and service that install nothing.
 *
 * Resolves to the executable, its arguments, and `cleanup` for any staged files.
 */
export async function prepareLaunch({
  arguments_ = process.argv.slice(2),
  platform = process.platform,
  architecture = process.arch,
  cacheRoot = process.env[TEST_HOME_VARIABLE] ?? join(homedir(), ".ghostlight"),
  fetchImpl = globalThis.fetch,
  reporter = () => {},
  installImpl = installDownload,
} = {}) {
  const home = resolve(cacheRoot);
  const directory = join(home, "bin");
  const executables = executableNames(platform, architecture);
  const installedEdge = join(directory, executables[EDGE]);
  const installedService = join(directory, executables[SERVICE]);
  const launch = !neverInstalls(arguments_);
  const nothingToClean = async () => {};
  const readPackage = async () => {
    const packageJson = JSON.parse(await readFile(join(PACKAGE_ROOT, "package.json"), "utf8"));
    const manifest = JSON.parse(await readFile(join(PACKAGE_ROOT, "checksums.json"), "utf8"));
    validateChecksums(manifest, packageJson.version, publishedAssetNames());
    return { packageJson, manifest };
  };
  const stageOptions = { platform, architecture, cacheRoot: home, fetchImpl, reporter };
  const wholeSet = executables.map((_, index) => index);

  if (arguments_[0] === DEPLOYMENT_COMMAND) {
    const { packageJson, manifest } = await readPackage();
    const stage = await stageDownloads({ ...stageOptions, indexes: wholeSet, packageJson, manifest });
    return {
      executable: join(stage, executables[SERVICE]),
      arguments_,
      cleanup: () => rm(stage, { recursive: true, force: true }),
    };
  }

  if (launch) {
    const packageVersion = JSON.parse(await readFile(join(PACKAGE_ROOT, "package.json"), "utf8")).version;
    const development = (await readInstallRecord(directory))?.channel === DEVELOPMENT_CHANNEL;
    const present = existsSync(installedEdge) && existsSync(installedService);
    if (development || (present && await installedSetIsCurrent(directory, packageVersion))) {
      return { executable: installedEdge, arguments_: edgeArguments(arguments_), cleanup: nothingToClean };
    }
    const { packageJson, manifest } = await readPackage();
    const stage = await stageDownloads({ ...stageOptions, indexes: wholeSet, packageJson, manifest });
    try {
      await installImpl(join(stage, executables[SERVICE]), directory);
    } finally {
      await rm(stage, { recursive: true, force: true });
    }
    return { executable: installedEdge, arguments_: edgeArguments(arguments_), cleanup: nothingToClean };
  }

  if (existsSync(installedEdge) && existsSync(installedService)) {
    return { executable: installedEdge, arguments_, cleanup: nothingToClean };
  }
  const { packageJson, manifest } = await readPackage();
  const stage = await stageDownloads({ ...stageOptions, indexes: [EDGE, SERVICE], packageJson, manifest });
  return {
    executable: join(stage, executables[EDGE]),
    arguments_,
    cleanup: () => rm(stage, { recursive: true, force: true }),
  };
}

/** Runs the staged service's `deployment install`, which installs every staged image into `bin`. */
function installDownload(executable) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(executable, ["deployment", "install"], {
      stdio: ["ignore", "ignore", "inherit"], windowsHide: true,
    });
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (code === 0) resolvePromise();
      else reject(new Error(`Ghostlight installation failed (${signal ?? code})`));
    });
  });
}

async function main() {
  const launch = await prepareLaunch({ reporter: (message) => console.error(`ghostlight: ${message}`) });
  const child = spawn(launch.executable, launch.arguments_, { stdio: "inherit" });
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.on(signal, () => child.kill(signal));
  }
  child.on("error", async (error) => {
    console.error(`ghostlight: ${error.message}`);
    process.exitCode = 1;
    await launch.cleanup();
  });
  child.on("exit", async (code, signal) => {
    process.exitCode = code ?? (signal ? 1 : 0);
    await launch.cleanup();
  });
}

export function isMain(argument = process.argv[1], moduleUrl = import.meta.url) {
  if (!argument) {
    return false;
  }
  try {
    return realpathSync(argument) === realpathSync(fileURLToPath(moduleUrl));
  } catch {
    return false;
  }
}

if (isMain()) {
  main().catch((error) => {
    console.error(`ghostlight: ${error.message}`);
    process.exitCode = 1;
  });
}
