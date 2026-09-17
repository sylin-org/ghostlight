// Ghostlight in Browser -- Firefox WebExtension Adapter (Gecko Dumb Shell)
//
// Governed browser automation over the interactive user's authenticated Firefox session.
// Implements the Ghostlight browser adapter protocol (ADR-0178, ADR-0179).

"use strict";

function logDebug(...args) {
  const msg = args.map((a) => (typeof a === "object" ? JSON.stringify(a) : String(a))).join(" ");
  console.log("[Ghostlight Firefox]", ...args);
  if (typeof dump === "function") {
    dump(`[Ghostlight Firefox] ${msg}\n`);
  }
}

if (typeof dump === "function") {
  dump("[Ghostlight Firefox] background.js starting execution\n");
}

const browserApi = typeof browser !== "undefined" ? browser : chrome;
const shared = typeof GhostlightShared !== "undefined"
  ? GhostlightShared
  : (typeof require !== "undefined" ? require("./lib/shared.js") : null);

const HOST_NAME = shared?.NATIVE_HOST_NAME || "org.sylin.ghostlight";
const BROWSER_PLATFORM = shared?.BROWSER_PLATFORM || "ghostlight/gecko";
const BROWSER_NAME = shared?.BROWSER_NAME || "Firefox";
const SERVICE_INSTALL_URL = "https://sylin.org/ghostlight/service/post-install/?browser=firefox";

let nativePort = null;
let nativeConnectionAttempt = null;
let connectionAttemptNumber = 0;
let reconnectTimer = null;

let browserId = null;
let adapterEpoch = null;
let serviceVersion = null;
let serviceEpoch = null;
let connectionState = {
  connected: false,
  compatible: true,
  lastError: null
};

// Generate a random hex identifier
function randomHex(length = 16) {
  const bytes = new Uint8Array(length / 2);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

// Initialize persistent browser ID and ephemeral adapter epoch
async function initializeIdentity() {
  if (!adapterEpoch) {
    adapterEpoch = `adapter_${randomHex(16)}`;
  }
  if (!browserId) {
    try {
      const stored = await browserApi.storage.local.get("ghostlight_browser_id");
      if (stored?.ghostlight_browser_id) {
        browserId = stored.ghostlight_browser_id;
      } else {
        browserId = `browser_firefox_${randomHex(16)}`;
        await browserApi.storage.local.set({ ghostlight_browser_id: browserId });
      }
    } catch {
      browserId = `browser_firefox_${randomHex(16)}`;
    }
  }
}

// Check whether Firefox currently holds window focus
async function isWindowFocused() {
  try {
    const current = await browserApi.windows.getCurrent();
    return Boolean(current?.focused);
  } catch {
    return false;
  }
}

// Send a frame over the native messaging port
function send(frame) {
  if (!nativePort) {
    logDebug("Cannot send frame, nativePort is null:", frame);
    return false;
  }
  try {
    nativePort.postMessage(frame);
    logDebug("Sent frame to native host:", frame);
    return true;
  } catch (error) {
    logDebug("Failed to postMessage:", error);
    return false;
  }
}

// Establish connection to native messaging host
function connectNative(trigger = "background") {
  logDebug(`connectNative triggered by ${trigger}`);
  if (nativePort) return Promise.resolve();
  if (nativeConnectionAttempt) return nativeConnectionAttempt;
  nativeConnectionAttempt = establishConnection()
    .finally(() => { nativeConnectionAttempt = null; });
  return nativeConnectionAttempt;
}

async function establishConnection() {
  const attempt = ++connectionAttemptNumber;
  logDebug(`establishConnection attempt=${attempt} host=${HOST_NAME}`);
  try {
    await initializeIdentity();
    if (nativePort) return;

    logDebug(`Calling browserApi.runtime.connectNative(${HOST_NAME})...`);
    const port = browserApi.runtime.connectNative(HOST_NAME);
    nativePort = port;
    logDebug(`Connected native port established for ${HOST_NAME}`);

    port.onMessage.addListener((frame) => {
      logDebug(`Message received from native host:`, frame);
      if (nativePort !== port) return;
      handleNativeMessage(frame, port).catch((err) => {
        logDebug(`Error handling message:`, err);
        connectionState.lastError = String(err?.message || err);
      });
    });

    port.onDisconnect.addListener(() => {
      const errorMsg = browserApi.runtime.lastError?.message || "Native connection disconnected.";
      logDebug(`Native port disconnected (attempt ${attempt}): ${errorMsg}`);
      if (nativePort !== port) return;
      nativePort = null;
      connectionState.connected = false;
      connectionState.lastError = errorMsg;
      scheduleReconnect(attempt);
    });

    const manifestVersion = browserApi.runtime.getManifest()?.version || "1.3.9";
    const attended = await isWindowFocused();

    const helloSent = send({
      kind: "hello",
      major: shared?.ADAPTER_PROTOCOL_MAJOR || 2,
      adapter_version: manifestVersion,
      browser_id: browserId,
      adapter_epoch: adapterEpoch,
      browser_name: BROWSER_NAME,
      platform: BROWSER_PLATFORM,
      attended,
      capabilities: shared?.ADAPTER_CAPABILITIES || []
    });

    if (helloSent) {
      clearReconnectTimer();
      connectionState.connected = true;
      connectionState.lastError = null;
    }
  } catch (error) {
    logDebug(`establishConnection error:`, error);
    nativePort = null;
    connectionState.connected = false;
    connectionState.lastError = String(error?.message || error);
    scheduleReconnect(attempt);
  }
}

function scheduleReconnect(attempt) {
  if (reconnectTimer || typeof module !== "undefined") return;
  const delay = Math.min(500 * Math.pow(1.5, Math.min(attempt, 8)), 10000);
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectNative("retry_timer");
  }, delay);
}

