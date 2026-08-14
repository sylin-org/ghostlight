// SPDX-License-Identifier: Apache-2.0 OR MIT
"use strict";

const path = require("node:path");
const { spawnSync } = require("node:child_process");

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
  let orchestrator;
  let mcpConnector;
  try {
    orchestrator = binaryPath("ghostlight", { platform, architecture, serverDirectory });
    mcpConnector = binaryPath("ghostlight-mcp-connector", { platform, architecture, serverDirectory });
  } catch (error) {
    standardError.write(`Ghostlight MCPB setup failed: ${error.message}\n`);
    return 1;
  }

  const setup = spawnProcess(orchestrator, ["install", "--no-clients", "--no-open"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  if (setup.error || setup.status !== 0) {
    standardError.write(detail("Ghostlight browser connection could not be installed.", setup));
    return Number.isInteger(setup.status) ? setup.status : 1;
  }
  const session = spawnProcess(mcpConnector, [], { stdio: "inherit", windowsHide: true });
  if (session.error) {
    standardError.write(detail("Ghostlight MCP connector could not start.", session));
    return 1;
  }
  return Number.isInteger(session.status) ? session.status : 1;
}

if (require.main === module) process.exitCode = run();

module.exports = { binaryPath, run, targetFor };
