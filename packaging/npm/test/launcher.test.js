import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

import {
  assertAllowedDownload,
  assetNames,
  ensureBinary,
  executableNames,
  isMain,
  publishedAssetNames,
  releaseTarget,
  selectedExecutable,
  validateChecksums,
} from "../bin/ghostlight.js";

test("an installed npm bin symlink still invokes the launcher", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ghostlight-npm-bin-"));
  const module = join(directory, "ghostlight.js");
  const command = join(directory, "ghostlight");
  await writeFile(module, "launcher");
  await symlink(module, command);
  try {
    assert.equal(isMain(command, pathToFileURL(module)), true);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("the published platforms resolve to exact release targets", () => {
  assert.equal(releaseTarget("win32", "x64"), "x86_64-pc-windows-msvc");
  assert.equal(releaseTarget("linux", "x64"), "x86_64-unknown-linux-gnu");
  assert.throws(() => releaseTarget("linux", "arm64"), /does not publish/);
});

test("every platform downloads the complete three-process unit", () => {
  assert.deepEqual(assetNames("linux", "x64"), [
    "ghostlight-x86_64-unknown-linux-gnu",
    "ghostlight-mcp-connector-x86_64-unknown-linux-gnu",
    "ghostlight-browser-connector-x86_64-unknown-linux-gnu",
  ]);
  assert.ok(assetNames("win32", "x64").every((name) => name.endsWith(".exe")));
});

test("verified assets are cached under exact trusted sibling names", () => {
  assert.deepEqual(executableNames("linux", "x64"), [
    "ghostlight",
    "ghostlight-mcp-connector",
    "ghostlight-browser-connector",
  ]);
  assert.deepEqual(executableNames("win32", "x64"), [
    "ghostlight.exe",
    "ghostlight-mcp-connector.exe",
    "ghostlight-browser-connector.exe",
  ]);
});

test("a bare launch is MCP stdio and every CLI request reaches the orchestrator", () => {
  assert.match(selectedExecutable([], "linux", "x64"), /mcp-connector/);
  assert.equal(
    selectedExecutable(["install"], "linux", "x64"),
    "ghostlight",
  );
});

test("the checksum manifest must bind the exact version and file set", () => {
  const names = publishedAssetNames();
  const binaries = Object.fromEntries(names.map((name) => [name, "a".repeat(64)]));
  assert.doesNotThrow(() =>
    validateChecksums({ version: "1.0.0", algorithm: "sha256", binaries }, "1.0.0", names),
  );
  assert.throws(
    () => validateChecksums({ version: "0.8.0", algorithm: "sha256", binaries }, "1.0.0", names),
    /does not match/,
  );
  assert.throws(
    () => validateChecksums({ version: "1.0.0", algorithm: "sha256", binaries: {} }, "1.0.0", names),
    /complete binary checksum set/,
  );
});

test("downloads accept only the fixed HTTPS release host chain", () => {
  for (const host of [
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
  ]) {
    assert.doesNotThrow(() => assertAllowedDownload(`https://${host}/asset`));
  }
  assert.throws(() => assertAllowedDownload("http://github.com/asset"), /untrusted/);
  assert.throws(() => assertAllowedDownload("https://example.com/asset"), /untrusted/);
});

test("a matching cached binary is reused without a network request", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ghostlight-npm-cache-"));
  const path = join(directory, "ghostlight");
  const bytes = Buffer.from("trusted-binary");
  const expectedHash = createHash("sha256").update(bytes).digest("hex");
  await writeFile(path, bytes);
  let fetched = false;
  let announced = false;
  try {
    const changed = await ensureBinary({
      path,
      url: "https://github.com/sylin-org/ghostlight/releases/download/v1.0.0/ghostlight",
      expectedHash,
      fetchImpl: async () => {
        fetched = true;
      },
      onDownload: () => {
        announced = true;
      },
    });
    assert.equal(changed, false);
    assert.equal(fetched, false);
    assert.equal(announced, false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("a corrupt cached binary is replaced only by verified bytes", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ghostlight-npm-repair-"));
  const path = join(directory, "ghostlight");
  const trusted = Buffer.from("fresh-trusted-binary");
  const expectedHash = createHash("sha256").update(trusted).digest("hex");
  await writeFile(path, "corrupt");
  let announced = false;
  try {
    const changed = await ensureBinary({
      path,
      url: "https://github.com/sylin-org/ghostlight/releases/download/v1.0.0/ghostlight",
      expectedHash,
      fetchImpl: async () => new Response(trusted, { status: 200 }),
      onDownload: () => {
        announced = true;
      },
    });
    assert.equal(changed, true);
    assert.equal(announced, true);
    assert.deepEqual(await readFile(path), trusted);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("unverified download bytes never replace the cache", async () => {
  const directory = await mkdtemp(join(tmpdir(), "ghostlight-npm-reject-"));
  const path = join(directory, "ghostlight");
  await writeFile(path, "old-corrupt");
  try {
    await assert.rejects(
      ensureBinary({
        path,
        url: "https://github.com/sylin-org/ghostlight/releases/download/v1.0.0/ghostlight",
        expectedHash: "a".repeat(64),
        fetchImpl: async () => new Response("wrong", { status: 200 }),
      }),
      /failed checksum/,
    );
    assert.equal(await readFile(path, "utf8"), "old-corrupt");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
