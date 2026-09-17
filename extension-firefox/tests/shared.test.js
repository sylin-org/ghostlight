"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const shared = require("../lib/shared.js");

test("Firefox adapter protocol constants match bridge expectations", () => {
  assert.equal(shared.ADAPTER_PROTOCOL_MAJOR, 2);
  assert.equal(shared.BROWSER_PLATFORM, "ghostlight/gecko");
  assert.equal(shared.BROWSER_NAME, "Firefox");
  assert.equal(shared.NATIVE_HOST_NAME, "org.sylin.ghostlight");
});

test("Firefox adapter advertises closed capabilities with correct revisions", () => {
  const capNames = shared.ADAPTER_CAPABILITIES.map((c) => c.name);
  assert.ok(capNames.includes("tabs"));
  assert.ok(capNames.includes("atomic_tab_open"));
  assert.ok(capNames.includes("navigation"));
  assert.ok(capNames.includes("capture"));
  assert.ok(capNames.includes("script"));
  assert.ok(capNames.includes("window_geometry"));
  assert.ok(capNames.includes("presentation"));
  assert.ok(capNames.includes("adapter_liveness"));
  assert.ok(capNames.includes("adapter_attention"));
  assert.ok(capNames.includes("document_scope"));

  const navCap = shared.ADAPTER_CAPABILITIES.find((c) => c.name === "navigation");
  assert.equal(navCap.revision, 2);
});

test("heartbeat acknowledgement echoes sequence number", () => {
  assert.deepEqual(shared.heartbeatAcknowledgement({ kind: "heartbeat", sequence: 42 }), {
    kind: "heartbeat_ack",
    sequence: 42
  });
  assert.equal(shared.heartbeatAcknowledgement({ kind: "other", sequence: 42 }), null);
  assert.equal(shared.heartbeatAcknowledgement({ kind: "heartbeat", sequence: -1 }), null);
});

test("linkState distinguishes connected, absent, and unreachable", () => {
  assert.equal(shared.linkState({ connected: true, compatible: true }), "connected");
  assert.equal(
    shared.linkState({ connected: false, compatible: true, lastError: "No such native application org.sylin.ghostlight" }),
    "host_absent"
  );
  assert.equal(
    shared.linkState({ connected: false, compatible: true, lastError: "Connection refused" }),
    "unreachable"
  );
});

test("Firefox extension opens the service-first handoff on install", () => {
  const fs = require("node:fs");
  const path = require("node:path");
  const source = fs.readFileSync(path.join(__dirname, "..", "background.js"), "utf8");
  assert.match(
    source,
    /const SERVICE_INSTALL_URL = "https:\/\/sylin\.org\/ghostlight\/service\/post-install\/\?browser=firefox";/
  );
  assert.match(
    source,
    /if \(details\?\.reason === "install"\) \{\s+browserApi\.tabs\.create\(\{ url: SERVICE_INSTALL_URL \}\)/
  );
});

test("Firefox popup and setup pages use Firefox-specific copy and post-install route", () => {
  const fs = require("node:fs");
  const path = require("node:path");
  const popupHtml = fs.readFileSync(path.join(__dirname, "..", "popup.html"), "utf8");
  const popupJs = fs.readFileSync(path.join(__dirname, "..", "popup.js"), "utf8");
  const setupHtml = fs.readFileSync(path.join(__dirname, "..", "setup.html"), "utf8");
  const optionsJs = fs.readFileSync(path.join(__dirname, "..", "options.js"), "utf8");

  assert.ok(!popupHtml.includes("chrome://extensions/shortcuts"), "popup.html must not reference chrome:// shortcuts");
  assert.ok(popupHtml.includes("about:addons"), "popup.html must reference about:addons");
  assert.ok(!popupHtml.includes("release-debugger-button"), "popup.html must not include debugger button");
  assert.ok(!popupHtml.includes("Chrome profile"), "popup.html must not reference Chrome profile");
  assert.ok(popupHtml.includes("Firefox profile"), "popup.html must reference Firefox profile");

  assert.ok(popupJs.includes("https://sylin.org/ghostlight/service/post-install/?browser=firefox"));
  assert.ok(!popupJs.includes("chromium-extension/post-install"));
  assert.ok(popupJs.includes("Controlling "));
  assert.ok(!popupJs.includes("Debugger attached to"));

  assert.ok(!setupHtml.includes("Chrome"), "setup.html must not reference Chrome");
  assert.ok(setupHtml.includes("Firefox"), "setup.html must reference Firefox");
  assert.ok(setupHtml.includes("https://sylin.org/ghostlight/service/post-install/?browser=firefox"));

  assert.ok(optionsJs.includes("https://sylin.org/ghostlight/service/post-install/?browser=firefox"));
  assert.ok(!optionsJs.includes("Chrome profile"), "options.js must not reference Chrome profile");
});
