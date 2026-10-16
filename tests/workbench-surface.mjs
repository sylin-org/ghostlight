// The workbench surface, actually executed.
//
// Every other guard over this window reads its source as text, which can only ever check that the
// right strings are present. None of them could tell that the window did not start: one missing
// element id threw at module scope and silently abandoned the snapshot, the change subscription
// and the heartbeat behind it, and the guards stayed green.
//
// This runs the real modules against a minimal DOM with one panel deliberately broken, and
// asserts what a person would check by looking: the window still comes up, the failure is
// visible, the rest of the pass continues, and the broken panel is retried rather than
// remembered as finished.
//
// Run with: node tests/workbench-surface.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import { join, resolve } from "node:path";

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
const failedSetupManual = { open: false };
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
const confirmAccept = node("confirm-accept");
confirmAccept.dataset.confirm = "accept";
confirmAccept.closest = (selector) => selector === "[data-confirm]" ? confirmAccept : null;
const confirmCancel = node("confirm-cancel");
nodes.get("confirm-dialog").querySelector = (selector) =>
  selector === '[data-confirm="accept"]' ? confirmAccept
    : selector === '[data-confirm="cancel"]' ? confirmCancel : null;

const reported = [];
let heartbeat = false;

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
      word: watch ? "On -- anyone on this computer can open this view" : "Off -- only this window shows Ghostlight",
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
      label: "Notifications", word: "Send one local checkup notification. Nothing leaves this device.",
      acts: { test_notification: act() }
    },
    service: { word: "Ghostlight service is up; 1 MCP client and 1 browser connected." },
    you: {
      context, word: control ? "Full control" : "Watching",
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
  context: "control", word: "Full control",
  sentence: "Others on this computer can watch this view in a browser.",
  instance: "instance_1", opener: "service",
  acts: Object.fromEntries(["search", "check_policy", "rescan", "continue_in_window", "hold", "resume",
    "start_session", "end_session", "apply_policy", "remove_policy", "set_diagnostics", "set_watching",
    "setup_all", "harness_act", "test_notification", "open", "reveal_diagnostics", "locate", "quit"]
    .map((act) => [act, { available: true, why_not: null }])),
  product: { version: "1.0.0", license: "Apache-2.0 OR MIT" },
  links: {}
};
const managedHarnesses = [];
let pushChange = null;
let policyReply = null;

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
    id: "codex", product_id: "codex", name: "Codex", target: "User", icon: "codex.svg", state: "updatable",
    detail: "The connector path belongs to an older installation.",
    can_install: true, can_fix: false, can_uninstall: false, can_download: true, can_locate: true,
    config_path: "/home/test/.codex/config.toml", connector_command: "/opt/ghostlight/ghostlight mcp",
    manual_setup: "[mcp_servers.ghostlight]"
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
    // The tab's state is authored by the orchestrator now. The surface must render exactly what
    // it is handed, so this fixture carries a sentence no local computation could have produced.
    policy: {
      situation: "layered",
      detail: "Example Org sets the rules, and you have narrowed them further.",
      tone: "applied"
    }
  },
  controls: controlsPart(),
  overview: {}
});

const sandbox = {
  console: { error: (detail) => reported.push(String(detail)), log() {}, warn() {} },
  document: {
    getElementById: (id) => nodes.get(id) ?? null,
    querySelectorAll: (sel) => (sel === "[id]" ? [...nodes.values()] : []),
    querySelector: (sel) => sel === '[data-harness-manual="qwen-code"]' ? failedSetupManual : null,
    addEventListener: (kind, handler) => {
      listeners.push(`document:${kind}`);
      if (!documentHandlers.has(kind)) documentHandlers.set(kind, []);
      documentHandlers.get(kind).push(handler);
    },
    removeEventListener() {},
    hidden: false, body: node("body"),
    createElement: () => node("x"), createDocumentFragment: () => node("f")
  },
  window: {
    addEventListener: (kind) => listeners.push(`window:${kind}`),
    location: nativeBar,
    history: nativeBar.history,
    __TAURI__: {
      core: { invoke: async (cmd, args) => {
        invoked.push(cmd);
        if (cmd === "workbench_context") return controlContext;
        if (cmd === "set_diagnostics") return { layer: args.on ? "marker" : "off", directory: null, used_bytes: 0 };
        if (cmd === "continue_in_window") return null;
        if (cmd === "quit_ghostlight") return { message: QUITTING_SENTENCE };
        if (cmd === "workbench_snapshot") return snapshot();
        if (cmd === "workbench_policy" && policyReply) return policyReply();
        if (cmd === "setup_detected_harnesses") {
          bulkSetups += 1;
          return { set_up: 1, updated: 1, needs_attention: 0, failures: [], message: "Set up everything." };
        }
        if (cmd === "manage_harness") {
          managedHarnesses.push(args);
          if (args.action === "fix") return { changed: true, message: "Fixed Cline." };
          throw new Error("deliberate setup failure");
        }
        return [];
      } },
      event: { listen: async (_name, handler) => { pushChange = (change) => handler({ payload: change }); return () => {}; } }
    }
  },
  setTimeout: (fn, ms) => { if (typeof fn === "function" && !ms) fn(); return 0; },
  clearTimeout() {}, clearInterval() {},
  setInterval: () => { heartbeat = true; return 0; },
  Date, JSON, Math, String, Number, Boolean, Array, Object, Set, Map, Promise, Error,
  isNaN, parseInt, parseFloat, performance: { now: () => 0 }, crypto: { randomUUID: () => "x" }
};
sandbox.globalThis = sandbox;
sandbox.window.document = sandbox.document;
vm.createContext(sandbox);

