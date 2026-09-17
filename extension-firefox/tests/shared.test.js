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
