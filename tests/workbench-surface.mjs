// The workbench surface, actually executed.
//
// This runs the real modules against a minimal DOM with one panel deliberately broken, and
// asserts what a person would check by looking: the window still comes up, the failure is
// visible, the rest of the pass continues, and the broken panel is retried rather than
// remembered as finished. Every check reads what the page drew, never the page's source.
//
// Run with: node tests/workbench-surface.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import { join, resolve } from "node:path";
import { setupChecks } from "./workbench-setup-surface.mjs";

const repository = resolve(import.meta.dirname, "..");
const ui = join(repository, "crates", "orchestrator", "ui");
const markup = readFileSync(join(ui, "index.html"), "utf8");

// The page loads its modules in a deliberate order, and so must this. Reading that order out of
// the markup rather than repeating it here keeps the two from drifting when a module is added.
const ORDER = [...markup.matchAll(/<script src="([^"]+)"><\/script>/g)].map(([, src]) => src);

// Ids come from the markup, the same way the surface derives them. Hand-listing them here would
// repeat the exact mistake this harness exists to check.
const ids = [...markup.matchAll(/id="([^"]+)"/g)].map((m) => m[1]);

const listeners = [];
const documentHandlers = new Map();
function dispatchDocument(kind, event) {
  for (const handler of documentHandlers.get(kind) ?? []) handler(event);
}
const elementHandlers = new Map();
const node = (id) => ({
  id, textContent: "", innerHTML: "", className: "", hidden: true, disabled: false, scrollTop: 0,
  style: { setProperty() {} }, dataset: {}, classList: { toggle() {}, add() {}, remove() {} },
  addEventListener: (kind, handler) => {
    listeners.push(`${id}:${kind}`);
    elementHandlers.set(`${id}:${kind}`, handler);
  },
  removeEventListener() {}, setAttribute() {}, removeAttribute() {},
  querySelector: () => null, querySelectorAll: () => [], appendChild() {}, prepend() {},
  replaceChildren() {}, remove() {}, closest: () => null, focus() {}, append() {},
  insertBefore() {}, contains: () => false, getBoundingClientRect: () => ({ left: 0, top: 0, width: 1, height: 1 })
});
const nodes = new Map(ids.map((id) => [id, node(id)]));
// Every element the page creates, so a row the view builds can be read back.
const created = [];

// Break exactly one panel, every time it is asked to render, and count the attempts. About writes
// its version disc first, so a disc that refuses every write fails the whole panel. A panel that
// is retried is attempted again on the next snapshot; one memoised as finished never is.
let aboutAttempts = 0;
Object.defineProperty(nodes.get("about-version"), "textContent", {
  get: () => "",
  set: () => {
    aboutAttempts += 1;
    throw new Error("deliberate About failure");
  }
});
const confirmAccept = node("confirm-accept");
confirmAccept.dataset.confirm = "accept";
confirmAccept.closest = (selector) => selector === "[data-confirm]" ? confirmAccept : null;
const confirmCancel = node("confirm-cancel");
nodes.get("confirm-dialog").querySelector = (selector) =>
  selector === '[data-confirm="accept"]' ? confirmAccept
    : selector === '[data-confirm="cancel"]' ? confirmCancel : null;

const reported = [];

// The address bar of a page: what the page reads on load, and what it writes in place of a new
// history entry.
const addressBar = (hash = "") => {
  const bar = { hash, replaced: [], pushed: [] };
  bar.history = {
    replaceState: (_state, _title, url) => {
      bar.replaced.push(url);
      bar.hash = url;
    },
    pushState: (_state, _title, url) => bar.pushed.push(url)
  };
  return bar;
};
const nativeBar = addressBar();
const QUITTING_SENTENCE = "Ghostlight is quitting.";

// The controls part exactly as the service shapes it: every act says whether this context may do
// it, and `you` names the context. The window may do all of it; a watcher may only pause.
const SHEET_WHY = "Change this in the Ghostlight window.";
const SHEET_PRIVATE = "Watching is off: only this window shows Ghostlight.";
const SHEET_WATCHED = "Others on this computer can watch this view in a browser.";
const SHEET_WATCHER = "Anyone on this computer can open this view. Changes are made in the Ghostlight window.";
const SHEET_NOT_SERVED = "Ghostlight is not serving a watch view on this computer.";
const controlsPart = ({
  context = "control", watch = true, runtime = "active", note = null, layer = "marker", served = true
} = {}) => {
  const control = context === "control";
  const act = (may = false) => (control || may ? { available: true, why_not: null } : { available: false, why_not: SHEET_WHY });
  const on = layer !== "off";
  return {
    runtime: {
      label: "Browser work", state: runtime,
      word: { active: "Running", held: "Paused", ended: "Session ended" }[runtime], note,
      acts: { hold: act(true), resume: act(), start_session: act(), end_session: act() }
    },
    viewing: served ? {
      label: "Others can watch", watch, url: "http://127.0.0.1:41234/",
      word: watch ? "On: anyone on this computer can open this view" : "Off: only this window shows Ghostlight",
      acts: { set_watching: act() }
    } : {
      label: "Others can watch", watch: false, url: null, word: SHEET_NOT_SERVED,
      acts: { set_watching: { available: false, why_not: SHEET_NOT_SERVED } }
    },
    diagnostics: {
      label: "Process diagnostics", on, layer, used_bytes: 4242, folder: "~/.cache/ghostlight",
      word: on ? `On (${layer})` : "Off",
      acts: { set_diagnostics: act(), reveal_diagnostics: act() }
    },
    notifications: {
      label: "Notifications", state: "On", word: "Send one local checkup notification. Nothing leaves this device.",
      acts: { test_notification: act() }
    },
    browser: {
      set_in: "Set in the browser",
      effects: { label: "Agent activity effects", word: "The cursor, click ripples and scan-lines on a page an agent works in. They start on." },
      captions: { label: "Action captions", word: "A caption at the bottom of the page names each action. They start off." }
    },
    service: { word: "Ghostlight is running; 1 agent and 1 browser connected." },
    you: {
      context, word: control ? (watch ? "Shared" : null) : "Watching",
      sentence: control ? (watch ? SHEET_WATCHED : SHEET_PRIVATE) : SHEET_WATCHER
    }
  };
};
let snapshots = 0;
let bulkSetups = 0;
let feedFixture = false;
const invoked = [];
// The window's own context: every act is available, so a page in it draws everything.
const controlContext = {
  context: "control", word: "Shared",
  sentence: "Others on this computer can watch this view in a browser.",
  instance: "instance_1", opener: "service",
  acts: Object.fromEntries(["search", "check_policy", "rescan", "continue_in_window", "hold", "resume",
    "start_session", "end_session", "apply_policy", "remove_policy", "set_diagnostics", "set_watching",
    "setup_all", "harness_act", "test_notification", "open", "reveal_diagnostics", "locate", "setup_act", "quit"]
    .map((act) => [act, { available: true, why_not: null }])),
  product: { version: "1.0.0", license: "Apache-2.0 OR MIT" },
  links: {}
};
const managedHarnesses = [];
let pushChange = null;
/** Push a change down the stream and let the page read it, as the service's own would arrive. */
const pushed = async (change) => {
  // One sequence numbers changes and snapshots, so a snapshot read after a change is later than it.
  snapshots = Math.max(snapshots, change.seq);
  pushChange(change);
  for (let turn = 0; turn < 4; turn += 1) await new Promise((resolve) => setImmediate(resolve));
};
let policyReply = null;

/** The agents on the roster these fixtures use, by the name the service gives each. */
const ROSTER_AGENTS = new Map([["Codex", "codex"], ["Zed", "zed"], ["Claude Code", "claude-code"]]);

/** The image address of a roster agent whose image is its monogram, which says nothing else. */
const monogram = (id) => ({ icon: `agent-icons/${id}?v=fedcba987654` });

/** The image address of an agent whose image is its app's own icon, with its line about where it came from. */
const appIcon = (id, app) => ({ icon: `agent-icons/${id}?v=0123456789ab`, about: `Icon from the installed ${app} app.` });

/**
 * An agent as the service names it: every roster agent has its image address (Codex's serves the
 * icon the service found in its app, the others their monograms), the command line its prompt, and
 * any other name, or none, the unknown glyph. An agent Ghostlight does not know is identified by
 * the name it reported.
 */
const namedAgent = (name) => {
  if (name === "Command line") return { id: "command_line", name, symbol: { glyph: "command_line" } };
  const id = ROSTER_AGENTS.get(name);
  if (id === "codex") return { id, name, symbol: appIcon(id, name) };
  if (id) return { id, name, symbol: monogram(id) };
  if (name == null) return { id: "unknown", name: "Unknown agent", symbol: { glyph: "unknown" } };
  return { id: `unknown:${name}`, name, symbol: { glyph: "unknown" } };
};

const snapshot = () => ({
  seq: ++snapshots,
  service: { version: "1.0.0", started_at_ms: 0, runtime_state: "active" },
  sessions: [],
  operations: feedFixture ? [
    { invocation: "clear-running-1", workspace: "w", tool: "browser_read", activity: "Reading", phase: "running" },
    { invocation: "clear-running-2", workspace: "w", tool: "browser_read", activity: "Reading", phase: "running" }
  ] : [],
  browsers: [],
  history: feedFixture ? [{
    invocation: "clear-done", workspace: "w", tool: "browser_read", capability: "read",
    status: "succeeded", effect: "none", allowed: true, summary: "Read 5 words.", complete: true
  }] : [],
  harnesses: [{
    id: "codex", product_id: "codex", name: "Codex", label: "Codex", target: "User",
    agent: { id: "codex", name: "Codex", symbol: monogram("codex") }, state: "updatable",
    detail: "The connector path belongs to an older installation.",
    can_install: true, can_fix: false, can_uninstall: false, can_download: true, can_locate: true,
    config_path: "/home/test/.codex/config.toml", connector_command: "/opt/ghostlight/ghostlight mcp",
    manual_setup: "[mcp_servers.ghostlight]"
  }, {
    id: "qwen-code", product_id: "qwen-code", name: "Qwen Code", label: "Qwen Code", target: "CLI",
    agent: { id: "qwen-code", name: "Qwen Code", symbol: monogram("qwen-code") },
    state: "not_detected", detail: "Not detected.", can_install: true, can_fix: false, can_uninstall: false,
    can_download: true, can_locate: true, copy_only: false, config_path: "/home/test/.qwen/settings.json",
    connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "qwen setup"
  }],
  diagnostics: [],
  // The aggregate answer is the orchestrator's, exactly like the policy sentence below. The
  // fixture carries a word the window has no way to compute, so a surface that authored its own
  // would visibly disagree with this.
  readiness: {
    state: "ready",
    word: "Ready",
    detail: "Connected and idle. Agents can work when they ask.",
    tone: "quiet",
    invites_control: true
  },
  configuration: {
    local_policy_configured: true,
    local_policy_active: true,
    local_policy_valid: true,
    managed_authority_configured: false,
    managed_authority_active: false,
    managed_authority_valid: true,
    managed_policy: { configured: false },
    // The orchestrator authors the tab's state; the surface renders exactly what it is handed, so
    // this fixture carries a sentence no local computation could have produced.
    policy: {
      situation: "layered",
      detail: "Example Org sets the rules, and you have narrowed them further.",
      tone: "applied"
    }
  },
  controls: controlsPart(),
  overview: {}
});

/** The command a request to the window's own service is, by the route it goes to. */
function windowCommandAt(method, pathname) {
  const routes = {
    "GET /api/context": "workbench_context",
    "GET /api/snapshot": "workbench_snapshot",
    "GET /api/policy": "workbench_policy",
    "PUT /api/controls/diagnostics": "set_diagnostics",
    "POST /api/window": "continue_in_window",
    "POST /api/quit": "quit_ghostlight",
    "POST /api/integrations/setup-all": "setup_detected_harnesses",
    "POST /api/integrations/act": "manage_harness"
  };
  return routes[`${method} ${pathname}`] ?? null;
}

/** The setup part of a computer whose setup finished earlier: the window shows the workbench. */
const retiredSetup = {
  stage: "retired", eyebrow: "", headline: { lead: "", accent: "", trail: "" }, lead: null,
  steps: [
    { name: "This computer", state: "done", word: "Installed", sr: "This computer, done" },
    { name: "Your browser", state: "done", word: "Connected", sr: "Your browser, done" },
    { name: "Your agent", state: "done", word: "Connected", sr: "Your agent, done" }
  ],
  light: "lit", code: null, rows_label: null, rows: [], extra_code: null, live: null,
  connected_line: null, tagline: null
};

/** The parts of a snapshot answer, from the one flat snapshot this fixture is written as. */
function partsOf(flat) {
  return {
    status: {
      service: flat.service, readiness: flat.readiness, audit_health: flat.audit_health,
      audit_notice: flat.audit_notice, configuration: flat.configuration,
      ...(flat.runtime_note === undefined ? {} : { runtime_note: flat.runtime_note })
    },
    connections: { sessions: flat.sessions, browsers: flat.browsers },
    activity: { operations: flat.operations, history: flat.history, document_coverage: flat.document_coverage ?? [] },
    integrations: { harnesses: flat.harnesses },
    diagnostics: { diagnostics: flat.diagnostics },
    controls: flat.controls,
    setup: retiredSetup
  };
}

const sandbox = {
  console: { error: (detail) => reported.push(String(detail)), log() {}, warn() {} },
  document: {
    getElementById: (id) => nodes.get(id) ?? null,
    querySelectorAll: (sel) => (sel === "[id]" ? [...nodes.values()] : []),
    querySelector: () => null,
    addEventListener: (kind, handler) => {
      listeners.push(`document:${kind}`);
      if (!documentHandlers.has(kind)) documentHandlers.set(kind, []);
      documentHandlers.get(kind).push(handler);
    },
    removeEventListener() {},
    hidden: false, body: node("body"),
    createElement: () => {
      const made = node("x");
      created.push(made);
      return made;
    },
    createDocumentFragment: () => node("f")
  },
  window: {
    addEventListener: (kind) => listeners.push(`window:${kind}`),
    location: nativeBar,
    history: nativeBar.history,
    fetch: async (path, init = {}) => {
      const url = new URL(path, "http://127.0.0.1:41000");
      const command = windowCommandAt(init.method ?? "GET", url.pathname);
      if (command) invoked.push(command);
      const arguments_ = init.body ? JSON.parse(init.body) : {};
      const said = (status_, body) => ({
        ok: status_ >= 200 && status_ < 300, status: status_,
        headers: { get: () => null },
        text: async () => (body === undefined ? "" : JSON.stringify(body))
      });
      if (command === "workbench_context") return said(200, controlContext);
      if (command === "set_diagnostics") {
        return said(200, { layer: arguments_.on ? "marker" : "off", directory: null, used_bytes: 0 });
      }
      if (command === "continue_in_window") return said(200, {});
      if (command === "quit_ghostlight") return said(202, { message: QUITTING_SENTENCE });
      if (command === "workbench_snapshot") {
        const flat = snapshot();
        return said(200, { seq: flat.seq, instance: "instance_1", parts: partsOf(flat) });
      }
      if (command === "workbench_policy" && policyReply) return said(200, policyReply());
      if (command === "setup_detected_harnesses") {
        bulkSetups += 1;
        return said(200, { set_up: 1, updated: 1, needs_attention: 0, failures: [], message: "Set up everything." });
      }
      if (command === "manage_harness") {
        managedHarnesses.push({ id: arguments_.id, action: arguments_.action });
        if (arguments_.action === "fix") return said(200, { changed: true, message: "Fixed Cline." });
        return said(500, { error: { code: "failed", message: "deliberate setup failure" } });
      }
      if (url.pathname === "/api/events") {
        const stream = openStream();
        pushChange = (change) => stream.write(`data: ${JSON.stringify(change)}

`);
        return { ok: true, status: 200, headers: { get: () => null }, body: { getReader: () => stream.reader } };
      }
      return said(200, []);
    },
    open: () => null
  },
  navigator: { clipboard: { writeText: async () => undefined } },
  GhostlightAccess: Object.freeze({ bearer: "key-1" }),
  AbortController, TextDecoder, TextEncoder, MutationObserver: class { observe() {} },
  setTimeout: (fn, ms) => { if (typeof fn === "function" && !ms) fn(); return 0; },
  clearTimeout() {}, clearInterval() {},
  setInterval: () => 0,
  Date, JSON, Math, String, Number, Boolean, Array, Object, Set, Map, Promise, Error,
  isNaN, parseInt, parseFloat, performance: { now: () => 0 }, crypto: { randomUUID: () => "x" }
};
sandbox.globalThis = sandbox;
sandbox.window.document = sandbox.document;
vm.createContext(sandbox);

