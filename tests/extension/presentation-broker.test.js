// SPDX-License-Identifier: Apache-2.0 OR MIT

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const {
  createPresentationBroker,
} = require("../../extension/lib/presentation-broker.js");

const root = path.join(__dirname, "../..");
const workerSource = fs.readFileSync(path.join(root, "extension/service-worker.js"), "utf8");
const indicatorSource = fs.readFileSync(
  path.join(root, "extension/agent-visual-indicator.js"),
  "utf8"
);

function settle() {
  return new Promise((resolve) => setImmediate(resolve));
}

function ack(envelope, result = { success: true }) {
  return Object.assign({}, result, {
    presentationAck: {
      channel: envelope.presentation.channel,
      revision: envelope.presentation.revision,
      documentId: envelope.presentation.documentId,
    },
  });
}

function harness(overrides = {}) {
  const delivered = [];
  const activated = [];
  const broker = createPresentationBroker({
    deliver: async (tabId, documentId, envelope) => {
      delivered.push({ tabId, documentId, envelope });
      return ack(envelope, envelope.type === "AGENT_NARRATION"
        ? { shown: true, position: "bottom" }
        : { success: true });
    },
    activate: async (tabId) => {
      activated.push(tabId);
      return { ready: true };
    },
    deliveryWaitMs: 100,
    ...overrides,
  });
  return { broker, delivered, activated };
}

test("state waits for an exact ready document and acknowledges its revision", async () => {
  const h = harness();
  const published = h.broker.publishState(
    7,
    "narration",
    { type: "AGENT_NARRATION", text: "Working" },
    { ttlMs: 5000 }
  );
  await settle();
  assert.deepEqual(h.activated, [7]);
  assert.equal(h.delivered.length, 0);

  assert.equal(h.broker.documentReady(7, "doc-a"), true);
  const result = await published.delivery;
  assert.equal(result.shown, true);
  assert.equal(h.delivered.length, 1);
  assert.equal(h.delivered[0].documentId, "doc-a");
  assert.equal(h.delivered[0].envelope.presentation.revision, published.revision);
});

test("replacement retires the old waiter and stale metadata cannot acknowledge new state", async () => {
  let releaseFirst;
  const firstGate = new Promise((resolve) => { releaseFirst = resolve; });
  let calls = 0;
  const h = harness({
    deliver: async (_tabId, _documentId, envelope) => {
      calls += 1;
      if (calls === 1) await firstGate;
      return ack(envelope, { shown: true, position: "top" });
    },
  });
  h.broker.documentReady(4, "doc-a");
  const first = h.broker.publishState(4, "narration", { type: "AGENT_NARRATION", text: "One" });
  await settle();
  const second = h.broker.publishState(4, "narration", { type: "AGENT_NARRATION", text: "Two" });
  const firstResult = await first.delivery;
  assert.match(firstResult.reason, /replaced/);
  releaseFirst();
  const secondResult = await second.delivery;
  assert.equal(secondResult.shown, true);
  assert.equal(second.revision > first.revision, true);
});

test("navigation replays state but retires old-document events", async () => {
  const h = harness();
  h.broker.documentReady(9, "doc-a");
  h.broker.publishState(
    9,
    "control",
    { type: "SHOW_AGENT_INDICATORS" },
    { waitForDelivery: false }
  );
  await settle();
  h.broker.publishState(
    9,
    "attention:g1",
    { type: "AGENT_ATTENTION_REQUIRED", guid: "g1" },
    { waitForDelivery: false }
  );
  h.broker.publishEvent(9, { type: "AGENT_READ_SCAN" }, { waitForDelivery: false });
  await settle();
  assert.deepEqual(h.delivered.map((entry) => entry.envelope.type), [
    "SHOW_AGENT_INDICATORS",
    "AGENT_ATTENTION_REQUIRED",
    "AGENT_READ_SCAN",
  ]);

  h.broker.documentLoading(9);
  h.broker.publishEvent(
    9,
    { type: "AGENT_NAVIGATE_PILL", url: "https://example.org/" },
    { waitForDelivery: false }
  );
  h.broker.documentReady(9, "doc-b");
  await settle();
  assert.deepEqual(h.delivered.slice(3).map((entry) => entry.envelope.type), [
    "AGENT_ATTENTION_REQUIRED",
    "SHOW_AGENT_INDICATORS",
    "AGENT_NAVIGATE_PILL",
  ]);
  assert.equal(h.delivered.slice(3).every((entry) => entry.documentId === "doc-b"), true);
});

