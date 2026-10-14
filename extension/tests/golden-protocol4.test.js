"use strict";

// Pins adapter protocol 4 on the extension side. The fixtures in tests/golden/protocol4 are shared
// with the service's cargo tests; lib/protocol.js must equal vocabulary.json and every fixture must
// stay inside that vocabulary.

const test = require("node:test");
const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const protocol = require("../lib/protocol.js");

const GOLDEN = path.join(__dirname, "..", "..", "tests", "golden", "protocol4");
const SECTIONS = ["frames", "events", "requests", "receipts", "world", "tolerance"];

function readJson(relative) {
  return JSON.parse(fs.readFileSync(path.join(GOLDEN, ...relative.split("/")), "utf8"));
}

function section(name) {
  return fs.readdirSync(path.join(GOLDEN, name)).filter((file) => file.endsWith(".json")).sort().map((file) => ({
    file,
    stem: file.slice(0, -".json".length),
    relative: `${name}/${file}`,
    value: readJson(`${name}/${file}`)
  }));
}

const vocabulary = readJson("vocabulary.json");
const known = (list, value) => vocabulary[list].includes(value);
const sha256 = (data) => crypto.createHash("sha256").update(data).digest("hex");

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireKeys(value, keys, label) {
  assert.ok(isObject(value), `${label} must be an object`);
  for (const key of keys) assert.ok(key in value, `${label} is missing ${key}`);
}

function isDeeplyFrozen(value) {
  if (value === null || typeof value !== "object") return true;
  return Object.isFrozen(value) && Object.values(value).every(isDeeplyFrozen);
}

// True when `wide` holds everything `base` holds, with the same values, and possibly more fields.
function contains(wide, base) {
  if (Array.isArray(base)) {
    return Array.isArray(wide) && wide.length === base.length && base.every((item, index) => contains(wide[index], item));
  }
  if (isObject(base)) {
    return isObject(wide) && Object.keys(base).every((key) => key in wide && contains(wide[key], base[key]));
  }
  return wide === base;
}

test("protocol.js equals vocabulary.json and is frozen", () => {
  assert.deepStrictEqual(protocol.vocabulary, vocabulary);
  assert.ok(isDeeplyFrozen(protocol.vocabulary));
  assert.ok(Object.isFrozen(protocol));
});

test("protocol.js scalar constants come from the vocabulary", () => {
  assert.equal(protocol.PROTOCOL, 4);
  assert.equal(protocol.WORLD_NAME, "ghostlight");
  assert.equal(protocol.NATIVE_HOST_NAME, "org.sylin.ghostlight");
  assert.equal(protocol.PLATFORM, "ghostlight/chromium");
  assert.equal(protocol.DIRECT_FRAME_BYTES, vocabulary.limits.direct_frame_bytes);
  assert.equal(protocol.MAX_WORLD_CALLS, vocabulary.limits.max_world_calls);
  assert.equal(protocol.MAX_INPUT_STEPS, vocabulary.limits.max_input_steps);
  assert.equal(protocol.MAX_UNSEEN, vocabulary.limits.max_unseen);
  assert.equal(protocol.MAX_AX_NODES, vocabulary.limits.max_ax_nodes);
  assert.equal(protocol.MAX_PAUSE_MS, vocabulary.limits.max_pause_ms);
  assert.equal(protocol.URL_CHARS, vocabulary.limits.url_chars);
  assert.equal(protocol.TITLE_CHARS, vocabulary.limits.title_chars);
  assert.equal(protocol.HELLO_TIMEOUT_MS, vocabulary.limits.hello_timeout_ms);
});

test("protocol.isKnown answers membership and rejects an unknown list", () => {
  assert.equal(protocol.isKnown("primitives", "navigate"), true);
  assert.equal(protocol.isKnown("primitives", "install_assets"), false);
  assert.throws(() => protocol.isKnown("not_a_list", "x"), TypeError);
});

