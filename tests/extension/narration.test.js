// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/narration.js (ADR-0072).

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const {
  createNarrationStore,
  normalizeDuration,
  normalizePosition,
} = require("../../extension/lib/narration.js");

test("one_narration_per_tab_replaces_immediately", () => {
  let now = 1000;
  const store = createNarrationStore(() => now);
  const first = store.show(7, "First phase", "top", 5000);
  assert.strictEqual(first.replaced, false);
  const second = store.show(7, "Second phase", "bottom", 6000);
  assert.strictEqual(second.replaced, true);
  assert.notStrictEqual(second.record.generation, first.record.generation);
  assert.strictEqual(store.current(7).text, "Second phase");
});

test("stale_generation_cannot_remove_a_replacement", () => {
  const store = createNarrationStore(() => 1000);
  const first = store.show(7, "First", "bottom", 5000).record;
  const second = store.show(7, "Second", "bottom", 5000).record;
  assert.strictEqual(store.remove(7, first.generation), false);
  assert.strictEqual(store.current(7).generation, second.generation);
  assert.strictEqual(store.remove(7, second.generation), true);
  assert.strictEqual(store.current(7), null);
});

test("expiry_and_navigation_replay_use_only_the_remaining_duration", () => {
  let now = 1000;
  const store = createNarrationStore(() => now);
  store.show(4, "Still working", "auto", 5000);
  now = 3400;
  assert.deepStrictEqual(
    store.current(4),
    {
      generation: 1,
      text: "Still working",
      position: "auto",
      durationMs: 5000,
      deadline: 6000,
      remainingMs: 2600,
    }
  );
  now = 6000;
  assert.strictEqual(store.current(4), null);
});

test("tabs_are_independent_and_clear_returns_every_live_tab", () => {
  const store = createNarrationStore(() => 1000);
  store.show(1, "One", "top", 5000);
  store.show(2, "Two", "bottom", 5000);
  assert.deepStrictEqual(store.clear(), [1, 2]);
  assert.strictEqual(store.current(1), null);
  assert.strictEqual(store.current(2), null);
});

test("defense_in_depth_normalizes_position_and_duration", () => {
  assert.strictEqual(normalizePosition("sideways"), "auto");
  assert.strictEqual(normalizeDuration(undefined), 5000);
  assert.strictEqual(normalizeDuration(20), 1000);
  assert.strictEqual(normalizeDuration(90000), 30000);
});

test("renderer_contract_is_pointer_transparent_text_only_and_capture_hidden", () => {
  const source = fs.readFileSync(
    path.join(__dirname, "../../extension/agent-visual-indicator.js"),
    "utf8"
  );
  assert.match(source, /narrationText\.textContent = text/);
  assert.match(source, /ghostlight-narration-layer/);
  assert.match(source, /pointer-events:none/);
  assert.match(source, /if \(narrationLayer\) narrationLayer\.style\.display = v \? "none" : ""/);
  assert.match(source, /prefers-reduced-motion:reduce/);
  assert.match(source, /shown: false, reason: "visual effects are disabled"/);
  assert.match(source, /max-width:min\(84vw,900px\)/);
  assert.match(source, /ghostlight-narration-time[^}]+ghostlight-dots/);
  assert.doesNotMatch(source, /ghostlight-narration-progress/);
  assert.match(source, /return \{ shown: true, position \}/);
  assert.match(source, /width:min\(88vw,620px\)/);
  assert.match(source, /setTimeout\(dismissNotification, Math\.max\(500/);
  assert.match(source, /ghostlight-attention-layer/);
  assert.match(source, /backdrop-filter:blur\(5px\)/);
  assert.match(source, /Resume \+ quiet site repeats/);
});

test("worker_contract_replays_and_clears_transient_state", () => {
  const source = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  assert.match(source, /info\.status === "complete"/);
  assert.match(source, /renderNarration\(tabId, narration\)/);
  assert.match(source, /narrationStore\.remove\(tabId\)/);
  assert.match(source, /for \(const tabId of narrationStore\.clear\(\)\)/);
  assert.match(source, /type: "AGENT_NARRATION_CLEAR"/);
});

test("manifest_loads_the_pure_placement_module_before_the_renderer", () => {
  const manifest = JSON.parse(fs.readFileSync(
    path.join(__dirname, "../../extension/manifest.json"),
    "utf8"
  ));
  const visualScripts = manifest.content_scripts.find((entry) =>
    entry.js.includes("agent-visual-indicator.js")
  ).js;
  assert.deepStrictEqual(visualScripts, ["lib/narration-placement.js", "agent-visual-indicator.js"]);
});