function clearReconnectTimer() {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
}

// Process incoming frames from orchestrator
async function handleNativeMessage(frame, port) {
  if (!frame || typeof frame !== "object") return;

  switch (frame.kind) {
    case "hello_accepted":
      serviceVersion = frame.service_version;
      serviceEpoch = frame.service_epoch;
      connectionState.connected = true;
      break;

    case "heartbeat":
      if (typeof frame.sequence === "number") {
        send({ kind: "heartbeat_ack", sequence: frame.sequence });
      }
      break;

    case "request":
      await executeRequest(frame.request);
      break;

    default:
      break;
  }
}

// Execute physical primitive requested by orchestrator
// Execute physical primitive requested by orchestrator
async function executeRequest(request) {
  if (!request || !request.command) return;
  const { correlation, command } = request;

  try {
    const outcome = await executeCommand(command);
    logDebug(`executeCommand outcome:`, outcome.outcome);
    send({
      kind: "receipt",
      receipt: {
        correlation,
        result: outcome
      }
    });
  } catch (error) {
    logDebug(`executeCommand error:`, error);
    send({
      kind: "receipt",
      receipt: {
        correlation,
        result: {
          outcome: "effect_unknown",
          reason: String(error?.message || error)
        }
      }
    });
  }
}

// Convert raw browser tab into typed PhysicalTab record
function physicalTab(tab) {
  return {
    tab_id: tab.id,
    title: tab.title || "",
    url: tab.url || tab.pendingUrl || "about:blank",
    active: Boolean(tab.active),
    readiness: shared?.readinessForStatus(tab.status) || (tab.status === "complete" ? "complete" : "loading")
  };
}

// Dispatch individual command to browser API
async function executeCommand(command) {
  const name = command.command || command.type;

  switch (name) {
    case "set_preload_script":
      return handleSetPreloadScript(command.script);

    case "list_tabs":
      return handleListTabs();

    case "focus_tab":
      return handleFocusTab(command.tab_id);

    case "open_tab":
      return handleOpenTab(command.url);

    case "close_tab":
      return handleCloseTab(command.tab_id);

    case "navigate":
    case "navigate_tab":
      return handleNavigateTab(command.tab_id, command.url);

    case "screenshot":
    case "capture_screenshot":
      return handleCaptureScreenshot(command.tab_id);

    case "resize_window":
    case "window_geometry":
      return handleWindowGeometry(command.tab_id, command.width, command.height, command.bounds || command);

    case "describe_documents":
      return handleDescribeDocuments(command);

    case "in_documents":
      return handleInDocuments(command);

    case "presentation":
    case "present":
      return { outcome: "presented", rendered: true };

    default:
      throw new Error(`unsupported command: ${name}`);
  }
}

// Describe document inventory in a tab
async function handleDescribeDocuments(command) {
  const tab = await browserApi.tabs.get(command.tab_id).catch(() => null);
  const docId = `doc_${command.tab_id}`;
  return {
    outcome: "documents",
    tab_id: command.tab_id,
    inventory: {
      documents: [{
        id: docId,
        url: tab?.url || "about:blank",
        parent: null,
        supported: true
      }],
      subjects: [docId],
      unresolved: false,
      incomplete: false
    }
  };
}