test("vocabulary lists hold unique names and the numbers match the spec", () => {
  for (const [key, value] of Object.entries(vocabulary)) {
    if (!Array.isArray(value)) continue;
    assert.equal(new Set(value).size, value.length, `${key} repeats a name`);
    assert.ok(value.length > 0, `${key} is empty`);
  }
  assert.equal(vocabulary.primitives.length, 17);
  assert.equal(vocabulary.receipt_failures.length, 13);
  assert.equal(vocabulary.call_failures.length, 10);
  assert.equal(vocabulary.world_message_kinds.length, 32);
  assert.equal(vocabulary.limits.direct_frame_bytes, 768 * 1024);
});

test("pre-effect codes are every asset error code except the three that can follow an effect", () => {
  const afterEffect = new Set(["cancelled", "not_retained", "page_exception"]);
  assert.deepStrictEqual(vocabulary.pre_effect_codes, vocabulary.asset_error_codes.filter((code) => !afterEffect.has(code)));
});

test("every fixture parses, is ASCII, and ends with one newline", () => {
  const files = [path.join(GOLDEN, "vocabulary.json")];
  for (const name of SECTIONS) {
    for (const entry of section(name)) files.push(path.join(GOLDEN, name, entry.file));
  }
  for (const file of files) {
    const text = fs.readFileSync(file, "utf8");
    assert.ok(/^[\x00-\x7f]*$/.test(text), `${file} holds a non-ASCII character`);
    assert.ok(text.endsWith("}\n") && !text.endsWith("\n\n"), `${file} must end with one newline`);
    JSON.parse(text);
  }
});

test("fixture directories hold only json files", () => {
  // assets.sha256 pins the page bundle; the level-4 asset tests own it.
  assert.deepStrictEqual(fs.readdirSync(GOLDEN).sort(), [...SECTIONS, "vocabulary.json", "assets.sha256"].sort());
  for (const name of SECTIONS) {
    for (const file of fs.readdirSync(path.join(GOLDEN, name))) assert.ok(file.endsWith(".json"), `${name}/${file}`);
  }
});

test("frame fixtures cover every frame kind that is not a request, receipt or event", () => {
  const frames = section("frames");
  const directional = [
    ...vocabulary.frame_kinds.extension_to_service,
    ...vocabulary.frame_kinds.service_to_extension,
    ...vocabulary.frame_kinds.relay_to_extension
  ].filter((kind) => !["request", "receipt", "event"].includes(kind));
  assert.deepStrictEqual(frames.map((entry) => entry.stem).sort(), [...directional].sort());
  for (const { stem, value } of frames) assert.equal(value.kind, stem);
});

test("the hello fixture states protocol 4 and the chromium platform", () => {
  const hello = readJson("frames/hello.json");
  assert.equal(hello.protocol, vocabulary.protocol);
  assert.equal(hello.platform, vocabulary.platform);
  assert.match(hello.browser_id, /^browser_[0-9a-z]+$/);
  assert.equal(typeof hello.attended, "boolean");
  assert.match(hello.engine_version, /^[0-9.]{1,32}$/);
});

test("the hello_accepted fixture sha256 matches its source", () => {
  const frame = readJson("frames/hello_accepted.json");
  assert.match(frame.assets.id, /^[0-9a-f]{64}$/);
  assert.equal(sha256(frame.assets.source), frame.assets.id);
  assert.ok(known("control_states", frame.control_state));
  assert.equal(typeof frame.service_epoch, "string");
});

test("the command_chunk fixture reassembles into a request frame", () => {
  const chunk = readJson("frames/command_chunk.json");
  assert.equal(chunk.count, 1);
  assert.equal(chunk.index, 0);
  const bytes = Buffer.from(chunk.data, "base64");
  assert.equal(bytes.length, chunk.total_bytes);
  assert.equal(sha256(bytes), chunk.sha256);
  const request = JSON.parse(bytes.toString("utf8"));
  assert.equal(request.kind, "request");
  assert.equal(request.correlation, chunk.correlation);
  assert.ok(known("primitives", request.primitive.name));
});

