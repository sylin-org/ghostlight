"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const Capture = require("../lib/capture.js");
const World = require("../lib/world.js");
const { fakeChrome, assets, wire } = require("./p4-fakes.js");
const { golden, goldenArgs, execute } = require("./p4-chrome.js");

const TAB = 12;
const METRICS = {
  cssVisualViewport: { pageX: 0, pageY: 0, clientWidth: 1280, clientHeight: 720 },
  cssContentSize: { width: 1280, height: 2400 }
};

/**
 * Wires the capture primitives over a scripted debugger. `shots` lists the base64 `data` strings that
 * successive Page.captureScreenshot calls return; `metrics` are the layout metrics; `gate` is an optional
 * function awaited by each screenshot, given its call index.
 */
async function setup({ shots = ["/9j/4AAQSkZJRgABAQ=="], metrics = METRICS, gate = null, image = null, zoom = 1, ratio = 2 } = {}) {
  const chrome = fakeChrome();
  const ambient = [];
  let taken = 0;
  chrome.handlers["Page.getLayoutMetrics"] = () => metrics;
  chrome.handlers["Page.captureScreenshot"] = async () => {
    const index = taken;
    taken += 1;
    if (gate) await gate(index);
    const data = shots[Math.min(index, shots.length - 1)];
    if (data instanceof Error) throw data;
    return { data };
  };
  chrome.handlers["Runtime.callFunctionOn"] = (_target, params) => {
    if (params.functionDeclaration === World.SHIMS.DPR) return { result: { value: ratio } };
    if (params.functionDeclaration === World.SHIMS.AMBIENT) ambient.push(params.arguments[1].value);
    return { result: { value: true } };
  };
  const parts = wire(chrome);
  await parts.world.setAssets(assets());
  const capture = Capture.create({
    lifecycle: parts.lifecycle,
    worldCalls: parts.world,
    chromeApi: { tabs: { getZoom: async () => zoom } },
    imageSize: image ?? (async () => ({ width: 1280, height: 720 }))
  });
  return { ...parts, chrome, capture, ambient, screenshots: () => chrome.calls.filter((call) => call.method === "Page.captureScreenshot") };
}

function matches(result, receipt) {
  if (receipt.ok) {
    assert.deepEqual(result.ok, receipt.ok);
  } else {
    assert.equal(result.failure, receipt.failure);
    assert.equal(result.message, receipt.message);
  }
  assert.equal(result.dispatched, receipt.dispatched);
}

test("view and capture carry validate, run, addresses and tabOf", async () => {
  const { capture } = await setup();
  for (const handler of [capture.view, capture.capture]) {
    assert.equal(typeof handler.validate, "function");
    assert.equal(typeof handler.run, "function");
    assert.equal(handler.addresses({}), true);
    assert.equal(handler.tabOf({ tab_id: 5 }), 5);
  }
});

test("every golden view and capture request passes validation", async () => {
  const { capture } = await setup();
  capture.view.validate(goldenArgs("view"));
  for (const name of ["capture.viewport", "capture.full_page", "capture.viewport_rect", "capture.page_rect"]) {
    assert.doesNotThrow(() => capture.capture.validate(goldenArgs(name)), name);
  }
});

test("bad capture arguments are invalid_request", async () => {
  const { capture } = await setup();
  const good = goldenArgs("capture.viewport");
  const bad = [
    { ...good, tab_id: 0 },
    { ...good, clip: { clip: "everything" } },
    { ...good, clip: { clip: "page_rect", x: -1, y: 0, width: 10, height: 10 } },
    { ...good, clip: { clip: "page_rect", x: 0, y: 0, width: 0, height: 10 } },
    { ...good, clip: { clip: "viewport_rect", left: "1", top: 0, width: 10, height: 10 } },
    { ...good, budget: { ...good.budget, qualities: [] } },
    { ...good, budget: { ...good.budget, qualities: [101] } },
    { ...good, budget: { ...good.budget, min_scale: 0 } },
    { ...good, budget: { ...good.budget, magnify: "yes" } },
    { ...good, budget: { ...good.budget, max_base64_chars: 0 } },
    { ...good, budget: null }
  ];
  for (const args of bad) {
    const result = await execute(capture.capture, args);
    assert.equal(result.failure, "invalid_request", JSON.stringify(args));
    assert.equal(result.dispatched, false);
  }
});

