import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, resolve } from "node:path";

const repository = resolve(import.meta.dirname, "..");
const ui = join(repository, "crates", "orchestrator", "ui");
const port = Number(process.env.GHOSTLIGHT_PREVIEW_PORT || 41737);
const connectorCommand = "/opt/ghostlight/ghostlight-mcp-connector";

const harnessDetail = Object.freeze({
  installed: "Ghostlight is registered for this user context.",
  available: "Detected and ready for an explicit Ghostlight registration.",
  updatable: "Ghostlight is registered through an older Ghostlight installation or executable.",
  // ADR-0135: blocked targets name their actual cause and carry precomposed evidence.
  needs_attention: "A foreign entry holds this target's Ghostlight registration; it was left untouched.",
  not_detected: "Not detected. You can prepare its user configuration before installing it."
});

const jsonSetup = (collection = "mcpServers") => JSON.stringify({
  [collection]: { ghostlight: { command: connectorCommand, args: [], env: {} } }
}, null, 2);

const previewHarness = ({
  id, productId = id, name, target, icon = `${id}.svg`, state, configPath,
  canDownload = true, manualSetup = jsonSetup(), detail, evidence
}) => ({
  id,
  product_id: productId,
  name,
  target,
  icon,
  state,
  detail: detail ?? harnessDetail[state],
  ...(evidence ? { evidence } : {}),
  can_install: ["available", "updatable", "not_detected"].includes(state),
  can_uninstall: state === "installed",
  can_download: canDownload,
  can_locate: true,
  config_path: configPath,
  connector_command: connectorCommand,
  manual_setup: manualSetup
});

// This is the complete fixed registry, deliberately shuffled. The surface must form its four
// semantic groups and alphabetize products without inheriting registry or fixture order.
const harnesses = [
  previewHarness({ id: "qwen-code", name: "Qwen Code", target: "CLI", state: "not_detected", configPath: "/home/test/.qwen/settings.json" }),
  previewHarness({ id: "cline-vscode", productId: "cline", name: "Cline", target: "Visual Studio Code", icon: "cline.svg", state: "available", configPath: "/home/test/.config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" }),
  previewHarness({
    id: "claude-desktop", name: "Claude Desktop", target: "User", state: "needs_attention",
    configPath: "/home/test/.config/Claude/claude_desktop_config.json", canDownload: false,
    detail: "This configuration could not be parsed; it was left untouched.",
    evidence: "The parser reported: expected value at line 1 column 2 of the JSON input"
  }),
  previewHarness({ id: "windsurf", name: "Windsurf", target: "User", state: "available", configPath: "/home/test/.codeium/windsurf/mcp_config.json" }),
  previewHarness({ id: "devin", productId: "windsurf", name: "Devin", target: "Devin user", icon: "windsurf.svg", state: "installed", configPath: "/home/test/.config/devin/mcp_config.json" }),
  previewHarness({ id: "crush", name: "Crush", target: "User", state: "not_detected", configPath: "/home/test/.config/crush/crush.json", manualSetup: jsonSetup("mcp") }),
  previewHarness({ id: "codex", name: "Codex", target: "User", state: "installed", configPath: "/home/test/.codex/config.toml", manualSetup: `[mcp_servers.ghostlight]\ncommand = "${connectorCommand}"\nargs = []\n` }),
  previewHarness({ id: "continue", name: "Continue", target: "CLI and IDE", state: "updatable", configPath: "/home/test/.continue/config.yaml", manualSetup: `name: Local Config\nversion: 1.0.0\nschema: v1\nmcpServers:\n  - name: Ghostlight\n    command: "${connectorCommand}"\n    args: []\n` }),
  previewHarness({ id: "antigravity", name: "Antigravity", target: "CLI", state: "not_detected", configPath: "/home/test/.gemini/config/mcp_config.json" }),
  previewHarness({
    id: "cline-cursor", productId: "cline", name: "Cline", target: "Cursor", icon: "cline.svg",
    state: "needs_attention",
    configPath: "/home/test/.config/Cursor/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
    evidence: `Found \`npx other-agent\` where Ghostlight's connector belongs; Ghostlight maintains \`${connectorCommand}\` there and changed nothing.`
  }),
  previewHarness({ id: "cursor", name: "Cursor", target: "User", state: "available", configPath: "/home/test/.cursor/mcp.json" }),
  previewHarness({ id: "zcode", name: "ZCode", target: "User", state: "installed", configPath: "/home/test/.zcode/cli/config.json" }),
  previewHarness({ id: "zed", name: "Zed", target: "User", state: "installed", configPath: "/home/test/.config/zed/settings.json", manualSetup: jsonSetup("context_servers") }),
  previewHarness({ id: "goose", name: "goose", target: "CLI and desktop", state: "not_detected", configPath: "/home/test/.config/goose/config.yaml", manualSetup: `extensions:\n  ghostlight:\n    type: stdio\n    name: ghostlight\n    display_name: Ghostlight\n    enabled: true\n    cmd: "${connectorCommand}"\n    args: []\n    envs: {}\n    timeout: 300\n` }),
  previewHarness({ id: "copilot-cli", name: "GitHub Copilot CLI", target: "CLI", state: "available", configPath: "/home/test/.copilot/mcp-config.json" }),
  previewHarness({ id: "cline-cli", productId: "cline", name: "Cline", target: "CLI", icon: "cline.svg", state: "installed", configPath: "/home/test/.cline/data/settings/cline_mcp_settings.json" }),
  previewHarness({
    id: "junie", name: "Junie", target: "CLI and JetBrains", state: "needs_attention",
    configPath: "/home/test/.junie/mcp/mcp.json",
    evidence: `Found an entry without a usable command where Ghostlight's connector belongs; Ghostlight maintains \`${connectorCommand}\` there and changed nothing.`
  }),
  previewHarness({ id: "opencode", name: "OpenCode", target: "User", state: "not_detected", configPath: "/home/test/.config/opencode/opencode.json", manualSetup: jsonSetup("mcp") }),
  previewHarness({ id: "kiro", name: "Kiro", target: "CLI and IDE", state: "available", configPath: "/home/test/.kiro/settings/mcp.json" }),
  previewHarness({
    id: "kilo-code", name: "Kilo Code", target: "CLI", state: "needs_attention",
    configPath: "/home/test/.config/kilo/kilo.json", manualSetup: jsonSetup("mcp"),
    evidence: `Found \`alien-cli --mcp\` where Ghostlight's connector belongs; Ghostlight maintains \`${connectorCommand}\` there and changed nothing.`
  }),
  previewHarness({ id: "claude-code", name: "Claude Code", target: "User", state: "available", configPath: "/home/test/.claude.json" }),
  previewHarness({ id: "cline-windsurf", productId: "cline", name: "Cline", target: "Windsurf", icon: "cline.svg", state: "not_detected", configPath: "/home/test/.config/Windsurf/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" }),
  previewHarness({ id: "vscode", name: "Visual Studio Code", target: "User", state: "available", configPath: "/home/test/.config/Code/User/mcp.json", manualSetup: jsonSetup("servers") })
];

