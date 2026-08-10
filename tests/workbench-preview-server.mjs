import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, resolve } from "node:path";

const repository = resolve(import.meta.dirname, "..");
const ui = join(repository, "crates", "orchestrator", "ui");
const port = Number(process.env.GHOSTLIGHT_PREVIEW_PORT || 41737);

const snapshot = {
  generated_at_ms: Date.now(),
  service: { version: "1.0.0", started_at_ms: Date.now() - 540000, runtime_state: "active" },
  overview: { active_sessions: 2, active_operations: 2, connected_browsers: 2, blocked_in_history: 1 },
  sessions: [
    { id: "workspace_codex", client_label: "Codex", leased: true, tab_count: 3, held_tab_count: 0, active_operations: 1 },
    { id: "workspace_claude", client_label: "Claude Code", leased: true, tab_count: 1, held_tab_count: 0, active_operations: 1 }
  ],
  operations: [
    { invocation: "invocation_read", workspace: "workspace_codex", tool: "browser_read_page", activity: "Reading page", started_at_ms: Date.now() - 12000, phase: "running" },
    { invocation: "invocation_fill", workspace: "workspace_claude", tool: "browser_fill_form", activity: "Filling form", started_at_ms: Date.now() - 35000, phase: "attention" }
  ],
  browsers: [
    { id: "browser_chrome", family: "Chrome", adapter_version: "1.0.0", connected: true },
    { id: "browser_edge", family: "Edge", adapter_version: "1.0.0", connected: true }
  ],
  history: [
    { timestamp_ms: Date.now() - 90000, invocation: "invocation_blocked", workspace: "workspace_codex", tool: "browser_close_tab", capability: "action", allowed: false, reason: "tab_close_denied", status: "blocked", effect: "none" },
    { timestamp_ms: Date.now() - 240000, invocation: "invocation_open", workspace: "workspace_codex", tool: "browser_open_page", capability: "read", allowed: true, reason: "permitted", status: "succeeded", effect: "committed" }
  ],
  diagnostics: [
    { id: "service", label: "Orchestrator", severity: "passing", detail: "Ghostlight is accepting local connections." },
    { id: "browser", label: "Browser adapters", severity: "passing", detail: "Compatible browser adapters are connected." },
    { id: "authority", label: "Authority", severity: "passing", detail: "Configured authority sources are valid." }
  ],
  harnesses: [
    { id: "codex", name: "Codex", state: "installed", detail: "Ghostlight is registered for this user context.", can_install: false, can_uninstall: true },
    { id: "claude-code", name: "Claude Code", state: "available", detail: "Detected and ready for an explicit Ghostlight registration.", can_install: true, can_uninstall: false },
    { id: "cursor", name: "Cursor", state: "not_detected", detail: "Not detected. You can prepare its user configuration before installing it.", can_install: true, can_uninstall: false }
  ],
  configuration: { runtime_state: "active", local_policy_configured: true, local_policy_valid: true, managed_authority_configured: false, managed_authority_valid: true, runtime_control_file_configured: false }
};

const fixture = `window.__TAURI__ = { core: { invoke: async (command, args = {}) => {
  if (command === "workbench_snapshot") return window.__GHOSTLIGHT_PREVIEW__;
  if (command === "workbench_search") return [];
  if (command === "apply_runtime_intent") {
    const state = args.intent === "end_session" ? "ended" : args.intent === "hold" ? "held" : "active";
    window.__GHOSTLIGHT_PREVIEW__.service.runtime_state = state;
    window.__GHOSTLIGHT_PREVIEW__.configuration.runtime_state = state;
    return { accepted: true, runtime_state: state, browser_notified: true, message: "Runtime control updated." };
  }
  if (command === "refresh_harnesses") return window.__GHOSTLIGHT_PREVIEW__.harnesses;
  if (command === "test_notification") return null;
  if (command === "manage_harness") return { changed: false, summary: {}, message: "Preview action completed." };
  throw new Error("Unknown preview command " + command);
} } }; window.__GHOSTLIGHT_PREVIEW__ = ${JSON.stringify(snapshot)};`;

const types = { ".css": "text/css", ".js": "text/javascript", ".png": "image/png" };

createServer(async (request, response) => {
  try {
    const pathname = new URL(request.url, `http://${request.headers.host}`).pathname;
    if (pathname === "/fixture.js") {
      response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
      response.end(fixture);
      return;
    }
    const name = pathname === "/" ? "index.html" : pathname.slice(1);
    if (!new Set(["index.html", "styles.css", "app.js", "ghostlight.png"]).has(name)) {
      response.writeHead(404).end();
      return;
    }
    let body = await readFile(join(ui, name));
    if (name === "index.html") {
      body = Buffer.from(body.toString("utf8").replace(
        '<script src="app.js"></script>',
        '<script src="fixture.js"></script><script src="app.js"></script>'
      ));
    }
    response.writeHead(200, { "content-type": types[extname(name)] || "text/html; charset=utf-8", "cache-control": "no-store" });
    response.end(body);
  } catch (error) {
    response.writeHead(500, { "content-type": "text/plain" });
    response.end(String(error));
  }
}).listen(port, "127.0.0.1", () => console.log(`http://127.0.0.1:${port}`));
