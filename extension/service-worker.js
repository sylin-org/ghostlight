// Browser MCP -- background service worker.
//
// Policy-free CDP executor + native-messaging endpoint + tab-group manager. It holds MECHANISM
// only; all governance (domains, tool classification, audit) lives in the Rust binary. It receives
// { id, type: "tool_request", tool, args } and replies { id, type: "tool_response", result } or
// { id, type: "tool_error", error, hop?, detail? }. `hop` (only ever "cdp" or "page") and `detail`
// are optional and are mechanism tags (which layer threw), never policy; an absent `hop` means the
// binary attributes the failure to the extension itself. Chrome frames native messages (4-byte LE)
// for us via the Port.

const NATIVE_HOST = "org.sylin.browser_mcp";
const GROUP_TITLE = "Browser MCP";

let nativePort = null;
let groupId = null;
const attached = new Map(); // tabId -> { domains: Set<string> }
const consoleBuffer = new Map(); // tabId -> { host, items: [{ level, text }] }
const networkBuffer = new Map(); // tabId -> { host, items: [{ requestId, method, url, status, mimeType, errorText, canceled }] }
const screenshotCtx = new Map(); // tabId -> { vpW, vpH, shotW, shotH, offX, offY, regionW, regionH } (set on each screenshot/zoom)
const tabHost = new Map(); // tabId -> hostname of the tab's current URL ("" when none)

// A rejected promise must not tear down the service worker.
self.addEventListener("unhandledrejection", (e) => e.preventDefault());

// --- Native messaging + Manifest V3 keepalive ---
chrome.alarms.create("keepalive", { periodInMinutes: 0.4 });
chrome.alarms.onAlarm.addListener((a) => {
  if (a.name === "keepalive" && !nativePort) connect();
});

function connect() {
  if (nativePort) return;
  try {
    nativePort = chrome.runtime.connectNative(NATIVE_HOST);
    nativePort.onMessage.addListener((msg) => {
      if (msg && msg.type === "tool_request" && msg.id) {
        dispatch(msg.id, msg.tool, msg.args || {});
      }
    });
    nativePort.onDisconnect.addListener(() => {
      nativePort = null;
      setTimeout(connect, 2000);
    });
  } catch {
    nativePort = null;
    setTimeout(connect, 2000);
  }
}

function reply(id, result) {
  try { nativePort && nativePort.postMessage({ id, type: "tool_response", result }); } catch { /* port gone */ }
}
// Tag an error with the hop (mechanism, not policy) that threw it, plus optional debug-only detail.
function hopError(hop, message, detail) {
  const err = new Error(message);
  err.hop = hop;
  if (detail) err.detail = String(detail);
  return err;
}
function fail(id, error) {
  const msg = { id, type: "tool_error", error: (error && error.message) || String(error) };
  if (error && error.hop) msg.hop = error.hop;
  if (error && error.detail) msg.detail = error.detail;
  try { nativePort && nativePort.postMessage(msg); } catch { /* port gone */ }
}

// --- CDP ---
const attaching = new Map(); // tabId -> in-flight attach promise (prevents concurrent double-attach)
async function ensureAttached(tabId) {
  if (attached.has(tabId)) return;
  if (attaching.has(tabId)) return attaching.get(tabId);
  const p = (async () => {
    try {
      await chrome.debugger.attach({ tabId }, "1.3");
    } catch (e) {
      throw hopError("cdp", `debugger attach failed: ${(e && e.message) || e}`);
    }
    attached.set(tabId, { domains: new Set() });
    try {
      const t = await chrome.tabs.get(tabId);
      tabHost.set(tabId, hostOf(t.url || ""));
    } catch { /* tab gone */ }
  })();
  attaching.set(tabId, p);
  try { await p; } finally { attaching.delete(tabId); }
}
// Coordinate model (harvest step 4, official v1.0.78): NO device-metrics override. Each screenshot
// probes the CSS viewport + DPR, captures at native resolution, downscales to a token budget, and
// records a per-tab ScreenshotContext. Model coordinates (read off that downscaled image) are then
// rescaled back to CSS viewport pixels before Input dispatch. ref-derived coordinates are already
// CSS px and are NOT rescaled.
const PX_PER_TOKEN = 28, MAX_TOKENS = 1568, MAX_SIDE = 1568, MAX_SCREENSHOT_B64 = 1100000;

async function probeViewport(tabId) {
  const r = await cdp(tabId, "Runtime.evaluate", {
    expression: "({w:innerWidth,h:innerHeight,d:window.devicePixelRatio||1})",
    returnByValue: true,
  });
  const v = r && r.result && r.result.value;
  if (!v || !v.w || !v.h) throw hopError("page", "failed to probe viewport");
  return { vpW: v.w, vpH: v.h, dpr: v.d || 1 };
}
// Target screenshot dimensions (derived from the CSS viewport) under the token + longest-side budget.
function targetDims(vpW, vpH) {
  let w = vpW, h = vpH;
  const tokens = Math.ceil(w / PX_PER_TOKEN) * Math.ceil(h / PX_PER_TOKEN);
  if (tokens > MAX_TOKENS) { const s = Math.sqrt(MAX_TOKENS / tokens); w = Math.round(w * s); h = Math.round(h * s); }
  const longest = Math.max(w, h);
  if (longest > MAX_SIDE) { const s = MAX_SIDE / longest; w = Math.round(w * s); h = Math.round(h * s); }
  return { w: Math.max(1, w), h: Math.max(1, h) };
}
// Largest capture scale for a region of CSS size w x h that keeps the output inside the token +
// longest-side budget; magnifies a small region, shrinks a large one.
function zoomScale(w, h) {
  let s = Math.min(MAX_SIDE / Math.max(w, h), Math.sqrt((MAX_TOKENS * PX_PER_TOKEN * PX_PER_TOKEN) / (w * h)));
  while (s > 0 && Math.ceil(Math.round(w * s) / PX_PER_TOKEN) * Math.ceil(Math.round(h * s) / PX_PER_TOKEN) > MAX_TOKENS) s *= 0.98;
  return s;
}
function bytesFromBase64(b64) {
  const bin = atob(b64), bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}