// Break exactly one panel, every time it is asked to render, and count the attempts. A panel that
// is retried is attempted again on the next snapshot; one memoised as finished never is.
const sources = ORDER.map((name) => {
  let text = readFileSync(join(ui, name), "utf8");
  if (name.endsWith("view.js")) {
    text = text.replace(
      "    function about({ version, license, counts }) {",
      "    function about({ version, license, counts }) { globalThis.__aboutAttempts = (globalThis.__aboutAttempts || 0) + 1;"
        + " throw new Error('deliberate About failure');"
    );
  }
  return text;
});
if (!sources.some((text) => text.includes("deliberate About failure"))) {
  console.log("FAIL  the harness could not break a panel; its injection point moved");
  process.exit(1);
}

let bootThrew = null;
try {
  vm.runInContext(sources.join("\n"), sandbox);
} catch (error) {
  bootThrew = error.message;
}

await new Promise((r) => setTimeout(r, 60));
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
// A second snapshot: whatever the first pass did with the broken panel, this is where a memo
// would show itself by never trying again.
sandbox.globalThis.__second = true;
const before = sandbox.__aboutAttempts ?? 0;
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
pushChange(statusPush(seqBeforePush + 1, "held", "Paused from the tray."));
const wordAfterPush = nodes.get("state-word").textContent;
const wordTitleAfterPush = nodes.get("state-word").title;
const wheelAfterPush = nodes.get("wheel").dataset.intent;
const requestsAfterPush = snapshotRequests();
pushChange(statusPush(seqBeforePush + 2, "active", null));
const titleAfterResume = nodes.get("state-word").title;
pushChange({ seq: seqBeforePush + 3, change: { kind: "part_changed", part: "integrations", etag: "tag-1" } });
pushChange({ seq: seqBeforePush + 4, change: { kind: "part_changed", part: "policy", etag: "tag-2" } });
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
const integrationsHtml = nodes.get("integration-grid").innerHTML;
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

