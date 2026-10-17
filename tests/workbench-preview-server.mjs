import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, resolve } from "node:path";
import { setupStages as stagesOfSetup } from "./workbench-setup-fixtures.mjs";

const repository = resolve(import.meta.dirname, "..");
const ui = join(repository, "crates", "orchestrator", "ui");
const extensionFolder = join(repository, "extension");
const port = Number(process.env.GHOSTLIGHT_PREVIEW_PORT || 41737);
const connectorPath = "/opt/ghostlight/ghostlight";
const connectorCommand = `${connectorPath} mcp`;
// The one app icon the preview shows, a small synthetic PNG, served the way the service serves an
// agent's cached icon.
const agentIconFile = join(repository, "tests", "fixtures", "workbench", "agent-icon.png");
const AGENT_ICON_DIRECTORY = "agent-icons";

const harnessDetail = Object.freeze({
  installed: "Ghostlight is registered for this user context.",
  available: "Detected and ready for an explicit Ghostlight registration.",
  updatable: "Ghostlight's saved registration needs an update.",
  // ADR-0135: blocked targets name their actual cause and carry precomposed evidence.
  needs_attention: "A foreign entry holds this target's Ghostlight registration; it was left untouched.",
  not_detected: "Not detected. You can prepare its user configuration before installing it."
});

const jsonSetup = (collection = "mcpServers") => JSON.stringify({
  [collection]: { ghostlight: { command: connectorPath, args: ["mcp"], env: {} } }
}, null, 2);

const previewHarness = ({
  id, agentId = id, productId = id, name, label = name, target, monogram = `${id}.svg`, state, configPath,
  canDownload = true, manualSetup = jsonSetup(), detail, evidence, copyOnly = false, iconFound = false
}) => ({
  id,
  product_id: productId,
  name,
  label,
  target,
  // The agent the row sets up, as the service names it: one id for every host of one agent, and
  // its packaged monogram tile, or the app's own icon when the service found one.
  agent: {
    id: agentId, name,
    symbol: iconFound
      ? { icon: `${AGENT_ICON_DIRECTORY}/${agentId}?v=0123456789ab`, about: `Icon from the installed ${name} app.` }
      : { src: `integrations/${monogram}` }
  },
  state,
  detail: detail ?? harnessDetail[state],
  ...(evidence ? { evidence } : {}),
  can_install: ["available", "updatable", "not_detected"].includes(state) && !copyOnly,
  can_uninstall: state === "installed",
  can_download: canDownload,
  can_locate: true,
  copy_only: copyOnly,
  config_path: configPath,
  connector_command: connectorCommand,
  manual_setup: manualSetup
});

