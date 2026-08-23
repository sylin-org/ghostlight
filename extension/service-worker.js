importScripts("lib/shared.js", "lib/state.js", "lib/topology.js", "lib/engine.js", "lib/debugger.js", "lib/script-evaluator.js", "lib/diagnostics.js", "lib/recording.js", "lib/chunks.js", "lib/presentation-queue.js", "lib/screenshot.js");

const shared = globalThis.GhostlightShared;
const stateApi = globalThis.GhostlightState;
const screenshotApi = globalThis.GhostlightScreenshot;
const scriptEvaluator = globalThis.GhostlightScriptEvaluator;
const HOST_NAME = shared.NATIVE_HOST_NAME;
const SERVICE_INSTALL_URL = "https://sylin.org/ghostlight/chromium-extension/post-install/";
const adapterEpoch = `adapter_${crypto.randomUUID().replaceAll("-", "")}`;
const operationEngine = globalThis.GhostlightOperationEngine.create({
  load: async () => (await chrome.storage.session.get(stateApi.OPERATIONS_KEY))[stateApi.OPERATIONS_KEY],
  save: async (value) => chrome.storage.session.set({ [stateApi.OPERATIONS_KEY]: value })
});
const debuggerLifecycle = globalThis.GhostlightDebuggerLifecycle.create(chrome.debugger);
const diagnostics = globalThis.GhostlightDiagnostics.create({
  onExpired: (tabId) => {
    disableDiagnosticCapture([tabId]).catch(() => {});
  }
});
const commandChunks = globalThis.GhostlightCommandChunks.create({
  decodeBase64: (value) => Uint8Array.from(atob(value), (character) => character.charCodeAt(0)),
  decodeUtf8: (bytes) => new TextDecoder("utf-8", { fatal: true }).decode(bytes),
  sha256Hex: async (bytes) => {
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
  }
});
const navigationWatchers = new Map();
const dragInterceptions = new Map();
const cancelled = new Set();
const activity = new Map();
const topology = globalThis.GhostlightTopology.create(chrome, stateApi.TOPOLOGY_KEY);
const presentationQueue = globalThis.GhostlightPresentationQueue.create();
let nativePort = null;
let nativeConnectionAttempt = null;
let browserId = null;
let preferences = { ...stateApi.DEFAULT_PREFERENCES };
let browserNegotiation = Promise.resolve();
let liveState = {
  connected: false,
  compatible: true,
  service_version: null,
  control_state: "active",
  last_error: null
};

const recording = globalThis.GhostlightRecording.create({
  onStop: (tabId) => {
    chrome.debugger.sendCommand({ tabId }, "Page.stopScreencast").catch(() => {});
    setRecordingPresentation(tabId, false).catch(() => {});
    publishUiState();
  }
});

function send(frame) {
  if (!nativePort) return false;
  nativePort.postMessage(frame);
  return true;
}

async function initializeLocalState() {
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
  await topology.restore();
  const presentationState = await chrome.storage.session.get(stateApi.PRESENTATIONS_KEY);
  presentationQueue.restore(presentationState[stateApi.PRESENTATIONS_KEY]);
  const tabs = await chrome.tabs.query({});
  await Promise.all(tabs
    .filter((tab) => tab.id && topology.workspaceFor(tab.id))
    .map(async (tab) => {
      await retainManagedDebugger(tab.id);
      await syncPresentationState(tab.id);
      await flushPendingPresentation(tab.id);
    }));
}

async function tabPreservationEnabled() {
  const stored = await chrome.storage.local.get(stateApi.PRESERVE_TABS_KEY);
  const preserveTabs = stored[stateApi.PRESERVE_TABS_KEY] !== false;
  preferences = { ...preferences, preserveTabs };
  return preserveTabs;
}

function uiSnapshot() {
  return {
    ...liveState,
    adapter_version: chrome.runtime.getManifest().version,
    browser_id: browserId,
    attached_tabs: debuggerLifecycle.attachedCount(),
    recording_tabs: recording.count(),
    unseen_denials: presentationQueue.size(),
    activity: Array.from(activity.values()).slice(-8),
    // Classified from the raw reason before it is generalized below, so surfaces get the closed
    // value and never the message text.
    link_state: shared.linkState({
      connected: liveState.connected,
      compatible: liveState.compatible,
      lastError: liveState.last_error
    }),
    last_error: preferences.diagnostics ? liveState.last_error : liveState.last_error ? "The local Ghostlight service is unavailable." : null
  };
}

function updateBadge() {
  const unseen = presentationQueue.size();
  const badge = unseen > 0
    ? { text: "!", color: "#ef4444" }
    : stateApi.badge({ ...liveState, recording_tabs: recording.count() });
  chrome.action.setBadgeBackgroundColor({ color: badge.color });
  chrome.action.setBadgeText({ text: badge.text });
  chrome.action.setTitle({ title: unseen > 0 ? "Ghostlight has an unseen guardrail notice" : "Ghostlight in Browser" });
}

function publishUiState() {
  updateBadge();
  chrome.runtime.sendMessage({ kind: "ui_state_changed" }).catch(() => {});
}

function setConnection(patch) {
  liveState = { ...liveState, ...patch };
  publishUiState();
}

async function retainManagedDebugger(tabId) {
  try {
    await debuggerLifecycle.retain(tabId);
  } catch (error) {
    setConnection({ last_error: shared.bounded(error?.message ?? error, 500) });
  }
}

function connectNative() {
  if (nativePort) return Promise.resolve();
  if (nativeConnectionAttempt) return nativeConnectionAttempt;
  nativeConnectionAttempt = establishNativeConnection()
    .finally(() => { nativeConnectionAttempt = null; });
  return nativeConnectionAttempt;
}

async function establishNativeConnection() {
  try {
    if (!browserId) await initializeLocalState();
    if (nativePort) return;
    const port = chrome.runtime.connectNative(HOST_NAME);
    nativePort = port;
    port.onMessage.addListener((frame) => {
      if (nativePort !== port) return;
      onNativeMessage(frame, port).catch((error) => {
        setConnection({ last_error: shared.bounded(error?.message ?? error, 500) });
      });
    });
    port.onDisconnect.addListener(() => {
      const disconnectError = chrome.runtime.lastError?.message || "Native connection ended.";
      if (nativePort !== port) return;
      nativePort = null;
      settleServiceBoundaryState().catch(() => {});
      setConnection({ connected: false, service_version: null, last_error: disconnectError });
      broadcastRuntimeState("disconnected").catch(() => {});
      chrome.alarms.create("ghostlight-reconnect", { delayInMinutes: 0.05 });
    });
    send({
      kind: "hello",
      major: shared.ADAPTER_PROTOCOL_MAJOR,
      adapter_version: chrome.runtime.getManifest().version,
      browser_id: browserId,
      adapter_epoch: adapterEpoch,
      browser_name: shared.browserName(navigator.userAgentData?.brands),
      attended: await holdsFocusedWindow(),
      capabilities: shared.ADAPTER_CAPABILITIES
    });
  } catch (error) {
    nativePort = null;
    setConnection({ connected: false, service_version: null, last_error: shared.bounded(error?.message ?? error, 500) });
    chrome.alarms.create("ghostlight-reconnect", { delayInMinutes: 0.05 });
  }
}

function onExtensionInstalled(details) {
  connectNative();
  if (details?.reason === "install") {
    chrome.tabs.create({ url: SERVICE_INSTALL_URL }).catch(() => {});
  }
}