test("small frames use known states and sequences", () => {
  assert.ok(known("control_states", readJson("frames/control_state.json").state));
  assert.equal(readJson("frames/heartbeat.json").sequence, readJson("frames/heartbeat_ack.json").sequence);
  assert.equal(typeof readJson("frames/cancel.json").correlation, "string");
  assert.deepStrictEqual(readJson("frames/backend_unavailable.json"), { kind: "backend_unavailable" });
  const notice = readJson("frames/notice.json");
  requireKeys(notice, ["tab_id", "signal", "defer_until_visible"], "notice");
  requireKeys(notice.signal, ["invocation", "signal", "activity", "phase"], "notice.signal");
  requireKeys(readJson("frames/error.json"), ["code", "message"], "error");
});

test("event fixtures cover every event and use known values", () => {
  const events = section("events");
  assert.deepStrictEqual(events.map((entry) => entry.stem).sort(), [...vocabulary.events].sort());
  for (const { stem, value } of events) {
    assert.equal(value.kind, "event");
    assert.equal(value.event.event, stem);
    if (stem === "readiness_changed") assert.ok(known("readiness", value.event.readiness));
    if (stem === "runtime_control_requested") assert.ok(known("runtime_control_intents", value.event.intent));
    if (stem === "document_committed") requireKeys(value.event, ["tab_id", "url"], stem);
    if (stem === "dialog_changed") requireKeys(value.event, ["tab_id", "present", "dialog_type"], stem);
  }
});

const EXPECTED_REQUESTS = [
  "list_tabs", "open_tab", "open_tab.adopt", "navigate.url", "navigate.back", "navigate.forward", "navigate.reload", "focus_tab",
  "close_tab", "set_zoom", "resize_window", "frames", "call.sequence", "call.parallel", "call.race", "call.owner", "evaluate_main",
  "view", "input.mouse", "input.key", "input.insert_text", "input.drag", "input.pause", "input.precondition", "capture.viewport",
  "capture.full_page", "capture.viewport_rect", "capture.page_rect", "dialog.status", "dialog.handle", "diagnostics.read",
  "diagnostics.clear", "recording.start", "recording.status", "recording.stop", "recording.export.download",
  "recording.export.client", "recording.export.page", "recording.discard", "accessibility_snapshot"
];

function checkGuard(guard, label) {
  requireKeys(guard, ["documents", "exact"], label);
  assert.equal(typeof guard.exact, "boolean");
  assert.ok(guard.documents.length >= 1, `${label} lists no document`);
  for (const document of guard.documents) requireKeys(document, ["frame_id", "loader_id", "url"], `${label} document`);
}

function checkFrameRef(ref, label) {
  requireKeys(ref, ["frame_id", "loader_id"], label);
  assert.match(ref.frame_id, /^[0-9A-F]{32}$/);
  assert.match(ref.loader_id, /^[0-9A-F]{32}$/);
}

const worldFixtures = new Map(section("world").map((entry) => [entry.stem, entry.value]));

// A world call carries a message that world.rs builds; the fixture message must be the world fixture.
function checkWorldCall(call, label) {
  requireKeys(call, ["message"], label);
  assert.ok(known("world_message_kinds", call.message.kind), `${label}: unknown world kind ${call.message.kind}`);
  assert.deepStrictEqual(call.message, worldFixtures.get(call.message.kind), `${label}: message differs from world/${call.message.kind}.json`);
  if (call.frame) checkFrameRef(call.frame, `${label}.frame`);
  if (call.owner_of !== undefined) assert.equal(typeof call.owner_of, "string");
}

function checkInputStep(step, label) {
  assert.ok(known("input_steps", step.step), `${label}: unknown step ${step.step}`);
  if (step.step === "mouse") {
    assert.ok(known("mouse_actions", step.action));
    assert.ok(known("mouse_buttons", step.button));
    requireKeys(step, ["x", "y", "buttons", "click_count", "modifiers", "delta_x", "delta_y", "force"], label);
  }
  if (step.step === "key") {
    assert.ok(known("key_actions", step.action));
    requireKeys(step, ["key", "modifiers"], label);
  }
  if (step.step === "insert_text") assert.equal(typeof step.text, "string");
  if (step.step === "drag") {
    assert.ok(step.path.length >= 2);
    for (const point of step.path) requireKeys(point, ["x", "y"], `${label} point`);
  }
  if (step.step === "pause") assert.ok(step.ms >= 0 && step.ms <= vocabulary.limits.max_pause_ms, `${label}: pause too long`);
}

