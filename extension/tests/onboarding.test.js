"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const shared = require("../lib/shared.js");

const NOT_INSTALLED_HERE = "Ghostlight is not installed on this computer yet.";
const SYNCED_PROFILE_LINE =
  "The extension came with your Chrome profile. Install Ghostlight here to connect it.";
const SETUP_ROUTE_LABEL = "Set up Ghostlight";

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

test("the popup names the host-absent state with the pinned sentence", () => {
  const markup = read("popup.html");
  const script = read("popup.js");
  assert.match(markup, new RegExp(NOT_INSTALLED_HERE.replace(/\./g, "\\.")));
  assert.match(markup, new RegExp(SYNCED_PROFILE_LINE.replace(/\./g, "\\.")));
  assert.ok(script.includes(NOT_INSTALLED_HERE));
  // The ordinary unreachable wording must survive alongside it, or the distinction is lost.
  assert.ok(script.includes("Waiting for the Ghostlight service..."));
});

test("the popup offers the setup route only in the host-absent state", () => {
  const markup = read("popup.html");
  const script = read("popup.js");
  assert.match(markup, /<section id="setup-section" hidden/);
  assert.ok(markup.includes(SETUP_ROUTE_LABEL));
  assert.match(script, /setupSection\.hidden = snapshot\.link_state !== HOST_ABSENT/);
});

test("the options connection card renders the same state in the same words", () => {
  const markup = read("options.html");
  const script = read("options.js");
  assert.match(markup, /<button id="setup-route" type="button" hidden>/);
  assert.ok(script.includes(NOT_INSTALLED_HERE));
  assert.ok(script.includes(SYNCED_PROFILE_LINE));
  assert.match(script, /setupRoute\.hidden = snapshot\.link_state !== "host_absent"/);
});

test("the bundled setup page needs no network and contains no product state", () => {
  const markup = read("setup.html");
  assert.ok(markup.includes(NOT_INSTALLED_HERE));
  assert.ok(markup.includes("npx -y ghostlight install"));
  assert.ok(markup.includes("ghostlight doctor"));
  // One outbound link is allowed, to the canonical walkthrough. Nothing may be fetched to render.
  assert.doesNotMatch(markup, /<script/);
  assert.doesNotMatch(markup, /<img[^>]+src="https?:/);
  assert.doesNotMatch(markup, /<link[^>]+href="https?:/);
});

test("the bundled setup page is packaged for the store build", () => {
  // The offline fallback only exists if the packaging allowlist ships it. A page that works
  // unpacked and is missing from the store ZIP is the worst version of this feature.
  const packaging = readFileSync(
    join(__dirname, "..", "..", "scripts", "package-extension.ps1"),
    "utf8"
  );
  assert.match(packaging, /"setup\.html"/);
});

test("the snapshot carries the closed classification and never the raw reason", () => {
  const worker = read("service-worker.js");
  assert.match(worker, /link_state: shared\.linkState\(\{/);
  // The classification is computed from the live state before last_error is generalized, so a
  // surface can distinguish the states without ever seeing Chrome's message text.
  const snapshotStart = worker.indexOf("function uiSnapshot()");
  const snapshotEnd = worker.indexOf("function updateBadge()");
  const body = worker.slice(snapshotStart, snapshotEnd);
  assert.ok(body.indexOf("link_state") < body.indexOf("last_error: preferences.diagnostics"));
});

test("the popup renders control state and computes none of its own", () => {
  const script = read("popup.js");
  // A policy attention hold and a person's pause both stop work, but the popup must not tell
  // someone they paused Ghostlight when policy did (ADR-0126 Decision 6).
  assert.match(script, /snapshot\.control_state === "attention"/);
  assert.match(script, /snapshot\.control_state === "held"/);
  assert.doesNotMatch(script, /\["held", "attention"\]\.includes/);
  // Control is requested from the orchestrator; the popup never decides the state itself.
  assert.match(script, /kind: "runtime_control"/);
});