const expectedHarnessIds = [
  "antigravity", "claude-code", "claude-desktop", "cline-cli", "cline-cursor",
  "cline-vscode", "cline-windsurf", "codex", "continue", "copilot-cli", "crush", "cursor",
  "devin", "goose", "junie", "kilo-code", "kiro", "opencode", "qwen-code", "vscode",
  "windsurf", "zcode", "zed"
];
const actualHarnessIds = harnesses.map((harness) => harness.id).sort();
const previewProductCount = new Set(harnesses.map((harness) => harness.product_id)).size;
if (JSON.stringify(actualHarnessIds) !== JSON.stringify(expectedHarnessIds)
  || previewProductCount !== 19) {
  throw new Error(`preview harness roster drifted: ${harnesses.length} rows / ${previewProductCount} products`);
}

const snapshot = {
  seq: 0,
  generated_at_ms: Date.now(),
  service: { version: "1.0.0", started_at_ms: Date.now() - 540000, runtime_state: "active" },
  overview: { active_sessions: 2, active_operations: 2, connected_browsers: 2, blocked_in_history: 1 },
  sessions: [
    { id: "workspace_codex", client_label: "Codex", channel: "mcp", leased: true, tab_count: 3, held_tab_count: 0, active_operations: 1 },
    { id: "workspace_claude", client_label: "Claude Code", channel: "mcp", leased: true, tab_count: 1, held_tab_count: 0, active_operations: 1 },
    { id: "workspace_script", client_label: "ghostlight call", channel: "cli", leased: false, tab_count: 1, held_tab_count: 0, active_operations: 0 }
  ],
  operations: [
    { invocation: "invocation_read", workspace: "workspace_codex", tool: "browser_read", activity: "Reading page", capability: "read", started_at_ms: Date.now() - 12000, phase: "running" },
    { invocation: "invocation_fill", workspace: "workspace_claude", tool: "browser_fill_form", activity: "Filling form", capability: "write", started_at_ms: Date.now() - 35000, phase: "attention" }
  ],
  browsers: [
    { id: "browser_chrome", family: "Chrome", adapter_version: "1.0.0", connected: true },
    { id: "browser_edge", family: "Edge", adapter_version: "1.0.0", connected: true }
  ],
  history: [
    { timestamp_ms: Date.now() - 90000, invocation: "invocation_blocked", workspace: "workspace_codex", tool: "browser_tabs", capability: "action", allowed: false, reason: "tab_close_denied", status: "blocked", effect: "none", summary: "Authority blocked the browser job.", duration_ms: 120, observed: {}, channel: "mcp" },
    { timestamp_ms: Date.now() - 240000, invocation: "invocation_open", workspace: "workspace_codex", tool: "browser_navigate", capability: "action", allowed: true, reason: "permitted", status: "succeeded", effect: "committed", summary: "Opened slow.example.org.", duration_ms: 8100, observed: { host: "slow.example.org", readiness: "loading" }, channel: "cli" }
  ],
  diagnostics: [
    { id: "service", label: "Orchestrator", severity: "passing", detail: "Ghostlight is accepting local connections." },
    { id: "browser", label: "Browser adapters", severity: "passing", detail: "Compatible browser adapters are connected." },
    { id: "authority", label: "Authority", severity: "passing", detail: "Configured authority sources are valid." }
  ],
  harnesses,
  configuration: {
    runtime_state: "active",
    local_policy_configured: true,
    local_policy_active: true,
    local_policy_valid: true,
    managed_authority_configured: false,
    managed_authority_active: false,
    managed_authority_valid: true,
    runtime_control_file_configured: false,
    managed_policy: { configured: false },
    policy: {
      situation: "layered",
      detail: "Northwind sets the rules, and you have narrowed them further.",
      tone: "applied"
    }
  }
};

