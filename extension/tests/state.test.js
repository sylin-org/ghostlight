"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const state = require("../lib/state.js");

test("a held session wears the accent badge with two bars", () => {
  assert.deepEqual(state.badge({ control_state: "held", recording_tabs: 0 }), { text: "II", color: "#5eead4" });
  assert.deepEqual(state.badge({ control_state: "held", recording_tabs: 2 }), { text: "II", color: "#5eead4" });
});

test("a recording wears the red badge", () => {
  assert.deepEqual(state.badge({ control_state: "active", recording_tabs: 1 }), { text: "REC", color: "#ef4444" });
});

test("otherwise the badge carries no text", () => {
  for (const control_state of ["active", "ended"]) {
    assert.equal(state.badge({ control_state, recording_tabs: 0 }).text, "");
  }
});

test("the badge text is dark enough to read on the accent", () => {
  assert.equal(state.BADGE_TEXT_COLOR, "#08110f");
});