chrome.runtime.onInstalled.addListener(onExtensionInstalled);
chrome.runtime.onStartup.addListener(connectNative);
chrome.alarms.onAlarm.addListener((alarm) => { if (alarm.name === "ghostlight-reconnect") connectNative(); });

chrome.webNavigation.onCommitted.addListener((details) => {
  if (details.frameId !== 0) return;
  cancelDragInterception(details.tabId);
  recording.noteUrl(details.tabId, details.url);
  const watcher = navigationWatchers.get(details.tabId);
  watcher?.commits.push(details.url);
  send(shared.browserEventFrame({ event: "document_committed", tab_id: details.tabId, url: details.url, correlation: watcher?.correlation }));
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (!changeInfo.status) return;
  send(shared.browserEventFrame({ event: "readiness_changed", tab_id: tabId, readiness: shared.readinessForStatus(changeInfo.status) }));
  if (changeInfo.status === "complete" && topology.workspaceFor(tabId)) {
    syncPresentationState(tabId)
      .then(() => flushPendingPresentation(tabId))
      .catch(() => {});
  }
});

chrome.tabs.onActivated.addListener(({ tabId }) => {
  flushPendingPresentation(tabId).catch(() => {});
});

// Whether this browser currently holds a focused window.
//
// Reported at connection time so a browser that is already in front is routable immediately.
// Connecting is not attention on its own: a browser that attaches while the person is working
// elsewhere says nothing here and does not disturb the established order (ADR-0084 D2).
async function holdsFocusedWindow() {
  try {
    const window = await chrome.windows.getLastFocused();
    return Boolean(window?.focused);
  } catch {
    return false;
  }
}

chrome.windows.onFocusChanged.addListener((windowId) => {
  if (windowId === chrome.windows.WINDOW_ID_NONE) return;
  // The person just turned to this browser. Which window is a routing detail the service does
  // not model yet; that this browser was attended is the fact it needs.
  send(shared.browserEventFrame({ event: "attended" }));
  chrome.tabs.query({ active: true, windowId })
    .then((tabs) => tabs[0]?.id && flushPendingPresentation(tabs[0].id))
    .catch(() => {});
});

chrome.tabs.onCreated.addListener((tab) => {
  if (!tab.openerTabId || !tab.id) return;
  const workspace = topology.workspaceFor(tab.openerTabId);
  if (!workspace) return;
  send(shared.browserEventFrame({ event: "child_tab_opened", tab: physicalTab(tab), opener_tab_id: tab.openerTabId }));
  topology.assign(tab.id, workspace)
    .then(async () => {
      await retainManagedDebugger(tab.id);
      return syncPresentationState(tab.id);
    })
    .catch((error) => setConnection({ last_error: shared.bounded(error?.message ?? error, 500) }));
});
chrome.tabs.onAttached.addListener((tabId) => {
  if (!topology.workspaceFor(tabId)) return;
  topology.reattach(tabId)
    .then(() => retainManagedDebugger(tabId))
    .catch((error) => setConnection({ last_error: shared.bounded(error?.message ?? error, 500) }));
});
chrome.tabs.onRemoved.addListener((tabId) => {
  const activeRecording = recording.interruptTab(tabId, "browser_detached");
  diagnostics.forget(tabId);
  debuggerLifecycle.forget(tabId);
  cancelDragInterception(tabId);
  topology.forget(tabId).catch(() => {});
  if (presentationQueue.forget(tabId)) {
    persistPresentationQueue().then(publishUiState).catch(() => {});
  }
  if (activeRecording) publishUiState();
  send(shared.browserEventFrame({ event: "tab_closed", tab_id: tabId }));
});

chrome.debugger.onEvent.addListener((source, method, params) => {
  if (!source.tabId) return;
  if (method === "Input.dragIntercepted") {
    dragInterceptions.get(source.tabId)?.resolve(params.data);
    return;
  }
  if (method === "Page.screencastFrame") {
    handleScreencastFrame(source.tabId, params).catch((error) => {
      setConnection({ last_error: shared.bounded(error?.message ?? error, 500) });
    });
    return;
  }
  if (method === "Runtime.executionContextCreated") {
    diagnostics.executionContextCreated(source.tabId, params);
    return;
  }
  if (method === "Runtime.executionContextDestroyed") {
    diagnostics.executionContextDestroyed(source.tabId, params);
    return;
  }
  if (method === "Runtime.executionContextsCleared") {
    diagnostics.executionContextsCleared(source.tabId);
    return;
  }
  if (method === "Runtime.consoleAPICalled") {
    diagnostics.consoleAPICalled(source.tabId, params);
    return;
  }
  if (method === "Runtime.exceptionThrown") {
    diagnostics.exceptionThrown(source.tabId, params);
    return;
  }
  if (method === "Network.requestWillBeSent") {
    diagnostics.requestWillBeSent(source.tabId, params);
    return;
  }
  if (method === "Network.responseReceived") {
    diagnostics.responseReceived(source.tabId, params);
    return;
  }
  if (method === "Network.loadingFailed") {
    diagnostics.loadingFailed(source.tabId, params);
    return;
  }
  if (method === "Page.javascriptDialogOpening") {
    debuggerLifecycle.openDialog(source.tabId, params.type);
    send(shared.browserEventFrame({ event: "dialog_changed", tab_id: source.tabId, present: true, dialog_type: params.type || "unknown" }));
  }
  if (method === "Page.javascriptDialogClosed") {
    debuggerLifecycle.closeDialog(source.tabId).catch(() => {});
    send(shared.browserEventFrame({ event: "dialog_changed", tab_id: source.tabId, present: false, dialog_type: "unknown" }));
  }
});
chrome.debugger.onDetach.addListener((source) => {
  if (!source.tabId) return;
  const activeRecording = recording.interruptTab(source.tabId, "browser_detached");
  diagnostics.forget(source.tabId);
  debuggerLifecycle.detached(source.tabId);
  cancelDragInterception(source.tabId);
  publishUiState();
});

async function handleScreencastFrame(tabId, params) {
  try {
    await chrome.debugger.sendCommand({ tabId }, "Page.screencastFrameAck", { sessionId: params.sessionId });
  } catch (_error) {
    // A detached target has no compositor flow left to unblock.
  }
  recording.append(tabId, params.data, "screencast", Date.now());
}

async function disableDiagnosticCapture(tabIds) {
  await Promise.all((tabIds ?? []).flatMap((tabId) => ["Runtime", "Network"]
    .map((domain) => debuggerLifecycle.disableDomain(tabId, domain).catch(() => {}))));
}

async function settleServiceBoundaryState() {
  commandChunks.clear();
  await interruptAllRecordings("service_disconnected");
  await disableDiagnosticCapture(diagnostics.clearAll());
}

async function readDiagnostics(command) {
  if (!Number.isSafeInteger(command.limit) || command.limit < 1 || command.limit > 200) {
    throw new RangeError("diagnostic limit must be from 1 through 200");
  }
  const captureStarted = diagnostics.enable(command.tab_id);
  let sourceStarted = false;
  try {
    if (command.source === "both" || command.source === "console") {
      sourceStarted = await debuggerLifecycle.enableDomain(command.tab_id, "Runtime") || sourceStarted;
    }
    if (command.source === "both" || command.source === "network") {
      sourceStarted = await debuggerLifecycle.enableDomain(command.tab_id, "Network") || sourceStarted;
    }
  } catch (error) {
    if (captureStarted) {
      diagnostics.forget(command.tab_id);
      await disableDiagnosticCapture([command.tab_id]).catch(() => {});
    }
    throw error;
  }
  const result = diagnostics.read(command.tab_id, {
    source: command.source,
    detail: command.detail,
    match_text: command.match_text,
    after: command.after,
    limit: command.limit
  });
  result.capture_started = result.capture_started || sourceStarted;
  return { outcome: "diagnostics_read", tab_id: command.tab_id, ...result };
}

