// SPDX-License-Identifier: Apache-2.0 OR MIT
// Architectural wiring checks for the browser input boundary.

const { test } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const root = path.join(__dirname, "../..");
const worker = fs.readFileSync(path.join(root, "extension/service-worker.js"), "utf8");
const content = fs.readFileSync(path.join(root, "extension/content.js"), "utf8");
const manifest = JSON.parse(fs.readFileSync(path.join(root, "extension/manifest.json"), "utf8"));

test("worker routes input packets through pure planners", () => {
  assert.match(worker, /importScripts\([^\n]+"lib\/input-events\.js"/);
  assert.match(worker, /const plan = keyDispatchPlan\(combo\)/);
  assert.match(worker, /const dispatch = textDispatchPlan\(a\.text\)/);
  assert.match(worker, /mouseButtonEvent\("mousePressed", sx, sy, "left", modifiers, 1\)/);
  assert.match(worker, /mouseMoveEvent\(tx, ty, modifiers, BUTTON_BITS\.left\)/);
  assert.match(worker, /mouseWheelEvent\(c\[0\], c\[1\], deltaX, deltaY, modifiers\)/);
});

test("type reports Unicode code points after CRLF normalization", () => {
  assert.match(worker, /Typed \$\{dispatch\.characterCount\} character\(s\)\./);
  assert.doesNotMatch(worker, /Typed \$\{a\.text\.length\}/);
});

test("false-success input paths carry explicit guards or qualified results", () => {
  assert.match(worker, /if \(!a\.ref && !a\.coordinate\) return text\("ref or coordinate is required for scroll_to\."\)/);
  assert.match(content, /Page signaled handling for screenshot drag\/drop/);
  assert.match(content, /the page did not signal handling/);
  assert.doesNotMatch(content, /output: "Dropped screenshot/);
});

test("native HTML drag uses a bounded action-scoped coordinator", () => {
  assert.match(worker, /Input\.setInterceptDrags", \{ enabled: true \}/);
  assert.match(worker, /Input\.setInterceptDrags", \{ enabled: false \}/);
  assert.match(worker, /method === "Input\.dragIntercepted"/);
  assert.match(worker, /dragCoordinator\.intercepted\(tabId, params && params\.data\)/);
  assert.match(worker, /Input\.dispatchDragEvent", dragEvent\("dragEnter"/);
  assert.match(worker, /Input\.dispatchDragEvent", dragEvent\("dragOver"/);
  assert.match(worker, /Input\.dispatchDragEvent", dragEvent\("drop"/);
  assert.match(worker, /Input\.cancelDragging/);
  assert.match(worker, /nativeExpected \? DRAG_INTERCEPT_WAIT_MS : DRAG_INTERCEPT_GRACE_MS/);
  assert.match(worker, /const activeDragOperations = new Map\(\)/);
  assert.match(worker, /info\.status === "loading"[\s\S]*?cancelActiveDrag\(tabId\)/);
  assert.match(worker, /clearTabState\(tabId\)[\s\S]*?activeDragOperations\.delete\(tabId\)/);
});

test("dragstart observation retains structure only and loads before content", () => {
  const start = content.indexOf("const DRAG_OBSERVATION_MESSAGES");
  const end = content.indexOf("// ADR-0078 D2", start);
  const observer = content.slice(start, end);
  assert.match(observer, /event\.isTrusted/);
  assert.match(observer, /event\.defaultPrevented/);
  assert.match(observer, /return \{ started: observation\.started, cancelled: observation\.cancelled \}/);
  assert.doesNotMatch(observer, /dataTransfer|event\.target|textContent|innerText|value/);

  const contentScripts = manifest.content_scripts[0].js;
  assert.ok(contentScripts.indexOf("lib/drag-session.js") < contentScripts.indexOf("content.js"));
  assert.match(worker, /"lib\/drag-session\.js", "content\.js"/);
});
