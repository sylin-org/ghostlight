// SPDX-License-Identifier: Apache-2.0 OR MIT

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const root = path.join(__dirname, "../..");
const workerSource = fs.readFileSync(path.join(root, "extension/service-worker.js"), "utf8");
const contentSource = fs.readFileSync(path.join(root, "extension/content.js"), "utf8");
const rendererSource = fs.readFileSync(
  path.join(root, "extension/agent-visual-indicator.js"),
  "utf8"
);
const manifest = JSON.parse(fs.readFileSync(path.join(root, "extension/manifest.json"), "utf8"));

test("worker derives a bounded key cue from post-dispatch target observations", () => {
  const cueFunction = workerSource.slice(
    workerSource.indexOf("function keystrokeCue"),
    workerSource.indexOf("function scrollCue")
  );
  const keyAction = workerSource.slice(
    workerSource.indexOf('    case "key"'),
    workerSource.indexOf('    case "scroll"')
  );
  assert.match(cueFunction, /AGENT_KEYSTROKE", cue/);
  assert.doesNotMatch(cueFunction, /text/);
  assert.match(keyAction, /beginKeyCueObservation\(tabId, combos\.length \* repeat\)/);
  assert.match(keyAction, /finishKeyCueObservation\(tabId, observationToken\)/);
  assert.match(keyAction, /keystrokeCue\(tabId, keyCuePresentation\(a\.text, observedTargets, repeat\)\)/);
  assert.ok(keyAction.indexOf("await pressKey") < keyAction.indexOf("keystrokeCue(tabId"));
  assert.doesNotMatch(workerSource, /AGENT_KEYSTROKE", text/);
});

test("content observer retains only bounded structural classes from trusted events", () => {
  const observer = contentSource.slice(
    contentSource.indexOf("const KEY_CUE_OBSERVATION_MAX_EVENTS"),
    contentSource.indexOf("// ADR-0078 D2")
  );
  assert.match(observer, /event\.isTrusted/);
  assert.match(observer, /sensitive\(target\)/);
  assert.match(observer, /KEY_CUE_OBSERVATION_MAX_EVENTS = 600/);
  assert.match(observer, /targetStates\.push\(keyCueTarget\(event\)\)/);
  assert.doesNotMatch(observer, /event\.key|target\.value|textContent/);
});

test("renderer validates derived chords and inserts labels only through textContent", () => {
  const renderer = rendererSource.slice(
    rendererSource.indexOf("function keystrokeLozenge"),
    rendererSource.indexOf("function scrollCue")
  );
  assert.match(renderer, /isKeyCuePresentation\(cue\)/);
  assert.match(renderer, /ghostlight-private-keycap/);
  assert.match(renderer, /KEY_CUE_PRIVATE_TOKEN/);
  assert.match(renderer, /cap\.textContent/);
  assert.match(renderer, /plus\.textContent/);
  assert.match(renderer, /sequenceSeparator\.textContent/);
  assert.doesNotMatch(renderer, /innerHTML/);
});

test("every renderer activation path loads the shared key presentation domain first", () => {
  const contentScripts = manifest.content_scripts[0].js;
  const visualScripts = manifest.content_scripts[1].js;
  assert.equal(contentScripts.indexOf("lib/keys.js") < contentScripts.indexOf("content.js"), true);
  assert.equal(visualScripts.includes("lib/keys.js"), true);
  assert.equal(visualScripts.indexOf("lib/keys.js") < visualScripts.indexOf("agent-visual-indicator.js"), true);
  assert.match(workerSource, /VISUAL_SCRIPT_FILES = \[[^\n]*"lib\/keys\.js"[^\n]*"agent-visual-indicator\.js"/);
  assert.match(workerSource, /"lib\/actionable\.js", "lib\/keys\.js", "lib\/drag-session\.js", "content\.js"/);
});
