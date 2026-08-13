// The workbench surface, actually executed.
//
// Every other guard over this window reads its source as text, which can only ever check that
// the right strings are present. None of them could tell that the window did not start: one
// missing element id threw at module scope and silently abandoned the snapshot, the change
// subscription and the heartbeat behind it, and the guards stayed green.
//
// This runs app.js against a minimal DOM with one panel deliberately broken, and asserts what a
// person would check by looking: the window still comes up, the failure is visible, the rest of
// the pass continues, and the broken panel is retried rather than remembered as finished.
//
// Run with: node tests/workbench-surface.mjs
import { readFileSync } from "node:fs";
import vm from "node:vm";
import { join, resolve } from "node:path";

const repository = resolve(import.meta.dirname, "..");
const ui = join(repository, "crates", "orchestrator", "ui");
const source = readFileSync(join(ui, "app.js"), "utf8");
const markup = readFileSync(join(ui, "index.html"), "utf8");

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
  replaceChildren() {}, remove() {}, closest: () => null, focus() {}, append() {}, insertBefore() {}, contains: () => false
});
const nodes = new Map(ids.map((id) => [id, node(id)]));

const reported = [];
let heartbeat = false;
let snapshotFetched = false;

const snapshot = {
  seq: 1, service: { version: "1.0.0", started_at_ms: 0, runtime_state: "active" },
  sessions: [], operations: [], browsers: [], history: [], harnesses: [],
  diagnostics: [], configuration: {}, overview: {}
};

const sandbox = {
  console: { error: (detail) => reported.push(String(detail)), log() {}, warn() {} },
  document: {
    getElementById: (id) => nodes.get(id) ?? null,
    querySelectorAll: (sel) => (sel === "[id]" ? [...nodes.values()] : []),
    querySelector: () => null,
    addEventListener: (kind) => listeners.push(`document:${kind}`),
    hidden: false, body: node("body"),
    createElement: () => node("x"), createDocumentFragment: () => node("f")
  },
  window: {
    addEventListener: (kind) => listeners.push(`window:${kind}`),
    __TAURI__: {
      core: { invoke: async (cmd) => {
        if (cmd === "workbench_snapshot") { snapshotFetched = true; return snapshot; }
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

// Break exactly one panel, every time it is asked to render.
let broken = source.replace(
  "function paintAbout(snapshot) {",
  "function paintAbout(snapshot) { throw new Error('deliberate About failure');"
);
// Expose the surface's own state, and whatever resync swallowed on its quiet path, so a failing
// check reports the reason instead of leaving the harness to be guessed at.
broken = broken.replace(
  "const el = Object.create(null);",
  "globalThis.__state = state;\nconst el = Object.create(null);"
);
broken = broken.replace(
  "  } catch (error) {\n    state.connected = false;",
  "  } catch (error) {\n    globalThis.__caught = String((error && error.stack) || error);\n    state.connected = false;"
);

let bootThrew = null;
try {
  vm.runInContext(broken, sandbox);
} catch (error) {
  bootThrew = error.message;
}

await new Promise((r) => setTimeout(r, 80));

const painted = Object.keys(sandbox.__state?.painted ?? {});
const checks = [
  ["boot completed without throwing", bootThrew === null, bootThrew],
  ["heartbeat installed", heartbeat],
  ["surface wired", listeners.some((l) => l.startsWith("document:click"))],
  ["global error handlers attached",
    listeners.includes("window:error") && listeners.includes("window:unhandledrejection")],
  ["snapshot still fetched", snapshotFetched],
  ["the pass continued past the broken panel", painted.length > 0, `painted: ${painted}`],
  ["failure reported", reported.some((r) => r.includes("deliberate About failure")),
    `reported: ${JSON.stringify(reported)} | resync caught: ${(sandbox.__caught || "nothing").split("\n")[0]}`],
  ["the broken panel is not memoised as done", !painted.includes("about"), `painted: ${painted}`]
];

let ok = true;
for (const [name, pass, detail] of checks) {
  if (!pass) ok = false;
  console.log(`${pass ? "PASS" : "FAIL"}  ${name}${pass || !detail ? "" : `\n        ${detail}`}`);
}
if (!ok) process.exit(1);
console.log("");
console.log("workbench surface ok: a broken panel costs its own panel and nothing else");
