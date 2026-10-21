// Ghostlight -- the service worker of adapter protocol 4.
//
// The worker is the extension's whole bootstrap and router. It opens the native port and sends the
// hello as the first frame, answers each service frame (a request gets exactly one receipt), keeps the
// set of tabs Ghostlight controls, forwards browser events for those tabs only, and feeds Chrome's
// events to the modules in lib/. It makes no policy decision: whether a tab may be touched, what a
// page message means and what an effect was are the service's business.
importScripts("lib/protocol.js", "lib/messages.js", "lib/shared.js", "lib/state.js", "lib/connection-log.js", "lib/chunks.js",
  "lib/sessions.js", "lib/arguments.js", "lib/inventory.js", "lib/world.js", "lib/input.js", "lib/tabs.js",
  "lib/capture.js", "lib/presentation-queue.js", "lib/notice.js", "lib/diagnostics.js", "lib/recording.js",
  "lib/buffers.js", "vendor/acorn.js", "lib/evaluate.js", "lib/primitives.js");

const protocol = globalThis.GhostlightProtocol;
const shared = globalThis.GhostlightShared;
const stateApi = globalThis.GhostlightState;
const manifest = chrome.runtime.getManifest();

const WELCOME_PAGE = "setup.html";
const SERVICE_UNAVAILABLE = "The local Ghostlight service is unavailable.";
const ASSETS_REJECTED = "The service sent page code that failed its checksum.";
const NATIVE_ENDED = "Native connection ended.";
const OPTIONS_ONLY = "Open extension options to export developer diagnostics.";
const UNKNOWN_MESSAGE = "Unknown extension message.";
const RECONNECT_ALARM = "ghostlight-reconnect";
const RECONNECT_DELAY_MS = 3_000;
const RECONNECT_ALARM_MINUTES = 0.05;
const ERROR_CHARS = 500;
const ENGINE_VERSION = /^[0-9.]{1,32}$/;
const ENGINE_USER_AGENT = /Chrome\/([0-9.]{1,32})/;
const PLACEHOLDER_BRAND = /not.*a.*brand/i;
const ENGINE_BRAND = "Chromium";
const CONTROLLED_KEY = "ghostlight.controlled";
// The epoch of the service that granted the custody in CONTROLLED_KEY, kept beside it.
const SERVICE_EPOCH_KEY = "ghostlight.service_epoch";

// How the first connection of this worker ended, as far as the welcome page cares.
const FIRST_CONNECTION = Object.freeze({ ACCEPTED: "accepted", HOST_ABSENT: "host_absent", OTHER: "other" });

const CONTROL = Object.freeze({ MANAGED: "managed", RELEASED: "released" });
const RUNTIME = protocol.RUNTIME_STATE;
const STOP_REASON = protocol.RECORDING_STOP_REASON;
const RECEIPT_FAILURE = protocol.RECEIPT_FAILURE;
const FRAME = protocol.FRAME;
// The field of a backend_unavailable frame by which the browser relay says no service is there at all.
const BACKEND_SERVICE_FIELD = "service";
const BACKEND_SERVICE_ABSENT = "absent";
const EVENT = protocol.EVENT;
const MESSAGE = globalThis.GhostlightMessages.PAGE;
const DEBUGGER_EVENT = Object.freeze({
  DIALOG_OPENING: "Page.javascriptDialogOpening",
  DIALOG_CLOSED: "Page.javascriptDialogClosed",
  DRAG_INTERCEPTED: "Input.dragIntercepted",
  SCREENCAST_FRAME: "Page.screencastFrame"
});
// The diagnostics store method that takes each Runtime and Network event.
const DIAGNOSTIC_EVENTS = Object.freeze({
  "Runtime.executionContextCreated": "executionContextCreated",
  "Runtime.executionContextDestroyed": "executionContextDestroyed",
  "Runtime.executionContextsCleared": "executionContextsCleared",
  "Runtime.consoleAPICalled": "consoleAPICalled",
  "Runtime.exceptionThrown": "exceptionThrown",
  "Network.requestWillBeSent": "requestWillBeSent",
  "Network.responseReceived": "responseReceived",
  "Network.loadingFailed": "loadingFailed"
});

const adapterEpoch = `adapter_${crypto.randomUUID().replaceAll("-", "")}`;
const connectionEvents = globalThis.GhostlightConnectionLog.EVENTS;
const connectionLog = globalThis.GhostlightConnectionLog.create({
  storage: chrome.storage.local,
  debugKey: stateApi.DEBUG_KEY,
  context: {
    epoch: adapterEpoch,
    adapter_version: manifest.version,
    extension_id: chrome.runtime.id,
    browser_version: navigator.userAgent
  }
});
connectionLog.record(connectionEvents.WORKER_STARTED);

