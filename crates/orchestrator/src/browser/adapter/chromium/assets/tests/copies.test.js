"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

const root = join(__dirname, "..", "..", "..", "..", "..", "..", "..", "..");

// The extension worker and the page bundle both run these two files. Each keeps its own copy so
// an asset change never touches a file the published extension loads. Until the worker stops
// loading its copy, a one-sided edit would make the worker and the pages disagree.
for (const name of ["shared.js", "form-diagnostics.js"]) {
  test(`${name} matches the copy the extension worker loads`, () => {
    const asset = readFileSync(join(__dirname, "..", name), "utf8");
    const worker = readFileSync(join(root, "extension", "lib", name), "utf8");
    assert.equal(asset, worker);
  });
}
