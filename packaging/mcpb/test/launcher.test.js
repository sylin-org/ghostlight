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

test("binary paths keep the sibling executables in one target directory", () => {
  const serverDirectory = path.join("fixture", "server");
  for (const name of ["ghostlight", "ghostlight-service", "ghostlight-desktop"]) {
    assert.equal(
      binaryPath(name, { platform: "win32", architecture: "x64", serverDirectory }),
      path.join(serverDirectory, "bin", "x86_64-pc-windows-msvc", `${name}.exe`),
    );
  }
});

test("versions order by the service's rule, on the vectors install/record.rs reads", () => {
  const vectors = JSON.parse(fs.readFileSync(path.join(__dirname, "..", "..", "version-order.json"), "utf8"));
  for (const [index, version] of vectors.ascending.entries()) {
    assert.equal(compareVersions(version, version), 0, version);
    for (const later of vectors.ascending.slice(index + 1)) {
      assert.equal(compareVersions(version, later), -1, `${version} < ${later}`);
      assert.equal(compareVersions(later, version), 1, `${later} > ${version}`);
    }
  }
  for (const [left, right] of vectors.equal) {
    assert.equal(compareVersions(left, right), 0, `${left} = ${right}`);
  }
  for (const malformed of vectors.malformed) {
    assert.equal(compareVersions(malformed, "2.0.0"), null, JSON.stringify(malformed));
    assert.equal(compareVersions("2.0.0", malformed), null, JSON.stringify(malformed));
  }
  assert.equal(compareVersions(undefined, "2.0.0"), null);
});

/**
 * A temporary Ghostlight home, optionally holding an install record and the edge, service and
 * shell a finished install leaves beside it. `missing` names an installed image to leave out.
 */
function homeWithRecord(record, { missing } = {}) {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "ghostlight-mcpb-"));
  if (record !== undefined) {
    fs.mkdirSync(path.join(home, "bin"));
    fs.writeFileSync(path.join(home, "bin", "install.json"), record);
    for (const name of ["ghostlight", "ghostlight-service", "ghostlight-desktop"]) {
      if (name !== missing) fs.writeFileSync(path.join(home, "bin", `${name}.exe`), "");
    }
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

test("an absent install record installs from the bundle off MCP stdout, then starts the installed MCP relay", () => {
  const { status, calls, errors, home } = launch(undefined);
  assert.equal(status, 0);
  assert.deepEqual(errors, []);
  assert.equal(calls.length, 2);
  assert.equal(path.basename(calls[0].command), "ghostlight-service.exe");
  assert.deepEqual(calls[0].args, ["deployment", "install"]);
  assert.deepEqual(calls[0].options.stdio, ["ignore", "pipe", "pipe"]);
  assert.equal(calls[1].command, path.join(home, "bin", "ghostlight.exe"));
  assert.deepEqual(calls[1].args, ["mcp"]);
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
    assert.deepEqual(calls.map((call) => call.args), [["deployment", "install"], ["mcp"]], record);
  }
});

test("the same or a newer install record starts the installed MCP relay without installing", () => {
  for (const version of ["2.0.0", "2.0.1", "2.1.0", "10.0.0"]) {
    const { status, calls, home } = launch(JSON.stringify({ service_version: version }));
    assert.equal(status, 0, version);
    assert.equal(calls.length, 1, version);
    assert.equal(calls[0].command, path.join(home, "bin", "ghostlight.exe"), version);
    assert.deepEqual(calls[0].args, ["mcp"], version);
    assert.equal(calls[0].options.stdio, "inherit");
  }
});

test("a current record whose installed edge or service is missing installs first", () => {
  for (const missing of ["ghostlight", "ghostlight-service"]) {
    const { status, calls, home } = launch(JSON.stringify({ service_version: "2.0.0" }), { missing });
    assert.equal(status, 0, missing);
    assert.deepEqual(calls.map((call) => call.args), [["deployment", "install"], ["mcp"]], missing);
    assert.equal(calls[1].command, path.join(home, "bin", "ghostlight.exe"), missing);
  }
});

test("a same-version record whose shell is missing installs first, so the install puts it back", () => {
  const { status, calls } = launch(JSON.stringify({ service_version: BUNDLE_VERSION }), { missing: "ghostlight-desktop" });
  assert.equal(status, 0);
  assert.deepEqual(calls.map((call) => call.args), [["deployment", "install"], ["mcp"]]);
});

test("a newer record whose shell is missing starts the installed MCP relay, since the bundle cannot install over it", () => {
  const { status, calls, home } = launch(JSON.stringify({ service_version: "2.1.0" }), { missing: "ghostlight-desktop" });
  assert.equal(status, 0);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, path.join(home, "bin", "ghostlight.exe"));
  assert.deepEqual(calls[0].args, ["mcp"]);
});

test("a development record starts the installed MCP relay without installing, whatever its version", () => {
  const { status, calls, errors, home } = launch(JSON.stringify({ service_version: "1.0.0", channel: "development" }));
  assert.equal(status, 0);
  assert.deepEqual(errors, []);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, path.join(home, "bin", "ghostlight.exe"));
  assert.deepEqual(calls[0].args, ["mcp"]);
  assert.equal(calls[0].options.stdio, "inherit");
});

test("a development record whose edge is gone names the way back and installs nothing", () => {
  const { status, calls, errors, home } = launch(
    JSON.stringify({ service_version: "1.0.0", channel: "development" }),
    { missing: "ghostlight" },
  );
  assert.notEqual(status, 0);
  assert.equal(calls.length, 0);
  const printed = errors.join("");
  assert.ok(printed.includes(`development build, but ${path.join(home, "bin", "ghostlight.exe")} is missing`));
  assert.ok(printed.includes("npx ghostlight deployment install --restore"));
  assert.ok(printed.includes("-Restore"));
  assert.ok(printed.includes("get.sh | sh -s -- --restore"));
});

test("the install record is read from the bin directory of the Ghostlight home", () => {
  const home = homeWithRecord(JSON.stringify({ service_version: "2.0.0" }));
  try {
    assert.equal(installIsCurrent(home, "2.0.0", "win32"), true);
    assert.equal(installIsCurrent(path.join(home, "missing"), "2.0.0", "win32"), false);
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
  }
});

test("an install failure is returned and the MCP relay never starts", () => {
  const { status, calls, errors } = launch(undefined, {
    spawn: () => ({ status: 7, stdout: "", stderr: "registration refused" }),
  });
  assert.equal(status, 7);
  assert.equal(calls.length, 1);
  assert.match(errors.join(""), /registration refused/);
});
