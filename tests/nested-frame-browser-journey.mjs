// Regression journey for two fixed defects of the extension, in real Chromium.
//
// (a) After a tab leaves a page that holds a cross-site frame inside a cross-site frame, the frames
//     receipt is complete, lists only the documents of the page now shown, and no debugger command
//     goes to a session Chrome no longer knows. Chrome ends the inner frame's session with the
//     outer frame's and never says so, and the extension kept reading it.
// (b) A viewport capture of a page whose cross-process frame has a CSS minimum size and is busy
//     when resized shows that frame at its full size, every time. A capture that asks Chrome to
//     capture beyond the viewport makes Chrome squeeze the view to one pixel and restore it, and
//     the picture was taken while the frame was still squeezed.
//
// The shipped extension runs as it is: its worker, its modules, the real chrome.debugger and
// chrome.webNavigation, and the worker's own image code. Only native-port discovery is replaced
// (tests/lib/browser.mjs), and the peer behind that port is this file. It plays the service side
// of adapter protocol 4 in process: it answers the hello with the golden hello_accepted, sends
// requests shaped like the golden requests and collects the receipts. No Ghostlight executable is
// built or started, nothing from extension/lib is loaded into Node, and no existing profile,
// native-host registration or installed extension is used. The pages are read through the
// journey's own DevTools session, never through the extension.
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { BEFORE_COMMAND_HOOK, BrowserRig, EXTENSION_DIRECTORY, extensionManifest } from "./lib/browser.mjs";
import { closed, decodeFrames, encodeFrame, EVIDENCE_VARIABLE, repository, until } from "./lib/home.mjs";
import { adapter } from "./lib/peers.mjs";

// ---- the two promises ----

const CAPTURE_PROMISE = "a viewport capture shows a busy cross-process frame at its full size";
const LEAVING_PROMISE = "after a tab leaves nested cross-site frames the frames receipt is complete and no ended session is read";

/** The browser scratch prefix of this journey, without its dash; tests/lib/chromium.mjs lists it. */
const LABEL = "nested-frame-browser";

// ---- the fixture: three sites on one loopback listener ----

const LOOPBACK = "127.0.0.1";
// Three registrable names, so Chromium gives each document its own process. The browser resolves
// them through its host rules, which need no IPv6 loopback listener, and reaches nothing else.
const SITE = Object.freeze({ TOP: "nested-top.test", APP: "nested-app.test", INNER: "nested-inner.test" });
const HOST_RULES = [...Object.values(SITE).map((host) => `MAP ${host} ${LOOPBACK}`), "MAP * ~NOTFOUND", `EXCLUDE ${LOOPBACK}`].join(", ");
const PATH = Object.freeze({ TOP: "/top", APP: "/app", INNER: "/inner", PLAIN: "/plain" });
const BLANK_URL = "about:blank";

/** The id of the top page's frame element. */
const FRAME_ID = "app";
/** The frame's CSS minimum size: what it shrinks to when Chrome squeezes the view to one pixel. */
const FRAME_MIN = Object.freeze({ width: 300, height: 200 });
/** The solid color of the app page, by which a picture shows where the frame is. */
const FRAME_COLOR = Object.freeze([0, 0, 255]);
/** The nested frame inside the app page: small, and away from every edge of the app page. */
const NESTED = Object.freeze({ width: 160, height: 80, inset: 20, color: "rgb(0, 160, 0)" });
/**
 * How long the app page keeps its thread when it grows. It shrinks at once. So when Chrome
 * squeezes and restores the view, the frame has drawn itself small and is still small when the
 * picture is taken: the defect shows every time and not only when a race falls that way. An
 * extension with the defect failed every capture from 250 ms up, and half of them at 100 ms.
 */
const GROW_BUSY_MS = 1500;
/**
 * The blocks the top page lays out on every change of its size, as a heavy page does. Chrome's
 * squeeze and restore are two layouts of the top page, and the frame draws itself small only when
 * a frame of its own falls between them. With a top page that lays out in no time that happened
 * in about one capture of four. An extension with the defect failed every capture from 20,000
 * blocks up; this many take about 45 ms a layout on the machine that calibrated them.
 */
