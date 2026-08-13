// SPDX-License-Identifier: Apache-2.0 OR MIT
"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
const test = require("node:test");
const { binaryPath, run, targetFor } = require("../server/launch.js");

test("the bundle maps every Claude Desktop platform", () => {
  assert.equal(targetFor("win32", "x64"), "x86_64-pc-windows-msvc");
  assert.equal(targetFor("darwin", "arm64"), "aarch64-apple-darwin");
  assert.equal(targetFor("darwin", "x64"), "x86_64-apple-darwin");
  assert.throws(() => targetFor("linux", "x64"), /does not support/);
});

test("binary paths keep the three sibling executables in one target directory", () => {
  const serverDirectory = path.join("fixture", "server");
  assert.equal(
    binaryPath("ghostlight", { platform: "win32", architecture: "x64", serverDirectory }),
    path.join(serverDirectory, "bin", "x86_64-pc-windows-msvc", "ghostlight.exe"),
  );
  assert.equal(
    binaryPath("ghostlight-browser-connector", {
      platform: "darwin",
      architecture: "arm64",
      serverDirectory,
    }),
    path.join(serverDirectory, "bin", "aarch64-apple-darwin", "ghostlight-browser-connector"),
  );
});

test("browser setup stays off MCP stdout and precedes the stdio edge", () => {
  const calls = [];
  const status = run({
    platform: "win32",
    architecture: "x64",
    serverDirectory: path.join("fixture", "server"),
    spawnSyncImpl(command, args, options) {
      calls.push({ command, args, options });
      return { status: 0, stdout: "setup output", stderr: "" };
    },
    stderr: { write: () => assert.fail("unexpected stderr") },
  });
  assert.equal(status, 0);
  assert.deepEqual(calls[0].args, ["install", "--no-clients", "--no-open"]);
  assert.deepEqual(calls[0].options.stdio, ["ignore", "pipe", "pipe"]);
  assert.match(calls[1].command, /ghostlight-mcp-connector\.exe$/);
  assert.equal(calls[1].options.stdio, "inherit");
});

test("macOS executable permissions are repaired before either process starts", () => {
  const changed = [];
  const status = run({
    platform: "darwin",
    architecture: "x64",
    serverDirectory: path.join("fixture", "server"),
    chmodSyncImpl: (file, mode) => changed.push({ file, mode }),
    spawnSyncImpl: () => ({ status: 0, stdout: "", stderr: "" }),
    stderr: { write: () => assert.fail("unexpected stderr") },
  });
  assert.equal(status, 0);
  assert.equal(changed.length, 3);
  assert.ok(changed.every(({ mode }) => mode === 0o755));
});

test("a setup failure is returned and the MCP connector never starts", () => {
  const calls = [];
  const errors = [];
  const status = run({
    platform: "win32",
    architecture: "x64",
    serverDirectory: path.join("fixture", "server"),
    spawnSyncImpl(command) {
      calls.push(command);
      return { status: 7, stdout: "", stderr: "registration refused" };
    },
    stderr: { write: (value) => errors.push(value) },
  });
  assert.equal(status, 7);
  assert.equal(calls.length, 1);
  assert.match(errors.join(""), /registration refused/);
});
