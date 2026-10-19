// The design-review preview: the window's real page served with a synthetic service, so a
// reviewer can look at every state the window draws without a Ghostlight running. No person ever
// sees it; it exists so the window can be judged state by state.
//
// Run it with `node tests/workbench-preview-server.mjs`. GHOSTLIGHT_PREVIEW_PORT picks the port
// (41737 by default, 0 for any free one); the address it serves is printed on start.
// GHOSTLIGHT_PREVIEW_SCENARIO=h4 serves the still history scenario the history journey reads.
//
// Query parameters choose what the page is shown, and combine:
//   ?readiness=working        (the default) Ready, with work running, waiting and starting again;
//                             the band says Working while anything is in flight.
//   ?readiness=ready          Ready and idle: nothing runs and nothing starts.
//   ?readiness=not_connected  No browser connected and none would start.
//   ?readiness=paused         Paused, with the waiting work held. Resume puts it back in flight,
//                             settles it, and starts new work as on the working page.
//   ?readiness=ended          Session ended.
//   ?readiness=channel_off    MCP agents switched off in your rules and the command line on. A
//                             command-line agent is connected, so the band stays Ready and says
//                             which agents can work; Policy and Status name the switch.
//   ?readiness=turned_off     Both channels switched off in your rules: the band says Turned off,
//                             held, and Policy shows nothing available.
//   ?setup=<stage>            The setup view at one of its stages (workbench-setup-fixtures.mjs).
//   ?browsers=none            No browser connected, with Google Chrome starting on request.
//   ?agents=connected         Every agent found on this computer has Ghostlight.
//   ?history=capped           410 recorded actions, more than the window's list holds.
// Every page shows the Policy destination with an organization and your rules, and history with
// succeeded, failed, kept, blocked, switched-off and invalid-call rows. Times are relative to
// page load: work that is running, waiting or held started seconds before the page opened.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
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
// The app icons the preview shows: the tiles the service makes of two small synthetic PNGs, a
// colored one on its light ground and an all-white one on its dark ground. A service test
// (crate::agent::image) makes them again from the sources beside them and compares.
const agentIconFolder = join(repository, "tests", "fixtures", "workbench");
// The packaged monogram tiles, which the service serves at an agent's address when it has no icon.
const monogramFolder = join(repository, "crates", "orchestrator", "src", "agent", "monograms");
const AGENT_ICON_DIRECTORY = "agent-icons";
/** What the service and this preview send with an agent's image (`workbench/web/routes.rs`). */
const IMAGE_POLICY = "default-src 'none'; sandbox";

/** An image as the service serves it at an agent's address: its media type, bytes and version. */
const agentImage = (file, type) => {
  const bytes = readFileSync(file);
  return { type, bytes, version: createHash("sha256").update(bytes).digest("hex").slice(0, 12) };
};

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

/** The image the preview serves at each agent's address, by agent id. */
const agentImages = new Map();

/** A roster agent wearing `image` at its address, and the sentence an app's own icon carries. */
const agentNamedBy = (id, name, image, appIcon) => {
  agentImages.set(id, image);
  const symbol = { icon: `${AGENT_ICON_DIRECTORY}/${id}?v=${image.version}` };
  return { id, name, symbol: appIcon ? { ...symbol, about: `Icon from the installed ${name} app.` } : symbol };
};

