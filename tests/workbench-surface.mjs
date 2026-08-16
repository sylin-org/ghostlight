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
const failedSetupManual = { open: false };
const node = (id) => ({
  id, textContent: "", innerHTML: "", className: "", hidden: true, disabled: false, scrollTop: 0,
  style: { setProperty() {} }, dataset: {}, classList: { toggle() {}, add() {}, remove() {} },
  addEventListener: (kind) => listeners.push(`${id}:${kind}`),
  removeEventListener() {}, setAttribute() {}, removeAttribute() {},
  querySelector: () => null, querySelectorAll: () => [], appendChild() {}, prepend() {},
  replaceChildren() {}, remove() {}, closest: () => null, focus() {}, append() {},
  insertBefore() {}, contains: () => false, getBoundingClientRect: () => ({ left: 0, top: 0, width: 1, height: 1 })
});
const nodes = new Map(ids.map((id) => [id, node(id)]));

const reported = [];
let heartbeat = false;
let snapshots = 0;

const snapshot = () => ({
  seq: ++snapshots,
  service: { version: "1.0.0", started_at_ms: 0, runtime_state: "active" },
  sessions: [], operations: [], browsers: [], history: [],
  harnesses: [{
    id: "codex", product_id: "codex", name: "Codex", target: "User", icon: "codex.svg", state: "updatable",
    detail: "The connector path belongs to an older installation.",
    can_install: true, can_uninstall: false, can_download: true, can_locate: true,
    config_path: "/home/test/.codex/config.toml", connector_command: "/opt/ghostlight/ghostlight-mcp-connector",
    manual_setup: "[mcp_servers.ghostlight]"
  }],
  diagnostics: [],
  configuration: {
    local_policy_configured: true,
    local_policy_active: true,
    local_policy_valid: true,
    managed_authority_configured: false,
    managed_authority_active: false,
    managed_authority_valid: true,
    runtime_control_file_configured: false,
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
      documentHandlers.set(kind, handler);
    },
    removeEventListener() {},
    hidden: false, body: node("body"),
    createElement: () => node("x"), createDocumentFragment: () => node("f")
  },
  window: {
    addEventListener: (kind) => listeners.push(`window:${kind}`),
    __TAURI__: {
      core: { invoke: async (cmd) => {
        if (cmd === "workbench_snapshot") return snapshot();
        if (cmd === "manage_harness") throw new Error("deliberate setup failure");
        return [];
      } },
      event: { listen: async () => () => {} }
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
documentHandlers.get("click")({
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

const connections = nodes.get("connections");
// Read what the booted surface drew before anything below repaints the same nodes: the second
// view instance shares this one stub document, so a later render would answer for the first.
const integrationsHtml = nodes.get("integration-grid").innerHTML;
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
    { kind: "organization", title: "Example Org", policy_name: "Support", version: "1", mode: "enforce",
      rules: [{ id: "support", description: "Ordinary support work", allow: ["support.example.test"], deny: [], allowed: ["read"], mode: "enforce", note: null }],
      settings: [{ key: "privacy.preserve_target_names", value: "false", level: "mandatory" }],
      path: null, document: '{"schema": 3}' },
    { kind: "user", title: "Your rules", policy_name: "Your rules", version: "1", mode: "enforce",
      rules: [{ id: "leftover", description: null, allow: ["other.test"], deny: [], allowed: ["read"], mode: "enforce", note: "no_effect" }],
      settings: [{ key: "browser.tabs.allow_close", value: "false", level: "mandatory" }],
      path: "state/user-policy.json", document: '{"schema": 3}' }
  ],
  ceilings: ["localhost and any name ending in .localhost.", "Loopback and link-local addresses."],
  user_layer: {
    source: "workbench",
    authoring_allowed: editable,
    editable,
    path: "state/user-policy.json",
    blocked_reason: editable ? null : "Example Org does not allow rules to be set on this machine."
  },
  passport: { configured: false, contacts: [] }
});

// A second view over the same stub document. The booted surface holds its own instance; this one
// exists so the destination can be drawn on demand without a real click.
const view = sandbox.globalThis.GhostlightView.create({ onFailure: (what, error) => reported.push(`${what}: ${error?.message ?? error}`) });

view.collections({
  sessions: [], browsers: [], diagnostics: [], history: [], service: { version: "1.0.0" },
  configuration: { managed_policy: { configured: false } },
  harnesses: [
    { id: "cline-cli", product_id: "cline", name: "Cline", target: "CLI", icon: "cline.svg",
      state: "installed", detail: "Current.", can_install: false, can_uninstall: true,
      can_download: true, can_locate: true, config_path: "/tmp/cline-cli.json",
      connector_command: "/opt/ghostlight/ghostlight-mcp-connector", manual_setup: "cli setup" },
    { id: "cline-vscode", product_id: "cline", name: "Cline", target: "Visual Studio Code",
      icon: "cline.svg", state: "available", detail: "Detected.", can_install: true,
      can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/cline-vscode.json",
      connector_command: "/opt/ghostlight/ghostlight-mcp-connector", manual_setup: "editor setup" },
    { id: "qwen-code", product_id: "qwen-code", name: "Qwen Code", target: "CLI",
      icon: "qwen-code.svg", state: "not_detected", detail: "Not detected.", can_install: true,
      can_uninstall: false, can_download: true, can_locate: true, config_path: "/tmp/qwen.json",
      connector_command: "/opt/ghostlight/ghostlight-mcp-connector", manual_setup: "qwen setup" }
  ]
}, new Set());
const rosterHtml = nodes.get("integration-grid").innerHTML;

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

view.policy(compiled(true));
const board = nodes.get("capability-board").innerHTML;
const organization = nodes.get("policy-organization").innerHTML;
const documents = nodes.get("policy-documents").innerHTML;
const settingsShown = nodes.get("policy-settings").innerHTML;
const rules = nodes.get("rule-list").innerHTML;
const editorShown = !nodes.get("policy-editor").hidden;
// Permissions are authored here too, phrased as what a person can do, and only ever in the
// direction that tightens underneath. The fixture's user layer already restricts tab closing and
// the fixture's organization layer already forces the command line off, so the first render
// exercises both a person's own toggle and an organization ceiling at once.
const permissionsOnLoad = nodes.get("setting-groups").innerHTML;
const authoredOnLoad = JSON.parse(view.draftDocument()).config;
view.setPermission("channels.mcp.enabled", false);
view.setSacred("vault.example.test, *.finance.example.test");
const authoredAfterEdit = JSON.parse(view.draftDocument()).config;
view.setPermission("browser.tabs.allow_close", true);
const authoredAfterClearing = JSON.parse(view.draftDocument()).config;

// Opening and closing one organization rule: the detail belongs to the row, not to the page.
view.toggleRule("org:Example Org:support");
const openedDetail = nodes.get("rule-list").innerHTML;
view.toggleRule("org:Example Org:support");
const closedAgain = nodes.get("rule-list").innerHTML;

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

const checks = [
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
  ["plural harness targets share one recognizable product card",
    (rosterHtml.match(/class="tile integration-card/g) ?? []).length === 2
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
    nodes.get("policy-ceilings").innerHTML.includes(".localhost")],
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
    permissionsOnLoad.includes("Where agents may connect")
      && permissionsOnLoad.includes("In the browser")
      && permissionsOnLoad.includes("Privacy")
      && permissionsOnLoad.includes("MCP clients")
      && permissionsOnLoad.includes("Command line")
      && !permissionsOnLoad.includes("channels.cli.enabled</span>"),
    `permissions: ${JSON.stringify(permissionsOnLoad)}`],
  ["a permission on by default renders checked, and a person's own restriction renders unchecked",
    /data-restriction="channels\.mcp\.enabled"\s+checked/.test(permissionsOnLoad)
      && /data-restriction="browser\.tabs\.allow_close"(?!\s+checked)/.test(permissionsOnLoad),
    `permissions: ${JSON.stringify(permissionsOnLoad)}`],
  ["an organization ceiling disables the switch and names who set it",
    permissionsOnLoad.includes("Example Org already turned this off.")
      && /data-restriction="privacy\.preserve_target_names"[^>]*disabled/.test(permissionsOnLoad),
    `permissions: ${JSON.stringify(permissionsOnLoad)}`],
  ["the way in is a real destination, not prose: MCP links to Integrations, CLI to the scripting guide",
    permissionsOnLoad.includes('data-view="integrations"')
      && permissionsOnLoad.includes('data-destination="scripting_guide"'),
    `permissions: ${JSON.stringify(permissionsOnLoad)}`],
  ["an authored restriction is read back into the draft",
    authoredOnLoad.length === 1
      && authoredOnLoad[0].key === "browser.tabs.allow_close"
      && authoredOnLoad[0].value === false
      && authoredOnLoad[0].level === "mandatory",
    `authored: ${JSON.stringify(authoredOnLoad)}`],
  ["turning a permission off authors only the tightening value",
    authoredAfterEdit.some((entry) => entry.key === "channels.mcp.enabled" && entry.value === false)
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
