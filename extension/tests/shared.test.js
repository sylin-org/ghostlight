"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { createHash } = require("node:crypto");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const shared = require("../lib/shared.js");
const protocol = require("../lib/protocol.js");
const state = require("../lib/state.js");
const presentationQueue = require("../lib/presentation-queue.js");

test("activity labels are fixed and content-free", () => {
  assert.equal(shared.activityLabel("read"), "Reading page");
  assert.equal(shared.activityLabel("script"), "Running JavaScript");
  assert.equal(shared.activityLabel("page secret"), "Ghostlight");
});

test("bounded text is cut to the limit and never throws on a missing value", () => {
  assert.equal(shared.bounded("abcdef", 3), "abc");
  assert.equal(shared.bounded(undefined, 3), "");
  assert.equal(shared.bounded(null, 3), "");
});

test("tab status maps to the closed readiness vocabulary", () => {
  assert.equal(shared.readinessForStatus("loading"), "loading");
  assert.equal(shared.readinessForStatus("complete"), "complete");
  assert.equal(shared.readinessForStatus("unloaded"), "unknown");
  assert.equal(shared.readinessForStatus(undefined), "unknown");
});

test("browser events use the nested typed bridge envelope", () => {
  assert.deepEqual(shared.browserEventFrame({ event: "tab_closed", tab_id: 7 }), {
    kind: "event",
    event: { event: "tab_closed", tab_id: 7 }
  });
});


test("adapter liveness acknowledgements echo only bounded heartbeat sequences", () => {
  assert.deepEqual(shared.heartbeatAcknowledgement({ kind: "heartbeat", sequence: 7 }), {
    kind: "heartbeat_ack",
    sequence: 7
  });
  assert.equal(shared.heartbeatAcknowledgement({ kind: "request", sequence: 7 }), null);
  assert.equal(shared.heartbeatAcknowledgement({ kind: "heartbeat", sequence: 0 }), null);
  assert.equal(shared.heartbeatAcknowledgement({ kind: "heartbeat", sequence: 2 ** 32 }), null);
});


test("browser names come from the specific brand, never a generic or placeholder one", () => {
  assert.equal(shared.browserName([
    { brand: "Not_A Brand", version: "8" },
    { brand: "Chromium", version: "140" },
    { brand: "Microsoft Edge", version: "140" }
  ]), "Microsoft Edge");
  assert.equal(shared.browserName([
    { brand: "Chromium", version: "140" },
    { brand: "Google Chrome", version: "140" }
  ]), "Google Chrome");
  // An adapter that cannot name itself says nothing rather than guessing.
  assert.equal(shared.browserName([{ brand: "Chromium", version: "140" }]), null);
  assert.equal(shared.browserName(undefined), null);
  assert.equal(shared.browserName([{ brand: "x".repeat(80) }]).length, 40);
});


test("the unpacked rewrite preserves the established extension and host identity", () => {
  const manifest = JSON.parse(readFileSync(join(__dirname, "..", "manifest.json"), "utf8"));
  const digest = createHash("sha256").update(Buffer.from(manifest.key, "base64")).digest().subarray(0, 16);
  const alphabet = "abcdefghijklmnop";
  const id = Array.from(digest).map((byte) => `${alphabet[byte >> 4]}${alphabet[byte & 15]}`).join("");
  assert.equal(id, "cjcmhepmagomefjggkcohdbfemacojoa");
  assert.equal(protocol.NATIVE_HOST_NAME, "org.sylin.ghostlight");
});