async function clearDiagnostics(command) {
  const clearedCount = diagnostics.forgetMany(command.tab_ids);
  await disableDiagnosticCapture(command.tab_ids);
  return { outcome: "diagnostics_cleared", cleared_count: clearedCount };
}

async function onNativeMessage(frame, sourcePort = nativePort) {
  if (sourcePort && nativePort !== sourcePort) return;
  const heartbeat = shared.heartbeatAcknowledgement(frame);
  if (heartbeat) {
    send(heartbeat);
    return;
  }
  if (frame.kind === "command_chunk") {
    await browserNegotiation;
    if (sourcePort && nativePort !== sourcePort) return;
    commandChunks.accept(
      frame,
      (requestFrame) => {
        onNativeMessage(requestFrame, sourcePort).catch((error) => {
          setConnection({ last_error: shared.bounded(error?.message ?? error, 500) });
        });
      },
      (correlation, reason) => send({
        kind: "error",
        correlation,
        code: "command_chunk_rejected",
        message: shared.bounded(reason, 500),
        effect_unknown: false
      })
    );
    return;
  }
  if (frame.kind === "backend_unavailable") {
    await settleServiceBoundaryState();
    setConnection({ connected: false, service_version: null, last_error: "The local Ghostlight service is unavailable." });
    await broadcastRuntimeState("disconnected");
    return;
  }
  if (frame.kind === "hello_accepted") {
    browserNegotiation = (async () => {
      const changed = await operationEngine.activate(frame.service_epoch);
      if (changed) await settleServiceBoundaryState();
    })();
    await browserNegotiation;
    if (sourcePort && nativePort !== sourcePort) return;
    setConnection({
      connected: true,
      compatible: frame.major === shared.ADAPTER_PROTOCOL_MAJOR,
      service_version: frame.service_version,
      control_state: stateApi.controlState(frame.control_state),
      last_error: frame.major === shared.ADAPTER_PROTOCOL_MAJOR ? null : "The service uses an incompatible browser adapter protocol."
    });
    await applyRuntimeState(stateApi.controlState(frame.control_state));
    return;
  }
  if (frame.kind === "acknowledge") {
    await operationEngine.acknowledge(frame.correlation);
    return;
  }
  if (frame.kind === "control_state") {
    const controlState = stateApi.controlState(frame.state);
    await applyRuntimeState(controlState);
    return;
  }
  if (frame.kind === "error" && !frame.correlation) {
    setConnection({ last_error: shared.bounded(frame.message, 500) });
    return;
  }
  if (frame.kind !== "request") return;
  const request = frame.request;
  try {
    await browserNegotiation;
    if (sourcePort && nativePort !== sourcePort) return;
    const result = await operationEngine.execute(request.correlation, () => dispatch(request));
    send({ kind: "receipt", receipt: { correlation: request.correlation, result } });
  } catch (error) {
    const code = typeof error?.code === "string" ? error.code : "primitive_failed";
    send({ kind: "error", correlation: request.correlation, code, message: shared.bounded(error?.message ?? error, 500), effect_unknown: Boolean(error?.effectUnknown) });
  }
}

async function dispatch(request) {
  const command = request.command;
  if (command.command === "cancel") {
    cancelled.add(command.correlation);
    return { outcome: "cancelled" };
  }
  if (cancelled.delete(request.correlation)) return { outcome: "cancelled" };
  if (command.command === "list_tabs") return { outcome: "tabs", tabs: (await chrome.tabs.query({})).map(physicalTab) };
  if (command.command === "focus_tab") {
    const tab = await chrome.tabs.update(command.tab_id, { active: true });
    const window = await chrome.windows.update(tab.windowId, { focused: true });
    return { outcome: "tab_focused", tab_id: command.tab_id, active: Boolean(tab.active), window_focused: Boolean(window.focused) };
  }
  if (command.command === "open_tab") return openTab(request.correlation, request.workspace, command);
  if (command.command === "navigate") return navigate(request.correlation, command);
  if (command.command === "traverse_history") return traverseHistory(request.correlation, command);
  if (command.command === "reload") return reload(request.correlation, command);
  if (command.command === "close_tab") {
    if (await tabPreservationEnabled()) {
      throw Object.assign(
        new Error("Ghostlight is preserving controlled tabs by local browser choice."),
        { code: "local_interlock" }
      );
    }
    await chrome.tabs.remove(command.tab_id);
    return { outcome: "tab_closed", tab_id: command.tab_id };
  }
  if (command.command === "read_text") {
    const result = await content(command.tab_id, { kind: "read_text", locator: command.locator, max_chars: command.max_chars });
    return { outcome: "text", tab_id: command.tab_id, ...result };
  }
  if (command.command === "inspect") {
    const result = await content(command.tab_id, { kind: "inspect", inspect_kind: command.kind, max_items: command.max_items });
    return { outcome: "targets", tab_id: command.tab_id, targets: result.targets };
  }
  if (command.command === "find") {
    const result = await content(command.tab_id, { kind: "find", text: command.text, find_kind: command.kind, max_results: command.max_results });
    return { outcome: "targets", tab_id: command.tab_id, targets: result.targets };
  }
  if (command.command === "describe_targets") {
    const result = await content(command.tab_id, { kind: "describe", locators: command.locators });
    return { outcome: "targets_described", tab_id: command.tab_id, targets: result.targets };
  }
  if (command.command === "screenshot") return screenshot(command);
  if (command.command === "screenshot_region") return screenshot(command);
  if (command.command === "activate") return activate(request.correlation, command);
  if (command.command === "activate_point") return activatePoint(request.correlation, command);
  if (command.command === "scroll") {
    const result = await content(command.tab_id, { kind: "scroll", locator: command.locator, direction: command.direction, amount: command.amount });
    return { outcome: "scrolled", tab_id: command.tab_id, x: result.x, y: result.y, subject: result.subject };
  }
  if (command.command === "set_zoom") {
    await chrome.tabs.setZoom(command.tab_id, command.zoom);
    return { outcome: "zoomed", tab_id: command.tab_id, zoom: await chrome.tabs.getZoom(command.tab_id) };
  }
  if (command.command === "resize_window") return resizeWindow(command);
  if (command.command === "hover") return hoverLocator(command);
  if (command.command === "hover_point") return hoverPoint(command);
  if (command.command === "fill") return fill(request.correlation, command);
  if (command.command === "type_text") return typeText(request.correlation, command);
  if (command.command === "press_key") return pressKey(request.correlation, command);
  if (command.command === "drag") return dragLocators(request.correlation, command);
  if (command.command === "drag_points") return dragPoints(request.correlation, command);
  if (command.command === "upload_files") {
    const result = await content(command.tab_id, { kind: "upload_files", locator: command.locator, files: command.files });
    return { outcome: "files_uploaded", tab_id: command.tab_id, uploaded_count: result.uploaded_count, uploaded_bytes: result.uploaded_bytes, subject: result.subject };
  }
  if (command.command === "evaluate_script") return evaluateScript(request.correlation, command);
  if (command.command === "observe") {
    const result = await content(command.tab_id, { kind: "observe", condition: command.condition, value: command.value, locator: command.locator, timeout_ms: command.timeout_ms });
    return { outcome: "observed", tab_id: command.tab_id, ...result };
  }
  if (command.command === "inspect_dialog") return inspectDialog(command.tab_id);
  if (command.command === "handle_dialog") return handleDialog(command);
  if (command.command === "read_diagnostics") return readDiagnostics(command);
  if (command.command === "clear_diagnostics") return clearDiagnostics(command);
  if (command.command === "start_recording") return startRecording(request.workspace, command);
  if (command.command === "status_recording") return recordingResult("recording_status", recording.status(request.workspace, command.recording_id));
  if (command.command === "stop_recording") return stopRecording(request.workspace, command);
  if (command.command === "export_recording") return exportRecording(request.workspace, command);
  if (command.command === "discard_recording") return discardRecording(request.workspace, command);
  if (command.command === "present") {
    updateActivity(command.signal, request.workspace);
    return { outcome: "presented", rendered: await deliverPresentation(request.workspace, command.signal) };
  }
  throw new Error("unknown browser primitive");
}

