// SPDX-License-Identifier: Apache-2.0 OR MIT

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const findVisual = require("../../extension/lib/find-visual.js");

const root = path.join(__dirname, "../..");
const workerSource = fs.readFileSync(path.join(root, "extension/service-worker.js"), "utf8");
const contentSource = fs.readFileSync(path.join(root, "extension/content.js"), "utf8");
const rendererSource = fs.readFileSync(
  path.join(root, "extension/agent-visual-indicator.js"),
  "utf8"
);
const manifestSource = fs.readFileSync(path.join(root, "extension/manifest.json"), "utf8");

test("find visual messages contain only fixed lifecycle and aggregate result state", () => {
  assert.deepEqual(findVisual.message(findVisual.PHASES.START), {
    type: "AGENT_FIND_VISUAL",
    phase: "start",
  });
  assert.deepEqual(findVisual.message(findVisual.PHASES.FOUND, 20, true), {
    type: "AGENT_FIND_VISUAL",
    phase: "found",
    count: 20,
    more: true,
  });
  assert.deepEqual(findVisual.message(findVisual.PHASES.EMPTY), {
    type: "AGENT_FIND_VISUAL",
    phase: "empty",
    count: 0,
    more: false,
  });
  assert.deepEqual(findVisual.message(findVisual.PHASES.CANCEL), {
    type: "AGENT_FIND_VISUAL",
    phase: "cancel",
  });
});

test("find visual vocabulary rejects unknown phases and invalid counts", () => {
  assert.throws(() => findVisual.message("searching"), /unknown.*phase/);
  assert.throws(() => findVisual.message(findVisual.PHASES.FOUND, 0), /count/);
  assert.throws(() => findVisual.message(findVisual.PHASES.FOUND, 21), /count/);
  assert.equal(findVisual.isMessage({ type: findVisual.TYPE, phase: "found", count: 1, more: false }), true);
  assert.equal(findVisual.isMessage({ type: findVisual.TYPE, phase: "found", count: 1 }), false);
});

test("find uses its own broker lane and keeps DOM presentation local", () => {
  const findTool = workerSource.slice(
    workerSource.indexOf("  async find(a)"),
    workerSource.indexOf("  async form_input(a)")
  );
  assert.match(workerSource, /importScripts\([^\n]*lib\/find-visual\.js/);
  assert.match(workerSource, /channel: self\.GhostlightFindVisual\.CHANNEL/);
  assert.match(findTool, /await startFindVisual\(tabId\)/);
  assert.match(findTool, /present: true/);
  assert.doesNotMatch(findTool, /readScan/);
  assert.match(contentSource, /self\.GhostlightFx\.findResults\(entries/);
  assert.match(contentSource, /document\.createRange\(\)/);
  assert.doesNotMatch(workerSource, /GhostlightFindVisual\.message\([^\n]*(query|text)/);
  assert.match(manifestSource, /lib\/find-visual\.js/);
});

test("renderer distinguishes ranked text, shape fallback, and offscreen results", () => {
  assert.match(rendererSource, /::highlight\(ghostlight-find-primary\)/);
  assert.match(rendererSource, /::highlight\(ghostlight-find-secondary\)/);
  assert.match(rendererSource, /ghostlight-find-halo/);
  assert.match(rendererSource, /ensureFindEdge\("top"\)/);
  assert.match(rendererSource, /ensureFindEdge\("bottom"\)/);
  assert.match(rendererSource, /rect\.bottom <= 0/);
  assert.match(rendererSource, /rect\.top >= window\.innerHeight/);
  assert.match(rendererSource, /prefers-reduced-motion:reduce/);
  assert.match(rendererSource, /pointer-events:none/);
  assert.match(rendererSource, /if \(v\) clearFindVisual\(\)/);
});
