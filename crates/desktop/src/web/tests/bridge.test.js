"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const SOURCE = fs.readFileSync(path.join(__dirname, "..", "bridge.js"), "utf8");

function load() {
  const posted = [];
  const errors = [];
  const window = { ipc: { postMessage: (text) => posted.push(text) } };
  const context = vm.createContext({
    window,
    console: { error: (error) => errors.push(error) },
    JSON,
    Object,
  });
  vm.runInContext(SOURCE, context);
  return { window, posted, errors };
}

test("post sends the JSON text through the ipc channel", () => {
  const { window, posted } = load();
  window.ghostlightShell.post({ kind: "ping", n: 1 });
  assert.deepEqual(posted, ['{"kind":"ping","n":1}']);
});

test("listen receives delivered messages in registration order", () => {
  const { window } = load();
  const seen = [];
  window.ghostlightShell.listen((value) => seen.push(["a", value]));
  window.ghostlightShell.listen((value) => seen.push(["b", value]));
  window.__ghostlightShellDeliver('{"x":1}');
  window.__ghostlightShellDeliver("[2]");
  assert.deepEqual(JSON.parse(JSON.stringify(seen)), [
    ["a", { x: 1 }],
    ["b", { x: 1 }],
    ["a", [2]],
    ["b", [2]],
  ]);
});

test("the function listen returns stops delivery", () => {
  const { window } = load();
  const seen = [];
  const stop = window.ghostlightShell.listen((value) => seen.push(value));
  window.__ghostlightShellDeliver("1");
  stop();
  window.__ghostlightShellDeliver("2");
  assert.deepEqual(seen, [1]);
});

test("a throwing handler is reported and the next handler still runs", () => {
  const { window, errors } = load();
  const seen = [];
  window.ghostlightShell.listen(() => {
    throw new Error("boom");
  });
  window.ghostlightShell.listen((value) => seen.push(value));
  window.__ghostlightShellDeliver("3");
  assert.deepEqual(seen, [3]);
  assert.equal(errors.length, 1);
});

test("ghostlightShell cannot be replaced or redefined", () => {
  const { window } = load();
  const original = window.ghostlightShell;
  assert.throws(() => {
    window.ghostlightShell = {};
  }, TypeError);
  assert.equal(window.ghostlightShell, original);
  assert.throws(() =>
    Object.defineProperty(window, "ghostlightShell", { value: {} }),
  );
  assert.ok(Object.isFrozen(original));
});

test("the delivery entry point is not enumerable and cannot be replaced", () => {
  const { window } = load();
  assert.equal(Object.keys(window).includes("__ghostlightShellDeliver"), false);
  const original = window.__ghostlightShellDeliver;
  assert.throws(() => {
    window.__ghostlightShellDeliver = () => {};
  }, TypeError);
  assert.equal(window.__ghostlightShellDeliver, original);
});
