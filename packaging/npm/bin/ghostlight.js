#!/usr/bin/env node

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, realpathSync } from "node:fs";
import { chmod, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
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

export async function prepareLaunch({
  arguments_: arguments_ = process.argv.slice(2),
  platform = process.platform,
  architecture = process.arch,
  cacheRoot = process.env.GHOSTLIGHT_HOME ?? join(homedir(), ".ghostlight"),
  fetchImpl = globalThis.fetch,
  reporter = () => {},
} = {}) {
  const packageJson = JSON.parse(await readFile(join(PACKAGE_ROOT, "package.json"), "utf8"));
  const manifest = JSON.parse(await readFile(join(PACKAGE_ROOT, "checksums.json"), "utf8"));
  const assets = assetNames(platform, architecture);
  const executables = executableNames(platform, architecture);
  validateChecksums(manifest, packageJson.version, publishedAssetNames());
  const directory = join(cacheRoot, "bin", `v${packageJson.version}`);
  for (const [index, asset] of assets.entries()) {
    const url = `${RELEASE_ROOT}/v${packageJson.version}/${asset}`;
    const changed = await ensureBinary({
      path: join(directory, executables[index]),
      url,
      expectedHash: manifest.binaries[asset],
      fetchImpl,
      onDownload: () => reporter(
        `Downloading Ghostlight ${packageJson.version} for ${platform}/${architecture} (${index + 1}/${assets.length})...`,
      ),
    });
    if (changed) {
      reporter(`Verified ${EXECUTABLES[index]}.`);
    }
  }
  return {
    executable: join(directory, selectedExecutable(arguments_, platform, architecture)),
    arguments_,
  };
}

async function main() {
  const launch = await prepareLaunch({ reporter: (message) => console.error(`ghostlight: ${message}`) });
  const child = spawn(launch.executable, launch.arguments_, { stdio: "inherit" });
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.on(signal, () => child.kill(signal));
  }
  child.on("error", (error) => {
    console.error(`ghostlight: ${error.message}`);
    process.exitCode = 1;
  });
  child.on("exit", (code, signal) => {
    process.exitCode = code ?? (signal ? 1 : 0);
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