async function persistPresentationQueue() {
  await chrome.storage.session.set({ [stateApi.PRESENTATIONS_KEY]: presentationQueue.snapshot() });
}

async function presentationTab(workspace, signal) {
  if (signal.tab_id) return signal.tab_id;
  const owned = new Set(topology.tabsFor(workspace));
  const ownedTabs = (await chrome.tabs.query({})).filter((tab) => owned.has(tab.id));
  return ownedTabs.find((tab) => tab.active)?.id ?? (ownedTabs.length === 1 ? ownedTabs[0].id : undefined);
}

async function tabIsVisible(tabId) {
  try {
    const tab = await chrome.tabs.get(tabId);
    if (!tab.active || !Number.isInteger(tab.windowId)) return false;
    return Boolean((await chrome.windows.get(tab.windowId)).focused);
  } catch (_error) {
    return false;
  }
}

async function deferPresentation(workspace, tabId, signal) {
  if (!presentationQueue.defer(workspace, tabId, signal)) return;
  await persistPresentationQueue();
  publishUiState();
}

async function deliverPresentation(workspace, signal) {
  const tabId = await presentationTab(workspace, signal);
  if (!tabId) return false;
  if (signal.signal === "denial" && !(await tabIsVisible(tabId))) {
    await deferPresentation(workspace, tabId, signal);
    return false;
  }
  const result = await content(tabId, { kind: "present", signal, preferences }, true);
  if (result.presented) {
    if (presentationQueue.forget(tabId)) {
      await persistPresentationQueue();
      publishUiState();
    }
    return true;
  }
  if (signal.signal === "denial") await deferPresentation(workspace, tabId, signal);
  return false;
}

async function flushPendingPresentation(tabId) {
  const pending = presentationQueue.get(tabId);
  if (!pending || !(await tabIsVisible(tabId))) return false;
  const result = await content(tabId, { kind: "present", signal: pending.signal, preferences }, true);
  if (!result.presented) return false;
  presentationQueue.forget(tabId);
  await persistPresentationQueue();
  publishUiState();
  return true;
}

function updateActivity(signal, workspace) {
  const item = {
    invocation: signal.invocation,
    workspace,
    client_label: topology.titleFor(workspace)?.replace(/^Ghostlight - /, "") || "MCP client",
    label: shared.activityLabel(signal.activity),
    phase: shared.bounded(signal.phase, 80)
  };
  if (signal.signal === "completion") activity.delete(signal.invocation);
  else activity.set(signal.invocation, item);
  publishUiState();
}

function physicalTab(tab) {
  return {
    tab_id: tab.id,
    title: shared.bounded(tab.title, 500),
    url: tab.url || tab.pendingUrl || "about:blank",
    active: Boolean(tab.active),
    readiness: shared.readinessForStatus(tab.status)
  };
}

async function resizeWindow(command) {
  if (!Number.isSafeInteger(command.width) || command.width < 320 || command.width > 7680) {
    throw new RangeError("window width must be from 320 through 7680");
  }
  if (!Number.isSafeInteger(command.height) || command.height < 240 || command.height > 4320) {
    throw new RangeError("window height must be from 240 through 4320");
  }
  const tab = await chrome.tabs.get(command.tab_id);
  let resized;
  try {
    resized = await chrome.windows.update(tab.windowId, { width: command.width, height: command.height });
    const affected = (await chrome.tabs.query({ windowId: tab.windowId }))
      .map((item) => item.id)
      .filter(Number.isSafeInteger)
      .sort((left, right) => left - right);
    if (!affected.includes(command.tab_id)) affected.push(command.tab_id);
    affected.sort((left, right) => left - right);
    return {
      outcome: "window_resized",
      tab_id: command.tab_id,
      width: Number.isSafeInteger(resized?.width) ? resized.width : command.width,
      height: Number.isSafeInteger(resized?.height) ? resized.height : command.height,
      affected_tab_ids: affected
    };
  } catch (error) {
    if (resized) error.effectUnknown = true;
    throw error;
  }
}

async function interruptAllRecordings(reason) {
  const summaries = recording.interruptAll(reason);
  await Promise.all(summaries.map((summary) =>
    chrome.debugger.sendCommand({ tabId: summary.tab_id }, "Page.stopScreencast").catch(() => {})));
  publishUiState();
}

async function captureRecordingFrame(state, frameKind) {
  await ensureDebugger(state.tabId);
  await content(state.tabId, { kind: "presentation_visibility", hidden: true }, true);
  try {
    const metrics = await chrome.debugger.sendCommand({ tabId: state.tabId }, "Page.getLayoutMetrics");
    const visual = metrics.cssVisualViewport || metrics.visualViewport;
    const clip = {
      x: visual.pageX ?? 0,
      y: visual.pageY ?? 0,
      width: Math.max(1, visual.clientWidth),
      height: Math.max(1, visual.clientHeight),
      scale: Math.max(0.05, Math.min(1, globalThis.GhostlightRecording.MAX_WIDTH / visual.clientWidth, globalThis.GhostlightRecording.MAX_HEIGHT / visual.clientHeight))
    };
    const capture = await chrome.debugger.sendCommand({ tabId: state.tabId }, "Page.captureScreenshot", {
      format: "jpeg",
      quality: globalThis.GhostlightRecording.JPEG_QUALITY,
      clip,
      captureBeyondViewport: true,
      fromSurface: true
    });
    const dimensions = await imageDimensions(capture.data, clip);
    if (dimensions.width > globalThis.GhostlightRecording.MAX_WIDTH || dimensions.height > globalThis.GhostlightRecording.MAX_HEIGHT) {
      throw new Error("recording frame exceeded its negotiated dimensions");
    }
    return recording.append(state.tabId, capture.data, frameKind, Date.now());
  } finally {
    await content(state.tabId, { kind: "presentation_visibility", hidden: false }, true);
    await detachDebugger(state.tabId);
  }
}

