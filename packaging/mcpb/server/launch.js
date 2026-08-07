// SPDX-License-Identifier: Apache-2.0 OR MIT
"use strict";

const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { chmodSync } = require("node:fs");

const TARGETS = Object.freeze({
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "win32-x64": "x86_64-pc-windows-msvc",
});

function targetTriple(platform = process.platform, arch = process.arch) {
  const target = TARGETS[`${platform}-${arch}`];
  if (!target) {
    throw new Error(`unsupported Ghostlight MCPB platform: ${platform}/${arch}`);
  }
  return target;
}

function executablePath(name, options = {}) {
  const platform = options.platform || process.platform;
  const arch = options.arch || process.arch;
  const serverDir = options.serverDir || __dirname;
  const suffix = platform === "win32" ? ".exe" : "";
  return path.join(serverDir, "bin", targetTriple(platform, arch), `${name}${suffix}`);
}

function renderFailure(label, result) {
  const details = [result.error && result.error.message, result.stdout, result.stderr]
    .filter(Boolean)
    .map((value) => String(value).trim())
    .filter(Boolean)
    .join("\n");
  return `${label}${details ? `\n${details}` : ""}\n`;
}

function run(options = {}) {
  const runProcess = options.spawnSyncImpl || spawnSync;
  const platform = options.platform || process.platform;
  const arch = options.arch || process.arch;
  const serverDir = options.serverDir || __dirname;
  const stderr = options.stderr || process.stderr;
  const makeExecutable = options.chmodSyncImpl || chmodSync;
  let cli;
  let edge;
  let browserRelay;

  try {
    cli = executablePath("ghostlight", { platform, arch, serverDir });
    edge = executablePath("ghostlight-mcp-connector", { platform, arch, serverDir });
    browserRelay = executablePath("ghostlight-browser-connector", { platform, arch, serverDir });
  } catch (error) {
    stderr.write(`${error.message}\n`);
    return 1;
  }

  if (platform === "darwin") {
    try {
      makeExecutable(cli, 0o755);
      makeExecutable(edge, 0o755);
      makeExecutable(browserRelay, 0o755);
    } catch (error) {
      stderr.write(`Ghostlight could not prepare its packaged binaries.\n${error.message}\n`);
      return 1;
    }
  }

  const setup = runProcess(cli, ["install", "--no-clients", "--no-open"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  if (setup.error || setup.status !== 0) {
    stderr.write(renderFailure("Ghostlight browser-side setup failed.", setup));
    return setup.status || 1;
  }

  const session = runProcess(edge, [], {
    stdio: "inherit",
    windowsHide: true,
  });
  if (session.error) {
    stderr.write(renderFailure("Ghostlight MCP edge failed to start.", session));
    return 1;
  }
  return Number.isInteger(session.status) ? session.status : 1;
}

if (require.main === module) {
  process.exitCode = run();
}

module.exports = { executablePath, run, targetTriple };