let bootThrew = null;
try {
  vm.runInContext(ORDER.map((name) => readFileSync(join(ui, name), "utf8")).join("\n"), sandbox);
} catch (error) {
  bootThrew = error.message;
}

await new Promise((r) => setTimeout(r, 60));
const bootRail = nodes.get("agents-list").innerHTML;
const bootPane = nodes.get("agents-pane").innerHTML;
const failedSetupButton = {
  disabled: false,
  dataset: {
    harness: "qwen-code", harnessOperation: "manage", harnessAction: "install",
    harnessName: "Qwen Code"
  }
};
dispatchDocument("click", {
  target: {
    closest: (selector) => selector === "[data-harness-operation]" ? failedSetupButton : null
  }
});
await new Promise((r) => setTimeout(r, 0));
await new Promise((r) => setTimeout(r, 0));
const paneAfterFailure = nodes.get("agents-pane").innerHTML;
// A second snapshot: whatever the first pass did with the broken panel, this is where a memo
// would show itself by never trying again.
const before = aboutAttempts;
await new Promise((r) => setTimeout(r, 60));

// Settings as the first snapshot drew it, read before anything below repaints it.
const sheetNode = (id) => nodes.get(id);
const sheetAfterBoot = {
  railScreen: sheetNode("rail-screen-state").textContent,
  railSharing: sheetNode("rail-sharing-state").textContent,
  railBrowserWork: sheetNode("rail-browser-work-state").textContent,
  railNotifications: sheetNode("rail-notifications-state").textContent,
  railDiagnostics: sheetNode("rail-diagnostics-state").textContent,
  who: sheetNode("settings-sharing-who").textContent,
  quitShown: !sheetNode("settings-quit").hidden,
  service: sheetNode("status-service").textContent,
  runtimeLabel: sheetNode("controls-runtime-label").textContent,
  runtimeWord: sheetNode("controls-runtime-word").textContent,
  hold: sheetNode("controls-hold").hidden,
  resume: sheetNode("controls-resume").hidden,
  start: sheetNode("controls-start").hidden,
  end: sheetNode("controls-end").hidden,
  viewingWord: sheetNode("controls-viewing-word").textContent,
  viewingUrl: sheetNode("controls-viewing-url").textContent,
  viewingToggle: sheetNode("controls-viewing-toggle").textContent,
  viewingToggleHidden: sheetNode("controls-viewing-toggle").hidden,
  diagnosticsWord: sheetNode("controls-diagnostics-word").textContent,
  diagnosticsPlace: sheetNode("controls-diagnostics-place").textContent,
  diagnosticsToggle: sheetNode("controls-diagnostics-toggle").textContent,
  diagnosticsToggleHidden: sheetNode("controls-diagnostics-toggle").hidden,
  diagnosticsReveal: sheetNode("controls-diagnostics-reveal").hidden,
  notificationsWord: sheetNode("controls-notifications-word").textContent,
  test: sheetNode("test-notification").hidden,
  continueLine: sheetNode("settings-sharing-continue").hidden,
  chip: sheetNode("context-chip").textContent,
  chipTitle: sheetNode("context-chip").title,
  address: nativeBar.replaced.length,
  errors: [...reported]
};

// The orchestrator pushes a changed status. The band, the wheel and the sentence behind the word
// change with no snapshot asked for, and a hint about a part nobody is looking at asks for nothing.
const snapshotRequests = () => invoked.filter((cmd) => cmd === "workbench_snapshot").length;
const statusPush = (seq, runtime, note) => ({
  seq,
  change: {
    kind: "status_changed",
    status: {
      service: { version: "1.0.0", started_at_ms: 0, runtime_state: runtime },
      readiness: runtime === "held"
        ? { state: "paused", word: "Paused", detail: "Agents are paused.", tone: "warning", invites_control: true }
        : { state: "ready", word: "Ready", detail: "Connected and idle. Agents can work when they ask.", tone: "quiet", invites_control: true },
      audit_health: {},
      audit_notice: "",
      configuration: {
        policy: { situation: "layered", detail: "Example Org sets the rules, and you have narrowed them further.", tone: "applied" },
        managed_policy: { configured: false }
      },
      ...(note ? { runtime_note: note } : {})
    }
  }
});
const requestsBeforePush = snapshotRequests();
const seqBeforePush = snapshots;
const wordBeforePush = nodes.get("state-word").textContent;
await pushed(statusPush(seqBeforePush + 1, "held", "Paused from the tray."));
const wordAfterPush = nodes.get("state-word").textContent;
const wordTitleAfterPush = nodes.get("state-word").title;
const wheelAfterPush = nodes.get("wheel").dataset.intent;
const requestsAfterPush = snapshotRequests();
await pushed(statusPush(seqBeforePush + 2, "active", null));
const titleAfterResume = nodes.get("state-word").title;
await pushed({ seq: seqBeforePush + 3, change: { kind: "part_changed", part: "integrations", etag: "tag-1" } });
await pushed({ seq: seqBeforePush + 4, change: { kind: "part_changed", part: "policy", etag: "tag-2" } });
await new Promise((r) => setTimeout(r, 0));
const requestsAfterHints = snapshotRequests();
const policyReadsAfterHints = invoked.filter((cmd) => cmd === "workbench_policy").length;
dispatchDocument("click", {
  target: { closest: (selector) => selector === "[data-view]" ? { dataset: { view: "integrations" } } : null }
});
await new Promise((r) => setTimeout(r, 0));
const requestsAfterOpening = snapshotRequests();
dispatchDocument("click", {
  target: { closest: (selector) => selector === "[data-view]" ? { dataset: { view: "monitor" } } : null }
});

const connections = nodes.get("connections");
// Read what the booted surface drew before anything below repaints the same nodes: the second
// view instance shares this one stub document, so a later render would answer for the first.
const setupEverythingButton = nodes.get("setup-integrations");
const setupEverythingWasActionable = !setupEverythingButton.disabled
  && setupEverythingButton.dataset.actionable === "true";
await elementHandlers.get("setup-integrations:click")({ currentTarget: setupEverythingButton });
const policy = nodes.get("policy-state");

// The Policy destination, drawn from a compiled view exactly as the orchestrator hands it over.
// Nothing here is computed by the window, so the assertions below are about rendering fidelity:
// the sentence, the decider on every line, the boundaries that survive an all-open machine, and
// an editor that appears only when the person is actually allowed to author.
const compiled = (editable) => ({
  situation: "layered",
  headline: "Example Org sets the rules, and you have narrowed them further.",
  organization: {
    name: "Example Org",
    statement: "Ask the service desk for an exception.",
    url: "https://example.test/policy",
    contacts: [{ kind: "email", value: "security@example.test", label: "Security team" }]
  },
  capabilities: [
    { capability: "read", label: "Look at pages", covers: "Read page text.", state: "some_allowed", detail: "Refused everywhere except the sites Example Org allowed.", decided_by: ["organization"] },
    { capability: "action", label: "Click and type", covers: "Click and type.", state: "available", detail: "Available on ordinary websites. Nothing narrows it.", decided_by: [] },
    { capability: "write", label: "Fill in forms", covers: "Enter information.", state: "unavailable", detail: "Not available. Example Org does not allow it anywhere.", decided_by: ["organization"] },
    { capability: "execute", label: "Run page code", covers: "Run JavaScript.", state: "unavailable", detail: "Not available. Example Org does not allow it anywhere.", decided_by: ["organization"] }
  ],
  layers: [
    { kind: "organization", title: "Example Org", policy_name: "Support", version: "1",
      rules: [{ id: "support", description: "Ordinary support work", allow: ["support.example.test"], deny: [], allowed: ["read"], note: null }],
      settings: [
        { key: "privacy.preserve_target_names", value: "false", level: "mandatory" },
        { key: "channels.cli.enabled", value: "false", level: "mandatory" },
        { key: "browser.startup", value: '"on_demand"', level: "mandatory" }
      ],
      path: null, document: '{"schema": 4}' },
    { kind: "user", title: "Your rules", policy_name: "Your rules", version: "1",
      rules: [{ id: "leftover", description: null, allow: ["other.test"], deny: [], allowed: ["read"], note: "no_effect" }],
      settings: [{ key: "browser.tabs.allow_close", value: "false", level: "mandatory" }],
      path: "state/user-policy.json", document: '{"schema": 4}' }
  ],
  ceilings: [
    "Anything that is not an ordinary http or https address.",
    "Ghostlight's own control page on this computer is never automated."
  ],
  user_layer: {
    source: "workbench",
    editable,
    path: "state/user-policy.json",
    blocked_reason: editable ? null : "Example Org does not allow rules to be set on this machine."
  },
  browser_startup: { value: "on_demand", decided_by: "organization", organization_ceiling: "on_demand" },
  passport: { configured: false, contacts: [] }
});

// A second view over the same stub document. The booted surface holds its own instance; this one
// exists so the destination can be drawn on demand without a real click.
const view = sandbox.globalThis.GhostlightView.create({ onFailure: (what, error) => reported.push(`${what}: ${error?.message ?? error}`) });

// The Integrations page drawn on demand, over every state an agent can be in. The ids differ from
// the booted roster's, so the booted page's own listeners leave this one alone.
const AGENT_COMMAND = "/opt/ghostlight/ghostlight mcp";
const agentRow = (id, name, state, extra = {}) => ({
  id, product_id: id, name, label: name, target: "User", agent: { id, name, symbol: monogram(id) },
  state, detail: "Said by the service.",
  can_install: ["available", "updatable", "not_detected"].includes(state) && !extra.copy_only,
  can_fix: false, can_uninstall: state === "installed", can_download: true, can_locate: true, copy_only: false,
  config_path: `/home/test/.${id}/settings.json`, connector_command: AGENT_COMMAND,
  manual_setup: `{\n  "servers": {\n    "${id}": "ghostlight"\n  }\n}`, ...extra
});
const agentRoster = [
  agentRow("zcode", "ZCode", "not_detected"),
  agentRow("windsurf", "Windsurf", "installed",
    { agent: { id: "windsurf", name: "Windsurf", symbol: appIcon("windsurf", "Windsurf") } }),
  agentRow("goose", "goose", "available", { copy_only: true }),
  agentRow("junie", "Junie", "needs_attention", {
    can_install: false, can_fix: true,
    evidence: "Found another program, `alien-agent` with 1 argument, where Ghostlight's entry belongs; Ghostlight maintains `/opt/ghostlight/ghostlight` there and changed nothing."
  }),
  agentRow("antigravity", "Antigravity", "not_detected"),
  agentRow("cursor", "Cursor", "updatable"),
  agentRow("zed", "Zed", "installed"),
  agentRow("kilo-code", "Kilo Code", "needs_attention", {
    can_install: false, evidence: "Found an entry without a usable command where Ghostlight's entry belongs; Ghostlight maintains `/opt/ghostlight/ghostlight` there and changed nothing."
  }),
  agentRow("claude-code", "Claude Code", "available"),
  agentRow("continue", "Continue", "not_detected", { copy_only: true }),
  agentRow("crush", "Crush", "not_detected")
];
view.collections({
  sessions: [
    { id: "w-zed-1", agent: namedAgent("Zed"), channel: "mcp", tab_count: 1, running_operations: 0 },
    { id: "w-zed-2", agent: namedAgent("Zed"), channel: "mcp", tab_count: 2, running_operations: 1 },
    { id: "w-cli", agent: namedAgent("Command line"), channel: "cli", tab_count: 1, running_operations: 0 }
  ],
  // The service says which rows are in use; the page draws what it says.
  harnesses_in_use: ["zed"],
  browsers: [], diagnostics: [], history: [], service: { version: "1.0.0" },
  configuration: { managed_policy: { configured: false } },
  harnesses: agentRoster
}, new Set());
const agentsShown = {
  heading: nodes.get("integrations-heading").innerHTML,
  caption: nodes.get("integrations-caption").textContent,
  setupTitle: nodes.get("setup-integrations").title,
  command: nodes.get("integrations-command").textContent,
  copyCommand: { hidden: nodes.get("copy-command").hidden, harness: nodes.get("copy-command").dataset.harness },
  rail: nodes.get("agents-list").innerHTML,
  opening: nodes.get("agents-pane").innerHTML
};
const railHeads = (html) => [...html.matchAll(/<h2 class="agents-group-head"><span>([^<]+)<\/span><span class="agents-count">(\d+)<\/span>/g)]
  .map(([, title, count]) => `${title} ${count}`);
const railNames = (html) => [...html.matchAll(/data-agent="([^"]+)"/g)].map(([, id]) => id);
const clickAgent = (selector, dataset) => dispatchDocument("click", {
  target: { closest: (asked) => (asked === selector ? { dataset } : null) }
});
clickAgent("[data-agent-fold]", {});
const railOpened = nodes.get("agents-list").innerHTML;
clickAgent("[data-agent-fold]", {});
elementHandlers.get("agents-find:input")({ target: { value: "zco" } });
const railSearched = nodes.get("agents-list").innerHTML;
elementHandlers.get("agents-find:input")({ target: { value: "nothing like it" } });
const railNoMatch = nodes.get("agents-list").innerHTML;
elementHandlers.get("agents-find:input")({ target: { value: "" } });
const agentPanes = Object.fromEntries(["kilo-code", "cursor", "zed", "windsurf", "claude-code", "goose", "antigravity", "continue"]
  .map((id) => {
    clickAgent("[data-agent]", { agent: id });
    return [id, nodes.get("agents-pane").innerHTML];
  }));
const railWithAbsentPicked = nodes.get("agents-list").innerHTML;
const everyPane = Object.values(agentPanes).join("") + agentsShown.opening;

// Remove asks first and names the sessions that use Ghostlight now; cancelling sends nothing.
const managedBeforeRemove = managedHarnesses.length;
dispatchDocument("click", {
  target: {
    closest: (selector) => selector === "[data-harness-operation]" ? {
      disabled: false,
      dataset: {
        harness: "zed", harnessOperation: "manage", harnessAction: "uninstall", harnessName: "Zed",
        harnessState: "installed", harnessFile: "settings.json", harnessSessions: "2"
      }
    } : null
  }
});
await new Promise((r) => setTimeout(r, 0));
const removalAsked = {
  shown: !nodes.get("confirm-dialog").hidden,
  title: nodes.get("confirm-title").textContent,
  detail: nodes.get("confirm-detail").textContent,
  action: confirmAccept.textContent
};
dispatchDocument("click", { target: { closest: (selector) => selector === "[data-confirm]" ? { dataset: { confirm: "cancel" } } : null } });
await new Promise((r) => setTimeout(r, 0));
const removalSentNothing = managedHarnesses.length === managedBeforeRemove;

const fixButton = {
  disabled: false,
  dataset: {
    harness: "junie", harnessOperation: "manage", harnessAction: "fix",
    harnessName: "Junie", harnessFile: "settings.json"
  }
};
dispatchDocument("click", {
  target: {
    closest: (selector) => selector === "[data-harness-operation]" ? fixButton : null
  }
});
await new Promise((r) => setTimeout(r, 0));
const fixConfirmationShown = !nodes.get("confirm-dialog").hidden
  && nodes.get("confirm-title").textContent.includes("Junie")
  && nodes.get("confirm-title").textContent.includes("settings.json")
  && nodes.get("confirm-detail").textContent.includes("backed up")
  && confirmAccept.textContent === "Fix";
dispatchDocument("click", { target: confirmAccept });
await new Promise((r) => setTimeout(r, 0));
const fixWasManaged = managedHarnesses.some((call) =>
  call.id === "junie" && call.action === "fix");