// This is the complete fixed registry, deliberately shuffled, with every state an agent can be in:
// connected (Codex and Claude Code in use), found, found but copy-only (goose), needing an update
// (Cursor), needing attention with and without evidence, and not on this computer (Continue is
// copy-only). The rail must form its groups without inheriting registry or fixture order.
const harnesses = [
  previewHarness({ id: "qwen-code", name: "Qwen Code", target: "CLI", state: "not_detected", configPath: "/home/test/.qwen/settings.json" }),
  previewHarness({ id: "cline-vscode", agentId: "cline", productId: "cline", name: "Cline", label: "Cline (Visual Studio Code)", target: "Visual Studio Code", monogram: "cline.svg", state: "available", configPath: "/home/test/.config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" }),
  previewHarness({
    id: "claude-desktop", name: "Claude Desktop", target: "User", state: "needs_attention",
    configPath: "/home/test/.config/Claude/claude_desktop_config.json", canDownload: false,
    detail: "This configuration could not be parsed; it was left untouched."
  }),
  previewHarness({ id: "windsurf", name: "Windsurf", target: "User", state: "available", configPath: "/home/test/.codeium/windsurf/mcp_config.json" }),
  previewHarness({ id: "devin", productId: "windsurf", name: "Devin", target: "Devin user", monogram: "windsurf.svg", state: "installed", configPath: "/home/test/.config/devin/mcp_config.json" }),
  previewHarness({ id: "crush", name: "Crush", target: "User", state: "not_detected", configPath: "/home/test/.config/crush/crush.json", manualSetup: jsonSetup("mcp") }),
  previewHarness({ id: "codex", name: "Codex", target: "User", state: "installed", configPath: "/home/test/.codex/config.toml", manualSetup: `[mcp_servers.ghostlight]\ncommand = "${connectorPath}"\nargs = ["mcp"]\n`, iconFound: true }),
  previewHarness({ id: "continue", name: "Continue", target: "CLI and IDE", state: "not_detected", copyOnly: true, configPath: "/home/test/.continue/config.yaml", manualSetup: `name: Local Config\nversion: 1.0.0\nschema: v1\nmcpServers:\n  - name: Ghostlight\n    command: "${connectorPath}"\n    args: ["mcp"]\n` }),
  previewHarness({ id: "antigravity", name: "Antigravity", target: "CLI", state: "not_detected", configPath: "/home/test/.gemini/config/mcp_config.json" }),
  previewHarness({
    id: "cline-cursor", agentId: "cline", productId: "cline", name: "Cline", label: "Cline (Cursor)", target: "Cursor", monogram: "cline.svg",
    state: "needs_attention",
    configPath: "/home/test/.config/Cursor/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json",
    evidence: `Found another program, \`npx\` with 2 arguments, where Ghostlight's entry belongs; Ghostlight maintains \`${connectorPath}\` there and changed nothing.`
  }),
  previewHarness({ id: "cursor", name: "Cursor", target: "User", state: "updatable", configPath: "/home/test/.cursor/mcp.json" }),
  previewHarness({ id: "zcode", name: "ZCode", target: "User", state: "installed", configPath: "/home/test/.zcode/cli/config.json" }),
  previewHarness({ id: "zed", name: "Zed", target: "User", state: "installed", configPath: "/home/test/.config/zed/settings.json", manualSetup: jsonSetup("context_servers") }),
  previewHarness({ id: "goose", name: "goose", target: "CLI and desktop", state: "available", copyOnly: true, configPath: "/home/test/.config/goose/config.yaml", manualSetup: `extensions:\n  ghostlight:\n    type: stdio\n    name: ghostlight\n    display_name: Ghostlight\n    enabled: true\n    cmd: "${connectorPath}"\n    args: ["mcp"]\n    envs: {}\n    timeout: 300\n` }),
  previewHarness({ id: "copilot-cli", name: "GitHub Copilot CLI", target: "CLI", state: "available", configPath: "/home/test/.copilot/mcp-config.json" }),
  previewHarness({ id: "cline-cli", agentId: "cline", productId: "cline", name: "Cline", label: "Cline (CLI)", target: "CLI", monogram: "cline.svg", state: "installed", configPath: "/home/test/.cline/data/settings/cline_mcp_settings.json" }),
  previewHarness({
    id: "junie", name: "Junie", target: "CLI and JetBrains", state: "needs_attention",
    configPath: "/home/test/.junie/mcp/mcp.json",
    evidence: `Found an entry without a usable command where Ghostlight's entry belongs; Ghostlight maintains \`${connectorPath}\` there and changed nothing.`
  }),
  previewHarness({ id: "opencode", name: "OpenCode", target: "User", state: "not_detected", configPath: "/home/test/.config/opencode/opencode.json", manualSetup: jsonSetup("mcp") }),
  previewHarness({ id: "kiro", name: "Kiro", target: "CLI and IDE", state: "available", configPath: "/home/test/.kiro/settings/mcp.json" }),
  previewHarness({
    id: "kilo-code", name: "Kilo Code", target: "CLI", state: "needs_attention",
    configPath: "/home/test/.config/kilo/kilo.json", manualSetup: jsonSetup("mcp"),
    evidence: `Found another program, \`alien-cli\` with 1 argument, where Ghostlight's entry belongs; Ghostlight maintains \`${connectorPath}\` there and changed nothing.`
  }),
  previewHarness({ id: "claude-code", name: "Claude Code", target: "User", state: "installed", configPath: "/home/test/.claude.json" }),
  previewHarness({ id: "cline-windsurf", agentId: "cline", productId: "cline", name: "Cline", label: "Cline (Windsurf)", target: "Windsurf", monogram: "cline.svg", state: "not_detected", configPath: "/home/test/.config/Windsurf/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" }),
  previewHarness({ id: "vscode", name: "Visual Studio Code", target: "User", state: "available", configPath: "/home/test/.config/Code/User/mcp.json", manualSetup: jsonSetup("servers") })
];

/** The command line and an agent nothing names, the catalog's two reserved entries. */
const COMMAND_LINE_AGENT = { id: "command_line", name: "Command line", symbol: { glyph: "command_line" } };
const UNKNOWN_AGENT = { id: "unknown", name: "Unknown agent", symbol: { glyph: "unknown" } };

/**
 * The agent the service names by `name`: the roster's agent of that name, the command line, or an
 * agent Ghostlight does not know, identified by the name it reported.
 */