// ---- connection state ---------------------------------------------------------------------------

let nativePort = null;
let nativeConnectionAttempt = null;
let connectionAttemptNumber = 0;
let browserId = null;
let life = null;
let preferences = { ...stateApi.DEFAULT_PREFERENCES };
let serviceEpoch = null;
let negotiation = Promise.resolve();
let helloTimer = null;
let helloUnanswered = false;
let retryTimer = null;
let backendLost = false;
let persistChain = Promise.resolve();
// `adapter_outdated` is set when the service refuses this extension as outdated and cleared only
// by an accepted hello; `adapter_refusal` is the sentence the service sent with that refusal.
let liveState = {
  connected: false,
  compatible: true,
  service_outdated: false,
  adapter_outdated: false,
  adapter_refusal: null,
  control_state: RUNTIME.ACTIVE,
  last_error: null
};

// tab id -> CONTROL.MANAGED or CONTROL.RELEASED. Only these tabs have events and ambient state.
const controlled = new Map();

function send(frame) {
  if (!nativePort) return false;
  try {
    nativePort.postMessage(shared.wellFormed(frame));
    return true;
  } catch (_error) {
    return false;
  }
}

function sendEvent(event) {
  return send(shared.browserEventFrame(event));
}

// Browser events are for tabs Ghostlight controls. An opener, a group label or passive browsing never adds one.
function sendControlledEvent(event) {
  if (controlled.has(event.tab_id)) sendEvent(event);
}

function recordError(error) {
  setConnection({ last_error: shared.bounded(error?.message ?? error, ERROR_CHARS) });
}

function controlStateOf(value) {
  return protocol.isKnown("control_states", value) ? value : null;
}

function managedIds() {
  return Array.from(controlled, ([tabId, state]) => (state === CONTROL.MANAGED ? tabId : null)).filter((tabId) => tabId !== null);
}

function runtimeFact() {
  return liveState.connected && liveState.compatible ? liveState.control_state : RUNTIME.DISCONNECTED;
}

function persistControlled() {
  const snapshot = Array.from(controlled);
  persistChain = persistChain
    .then(() => chrome.storage.session.set({ [CONTROLLED_KEY]: snapshot }))
    .catch(() => {});
  return persistChain;
}

function persistPresentationQueue() {
  return chrome.storage.session.set({ [stateApi.PRESENTATIONS_KEY]: presentationQueue.snapshot() });
}

// What the popup, the options page and the setup page draw. The link is classified here from the
// raw reason, so a surface gets the closed value and never Chrome's message text.
function uiSnapshot() {
  return {
    connected: liveState.connected,
    compatible: liveState.compatible,
    control_state: liveState.control_state,
    link_state: shared.linkState({
      connected: liveState.connected,
      compatible: liveState.compatible,
      outdated: liveState.service_outdated,
      adapterOutdated: liveState.adapter_outdated,
      lastError: liveState.last_error
    }),
    adapter_refusal: liveState.adapter_outdated ? liveState.adapter_refusal : null,
    attached_tabs: lifecycle.attachedCount(),
    recording_tabs: recording.count(),
    activity: notice.activityList()
  };
}

function updateBadge() {
  const unseen = presentationQueue.size();
  const badge = unseen > 0
    ? { text: "!", color: "#ef4444" }
    : stateApi.badge({ ...liveState, recording_tabs: recording.count() });
  chrome.action.setBadgeBackgroundColor({ color: badge.color });
  chrome.action.setBadgeTextColor?.({ color: stateApi.BADGE_TEXT_COLOR });
  chrome.action.setBadgeText({ text: badge.text });
  chrome.action.setTitle({ title: unseen > 0 ? "Ghostlight has an unseen guardrail notice" : manifest.name });
}

function publishUiState() {
  updateBadge();
  chrome.runtime.sendMessage({ kind: MESSAGE.UI_STATE_CHANGED }).catch(() => {});
}

function setConnection(patch) {
  liveState = { ...liveState, ...patch };
  publishUiState();
}

// ---- modules ------------------------------------------------------------------------------------

