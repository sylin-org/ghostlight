// SPDX-License-Identifier: Apache-2.0 OR MIT
"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
const test = require("node:test");
const { binaryPath, run, targetFor } = require("../server/launch.js");

test("the bundle maps the supported Claude Desktop platform", () => {
  assert.equal(targetFor("win32", "x64"), "x86_64-pc-windows-msvc");
  assert.throws(() => targetFor("linux", "x64"), /does not support/);
});

test("binary paths keep the three sibling executables in one target directory", () => {
  const serverDirectory = path.join("fixture", "server");
  assert.equal(
    binaryPath("ghostlight", { platform: "win32", architecture: "x64", serverDirectory }),
    path.join(serverDirectory, "bin", "x86_64-pc-windows-msvc", "ghostlight.exe"),
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