const ARGUMENT_CHECKS = {
  list_tabs(args) {
    if (args.tab_ids !== undefined) assert.ok(args.tab_ids.every(Number.isInteger));
  },
  open_tab(args) {
    requireKeys(args, ["group_key", "group_title", "focus_window", "ready_timeout_ms"], "open_tab");
    assert.ok(args.url !== undefined || args.adopt_tab_id !== undefined, "open_tab needs a url unless it adopts");
    assert.ok(!args.url || args.url.length <= vocabulary.limits.url_chars);
  },
  navigate(args) {
    requireKeys(args, ["tab_id", "to", "discard_beforeunload", "ready_timeout_ms"], "navigate");
    assert.ok(known("navigation_targets", args.to.to));
    if (args.to.to === "url") assert.ok(args.to.url.length <= vocabulary.limits.url_chars);
  },
  focus_tab(args) { requireKeys(args, ["tab_id"], "focus_tab"); },
  close_tab(args) { requireKeys(args, ["tab_id", "released"], "close_tab"); },
  set_zoom(args) { requireKeys(args, ["tab_id", "zoom"], "set_zoom"); },
  resize_window(args) {
    requireKeys(args, ["tab_id", "width", "height"], "resize_window");
    const { resize_width: w, resize_height: h } = vocabulary.limits;
    assert.ok(args.width >= w.min && args.width <= w.max && args.height >= h.min && args.height <= h.max);
  },
  frames(args) { requireKeys(args, ["tab_id"], "frames"); },
  call(args) {
    requireKeys(args, ["tab_id", "mode", "calls", "settle_ms", "timeout_ms"], "call");
    assert.ok(known("call_modes", args.mode));
    assert.ok(args.calls.length >= 1 && args.calls.length <= vocabulary.limits.max_world_calls);
    args.calls.forEach((call, index) => checkWorldCall(call, `calls[${index}]`));
    if (args.guard) checkGuard(args.guard, "call.guard");
  },
  evaluate_main(args) {
    requireKeys(args, ["tab_id", "script", "max_result_chars", "settle_ms", "timeout_ms"], "evaluate_main");
    if (args.guard) checkGuard(args.guard, "evaluate_main.guard");
  },
  view(args) { requireKeys(args, ["tab_id"], "view"); },
  input(args) {
    requireKeys(args, ["tab_id", "steps", "settle_ms", "timeout_ms"], "input");
    assert.ok(args.steps.length >= 1 && args.steps.length <= vocabulary.limits.max_input_steps);
    args.steps.forEach((step, index) => checkInputStep(step, `steps[${index}]`));
    if (args.precondition) checkWorldCall(args.precondition, "precondition");
    if (args.guard) checkGuard(args.guard, "input.guard");
  },
  capture(args) {
    requireKeys(args, ["tab_id", "clip", "budget"], "capture");
    assert.ok(known("clips", args.clip.clip));
    requireKeys(args.budget, ["max_side", "max_pixels", "min_scale", "magnify", "qualities", "max_base64_chars"], "budget");
    if (args.clip.clip === "viewport_rect") requireKeys(args.clip, ["left", "top", "width", "height"], "clip");
    if (args.clip.clip === "page_rect") requireKeys(args.clip, ["x", "y", "width", "height"], "clip");
  },
  dialog(args) {
    requireKeys(args, ["tab_id", "op"], "dialog");
    assert.ok(known("dialog_ops", args.op.op));
    if (args.op.op === "handle") assert.equal(typeof args.op.accept, "boolean");
  },
  diagnostics(args) {
    assert.ok(known("diagnostics_ops", args.op));
    if (args.op === "read") {
      requireKeys(args, ["tab_id", "source", "detail", "limit"], "diagnostics.read");
      assert.ok(known("diagnostic_sources", args.source));
      assert.ok(known("diagnostic_details", args.detail));
      assert.ok(args.limit >= vocabulary.limits.diagnostics_limit.min && args.limit <= vocabulary.limits.diagnostics_limit.max);
      if (args.guard) checkGuard(args.guard, "diagnostics.guard");
    } else {
      assert.ok(Array.isArray(args.tab_ids));
    }
  },
  recording(args) {
    assert.ok(known("recording_ops", args.op));
    if (args.op === "start") {
      requireKeys(args, ["tab_id"], "recording.start");
      if (args.watch) for (const document of args.watch) requireKeys(document, ["frame_id", "loader_id", "url"], "watch");
    } else {
      requireKeys(args, ["recording_id"], `recording.${args.op}`);
    }
    if (args.op === "export") {
      requireKeys(args, ["destination", "max_output_bytes"], "recording.export");
      assert.ok(known("recording_destinations", args.destination.destination));
      if (args.destination.destination === "download") assert.equal(typeof args.destination.file_name, "string");
      if (args.destination.destination === "page") {
        requireKeys(args.destination, ["tab_id", "call"], "page destination");
        checkWorldCall(args.destination.call, "page destination call");
        if (args.destination.guard) checkGuard(args.destination.guard, "page destination guard");
      }
    }
  },
  accessibility_snapshot(args) {
    requireKeys(args, ["tab_id", "frame", "max_nodes"], "accessibility_snapshot");
    checkFrameRef(args.frame, "frame");
    assert.ok(args.max_nodes <= vocabulary.limits.max_ax_nodes);
    if (args.guard) checkGuard(args.guard, "accessibility_snapshot.guard");
  }
};

