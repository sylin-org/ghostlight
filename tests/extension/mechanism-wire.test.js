// SPDX-License-Identifier: Apache-2.0 OR MIT
// R4 contract tests for the negotiated typed browser-mechanism request wire.

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const {
  MECHANISM_IDS,
  MECHANISM_REQUEST_V1,
  NAVIGATION_READINESS_V1,
  ATOMIC_TAB_OPEN_V1,
  STRICT_SENSITIVE_WRITES_V1,
  normalizeIncomingRequest,
  translateMechanismRequest,
} = require("../../extension/lib/mechanism-wire.js");
const {
  createResponseScope,
  createToolResponder,
} = require("../../extension/lib/execution-response.js");

const EXPECTED_TOOLS = {
  "workspace.tabs.inspect": "tabs_context_mcp",
  "workspace.tabs.ensure": "tabs_context_mcp",
  "workspace.tab.create": "tabs_create_mcp",
  "workspace.tab.open": "tabs_open_mcp",
  "tab.focus": "tab_control",
  "tab.close": "tab_control",
  "navigate.url": "navigate_readiness_start",
  "navigate.back": "navigate_readiness_start",
  "navigate.forward": "navigate_readiness_start",
  "navigate.reload": "navigate_readiness_start",
  "navigation.await_readiness": "navigation_readiness_await",
  "navigation.verify_document": "navigation_readiness_verify",
  "page.snapshot": "read_page",
  "page.read_text": "get_page_text",
  "page.find": "find",
  "screenshot.viewport": "computer",
  "screenshot.region": "computer",
  "element.resolve": "resolve_actionable_internal",
  "target.cue": "target_cue_internal",
  "pointer.click": "computer",
  "pointer.hover": "computer",
  "pointer.drag": "computer",
  "text.type": "computer",
  "key.press": "computer",
  "wheel.scroll": "computer",
  "scroll.target_into_view": "computer",
  "scroll.viewport_to_offset": "computer",
  "form.inspect": "form_structure_internal",
  "form.set_value": "form_input",
  "wait.delay": "computer",
  "wait.until": "wait_for",
  "dialog.inspect": "dialog",
  "dialog.accept": "dialog",
  "dialog.dismiss": "dialog",
  "dialog.respond": "dialog",
  "viewport.resize": "resize_window",
  "upload.files": "file_upload",
  "upload.image": "upload_image_exec",
  "console.read": "read_console_messages",
  "network.read": "read_network_requests",
  "page.evaluate": "javascript_tool",
  "recording.start": "gif_capture_start",
  "recording.stop": "gif_capture_stop",
  "points.rescale": "rescale_coords",
  "narration.show": "narrate",
};

function frame(mechanism, input = {}) {
  return {
    id: "request-1",
    type: "mechanism_request",
    mechanism,
    input,
    guid: "workspace-1",
    resultFeatures: ["tabDeltaV1"],
    execution: { class: "scheduled", commandId: "command-1" },
  };
}

test("the closed adapter vocabulary exhaustively covers all 46 mechanism ids", () => {
  assert.equal(MECHANISM_REQUEST_V1, "mechanismRequestV1");
  assert.equal(NAVIGATION_READINESS_V1, "navigationReadinessV1");
  assert.equal(ATOMIC_TAB_OPEN_V1, "atomicTabOpenV1");
  assert.equal(STRICT_SENSITIVE_WRITES_V1, "strictSensitiveWritesV1");
  assert.equal(MECHANISM_IDS.length, 46);
  assert.equal(Object.keys(EXPECTED_TOOLS).length, 45);

  for (const mechanism of MECHANISM_IDS) {
    const input = mechanism === "target.cue"
      ? { cue_kind: "click", point: [1, 2] }
      : mechanism === "pointer.click"
        ? { button: "left", count: 1 }
        : mechanism === "tab.url_query"
          ? { tab: 4 }
          : mechanism.startsWith("navigate.")
            ? { readiness: {} }
            : {};
    const normalized = translateMechanismRequest(frame(mechanism, input));
    if (mechanism === "tab.url_query") {
      assert.equal(normalized.type, "tab_url_request", mechanism);
      assert.equal(normalized.tabId, 4, mechanism);
      assert.equal(normalized.tool, undefined, mechanism);
    } else {
      assert.equal(normalized.type, "tool_request", mechanism);
      assert.equal(normalized.tool, EXPECTED_TOOLS[mechanism], mechanism);
      assert.equal(typeof normalized.args, "object", mechanism);
    }
  }
});