function base64FromBytes(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}
async function encodeJpeg(bitmap, w, h, quality) {
  const canvas = new OffscreenCanvas(w, h);
  const ctx = canvas.getContext("2d");
  ctx.drawImage(bitmap, 0, 0, w, h);
  const blob = await canvas.convertToBlob({ type: "image/jpeg", quality });
  return base64FromBytes(new Uint8Array(await blob.arrayBuffer()));
}
// Map a model-provided coordinate (read off the downscaled screenshot) back to CSS viewport px.
// Passthrough when no screenshot has been taken for the tab (nothing to map against). A zoomed
// capture carries a region offset (offX, offY) that the mapped point is added back onto.
function rescaleCoord(tabId, x, y) {
  const c = screenshotCtx.get(tabId);
  if (!c || !c.shotW || !c.shotH) return [Math.round(x), Math.round(y)];
  const rw = c.regionW || c.vpW, rh = c.regionH || c.vpH;
  return [Math.round((c.offX || 0) + (x * rw) / c.shotW), Math.round((c.offY || 0) + (y * rh) / c.shotH)];
}
async function cdp(tabId, method, params) {
  await ensureAttached(tabId);
  try {
    return await chrome.debugger.sendCommand({ tabId }, method, params || {});
  } catch (e) {
    throw hopError("cdp", `${method} failed: ${(e && e.message) || e}`);
  }
}
async function enableDomain(tabId, domain) {
  const state = attached.get(tabId);
  if (!state) throw new Error("not attached");
  if (state.domains.has(domain)) return;
  await chrome.debugger.sendCommand({ tabId }, domain + ".enable", {});
  state.domains.add(domain);
}
chrome.tabs.onRemoved.addListener((tabId) => {
  if (attached.has(tabId)) {
    try { chrome.debugger.detach({ tabId }); } catch { /* already gone */ }
    attached.delete(tabId);
  }
  consoleBuffer.delete(tabId);
  networkBuffer.delete(tabId);
  screenshotCtx.delete(tabId);
  tabHost.delete(tabId);
});
chrome.debugger.onDetach.addListener((src) => attached.delete(src.tabId));

// --- Console / network buffering (join network events by requestId, unlike the reference) ---
function hostOf(url) {
  try { return new URL(url).hostname; } catch { return ""; }
}
chrome.tabs.onUpdated.addListener((tabId, info) => {
  if (info.url !== undefined) tabHost.set(tabId, hostOf(info.url));
});
// Render an uncaught-exception CDP event as one single-line string: base message, then an
// optional (url:line) location, then an optional compact [at frame, frame, ...] stack.
function exceptionText(details) {
  const exc = details.exception;
  let base;
  if (exc && typeof exc.description === "string" && exc.description) {
    base = exc.description.split("\n")[0];
  } else if (exc && exc.value !== undefined) {
    base = String(exc.value);
  } else if (typeof details.text === "string" && details.text) {
    base = details.text;
  } else {
    base = "Uncaught exception";
  }
  let out = base;
  if (typeof details.url === "string" && details.url) {
    // CDP line numbers are 0-based; add 1 for the human-readable line reported here.
    out += typeof details.lineNumber === "number" ? ` (${details.url}:${details.lineNumber + 1})` : ` (${details.url})`;
  }
  const frames = details.stackTrace && Array.isArray(details.stackTrace.callFrames) ? details.stackTrace.callFrames : [];
  if (frames.length) {
    const rendered = frames.slice(0, 3).map((f) => `${f.functionName || "<anonymous>"}@${f.url}:${f.lineNumber + 1}`);
    out += ` [at ${rendered.join(", ")}]`;
  }
  return out;
}
chrome.debugger.onEvent.addListener((src, method, params) => {
  const tabId = src.tabId;
  if (method === "Runtime.consoleAPICalled") {
    // Single console source. Both the Runtime domain (Runtime.consoleAPICalled) and the
    // deprecated Console domain (Console.messageAdded) report the same console.* call, so
    // enabling and buffering both double-counts every message. We keep only the richer
    // Runtime event (structured args + method-accurate `type`) and never enable Console.
    const text = (params.args || []).map((a) => a.value !== undefined ? a.value : (a.description || "")).join(" ");
    pushCapped(consoleBuffer, tabId, { level: params.type || "log", text });
  } else if (method === "Runtime.exceptionThrown") {
    pushCapped(consoleBuffer, tabId, { level: "exception", text: exceptionText(params.exceptionDetails || {}) });
  } else if (method === "Network.requestWillBeSent" && params.request) {
    pushCapped(networkBuffer, tabId, { requestId: params.requestId, method: params.request.method, url: params.request.url, status: 0 });
  } else if (method === "Network.responseReceived" && params.response) {
    const buf = bufferFor(networkBuffer, tabId, tabHost.get(tabId));
    const existing = buf.items.find((r) => r.requestId === params.requestId);
    if (existing) { existing.status = params.response.status; existing.mimeType = params.response.mimeType; }
    else pushCapped(networkBuffer, tabId, { requestId: params.requestId, method: "?", url: params.response.url, status: params.response.status, mimeType: params.response.mimeType });
  } else if (method === "Network.loadingFailed" && params.requestId) {
    const buf = bufferFor(networkBuffer, tabId, tabHost.get(tabId));
    const existing = buf.items.find((r) => r.requestId === params.requestId);
    if (existing) {
      existing.status = 503;
      if (params.errorText) existing.errorText = params.errorText;
      existing.canceled = !!params.canceled;
    }
  }
});
// Buffers are owned by the tab's current hostname, per the read_console_messages /
// read_network_requests schema contract; a hostname change replaces the buffer with a fresh one.
function bufferFor(map, tabId, host) {
  let buf = map.get(tabId);
  if (!buf || (host !== undefined && buf.host !== undefined && buf.host !== host)) {
    buf = { host, items: [] };
    map.set(tabId, buf);
  } else if (buf.host === undefined && host !== undefined) {
    buf.host = host; // entries captured before the host was known belong to the first host learned
  }
  return buf;
}
function pushCapped(map, tabId, item) {
  const buf = bufferFor(map, tabId, tabHost.get(tabId));
  buf.items.push(item);
  if (buf.items.length > 1000) buf.items.splice(0, buf.items.length - 1000);
}