test("action signature lifecycle events stay ordered and document-local", async () => {
  const h = harness();
  h.broker.documentReady(14, "doc-a");
  h.broker.publishEvent(
    14,
    { type: "AGENT_ACTION_SIGNATURE", kind: "javascript", phase: "start" },
    { channel: "action-signature", waitForDelivery: false }
  );
  h.broker.publishEvent(
    14,
    { type: "AGENT_ACTION_SIGNATURE", kind: "javascript", phase: "finish" },
    { channel: "action-signature", waitForDelivery: false }
  );
  await settle();
  assert.deepEqual(h.delivered.map((entry) => entry.envelope.phase), ["start", "finish"]);

  h.broker.documentLoading(14);
  h.broker.documentReady(14, "doc-b");
  await settle();
  assert.equal(h.delivered.length, 2);
});

test("a ready message for a new document retires queued effects when loading was missed", async () => {
  const h = harness();
  h.broker.documentReady(3, "doc-a");
  h.broker.documentLoading(3);
  h.broker.documentReady(3, "doc-b");
  h.broker.publishEvent(3, { type: "AGENT_CLICK_RIPPLE" }, { waitForDelivery: false });
  h.broker.documentReady(3, "doc-c");
  await settle();
  assert.equal(h.delivered.length, 1);
  assert.equal(h.delivered[0].documentId, "doc-b");
  assert.equal(h.delivered.some((entry) => entry.documentId === "doc-c"), false);
});

test("expired state is not restored or replayed", async () => {
  let now = 1000;
  const h = harness({ now: () => now });
  h.broker.publishState(
    2,
    "notification",
    { type: "AGENT_NOTIFICATION", title: "Blocked" },
    { deadline: 2000, waitForDelivery: false }
  );
  const saved = h.broker.snapshot();
  now = 2500;
  const restored = harness({ now: () => now });
  assert.equal(restored.broker.restore(saved), true);
  restored.broker.documentReady(2, "doc-b");
  await settle();
  assert.equal(restored.delivered.length, 0);
  assert.equal(restored.broker.stats().states, 0);
});

test("awaited deadlines stay referenced while background deadlines may unref", async () => {
  function timerSeam() {
    const timers = [];
    return {
      timers,
      setTimer: (callback, delayMs) => {
        const timer = {
          callback,
          delayMs,
          unreferenced: false,
          unref() { this.unreferenced = true; },
        };
        timers.push(timer);
        return timer;
      },
      clearTimer: () => {},
    };
  }

  const deliveryTimers = timerSeam();
  const deliveryHarness = harness(deliveryTimers);
  const published = deliveryHarness.broker.publishState(
    12,
    "narration",
    { type: "AGENT_NARRATION", text: "Working" },
    { ttlMs: 5000 }
  );
  assert.deepEqual(
    deliveryTimers.timers.map((timer) => timer.unreferenced),
    [true, false]
  );
  deliveryHarness.broker.destroyTab(12);
  assert.match((await published.delivery).reason, /tab closed/);

  const readyTimers = timerSeam();
  const readyHarness = harness(readyTimers);
  const capture = readyHarness.broker.publishCapture(13, { type: "HIDE_FOR_TOOL_USE" });
  await settle();
  assert.equal(readyTimers.timers.length, 1);
  assert.equal(readyTimers.timers[0].unreferenced, false);
  readyTimers.timers[0].callback();
  assert.equal((await capture).unavailable, true);
});