const agentNamed = (name) => (name === COMMAND_LINE_AGENT.name ? COMMAND_LINE_AGENT
  : harnesses.find((harness) => harness.name === name)?.agent ?? { ...UNKNOWN_AGENT, id: `${UNKNOWN_AGENT.id}:${name}`, name });

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

// The setup part of a computer whose setup finished earlier: the window shows the workbench.
const setupFixture = {
  stage: "retired", eyebrow: "", headline: { lead: "", accent: "", trail: "" }, lead: null,
  steps: [
    { name: "This computer", state: "done", word: "Installed", sr: "This computer, done" },
    { name: "Your browser", state: "done", word: "Connected", sr: "Your browser, done" },
    { name: "Your agent", state: "done", word: "Connected", sr: "Your agent, done" }
  ],
  light: "lit", code: null, rows_label: null, rows: [], extra_code: null, live: null,
  connected_line: null, tagline: null
};

const setupStates = { retired: setupFixture, ...stagesOfSetup };

// The controls part Settings is drawn from, as the window sees it: every act available, watching on.
const controlsFixture = {
  runtime: {
    label: "Browser work", state: "active", word: "Running", note: null,
    acts: Object.fromEntries(["hold", "resume", "start_session", "end_session"].map((act) => [act, { available: true, why_not: null }]))
  },
  viewing: {
    label: "Others can watch", watch: true, url: "http://127.0.0.1:41234/",
    word: "On -- anyone on this computer can open this view",
    acts: { set_watching: { available: true, why_not: null } }
  },
  diagnostics: {
    label: "Process diagnostics", on: false, layer: "off", used_bytes: 0, folder: "~/.ghostlight/logs", word: "Off",
    acts: { set_diagnostics: { available: true, why_not: null }, reveal_diagnostics: { available: true, why_not: null } }
  },
  notifications: {
    label: "Notifications", word: "Send one local checkup notification. Nothing leaves this device.",
    acts: { test_notification: { available: true, why_not: null } }
  },
  service: { word: "Ghostlight service is up; 3 MCP clients and 2 browsers connected." },
  you: { context: "control", word: "Full control", sentence: "Others on this computer can watch this view in a browser." }
};

/** What durable evidence says of an MCP connection, which keeps no claim (`language::provenance`). */
const DURABLE_MCP_EXPLANATION = "The application name is the MCP client's own claim. Ghostlight does not verify it.";

/** What a record from before connection identities says of its connection. */
const LEGACY_EXPLANATION = "Earlier history did not establish the connecting executable.";

/**
 * The connection evidence a preview record carries, as the service shapes it: a live claim, or
 * none for durable evidence restored from history.
 */
function previewEvidence(connection_id, reported_application, channel) {
  const explanation = channel === "cli" ? "A ghostlight call session is a local program."
    : reported_application === null ? DURABLE_MCP_EXPLANATION
      : `"${reported_application}" is what the agent calls itself; Ghostlight does not verify it.`;
  return { connection_id, reported_application, channel, explanation };
}

