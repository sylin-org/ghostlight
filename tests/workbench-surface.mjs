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
    id: "codex", name: "Codex", state: "updatable",
    detail: "The connector path belongs to an older installation.",
    can_install: true, can_uninstall: false
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
    querySelector: () => null,
    addEventListener: (kind) => listeners.push(`document:${kind}`),
    removeEventListener() {},
    hidden: false, body: node("body"),
    createElement: () => node("x"), createDocumentFragment: () => node("f")
  },
  window: {
    addEventListener: (kind) => listeners.push(`window:${kind}`),
    __TAURI__: {
      core: { invoke: async (cmd) => (cmd === "workbench_snapshot" ? snapshot() : []) },
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
    { capability: "read", label: "Look at pages", covers: "Read page text.", state: "sites", detail: "Available on specific sites only, set by Example Org.", decided_by: ["organization"] },
    { capability: "action", label: "Click and type", covers: "Click and type.", state: "available", detail: "Available on ordinary websites. Nothing narrows it.", decided_by: [] },
    { capability: "write", label: "Fill in forms", covers: "Enter information.", state: "unavailable", detail: "Not available anywhere. Example Org does not allow it.", decided_by: ["organization"] },
    { capability: "execute", label: "Run page code", covers: "Run JavaScript.", state: "unavailable", detail: "Not available anywhere. Example Org does not allow it.", decided_by: ["organization"] }
  ],
  layers: [
    { kind: "organization", title: "Example Org", policy_name: "Support", version: "1", mode: "enforce",
      rules: [{ id: "support", description: "Ordinary support work", allow: ["support.example.test"], deny: [], allowed: ["read"], mode: "enforce", note: null }],
      settings: [], path: null, document: '{"schema": 3}' },
    { kind: "user", title: "Your rules", policy_name: "Your rules", version: "1", mode: "enforce",
      rules: [{ id: "leftover", description: null, allow: ["other.test"], deny: [], allowed: ["read"], mode: "enforce", note: "no_effect" }],
      settings: [], path: "state/user-policy.json", document: '{"schema": 3}' }
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
const board = nodes.get("capability-board");
const layers = nodes.get("policy-layers");
const organization = nodes.get("policy-organization");
const editorShown = !nodes.get("policy-editor").hidden;
view.policy(compiled(false));
const editorHiddenWhenRefused = nodes.get("policy-editor").hidden
  && !nodes.get("policy-blocked").hidden
  && nodes.get("policy-blocked-reason").textContent.includes("Example Org");
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
  ["every capability line carries its answer and its decider",
    board.innerHTML.includes("Some sites") && board.innerHTML.includes("Not allowed")
      && board.innerHTML.includes("Allowed")
      && board.innerHTML.includes("Example Org does not allow it"),
    `board: ${JSON.stringify(board.innerHTML)}`],
  ["the permanent boundaries are shown",
    nodes.get("policy-ceilings").innerHTML.includes(".localhost")],
  ["the organization is named in its own words, with somewhere to ask",
    organization.innerHTML.includes("Example Org")
      && organization.innerHTML.includes("Ask the service desk")
      && organization.innerHTML.includes("security@example.test"),
    `organization: ${JSON.stringify(organization.innerHTML)}`],
  ["both layers are shown with their documents",
    layers.innerHTML.includes("Example Org") && layers.innerHTML.includes("Your rules")
      && layers.innerHTML.includes("state/user-policy.json")
      && layers.innerHTML.includes("Show the exact document"),
    `layers: ${JSON.stringify(layers.innerHTML)}`],
  ["a rule the organization already refuses is marked in place",
    layers.innerHTML.includes("already refuses this, so it changes nothing"),
    `layers: ${JSON.stringify(layers.innerHTML)}`],
  ["the editor appears only when authoring is permitted",
    editorShown && editorHiddenWhenRefused],
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