// A refusal has to lead somewhere. The deciding rule and the denial handle are recorded on every
// enforced denial, and an organization that supplied contacts supplied them for this moment.
view.collections({
  sessions: [], browsers: [], harnesses: [], diagnostics: [], history: [], service: { version: "1.0.0" },
  configuration: {
    managed_policy: {
      configured: true, organization: "Example Org",
      contacts: [{ kind: "email", value: "security@example.test", label: "Security team" }]
    }
  }
}, new Set());
view.hero({
  invocation: "i9", workspace: "w1", tool: "browser_execute", capability: "execute",
  settled: true, allowed: false, phase: "blocked", reason: "policy_denied",
  policyTier: "managed", grantId: "support-sites", denialId: "D-a1b2c3",
  summary: "Refused.", endedAt: Date.now(), durationMs: 12
}, false);
const refusal = nodes.get("hero-body").innerHTML;


// A policy hint while the person has rules edited and not applied. The booted window must leave
// the draft exactly as it is and say so once, then read the policy when the edits are discarded.
policyReply = () => compiled(true);
const appDraft = (code) => vm.runInContext(code, sandbox);
const policyReads = () => invoked.filter((cmd) => cmd === "workbench_policy").length;
let hintSeq = snapshots;
dispatchDocument("click", {
  target: { closest: (selector) => selector === "[data-view]" ? { dataset: { view: "policy" } } : null }
});
await new Promise((r) => setTimeout(r, 0));
const readsOnOpening = policyReads();
// The service turns watching off and diagnostics off. The sheet and the chip follow the pushed
// part with no snapshot asked for, and the chip is redrawn from the context the part names.
const snapshotsBeforeControls = snapshotRequests();
await pushed({
  seq: ++hintSeq,
  change: { kind: "controls_changed", controls: controlsPart({ watch: false, layer: "off" }) }
});
const sheetAfterPush = {
  railSharing: sheetNode("rail-sharing-state").textContent,
  railDiagnostics: sheetNode("rail-diagnostics-state").textContent,
  viewingWord: sheetNode("controls-viewing-word").textContent,
  viewingToggle: sheetNode("controls-viewing-toggle").textContent,
  diagnosticsWord: sheetNode("controls-diagnostics-word").textContent,
  diagnosticsToggle: sheetNode("controls-diagnostics-toggle").textContent,
  chip: sheetNode("context-chip").textContent,
  chipHidden: sheetNode("context-chip").hidden,
  chipTitle: sheetNode("context-chip").title,
  snapshots: snapshotRequests() - snapshotsBeforeControls
};
await pushed({ seq: ++hintSeq, change: { kind: "part_changed", part: "policy", etag: "tag-clean" } });
await new Promise((r) => setTimeout(r, 0));
const readsAfterCleanHint = policyReads();
appDraft("view.addRule()");
const draftBeforeHint = appDraft("view.draftDocument()");
const ruleListBeforeHint = nodes.get("rule-list").innerHTML;
await pushed({ seq: ++hintSeq, change: { kind: "part_changed", part: "policy", etag: "tag-dirty" } });
await new Promise((r) => setTimeout(r, 0));
const readsAfterDirtyHint = policyReads();
const draftAfterHint = appDraft("view.draftDocument()");
const draftStillDirty = appDraft("view.draftIsDirty()");
const ruleListAfterHint = nodes.get("rule-list").innerHTML;
const editorLineAfterHint = nodes.get("editor-status").textContent;
elementHandlers.get("discard-policy:click")();
await new Promise((r) => setTimeout(r, 0));
const readsAfterDiscard = policyReads();
const draftCleanAfterDiscard = !appDraft("view.draftIsDirty()");
dispatchDocument("click", {
  target: { closest: (selector) => selector === "[data-view]" ? { dataset: { view: "monitor" } } : null }
});

view.policy(compiled(true));
const board = nodes.get("capability-board").innerHTML;
const organization = nodes.get("policy-organization").innerHTML;
const documents = nodes.get("policy-documents").innerHTML;
const settingsShown = nodes.get("policy-settings").innerHTML;
const rules = nodes.get("rule-list").innerHTML;
const editorShown = !nodes.get("policy-editor").hidden;
// Permissions are authored here too, phrased as what a person can do, and only ever in the
// direction that tightens underneath. The fixture's user layer already restricts tab closing and
// the fixture's organization layer already forces page-authored names off, so the first render
// exercises both a person's own toggle and an organization ceiling at once.
const permissionsOnLoad = nodes.get("setting-groups").innerHTML;
const authoredOnLoad = JSON.parse(view.draftDocument()).config;
view.setChoice("browser.startup", "manual");
const authoredAfterStartup = JSON.parse(view.draftDocument()).config;
view.setPermission("browser.screenshots.enabled", false);
view.setSacred("vault.example.test, *.finance.example.test");
const authoredAfterEdit = JSON.parse(view.draftDocument()).config;
view.setPermission("browser.tabs.allow_close", true);
const authoredAfterClearing = JSON.parse(view.draftDocument()).config;
view.setPermission("channels.mcp.enabled", false);
const authoredChannelOff = JSON.parse(view.draftDocument()).config;
const channelSwitchesOff = nodes.get("setting-groups").innerHTML;

// Opening and closing one organization rule: the detail belongs to the row, not to the page.
view.toggleRule("org:Example Org:support");
const openedDetail = nodes.get("rule-list").innerHTML;
view.toggleRule("org:Example Org:support");
const closedAgain = nodes.get("rule-list").innerHTML;

const startupPinned = compiled(true);
startupPinned.layers[0].settings.find((entry) => entry.key === "browser.startup").value = '"manual"';
startupPinned.browser_startup = {
  value: "manual", decided_by: "organization", organization_ceiling: "manual"
};
view.policy(startupPinned);
const pinnedStartupControl = nodes.get("setting-groups").innerHTML;

view.policy(compiled(false));
const editorHiddenWhenRefused = nodes.get("policy-editor").hidden
  && !nodes.get("policy-blocked").hidden
  && nodes.get("policy-blocked-reason").textContent.includes("Example Org");

/*
 * A real browser nulls Event.currentTarget at the first await, so Re-check must take its button
 * before it awaits; this checks it against an event that expires the same way.
 */
const reCheckButton = node("refresh-integrations");
let dispatchEnded = false;
const clickEvent = { get currentTarget() { return dispatchEnded ? null : reCheckButton; } };
queueMicrotask(() => { dispatchEnded = true; });
let reCheckThrew = null;
try {
  await elementHandlers.get("refresh-integrations:click")(clickEvent);
} catch (error) {
  reCheckThrew = error.message;
}

// The never-settled treatment: the predicate lives beside the readiness words, and a row the
// view builds for a doubtful readiness marks its duration cell. It uses the view above, since a
// new view would take over the document listeners the checks below dispatch to.
const wordsApi = sandbox.globalThis.GhostlightWords;
const durationCellOf = (readiness) => {
  const record = {
    invocation: `duration-${readiness}`, workspace: "w", tool: "browser_read", capability: "read",
    status: "succeeded", effect: "none", allowed: true, complete: true, summary: "Read 5 words.",
    timestamp_ms: 1, duration_ms: 10, observed: { readiness }
  };
  const entries = sandbox.globalThis.GhostlightEntries;
  view.rebuildFeed([
    entries.entryFromRecord({ ...record, invocation: "duration-newest" }, null),
    entries.entryFromRecord(record, null)
  ]);
  const row = created.filter((made) => String(made.className).startsWith("row ")).at(-1);
  return /<div class="row-dur[^"]*">/.exec(row?.innerHTML ?? "")?.[0] ?? "";
};
const loadingDuration = durationCellOf("loading");
const interactiveDuration = durationCellOf("interactive");

// The ADR-0145 status card: rendered from the orchestrator-owned state, with both person acts.
view.collections({
  sessions: [], browsers: [],
  diagnostics: [{ severity: "passing", label: "Ghostlight", detail: "Running and accepting agents." }],
  history: [], service: { version: "1.0.0", started_at_ms: 0, runtime_state: "active" },
  configuration: { managed_policy: { configured: false } },
  harnesses: []
}, []);

const historyChecks = [];
{
  const store = sandbox.globalThis.GhostlightStore.create({ setTimer: () => 0, clearTimer() {} });
  const active = { invocation: "active-h8", workspace: "w", tool: "browser_read", activity: "Reading", phase: "running" };
  const waiting = { ...active, invocation: "waiting-h8", phase: "waiting", activity: "Waiting for earlier browser work" };
  store.applySnapshot({ seq: 0, service: { runtime_state: "active" }, operations: [active], history: [], sessions: [] }, true);
  store.applyChange({ seq: 1, change: { kind: "operation_started", operation: waiting } });
  historyChecks.push(["waiting work stays live beneath the running action", store.hero().invocation === active.invocation
    && store.feed().length === 2 && sandbox.globalThis.GhostlightEntries.isPending(store.feed()[1])]);
  store.applyChange({ seq: 2, change: { kind: "operation_started", operation: { ...waiting, phase: "running", activity: "Reading" } } });
  historyChecks.push(["admitted work becomes the running action without a duplicate row", store.hero().invocation === waiting.invocation && store.feed().length === 2]);
}
{
  const receipt = { invocation: "group", workspace: "w", tool: "browser_read", capability: "read",
    status: "succeeded", effect: "none", allowed: true, summary: "Read 5 words.", complete: true,
    permissions: { checks: [{ requirements: ["read"], host: "example.com" }], truncated: false },
    permission_explanations: ['Allowed: local policy (<script>never executable</script>).'] };
  const group = { ...receipt, tool: "browser_flow", complete: false,
    summary: "Completed 1 of 3 steps.", steps: [
      { position: 1, state: "recorded", record: receipt },
      { position: 2, state: "unconfirmed", tool: "browser_execute", record: null },
      { position: 3, state: "pending", record: null }
    ] };
  const store = sandbox.globalThis.GhostlightStore.create({ setTimer: () => 0, clearTimer() {} });
  const operation = { invocation: "group", workspace: "w", tool: "browser_flow", activity: "Working", phase: "running" };
  store.applySnapshot({ seq: 0, service: { runtime_state: "active" }, operations: [operation], history: [group], sessions: [] }, true);
  historyChecks.push(["snapshot merges live composition with its child history", store.feed().length === 1 && !store.hero().settled && store.hero().steps.length === 3]);
  view.hero(store.hero(), false);
  const collapsed = nodes.get("hero-body").innerHTML;
  historyChecks.push(["composition details begin collapsed and escape retained explanations", collapsed.includes('data-history-details="group:steps"') && !collapsed.includes('data-history-details="group:steps" open') && collapsed.includes('&lt;script&gt;') && !collapsed.includes('<script>never')]);
  let scrolled = 0;
  // These standalone view checks target the most recently created view's own listener.
  documentHandlers.get("toggle").at(-1)({ target: { dataset: { historyDetails: "group:steps" }, open: true,
    classList: { contains: () => true }, querySelector: () => ({ scrollIntoView: () => scrolled++ }) } });
  store.applyChange({ seq: 1, change: { kind: "composition_changed", record: { ...group, summary: "Completed 2 of 3 steps." } } });
  view.hero(store.hero(), false);
  historyChecks.push(["incremental receipts preserve expansion without settling or repeated scrolling", !store.hero().settled && nodes.get("hero-body").innerHTML.includes('data-history-details="group:steps" open') && scrolled === 1]);
  store.applyChange({ seq: 2, change: { kind: "operation_settled", record: { ...group, complete: true } } });
  historyChecks.push(["only the final parent receipt settles the composition", store.hero().settled && store.feed().length === 1]);
  const restored = sandbox.globalThis.GhostlightStore.create({ setTimer: () => 0, clearTimer() {} });
  restored.applySnapshot({ seq: 0, service: { runtime_state: "active" }, operations: [], history: [group], sessions: [] }, true);
  historyChecks.push(["a restored missing completion never fabricates running work", restored.hero().settled]);
}

// C1: each action keeps its connection's evidence, independent of the current session label.
const provenanceChecks = [];
{
  const evidence = {
    connection_id: "connection_original", reported_application: "Original <script>claim</script>",
    channel: "mcp", explanation: '"Original <script>claim</script>" is what the agent calls itself; Ghostlight does not verify it.'
  };
  const claimed = namedAgent("Original <script>claim</script>");
  const laterSession = { id: "shared-workspace", agent: namedAgent("Later application"), channel: "cli" };
  const connectionView = sandbox.globalThis.GhostlightView.create({ sessionFor: () => laterSession });
  const entries = sandbox.globalThis.GhostlightEntries;
  const operation = { invocation: "provenance-work", workspace: laterSession.id, tool: "browser_read",
    phase: "running", activity: "Reading", provenance: evidence, agent: claimed };
  const record = { ...operation, complete: true, summary: "Read 5 words.", status: "succeeded",
    allowed: true, effect: "none", channel: "mcp", provenance: evidence, agent: claimed };
  connectionView.hero(entries.entryFromOperation(operation), false);
  let html = nodes.get("hero-body").innerHTML;
  provenanceChecks.push(["live action attribution uses its own connection and escapes the claim",
    html.includes("Original &lt;script&gt;claim&lt;/script&gt;")
      && !html.includes("Later application") && !html.includes("<script>claim")]);
  const restored = { ...record, provenance: { ...evidence, reported_application: null,
    explanation: "The agent's name is its own claim. Ghostlight does not verify it." }, agent: namedAgent("Codex") };
  connectionView.hero(entries.entryFromRecord(restored), false);
  html = nodes.get("hero-body").innerHTML;
  provenanceChecks.push(["restored history keeps its connection, behind Copy id, without borrowing a current claim",
    html.includes('data-copy-connection="connection_original"') && html.includes("own claim")
      && !html.includes("Later application") && !html.includes("Not retained") && !html.includes("Reported application"), html]);
  connectionView.hero(entries.entryFromRecord({ ...record, provenance: null, channel: null, agent: namedAgent(null) }), false);
  html = nodes.get("hero-body").innerHTML;
  provenanceChecks.push(["legacy history names missing evidence without borrowing current-session identity",
    html.includes("Connection details were not recorded.") && !html.includes("Later application") && !html.includes("via cli")]);
  const group = { ...record, tool: "browser_flow", steps: [{ position: 1, state: "recorded", record }] };
  connectionView.hero(entries.entryFromRecord(group), false);
  html = nodes.get("hero-body").innerHTML;
  provenanceChecks.push(["a flow shows its connection once, collapsed, and not under each step",
    html.includes('data-history-details="provenance-work:connection"')
      && !html.includes('data-history-details="provenance-work:step:1:connection"')
      && !html.includes('data-history-details="provenance-work:connection" open'), html]);
}

