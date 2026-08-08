// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- policy-free adapter for the typed browser-mechanism request wire.
//
// The service owns operation meaning, policy, and client profiles. This module knows only the
// closed physical mechanism vocabulary and how each canonical input reaches the existing Chrome
// handler. Covered legacy request frames pass through unchanged during the adapter skew window.
(function initMechanismWire(root) {
"use strict";

const MECHANISM_REQUEST_V1 = "mechanismRequestV1";
const MECHANISM_REQUEST = "mechanism_request";
const TOOL_REQUEST = "tool_request";
const TAB_URL_REQUEST = "tab_url_request";

const MECHANISM_IDS = Object.freeze([
  "workspace.tabs.inspect",
  "workspace.tabs.ensure",
  "workspace.tab.create",
  "tab.focus",
  "tab.close",
  "navigate.url",
  "navigate.back",
  "navigate.forward",
  "navigate.reload",
  "page.snapshot",
  "page.read_text",
  "page.find",
  "screenshot.viewport",
  "screenshot.region",
  "element.resolve",
  "target.cue",
  "pointer.click",
  "pointer.hover",
  "pointer.drag",
  "text.type",
  "key.press",
  "wheel.scroll",
  "scroll.target_into_view",
  "scroll.viewport_to_offset",
  "form.inspect",
  "form.set_value",
  "wait.delay",
  "wait.until",
  "dialog.inspect",
  "dialog.accept",
  "dialog.dismiss",
  "dialog.respond",
  "viewport.resize",
  "upload.files",
  "upload.image",
  "console.read",
  "network.read",
  "page.evaluate",
  "recording.start",
  "recording.stop",
  "points.rescale",
  "narration.show",
  "tab.url_query",
]);
const MECHANISM_ID_SET = new Set(MECHANISM_IDS);

// These spellings belong only to the covered pre-mechanism request grammar. Rejecting them keeps
// canonical input and compatibility input from becoming two authorities inside the new frame.
const LEGACY_INPUT_FIELDS = Object.freeze([
  "action",
  "tabId",
  "createIfEmpty",
  "ref",
  "ref_id",
  "coordinate",
  "start_coordinate",
  "scroll_direction",
  "scroll_amount",
  "onlyErrors",
  "urlPattern",
  "imageId",
  "mimeType",
  "recordingId",
  "maxSide",
  "minIntervalMs",
  "leaseMs",
  "hardTimeoutMs",
]);

function own(object, key) {
  return Object.prototype.hasOwnProperty.call(object, key);
}

function fail(message) {
  throw new Error(message);
}

function requireInput(message) {
  if (!message.input || typeof message.input !== "object" || Array.isArray(message.input)) {
    fail(`mechanism ${String(message.mechanism)} input must be an object`);
  }
  for (const field of LEGACY_INPUT_FIELDS) {
    if (own(message.input, field)) {
      fail(`mechanism ${message.mechanism} input uses legacy field ${field}`);
    }
  }
  if (Array.isArray(message.input.files) &&
      message.input.files.some((file) => file && typeof file === "object" && own(file, "mimeType"))) {
    fail(`mechanism ${message.mechanism} input uses legacy nested field mimeType`);
  }
  return { ...message.input };
}

function rename(args, canonical, legacy) {
  if (!own(args, canonical)) return;
  args[legacy] = args[canonical];
  delete args[canonical];
}

function action(args, name, tool) {
  args.action = name;
  return tool;
}

function fixedField(args, key, value, tool) {
  args[key] = value;
  return tool;
}

function flattenTargetReference(args) {
  if (!own(args, "target")) return;
  const target = args.target;
  if (!target || typeof target !== "object" || Array.isArray(target) || !own(target, "ref")) {
    fail("physical target requires target.ref");
  }
  args.ref = target.ref;
  delete args.target;
}

function renameFileMediaTypes(args) {
  if (!own(args, "files")) return;
  if (!Array.isArray(args.files)) fail("upload.files files must be an array");
  args.files = args.files.map((file) => {
    if (!file || typeof file !== "object" || Array.isArray(file)) {
      fail("upload.files entries must be objects");
    }
    const normalized = { ...file };
    rename(normalized, "mime_type", "mimeType");
    return normalized;
  });
}

function mapTargetCue(args) {
  const cueKind = args.cue_kind;
  if (typeof cueKind !== "string") fail("target.cue requires cue_kind");
  let legacyAction;
  switch (cueKind) {
    case "click": legacyAction = "left_click"; break;
    case "right_click": legacyAction = "right_click"; break;
    case "double_click": legacyAction = "double_click"; break;
    case "triple_click": legacyAction = "triple_click"; break;
    case "hover": legacyAction = "hover"; break;
    case "scroll_into_view": legacyAction = "scroll_to"; break;
    case "set_value": legacyAction = "set_value"; break;
    default: fail(`unknown target.cue kind: ${cueKind}`);
  }
  if (!Array.isArray(args.point) || args.point.length !== 2) {
    fail("target.cue requires a two-item point");
  }
  const point = args.point;
  delete args.cue_kind;
  delete args.point;
  args.x = point[0];
  args.y = point[1];
  return action(args, legacyAction, "target_cue_internal");
}

function mapPointerClick(args) {
  const button = args.button;
  const count = args.count;
  if (typeof button !== "string") fail("pointer.click requires button");
  if (!Number.isSafeInteger(count) || count < 0) fail("pointer.click requires count");
  const legacyAction = button === "left" && count === 1 ? "left_click"
    : button === "right" && count === 1 ? "right_click"
      : button === "left" && count === 2 ? "double_click"
        : button === "left" && count === 3 ? "triple_click"
          : null;
  if (!legacyAction) fail(`unsupported pointer.click button/count pair: ${button}/${count}`);
  delete args.button;
  delete args.count;
  rename(args, "point", "coordinate");
  flattenTargetReference(args);
  return action(args, legacyAction, "computer");
}

function mechanismTool(mechanism, args) {
  rename(args, "tab", "tabId");
  switch (mechanism) {
    case "workspace.tabs.inspect":
      rename(args, "create_if_empty", "createIfEmpty");
      return "tabs_context_mcp";
    case "workspace.tabs.ensure":
      delete args.create_if_empty;
      args.createIfEmpty = true;
      return "tabs_context_mcp";
    case "workspace.tab.create": return "tabs_create_mcp";
    case "tab.focus": return action(args, "focus", "tab_control");
    case "tab.close": return action(args, "close", "tab_control");
    case "navigate.url": return "navigate";
    case "navigate.back": return fixedField(args, "url", "back", "navigate");
    case "navigate.forward": return fixedField(args, "url", "forward", "navigate");
    case "navigate.reload": return action(args, "reload", "tab_control");
    case "page.snapshot":
      rename(args, "scope_ref", "ref_id");
      return "read_page";
    case "page.read_text": return "get_page_text";
    case "page.find": return "find";
    case "screenshot.viewport": return action(args, "screenshot", "computer");
    case "screenshot.region": return action(args, "zoom", "computer");
    case "element.resolve": return "resolve_actionable_internal";
    case "target.cue": return mapTargetCue(args);
    case "pointer.click": return mapPointerClick(args);
    case "pointer.hover":
      rename(args, "point", "coordinate");
      flattenTargetReference(args);
      return action(args, "hover", "computer");
    case "pointer.drag":
      rename(args, "from", "start_coordinate");
      rename(args, "to", "coordinate");
      return action(args, "left_click_drag", "computer");
    case "text.type": return action(args, "type", "computer");
    case "key.press":
      rename(args, "key", "text");
      return action(args, "key", "computer");
    case "wheel.scroll":
      rename(args, "point", "coordinate");
      rename(args, "direction", "scroll_direction");
      rename(args, "amount", "scroll_amount");
      flattenTargetReference(args);
      return action(args, "scroll", "computer");
    case "scroll.target_into_view":
      flattenTargetReference(args);
      return action(args, "scroll_to", "computer");
    case "scroll.viewport_to_offset":
      rename(args, "point", "coordinate");
      return action(args, "scroll_to", "computer");
    case "form.inspect": return "form_structure_internal";
    case "form.set_value":
      flattenTargetReference(args);
      return "form_input";
    case "wait.delay":
      rename(args, "seconds", "duration");
      return action(args, "wait", "computer");
    case "wait.until": return "wait_for";
    case "dialog.inspect": return action(args, "status", "dialog");
    case "dialog.accept": return action(args, "accept", "dialog");
    case "dialog.dismiss": return action(args, "dismiss", "dialog");
    case "dialog.respond": return action(args, "respond", "dialog");
    case "viewport.resize": return "resize_window";
    case "upload.files":
      flattenTargetReference(args);
      renameFileMediaTypes(args);
      return "file_upload";
    case "upload.image":
      flattenTargetReference(args);
      rename(args, "point", "coordinate");
      rename(args, "mime_type", "mimeType");
      return "upload_image_exec";
    case "console.read":
      rename(args, "only_errors", "onlyErrors");
      return "read_console_messages";
    case "network.read":
      rename(args, "url_pattern", "urlPattern");
      return "read_network_requests";
    case "page.evaluate":
      rename(args, "script", "text");
      return action(args, "javascript_exec", "javascript_tool");
    case "recording.start":
      rename(args, "recording_id", "recordingId");
      rename(args, "max_side", "maxSide");
      rename(args, "min_interval_ms", "minIntervalMs");
      rename(args, "lease_ms", "leaseMs");
      rename(args, "hard_timeout_ms", "hardTimeoutMs");
      return "gif_capture_start";
    case "recording.stop":
      rename(args, "recording_id", "recordingId");
      return "gif_capture_stop";
    case "points.rescale": return "rescale_coords";
    case "narration.show": return "narrate";
    case "tab.url_query": fail("tab.url_query uses the auxiliary request path");
    default: fail(`unknown browser mechanism: ${mechanism}`);
  }
}

function normalizedEnvelope(message, type) {
  const normalized = { ...message, type };
  delete normalized.mechanism;
  delete normalized.input;
  return normalized;
}

function translateMechanismRequest(message) {
  if (!message || typeof message !== "object" || Array.isArray(message)) {
    fail("mechanism request must be an object");
  }
  if ((typeof message.id !== "string" && typeof message.id !== "number") ||
      String(message.id).length === 0) {
    fail("mechanism request requires an id");
  }
  if (typeof message.mechanism !== "string" || !MECHANISM_ID_SET.has(message.mechanism)) {
    fail(`unknown browser mechanism: ${String(message.mechanism)}`);
  }
  const args = requireInput(message);
  if (message.mechanism === "tab.url_query") {
    if (!Number.isSafeInteger(args.tab)) fail("tab.url_query requires a numeric tab");
    const normalized = normalizedEnvelope(message, TAB_URL_REQUEST);
    normalized.tabId = args.tab;
    return normalized;
  }
  const normalized = normalizedEnvelope(message, TOOL_REQUEST);
  normalized.tool = mechanismTool(message.mechanism, args);
  normalized.args = args;
  return normalized;
}

function normalizeIncomingRequest(message) {
  if (!message || typeof message !== "object" || Array.isArray(message)) return null;
  if (message.type === TOOL_REQUEST || message.type === TAB_URL_REQUEST) return message;
  if (message.type === MECHANISM_REQUEST) return translateMechanismRequest(message);
  return null;
}

const GhostlightMechanismWire = Object.freeze({
  MECHANISM_IDS,
  MECHANISM_REQUEST_V1,
  normalizeIncomingRequest,
  translateMechanismRequest,
});
if (typeof module !== "undefined" && module.exports) {
  module.exports = GhostlightMechanismWire;
} else {
  root.GhostlightMechanismWire = GhostlightMechanismWire;
}
})(typeof self !== "undefined" ? self : globalThis);