const snapshot = {
  seq: 0,
  generated_at_ms: Date.now(),
  service: { version: "1.0.0", started_at_ms: Date.now() - 540000, runtime_state: "active" },
  overview: { active_sessions: 2, active_operations: 2, connected_browsers: 2, blocked_in_history: 1 },
  sessions: [
    { id: "workspace_codex", agent: agentNamed("Codex"), channel: "mcp", leased: true, tab_count: 3, held_tab_count: 0, active_operations: 1, browser: "Chrome" },
    // Codex starts a session per request, so its card holds more lines than it shows.
    ...[2, 3, 4, 5, 6].map((at) => ({ id: `workspace_codex_${at}`, agent: agentNamed("Codex"), channel: "mcp", leased: false, tab_count: 0, held_tab_count: 0, active_operations: 0 })),
    { id: "workspace_claude", agent: agentNamed("Claude Code"), channel: "mcp", leased: true, tab_count: 1, held_tab_count: 0, active_operations: 1, browser: "Edge" },
    { id: "workspace_script", agent: agentNamed("Command line"), channel: "cli", leased: false, tab_count: 1, held_tab_count: 0, active_operations: 0, browser: "Chrome" }
  ],
  operations: [
    { invocation: "invocation_read", workspace: "workspace_codex", tool: "browser_read", activity: "Reading page", capability: "read", started_at_ms: Date.now() - 12000, phase: "running" },
    { invocation: "invocation_fill", workspace: "workspace_claude", tool: "browser_fill_form", activity: "Filling form", capability: "write", started_at_ms: Date.now() - 35000, phase: "held" }
  ],
  // The rows the service says are in use: a live session of the agent with its entry in place.
  harnesses_in_use: ["codex", "claude-code"],
  browsers: [
    { id: "browser_chrome", family: "Chrome", adapter_version: "1.0.0", connected: true, tab_count: 4 },
    { id: "browser_edge", family: "Edge", adapter_version: "1.0.0", connected: true, tab_count: 1 }
  ],
  // The activity list names each agent, marks the command line, and shows no browser for a call
  // that never reached one; a restored record whose agent the service cannot name reads "Unknown agent".
  history: [
    { timestamp_ms: Date.now() - 60000, invocation: "invocation_disconnected", workspace: "workspace_claude", tool: "browser_press_key", capability: "action", allowed: true, reason: "browser_disconnected", status: "failed", effect: "none", summary: "The browser this session works in is not connected. Nothing was sent.", duration_ms: 40, observed: {}, channel: "mcp", provenance: previewEvidence("connection_claude", "claude-code", "mcp") },
    { timestamp_ms: Date.now() - 90000, invocation: "invocation_blocked", workspace: "workspace_codex", tool: "browser_tabs", capability: "action", allowed: false, reason: "tab_close_denied", status: "blocked", effect: "none", summary: "Authority blocked the browser job.", duration_ms: 120, observed: {}, channel: "mcp", browser: "Chrome", provenance: previewEvidence("connection_codex", "codex-mcp-client", "mcp") },
    { timestamp_ms: Date.now() - 240000, invocation: "invocation_open", workspace: "workspace_script", tool: "browser_navigate", capability: "action", allowed: true, reason: "permitted", status: "succeeded", effect: "committed", summary: "Opened slow.example.org.", duration_ms: 8100, observed: { host: "slow.example.org", readiness: "loading" }, channel: "cli", browser: "Chrome", provenance: previewEvidence("connection_script", null, "cli") },
    { timestamp_ms: Date.now() - 420000, invocation: "invocation_restored", workspace: "workspace_0193f1c2a8b4e7d6825", tool: "policy_explain", capability: "read", allowed: true, reason: "permitted", status: "succeeded", effect: "none", summary: "Explained the policy.", duration_ms: 12, observed: {}, channel: "mcp", provenance: previewEvidence("connection_restored", null, "mcp") }
  ],
  diagnostics: [
    { id: "service", label: "Orchestrator", severity: "passing", detail: "Ghostlight is accepting local connections." },
    { id: "browser", label: "Browser adapters", severity: "passing", detail: "Compatible browser adapters are connected." },
    { id: "authority", label: "Authority", severity: "passing", detail: "Configured authority sources are valid." }
  ],
  harnesses,
  controls: controlsFixture,
  setup: setupFixture,
  configuration: {
    runtime_state: "active",
    local_policy_configured: true,
    local_policy_active: true,
    local_policy_valid: true,
    managed_authority_configured: false,
    managed_authority_active: false,
    managed_authority_valid: true,
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
      kind: "organization", title: "Northwind", policy_name: "Support workspace", version: "2026-08-14",
      rules: [{ id: "support-sites", description: "Ordinary support work", allow: ["support.northwind.example", "*.support.northwind.example"], deny: ["admin.support.northwind.example"], allowed: ["read", "action"], note: null }],
      settings: [{ key: "browser.tabs.allow_close", value: "false", level: "mandatory" }],
      path: null,
      document: '{\n  "schema": 4,\n  "name": "Support workspace",\n  "version": "2026-08-14"\n}'
    },
    {
      kind: "user", title: "Your rules", policy_name: "Your rules", version: "2026-08-14",
      rules: [
        { id: "rule-1", description: null, allow: ["*.support.northwind.example"], deny: [], allowed: ["read", "action"], note: null },
        { id: "rule-2", description: "Left over from last week", allow: ["one.support.northwind.example"], deny: [], allowed: ["read"], note: "unreachable" }
      ],
      settings: [],
      path: "C:/Users/you/AppData/Local/Ghostlight/user-policy.json",
      document: '{\n  "schema": 4,\n  "name": "Your rules",\n  "version": "2026-08-14"\n}'
    }
  ],
  ceilings: [
    "Anything that is not an ordinary http or https address.",
    "Ghostlight's own control page on this computer is never automated."
  ],
  user_layer: {
    source: "workbench",
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

// A stable H4 scenario exercises the real bundled view with bounded synthetic receipts.
const historyScenario = process.env.GHOSTLIGHT_PREVIEW_SCENARIO === "h4";
if (historyScenario) {
  const permission = (allowed, capability) => ({ checks: [{ requirements: capability.split(" + "), host: "example.com", allowed, reason: allowed ? "permitted" : "capability_denied", layers: [{ tier: "managed", grants: ["browsing"], allowed }] }], truncated: false });
  const child = (tool, summary, status, effect, capability) => ({ invocation: "history_example", workspace: "workspace_codex", tool, summary, status, effect, capability, allowed: status === "succeeded", complete: true, duration_ms: 120, observed: {}, permissions: permission(status === "succeeded", capability), permission_explanations: [status === "succeeded" ? "Allowed: organization policy permits this work (browsing)." : "Refused: organization policy refuses this work (browsing)."] });
  snapshot.operations = [];
  snapshot.history.unshift({ invocation: "history_example", workspace: "workspace_codex", tool: "browser_flow", capability: "", allowed: false, reason: "capability_denied", status: "blocked", effect: "partial", summary: "Completed 2 of 4 steps. Step 3 blocked by policy.", complete: true, timestamp_ms: Date.now(), duration_ms: 920, observed: {}, steps: [
    { position: 1, state: "recorded", tool: "browser_navigate", record: child("browser_navigate", "Opened example.com.", "succeeded", "applied", "read") },
    { position: 2, state: "recorded", tool: "browser_fill_form", record: child("browser_fill_form", "Filled 3 fields on example.com.", "succeeded", "applied", "read + write") },
    { position: 3, state: "recorded", tool: "browser_execute", record: child("browser_execute", "Policy blocked JavaScript execution on example.com.", "blocked", "none", "execute") },
    { position: 4, state: "not_run", tool: "browser_read", record: null }
  ] });
}

// C1 fixtures carry connection evidence: the name each agent's client reported for itself.
const REPORTED = { "Codex": "codex-mcp-client", "Claude Code": "claude-code", "Command line": "ghostlight call" };
for (const session of snapshot.sessions) {
  const reported = REPORTED[session.agent.name];
  session.connections = [{
    connection_id: `connection_${session.id}`, reported_application: reported, channel: session.channel,
    explanation: session.channel === "cli"
      ? "A ghostlight call session is a local program. Ghostlight does not verify the scripts and instructions it runs."
      : `"${reported}" is what the agent calls itself; Ghostlight does not verify it.`
  }];
}

/**
 * The agent the service names for work from the work's own evidence, never from its session
 * (`Agent::of_evidence`): the command line from its channel, the agent a live claim names, and
 * the unknown agent for evidence that keeps no claim, as a record restored from history does.
 */
const agentOfEvidence = (evidence) => {
  if (evidence?.channel === "cli") return COMMAND_LINE_AGENT;
  const claim = evidence?.reported_application;
  if (!claim) return UNKNOWN_AGENT;
  return agentNamed(Object.keys(REPORTED).find((name) => REPORTED[name] === claim) ?? claim);
};

/**
 * Give a record the evidence the service keeps beside it: its own when it has some, the live
 * session's connection for work recorded in this run, and for a restored record with none only
 * what its channel says. Its agent comes from that evidence.
 */
const retainConnection = (record) => {
  if (record.provenance === undefined) {
    const session = snapshot.sessions.find((item) => item.id === record.workspace);
    record.provenance = session ? structuredClone(session.connections[0])
      : record.channel ? { connection_id: null, reported_application: null, channel: record.channel, explanation: LEGACY_EXPLANATION }
        : null;
  }
  record.agent = structuredClone(agentOfEvidence(record.provenance));
  for (const step of record.steps ?? []) if (step.record) retainConnection(step.record);
};
snapshot.operations.forEach(retainConnection);
snapshot.history.forEach(retainConnection);

// `?browsers=none` shows At a glance with no browser connected, as the service words it when
// Chrome would start on request.
const withoutBrowsers = (flat) => ({ ...flat, browsers: [], no_browser: "Starts Google Chrome on request" });

// `?agents=connected` shows Integrations when every agent found on this computer has Ghostlight.
const allConnected = (flat) => ({
  ...flat,
  harnesses: flat.harnesses.map((harness) => (harness.state === "not_detected" ? harness : {
    ...harness, state: "installed", detail: harnessDetail.installed, evidence: undefined,
    can_install: false, can_uninstall: true
  }))
});

const fixtureFor = (stage, noBrowsers, connected) => `window.__GHOSTLIGHT_PREVIEW__ = ${JSON.stringify((connected ? allConnected : (flat) => flat)((noBrowsers ? withoutBrowsers : (flat) => flat)({ ...snapshot, setup: setupStates[stage === "done" ? "agent" : stage] })))};
window.__GHOSTLIGHT_SETUP_STATES__ = ${JSON.stringify(setupStates)};
window.__GHOSTLIGHT_SETUP_START__ = ${JSON.stringify(stage)};
window.__GHOSTLIGHT_SCRIPT__ = ${JSON.stringify(script)};
(() => {
  const preview = window.__GHOSTLIGHT_PREVIEW__;
  const listeners = [];
  let counter = 0;
  let step = 0;

  // The service's snapshot holds a settled or composed record once; a later snapshot must too.
  const foldHistory = record => {
    const index = preview.history.findIndex(item => item.invocation === record.invocation);
    if (index >= 0) preview.history[index] = record;
    else preview.history.unshift(record);
  };

  const publish = change => {
    if (change.kind === "operation_settled" || change.kind === "composition_changed") foldHistory(change.record);
    preview.seq += 1;
    const event = { seq: preview.seq, change };
    for (const handler of listeners) handler({ payload: event });
  };

  // The sheet follows the service: a change to the runtime, watching or diagnostics is pushed whole.
  const publishControls = () => {
    const controls = preview.controls;
    controls.runtime.state = preview.service.runtime_state;
    controls.runtime.word = { active: "Running", held: "Paused", ended: "Session ended" }[controls.runtime.state];
    controls.viewing.word = controls.viewing.watch
      ? "On -- anyone on this computer can open this view"
      : "Off -- only this window shows Ghostlight";
    controls.you.sentence = controls.viewing.watch
      ? "Others on this computer can watch this view in a browser."
      : "Watching is off: only this window shows Ghostlight.";
    controls.diagnostics.word = controls.diagnostics.on ? "On (" + controls.diagnostics.layer + ")" : "Off";
    publish({ kind: "controls_changed", controls: JSON.parse(JSON.stringify(controls)) });
  };

  // The setup view: show a stage, and tell the page as the service would. The preview plays the
  // journey a person has: Add or Open starts a wait and the browser connects a few seconds later,
  // and Connect turns an agent's row into the restart line.
  const setupStates = window.__GHOSTLIGHT_SETUP_STATES__;
  let setupTurn = 0;
  const setSetup = stage => {
    preview.setup = JSON.parse(JSON.stringify(setupStates[stage]));
    publish({ kind: "part_changed", part: "setup", etag: stage + ":" + (setupTurn += 1) });
  };
  window.__GHOSTLIGHT_SETUP__ = setSetup;
  if (window.__GHOSTLIGHT_SETUP_START__ === "done") setTimeout(() => setSetup("done"), 3000);

  const runOne = () => {
    if (preview.service.runtime_state !== "active") return;
    counter += 1;
    const blocked = counter % 9 === 0;
    const spec = blocked
      ? { tool: "browser_execute", activity: "Running JavaScript", capability: "execute", ms: 900, effect: "none" }
      : window.__GHOSTLIGHT_SCRIPT__[step++ % window.__GHOSTLIGHT_SCRIPT__.length];
    const session = preview.sessions[counter % preview.sessions.length];
    const operation = {
      invocation: "invocation_preview_" + counter,
      workspace: session.id,
      tool: spec.tool,
      activity: spec.activity,
      capability: spec.capability,
      started_at_ms: Date.now(),
      phase: "running",
      agent: session.agent
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
        // The record carries the intake and the agent of the session that ran it, as the service's do.
        channel: session.channel,
        agent: session.agent,
        provenance: {
          connection_id: "connection_" + session.id,
          reported_application: session.channel === "cli" ? null : session.agent.name,
          channel: session.channel,
          explanation: session.channel === "cli" ? "A ghostlight call session is a local program."
            : '"' + session.agent.name + '" is what the agent calls itself; Ghostlight does not verify it.'
        }
      };
      publish({ kind: "operation_settled", record });
      setTimeout(runOne, blocked ? 3000 : 700 + Math.random() * 1300);
    }, spec.ms);
  };

  // What the service answers for each command. The page asks over HTTP, as it does in the window.
  const answer = async (command, args = {}) => {
    if (command === "workbench_context") {
      // The window's own context: every act is available, so the page draws all of it.
      const acts = ["search", "check_policy", "rescan", "continue_in_window", "hold", "resume",
        "start_session", "end_session", "apply_policy", "remove_policy", "set_diagnostics", "set_watching",
        "setup_all", "harness_act", "test_notification", "open", "reveal_diagnostics", "locate", "setup_act", "quit"];
      return {
        context: "control", word: "Full control",
        sentence: "Others on this computer can watch this view in a browser.",
        instance: "preview", opener: "service",
        acts: Object.fromEntries(acts.map((act) => [act, { available: true, why_not: null }])),
        product: { version: "1.0.0", license: "Apache-2.0 OR MIT" }, links: {}
      };
    }
    if (command === "set_diagnostics") {
      preview.controls.diagnostics.on = args.on;
      preview.controls.diagnostics.layer = args.on ? "marker" : "off";
      publishControls();
      return { layer: args.on ? "marker" : "off", directory: null, used_bytes: 0 };
    }
    if (command === "set_watching") {
      preview.controls.viewing.watch = args.watch;
      publishControls();
      return preview.controls;
    }
    if (command === "continue_in_window") return null;
    if (command === "quit_ghostlight") return { message: "Ghostlight is quitting." };
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
    if (command === "apply_user_policy") return { accepted: true, runtime_state: "active", message: "Your rules are applied: Your rules 2026-08-14." };
    if (command === "remove_user_policy") return { accepted: true, runtime_state: "active", message: "Your rules are removed." };
    if (command === "apply_runtime_intent") {
      const state = args.intent === "end_session" ? "ended" : args.intent === "hold" ? "held" : "active";
      preview.service.runtime_state = state;
      preview.configuration.runtime_state = state;
      publish({ kind: "runtime_changed", runtime_state: state });
      publishControls();
      if (state === "active") runOne();
      // Mirrors crates/orchestrator/src/language/control.rs: the confirmation states the
      // authority's state only, never browser delivery.
      const confirmation = {
        active: "Running. Agents can ask for browser work again. Earlier actions are not repeated.",
        held: "Paused. Agents' browser actions will be refused until you resume.",
        ended: "Session ended. Agents' browser actions will be refused until you start a new session."
      };
      return { accepted: true, runtime_state: state, message: confirmation[state] };
    }
    if (command === "refresh_harnesses") return preview.harnesses;
    if (command === "test_notification") return null;
    if (command === "manage_harness") {
      const row = preview.setup.rows.find(each => each.id === args.id);
      if (row) {
        Object.assign(row, { detail: "Restart " + row.name + " to finish.", tone: "attention", action: null });
        publish({ kind: "part_changed", part: "setup", etag: "connect:" + (setupTurn += 1) });
      }
      return { changed: false, summary: {}, message: "Preview action completed." };
    }
    if (command === "setup_act") {
      if (args.kind === "add_in_browser" || args.kind === "open_browser") setTimeout(() => setSetup("agent"), 4000);
      return preview.setup;
    }
    throw new Error("Unknown preview command " + command);
  };

  // The page speaks to its service over loopback HTTP with the key the shell hands to its window.
  // The preview answers the same routes from the fixture above.
  window.GhostlightAccess = Object.freeze({ bearer: "preview" });
  const routes = {
    "GET /api/context": "workbench_context",
    "GET /api/snapshot": "workbench_snapshot",
    "POST /api/search": "workbench_search",
    "GET /api/policy": "workbench_policy",
    "POST /api/policy/preview": "preview_user_policy",
    "PUT /api/policy/user": "apply_user_policy",
    "DELETE /api/policy/user": "remove_user_policy",
    "POST /api/runtime": "apply_runtime_intent",
    "PUT /api/controls/diagnostics": "set_diagnostics",
    "PUT /api/controls/viewing": "set_watching",
    "POST /api/integrations/rescan": "refresh_harnesses",
    "POST /api/integrations/act": "manage_harness",
    "POST /api/setup/act": "setup_act",
    "POST /api/notifications/test": "test_notification",
    "POST /api/window": "continue_in_window",
    "POST /api/quit": "quit_ghostlight"
  };
  const reply = (status, body) => new Response(body === undefined ? "" : JSON.stringify(body), {
    status, headers: { "content-type": "application/json" }
  });
  const partsOf = flat => ({
    status: {
      service: flat.service, readiness: flat.readiness, audit_health: flat.audit_health,
      audit_notice: flat.audit_notice, configuration: flat.configuration,
      ...(flat.runtime_note === undefined ? {} : { runtime_note: flat.runtime_note })
    },
    connections: {
      sessions: flat.sessions, browsers: flat.browsers, harnesses_in_use: flat.harnesses_in_use,
      ...(flat.no_browser ? { no_browser: flat.no_browser } : {})
    },
    activity: { operations: flat.operations, history: flat.history, document_coverage: flat.document_coverage || [] },
    integrations: { harnesses: flat.harnesses },
    diagnostics: { diagnostics: flat.diagnostics },
    controls: flat.controls,
    setup: flat.setup
  });
  const realFetch = window.fetch.bind(window);
  window.fetch = async (path, init = {}) => {
    const url = new URL(path, window.location.href);
    if (!url.pathname.startsWith("/api/")) return realFetch(path, init);
    if (url.pathname === "/api/events") {
      const encoder = new TextEncoder();
      let handler = null;
      const body = new ReadableStream({
        start(controller) {
          const hello = { context: "control", instance: "preview", seq: preview.seq };
          controller.enqueue(encoder.encode("event: hello\\ndata: " + JSON.stringify(hello) + "\\n\\n"));
          handler = event => controller.enqueue(encoder.encode("data: " + JSON.stringify(event.payload) + "\\n\\n"));
          listeners.push(handler);
        },
        cancel() {
          const at = listeners.indexOf(handler);
          if (at >= 0) listeners.splice(at, 1);
        }
      });
      return new Response(body, { status: 200, headers: { "content-type": "text/event-stream" } });
    }
    const command = routes[(init.method || "GET") + " " + url.pathname];
    if (!command) return reply(404, { error: { code: "not_found", message: "There is nothing at this address." } });
    try {
      const result = await answer(command, init.body ? JSON.parse(init.body) : {});
      if (command === "workbench_snapshot") {
        return reply(200, { seq: result.seq, instance: "preview", parts: partsOf(result) });
      }
      return reply(command === "quit_ghostlight" ? 202 : 200, result === null ? {} : result);
    } catch (error) {
      return reply(500, { error: { code: "failed", message: String(error) } });
    }
  };

  if (!${historyScenario}) setTimeout(runOne, 1200);
  window.__GHOSTLIGHT_PUBLISH__ = publish;
})();`;

const types = {
  ".css": "text/css", ".js": "text/javascript", ".png": "image/png", ".svg": "image/svg+xml"
};

const server = createServer(async (request, response) => {
  try {
    const address = new URL(request.url, `http://${request.headers.host}`);
    const pathname = address.pathname;
    const stage = address.searchParams.get("setup") ?? "retired";
    if (!Object.hasOwn(setupStates, stage)) {
      response.writeHead(400, { "content-type": "text/plain" }).end(`setup is one of ${Object.keys(setupStates).join(", ")}`);
      return;
    }
    if (pathname === "/fixture.js") {
      response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
      response.end(fixtureFor(stage, address.searchParams.get("browsers") === "none",
        address.searchParams.get("agents") === "connected"));
      return;
    }
    const name = pathname === "/" ? "index.html" : pathname.slice(1);
    const [directory, iconOf] = name.split("/");
    if (directory === AGENT_ICON_DIRECTORY) {
      const found = harnesses.some((harness) => harness.agent.id === iconOf && harness.agent.symbol.icon);
      if (!found) {
        response.writeHead(404).end();
        return;
      }
      response.writeHead(200, { "content-type": "image/png", "cache-control": "no-cache" });
      response.end(await readFile(agentIconFile));
      return;
    }
    // The setup view shares two of the extension's files, which the service serves under /extension/.
    const shared = { "extension/kit.css": join(extensionFolder, "kit.css"),
      "extension/ghostlight-mascot.png": join(extensionFolder, "icons", "ghostlight-mascot.png") };
    const served = new Set(["index.html", "styles.css", "app.js", "ghostlight.png"]);
    if (!served.has(name) && !Object.hasOwn(shared, name) && !/^lib\/[a-z-]+\.js$/.test(name)
      && !/^kit\/[a-z]+\.(js|css)$/.test(name) && !/^integrations\/[a-z0-9-]+\.svg$/.test(name)) {
      response.writeHead(404).end();
      return;
    }
    let body = await readFile(shared[name] ?? join(ui, name));
    if (name === "index.html") {
      body = Buffer.from(body.toString("utf8").replace(
        '<script src="app.js"></script>',
        `<script src="fixture.js?${address.searchParams}"></script><script src="app.js"></script>`
      ));
    }
    response.writeHead(200, { "content-type": types[extname(name)] || "text/html; charset=utf-8", "cache-control": "no-store" });
    response.end(body);
  } catch (error) {
    response.writeHead(500, { "content-type": "text/plain" });
    response.end(String(error));
  }
});
server.listen(port, "127.0.0.1", () => console.log(`http://127.0.0.1:${server.address().port}`));