// At a glance: one summary, two shelves, and an agent's card in place of a list of connections.
const glanceChecks = [];
{
  const glanceView = sandbox.globalThis.GhostlightView.create({});
  const claim = (id, agent, extra = {}) => ({
    id, agent: namedAgent(agent), channel: "mcp", tab_count: 0, running_operations: 0,
    connections: [{ connection_id: `connection_${id}`, reported_application: agent, channel: "mcp",
      explanation: `"${agent}" is what the agent calls itself; Ghostlight does not verify it.` }],
    ...extra
  });
  const sessions = [
    ...Array.from({ length: 5 }, (_, at) => claim(`codex_${at}`, "Codex")),
    claim("claude", "Claude <b>Code</b>", { tab_count: 2, running_operations: 1, browser: "Google Chrome" }),
    { ...claim("script", "Command line"), channel: "cli",
      connections: [{ connection_id: "connection_script", channel: "cli",
        explanation: "A ghostlight call session is a local program." }] }
  ];
  const browsers = [{ id: "chrome", family: "Google Chrome", adapter_version: "2.0.0", tab_count: 2, connected: true }];
  const draw = (facts) => {
    glanceView.collections({ ...snapshot(), ...facts }, new Set());
    return nodes.get("connections").innerHTML;
  };
  let html = draw({ sessions, browsers });
  glanceChecks.push(["an agent's image waits for its address in the summary face and on its shelf chip, beside glyphs, and an app's icon keeps its tile",
    html.includes('<span class="symbol symbol-face" aria-hidden="true"><img data-icon="agent-icons/codex?v=0123456789ab" alt="" width="20" height="20"></span>')
      && html.includes('<span class="symbol symbol-chip" aria-hidden="true"><img data-icon="agent-icons/codex?v=0123456789ab" alt="" width="14" height="14"></span>')
      && html.includes('<span class="symbol symbol-face" aria-hidden="true"><svg')
      && !html.includes(" src=\"agent-icons/") && !html.includes("integrations/")
      && !html.includes("Icon from the installed"), html]);
  glanceChecks.push(["at a glance is one closed summary that counts agents and browsers",
    html.startsWith('<details class="glance" data-page-details="glance">')
      && html.includes(">3 agents<") && html.includes(">1 browser<") && html.includes(">Details<")
      && !html.includes("Connection details") && !html.includes("Intake"), html]);
  glanceChecks.push(["the shelves give each agent one chip with a count, and the working one wears the busy glyph",
    html.includes('data-glance-agent="codex"') && html.includes('<span class="tally">5</span>')
      && /class="chip glance-chip busy"[^>]*data-glance-agent="unknown:Claude &lt;b&gt;Code&lt;\/b&gt;"/.test(html)
      && html.includes(">Google Chrome<small>2 tabs</small>") && !html.includes("<b>Code</b>"), html]);
  documentHandlers.get("toggle").at(-1)({ target: { dataset: { pageDetails: "glance" }, open: true,
    classList: { contains: () => false } } });
  html = draw({ sessions: sessions.slice(1), browsers });
  glanceChecks.push(["the opened summary stays open across a refresh",
    html.startsWith('<details class="glance" data-page-details="glance" open>'), html.slice(0, 80)]);
  glanceView.rebuildFeed([]);
  html = draw({ sessions: sessions.slice(2), browsers });
  glanceChecks.push(["the opened summary stays open when the activity feed is rebuilt",
    html.startsWith('<details class="glance" data-page-details="glance" open>'), html.slice(0, 80)]);
  const press = (name) => documentHandlers.get("click").forEach((handler) => handler({ target: {
    closest: (selector) => (selector === "[data-glance-agent]" ? { dataset: { glanceAgent: name } } : null) } }));
  press("codex");
  html = draw({ sessions, browsers });
  glanceChecks.push(["an agent's card says at most four sessions, how many more, and the trust note once",
    (html.match(/class="glance-line"/g) ?? []).length === 4 && html.includes(">and 1 more<")
      && html.includes('<span class="symbol symbol-card" aria-hidden="true"><img data-icon="agent-icons/codex?v=0123456789ab" alt="" width="16" height="16"></span>')
      && (html.match(/is what the agent calls itself/g) ?? []).length === 1
      && html.includes('data-copy-connection="connection_codex_0"') && html.includes(">Copy id<")
      && !html.includes(">connection_codex_0<"), html]);
  press("codex");
  html = draw({ sessions, browsers });
  glanceChecks.push(["pressing the agent again closes its card", !html.includes('class="glance-card"'), html]);
  for (const line of ["Starts Google Chrome on request", "No browser connected"]) {
    html = draw({ sessions, browsers: [], no_browser: line });
    glanceChecks.push([`with no browser the shelf says: ${line}`,
      html.includes(`<span class="chip glance-chip missing"><span class="dot"></span>${line}</span>`)
        && html.includes(">no browser<"), html]);
  }
  const unnamed = { ...claim("unnamed", null), connections: [{ connection_id: "connection_unnamed",
    channel: "mcp", explanation: "The application name is the MCP client's own claim. Ghostlight does not verify it." }] };
  html = draw({ sessions: [unnamed, { ...unnamed, id: "unnamed_2" }], browsers });
  glanceChecks.push(["a session whose client reported no name is one Unknown agent, never a blank chip",
    html.includes('data-glance-agent="unknown"') && html.includes('>Unknown agent <span class="tally">2</span>')
      && !html.includes('data-glance-agent=""') && html.includes(">1 agent<"), html]);
}

// H7: an existing health surface stays quiet when healthy and keeps failed storage explicit.
const healthSnapshot = snapshot();
healthSnapshot.audit_notice = "History cannot be saved. Browser work can continue. Ghostlight checks automatically.";
view.collections(healthSnapshot, new Set());
const auditNoticeVisible = nodes.get("audit-health").hidden === false && nodes.get("audit-health").textContent === healthSnapshot.audit_notice;
healthSnapshot.audit_notice = "History is saving. 2 earlier receipts were not confirmed saved.";
view.collections(healthSnapshot, new Set());
const recoveredGapVisible = nodes.get("audit-health").textContent.includes("2 earlier receipts");
healthSnapshot.audit_notice = "";
view.collections(healthSnapshot, new Set());
const healthyAuditQuiet = nodes.get("audit-health").hidden;
view.policy(compiled(true));
view.setChoice("audit.availability", "require_audit");
const authoredAudit = JSON.parse(view.draftDocument()).config;
const auditPinned = compiled(true);
auditPinned.audit = { mode: "require_audit", source: "organization", organization_mode: "require_audit" };
view.policy(auditPinned);
const auditPinnedMarkup = nodes.get("setting-groups").innerHTML;
// Clear view is disposable view state: running work stays, completed work is hidden, and nothing
// is sent to the orchestrator, so the audit record cannot change.
feedFixture = true;
elementHandlers.get("refresh-status:click")();
await new Promise((r) => setTimeout(r, 0));
const queueBeforeClear = nodes.get("queue-count").textContent;
const clearEnabled = nodes.get("clear-monitor").disabled === false;
invoked.length = 0;
elementHandlers.get("clear-monitor:click")();
await new Promise((r) => setTimeout(r, 0));
const invokedByClear = [...invoked];
const queueAfterClear = nodes.get("queue-count").textContent;
const clearSpent = nodes.get("clear-monitor").disabled === true;
elementHandlers.get("refresh-status:click")();
await new Promise((r) => setTimeout(r, 0));
const queueAfterResync = nodes.get("queue-count").textContent;
feedFixture = false;

// ---------------------------------------------------------------------------------------------
// The same page in each of the three contexts, over loopback HTTP: with the key the shell hands its
// window (Control), and without it (Watch, and Closed while watching is off).
//
// Each run boots the real modules against a fresh stub document and a fake service that records
// every request it is sent, so the assertions are about what a person would see and what the
// page asked for, not about how the page is written.
// ---------------------------------------------------------------------------------------------
const CONTINUE_SENTENCE = "Change this in the Ghostlight window.";
const STALE_SENTENCE = "Your rules changed in another window; your draft is kept.";
const CLOSED_SENTENCE = "Watching is off on this computer. Turn it on in the Ghostlight window.";
const LINKS = { home: "https://example.test/home", source: "https://example.test/source" };
const ACT_KEYS = ["search", "check_policy", "rescan", "continue_in_window", "hold", "resume", "start_session",
  "end_session", "apply_policy", "remove_policy", "set_diagnostics", "set_watching", "setup_all", "harness_act",
  "test_notification", "open", "reveal_diagnostics", "locate", "setup_act", "quit"];
const WATCHER_ACTS = new Set(["search", "check_policy", "rescan", "continue_in_window", "hold"]);

const contextFor = (kind) => {
  if (kind === "closed") {
    return {
      context: "closed", word: "Watching is off", sentence: CLOSED_SENTENCE,
      product: { version: "2.0.0", license: "Apache-2.0 OR MIT" }, links: LINKS
    };
  }
  const control = kind === "control";
  return {
    context: kind, word: control ? "Shared" : "Watching",
    sentence: control ? "Others on this computer can watch this view in a browser."
      : "Anyone on this computer can open this view. Changes are made in the Ghostlight window.",
    instance: "instance_1", opener: control ? "service" : "anchor",
    acts: Object.fromEntries(ACT_KEYS.map((act) => [act, control || WATCHER_ACTS.has(act)
      ? { available: true, why_not: null } : { available: false, why_not: CONTINUE_SENTENCE }])),
    product: { version: "2.0.0", license: "Apache-2.0 OR MIT" }, links: LINKS
  };
};

const roster = () => [
  { id: "codex", product_id: "codex", name: "Codex", label: "Codex", target: "User",
    agent: { id: "codex", name: "Codex", symbol: monogram("codex") }, state: "updatable",
    detail: "Old owned connector.", can_install: true, can_fix: false, can_uninstall: false, can_download: true,
    can_locate: true, config_path: "~/.codex/config.toml", connector_command: "/opt/ghostlight/ghostlight mcp",
    manual_setup: "codex setup", download_url: "https://example.test/codex" },
  { id: "zed", product_id: "zed", name: "Zed", label: "Zed", target: "User",
    agent: { id: "zed", name: "Zed", symbol: monogram("zed") }, state: "installed",
    detail: "Current.", can_install: false, can_fix: false, can_uninstall: true, can_download: true,
    can_locate: true, config_path: "~/.config/zed/settings.json", connector_command: "/opt/ghostlight/ghostlight mcp",
    manual_setup: "zed setup", download_url: "https://example.test/zed" },
  { id: "junie", product_id: "junie", name: "Junie", label: "Junie", target: "CLI",
    agent: { id: "junie", name: "Junie", symbol: monogram("junie") }, state: "needs_attention",
    detail: "Foreign entry preserved.", can_install: false, can_fix: true, can_uninstall: false, can_download: true,
    can_locate: true, config_path: "~/.junie/mcp.json", connector_command: "/opt/ghostlight/ghostlight mcp",
    manual_setup: "junie setup", download_url: "https://example.test/junie" },
  { id: "qwen-code", product_id: "qwen-code", name: "Qwen Code", label: "Qwen Code", target: "CLI",
    agent: { id: "qwen-code", name: "Qwen Code", symbol: monogram("qwen-code") },
    state: "not_detected", detail: "Not detected.", can_install: true, can_fix: false, can_uninstall: false,
    can_download: true, can_locate: true, config_path: "~/.qwen/settings.json",
    connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "qwen setup",
    download_url: "https://example.test/qwen" }
];

/** A stream the fake service holds open, which the test writes events into. */
function openStream() {
  const queued = [];
  const encoder = new TextEncoder();
  let waiting = null;
  return {
    write(text) {
      const value = encoder.encode(text);
      if (waiting) {
        const resolve = waiting;
        waiting = null;
        resolve({ done: false, value });
      } else {
        queued.push(value);
      }
    },
    reader: {
      read: () => (queued.length ? Promise.resolve({ done: false, value: queued.shift() })
        : new Promise((resolve) => { waiting = resolve; }))
    }
  };
}

/** The service the page talks to, in one context. */
function fakeService(kind, { served = true } = {}) {
  const state = {
    seq: 5, runtime: "active", note: null, requests: [], stream: null, bearer: "key-1",
    policyTag: "tag-1", stale: false, watching: true
  };
  const controls = () => controlsPart({
    context: kind, watch: state.watching, runtime: state.runtime, note: state.note, served
  });
  const status = () => ({
    service: { version: "2.0.0", started_at_ms: 0, runtime_state: state.runtime },
    readiness: state.runtime === "held"
      ? { state: "paused", word: "Paused", detail: "Agents are paused.", tone: "held", invites_control: true }
      : { state: "ready", word: "Ready", detail: "Connected and idle.", tone: "quiet", invites_control: true },
    audit_health: {}, audit_notice: "",
    configuration: {
      policy: { situation: "layered", detail: "Your rules apply.", tone: "applied" },
      managed_policy: { configured: false }
    },
    ...(state.note ? { runtime_note: state.note } : {})
  });
  const parts = () => ({
    status: status(),
    connections: kind === "watch" ? WATCHED_CONNECTIONS : { sessions: [], browsers: [] },
    activity: { operations: [], history: [], document_coverage: [] },
    integrations: { harnesses: roster() },
    diagnostics: {
      diagnostics: [{ severity: "passing", label: "Ghostlight", detail: "Running and accepting agents." }]
    },
    controls: controls(),
    ...(kind === "control" ? { setup: retiredSetup } : {})
  });
  const emit = (change) => {
    state.seq += 1;
    state.stream?.write(`id: instance_1-${state.seq}\ndata: ${JSON.stringify({ seq: state.seq, change })}\n\n`);
  };
  const reply = (status_, body, headers = {}) => ({
    ok: status_ >= 200 && status_ < 300, status: status_,
    headers: { get: (name) => headers[name.toLowerCase()] ?? null },
    text: async () => (body === undefined ? "" : JSON.stringify(body))
  });

  async function fetch(path, init = {}) {
    const url = new URL(path, "http://127.0.0.1:41000");
    state.requests.push({
      method: init.method ?? "GET", path: url.pathname, query: url.search, headers: init.headers ?? {},
      body: init.body ?? null, credentials: init.credentials, cache: init.cache
    });
    if (url.pathname === "/api/context") return reply(200, contextFor(kind));
    if (url.pathname === "/api/snapshot") return reply(200, { seq: state.seq, instance: "instance_1", parts: parts() });
    if (url.pathname === "/api/events") {
      state.stream = openStream();
      state.stream.write(`event: hello\ndata: ${JSON.stringify({ context: kind, instance: "instance_1", seq: state.seq })}\n\n`);
      return {
        ok: true, status: 200, headers: { get: () => null },
        body: { getReader: () => state.stream.reader }
      };
    }
    if (url.pathname === "/api/runtime") {
      const { intent } = JSON.parse(init.body);
      state.runtime = intent === "hold" ? "held" : "active";
      state.note = intent === "hold" ? "Paused from a watching page or a local program." : null;
      const answer = reply(200, { accepted: true, runtime_state: state.runtime, message: "Paused." },
        { "ghostlight-seq": String(state.seq + 2) });
      Promise.resolve().then(() => {
        emit({ kind: "status_changed", status: status() });
        emit({ kind: "controls_changed", controls: controls() });
      });
      return answer;
    }
    if (url.pathname === "/api/controls/viewing") {
      state.watching = JSON.parse(init.body).watch;
      const answer = reply(200, controls(), { "ghostlight-seq": String(state.seq + 1) });
      Promise.resolve().then(() => emit({ kind: "controls_changed", controls: controls() }));
      return answer;
    }
    if (url.pathname === "/api/quit") return reply(202, { message: QUITTING_SENTENCE });
    if (url.pathname === "/api/integrations/rescan") return reply(200, roster(), { "ghostlight-seq": String(state.seq) });
    if (url.pathname === "/api/window" || url.pathname === "/api/open") return reply(200, {});
    if (url.pathname === "/api/policy") {
      return reply(200, { ...compiled(true), etag: state.policyTag });
    }
    if (url.pathname === "/api/policy/user") {
      if (state.stale) {
        state.policyTag = "tag-2";
        return reply(412, { error: { code: "stale", message: STALE_SENTENCE } });
      }
      return reply(200, { accepted: true, runtime_state: "active", message: "Your rules are applied." },
        { "ghostlight-seq": String(state.seq) });
    }
    return reply(404, { error: { code: "not_found", message: "There is nothing at this address." } });
  }
  return { state, fetch, emit, status };
}

/** What a watching screen is shown connected: one agent and no browser, which one starts on request. */
const WATCHED_CONNECTIONS = {
  sessions: [{ id: "workspace_codex", agent: namedAgent("Codex"), channel: "mcp",
    tab_count: 0, running_operations: 0,
    connections: [{ connection_id: "connection_watched", reported_application: "codex-mcp-client",
      channel: "mcp", explanation: "\"codex-mcp-client\" is what the agent calls itself; Ghostlight does not verify it." }] }],
  browsers: [],
  no_browser: "Starts Google Chrome on request"
};

