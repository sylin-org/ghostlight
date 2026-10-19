"use strict";

// Drives the golden protocol 4 requests through the dispatcher over the real modules (sessions,
// inventory, world, input, tabs, capture, evaluate, buffers) and a scripted browser, and compares
// the whole receipt frame with its golden fixture: ok or failure, dispatched, committed urls and the
// tab afterwards. Nothing here stubs a handler. The scripts below put the browser into the state
// each fixture describes: a guard that no longer matches, a refused attach, a tab that is gone, a
// dialog, a cancel in the middle of a batch, a deadline, a page answer, an encoder reply.
//
// A receipt message is advisory text (spec 1.3). Where the extension forwards the browser's own
// words, or writes words of its own that differ from the fixture's sample, the case names the
// message it expects and the rest of the frame is still compared whole.

const test = require("node:test");
const assert = require("node:assert/strict");
const World = require("../lib/world.js");
const { ROOT, node, fakeChrome, assets } = require("./p4-fakes.js");
const { golden } = require("./p4-chrome.js");
const { TAB, URL, ACCOUNT_TAB, names, wireFrame, gate, rig } = require("./p4-rig.js");

const ACCOUNT = "2F7A9C41D0E85B36A1C4F290E7B3D568";
const ACCOUNT_LOADER = "9B04E6A3C1D8F25747A0B9E3C6D1F482";
const EMBED = "6D1C84E9A7F203B5C9E4D7A1B82F0C36";
const EMBED_LOADER = "C7E2A9F5031B8D64E9A0C3F7D2B16E85";
const EMBED_URL = "https://widgets.example.net/embed";
const ADS_URL = "https://ads.example.org/slot";
const MOVED_LOADER = "0000000000000000000000000000FFFF";

// What the extension says when the guard of a request no longer matches, and what Chrome says when
// the debugger cannot attach or has gone.
const GUARD_MESSAGE = "The page changed since it was read.";
const ATTACH_REFUSED = "Cannot attach to this target.";
const ATTACH_TAKEN = "Another debugger is already attached to the tab with id: 12.";
const DETACHED = "Detached while handling command.";

// The recording the fixtures describe is called rec_41c7. The store mints ids of the form
// recording_<token> and refuses any other, so the case asks for the minted id and the receipt is
// mapped back. The id is opaque to the service.
const MINTED_ID = "recording_41c7";
const SAMPLE_ID = "rec_41c7";

const requests = names("requests").map((stem) => golden(`requests/${stem}`));
const requestOf = (correlation) => requests.find((entry) => entry.correlation === correlation);

// ---- the browser the fixtures describe --------------------------------------------------------

/**
 * The fixtures' page: an account document with one same-process embedded document. `ads` adds a
 * third document that no debugger session reports. `topLoader` moves the top document on.
 */
function pageChrome({ ads = false, topLoader = ACCOUNT_LOADER } = {}) {
  const tree = node(ACCOUNT, topLoader, URL, null, [node(EMBED, EMBED_LOADER, EMBED_URL, ACCOUNT)]);
  const frames = [
    { frameId: 0, documentLifecycle: "active", url: URL },
    { frameId: 1, documentLifecycle: "active", url: EMBED_URL }
  ];
  if (ads) frames.push({ frameId: 2, documentLifecycle: "active", url: ADS_URL });
  return fakeChrome({ trees: { [ROOT]: tree }, frames });
}

const returns = (envelope) => ({ result: { value: envelope } });
const okValue = (value) => returns({ ok: true, value });

/** Scripts the page: `answer(message, token, params)` is the raw CDP reply to each world call. */
function scriptPage(chrome, answer) {
  chrome.handlers["Runtime.callFunctionOn"] = async (target, params) => {
    if (params.functionDeclaration !== World.SHIMS.CALL) return { result: { value: true } };
    const [, message, , token] = params.arguments.map((entry) => entry.value);
    return answer(message, token, params);
  };
}

/** Scripts a method so it answers per call: `handle(callNumber, params)`. */
function scriptMethod(chrome, method, handle) {
  let count = 0;
  chrome.handlers[method] = async (_target, params) => handle(++count, params);
}

const refuseAttach = (message) => async () => { throw new Error(message); };

/** The recording the fixtures describe: 18 frames holding 912340 bytes, started at `startedAt`. */
function recordedPage({ startedAt, now }) {
  const wall = { time: startedAt };
  return {
    wall,
    recordingOptions: { now: () => wall.time, newId: () => MINTED_ID },
    seed(parts) {
      parts.recording.start(TAB, URL);
      const sizes = [...Array(17).fill(50000), 62340];
      sizes.forEach((size, index) => {
        wall.time = startedAt + index * 230;
        parts.recording.append(TAB, Buffer.alloc(size, index + 1).toString("base64"), index === 0 ? "seed" : "screencast", wall.time);
      });
      wall.time = now;
    },
    freeze(parts) {
      const selected = parts.recording.beginStop(MINTED_ID);
      parts.recording.finishStop(selected.state, "explicit");
    }
  };
}

