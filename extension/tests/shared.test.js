"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { createHash } = require("node:crypto");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const shared = require("../lib/shared.js");
const state = require("../lib/state.js");
const topology = require("../lib/topology.js");
const presentationQueue = require("../lib/presentation-queue.js");
require("../lib/presentation.js");
const presentation = globalThis.GhostlightPresentation;

test("credential metadata is classified without inspecting values", () => {
  assert.equal(shared.isCredentialMetadata({ type: "password" }), true);
  assert.equal(shared.isCredentialMetadata({ autocomplete: "current-password" }), true);
  assert.equal(shared.isCredentialMetadata({ name: "otp_code" }), true);
  assert.equal(shared.isCredentialMetadata({ type: "text", name: "display_name" }), false);
});

test("a plain primary click plans the native dispatch", () => {
  assert.deepEqual(shared.activationPlan({ button: "primary", click_count: 1, modifiers: [] }), { native: true, clicks: [] });
});

test("modified and repeated clicks plan synthetic events with the right geometry", () => {
  const double = shared.activationPlan({ button: "primary", click_count: 2, modifiers: [] });
  assert.equal(double.native, false);
  assert.equal(double.clicks.length, 2);
  for (const init of double.clicks) {
    assert.equal(init.button, 0);
    assert.equal(init.detail, 2);
    assert.equal(init.bubbles, true);
    assert.equal(init.cancelable, true);
    assert.equal(init.composed, true);
  }
  const right = shared.activationPlan({ button: "right", click_count: 1, modifiers: [] });
  assert.deepEqual(right.clicks, [{ bubbles: true, cancelable: true, composed: true, button: 2, detail: 1, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false }]);
  const middle = shared.activationPlan({ button: "middle", click_count: 3, modifiers: [] });
  assert.equal(middle.clicks.length, 3);
  assert.ok(middle.clicks.every((init) => init.button === 1 && init.detail === 3));
});

test("modifier keys ride every planned synthetic click", () => {
  const plan = shared.activationPlan({ button: "primary", click_count: 1, modifiers: ["Control", "Shift"] });
  assert.equal(plan.native, false);
  assert.deepEqual(plan.clicks, [{ bubbles: true, cancelable: true, composed: true, button: 0, detail: 1, ctrlKey: true, metaKey: false, shiftKey: true, altKey: false }]);
});

test("presentation labels are fixed and content-free", () => {
  assert.equal(shared.presentationLabel("start"), "Ghostlight starting");
  assert.equal(shared.presentationLabel("attention"), "Ghostlight needs you");
  assert.equal(shared.presentationLabel("page secret"), "Ghostlight");
  assert.equal(shared.activityLabel("read"), "Reading page");
  assert.equal(shared.activityLabel("script"), "Running JavaScript");
  assert.equal(shared.activityLabel("page secret"), "Ghostlight");
});