// The compiled policy the Policy destination reads. Shaped exactly like the orchestrator's own
// projection so the preview exercises the real rendering, including a rule it can prove is inert.
const policyView = {
  situation: "layered",
  headline: "Northwind sets the rules, and you have narrowed them further.",
  organization: {
    name: "Northwind",
    statement: "Browser work stays inside approved support sites.",
    url: "https://northwind.example/browser-policy",
    contacts: [{ kind: "email", value: "security@northwind.example", label: "Security team" }]
  },
  capabilities: [
    { capability: "read", label: "Look at pages", covers: "Read page text, take screenshots, scroll, and find things on a page.", state: "some_allowed", detail: "Refused everywhere except the sites Northwind allowed.", decided_by: ["organization"] },
    { capability: "action", label: "Click and type", covers: "Click, type, press keys, drag, and move through history.", state: "some_allowed", detail: "Refused everywhere except the sites Northwind and you allowed.", decided_by: ["organization", "user"] },
    { capability: "write", label: "Fill in forms", covers: "Enter information into forms and upload files.", state: "unavailable", detail: "Not available. Northwind does not allow it anywhere.", decided_by: ["organization"] },
    { capability: "execute", label: "Run page code", covers: "Run JavaScript inside a page.", state: "unavailable", detail: "Not available. Northwind does not allow it anywhere.", decided_by: ["organization"] }
  ],
  layers: [
    {
      kind: "organization", title: "Northwind", policy_name: "Support workspace", version: "2026-08-14", mode: "enforce",
      rules: [{ id: "support-sites", description: "Ordinary support work", allow: ["support.northwind.example", "*.support.northwind.example"], deny: ["admin.support.northwind.example"], allowed: ["read", "action"], mode: "enforce", note: null }],
      settings: [{ key: "browser.tabs.allow_close", value: "false", level: "mandatory" }],
      path: null,
      document: '{\n  "schema": 3,\n  "name": "Support workspace",\n  "version": "2026-08-14"\n}'
    },
    {
      kind: "user", title: "Your rules", policy_name: "Your rules", version: "2026-08-14", mode: "enforce",
      rules: [
        { id: "rule-1", description: null, allow: ["*.support.northwind.example"], deny: [], allowed: ["read", "action"], mode: "enforce", note: null },
        { id: "rule-2", description: "Left over from last week", allow: ["one.support.northwind.example"], deny: [], allowed: ["read"], mode: "enforce", note: "unreachable" }
      ],
      settings: [],
      path: "C:/Users/you/AppData/Local/Ghostlight/user-policy.json",
      document: '{\n  "schema": 3,\n  "name": "Your rules",\n  "version": "2026-08-14"\n}'
    }
  ],
  ceilings: [
    "Anything that is not an ordinary http or https address."
  ],
  user_layer: {
    source: "workbench",
    authoring_allowed: true,
    editable: true,
    path: "C:/Users/you/AppData/Local/Ghostlight/user-policy.json",
    blocked_reason: null
  },
  passport: {
    configured: true, verified: true, freshness: "fresh", sequence: 7,
    organization: "Northwind", rationale: "Browser work stays inside approved support sites.",
    contacts: [{ kind: "email", value: "security@northwind.example", label: "Security team" }],
    source_class: "https", last_success_ms: Date.now() - 240000, last_attempt_ms: Date.now() - 240000
  }
};

