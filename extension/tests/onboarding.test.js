"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const shared = require("../lib/shared.js");

const NOT_INSTALLED_HERE = "Ghostlight is not installed on this computer yet.";
const SYNCED_PROFILE_LINE =
  "The extension came with your Chrome profile. Install Ghostlight here to connect it.";

function read(name) {
  return readFileSync(join(__dirname, "..", name), "utf8");
}

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

test("the options connection card renders the same state in the same words", () => {
  const markup = read("options.html");
  const script = read("options.js");
  assert.match(markup, /<button id="setup-route" type="button" hidden>/);
  assert.ok(script.includes(NOT_INSTALLED_HERE));
  assert.ok(script.includes(SYNCED_PROFILE_LINE));
  assert.match(script, /setupRoute\.hidden = snapshot\.link_state !== "host_absent"/);
});

test("the developer diagnostics card describes only the connection log", () => {
  const markup = read("options.html");
  assert.ok(markup.includes("Saves bounded connection details for this browser and shows the raw connection error."));
  assert.doesNotMatch(markup, /form[- ](reset|trac)/i);
});

test("an outdated service outranks every disconnect reason, and a connected link is never reclassified", () => {
  assert.equal(
    shared.linkState({ connected: false, compatible: false, outdated: true, lastError: "Specified native messaging host not found." }),
    shared.LINK_SERVICE_OUTDATED
  );
  assert.equal(shared.linkState({ connected: false, compatible: false, outdated: true }), "service_outdated");
  assert.equal(shared.linkState({ connected: true, compatible: true, outdated: false }), shared.LINK_CONNECTED);
});

test("the options card names the outdated service with the pinned sentence", () => {
  const sentence = "Update Ghostlight on this computer.";
  assert.ok(read("options.js").includes(sentence));
  assert.match(read("options.js"), /snapshot\.link_state === "service_outdated"/);
});
