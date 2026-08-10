importScripts("lib/shared.js", "lib/state.js", "lib/topology.js", "lib/engine.js", "lib/debugger.js", "lib/presentation-queue.js");

const shared = globalThis.GhostlightShared;
const stateApi = globalThis.GhostlightState;
const HOST_NAME = shared.NATIVE_HOST_NAME;
const adapterEpoch = `adapter_${crypto.randomUUID().replaceAll("-", "")}`;
const operationEngine = globalThis.GhostlightOperationEngine.create({
  load: async () => (await chrome.storage.session.get(stateApi.OPERATIONS_KEY))[stateApi.OPERATIONS_KEY],
  save: async (value) => chrome.storage.session.set({ [stateApi.OPERATIONS_KEY]: value })
});
const debuggerLifecycle = globalThis.GhostlightDebuggerLifecycle.create(chrome.debugger);
const navigationWatchers = new Map();
const cancelled = new Set();
const activity = new Map();
const topology = globalThis.GhostlightTopology.create(chrome, stateApi.TOPOLOGY_KEY);
const presentationQueue = globalThis.GhostlightPresentationQueue.create();
let nativePort = null;
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
      await content(tab.id, { kind: "managed_scope", active: true }, true);
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
    recording_tabs: 0,
    unseen_denials: presentationQueue.size(),
    activity: Array.from(activity.values()).slice(-8),
    last_error: preferences.diagnostics ? liveState.last_error : liveState.last_error ? "The local Ghostlight service is unavailable." : null
  };
}

