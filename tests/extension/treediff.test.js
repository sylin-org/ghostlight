// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/treediff.js (ADR-0037 Decision 3: read_page diff mode),
// PINS.md SS11.

const { test } = require("node:test");
const assert = require("node:assert");
const { diffLines } = require("../../extension/lib/treediff.js");

test("the pinned oracle: ref-keyed changed/removed/added", () => {
  const oldLines = ['ref_1 button "A"', 'ref_2 link "B"'];
  const newLines = ['ref_1 button "A2"', 'ref_3 link "C"'];
  assert.deepStrictEqual(diffLines(oldLines, newLines), {
    changed: ['ref_1 button "A2"'],
    removed: ['ref_2 link "B"'],
    added: ['ref_3 link "C"'],
  });
});

test("identical inputs yield all three empty", () => {
  const lines = ['ref_1 button "A"', 'ref_2 link "B"'];
  const d = diffLines(lines, lines);
  assert.deepStrictEqual(d.added, []);
  assert.deepStrictEqual(d.removed, []);
  assert.deepStrictEqual(d.changed, []);
});

test("keyless lines compare by whole-line identity", () => {
  // No ref token: identity is the whole line. A changed keyless line is removed + added, not
  // "changed" (there is no stable key to call it the same line).
  const d = diffLines(["heading Intro", "footer"], ["heading Intro!", "footer"]);
  assert.deepStrictEqual(d.changed, []);
  assert.deepStrictEqual(d.removed, ["heading Intro"]);
  assert.deepStrictEqual(d.added, ["heading Intro!"]);
});
