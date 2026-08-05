// SPDX-License-Identifier: Apache-2.0 OR MIT
"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
const test = require("node:test");
const { executablePath, run, targetTriple } = require("../server/launch.js");

test("maps every packaged Claude Desktop platform", () => {
  assert.equal(targetTriple("win32", "x64"), "x86_64-pc-windows-msvc");
  assert.equal(targetTriple("darwin", "arm64"), "aarch64-apple-darwin");
  assert.equal(targetTriple("darwin", "x64"), "x86_64-apple-darwin");
  assert.throws(() => targetTriple("linux", "x64"), /unsupported Ghostlight MCPB platform/);
});

test("uses the executable suffix only on Windows", () => {
  const root = path.join("tmp", "server");
  assert.equal(
    executablePath("ghostlight", { platform: "win32", arch: "x64", serverDir: root }),
    path.join(root, "bin", "x86_64-pc-windows-msvc", "ghostlight.exe")
  );
  assert.equal(
    executablePath("ghostlight-mcp-connector", { platform: "win32", arch: "x64", serverDir: root }),
    path.join(root, "bin", "x86_64-pc-windows-msvc", "ghostlight-mcp-connector.exe")
  );
  assert.equal(
    executablePath("ghostlight-browser-connector", { platform: "darwin", arch: "arm64", serverDir: root }),
    path.join(root, "bin", "aarch64-apple-darwin", "ghostlight-browser-connector")
  );
});

test("registers only the browser side before launching the protocol-versioned MCP edge", () => {
  const calls = [];
  const spawnSyncImpl = (command, args, options) => {
    calls.push({ command, args, options });
    return { status: 0, stdout: "installer output that must stay off MCP stdout", stderr: "" };
  };
  const stderrWrites = [];
  const status = run({
    platform: "win32",
    arch: "x64",
    serverDir: path.join("tmp", "server"),
    spawnSyncImpl,
    stderr: { write: (value) => stderrWrites.push(value) },
  });

  assert.equal(status, 0);
  assert.deepEqual(calls[0].args, ["install", "--no-clients", "--no-open"]);
  assert.deepEqual(calls[0].options.stdio, ["ignore", "pipe", "pipe"]);
  assert.match(calls[1].command, /ghostlight-mcp-connector\.exe$/);
  assert.deepEqual(calls[1].args, []);
  assert.equal(calls[1].options.stdio, "inherit");
  assert.deepEqual(stderrWrites, []);
});

test("restores executable permissions for packaged macOS binaries", () => {
  const chmodCalls = [];
  const status = run({
    platform: "darwin",
    arch: "arm64",
    serverDir: path.join("tmp", "server"),
    chmodSyncImpl: (file, mode) => chmodCalls.push({ file, mode }),
    spawnSyncImpl: () => ({ status: 0, stdout: "", stderr: "" }),
    stderr: { write: () => assert.fail("unexpected stderr") },
  });

  assert.equal(status, 0);
  assert.equal(chmodCalls.length, 3);
  assert.ok(chmodCalls.every((call) => call.mode === 0o755));
  assert.ok(chmodCalls.some((call) => /ghostlight-mcp-connector$/.test(call.file)));
  assert.ok(chmodCalls.some((call) => /ghostlight-browser-connector$/.test(call.file)));
});

test("reports setup failure on stderr and never starts the MCP edge", () => {
  const calls = [];
  const stderrWrites = [];
  const status = run({
    platform: "darwin",
    arch: "x64",
    serverDir: path.join("tmp", "server"),
    chmodSyncImpl: () => {},
    spawnSyncImpl: (command, args) => {
      calls.push({ command, args });
      return { status: 7, stdout: "setup detail", stderr: "setup error" };
    },
    stderr: { write: (value) => stderrWrites.push(value) },
  });

  assert.equal(status, 7);
  assert.equal(calls.length, 1);
  assert.match(stderrWrites.join(""), /Ghostlight browser-side setup failed/);
  assert.match(stderrWrites.join(""), /setup error/);
});
