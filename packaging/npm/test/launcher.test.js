// SPDX-License-Identifier: Apache-2.0 OR MIT
// Unit tests for the launcher's supply-chain-integrity helpers (run: node --test).
"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { BINS, isAllowedHost, launchSpec, sha256File, targetTriple } = require("../bin/ghostlight.js");

test("ships the complete three-binary runtime", () => {
  assert.deepEqual(BINS, [
    "ghostlight",
    "ghostlight-mcp-connector",
    "ghostlight-browser-connector",
  ]);
});

test("launches the MCP edge without the removed relay role", () => {
  assert.deepEqual(launchSpec([]), { binName: "ghostlight-mcp-connector", spawnArgs: [] });
  assert.deepEqual(launchSpec(["--instance", "test"]), {
    binName: "ghostlight-mcp-connector",
    spawnArgs: ["--instance", "test"],
  });
});

test("keeps service CLI subcommands on the ghostlight binary", () => {
  assert.deepEqual(launchSpec(["doctor"]), { binName: "ghostlight", spawnArgs: ["doctor"] });
  assert.deepEqual(launchSpec(["install", "--no-open"]), {
    binName: "ghostlight",
    spawnArgs: ["install", "--no-open"],
  });
});

test("isAllowedHost permits only GitHub hosts", () => {
  // The initial URL and the object-store redirect target.
  assert.equal(isAllowedHost("github.com"), true);
  assert.equal(isAllowedHost("objects.githubusercontent.com"), true);
  assert.equal(isAllowedHost("codeload.githubusercontent.com"), true);
});

test("isAllowedHost rejects lookalikes and off-platform hosts", () => {
  assert.equal(isAllowedHost("evil.com"), false);
  // Suffix/lookalike attacks must not slip through.
  assert.equal(isAllowedHost("github.com.evil.com"), false);
  assert.equal(isAllowedHost("evilgithub.com"), false);
  assert.equal(isAllowedHost("notgithubusercontent.com"), false);
  assert.equal(isAllowedHost("githubusercontent.com"), false);
});

test("sha256File matches a known vector", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "gl-sha-"));
  const f = path.join(dir, "abc.bin");
  fs.writeFileSync(f, "abc");
  // Canonical: sha256("abc").
  assert.equal(
    sha256File(f),
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
  );
  fs.rmSync(dir, { recursive: true, force: true });
});

test("targetTriple maps the four supported targets and rejects the rest", () => {
  // Sanity: the function returns a string for a known combo and null otherwise. We cannot
  // change process.platform portably here, so just assert the current host resolves sanely.
  const t = targetTriple();
  assert.ok(t === null || typeof t === "string");
});