test("the manifest declares the complete local product surface", () => {
  const root = join(__dirname, "..");
  const manifest = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));
  assert.equal(manifest.name, "Ghostlight in Browser");
  assert.equal(manifest.description, "Governed browser automation over your own authenticated session, for AI agents.");
  assert.equal(manifest.minimum_chrome_version, "125");
  assert.equal(manifest.version, "2.0.1");
  assert.equal(manifest.background.service_worker, "worker.js");
  assert.equal(manifest.action.default_title, "Ghostlight in Browser");
  assert.equal(manifest.action.default_popup, "popup.html");
  assert.equal(manifest.options_ui.page, "options.html");
  assert.equal(manifest.commands["toggle-hold"].suggested_key.default, "Alt+Shift+P");
  assert.equal(manifest.commands["toggle-hold"].description, "Pause or resume agent browsing (take the wheel)");
  assert.deepEqual(
    manifest.permissions,
    ["alarms", "debugger", "downloads", "nativeMessaging", "offscreen", "storage", "tabGroups", "tabs", "webNavigation", "windows"]
  );
  const iconDigests = {
    16: "95d754348d4fabfb0412e32319226dd52615864ae511b8b492bef739f555d224",
    32: "645b2b436975da68006fcf5bf89242f55f2988e468cd6278679cffb31a3b2dc8",
    48: "9cdf8201880b2aec05f22d1dbb68822187dbe39f25d425b57960a6affca064de",
    128: "153e65ae92af61a7cd2dcbe38c59e6875287a9e3f0208fb4e73f781292327a67"
  };
  for (const size of [16, 32, 48, 128]) {
    const relative = manifest.icons[String(size)];
    assert.equal(relative, `icons/icon${size}.png`);
    const bytes = readFileSync(join(root, relative));
    assert.equal(bytes.subarray(1, 4).toString("ascii"), "PNG");
    assert.equal(bytes.readUInt32BE(16), size);
    assert.equal(bytes.readUInt32BE(20), size);
    assert.equal(createHash("sha256").update(bytes).digest("hex"), iconDigests[size]);
  }
});


test("adapter state is content-free and deterministic", () => {
  assert.equal(state.PRESENTATIONS_KEY, "ghostlight.presentations");
  assert.equal(state.newBrowserId(() => "1234-5678"), "browser_12345678");
  assert.deepEqual(state.preferences({ effects: false, captions: 0, diagnostics: true }), {
    effects: false,
    captions: false,
    diagnostics: true,
    preserveTabs: true
  });
  assert.deepEqual(state.preferencesFromStorage({}), state.DEFAULT_PREFERENCES);
  assert.equal(state.preferences({ preserveTabs: false }).preserveTabs, false);
  assert.deepEqual(state.preferencesForStorage({ effects: false, captions: true, diagnostics: true }), {
    ghostlight_effects: false,
    ghostlight_captions: true,
    ghostlight_debug: true,
    ghostlight_preserve_tabs: true
  });
  assert.deepEqual(state.badge({ connected: true, compatible: true, control_state: "held" }), { text: "II", color: "#5eead4" });
});


test("unseen denial delivery is bounded, coalesced, and expires", () => {
  let now = 1000;
  const queue = presentationQueue.create({ limit: 2, ttlMs: 100, now: () => now });
  const signal = (tabId, phase) => ({
    invocation: `invocation_${tabId}`,
    signal: "denial",
    activity: "quiet",
    phase,
    detail: "A configured guardrail prevented it.",
    tab_id: tabId
  });
  assert.equal(queue.defer(1, signal(1, "one")), true);
  now += 1;
  assert.equal(queue.defer(1, signal(1, "newest")), true);
  assert.equal(queue.size(), 1);
  assert.equal(queue.get(1).signal.phase, "newest");
  now += 1;
  queue.defer(2, signal(2, "two"));
  now += 1;
  queue.defer(3, signal(3, "three"));
  assert.deepEqual(queue.snapshot().map((entry) => entry.tabId), [2, 3]);
  now += 100;
  assert.equal(queue.size(), 0);
});

test("the refusal code the worker recognizes is the one the service sends", () => {
  const wire = readFileSync(join(__dirname, "..", "..", "crates", "orchestrator", "src", "browser", "adapter", "chromium", "protocol4", "wire.rs"), "utf8");
  const sent = /pub const ADAPTER_INCOMPATIBLE: &str = "([a-z_]+)";/.exec(wire);
  assert.ok(sent, "wire.rs declares the adapter_incompatible error code");
  assert.equal(shared.ADAPTER_INCOMPATIBLE, sent[1]);
  const golden = JSON.parse(readFileSync(join(__dirname, "..", "..", "tests", "golden", "protocol4", "frames", "error.json"), "utf8"));
  assert.equal(golden.code, shared.ADAPTER_INCOMPATIBLE);
});