test("request fixtures are exactly the specified variants", () => {
  assert.deepStrictEqual(section("requests").map((entry) => entry.stem).sort(), [...EXPECTED_REQUESTS].sort());
});

test("request fixtures use known primitives and valid arguments, and cover every primitive", () => {
  const seen = new Set();
  for (const { stem, value } of section("requests")) {
    assert.equal(value.kind, "request", stem);
    assert.equal(typeof value.correlation, "string", stem);
    assert.ok(known("primitives", value.primitive.name), `${stem}: unknown primitive ${value.primitive.name}`);
    assert.equal(value.primitive.name, stem.split(".")[0], `${stem}: file name does not lead with the primitive`);
    ARGUMENT_CHECKS[value.primitive.name](value.primitive.args, stem);
    seen.add(value.primitive.name);
  }
  assert.deepStrictEqual([...seen].sort(), [...vocabulary.primitives].sort());
  assert.deepStrictEqual(Object.keys(ARGUMENT_CHECKS).sort(), [...vocabulary.primitives].sort());
});

test("request variants exercise every closed choice they are named for", () => {
  const byStem = new Map(section("requests").map((entry) => [entry.stem, entry.value.primitive.args]));
  assert.deepStrictEqual(
    ["url", "back", "forward", "reload"].map((to) => byStem.get(`navigate.${to}`).to.to), vocabulary.navigation_targets);
  assert.deepStrictEqual(["sequence", "parallel", "race"].map((mode) => byStem.get(`call.${mode}`).mode), vocabulary.call_modes);
  assert.ok(byStem.get("call.owner").calls.every((call) => typeof call.owner_of === "string"));
  assert.ok(byStem.get("open_tab.adopt").adopt_tab_id !== undefined);
  assert.deepStrictEqual(
    ["viewport", "full_page", "viewport_rect", "page_rect"].map((clip) => byStem.get(`capture.${clip}`).clip.clip), vocabulary.clips);
  const steps = new Set();
  for (const variant of ["mouse", "key", "insert_text", "drag", "pause", "precondition"]) {
    for (const step of byStem.get(`input.${variant}`).steps) steps.add(step.step);
  }
  assert.deepStrictEqual([...steps].sort(), [...vocabulary.input_steps].sort());
  assert.ok(byStem.get("input.precondition").precondition);
  const actions = (field) => new Set(byStem.get("input.mouse").steps.map((step) => step[field]));
  assert.deepStrictEqual([...actions("action")].sort(), [...vocabulary.mouse_actions].sort());
  assert.deepStrictEqual([...new Set(byStem.get("input.key").steps.map((step) => step.action))].sort(), [...vocabulary.key_actions].sort());
});