/** One page: the real modules over a stub document, in one context. */
function bootHttpPage(kind, options = {}) {
  const service = fakeService(kind, options);
  const bar = addressBar(options.hash ?? "");
  const pageNodes = new Map(ids.map((id) => [id, node(id)]));
  // The markup says which elements start out hidden; the stub starts from the same truth.
  const startsHidden = new Set([...markup.matchAll(/<[^>]*\bid="([^"]+)"[^>]*>/g)]
    .filter(([tag]) => /\shidden[\s>=]/.test(tag)).map(([, id]) => id));
  for (const [id, element] of pageNodes) element.hidden = startsHidden.has(id);
  const pageDocumentHandlers = new Map();
  const pageWindowHandlers = new Map();
  const pageElementHandlers = new Map();
  const opened = [];
  const pageReported = [];
  const copied = [];
  for (const [id, element] of pageNodes) {
    element.addEventListener = (type, handler) => pageElementHandlers.set(`${id}:${type}`, handler);
  }
  const page = {
    console: { error: (detail) => pageReported.push(String(detail)), log() {}, warn() {} },
    document: {
      getElementById: (id) => pageNodes.get(id) ?? null,
      querySelectorAll: (sel) => (sel === "[id]" ? [...pageNodes.values()] : []),
      querySelector: () => null,
      addEventListener: (type, handler) => {
        if (!pageDocumentHandlers.has(type)) pageDocumentHandlers.set(type, []);
        pageDocumentHandlers.get(type).push(handler);
      },
      removeEventListener() {},
      hidden: false, body: node("body"),
      createElement: () => node("x"), createDocumentFragment: () => node("f")
    },
    window: {
      addEventListener: (type, handler) => pageWindowHandlers.set(type, handler),
      location: bar,
      history: bar.history,
      fetch: service.fetch,
      open: (...args) => opened.push(args)
    },
    navigator: { clipboard: { writeText: async (text) => copied.push(text) } },
    GhostlightAccess: kind === "control" ? Object.freeze({ bearer: service.state.bearer }) : undefined,
    setTimeout: (fn, ms) => { if (typeof fn === "function" && !ms) fn(); return 0; },
    clearTimeout() {}, clearInterval() {}, setInterval: () => 0,
    AbortController, TextDecoder, TextEncoder, MutationObserver: class { observe() {} },
    Date, JSON, Math, String, Number, Boolean, Array, Object, Set, Map, Promise, Error,
    isNaN, parseInt, parseFloat, performance: { now: () => 0 }, crypto: { randomUUID: () => "x" }
  };
  page.globalThis = page;
  page.window.document = page.document;
  vm.createContext(page);
  let threw = null;
  try {
    vm.runInContext(ORDER.map((name) => readFileSync(join(ui, name), "utf8")).join("\n"), page);
  } catch (error) {
    threw = error.message;
  }
  const click = (target) => {
    for (const handler of pageDocumentHandlers.get("click") ?? []) handler({ target });
  };
  const clickOn = (selector, element) =>
    click({ closest: (asked) => (asked === selector ? element : null) });
  const settle = () => new Promise((resolve) => setImmediate(resolve));
  const text = (id) => pageNodes.get(id);
  return { service, page, bar, nodes: pageNodes, opened, reported: pageReported, copied, click, clickOn, settle,
    text, threw, windowHandlers: pageWindowHandlers, documentHandlers: pageDocumentHandlers,
    handlers: pageElementHandlers };
}

// ---- Watch ----------------------------------------------------------------------------------
const watching = bootHttpPage("watch");
await watching.settle();
await watching.settle();
const watchRequests = () => watching.service.state.requests;
const watchSnapshots = () => watchRequests().filter((request) => request.path === "/api/snapshot").length;
const watchOpening = {
  chip: watching.text("context-chip").textContent,
  chipTitle: watching.text("context-chip").title,
  setup: watching.text("setup-integrations").hidden,
  endSession: watching.text("controls-end").hidden,
  sendTest: watching.text("test-notification").hidden,
  sheetPause: watching.text("controls-hold").hidden,
  sheetViewingToggle: watching.text("controls-viewing-toggle").hidden,
  sheetDiagnosticsToggle: watching.text("controls-diagnostics-toggle").hidden,
  sheetReveal: watching.text("controls-diagnostics-reveal").hidden,
  sheetContinue: watching.text("settings-sharing-continue").hidden,
  sheetContinueWhy: watching.text("settings-sharing-why").textContent,
  sheetContinues: ["sharing", "browser-work", "notifications", "diagnostics"]
    .map((section) => [watching.text(`settings-${section}-continue`).hidden, watching.text(`settings-${section}-why`).textContent]),
  sheetViewingWord: watching.text("controls-viewing-word").textContent,
  quitHidden: watching.text("settings-quit").hidden,
  who: watching.text("settings-sharing-who").textContent,
  rail: ["screen", "sharing", "browser-work", "notifications", "diagnostics"]
    .map((section) => watching.text(`rail-${section}-state`).textContent),
  statusService: watching.text("status-service").textContent,
  statusServiceHidden: watching.text("status-service").hidden,
  addressWritten: watching.bar.replaced.length,
  recheck: watching.text("refresh-integrations").hidden,
  wheelHidden: watching.text("wheel").hidden,
  bandContinue: watching.text("band-continue").hidden,
  roster: watching.text("agents-list").innerHTML + watching.text("agents-pane").innerHTML,
  copyCommandHidden: watching.text("copy-command").hidden,
  rosterLine: watching.text("integrations-continue").innerHTML,
  rosterLineHidden: watching.text("integrations-continue").hidden,
  status: watching.text("diagnostic-grid").innerHTML,
  applyHidden: watching.text("apply-policy").hidden,
  policyLine: watching.text("policy-continue").innerHTML,
  policyLineHidden: watching.text("policy-continue").hidden
};
watching.clickOn("[data-agent]", { dataset: { agent: "qwen-code" } });
const watchQwenPane = watching.text("agents-pane").innerHTML;
const watchGlance = { closed: watching.text("connections").innerHTML };
watching.clickOn("[data-glance-agent]", { dataset: { glanceAgent: "codex" } });
watchGlance.card = watching.text("connections").innerHTML;
watching.clickOn("[data-copy-connection]", { dataset: { copyConnection: "connection_watched" } });
await watching.settle();
watchGlance.copied = [...watching.copied];
watchGlance.toast = watching.text("toast").textContent;
watching.clickOn("[data-glance-agent]", { dataset: { glanceAgent: "codex" } });
const watchBeforeFocus = watchRequests().length;
watching.windowHandlers.get("focus")?.();
await watching.settle();
const watchFocusSent = watchRequests().length - watchBeforeFocus;
const watchBeforeHold = { snapshots: watchSnapshots(), requests: watchRequests().length };
watching.clickOn("#wheel", { dataset: { intent: "hold" }, disabled: false });
await watching.settle();
await watching.settle();
await watching.settle();
const holdRequest = watchRequests().find((request) => request.path === "/api/runtime");
const watchHeld = {
  wheelHidden: watching.text("wheel").hidden,
  state: watching.text("wheel-state").hidden,
  word: watching.text("wheel-state-word").textContent,
  note: watching.text("wheel-state-note").textContent,
  continueHidden: watching.text("band-continue").hidden,
  continueMarkup: watching.text("band-continue").innerHTML,
  snapshotsAfter: watchSnapshots(),
  sheetRuntime: watching.text("controls-runtime-word").textContent,
  sheetNote: watching.text("controls-runtime-note").textContent,
  sheetResume: watching.text("controls-resume").hidden,
  sheetContinue: watching.text("settings-browser-work-continue").hidden
};
const watchBeforeAbout = watchRequests().length;
watching.clickOn("[data-destination]", { dataset: { destination: "home" } });
await watching.settle();
const watchAbout = { requests: watchRequests().length - watchBeforeAbout, opened: watching.opened.map((call) => [...call]) };
const watchShowing = vm.runInContext("view.showing()", watching.page);
watching.clickOn("[data-continue]", { dataset: {} });
await watching.settle();
const continueRequest = watchRequests().find((request) => request.path === "/api/window");
// A Continue line inside a Settings section continues at that section.
const windowRequests = () => watchRequests().filter((request) => request.path === "/api/window");
const sectionContinue = (section) => {
  const button = { disabled: false, dataset: {}, closest: (asked) => (asked === ".settings-section" ? { dataset: { section } } : null) };
  watching.click({
    closest: (asked) => (asked === "[data-settings-continue]" ? button
      : asked === "#view-settings" ? watching.text("view-settings") : null)
  });
};
sectionContinue("diagnostics");
await watching.settle();
sectionContinue("sharing");
await watching.settle();
const sectionContinues = windowRequests().slice(1).map((request) => request.body);
// A Continue line inside the roster continues at Integrations.
watching.clickOn("[data-continue]", { dataset: { address: "integrations" } });
await watching.settle();
const rosterContinue = windowRequests()[3]?.body;
watching.clickOn("[data-view]", { dataset: { view: "integrations" } });
await watching.settle();
const rescanRequests = watchRequests().filter((request) => request.path === "/api/integrations/rescan").length;

// ---- Control, over HTTP ---------------------------------------------------------------------
const controlling = bootHttpPage("control");
await controlling.settle();
await controlling.settle();
const controlRequests = controlling.service.state.requests;
const controlSnapshots = () => controlRequests.filter((request) => request.path === "/api/snapshot").length;
const controlOpening = {
  chip: controlling.text("context-chip").textContent,
  setup: controlling.text("setup-integrations").hidden,
  endSession: controlling.text("controls-end").hidden,
  sendTest: controlling.text("test-notification").hidden,
  sheetPause: controlling.text("controls-hold").hidden,
  sheetViewingToggle: controlling.text("controls-viewing-toggle").hidden,
  sheetDiagnosticsToggle: controlling.text("controls-diagnostics-toggle").hidden,
  sheetReveal: controlling.text("controls-diagnostics-reveal").hidden,
  sheetContinues: ["sharing", "browser-work", "notifications", "diagnostics"].map((section) => controlling.text(`settings-${section}-continue`).hidden),
  quitHidden: controlling.text("settings-quit").hidden,
  who: controlling.text("settings-sharing-who").textContent,
  statusService: controlling.text("status-service").textContent,
  addressWritten: controlling.bar.replaced.length,
  wheelHidden: controlling.text("wheel").hidden,
  bandContinue: controlling.text("band-continue").hidden,
  rosterLineHidden: controlling.text("integrations-continue").hidden,
  roster: controlling.text("agents-list").innerHTML + controlling.text("agents-pane").innerHTML,
  status: controlling.text("diagnostic-grid").innerHTML,
  applyHidden: controlling.text("apply-policy").hidden,
  policyLineHidden: controlling.text("policy-continue").hidden
};
const controlPanes = Object.fromEntries(["junie", "codex", "zed", "qwen-code"].map((id) => {
  controlling.clickOn("[data-agent]", { dataset: { agent: id } });
  return [id, controlling.text("agents-pane").innerHTML];
}));
controlling.clickOn("[data-harness-operation]", {
  dataset: { harness: "zed", harnessOperation: "copy", copyKind: "command", harnessName: "Zed" }
});
await controlling.settle();
controlling.clickOn("[data-harness-operation]", {
  dataset: { harness: "zed", harnessOperation: "copy", copyKind: "setup", harnessName: "Zed" }
});
await controlling.settle();
const toastAfterCopy = controlling.text("toast").textContent;
const controlSnapshotsBeforeHold = controlSnapshots();
controlling.clickOn("#wheel", { dataset: { intent: "hold" }, disabled: false });
await controlling.settle();
await controlling.settle();
await controlling.settle();
const controlHold = controlRequests.find((request) => request.path === "/api/runtime");
const controlSnapshotsAfterHold = controlSnapshots();
const controlWordAfterHold = controlling.text("wheel").dataset.intent;
controlling.clickOn("[data-destination]", { dataset: { destination: "home" } });
await controlling.settle();
const controlOpenedLinks = controlling.opened.length;
const openRequest = controlRequests.find((request) => request.path === "/api/open");
controlling.clickOn("[data-harness-operation]", {
  dataset: { harness: "qwen-code", harnessOperation: "download", harnessName: "Qwen Code" }
});
await controlling.settle();
const downloadRequest = controlRequests.filter((request) => request.path === "/api/open")[1];

// Settings in the window: a press inside it acts through the transport, and its state follows the
// change the act caused without a snapshot.
const clickInSettings = (page, selector, element) => page.click({
  closest: (asked) => (asked === selector ? element : asked === "#view-settings" ? page.text("view-settings") : null)
});
const where = (page) => ({
  view: vm.runInContext("view.showing()", page.page),
  section: vm.runInContext("view.showingSection()", page.page),
  hash: page.bar.hash,
  replaced: [...page.bar.replaced],
  pushed: [...page.bar.pushed]
});
const controlSnapshotsBeforeSheet = controlSnapshots();
clickInSettings(controlling, "[data-viewing]", { dataset: { viewing: "toggle" }, disabled: false });
await controlling.settle();
await controlling.settle();
await controlling.settle();
const viewingRequest = controlRequests.find((request) => request.path === "/api/controls/viewing");
const sheetAfterOff = {
  rail: controlling.text("rail-sharing-state").textContent,
  who: controlling.text("settings-sharing-who").textContent,
  word: controlling.text("controls-viewing-word").textContent,
  toggle: controlling.text("controls-viewing-toggle").textContent,
  chipTitle: controlling.text("context-chip").title,
  snapshots: controlSnapshots() - controlSnapshotsBeforeSheet
};
clickInSettings(controlling, "[data-intent]", { dataset: { intent: "end_session" }, disabled: false });
await controlling.settle();
await controlling.settle();
const intentRequests = controlRequests.filter((request) => request.path === "/api/runtime");

// The rail: an entry shows its section and writes its address in place, the tab returns to the
// section shown last, and the band's own wheel is not Settings' to answer.
controlling.clickOn("[data-settings-section]", { dataset: { settingsSection: "browser-work" } });
const afterRail = where(controlling);
controlling.clickOn("[data-view]", { dataset: { view: "about" } });
controlling.clickOn("[data-view]", { dataset: { view: "settings" } });
const afterTab = where(controlling);

// The rules: the tag the page read is the tag a write quotes, and a stale refusal keeps the draft.
controlling.clickOn("[data-view]", { dataset: { view: "policy" } });
await controlling.settle();
await controlling.settle();
vm.runInContext("view.addRule()", controlling.page);
const draftBeforeStale = vm.runInContext("view.draftDocument()", controlling.page);
const controlPolicyReads = () => controlRequests.filter((request) => request.path === "/api/policy").length;
const readsBeforeStale = controlPolicyReads();
controlling.service.state.stale = true;
controlling.handlers.get("apply-policy:click")();
await controlling.settle();
await controlling.settle();
await controlling.settle();
const staleWrite = controlRequests.find((request) => request.path === "/api/policy/user");
const staleOutcome = {
  toast: controlling.text("alert-text").textContent,
  dirty: vm.runInContext("view.draftIsDirty()", controlling.page),
  draft: vm.runInContext("view.draftDocument()", controlling.page),
  reads: controlPolicyReads() - readsBeforeStale
};
controlling.service.state.stale = false;
controlling.handlers.get("apply-policy:click")();
await controlling.settle();
await controlling.settle();
await controlling.settle();
const writes = controlRequests.filter((request) => request.path === "/api/policy/user");

// Quit asks first, inline; only the second press goes to the service, and the band then says the
// service is quitting instead of reconnecting.
const quitRequests = () => controlRequests.filter((request) => request.path === "/api/quit");
const quitAsk = { dataset: { quit: "ask" }, disabled: false };
const quitYes = { dataset: { quit: "yes" }, disabled: false };
const quitNo = { dataset: { quit: "no" }, disabled: false };
clickInSettings(controlling, "[data-quit]", quitAsk);
const quitAsked = {
  question: controlling.text("settings-quit-question").hidden === false,
  ask: controlling.text("settings-quit-ask").hidden,
  requests: quitRequests().length
};
clickInSettings(controlling, "[data-quit]", quitNo);
const quitCancelled = {
  question: controlling.text("settings-quit-question").hidden,
  ask: controlling.text("settings-quit-ask").hidden,
  requests: quitRequests().length
};
clickInSettings(controlling, "[data-quit]", quitAsk);
clickInSettings(controlling, "[data-quit]", quitYes);
await controlling.settle();
await controlling.settle();
await controlling.settle();
const quitDone = {
  requests: quitRequests().map((request) => [request.method, request.body, request.headers.Authorization]),
  word: controlling.text("state-word").textContent,
  wheelDisabled: controlling.text("wheel").disabled,
  showing: vm.runInContext("view.showing()", controlling.page)
};
// A stream lost and found again under a quitting page does not turn the sentence back into "Reconnecting".
vm.runInContext("view.reconnecting(true); view.reconnecting(false)", controlling.page);
const quitAfterReconnect = controlling.text("state-word").textContent;