const cancellations = globalThis.GhostlightPrimitives.createCancellations();
const presentationQueue = globalThis.GhostlightPresentationQueue.create();
const diagnostics = globalThis.GhostlightDiagnostics.create({
  onExpired: (tabId) => buffers.captureExpired(tabId)
});
const recording = globalThis.GhostlightRecording.create({
  onStop: (...stop) => buffers.recordingEnded(...stop)
});
const commandChunks = globalThis.GhostlightCommandChunks.create({
  decodeBase64: (value) => Uint8Array.from(atob(value), (character) => character.charCodeAt(0)),
  decodeUtf8: (bytes) => new TextDecoder("utf-8", { fatal: true }).decode(bytes),
  sha256Hex: async (bytes) => {
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
  }
});
const lifecycle = globalThis.GhostlightSessions.create(chrome.debugger, {
  installer: (session) => world.register(session),
  onRelease: (tabId) => world.disposeTab(tabId),
  onSessionGone: (tabId, sessionId) => world.sessionGone(tabId, sessionId)
});
const inventory = globalThis.GhostlightInventory.create({ lifecycle, webNavigation: chrome.webNavigation });
const world = globalThis.GhostlightWorld.create({
  lifecycle,
  inventory,
  isCancelled: cancellations.isCancelled,
  local: {
    preferences: () => preferences,
    managed: (tabId) => controlled.get(tabId) === CONTROL.MANAGED,
    recording: (tabId) => Boolean(recording.activeForTab(tabId)),
    runtime: runtimeFact
  }
});

// The controlled-tab hook the tab primitives and the dispatcher call. `manage` runs before every
// primitive that addresses a tab: it records the tab as controlled, keeps its debugger session, and
// tells the page when the tab is new to Ghostlight or its session was lost. `release` ends control
// of a tab the person kept: it is observed until its next top-level commit and then forgotten.
const controlledTabs = Object.freeze({
  async manage(tabId) {
    const was = controlled.get(tabId);
    const attached = lifecycle.rootSession(tabId) !== null;
    controlled.set(tabId, CONTROL.MANAGED);
    if (was !== CONTROL.MANAGED) persistControlled();
    try {
      await lifecycle.retain(tabId);
    } catch (error) {
      recordError(error);
      throw error;
    }
    if (was !== CONTROL.MANAGED || !attached) await world.ambientAll(tabId).catch(() => false);
  },
  async release(tabId) {
    controlled.set(tabId, CONTROL.RELEASED);
    persistControlled();
    await lifecycle.unretain(tabId);
    await world.ambientAll(tabId).catch(() => false);
  }
});

const input = globalThis.GhostlightInput.create({
  lifecycle, inventory, worldCalls: world, isCancelled: cancellations.isCancelled
});
const tabs = globalThis.GhostlightTabs.create({
  lifecycle, controlled: controlledTabs, isCancelled: cancellations.isCancelled
});
const capture = globalThis.GhostlightCapture.create({ lifecycle, worldCalls: world });
const notice = globalThis.GhostlightNotice.create({
  worldCalls: world,
  inventory,
  queue: presentationQueue,
  changed: () => {
    persistPresentationQueue().catch(() => {});
    publishUiState();
  }
});
const buffers = globalThis.GhostlightBuffers.create({
  lifecycle, inventory, worldCalls: world, diagnostics, recording, changed: publishUiState
});
const evaluate = globalThis.GhostlightEvaluate.create({ lifecycle, inventory });
const primitives = globalThis.GhostlightPrimitives.create({
  send, controlled: controlledTabs, cancellations, lifecycle, inventory, world, tabs, capture, input, buffers, evaluate
});

// ---- local state --------------------------------------------------------------------------------

async function initializeLocalState() {
  const started = Date.now();
  connectionLog.record(connectionEvents.INITIALIZE_STARTED);
  const stored = await chrome.storage.local.get([
    stateApi.BROWSER_ID_KEY,
    stateApi.EFFECTS_KEY,
    stateApi.CAPTIONS_KEY,
    stateApi.DEBUG_KEY,
    stateApi.PRESERVE_TABS_KEY
  ]);
  browserId = stored[stateApi.BROWSER_ID_KEY];
  if (!browserId) {
    browserId = stateApi.newBrowserId(() => crypto.randomUUID());
    await chrome.storage.local.set({ [stateApi.BROWSER_ID_KEY]: browserId });
  }
  preferences = stateApi.preferencesFromStorage(stored);
  await chrome.storage.local.set(stateApi.preferencesForStorage(preferences));
  const session = await chrome.storage.session.get([
    stateApi.LIFE_KEY,
    CONTROLLED_KEY,
    SERVICE_EPOCH_KEY,
    stateApi.PRESENTATIONS_KEY
  ]);
  life = session[stateApi.LIFE_KEY];
  if (typeof life !== "string" || !life) {
    life = stateApi.newLife(() => crypto.randomUUID());
    await chrome.storage.session.set({ [stateApi.LIFE_KEY]: life }).catch(() => {});
  }
  presentationQueue.restore(session[stateApi.PRESENTATIONS_KEY]);
  serviceEpoch = typeof session[SERVICE_EPOCH_KEY] === "string" ? session[SERVICE_EPOCH_KEY] : null;
  await restoreControlled(session[CONTROLLED_KEY]);
  connectionLog.record(connectionEvents.INITIALIZE_FINISHED, { browser_id: browserId, duration_ms: Date.now() - started });
}