// Execute primitive scoped to documents
async function handleInDocuments(command) {
  const result = await executeCommand(command.primitive);
  return {
    outcome: "in_documents",
    result,
    observation: {
      visited: command.scope?.allowed || [],
      unavailable: [],
      limited_by_size: Boolean(result?.truncated),
      masked_regions: 0
    }
  };
}

// Install preload Glass UI script across all web documents
async function handleSetPreloadScript(script) {
  globalThis.ghostlightPreloadScript = script;
  if (browserApi.scripting?.registerContentScripts) {
    try {
      await browserApi.scripting.unregisterContentScripts({ ids: ["ghostlight-glass"] }).catch(() => {});
      await browserApi.scripting.registerContentScripts([{
        id: "ghostlight-glass",
        matches: ["http://*/*", "https://*/*"],
        js: [{ code: script }],
        runAt: "document_start",
        allFrames: true
      }]);
    } catch (error) {
      logDebug("Ghostlight preload script registration warning:", error);
    }
  }
  return { outcome: "set_preload_script" };
}

// Query physical tabs and format as PhysicalTab records
async function handleListTabs() {
  const rawTabs = await browserApi.tabs.query({});
  const tabs = rawTabs.map(physicalTab);
  return { outcome: "tabs", tabs };
}

// Focus a physical tab and bring its window to foreground
async function handleFocusTab(tabId) {
  const tab = await browserApi.tabs.update(tabId, { active: true });
  let windowFocused = true;
  if (tab?.windowId) {
    await browserApi.windows.update(tab.windowId, { focused: true }).catch(() => {});
  }
  return {
    outcome: "tab_focused",
    tab_id: tabId,
    active: true,
    window_focused: windowFocused
  };
}

async function waitForReady(tabId, timeoutMs = 4000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const tab = await browserApi.tabs.get(tabId);
      if (!tab || !tab.status || tab.status === "complete") return tab;
      if (tab.status !== "loading" && (tab.url || tab.pendingUrl)) return tab;
    } catch {
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  return browserApi.tabs.get(tabId).catch(() => null);
}

// Create a new tab and return its observed landing
async function handleOpenTab(url) {
  const targetUrl = url || "about:blank";
  const created = await browserApi.tabs.create({ url: targetUrl, active: true });
  const tab = (await waitForReady(created.id)) || created;
  return {
    outcome: "tab_opened",
    tab: physicalTab(tab),
    committed_urls: [targetUrl],
    reused: false
  };
}

// Close a physical tab
async function handleCloseTab(tabId) {
  await browserApi.tabs.remove(tabId);
  return {
    outcome: "tab_closed",
    tab_id: tabId
  };
}

// Navigate a physical tab to a new URL
async function handleNavigateTab(tabId, url) {
  await browserApi.tabs.update(tabId, { url });
  const tab = (await waitForReady(tabId)) || (await browserApi.tabs.get(tabId).catch(() => null));
  const updatedTab = tab || { id: tabId, url, active: true, status: "complete" };
  return {
    outcome: "navigated",
    tab: physicalTab(updatedTab),
    committed_urls: [url]
  };
}

// Capture screenshot of visible tab
async function handleCaptureScreenshot(tabId) {
  const options = { format: "jpeg", quality: 80 };
  let windowId = null;
  let targetTabId = tabId;
  if (targetTabId) {
    try {
      const tab = await browserApi.tabs.get(targetTabId);
      if (tab?.windowId) windowId = tab.windowId;
    } catch {}
  }
  if (!windowId) {
    const current = await browserApi.windows.getCurrent().catch(() => null);
    windowId = current?.id ?? null;
  }
  if (!targetTabId) {
    const activeTabs = await browserApi.tabs.query({ active: true, currentWindow: true }).catch(() => []);
    targetTabId = activeTabs[0]?.id ?? 1;
  }

  let dataUrl;
  if (typeof browserApi.tabs.captureTab === "function" && targetTabId) {
    dataUrl = await browserApi.tabs.captureTab(targetTabId, options);
  } else if (typeof browserApi.tabs.captureVisibleTab === "function") {
    dataUrl = windowId != null
      ? await browserApi.tabs.captureVisibleTab(windowId, options)
      : await browserApi.tabs.captureVisibleTab(options);
  } else {
    throw new Error("neither captureTab nor captureVisibleTab is available");
  }
  const base64Data = dataUrl ? dataUrl.replace(/^data:image\/[a-z]+;base64,/, "") : "";
  const win = windowId ? await browserApi.windows.get(windowId).catch(() => null) : null;
  const width = win?.width || 1280;
  const height = win?.height || 800;

  return {
    outcome: "screenshot",
    tab_id: targetTabId,
    mime_type: "image/jpeg",
    data: base64Data,
    width,
    height,
    viewport: {
      scope: "viewport",
      page_x: 0,
      page_y: 0,
      css_width: width,
      css_height: height,
      visual_page_x: 0,
      visual_page_y: 0,
      visual_css_width: width,
      visual_css_height: height,
      device_scale: 1,
      zoom: 1,
      output_scale: 1
    }
  };
}