const BALLAST_BLOCKS = 60_000;

const origin = (host, port) => `http://${host}:${port}`;

/** The fixture page at `path`, or null. */
function fixturePage(path, port) {
  switch (path) {
    case PATH.TOP:
      return `<!doctype html><title>Nested frame fixture: top</title><style>
html, body { margin: 0; height: 100%; background: #fff; overflow: hidden; }
#ballast { position: absolute; left: 0; top: 0; width: 100%; height: 100%; overflow: hidden; visibility: hidden; }
#ballast p { width: 50%; height: 1px; margin: 0; }
iframe { position: absolute; left: 0; top: 0; width: 100%; height: 100%; min-width: ${FRAME_MIN.width}px; min-height: ${FRAME_MIN.height}px; border: 0; }
</style><div id="ballast"></div><iframe id="${FRAME_ID}" src="${origin(SITE.APP, port)}${PATH.APP}"></iframe>
<script>document.getElementById("ballast").innerHTML = "<p></p>".repeat(${BALLAST_BLOCKS});</script>`;
    case PATH.APP:
      return `<!doctype html><title>Nested frame fixture: app</title><style>
html, body { margin: 0; height: 100%; background: rgb(${FRAME_COLOR.join(", ")}); }
iframe { position: absolute; right: ${NESTED.inset}px; bottom: ${NESTED.inset}px; width: ${NESTED.width}px; height: ${NESTED.height}px; border: 0; }
</style><iframe src="${origin(SITE.INNER, port)}${PATH.INNER}"></iframe><script>
let width = innerWidth;
let height = innerHeight;
addEventListener("resize", () => {
  // A frame has no size until its parent lays it out; that first size is not growth.
  const grew = width > 0 && height > 0 && (innerWidth > width || innerHeight > height);
  width = innerWidth;
  height = innerHeight;
  if (!grew) return;
  const end = performance.now() + ${GROW_BUSY_MS};
  while (performance.now() < end) { /* an app that is slow to grow */ }
});
</script>`;
    case PATH.INNER:
      return `<!doctype html><title>Nested frame fixture: inner</title><body style="margin: 0; background: ${NESTED.color}">`;
    case PATH.PLAIN:
      return "<!doctype html><title>Nested frame fixture: plain</title><p>This page holds no frame.</p>";
    default:
      return null;
  }
}

const fixture = createServer((request, response) => {
  const body = fixturePage(new URL(request.url, "http://fixture").pathname, fixture.address().port);
  if (body === null) {
    response.writeHead(404);
    response.end();
    return;
  }
  response.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
  response.end(body);
});

// ---- the service side of adapter protocol 4, in process ----

/** One golden file of adapter protocol 4. */
const golden = (section, name) => JSON.parse(readFileSync(join(repository, "tests", "golden", "protocol4", section, `${name}.json`), "utf8"));
/** The golden requests this journey sends, by file name; each is sent with the journey's own values. */
const REQUEST = Object.freeze({ OPEN_TAB: "open_tab", FRAMES: "frames", CAPTURE_VIEWPORT: "capture.viewport", NAVIGATE_URL: "navigate.url" });
const HELLO_ACCEPTED = golden("frames", "hello_accepted");
/** The frame kinds of adapter protocol 4, from the vocabulary the extension and the service share. */
const frameKind = closed([...adapter.frame_kinds.extension_to_service, ...adapter.frame_kinds.service_to_extension]);
/** The prefix of a correlation, as the service writes it. */
const CORRELATION_PREFIX = "p4_";
/** Time the extension has to answer one request. */
const RECEIPT_MS = 30_000;
/** Time a tab has to become ready, sent with each request that loads a page. */
const READY_TIMEOUT_MS = 20_000;

/**
 * What the browser rig takes for a test home: it offers `edge` and `start()`, and the "edge" it
 * starts is an object in this process that speaks native-messaging frames as the service would.
 */
