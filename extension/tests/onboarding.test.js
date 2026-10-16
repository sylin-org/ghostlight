"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const shared = require("../lib/shared.js");

test("a missing native host is classified as host-absent, not merely unreachable", () => {
  assert.equal(
    shared.linkState({
      connected: false,
      compatible: true,
      lastError: "Specified native messaging host not found."
    }),
    shared.LINK_HOST_ABSENT
  );
});

test("an unrecognized disconnect reason falls back to the unreachable state", () => {
  for (const lastError of [
    "Native connection ended.",
    "Native host has exited.",
    "",
    null,
    undefined
  ]) {
    assert.equal(
      shared.linkState({ connected: false, compatible: true, lastError }),
      shared.LINK_UNREACHABLE
    );
  }
});

test("a connected snapshot is never classified as host-absent", () => {
  assert.equal(
    shared.linkState({
      connected: true,
      compatible: true,
      lastError: "Specified native messaging host not found."
    }),
    shared.LINK_CONNECTED
  );
});

test("an outdated service outranks every disconnect reason, and a connected link is never reclassified", () => {
  assert.equal(
    shared.linkState({ connected: false, compatible: false, outdated: true, lastError: "Specified native messaging host not found." }),
    shared.LINK_SERVICE_OUTDATED
  );
  assert.equal(shared.linkState({ connected: false, compatible: false, outdated: true }), "service_outdated");
  assert.equal(shared.linkState({ connected: true, compatible: true, outdated: false }), shared.LINK_CONNECTED);
});