const previewHarness = ({
  id, agentId = id, productId = id, name, label = name, target, monogram = `${id}.svg`, state, configPath,
  canDownload = true, manualSetup = jsonSetup(), detail, evidence, copyOnly = false, appIcon
}) => ({
  id,
  product_id: productId,
  name,
  label,
  target,
  // The agent the row sets up, as the service names it: one id for every host of one agent, and
  // its one image address, which serves the app's own icon on its tile when the service found one
  // (and then says where it came from) and its packaged monogram otherwise.
  agent: agentNamedBy(agentId, name, appIcon
    ? agentImage(join(agentIconFolder, appIcon), "image/png")
    : agentImage(join(monogramFolder, monogram), "image/svg+xml"), Boolean(appIcon)),
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
  previewHarness({ id: "codex", name: "Codex", target: "User", state: "installed", configPath: "/home/test/.codex/config.toml", manualSetup: `[mcp_servers.ghostlight]\ncommand = "${connectorPath}"\nargs = ["mcp"]\n`, appIcon: "agent-icon-tile.png" }),
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
  previewHarness({ id: "claude-code", name: "Claude Code", target: "User", state: "installed", configPath: "/home/test/.claude.json", appIcon: "agent-icon-white-tile.png" }),
  previewHarness({ id: "cline-windsurf", agentId: "cline", productId: "cline", name: "Cline", label: "Cline (Windsurf)", target: "Windsurf", monogram: "cline.svg", state: "not_detected", configPath: "/home/test/.config/Windsurf/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json" }),
  previewHarness({ id: "vscode", name: "Visual Studio Code", target: "User", state: "available", configPath: "/home/test/.config/Code/User/mcp.json", manualSetup: jsonSetup("servers") })
];

/**
 * The command line, an agent nothing names, and the earlier session of work read back from an
 * earlier run whose record kept no agent: the catalog's reserved entries.
 */
const COMMAND_LINE_AGENT = { id: "command_line", name: "Command line", symbol: { glyph: "command_line" } };
const UNKNOWN_AGENT = { id: "unknown", name: "Unknown agent", symbol: { glyph: "unknown" } };
const EARLIER_AGENT = {
  id: "earlier", name: "Earlier session", symbol: { glyph: "unknown" }, note: "Recorded in an earlier session. Ghostlight kept no agent name for it."
};

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

// The acts each runtime state accepts, in the order governance::offers names them.
const OFFERS = Object.freeze({ active: ["hold", "end_session"], held: ["resume", "end_session"], ended: ["start_session"] });

// The controls part Settings is drawn from, as the window sees it: every act available, watching on.
const controlsFixture = {
  runtime: {
    label: "Browser work", state: "active", word: "Running", note: null, offers: OFFERS.active,
    acts: Object.fromEntries(["hold", "resume", "start_session", "end_session"].map((act) => [act, { available: true, why_not: null }]))
  },
  viewing: {
    label: "Others can watch", watch: true, url: "http://127.0.0.1:41234/",
    word: "On: anyone on this computer can open this view",
    acts: { set_watching: { available: true, why_not: null } }
  },
  diagnostics: {
    label: "Process diagnostics", on: false, layer: "off", used_bytes: 0, folder: "~/.ghostlight/logs", word: "Off",
    acts: { set_diagnostics: { available: true, why_not: null }, reveal_diagnostics: { available: true, why_not: null } }
  },
  notifications: {
    label: "Notifications", state: "On", word: "Send one local checkup notification. Nothing leaves this device.",
    acts: { test_notification: { available: true, why_not: null } }
  },
  browser: {
    set_in: "Set in the browser",
    effects: { label: "Agent activity effects", word: "The cursor, click ripples and scan-lines on a page an agent works in. They start on." },
    captions: { label: "Action captions", word: "A caption at the bottom of the page names each action. They start off." }
  },
  // Said by `summarized` from the sessions and browsers each page shows.
  service: { word: null },
  you: { context: "control", word: "Shared", sentence: "Others on this computer can watch this view in a browser." }
};

/** What an MCP connection a claim named, seen with no source, establishes (`language::provenance`). */
const CLAIM_CAVEAT = "The agent's name is its own claim. Ghostlight does not verify it.";

/** Whether the name a client reported says more than the agent it names (`language::provenance`). */
const plain = (text) => text.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
const saysMore = (reported, agent) => agent.id === UNKNOWN_AGENT.id || plain(reported) !== plain(agent.name);

/** What Ghostlight cannot vouch for in any command-line session. */
const CLI_SCRIPTS_UNVERIFIED = "Ghostlight does not verify the scripts and instructions it runs.";

/** What a record from before connection identities says of its connection. */
const LEGACY_EXPLANATION = "Earlier history did not establish the connecting executable.";

/** The name each preview agent's client reports for itself. */
const REPORTED = { "Codex": "codex-mcp-client", "Claude Code": "claude-code", "Command line": "ghostlight call" };

/** The roster agent whose catalog id is `id`, or none. */
const catalogAgent = (id) => harnesses.find((harness) => harness.agent.id === id)?.agent ?? null;

/**
 * The connection evidence a preview record carries and the agent it names, as
 * `language::provenance::details` makes them from what the record kept (its connection id, its
 * channel, the claimed catalog agent and the source Ghostlight saw start it) and the name its
 * client `reported`, which only a live connection has. The command line names its own agent. On
 * MCP a live claim names the agent, else the kept claimed agent, else the source's catalog agent,
 * else the unknown agent, or the earlier session for a record with no source. Only live words are
 * quoted, and only where they say more than the agent's name. No preview source is an app the
 * claimed agent runs inside, so the fixture leaves out that sentence.
 */
function previewDetails({ connection_id = null, channel, claimed_agent = null, source = null }, reported = null) {
  const sourceName = source && (catalogAgent(source.agent)?.name ?? source.image);
  let agent;
  let explanation;
  if (channel === "cli") {
    agent = COMMAND_LINE_AGENT;
    explanation = source ? `A ghostlight call from ${sourceName}. ${CLI_SCRIPTS_UNVERIFIED}`
      : `A ghostlight call session is a local program. ${CLI_SCRIPTS_UNVERIFIED}`;
  } else {
    const claim = reported ? agentNamed(Object.keys(REPORTED).find((name) => REPORTED[name] === reported) ?? reported)
      : catalogAgent(claimed_agent);
    agent = claim ?? catalogAgent(source?.agent) ?? (reported !== null || source ? UNKNOWN_AGENT : EARLIER_AGENT);
    const quoted = reported && saysMore(reported, agent) && `"${reported}" is what the agent calls itself`;
    explanation = !source ? (quoted ? `${quoted}; Ghostlight does not verify it.` : claim ? CLAIM_CAVEAT : "")
      : source.agent === agent.id ? `Started by ${sourceName}.`
        : quoted ? `${quoted}. Started by ${sourceName}.`
          : claim ? `${CLAIM_CAVEAT} Started by ${sourceName}.` : `Started by ${sourceName}.`;
  }
  return {
    provenance: {
      connection_id, reported_application: reported || null, channel,
      explanation: connection_id === null && explanation ? LEGACY_EXPLANATION : explanation
    },
    agent
  };
}

/**
 * The moment every fixture time is written against. The page moves each time forward by however
 * long ago this was, so the fixture reads as if it started just before the page opened.
 */
const FIXTURE_EPOCH = Date.now();
const ago = (ms) => FIXTURE_EPOCH - ms;

/**
 * The service's readiness answer, mirrored from `language/readiness.rs`: each state's word,
 * detail and tone. The page resolves the state from the fixture's facts in the service's order.
 */
const READINESS = {
  session_ended: { word: "Session ended", detail: "This session was ended. Start a new session to let agents work again.", tone: "held" },
  paused: { word: "Paused", detail: "Browser work is paused. Resume when you are ready.", tone: "held" },
  turned_off: { word: "Turned off", detail: "A policy turns off work from the agents that are connected. Open Policy to see the switches.", tone: "held" },
  not_connected: { word: "Not connected", detail: "No browser is connected. Open a supported Chromium browser with the Ghostlight extension installed.", tone: "offline" },
  policy_invalid: { word: "Policy not valid", detail: "A configured policy is not valid, so Ghostlight refuses all browser work. Open Policy to see what applies. A policy file still in schema 3 needs ghostlight policy migrate <file>.", tone: "attention" },
  working: { word: "Working", detail: "An agent is working in your browser right now.", tone: "working" },
  ready: { word: "Ready", detail: "Connected and idle. Agents can work when they ask.", tone: "quiet" }
};

/**
 * What the service says about the switches each `?readiness=` page turns off, mirrored from
 * `language/channel_switch.rs`: which switches your rules turn off, the band's sentence while
 * another channel can work (`ready`) and while none of the connected agents can (`held`), and
 * the line the Policy headline and the Status rules check add (`policy`).
 */
const CHANNELS_OFF = {
  channel_off: {
    mcp: "user",
    ready: "MCP agents are turned off; command-line agents can work.",
    held: "Your policy turns off work from MCP agents.",
    policy: "MCP agents are turned off. Command-line work may continue.",
    capability: "Only command-line work may use it: MCP agents are turned off."
  },
  turned_off: {
    mcp: "user",
    cli: "user",
    held: "Your policy turns off work from MCP agents and the command line.",
    policy: "MCP agents and the command line are turned off, so no agent may work."
  }
};

/** What the window shows with no browser connected and none starting (`language::workbench`). */
const NO_BROWSER = "No browser connected";

/** The browser the `?browsers=none` page starts on request. */
const STARTS_ON_REQUEST = "Google Chrome";

/**
 * What the Status Browser check says with no browser connected, mirrored from
 * `language::workbench::STATUS_BROWSER_WAITING` and `language::readiness::starts_when_asked`.
 */
const BROWSER_WAITING = "Waiting for Ghostlight in Browser.";
const startsWhenAsked = (browser) => `Starts ${browser} when an agent asks.`;

/** What a waiting operation's activity says (`language::outcome::WAITING_FOR_WORK`). */
const WAITING_FOR_WORK = "Waiting for earlier browser work";

// The organization as the service names it, in the status part and the compiled policy alike.
const northwind = {
  identity: {
    name: "Northwind",
    statement: "Browser work stays inside approved support sites.",
    url: "https://northwind.example/browser-policy",
    contacts: [{ kind: "email", value: "security@northwind.example", label: "Security team" }]
  },
  title: "Northwind",
  in_sentence: "Northwind"
};

const snapshot = {
  seq: 0,
  generated_at_ms: FIXTURE_EPOCH,
  service: { version: "1.0.0", started_at_ms: ago(540000), runtime_state: "active" },
  overview: { active_sessions: 2, active_operations: 2, connected_browsers: 2, blocked_in_history: 1 },
  sessions: [
    { id: "workspace_codex", agent: agentNamed("Codex"), channel: "mcp", tab_count: 3, browser: "Chrome" },
    // Codex starts a session per request, so its card holds more lines than it shows.
    ...[2, 3, 4, 5, 6].map((at) => ({ id: `workspace_codex_${at}`, agent: agentNamed("Codex"), channel: "mcp", tab_count: 0 })),
    { id: "workspace_claude", agent: agentNamed("Claude Code"), channel: "mcp", tab_count: 1, browser: "Edge" },
    { id: "workspace_script", agent: agentNamed("Command line"), channel: "cli", tab_count: 1, browser: "Chrome" }
  ],
  operations: [
    { invocation: "invocation_read", workspace: "workspace_codex", tool: "browser_read", activity: "Reading page", capability: "read", started_at_ms: ago(12000), phase: "running" },
    { invocation: "invocation_fill", workspace: "workspace_claude", tool: "browser_fill_form", activity: WAITING_FOR_WORK, capability: "write", started_at_ms: ago(35000), phase: "waiting" }
  ],
  // The rows the service says are in use: a live session of the agent with its entry in place.
  harnesses_in_use: ["codex", "claude-code"],
  browsers: [
    { id: "browser_chrome", family: "Chrome", adapter_version: "1.0.0", connected: true, tab_count: 4 },
    { id: "browser_edge", family: "Edge", adapter_version: "1.0.0", connected: true, tab_count: 1 }
  ],
  // The activity list names each agent, marks the command line, and shows no browser for a call
  // that never reached one. A restored record is named for the agent it kept as claimed, and one
  // that kept no claim and no source reads "Unknown agent".
  history: [
    { timestamp_ms: ago(60000), invocation: "invocation_disconnected", workspace: "workspace_claude", tool: "browser_press_key", capability: "action", allowed: true, reason: "browser_disconnected", status: "failed", outcome: "failed", effect: "none", summary: "The browser this session works in is not connected. Nothing was sent.", duration_ms: 40, observed: {}, channel: "mcp", ...previewDetails({ connection_id: "connection_claude", channel: "mcp" }, "claude-code") },
    { timestamp_ms: ago(90000), invocation: "invocation_blocked", workspace: "workspace_codex", tool: "browser_tabs", capability: "action", allowed: false, reason: "tab_close_denied", status: "blocked", outcome: "refused", effect: "none", summary: "Authority blocked the browser job.", duration_ms: 120, observed: {}, channel: "mcp", browser: "Chrome", ...previewDetails({ connection_id: "connection_codex", channel: "mcp" }, "codex-mcp-client") },
    // A tab the browser's Preserve Ghostlight tabs setting kept open: refused, but not by policy.
    { timestamp_ms: ago(120000), invocation: "invocation_kept", workspace: "workspace_codex", tool: "browser_tabs", capability: "action", allowed: true, reason: "browser_local_interlock", status: "blocked", outcome: "kept", effect: "none", summary: "Kept the tab open: Preserve Ghostlight tabs is on. To let agents close tabs, turn it off in the Ghostlight in Browser settings.", duration_ms: 60, observed: {}, channel: "mcp", browser: "Chrome", ...previewDetails({ connection_id: "connection_codex", channel: "mcp" }, "codex-mcp-client") },
    // Work from a channel your rules switched off, and a call that does not match the catalog.
    { timestamp_ms: ago(150000), invocation: "invocation_channel_off", workspace: "workspace_claude", tool: "browser_read", capability: "read", allowed: false, reason: "channel_denied", policy_tier: "user", status: "blocked", outcome: "refused", effect: "none", summary: "Your policy turns off work from MCP agents.", permission_explanations: ["Refused. Your policy turns off work from MCP agents."], permissions: { checks: [{ requirements: [], host: null, allowed: false, reason: "channel_denied", layers: [] }], truncated: false }, duration_ms: 4, observed: {}, channel: "mcp", ...previewDetails({ connection_id: "connection_claude", channel: "mcp" }, "claude-code") },
    { timestamp_ms: ago(180000), invocation: "invocation_invalid", workspace: "workspace_codex", tool: "browser_click", capability: "local", allowed: false, reason: "invalid_request", status: "failed", outcome: "failed", effect: "none", summary: "The call does not match the Ghostlight catalog.", duration_ms: 2, observed: {}, channel: "mcp", ...previewDetails({ connection_id: "connection_codex", channel: "mcp" }, "codex-mcp-client") },
    { timestamp_ms: ago(240000), invocation: "invocation_open", workspace: "workspace_script", tool: "browser_navigate", capability: "action", allowed: true, reason: "permitted", status: "succeeded", outcome: "succeeded", effect: "committed", summary: "Opened slow.example.org.", duration_ms: 8100, observed: { host: "slow.example.org", readiness: "loading" }, channel: "cli", browser: "Chrome", ...previewDetails({ connection_id: "connection_script", channel: "cli" }) },
    { timestamp_ms: ago(420000), invocation: "invocation_restored", workspace: "workspace_0193f1c2a8b4e7d6825", tool: "policy_explain", capability: "read", allowed: true, reason: "permitted", status: "succeeded", outcome: "succeeded", effect: "none", summary: "Explained the policy.", duration_ms: 12, observed: {}, channel: "mcp", ...previewDetails({ connection_id: "connection_restored", channel: "mcp" }) },
    { timestamp_ms: ago(480000), invocation: "invocation_restored_claimed", workspace: "workspace_0193f1c2a8b4e7d6826", tool: "browser_read", capability: "read", allowed: true, reason: "permitted", status: "succeeded", outcome: "succeeded", effect: "none", summary: "Read example.com.", duration_ms: 30, observed: {}, channel: "mcp", ...previewDetails({ connection_id: "connection_restored_claimed", channel: "mcp", claimed_agent: "claude-code", source: { agent: "cursor", image: "Cursor.exe" } }) }
  ],
  diagnostics: [
    { id: "service", label: "Ghostlight", severity: "passing", detail: "Running and accepting agents." },
    { id: "browser", label: "Browser", severity: "passing", detail: "Ghostlight in Browser is connected in Google Chrome and Microsoft Edge." },
    { id: "authority", label: "Rules", severity: "passing", detail: "Your rules are valid." },
    { id: "audit", label: "History", severity: "passing", detail: "History is saving." }
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
    organization: northwind,
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
  organization: northwind,
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
    source_class: "https", last_success_ms: ago(240000), last_attempt_ms: ago(240000)
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
  const child = (tool, summary, status, effect, capability) => ({ invocation: "history_example", workspace: "workspace_codex", tool, summary, status, outcome: status === "succeeded" ? "succeeded" : "refused", effect, capability, allowed: status === "succeeded", complete: true, duration_ms: 120, observed: {}, permissions: permission(status === "succeeded", capability), permission_explanations: [status === "succeeded" ? "Allowed: organization policy permits this work (browsing)." : "Refused: organization policy refuses this work (browsing)."] });
  snapshot.operations = [];
  snapshot.history.unshift({ invocation: "history_example", workspace: "workspace_codex", tool: "browser_flow", capability: "", allowed: false, reason: "capability_denied", status: "blocked", outcome: "refused", effect: "partial", summary: "Completed 2 of 4 steps. Step 3 blocked by policy.", complete: true, timestamp_ms: FIXTURE_EPOCH, duration_ms: 920, observed: {}, steps: [
    { position: 1, state: "recorded", tool: "browser_navigate", record: child("browser_navigate", "Opened example.com.", "succeeded", "applied", "read") },
    { position: 2, state: "recorded", tool: "browser_fill_form", record: child("browser_fill_form", "Filled 3 fields on example.com.", "succeeded", "applied", "read + write") },
    { position: 3, state: "recorded", tool: "browser_execute", record: child("browser_execute", "Policy blocked JavaScript execution on example.com.", "blocked", "none", "execute") },
    { position: 4, state: "not_run", tool: "browser_read", record: null }
  ] });
}

/** What a preview session's live connection says, with the name its client reported. */
const liveDetails = (session) => previewDetails({ connection_id: `connection_${session.id}`, channel: session.channel }, REPORTED[session.agent.name] ?? "");

// C1 fixtures carry connection evidence: the name each agent's client reported for itself.
for (const session of snapshot.sessions) session.connections = [liveDetails(session).provenance];

/**
 * Give a record the evidence the service keeps beside it and the agent that evidence names: its
 * own when it has some, the live session's connection for work recorded in this run, only what
 * its channel says for a restored record with none, and the unknown agent with no evidence.
 */
const retainConnection = (record) => {
  if (record.provenance === undefined) {
    const session = snapshot.sessions.find((item) => item.id === record.workspace);
    const said = session ? liveDetails(session) : record.channel ? previewDetails({ channel: record.channel }) : null;
    record.provenance = said?.provenance ?? null;
    record.agent = said?.agent ?? UNKNOWN_AGENT;
  }
  record.agent = structuredClone(record.agent);
  for (const step of record.steps ?? []) if (step.record) retainConnection(step.record);
};
snapshot.operations.forEach(retainConnection);
snapshot.history.forEach(retainConnection);

// `?history=capped` holds more recorded actions than the window's list shows, all older than the
// rows above, so the band's count and the list's length differ.
const CAPPED_HISTORY_LENGTH = 410;
const olderHistory = Array.from({ length: CAPPED_HISTORY_LENGTH - snapshot.history.length }, (_, at) => {
  const spec = script[at % script.length];
  const record = {
    timestamp_ms: ago(600000 + at * 45000), invocation: `invocation_older_${at}`, workspace: "workspace_codex",
    tool: spec.tool, capability: spec.capability, allowed: true, reason: "permitted", status: "succeeded", outcome: "succeeded",
    effect: spec.effect, summary: spec.summary, duration_ms: spec.ms, observed: spec.observed, channel: "mcp", browser: "Chrome"
  };
  retainConnection(record);
  return record;
});
const withCappedHistory = (flat) => ({ ...flat, history: [...flat.history, ...olderHistory] });

/**
 * The page with no browser connected, as the service says it: At a glance names what starts on
 * request, and the Status Browser check passes when `starts` would start on an agent's request or
 * warns that it waits for one when nothing would (`gatherStatus` in `workbench/mod.rs`).
 */
const disconnected = (flat, starts) => ({
  ...flat,
  browsers: [],
  no_browser: starts ? `Starts ${starts} on request` : NO_BROWSER,
  diagnostics: flat.diagnostics.map((item) => (item.id !== "browser" ? item : starts
    ? { ...item, severity: "passing", detail: startsWhenAsked(starts) }
    : { ...item, severity: "warning", detail: BROWSER_WAITING }))
});

// `?browsers=none` shows the page with no browser connected and Google Chrome starting on request.
const withoutBrowsers = (flat) => disconnected(flat, STARTS_ON_REQUEST);

/** What Settings says of the runtime in each of its states (`language::control`). */
const RUNTIME_WORD = { active: "Running", held: "Paused", ended: "Session ended" };

/** The page with the runtime in `state`, said alike by the service, its configuration and Settings. */
const withRuntime = (flat, state) => {
  const changed = structuredClone(flat);
  changed.service.runtime_state = state;
  changed.configuration.runtime_state = state;
  changed.controls.runtime.state = state;
  changed.controls.runtime.word = RUNTIME_WORD[state];
  changed.controls.runtime.offers = OFFERS[state];
  return changed;
};

/**
 * The one sentence Status and the tray say about the service, mirrored from
 * `language::workbench::service_line`: the agents connected, each counted once however many
 * sessions it has, and the browsers.
 */
const serviceLine = (agents, browsers) => {
  const parts = [[agents, "agent", "agents"], [browsers, "browser", "browsers"]]
    .filter(([count]) => count > 0)
    .map(([count, one, many]) => `${count} ${count === 1 ? one : many}`);
  return parts.length === 0 ? "Ghostlight is running; nothing is connected yet."
    : `Ghostlight is running; ${parts.join(" and ")} connected.`;
};

/**
 * The page with the overview, the connections part's counts and the Status summary counting what
 * it shows connected.
 */
const summarized = (flat) => {
  const changed = structuredClone(flat);
  const agents = new Set(changed.sessions.map((session) => session.agent.id)).size;
  const connections = changed.sessions.flatMap((session) => session.connections ?? []);
  // What the connections part counts, as the tray says it (workbench `ConnectionCounts`).
  changed.counts = {
    agents,
    mcp_clients: connections.filter((connection) => connection.channel === "mcp").length,
    command_line: connections.filter((connection) => connection.channel === "cli").length,
    browsers: changed.browsers.length
  };
  changed.overview.connected_browsers = changed.browsers.length;
  changed.controls.service.word = serviceLine(agents, changed.browsers.length);
  return changed;
};

/** The page with nothing in flight. */
const idle = (flat) => ({ ...flat, operations: [] });

/** How many operations run in `workspace` now, as the service counts a session's (`SessionSummary`). */
const runningIn = (operations, workspace) =>
  operations.filter((operation) => operation.workspace === workspace && operation.phase === "running").length;

/** Each session with its running count, from the work the page is shown in flight. */
const counted = (flat) => ({
  ...flat, sessions: flat.sessions.map((session) => ({ ...session, running_operations: runningIn(flat.operations, session.id) }))
});

/** What a held operation shows again: what it was doing, no longer the wait. */
const HELD_ACTIVITY = { browser_read: "Reading page", browser_fill_form: "Filling form" };
const heldWork = (operation) => ({ ...operation, phase: "held", activity: HELD_ACTIVITY[operation.tool] ?? operation.activity });

/**
 * What each `?readiness=` value does to the page. The band's answer is not written here: the page
 * resolves it from the facts these leave, as the service does. Only `working` starts new work on
 * load; a Resume starts it on any page.
 */
const READINESS_SCENES = {
  working: (flat) => flat,
  ready: (flat) => idle(flat),
  not_connected: (flat) => disconnected(idle(flat), null),
  paused: (flat) => withRuntime({ ...flat, operations: flat.operations.map(heldWork) }, "held"),
  ended: (flat) => withRuntime(idle(flat), "ended"),
  channel_off: (flat) => switchedOff(idle(flat), CHANNELS_OFF.channel_off),
  turned_off: (flat) => switchedOff(idle(flat), CHANNELS_OFF.turned_off)
};
const TICKING_SCENE = "working";

/** The page with `off` switched off in your rules: the band's facts, and Status naming the switch. */
function switchedOff(flat, off) {
  const changed = structuredClone(flat);
  changed.channels_off = off;
  const rules = changed.diagnostics.find((item) => item.id === "authority");
  rules.detail = `${rules.detail} ${off.policy}`;
  return changed;
}

/** The switch settings your rules author on the `?readiness=` pages that turn channels off. */
const SWITCH_SETTING = {
  mcp: { key: "channels.mcp.enabled", value: "false", level: "mandatory" },
  cli: { key: "channels.cli.enabled", value: "false", level: "mandatory" }
};

/**
 * The policy as the service compiles it with your rules switching `off` off: the headline names the
 * switch, and each capability says who may still use it or, with every channel off, that nothing may.
 */
function policySwitchedOff(off) {
  const view = structuredClone(policyView);
  const user = view.layers.find((layer) => layer.kind === "user");
  for (const channel of ["mcp", "cli"]) if (off[channel]) user.settings.push(SWITCH_SETTING[channel]);
  view.headline = `${view.headline} ${off.policy}`;
  for (const line of view.capabilities) {
    if (off.mcp && off.cli) {
      Object.assign(line, { state: "unavailable", detail: `Not available. ${off.held}` });
    } else if (line.state !== "unavailable") {
      line.detail = `${line.detail} ${off.capability}`;
    }
    if (line.state !== "unavailable" || (off.mcp && off.cli)) {
      line.decided_by = [...new Set([...line.decided_by, "user"])];
    }
  }
  return view;
}

/** The policy each `?readiness=` page shows. */
const policyFor = (scene) => (CHANNELS_OFF[scene] ? policySwitchedOff(CHANNELS_OFF[scene]) : policyView);

// `?agents=connected` shows Integrations when every agent found on this computer has Ghostlight.
const allConnected = (flat) => ({
  ...flat,
  harnesses: flat.harnesses.map((harness) => (harness.state === "not_detected" ? harness : {
    ...harness, state: "installed", detail: harnessDetail.installed, evidence: undefined,
    can_install: false, can_uninstall: true
  }))
});

/** The page's starting snapshot for the query parameters `choice` holds. */
const pageSnapshot = ({ stage, noBrowsers, connected, capped, scene }) => [
  noBrowsers && withoutBrowsers,
  connected && allConnected,
  capped && withCappedHistory,
  READINESS_SCENES[scene],
  counted,
  summarized
].filter(Boolean).reduce((flat, change) => change(flat), { ...snapshot, setup: setupStates[stage === "done" ? "agent" : stage] });

/**
 * The page's fixture script for the query parameters `choice` holds. What a session's live
 * connection says is composed here, where the provenance rules live, and handed to the page by
 * session id, so work the page settles carries the same evidence as the snapshot's.
 */
const fixtureFor = (choice) => {
  const page = pageSnapshot(choice);
  const details = Object.fromEntries(page.sessions.map((session) => [session.id, liveDetails(session)]));
  return `window.__GHOSTLIGHT_PREVIEW__ = ${JSON.stringify(page)};
window.__GHOSTLIGHT_DETAILS__ = ${JSON.stringify(details)};
window.__GHOSTLIGHT_SETUP_STATES__ = ${JSON.stringify(setupStates)};
window.__GHOSTLIGHT_SETUP_START__ = ${JSON.stringify(choice.stage)};
window.__GHOSTLIGHT_SCRIPT__ = ${JSON.stringify(script)};
window.__GHOSTLIGHT_POLICY__ = ${JSON.stringify(policyFor(choice.scene))};
(() => {
  const preview = window.__GHOSTLIGHT_PREVIEW__;
  const listeners = [];
  let counter = 0;
  let step = 0;
  // The still history scenario never starts work; otherwise the working page starts on load and
  // any page starts on Resume.
  const still = ${historyScenario};
  const ticks = !still && ${choice.scene === TICKING_SCENE};
  let looping = false;

  // Every fixture time moves forward to this page load, so work in flight began seconds ago.
  const shift = Date.now() - ${FIXTURE_EPOCH};
  const MOMENT = /^(timestamp_ms|started_at_ms|generated_at_ms|last_[a-z]+_ms)$/;
  const rebase = value => {
    if (Array.isArray(value)) value.forEach(rebase);
    else if (value && typeof value === "object") {
      for (const [key, item] of Object.entries(value)) {
        if (MOMENT.test(key) && typeof item === "number") value[key] = item + shift;
        else rebase(item);
      }
    }
  };
  rebase(preview);
  rebase(window.__GHOSTLIGHT_POLICY__);

  // The band's answer, resolved from the fixture's facts in the order language/readiness.rs uses.
  const READINESS = ${JSON.stringify(READINESS)};
  const resolveReadiness = () => {
    const runtime = preview.service.runtime_state;
    const configuration = preview.configuration;
    const connected = preview.browsers.length > 0;
    const starts = !connected && preview.no_browser && preview.no_browser !== ${JSON.stringify(NO_BROWSER)}
      ? ${JSON.stringify(STARTS_ON_REQUEST)} : null;
    // Policy refuses every agent that could ask: every channel is off, or every connected agent's is.
    const off = preview.channels_off ?? {};
    const agentsOn = channel => preview.sessions.some(session => session.channel === channel);
    const anyAgent = agentsOn("mcp") || agentsOn("cli");
    const turnedOff = Boolean(off.mcp && off.cli)
      || (anyAgent && (!agentsOn("mcp") || Boolean(off.mcp)) && (!agentsOn("cli") || Boolean(off.cli)));
    const state = runtime === "ended" ? "session_ended"
      : runtime === "held" ? "paused"
        : turnedOff ? "turned_off"
          : !connected && !starts ? "not_connected"
            : configuration.local_policy_valid === false || configuration.managed_authority_valid === false ? "policy_invalid"
              : preview.operations.length > 0 ? "working" : "ready";
    const words = READINESS[state];
    const readyLine = [starts && "Starts " + starts + " when an agent asks.", off.ready].filter(Boolean).join(" ");
    const detail = state === "turned_off" ? off.held : state === "ready" && readyLine ? readyLine : words.detail;
    return { state, word: words.word, detail, tone: words.tone, invites_control: true };
  };
  preview.readiness = resolveReadiness();

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

  // The status and connections parts follow the facts: a changed part is pushed whole, as the
  // service does after work starts, changes phase or settles.
  const runningIn = ${runningIn.toString()};
  const publishLive = () => {
    const readiness = resolveReadiness();
    if (JSON.stringify(readiness) !== JSON.stringify(preview.readiness)) {
      preview.readiness = readiness;
      publish({ kind: "status_changed", status: partsOf(preview).status });
    }
    let recounted = false;
    for (const session of preview.sessions) {
      const running = runningIn(preview.operations, session.id);
      if (session.running_operations !== running) {
        session.running_operations = running;
        recounted = true;
      }
    }
    if (recounted) publish({ kind: "connections_changed", connections: partsOf(preview).connections });
  };

  // The sheet follows the service: a change to the runtime, watching or diagnostics is pushed whole.
  const publishControls = () => {
    const controls = preview.controls;
    controls.runtime.state = preview.service.runtime_state;
    controls.runtime.word = { active: "Running", held: "Paused", ended: "Session ended" }[controls.runtime.state];
    controls.runtime.offers = OFFERS[controls.runtime.state];
    controls.viewing.word = controls.viewing.watch
      ? "On: anyone on this computer can open this view"
      : "Off: only this window shows Ghostlight";
    // The window wears the Sharing word while others can watch, and no chip while only it shows.
    controls.you.word = controls.viewing.watch ? "Shared" : null;
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

  // Settle an operation in flight after its spec's time, as the service would, then call next.
  const settle = (operation, spec, session, blocked, next) => {
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
        outcome: blocked ? "refused" : "succeeded",
        effect: blocked ? "none" : spec.effect,
        summary: blocked ? "Authority blocked the browser job." : spec.summary,
        duration_ms: spec.ms,
        observed: blocked ? {} : spec.observed,
        // The record carries the intake and the evidence of the session that ran it, as the service's do.
        channel: session.channel,
        ...JSON.parse(JSON.stringify(window.__GHOSTLIGHT_DETAILS__[session.id]))
      };
      publish({ kind: "operation_settled", record });
      publishLive();
      next();
    }, spec.ms);
  };

  // The run loop: start one piece of work, settle it, and start the next while the runtime runs.
  const runOne = () => {
    if (preview.service.runtime_state !== "active") {
      looping = false;
      return;
    }
    looping = true;
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
    publishLive();

    settle(operation, spec, session, blocked, () => setTimeout(runOne, blocked ? 3000 : 700 + Math.random() * 1300));
  };

  // Resume puts held work back in flight: each held operation runs what it was doing and settles.
  const releaseHeld = () => {
    for (const operation of preview.operations.filter(each => each.phase === "held")) {
      operation.phase = "running";
      publish({ kind: "operation_changed", operation: { ...operation } });
      const spec = window.__GHOSTLIGHT_SCRIPT__.find(each => each.tool === operation.tool);
      const session = preview.sessions.find(each => each.id === operation.workspace);
      settle(operation, spec, session, false, () => {});
    }
    publishLive();
  };

  // What the service answers for each command. The page asks over HTTP, as it does in the window.
  const answer = async (command, args = {}) => {
    if (command === "workbench_context") {
      // The window's own context: every act is available, so the page draws all of it.
      const acts = ["search", "check_policy", "rescan", "continue_in_window", "hold", "resume",
        "start_session", "end_session", "apply_policy", "remove_policy", "set_diagnostics", "set_watching",
        "setup_all", "harness_act", "test_notification", "open", "reveal_diagnostics", "locate", "setup_act", "quit"];
      return {
        context: "control", word: "Shared",
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
    if (command === "workbench_policy") return window.__GHOSTLIGHT_POLICY__;
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
      publishLive();
      publishControls();
      if (state === "active") releaseHeld();
      if (state === "active" && !still && !looping) runOne();
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
      sessions: flat.sessions, browsers: flat.browsers, counts: flat.counts, harnesses_in_use: flat.harnesses_in_use,
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

  if (ticks) {
    looping = true;
    setTimeout(runOne, 1200);
  }
  window.__GHOSTLIGHT_PUBLISH__ = publish;
})();`;
};

const types = {
  ".css": "text/css", ".js": "text/javascript", ".png": "image/png"
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
    const scene = address.searchParams.get("readiness") ?? TICKING_SCENE;
    if (!Object.hasOwn(READINESS_SCENES, scene)) {
      response.writeHead(400, { "content-type": "text/plain" }).end(`readiness is one of ${Object.keys(READINESS_SCENES).join(", ")}`);
      return;
    }
    if (pathname === "/fixture.js") {
      response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
      response.end(fixtureFor({
        stage, scene,
        noBrowsers: address.searchParams.get("browsers") === "none",
        connected: address.searchParams.get("agents") === "connected",
        capped: address.searchParams.get("history") === "capped"
      }));
      return;
    }
    const name = pathname === "/" ? "index.html" : pathname.slice(1);
    const [directory, iconOf, ...beyond] = name.split("/");
    if (directory === AGENT_ICON_DIRECTORY) {
      const image = beyond.length === 0 ? agentImages.get(iconOf) : undefined;
      if (!image) {
        response.writeHead(404).end();
        return;
      }
      response.writeHead(200, {
        "content-type": image.type, "cache-control": "no-cache", etag: `"${image.version}"`,
        "x-content-type-options": "nosniff", "content-security-policy": IMAGE_POLICY
      });
      response.end(request.method === "HEAD" ? undefined : image.bytes);
      return;
    }
    // The setup view shares two of the extension's files, which the service serves under /extension/.
    const shared = { "extension/kit.css": join(extensionFolder, "kit.css"),
      "extension/ghostlight-mascot.png": join(extensionFolder, "icons", "ghostlight-mascot.png") };
    const served = new Set(["index.html", "styles.css", "app.js", "ghostlight.png"]);
    if (!served.has(name) && !Object.hasOwn(shared, name) && !/^lib\/[a-z-]+\.js$/.test(name)
      && !/^kit\/[a-z]+\.(js|css)$/.test(name)) {
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