test("view reads the layout metrics, the isolated-world ratio and the zoom, matching the golden receipt", async () => {
  const { capture, lifecycle, chrome } = await setup();
  const result = await execute(capture.view, goldenArgs("view"));
  matches(result, golden("receipts/view.ok"));
  assert.equal(lifecycle.attachedCount(), 0);
  assert.equal(chrome.calls.some((call) => call.method === "Runtime.evaluate" && call.params.expression?.includes("devicePixelRatio")), false);
});

test("view falls back to a device scale of 1 when the ratio is unreadable", async () => {
  const { capture } = await setup({ ratio: null });
  assert.equal((await execute(capture.view, goldenArgs("view"))).ok.device_scale, 1);
});

test("a viewport capture matches the golden receipt and asks Chrome for a scale-one jpeg clip", async () => {
  const { capture, screenshots } = await setup();
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  matches(result, golden("receipts/capture.ok"));
  assert.deepEqual(screenshots().map((call) => call.params), [{
    format: "jpeg", quality: 80, clip: { x: 0, y: 0, width: 1280, height: 720, scale: 1 }, captureBeyondViewport: true, fromSurface: true
  }]);
});

test("the clip kinds map to page space with the same metrics read", async () => {
  const moved = { ...METRICS, cssVisualViewport: { pageX: 100, pageY: 300, clientWidth: 1280, clientHeight: 720 } };
  const cases = [
    ["capture.viewport", { x: 100, y: 300, width: 1280, height: 720 }],
    ["capture.viewport_rect", { x: 140, y: 380, width: 600, height: 400 }],
    ["capture.page_rect", { x: 0, y: 1200, width: 800, height: 600 }]
  ];
  for (const [name, clip] of cases) {
    const { capture } = await setup({ metrics: moved, image: async () => ({ width: 1, height: 1 }) });
    const result = await execute(capture.capture, goldenArgs(name));
    assert.deepEqual(result.ok.clip, clip, name);
    assert.equal(result.ok.view.visual_page_x, 100);
  }
  const { capture } = await setup({ metrics: moved });
  const full = await execute(capture.capture, goldenArgs("capture.full_page"));
  assert.deepEqual(full.ok.clip, { x: 0, y: 0, width: 1280, height: 2400 });
});

test("a viewport_rect that starts left of the page is clamped at zero", async () => {
  const { capture } = await setup();
  const args = { ...goldenArgs("capture.viewport_rect"), clip: { clip: "viewport_rect", left: -50, top: -20, width: 300, height: 200 } };
  const result = await execute(capture.capture, args);
  assert.deepEqual([result.ok.clip.x, result.ok.clip.y], [0, 0]);
});

test("the capture scale comes from the budget: shrunk to fit, magnified only when allowed", async () => {
  const { capture, screenshots } = await setup({ image: async () => ({ width: 1, height: 1 }) });
  await execute(capture.capture, goldenArgs("capture.full_page"));
  const shrunk = screenshots()[0].params.clip.scale;
  assert.ok(shrunk < 1);
  assert.ok(1280 * shrunk <= 1568 && 2400 * shrunk <= 1568);
  assert.ok(1280 * 2400 * shrunk * shrunk <= 1150000 + 1);

  const budget = goldenArgs("capture.viewport_rect").budget;
  assert.equal(budget.magnify, true);
  assert.ok(Capture.outputScale(600, 400, budget) > 1);
  assert.equal(Capture.outputScale(600, 400, { ...budget, magnify: false }), 1);
  assert.equal(Capture.outputScale(6000, 6000, { ...budget, magnify: false, min_scale: 0.3 }), 0.3);
});

test("the quality ladder takes the first quality that fits", async () => {
  const { capture, screenshots } = await setup({ shots: ["x".repeat(2_000_000), "y".repeat(1_500_000), "z".repeat(1_000_000)] });
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  assert.equal(result.ok.data.length, 1_000_000);
  assert.deepEqual(screenshots().map((call) => call.params.quality), [80, 60, 40]);
});

test("a capture that fits at the first quality stops there", async () => {
  const { capture, screenshots } = await setup({ shots: ["x".repeat(1_400_000)] });
  await execute(capture.capture, goldenArgs("capture.viewport"));
  assert.equal(screenshots().length, 1);
});