// Rebuilds the controlled set from storage, keeping only tabs that still exist.
async function restoreControlled(stored) {
  const live = new Set((await chrome.tabs.query({})).map((tab) => tab.id));
  controlled.clear();
  for (const entry of Array.isArray(stored) ? stored : []) {
    const [tabId, state] = Array.isArray(entry) ? entry : [];
    if (live.has(tabId) && (state === CONTROL.MANAGED || state === CONTROL.RELEASED)) controlled.set(tabId, state);
  }
}

// A restarted worker keeps its managed tabs: each one is retained again. Page code exists only once
// the service has accepted the hello, so telling the tabs their state, showing any waiting notice and
// releasing custody that a service no longer running granted happen in negotiate().
async function resumeControlledTabs() {
  await Promise.allSettled(managedIds().map((tabId) => lifecycle.retain(tabId)));
}

// ---- welcome ------------------------------------------------------------------------------------

// The setup page opens once, for an install whose first connection finds the service absent. Only a
// definite outcome settles it: a port that is open and unanswered is not one, because Chrome reports a
// missing host through the disconnect a moment after connectNative returns. A worker restart forgets
// the pending welcome and opens nothing.
let firstConnection = null;
let welcomePending = false;

function settleWelcome(outcome) {
  firstConnection ??= outcome;
  if (!welcomePending) return;
  welcomePending = false;
  if (outcome === FIRST_CONNECTION.HOST_ABSENT) chrome.tabs.create({ url: chrome.runtime.getURL(WELCOME_PAGE) }).catch(() => {});
}

function failedConnection(reason) {
  const absent = shared.linkState({ connected: false, compatible: false, lastError: reason }) === shared.LINK_HOST_ABSENT;
  settleWelcome(absent ? FIRST_CONNECTION.HOST_ABSENT : FIRST_CONNECTION.OTHER);
}

// ---- native connection --------------------------------------------------------------------------

function connectNative(trigger = "worker") {
  connectionLog.record(connectionEvents.CONNECT_REQUESTED, {
    trigger: typeof trigger === "string" ? trigger : "event",
    has_port: Boolean(nativePort),
    pending: Boolean(nativeConnectionAttempt)
  });
  if (nativePort) return Promise.resolve();
  if (nativeConnectionAttempt) return nativeConnectionAttempt;
  nativeConnectionAttempt = establishNativeConnection().finally(() => { nativeConnectionAttempt = null; });
  return nativeConnectionAttempt;
}

// Whether this browser currently holds a focused window. Reported in the hello so a browser that is
// already in front is routable at once. Connecting is not attention on its own.
async function holdsFocusedWindow() {
  try {
    const window = await chrome.windows.getLastFocused();
    return Boolean(window?.focused);
  } catch (_error) {
    return false;
  }
}

// The browser engine's full version: informational for the service, never a routing key.
async function engineVersion() {
  try {
    const high = await navigator.userAgentData?.getHighEntropyValues?.(["fullVersionList"]);
    const entries = Array.isArray(high?.fullVersionList) ? high.fullVersionList : [];
    const named = entries.find((entry) => entry?.brand === ENGINE_BRAND)
      ?? entries.find((entry) => typeof entry?.brand === "string" && !PLACEHOLDER_BRAND.test(entry.brand));
    if (ENGINE_VERSION.test(named?.version)) return named.version;
  } catch (_error) { /* the user-agent string answers instead */ }
  return ENGINE_USER_AGENT.exec(navigator.userAgent ?? "")?.[1];
}