test("presentation preserves the established Ghostlight palette and motion", () => {
  assert.deepEqual(presentation.visualIdentity, {
    sky: "#38bdf8",
    ink: "#eaf6ff",
    ground: "#0c0f14",
    spring: "cubic-bezier(.22,1,.36,1)",
    cursor_ms: 150,
    border_breathe_ms: 4000,
    ripple_ms: 620,
    field_splash_ms: 700,
    read_scan_ms: 1450,
    navigation_ms: 1600,
    screenshot_ms: 1500,
    zoom_ms: 1150,
    denial_ms: 5000
  });
  const root = join(__dirname, "..");
  const renderer = readFileSync(join(root, "lib", "presentation.js"), "utf8");
  const chrome = readFileSync(join(root, "ui.css"), "utf8");
  assert.match(renderer, /M0 0 L0 19 L5 14\.5 L8\.2 22/);
  assert.match(renderer, /ghostlight-capframe 1500ms cubic-bezier\(\.5,0,\.2,1\)/);
  assert.match(renderer, /managed && runtimeReachable && !recordingActive/);
  assert.match(renderer, /function setRecording\(value\)/);
  assert.match(renderer, /host\.style\.display = hiddenForTool \? "none" : "block"/);
  assert.match(renderer, /role", "status"/);
  assert.match(renderer, /aria-live", "polite"/);
  assert.match(renderer, /aria-atomic", "true"/);
  assert.match(renderer, /\.denial-ribbon\{animation:none!important\}/);
  // The guardrail must rise, not squash: scaling a flex row vertically deforms its badge.
  assert.doesNotMatch(renderer, /ghostlight-notif-grow\{0%\{opacity:0;transform:scaleY\(0\)\}/);
  // One ring per click, dashed for a secondary button (tool-visual-signatures.md).
  assert.match(renderer, /\.ripple\.secondary\{border-style:dashed\}/);
  assert.match(renderer, /const clicks = Math\.min\(3, Math\.max\(1, Number\(shape && shape\.clicks\) \|\| 1\)\)/);
  assert.match(renderer, /index \* CLICK_STAGGER_MS/);
  // The read scan must reach zero, or an interrupted sweep leaves a lit bar.
  assert.doesNotMatch(renderer, /100%\{opacity:\.85;transform:translateY\(100vh\)\}/);

  // Identity reaches the stylesheet once as custom properties; the vocabulary below is static
  // CSS, so a colour or curve changes in exactly one place.
  assert.match(renderer, /:host\{all:initial;\$\{TOKENS\}\}/);
  assert.match(renderer, /--gl-sky:\$\{SKY\};--gl-argb:\$\{SKY_RGB\}/);
  const stylesheet = renderer.slice(renderer.indexOf("style.textContent = `"), renderer.indexOf("`;", renderer.indexOf("style.textContent = `")));
  const interpolations = stylesheet.match(/\$\{[A-Z_]+\}/g) || [];
  assert.deepEqual(
    [...new Set(interpolations)].sort(),
    ["${REDUCED_FADE_SELECTOR}", "${TOKENS}"],
    "the stylesheet must stay static apart from its tokens and generated reduced-motion list"
  );

  // Reduced-motion coverage is generated from the registry, so a new effect cannot silently
  // keep animating for someone who asked it not to.
  assert.match(renderer, /\$\{REDUCED_FADE_SELECTOR\}\{animation-name:ghostlight-fade!important/);
  assert.doesNotMatch(renderer, /\.trail-dot,\.field-shimmer,\.field-splash,\.target-glow/);
  const registrySource = renderer.slice(renderer.indexOf("const TRANSIENT_EFFECTS"), renderer.indexOf("REDUCED_FADE_SELECTOR"));
  const registry = [...registrySource.matchAll(/"([a-z- ]+)"/g)].map((match) => match[1]);
  assert.ok(registry.length >= 18, `expected the full transient vocabulary, saw ${registry.length}`);
  for (const name of registry) {
    assert.ok(
      stylesheet.includes(`.${name}{`),
      `${name} is in the effect registry but has no rule in the stylesheet`
    );
  }

  // Teardown is derived from each row's beat, never hand-picked at the call site.
  assert.match(renderer, /setTimeout\(remove, lifetimeFor\(className\)\)/);
  assert.doesNotMatch(renderer, /addEffect\([^)]*,\s*\d+\)/, "an effect lifetime was hand-picked");

  // Every ephemeral effect must own a beat, or it would tear down after the grace alone.
  const withBeat = new Set([...registrySource.matchAll(/(?:effect|selector): "([a-z- ]+)",\s*beat:/g)].map((match) => match[1]));
  const created = [...new Set([...renderer.matchAll(/addEffect\("([a-z-]+)"/g)].map((match) => match[1]))];
  assert.ok(created.length >= 8, `expected the ephemeral call sites, saw ${created.length}`);
  for (const name of created) {
    assert.ok(withBeat.has(name), `addEffect("${name}") has no beat in the effect registry`);
  }
  assert.match(renderer, /setTimeout\(\(\) => denialLayer\.replaceChildren\(\), DENIAL_MS\)/);
  assert.match(chrome, /#0a0e17/);
  assert.match(chrome, /rgba\(56,189,248,\.10\)/);
  assert.match(chrome, /transition: color \.25s, border-color \.25s, background \.25s/);
  assert.doesNotMatch(chrome, /\.save-status/);
});

test("modifier masks match the CDP vocabulary", () => {
  assert.equal(shared.modifierMask(["Alt", "Control", "Shift"]), 11);
  assert.equal(shared.modifierMask([]), 0);
});

test("named keys receive physical CDP codes", () => {
  assert.deepEqual(shared.keyDescriptor("Enter"), { key: "Enter", code: "Enter", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 });
  assert.deepEqual(shared.keyDescriptor("x"), { key: "x", text: "x" });
});

test("drag packets hold the left button through movement and always release", () => {
  const packets = shared.dragPackets({ x: 10, y: 20 }, { x: 30, y: 60 }, 2);
  assert.deepEqual(packets, [
    { type: "mouseMoved", x: 10, y: 20 },
    { type: "mousePressed", x: 10, y: 20, button: "left", clickCount: 1 },
    { type: "mouseMoved", x: 20, y: 40, button: "left", buttons: 1, force: 1 },
    { type: "mouseMoved", x: 30, y: 60, button: "left", buttons: 1, force: 1 },
    { type: "mouseReleased", x: 30, y: 60, button: "left", clickCount: 1 }
  ]);
  assert.throws(() => shared.dragPackets({ x: 0, y: 0 }, { x: 1, y: 1 }, 0), /drag steps/);
});

test("drag execution scopes native interception and keeps drag data inside the worker", () => {
  const worker = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  assert.match(worker, /Input\.setInterceptDrags", \{ enabled: true \}/);
  assert.match(worker, /method === "Input\.dragIntercepted"/);
  assert.match(worker, /for \(const type of \["dragEnter", "dragOver", "drop"\]\)/);
  assert.match(worker, /Input\.dispatchDragEvent/);
  assert.match(worker, /Input\.cancelDragging/);
  assert.doesNotMatch(worker, /params\.data\.(items|files|dragOperationsMask)/);
});

test("browser events use the nested typed bridge envelope", () => {
  assert.deepEqual(shared.browserEventFrame({ event: "tab_closed", tab_id: 7 }), {
    kind: "event",
    event: { event: "tab_closed", tab_id: 7 }
  });
});

test("the adapter advertises stable versioned physical capabilities", () => {
  assert.equal(shared.ADAPTER_PROTOCOL_MAJOR, 2);
  const revisionFor = (name) => ({ script: 2, pointer_input: 2, keyboard_input: 2, semantic_document: 3, files: 2, navigation: 2 }[name] ?? 1);
  assert.deepEqual(shared.ADAPTER_CAPABILITIES, [
    "tabs",
    "atomic_tab_open",
    "navigation",
    "semantic_document",
    "capture",
    "pointer_input",
    "keyboard_input",
    "files",
    "script",
    "observation",
    "dialogs",
    "operation_recovery",
    "presentation",
    "window_geometry",
    "diagnostics",
    "recording",
    "chunked_commands",
    "adapter_liveness",
    "adapter_attention"
  ].map((name) => ({ name, revision: revisionFor(name) })));
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

test("attention is reported on focus and at connection, never inferred from connecting", () => {
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  // Turning to the browser reports attention.
  assert.match(source, /chrome\.windows\.onFocusChanged\.addListener[\s\S]{0,400}?event: "attended"/);
  // Connecting reports the truth about focus rather than claiming it.
  assert.match(source, /attended: await holdsFocusedWindow\(\)/);
  assert.match(source, /chrome\.windows\.getLastFocused\(\)/);
});

test("native-host startup is one connection attempt across concurrent wake signals", () => {
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  assert.match(source, /let nativeConnectionAttempt = null;/);
  assert.match(source, /if \(nativeConnectionAttempt\) return nativeConnectionAttempt;/);
  assert.match(source, /nativeConnectionAttempt = establishNativeConnection\(\)/);
  assert.match(source, /if \(!browserId\) await initializeLocalState\(\);\s+if \(nativePort\) return;/);
  assert.doesNotMatch(source, /initializeLocalState\(\)\s*\.then\(connectNative\)/);
});

test("every native-port disconnect consumes Chrome's callback error", () => {
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  assert.match(
    source,
    /port\.onDisconnect\.addListener\(\(\) => \{\s+const disconnectError = chrome\.runtime\.lastError\?\.message[^;]+;\s+if \(nativePort !== port\) return;/
  );
});

test("a first extension install opens the service-first handoff", () => {
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  assert.match(source, /const SERVICE_INSTALL_URL = "https:\/\/sylin\.org\/ghostlight\/chromium-extension\/post-install\/";/);
  assert.match(source, /if \(details\?\.reason === "install"\) \{\s+chrome\.tabs\.create\(\{ url: SERVICE_INSTALL_URL \}\)/);
  assert.match(source, /chrome\.runtime\.onInstalled\.addListener\(onExtensionInstalled\)/);
});

test("page opening is one atomic physical primitive", () => {
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  assert.match(source, /topology\.open\(command\.url, workspace, command\.group_title/);
  assert.doesNotMatch(source, /chrome\.tabs\.create\(\{ url: "about:blank"/);
  assert.doesNotMatch(source, /command\.command === "create_tab"/);
});

test("debugger attachment lifetime follows controlled tab ownership", () => {
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  assert.match(source, /openedTab = await topology\.open[\s\S]*?await retainManagedDebugger\(openedTab\.id\);/);
  assert.match(source, /await debuggerLifecycle\.retain\(tabId\);/);
  assert.match(source, /chrome\.tabs\.onRemoved[\s\S]*?debuggerLifecycle\.forget\(tabId\);/);
  assert.match(source, /controlState === "ended"[\s\S]*?debuggerLifecycle\.detachAll\(\);/);
});

test("releasing a debugger session never depends on the native connection", () => {
  // A disconnected service leaves the automatic path -- controlState === "ended" above -- unable
  // to fire at all, since it only runs in response to a signal the service has to send. Without
  // a local escape hatch, Chrome's own "controlled by automated software" banner could then
  // outlive the service indefinitely: crashed, uninstalled, or simply never restarted. The fix is
  // this message kind, which must call detachAll() directly and must never be gated behind
  // nativePort the way requestRuntimeControl (used by every other popup action) is.
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  const handler = source.match(
    /"release_debugger_sessions"\)\s*\{([\s\S]*?)\n\s{4}\}/
  );
  assert.ok(handler, "the release_debugger_sessions message handler was not found");
  assert.match(handler[1], /debuggerLifecycle\.detachAll\(\)/);
  assert.doesNotMatch(
    handler[1],
    /nativePort|requestRuntimeControl/,
    "release_debugger_sessions must stay reachable with no live service connection"
  );

  const popup = readFileSync(join(__dirname, "..", "popup.js"), "utf8");
  assert.match(popup, /kind:\s*"release_debugger_sessions"/);
  const button = popup.match(/function renderReleaseDebugger\(snapshot\) \{([\s\S]*?)\n {2}\}/);
  assert.ok(button, "renderReleaseDebugger was not found");
  assert.doesNotMatch(
    button[1],
    /snapshot\.connected|snapshot\.compatible/,
    "the release button must not be gated on connection state the way End session is"
  );
});

test("giving up on a stuck download cancels it before its blob URL is revoked", () => {
  // exportRecording's finally block revokes the object_url unconditionally the moment
  // downloadSettled's promise settles, success or failure. Timing out without also requesting
  // cancellation used to leave Chrome writing from a URL that had just gone dead -- on a slow
  // disk or a large recording, that can truncate the delivered file instead of merely failing
  // to report on time.
  const source = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  const timeoutBody = source.match(/setTimeout\(\(\) => \{([\s\S]*?)\}, DOWNLOAD_SETTLE_MS\)/);
  assert.ok(timeoutBody, "the download settle timeout was not found");
  assert.match(timeoutBody[1], /chrome\.downloads\.cancel\(downloadId\)/);
  const cancelIndex = timeoutBody[1].indexOf("chrome.downloads.cancel");
  const settleIndex = timeoutBody[1].indexOf("settle(new Error");
  assert.ok(
    cancelIndex >= 0 && settleIndex > cancelIndex,
    "cancellation must be requested before the promise settles and the caller revokes the URL"
  );
});

test("adapter protocol two wires the new physical mechanisms at the Chrome seam", () => {
  const root = join(__dirname, "..");
  const worker = readFileSync(join(root, "service-worker.js"), "utf8");
  assert.match(worker, /command\.command === "resize_window"/);
  assert.match(worker, /command\.command === "screenshot_region"/);
  assert.match(worker, /command\.command === "screenshot_region"[\s\S]*?validateView\(command\.tab_id, command\.expected_viewport\)/);
  assert.match(worker, /screenshotApi\.regionClip\(command\.region\)/);
  assert.match(worker, /command\.command === "read_diagnostics"/);
  assert.match(worker, /command\.command === "clear_diagnostics"/);
  assert.match(worker, /command\.command === "start_recording"/);
  assert.match(worker, /command\.command === "status_recording"/);
  assert.match(worker, /command\.command === "stop_recording"/);
  assert.match(worker, /command\.command === "export_recording"/);
  assert.match(worker, /command\.command === "discard_recording"/);
  assert.match(worker, /frame\.kind === "command_chunk"/);
  assert.match(worker, /Page\.screencastFrameAck/);
  assert.match(worker, /GhostlightRecording\.MAX_WIDTH \/ visual\.clientWidth/);
  assert.match(worker, /quality: globalThis\.GhostlightRecording\.JPEG_QUALITY/);
  assert.match(worker, /await setRecordingPresentation\(command\.tab_id, true\)/);
  assert.match(worker, /setRecordingPresentation\(tabId, false\)/);
  assert.match(worker, /syncPresentationState\(tabId\)/);
  assert.match(worker, /recording_tabs: recording\.count\(\)/);
  assert.match(worker, /stateApi\.badge\(\{ \.\.\.liveState, recording_tabs: recording\.count\(\) \}\)/);
  assert.match(worker, /frame\.kind === "backend_unavailable"[\s\S]*?settleServiceBoundaryState\(\)/);
  assert.match(worker, /chrome\.tabs\.query\(\{ windowId: tab\.windowId \}\)/);
});

test("browser actions return the subject in the effect receipt without a describe round trip", () => {
  const root = join(__dirname, "..");
  const content = readFileSync(join(root, "content.js"), "utf8");
  const worker = readFileSync(join(root, "service-worker.js"), "utf8");

  assert.match(content, /sendResponse\(\{ ok: true, result: \{ activated: true, subject \} \}\)/);
  assert.match(
    content,
    /sendResponse\(\{ ok: true, result: \{ activated: true, subject \} \}\)[\s\S]*?element\.click\(\)/,
    "the activation reply must cross to the worker before the blocking dispatch runs"
  );
  assert.match(
    content,
    /sendResponse\(\{ ok: true, result: \{ filled_count: message\.fields\.length, submitted: Boolean\(submitElement\) \} \}\)[\s\S]*?submitElement\.click\(\)/,
    "the fill reply must cross to the worker before the verified submit runs"
  );
  assert.match(content, /kind === "box"[\s\S]*?rectangle: viewportRectangle\(element\), subject: actionSubject\(element\)/);
  assert.match(worker, /outcome: "activated"[\s\S]*?subject: result\.subject/);
  assert.match(worker, /outcome: "typed"[\s\S]*?subject: target\.subject/);
  assert.match(worker, /outcome: "dragged"[\s\S]*?source_subject: sourceSubject[\s\S]*?destination_subject: destinationSubject/);
  assert.doesNotMatch(worker, /describe_targets[\s\S]*?action label/);
});

test("the browser owns the whole recording capability, encoding included", () => {
  const root = join(__dirname, "..");
  const worker = readFileSync(join(root, "service-worker.js"), "utf8");
  const page = readFileSync(join(root, "offscreen.html"), "utf8");
  const offscreen = readFileSync(join(root, "offscreen.js"), "utf8");

  // The encode runs in an offscreen document, not in a worker Chrome may evict mid-encode.
  assert.match(worker, /chrome\.offscreen\.createDocument/);
  assert.match(page, /vendor\/gifenc\.js/);
  assert.match(page, /lib\/gif\.js/);
  assert.match(offscreen, /GhostlightGif\.create\(\{ decode: decodeFrame \}\)/);

  const body = worker.match(/async function exportRecording[\s\S]*?\n}\n/)[0];
  // The negative control: frames really do travel, from the registry to the encoder beside it.
  // Without this, the assertion below would pass just as happily if nothing moved at all.
  assert.match(body, /frames: selected\.frames/);
  // And they stop there. What comes back is one artifact and its measurements.
  const receipt = body.match(/return \{\n\s+outcome: "recording_exported"[\s\S]*?\n\s+\};/)[0];
  assert.doesNotMatch(receipt, /frames/);
  assert.match(receipt, /encoded: encoded\.measurements/);

  // Bytes take one form, chosen by where they are going. A download never gets base64 and a
  // client return never gets an object URL the caller could not read anyway.
  assert.match(offscreen, /transfer === "object_url"/);
  // An open offscreen document keeps this worker awake, and a worker that never sleeps never
  // forgets recording bytes.
  assert.match(body, /closeEncoder\(\)/);
});

test("service epochs and browser loss share one volatile-state teardown seam", () => {
  const worker = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  const cleanup = worker.match(/async function settleServiceBoundaryState[\s\S]*?\n}\n/)[0];
  assert.match(cleanup, /commandChunks\.clear\(\)[\s\S]*?interruptAllRecordings\("service_disconnected"\)[\s\S]*?diagnostics\.clearAll\(\)/);
  assert.match(worker, /onDisconnect[\s\S]*?settleServiceBoundaryState\(\)/);
  assert.match(worker, /operationEngine\.activate\(frame\.service_epoch\)[\s\S]*?if \(changed\) await settleServiceBoundaryState\(\)/);
  assert.match(worker, /frame\.kind === "command_chunk"[\s\S]*?await browserNegotiation[\s\S]*?commandChunks\.accept/);
  assert.match(worker, /onNativeMessage\(frame, port\)/);
});

test("console diagnostics receive execution-context provenance before disclosure", () => {
  const worker = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  assert.match(worker, /Runtime\.executionContextCreated[\s\S]*?diagnostics\.executionContextCreated/);
  assert.match(worker, /Runtime\.executionContextDestroyed[\s\S]*?diagnostics\.executionContextDestroyed/);
  assert.match(worker, /Runtime\.executionContextsCleared[\s\S]*?diagnostics\.executionContextsCleared/);
});

test("diagnostic teardown forgets volatile rings before disabling optional CDP domains", () => {
  const worker = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  const cleanup = worker.match(/async function clearDiagnostics[\s\S]*?\n}\n/)[0];
  assert.match(cleanup, /diagnostics\.forgetMany\(command\.tab_ids\)[\s\S]*?disableDiagnosticCapture\(command\.tab_ids\)/);
  assert.match(cleanup, /outcome: "diagnostics_cleared", cleared_count: clearedCount/);
});

test("the unpacked rewrite preserves the established extension and host identity", () => {
  const manifest = JSON.parse(readFileSync(join(__dirname, "..", "manifest.json"), "utf8"));
  const digest = createHash("sha256").update(Buffer.from(manifest.key, "base64")).digest().subarray(0, 16);
  const alphabet = "abcdefghijklmnop";
  const id = Array.from(digest).map((byte) => `${alphabet[byte >> 4]}${alphabet[byte & 15]}`).join("");
  assert.equal(id, "cjcmhepmagomefjggkcohdbfemacojoa");
  assert.equal(shared.NATIVE_HOST_NAME, "org.sylin.ghostlight");
});

test("the manifest declares the complete local product surface", () => {
  const root = join(__dirname, "..");
  const manifest = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));
  assert.equal(manifest.name, "Ghostlight in Browser");
  assert.equal(manifest.description, "Governed browser automation over your own authenticated session, for AI agents.");
  assert.equal(manifest.minimum_chrome_version, "116");
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
  assert.equal(state.OPERATIONS_KEY, "ghostlight.operations");
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
  assert.equal(state.connectionLabel({ connected: true, compatible: true, control_state: "attention" }), "Needs attention");
  assert.deepEqual(state.badge({ connected: true, compatible: true, control_state: "held" }), { text: "II", color: "#38bdf8" });
  assert.deepEqual(state.badge({ connected: true, compatible: true, control_state: "attention" }), { text: "!", color: "#dc2626" });
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
  assert.equal(queue.defer("workspace_a", 1, signal(1, "one")), true);
  now += 1;
  assert.equal(queue.defer("workspace_a", 1, signal(1, "newest")), true);
  assert.equal(queue.size(), 1);
  assert.equal(queue.get(1).signal.phase, "newest");
  now += 1;
  queue.defer("workspace_a", 2, signal(2, "two"));
  now += 1;
  queue.defer("workspace_a", 3, signal(3, "three"));
  assert.deepEqual(queue.snapshot().map((entry) => entry.tabId), [2, 3]);
  now += 100;
  assert.equal(queue.size(), 0);
});

test("the adapter defers unseen denials without focusing browser tabs", () => {
  const worker = readFileSync(join(__dirname, "..", "service-worker.js"), "utf8");
  assert.match(worker, /signal\.signal === "denial" && !\(await tabIsVisible\(tabId\)\)/);
  assert.match(worker, /chrome\.tabs\.onActivated[\s\S]*?flushPendingPresentation/);
  assert.match(worker, /chrome\.windows\.onFocusChanged[\s\S]*?flushPendingPresentation/);
  const flush = worker.match(/async function flushPendingPresentation[\s\S]*?\n}\n/)[0];
  assert.doesNotMatch(flush, /chrome\.tabs\.update|chrome\.windows\.update/);
});

test("model-driven close obeys the local preserve-tabs interlock", () => {
  const root = join(__dirname, "..");
  const worker = readFileSync(join(root, "service-worker.js"), "utf8");
  const options = readFileSync(join(root, "options.html"), "utf8");
  assert.match(worker, /async function tabPreservationEnabled\(\)[\s\S]*?chrome\.storage\.local\.get\(stateApi\.PRESERVE_TABS_KEY\)/);
  assert.match(worker, /if \(await tabPreservationEnabled\(\)\)[\s\S]*?code: "local_interlock"[\s\S]*?chrome\.tabs\.remove/);
  // A released close that the interlock refuses unbinds the tab, so the reuse ladder can adopt
  // it later (ADR-0137).
  assert.match(
    worker,
    /if \(command\.released\) topology\.forget\(command\.tab_id\)[\s\S]*?code: "local_interlock"/
  );
  assert.match(options, /id="preserve-tabs"/);
  assert.match(options, /You can always close tabs yourself\./);
});

test("workspace topology accepts only bounded Ghostlight group titles", () => {
  assert.equal(topology.GROUP_PREFIX, "Ghostlight - ");
  assert.equal(topology.GROUP_COLOR, "blue");
  assert.equal(topology.validTitle("Ghostlight - Codex"), true);
  assert.equal(topology.validTitle("Personal"), false);
});

test("reuse picks the exact-url unbound tab, then lowest id, and never a bound or non-web tab", async () => {
  const bound = { id: 1, url: "https://example.com/", windowId: 4 };
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async query() {
        return [
          { id: 2, url: "https://example.com/other", windowId: 4 },
          bound,
          { id: 3, url: "https://example.net/", windowId: 4 },
          { id: 4, url: "chrome://version", windowId: 4 },
          { id: undefined, url: "https://example.com/", windowId: 4 }
        ];
      },
      async get(id) { return { id, windowId: 4 }; },
      async group(value) { return value.groupId ?? 11; }
    },
    tabGroups: {
      async get(id) { return { id, windowId: 4, title: "Ghostlight - Codex" }; },
      async query() { return []; },
      async update() {}
    }
  };
  const manager = topology.create(chromeApi, "topology");
  // Tab 1 belongs to a workspace, so it must never be adopted even on an exact match.
  await manager.assign(bound.id, "workspace_a", "Ghostlight - Codex");
  assert.deepEqual(await manager.findReusable("https://example.com/other"), {
    id: 2, url: "https://example.com/other", windowId: 4
  });
  // Without an exact match, the lowest-id same-host unbound tab wins.
  assert.deepEqual(await manager.findReusable("https://example.com/"), {
    id: 2, url: "https://example.com/other", windowId: 4
  });
  assert.equal(await manager.findReusable("https://no-host-match.test/"), null);
  assert.equal(await manager.findReusable("chrome://version"), null);
});

test("same-title duplicate groups merge into the one canonical group", async () => {  const merged = [];
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async get(id) { return { id, windowId: 4 }; },
      async query(value) {
        return value.groupId === 12 ? [{ id: 12, windowId: 5 }] : [{ id: 13, windowId: 6 }];
      },
      async group(value) { merged.push(value); return value.groupId; }
    },
    tabGroups: {
      async get(id) { return { id, windowId: 4, title: "Ghostlight - Codex" }; },
      async query() {
        return [
          { id: 12, windowId: 5, title: "Ghostlight - Codex" },
          { id: 9, windowId: 4, title: "Ghostlight - Codex" },
          { id: 13, windowId: 6, title: "Ghostlight - Codex" }
        ];
      },
      async update() {}
    }
  };
  const manager = topology.create(chromeApi, "topology");
  await manager.assign(7, "workspace_a", "Ghostlight - Codex");
  // Strays move into the lowest-id canonical group first; Chromium then deletes the emptied
  // duplicates itself. The new tab joins the same canonical group afterwards.
  assert.deepEqual(merged, [
    { groupId: 9, tabIds: [12] },
    { groupId: 9, tabIds: [13] },
    { groupId: 9, tabIds: [7] }
  ]);
});

test("workspace topology reuses the established exact-title blue group", async () => {  let grouped = null;
  let updated = null;
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async get(id) { return { id, windowId: 4 }; },
      async group(value) { grouped = value; return value.groupId ?? 11; }
    },
    tabGroups: {
      async get(id) { return { id, windowId: 4 }; },
      async query() { return [{ id: 9, windowId: 4, title: "Ghostlight - Codex" }]; },
      async update(id, value) { updated = { id, ...value }; }
    }
  };
  const manager = topology.create(chromeApi, "topology");
  await manager.assign(7, "workspace_a", "Ghostlight - Codex");
  assert.deepEqual(grouped, { groupId: 9, tabIds: [7] });
  assert.deepEqual(updated, { id: 9, title: "Ghostlight - Codex", color: "blue", collapsed: false });
  assert.equal(manager.titleFor("workspace_a"), "Ghostlight - Codex");
  assert.deepEqual(manager.tabsFor("workspace_a"), [7]);
  assert.deepEqual(manager.tabsFor("workspace_b"), []);
});

test("opening reuses the exact-title group across browser windows", async () => {
  let created = null;
  let grouped = null;
  let focused = null;
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async create(value) { created = value; return { id: 7, windowId: value.windowId }; },
      async get(id) { return { id, windowId: 22 }; },
      async move() { throw new Error("the tab was created in the wrong window"); },
      async group(value) { grouped = value; return value.groupId ?? 11; }
    },
    tabGroups: {
      async get(id) { return { id, windowId: 22, title: "Ghostlight - Codex" }; },
      async query() { return [{ id: 9, windowId: 22, title: "Ghostlight - Codex" }]; },
      async update() {}
    },
    windows: {
      async create() { throw new Error("an existing group must not create a window"); },
      async update(id, value) { focused = { id, ...value }; return { id, ...value }; }
    }
  };
  const manager = topology.create(chromeApi, "topology");
  await manager.open("https://example.com", "workspace_a", "Ghostlight - Codex");
  assert.deepEqual(created, { url: "https://example.com", active: true, windowId: 22 });
  assert.deepEqual(grouped, { groupId: 9, tabIds: [7] });
  assert.deepEqual(focused, { id: 22, focused: true });
});