// --- Tab group (created lazily; recovered from live state after a service-worker restart) ---
async function ensureGroup(create) {
  if (groupId !== null) {
    try { await chrome.tabGroups.get(groupId); return; } catch { groupId = null; }
  }
  const groups = await chrome.tabGroups.query({ title: GROUP_TITLE });
  if (groups.length) { groupId = groups[0].id; return; }
  if (!create) return;
  const win = await chrome.windows.create({ focused: true, url: "about:blank" });
  const gid = await chrome.tabs.group({ tabIds: [win.tabs[0].id] });
  await chrome.tabGroups.update(gid, { title: GROUP_TITLE, color: "blue" });
  groupId = gid;
}
async function groupTabs() {
  return groupId === null ? [] : chrome.tabs.query({ groupId });
}
async function inGroup(tabId) {
  // Always consult live state; the in-memory groupId can be stale after a restart.
  try {
    const tab = await chrome.tabs.get(tabId);
    if (tab.groupId !== -1 && groupId === null) {
      const g = await chrome.tabGroups.get(tab.groupId);
      if (g.title === GROUP_TITLE) groupId = g.id;
    }
    return tab.groupId === groupId;
  } catch {
    return false;
  }
}
function tabContext(tabs) {
  const available = tabs.map((t) => ({ tabId: t.id, title: t.title || "", url: t.url || "" }));
  return text(JSON.stringify({ mcpGroupId: groupId, tabs: available }, null, 2));
}

// --- Content-script bridge (inject on demand) ---
async function content(tabId, message) {
  try {
    return await chrome.tabs.sendMessage(tabId, message);
  } catch {
    try {
      await chrome.scripting.executeScript({ target: { tabId }, files: ["content.js"] });
      return await chrome.tabs.sendMessage(tabId, message);
    } catch (e) {
      throw hopError(
        "page",
        "content script unavailable on this page (script injection blocked)",
        (e && e.message) || e
      );
    }
  }
}

// --- MCP result helpers ---
function text(t) {
  return { content: [{ type: "text", text: t }] };
}
function textImage(t, base64) {
  return { content: [{ type: "text", text: t }, { type: "image", data: base64, mimeType: "image/jpeg" }] };
}

// --- Screenshot pipeline: capture native, downscale to the token budget, record ScreenshotContext ---
async function screenshot(tabId) {
  await ensureAttached(tabId);
  const { vpW, vpH, dpr } = await probeViewport(tabId);
  // Hide the phantom cursor / glow so they never appear in the model's screenshot.
  await sendToTab(tabId, { type: "HIDE_FOR_TOOL_USE" });
  await sleep(40);
  let cap;
  try {
    cap = await cdp(tabId, "Page.captureScreenshot", { format: "jpeg", quality: 80, captureBeyondViewport: false });
  } finally {
    sendToTab(tabId, { type: "SHOW_AFTER_TOOL_USE" });
  }
  const { w, h } = targetDims(vpW, vpH);
  // Default to the raw native capture (dims = CSS viewport * DPR) if canvas downscaling is unavailable.
  let base64 = cap.data, shotW = Math.round(vpW * dpr), shotH = Math.round(vpH * dpr);
  try {
    const bitmap = await createImageBitmap(new Blob([bytesFromBase64(cap.data)], { type: "image/jpeg" }));
    base64 = await encodeJpeg(bitmap, w, h, 0.55);
    if (base64.length > MAX_SCREENSHOT_B64) base64 = await encodeJpeg(bitmap, w, h, 0.3);
    shotW = w; shotH = h;
    if (bitmap.close) bitmap.close();
  } catch { /* OffscreenCanvas/createImageBitmap unavailable: keep the raw native capture */ }
  // A full screenshot resets the zoom offset: subsequent coordinates map against the whole viewport.
  screenshotCtx.set(tabId, { vpW, vpH, shotW, shotH, offX: 0, offY: 0, regionW: vpW, regionH: vpH });
  return base64;
}