test("capture bypasses the effect queue and activates a missing renderer before degrading", async () => {
  let releaseEffect;
  const effectGate = new Promise((resolve) => { releaseEffect = resolve; });
  const order = [];
  const h = harness({
    deliver: async (_tabId, _documentId, envelope) => {
      order.push(envelope.presentation.channel);
      if (envelope.presentation.channel === "effect") await effectGate;
      return ack(envelope);
    },
  });
  const missing = await h.broker.publishCapture(5, { type: "HIDE_FOR_TOOL_USE" });
  assert.equal(missing.unavailable, true);
  assert.deepEqual(h.activated, [5]);

  h.broker.documentReady(5, "doc-a");
  h.broker.publishEvent(5, { type: "AGENT_READ_SCAN" }, { waitForDelivery: false });
  await settle();
  const capturePromise = h.broker.publishCapture(5, { type: "HIDE_FOR_TOOL_USE" });
  h.broker.publishEvent(5, { type: "AGENT_CLICK_RIPPLE" }, { waitForDelivery: false });
  await settle();
  assert.deepEqual(order, ["effect"]);
  releaseEffect();
  const capture = await capturePromise;
  assert.equal(capture.success, true);
  await settle();
  assert.deepEqual(order, ["effect", "capture", "effect"]);
});

test("activation failure resolves a waiting caller truthfully while retaining timed state", async () => {
  const h = harness({
    activate: async () => ({ ready: false, reason: "content script unavailable on this page" }),
  });
  const published = h.broker.publishState(
    11,
    "narration",
    { type: "AGENT_NARRATION", text: "Working" },
    { ttlMs: 5000 }
  );
  const result = await published.delivery;
  assert.equal(result.shown, false);
  assert.match(result.reason, /unavailable/);
  assert.equal(h.broker.stats().states, 1);
});

test("event and byte bounds retire optional payloads without growing unbounded", async () => {
  const h = harness({ maxEventsPerTab: 2, maxRetainedBytes: 120 });
  h.broker.publishEvent(1, { type: "A", value: "x".repeat(30) }, { waitForDelivery: false });
  h.broker.publishEvent(1, { type: "B", value: "x".repeat(30) }, { waitForDelivery: false });
  h.broker.publishEvent(1, { type: "C", value: "x".repeat(30) }, { waitForDelivery: false });
  assert.equal(h.broker.stats().events <= 2, true);
  assert.equal(h.broker.stats().bytes <= 120, true);
});

test("destroying a tab erases state, events, and persisted snapshot content", () => {
  const h = harness();
  h.broker.publishState(
    8,
    "attention:g",
    { type: "AGENT_ATTENTION_REQUIRED", guid: "g" },
    { waitForDelivery: false }
  );
  h.broker.publishEvent(8, { type: "AGENT_READ_SCAN" }, { waitForDelivery: false });
  assert.equal(h.broker.destroyTab(8), true);
  assert.deepEqual(h.broker.stats(), { tabs: 0, states: 0, events: 0, bytes: 0 });
  assert.deepEqual(h.broker.snapshot().tabs, []);
});

test("worker derives readiness from Chrome sender metadata and targets the exact document", () => {
  assert.match(workerSource, /msg\.type === "GHOSTLIGHT_PRESENTATION_READY"/);
  assert.match(workerSource, /const tabId = sender && sender\.tab && sender\.tab\.id/);
  assert.match(workerSource, /const documentId = sender && sender\.documentId/);
  assert.match(workerSource, /const accepted = managedTabs\.has\(tabId\)/);
  assert.match(workerSource, /chrome\.tabs\.sendMessage\(tabId, envelope, \{ documentId \}\)/);
  assert.match(workerSource, /frameIds: \[0\]/);
  assert.match(workerSource, /files: VISUAL_SCRIPT_FILES/);
  assert.match(workerSource, /info\.status === "complete" && managedTabs\.has\(tabId\)/);
});

test("renderer announces only after listener installation and echoes exact broker metadata", () => {
  const listenerAt = indicatorSource.indexOf("chrome.runtime.onMessage.addListener");
  const finalReadyAt = indicatorSource.lastIndexOf("announcePresentationReady();");
  assert.equal(listenerAt >= 0, true);
  assert.equal(finalReadyAt > listenerAt, true);
  assert.match(indicatorSource, /channel: metadata\.channel/);
  assert.match(indicatorSource, /revision: metadata\.revision/);
  assert.match(indicatorSource, /documentId: metadata\.documentId/);
});