async function startRecording(workspace, command) {
  const tab = await chrome.tabs.get(command.tab_id);
  const started = recording.start(workspace, command.tab_id, tab.url);
  await setRecordingPresentation(command.tab_id, true);
  if (started.existing) return { outcome: "recording_started", summary: started.existing, existing: true };
  const state = recording.activeForTab(command.tab_id);
  let screencastAttempted = false;
  try {
    await captureRecordingFrame(state, "seed").catch(() => false);
    if (!recording.activeForTab(command.tab_id)) throw new Error("recording ended during startup");
    await debuggerLifecycle.enableDomain(command.tab_id, "Page");
    screencastAttempted = true;
    await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Page.startScreencast", {
      format: "jpeg",
      quality: globalThis.GhostlightRecording.JPEG_QUALITY,
      maxWidth: globalThis.GhostlightRecording.MAX_WIDTH,
      maxHeight: globalThis.GhostlightRecording.MAX_HEIGHT,
      everyNthFrame: 1
    });
    publishUiState();
    return { outcome: "recording_started", summary: recording.status(workspace, state.id).summary, existing: false };
  } catch (error) {
    recording.discard(workspace, state.id);
    if (screencastAttempted) chrome.debugger.sendCommand({ tabId: command.tab_id }, "Page.stopScreencast").catch(() => {});
    publishUiState();
    throw error;
  }
}

function recordingResult(outcome, result) {
  if (result.notFound) return { outcome: "recording_not_found" };
  if (result.ambiguous) return { outcome: "recording_ambiguous", recording_ids: result.ambiguous };
  return { outcome, ...result };
}

async function stopRecording(workspace, command) {
  const selected = recording.beginStop(workspace, command.recording_id);
  if (!selected.state) return recordingResult("recording_stopped", selected);
  const state = selected.state;
  if (state.state !== "recording") {
    return { outcome: "recording_stopped", summary: selected.summary, changed: false };
  }
  try {
    await captureRecordingFrame(state, "final").catch(() => false);
    await chrome.debugger.sendCommand({ tabId: state.tabId }, "Page.stopScreencast");
    const summary = recording.finishStop(state, "explicit");
    publishUiState();
    return { outcome: "recording_stopped", summary, changed: true };
  } catch (error) {
    recording.interruptTab(state.tabId, "browser_detached");
    chrome.debugger.sendCommand({ tabId: state.tabId }, "Page.stopScreencast").catch(() => {});
    publishUiState();
    throw error;
  }
}

async function discardRecording(workspace, command) {
  const result = recording.discard(workspace, command.recording_id);
  if (result.notFound || result.ambiguous) return recordingResult("recording_discarded", result);
  if (result.active) {
    await chrome.debugger.sendCommand({ tabId: result.tabId }, "Page.stopScreencast").catch(() => {});
  }
  publishUiState();
  return { outcome: "recording_discarded", recording_id: result.recordingId, released_bytes: result.releasedBytes };
}

// The encoder lives in an offscreen document because Chrome may evict this worker mid-encode,
// and because object URLs -- the way a GIF reaches the download mechanism without anyone reading
// its bytes -- do not exist in a service worker at all.
const ENCODER_DOCUMENT = "offscreen.html";
const ENCODER_TARGET = "ghostlight-offscreen";
const DOWNLOAD_SETTLE_MS = 30_000;
let encoderSetup = null;

async function ensureEncoder() {
  if (await chrome.offscreen.hasDocument()) return;
  if (!encoderSetup) {
    encoderSetup = chrome.offscreen.createDocument({
      url: ENCODER_DOCUMENT,
      reasons: ["BLOBS"],
      justification: "Decode recording frames and encode one animated GIF away from the service worker."
    }).finally(() => { encoderSetup = null; });
  }
  await encoderSetup;
}

async function askEncoder(message) {
  await ensureEncoder();
  const response = await chrome.runtime.sendMessage({ target: ENCODER_TARGET, ...message });
  if (!response?.ok) throw new Error(response?.reason || "recording encoding failed");
  return response;
}

// An open offscreen document keeps this worker awake, and a worker that never sleeps never
// forgets recording bytes. Closing it is part of the volatility promise, not tidiness.
async function closeEncoder() {
  try {
    if (await chrome.offscreen.hasDocument()) await chrome.offscreen.closeDocument();
  } catch (_error) {}
}

function downloadSettled(downloadId) {
  return new Promise((resolve, reject) => {
    const settle = (error) => {
      chrome.downloads.onChanged.removeListener(watch);
      clearTimeout(timer);
      if (error) reject(error); else resolve();
    };
    const observe = (state) => {
      if (state === "complete") settle();
      if (state === "interrupted") settle(new Error("the browser could not write the replay"));
    };
    const watch = (delta) => {
      if (delta.id === downloadId && delta.state) observe(delta.state.current);
    };
    const timer = setTimeout(() => {
      // The caller's finally block revokes the source blob URL the instant this promise settles,
      // whether it resolves or rejects. Giving up here without also stopping the download used
      // to leave Chrome writing from a URL that had just gone dead -- a slow disk, a large
      // recording, or a synced Downloads folder could then deliver a truncated file while still
      // reporting success from a stale in-flight search, or a spurious failure if it did not.
      // Requesting cancellation first means the write has genuinely stopped by the time the URL
      // disappears, not merely that this function stopped waiting for it to.
      chrome.downloads.cancel(downloadId).catch(() => {});
      settle(new Error("the browser did not finish writing the replay"));
    }, DOWNLOAD_SETTLE_MS);
    chrome.downloads.onChanged.addListener(watch);
    // The download can finish before the listener attaches, and then no change ever arrives.
    chrome.downloads.search({ id: downloadId }).then(([item]) => observe(item?.state)).catch(() => {});
  });
}

async function deliverRecording(destination, encoded) {
  if (destination.destination === "target") {
    const result = await content(destination.tab_id, {
      kind: "upload_files",
      locator: destination.locator,
      files: [{
        name: destination.file_name,
        media_type: encoded.mime_type,
        data: encoded.data,
        size: encoded.measurements.byte_count
      }]
    });
    if (result.uploaded_count !== 1) throw new Error("the page did not accept the replay");
    return { delivery: "attached", tab_id: destination.tab_id };
  }
  if (destination.destination === "download") {
    const downloadId = await chrome.downloads.download({
      url: encoded.object_url,
      filename: destination.file_name,
      saveAs: false
    });
    await downloadSettled(downloadId);
    return { delivery: "downloaded" };
  }
  return { delivery: "returned", mime_type: encoded.mime_type, data: encoded.data };
}

// One command, one artifact. The orchestrator says whether recording may happen and where the
// result goes; the browser records, encodes, and delivers. Frames never leave (ADR-0109).
async function exportRecording(workspace, command) {
  const selected = recording.retained(workspace, command.recording_id);
  if (selected.notFound || selected.ambiguous) return recordingResult("recording_exported", selected);
  const destination = command.destination;
  let encoded = null;
  try {
    encoded = await askEncoder({
      kind: "encode_recording",
      frames: selected.frames,
      max_bytes: command.max_output_bytes,
      // Bytes materialize in exactly one form, chosen by where they are going: an object URL the
      // browser downloads without anyone reading it, or base64 for a destination that must.
      transfer: destination.destination === "download" ? "object_url" : "base64"
    });
    const delivery = await deliverRecording(destination, encoded);
    return {
      outcome: "recording_exported",
      summary: selected.summary,
      encoded: encoded.measurements,
      delivery
    };
  } catch (error) {
    return { outcome: "recording_export_failed", reason: shared.bounded(error?.message ?? error, 200) };
  } finally {
    if (encoded?.object_url) {
      await chrome.runtime.sendMessage({ target: ENCODER_TARGET, kind: "release_recording", object_url: encoded.object_url }).catch(() => {});
    }
    await closeEncoder();
  }
}