// --- Zoom: capture a clipped, magnified region and record it as the tab's coordinate context ---
async function zoomScreenshot(tabId, region) {
  await ensureAttached(tabId);
  const r = await cdp(tabId, "Runtime.evaluate", {
    expression: "({w:innerWidth,h:innerHeight,sx:window.scrollX||0,sy:window.scrollY||0})",
    returnByValue: true,
  });
  const v = r && r.result && r.result.value;
  if (!v || !v.w || !v.h) throw hopError("page", "failed to probe viewport");
  const vpW = v.w, vpH = v.h, sx = v.sx || 0, sy = v.sy || 0;
  // Rescale against the context as it was BEFORE this zoom, so a zoom issued against a previous
  // zoomed screenshot composes correctly (chained zooms).
  const [rx0, ry0] = rescaleCoord(tabId, region[0], region[1]);
  const [rx1, ry1] = rescaleCoord(tabId, region[2], region[3]);
  const x0 = Math.min(Math.max(rx0, 0), vpW), y0 = Math.min(Math.max(ry0, 0), vpH);
  const x1 = Math.min(Math.max(rx1, 0), vpW), y1 = Math.min(Math.max(ry1, 0), vpH);
  const clamped = x0 !== rx0 || y0 !== ry0 || x1 !== rx1 || y1 !== ry1;
  const w = x1 - x0, h = y1 - y0;
  if (w < 1 || h < 1) return { error: "zoom region is empty or entirely outside the visible viewport." };
  const s = zoomScale(w, h);
  await sendToTab(tabId, { type: "HIDE_FOR_TOOL_USE" });
  await sleep(40);
  let cap;
  try {
    cap = await cdp(tabId, "Page.captureScreenshot", {
      format: "jpeg", quality: 80,
      // clip is document-relative CSS pixels, not viewport-relative, so the scroll offset is added.
      clip: { x: sx + x0, y: sy + y0, width: w, height: h, scale: s },
      captureBeyondViewport: false,
    });
  } finally {
    sendToTab(tabId, { type: "SHOW_AFTER_TOOL_USE" });
  }
  let shotW = Math.max(1, Math.round(w * s)), shotH = Math.max(1, Math.round(h * s));
  let base64 = cap.data;
  try {
    const bitmap = await createImageBitmap(new Blob([bytesFromBase64(cap.data)], { type: "image/jpeg" }));
    base64 = await encodeJpeg(bitmap, bitmap.width, bitmap.height, 0.55);
    if (base64.length > MAX_SCREENSHOT_B64) base64 = await encodeJpeg(bitmap, bitmap.width, bitmap.height, 0.3);
    shotW = bitmap.width; shotH = bitmap.height;
    if (bitmap.close) bitmap.close();
  } catch { /* OffscreenCanvas/createImageBitmap unavailable: keep the raw native capture */ }
  screenshotCtx.set(tabId, { vpW, vpH, shotW, shotH, offX: x0, offY: y0, regionW: w, regionH: h });
  return { base64, x0, y0, x1, y1, clamped };
}