function checkTab(tab, label) {
  requireKeys(tab, ["tab_id", "title", "url", "active", "readiness"], label);
  assert.ok(known("readiness", tab.readiness), `${label}: unknown readiness ${tab.readiness}`);
  assert.ok(tab.title.length <= vocabulary.limits.title_chars);
  assert.ok(tab.url.length <= vocabulary.limits.url_chars);
}

function checkAnswer(answer, label) {
  assert.ok(known("call_answers", answer.status), `${label}: unknown call answer ${answer.status}`);
  if (answer.status === "returned") {
    requireKeys(answer, ["envelope"], label);
    assert.equal(typeof answer.envelope.ok, "boolean");
    if (answer.envelope.ok === false) {
      assert.ok(known("asset_error_codes", answer.envelope.error.code), `${label}: unknown asset error code`);
    }
  }
  if (answer.status === "failed") {
    requireKeys(answer, ["code", "message", "started"], label);
    assert.ok(known("call_failures", answer.code), `${label}: unknown call failure ${answer.code}`);
  }
}

const OK_CHECKS = {
  list_tabs(ok) { ok.tabs.forEach((tab, index) => checkTab(tab, `tabs[${index}]`)); },
  open_tab(ok) { checkTab(ok.tab, "tab"); assert.equal(typeof ok.adopted, "boolean"); },
  navigate(ok) { checkTab(ok.tab, "tab"); },
  focus_tab(ok) { requireKeys(ok, ["tab_id", "active", "window_focused"], "focus_tab"); },
  close_tab(ok) { requireKeys(ok, ["tab_id"], "close_tab"); },
  set_zoom(ok) { requireKeys(ok, ["tab_id", "zoom"], "set_zoom"); },
  resize_window(ok) { requireKeys(ok, ["tab_id", "width", "height", "affected_tab_ids"], "resize_window"); },
  frames(ok) {
    requireKeys(ok, ["frames", "complete"], "frames");
    for (const frame of ok.frames) requireKeys(frame, ["frame_id", "loader_id", "url"], "frame");
    assert.ok((ok.unseen ?? []).length <= vocabulary.limits.max_unseen);
  },
  call(ok) { ok.answers.forEach((answer, index) => checkAnswer(answer, `answers[${index}]`)); },
  evaluate_main(ok) {
    assert.ok("value" in ok || "exception" in ok);
  },
  view(ok) {
    requireKeys(ok, ["visual_page_x", "visual_page_y", "visual_css_width", "visual_css_height", "content_css_width",
      "content_css_height", "device_scale", "zoom"], "view");
  },
  input(ok) {
    requireKeys(ok, ["completed_steps", "native_drag"], "input");
    if (ok.precondition) checkAnswer(ok.precondition, "precondition");
    if (ok.stopped) assert.ok(["dialog", "cancelled"].includes(ok.stopped.reason));
  },
  capture(ok) {
    requireKeys(ok, ["mime_type", "data", "width", "height", "output_scale", "clip", "view"], "capture");
    requireKeys(ok.clip, ["x", "y", "width", "height"], "capture.clip");
  },
  dialog(ok) { requireKeys(ok, ["present", "dialog_type"], "dialog"); },
  diagnostics(ok) {
    for (const entry of ok.entries ?? []) {
      assert.ok(["console", "network"].includes(entry.entry));
      requireKeys(entry, ["cursor", "timestamp_ms", "url"], "entry");
    }
  },
  recording(ok) {
    assert.ok(["started", "status", "stopped", "exported", "export_failed", "discarded", "not_found"].includes(ok.result), ok.result);
    if (["started", "status", "stopped", "exported"].includes(ok.result)) {
      requireKeys(ok.summary, ["recording_id", "tab_id", "state", "frame_count", "bytes_held", "duration_ms", "source_urls"], "summary");
    }
    if (ok.result === "exported") {
      requireKeys(ok.encoded, ["frame_count", "captured_frame_count", "duration_ms", "width", "height", "byte_count"], "encoded");
      assert.ok(["attached", "downloaded", "returned"].includes(ok.delivery.delivery));
    }
  },
  accessibility_snapshot(ok) {
    requireKeys(ok, ["nodes", "truncated"], "accessibility_snapshot");
    assert.ok(ok.nodes.length <= vocabulary.limits.max_ax_nodes);
    for (const node of ok.nodes) requireKeys(node, ["node_id", "role"], "ax node");
  }
};

