// SPDX-License-Identifier: Apache-2.0 OR MIT
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

/** Environment variable that moves the Ghostlight home. It exists for journeys only. */
const TEST_HOME_VARIABLE = "GHOSTLIGHT_TEST_HOME";
/** Record the installer writes next to the installed executables once a service is running. */
const INSTALL_RECORD_PATH = path.join("bin", "install.json");

/** The edge: its `mcp` command is the stdio server this launcher runs. */
const EDGE_IMAGE = "ghostlight";
/** The service: the bundled copy installs the set, and the installed copy serves the edge. */
const SERVICE_IMAGE = "ghostlight-service";
/** The shell: the window the installed service opens. */
const SHELL_IMAGE = "ghostlight-desktop";
/** The edge command that starts the MCP stdio relay. */
const MCP_COMMAND = "mcp";
/** The channel `install.json` records for a build the development loop installed. */
const DEVELOPMENT_CHANNEL = "development";
/** The way back from a development build to a release, in the words the service uses. */
const RESTORE_ROUTE = [
  "To return to a release, run one of these:",
  "  npx ghostlight deployment install --restore",
  "  PowerShell: & ([scriptblock]::Create((irm https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.ps1))) -Restore",
  "  macOS and Linux: curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.sh | sh -s -- --restore",
].join("\n");

const TARGETS = Object.freeze({
  "win32/x64": "x86_64-pc-windows-msvc",
});

function targetFor(platform = process.platform, architecture = process.arch) {
  const target = TARGETS[`${platform}/${architecture}`];
  if (!target) throw new Error(`Ghostlight MCPB does not support ${platform}/${architecture}`);
  return target;
}

function binaryPath(name, options = {}) {
  const platform = options.platform ?? process.platform;
  const architecture = options.architecture ?? process.arch;
  const serverDirectory = options.serverDirectory ?? __dirname;
  const extension = platform === "win32" ? ".exe" : "";
  return path.join(serverDirectory, "bin", targetFor(platform, architecture), `${name}${extension}`);
}

/**
 * Orders two release versions as a negative, zero or positive number, or null when either is not
 * `major.minor.patch` with an optional pre-release suffix. A pre-release sorts below its release.
 */
function compareVersions(left, right) {
  const parse = (value) => {
    const match = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(String(value));
    return match ? { numbers: match.slice(1, 4).map(Number), pre: match[4] } : null;
  };
  const a = parse(left);
  const b = parse(right);
  if (!a || !b) return null;
  for (let index = 0; index < 3; index++) {
    if (a.numbers[index] !== b.numbers[index]) return a.numbers[index] - b.numbers[index];
  }
  if (a.pre === b.pre) return 0;
  if (a.pre === undefined) return 1;
  if (b.pre === undefined) return -1;
  return a.pre < b.pre ? -1 : 1;
}

/** The Ghostlight home directory: the test override, otherwise `~/.ghostlight`. */
function ghostlightHome(options = {}) {
  const env = options.env ?? process.env;
  return env[TEST_HOME_VARIABLE] ?? path.join((options.homedir ?? os.homedir)(), ".ghostlight");
}

/** The version this bundle carries, from the manifest beside the server directory. */
function bundleVersion(serverDirectory) {
  const manifest = path.join(serverDirectory, "..", "manifest.json");
  return JSON.parse(fs.readFileSync(manifest, "utf8")).version;
}

/** The image named `name` that the install put under the Ghostlight home. */
function installedImage(home, platform, name) {
  return path.join(home, "bin", `${name}${platform === "win32" ? ".exe" : ""}`);
}

/** The installed `install.json`, or null when it is absent or unreadable. */
function readInstallRecord(home) {
  try {
    return JSON.parse(fs.readFileSync(path.join(home, INSTALL_RECORD_PATH), "utf8"));
  } catch {
    return null;
  }
}

/** The sentence for a development installation whose edge is gone, as the service words it. */
function developmentExecutableMissing(executable) {
  return `The installed Ghostlight is a development build, but ${executable} is missing. ${RESTORE_ROUTE}\n`;
}

/**
 * True when the installed service is newer than this bundle, or the same version with its shell
 * in place. The service treats a missing shell of its own version as damage and puts it back, so
 * that start installs; it never installs over a newer service, so that set runs without its
 * shell. An absent, unreadable or unordered `install.json` counts as older, so the bundle
 * installs itself.
 */
function installIsCurrent(home, version, platform = process.platform) {
  const order = compareVersions(readInstallRecord(home)?.service_version, version);
  return order !== null
    && (order > 0 || (order === 0 && fs.existsSync(installedImage(home, platform, SHELL_IMAGE))));
}

function detail(label, result) {
  const lines = [label, result.error?.message, result.stdout, result.stderr]
    .filter(Boolean)
    .map((value) => String(value).trim())
    .filter(Boolean);
  return `${lines.join("\n")}\n`;
}

function run(options = {}) {
  const platform = options.platform ?? process.platform;
  const architecture = options.architecture ?? process.arch;
  const serverDirectory = options.serverDirectory ?? __dirname;
  const spawnProcess = options.spawnSyncImpl ?? spawnSync;
  const standardError = options.stderr ?? process.stderr;
  const home = ghostlightHome(options);
  const edge = installedImage(home, platform, EDGE_IMAGE);
  const installedService = installedImage(home, platform, SERVICE_IMAGE);
  let bundledService;
  try {
    bundledService = binaryPath(SERVICE_IMAGE, { platform, architecture, serverDirectory });
  } catch (error) {
    standardError.write(`Ghostlight MCPB setup failed: ${error.message}\n`);
    return 1;
  }

  const development = readInstallRecord(home)?.channel === DEVELOPMENT_CHANNEL;
  if (development && !fs.existsSync(edge)) {
    standardError.write(developmentExecutableMissing(edge));
    return 1;
  }
  let current;
  try {
    current = development || (fs.existsSync(edge) && fs.existsSync(installedService)
      && installIsCurrent(home, options.version ?? bundleVersion(serverDirectory), platform));
  } catch (error) {
    standardError.write(`Ghostlight MCPB setup failed: ${error.message}\n`);
    return 1;
  }
  if (!current) {
    const setup = spawnProcess(bundledService, ["deployment", "install"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    if (setup.error || setup.status !== 0) {
      standardError.write(detail("Ghostlight could not be installed.", setup));
      return Number.isInteger(setup.status) ? setup.status : 1;
    }
  }
  const session = spawnProcess(edge, [MCP_COMMAND], { stdio: "inherit", windowsHide: true });
  if (session.error) {
    standardError.write(detail("Ghostlight could not start its MCP relay.", session));
    return 1;
  }
  return Number.isInteger(session.status) ? session.status : 1;
}

if (require.main === module) process.exitCode = run();

module.exports = { binaryPath, compareVersions, installIsCurrent, run, targetFor };