// Everything asynchronous happens before the port opens, so the hello is the first frame on it.
async function establishNativeConnection() {
  const attempt = ++connectionAttemptNumber;
  let stage = "initialize";
  try {
    if (!browserId) {
      await initializeLocalState();
      resumeControlledTabs().catch(recordError);
    }
    if (nativePort) return;
    stage = "attention";
    const attended = await holdsFocusedWindow();
    const engine = await engineVersion();
    if (nativePort) return;
    stage = "connect_native";
    const port = chrome.runtime.connectNative(protocol.NATIVE_HOST_NAME);
    connectionLog.record(connectionEvents.NATIVE_PORT_OPENED, { attempt });
    nativePort = port;
    backendLost = false;
    port.onMessage.addListener((frame) => {
      onNativeMessage(frame, port).catch(recordError);
    });
    port.onDisconnect.addListener(() => onPortClosed(port, attempt));
    stage = "hello";
    send({
      kind: FRAME.HELLO,
      protocol: protocol.PROTOCOL,
      browser_id: browserId,
      life: life ?? undefined,
      browser_name: shared.browserName(navigator.userAgentData?.brands) ?? undefined,
      platform: protocol.PLATFORM,
      engine_version: engine,
      adapter_version: manifest.version,
      attended
    });
    connectionLog.record(connectionEvents.NATIVE_HELLO_SENT, { attempt, browser_id: browserId });
    clearRetryTimer();
    chrome.alarms.clear(RECONNECT_ALARM).catch(() => {});
    awaitHelloAnswer();
  } catch (error) {
    nativePort = null;
    setConnection({ connected: false, last_error: shared.bounded(error?.message ?? error, ERROR_CHARS) });
    connectionLog.record(connectionEvents.CONNECT_FAILED, { attempt, stage, error: shared.bounded(error?.message ?? error, ERROR_CHARS) });
    failedConnection(error?.message);
    scheduleRetry(attempt);
  }
}

// The wait for the service to answer a hello starts with the first hello and ends when a hello_accepted
// arrives, or when the browser relay says no service is there at all (a backend_unavailable whose
// service is absent), which reads as not connected. Nothing else ends it: the service closing the
// socket, a bare backend_unavailable and the port being replaced do not, because a 1.3.x service answers
// a protocol 4 hello that way, again and again, and never accepts it.
function awaitHelloAnswer() {
  if (helloUnanswered) return;
  helloUnanswered = true;
  helloTimer = setTimeout(onHelloTimeout, protocol.HELLO_TIMEOUT_MS);
}

function endHelloWait() {
  helloUnanswered = false;
  if (helloTimer) clearTimeout(helloTimer);
  helloTimer = null;
}

// A service that has not accepted the hello in time is one this extension cannot work with.
function onHelloTimeout() {
  helloTimer = null;
  settleWelcome(FIRST_CONNECTION.OTHER);
  setConnection({ connected: false, compatible: false, service_outdated: true });
}

// The service refused this extension's hello because it does not speak the service's protocol. The
// link says to update Ghostlight in Browser until a later hello is accepted, and the reconnect after
// the browser relay exits waits for the delayed retry instead of restarting the native host at once.
function adapterRefused(message) {
  connectionLog.record(connectionEvents.ADAPTER_REFUSED);
  endHelloWait();
  settleWelcome(FIRST_CONNECTION.OTHER);
  const refusal = shared.bounded(message, ERROR_CHARS);
  setConnection({
    connected: false,
    compatible: false,
    service_outdated: false,
    adapter_outdated: true,
    adapter_refusal: refusal === "" ? null : refusal
  });
}

function clearRetryTimer() {
  if (retryTimer) clearTimeout(retryTimer);
  retryTimer = null;
}

function scheduleRetry(attempt) {
  if (!retryTimer) {
    retryTimer = setTimeout(() => {
      retryTimer = null;
      if (!nativePort) connectNative("retry_timer");
    }, RECONNECT_DELAY_MS);
  }
  Promise.resolve().then(() => chrome.alarms.create(RECONNECT_ALARM, { delayInMinutes: RECONNECT_ALARM_MINUTES }))
    .then(() => chrome.alarms.get(RECONNECT_ALARM))
    .then((alarm) => connectionLog.record(connectionEvents.RETRY_SCHEDULED, { attempt, scheduled_time: alarm?.scheduledTime ?? 0 }))
    .catch((error) => connectionLog.record(connectionEvents.RETRY_FAILED, { attempt, error: shared.bounded(error?.message ?? error, ERROR_CHARS) }));
}

// The port is gone. In-flight primitives are cancelled and their receipts dropped, pages are told the
// service is away, and the connection comes back at once when the connector left on purpose after it
// said the service was unavailable, else after a short delay. A service that refused this extension
// as outdated would refuse it again, so that case always takes the delay.
function onPortClosed(port, attempt, reason = chrome.runtime.lastError?.message || NATIVE_ENDED) {
  if (nativePort !== port) return;
  nativePort = null;
  const orderly = backendLost;
  backendLost = false;
  connectionLog.record(connectionEvents.NATIVE_DISCONNECTED, { attempt, error: reason });
  failedConnection(reason);
  setConnection({ connected: false, last_error: reason });
  settleServiceBoundaryState().catch(recordError);
  broadcastAmbient();
  if (orderly && !liveState.adapter_outdated) connectNative("service_returned");
  else scheduleRetry(attempt);
}

