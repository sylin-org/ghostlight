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
// What the scripted Chrome answers a capture with. The fake image tool gives a picture its size.
const PNG = "iVBORw0KGgo=";
const JPEG = "/9j/4AAQSkZJRgABAQ==";
// The only parameters a photograph may carry: nothing in them makes Chrome lay the page out again.
const PHOTOGRAPH = { format: "png", optimizeForSpeed: true, fromSurface: true };

/** The layout metrics of a viewport scrolled to `pageX`, `pageY`, with `extra` visual viewport facts. */
function scrolled(pageX, pageY, extra = {}) {
  return { ...METRICS, cssVisualViewport: { ...METRICS.cssVisualViewport, pageX, pageY, ...extra } };
}

/**
 * Wires the capture primitives over a scripted debugger and a fake image tool.
 *
 * `shots` answers Page.captureScreenshot: a list by call index (the last repeats; an Error is thrown)
 * or a function of the parameters. `metrics` is one layout read or a list of successive reads (the
 * last repeats). `gate` is an optional function awaited by each screenshot, given its call index.
 * `ratio` is what the device pixel ratio shim reads, and `geometry` overrides the page facts the
 * geometry shim reads (null makes them unreadable). The fake image tool opens a PNG as a picture of
 * `surface` (default: the window's inner size in device pixels) and a JPEG as one of `measured`
 * (default: it cannot decode one); successive `jpeg()` calls resolve the `encoded` strings (the last
 * repeats). `tool` replaces the fake image tool.
 */