// --- Input helpers ---
function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}
// --- Visual indicator (best-effort; the content script is absent on chrome:// and similar pages) ---
function sendToTab(tabId, msg) {
  return chrome.tabs.sendMessage(tabId, msg).catch(() => {});
}
function showActivity(tabId) { sendToTab(tabId, { type: "SHOW_AGENT_INDICATORS" }); }
// Move the phantom cursor to a (rescaled, CSS-px) point and wait for it to settle, so the user sees
// the pointer arrive before the action fires. Resolves immediately if no indicator is present.
function moveCursor(tabId, x, y) { return sendToTab(tabId, { type: "UPDATE_PHANTOM_CURSOR", x, y }); }
const KEY_MAP = {
  enter: "Enter", return: "Enter", tab: "Tab", escape: "Escape", esc: "Escape",
  backspace: "Backspace", delete: "Delete", space: " ",
  up: "ArrowUp", down: "ArrowDown", left: "ArrowLeft", right: "ArrowRight",
  arrowup: "ArrowUp", arrowdown: "ArrowDown", arrowleft: "ArrowLeft", arrowright: "ArrowRight",
  home: "Home", end: "End", pageup: "PageUp", pagedown: "PageDown",
};
// DOM MouseEvent.buttons bitmask per button name.
const BUTTON_BITS = { left: 1, right: 2, middle: 4 };
// Delay between press and release, and between click iterations, matching this file's rhythm.
const CLICK_GAP_MS = 40;
function modifierBits(str) {
  let bits = 0;
  for (const p of (str || "").toLowerCase().split("+").map((x) => x.trim())) {
    if (p === "ctrl" || p === "control") bits |= 2;
    else if (p === "alt") bits |= 1;
    else if (p === "shift") bits |= 8;
    else if (["meta", "cmd", "command", "win", "windows"].includes(p)) bits |= 4;
  }
  return bits;
}
async function click(tabId, x, y, opts) {
  const modifiers = opts.modifiers || 0, button = opts.button || "left", clickCount = opts.clickCount || 1;
  const bit = BUTTON_BITS[button] || 0;
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x, y, modifiers, buttons: 0, force: 0 });
  await sleep(CLICK_GAP_MS);
  // Real N-clicks are N press/release pairs with clickCount incrementing 1..N, not one pair with
  // clickCount set to N.
  for (let i = 1; i <= clickCount; i++) {
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount: i, modifiers, buttons: bit, force: 0.5 });
    await sleep(CLICK_GAP_MS);
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount: i, modifiers, buttons: 0, force: 0 });
    if (i < clickCount) await sleep(CLICK_GAP_MS);
  }
}
async function resolveCoords(tabId, args) {
  // Model-provided coordinates are read off the (downscaled) screenshot -> rescale to CSS px.
  if (args.coordinate) return rescaleCoord(tabId, args.coordinate[0], args.coordinate[1]);
  // ref coordinates come from getBoundingClientRect (already CSS viewport px) -> do NOT rescale.
  if (args.ref) {
    const r = await content(tabId, { type: "refCoordinates", ref: args.ref });
    if (r && r.result) return [r.result.x, r.result.y];
    // The engine is truthful: a stale ref is a failure, never a silent [0, 0] substitution.
    throw hopError("page", `Element ${args.ref} not found; the page may have changed since it was read`);
  }
  return null;
}
// Scrollable-ancestor predicate shared by probeScrollState and directScrollFallback: an element
// counts as scrollable when its computed overflow allows scrolling AND its content overflows.
const SCROLLABLE_FINDER_SNIPPET = `
function findScrollable(px, py) {
  let el = document.elementFromPoint(px, py);
  while (el) {
    const cs = getComputedStyle(el);
    const overflowScrollable = cs.overflowY === "auto" || cs.overflowY === "scroll" || cs.overflowX === "auto" || cs.overflowX === "scroll";
    const sizeScrollable = el.scrollHeight > el.clientHeight || el.scrollWidth > el.clientWidth;
    if (overflowScrollable && sizeScrollable) return el;
    el = el.parentElement;
  }
  return null;
}`;
// Reads the window scroll position plus the scrollable-ancestor state at (x, y), for a before/
// after comparison around a wheel dispatch. Resolves to null (never throws) on any failure.
async function probeScrollState(tabId, x, y) {
  const px = Math.round(x), py = Math.round(y);
  const expression = `(() => {${SCROLLABLE_FINDER_SNIPPET}
    const el = findScrollable(${px}, ${py});
    return {
      winX: window.scrollX, winY: window.scrollY,
      hasEl: !!el,
      elX: el ? el.scrollLeft : null,
      elY: el ? el.scrollTop : null,
    };
  })()`;
  try {
    const r = await cdp(tabId, "Runtime.evaluate", { expression, returnByValue: true });
    if (!r || r.exceptionDetails || !r.result || r.result.value === undefined) return null;
    return r.result.value;
  } catch {
    return null;
  }
}
// Direct scrollBy on the nearest scrollable ancestor (or window), used when a dispatched wheel
// event did not move anything (preventDefault, virtualized lists, etc). Resolves to null (never
// throws) on any failure. dx/dy must be the same deltaX/deltaY already computed for the wheel.
async function directScrollFallback(tabId, x, y, dx, dy) {
  const px = Math.round(x), py = Math.round(y), pdx = Math.round(dx), pdy = Math.round(dy);
  const expression = `(() => {${SCROLLABLE_FINDER_SNIPPET}
    const el = findScrollable(${px}, ${py});
    const target = el || window;
    const beforeX = el ? el.scrollLeft : window.scrollX;
    const beforeY = el ? el.scrollTop : window.scrollY;
    target.scrollBy({ left: ${pdx}, top: ${pdy}, behavior: "instant" });
    const afterX = el ? el.scrollLeft : window.scrollX;
    const afterY = el ? el.scrollTop : window.scrollY;
    // 5px threshold matches the moved-more-than-5px verification contract.
    return { moved: Math.abs(afterX - beforeX) > 5 || Math.abs(afterY - beforeY) > 5, usedWindow: !el };
  })()`;
  try {
    const r = await cdp(tabId, "Runtime.evaluate", { expression, returnByValue: true });
    if (!r || r.exceptionDetails || !r.result || r.result.value === undefined) return null;
    return r.result.value;
  } catch {
    return null;
  }
}
async function pressKey(tabId, combo) {
  const parts = combo.split("+").map((p) => p.trim().toLowerCase());
  let modifiers = 0;
  let key = combo;
  if (parts.length > 1) {
    key = "";
    for (const p of parts) {
      if (p === "ctrl" || p === "control") modifiers |= 2;
      else if (p === "alt") modifiers |= 1;
      else if (p === "shift") modifiers |= 8;
      else if (["meta", "cmd", "command", "win", "windows"].includes(p)) modifiers |= 4;
      else key = KEY_MAP[p] || p;
    }
  } else {
    key = KEY_MAP[parts[0]] || combo;
  }
  // Reload chords (ctrl/cmd+r, F5): Chrome will not reload from a synthetic key event delivered to
  // the renderer, so intercept and drive the reload directly (shift => bypass cache / hard reload).
  const bare = (key || "").toLowerCase();
  const ctrlOrCmd = (modifiers & 2) !== 0 || (modifiers & 4) !== 0;
  if ((ctrlOrCmd && bare === "r") || bare === "f5") {
    await chrome.tabs.reload(tabId, { bypassCache: (modifiers & 8) !== 0 });
    return;
  }
  // Include the Windows virtual key code so Chrome maps modified combos (ctrl+a, ctrl+c, ...) to
  // real editing commands; without it a modified keyDown arrives but triggers no edit action.
  const code = keyCode(key);
  const vk = vkCode(key);
  const evt = { key, code, modifiers, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk };
  await cdp(tabId, "Input.dispatchKeyEvent", { type: "keyDown", ...evt });
  await cdp(tabId, "Input.dispatchKeyEvent", { type: "keyUp", ...evt });
  await sleep(20);
}
// Best-effort DOM `code` for a resolved key, so pages that branch on event.code / keyCode work.
function keyCode(key) {
  if (key.length === 1) {
    if (/[a-zA-Z]/.test(key)) return "Key" + key.toUpperCase();
    if (/[0-9]/.test(key)) return "Digit" + key;
    if (CODE_PUNCT[key]) return CODE_PUNCT[key];
  }
  return key; // named keys (Enter, Tab, ArrowUp, ...) use the key name as their code
}
// Windows virtual key codes, so Chrome interprets shortcuts (ctrl+a select-all, etc.) as commands.
const VK_NAMED = {
  Enter: 13, Tab: 9, Escape: 27, Backspace: 8, Delete: 46, " ": 32,
  ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39,
  Home: 36, End: 35, PageUp: 33, PageDown: 34, Insert: 45,
};
// Windows virtual key codes for US-QWERTY punctuation keys (VK_OEM_*).
const VK_PUNCT = {
  ";": 186, "=": 187, ",": 188, "-": 189, ".": 190, "/": 191,
  "`": 192, "[": 219, "\\": 220, "]": 221, "'": 222,
};
// DOM `code` values for US-QWERTY punctuation keys (and Space).
const CODE_PUNCT = {
  ";": "Semicolon", "=": "Equal", ",": "Comma", "-": "Minus",
  ".": "Period", "/": "Slash", "`": "Backquote", "[": "BracketLeft",
  "\\": "Backslash", "]": "BracketRight", "'": "Quote", " ": "Space",
};
function vkCode(key) {
  if (key.length === 1) {
    const up = key.toUpperCase();
    if (up >= "A" && up <= "Z") return up.charCodeAt(0); // A-Z -> 65-90
    if (key >= "0" && key <= "9") return key.charCodeAt(0); // 0-9 -> 48-57
    if (VK_PUNCT[key]) return VK_PUNCT[key];
  }
  return VK_NAMED[key] || 0;
}
// US-QWERTY: shifted printable -> the unshifted character on the same key.
const SHIFT_BASE = {
  "!": "1", "@": "2", "#": "3", "$": "4", "%": "5", "^": "6",
  "&": "7", "*": "8", "(": "9", ")": "0",
  "_": "-", "+": "=", "{": "[", "}": "]", "|": "\\", ":": ";",
  '"': "'", "<": ",", ">": ".", "?": "/", "~": "`",
};
// Resolve one typed character to Input.dispatchKeyEvent fields, or null when the character has no
// key mapping (control characters, non-ASCII) and must fall back to Input.insertText instead.
function charKeyInfo(ch) {
  if (ch === "\n" || ch === "\r") {
    return { key: "Enter", code: "Enter", vk: 13, shift: false, text: "\r", unmodifiedText: "\r" };
  }
  if (ch < " " || ch > "~") return null;
  let base = ch, shift = false;
  if (ch >= "A" && ch <= "Z") { base = ch.toLowerCase(); shift = true; }
  else if (SHIFT_BASE[ch]) { base = SHIFT_BASE[ch]; shift = true; }
  return { key: ch, code: keyCode(base), vk: vkCode(base), shift, text: ch, unmodifiedText: base };
}
function waitForLoad(tabId) {
  return new Promise((resolve) => {
    const listener = (id, info) => {
      if (id === tabId && info.status === "complete") {
        chrome.tabs.onUpdated.removeListener(listener);
        resolve();
      }
    };
    chrome.tabs.onUpdated.addListener(listener);
    setTimeout(() => { chrome.tabs.onUpdated.removeListener(listener); resolve(); }, 10000);
  });
}