// ---- Control, with no watch view served -------------------------------------------------------
const unserved = bootHttpPage("control", { served: false });
await unserved.settle();
await unserved.settle();
const unservedSheet = {
  viewingWord: unserved.text("controls-viewing-word").textContent,
  viewingToggle: unserved.text("controls-viewing-toggle").hidden,
  viewingCopy: unserved.text("controls-viewing-copy").hidden,
  continueLine: unserved.text("settings-sharing-continue").hidden,
  pause: unserved.text("controls-hold").hidden
};

// ---- Closed ---------------------------------------------------------------------------------
const closedPage = bootHttpPage("closed");
await closedPage.settle();
await closedPage.settle();
const closedRequests = closedPage.service.state.requests;
const closedViews = ["closed-monitor", "closed-integrations", "closed-status", "closed-policy"]
  .map((id) => [closedPage.text(id).textContent, closedPage.text(id).hidden]);
const closedAbout = closedPage.text("about-facts").innerHTML;
closedPage.clickOn("[data-view]", { dataset: { view: "policy" } });
await closedPage.settle();
closedPage.clickOn("[data-destination]", { dataset: { destination: "source" } });
await closedPage.settle();
const closedSettings = {
  lines: ["sharing", "browser-work", "notifications", "diagnostics"].map((section) => [
    closedPage.text(`settings-${section}-closed`).textContent,
    closedPage.text(`settings-${section}-closed`).hidden,
    closedPage.text(`settings-${section}-rows`).hidden,
    closedPage.text(`rail-${section}-state`).textContent
  ]),
  rail: closedPage.text("rail-screen-state").textContent,
  quitHidden: closedPage.text("settings-quit").hidden,
  statusService: closedPage.text("status-service").textContent
};
const closedState = {
  requests: closedRequests.map((request) => `${request.method} ${request.path}`),
  chip: closedPage.text("context-chip").textContent,
  wheelHidden: closedPage.text("wheel").hidden,
  wordText: closedPage.text("state-word").textContent,
  opened: closedPage.opened.map((call) => [...call])
};

// ---- Addresses: every tab has one, and a page opens on the one it is given ------------------------
const addressed = async (kind, hash) => {
  const opened = bootHttpPage(kind, { hash });
  await opened.settle();
  await opened.settle();
  return opened;
};
const atSection = await addressed("control", "#settings/sharing");
const atSectionWhere = where(atSection);
const atBare = await addressed("control", "#settings");
const atBareWhere = where(atBare);
const atUnknown = await addressed("control", "#nowhere");
const atUnknownWhere = where(atUnknown);
const atNothing = await addressed("control", "");
const atNothingWhere = where(atNothing);
const atPolicy = await addressed("control", "#policy");
const policyReadsAtAddress = atPolicy.service.state.requests.filter((request) => request.path === "/api/policy").length;
const atWatch = await addressed("watch", "#settings/browser-work");
const atWatchWhere = where(atWatch);
const atClosed = await addressed("closed", "#settings/diagnostics");
const atClosedWhere = where(atClosed);
const atClosedLine = atClosed.text("settings-diagnostics-closed").textContent;
// A hash typed or followed afterwards moves the page, and nothing is added to the history.
atNothing.bar.hash = "#settings/diagnostics";
atNothing.windowHandlers.get("hashchange")();
await atNothing.settle();
const afterHashChange = where(atNothing);
atNothing.bar.hash = "#about";
atNothing.windowHandlers.get("hashchange")();
const afterAbout = where(atNothing);
atNothing.bar.hash = "#settings/nowhere";
atNothing.windowHandlers.get("hashchange")();
const afterUnknownChange = where(atNothing);
atNothing.bar.hash = "";
atNothing.windowHandlers.get("hashchange")();
const afterEmptyChange = where(atNothing);
const addressedPages = [atSection, atBare, atUnknown, atNothing, atPolicy, atWatch, atClosed];

const httpChecks = [
  ["a watching page sees the same summary, shelves and card, and may copy an id",
    watchGlance.closed.includes('class="glance"') && watchGlance.closed.includes("Starts Google Chrome on request")
      && watchGlance.card.includes('class="glance-card"') && watchGlance.card.includes("is what the agent calls itself")
      && JSON.stringify(watchGlance.copied) === JSON.stringify(["connection_watched"])
      && watchGlance.toast === "Connection id copied.",
    JSON.stringify(watchGlance)],
  ["over HTTP a watching page says so in its chip and keeps the service's sentence beside it",
    watchOpening.chip === "Watching" && watchOpening.chipTitle.includes("Changes are made in the Ghostlight window"),
    JSON.stringify(watchOpening.chip)],
  ["a watching page draws no control for a change it cannot make",
    watchOpening.setup === true && watchOpening.endSession === true && watchOpening.sendTest === true
      && watchOpening.sheetViewingToggle === true && watchOpening.sheetDiagnosticsToggle === true
      && watchOpening.sheetReveal === true
      && !/>Set up<|>Update<|>Connect<|>Prepare<|Remove from|>Fix<|>Locate<|Turn on|Turn off|Open log folder/
        .test(watchOpening.roster + watchQwenPane + watchOpening.status)
      && watchOpening.recheck === false,
    JSON.stringify({ ...watchOpening, roster: undefined, status: undefined })],
  ["a watching page keeps the state of every agent as text and offers Copy and Install as plain controls",
    watchOpening.roster.includes("Needs an update") && watchOpening.roster.includes("Needs your attention")
      && watchOpening.roster.includes(">Copy setup<") && watchOpening.copyCommandHidden === false
      && watchQwenPane.includes('<a class="primary-button" href="https://example.test/qwen"')
      && !watchQwenPane.includes('data-harness-operation="download"'),
    watchOpening.roster + watchQwenPane],
  ["a watching page shows one Continue link for the roster, one Continue line for each section of Settings and none on Status",
    watchOpening.rosterLineHidden === false && !watchOpening.rosterLine.includes(CONTINUE_SENTENCE)
      && watchOpening.rosterLine.includes("data-continue") && watchOpening.rosterLine.includes("Continue in the Ghostlight window")
      && !watchOpening.status.includes("data-continue")
      && watchOpening.sheetContinue === false && watchOpening.sheetContinueWhy === CONTINUE_SENTENCE
      && watchOpening.sheetContinues.every(([hidden, why]) => hidden === false && why === CONTINUE_SENTENCE),
    watchOpening.rosterLine + watchOpening.status],
  ["Settings in a watching page shows Pause while agents run and the rest as state, with no Quit",
    watchOpening.sheetPause === false && watchOpening.quitHidden === true
      && watchOpening.sheetViewingWord === "On: anyone on this computer can open this view"
      && JSON.stringify(watchOpening.rail) === JSON.stringify([
        "Comfortable, standard text, motion follows the system", "On: anyone on this computer can open this view", "Running",
        "On", "On (marker)"]),
    JSON.stringify(watchOpening)],
  ["Sharing says who can see a watching page, and Status carries the service's sentence at its top",
    watchOpening.who === SHEET_WATCHER
      && watchOpening.statusService === "Ghostlight is running; 1 agent and 1 browser connected."
      && watchOpening.statusServiceHidden === false,
    JSON.stringify({ who: watchOpening.who, status: watchOpening.statusService })],
  ["a watching page draws the wheel while agents are running, and Policy with no Apply under one line naming the window",
    watchOpening.wheelHidden === false && watchOpening.bandContinue === true
      && watchOpening.applyHidden === true && watchOpening.policyLineHidden === false
      && watchOpening.policyLine.includes("<span>Changes are made in the Ghostlight window.</span>")
      && watchOpening.policyLine.includes("Continue in the Ghostlight window"),
    JSON.stringify({ wheel: watchOpening.wheelHidden, continue: watchOpening.bandContinue, apply: watchOpening.applyHidden,
      line: watchOpening.policyLine })],
  ["a pause from a watching page is one keyless POST and is shown by waiting for its change, not by asking again",
    holdRequest?.method === "POST" && holdRequest.body === '{"intent":"hold"}' && holdRequest.headers.Authorization === undefined
      && holdRequest.credentials === "omit" && holdRequest.cache === "no-store"
      && watchHeld.snapshotsAfter === watchBeforeHold.snapshots,
    JSON.stringify({ holdRequest, snapshots: [watchBeforeHold.snapshots, watchHeld.snapshotsAfter] })],
  ["while held, a watching page shows the state and its note where the wheel was, with the Continue line in the band",
    watchHeld.wheelHidden === true && watchHeld.state === false && watchHeld.word === "Paused"
      && watchHeld.note === "Paused from a watching page or a local program." && watchHeld.continueHidden === false
      && watchHeld.continueMarkup.includes("data-continue"),
    JSON.stringify(watchHeld)],
  ["while held, the sheet of a watching page says Paused and where it came from, and offers no Resume",
    watchHeld.sheetRuntime === "Paused" && watchHeld.sheetNote === "Paused from a watching page or a local program."
      && watchHeld.sheetResume === true && watchHeld.sheetContinue === false,
    JSON.stringify(watchHeld)],
  ["the About card of a watching page sends nothing and opens the Home link in a new tab",
    watchAbout.requests === 0 && watchAbout.opened.length === 1
      && watchAbout.opened[0][0] === LINKS.home && watchAbout.opened[0][1] === "_blank"
      && watchAbout.opened[0][2] === "noopener,noreferrer",
    JSON.stringify(watchAbout)],
  ["a window focus sends nothing", watchFocusSent === 0, `sent=${watchFocusSent}`],
  ["Continue in the Ghostlight window is one POST asking for the workbench at the screen shown",
    continueRequest?.method === "POST"
      && continueRequest.body === JSON.stringify({ view: "workbench", address: watchShowing }),
    JSON.stringify(continueRequest)],
  ["Continue in a Settings section asks for the window at that section's address",
    JSON.stringify(sectionContinues)
      === JSON.stringify(['{"view":"workbench","address":"settings/diagnostics"}', '{"view":"workbench","address":"settings/sharing"}']),
    JSON.stringify(sectionContinues)],
  ["Continue in the roster asks for the window at Integrations",
    rosterContinue === '{"view":"workbench","address":"integrations"}', String(rosterContinue)],
  ["opening Integrations looks for the clients once", rescanRequests === 1, `rescans=${rescanRequests}`],
  ["a page with the key draws every control the window draws and no Continue line",
    controlOpening.chip === "Shared" && controlOpening.setup === false && controlOpening.endSession === false
      && controlOpening.sendTest === false && controlOpening.wheelHidden === false
      && controlOpening.bandContinue === true && controlOpening.rosterLineHidden === true
      && controlOpening.applyHidden === false && controlOpening.policyLineHidden === true
      && !(controlOpening.roster + controlOpening.status).includes("data-continue")
      && controlPanes.codex.includes(">Update</button>") && controlPanes.codex.includes('data-harness-state="updatable"')
      && controlPanes.zed.includes(">Remove from Zed</button>")
      && controlPanes.junie.includes(">Fix</button>") && controlPanes.junie.includes(">Locate</button>")
      && controlPanes["qwen-code"].includes('data-harness-operation="download"')
      && controlOpening.sheetPause === false
      && controlOpening.sheetViewingToggle === false && controlOpening.sheetDiagnosticsToggle === false
      && controlOpening.sheetReveal === false && controlOpening.sheetContinues.every((hidden) => hidden === true),
    JSON.stringify({ ...controlOpening, roster: undefined, status: undefined })],
  ["the window's Settings offers Quit, says who can see, and Status carries the service's sentence",
    controlOpening.quitHidden === false && controlOpening.who === SHEET_WATCHED
      && controlOpening.statusService === "Ghostlight is running; 1 agent and 1 browser connected."
      && controlOpening.addressWritten === 0,
    JSON.stringify({ quit: controlOpening.quitHidden, who: controlOpening.who, status: controlOpening.statusService })],
  ["the window with no watch view served says so once, offers no toggle, and does not send itself to the window",
    unservedSheet.viewingWord === SHEET_NOT_SERVED && unservedSheet.viewingToggle === true
      && unservedSheet.viewingCopy === true && unservedSheet.continueLine === true
      && unservedSheet.pause === false && unserved.threw === null && unserved.reported.length === 0,
    JSON.stringify({ unservedSheet, threw: unserved.threw, reported: unserved.reported })],
  ["a rail entry shows its section and writes its address in place, and the Settings tab returns to the last one",
    afterRail.view === "settings" && afterRail.section === "browser-work" && afterRail.hash === "#settings/browser-work"
      && afterRail.pushed.length === 0 && afterTab.view === "settings" && afterTab.hash === "#settings/browser-work"
      && afterTab.pushed.length === 0,
    JSON.stringify({ afterRail, afterTab })],
  ["Turn off in Settings is one keyed PUT, waited for as a change; the rail, the sentence and the chip follow",
    viewingRequest?.method === "PUT" && viewingRequest.body === '{"watch":false}'
      && viewingRequest.headers.Authorization === "Bearer key-1" && viewingRequest.credentials === "omit"
      && sheetAfterOff.word === "Off: only this window shows Ghostlight" && sheetAfterOff.toggle === "Turn on"
      && sheetAfterOff.rail === "Off: only this window shows Ghostlight" && sheetAfterOff.who === SHEET_PRIVATE
      && sheetAfterOff.chipTitle === SHEET_PRIVATE && sheetAfterOff.snapshots === 0,
    JSON.stringify({ viewingRequest, sheetAfterOff })],
  ["Quit asks inline first, Cancel puts the button back, and only the second press is one keyed POST",
    quitAsked.question && quitAsked.ask === true && quitAsked.requests === 0
      && quitCancelled.question === true && quitCancelled.ask === false && quitCancelled.requests === 0
      && JSON.stringify(quitDone.requests) === JSON.stringify([["POST", "{}", "Bearer key-1"]]),
    JSON.stringify({ quitAsked, quitCancelled, quitDone })],
  ["after Quit the band says Ghostlight is quitting, not Reconnecting, and cannot be pressed",
    quitDone.word === QUITTING_SENTENCE && quitDone.wheelDisabled === true && quitAfterReconnect === QUITTING_SENTENCE,
    JSON.stringify({ quitDone, quitAfterReconnect })],
  ["the window opens on a typed address, a bare one is made whole, an unknown one falls back, and none adds history",
    atSectionWhere.view === "settings" && atSectionWhere.section === "sharing" && atSectionWhere.replaced.length === 0
      && atBareWhere.view === "settings" && atBareWhere.hash === "#settings/screen"
      && atUnknownWhere.view === "monitor" && atUnknownWhere.hash === "#monitor"
      && atNothingWhere.view === "monitor" && atNothingWhere.replaced.length === 0
      && addressedPages.every((opened) => opened.bar.pushed.length === 0),
    JSON.stringify({ atSectionWhere, atBareWhere, atUnknownWhere, atNothingWhere })],
  ["a page opened on #policy reads the rules once, as a click on the tab does",
    policyReadsAtAddress === 1, `reads=${policyReadsAtAddress}`],
  ["a hash that changes under an open page moves it, an unknown one falls back, and an emptied one opens the default",
    afterHashChange.view === "settings" && afterHashChange.section === "diagnostics"
      && afterAbout.view === "about" && afterUnknownChange.view === "monitor" && afterUnknownChange.hash === "#monitor"
      && afterEmptyChange.view === "monitor",
    JSON.stringify({ afterHashChange, afterAbout, afterUnknownChange, afterEmptyChange })],
  ["a watching page and a closed one open on a section too",
    atWatchWhere.view === "settings" && atWatchWhere.section === "browser-work"
      && atClosedWhere.view === "settings" && atClosedWhere.section === "diagnostics"
      && atClosedLine === CLOSED_SENTENCE,
    JSON.stringify({ atWatchWhere, atClosedWhere, atClosedLine })],
  ["End session in the sheet asks for the end_session intent",
    intentRequests.some((request) => request.body === '{"intent":"end_session"}'),
    JSON.stringify(intentRequests.map((request) => request.body))],
  ["every request the window makes carries the key and none carries credentials",
    controlRequests.length > 0
      && controlRequests.every((request) => request.headers.Authorization === "Bearer key-1" && request.credentials === "omit"),
    JSON.stringify(controlRequests.map((request) => [request.path, request.headers.Authorization]))],
  ["Copy puts the client's own command and setup on the clipboard and says so",
    JSON.stringify(controlling.copied) === JSON.stringify(["/opt/ghostlight/ghostlight mcp", "zed setup"])
      && toastAfterCopy === "Manual setup copied.",
    JSON.stringify(controlling.copied)],
  ["the window's pause is waited for as well", controlHold?.headers.Authorization === "Bearer key-1"
    && controlSnapshotsAfterHold === controlSnapshotsBeforeHold && controlWordAfterHold === "resume",
    `snapshots=${controlSnapshotsBeforeHold}->${controlSnapshotsAfterHold} intent=${controlWordAfterHold}`],
  ["Install in a page that cannot open links asks the service for that host's page by its harness id",
    downloadRequest?.body === '{"harness":"qwen-code"}'
      && controlPanes["qwen-code"].includes('data-harness-operation="download" data-harness="qwen-code"')
      && !controlPanes["qwen-code"].includes("data-product"),
    JSON.stringify(downloadRequest)],
  ["the window asks its service to open a link, and a tab opens it itself",
    controlOpenedLinks === 0 && openRequest?.body === '{"destination":"home"}'
      && openRequest.headers.Authorization === "Bearer key-1" && watchAbout.opened.length === 1,
    JSON.stringify(openRequest)],
  ["a write quotes the tag the page read, and a stale refusal says so, keeps the draft and reads the rules again",
    staleWrite?.method === "PUT" && staleWrite.headers["If-Match"] === '"tag-1"'
      && staleOutcome.toast === STALE_SENTENCE && staleOutcome.dirty && staleOutcome.draft === draftBeforeStale
      && staleOutcome.reads === 1,
    JSON.stringify({ staleWrite, staleOutcome })],
  ["the next write quotes the new tag and goes through",
    writes.length === 2 && writes[1].headers["If-Match"] === '"tag-2"'
      && controlling.text("toast").textContent === "Your rules are applied.",
    JSON.stringify(writes.map((request) => request.headers))],
  ["a closed page keeps This screen and gives every other section one line saying watching is off",
    closedSettings.lines.every(([line, lineHidden, rowsHidden, rail]) =>
      line === CLOSED_SENTENCE && lineHidden === false && rowsHidden === true && rail === "")
      && closedSettings.rail === "Comfortable, standard text, motion follows the system" && closedSettings.quitHidden === true
      && closedSettings.statusService === "",
    JSON.stringify(closedSettings)],
  ["a closed page asks for its context and nothing else",
    JSON.stringify(closedState.requests) === JSON.stringify(["GET /api/context"]),
    JSON.stringify(closedState.requests)],
  ["a closed page says why in every data view and draws no wheel and no lamp state",
    closedViews.every(([line, hidden]) => line === CLOSED_SENTENCE && hidden === false)
      && closedState.chip === "Watching is off" && closedState.wheelHidden === true && closedState.wordText === ""
      && closedPage.service.state.requests.length === 1,
    JSON.stringify({ closedViews, closedState })],
  ["a closed page still shows the About page in full, without the counts that come from data",
    closedAbout.includes("<dt>Version</dt><dd>2.0.0</dd>") && closedAbout.includes("<dt>License</dt><dd>Apache-2.0 OR MIT</dd>")
      && !closedAbout.includes("Agents") && !closedAbout.includes("Recorded"),
    closedAbout],
  ["a closed page opens an About link in a new tab", closedState.opened.length === 1
    && closedState.opened[0][0] === LINKS.source, JSON.stringify(closedState.opened)],
  ["no page threw while booting", [watching, controlling, closedPage].every((p) => p.threw === null && p.reported.length === 0),
    JSON.stringify([watching, controlling, closedPage].map((p) => [p.threw, p.reported]))],
  ["no page opened on an address threw or reported",
    addressedPages.every((p) => p.threw === null && p.reported.length === 0),
    JSON.stringify(addressedPages.map((p) => [p.threw, p.reported]))]
];