view.collections({
  sessions: [], browsers: [], diagnostics: [], history: [], service: { version: "1.0.0" },
  configuration: { managed_policy: { configured: false } },
  // Deliberately shuffled across every raw target state. Product cards must classify their mixed
  // targets first, then render the four semantic groups in product-owned order with names sorted
  // inside each group. Neither registry order nor the order of targets in this fixture may leak.
  harnesses: [
    { id: "qwen-code", product_id: "qwen-code", name: "Qwen Code", target: "CLI",
      icon: "qwen-code.svg", state: "not_detected", detail: "Not detected.", can_install: true,
      can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/qwen.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "qwen setup" },
    { id: "windsurf", product_id: "windsurf", name: "Windsurf", target: "User",
      icon: "windsurf.svg", state: "available", detail: "Detected.", can_install: true,
      can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/windsurf.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "windsurf setup" },
    { id: "cline-vscode", product_id: "cline", name: "Cline", target: "Visual Studio Code",
      icon: "cline.svg", state: "available", detail: "Detected.", can_install: true,
      can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/cline-vscode.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "editor setup" },
    { id: "zed", product_id: "zed", name: "Zed", target: "User", icon: "zed.svg",
      state: "installed", detail: "Current.", can_install: false, can_uninstall: true,
      can_download: true, can_locate: true, config_path: "/tmp/zed.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "zed setup" },
    { id: "junie-cli", product_id: "junie", name: "Junie", target: "CLI", icon: "junie.svg",
      state: "available", detail: "Detected.", can_install: true, can_uninstall: false,
      can_download: true, can_locate: true, config_path: "/tmp/junie.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "junie setup" },
    { id: "antigravity", product_id: "antigravity", name: "Antigravity", target: "CLI",
      icon: "antigravity.svg", state: "not_detected", detail: "Not detected.", can_install: true,
      can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/antigravity.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "antigravity setup" },
    { id: "codex", product_id: "codex", name: "Codex", target: "User", icon: "codex.svg",
      state: "updatable", detail: "Old owned connector.", can_install: true, can_uninstall: false,
      can_download: true, can_locate: true, config_path: "/tmp/codex.toml",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "codex setup" },
    { id: "cline-cli", product_id: "cline", name: "Cline", target: "CLI", icon: "cline.svg",
      state: "installed", detail: "Current.", can_install: false, can_uninstall: true,
      can_download: true, can_locate: true, config_path: "/tmp/cline-cli.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "cli setup" },
    { id: "claude-code", product_id: "claude-code", name: "Claude Code", target: "User",
      icon: "claude-code.svg", state: "available", detail: "Detected.", can_install: true,
      can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/claude.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "claude setup" },
    { id: "junie-jetbrains", product_id: "junie", name: "Junie", target: "JetBrains",
      icon: "junie.svg", state: "needs_attention", detail: "Foreign entry preserved.", can_install: false,
      can_fix: true, can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/junie.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "junie setup",
      evidence: "Found another program, `alien-agent` with 1 argument, where Ghostlight's entry belongs; Ghostlight maintains `/opt/ghostlight/ghostlight` there and changed nothing." },
    { id: "cline-cursor", product_id: "cline", name: "Cline", target: "Cursor",
      icon: "cline.svg", state: "needs_attention", detail: "Foreign entry preserved.", can_install: false,
      can_fix: false, can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/cline-cursor.json",
      connector_command: "/opt/ghostlight/ghostlight mcp", manual_setup: "cursor setup" }
  ]
}, new Set());
const rosterHtml = nodes.get("integration-grid").innerHTML;
const rosterCards = [...rosterHtml.matchAll(
  /<article class="([^"]*\bintegration-card\b[^"]*)">([\s\S]*?)<\/article>/g
)].map(([, classes, html]) => ({
  classes,
  html,
  name: html.match(/<h3>([^<]+)<\/h3>/)?.[1] ?? ""
}));
const rosterCard = (name) => rosterCards.find((card) => card.name === name);

const fixButton = {
  disabled: false,
  dataset: {
    harness: "cline-cli", harnessOperation: "manage", harnessAction: "fix",
    harnessName: "Cline", harnessTarget: "CLI"
  }
};
dispatchDocument("click", {
  target: {
    closest: (selector) => selector === "[data-harness-operation]" ? fixButton : null
  }
});
await new Promise((r) => setTimeout(r, 0));
const fixConfirmationShown = !nodes.get("confirm-dialog").hidden
  && nodes.get("confirm-title").textContent.includes("Cline")
  && nodes.get("confirm-title").textContent.includes("CLI")
  && nodes.get("confirm-detail").textContent.includes("backed up")
  && confirmAccept.textContent === "Fix";
dispatchDocument("click", { target: confirmAccept });
await new Promise((r) => setTimeout(r, 0));
const fixWasManaged = managedHarnesses.some((call) =>
  call.id === "cline-cli" && call.action === "fix");

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
view.hero({
  invocation: "legacy", workspace: "w1", tool: "browser_execute", capability: "execute",
  settled: true, allowed: false, phase: "blocked", reason: "host_denied",
  policyTier: "session", denialId: "D-legacy", summary: "Blocked by this call's restrict_hosts.",
  permissionExplanations: ["Refused: request restrictions refused this work."], endedAt: Date.now(), durationMs: 0
}, false);
const legacyRefusal = nodes.get("hero-body").innerHTML;
assert.ok(legacyRefusal.includes("request restrictions refused this work"));
assert.ok(!legacyRefusal.includes("your own rules"));
assert.ok(!legacyRefusal.includes("See the policy"));


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
pushChange({
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
  chipTitle: sheetNode("context-chip").title,
  snapshots: snapshotRequests() - snapshotsBeforeControls
};
pushChange({ seq: ++hintSeq, change: { kind: "part_changed", part: "policy", etag: "tag-clean" } });
await new Promise((r) => setTimeout(r, 0));
const readsAfterCleanHint = policyReads();
appDraft("view.addRule()");
const draftBeforeHint = appDraft("view.draftDocument()");
const ruleListBeforeHint = nodes.get("rule-list").innerHTML;
pushChange({ seq: ++hintSeq, change: { kind: "part_changed", part: "policy", etag: "tag-dirty" } });
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
 * A real browser nulls Event.currentTarget once dispatch finishes, which is at the first `await`
 * in an async listener -- well before an awaited call settles. `withButton` used to read
 * `event.currentTarget` a second time in its `finally` block, after that point, which threw on
 * every use of Re-check or Send test and left the button stuck disabled. The synthetic `node()`
 * stub above is a plain object and cannot reproduce that expiry, so this checks the one function
 * where it matters against an event shaped like the real one: valid once, synchronously, then gone.
 */
const reCheckButton = node("refresh-integrations");
let dispatchEnded = false;
const clickEvent = { get currentTarget() { return dispatchEnded ? null : reCheckButton; } };
queueMicrotask(() => { dispatchEnded = true; });
let withButtonThrew = null;
try {
  await sandbox.withButton(clickEvent, async () => {}, "done");
} catch (error) {
  withButtonThrew = error.message;
}

// The never-settled treatment: the predicate lives beside the readiness words, the row applies
// it on the duration cell, and the stylesheet carries the amber tone. The stub DOM cannot
// accumulate row markup, so the row and stylesheet halves are pinned against their sources --
// the same text-guard practice every other window guard uses.
const wordsApi = sandbox.globalThis.GhostlightWords;
const viewSource = readFileSync(join(ui, "lib", "view.js"), "utf8");
const stylesheet = readFileSync(join(ui, "styles.css"), "utf8");

// The ADR-0145 status card: rendered from the orchestrator-owned state, with both person acts.
view.collections({
  sessions: [], browsers: [],
  diagnostics: [{ severity: "passing", label: "Orchestrator", detail: "Ghostlight is accepting local connections." }],
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
    && store.feed().length === 2 && sandbox.globalThis.GhostlightEntries.isRunning(store.feed()[1])]);
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
    channel: "mcp", explanation: "The application name is the MCP client's own claim. Ghostlight does not verify it."
  };
  const laterSession = { id: "shared-workspace", client_label: "Later application", channel: "cli" };
  const connectionView = sandbox.globalThis.GhostlightView.create({ sessionFor: () => laterSession });
  const entries = sandbox.globalThis.GhostlightEntries;
  const operation = { invocation: "provenance-work", workspace: laterSession.id, tool: "browser_read",
    phase: "running", activity: "Reading", provenance: evidence };
  const record = { ...operation, complete: true, summary: "Read 5 words.", status: "succeeded",
    allowed: true, effect: "none", channel: "mcp", provenance: evidence };
  connectionView.hero(entries.entryFromOperation(operation), false);
  let html = nodes.get("hero-body").innerHTML;
  provenanceChecks.push(["live action attribution uses its own connection and escapes the claim",
    html.includes("Original &lt;script&gt;claim&lt;/script&gt;")
      && !html.includes("Later application") && !html.includes("<script>claim")]);
  const restored = { ...record, provenance: { ...evidence, reported_application: null } };
  connectionView.hero(entries.entryFromRecord(restored), false);
  html = nodes.get("hero-body").innerHTML;
  provenanceChecks.push(["restored history keeps its connection without borrowing a current claim",
    html.includes("Not retained in history") && html.includes("connection_original")
      && !html.includes("Later application") && html.includes("MCP client")]);
  connectionView.hero(entries.entryFromRecord({ ...record, provenance: null, channel: null }), false);
  html = nodes.get("hero-body").innerHTML;
  provenanceChecks.push(["legacy history names missing evidence without borrowing current-session identity",
    html.includes("Connection details were not recorded.") && !html.includes("Later application") && !html.includes("via cli")]);
  const group = { ...record, tool: "browser_flow", steps: [{ position: 1, state: "recorded", record }] };
  connectionView.hero(entries.entryFromRecord(group), false);
  html = nodes.get("hero-body").innerHTML;
  provenanceChecks.push(["parent and recorded child each expose collapsed connection evidence",
    html.includes('data-history-details="provenance-work:connection"')
      && html.includes('data-history-details="provenance-work:step:1:connection"')
      && !html.includes('data-history-details="provenance-work:connection" open')]);
  const sessions = [{ ...laterSession, tab_count: 2, active_operations: 0, connections: [evidence,
    { ...evidence, connection_id: "connection_other", reported_application: "Second application",
      channel: "cli", explanation: "A ghostlight call session is a local program." }] }];
  const sessionSnapshot = { ...snapshot(), sessions };
  connectionView.collections(sessionSnapshot, new Set());
  html = nodes.get("connections").innerHTML;
  provenanceChecks.push(["shared sessions retain every connection with its authored explanation",
    html.includes("Original &lt;script&gt;claim&lt;/script&gt;") && html.includes("Second application")
      && html.includes("A ghostlight call session is a local program.") && !html.includes("trusted")
      && !html.includes('data-history-details="sessions:connections" open')]);
  documentHandlers.get("toggle").at(-1)({ target: { dataset: { historyDetails: "sessions:connections" }, open: true,
    classList: { contains: () => false } } });
  connectionView.collections({ ...sessionSnapshot, sessions: [{ ...sessions[0], active_operations: 1 }] }, new Set());
  provenanceChecks.push(["connection details stay open across session refresh",
    nodes.get("connections").innerHTML.includes('data-history-details="sessions:connections" open')]);
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
// The same page with no native shell: over loopback HTTP, in the three contexts.
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
  "test_notification", "open", "reveal_diagnostics", "locate", "quit"];
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
    context: kind, word: control ? "Full control" : "Watching",
    sentence: control ? "Others on this computer can watch this view in a browser."
      : "Anyone on this computer can open this view. Changes are made in the Ghostlight window.",
    instance: "instance_1", opener: control ? "service" : "anchor",
    acts: Object.fromEntries(ACT_KEYS.map((act) => [act, control || WATCHER_ACTS.has(act)
      ? { available: true, why_not: null } : { available: false, why_not: CONTINUE_SENTENCE }])),
    product: { version: "2.0.0", license: "Apache-2.0 OR MIT" }, links: LINKS
  };
};

