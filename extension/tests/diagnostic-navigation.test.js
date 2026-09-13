"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

test("tracked navigation restarts its volatile ring without disabling CDP capture", () => {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  const start = source.indexOf("chrome.webNavigation.onBeforeNavigate.addListener");
  const end = source.indexOf("chrome.tabs.onUpdated.addListener", start);
  assert.ok(start >= 0 && end > start);
  const listener = source.slice(start, end);
  assert.match(listener, /diagnostics\.forget\(details\.tabId\)/);
  assert.match(listener, /diagnostics\.enable\(details\.tabId\)/);
  assert.doesNotMatch(listener, /disableDiagnosticCapture/);
});