test("typed envelopes preserve execution and additive metadata while normalizing canonical input", () => {
  const original = {
    id: "7",
    type: "mechanism_request",
    mechanism: "navigate.url",
    input: {
      tab: 9,
      url: "https://example.com/",
      readiness: { settle: true, timeout_ms: 10000, min_ms: 0 },
      future_input: { mode: "additive" },
    },
    guid: "workspace-7",
    resultFeatures: ["tabDeltaV1", "futureResultV1"],
    execution: { class: "scheduled", commandId: "command-7" },
    workspace: { groupTitle: "Ghostlight - Example" },
    futureEnvelope: { retained: true },
  };
  const normalized = normalizeIncomingRequest(original);

  assert.deepEqual(normalized, {
    id: "7",
    type: "tool_request",
    guid: "workspace-7",
    resultFeatures: ["tabDeltaV1", "futureResultV1"],
    execution: { class: "scheduled", commandId: "command-7" },
    workspace: { groupTitle: "Ghostlight - Example" },
    futureEnvelope: { retained: true },
    tool: "navigate_readiness_start",
    args: {
      url: "https://example.com/",
      readiness: { settle: true, timeout_ms: 10000, min_ms: 0 },
      future_input: { mode: "additive" },
      tabId: 9,
    },
  });
  assert.equal(original.type, "mechanism_request");
  assert.deepEqual(original.input, {
    tab: 9,
    url: "https://example.com/",
    readiness: { settle: true, timeout_ms: 10000, min_ms: 0 },
    future_input: { mode: "additive" },
  });
});

test("typed navigation without explicit readiness preserves the R4 adapter path", () => {
  for (const [mechanism, input, tool, expectedArgs] of [
    ["navigate.url", { tab: 1, url: "https://example.com/" }, "navigate", {
      tabId: 1,
      url: "https://example.com/",
    }],
    ["navigate.back", { tab: 1 }, "navigate", { tabId: 1, url: "back" }],
    ["navigate.forward", { tab: 1 }, "navigate", { tabId: 1, url: "forward" }],
    ["navigate.reload", { tab: 1 }, "tab_control", { tabId: 1, action: "reload" }],
  ]) {
    const normalized = translateMechanismRequest(frame(mechanism, input));
    assert.equal(normalized.tool, tool, mechanism);
    assert.deepEqual(normalized.args, expectedArgs, mechanism);
  }
});

test("legacy tool_request remains unchanged through the exported dual reader", () => {
  const legacy = {
    id: "legacy-1",
    type: "tool_request",
    tool: "computer",
    args: { action: "hover", tabId: 5, coordinate: [10, 20] },
    guid: "workspace-1",
    resultFeatures: ["tabDeltaV1"],
    execution: { class: "scheduled" },
  };
  assert.strictEqual(normalizeIncomingRequest(legacy), legacy);
});

test("legacy tab_url_request remains unchanged through the exported dual reader", () => {
  const legacy = {
    id: "legacy-url-1",
    type: "tab_url_request",
    tabId: 5,
    execution: { class: "instrumentation" },
  };
  assert.strictEqual(normalizeIncomingRequest(legacy), legacy);
  assert.equal(normalizeIncomingRequest({ id: "other", type: "hold_state" }), null);
});

test("typed tab URL queries select the auxiliary grammar and retain correlation metadata", () => {
  const normalized = normalizeIncomingRequest({
    id: "url-1",
    type: "mechanism_request",
    mechanism: "tab.url_query",
    input: { tab: 42 },
    execution: { class: "instrumentation", commandId: "url-command" },
    futureEnvelope: true,
  });
  assert.deepEqual(normalized, {
    id: "url-1",
    type: "tab_url_request",
    execution: { class: "instrumentation", commandId: "url-command" },
    futureEnvelope: true,
    tabId: 42,
  });
});

test("action-bearing mechanisms select the exact covered handler action", () => {
  const cases = [
    ["tab.focus", {}, "focus"],
    ["tab.close", {}, "close"],
    ["navigate.reload", { readiness: {} }, "reload"],
    ["screenshot.viewport", {}, "screenshot"],
    ["screenshot.region", {}, "zoom"],
    ["pointer.hover", {}, "hover"],
    ["pointer.drag", {}, "left_click_drag"],
    ["text.type", {}, "type"],
    ["key.press", {}, "key"],
    ["wheel.scroll", {}, "scroll"],
    ["scroll.target_into_view", {}, "scroll_to"],
    ["scroll.viewport_to_offset", {}, "scroll_to"],
    ["wait.delay", {}, "wait"],
    ["dialog.inspect", {}, "status"],
    ["dialog.accept", {}, "accept"],
    ["dialog.dismiss", {}, "dismiss"],
    ["dialog.respond", {}, "respond"],
    ["page.evaluate", {}, "javascript_exec"],
  ];
  for (const [mechanism, input, expected] of cases) {
    assert.equal(translateMechanismRequest(frame(mechanism, input)).args.action, expected, mechanism);
  }
});

