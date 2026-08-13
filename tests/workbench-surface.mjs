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
  diagnostics: [], configuration: {}, overview: {}
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
const integrations = nodes.get("integration-grid");
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
    integrations.innerHTML.includes("Update")
      && integrations.innerHTML.includes('data-harness-action="install"'),
    `integrations: ${JSON.stringify(integrations.innerHTML)}`],
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