class ServiceStandIn {
  constructor() {
    this.edge = "the service side of adapter protocol 4, in process";
    this.hellos = [];
    this.receipts = new Map();
    this.link = null;
    this.sent = 0;
  }

  /** One port of the extension: the shape of a started edge, with frames in on stdin and out on stdout. */
  start() {
    const child = new EventEmitter();
    child.stdout = new EventEmitter();
    child.output = "";
    let buffer = Buffer.alloc(0);
    let open = true;
    const link = { send: (frame) => { if (open) child.stdout.emit("data", encodeFrame(frame)); } };
    child.stdin = {
      write: (bytes) => {
        const { frames, rest } = decodeFrames(Buffer.concat([buffer, bytes]));
        buffer = rest;
        for (const frame of frames) this.take(frame, link);
      },
      end: () => {
        if (!open) return;
        open = false;
        if (this.link === link) this.link = null;
        child.emit("close", 0, null);
      }
    };
    return child;
  }

  take(frame, link) {
    if (frame.kind === frameKind.hello) {
      this.hellos.push(frame);
      this.link = link;
      link.send(HELLO_ACCEPTED);
    } else if (frame.kind === frameKind.receipt) {
      this.receipts.set(frame.correlation, frame);
    }
  }

  /** Sends the golden request `name` with `args` over the golden's own, and returns its one receipt. */
  async request(name, args = {}) {
    const frame = golden("requests", name);
    const correlation = `${CORRELATION_PREFIX}${(this.sent += 1).toString(16).padStart(4, "0")}`;
    assert.ok(this.link, "the extension holds no open port to the service stand-in");
    this.link.send({ ...frame, correlation, primitive: { name: frame.primitive.name, args: { ...frame.primitive.args, ...args } } });
    const receipt = await until(() => this.receipts.get(correlation), `the receipt of ${name}`, RECEIPT_MS);
    this.receipts.delete(correlation);
    return receipt;
  }
}

// ---- what the journey records in the worker ----

const CDP_FRAME_TREE = "Page.getFrameTree";
const CDP_ATTACHED = "Target.attachedToTarget";
const CDP_DETACHED = "Target.detachedFromTarget";
/** The worker global the record lives in. */
const RECORD = "__journeyNestedFrames";
// The session of every frame tree the extension reads, through the rig's command hook, and every
// session Chrome announces or ends, through a listener of the journey's own on chrome.debugger.
const START_RECORD = `(() => {
  const record = globalThis.${RECORD} = { reads: [], sessions: [] };
  globalThis.${BEFORE_COMMAND_HOOK} = async (target, method) => {
    if (method === ${JSON.stringify(CDP_FRAME_TREE)}) record.reads.push(target?.sessionId ?? null);
  };
  chrome.debugger.onEvent.addListener((source, method, params) => {
    if (method === ${JSON.stringify(CDP_ATTACHED)}) {
      record.sessions.push({ attached: true, session: params.sessionId, through: source.sessionId ?? null, url: params.targetInfo?.url ?? "" });
    } else if (method === ${JSON.stringify(CDP_DETACHED)}) {
      record.sessions.push({ attached: false, session: params.sessionId, through: source.sessionId ?? null });
    }
  });
  return true;
})()`;
const FORGET_READS = `(globalThis.${RECORD}.reads.length = 0, true)`;

/** The child sessions Chrome announced and has not ended, each with the session that announced it. */
function liveSessions(sessions) {
  const live = new Map();
  for (const entry of sessions) {
    if (entry.attached) live.set(entry.session, entry);
    else live.delete(entry.session);
  }
  return [...live.values()];
}

// ---- pictures ----

const PNG = "image/png";
/** How far a channel may be from the frame's color: JPEG moves a flat color by a few steps. */
const COLOR_TOLERANCE = 48;
/** How far, in picture pixels, the color's edge may be from the frame's edge. */
const EDGE_TOLERANCE_PX = 4;
/** The share of the frame's box its color must cover; the nested frame takes a little of it. */
const MIN_COVERAGE = 0.8;
/** How many captures the extension is asked for. */
const CAPTURES = 5;
/** Time the screen has to show the frame at its full size before a capture. */
const SETTLE_MS = 20_000;
/** Time the view has to hold still around one capture. */
const STILL_MS = 30_000;
/** How far, in CSS pixels, two readings of one view may differ: a fractional device scale rounds them apart. */
const VIEW_TOLERANCE_CSS = 1;
/** Time the nested page has to load its three documents. */
const DOCUMENTS_MS = 20_000;

