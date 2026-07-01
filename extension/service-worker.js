// Browser MCP -- background service worker.
//
// Policy-free CDP executor + native-messaging endpoint + tab-group manager. It holds MECHANISM
// only; all governance (domains, tool classification, audit) lives in the Rust binary. It receives
// { id, type: "tool_request", tool, args } and replies { id, type: "tool_response", result } or
// { id, type: "tool_error", error }. Chrome frames native messages (4-byte LE) for us via the Port.

const NATIVE_HOST = "org.sylin.browser_mcp";
const GROUP_TITLE = "Browser MCP";

let nativePort = null;
let groupId = null;
const attached = new Map(); // tabId -> { domains: Set<string> }
const consoleBuffer = new Map(); // tabId -> [{ level, text }]
const networkBuffer = new Map(); // tabId -> [{ requestId, method, url, status, mimeType }]

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
function fail(id, error) {
  try { nativePort && nativePort.postMessage({ id, type: "tool_error", error: String(error) }); } catch { /* port gone */ }
}

// --- CDP ---
const attaching = new Map(); // tabId -> in-flight attach promise (prevents concurrent double-attach)
async function ensureAttached(tabId) {
  if (attached.has(tabId)) return;
  if (attaching.has(tabId)) return attaching.get(tabId);
  const p = (async () => {
    await chrome.debugger.attach({ tabId }, "1.3");
    attached.set(tabId, { domains: new Set() });
    await applyDeviceMetrics(tabId);
  })();
  attaching.set(tabId, p);
  try { await p; } finally { attaching.delete(tabId); }
}
// deviceScaleFactor:1 so screenshot pixels match Input.dispatch coordinates regardless of DPI.
async function applyDeviceMetrics(tabId) {
  const tab = await chrome.tabs.get(tabId);
  const win = await chrome.windows.get(tab.windowId);
  await chrome.debugger.sendCommand({ tabId }, "Emulation.setDeviceMetricsOverride", {
    width: win.width,
    height: win.height,
    deviceScaleFactor: 1,
    mobile: false,
  });
}
async function cdp(tabId, method, params) {
  await ensureAttached(tabId);
  return chrome.debugger.sendCommand({ tabId }, method, params || {});
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
});
chrome.debugger.onDetach.addListener((src) => attached.delete(src.tabId));

// --- Console / network buffering (join network events by requestId, unlike the reference) ---
chrome.debugger.onEvent.addListener((src, method, params) => {
  const tabId = src.tabId;
  if (method === "Runtime.consoleAPICalled") {
    // Single console source. Both the Runtime domain (Runtime.consoleAPICalled) and the
    // deprecated Console domain (Console.messageAdded) report the same console.* call, so
    // enabling and buffering both double-counts every message. We keep only the richer
    // Runtime event (structured args + method-accurate `type`) and never enable Console.
    const text = (params.args || []).map((a) => a.value !== undefined ? a.value : (a.description || "")).join(" ");
    pushCapped(consoleBuffer, tabId, { level: params.type || "log", text });
  } else if (method === "Network.requestWillBeSent" && params.request) {
    pushCapped(networkBuffer, tabId, { requestId: params.requestId, method: params.request.method, url: params.request.url, status: 0 });
  } else if (method === "Network.responseReceived" && params.response) {
    const arr = networkBuffer.get(tabId) || [];
    const existing = arr.find((r) => r.requestId === params.requestId);
    if (existing) { existing.status = params.response.status; existing.mimeType = params.response.mimeType; }
    else pushCapped(networkBuffer, tabId, { requestId: params.requestId, method: "?", url: params.response.url, status: params.response.status, mimeType: params.response.mimeType });
  }
});
function pushCapped(map, tabId, item) {
  const arr = map.get(tabId) || [];
  arr.push(item);
  if (arr.length > 1000) arr.splice(0, arr.length - 1000);
  map.set(tabId, arr);
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
    await chrome.scripting.executeScript({ target: { tabId }, files: ["content.js"] });
    return chrome.tabs.sendMessage(tabId, message);
  }
}

// --- MCP result helpers ---
function text(t) {
  return { content: [{ type: "text", text: t }] };
}
function textImage(t, base64) {
  return { content: [{ type: "text", text: t }, { type: "image", data: base64, mimeType: "image/jpeg" }] };
}

// --- Screenshot pipeline (JPEG quality 55, fall back to 30 above ~500KB) ---
async function screenshot(tabId) {
  await ensureAttached(tabId);
  const opts = { format: "jpeg", quality: 55, optimizeForSpeed: true, captureBeyondViewport: false };
  let r = await cdp(tabId, "Page.captureScreenshot", opts);
  if (r.data.length > 500000) {
    r = await cdp(tabId, "Page.captureScreenshot", { ...opts, quality: 30 });
  }
  return r.data;
}