const withRecordingId = (args) => ({ ...args, recording_id: MINTED_ID });
const GIF = {
  ok: true,
  measurements: { frame_count: 18, captured_frame_count: 18, duration_ms: 4200, width: 1280, height: 720, byte_count: 734002 },
  mime_type: "image/gif",
  data: "R0lGODlhAQABAAAAACw="
};

// ---- the cases --------------------------------------------------------------------------------
//
// Each case is keyed by receipt fixture. Fields: `chrome` builds the scripted debugger, `tabs` the
// tabs, `attach` replaces chrome.debugger.attach, `recordingOptions` shapes the recording store,
// `args` rewrites the golden request's arguments, `arrange(parts)` scripts the browser, `drive`
// replaces the plain run for cases that need to act while the primitive is in flight, `message`
// is the receipt message the extension is expected to produce when it is not the fixture's.

const assetsReady = (parts) => parts.world.setAssets(assets());

const ACCOUNT_BOX = { rectangle: { x: 10, y: 20, width: 100, height: 40 }, subject: { role: "button", name: "Save" } };

function callScript(parts) {
  scriptPage(parts.chrome, (message) => {
    if (message.kind === "box") return okValue(ACCOUNT_BOX);
    return okValue({ activated: true });
  });
}

// Five parallel-looking calls resolve in the shapes of call.ok_mixed: the race ends at the first
// ok answer, and the call still waiting is skipped.
function mixedRace() {
  const waiting = gate();
  return {
    waiting,
    args: (args) => ({ ...args, mode: "race", timeout_ms: 6000, calls: Array.from({ length: 5 }, () => args.calls[0]) }),
    arrange: async (parts) => {
      await assetsReady(parts);
      scriptPage(parts.chrome, async (_message, token) => {
        const index = Number(token.split(":").at(-1));
        if (index === 0) {
          await new Promise((resolve) => setTimeout(resolve, 20));
          return okValue(ACCOUNT_BOX);
        }
        if (index === 1) return returns({ ok: false, error: { code: "stale_locator", message: "stale browser locator" } });
        if (index === 2) throw new Error("Cannot find context with specified id");
        if (index === 3) return { exceptionDetails: { exception: { description: "TypeError: x is not a function" } } };
        await waiting.promise;
        return okValue(ACCOUNT_BOX);
      });
    }
  };
}

const mixed = mixedRace();
const exportedPage = recordedPage({ startedAt: 1790999995800, now: 1791000000000 });
const exportDocumentChanged = recordedPage({ startedAt: 1790999995800, now: 1791000000000 });
const runningRecording = () => recordedPage({ startedAt: 1790999880000, now: 1790999884200 });
const stoppedRecording = recordedPage({ startedAt: 1790999995800, now: 1791000000000 });
const statusRecording = runningRecording();
const existingRecording = runningRecording();
const discardRecording = runningRecording();

function exportArrange(page) {
  return async (parts) => {
    await assetsReady(parts);
    scriptPage(parts.chrome, () => okValue({ attached: true }));
    parts.encoder.reply = () => GIF;
    page.seed(parts);
    page.freeze(parts);
  };
}

