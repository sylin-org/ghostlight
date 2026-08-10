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
  assert.match(renderer, /role", "status"/);
  assert.match(renderer, /aria-live", "polite"/);
  assert.match(renderer, /aria-atomic", "true"/);
  assert.match(renderer, /\.denial-ribbon\{animation:none!important\}/);
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

test("browser events use the nested typed bridge envelope", () => {
  assert.deepEqual(shared.browserEventFrame({ event: "tab_closed", tab_id: 7 }), {
    kind: "event",
    event: { event: "tab_closed", tab_id: 7 }
  });
});

test("the adapter advertises stable versioned physical capabilities", () => {
  assert.equal(shared.ADAPTER_PROTOCOL_MAJOR, 1);
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
    "presentation"
  ].map((name) => ({ name, revision: 1 })));
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
    ["alarms", "debugger", "nativeMessaging", "storage", "tabGroups", "tabs", "webNavigation", "windows"]
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
  assert.match(options, /id="preserve-tabs"/);
  assert.match(options, /You can always close tabs yourself\./);
});

test("workspace topology accepts only bounded Ghostlight group titles", () => {
  assert.equal(topology.GROUP_PREFIX, "Ghostlight - ");
  assert.equal(topology.GROUP_COLOR, "blue");
  assert.equal(topology.validTitle("Ghostlight - Codex"), true);
  assert.equal(topology.validTitle("Personal"), false);
});

test("workspace topology reuses the established exact-title blue group", async () => {
  let grouped = null;
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
