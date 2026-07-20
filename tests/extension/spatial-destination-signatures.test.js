// SPDX-License-Identifier: Apache-2.0 OR MIT
// Source contracts for ADR-0089's destination-aware spatial cues.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const indicator = fs.readFileSync(path.join(root, "extension/agent-visual-indicator.js"), "utf8");
const content = fs.readFileSync(path.join(root, "extension/content.js"), "utf8");

test("ref scroll lands its cue on the exact content-script target", () => {
  assert.match(content, /el\.scrollIntoView\(\{ block: "center", behavior: "instant" \}\);\s*scrollTargetFx\(el\)/);
  assert.match(indicator, /function scrollTarget\(target\)/);
  assert.match(indicator, /ghostlight-scroll-target/);
  assert.match(indicator, /sensibleDestinationRect\(target\)/);
});

test("semantic scroll composition suppresses a duplicate target halo", () => {
  assert.match(indicator, /recentSemanticTarget = target \? \{ target, at: Date\.now\(\) \} : null/);
  assert.match(indicator, /recentSemanticTarget\.target === target/);
  assert.match(indicator, /if \(!duplicateSemanticTarget\) destinationHalo\(rect, "st-h"\)/);
});

test("coordinate image placement uses a fixed content-free photo treatment", () => {
  const setImageStart = content.indexOf("function setImage(");
  const setImageEnd = content.indexOf("function refCoordinates(", setImageStart);
  const setImage = content.slice(setImageStart, setImageEnd);
  assert.match(setImage, /imageDropFx\(el, x, y\)/);
  assert.doesNotMatch(setImage, /imageDropFx\([^\n]*(?:dataB64|filename|mimeType|name|type)/);

  const imageDropStart = indicator.indexOf("function imageDrop(");
  const imageDropEnd = indicator.indexOf("function renderControlBorder(", imageDropStart);
  const imageDrop = indicator.slice(imageDropStart, imageDropEnd);
  assert.match(imageDrop, /ghostlight-image-drop/);
  assert.match(imageDrop, /Image drop dispatched/);
  assert.doesNotMatch(imageDrop, /filename|mimeType|dataB64|accepted|handled/);
});

test("destination cues preserve visual safety invariants", () => {
  assert.match(indicator, /function viewportRectForElement\(target\)/);
  assert.match(indicator, /ownerWindow\.frameElement/);
  assert.match(indicator, /if \(hiddenForTool \|\| document\.hidden\) return/);
  assert.match(indicator, /ghostlight-scroll-target-rm/);
  assert.match(indicator, /ghostlight-image-drop-rm/);
  assert.match(indicator, /pointer-events:none/);
  assert.match(indicator, /id = FX_LAYER_ID \+ "-st"/);
  assert.match(indicator, /id = FX_LAYER_ID \+ "-id"/);
});
