#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
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
const EXECUTABLES = [
  "ghostlight",
  "ghostlight-mcp-connector",
  "ghostlight-browser-connector",
];

/** Environment variable that moves the Ghostlight home. It exists for journeys only. */
const TEST_HOME_VARIABLE = "GHOSTLIGHT_TEST_HOME";
/** Record the installer writes into the install directory once a service is running. */
const INSTALL_RECORD_NAME = "install.json";
/** Marker file in the Ghostlight home that selects a development installation. */
const DEVELOPMENT_MARKER_NAME = "development";
/** First commands that start the service or make it installed. Every other command is read-only. */
const LAUNCH_COMMANDS = new Set(["install", "open", "serve"]);

/** The Linux shared library the desktop service links, and the packages that provide it. */
const WEBKIT_LIBRARY = "libwebkit2gtk-4.1.so.0";
const WEBKIT_PACKAGES = "libwebkit2gtk-4.1-0 (Debian, Ubuntu), webkit2gtk4.1 (Fedora), webkit2gtk-4.1 (Arch)";
const LIBRARY_DIRECTORIES = [
  "/usr/lib/x86_64-linux-gnu",
  "/lib/x86_64-linux-gnu",
  "/usr/lib64",
  "/usr/lib",
  "/usr/local/lib",
];

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

export function selectedExecutable(arguments_, platform = process.platform, architecture = process.arch) {
  const [orchestrator, mcpConnector] = executableNames(platform, architecture);
  return arguments_.length === 0 ? mcpConnector : orchestrator;
}

/**
 * True when the command starts or installs the service: no arguments (the MCP connector),
 * `install`, `open` or `serve`. Every other command only reads and never installs.
 */
export function launchesService(arguments_) {
  return arguments_.length === 0 || LAUNCH_COMMANDS.has(arguments_[0]);
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

/** True when the dynamic loader lists, or a known library directory holds, the WebKit library. */
export function webkitLibraryPresent({ run = spawnSync, exists = existsSync } = {}) {
  for (const loader of ["ldconfig", "/sbin/ldconfig", "/usr/sbin/ldconfig"]) {
    const result = run(loader, ["-p"], { encoding: "utf8", windowsHide: true });
    if (!result.error && result.status === 0 && String(result.stdout).includes(WEBKIT_LIBRARY)) {
      return true;
    }
  }
  return LIBRARY_DIRECTORIES.some((directory) => exists(`${directory}/${WEBKIT_LIBRARY}`));
}

/** Throws a sentence naming the missing package when Linux lacks the library the service needs. */
export function requireLinuxLibraries(platform, probe = webkitLibraryPresent) {
  if (platform === "linux" && !probe()) {
    throw new Error(
      `Ghostlight needs ${WEBKIT_LIBRARY}, which is not installed. Install its package: ${WEBKIT_PACKAGES}.`,
    );
  }
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
 * Decides what to run and makes it ready. Launch commands (see `launchesService`) run the
 * installed set when `install.json` is the same as or newer than this package, or when the
 * development marker exists; otherwise the verified download installs itself first. Every other
 * command runs the installed service, or a verified staged copy that installs nothing.
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
  libraryProbe = webkitLibraryPresent,
} = {}) {
  const home = resolve(cacheRoot);
  const directory = join(home, "bin");
  const executables = executableNames(platform, architecture);
  const installed = join(directory, selectedExecutable(arguments_, platform, architecture));
  const launch = launchesService(arguments_);
  const nothingToClean = async () => {};
  const readPackage = async () => {
    const packageJson = JSON.parse(await readFile(join(PACKAGE_ROOT, "package.json"), "utf8"));
    const manifest = JSON.parse(await readFile(join(PACKAGE_ROOT, "checksums.json"), "utf8"));
    validateChecksums(manifest, packageJson.version, publishedAssetNames());
    return { packageJson, manifest };
  };
  const stageOptions = { platform, architecture, cacheRoot: home, fetchImpl, reporter };

  if (launch) {
    const packageVersion = JSON.parse(await readFile(join(PACKAGE_ROOT, "package.json"), "utf8")).version;
    const development = existsSync(join(home, DEVELOPMENT_MARKER_NAME));
    if (development || (existsSync(installed) && await installedSetIsCurrent(directory, packageVersion))) {
      requireLinuxLibraries(platform, libraryProbe);
      return { executable: installed, arguments_, cleanup: nothingToClean };
    }
    const { packageJson, manifest } = await readPackage();
    requireLinuxLibraries(platform, libraryProbe);
    const stage = await stageDownloads({ ...stageOptions, indexes: executables.map((_, index) => index), packageJson, manifest });
    try {
      await installImpl(join(stage, executables[0]), directory);
    } finally {
      await rm(stage, { recursive: true, force: true });
    }
    return { executable: installed, arguments_, cleanup: nothingToClean };
  }

  const service = join(directory, executables[0]);
  if (existsSync(service)) {
    requireLinuxLibraries(platform, libraryProbe);
    return { executable: service, arguments_, cleanup: nothingToClean };
  }
  const { packageJson, manifest } = await readPackage();
  requireLinuxLibraries(platform, libraryProbe);
  const stage = await stageDownloads({ ...stageOptions, indexes: [0], packageJson, manifest });
  return {
    executable: join(stage, executables[0]),
    arguments_,
    cleanup: () => rm(stage, { recursive: true, force: true }),
  };
}

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