async function content(tabId, message, optional = false) {
  if (!tabId) {
    if (optional) return { presented: false };
    throw new Error("browser primitive requires a tab");
  }
  try {
    const response = await chrome.tabs.sendMessage(tabId, message);
    if (!response?.ok) throw new Error(response?.error || "content primitive failed");
    return response.result;
  } catch (error) {
    if (optional) return { presented: false };
    throw error;
  }
}

async function setRecordingPresentation(tabId, active) {
  return content(tabId, { kind: "recording_state", active }, true);
}

async function syncPresentationState(tabId) {
  await content(tabId, { kind: "managed_scope", active: true }, true);
  await setRecordingPresentation(tabId, Boolean(recording.activeForTab(tabId)));
}

async function broadcastRuntimeState(controlState) {
  const tabs = await chrome.tabs.query({});
  await Promise.all(tabs
    .filter((tab) => tab.id && topology.workspaceFor(tab.id))
    .map((tab) => content(tab.id, { kind: "runtime_state", state: controlState }, true)));
}

async function applyRuntimeState(controlState) {
  setConnection({ control_state: controlState });
  if (controlState !== "active") {
    await interruptAllRecordings("runtime_held");
    await disableDiagnosticCapture(diagnostics.clearAll());
  }
  if (controlState === "ended") {
    await debuggerLifecycle.detachAll();
  }
  await broadcastRuntimeState(controlState);
}

async function waitForReady(tabId, correlation, timeoutMs = 8000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    const tab = await chrome.tabs.get(tabId);
    if (tab.status === "complete") return tab;
    if (tab.status !== "loading" && (tab.url || tab.pendingUrl)) return tab;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return chrome.tabs.get(tabId);
}

async function openTab(correlation, workspace, command) {
  const commits = [];
  let openedTab = null;
  try {
    openedTab = await topology.open(command.url, workspace, command.group_title, (tab) => {
      openedTab = tab;
      navigationWatchers.set(tab.id, { correlation, commits });
    });
    await retainManagedDebugger(openedTab.id);
    const landed = await waitForReady(openedTab.id, correlation);
    return { outcome: "tab_opened", tab: physicalTab(landed), committed_urls: commits };
  } catch (error) {
    if (openedTab?.id !== undefined) error.effectUnknown = true;
    throw error;
  } finally {
    if (openedTab?.id !== undefined) navigationWatchers.delete(openedTab.id);
  }
}

async function navigate(correlation, command) {
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    await chrome.tabs.update(command.tab_id, { url: command.url, active: true });
    const tab = await waitForReady(command.tab_id, correlation);
    return { outcome: "navigated", tab: physicalTab(tab), committed_urls: commits };
  } catch (error) {
    if (!error.effectUnknown) error.effectUnknown = true;
    throw error;
  } finally {
    navigationWatchers.delete(command.tab_id);
  }
}

async function traverseHistory(correlation, command) {
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    if (command.direction === "back") await chrome.tabs.goBack(command.tab_id);
    else if (command.direction === "forward") await chrome.tabs.goForward(command.tab_id);
    else throw new Error("unknown history direction");
    await new Promise((resolve) => setTimeout(resolve, 75));
    const tab = await waitForReady(command.tab_id, correlation);
    return { outcome: "navigated", tab: physicalTab(tab), committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
    throw error;
  } finally {
    navigationWatchers.delete(command.tab_id);
  }
}

async function reload(correlation, command) {
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    await chrome.tabs.reload(command.tab_id, { bypassCache: command.bypass_cache });
    await new Promise((resolve) => setTimeout(resolve, 75));
    const tab = await waitForReady(command.tab_id, correlation);
    return { outcome: "navigated", tab: physicalTab(tab), committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
    throw error;
  } finally {
    navigationWatchers.delete(command.tab_id);
  }
}

async function activate(correlation, command) {
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    const result = await content(command.tab_id, { kind: "activate", locator: command.locator, button: command.button, click_count: command.click_count });
    await new Promise((resolve) => setTimeout(resolve, 250));
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "activated", tab: physicalTab(tab), subject: result.subject, committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
    throw error;
  } finally { navigationWatchers.delete(command.tab_id); }
}

async function fill(correlation, command) {
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    const result = await content(command.tab_id, { kind: "fill", fields: command.fields, submit_locator: command.submit_locator });
    await new Promise((resolve) => setTimeout(resolve, result.submitted ? 250 : 25));
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "filled", tab: physicalTab(tab), filled_count: result.filled_count, submitted: result.submitted, committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
    throw error;
  } finally { navigationWatchers.delete(command.tab_id); }
}

async function typeText(correlation, command) {
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    const target = await content(command.tab_id, { kind: command.clear_first ? "clear" : "focus", locator: command.locator });
    if (command.clear_first) await content(command.tab_id, { kind: "focus", locator: command.locator });
    await ensureDebugger(command.tab_id);
    await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Input.insertText", { text: command.text });
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "typed", tab: physicalTab(tab), character_count: Array.from(command.text).length, subject: target.subject, committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
    throw error;
  } finally {
    navigationWatchers.delete(command.tab_id);
    await detachDebugger(command.tab_id);
  }
}

async function dispatchDrag(tabId, start, end) {
  const packets = shared.dragPackets(start, end);
  const finalPacket = packets.at(-1);
  let interceptEnabled = false;
  let nextHeldPacket = 2;
  let pressed = false;
  let released = false;
  const interception = beginDragInterception(tabId);
  await content(tabId, { kind: "drag_observation_arm" });
  try {
    try {
      await chrome.debugger.sendCommand({ tabId }, "Input.setInterceptDrags", { enabled: true });
      interceptEnabled = true;
    } catch (_unsupported) {
      cancelDragInterception(tabId);
    }

    await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", packets[0]);
    await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", packets[1]);
    pressed = true;

    if (interceptEnabled) {
      for (; nextHeldPacket < packets.length - 1; nextHeldPacket += 1) {
        await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", packets[nextHeldPacket]);
        const observed = await content(tabId, { kind: "drag_observation_status" });
        if (!observed.started) continue;
        nextHeldPacket += 1;
        await chrome.debugger.sendCommand({ tabId }, "Input.setInterceptDrags", { enabled: false });
        interceptEnabled = false;
        if (!observed.cancelled) {
          const dragData = await waitForDragInterception(interception);
          if (dragData) {
            for (const type of ["dragEnter", "dragOver", "drop"]) {
              await chrome.debugger.sendCommand({ tabId }, "Input.dispatchDragEvent", {
                type,
                x: end.x,
                y: end.y,
                data: dragData
              });
            }
            await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", finalPacket);
            released = true;
            return;
          }
        }
        break;
      }
    }

    if (interceptEnabled) {
      await chrome.debugger.sendCommand({ tabId }, "Input.setInterceptDrags", { enabled: false });
      interceptEnabled = false;
    }
    for (; nextHeldPacket < packets.length - 1; nextHeldPacket += 1) {
      await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", packets[nextHeldPacket]);
    }
    await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", finalPacket);
    released = true;
  } finally {
    cancelDragInterception(tabId);
    await content(tabId, { kind: "drag_observation_finish" }, true);
    if (interceptEnabled) {
      await chrome.debugger.sendCommand({ tabId }, "Input.setInterceptDrags", { enabled: false }).catch(() => {});
    }
    if (pressed && !released) {
      await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", finalPacket).catch(() => {});
      await chrome.debugger.sendCommand({ tabId }, "Input.cancelDragging").catch(() => {});
    }
  }
}