/** Effect-class ok receipts (spec section 5): the Effect primitives, a dialog handle and a recording export into a page. */
const EFFECT_PRIMITIVES = new Set([
  "open_tab", "navigate", "focus_tab", "close_tab", "set_zoom", "resize_window", "evaluate_main", "input", "call"
]);

function isEffectOk(name, outcome, ok) {
  if (EFFECT_PRIMITIVES.has(name)) return true;
  if (name === "dialog") return outcome === "ok_handled";
  if (name === "recording") return ok.delivery?.delivery === "attached";
  return false;
}

test("receipt fixtures name a primitive and one outcome, and use known codes", () => {
  const covered = new Set();
  const okPrimitives = new Set();
  for (const { stem, value } of section("receipts")) {
    const [name, ...rest] = stem.split(".");
    const outcome = rest.join(".");
    assert.ok(outcome.length > 0, `${stem}: receipt files are named <primitive>.<case>`);
    assert.equal(value.kind, "receipt", stem);
    assert.equal(typeof value.correlation, "string", stem);
    assert.equal(typeof value.dispatched, "boolean", stem);
    assert.ok(Array.isArray(value.committed_urls), stem);
    assert.ok(value.committed_urls.every((url) => typeof url === "string" && url.length <= vocabulary.limits.url_chars), stem);
    if (value.tab !== undefined) {
      checkTab(value.tab, `${stem}.tab`);
      assert.equal(value.dispatched, true, `${stem}: a tab is read only after dispatch`);
    }
    const hasOk = "ok" in value;
    const hasFailure = "failure" in value;
    assert.notEqual(hasOk, hasFailure, `${stem}: a receipt has exactly one of ok and failure`);
    if (hasOk) {
      assert.ok(name !== "unknown_primitive" && known("primitives", name), `${stem}: unknown primitive`);
      assert.ok(outcome === "ok" || outcome.startsWith("ok_"), `${stem}: an ok receipt is named <primitive>.ok[_case]`);
      OK_CHECKS[name](value.ok);
      if (!isEffectOk(name, outcome, value.ok)) {
        assert.equal(value.dispatched, false, `${stem}: a Read receipt is never dispatched`);
        assert.equal(value.tab, undefined, `${stem}: a Read receipt carries no tab`);
      }
      okPrimitives.add(name);
    } else {
      assert.ok(known("receipt_failures", value.failure), `${stem}: unknown failure ${value.failure}`);
      assert.equal(outcome, value.failure, `${stem}: the case names the failure code`);
      assert.ok(name === "unknown_primitive" || known("primitives", name), `${stem}: unknown primitive`);
      if (value.failure === "unsupported" || value.failure === "invalid_request" || value.failure === "tab_missing") {
        assert.equal(value.dispatched, false, `${stem}: ${value.failure} is never dispatched`);
      }
      covered.add(value.failure);
      assert.ok(value.message === undefined || value.message.length <= 500, stem);
    }
  }
  assert.deepStrictEqual([...okPrimitives].sort(), [...vocabulary.primitives].sort(), "every primitive has an ok receipt");
  assert.deepStrictEqual([...covered].sort(), [...vocabulary.receipt_failures].sort(), "every failure code has a receipt");
  assert.deepStrictEqual(Object.keys(OK_CHECKS).sort(), [...vocabulary.primitives].sort());
});

test("the unknown primitive receipt is the unsupported answer", () => {
  const receipt = readJson("receipts/unknown_primitive.unsupported.json");
  assert.equal(receipt.failure, "unsupported");
  assert.equal(receipt.dispatched, false);
  assert.deepStrictEqual(receipt.committed_urls, []);
});

test("call receipts exercise every call answer status and call failure code", () => {
  const statuses = new Set();
  const codes = new Set();
  for (const { value } of section("receipts")) {
    for (const answer of value.ok?.answers ?? []) {
      statuses.add(answer.status);
      if (answer.status === "failed") codes.add(answer.code);
    }
  }
  assert.deepStrictEqual([...statuses].sort(), [...vocabulary.call_answers].sort());
  assert.ok(codes.size >= 2 && [...codes].every((code) => known("call_failures", code)));
});