// Read or update window geometry
async function handleWindowGeometry(tabId, width, height, bounds) {
  let targetWidth = width || bounds?.width || 1280;
  let targetHeight = height || bounds?.height || 800;
  let windowId = bounds?.window_id;
  let targetTabId = tabId || 1;

  if (targetTabId) {
    try {
      const tab = await browserApi.tabs.get(targetTabId);
      if (tab?.windowId) windowId = tab.windowId;
    } catch {}
  }
  if (!windowId) {
    const current = await browserApi.windows.getCurrent().catch(() => null);
    windowId = current?.id;
  }
  if (windowId) {
    await browserApi.windows.update(windowId, { width: targetWidth, height: targetHeight }).catch(() => {});
  }
  const win = windowId ? await browserApi.windows.get(windowId).catch(() => null) : null;
  return {
    outcome: "window_resized",
    tab_id: targetTabId,
    width: win?.width || targetWidth,
    height: win?.height || targetHeight,
    affected_tab_ids: [targetTabId]
  };
}

// Listen to focus changes to notify orchestrator of user attention
if (browserApi.windows?.onFocusChanged) {
  browserApi.windows.onFocusChanged.addListener(async (windowId) => {
    if (!nativePort) return;
    if (windowId === browserApi.windows.WINDOW_ID_NONE) return;
    send(shared.browserEventFrame({
      event: "attended"
    }));
  });
}

// Notify orchestrator when tab closes
if (browserApi.tabs?.onRemoved) {
  browserApi.tabs.onRemoved.addListener((tabId) => {
    if (!nativePort) return;
    send(shared.browserEventFrame({
      event: "tab_closed",
      tab_id: tabId
    }));
  });
}

// Notify orchestrator when tab readiness changes
if (browserApi.tabs?.onUpdated) {
  browserApi.tabs.onUpdated.addListener((tabId, changeInfo) => {
    if (!nativePort || !changeInfo.status) return;
    send(shared.browserEventFrame({
      event: "readiness_changed",
      tab_id: tabId,
      readiness: shared.readinessForStatus(changeInfo.status)
    }));
  });
}

// Notify orchestrator when a top-level document commits
if (browserApi.webNavigation?.onCommitted) {
  browserApi.webNavigation.onCommitted.addListener((details) => {
    if (!nativePort || details.frameId !== 0) return;
    send(shared.browserEventFrame({
      event: "document_committed",
      tab_id: details.tabId,
      url: details.url
    }));
  });
}

// Listen to extension lifecycle events
if (browserApi.runtime?.onInstalled) {
  browserApi.runtime.onInstalled.addListener((details) => {
    logDebug("browserApi.runtime.onInstalled fired", details?.reason);
    connectNative("installed");
    if (details?.reason === "install") {
      browserApi.tabs.create({ url: SERVICE_INSTALL_URL }).catch(() => {});
    }
  });
}
if (browserApi.runtime?.onStartup) {
  browserApi.runtime.onStartup.addListener(() => {
    logDebug("browserApi.runtime.onStartup fired");
    connectNative("browser_startup");
  });
}

// Start native connection on startup in extension environment
if (typeof module === "undefined") {
  connectNative("startup");
}

// Export internals for test harnesses
if (typeof module !== "undefined" && module.exports) {
  module.exports = {
    connectNative,
    executeCommand,
    executeRequest,
    handleNativeMessage,
    handleListTabs,
    handleFocusTab,
    handleOpenTab,
    handleCloseTab,
    handleNavigateTab,
    handleSetPreloadScript,
    handleCaptureScreenshot,
    handleWindowGeometry,
    getConnectionState: () => ({ ...connectionState }),
    getBrowserId: () => browserId,
    setBrowserId: (id) => { browserId = id; },
    getAdapterEpoch: () => adapterEpoch,
    setAdapterEpoch: (epoch) => { adapterEpoch = epoch; }
  };
}