function beginDragInterception(tabId) {
  cancelDragInterception(tabId);
  let resolve;
  const promise = new Promise((settle) => { resolve = settle; });
  const interception = { promise, resolve };
  dragInterceptions.set(tabId, interception);
  return interception;
}

function cancelDragInterception(tabId) {
  const interception = dragInterceptions.get(tabId);
  if (!interception) return;
  dragInterceptions.delete(tabId);
  interception.resolve(null);
}

async function waitForDragInterception(interception) {
  return Promise.race([
    interception.promise,
    new Promise((resolve) => setTimeout(() => resolve(null), 500))
  ]);
}

async function dragWithPoints(correlation, tabId, start, end, sourceSubject = null, destinationSubject = null) {
  const commits = [];
  navigationWatchers.set(tabId, { correlation, commits });
  try {
    await dispatchDrag(tabId, start, end);
    const tab = await chrome.tabs.get(tabId);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "dragged", tab: physicalTab(tab), source_subject: sourceSubject, destination_subject: destinationSubject, committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
    throw error;
  } finally {
    navigationWatchers.delete(tabId);
  }
}

async function dragLocators(correlation, command) {
  const geometry = await content(command.tab_id, {
    kind: "drag_geometry",
    source_locator: command.source_locator,
    destination_locator: command.destination_locator
  });
  await ensureDebugger(command.tab_id);
  try {
    return await dragWithPoints(
      correlation,
      command.tab_id,
      { x: geometry.source.left + geometry.source.width / 2, y: geometry.source.top + geometry.source.height / 2 },
      { x: geometry.destination.left + geometry.destination.width / 2, y: geometry.destination.top + geometry.destination.height / 2 },
      geometry.source_subject,
      geometry.destination_subject
    );
  } finally {
    await detachDebugger(command.tab_id);
  }
}

async function dragPoints(correlation, command) {
  await ensureDebugger(command.tab_id);
  try {
    await validateView(command.tab_id, command.expected_viewport);
    const start = await content(command.tab_id, { kind: "viewport_point", x: command.start.x, y: command.start.y });
    const end = await content(command.tab_id, { kind: "viewport_point", x: command.end.x, y: command.end.y });
    return await dragWithPoints(correlation, command.tab_id, start, end, start.subject, end.subject);
  } finally {
    await detachDebugger(command.tab_id);
  }
}

const SCRIPT_SETTLE_MS = 100;

async function evaluateScript(correlation, command) {
  await ensureDebugger(command.tab_id);
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  const send = (method, params) => chrome.debugger.sendCommand({ tabId: command.tab_id }, method, params);
  try {
    const value = await scriptEvaluator.evaluate(send, command.script, command.max_result_chars);
    const serialized = JSON.stringify(value ?? null);
    const bounded = serialized.slice(0, command.max_result_chars);
    await new Promise((resolve) => setTimeout(resolve, SCRIPT_SETTLE_MS));
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "script_evaluated", tab: physicalTab(tab), value: bounded, truncated: serialized.length > bounded.length, committed_urls: commits };
  } catch (error) {
    if (error.effectUnknown === undefined) error.effectUnknown = true;
    throw error;
  } finally {
    navigationWatchers.delete(command.tab_id);
    await detachDebugger(command.tab_id);
  }
}

async function ensureDebugger(tabId) {
  await debuggerLifecycle.acquire(tabId);
}

async function detachDebugger(tabId) {
  await debuggerLifecycle.release(tabId);
}

async function screenshot(command) {
  await ensureDebugger(command.tab_id);
  await content(command.tab_id, { kind: "presentation_visibility", hidden: true }, true);
  try {
    const metrics = await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Page.getLayoutMetrics");
    const visual = metrics.cssVisualViewport || metrics.visualViewport;
    let clip;
    let scope;
    if (command.command === "screenshot_region") {
      await validateView(command.tab_id, command.expected_viewport);
      clip = screenshotApi.regionClip(command.region);
      scope = "region";
    } else if (command.locator) {
      const rect = await content(command.tab_id, { kind: "geometry", locator: command.locator });
      clip = screenshotApi.ordinaryClip(Math.max(0, rect.x), Math.max(0, rect.y), Math.max(1, rect.width), Math.max(1, rect.height));
      scope = "target";
    } else if (command.full_page) {
      const size = metrics.cssContentSize || metrics.contentSize;
      clip = screenshotApi.ordinaryClip(0, 0, Math.max(1, size.width), Math.max(1, size.height));
      scope = "full_page";
    } else {
      clip = screenshotApi.ordinaryClip(visual.pageX ?? 0, visual.pageY ?? 0, Math.max(1, visual.clientWidth), Math.max(1, visual.clientHeight));
      scope = "viewport";
    }
    let capture = await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Page.captureScreenshot", { format: "jpeg", quality: screenshotApi.JPEG_QUALITY, clip, captureBeyondViewport: true, fromSurface: true });
    if (capture.data.length > screenshotApi.MAX_BASE64_CHARS) capture = await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Page.captureScreenshot", { format: "jpeg", quality: screenshotApi.FALLBACK_JPEG_QUALITY, clip, captureBeyondViewport: true, fromSurface: true });
    const ratio = await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Runtime.evaluate", { expression: "window.devicePixelRatio", returnByValue: true });
    const dimensions = await imageDimensions(capture.data, clip);
    return {
      outcome: "screenshot",
      tab_id: command.tab_id,
      mime_type: "image/jpeg",
      data: capture.data,
      width: dimensions.width,
      height: dimensions.height,
      viewport: {
        scope,
        page_x: clip.x,
        page_y: clip.y,
        css_width: clip.width,
        css_height: clip.height,
        visual_page_x: visual.pageX ?? 0,
        visual_page_y: visual.pageY ?? 0,
        visual_css_width: visual.clientWidth,
        visual_css_height: visual.clientHeight,
        device_scale: Number(ratio.result?.value || 1),
        zoom: await chrome.tabs.getZoom(command.tab_id),
        output_scale: dimensions.outputScale
      }
    };
  } finally {
    await content(command.tab_id, { kind: "presentation_visibility", hidden: false }, true);
    await detachDebugger(command.tab_id);
  }
}

async function imageDimensions(base64, clip) {
  try {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
    const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/jpeg" }));
    const width = bitmap.width;
    const height = bitmap.height;
    bitmap.close();
    const scaleX = width / clip.width;
    const scaleY = height / clip.height;
    if (!near(scaleX, scaleY, 0.02)) throw new Error("screenshot output scale is inconsistent");
    return { width, height, outputScale: (scaleX + scaleY) / 2 };
  } catch (_error) {
    return {
      width: Math.round(clip.width * clip.scale),
      height: Math.round(clip.height * clip.scale),
      outputScale: clip.scale
    };
  }
}

function near(left, right, tolerance = 1) {
  return Number.isFinite(left) && Number.isFinite(right) && Math.abs(left - right) <= tolerance;
}