function updateBadge() {
  const unseen = presentationQueue.size();
  const badge = unseen > 0 ? { text: "!", color: "#ef4444" } : stateApi.badge(liveState);
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

async function connectNative() {
  if (nativePort) return;
  try {
    if (!browserId) await initializeLocalState();
    const port = chrome.runtime.connectNative(HOST_NAME);
    nativePort = port;
    port.onMessage.addListener(onNativeMessage);
    port.onDisconnect.addListener(() => {
      if (nativePort !== port) return;
      nativePort = null;
      setConnection({ connected: false, service_version: null, last_error: chrome.runtime.lastError?.message || "Native connection ended." });
      broadcastRuntimeState("disconnected").catch(() => {});
      chrome.alarms.create("ghostlight-reconnect", { delayInMinutes: 0.05 });
    });
    send({
      kind: "hello",
      major: shared.ADAPTER_PROTOCOL_MAJOR,
      adapter_version: chrome.runtime.getManifest().version,
      browser_id: browserId,
      adapter_epoch: adapterEpoch,
      capabilities: shared.ADAPTER_CAPABILITIES
    });
  } catch (error) {
    nativePort = null;
    setConnection({ connected: false, service_version: null, last_error: shared.bounded(error?.message ?? error, 500) });
    chrome.alarms.create("ghostlight-reconnect", { delayInMinutes: 0.05 });
  }
}

chrome.runtime.onInstalled.addListener(connectNative);
chrome.runtime.onStartup.addListener(connectNative);
chrome.alarms.onAlarm.addListener((alarm) => { if (alarm.name === "ghostlight-reconnect") connectNative(); });

chrome.webNavigation.onCommitted.addListener((details) => {
  if (details.frameId !== 0) return;
  const watcher = navigationWatchers.get(details.tabId);
  watcher?.commits.push(details.url);
  send(shared.browserEventFrame({ event: "document_committed", tab_id: details.tabId, url: details.url, correlation: watcher?.correlation }));
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (!changeInfo.status) return;
  send(shared.browserEventFrame({ event: "readiness_changed", tab_id: tabId, readiness: shared.readinessForStatus(changeInfo.status) }));
  if (changeInfo.status === "complete" && topology.workspaceFor(tabId)) {
    content(tabId, { kind: "managed_scope", active: true }, true)
      .then(() => flushPendingPresentation(tabId))
      .catch(() => {});
  }
});

chrome.tabs.onActivated.addListener(({ tabId }) => {
  flushPendingPresentation(tabId).catch(() => {});
});

chrome.windows.onFocusChanged.addListener((windowId) => {
  if (windowId === chrome.windows.WINDOW_ID_NONE) return;
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
      return content(tab.id, { kind: "managed_scope", active: true }, true);
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
  debuggerLifecycle.forget(tabId);
  topology.forget(tabId).catch(() => {});
  if (presentationQueue.forget(tabId)) {
    persistPresentationQueue().then(publishUiState).catch(() => {});
  }
  send(shared.browserEventFrame({ event: "tab_closed", tab_id: tabId }));
});

chrome.debugger.onEvent.addListener((source, method, params) => {
  if (!source.tabId) return;
  if (method === "Page.javascriptDialogOpening") {
    debuggerLifecycle.openDialog(source.tabId, params.type);
    send(shared.browserEventFrame({ event: "dialog_changed", tab_id: source.tabId, present: true, dialog_type: params.type || "unknown" }));
  }
  if (method === "Page.javascriptDialogClosed") {
    debuggerLifecycle.closeDialog(source.tabId).catch(() => {});
    send(shared.browserEventFrame({ event: "dialog_changed", tab_id: source.tabId, present: false, dialog_type: "unknown" }));
  }
});
chrome.debugger.onDetach.addListener((source) => { if (source.tabId) debuggerLifecycle.detached(source.tabId); });

async function onNativeMessage(frame) {
  if (frame.kind === "backend_unavailable") {
    setConnection({ connected: false, service_version: null, last_error: "The local Ghostlight service is unavailable." });
    await broadcastRuntimeState("disconnected");
    return;
  }
  if (frame.kind === "hello_accepted") {
    browserNegotiation = operationEngine.activate(frame.service_epoch);
    await browserNegotiation;
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
  if (command.command === "activate") return activate(request.correlation, command);
  if (command.command === "activate_point") return activatePoint(request.correlation, command);
  if (command.command === "scroll") {
    const result = await content(command.tab_id, { kind: "scroll", locator: command.locator, direction: command.direction, amount: command.amount });
    return { outcome: "scrolled", tab_id: command.tab_id, x: result.x, y: result.y };
  }
  if (command.command === "set_zoom") {
    await chrome.tabs.setZoom(command.tab_id, command.zoom);
    return { outcome: "zoomed", tab_id: command.tab_id, zoom: await chrome.tabs.getZoom(command.tab_id) };
  }
  if (command.command === "hover") return hoverLocator(command);
  if (command.command === "hover_point") return hoverPoint(command);
  if (command.command === "fill") return fill(request.correlation, command);
  if (command.command === "type_text") return typeText(request.correlation, command);
  if (command.command === "press_key") return pressKey(request.correlation, command);
  if (command.command === "drag") return dragLocators(request.correlation, command);
  if (command.command === "drag_points") return dragPoints(request.correlation, command);
  if (command.command === "upload_files") {
    const result = await content(command.tab_id, { kind: "upload_files", locator: command.locator, files: command.files });
    return { outcome: "files_uploaded", tab_id: command.tab_id, uploaded_count: result.uploaded_count, uploaded_bytes: result.uploaded_bytes };
  }
  if (command.command === "evaluate_script") return evaluateScript(request.correlation, command);
  if (command.command === "observe") {
    const result = await content(command.tab_id, { kind: "observe", condition: command.condition, value: command.value, locator: command.locator, timeout_ms: command.timeout_ms });
    return { outcome: "observed", tab_id: command.tab_id, ...result };
  }
  if (command.command === "inspect_dialog") return inspectDialog(command.tab_id);
  if (command.command === "handle_dialog") return handleDialog(command);
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

async function broadcastRuntimeState(controlState) {
  const tabs = await chrome.tabs.query({});
  await Promise.all(tabs
    .filter((tab) => tab.id && topology.workspaceFor(tab.id))
    .map((tab) => content(tab.id, { kind: "runtime_state", state: controlState }, true)));
}

async function applyRuntimeState(controlState) {
  setConnection({ control_state: controlState });
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
    await content(command.tab_id, { kind: "activate", locator: command.locator, button: command.button, click_count: command.click_count });
    await new Promise((resolve) => setTimeout(resolve, 250));
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "activated", tab: physicalTab(tab), committed_urls: commits };
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
    await content(command.tab_id, { kind: command.clear_first ? "clear" : "focus", locator: command.locator });
    if (command.clear_first) await content(command.tab_id, { kind: "focus", locator: command.locator });
    await ensureDebugger(command.tab_id);
    await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Input.insertText", { text: command.text });
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "typed", tab: physicalTab(tab), character_count: Array.from(command.text).length, committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
    throw error;
  } finally {
    navigationWatchers.delete(command.tab_id);
    await detachDebugger(command.tab_id);
  }
}

async function dispatchDrag(tabId, start, end) {
  const steps = 12;
  await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", { type: "mouseMoved", x: start.x, y: start.y });
  await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", { type: "mousePressed", x: start.x, y: start.y, button: "left", clickCount: 1 });
  for (let step = 1; step <= steps; step += 1) {
    const ratio = step / steps;
    await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: start.x + (end.x - start.x) * ratio,
      y: start.y + (end.y - start.y) * ratio,
      button: "left",
      buttons: 1
    });
  }
  await chrome.debugger.sendCommand({ tabId }, "Input.dispatchMouseEvent", { type: "mouseReleased", x: end.x, y: end.y, button: "left", clickCount: 1 });
}

