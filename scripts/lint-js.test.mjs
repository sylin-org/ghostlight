// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- tests for scripts/lint-js.mjs: small fixtures for each rule and scope form.

import assert from "node:assert/strict";
import test from "node:test";
import { kindOf, lintSources } from "./lint-js.mjs";

/** Lint one source text as the given kind and return "line: text" strings. */
function lint(text, kind = "node") {
  return lintSources([{ file: "fixture.js", text }], kind).map((m) => `${m.line}: ${m.text}`);
}

test("a binding that is never read is reported", () => {
  assert.deepEqual(lint("const a = 1;\nconst b = 2;\nconsole.log(b);"), ["1: 'a' is declared but never read"]);
});

test("a binding only assigned is reported as assigned", () => {
  assert.deepEqual(lint("let a = 1;\na = 2;"), ["1: 'a' is assigned but never read"]);
});

test("a compound assignment or update whose value is used counts as a read", () => {
  assert.deepEqual(lint("let i = 0;\nconst xs = [];\nxs[i++] = 1;\nconsole.log(xs);"), []);
  assert.deepEqual(lint("let n = 0;\nn++;\nn += 2;"), ["1: 'n' is assigned but never read"]);
});

test("an undefined name is reported and a known global is not", () => {
  assert.deepEqual(lint("console.log(missing, JSON, Math);"), ["1: 'missing' is not defined"]);
});

test("typeof guards an undeclared name", () => {
  assert.deepEqual(lint("if (typeof maybe !== 'undefined') console.log(1);"), []);
});

test("globals depend on the kind", () => {
  assert.deepEqual(lint("chrome.runtime.id;", "worker"), []);
  assert.deepEqual(lint("chrome.runtime.id;", "node"), ["1: 'chrome' is not defined"]);
  assert.deepEqual(lint("document.title;", "page"), []);
  assert.deepEqual(lint("console.log(__dirname);", "module"), ["1: '__dirname' is not defined"]);
});

test("an underscore name is allowed", () => {
  assert.deepEqual(lint("const _a = 1;\ntry { throw 1; } catch (_) {}"), []);
});

test("var hoists to the function and a function declaration hoists to its scope", () => {
  assert.deepEqual(lint("console.log(later());\nfunction later() { return v; }\nvar v = 1;"), []);
  assert.deepEqual(lint("function f() { if (true) { var x = 1; } return x; }\nf();"), []);
});

test("let and const are block scoped", () => {
  assert.deepEqual(lint("{ const a = 1; console.log(a); }\nconsole.log(a);"), ["2: 'a' is not defined"]);
});

test("shadowing keeps the outer binding unused", () => {
  assert.deepEqual(lint("const a = 1;\nfunction f(a) { return a; }\nf(2);"), ["1: 'a' is declared but never read"]);
});

test("a parameter before the last used one is allowed, after it is reported", () => {
  assert.deepEqual(lint("function f(a, b) { return b; }\nf(1, 2);"), []);
  assert.deepEqual(lint("function f(a, b) { return a; }\nf(1, 2);"), ["1: 'b' is declared but never read"]);
});

test("destructuring declares each name", () => {
  assert.deepEqual(lint("const { a, b: [c, d], e } = {};\nconsole.log(c);"), [
    "1: 'a' is declared but never read",
    "1: 'd' is declared but never read",
    "1: 'e' is declared but never read"
  ]);
});

test("a destructured name beside a rest element is allowed", () => {
  assert.deepEqual(lint("const { a, ...rest } = {};\nconsole.log(rest);"), []);
});

test("destructuring defaults and computed keys are reads", () => {
  assert.deepEqual(lint("const k = 'x';\nconst d = 1;\nconst { [k]: v = d } = {};\nconsole.log(v);"), []);
});

test("destructuring assignment writes existing names", () => {
  assert.deepEqual(lint("let a, b;\n[a, b] = [1, 2];\nconsole.log(a);"), ["1: 'b' is assigned but never read"]);
});

test("object keys and member properties are not references", () => {
  assert.deepEqual(lint("const o = { missing: 1 };\nconsole.log(o.other, o);"), []);
});

test("a named function expression and a class name are not reported", () => {
  assert.deepEqual(lint("const f = function inner() {};\nconst C = class Name {};\nconsole.log(f, C);"), []);
});

test("an unused function or class declaration is reported", () => {
  assert.deepEqual(lint("function f() {}\nclass C {}"), ["1: 'f' is declared but never read", "2: 'C' is declared but never read"]);
});

test("catch parameters and loop variables are bindings", () => {
  assert.deepEqual(lint("try { 1; } catch (e) { }"), ["1: 'e' is declared but never read"]);
  assert.deepEqual(lint("for (const x of [1]) { }"), ["1: 'x' is declared but never read"]);
});

test("imports and exports in a module", () => {
  assert.deepEqual(lint("import fs from 'node:fs';\nexport const a = 1;", "module"), ["1: 'fs' is declared but never read"]);
});

test("an anonymous default export function or class is not a binding", () => {
  assert.deepEqual(lint("export default function () {}", "module"), []);
  assert.deepEqual(lint("export default class {}", "module"), []);
});

test("document names are undefined in the worker and allowed in an extension page", () => {
  assert.deepEqual(lint("document.title;", "worker"), ["1: 'document' is not defined"]);
  assert.deepEqual(lint("window.name;", "worker"), ["1: 'window' is not defined"]);
  assert.deepEqual(lint("document.title; chrome.runtime.id;", "extpage"), []);
});

test("page assets share one scope and may return at the top", () => {
  const sources = [
    { file: "dir/chromium/protocol4/assets/world.js", text: "if (!document) return;\nconsole.log(helper(__ghostlightAssetId));" },
    { file: "dir/chromium/protocol4/assets/shared.js", text: "function helper(x) { return x; }" }
  ];
  assert.deepEqual(lintSources(sources), []);
});

test("a syntax error is reported once", () => {
  assert.equal(lint("const = ;").length, 1);
});

test("kinds follow the path", () => {
  assert.equal(kindOf("scripts/lint-js.mjs"), "module");
  assert.equal(kindOf("extension/tests/worker.test.js"), "node");
  assert.equal(kindOf("crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/world.js"), "page");
  assert.equal(kindOf("extension/lib/world.js"), "worker");
  assert.equal(kindOf("extension/worker.js"), "worker");
  assert.equal(kindOf("extension/popup.js"), "extpage");
  assert.equal(kindOf("extension/offscreen.js"), "extpage");
  assert.equal(kindOf("extension/lib/state.js"), "worker");
  assert.equal(kindOf("extension/lib/recording.js"), "worker");
  assert.equal(kindOf("extension/lib/gif.js"), "extpage");
});