async function validateView(tabId, expected) {
  const metrics = await chrome.debugger.sendCommand({ tabId }, "Page.getLayoutMetrics");
  const visual = metrics.cssVisualViewport || metrics.visualViewport;
  const ratio = await chrome.debugger.sendCommand({ tabId }, "Runtime.evaluate", { expression: "window.devicePixelRatio", returnByValue: true });
  const zoom = await chrome.tabs.getZoom(tabId);
  const current = {
    visual_page_x: visual.pageX ?? 0,
    visual_page_y: visual.pageY ?? 0,
    visual_css_width: visual.clientWidth,
    visual_css_height: visual.clientHeight,
    device_scale: Number(ratio.result?.value || 1),
    zoom
  };
  if (!near(current.visual_page_x, expected.visual_page_x)
    || !near(current.visual_page_y, expected.visual_page_y)
    || !near(current.visual_css_width, expected.visual_css_width)
    || !near(current.visual_css_height, expected.visual_css_height)
    || !near(current.device_scale, expected.device_scale, 0.02)
    || !near(current.zoom, expected.zoom, 0.001)) {
    throw new Error("stale screenshot view");
  }
}

async function pointInViewport(tabId, point) {
  return content(tabId, { kind: "scroll_point", x: point.x, y: point.y });
}

async function dispatchClick(tabId, point, button, clickCount) {
  const name = button === "middle" ? "middle" : button === "secondary" ? "right" : "left";
  for (let count = 1; count <= clickCount; count += 1) {
    await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y, button: name });
    await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", { type: "mousePressed", x: point.x, y: point.y, button: name, clickCount: count });
    await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", { type: "mouseReleased", x: point.x, y: point.y, button: name, clickCount: count });
  }
}

async function activatePoint(correlation, command) {
  await ensureDebugger(command.tab_id);
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    await validateView(command.tab_id, command.expected_viewport);
    const point = await pointInViewport(command.tab_id, command.point);
    await dispatchClick(command.tab_id, point, command.button, command.click_count);
    await new Promise((resolve) => setTimeout(resolve, 250));
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "activated", tab: physicalTab(tab), subject: point.subject, committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
    throw error;
  } finally {
    navigationWatchers.delete(command.tab_id);
    await detachDebugger(command.tab_id);
  }
}

async function hoverLocator(command) {
  const geometry = await content(command.tab_id, { kind: "hover", locator: command.locator });
  await ensureDebugger(command.tab_id);
  try {
    await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: geometry.rectangle.left + geometry.rectangle.width / 2,
      y: geometry.rectangle.top + geometry.rectangle.height / 2
    });
    return { outcome: "hovered", tab_id: command.tab_id, subject: geometry.subject };
  } finally {
    await detachDebugger(command.tab_id);
  }
}

async function hoverPoint(command) {
  await ensureDebugger(command.tab_id);
  try {
    await validateView(command.tab_id, command.expected_viewport);
    const point = await pointInViewport(command.tab_id, command.point);
    await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y });
    return { outcome: "hovered", tab_id: command.tab_id, subject: point.subject };
  } finally {
    await detachDebugger(command.tab_id);
  }
}

async function pressKey(correlation, command) {
  const target = command.locator ? await content(command.tab_id, { kind: "focus", locator: command.locator }) : null;
  await ensureDebugger(command.tab_id);
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    const modifiers = shared.modifierMask(command.modifiers);
    const descriptor = shared.keyDescriptor(command.key);
    await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Input.dispatchKeyEvent", { type: "keyDown", ...descriptor, modifiers });
    const { text: _text, ...keyUp } = descriptor;
    await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Input.dispatchKeyEvent", { type: "keyUp", ...keyUp, modifiers });
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "key_pressed", tab: physicalTab(tab), key: command.key, subject: target?.subject, committed_urls: commits };
  } catch (error) { error.effectUnknown = true; throw error; }
  finally { navigationWatchers.delete(command.tab_id); await detachDebugger(command.tab_id); }
}

async function inspectDialog(tabId) {
  const known = debuggerLifecycle.currentDialog(tabId);
  if (known) return { outcome: "dialog", tab_id: tabId, present: true, dialog_type: known.type };
  await ensureDebugger(tabId);
  try {
    await new Promise((resolve) => setTimeout(resolve, 50));
    const dialog = debuggerLifecycle.currentDialog(tabId);
    return { outcome: "dialog", tab_id: tabId, present: Boolean(dialog), dialog_type: dialog?.type || "unknown" };
  } finally {
    await detachDebugger(tabId);
  }
}

async function handleDialog(command) {
  await ensureDebugger(command.tab_id);
  const type = debuggerLifecycle.currentDialog(command.tab_id)?.type || "unknown";
  try {
    await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Page.handleJavaScriptDialog", { accept: command.accept, promptText: command.text });
    await debuggerLifecycle.closeDialog(command.tab_id);
    return { outcome: "dialog_handled", tab_id: command.tab_id, dialog_type: type, accepted: command.accept };
  } catch (error) { error.effectUnknown = true; throw error; }
  finally { await detachDebugger(command.tab_id); }
}

function requestRuntimeControl(intent) {
  if (!nativePort) throw new Error("Ghostlight service is disconnected.");
  if (!["toggle_hold", "hold", "resume", "end_session", "start_session"].includes(intent)) {
    throw new Error("Unknown runtime control intent.");
  }
  send(shared.browserEventFrame({ event: "runtime_control_requested", intent }));
}

chrome.commands.onCommand.addListener((command) => {
  if (command !== "toggle-hold") return;
  try {
    requestRuntimeControl("toggle_hold");
  } catch (error) {
    setConnection({ last_error: shared.bounded(error?.message ?? error, 500) });
  }
});

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  // Encoder traffic belongs to the offscreen document. Answering it here would race the real
  // handler and turn a working encode into "Unknown extension message."
  if (message?.target === "ghostlight-offscreen") return false;
  Promise.resolve().then(async () => {
    if (message?.kind === "ui_state_changed") return null;
    if (message?.kind === "ui_snapshot") return uiSnapshot();
    if (message?.kind === "runtime_control") {
      requestRuntimeControl(message.intent);
      return { queued: true };
    }
    if (message?.kind === "release_debugger_sessions") {
      // A purely local, mechanical release -- unlike runtime_control, this never touches the
      // native port and needs no session state at all. It exists because releasing a debugger
      // attachment (and Chrome's own "controlled by automated software" banner with it) is not a
      // governance decision the orchestrator has to make; it is the same kind of thing as closing
      // the tab by hand. If the service is gone for good -- crashed, uninstalled, never
      // restarted -- the normal path never fires (it only detaches on an explicit "ended" signal
      // the service has to send), and the person is left with no way to clear the banner short of
      // Chrome's own infobar or closing every tab. This is that way.
      await debuggerLifecycle.detachAll();
      return { released: true };
    }
    if (message?.kind === "attention_action") {
      if (message.disposition === "keep_paused") return { queued: false };
      let intent = message.disposition;
      if (intent === "resume_quiet") {
        preferences = stateApi.preferences({ ...preferences, effects: false, captions: false });
        await chrome.storage.local.set(stateApi.preferencesForStorage(preferences));
        intent = "resume";
      }
      requestRuntimeControl(intent);
      return { queued: true };
    }
    if (message?.kind === "get_preferences") return preferences;
    if (message?.kind === "set_preferences") {
      preferences = stateApi.preferences(message.preferences);
      await chrome.storage.local.set(stateApi.preferencesForStorage(preferences));
      return preferences;
    }
    throw new Error("Unknown extension message.");
  }).then((value) => sendResponse({ ok: true, value }))
    .catch((error) => sendResponse({ ok: false, error: shared.bounded(error?.message ?? error, 500) }));
  return true;
});

connectNative();
