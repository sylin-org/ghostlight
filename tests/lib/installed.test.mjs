// Unit tests for how a live journey finds the installed images. Nothing here starts a process.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { EDGE_NAME, SERVICE_NAME, suffix } from "./home.mjs";
import { homeBinDirectory, installedBinDirectory, installedEdge, installedImages } from "./installed.mjs";

/** Run `body` with a temporary Ghostlight home holding the named images, and the environment pointed at it. */
function withHome(images, body, binDirectory) {
  const home = mkdtempSync(join(tmpdir(), "ghostlight-installed-"));
  const bin = join(home, "bin");
  mkdirSync(bin);
  for (const name of images) writeFileSync(join(bin, name), `image:${name}`);
  const saved = { home: process.env.GHOSTLIGHT_TEST_HOME, bin: process.env.GHOSTLIGHT_BIN_DIR };
  process.env.GHOSTLIGHT_TEST_HOME = home;
  if (binDirectory === undefined) delete process.env.GHOSTLIGHT_BIN_DIR;
  else process.env.GHOSTLIGHT_BIN_DIR = binDirectory(home);
  try {
    body(bin);
  } finally {
    for (const [key, value] of [["GHOSTLIGHT_TEST_HOME", saved.home], ["GHOSTLIGHT_BIN_DIR", saved.bin]]) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
    rmSync(home, { recursive: true, force: true });
  }
}

const DESKTOP_NAME = `ghostlight-desktop${suffix}`;

test("the installed bin is the Ghostlight home's own unless the same directory is named", () => {
  withHome([EDGE_NAME, SERVICE_NAME], (bin) => {
    assert.equal(homeBinDirectory(), bin);
    assert.equal(installedBinDirectory(), bin);
  });
  withHome([EDGE_NAME, SERVICE_NAME], (bin) => {
    assert.equal(installedBinDirectory(), bin);
  }, (home) => join(home, "bin"));
});

test("a bin directory outside the home is refused, because an edge there is a carrier", () => {
  withHome([EDGE_NAME, SERVICE_NAME], () => {
    assert.throws(() => installedBinDirectory(), /carrier/);
  }, (home) => join(home, "elsewhere"));
});

test("the installed edge must exist", () => {
  withHome([SERVICE_NAME], (bin) => {
    assert.throws(() => installedEdge(bin), { code: "ENOENT" });
  });
  withHome([EDGE_NAME, SERVICE_NAME], (bin) => {
    assert.equal(installedEdge(bin), join(bin, EDGE_NAME));
  });
});

test("the images record the edge and the service, and the shell only when it is installed", () => {
  withHome([EDGE_NAME, SERVICE_NAME], (bin) => {
    const images = installedImages(bin);
    assert.deepEqual(Object.keys(images), ["ghostlight", "ghostlight-service"]);
    assert.equal(images.ghostlight.sha256, createHash("sha256").update(`image:${EDGE_NAME}`).digest("hex"));
  });
  withHome([EDGE_NAME, SERVICE_NAME, DESKTOP_NAME], (bin) => {
    assert.deepEqual(Object.keys(installedImages(bin)), ["ghostlight", "ghostlight-service", "ghostlight-desktop"]);
  });
});

test("an installed set without its service is refused", () => {
  withHome([EDGE_NAME], (bin) => {
    assert.throws(() => installedImages(bin), { code: "ENOENT" });
  });
});