// The port is closed from this side, which Chrome does not report back as a disconnect.
function closePort(port, reason) {
  try {
    port.disconnect();
  } catch (_error) { /* a port that is already gone needs no closing */ }
  onPortClosed(port, connectionAttemptNumber, reason);
}

// What must not outlive the service it belonged to: work in flight, partial transfers, focus
// emulation, recordings and diagnostics.
async function settleServiceBoundaryState() {
  primitives.disconnect();
  commandChunks.clear();
  try {
    await lifecycle.setFocusEmulationEnabled(false);
  } finally {
    await buffers.settle(STOP_REASON.SERVICE_DISCONNECTED);
  }
}

// Custody ends with the service that granted it. On a new service epoch every managed tab is
// released: its debugger session, focus emulation and glow go, and it stays released until its next
// top-level commit. The service takes a tab back with its next call that addresses it.
async function releaseManagedTabs() {
  await Promise.allSettled(managedIds().map((tabId) => controlledTabs.release(tabId)));
}

async function serviceLost(message, absent = false) {
  connectionLog.record(connectionEvents.BACKEND_UNAVAILABLE);
  backendLost = true;
  if (absent) endHelloWait();
  setConnection({
    connected: false,
    last_error: message,
    ...(absent ? { service_outdated: false } : {})
  });
  await settleServiceBoundaryState();
  await broadcastAmbient();
}

// ---- frame router -------------------------------------------------------------------------------

async function onNativeMessage(frame, port) {
  if (port !== nativePort) return;
  switch (frame?.kind) {
    case FRAME.HEARTBEAT: {
      const acknowledgement = shared.heartbeatAcknowledgement(frame);
      if (acknowledgement) send(acknowledgement);
      return;
    }
    case FRAME.HELLO_ACCEPTED:
      return accept(frame, port);
    case FRAME.COMMAND_CHUNK:
      await negotiation;
      if (port !== nativePort) return;
      commandChunks.accept(
        frame,
        (request) => { onNativeMessage(request, port).catch(recordError); },
        (correlation, reason) => {
          if (typeof correlation === "string") primitives.reject(correlation, RECEIPT_FAILURE.INVALID_REQUEST, reason);
        }
      );
      return;
    case FRAME.REQUEST:
      return runRequest(frame, port);
    case FRAME.CANCEL:
      if (typeof frame.correlation === "string") primitives.cancel(frame.correlation);
      return;
    case FRAME.NOTICE:
      await notice.deliver(frame);
      return;
    case FRAME.CONTROL_STATE: {
      await negotiation;
      const state = controlStateOf(frame.state);
      if (port === nativePort && state) await applyRuntimeState(state);
      return;
    }
    case FRAME.BACKEND_UNAVAILABLE:
      return serviceLost(SERVICE_UNAVAILABLE, frame[BACKEND_SERVICE_FIELD] === BACKEND_SERVICE_ABSENT);
    case FRAME.ERROR:
      if (frame.code === shared.ADAPTER_INCOMPATIBLE) return adapterRefused(frame.message);
      setConnection({ last_error: shared.bounded(frame.message, ERROR_CHARS) });
      return;
    default:
      return;
  }
}

// A request waits for the hello_accepted work before it runs, so it never meets a world without
// page code. It gets exactly one receipt, from the dispatcher.
async function runRequest(frame, port) {
  await negotiation;
  if (port !== nativePort || typeof frame.correlation !== "string") return;
  await primitives.run(frame.correlation, frame.primitive);
}

function accept(frame, port) {
  endHelloWait();
  settleWelcome(FIRST_CONNECTION.ACCEPTED);
  negotiation = negotiation.then(() => negotiate(frame, port)).catch(recordError);
  return negotiation;
}

async function negotiate(frame, port) {
  if (port !== nativePort) return;
  if (!(await world.setAssets(frame.assets))) {
    setConnection({ compatible: false });
    closePort(port, ASSETS_REJECTED);
    return;
  }
  if (port !== nativePort) return;
  if (frame.service_epoch !== serviceEpoch) {
    await settleServiceBoundaryState();
    await releaseManagedTabs();
    serviceEpoch = frame.service_epoch;
    await chrome.storage.session.set({ [SERVICE_EPOCH_KEY]: serviceEpoch }).catch(() => {});
    if (port !== nativePort) return;
  }
  backendLost = false;
  connectionLog.record(connectionEvents.HELLO_ACCEPTED, { service_version: frame.service_version });
  setConnection({
    connected: true,
    compatible: true,
    service_outdated: false,
    adapter_outdated: false,
    adapter_refusal: null,
    last_error: null
  });
  await applyRuntimeState(controlStateOf(frame.control_state) ?? RUNTIME.ACTIVE);
  await Promise.all(managedIds().map((tabId) => notice.flush(tabId).catch(() => false)));
}