const checks = [
  ...httpChecks,
  ["the window's page wears the Sharing word while others can watch, and draws every control",
    sheetAfterBoot.chip === "Shared"
      && nodes.get("setup-integrations").hidden === false && sheetAfterBoot.end === false
      && sheetAfterBoot.test === false && nodes.get("integrations-continue").hidden === true
      && nodes.get("band-continue").hidden === true && sheetAfterBoot.continueLine === true,
    `chip=${sheetAfterBoot.chip}`],
  ["Settings is drawn from the snapshot's controls part in the service's words, and Status carries the sentence",
    sheetAfterBoot.runtimeLabel === "Browser work"
      && sheetAfterBoot.runtimeWord === "Running" && sheetAfterBoot.hold === false
      && sheetAfterBoot.resume === true && sheetAfterBoot.start === true
      && sheetAfterBoot.service === "Ghostlight is running; 1 agent and 1 browser connected."
      && sheetAfterBoot.viewingWord === "On: anyone on this computer can open this view"
      && sheetAfterBoot.viewingUrl === "http://127.0.0.1:41234/"
      && sheetAfterBoot.viewingToggle === "Turn off" && sheetAfterBoot.viewingToggleHidden === false
      && sheetAfterBoot.notificationsWord.startsWith("Send one local checkup notification"),
    JSON.stringify(sheetAfterBoot)],
  ["the rail states the sections, and the window is offered Quit and told who can see",
    sheetAfterBoot.railScreen === "Comfortable, standard text, motion follows the system"
      && sheetAfterBoot.railSharing === "On: anyone on this computer can open this view"
      && sheetAfterBoot.railBrowserWork === "Running" && sheetAfterBoot.railDiagnostics === "On (marker)"
      && sheetAfterBoot.railNotifications === "On"
      && sheetAfterBoot.quitShown && sheetAfterBoot.who.startsWith("Others on this computer can watch")
      && sheetAfterBoot.address === 0
      && sheetAfterBoot.errors.every((error) => error.includes("deliberate About failure")),
    JSON.stringify(sheetAfterBoot)],
  ["the diagnostics section renders the state and both person acts",
    sheetAfterBoot.diagnosticsWord === "On (marker)" && sheetAfterBoot.diagnosticsPlace === "~/.cache/ghostlight (4.1 KB)"
      && sheetAfterBoot.diagnosticsToggle === "Turn off" && sheetAfterBoot.diagnosticsToggleHidden === false
      && sheetAfterBoot.diagnosticsReveal === false,
    JSON.stringify(sheetAfterBoot)],
  ["a pushed controls part updates Settings and takes the chip away once only the window shows Ghostlight, with no snapshot",
    sheetAfterPush.railSharing === "Off: only this window shows Ghostlight" && sheetAfterPush.railDiagnostics === "Off"
      && sheetAfterPush.viewingWord === "Off: only this window shows Ghostlight"
      && sheetAfterPush.viewingToggle === "Turn on" && sheetAfterPush.diagnosticsWord === "Off"
      && sheetAfterPush.diagnosticsToggle === "Turn on" && sheetAfterPush.chip === "" && sheetAfterPush.chipHidden === true
      && sheetAfterPush.chipTitle === SHEET_PRIVATE && sheetAfterPush.snapshots === 0,
    JSON.stringify(sheetAfterPush)],
  ["the Status page draws no process diagnostics, which moved to Settings",
    !nodes.get("diagnostic-grid").innerHTML.includes("Process diagnostics"),
    nodes.get("diagnostic-grid").innerHTML],
  ["a pushed status changes the word, the sentence behind it and the wheel with no snapshot",
    wordBeforePush === "Ready" && wordAfterPush === "Paused"
      && wordTitleAfterPush === "Paused from the tray."
      && wheelAfterPush === "resume" && requestsAfterPush === requestsBeforePush
      && titleAfterResume === "Connected and idle. Agents can work when they ask.",
    `before=${wordBeforePush} after=${wordAfterPush} title=${wordTitleAfterPush} wheel=${wheelAfterPush} requests=${requestsBeforePush}->${requestsAfterPush}`],
  ["a hint about a part nobody is looking at asks for nothing until it is opened",
    requestsAfterHints === requestsAfterPush && policyReadsAfterHints === 0
      && requestsAfterOpening === requestsAfterHints + 1,
    `requests=${requestsAfterPush}->${requestsAfterHints}->${requestsAfterOpening} policy=${policyReadsAfterHints}`],
  ["a policy hint leaves unsaved rule edits alone and says the rules changed elsewhere",
    readsAfterCleanHint === readsOnOpening + 1 && readsAfterDirtyHint === readsAfterCleanHint
      && draftStillDirty && draftAfterHint === draftBeforeHint && ruleListAfterHint === ruleListBeforeHint
      && editorLineAfterHint === sandbox.globalThis.GhostlightWords.POLICY_CHANGED_ELSEWHERE
      && readsAfterDiscard === readsAfterDirtyHint + 1 && draftCleanAfterDiscard,
    `reads=${readsOnOpening}/${readsAfterCleanHint}/${readsAfterDirtyHint}/${readsAfterDiscard} dirty=${draftStillDirty} line=${editorLineAfterHint}`],
  ["Clear view hides completed work, keeps running work, and calls nothing",
    clearEnabled && queueBeforeClear === "2 actions" && queueAfterClear === "1 action"
      && queueAfterResync === "1 action" && clearSpent && invokedByClear.length === 0,
    `before=${queueBeforeClear} after=${queueAfterClear} resync=${queueAfterResync} invoked=${JSON.stringify(invokedByClear)}`],
  ["audit health stays visible through recovery gaps and quiet when healthy", auditNoticeVisible && recoveredGapVisible && healthyAuditQuiet],
  ["audit policy authors a closed choice and organization requirement disables relaxation", authoredAudit.some((item) => item.key === "audit.availability" && item.value === "require_audit") && /value="keep_working"[^>]*disabled/.test(auditPinnedMarkup)],
  ...historyChecks,
  ...provenanceChecks,
  ...glanceChecks,
  ["boot completed without throwing", bootThrew === null, bootThrew],
  ["surface wired", listeners.some((l) => l.startsWith("document:click"))],
  ["global error handlers attached",
    listeners.includes("window:error") && listeners.includes("window:unhandledrejection")],
  ["snapshot still fetched", snapshots > 0],
  ["the pass continued past the broken panel", connections.innerHTML.length > 0,
    `connections: ${JSON.stringify(connections.innerHTML)}`],
  ["an old owned agent entry is offered as an update, in the pane the page opens on",
    bootPane.includes(">Update</button>") && bootPane.includes('data-harness-action="install"')
      && bootRail.includes('data-agent="codex"') && bootRail.includes("Needs an update"),
    `pane: ${JSON.stringify(bootPane)}`],
  ["one setup-everything action reaches the aggregate orchestrator command",
    setupEverythingWasActionable && bulkSetups === 1
      && listeners.includes("setup-integrations:click"),
    `actionable=${setupEverythingWasActionable} calls=${bulkSetups}`],
  ["a failed automatic setup shows that agent, with the entry to paste by hand",
    paneAfterFailure.includes('id="agents-pane-title">Qwen Code</h2>') && paneAfterFailure.includes("qwen setup"),
    JSON.stringify(paneAfterFailure)],
  ["the rail groups agents by state and folds the ones not on this computer into one closed row",
    JSON.stringify(railHeads(agentsShown.rail)) === JSON.stringify(["Needs a look 3", "Connected 2", "Found, not connected 1", "Found, set up by hand 1"])
      && JSON.stringify(railNames(agentsShown.rail).slice(0, 7))
        === JSON.stringify(["junie", "kilo-code", "cursor", "zed", "windsurf", "claude-code", "goose"])
      && agentsShown.rail.includes("<b>4</b> more agents Ghostlight can join")
      && agentsShown.rail.includes('data-agent-fold aria-expanded="false"')
      && agentsShown.rail.includes('id="agents-absent" hidden>'),
    agentsShown.rail],
  ["the fold opens in place, and picking an agent inside it keeps it open",
    railOpened.includes('data-agent-fold aria-expanded="true"') && railOpened.includes('id="agents-absent">')
      && railWithAbsentPicked.includes('data-agent-fold aria-expanded="true"'),
    railOpened],
  ["the rail's search reaches a folded agent and says so when nothing matches",
    JSON.stringify(railHeads(railSearched)) === JSON.stringify(["Not on this computer 1"])
      && JSON.stringify(railNames(railSearched)) === JSON.stringify(["zcode"])
      && !railSearched.includes("data-agent-fold")
      && railNoMatch.includes("No agent called &quot;nothing like it&quot;."),
    railSearched + railNoMatch],
  ["a rail entry carries a second line only for what is its own",
    agentsShown.rail.includes('<span class="agent-entry-sub live">2 sessions, 3 tabs, working</span>')
      && agentsShown.rail.includes('<span class="agent-entry-sub">Copy setup only</span>')
      && agentsShown.rail.includes('<span class="agent-entry-sub offer">Needs an update</span>')
      && agentsShown.rail.includes('<span class="agent-entry-sub warn">Needs your attention</span>')
      && agentsShown.rail.includes('<span class="agent-dot d-live" aria-hidden="true"></span>')
      && agentsShown.rail.includes('<span class="sr-only">, Connected</span>'),
    agentsShown.rail],
  ["the page opens on the agent that needs attention, with its evidence and Fix",
    agentsShown.opening.includes('id="agents-pane-title">Junie</h2>')
      && agentsShown.rail.includes('data-agent="junie" aria-controls="agents-pane" aria-current="true"')
      && agentsShown.opening.includes("What Ghostlight found") && agentsShown.opening.includes('<code class="agent-code">alien-agent</code>') && !agentsShown.opening.includes("`")
      && agentsShown.opening.includes(">Fix</button>") && agentsShown.opening.includes("Backs up settings.json"),
    agentsShown.opening],
  ["an agent that needs attention and cannot be fixed gets numbered steps",
    agentPanes["kilo-code"].includes('<ol class="agent-steps" aria-label="How to fix it">')
      && agentPanes["kilo-code"].includes("Replace what Ghostlight found with the entry below")
      && agentPanes["kilo-code"].includes("data-agents-recheck") && !agentPanes["kilo-code"].includes(">Fix</button>"),
    agentPanes["kilo-code"]],
  ["each state's pane says one sentence for its agent and the step it allows, with what it does",
    agentPanes.cursor.includes(">Update</button>") && agentPanes.cursor.includes("an older Ghostlight wrote it")
      && agentPanes["claude-code"].includes(">Connect</button>")
      && agentPanes["claude-code"].includes("Claude Code is installed on this computer but does not know about Ghostlight yet.")
      && agentPanes.windsurf.includes("Ghostlight&#039;s entry is in Windsurf&#039;s settings.json, so Windsurf can use your browser.")
      && !agentPanes.windsurf.includes("agent-live")
      && agentPanes.antigravity.includes(">Install</button>") && agentPanes.antigravity.includes(">Prepare</button>")
      && agentPanes.antigravity.includes("You can install it, or prepare its settings first."),
    JSON.stringify(agentPanes)],
  ["every agent's image is drawn the same way on the rail and in the pane; an app's icon keeps its tile and says once where it came from, and a monogram says nothing",
    agentsShown.rail.includes('<span class="symbol symbol-rail" aria-hidden="true"><img data-icon="agent-icons/windsurf?v=0123456789ab" alt="" width="22" height="22"></span>')
      && agentsShown.rail.includes('<span class="symbol symbol-rail" aria-hidden="true"><img data-icon="agent-icons/zed?v=fedcba987654" alt="" width="22" height="22"></span>')
      && !agentsShown.rail.includes(" src=") && !agentsShown.rail.includes("symbol-icon")
      && agentPanes.windsurf.includes('<span class="symbol symbol-pane" aria-hidden="true"><img data-icon="agent-icons/windsurf?v=0123456789ab" alt="" width="46" height="46"></span>')
      && (agentPanes.windsurf.match(/<p class="agent-icon-about">Icon from the installed Windsurf app\.<\/p>/g) ?? []).length === 1
      && !agentPanes.zed.includes("agent-icon-about") && !agentPanes.cursor.includes("agent-icon-about"),
    agentsShown.rail + agentPanes.windsurf],
  ["an agent in use says its sessions and tabs, links to At a glance, and pulses on its wire",
    agentPanes.zed.includes("2 Zed sessions are connected right now, with 3 browser tabs open.")
      && agentPanes.zed.includes('data-view="monitor" data-show-agent="zed"')
      && agentPanes.zed.includes('class="agent-hero s-live"'),
    agentPanes.zed],
  ["a copy-only agent is connected by pasting, claims nothing about its file, and is never offered Connect or Prepare",
    agentPanes.goose.includes("Ghostlight never edits settings.json.")
      && agentPanes.goose.includes("so it cannot tell whether goose has Ghostlight&#039;s entry")
      && agentPanes.goose.includes(">Found, set up by hand</span>") && !agentPanes.goose.includes("not connected")
      && agentPanes.goose.includes('<button class="primary-button" type="button" data-harness-operation="copy"')
      && !agentPanes.goose.includes(">Connect</button>")
      && agentPanes.continue.includes(">Install</button>") && !agentPanes.continue.includes(">Prepare</button>"),
    agentPanes.goose + agentPanes.continue],
  ["Remove is a quiet link last in the pane, and it asks first naming the live sessions",
    agentPanes.zed.includes('<button class="agent-remove" type="button" data-harness-operation="manage" data-harness-action="uninstall"')
      && agentPanes.zed.includes('data-harness-sessions="2"') && agentPanes.zed.includes(">Remove from Zed</button>")
      && agentPanes.zed.trimEnd().endsWith("Remove from Zed</button></div></div>")
      && removalAsked.shown && removalAsked.title === "Remove Ghostlight from Zed?"
      && removalAsked.detail.includes("2 Zed sessions are using Ghostlight right now.")
      && removalAsked.detail.includes("settings.json") && removalAsked.action === "Remove" && removalSentNothing,
    JSON.stringify(removalAsked)],
  ["Fix confirms the backup before dispatching the closed action",
    fixConfirmationShown && fixWasManaged,
    `shown=${fixConfirmationShown} calls=${JSON.stringify(managedHarnesses)}`],
  ["the headline counts the agents with Ghostlight, and Set up everything says beforehand what it will do",
    agentsShown.heading === "Ghostlight is in <em>2 of the 6</em> agents it can check."
      && agentsShown.caption === "Connects Claude Code and updates Cursor. goose, Junie and Kilo Code need you."
      && agentsShown.setupTitle === agentsShown.caption,
    JSON.stringify(agentsShown.heading + " | " + agentsShown.caption)],
  ["the page offers one Copy MCP command, in its header, with the command shown",
    agentsShown.command === AGENT_COMMAND && agentsShown.copyCommand.hidden === false
      && agentsShown.copyCommand.harness === "zcode"
      && !(everyPane + agentsShown.rail).includes("Copy MCP command"),
    JSON.stringify(agentsShown.copyCommand)],
  ["the front door renders the orchestrator's readiness answer and never authors one",
    nodes.get("state-word").textContent === "Ready"
      && nodes.get("state-word").title === "Connected and idle. Agents can work when they ask."
      && sandbox.document.body.className === "runtime-quiet",
    `readiness: ${JSON.stringify({ word: nodes.get("state-word").textContent, tone: sandbox.document.body.className })}`],
  ["the band says the readiness word it is handed, and none when it is handed none",
    (() => {
      // Every word belongs to crates/orchestrator/src/language/readiness.rs. A word the window
      // made up would be a second source of truth for the one answer the front door gives.
      const bare = { sessions: [], browsers: [], history: [] };
      const told = { ...bare, readiness: { state: "ready", word: "Lantern lit", detail: "Said by the service.", tone: "quiet", invites_control: true } };
      view.band({ connected: true, snapshot: told, runtime: "active", running: 0 });
      const handed = nodes.get("state-word").textContent;
      view.band({ connected: true, snapshot: bare, runtime: "active", running: 0 });
      const none = nodes.get("state-word").textContent;
      return handed === "Lantern lit" && none === "";
    })(),
    "the band authors a readiness word"],
  ["control follows the projection rather than a locally derived connection",
    (() => {
      const wheel = nodes.get("wheel");
      const readiness = { state: "ready", word: "Ready", detail: "Connected.", tone: "quiet" };
      const snapshot = (invites) => ({ sessions: [], browsers: [], history: [], readiness: { ...readiness, ...invites } });
      view.band({ connected: true, snapshot: snapshot({}), runtime: "active", running: 0 });
      const withheld = wheel.disabled;
      view.band({ connected: true, snapshot: snapshot({ invites_control: true }), runtime: "active", running: 0 });
      const invited = !wheel.disabled;
      return withheld && invited;
    })(),
    "the wheel derives its own availability"],
  ["Pause and Resume stay available while the authority answers without a browser",
    (() => {
      // The orchestrator offers runtime control whenever it answers, because a pause is its own
      // state rather than something a browser holds. Only an unreachable authority disables it.
      const wheel = nodes.get("wheel");
      const notConnected = {
        sessions: [], browsers: [], history: [],
        readiness: { state: "not_connected", word: "Not connected", detail: "No browser.", tone: "offline", invites_control: true }
      };
      view.band({ connected: true, snapshot: notConnected, runtime: "active", running: 0 });
      const pause = { disabled: wheel.disabled, intent: wheel.dataset.intent };
      view.band({ connected: true, snapshot: notConnected, runtime: "held", running: 0 });
      const resume = { disabled: wheel.disabled, intent: wheel.dataset.intent };
      // A failed resync marks the window disconnected but keeps the last snapshot, whose readiness
      // still invites control. The lost connection must win over that stale answer.
      view.band({ connected: false, snapshot: notConnected, runtime: "held", running: 0 });
      const lost = wheel.disabled;
      view.band({ connected: false, snapshot: null, runtime: "held", running: 0 });
      const neverReached = wheel.disabled;
      view.band({ connected: true, snapshot: notConnected, runtime: "held", running: 0 });
      const regained = !wheel.disabled;
      return !pause.disabled && pause.intent === "hold"
        && !resume.disabled && resume.intent === "resume"
        && lost && neverReached && regained;
    })(),
    "the wheel follows a lost authority, or is disabled while a reachable authority offers control"],
  ["the policy tab carries the authored state behind it",
    policy.dataset.tone === "applied"
      && policy.title === "Example Org sets the rules, and you have narrowed them further.",
    `policy: ${JSON.stringify({ tone: policy.dataset.tone, title: policy.title })}`],
  ["a refusal names the rule, the handle, and who to ask",
    refusal.includes("Example Org") && refusal.includes("rule support-sites")
      && refusal.includes("D-a1b2c3") && refusal.includes("security@example.test")
      && refusal.includes('data-view="policy"'),
    `refusal: ${JSON.stringify(refusal)}`],
  ["a never-settled row is marked for finding by eye, and informational rows stay neutral",
    wordsApi.readinessNeedsAttention({ settled: true, observed: { readiness: "loading" } }) === true
      && wordsApi.readinessNeedsAttention({ settled: true, observed: { readiness: "unknown" } }) === true
      && wordsApi.readinessNeedsAttention({ settled: true, observed: { readiness: "interactive" } }) === false
      && wordsApi.readinessNeedsAttention({ settled: true, observed: { readiness: "complete" } }) === false
      && wordsApi.readinessNeedsAttention({ settled: false, observed: { readiness: "loading" } }) === false
      && loadingDuration === '<div class="row-dur unsettled">'
      && interactiveDuration === '<div class="row-dur">',
    `duration cells: ${JSON.stringify({ loading: loadingDuration, interactive: interactiveDuration })}`],
  ["the policy destination opens with the orchestrator's sentence",
    nodes.get("policy-headline").textContent
      === "Example Org sets the rules, and you have narrowed them further.",
    nodes.get("policy-headline").textContent],
  ["every capability line states its polarity and its decider",
    board.includes("Some sites allowed") && board.includes("Not allowed")
      && board.includes("Allowed")
      && board.includes("Example Org does not allow it anywhere"),
    `board: ${JSON.stringify(board)}`],
  ["the permanent boundaries are shown, Ghostlight's own control page among them",
    nodes.get("policy-ceilings").innerHTML.includes("not an ordinary http or https address")
      && nodes.get("policy-ceilings").innerHTML.includes("own control page on this computer is never automated")
      && !nodes.get("policy-ceilings").innerHTML.includes("localhost")],
  ["the organization is named in its own words, with somewhere to ask",
    organization.includes("Example Org")
      && organization.includes("Ask the service desk")
      && organization.includes("security@example.test"),
    `organization: ${JSON.stringify(organization)}`],
  ["every rule is one line, organization first, each naming whose it is",
    rules.indexOf("rule-theirs") < rules.indexOf("rule-mine")
      && rules.includes("On <b>support.example.test</b>, agents may look at pages.")
      && rules.includes(">Example Org</span>")
      && rules.includes(">Edit</span>"),
    `rules: ${JSON.stringify(rules)}`],
  ["a closed rule shows no detail pane until it is opened",
    !rules.includes("rule-detail")
      && rules.includes('aria-expanded="false"'),
    `rules: ${JSON.stringify(rules)}`],
  ["opening a rule reveals its detail, and closing puts it away",
    openedDetail.includes("rule-detail") && openedDetail.includes("Ordinary support work")
      && !closedAgain.includes("rule-detail"),
    `opened: ${JSON.stringify(openedDetail)}`],
  ["both layers keep their exact document and path",
    documents.includes("Example Org") && documents.includes("Your rules")
      && documents.includes("state/user-policy.json"),
    `documents: ${JSON.stringify(documents)}`],
  ["permissions are grouped by what a person thinks about, not by registered key",
    permissionsOnLoad.includes("In the browser")
      && permissionsOnLoad.includes("Privacy")
      && permissionsOnLoad.includes("Screenshots")
      && !permissionsOnLoad.includes("browser.screenshots.enabled</span>"),
    `permissions: ${JSON.stringify(permissionsOnLoad)}`],
  ["a permission on by default renders checked, and a person's own restriction renders unchecked",
    /data-restriction="browser\.screenshots\.enabled"\s+checked/.test(permissionsOnLoad)
      && /data-restriction="browser\.tabs\.allow_close"(?!\s+checked)/.test(permissionsOnLoad),
    `permissions: ${JSON.stringify(permissionsOnLoad)}`],
  ["an organization ceiling disables the switch and names who set it",
    permissionsOnLoad.includes("Example Org already turned this off.")
      && /data-restriction="privacy\.preserve_target_names"[^>]*disabled/.test(permissionsOnLoad),
    `permissions: ${JSON.stringify(permissionsOnLoad)}`],
  ["an authored restriction is read back into the draft",
    authoredOnLoad.length === 1
      && authoredOnLoad[0].key === "browser.tabs.allow_close"
      && authoredOnLoad[0].value === false
      && authoredOnLoad[0].level === "mandatory",
    `authored: ${JSON.stringify(authoredOnLoad)}`],
  ["browser startup is a closed choice that authors one string value",
    permissionsOnLoad.includes('data-setting-choice="browser.startup"')
      && permissionsOnLoad.includes('value="on_demand"')
      && permissionsOnLoad.includes('value="manual"')
      && authoredAfterStartup.some((entry) =>
        entry.key === "browser.startup" && entry.value === "manual"),
    `permissions: ${JSON.stringify(permissionsOnLoad)}, authored: ${JSON.stringify(authoredAfterStartup)}`],
  ["an organization manual ceiling pins and explains browser startup",
    /data-setting-choice="browser\.startup"[^>]*disabled/.test(pinnedStartupControl)
      && pinnedStartupControl.includes("Example Org requires browser auto-open to stay off."),
    `permissions: ${JSON.stringify(pinnedStartupControl)}`],
  ["turning a permission off authors only the tightening value",
    authoredAfterEdit.some((entry) => entry.key === "browser.screenshots.enabled" && entry.value === false)
      && authoredAfterEdit.some((entry) =>
        entry.key === "content.security.sacred_domains"
        && JSON.stringify(entry.value) === '["vault.example.test","*.finance.example.test"]'),
    `authored: ${JSON.stringify(authoredAfterEdit)}`],
  ["turning one back on removes it rather than authoring permission",
    !authoredAfterClearing.some((entry) => entry.key === "browser.tabs.allow_close"),
    `authored: ${JSON.stringify(authoredAfterClearing)}`],
  ["a restriction in force reads as a sentence, not a key",
    settingsShown.includes("Closing a tab stays something only you do."),
    `settings: ${JSON.stringify(settingsShown)}`],
  ["both channel switches render, on unless a layer turned them off, with the layer that did",
    permissionsOnLoad.includes("Who may work")
      && /data-restriction="channels\.mcp\.enabled"\s+checked/.test(permissionsOnLoad)
      && /data-restriction="channels\.cli\.enabled"[^>]*disabled/.test(permissionsOnLoad)
      && settingsShown.includes("Work from the command line is refused.")
      && /Work from the command line is refused\. <span class="owner">Example Org<\/span>/.test(settingsShown),
    `permissions: ${JSON.stringify(permissionsOnLoad)}, settings: ${JSON.stringify(settingsShown)}`],
  ["a channel switch states its honest limit wherever it is shown",
    permissionsOnLoad.includes("another program running as you could claim otherwise")
      && settingsShown.includes("another program running as you could claim otherwise")
      && settingsShown.split("another program running as you").length === 2,
    `permissions: ${JSON.stringify(permissionsOnLoad)}, settings: ${JSON.stringify(settingsShown)}`],
  ["the user layer turns a channel off by authoring its tightening value",
    authoredChannelOff.some((entry) =>
      entry.key === "channels.mcp.enabled" && entry.value === false && entry.level === "mandatory")
      && !/data-restriction="channels\.mcp\.enabled"\s+checked/.test(channelSwitchesOff)
      && channelSwitchesOff.includes("Work from MCP agents is refused."),
    `authored: ${JSON.stringify(authoredChannelOff)}`],
  ["the editor appears only when authoring is permitted",
    editorShown && editorHiddenWhenRefused],
  ["Re-check re-enables its button after currentTarget has expired, and never throws",
    reCheckThrew === null && reCheckButton.disabled === false,
    `threw: ${JSON.stringify(reCheckThrew)}, disabled: ${reCheckButton.disabled}`],
  ["failure reported", reported.some((r) => r.includes("deliberate About failure")),
    `reported: ${JSON.stringify(reported)}`],
  ["the failure names the panel", reported.some((r) => r.includes("painting about"))],
  ["the broken panel was attempted, not skipped", before > 0, `attempts: ${before}`]
];