test("world fixtures cover every message kind, each built with its own kind", () => {
  assert.deepStrictEqual([...worldFixtures.keys()].sort(), [...vocabulary.world_message_kinds].sort());
  for (const [stem, message] of worldFixtures) {
    assert.equal(message.kind, stem);
    assert.ok(Object.keys(message).every((key) => /^[a-z][a-z_]*$/.test(key)), `${stem}: odd field name`);
  }
  const settled = ["read_text", "inspect", "inspect_tree", "find", "query_semantic", "focus_text_field"];
  for (const kind of settled) requireKeys(worldFixtures.get(kind), ["settle_ms", "visual_settle"], kind);
  requireKeys(worldFixtures.get("verify_fill_retained"), ["stable_ms", "poll_ms", "limit_ms"], "verify_fill_retained");
  assert.equal(worldFixtures.get("verify_fill_retained").stable_ms, 250);
  assert.equal(worldFixtures.get("verify_fill_retained").poll_ms, 25);
  requireKeys(worldFixtures.get("observe"), ["condition", "timeout_ms", "until_satisfied"], "observe");
  requireKeys(worldFixtures.get("hit_point"), ["x", "y", "page"], "hit_point");
});

test("tolerance fixtures state how each side reads them", () => {
  const entries = section("tolerance");
  assert.ok(entries.length >= 10);
  const forms = new Set();
  for (const { stem, value } of entries) {
    requireKeys(value, ["description", "frame", "expect"], stem);
    const named = Object.keys(value.expect);
    assert.equal(named.length, 1, `${stem}: expect names exactly one form`);
    const [form] = named;
    forms.add(form);
    assert.ok(["decodes_as", "ignored", "maps_to", "answered"].includes(form), `${stem}: unknown expectation ${form}`);
    if (form === "decodes_as") {
      const base = readJson(value.expect.decodes_as);
      assert.ok(contains(value.frame, base), `${stem}: the frame must hold everything in ${value.expect.decodes_as}`);
      assert.ok(!contains(base, value.frame), `${stem}: the frame must add fields to ${value.expect.decodes_as}`);
    }
    if (form === "ignored") {
      assert.equal(value.expect.ignored, true);
      const frame = value.frame;
      const unknownKind = !["extension_to_service", "service_to_extension", "relay_to_extension"]
        .some((direction) => knownInDirection(direction, frame.kind));
      const unknownEvent = frame.kind === "event" && !known("events", frame.event.event);
      const unknownIntent = frame.kind === "event" && frame.event.event === "runtime_control_requested"
        && !known("runtime_control_intents", frame.event.intent);
      assert.ok(unknownKind || unknownEvent || unknownIntent, `${stem}: an ignored frame must hold an unknown name`);
    }
    if (form === "answered") {
      const answer = readJson(value.expect.answered);
      assert.equal(answer.correlation, value.frame.correlation);
      assert.ok(!known("primitives", value.frame.primitive.name));
    }
    if (form === "maps_to") assert.ok(["other", "unknown"].includes(value.expect.maps_to), stem);
  }
  assert.deepStrictEqual([...forms].sort(), ["answered", "decodes_as", "ignored", "maps_to"]);
});

function knownInDirection(direction, kind) {
  return vocabulary.frame_kinds[direction].includes(kind);
}

test("tolerance fixtures with unknown values hold exactly the unknown value they name", () => {
  const byStem = new Map(section("tolerance").map((entry) => [entry.stem, entry.value]));
  assert.ok(!known("receipt_failures", byStem.get("receipt.unknown_failure_code").frame.failure));
  assert.ok(!known("call_answers", byStem.get("call.unknown_answer_status").frame.ok.answers[0].status));
  assert.ok(!known("readiness", byStem.get("tab.unknown_readiness").frame.ok.tab.readiness));
  assert.ok(!["dialog", "cancelled"].includes(byStem.get("input.unknown_stop_reason").frame.ok.stopped.reason));
});