test("only the first tab for a workspace brings its Ghostlight window forward", async () => {
  let nextTabId = 7;
  let focusCount = 0;
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async create(value) { return { id: nextTabId++, windowId: value.windowId }; },
      async get(id) { return { id, windowId: 22 }; },
      async group(value) { return value.groupId; }
    },
    tabGroups: {
      async get(id) { return { id, windowId: 22, title: "Ghostlight - Codex" }; },
      async query() { return [{ id: 9, windowId: 22, title: "Ghostlight - Codex" }]; },
      async update() {}
    },
    windows: {
      async update(id, value) { focusCount += 1; return { id, ...value }; }
    }
  };
  const manager = topology.create(chromeApi, "topology");
  await manager.open("https://example.com/one", "workspace_a", "Ghostlight - Codex");
  await manager.open("https://example.com/two", "workspace_a", "Ghostlight - Codex");
  assert.equal(focusCount, 1);
});

test("opening creates the first URL directly in a dedicated Ghostlight window", async () => {
  let createdWindow = null;
  let grouped = null;
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async create() { throw new Error("the user's active window must not receive the tab"); },
      async query() { return []; },
      async get(id) { return { id, windowId: 30 }; },
      async group(value) { grouped = value; return 11; }
    },
    tabGroups: {
      async get() { throw new Error("no cached group"); },
      async query() { return []; },
      async update() {}
    },
    windows: {
      async create(value) {
        createdWindow = value;
        return { id: 30, tabs: [{ id: 8, windowId: 30 }] };
      },
      async update() { throw new Error("the new window already has the requested focus"); }
    }
  };
  const manager = topology.create(chromeApi, "topology");
  await manager.open("https://example.com", "workspace_a", "Ghostlight - Codex");
  assert.deepEqual(createdWindow, {
    url: "https://example.com",
    focused: true,
    type: "normal"
  });
  assert.deepEqual(grouped, { tabIds: [8] });
});