// --- computer (13 actions; screenshots only on screenshot/scroll/zoom) ---
async function computer(a) {
  const tabId = a.tabId;
  if (!(await inGroup(tabId))) return text(`Tab ${tabId} is not in the ${GROUP_TITLE} group.`);
  const modifiers = modifierBits(a.modifiers);
  showActivity(tabId); // best-effort "agent active" glow for the watching user

  switch (a.action) {
    case "screenshot":
      return textImage("Screenshot captured (jpeg).", await screenshot(tabId));
    case "zoom": {
      const r = a.region;
      if (!Array.isArray(r) || r.length !== 4 || !r.every((v) => Number.isFinite(v)))
        return text("region [x0, y0, x1, y1] is required for zoom.");
      if (!(r[2] > r[0]) || !(r[3] > r[1]))
        return text("zoom region is empty: x1 must be greater than x0 and y1 must be greater than y0.");
      const z = await zoomScreenshot(tabId, r);
      if (z.error) return text(z.error);
      return textImage(`Zoom region (${z.x0}, ${z.y0}) -> (${z.x1}, ${z.y1}) captured (jpeg${z.clamped ? "; clamped to the visible viewport" : ""}).`, z.base64);
    }
    case "wait": {
      const s = Math.min(a.duration || 1, 30);
      await sleep(s * 1000);
      return text(`Waited ${s}s.`);
    }
    case "left_click":
    case "right_click":
    case "double_click":
    case "triple_click":
    case "hover": {
      const c = await resolveCoords(tabId, a);
      if (!c) return text("coordinate or ref is required.");
      await moveCursor(tabId, c[0], c[1]); // show the pointer arrive before acting
      if (a.action === "hover") {
        await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x: c[0], y: c[1], modifiers });
        return text(`Hovered at (${c[0]}, ${c[1]}).`);
      }
      const button = a.action === "right_click" ? "right" : "left";
      const clickCount = a.action === "double_click" ? 2 : a.action === "triple_click" ? 3 : 1;
      await click(tabId, c[0], c[1], { button, clickCount, modifiers });
      return text(`${a.action} at (${c[0]}, ${c[1]}).`);
    }
    case "type": {
      if (!a.text) return text("text is required for type.");
      await ensureAttached(tabId);
      const chars = Array.from(a.text);
      for (let i = 0; i < chars.length; i++) {
        const ch = chars[i];
        // Windows-style newlines: skip the \r, let the following \n press Enter once.
        if (ch === "\r" && chars[i + 1] === "\n") continue;
        const info = charKeyInfo(ch);
        if (!info) {
          await cdp(tabId, "Input.insertText", { text: ch });
          await sleep(8);
          continue;
        }
        const mods = info.shift ? 8 : 0;
        const evt = {
          key: info.key, code: info.code, modifiers: mods,
          windowsVirtualKeyCode: info.vk, nativeVirtualKeyCode: info.vk,
        };
        await cdp(tabId, "Input.dispatchKeyEvent", { type: "keyDown", ...evt, text: info.text, unmodifiedText: info.unmodifiedText });
        await cdp(tabId, "Input.dispatchKeyEvent", { type: "keyUp", ...evt });
        await sleep(8);
      }
      return text(`Typed ${a.text.length} character(s).`);
    }
    case "key": {
      if (!a.text) return text("text is required for key.");
      await ensureAttached(tabId);
      const repeat = Math.min(a.repeat || 1, 100);
      for (let i = 0; i < repeat; i++) {
        for (const combo of a.text.split(" ").filter(Boolean)) await pressKey(tabId, combo);
      }
      return text(`Pressed: ${a.text} (x${repeat}).`);
    }
    case "scroll": {
      const c = (await resolveCoords(tabId, a)) || [0, 0];
      const dir = a.scroll_direction || "down";
      const amount = Math.min(a.scroll_amount || 3, 10);
      const deltaX = dir === "left" ? -amount * 100 : dir === "right" ? amount * 100 : 0;
      const deltaY = dir === "up" ? -amount * 100 : dir === "down" ? amount * 100 : 0;
      const before = await probeScrollState(tabId, c[0], c[1]);
      await moveCursor(tabId, c[0], c[1]);
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseWheel", x: c[0], y: c[1], deltaX, deltaY, modifiers });
      const scrolled = `Scrolled ${dir} by ${amount}.`;
      if (before === null) {
        // Verification unavailable (for example a mid-navigation page): same blind claim as before.
        await sleep(250);
        return textImage(scrolled, await screenshot(tabId));
      }
      await sleep(200);
      const after = await probeScrollState(tabId, c[0], c[1]);
      // Re-read failed: do not run the fallback, a blind fallback risks double-scrolling.
      if (after === null) return textImage(scrolled, await screenshot(tabId));
      // 5px threshold matches the moved-more-than-5px verification contract.
      const windowMoved = Math.abs(after.winX - before.winX) > 5 || Math.abs(after.winY - before.winY) > 5;
      const elementMoved = before.hasEl && after.hasEl &&
        (Math.abs((after.elX || 0) - (before.elX || 0)) > 5 || Math.abs((after.elY || 0) - (before.elY || 0)) > 5);
      if (windowMoved || elementMoved) return textImage(scrolled, await screenshot(tabId));
      const fb = await directScrollFallback(tabId, c[0], c[1], deltaX, deltaY);
      if (fb === null) {
        return textImage(
          `Scroll ${dir} had no effect at (${c[0]}, ${c[1]}); the direct scroll fallback could not run.`,
          await screenshot(tabId)
        );
      }
      if (fb.moved) {
        return textImage(
          `Scrolled ${dir} by ${amount} (mouse wheel had no effect; used direct scroll fallback).`,
          await screenshot(tabId)
        );
      }
      return textImage(
        `Scroll ${dir} had no effect at (${c[0]}, ${c[1]}); the page did not move at that position.`,
        await screenshot(tabId)
      );
    }
    case "scroll_to": {
      if (a.ref) {
        const r = await content(tabId, { type: "scrollToRef", ref: a.ref });
        // The engine is truthful: a stale ref is a failure, never a false "Scrolled to target.".
        if (!(r && r.result)) {
          throw hopError("page", `Element ${a.ref} not found; the page may have changed since it was read`);
        }
      } else if (a.coordinate) {
        await cdp(tabId, "Runtime.evaluate", { expression: `window.scrollTo(${a.coordinate[0]}, ${a.coordinate[1]})` });
      }
      await sleep(250);
      return text("Scrolled to target.");
    }
    case "left_click_drag": {
      if (!a.start_coordinate || !a.coordinate) return text("start_coordinate and coordinate are required.");
      // Both endpoints are model-provided (read off the screenshot) -> rescale to CSS px.
      const [sx, sy] = rescaleCoord(tabId, a.start_coordinate[0], a.start_coordinate[1]);
      const [ex, ey] = rescaleCoord(tabId, a.coordinate[0], a.coordinate[1]);
      await moveCursor(tabId, sx, sy);
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x: sx, y: sy, modifiers, buttons: 0, force: 0 });
      await sleep(40);
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mousePressed", x: sx, y: sy, button: "left", modifiers, buttons: BUTTON_BITS.left, force: 0.5 });
      await sleep(40);
      for (let i = 1; i <= 10; i++) {
        await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x: sx + ((ex - sx) * i) / 10, y: sy + ((ey - sy) * i) / 10, modifiers, buttons: BUTTON_BITS.left, force: 0.5 });
        await sleep(16);
      }
      await moveCursor(tabId, ex, ey);
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseReleased", x: ex, y: ey, button: "left", modifiers, buttons: 0, force: 0 });
      return text(`Dragged (${sx}, ${sy}) -> (${ex}, ${ey}).`);
    }
    default:
      return text(`Unknown computer action: ${a.action}`);
  }
}