test("when no quality fits, the capture fails with capture_too_large and the golden message", async () => {
  const { capture, screenshots, lifecycle } = await setup({ shots: ["x".repeat(1_400_001)] });
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  matches(result, golden("receipts/capture.capture_too_large"));
  assert.equal(screenshots().length, 3);
  assert.equal(lifecycle.attachedCount(), 0);
});

test("the measured image size and scale are reported, and an untrustworthy measure falls back to the request", async () => {
  const measured = await setup({ image: async () => ({ width: 640, height: 360 }) });
  const half = await execute(measured.capture.capture, goldenArgs("capture.viewport"));
  assert.deepEqual([half.ok.width, half.ok.height, half.ok.output_scale], [640, 360, 0.5]);

  const skewed = await setup({ image: async () => ({ width: 640, height: 720 }) });
  const fallback = await execute(skewed.capture.capture, goldenArgs("capture.viewport"));
  assert.deepEqual([fallback.ok.width, fallback.ok.height, fallback.ok.output_scale], [1280, 720, 1]);

  const broken = await setup({ image: async () => { throw new Error("no decoder"); } });
  const rounded = await execute(broken.capture.capture, goldenArgs("capture.viewport"));
  assert.deepEqual([rounded.ok.width, rounded.ok.height], [1280, 720]);
});

test("a capture hides Ghostlight's presentation while it runs and shows it again at the end", async () => {
  const { capture, ambient } = await setup();
  await execute(capture.capture, goldenArgs("capture.viewport"));
  assert.deepEqual(ambient.map((facts) => facts.hidden), [true, false]);
});

test("a failed capture still unhides, releases the debugger and reports the failure", async () => {
  const { capture, ambient, lifecycle } = await setup({ shots: [new Error("Page.captureScreenshot failed")] });
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  assert.equal(result.failure, "other");
  assert.deepEqual(ambient.map((facts) => facts.hidden), [true, false]);
  assert.equal(lifecycle.attachedCount(), 0);
});

test("a capture refused by the CDP allowlist is browser_refused and still unhides", async () => {
  const { capture, ambient } = await setup({ shots: [Object.assign(new Error("CDP method Page.captureScreenshot was refused."), { code: "browser_refused" })] });
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  matches(result, golden("receipts/capture.browser_refused"));
  assert.deepEqual(ambient.map((facts) => facts.hidden), [true, false]);
});

test("a failed layout read still unhides", async () => {
  const { capture, ambient, chrome } = await setup();
  chrome.handlers["Page.getLayoutMetrics"] = () => { throw new Error("target closed"); };
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  assert.equal(result.failure, "other");
  assert.deepEqual(ambient.map((facts) => facts.hidden), [true, false]);
});

test("concurrent captures keep the presentation hidden until the last one ends", async () => {
  let open;
  const gate = new Promise((resolve) => { open = resolve; });
  const { capture, ambient } = await setup({ gate: (index) => (index === 0 ? gate : null) });
  const first = execute(capture.capture, goldenArgs("capture.viewport"));
  const second = execute(capture.capture, goldenArgs("capture.viewport"));
  const winner = await Promise.race([first.then(() => "first"), second.then(() => "second")]);
  assert.equal(winner, "second");
  assert.equal(ambient.at(-1).hidden, true);
  open();
  await Promise.all([first, second]);
  assert.equal(ambient.at(-1).hidden, false);
  assert.equal(ambient.filter((facts) => facts.hidden === false).length, 1);
});

test("a capture or view with a JavaScript dialog open fails dialog_open and sends no screenshot", async () => {
  const { capture, lifecycle, screenshots, ambient } = await setup();
  await lifecycle.acquire(TAB);
  lifecycle.openDialog(TAB, { type: "alert", message: "Hi" });
  const shot = await execute(capture.capture, goldenArgs("capture.viewport"));
  matches(shot, golden("receipts/capture.dialog_open"));
  const view = await execute(capture.view, goldenArgs("view"));
  assert.equal(view.failure, "dialog_open");
  assert.equal(screenshots().length, 0);
  assert.equal(ambient.length, 0);
  await lifecycle.detachAll();
});

test("capture reads without dispatching and gives its debugger lease back", async () => {
  const { capture, lifecycle } = await setup();
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  assert.equal(result.dispatched, false);
  assert.equal(lifecycle.attachedCount(), 0);
});