// Representative work the monitor can page through, so the conveyor is reviewable.
const script = [
  { tool: "browser_navigate", activity: "Navigating", capability: "action", ms: 1700, effect: "committed", summary: "Opened example.com.", observed: { host: "example.com", readiness: "complete" } },
  { tool: "browser_read", activity: "Reading page", capability: "read", ms: 900, effect: "none", summary: "Read 1,240 words from example.com.", observed: { host: "example.com", count: 1240 } },
  { tool: "browser_find", activity: "Finding on page", capability: "read", ms: 700, effect: "none", summary: "Found 7 matches.", observed: { count: 7 } },
  { tool: "browser_fill_form", activity: "Filling form", capability: "write", ms: 2400, effect: "wrote", summary: "Filled 3 fields on example.com and submitted the form.", observed: { host: "example.com", readiness: "complete", count: 3 } },
  { tool: "browser_screenshot", activity: "Screenshot", capability: "read", ms: 1100, effect: "none", summary: "Captured the viewport at 1280x720.", observed: { width: 1280, height: 720 } },
  { tool: "browser_click", activity: "Clicking", capability: "action", ms: 800, effect: "clicked", summary: "Clicked a button on example.com.", observed: { host: "example.com", readiness: "interactive" } },
  { tool: "browser_wait", activity: "Waiting", capability: "read", ms: 1900, effect: "none", summary: "The target appeared on example.com in 2 seconds.", observed: { host: "example.com", readiness: "complete", count: 2 } },
  { tool: "browser_execute", activity: "Running JavaScript", capability: "execute", ms: 1500, effect: "executed", summary: "Executed JavaScript on example.com.", observed: { host: "example.com", readiness: "complete" } }
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
      ? { tool: "browser_execute", activity: "Running JavaScript", capability: "execute", ms: 900, effect: "none" }
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
        observed: blocked ? {} : spec.observed,
        // Alternate the intake so both read side by side in the queue.
        channel: counter % 3 === 0 ? "cli" : "mcp"
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
        if (command === "workbench_policy") return policyView;
        if (command === "preview_user_policy") {
          return {
            considered: 42,
            refused_total: 3,
            refused: [
              { tool: "browser_execute", host: "github.com", count: 2 },
              { tool: "browser_upload", host: null, count: 1 }
            ],
            summary: "3 of the last 42 recorded actions would have been refused."
          };
        }
        if (command === "apply_user_policy") return { accepted: true, runtime_state: "active", browser_notified: false, message: "Your rules are applied: Your rules 2026-08-14." };
        if (command === "remove_user_policy") return { accepted: true, runtime_state: "active", browser_notified: false, message: "Your rules are removed." };
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

const types = {
  ".css": "text/css", ".js": "text/javascript", ".png": "image/png", ".svg": "image/svg+xml"
};

createServer(async (request, response) => {
  try {
    const pathname = new URL(request.url, `http://${request.headers.host}`).pathname;
    if (pathname === "/fixture.js") {
      response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
      response.end(fixture);
      return;
    }
    const name = pathname === "/" ? "index.html" : pathname.slice(1);
    const served = new Set(["index.html", "styles.css", "app.js", "ghostlight.png"]);
    if (!served.has(name) && !/^lib\/[a-z]+\.js$/.test(name)
      && !/^integrations\/[a-z0-9-]+\.svg$/.test(name)) {
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
