// SPDX-License-Identifier: Apache-2.0 OR MIT
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const { binaryPath, compareVersions, installIsCurrent, run, targetFor } = require("../server/launch.js");

const BUNDLE_VERSION = "2.0.0";

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

test("versions order numerically and a pre-release sorts below its release", () => {
  assert.ok(compareVersions("2.0.0", "1.9.9") > 0);
  assert.ok(compareVersions("1.3.9", "1.3.15") < 0);
  assert.equal(compareVersions("2.0.0", "2.0.0"), 0);
  assert.ok(compareVersions("2.0.0-rc.1", "2.0.0") < 0);
  assert.equal(compareVersions("junk", "2.0.0"), null);
});

/**
 * A temporary Ghostlight home, optionally holding an install record and the installed connector
 * a finished install leaves beside it.
 */
function homeWithRecord(record, { connector = true } = {}) {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "ghostlight-mcpb-"));
  if (record !== undefined) {
    fs.mkdirSync(path.join(home, "bin"));
    fs.writeFileSync(path.join(home, "bin", "install.json"), record);
    if (connector) fs.writeFileSync(path.join(home, "bin", "ghostlight-mcp-connector.exe"), "");
  }
  return home;
}

/** Runs the launcher against a fake spawner; returns the exit status and the commands run. */
function launch(record, overrides = {}) {
  const home = homeWithRecord(record, overrides);
  const calls = [];
  const errors = [];
  try {
    const status = run({
      platform: "win32",
      architecture: "x64",
      serverDirectory: path.join("fixture", "server"),
      version: BUNDLE_VERSION,
      env: { GHOSTLIGHT_TEST_HOME: home },
      spawnSyncImpl(command, args, options) {
        calls.push({ command, args, options });
        return overrides.spawn ? overrides.spawn(command, args) : { status: 0, stdout: "setup output", stderr: "" };
      },
      stderr: { write: (value) => errors.push(value) },
    });
    return { status, calls, errors, home };
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
  }
}

test("an absent install record installs from the bundle off MCP stdout, then starts the installed stdio edge", () => {
  const { status, calls, errors, home } = launch(undefined);
  assert.equal(status, 0);
  assert.deepEqual(errors, []);
  assert.equal(calls.length, 2);
  assert.match(calls[0].command, /ghostlight\.exe$/);
  assert.deepEqual(calls[0].args, ["deployment", "install"]);
  assert.deepEqual(calls[0].options.stdio, ["ignore", "pipe", "pipe"]);
  assert.equal(calls[1].command, path.join(home, "bin", "ghostlight-mcp-connector.exe"));
  assert.deepEqual(calls[1].args, []);
  assert.equal(calls[1].options.stdio, "inherit");
});

test("an older or unreadable install record installs first", () => {
  for (const record of [
    JSON.stringify({ service_version: "1.3.15" }),
    JSON.stringify({ service_version: "2.0.0-rc.1" }),
    JSON.stringify({}),
    "{ not json",
  ]) {
    const { status, calls } = launch(record);
    assert.equal(status, 0, record);
    assert.deepEqual(calls.map((call) => call.args), [["deployment", "install"], []], record);
  }
});

test("the same or a newer install record starts the installed stdio edge without installing", () => {
  for (const version of ["2.0.0", "2.0.1", "2.1.0", "10.0.0"]) {
    const { status, calls, home } = launch(JSON.stringify({ service_version: version }));
    assert.equal(status, 0, version);
    assert.equal(calls.length, 1, version);
    assert.equal(calls[0].command, path.join(home, "bin", "ghostlight-mcp-connector.exe"), version);
    assert.equal(calls[0].options.stdio, "inherit");
  }
});

test("a current record whose installed connector is missing installs first", () => {
  const { status, calls, home } = launch(JSON.stringify({ service_version: "2.0.0" }), { connector: false });
  assert.equal(status, 0);
  assert.deepEqual(calls.map((call) => call.args), [["deployment", "install"], []]);
  assert.equal(calls[1].command, path.join(home, "bin", "ghostlight-mcp-connector.exe"));
});

test("the install record is read from the bin directory of the Ghostlight home", () => {
  const home = homeWithRecord(JSON.stringify({ service_version: "2.0.0" }));
  try {
    assert.equal(installIsCurrent(home, "2.0.0"), true);
    assert.equal(installIsCurrent(path.join(home, "missing"), "2.0.0"), false);
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
  }
});

test("an install failure is returned and the MCP connector never starts", () => {
  const { status, calls, errors } = launch(undefined, {
    spawn: () => ({ status: 7, stdout: "", stderr: "registration refused" }),
  });
  assert.equal(status, 7);
  assert.equal(calls.length, 1);
  assert.match(errors.join(""), /registration refused/);
});