async function dragWithPoints(correlation, tabId, start, end) {
  const commits = [];
  navigationWatchers.set(tabId, { correlation, commits });
  try {
    await dispatchDrag(tabId, start, end);
    const tab = await chrome.tabs.get(tabId);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "dragged", tab: physicalTab(tab), committed_urls: commits };
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
      { x: geometry.destination.left + geometry.destination.width / 2, y: geometry.destination.top + geometry.destination.height / 2 }
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
    return await dragWithPoints(correlation, command.tab_id, start, end);
  } finally {
    await detachDebugger(command.tab_id);
  }
}

async function evaluateScript(correlation, command) {
  await ensureDebugger(command.tab_id);
  const commits = [];
  navigationWatchers.set(command.tab_id, { correlation, commits });
  try {
    const evaluated = await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Runtime.evaluate", {
      expression: command.script,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true
    });
    if (evaluated.exceptionDetails) throw new Error(evaluated.exceptionDetails.text || "page script failed");
    const serialized = JSON.stringify(evaluated.result?.value ?? null);
    const value = serialized.slice(0, command.max_result_chars);
    await new Promise((resolve) => setTimeout(resolve, 100));
    const tab = await chrome.tabs.get(command.tab_id);
    if (cancelled.delete(correlation)) throw Object.assign(new Error("cancelled after dispatch"), { effectUnknown: true });
    return { outcome: "script_evaluated", tab: physicalTab(tab), value, truncated: serialized.length > value.length, committed_urls: commits };
  } catch (error) {
    error.effectUnknown = true;
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
    if (command.locator) {
      const rect = await content(command.tab_id, { kind: "geometry", locator: command.locator });
      clip = { x: Math.max(0, rect.x), y: Math.max(0, rect.y), width: Math.max(1, rect.width), height: Math.max(1, rect.height), scale: 1 };
      scope = "target";
    } else if (command.full_page) {
      const size = metrics.cssContentSize || metrics.contentSize;
      clip = { x: 0, y: 0, width: Math.max(1, size.width), height: Math.max(1, size.height), scale: 1 };
      scope = "full_page";
    } else {
      clip = { x: visual.pageX ?? 0, y: visual.pageY ?? 0, width: Math.max(1, visual.clientWidth), height: Math.max(1, visual.clientHeight), scale: 1 };
      scope = "viewport";
    }
    const scale = Math.min(1, 2400 / clip.width, 2400 / clip.height, Math.sqrt(4000000 / (clip.width * clip.height)));
    clip.scale = Math.max(0.05, scale);
    let capture = await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Page.captureScreenshot", { format: "jpeg", quality: 55, clip, captureBeyondViewport: true, fromSurface: true });
    if (capture.data.length > 6000000) capture = await chrome.debugger.sendCommand({ tabId: command.tab_id }, "Page.captureScreenshot", { format: "jpeg", quality: 30, clip, captureBeyondViewport: true, fromSurface: true });
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
    return { outcome: "activated", tab: physicalTab(tab), committed_urls: commits };
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
    return { outcome: "hovered", tab_id: command.tab_id };
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
    return { outcome: "hovered", tab_id: command.tab_id };
  } finally {
    await detachDebugger(command.tab_id);
  }
}

async function pressKey(correlation, command) {
  if (command.locator) await content(command.tab_id, { kind: "focus", locator: command.locator });
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
    return { outcome: "key_pressed", tab: physicalTab(tab), key: command.key, committed_urls: commits };
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
  Promise.resolve().then(async () => {
    if (message?.kind === "ui_state_changed") return null;
    if (message?.kind === "ui_snapshot") return uiSnapshot();
    if (message?.kind === "runtime_control") {
      requestRuntimeControl(message.intent);
      return { queued: true };
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

initializeLocalState()
  .then(connectNative)
  .catch((error) => setConnection({ last_error: shared.bounded(error?.message ?? error, 500) }));