// Quit from the window reaches the service's quit route, and the band then says so. It runs
// last, since the band stops vouching for anything afterwards.
const nativeClick = (selector, element) => dispatchDocument("click", {
  target: { closest: (asked) => (asked === selector ? element : asked === "#view-settings" ? nodes.get("view-settings") : null) }
});
nativeClick("[data-quit]", { dataset: { quit: "ask" }, disabled: false });
nativeClick("[data-quit]", { dataset: { quit: "yes" }, disabled: false });
await new Promise((r) => setTimeout(r, 0));
await new Promise((r) => setTimeout(r, 0));
const nativeQuit = {
  commands: invoked.filter((cmd) => cmd === "quit_ghostlight").length,
  word: nodes.get("state-word").textContent
};
checks.push(["Quit in the window is the service's quit route, and the band then says Ghostlight is quitting",
  nativeQuit.commands === 1 && nativeQuit.word === QUITTING_SENTENCE, JSON.stringify(nativeQuit)]);

// The window's setup view runs against a document that can hold a tree, in its own module.
checks.push(...await setupChecks());

let ok = true;
for (const [name, pass, detail] of checks) {
  if (!pass) ok = false;
  console.log(`${pass ? "PASS" : "FAIL"}  ${name}${pass || !detail ? "" : `\n        ${detail}`}`);
}
if (!ok) process.exit(1);
console.log("");
console.log("workbench surface ok: a broken panel costs its own panel and nothing else");