// --- Tool handlers ---
const handlers = {
  async tabs_context_mcp(a) {
    await ensureGroup(a.createIfEmpty);
    if (groupId === null) return text("No Browser MCP tab group. Call with createIfEmpty: true.");
    return tabContext(await groupTabs());
  },
  async tabs_create_mcp() {
    await ensureGroup(true);
    const tab = await chrome.tabs.create({ active: true });
    await chrome.tabs.group({ tabIds: [tab.id], groupId });
    const r = tabContext(await groupTabs());
    r.content[0].text = `Created tab ${tab.id}.\n` + r.content[0].text;
    return r;
  },
  async navigate(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the ${GROUP_TITLE} group.`);
    if (a.url === "back") {
      await chrome.tabs.goBack(a.tabId);
    } else if (a.url === "forward") {
      await chrome.tabs.goForward(a.tabId);
    } else {
      let url = a.url;
      if (!/^https?:\/\//i.test(url) && !/^(about|chrome|edge|brave):/i.test(url)) {
        url = "https://" + url.replace(/^[a-z]{1,6}:\/+/i, "");
      }
      try { new URL(url); } catch { return text(`Invalid URL: "${a.url}".`); }
      await chrome.tabs.update(a.tabId, { url });
    }
    await waitForLoad(a.tabId);
    const tab = await chrome.tabs.get(a.tabId);
    return text(`Navigated to ${tab.url}${tab.status !== "complete" ? " (still loading)" : ""}.`);
  },
  computer,
  async read_page(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    const r = await content(a.tabId, { type: "accessibilityTree", options: a });
    return text((r && r.result) || "Could not read the page.");
  },
  async get_page_text(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    const r = await content(a.tabId, { type: "pageText", max_chars: a.max_chars });
    return text((r && r.result) || "Could not extract page text.");
  },
  async find(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    const r = await content(a.tabId, { type: "find", query: a.query });
    const data = (r && r.result) || { results: [] };
    const results = data.results || [];
    if (!results.length) return text(`No elements matching "${a.query}".`);
    let out = `Found ${results.length} element(s):\n` + results.map((e) => `[${e.ref}] ${e.role} "${e.name}" at (${e.x}, ${e.y})`).join("\n");
    if (data.more) out += "\n(more than 20 matches; refine your query for the rest)";
    return text(out);
  },
  async form_input(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    const r = await content(a.tabId, { type: "setFormValue", ref: a.ref, value: a.value });
    // The engine is truthful: a content-script failure is a failure, never a masqueraded success.
    if (r && r.result && r.result.error) {
      const msg = r.result.error.endsWith(".") ? r.result.error.slice(0, -1) : r.result.error;
      throw hopError("page", msg);
    }
    return text(`Set ${a.ref} = ${JSON.stringify(a.value)}.`);
  },
  async javascript_tool(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    const r = await cdp(a.tabId, "Runtime.evaluate", { expression: a.text, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) return text(`Error: ${r.exceptionDetails.text || "exception"}`);
    const v = r.result;
    return text(v.value !== undefined ? JSON.stringify(v.value) : (v.description || String(v.type)));
  },
  async read_console_messages(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    await ensureAttached(a.tabId);
    // Only enable Runtime; the Console domain is the deprecated duplicate source (see onEvent).
    await enableDomain(a.tabId, "Runtime");
    const tab = await chrome.tabs.get(a.tabId);
    const host = hostOf(tab.url || "");
    tabHost.set(a.tabId, host);
    const buf = bufferFor(consoleBuffer, a.tabId, host);
    const total = buf.items.length;
    let msgs = buf.items;
    if (a.onlyErrors) msgs = msgs.filter((m) => ["error", "exception"].includes(m.level));
    if (a.pattern) {
      try { const re = new RegExp(a.pattern, "i"); msgs = msgs.filter((m) => re.test(m.text) || re.test(m.level)); }
      catch { msgs = msgs.filter((m) => m.text.includes(a.pattern)); }
    }
    msgs = msgs.slice(-(a.limit || 100));
    if (a.clear) consoleBuffer.set(a.tabId, { host, items: [] });
    if (msgs.length) return text(msgs.map((m) => `[${m.level}] ${m.text}`).join("\n"));
    const primary = total
      ? `${total} console message(s) recorded for this tab, but none matched your filter.`
      : "No console messages recorded for this tab.";
    return text(`${primary}\nNote: console tracking begins when this tool is first used on a tab. Reload the page to capture messages emitted during page load.`);
  },
  async read_network_requests(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    await ensureAttached(a.tabId);
    await enableDomain(a.tabId, "Network");
    const tab = await chrome.tabs.get(a.tabId);
    const host = hostOf(tab.url || "");
    tabHost.set(a.tabId, host);
    const buf = bufferFor(networkBuffer, a.tabId, host);
    const total = buf.items.length;
    let reqs = buf.items;
    if (a.urlPattern) reqs = reqs.filter((r) => r.url.includes(a.urlPattern));
    reqs = reqs.slice(-(a.limit || 100));
    if (a.clear) networkBuffer.set(a.tabId, { host, items: [] });
    if (reqs.length) return text(reqs.map((r) => `${r.method || "?"} ${r.url} ${r.status ? "-> " + r.status + (r.errorText ? " (" + r.errorText + ")" : "") : "(pending)"}`).join("\n"));
    const primary = total
      ? `${total} network request(s) recorded for this tab, but none matched your filter.`
      : "No network requests recorded for this tab.";
    return text(`${primary}\nNote: network tracking begins when this tool is first used on a tab. Reload the page to capture requests made during page load, or interact with the page to trigger new requests.`);
  },
  async resize_window(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    const tab = await chrome.tabs.get(a.tabId);
    await chrome.windows.update(tab.windowId, { width: a.width, height: a.height });
    // The viewport changed; drop any stale ScreenshotContext for this window's tabs so the next
    // screenshot re-establishes the coordinate mapping.
    for (const tabId of attached.keys()) {
      try {
        const t = await chrome.tabs.get(tabId);
        if (t.windowId === tab.windowId) screenshotCtx.delete(tabId);
      } catch { /* tab gone */ }
    }
    return text(`Resized window to ${a.width}x${a.height}.`);
  },
  async update_plan(a) {
    const domains = (a.domains || []).join(", ");
    const approach = (a.approach || []).map((s) => `- ${s}`).join("\n");
    return text(`Plan (auto-approved by the v1.0 engine):\nDomains: ${domains}\n${approach}`);
  },
};

async function dispatch(id, tool, args) {
  const handler = handlers[tool];
  if (!handler) return fail(id, `Unknown tool: ${tool}`);
  try {
    reply(id, await handler(args));
  } catch (e) {
    // Hop-tagged errors (cdp/page) pass through as-is; untagged errors keep the tool-name prefix.
    if (e && e.hop) fail(id, e);
    else fail(id, `${tool} failed: ${(e && e.message) || e}`);
  }
}

connect();
