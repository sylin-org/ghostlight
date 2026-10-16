"use strict";

// Pins the protocol-4 page bundle: how it installs, how it is disposed, what it may not touch, and
// the hash that is its identity. The bundle runs in a vm context with a fake DOM.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const bundle = require("./bundle.js");
const { createFakeDom, plain } = require("./fake-dom.js");

const REPO = path.join(__dirname, "..", "..", "..", "..", "..", "..", "..", "..", "..");
const GOLDEN = path.join(REPO, "tests", "golden", "protocol4");
const vocabulary = JSON.parse(fs.readFileSync(path.join(GOLDEN, "vocabulary.json"), "utf8"));

const SOURCE = bundle.expand();
const ID = bundle.sha256(SOURCE);
const OTHER_ID = bundle.sha256(`${SOURCE}\n// a different asset set`);

const LOCAL = Object.freeze({
  preferences: { effects: true, captions: false },
  managed: true,
  recording: false,
  runtime: "active",
  hidden: false
});

function install(dom, id = ID, source = SOURCE) {
  dom.run(bundle.registration(id, source), "world.js");
  return dom.context.__ghostlightRuntime;
}

function hostOf(dom) {
  return dom.document.documentElement.childNodes.find((node) => node.id === "ghostlight-presentation-root");
}

function sources() {
  return fs.readdirSync(bundle.ASSETS).filter((name) => name.endsWith(".js")).sort();
}

test("the include marker expands to the listed files in order, each followed by a newline", () => {
  assert.deepEqual(bundle.includes(), ["page-shared.js", "sensor.js", "presentation-css.js", "presentation.js", "content.js"]);
  const marker = "/*@include page-shared.js sensor.js presentation-css.js presentation.js content.js*/";
  const world = bundle.read("world.js");
  assert.equal(world.split(marker).length, 2, "world.js holds the marker exactly once");
  const [before, after] = world.split(marker);
  const body = bundle.includes().map((name) => `${bundle.read(name)}\n`).join("");
  assert.equal(SOURCE, `${before}${body}${after}`);
  assert.ok(!SOURCE.includes("@include"));
  assert.ok(!SOURCE.includes("\r"), "the expanded source holds LF line endings only");
});

test("the bundle hash equals the pinned asset id", () => {
  const pinned = fs.readFileSync(path.join(GOLDEN, "assets.sha256"), "utf8").trim();
  assert.match(pinned, /^[0-9a-f]{64}$/);
  assert.equal(ID, pinned, "an asset changed: update tests/golden/protocol4/assets.sha256 deliberately");
});

test("CODES equals the asset error codes of the vocabulary", () => {
  const match = /const CODES = new Set\(\[([^\]]*)\]\);/.exec(bundle.read("world.js"));
  assert.ok(match, "world.js defines CODES as a set literal");
  const codes = [...match[1].matchAll(/"([a-z_]+)"/g)].map((entry) => entry[1]);
  assert.deepEqual(codes, vocabulary.asset_error_codes);
});