// ---- runtime control ----------------------------------------------------------------------------

// Tells the page of every managed tab the current local facts (control state, preferences,
// recording, connection), best effort.
async function broadcastAmbient() {
  await Promise.all(managedIds().map((tabId) => world.ambientAll(tabId).catch(() => false)));
}

async function applyRuntimeState(state) {
  setConnection({ control_state: state });
  try {
    await lifecycle.setFocusEmulationEnabled(state === RUNTIME.ACTIVE && liveState.connected && liveState.compatible);
  } finally {
    if (state !== RUNTIME.ACTIVE) await buffers.settle(STOP_REASON.RUNTIME_HELD);
    await broadcastAmbient();
    if (state === RUNTIME.ENDED) await lifecycle.detachAll();
  }
}

function requestRuntimeControl(intent) {
  if (!nativePort) throw new Error("Ghostlight service is disconnected.");
  if (!protocol.isKnown("runtime_control_intents", intent)) throw new Error("Unknown runtime control intent.");
  sendEvent({ event: EVENT.RUNTIME_CONTROL_REQUESTED, intent });
}

// Asks the service to bring Ghostlight forward for a person in an extension page. Best effort: the
// service may have nothing to show, and that is never an error.
function requestShow() {
  if (!nativePort || !liveState.connected || !liveState.compatible) throw new Error("Ghostlight service is disconnected.");
  sendEvent({ event: EVENT.SHOW_REQUESTED });
}

// A purely local, mechanical release: it never touches the native port. If the service is gone for
// good, nothing else would clear Chrome's "controlled by automated software" banner.
async function releaseDebuggerSessions() {
  await buffers.settle(STOP_REASON.BROWSER_DETACHED);
  await lifecycle.detachAll();
}

// ---- Chrome listeners ---------------------------------------------------------------------------

chrome.runtime.onInstalled.addListener((details) => {
  connectionLog.record(connectionEvents.EXTENSION_INSTALLED, { trigger: details?.reason });
  connectNative("installed");
  if (details?.reason !== "install") return;
  welcomePending = true;
  if (firstConnection) settleWelcome(firstConnection);
});

chrome.runtime.onStartup.addListener(() => {
  connectionLog.record(connectionEvents.BROWSER_STARTED);
  connectNative("browser_startup");
});

chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name !== RECONNECT_ALARM) return;
  clearRetryTimer();
  connectionLog.record(connectionEvents.ALARM_FIRED, { scheduled_time: alarm.scheduledTime });
  connectNative("alarm");
});

chrome.commands.onCommand.addListener((command) => {
  if (command !== globalThis.GhostlightMessages.COMMAND.TOGGLE_HOLD) return;
  try {
    requestRuntimeControl("toggle_hold");
  } catch (error) {
    recordError(error);
  }
});

chrome.webNavigation.onCommitted.addListener((details) => {
  buffers.committed(details);
  if (details.frameId !== 0) return;
  primitives.committed(details.tabId, details.url);
  input.cancelDragInterception(details.tabId);
  const state = controlled.get(details.tabId);
  if (!state) return;
  sendEvent({ event: EVENT.DOCUMENT_COMMITTED, tab_id: details.tabId, url: shared.bounded(details.url, protocol.URL_CHARS) });
  if (state === CONTROL.RELEASED) {
    controlled.delete(details.tabId);
    persistControlled();
  }
});

// Only a navigation of the tab's own page starts a fresh diagnostics ring. An embedded document
// that navigates keeps the console and network history the tab gathered.
chrome.webNavigation.onBeforeNavigate.addListener((details) => {
  if (details.frameId === 0) buffers.beforeNavigate(details.tabId);
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (!changeInfo.status) return;
  sendControlledEvent({ event: EVENT.READINESS_CHANGED, tab_id: tabId, readiness: shared.readinessForStatus(changeInfo.status) });
  if (changeInfo.status === "complete" && controlled.get(tabId) === CONTROL.MANAGED) {
    world.ambientAll(tabId).then(() => notice.flush(tabId)).catch(() => {});
  }
});

chrome.tabs.onActivated.addListener(({ tabId }) => {
  notice.flush(tabId).catch(() => {});
});

