// SPDX-License-Identifier: Apache-2.0 OR MIT
// Source contract for ADR-0078's visible semantic-target cue.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const indicator = fs.readFileSync(path.join(root, "extension/agent-visual-indicator.js"), "utf8");
const content = fs.readFileSync(path.join(root, "extension/content.js"), "utf8");
const worker = fs.readFileSync(path.join(root, "extension/service-worker.js"), "utf8");

test("semantic target cue is deterministic, pointer-safe, and policy-free", () => {
  assert.match(worker, /AGENT_SEMANTIC_TARGET/);
  assert.match(indicator, /function semanticTarget\(x, y, action\)/);
  for (const caption of [
    "Click target", "Context-menu target", "Double-click target",
    "Hover target", "Scroll target", "Field target",
  ]) {
    assert.ok(indicator.includes(caption), caption);
  }
  assert.match(indicator, /pointer-events:none/);
  assert.doesNotMatch(indicator, /Capability::|grant_id|Governance|authorize\(/);
});

test("semantic cue stays out of reads and hides with the visual layer", () => {
  assert.match(content, /el\.id\.indexOf\("ghostlight-"\) === 0/);
  assert.match(indicator, /if \(hiddenForTool \|\| document\.hidden\) return/);
  assert.match(indicator, /g\.id = FX_LAYER_ID \+ "-g"/);
});