test("pointer click metadata chooses only the supported physical action", () => {
  for (const [button, count, expected] of [
    ["left", 1, "left_click"],
    ["right", 1, "right_click"],
    ["left", 2, "double_click"],
    ["left", 3, "triple_click"],
  ]) {
    const normalized = translateMechanismRequest(frame("pointer.click", {
      tab: 3,
      point: [10, 20],
      button,
      count,
    }));
    assert.deepEqual(normalized.args, {
      tabId: 3,
      coordinate: [10, 20],
      action: expected,
    });
  }
});

test("canonical field families retain exact presence while reaching legacy handlers", () => {
  const cases = [
    ["workspace.tabs.inspect", {}, {}],
    ["workspace.tabs.inspect", { create_if_empty: false }, { createIfEmpty: false }],
    ["workspace.tabs.ensure", {}, { createIfEmpty: true }],
    ["workspace.tabs.ensure", { create_if_empty: false }, { createIfEmpty: true }],
    ["navigate.back", { tab: 1 }, { tabId: 1, url: "back" }],
    ["navigation.await_readiness", {
      tab: 1,
      navigation_token: "n_1",
      document_handle: "d_1",
    }, {
      tabId: 1,
      navigation_token: "n_1",
      document_handle: "d_1",
    }],
    ["navigation.verify_document", {
      tab: 1,
      navigation_token: "n_1",
      document_handle: "d_1",
    }, {
      tabId: 1,
      navigation_token: "n_1",
      document_handle: "d_1",
    }],
    ["page.snapshot", { tab: 1, scope_ref: "ref_1" }, { tabId: 1, ref_id: "ref_1" }],
    ["pointer.drag", { tab: 1, from: [1, 2], to: [3, 4] }, {
      tabId: 1,
      start_coordinate: [1, 2],
      coordinate: [3, 4],
      action: "left_click_drag",
    }],
    ["key.press", { tab: 1, key: "Enter", repeat: 2 }, {
      tabId: 1,
      text: "Enter",
      repeat: 2,
      action: "key",
    }],
    ["wheel.scroll", { tab: 1, target: { ref: "ref_2" }, direction: "up", amount: 1 }, {
      tabId: 1,
      scroll_direction: "up",
      scroll_amount: 1,
      ref: "ref_2",
      action: "scroll",
    }],
    ["form.set_value", { tab: 1, target: { ref: "ref_3" }, value: "x" }, {
      tabId: 1,
      value: "x",
      ref: "ref_3",
    }],
    ["form.set_value", {
      tab: 1,
      target: { ref: "ref_4" },
      value: "x",
      reject_sensitive: true,
      expected_type: "text",
    }, {
      tabId: 1,
      value: "x",
      reject_sensitive: true,
      expected_type: "text",
      ref: "ref_4",
    }],
    ["wait.delay", { tab: 1, seconds: 2 }, { tabId: 1, duration: 2, action: "wait" }],
    ["upload.image", { tab: 1, point: [7, 8], mime_type: "image/png" }, {
      tabId: 1,
      coordinate: [7, 8],
      mimeType: "image/png",
    }],
    ["console.read", {}, {}],
    ["console.read", { clear: true, only_errors: true }, { clear: true, onlyErrors: true }],
    ["network.read", {}, {}],
    ["network.read", { clear: true, url_pattern: "api" }, { clear: true, urlPattern: "api" }],
    ["page.evaluate", { tab: 1, script: "return 1" }, {
      tabId: 1,
      text: "return 1",
      action: "javascript_exec",
    }],
    ["recording.start", {
      tab: 1,
      recording_id: "rec-1",
      generation: 2,
      max_side: 1568,
      min_interval_ms: 200,
      lease_ms: 15000,
      hard_timeout_ms: 120000,
    }, {
      tabId: 1,
      recordingId: "rec-1",
      generation: 2,
      maxSide: 1568,
      minIntervalMs: 200,
      leaseMs: 15000,
      hardTimeoutMs: 120000,
    }],
    ["recording.stop", { tab: 1, recording_id: "rec-1", generation: 2 }, {
      tabId: 1,
      recordingId: "rec-1",
      generation: 2,
    }],
  ];
  for (const [mechanism, input, expected] of cases) {
    assert.deepEqual(translateMechanismRequest(frame(mechanism, input)).args, expected, mechanism);
  }
});

