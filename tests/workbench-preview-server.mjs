import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, resolve } from "node:path";

const repository = resolve(import.meta.dirname, "..");
const ui = join(repository, "crates", "orchestrator", "ui");
const port = Number(process.env.GHOSTLIGHT_PREVIEW_PORT || 41737);

const snapshot = {
  seq: 0,
  generated_at_ms: Date.now(),
  service: { version: "1.0.0", started_at_ms: Date.now() - 540000, runtime_state: "active" },
  overview: { active_sessions: 2, active_operations: 2, connected_browsers: 2, blocked_in_history: 1 },
  sessions: [
    { id: "workspace_codex", client_label: "Codex", leased: true, tab_count: 3, held_tab_count: 0, active_operations: 1 },
    { id: "workspace_claude", client_label: "Claude Code", leased: true, tab_count: 1, held_tab_count: 0, active_operations: 1 }
  ],
  operations: [
    { invocation: "invocation_read", workspace: "workspace_codex", tool: "browser_read_page", activity: "Reading page", capability: "read", started_at_ms: Date.now() - 12000, phase: "running" },
    { invocation: "invocation_fill", workspace: "workspace_claude", tool: "browser_fill_form", activity: "Filling form", capability: "write", started_at_ms: Date.now() - 35000, phase: "attention" }
  ],
  browsers: [
    { id: "browser_chrome", family: "Chrome", adapter_version: "1.0.0", connected: true },
    { id: "browser_edge", family: "Edge", adapter_version: "1.0.0", connected: true }
  ],
  history: [
    { timestamp_ms: Date.now() - 90000, invocation: "invocation_blocked", workspace: "workspace_codex", tool: "browser_close_tab", capability: "action", allowed: false, reason: "tab_close_denied", status: "blocked", effect: "none", summary: "Authority blocked the browser job.", duration_ms: 120, observed: {} },
    { timestamp_ms: Date.now() - 240000, invocation: "invocation_open", workspace: "workspace_codex", tool: "browser_open_page", capability: "read", allowed: true, reason: "permitted", status: "succeeded", effect: "committed", summary: "Opened slow.example.org.", duration_ms: 8100, observed: { host: "slow.example.org", readiness: "loading" } }
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

// Representative work the monitor can page through, so the conveyor is reviewable.
const script = [
  { tool: "browser_open_page", activity: "Navigating", capability: "read", ms: 1700, effect: "committed", summary: "Opened example.com.", observed: { host: "example.com", readiness: "complete" } },
  { tool: "browser_read_page", activity: "Reading page", capability: "read", ms: 900, effect: "none", summary: "Read 1,240 words.", observed: { host: "example.com", count: 1240 } },
  { tool: "browser_find", activity: "Finding on page", capability: "read", ms: 700, effect: "none", summary: "Found 7 matches.", observed: { count: 7 } },
  { tool: "browser_fill_form", activity: "Filling form", capability: "write", ms: 2400, effect: "wrote", summary: "Filled 3 fields and submitted the form.", observed: { host: "example.com", readiness: "complete", count: 3 } },
  { tool: "browser_take_screenshot", activity: "Screenshot", capability: "read", ms: 1100, effect: "none", summary: "Captured the viewport at 1280x720.", observed: { width: 1280, height: 720 } },
  { tool: "browser_click", activity: "Clicking", capability: "action", ms: 800, effect: "clicked", summary: "Clicked a target on example.com.", observed: { host: "example.com", readiness: "interactive" } },
  { tool: "browser_wait", activity: "Waiting", capability: "read", ms: 1900, effect: "none", summary: "Wait condition target_present was satisfied after 1830 ms.", observed: { readiness: "complete", count: 1830 } },
  { tool: "browser_run_script", activity: "Running JavaScript", capability: "execute", ms: 1500, effect: "executed", summary: "Evaluated a script on example.com.", observed: { host: "example.com", readiness: "complete" } }
];

const fixture = `window.__GHOSTLIGHT_PREVIEW__ = ${JSON.stringify(snapshot)};
window.__GHOSTLIGHT_SCRIPT__ = ${JSON.stringify(script)};
(() => {
  const preview = window.__GHOSTLIGHT_PREVIEW__;
  const listeners = [];
  let counter = 0;
  let step = 0;

  const publish = change => {
    preview.seq += 1;
    const event = { seq: preview.seq, change };
    for (const handler of listeners) handler({ payload: event });
  };

  const runOne = () => {
    if (preview.service.runtime_state !== "active") return;
    counter += 1;
    const blocked = counter % 9 === 0;
    const spec = blocked
      ? { tool: "browser_run_script", activity: "Running JavaScript", capability: "execute", ms: 900, effect: "none" }
      : window.__GHOSTLIGHT_SCRIPT__[step++ % window.__GHOSTLIGHT_SCRIPT__.length];
    const operation = {
      invocation: "invocation_preview_" + counter,
      workspace: preview.sessions[counter % preview.sessions.length].id,
      tool: spec.tool,
      activity: spec.activity,
      capability: spec.capability,
      started_at_ms: Date.now(),
      phase: "running"
    };
    preview.operations.push(operation);
    publish({ kind: "operation_started", operation });

    setTimeout(() => {
      const index = preview.operations.indexOf(operation);
      if (index >= 0) preview.operations.splice(index, 1);
      const record = {
        timestamp_ms: Date.now(),
        invocation: operation.invocation,
        workspace: operation.workspace,
        tool: operation.tool,
        capability: operation.capability,
        allowed: !blocked,
        reason: blocked ? "policy_denied_execute" : "permitted",
        status: blocked ? "blocked" : "succeeded",
        effect: blocked ? "none" : spec.effect,
        summary: blocked ? "Authority blocked the browser job." : spec.summary,
        duration_ms: spec.ms,
        observed: blocked ? {} : spec.observed
      };
      preview.history.unshift(record);
      publish({ kind: "operation_settled", record });
      setTimeout(runOne, blocked ? 3000 : 700 + Math.random() * 1300);
    }, spec.ms);
  };

  window.__TAURI__ = {
    core: {
      invoke: async (command, args = {}) => {
        if (command === "workbench_snapshot") return preview;
        if (command === "workbench_search") return [];
        if (command === "apply_runtime_intent") {
          const state = args.intent === "end_session" ? "ended" : args.intent === "hold" ? "held" : "active";
          preview.service.runtime_state = state;
          preview.configuration.runtime_state = state;
          publish({ kind: "runtime_changed", runtime_state: state });
          if (state === "active") runOne();
          return { accepted: true, runtime_state: state, browser_notified: true, message: "Runtime control updated." };
        }
        if (command === "refresh_harnesses") return preview.harnesses;
        if (command === "test_notification") return null;
        if (command === "manage_harness") return { changed: false, summary: {}, message: "Preview action completed." };
        throw new Error("Unknown preview command " + command);
      }
    },
    event: {
      listen: async (name, handler) => { listeners.push(handler); return () => {}; }
    }
  };

  setTimeout(runOne, 1200);
})();`;

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