const FORBIDDEN = [
  ["chrome.", /(?<![\w$])chrome\??\.[A-Za-z_$]/],
  ["browser.", /(?<![\w$])browser\??\.[A-Za-z_$]/],
  ["fetch(", /(?<![\w$])fetch\(/],
  ["XMLHttpRequest", /XMLHttpRequest/],
  ["WebSocket", /WebSocket/],
  ["sendMessage", /sendMessage/],
  ["__ghostlight_dispatch__", /__ghostlight_dispatch__/],
  ["innerHTML assignment", /\.innerHTML\s*=/],
  ["insertAdjacentHTML", /insertAdjacentHTML/],
  ["outerHTML", /outerHTML/],
  ["a global Ghostlight namespace", /Ghostlight(Shared|Sensor|Presentation|PresentationCss|FormDiagnostics)/],
  ["form diagnostics", /formDiagnostics/i]
];

function violations(text) {
  return FORBIDDEN.filter(([, pattern]) => pattern.test(text)).map(([label]) => label);
}

test("the forbidden-API scan catches qualified and optional-chained forms", () => {
  assert.deepEqual(violations("globalThis.fetch(url)"), ["fetch("]);
  assert.deepEqual(violations("window.fetch(url)"), ["fetch("]);
  assert.deepEqual(violations("self.fetch(url)"), ["fetch("]);
  assert.deepEqual(violations("window.chrome.runtime.connect()"), ["chrome."]);
  assert.deepEqual(violations("chrome?.runtime"), ["chrome."]);
  assert.deepEqual(violations("globalThis.browser.runtime"), ["browser."]);
  assert.deepEqual(violations("browser?.runtime"), ["browser."]);
  assert.deepEqual(violations("the browser. Then"), []);
});

test("no protocol-4 source touches a browser API or the retired global", () => {
  for (const name of sources()) {
    assert.deepEqual(violations(bundle.read(name)), [], `${name} must not contain a forbidden API`);
  }
});

test("installing defines one frozen runtime object and no other global", () => {
  const dom = createFakeDom();
  const before = new Set(dom.globals());
  const runtime = install(dom);
  const added = dom.globals().filter((name) => !before.has(name));
  assert.deepEqual(added, ["__ghostlightRuntime"]);
  assert.equal(runtime.id, ID);
  assert.ok(Object.isFrozen(runtime));
  assert.deepEqual(Object.keys(runtime).sort(), ["ambient", "call", "cancel", "dispose", "id", "present"]);
});

test("a second install of the same id is a no-op", () => {
  const dom = createFakeDom();
  const first = install(dom);
  const second = install(dom);
  assert.equal(second, first);
});

test("an install with a different id disposes the previous runtime", async () => {
  const dom = createFakeDom();
  const first = install(dom);
  await first.call({ kind: "scroll_offset" }, { local: LOCAL });
  assert.ok(hostOf(dom), "the first runtime mounted its presentation host");
  const second = install(dom, OTHER_ID);
  assert.notEqual(second, first);
  assert.equal(second.id, OTHER_ID);
  assert.equal(dom.context.__ghostlightRuntime, second);
  assert.equal(hostOf(dom), undefined, "the first runtime removed its host");
});

test("call returns an envelope and never rejects", async () => {
  const dom = createFakeDom();
  const runtime = install(dom);

  assert.deepEqual(plain(await runtime.call({ kind: "scroll_offset" }, {})), { ok: true, value: { x: 0, y: 0 } });
  assert.deepEqual(plain(await runtime.call({ kind: "scroll_offset" })), { ok: true, value: { x: 0, y: 0 } });

  for (const message of [null, undefined, {}, { kind: 7 }]) {
    const reply = plain(await runtime.call(message, {}));
    assert.equal(reply.ok, false);
    assert.equal(reply.error.code, "invalid_request");
  }

  const unknown = plain(await runtime.call({ kind: "no_such_kind" }, {}));
  assert.equal(unknown.ok, false);
  assert.equal(unknown.error.code, "unknown_kind");

  const thrown = plain(await runtime.call({ kind: "describe", locators: ["locator_missing"] }, {}));
  assert.equal(thrown.ok, false);
  assert.equal(thrown.error.code, "stale_locator");
  assert.ok(thrown.error.message.length > 0 && thrown.error.message.length <= 500);

  const uncoded = plain(await runtime.call({ kind: "describe", get locators() { throw new Error("boom"); } }, {}));
  assert.equal(uncoded.error.code, "page_exception");
  assert.equal(uncoded.error.message, "boom");

  const long = plain(await runtime.call({ kind: "describe", get locators() { throw "x".repeat(900); } }, {}));
  assert.equal(long.ok, false);
  assert.equal(long.error.code, "page_exception");
  assert.ok(long.error.message.length <= 500);
});

test("the kinds the protocol retired answer unknown_kind", async () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  const retired = [
    "fill", "prepare_text_fill", "verify_text_fill_focus", "verify_fill_values", "frame_boxes", "point_context",
    "document_route", "drag_observation_arm", "drag_observation_status", "drag_observation_finish",
    "form_diagnostics_state", "presentation_visibility", "managed_scope", "recording_state", "runtime_state",
    "present", "clear"
  ];
  for (const kind of retired) {
    const reply = plain(await runtime.call({ kind }, {}));
    assert.equal(reply.error?.code, "unknown_kind", kind);
  }
});

test("a read runs through the closure dispatcher", async () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  const body = dom.element("body");
  body.append(dom.text("Hello from the page"));
  dom.setBody(body);
  const reply = plain(await runtime.call({ kind: "read_text", mode: "visible", max_chars: 100, visual_settle: false }, {}));
  assert.equal(reply.ok, true);
  assert.equal(reply.value.text, "Hello from the page");
  assert.equal(reply.value.title, "Fixture");
});

test("submit_fill answers before it clicks the submit control", async () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  const form = dom.element("form");
  const field = new dom.HTMLInputElement();
  field.type = "text";
  field.hidden = false;
  field.setAttribute("aria-label", "Name");
  field.closest = (selector) => (selector === "form" ? form : null);
  const submit = dom.element("button");
  submit.setAttribute("aria-label", "Send");
  const clicks = [];
  submit.click = () => clicks.push("click");
  form.append(field, submit);
  const body = dom.element("body");
  body.append(form);
  dom.setBody(body);

  const inspected = plain(await runtime.call({ kind: "inspect", inspect_kind: "controls", max_items: 10, visual_settle: false }, {}));
  const locatorOf = (name) => inspected.value.targets.find((target) => target.name === name).locator;
  const fieldLocator = locatorOf("Name");
  const submitLocator = locatorOf("Send");

  const reply = plain(await runtime.call({
    kind: "submit_fill",
    fields: [{ locator: fieldLocator, value: "x" }],
    submit_locator: submitLocator,
    allow_credentials: false
  }, {}));
  assert.deepEqual(reply, { ok: true, value: { submitted: true } });
  assert.deepEqual(clicks, [], "the click is the tail of the call");
  dom.advance(0);
  assert.deepEqual(clicks, ["click"]);
});