// Where a color shows in an encoded picture: its bounding box and its pixel count. A page decodes
// the picture and reads it back from a canvas; Node has no image decoder.
const MEASURE_COLOR = `(async ({ mime, data, color, tolerance }) => {
  const bytes = Uint8Array.from(atob(data), (character) => character.charCodeAt(0));
  const bitmap = await createImageBitmap(new Blob([bytes], { type: mime }));
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  context.drawImage(bitmap, 0, 0);
  const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
  const seen = { width: bitmap.width, height: bitmap.height, count: 0, left: bitmap.width, top: bitmap.height, right: 0, bottom: 0 };
  for (let y = 0; y < bitmap.height; y += 1) {
    for (let x = 0; x < bitmap.width; x += 1) {
      const at = (y * bitmap.width + x) * 4;
      if (color.some((channel, index) => Math.abs(pixels[at + index] - channel) > tolerance)) continue;
      seen.count += 1;
      seen.left = Math.min(seen.left, x);
      seen.top = Math.min(seen.top, y);
      seen.right = Math.max(seen.right, x + 1);
      seen.bottom = Math.max(seen.bottom, y + 1);
    }
  }
  return seen;
})`;

// The frame element's box in page space, and the visual viewport a plain capture pictures.
const FRAME_BOX = `(() => {
  const box = document.getElementById(${JSON.stringify(FRAME_ID)}).getBoundingClientRect();
  return {
    left: box.left + scrollX, top: box.top + scrollY, right: box.right + scrollX, bottom: box.bottom + scrollY,
    view: { x: visualViewport.pageLeft, y: visualViewport.pageTop, width: visualViewport.width, height: visualViewport.height }
  };
})()`;

/** Where a page-space box falls in a picture `{ width, height }` of the view `{ x, y, width, height }`. */
function inPicture(box, view, picture) {
  const scaleX = picture.width / view.width;
  const scaleY = picture.height / view.height;
  return {
    left: (box.left - view.x) * scaleX, top: (box.top - view.y) * scaleY,
    right: (box.right - view.x) * scaleX, bottom: (box.bottom - view.y) * scaleY
  };
}

/** The view a capture receipt says its picture shows. */
const statedView = (facts) => ({ x: facts.view.visual_page_x, y: facts.view.visual_page_y, width: facts.view.visual_css_width, height: facts.view.visual_css_height });

/** Whether two readings `{ x, y, width, height }` are of one view. */
const sameView = (left, right) => ["x", "y", "width", "height"].every((side) => Math.abs(left[side] - right[side]) <= VIEW_TOLERANCE_CSS);

/** Whether the color reaches every edge of `expected` and covers most of it. */
function fills(seen, expected) {
  const area = (expected.right - expected.left) * (expected.bottom - expected.top);
  return ["left", "top", "right", "bottom"].every((edge) => Math.abs(seen[edge] - expected[edge]) <= EDGE_TOLERANCE_PX)
    && seen.count >= MIN_COVERAGE * area;
}

const extent = (seen) => seen.count === 0 ? "nowhere"
  : `over ${seen.right - seen.left} x ${seen.bottom - seen.top} px at (${seen.left}, ${seen.top})`;
const boxOf = (expected) => `${Math.round(expected.right - expected.left)} x ${Math.round(expected.bottom - expected.top)} px at (${Math.round(expected.left)}, ${Math.round(expected.top)})`;

// ---- evidence and reporting ----

/** Keeps one piece of failure evidence where the hardening suite uploads it; nothing without the folder. */
function keep(name, content) {
  const folder = process.env[EVIDENCE_VARIABLE];
  if (!folder) return;
  mkdirSync(folder, { recursive: true });
  writeFileSync(join(folder, name), content);
}