test("an unobservable created window reports an unknown physical effect", async () => {
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: { async query() { return []; } },
    tabGroups: { async query() { return []; } },
    windows: { async create() { return undefined; } }
  };
  const manager = topology.create(chromeApi, "topology");
  await assert.rejects(
    manager.open("https://example.com", "workspace_a", "Ghostlight - Codex"),
    (error) => error.effectUnknown === true
  );
});

test("a new group reuses the existing Ghostlight work window", async () => {
  let created = null;
  let windowCreates = 0;
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async create(value) { created = value; return { id: 8, windowId: value.windowId }; },
      async get(id) { return { id, windowId: 41 }; },
      async group() { return 12; }
    },
    tabGroups: {
      async get() { throw new Error("no cached group"); },
      async query() { return [{ id: 4, windowId: 41, title: "Ghostlight - Another client" }]; },
      async update() {}
    },
    windows: {
      async create() { windowCreates += 1; },
      async update(id, value) { return { id, ...value }; }
    }
  };
  const manager = topology.create(chromeApi, "topology");
  await manager.open("https://example.com", "workspace_a", "Ghostlight - Codex");
  assert.deepEqual(created, { url: "https://example.com", active: true, windowId: 41 });
  assert.equal(windowCreates, 0);
});

test("concurrent same-name opens create one canonical group and window", async () => {
  const tabs = new Map();
  const groups = new Map();
  let nextTabId = 1;
  let windowCreates = 0;
  const grouped = [];
  const chromeApi = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async create(value) {
        const tab = { id: nextTabId++, windowId: value.windowId };
        tabs.set(tab.id, tab);
        return tab;
      },
      async query() { return []; },
      async get(id) { return tabs.get(id); },
      async group(value) {
        grouped.push(value);
        return value.groupId ?? 100;
      }
    },
    tabGroups: {
      async get(id) {
        const group = groups.get(id);
        if (!group) throw new Error("stale group");
        return group;
      },
      async query() { return Array.from(groups.values()); },
      async update(id, value) {
        groups.set(id, { id, windowId: 30, ...value });
      }
    },
    windows: {
      async create() {
        windowCreates += 1;
        const tab = { id: nextTabId++, windowId: 30 };
        tabs.set(tab.id, tab);
        return { id: 30, tabs: [tab] };
      },
      async update(id, value) { return { id, ...value }; }
    }
  };
  const manager = topology.create(chromeApi, "topology");
  await Promise.all([
    manager.open("https://example.com/one", "workspace_a", "Ghostlight - Codex"),
    manager.open("https://example.com/two", "workspace_b", "Ghostlight - Codex")
  ]);
  assert.equal(windowCreates, 1);
  assert.deepEqual(grouped, [
    { tabIds: [1] },
    { groupId: 100, tabIds: [2] }
  ]);
});