const CASES = {
  // ---- frames
  "frames.ok": { chrome: () => pageChrome({ ads: true }) },
  "frames.tab_missing": { attach: refuseAttach("No tab with id: 12.") },
  "frames.debugger_unavailable": { attach: refuseAttach(ATTACH_REFUSED), message: ATTACH_REFUSED },

  // ---- call
  "call.ok": { arrange: async (parts) => { await assetsReady(parts); callScript(parts); } },
  "call.ok_mixed": {
    args: mixed.args,
    arrange: mixed.arrange,
    drive: async (parts, start) => {
      try { await start(); } finally { mixed.waiting.open(); }
    }
  },
  "call.cancelled": {
    arrange: async (parts) => {
      await assetsReady(parts);
      scriptPage(parts.chrome, () => {
        parts.primitives.cancel("p4_000f");
        return okValue(ACCOUNT_BOX);
      });
    }
  },
  "call.document_changed": {
    chrome: () => pageChrome({ topLoader: MOVED_LOADER }),
    arrange: assetsReady,
    message: GUARD_MESSAGE
  },
  "call.debugger_unavailable": { attach: refuseAttach(ATTACH_REFUSED), message: ATTACH_REFUSED },
  "call.session_lost": {
    arrange: async (parts) => {
      await assetsReady(parts);
      callScript(parts);
      // The pages answered; Chrome then loses the debugger as the lease is given back.
      parts.chrome.detach = async () => { throw new Error("Debugger is not attached to the tab with id: 12."); };
    },
    message: "Debugger is not attached to the tab with id: 12."
  },

  // ---- input
  "input.ok": {},
  "input.ok_precondition": {
    arrange: async (parts) => {
      await assetsReady(parts);
      scriptPage(parts.chrome, () => okValue({ focused: true }));
    }
  },
  "input.ok_precondition_failed": {
    arrange: async (parts) => {
      await assetsReady(parts);
      scriptPage(parts.chrome, () => returns({ ok: false, error: { code: "focus_lost", message: "focus moved before typing" } }));
    }
  },
  "input.ok_dialog_stop": {
    arrange: (parts) => {
      // The third event blocks the renderer: a dialog opens and the command never answers.
      scriptMethod(parts.chrome, "Input.dispatchMouseEvent", (count) => {
        if (count < 3) return {};
        parts.world.announceDialog(TAB, "alert");
        return new Promise(() => {});
      });
    }
  },
  "input.ok_cancelled_stop": {
    arrange: (parts) => {
      scriptMethod(parts.chrome, "Input.dispatchMouseEvent", () => {
        parts.primitives.cancel("p4_0013");
        return {};
      });
    }
  },
  "input.ok_native_drag": {
    arrange: (parts) => {
      scriptMethod(parts.chrome, "Input.dispatchMouseEvent", (_count, params) => {
        // Chrome intercepts the drag as soon as the pressed button moves.
        if (params.type === "mouseMoved" && params.buttons === 1) parts.input.dragIntercepted(TAB, { items: [], dragOperationsMask: 1 });
        return {};
      });
    }
  },
  "input.document_changed": { chrome: () => pageChrome({ topLoader: MOVED_LOADER }), message: GUARD_MESSAGE },
  "input.invalid_request": {
    args: (args) => ({ ...args, steps: [args.steps[0], { step: "pause", ms: 1001 }] }),
    message: "steps[1].ms must be a whole number from 0 to 1000."
  },
  "input.session_lost": {
    arrange: (parts) => {
      scriptMethod(parts.chrome, "Input.dispatchMouseEvent", (count) => {
        if (count === 2) throw new Error(DETACHED);
        return {};
      });
    },
    message: DETACHED
  },
  "input.timeout": {
    arrange: (parts) => {
      // The first event takes the whole budget.
      scriptMethod(parts.chrome, "Input.dispatchMouseEvent", () => {
        parts.advance(5000);
        return {};
      });
    }
  },

  // ---- evaluate_main
  "evaluate_main.ok": {
    arrange: (parts) => { parts.chrome.handlers["Runtime.evaluate"] = async () => ({ result: { value: "Account" } }); }
  },
  "evaluate_main.ok_truncated": {
    args: (args) => ({ ...args, max_result_chars: 17 }),
    arrange: (parts) => { parts.chrome.handlers["Runtime.evaluate"] = async () => ({ result: { value: "a".repeat(40) } }); }
  },
  "evaluate_main.document_changed": { chrome: () => pageChrome({ topLoader: MOVED_LOADER }), message: GUARD_MESSAGE },
  "evaluate_main.invalid_request": {
    args: (args) => ({ ...args, script: "document.title)" }),
    message: "Unexpected token (1:14)"
  },
  "evaluate_main.timeout": {
    drive: async (parts, start, t) => {
      parts.chrome.handlers["Runtime.evaluate"] = () => new Promise(() => {});
      t.mock.timers.enable({ apis: ["setTimeout"] });
      const running = start();
      while (!parts.chrome.calls.some((call) => call.method === "Runtime.evaluate")) await new Promise((resolve) => setImmediate(resolve));
      t.mock.timers.tick(5000);
      await running;
    }
  },

  // ---- diagnostics
  "diagnostics.document_changed": { chrome: () => pageChrome({ topLoader: MOVED_LOADER }), message: GUARD_MESSAGE },

  // ---- recording
  "recording.ok_existing": {
    recordingOptions: existingRecording.recordingOptions,
    arrange: (parts) => existingRecording.seed(parts),
    ids: true
  },
  "recording.ok_status": {
    recordingOptions: statusRecording.recordingOptions,
    arrange: (parts) => statusRecording.seed(parts),
    ids: true
  },
  "recording.ok_stopped": {
    recordingOptions: stoppedRecording.recordingOptions,
    arrange: (parts) => stoppedRecording.seed(parts),
    ids: true
  },
  "recording.ok_discarded": {
    recordingOptions: discardRecording.recordingOptions,
    arrange: (parts) => discardRecording.seed(parts),
    ids: true
  },
  "recording.ok_exported_page": {
    recordingOptions: exportedPage.recordingOptions,
    arrange: exportArrange(exportedPage),
    ids: true
  },
  "recording.document_changed": {
    chrome: () => pageChrome({ topLoader: MOVED_LOADER }),
    recordingOptions: exportDocumentChanged.recordingOptions,
    arrange: exportArrange(exportDocumentChanged),
    ids: true,
    message: GUARD_MESSAGE
  },
  "recording.tab_missing": { tabs: [] },
  "recording.debugger_unavailable": { attach: refuseAttach(ATTACH_REFUSED), message: ATTACH_REFUSED },

  // ---- tabs
  "navigate.other": {
    tabs: [{ ...ACCOUNT_TAB, url: "https://example.com/", title: "Example Domain" }],
    arrange: (parts) => {
      parts.browser.tabs.update = async () => { throw new Error("The browser reported an unclassified error."); };
    }
  },
  "navigate.debugger_unavailable": { attach: refuseAttach(ATTACH_TAKEN), message: ATTACH_TAKEN },
  "open_tab.tab_missing": {},

  // ---- view, capture, dialog and accessibility snapshot
  "view.debugger_unavailable": { attach: refuseAttach(ATTACH_REFUSED), message: ATTACH_REFUSED },
  "capture.debugger_unavailable": { attach: refuseAttach(ATTACH_REFUSED), message: ATTACH_REFUSED },
  "dialog.debugger_unavailable": { attach: refuseAttach(ATTACH_REFUSED), message: ATTACH_REFUSED },
  "accessibility_snapshot.debugger_unavailable": { attach: refuseAttach(ATTACH_REFUSED), message: ATTACH_REFUSED }
};