async function setup({
  shots = (params) => (params.clip ? JPEG : PNG),
  metrics = METRICS,
  gate = null,
  zoom = 1,
  ratio = 2,
  geometry = {},
  surface = null,
  measured = null,
  encoded = [JPEG],
  tool = null
} = {}) {
  const chrome = fakeChrome();
  const ambient = [];
  const log = { opened: [], encodes: [], closed: 0, hiddenAtOpen: [] };
  const facts = geometry && { devicePixelRatio: ratio, innerWidth: 1280, innerHeight: 720, ...geometry };
  const reads = [metrics].flat();
  let read = 0;
  let taken = 0;
  chrome.handlers["Page.getLayoutMetrics"] = () => {
    read += 1;
    return reads[Math.min(read, reads.length) - 1];
  };
  chrome.handlers["Page.captureScreenshot"] = async (_target, params) => {
    const index = taken;
    taken += 1;
    if (gate) await gate(index);
    const data = typeof shots === "function" ? shots(params, index) : shots[Math.min(index, shots.length - 1)];
    if (data instanceof Error) throw data;
    return { data };
  };
  chrome.handlers["Runtime.callFunctionOn"] = (_target, params) => {
    if (params.functionDeclaration === World.SHIMS.DPR) return { result: { value: ratio } };
    if (params.functionDeclaration === World.SHIMS.GEOMETRY) return { result: { value: facts } };
    if (params.functionDeclaration === World.SHIMS.AMBIENT) ambient.push(params.arguments[1].value);
    return { result: { value: true } };
  };
  const fakeTool = async (data, mimeType) => {
    log.opened.push({ data, mimeType });
    log.hiddenAtOpen.push(ambient.at(-1)?.hidden);
    const size = mimeType === "image/png"
      ? surface ?? { width: facts.innerWidth * facts.devicePixelRatio, height: facts.innerHeight * facts.devicePixelRatio }
      : measured;
    if (!size) throw new Error("no decoder");
    return {
      ...size,
      async jpeg(options) {
        log.encodes.push(options);
        return encoded[Math.min(log.encodes.length, encoded.length) - 1];
      },
      close() { log.closed += 1; }
    };
  };
  const parts = wire(chrome);
  await parts.world.setAssets(assets());
  const capture = Capture.create({
    lifecycle: parts.lifecycle,
    worldCalls: parts.world,
    chromeApi: { tabs: { getZoom: async () => zoom } },
    imageTool: tool ?? fakeTool
  });
  const screenshots = () => chrome.calls.filter((call) => call.method === "Page.captureScreenshot");
  /** How each capture was taken, in order: "photograph" for a plain capture, "render" for one Chrome was given a clip for. */
  const mechanisms = () => screenshots().map((call) => (call.params.clip ? "render" : "photograph"));
  return { ...parts, chrome, capture, ambient, log, screenshots, mechanisms };
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

test("a capture refuses a bad tab id and budget in the shared words", async () => {
  const Arguments = require("../lib/arguments.js");
  const shared = (check) => {
    try {
      check();
    } catch (error) {
      return error.message;
    }
    throw new Error("the shared check passed");
  };
  const { capture } = await setup();
  const good = goldenArgs("capture.viewport");
  const tab = await execute(capture.capture, { ...good, tab_id: 0 });
  assert.equal(tab.message, shared(() => Arguments.requireTabId(0)));
  const side = await execute(capture.capture, { ...good, budget: { ...good.budget, max_side: 0 } });
  assert.equal(side.message, shared(() => Arguments.requireInteger(0, "budget.max_side", { minimum: 1 })));
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

test("a viewport capture is one photograph of the surface, cut and scaled off the page, matching the golden receipt", async () => {
  const { capture, chrome, log, screenshots, lifecycle } = await setup();
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  matches(result, golden("receipts/capture.ok"));
  assert.deepEqual(screenshots().map((call) => call.params), [PHOTOGRAPH], "one capture, with no clip and no captureBeyondViewport");
  assert.deepEqual(screenshots()[0].target, { tabId: TAB }, "on the root session");
  assert.equal(chrome.calls.some((call) => call.method.startsWith("Emulation.")), false, "nothing resizes or emulates the page");
  const page = chrome.calls.map((call) => call.method).filter((method) => method === "Page.getLayoutMetrics" || method === "Page.captureScreenshot");
  assert.deepEqual(page, ["Page.getLayoutMetrics", "Page.captureScreenshot", "Page.getLayoutMetrics"], "the layout is read again after the picture");
  assert.deepEqual(log.opened, [{ data: PNG, mimeType: "image/png" }]);
  assert.deepEqual(log.encodes, [{ crop: { x: 0, y: 0, width: 2560, height: 1440 }, width: 1280, height: 720, quality: 80 }],
    "the surface holds two device pixels for each CSS pixel");
  assert.equal(log.closed, 1, "the picture is given back");
  assert.equal(lifecycle.attachedCount(), 0, "the lease is given back");
});

test("a clip the viewport shows is cut out of a photograph in device pixels; the whole page and a clip beyond it are re-rendered", async () => {
  const moved = scrolled(100, 300);
  const rect = goldenArgs("capture.viewport_rect");
  const clamped = { ...rect, clip: { clip: "viewport_rect", left: -50, top: -20, width: 300, height: 200 } };
  // [request, metrics, the clip reported, the crop at a device pixel ratio of 1, the image size]
  const shown = [
    [goldenArgs("capture.viewport"), moved, { x: 100, y: 300, width: 1280, height: 720 }, { x: 0, y: 0, width: 1280, height: 720 }, [1280, 720]],
    [rect, moved, { x: 140, y: 380, width: 600, height: 400 }, { x: 40, y: 80, width: 600, height: 400 }, [1313, 876]],
    [goldenArgs("capture.page_rect"), scrolled(0, 1100), { x: 0, y: 1200, width: 800, height: 600 }, { x: 0, y: 100, width: 800, height: 600 }, [800, 600]],
    [clamped, METRICS, { x: 0, y: 0, width: 300, height: 200 }, { x: 0, y: 0, width: 300, height: 200 }, [1313, 876]]
  ];
  for (const [args, metrics, clip, crop, [width, height]] of shown) {
    for (const ratio of [1, 2]) {
      const { capture, log, mechanisms } = await setup({ metrics, ratio });
      const result = await execute(capture.capture, args);
      const label = `${JSON.stringify(args.clip)} at ${ratio}`;
      assert.deepEqual(mechanisms(), ["photograph"], label);
      assert.deepEqual(result.ok.clip, clip, label);
      assert.equal(result.ok.view.visual_page_x, metrics.cssVisualViewport.pageX, label);
      assert.deepEqual(log.encodes[0].crop, { x: crop.x * ratio, y: crop.y * ratio, width: crop.width * ratio, height: crop.height * ratio }, label);
      assert.deepEqual([result.ok.width, result.ok.height, log.encodes[0].width, log.encodes[0].height], [width, height, width, height], label);
      assert.equal(result.ok.output_scale, Capture.outputScale(clip.width, clip.height, args.budget), label);
    }
  }

  // A clip that reaches no more than a CSS pixel past the viewport is held to it, and the held clip is the one reported.
  const edge = { ...goldenArgs("capture.page_rect"), clip: { clip: "page_rect", x: 0, y: 1099.5, width: 1280.75, height: 600 } };
  const held = await setup({ metrics: scrolled(0, 1100) });
  const trimmed = await execute(held.capture.capture, edge);
  assert.deepEqual(held.mechanisms(), ["photograph"]);
  assert.deepEqual(trimmed.ok.clip, { x: 0, y: 1100, width: 1280, height: 599.5 });
  assert.deepEqual(held.log.encodes[0].crop, { x: 0, y: 0, width: 2560, height: 1199 });
  const past = await setup({ metrics: scrolled(0, 1100) });
  const drawn = await execute(past.capture.capture, { ...edge, clip: { ...edge.clip, y: 1098.5 } });
  assert.deepEqual(past.mechanisms(), ["render"], "a pixel and a half is beyond it");
  assert.deepEqual(drawn.ok.clip, { x: 0, y: 1098.5, width: 1280.75, height: 600 });

  // The viewport shows page rows 300 to 1020, so the golden page rectangle at 1200 lies beyond it.
  const beyond = await setup({ metrics: moved });
  const below = await execute(beyond.capture.capture, goldenArgs("capture.page_rect"));
  assert.deepEqual(below.ok.clip, { x: 0, y: 1200, width: 800, height: 600 });
  assert.deepEqual(beyond.screenshots().map((call) => call.params), [{
    format: "jpeg", quality: 80, clip: { x: 0, y: 1200, width: 800, height: 600, scale: 1 }, captureBeyondViewport: true, fromSurface: true
  }]);
  assert.deepEqual(beyond.log.encodes, [], "Chrome's own JPEG is passed on as it came");
  assert.equal(below.ok.data, JPEG);

  // The whole page is re-rendered even where the viewport shows all of it.
  const short = { ...METRICS, cssContentSize: { width: 1280, height: 720 } };
  for (const metrics of [moved, short]) {
    const whole = await setup({ metrics });
    const full = await execute(whole.capture.capture, goldenArgs("capture.full_page"));
    assert.deepEqual(full.ok.clip, { x: 0, y: 0, ...metrics.cssContentSize });
    assert.deepEqual(whole.mechanisms(), ["render"]);
    assert.equal(whole.screenshots()[0].params.captureBeyondViewport, true);
  }
});

test("the capture scale comes from the budget: shrunk to fit, magnified only when allowed", async () => {
  const { capture, screenshots } = await setup();
  await execute(capture.capture, goldenArgs("capture.full_page"));
  const shrunk = screenshots()[0].params.clip.scale;
  assert.ok(shrunk < 1);
  assert.ok(1280 * shrunk <= 1568 && 2400 * shrunk <= 1568);
  assert.ok(1280 * 2400 * shrunk * shrunk <= 1150000 + 1);

  // A photograph is sized by the same rule, in the extension: half the viewport's CSS size here.
  const viewport = goldenArgs("capture.viewport");
  const small = await setup();
  const half = await execute(small.capture.capture, { ...viewport, budget: { ...viewport.budget, max_side: 640 } });
  assert.deepEqual([half.ok.width, half.ok.height, half.ok.output_scale], [640, 360, 0.5]);
  assert.deepEqual([small.log.encodes[0].width, small.log.encodes[0].height], [640, 360]);

  const budget = goldenArgs("capture.viewport_rect").budget;
  assert.equal(budget.magnify, true);
  assert.ok(Capture.outputScale(600, 400, budget) > 1);
  assert.equal(Capture.outputScale(600, 400, { ...budget, magnify: false }), 1);
  assert.equal(Capture.outputScale(6000, 6000, { ...budget, magnify: false, min_scale: 0.3 }), 0.3);
});

test("the quality ladder encodes one photograph again until a quality fits, and stops at the first that does", async () => {
  const ladder = await setup({ encoded: ["x".repeat(2_000_000), "y".repeat(1_500_000), "z".repeat(1_000_000)] });
  const result = await execute(ladder.capture.capture, goldenArgs("capture.viewport"));
  assert.equal(result.ok.data, "z".repeat(1_000_000));
  assert.deepEqual(ladder.log.encodes.map((encode) => encode.quality), [80, 60, 40]);
  assert.equal(new Set(ladder.log.encodes.map((encode) => JSON.stringify({ ...encode, quality: 0 }))).size, 1, "the same cut every time");
  assert.equal(ladder.screenshots().length, 1, "the page is captured once, whatever the ladder needs");

  const first = await setup({ encoded: ["x".repeat(1_400_000)] });
  await execute(first.capture.capture, goldenArgs("capture.viewport"));
  assert.deepEqual(first.log.encodes.map((encode) => encode.quality), [80]);
});

test("when no quality fits, the capture fails with capture_too_large and the golden message", async () => {
  const { capture, log, lifecycle, mechanisms } = await setup({ encoded: ["x".repeat(1_400_001)] });
  const result = await execute(capture.capture, goldenArgs("capture.viewport"));
  matches(result, golden("receipts/capture.capture_too_large"));
  assert.equal(log.encodes.length, 3);
  assert.deepEqual(mechanisms(), ["photograph"], "too large is an answer, not a reason to re-render");
  assert.equal(log.closed, 1);
  assert.equal(lifecycle.attachedCount(), 0);
});

test("a re-rendered capture asks Chrome once for each quality and reports the size it measures", async () => {
  const args = goldenArgs("capture.page_rect");
  const ladder = await setup({ metrics: scrolled(100, 300), shots: ["x".repeat(2_000_000), "y".repeat(1_500_000), "z".repeat(1_000_000)] });
  const fitted = await execute(ladder.capture.capture, args);
  assert.equal(fitted.ok.data.length, 1_000_000);
  assert.deepEqual(ladder.screenshots().map((call) => call.params.quality), [80, 60, 40]);
  assert.deepEqual([fitted.ok.width, fitted.ok.height, fitted.ok.output_scale], [800, 600, 1], "an image that cannot be decoded has the size asked for");

  const large = await setup({ metrics: scrolled(100, 300), shots: ["x".repeat(1_400_001)] });
  matches(await execute(large.capture.capture, args), golden("receipts/capture.capture_too_large"));
  assert.equal(large.screenshots().length, 3);

  const measured = await setup({ metrics: scrolled(100, 300), measured: { width: 400, height: 300 } });
  const half = await execute(measured.capture.capture, args);
  assert.deepEqual([half.ok.width, half.ok.height, half.ok.output_scale], [400, 300, 0.5]);
  assert.deepEqual(measured.log.opened, [{ data: JPEG, mimeType: "image/jpeg" }]);
  assert.equal(measured.log.closed, 1);

  const skewed = await setup({ metrics: scrolled(100, 300), measured: { width: 400, height: 600 } });
  const fallback = await execute(skewed.capture.capture, args);
  assert.deepEqual([fallback.ok.width, fallback.ok.height, fallback.ok.output_scale], [800, 600, 1]);
});

test("a photograph is taken only where its geometry is certain; anything else is re-rendered", async () => {
  const viewport = goldenArgs("capture.viewport");
  // [what is unusual, the setup, how the capture is taken]
  const cases = [
    ["Chrome reports no pinch", { metrics: scrolled(0, 0, { scale: 1 }) }, ["photograph"]],
    ["a pinch zoomed page", { metrics: scrolled(0, 0, { scale: 1.25 }) }, ["render"]],
    ["a scrollbar beside the page", { geometry: { innerWidth: 1295 } }, ["photograph"]],
    ["page facts that cannot be read", { geometry: null }, ["render"]],
    ["a window with no size", { geometry: { innerHeight: 0 } }, ["render"]],
    ["a device pixel ratio that was not read", { geometry: { devicePixelRatio: null } }, ["render"]],
    ["a picture that is not the window that was measured", { surface: { width: 2000, height: 1440 } }, ["photograph", "render"]],
    ["a picture three columns wider than the window", { surface: { width: 2563, height: 1440 } }, ["photograph", "render"]],
    ["a picture one row short of the window", { surface: { width: 2560, height: 1439 } }, ["photograph"]],
    // An inner size is whole CSS pixels, so at five device pixels each it is known to five and no better.
    ["a zoomed window off by less than one CSS pixel", { ratio: 5, surface: { width: 6396, height: 3600 } }, ["photograph"]],
    ["a zoomed window off by more than one CSS pixel", { ratio: 5, surface: { width: 6394, height: 3600 } }, ["photograph", "render"]]
  ];
  for (const [name, options, expected] of cases) {
    const rig = await setup(options);
    const result = await execute(rig.capture.capture, viewport);
    assert.deepEqual(rig.mechanisms(), expected, name);
    assert.deepEqual(result.ok.clip, { x: 0, y: 0, width: 1280, height: 720 }, name);
    assert.deepEqual(rig.ambient.map((facts) => facts.hidden), expected.flatMap(() => [true, false]), `${name}: hidden for each capture, and shown after it`);
    if (expected.at(-1) === "render") assert.equal(result.ok.data, JPEG, name);
  }

  // The scrollbar strip is in the picture, on its right, and is left out of the cut.
  const strip = await setup({ geometry: { innerWidth: 1295 } });
  await execute(strip.capture.capture, viewport);
  assert.deepEqual(strip.log.encodes[0].crop, { x: 0, y: 0, width: 2560, height: 1440 });

  // A viewport left of or above the page's origin is re-rendered as it always was: the clip of a photograph is never negative.
  for (const metrics of [scrolled(-200, 0), scrolled(0, -0.5)]) {
    const before = await setup({ metrics });
    const result = await execute(before.capture.capture, viewport);
    assert.deepEqual(before.mechanisms(), ["render"]);
    assert.deepEqual(result.ok.clip, { x: metrics.cssVisualViewport.pageX, y: metrics.cssVisualViewport.pageY, width: 1280, height: 720 });
  }
});

test("each pixel of a photographed capture shows the page point the receipt says, at any device pixel ratio and fractional size", async () => {
  const region = goldenArgs("capture.viewport_rect");
  const plain = goldenArgs("capture.viewport");
  // A viewport of fractional size, scrolled to a fractional offset, in a window with a scrollbar strip.
  const metrics = { cssVisualViewport: { pageX: 12.4, pageY: 301.7, clientWidth: 1264.8, clientHeight: 703.2 }, cssContentSize: { width: 1264.8, height: 2400 } };
  const requests = [
    plain,
    { ...plain, budget: { ...plain.budget, max_side: 640 } },
    { ...region, clip: { clip: "viewport_rect", left: 40.3, top: 80.6, width: 333.3, height: 201.7 } },
    { ...region, clip: { clip: "viewport_rect", left: -0.6, top: 500.9, width: 1265.9, height: 202.8 } },
    { ...plain, clip: { clip: "page_rect", x: 100.25, y: 310.5, width: 77.7, height: 0.4 } },
    { ...plain, clip: { clip: "page_rect", x: 1270.2, y: 1003.4, width: 0.5, height: 0.5 } }
  ];
  for (const ratio of [1, 1.25, 1.5, 2]) {
    const surface = { width: Math.round(1280 * ratio), height: Math.round(720 * ratio) };
    for (const args of requests) {
      const rig = await setup({ metrics, ratio, surface });
      const { ok } = await execute(rig.capture.capture, args);
      const label = `${JSON.stringify(args.clip)} at ${ratio}`;
      assert.deepEqual(rig.mechanisms(), ["photograph"], label);
      const [{ crop, width, height }] = rig.log.encodes;
      assert.deepEqual([ok.width, ok.height], [width, height], `${label}: the size reported is the size encoded`);
      assert.ok(Number.isInteger(width) && Number.isInteger(height) && width >= 1 && height >= 1, label);
      assert.ok(ok.clip.x >= 0 && ok.clip.y >= 0 && ok.output_scale > 0, label);
      assert.ok(args.budget.magnify || ok.output_scale <= 1, label);
      assert.ok(width <= args.budget.max_side && height <= args.budget.max_side, label);
      for (const side of [crop.x, crop.y, crop.width, crop.height]) assert.ok(Number.isInteger(side), label);
      assert.ok(crop.x >= 0 && crop.y >= 0 && crop.width >= 1 && crop.height >= 1, label);
      assert.ok(crop.x + crop.width <= Math.round(1264.8 * ratio) && crop.y + crop.height <= Math.round(703.2 * ratio), `${label}: the scrollbar strip is left out`);
      // Along each axis: [the picture's page origin, the clip's start, the crop's start and length, the image's length].
      const axes = [
        [12.4, ok.clip.x, crop.x, crop.width, width],
        [301.7, ok.clip.y, crop.y, crop.height, height]
      ];
      // The coarser of an image pixel and a screen pixel, in CSS pixels: no picture is truer than that.
      const pixel = Math.max(1 / ok.output_scale, 1 / ratio);
      for (const [origin, start, first, length, size] of axes) {
        for (const at of [0, size / 2, size]) {
          const shown = origin + (first + at * (length / size)) / ratio;
          const said = start + at / ok.output_scale;
          assert.ok(Math.abs(shown - said) <= pixel, `${label}: image ${at} of ${size} shows ${shown}, the receipt says ${said}`);
        }
      }
    }
  }
});

test("an image tool that fails leaves the capture to be re-rendered", async () => {
  let closed = 0;
  const tools = [
    async () => { throw new Error("no canvas in this worker"); },
    async () => ({ width: 2560, height: 1440, jpeg: async () => { throw new Error("no encoder"); }, close() { closed += 1; } })
  ];
  for (const tool of tools) {
    const { capture, mechanisms, ambient, lifecycle } = await setup({ tool });
    const result = await execute(capture.capture, goldenArgs("capture.viewport"));
    assert.deepEqual(mechanisms(), ["photograph", "render"]);
    assert.equal(result.ok.data, JPEG);
    assert.deepEqual(result.ok.clip, { x: 0, y: 0, width: 1280, height: 720 });
    assert.deepEqual(ambient.map((facts) => facts.hidden), [true, false, true, false]);
    assert.equal(lifecycle.attachedCount(), 0);
  }
  assert.equal(closed, 2, "the photograph and the measured image are both given back");
});

test("a viewport that moves under a photograph gets one more, and the clip and view are those of the read the picture agrees with", async () => {
  const viewport = goldenArgs("capture.viewport");
  const once = await setup({ metrics: [scrolled(0, 0), scrolled(0, 40), scrolled(0, 40.5)] });
  const second = await execute(once.capture.capture, viewport);
  assert.deepEqual(once.mechanisms(), ["photograph", "photograph"]);
  assert.deepEqual(second.ok.clip, { x: 0, y: 40, width: 1280, height: 720 }, "the read between the two pictures, which the last read confirms");
  assert.equal(second.ok.view.visual_page_y, 40);
  assert.deepEqual(once.log.opened, [{ data: PNG, mimeType: "image/png" }], "only the last picture is decoded");

  // A viewport that is still moving after the second picture has no picture that agrees with a read,
  // so the clip of the last read is re-rendered: that image shows its clip wherever the viewport is.
  const twice = await setup({ metrics: [scrolled(0, 0), scrolled(0, 40), scrolled(0, 90)] });
  const last = await execute(twice.capture.capture, viewport);
  assert.deepEqual(twice.mechanisms(), ["photograph", "photograph", "render"], "never a third photograph");
  assert.deepEqual(twice.screenshots()[2].params.clip, { x: 0, y: 90, width: 1280, height: 720, scale: 1 });
  assert.deepEqual(last.ok.clip, { x: 0, y: 90, width: 1280, height: 720 });
  assert.equal(last.ok.view.visual_page_y, 90);
  assert.equal(last.ok.data, JPEG);
  assert.deepEqual(twice.log.opened.map((image) => image.mimeType), ["image/jpeg"], "neither photograph is used");
  assert.deepEqual(twice.ambient.map((facts) => facts.hidden), [true, false], "all of it inside one hold");

  // A page rectangle the viewport has left cannot be photographed any more.
  const left = await setup({ metrics: [scrolled(0, 1100), scrolled(0, 0)] });
  const gone = await execute(left.capture.capture, goldenArgs("capture.page_rect"));
  assert.deepEqual(left.mechanisms(), ["photograph", "render"]);
  assert.deepEqual(left.screenshots()[1].params.clip, { x: 0, y: 1200, width: 800, height: 600, scale: 1 });
  assert.equal(gone.ok.view.visual_page_y, 0);
  assert.deepEqual(left.ambient.map((facts) => facts.hidden), [true, false], "all of it inside one hold");
});

test("a capture hides Ghostlight's presentation only while Chrome reads pixels, and shows it again before the image work", async () => {
  const { capture, ambient, log } = await setup();
  await execute(capture.capture, goldenArgs("capture.viewport"));
  assert.deepEqual(ambient.map((facts) => facts.hidden), [true, false]);
  assert.deepEqual(log.hiddenAtOpen, [false], "the picture is decoded after the presentation is back");
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

test("a capture or view with a JavaScript dialog open fails dialog_open and asks the blocked page for nothing", async () => {
  const { capture, chrome, lifecycle, screenshots, ambient } = await setup();
  await lifecycle.acquire(TAB);
  lifecycle.openDialog(TAB, { type: "alert", message: "Hi" });
  chrome.calls.length = 0;
  const shot = await execute(capture.capture, goldenArgs("capture.viewport"));
  matches(shot, golden("receipts/capture.dialog_open"));
  const view = await execute(capture.view, goldenArgs("view"));
  assert.equal(view.failure, "dialog_open");
  assert.equal(screenshots().length, 0);
  assert.equal(ambient.length, 0);
  assert.deepEqual(chrome.calls, [], "not even its layout");
  await lifecycle.detachAll();
});

// Runs `work` with the browser's image interfaces replaced by recording stand-ins, which Node lacks.
// With `native` false the engine's own base64 methods are hidden too, as in a browser without them.
async function withBrowserImages({ native, encodedType = null, bytes }, work) {
  const calls = [];
  const saved = {
    createImageBitmap: globalThis.createImageBitmap,
    OffscreenCanvas: globalThis.OffscreenCanvas,
    fromBase64: Uint8Array.fromBase64,
    toBase64: Uint8Array.prototype.toBase64
  };
  globalThis.createImageBitmap = async (source, ...cut) => {
    const size = cut.length ? { width: cut[4].resizeWidth ?? cut[2], height: cut[4].resizeHeight ?? cut[3] } : { width: 2560, height: 1440 };
    const bitmap = { ...size, close() { calls.push(["close", bitmap]); } };
    calls.push(["bitmap", source, cut, bitmap]);
    return bitmap;
  };
  globalThis.OffscreenCanvas = class {
    constructor(width, height) { calls.push(["canvas", width, height]); }
    getContext(kind, options) { return { drawImage: (image, x, y) => calls.push(["draw", kind, options, image, x, y]) }; }
    async convertToBlob(options) {
      calls.push(["encode", options]);
      return new Blob([bytes], { type: encodedType ?? options.type });
    }
  };
  if (!native) {
    delete Uint8Array.fromBase64;
    delete Uint8Array.prototype.toBase64;
  }
  try {
    return await work(calls);
  } finally {
    for (const name of ["createImageBitmap", "OffscreenCanvas"]) {
      if (saved[name]) globalThis[name] = saved[name];
      else delete globalThis[name];
    }
    if (saved.fromBase64) Uint8Array.fromBase64 = saved.fromBase64;
    if (saved.toBase64) Uint8Array.prototype.toBase64 = saved.toBase64;
  }
}

test("the browser's image tool decodes once, cuts and scales once, and encodes padded base64 JPEG at each quality", async () => {
  // Enough bytes to span several text chunks, and a count that needs base64 padding.
  const bytes = Uint8Array.from({ length: 70_000 }, (_value, index) => (index * 7) % 256);
  const source = [137, 80, 78, 71, 13, 10, 26, 10, 255, 0];
  const crop = { x: 80, y: 160, width: 1200, height: 800 };
  for (const native of [true, false]) {
    await withBrowserImages({ native, bytes }, async (calls) => {
      const picture = await Capture.openImage(Buffer.from(source).toString("base64"), "image/png");
      assert.deepEqual([picture.width, picture.height], [2560, 1440]);
      const [, decoded, , surface] = calls[0];
      assert.equal(decoded.type, "image/png");
      assert.deepEqual(Array.from(new Uint8Array(await decoded.arrayBuffer())), source);

      const first = await picture.jpeg({ crop, width: 600, height: 400, quality: 80 });
      const second = await picture.jpeg({ crop, width: 600, height: 400, quality: 60 });
      assert.equal(first, Buffer.from(bytes).toString("base64"), `standard padded base64 (native ${native})`);
      assert.equal(second, first);
      const cuts = calls.filter(([kind, , cut]) => kind === "bitmap" && cut.length > 0);
      assert.equal(cuts.length, 1, "scaled once for both qualities");
      assert.equal(cuts[0][1], surface, "cut from the decoded picture");
      assert.deepEqual(cuts[0][2], [80, 160, 1200, 800, { resizeWidth: 600, resizeHeight: 400, resizeQuality: "high" }]);
      assert.deepEqual(calls.filter(([kind]) => kind === "canvas"), [["canvas", 600, 400]]);
      assert.deepEqual(calls.filter(([kind]) => kind === "draw"), [["draw", "2d", { alpha: false }, cuts[0][3], 0, 0]]);
      assert.deepEqual(calls.filter(([kind]) => kind === "encode").map(([, options]) => options),
        [{ type: "image/jpeg", quality: 0.8 }, { type: "image/jpeg", quality: 0.6 }]);

      await picture.jpeg({ crop: { ...crop, x: 0 }, width: 1200, height: 800, quality: 80 });
      const again = calls.filter(([kind, , cut]) => kind === "bitmap" && cut.length > 0);
      assert.deepEqual(again.map(([, , cut]) => cut)[1], [0, 160, 1200, 800, {}], "another cut is made anew, and one at its own size is copied, not resampled");
      assert.deepEqual(calls.filter(([kind]) => kind === "canvas").at(-1), ["canvas", 1200, 800]);
      picture.close();
      assert.deepEqual(calls.filter(([kind]) => kind === "close").map(([, bitmap]) => bitmap === surface), [false, false, true],
        "each scaled cut is given back once drawn, and the picture when it is closed");
    });
  }

  // A canvas that cannot encode a JPEG answers with a PNG. That is never passed on as a JPEG.
  await withBrowserImages({ native: true, bytes, encodedType: "image/png" }, async () => {
    const picture = await Capture.openImage(Buffer.from(source).toString("base64"), "image/png");
    await assert.rejects(picture.jpeg({ crop, width: 600, height: 400, quality: 80 }), /did not encode a JPEG/);
  });
});