test("target cues and file media types normalize without mutating canonical input", () => {
  for (const [cueKind, action] of [
    ["click", "left_click"],
    ["right_click", "right_click"],
    ["double_click", "double_click"],
    ["triple_click", "triple_click"],
    ["hover", "hover"],
    ["scroll_into_view", "scroll_to"],
    ["set_value", "set_value"],
  ]) {
    const cue = translateMechanismRequest(frame("target.cue", {
      tab: 7,
      cue_kind: cueKind,
      point: [12, 34],
    }));
    assert.deepEqual(cue.args, { tabId: 7, x: 12, y: 34, action });
  }

  const input = {
    tab: 8,
    target: { ref: "ref_2" },
    files: [{ name: "a.txt", mime_type: "text/plain", data: "YQ==" }],
  };
  const upload = translateMechanismRequest(frame("upload.files", input));
  assert.deepEqual(upload.args, {
    tabId: 8,
    ref: "ref_2",
    files: [{ name: "a.txt", mimeType: "text/plain", data: "YQ==" }],
  });
  assert.equal(input.files[0].mime_type, "text/plain");
  assert.equal(input.files[0].mimeType, undefined);
});

test("unknown ids and malformed canonical inputs fail closed", () => {
  const cases = [
    frame("invented.mechanism", {}),
    { ...frame("navigate.url", {}), input: null },
    { ...frame("navigate.url", {}), input: [] },
    frame("navigate.url", { tabId: 1 }),
    frame("pointer.hover", { action: "hover", point: [1, 2] }),
    frame("pointer.hover", { coordinate: [1, 2] }),
    frame("form.set_value", { target: {} }),
    frame("target.cue", { cue_kind: "invented", point: [1, 2] }),
    frame("target.cue", { cue_kind: "toString", point: [1, 2] }),
    frame("target.cue", { cue_kind: "click", point: [1] }),
    frame("pointer.click", { button: "middle", count: 1 }),
    frame("upload.files", { files: null }),
    frame("upload.files", { files: ["not-an-object"] }),
    frame("upload.files", { files: [{ mimeType: "text/plain" }] }),
    frame("upload.image", { mimeType: "image/png" }),
    frame("tab.url_query", { tab: "4" }),
  ];
  for (const candidate of cases) {
    assert.throws(() => normalizeIncomingRequest(candidate), Error);
  }
});

test("a rejected typed request is representable only as the existing tool_error reply", () => {
  let error;
  try {
    normalizeIncomingRequest(frame("unknown.mechanism", {}));
  } catch (caught) {
    error = caught;
  }
  const posted = [];
  createToolResponder("executor-1").fail(
    createResponseScope("request-1", { postMessage: (message) => posted.push(message) }),
    error
  );
  assert.deepEqual(posted, [{
    id: "request-1",
    type: "tool_error",
    error: "unknown browser mechanism: unknown.mechanism",
  }]);
});

test("service worker advertises runtime adapter identity and normalizes before dispatch", () => {
  const worker = fs.readFileSync(
    path.join(__dirname, "../../extension/service-worker.js"),
    "utf8"
  );
  assert.match(worker, /"lib\/mechanism-wire\.js"/);
  assert.match(worker, /adapterVersion: chrome\.runtime\.getManifest\(\)\.version/);
  assert.match(worker, /self\.GhostlightMechanismWire\.MECHANISM_REQUEST_V1/);
  assert.match(worker, /self\.GhostlightMechanismWire\.NAVIGATION_READINESS_V1/);
  assert.match(worker, /self\.GhostlightMechanismWire\.ATOMIC_TAB_OPEN_V1/);
  assert.match(worker, /self\.GhostlightMechanismWire\.STRICT_SENSITIVE_WRITES_V1/);
  const normalize = worker.indexOf("normalizeIncomingRequest(msg)");
  const dispatch = worker.indexOf('if (msg && msg.type === "tool_request"');
  const tabUrl = worker.indexOf('if (msg && msg.type === "tab_url_request"');
  assert.ok(normalize >= 0 && normalize < dispatch && dispatch < tabUrl);
  assert.match(worker.slice(normalize, dispatch), /\bfail\(/);
});

test("the shipped adapter declares its matching service compatibility block", () => {
  const manifest = JSON.parse(fs.readFileSync(
    path.join(__dirname, "../../extension/manifest.json"),
    "utf8"
  ));
  const compatibility = JSON.parse(fs.readFileSync(
    path.join(__dirname, "../../compatibility.json"),
    "utf8"
  ));
  assert.equal(manifest.version, "0.9.0");
  assert.deepEqual(
    compatibility.chromeAdapters.find((entry) => entry.adapterVersion === manifest.version),
    { adapterVersion: "0.9.0", serviceVersionBlock: "0.9" }
  );
});