test("renderer recovery clears stale roots while preserving visual trust invariants", () => {
  assert.match(indicatorSource, /stale = document\.getElementById\(id\)/);
  assert.match(indicatorSource, /narrationText\.textContent = text/);
  assert.match(indicatorSource, /pointer-events:none/);
  assert.match(indicatorSource, /prefers-reduced-motion:reduce/);
  assert.match(indicatorSource, /shown: false, reason: "visual effects are disabled"/);
  assert.match(indicatorSource, /case "AGENT_NOTIFICATION_CLEAR"/);
  assert.match(indicatorSource, /if \(attentionLayer\) return; \/\/ the persistent human-decision surface has visual priority/);
  assert.match(indicatorSource, /function showAttention\(msg\) \{\s+clearAttention\(\);\s+dismissNotification\(\)/);
  assert.match(indicatorSource, /if \(attentionLayer\) attentionLayer\.style\.display = v \? "none" : ""/);
  assert.match(indicatorSource, /attachShadow\(\{ mode: "closed" \}\)/);
  assert.match(indicatorSource, /backdrop-filter:blur\(5px\)/);
  assert.match(indicatorSource, /let controlActive = false/);
  assert.match(indicatorSource, /function showControlBorder\(\)/);
  assert.match(indicatorSource, /ghostlight-control-breathe 4s ease-in-out infinite/);
  assert.match(indicatorSource, /ghostlight-signature-layer/);
  assert.match(indicatorSource, /function effectiveSignaturePosition\(\)/);
  assert.match(indicatorSource, /prefers-reduced-motion:reduce/);
  assert.match(indicatorSource, /if \(signatureLayer\) signatureLayer\.style\.display = v \? "none" : ""/);
  assert.match(indicatorSource, /keyboard\.textContent = "\\u2328\\uFE0F"/);
  assert.doesNotMatch(workerSource, /keystrokeCue\(tabId, a\.text, "type"\)/);
  assert.match(indicatorSource, /visibilitychange/);
  assert.doesNotMatch(indicatorSource, /FADE_MS|fadeTimer|setTimeout\(hideControlBorder/);
  assert.doesNotMatch(indicatorSource, /ghostlight-narration-progress/);
});

test("worker routes visual effects, state, and capture barriers through one broker", () => {
  assert.match(workerSource, /presentationBroker\.publishEvent\(tabId, msg/);
  assert.match(workerSource, /presentationBroker\.publishState\(tabId, "narration"/);
  assert.match(workerSource, /presentationBroker\.publishState\(tabId, `attention:/);
  assert.match(workerSource, /function markTabManaged\(tabId\)/);
  assert.match(workerSource, /presentationBroker\.publishState\(tabId, "control"/);
  assert.match(workerSource, /clearMessage: \{ type: "HIDE_AGENT_INDICATORS" \}/);
  assert.equal((workerSource.match(/managedTabs\.add\(/g) || []).length, 1);
  assert.match(workerSource, /presentationBroker\.publishCapture\(tabId, \{ type: "HIDE_FOR_TOOL_USE" \}\)/);
  assert.match(workerSource, /function startActionSignature\(tabId, kind\)/);
  assert.match(workerSource, /channel: self\.GhostlightActionSignature\.CHANNEL/);
  assert.match(workerSource, /finishActionSignature\(tabId, self\.GhostlightActionSignature\.KINDS\.JAVASCRIPT\)/);
  assert.match(workerSource, /confirmActionSignature\(tabId, self\.GhostlightActionSignature\.KINDS\.SCREENSHOT\)/);
  assert.match(workerSource, /finishActionSignature\(tabId, self\.GhostlightActionSignature\.KINDS\.TYPING\)/);
  assert.match(workerSource, /finishActionSignature\(tabId, self\.GhostlightActionSignature\.KINDS\.WAIT\)/);
  assert.doesNotMatch(workerSource, /const narrationStore/);
  assert.doesNotMatch(workerSource, /const attentionStore/);
});
