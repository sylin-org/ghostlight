"use strict";

const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const test = require("node:test");

const extension = join(__dirname, "..");

test("the popup offers the diagnostics toggle only beside its state", () => {
  const markup = readFileSync(join(extension, "popup.html"), "utf8");
  assert.match(markup, /<section id="diagnostics-row" hidden>/);
  assert.match(markup, /<button id="diagnostics-toggle" type="button" class="link-button">/);
  const script = readFileSync(join(extension, "popup.js"), "utf8");
  assert.match(script, /diagnosticsRow\.hidden = !state;/);
  assert.match(script, /kind: "diagnostics_toggle"/);
});

test("the worker sends the toggle only when the service advertised it", () => {
  const source = readFileSync(join(extension, "service-worker.js"), "utf8");
  // The gate, not just the send: an older service never advertised diagnostics state, and the
  // unknown event would fail its decoder.
  const arm = source.match(/function requestDiagnosticsToggle\(\) \{[\s\S]*?\n\}/);
  assert.ok(arm, "requestDiagnosticsToggle exists");
  assert.match(arm[0], /if \(!liveState\.diagnostics\)/);
  assert.match(arm[0], /diagnostics_toggle_requested/);
  assert.match(source, /diagnostics: frame\.diagnostics \?\? null/);
  assert.match(source, /kind === "diagnostics_toggle"/);
});

test("hello and control-state frames feed the popup's diagnostics state", () => {
  const source = readFileSync(join(extension, "service-worker.js"), "utf8");
  assert.match(source, /diagnostics: null/);
  const controlArm = source.match(/if \(frame\.kind === "control_state"\) \{[\s\S]*?\n  \}/);
  assert.ok(controlArm, "control_state arm exists");
  assert.match(controlArm[0], /diagnostics: frame\.diagnostics \?\? null/);
});