// --- Input helpers ---
function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}
const KEY_MAP = {
  enter: "Enter", return: "Enter", tab: "Tab", escape: "Escape", esc: "Escape",
  backspace: "Backspace", delete: "Delete", space: " ",
  up: "ArrowUp", down: "ArrowDown", left: "ArrowLeft", right: "ArrowRight",
  arrowup: "ArrowUp", arrowdown: "ArrowDown", arrowleft: "ArrowLeft", arrowright: "ArrowRight",
  home: "Home", end: "End", pageup: "PageUp", pagedown: "PageDown",
};
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
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x, y, modifiers });
  await sleep(40);
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mousePressed", x, y, button, clickCount, modifiers });
  await sleep(40);
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button, clickCount, modifiers });
}
async function resolveCoords(tabId, args) {
  if (args.coordinate) return args.coordinate;
  if (args.ref) {
    const r = await content(tabId, { type: "refCoordinates", ref: args.ref });
    if (r && r.result) return [r.result.x, r.result.y];
  }
  return null;
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
  const code = keyCode(key);
  await cdp(tabId, "Input.dispatchKeyEvent", { type: "keyDown", key, code, modifiers });
  await cdp(tabId, "Input.dispatchKeyEvent", { type: "keyUp", key, code, modifiers });
  await sleep(20);
}
// Best-effort DOM `code` for a resolved key, so pages that branch on event.code / keyCode work.
function keyCode(key) {
  if (key.length === 1) {
    if (/[a-zA-Z]/.test(key)) return "Key" + key.toUpperCase();
    if (/[0-9]/.test(key)) return "Digit" + key;
  }
  return key; // named keys (Enter, Tab, ArrowUp, ...) use the key name as their code
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

  switch (a.action) {
    case "screenshot":
      return textImage("Screenshot captured (jpeg).", await screenshot(tabId));
    case "zoom":
      return textImage(`Zoom region ${JSON.stringify(a.region || [])} (jpeg).`, await screenshot(tabId));
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
      for (const ch of a.text) { await cdp(tabId, "Input.insertText", { text: ch }); await sleep(8); }
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
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseWheel", x: c[0], y: c[1], deltaX, deltaY, modifiers });
      await sleep(250);
      return textImage(`Scrolled ${dir} by ${amount}.`, await screenshot(tabId));
    }
    case "scroll_to": {
      if (a.ref) await content(tabId, { type: "scrollToRef", ref: a.ref });
      else if (a.coordinate) await cdp(tabId, "Runtime.evaluate", { expression: `window.scrollTo(${a.coordinate[0]}, ${a.coordinate[1]})` });
      await sleep(250);
      return text("Scrolled to target.");
    }
    case "left_click_drag": {
      if (!a.start_coordinate || !a.coordinate) return text("start_coordinate and coordinate are required.");
      const [sx, sy] = a.start_coordinate;
      const [ex, ey] = a.coordinate;
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x: sx, y: sy, modifiers });
      await sleep(40);
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mousePressed", x: sx, y: sy, button: "left", modifiers });
      await sleep(40);
      for (let i = 1; i <= 10; i++) {
        await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x: sx + ((ex - sx) * i) / 10, y: sy + ((ey - sy) * i) / 10, modifiers });
        await sleep(16);
      }
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseReleased", x: ex, y: ey, button: "left", modifiers });
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
    const r = await content(a.tabId, { type: "pageText" });
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
    if (r && r.result && r.result.error) return text(`Error: ${r.result.error}`);
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
    let msgs = consoleBuffer.get(a.tabId) || [];
    if (a.onlyErrors) msgs = msgs.filter((m) => ["error", "exception"].includes(m.level));
    if (a.pattern) {
      try { const re = new RegExp(a.pattern, "i"); msgs = msgs.filter((m) => re.test(m.text) || re.test(m.level)); }
      catch { msgs = msgs.filter((m) => m.text.includes(a.pattern)); }
    }
    msgs = msgs.slice(-(a.limit || 100));
    if (a.clear) consoleBuffer.set(a.tabId, []);
    return text(msgs.length ? msgs.map((m) => `[${m.level}] ${m.text}`).join("\n") : "No console messages matching the pattern.");
  },
  async read_network_requests(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    await ensureAttached(a.tabId);
    await enableDomain(a.tabId, "Network");
    let reqs = networkBuffer.get(a.tabId) || [];
    if (a.urlPattern) reqs = reqs.filter((r) => r.url.includes(a.urlPattern));
    reqs = reqs.slice(-(a.limit || 100));
    if (a.clear) networkBuffer.set(a.tabId, []);
    return text(reqs.length ? reqs.map((r) => `${r.method || "?"} ${r.url} ${r.status ? "-> " + r.status : "(pending)"}`).join("\n") : "No network requests matching the pattern.");
  },
  async resize_window(a) {
    if (!(await inGroup(a.tabId))) return text(`Tab ${a.tabId} is not in the group.`);
    const tab = await chrome.tabs.get(a.tabId);
    await chrome.windows.update(tab.windowId, { width: a.width, height: a.height });
    // Refresh the device-metrics override for every attached tab in this window so screenshots
    // and input coordinates track the new viewport.
    for (const tabId of attached.keys()) {
      try {
        const t = await chrome.tabs.get(tabId);
        if (t.windowId === tab.windowId) await applyDeviceMetrics(tabId);
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
    fail(id, `${tool} failed: ${(e && e.message) || e}`);
  }
}

connect();
