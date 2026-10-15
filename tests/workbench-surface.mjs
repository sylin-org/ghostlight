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
let snapshots = 0;
let bulkSetups = 0;
let feedFixture = false;
const invoked = [];
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
  process_diagnostics: { layer: "marker", directory: "/home/test/.cache/ghostlight", used_bytes: 4242 },
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
    __TAURI__: {
      core: { invoke: async (cmd, args) => {
        invoked.push(cmd);
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
      "    function about(snapshot) {",
      "    function about(snapshot) { globalThis.__aboutAttempts = (globalThis.__aboutAttempts || 0) + 1;"
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
  ceilings: ["Anything that is not an ordinary http or https address."],
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
  process_diagnostics: { layer: "marker", directory: "/home/test/.cache/ghostlight", used_bytes: 4242 },
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

const checks = [
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
  ["the status card renders the diagnostics state and both person acts",
    (() => {
      const grid = nodes.get("diagnostic-grid");
      return Boolean(grid) && grid.innerHTML.includes("Process diagnostics")
        && grid.innerHTML.includes("on (marker)")
        && grid.innerHTML.includes('data-diagnostics-operation="toggle"')
        && grid.innerHTML.includes('data-diagnostics-operation="reveal"');
    })()],
  ["the status card flips to Turn on when the layer changes under it",
    (() => {
      view.collections({
        sessions: [], browsers: [],
        diagnostics: [{ severity: "passing", label: "Orchestrator", detail: "Ghostlight is accepting local connections." }],
        history: [], service: { version: "1.0.0", started_at_ms: 0, runtime_state: "active" },
        process_diagnostics: { layer: "off", directory: "/home/test/.cache/ghostlight", used_bytes: 4242 },
        configuration: { managed_policy: { configured: false } },
        harnesses: []
      }, []);
      const grid = nodes.get("diagnostic-grid");
      return grid.innerHTML.includes("Turn on") && !grid.innerHTML.includes("Turn off")
        && grid.innerHTML.includes("Operational log is off");
    })()],
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
  ["the permanent boundaries are shown",
    nodes.get("policy-ceilings").innerHTML.includes("not an ordinary http or https address")
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

let ok = true;
for (const [name, pass, detail] of checks) {
  if (!pass) ok = false;
  console.log(`${pass ? "PASS" : "FAIL"}  ${name}${pass || !detail ? "" : `\n        ${detail}`}`);
}
if (!ok) process.exit(1);
console.log("");
console.log("workbench surface ok: a broken panel costs its own panel and nothing else");