const roster = () => [
  { id: "codex", product_id: "codex", name: "Codex", target: "User", icon: "codex.svg", state: "updatable",
    detail: "Old owned connector.", can_install: true, can_fix: false, can_uninstall: false, can_download: true,
    can_locate: true, config_path: "~/.codex/config.toml", connector_command: "/opt/ghostlight/ghostlight mcp",
    manual_setup: "codex setup", download_url: "https://example.test/codex" },
  { id: "zed", product_id: "zed", name: "Zed", target: "User", icon: "zed.svg", state: "installed",
    detail: "Current.", can_install: false, can_fix: false, can_uninstall: true, can_download: true,
    can_locate: true, config_path: "~/.config/zed/settings.json", connector_command: "/opt/ghostlight/ghostlight mcp",
    manual_setup: "zed setup", download_url: "https://example.test/zed" },
  { id: "junie", product_id: "junie", name: "Junie", target: "CLI", icon: "junie.svg", state: "needs_attention",
    detail: "Foreign entry preserved.", can_install: false, can_fix: true, can_uninstall: false, can_download: true,
    can_locate: true, config_path: "~/.junie/mcp.json", connector_command: "/opt/ghostlight/ghostlight mcp",
    manual_setup: "junie setup", download_url: "https://example.test/junie" },
  { id: "qwen-code", product_id: "qwen-code", name: "Qwen Code", target: "CLI", icon: "qwen-code.svg",
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
    connections: { sessions: [], browsers: [] },
    activity: { operations: [], history: [], document_coverage: [] },
    integrations: { harnesses: roster() },
    diagnostics: {
      diagnostics: [{ severity: "passing", label: "Orchestrator", detail: "Accepting connections." }]
    },
    controls: controls()
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
      state.note = intent === "hold" ? "Paused from a screen that is watching." : null;
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

/** One page: the real modules over a stub document, with no native shell. */
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
    AbortController, TextDecoder, TextEncoder,
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
  roster: watching.text("integration-grid").innerHTML,
  rosterLine: watching.text("integrations-continue").innerHTML,
  rosterLineHidden: watching.text("integrations-continue").hidden,
  status: watching.text("diagnostic-grid").innerHTML,
  apply: watching.text("apply-policy").textContent
};
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
  roster: controlling.text("integration-grid").innerHTML,
  status: controlling.text("diagnostic-grid").innerHTML,
  apply: controlling.text("apply-policy").textContent
};
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
  toast: controlling.text("toast").textContent,
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
  ["over HTTP a watching page says so in its chip and keeps the service's sentence beside it",
    watchOpening.chip === "Watching" && watchOpening.chipTitle.includes("Changes are made in the Ghostlight window"),
    JSON.stringify(watchOpening.chip)],
  ["a watching page draws no control for a change it cannot make",
    watchOpening.setup === true && watchOpening.endSession === true && watchOpening.sendTest === true
      && watchOpening.sheetViewingToggle === true && watchOpening.sheetDiagnosticsToggle === true
      && watchOpening.sheetReveal === true
      && !/>Set up<|>Update<|>Remove<|>Fix<|>Locate<|Turn on|Turn off|Open log folder/.test(watchOpening.roster + watchOpening.status)
      && watchOpening.recheck === false,
    JSON.stringify({ ...watchOpening, roster: undefined, status: undefined })],
  ["a watching page keeps the state of every client as text and offers Copy and Install as plain controls",
    watchOpening.roster.includes("Old owned connector.") && watchOpening.roster.includes(">Copy setup<")
      && watchOpening.roster.includes(">Copy MCP command<")
      && watchOpening.roster.includes('<a class="ghost-button" href="https://example.test/qwen"')
      && !watchOpening.roster.includes('data-harness-operation="download"'),
    watchOpening.roster],
  ["a watching page shows one Continue line for the roster, one for each section of Settings and none on Status",
    watchOpening.rosterLineHidden === false && watchOpening.rosterLine.includes(CONTINUE_SENTENCE)
      && watchOpening.rosterLine.includes("data-continue") && watchOpening.rosterLine.includes("Continue in the Ghostlight window")
      && !watchOpening.status.includes("data-continue")
      && watchOpening.sheetContinue === false && watchOpening.sheetContinueWhy === CONTINUE_SENTENCE
      && watchOpening.sheetContinues.every(([hidden, why]) => hidden === false && why === CONTINUE_SENTENCE),
    watchOpening.rosterLine + watchOpening.status],
  ["Settings in a watching page shows Pause while agents run and the rest as state, with no Quit",
    watchOpening.sheetPause === false && watchOpening.quitHidden === true
      && watchOpening.sheetViewingWord === "On -- anyone on this computer can open this view"
      && JSON.stringify(watchOpening.rail) === JSON.stringify([
        "Comfortable, motion follows the system", "On -- anyone on this computer can open this view", "Running",
        "Send one local checkup notification. Nothing leaves this device.", "On (marker)"]),
    JSON.stringify(watchOpening)],
  ["Sharing says who can see a watching page, and Status carries the service's sentence at its top",
    watchOpening.who === SHEET_WATCHER
      && watchOpening.statusService === "Ghostlight service is up; 1 MCP client and 1 browser connected."
      && watchOpening.statusServiceHidden === false,
    JSON.stringify({ who: watchOpening.who, status: watchOpening.statusService })],
  ["a watching page draws the wheel while agents are running, and Apply as a way to the window",
    watchOpening.wheelHidden === false && watchOpening.bandContinue === true
      && watchOpening.apply === "Apply in the Ghostlight window",
    JSON.stringify({ wheel: watchOpening.wheelHidden, continue: watchOpening.bandContinue, apply: watchOpening.apply })],
  ["a pause from a watching page is one keyless POST and is shown by waiting for its change, not by asking again",
    holdRequest?.method === "POST" && holdRequest.body === '{"intent":"hold"}' && holdRequest.headers.Authorization === undefined
      && holdRequest.credentials === "omit" && holdRequest.cache === "no-store"
      && watchHeld.snapshotsAfter === watchBeforeHold.snapshots,
    JSON.stringify({ holdRequest, snapshots: [watchBeforeHold.snapshots, watchHeld.snapshotsAfter] })],
  ["while held, a watching page shows the state and its note where the wheel was, with the Continue line in the band",
    watchHeld.wheelHidden === true && watchHeld.state === false && watchHeld.word === "Paused"
      && watchHeld.note === "Paused from a screen that is watching." && watchHeld.continueHidden === false
      && watchHeld.continueMarkup.includes("data-continue"),
    JSON.stringify(watchHeld)],
  ["while held, the sheet of a watching page says Paused and where it came from, and offers no Resume",
    watchHeld.sheetRuntime === "Paused" && watchHeld.sheetNote === "Paused from a screen that is watching."
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
  ["a page with the key draws every control the native shell draws and no Continue line",
    controlOpening.chip === "Full control" && controlOpening.setup === false && controlOpening.endSession === false
      && controlOpening.sendTest === false && controlOpening.wheelHidden === false
      && controlOpening.bandContinue === true && controlOpening.rosterLineHidden === true
      && controlOpening.apply === "Apply"
      && !(controlOpening.roster + controlOpening.status).includes("data-continue")
      && controlOpening.roster.includes(">Update</button>") && controlOpening.roster.includes(">Remove</button>")
      && controlOpening.roster.includes(">Fix</button>") && controlOpening.roster.includes(">Locate</button>")
      && controlOpening.roster.includes('data-harness-operation="download"')
      && controlOpening.roster.includes('data-harness-state="updatable"')
      && controlOpening.sheetPause === false
      && controlOpening.sheetViewingToggle === false && controlOpening.sheetDiagnosticsToggle === false
      && controlOpening.sheetReveal === false && controlOpening.sheetContinues.every((hidden) => hidden === true),
    JSON.stringify({ ...controlOpening, roster: undefined, status: undefined })],
  ["the window's Settings offers Quit, says who can see, and Status carries the service's sentence",
    controlOpening.quitHidden === false && controlOpening.who === SHEET_WATCHED
      && controlOpening.statusService === "Ghostlight service is up; 1 MCP client and 1 browser connected."
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
      && sheetAfterOff.word === "Off -- only this window shows Ghostlight" && sheetAfterOff.toggle === "Turn on"
      && sheetAfterOff.rail === "Off -- only this window shows Ghostlight" && sheetAfterOff.who === SHEET_PRIVATE
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
      && closedSettings.rail === "Comfortable, motion follows the system" && closedSettings.quitHidden === true
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
      && !closedAbout.includes("Sessions") && !closedAbout.includes("Recorded"),
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
  ["the native shell's page is told it has full control and draws every control",
    sheetAfterBoot.chip === "Full control"
      && nodes.get("setup-integrations").hidden === false && sheetAfterBoot.end === false
      && sheetAfterBoot.test === false && nodes.get("integrations-continue").hidden === true
      && nodes.get("band-continue").hidden === true && sheetAfterBoot.continueLine === true,
    `chip=${sheetAfterBoot.chip}`],
  ["Settings is drawn from the snapshot's controls part in the service's words, and Status carries the sentence",
    sheetAfterBoot.runtimeLabel === "Browser work"
      && sheetAfterBoot.runtimeWord === "Running" && sheetAfterBoot.hold === false
      && sheetAfterBoot.resume === true && sheetAfterBoot.start === true
      && sheetAfterBoot.service === "Ghostlight service is up; 1 MCP client and 1 browser connected."
      && sheetAfterBoot.viewingWord === "On -- anyone on this computer can open this view"
      && sheetAfterBoot.viewingUrl === "http://127.0.0.1:41234/"
      && sheetAfterBoot.viewingToggle === "Turn off" && sheetAfterBoot.viewingToggleHidden === false
      && sheetAfterBoot.notificationsWord.startsWith("Send one local checkup notification"),
    JSON.stringify(sheetAfterBoot)],
  ["the rail states the sections, and the window is offered Quit and told who can see",
    sheetAfterBoot.railScreen === "Comfortable, motion follows the system"
      && sheetAfterBoot.railSharing === "On -- anyone on this computer can open this view"
      && sheetAfterBoot.railBrowserWork === "Running" && sheetAfterBoot.railDiagnostics === "On (marker)"
      && sheetAfterBoot.railNotifications.startsWith("Send one local checkup notification")
      && sheetAfterBoot.quitShown && sheetAfterBoot.who.startsWith("Others on this computer can watch")
      && sheetAfterBoot.address === 0
      && sheetAfterBoot.errors.every((error) => error.includes("deliberate About failure")),
    JSON.stringify(sheetAfterBoot)],
  ["the diagnostics section renders the state and both person acts",
    sheetAfterBoot.diagnosticsWord === "On (marker)" && sheetAfterBoot.diagnosticsPlace === "~/.cache/ghostlight (4.1 KB)"
      && sheetAfterBoot.diagnosticsToggle === "Turn off" && sheetAfterBoot.diagnosticsToggleHidden === false
      && sheetAfterBoot.diagnosticsReveal === false,
    JSON.stringify(sheetAfterBoot)],
  ["a pushed controls part updates Settings and redraws the chip from the context it names, with no snapshot",
    sheetAfterPush.railSharing === "Off -- only this window shows Ghostlight" && sheetAfterPush.railDiagnostics === "Off"
      && sheetAfterPush.viewingWord === "Off -- only this window shows Ghostlight"
      && sheetAfterPush.viewingToggle === "Turn on" && sheetAfterPush.diagnosticsWord === "Off"
      && sheetAfterPush.diagnosticsToggle === "Turn on" && sheetAfterPush.chip === "Full control"
      && sheetAfterPush.chipTitle === SHEET_PRIVATE && sheetAfterPush.snapshots === 0,
    JSON.stringify(sheetAfterPush)],
  ["the Status page holds neither End session nor Send test",
    (() => {
      const status = markup.slice(markup.indexOf('id="view-status"'), markup.indexOf('id="view-policy"'));
      return !status.includes("end_session") && !status.includes("test-notification")
        && !status.includes("runtime-strip") && !status.includes("notifications-strip")
        && !nodes.get("diagnostic-grid").innerHTML.includes("Process diagnostics");
    })(),
    "the Status page still carries a control that moved to Settings"],
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
  ["boot completed without throwing", bootThrew === null, bootThrew],
  ["heartbeat installed", heartbeat],
  ["surface wired", listeners.some((l) => l.startsWith("document:click"))],
  ["global error handlers attached",
    listeners.includes("window:error") && listeners.includes("window:unhandledrejection")],
  ["snapshot still fetched", snapshots > 0],
  ["the pass continued past the broken panel", connections.innerHTML.length > 0,
    `connections: ${JSON.stringify(connections.innerHTML)}`],
  ["an old owned harness path is offered as an update",
    integrationsHtml.includes("Update")
      && integrationsHtml.includes('data-harness-action="install"'),
    `integrations: ${JSON.stringify(integrationsHtml)}`],
  ["one setup-everything action reaches the aggregate orchestrator command",
    setupEverythingWasActionable && bulkSetups === 1
      && listeners.includes("setup-integrations:click"),
    `actionable=${setupEverythingWasActionable} calls=${bulkSetups}`],
  ["integrations stay one flat roster with category order before alphabetic name order",
    !rosterHtml.includes("integration-group")
      && JSON.stringify(rosterCards.map((card) => card.name)) === JSON.stringify([
        "Cline", "Zed", "Claude Code", "Windsurf",
        "Codex", "Junie", "Antigravity", "Qwen Code"
      ]),
    `cards: ${JSON.stringify(rosterCards.map((card) => card.name))} roster: ${JSON.stringify(rosterHtml)}`],
  ["mixed products use Ready before Needs Attention before Available",
    rosterCard("Cline")?.classes.includes("integration-ready")
      && rosterCard("Cline")?.html.includes("Visual Studio Code")
      && rosterCard("Cline")?.html.includes("Foreign entry preserved.")
      && rosterCard("Junie")?.classes.includes("integration-needs-attention")
      && rosterCard("Junie")?.html.includes("Detected."),
    `roster: ${JSON.stringify(rosterHtml)}`],
  ["a blocked target shows the orchestrator's precomposed evidence and other targets show none",
    rosterCard("Junie")?.html.includes('<p class="integration-evidence">')
      && rosterCard("Junie")?.html.includes("`alien-agent`")
      && rosterCard("Junie")?.html.includes("changed nothing.")
      && !rosterCard("Cline")?.html.includes("integration-evidence"),
    `junie: ${JSON.stringify(rosterCard("Junie")?.html)}`],
  ["only a fixable foreign entry offers the explicit Fix action",
    rosterCard("Junie")?.html.includes('data-harness-action="fix"')
      && rosterCard("Junie")?.html.includes(">Fix</button>")
      && !rosterCard("Cline")?.html.includes('data-harness-action="fix"')
      && sources.join("\n").includes('view.confirmFix(name, target)'),
    `junie: ${JSON.stringify(rosterCard("Junie")?.html)}`],
  ["Fix confirms the backup before dispatching the closed action",
    fixConfirmationShown && fixWasManaged,
    `shown=${fixConfirmationShown} calls=${JSON.stringify(managedHarnesses)}`],
  ["every category keeps visible words and a distinct card tone",
    rosterCard("Cline")?.classes.includes("integration-ready")
      && rosterCard("Cline")?.html.includes('<span class="tile-state">Ready</span>')
      && rosterCard("Claude Code")?.classes.includes("integration-available")
      && rosterCard("Claude Code")?.html.includes('<span class="tile-state">Available</span>')
      && rosterCard("Codex")?.classes.includes("integration-needs-attention")
      && rosterCard("Codex")?.html.includes('<span class="tile-state">Needs Attention</span>')
      && rosterCard("Antigravity")?.classes.includes("integration-not-detected")
      && rosterCard("Antigravity")?.html.includes('<span class="tile-state">Not Detected</span>'),
    `roster: ${JSON.stringify(rosterHtml)}`],
  ["plural harness targets share one recognizable product card",
    (rosterHtml.match(/class="tile integration-card/g) ?? []).length === 8
      && rosterHtml.includes("Visual Studio Code")
      && rosterHtml.includes('src="integrations/cline.svg"'),
    `roster: ${JSON.stringify(rosterHtml)}`],
  ["configured and detected targets offer the right durable actions",
    rosterHtml.includes('<span class="tile-state">Ready</span>')
      && rosterHtml.includes(">Remove</button>")
      && rosterHtml.includes(">Set up</button>")
      && rosterHtml.includes(">Copy MCP command</button>"),
    `roster: ${JSON.stringify(rosterHtml)}`],
  ["a missing product offers one install, locate, and manual route",
    rosterHtml.includes('data-product="qwen-code"')
      && rosterHtml.includes('data-harness="qwen-code" data-harness-name="Qwen Code">Locate</button>')
      && rosterHtml.includes('data-copy-kind="command"')
      && rosterHtml.includes('data-harness-manual="qwen-code"'),
    `roster: ${JSON.stringify(rosterHtml)}`],
  ["failed automatic setup opens the target's manual route", failedSetupManual.open],
  ["the front door renders the orchestrator's readiness answer and never authors one",
    nodes.get("state-word").textContent === "Ready"
      && nodes.get("state-word").title === "Connected and idle. Agents can work when they ask."
      && sandbox.document.body.className === "runtime-quiet",
    `readiness: ${JSON.stringify({ word: nodes.get("state-word").textContent, tone: sandbox.document.body.className })}`],
  ["the window contains no readiness vocabulary of its own",
    (() => {
      const source = readFileSync(join(ui, "lib", "view.js"), "utf8");
      // Every word belongs to crates/orchestrator/src/language/readiness.rs. A literal here would
      // be a second source of truth for the one answer the front door exists to give.
      return !["Not connected", "Session ended", "Working", "Quiet"]
        .some((word) => source.includes(`"${word}"`));
    })(),
    "view.js authors a readiness word"],
  ["control follows the projection rather than a locally derived connection",
    (() => {
      const source = readFileSync(join(ui, "lib", "view.js"), "utf8");
      return source.includes("readiness?.invites_control")
        && !source.includes("el.wheel.disabled = !facts.connected");
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
  ["the policy tab is a tab, keeps its name, and carries the authored state behind it",
    markup.includes('class="tab policy-state"')
      && !ids.includes("policy-state-label")
      && policy.dataset.tone === "applied"
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
      && viewSource.includes('readinessNeedsAttention(entry)')
      && stylesheet.includes(".row-dur.unsettled"),
    `attention: ${JSON.stringify({
      loading: wordsApi.readinessNeedsAttention({ settled: true, observed: { readiness: "loading" } }),
      interactive: wordsApi.readinessNeedsAttention({ settled: true, observed: { readiness: "interactive" } })
    })}`],
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
  ["the editor appears only when authoring is permitted",
    editorShown && editorHiddenWhenRefused],
  ["withButton re-enables its button after currentTarget has expired, and never throws",
    withButtonThrew === null && reCheckButton.disabled === false,
    `threw: ${JSON.stringify(withButtonThrew)}, disabled: ${reCheckButton.disabled}`],
  ["failure reported", reported.some((r) => r.includes("deliberate About failure")),
    `reported: ${JSON.stringify(reported)}`],
  ["the failure names the panel", reported.some((r) => r.includes("painting about"))],
  ["the broken panel was attempted, not skipped", before > 0, `attempts: ${before}`]
];

// Quit from the native window reaches the shell's own command, and the band then says so. It runs
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
checks.push(["Quit in the native window is the shell's own command, and the band then says Ghostlight is quitting",
  nativeQuit.commands === 1 && nativeQuit.word === QUITTING_SENTENCE, JSON.stringify(nativeQuit)]);

let ok = true;
for (const [name, pass, detail] of checks) {
  if (!pass) ok = false;
  console.log(`${pass ? "PASS" : "FAIL"}  ${name}${pass || !detail ? "" : `\n        ${detail}`}`);
}
if (!ok) process.exit(1);
console.log("");
console.log("workbench surface ok: a broken panel costs its own panel and nothing else");