// A case with `ids` maps the minted id back in the receipt, and into the request: the golden
// request names rec_41c7, the store only knows recording_41c7.
function requestArgs(spec, base) {
  const args = structuredClone(base);
  const mapped = spec.ids && args.recording_id === SAMPLE_ID ? withRecordingId(args) : args;
  return spec.args ? spec.args(mapped) : mapped;
}

/** Compares a produced receipt with its fixture, whole. See the header for what `message` means. */
function assertReceipt(stem, frame, spec) {
  let produced = wireFrame(frame);
  if (spec.ids) produced = JSON.parse(JSON.stringify(produced).replaceAll(MINTED_ID, SAMPLE_ID));
  const expected = golden(`receipts/${stem}`);
  if (spec.message !== undefined) {
    assert.equal(produced.message, spec.message, `${stem}: the message`);
    delete produced.message;
    delete expected.message;
  }
  assert.deepEqual(produced, expected, stem);
}

for (const [stem, spec] of Object.entries(CASES)) {
  test(`${stem}: the real modules build the golden receipt`, async (t) => {
    const fixture = golden(`receipts/${stem}`);
    const { primitive } = requestOf(fixture.correlation);
    assert.equal(stem.split(".")[0], primitive.name, "the fixture answers the request that shares its correlation");
    const parts = rig({
      chrome: spec.chrome ? spec.chrome() : pageChrome(),
      tabs: spec.tabs,
      attach: spec.attach,
      recordingOptions: spec.recordingOptions
    });
    await spec.arrange?.(parts);
    const start = () => parts.primitives.run(fixture.correlation, { name: primitive.name, args: requestArgs(spec, primitive.args) });
    await (spec.drive ? spec.drive(parts, start, t) : start());
    assert.equal(parts.frames.length, 1, "exactly one receipt");
    assertReceipt(stem, parts.frames[0], spec);
  });
}

// ---- fixtures the extension cannot produce ----------------------------------------------------

test("every case names a receipt fixture", () => {
  const stems = new Set(names("receipts"));
  for (const stem of Object.keys(CASES)) assert.ok(stems.has(stem), stem);
});

test("input.cancelled is the service's cancel shape: the input module ends a cancelled batch with a stop, not a failure", async () => {
  // A cancel frame reaches the input module between steps, where it stops the batch and reports
  // `stopped: cancelled` in an ok outcome (input.ok_cancelled_stop). It never throws `cancelled`
  // after dispatching, so no scripted browser makes the extension send input.cancelled; the
  // fixture exists for the service's decoder, which must read a `cancelled` failure that carries
  // dispatched effects and a tab.
  const fixture = golden("receipts/input.cancelled");
  assert.equal(fixture.failure, "cancelled");
  assert.equal(fixture.dispatched, true);
  assert.ok(fixture.tab);
  const stop = golden("receipts/input.ok_cancelled_stop");
  assert.deepEqual(stop.ok.stopped, { reason: "cancelled" });
});