test("ambient facts apply on a call and through the ambient entry", async () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  await runtime.call({ kind: "scroll_offset" }, { local: { ...LOCAL, hidden: true } });
  const host = hostOf(dom);
  assert.ok(host, "a call with local facts mounts the host");
  assert.equal(host.style.display, "none");
  const scope = host.shadow.childNodes[1].childNodes[0];
  assert.equal(scope.classList.contains("on"), true, "the top document shows the managed border");

  assert.equal(runtime.ambient({ ...LOCAL, hidden: false, managed: false }), true);
  assert.equal(host.style.display, "block");
  assert.equal(scope.classList.contains("on"), false);
});

test("a frame other than the top applies only the hidden fact and renders only anchored signals", async () => {
  const dom = createFakeDom({ top: false });
  const runtime = install(dom);
  runtime.ambient({ ...LOCAL, hidden: true });
  const host = hostOf(dom);
  assert.equal(host.style.display, "none");
  const scope = host.shadow.childNodes[1].childNodes[0];
  runtime.ambient({ ...LOCAL, hidden: false });
  assert.equal(scope.classList.contains("on"), false, "perpetual visuals belong to the top document");

  const signal = { signal: "start", activity: "script", invocation: "i-1" };
  assert.equal(runtime.present(signal, LOCAL), false);
});

test("presentation builds its markup with DOM calls, so Trusted Types pages render", () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  const signal = { signal: "start", activity: "script", invocation: "i-1" };
  assert.equal(runtime.present(signal, LOCAL), true);
  assert.ok(dom.svgNamespacesCreated() >= 2, "the cursor and the work wheel are built as SVG elements");
  assert.equal(runtime.present({ signal: "denial", phase: "Blocked", detail: "Not allowed here." }, LOCAL), true);
});

test("dispose removes the host, the timers and the global", () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  runtime.present({ signal: "start", activity: "script", invocation: "i-1" }, LOCAL);
  assert.ok(hostOf(dom));
  assert.ok(dom.pendingTimers() > 0, "the signature started timers");

  runtime.dispose();
  assert.equal(hostOf(dom), undefined);
  assert.equal(dom.pendingTimers(), 0);
  assert.equal(dom.context.__ghostlightRuntime, undefined);
  assert.ok(!dom.globals().includes("__ghostlightRuntime"));
});

test("dispose ends a wait in progress and disconnects its observer", async () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  const pending = runtime.call({ kind: "inspect", inspect_kind: "controls", max_items: 5, settle_ms: 5000 }, { token: "t-1" });
  assert.equal(dom.activeObservers(), 1);
  assert.ok(dom.pendingTimers() > 0);

  runtime.dispose();
  const reply = plain(await pending);
  assert.equal(reply.ok, false);
  assert.equal(reply.error.code, "cancelled");
  assert.equal(dom.activeObservers(), 0);
  assert.equal(dom.pendingTimers(), 0);
});

test("cancel ends a wait with the cancelled code and reports whether the token was live", async () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  assert.equal(runtime.cancel("nobody"), false);
  const pending = runtime.call({ kind: "inspect", inspect_kind: "controls", max_items: 5, settle_ms: 5000 }, { token: "t-2" });
  assert.equal(runtime.cancel("t-2"), true);
  dom.advance(60);
  const reply = plain(await pending);
  assert.equal(reply.ok, false);
  assert.equal(reply.error.code, "cancelled");
  assert.equal(dom.activeObservers(), 0);
  assert.equal(dom.pendingTimers(), 0);
  assert.equal(runtime.cancel("t-2"), false, "a finished call forgets its token");
});

test("a visual settle also ends when cancelled", async () => {
  const dom = createFakeDom();
  const runtime = install(dom);
  const pending = runtime.call({ kind: "observe", condition: "visual_settle", timeout_ms: 5000 }, { token: "t-3" });
  dom.document.documentElement.getBoundingClientRect = () => ({ x: 0, y: 0, width: 1, height: 1 });
  assert.equal(runtime.cancel("t-3"), true);
  dom.advance(30);
  const reply = plain(await pending);
  assert.equal(reply.error?.code, "cancelled");
  assert.equal(dom.pendingTimers(), 0);
});