chrome.windows.onFocusChanged.addListener((windowId) => {
  if (windowId === chrome.windows.WINDOW_ID_NONE) return;
  // The person just turned to this browser. Which window is a routing detail the service does not
  // model; that this browser was attended is the fact it needs.
  sendEvent({ event: EVENT.ATTENDED });
  notice.flushWindow(windowId).catch(() => {});
});

chrome.tabs.onRemoved.addListener((tabId) => {
  sendControlledEvent({ event: EVENT.TAB_CLOSED, tab_id: tabId });
  buffers.tabDetached(tabId);
  lifecycle.forget(tabId);
  world.dropTab(tabId);
  input.cancelDragInterception(tabId);
  if (controlled.delete(tabId)) persistControlled();
  if (presentationQueue.forget(tabId)) persistPresentationQueue().then(publishUiState).catch(() => {});
});

chrome.debugger.onEvent.addListener((source, method, params) => {
  const tabId = source.tabId;
  if (!tabId) return;
  if (method === DEBUGGER_EVENT.DIALOG_OPENING) {
    const type = params.type || protocol.UNKNOWN_DIALOG_TYPE;
    const sessionId = source.sessionId ?? null;
    lifecycle.openDialog(tabId, { type, message: params.message, defaultPrompt: params.defaultPrompt, url: params.url, sessionId });
    sendControlledEvent({ event: EVENT.DIALOG_CHANGED, tab_id: tabId, present: true, dialog_type: type });
    tabs.acceptBeforeUnload(tabId, { type, sessionId }).catch(() => {});
    return;
  }
  if (method === DEBUGGER_EVENT.DIALOG_CLOSED) {
    lifecycle.closeDialog(tabId).catch(() => {});
    sendControlledEvent({ event: EVENT.DIALOG_CHANGED, tab_id: tabId, present: false, dialog_type: protocol.UNKNOWN_DIALOG_TYPE });
    return;
  }
  if (method === DEBUGGER_EVENT.DRAG_INTERCEPTED) {
    input.dragIntercepted(tabId, params.data).catch(() => {});
    return;
  }
  if (method === DEBUGGER_EVENT.SCREENCAST_FRAME) {
    if (!source.sessionId) buffers.screencastFrame(tabId, params).catch(recordError);
    return;
  }
  const feed = DIAGNOSTIC_EVENTS[method];
  if (feed) diagnostics[feed](tabId, params);
});

chrome.debugger.onDetach.addListener((source) => {
  const tabId = source.tabId;
  if (!tabId) return;
  lifecycle.detached(tabId);
  buffers.tabDetached(tabId);
  world.dropTab(tabId);
  input.cancelDragInterception(tabId);
  publishUiState();
});

// ---- extension pages ----------------------------------------------------------------------------

async function handleUiMessage(message, sender) {
  switch (message?.kind) {
    case MESSAGE.UI_SNAPSHOT:
      return uiSnapshot();
    case MESSAGE.GET_PREFERENCES:
      return preferences;
    case MESSAGE.SET_PREFERENCES:
      preferences = stateApi.preferences(message.preferences);
      await chrome.storage.local.set(stateApi.preferencesForStorage(preferences));
      await connectionLog.setEnabled(preferences.diagnostics);
      connectionLog.record(connectionEvents.PREFERENCES_CHANGED);
      broadcastAmbient().catch(() => {});
      return preferences;
    case MESSAGE.RUNTIME_CONTROL:
      requestRuntimeControl(message.intent);
      return { queued: true };
    case MESSAGE.SHOW_GHOSTLIGHT:
      requestShow();
      return { queued: true };
    case MESSAGE.RELEASE_DEBUGGER_SESSIONS:
      await releaseDebuggerSessions();
      return { released: true };
    case MESSAGE.CONNECTION_DIAGNOSTICS:
      if (sender.id !== chrome.runtime.id || sender.url !== chrome.runtime.getURL("options.html")) throw new Error(OPTIONS_ONLY);
      return connectionLog.snapshot();
    default:
      throw new Error(UNKNOWN_MESSAGE);
  }
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  // Encoder traffic belongs to the offscreen document. Answering it here would race the real
  // handler and turn a working encode into an unknown-message error.
  if (message?.target === globalThis.GhostlightMessages.OFFSCREEN.TARGET) return false;
  handleUiMessage(message, sender)
    .then((value) => sendResponse({ ok: true, value }))
    .catch((error) => sendResponse({ ok: false, error: shared.bounded(error?.message ?? error, ERROR_CHARS) }));
  return true;
});

connectNative();