/** `until`, with what `detail()` says added to its timeout. */
async function untilOrSay(test, label, timeout, detail) {
  try {
    return await until(test, label, timeout);
  } catch (error) {
    if (error.message !== `Timed out: ${label}`) throw error;
    throw new Error(`${error.message}: ${detail()}`);
  }
}

const broken = [];
// One promise fails without hiding what the other does: each is reported, and the journey fails
// at its end when any did not hold.
async function promise(name, check) {
  try {
    await check();
    console.log(`PASS ${name}`);
  } catch (error) {
    broken.push(name);
    console.error(`FAIL ${name}\n${error?.stack ?? error}`);
  }
}

// ---- the journey ----

const service = new ServiceStandIn();
const manifest = extensionManifest();
let rig;
let browser = "no browser";
try {
  await new Promise((done) => fixture.listen(0, LOOPBACK, done));
  const port = fixture.address().port;
  const url = (host, path) => `${origin(host, port)}${path}`;
  const nested = [url(SITE.TOP, PATH.TOP), url(SITE.APP, PATH.APP), url(SITE.INNER, PATH.INNER)];
  const plain = url(SITE.TOP, PATH.PLAIN);
  console.log(`Extension under test: ${EXTENSION_DIRECTORY} (${manifest.version})`);

  rig = await BrowserRig.start({ home: service, label: LABEL, hostRules: HOST_RULES });
  browser = rig.version;
  assert.equal(service.hellos[0]?.protocol, adapter.protocol, JSON.stringify(service.hellos));
  await rig.worker(START_RECORD);
  const record = () => rig.worker(`globalThis.${RECORD}`);
  // Pictures are decoded in the browser's first tab, apart from the page they show.
  const decoder = await rig.page(await rig.physicalTab(BLANK_URL));
  const measure = (mime, data) => decoder.evaluate(`${MEASURE_COLOR}(${JSON.stringify({ mime, data, color: FRAME_COLOR, tolerance: COLOR_TOLERANCE })})`);

  // A workspace's first tab comes into view, as the service asks for it; a window left behind
  // others may not be drawn at all.
  const opened = await service.request(REQUEST.OPEN_TAB, { url: nested[0], focus_window: true, ready_timeout_ms: READY_TIMEOUT_MS });
  assert.ok(opened.ok, JSON.stringify(opened));
  const tabId = opened.ok.tab.tab_id;
  const tab = await rig.page(tabId);
  const frames = () => service.request(REQUEST.FRAMES, { tab_id: tabId });
  const documents = (receipt) => (receipt.ok?.frames ?? []).map((frame) => frame.url).sort();
  let listed = null;
  await untilOrSay(async () => {
    listed = await frames();
    return listed.ok?.complete === true && listed.ok.unseen.length === 0 && documents(listed).join() === [...nested].sort().join();
  }, "a complete frames receipt of the three nested documents", DOCUMENTS_MS, () => JSON.stringify(listed));

  await promise(CAPTURE_PROMISE, async () => {
    // The frame as the screen shows it now. The journey's own capture names no clip, so Chrome
    // leaves the view alone.
    const onScreen = async () => {
      const box = await tab.evaluate(FRAME_BOX);
      const seen = await measure(PNG, (await tab.send("Page.captureScreenshot", { format: "png" })).data);
      return { box, seen, expected: inPicture(box, box.view, seen) };
    };
    // The frame's box once the screen shows the frame over all of it.
    const fullOnScreen = async () => {
      let screen = null;
      await untilOrSay(async () => {
        screen = await onScreen();
        return fills(screen.seen, screen.expected);
      }, "the app frame fills its box on screen", SETTLE_MS,
      () => `the screen shows its color ${extent(screen.seen)}; its box is ${boxOf(screen.expected)}`);
      return screen.box;
    };
    for (let shot = 1; shot <= CAPTURES; shot += 1) {
      // A capture counts when the view held still around it. The browser's own bars are still
      // sliding in when a tab has just opened, and they move the view under a measured box.
      let views = null;
      const { box, receipt } = await untilOrSay(async () => {
        const before = await fullOnScreen();
        const taken = await service.request(REQUEST.CAPTURE_VIEWPORT, { tab_id: tabId });
        assert.ok(taken.ok, JSON.stringify(taken));
        const after = await tab.evaluate(FRAME_BOX);
        views = { before: before.view, stated: statedView(taken.ok), after: after.view };
        return sameView(views.before, views.after) && sameView(views.after, views.stated) ? { box: after, receipt: taken } : null;
      }, "a capture of a view that held still", STILL_MS, () => JSON.stringify(views));
      assert.ok(box.right - box.left > FRAME_MIN.width && box.bottom - box.top > FRAME_MIN.height,
        `the frame must be larger than its CSS minimum for a squeeze to show: ${JSON.stringify(box)}`);
      const { data, mime_type: mime, ...facts } = receipt.ok;
      const seen = await measure(mime, data);
      const expected = inPicture(box, statedView(facts), facts);
      try {
        assert.deepEqual([seen.width, seen.height], [facts.width, facts.height], "the receipt states the size of its picture");
        assert.ok(fills(seen, expected),
          `capture ${shot} of ${CAPTURES} shows the frame's color ${extent(seen)}; its box is ${boxOf(expected)} of a ${facts.width} x ${facts.height} px picture`);
      } catch (error) {
        keep(`capture-${shot}.jpg`, Buffer.from(data, "base64"));
        keep(`capture-${shot}.json`, JSON.stringify({ facts, box, expected, seen }, null, 2));
        throw error;
      }
    }
  });

  await promise(LEAVING_PROMISE, async () => {
    const before = liveSessions((await record()).sessions);
    // The premise: the nested page gave the extension a session that another session announced.
    assert.ok(before.length >= 2, `the nested page must hold two child sessions before the tab leaves it: ${JSON.stringify(before)}`);
    assert.ok(before.some((entry) => before.some((parent) => parent.session === entry.through)),
      `one child session must be announced through another: ${JSON.stringify(before)}`);
    const to = { ...golden("requests", REQUEST.NAVIGATE_URL).primitive.args.to, url: plain };
    const left = await service.request(REQUEST.NAVIGATE_URL, { tab_id: tabId, to, ready_timeout_ms: READY_TIMEOUT_MS });
    assert.equal(left.ok?.tab.url, plain, JSON.stringify(left));
    const first = await frames();
    await rig.worker(FORGET_READS);
    const further = await frames();
    const after = await record();
    // What a frames receipt says, or the whole receipt when it is a failure.
    const told = (receipt) => receipt.ok
      ? { complete: receipt.ok.complete, documents: documents(receipt), unseen: receipt.ok.unseen }
      : receipt;
    const whole = { complete: true, documents: [plain], unseen: [] };
    try {
      assert.ok(after.reads.includes(null), `the further frames request must read the tab's own session: ${JSON.stringify(after.reads)}`);
      // One comparison, so a failure names every part that does not hold. The page now shown has
      // no frame, so a child session the further request read is one that ended with the old page.
      assert.deepEqual(
        { first_receipt: told(first), further_receipt: told(further), child_sessions_the_further_request_read: after.reads.filter((session) => session !== null) },
        { first_receipt: whole, further_receipt: whole, child_sessions_the_further_request_read: [] },
        `after the tab left its nested frames for ${plain}`);
    } catch (error) {
      keep("sessions.json", JSON.stringify({ before, sessions: after.sessions, first, further, further_reads: after.reads }, null, 2));
      throw error;
    }
  });

  if (broken.length === 0) {
    console.log(`Nested frame browser journey: 2 promises held with ${browser}; extension ${manifest.version} -> Chromium CDP, with the service side of adapter protocol 4 played in process.`);
  }
} finally {
  try {
    await rig?.stop();
  } finally {
    await new Promise((done) => { fixture.closeAllConnections(); fixture.close(done); });
  }
}
assert.deepEqual(broken, [], `the extension under test (${manifest.version}, ${EXTENSION_DIRECTORY}) does not keep every promise with ${browser}`);
