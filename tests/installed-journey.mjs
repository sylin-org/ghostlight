#!/usr/bin/env node
// The installed protocol-4 journey: the 2.0 break validated end to end through the INSTALLED
// service, never a second one.
//
// What it does:
//   1. Starts branded Chrome on a scratch profile over the DevTools pipe and loads the repository
//      extension with Extensions.loadUnpacked (see lib/cdp-pipe.mjs). The pinned manifest key
//      makes the id cjcmhepmagomefjggkcohdbfemacojoa, which the per-user native host registration
//      allows, so the extension reaches ~/.ghostlight/bin/ghostlight (the browser edge) and,
//      through relay 4, the one installed service.
//   2. Spawns `~/.ghostlight/bin/ghostlight mcp` (the MCP edge) as a small MCP stdio client.
//   3. Runs named checks against local fixtures served on two origins, then (with --live) against
//      the public demo pages, and prints one line per check.
//
// It never starts a service, never sets GHOSTLIGHT_TEST_HOME, and refuses to run when the
// installed layout is missing. Run it after the deployment steps and before the daily Chrome
// profile switches over. Exit codes: 0 every check passed, 1 a check or the setup failed,
// 2 refused or bad usage.
import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { policyRefreshed } from "./lib/policy-refresh.mjs";
import { createScratch, installScratchNativeHost, launchChrome, removeScratch } from "./lib/cdp-pipe.mjs";
import { EDGE_NAME, SERVICE_NAME, controlOnce, controlOp, quitReason } from "./lib/home.mjs";

const repository = resolve(fileURLToPath(new URL("..", import.meta.url)));

/** Development extension id the pinned manifest key produces. */
const EXTENSION_ID = "cjcmhepmagomefjggkcohdbfemacojoa";
/** MCP protocol revision the journey speaks. */
const MCP_PROTOCOL = "2025-11-25";
/** Largest image a tool result may carry, in base64 characters (the model API image limit). */
const IMAGE_BUDGET_CHARS = 5 * 1024 * 1024;
/** Default wait for one tool call. */
const CALL_TIMEOUT_MS = 60000;
/** Wait for the service to restart on demand. */
const RESTART_TIMEOUT_MS = 120000;
/** How long a quit must stay quit with nothing calling. */
const QUIT_HOLD_MS = 30000;
/** Failure reasons that only mean the extension has not reconnected to the restarted service yet. */
const BROWSER_RECONNECTING = ["browser_disconnected", "browser_not_answering"];
/** How long the scratch browser has to connect to the installed service. */
const CONNECT_TIMEOUT_MS = 120000;
/** Names of the tools the 2.0 catalog advertises. */
const EXPECTED_TOOLS = [
  "browser_tabs", "browser_navigate", "browser_history", "browser_window", "browser_read",
  "browser_inspect", "browser_find", "browser_screenshot", "browser_click", "browser_scroll",
  "browser_hover", "browser_fill_form", "browser_type_text", "browser_press_key", "browser_drag",
  "browser_wait", "browser_dialog", "browser_upload", "browser_execute", "browser_flow",
  "browser_record", "browser_diagnose", "policy_explain"
];
const NONCE = "installedjourney";
const SECRET = "journey-secret-4821";
const LIVE_ROOT = "https://sylin.org/ghostlight/demo";
const MODIFIER = process.platform === "darwin" ? "Meta" : "Control";
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

async function until(check, label, timeoutMs = 30000, intervalMs = 100) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await check();
    if (value) return value;
    if (Date.now() >= deadline) throw new Error(`Timed out: ${label}`);
    await sleep(intervalMs);
  }
}

// ---------------------------------------------------------------------------------------------
// Options and the refusal guards
// ---------------------------------------------------------------------------------------------

const USAGE = `usage: node tests/installed-journey.mjs [--chrome <path>] [--extension <dir>] [--only <check>...]
                                    [--live] [--hold <seconds>] [--keep-profile]
  --chrome       Chrome executable (default: the installed Google Chrome)
  --extension    unpacked extension directory (default: <repo>/extension)
  --only         run only the named checks (comma or space separated)
  --live         add the nine https://sylin.org/ghostlight/demo checks
  --hold         keep the scratch browser open this many seconds after the checks, so
                 scripts/demo-foundry.ps1 and scripts/demo-brief.ps1 can run against it
  --keep-profile keep the scratch profile directory`;

function usageError(message) {
  console.error(`${message}\n${USAGE}`);
  process.exit(2);
}

function parseArguments(argv) {
  const options = { chrome: null, extension: join(repository, "extension"), only: [], live: false, hold: 0, keepProfile: false };
  for (let index = 0; index < argv.length; index++) {
    const argument = argv[index];
    const value = () => {
      const next = argv[++index];
      if (next === undefined || next.startsWith("--")) usageError(`${argument} needs a value`);
      return next;
    };
    if (argument === "--chrome") options.chrome = resolve(value());
    else if (argument === "--extension") options.extension = resolve(value());
    else if (argument === "--only") {
      options.only.push(...value().split(","));
      while (argv[index + 1] !== undefined && !argv[index + 1].startsWith("--")) options.only.push(...argv[++index].split(","));
    } else if (argument === "--live") options.live = true;
    else if (argument === "--hold") {
      options.hold = Number(value());
      if (!Number.isFinite(options.hold) || options.hold < 0) usageError("--hold takes a number of seconds");
    } else if (argument === "--keep-profile") options.keepProfile = true;
    else if (argument === "--help" || argument === "-h") { console.log(USAGE); process.exit(0); }
    else usageError(`unknown option ${argument}`);
  }
  options.only = options.only.map((name) => name.trim()).filter(Boolean);
  return options;
}

function defaultChrome() {
  const candidates = {
    win32: [
      "C:/Program Files/Google/Chrome/Application/chrome.exe",
      "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
      join(process.env.LOCALAPPDATA || "", "Google/Chrome/Application/chrome.exe")
    ],
    darwin: ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"],
    linux: ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/opt/google/chrome/chrome"]
  }[process.platform] ?? [];
  return candidates.find((path) => existsSync(path)) ?? null;
}

const ghostlightHome = join(homedir(), ".ghostlight");
const installed = {
  edge: join(ghostlightHome, "bin", EDGE_NAME),
  service: join(ghostlightHome, "bin", SERVICE_NAME),
  policy: join(ghostlightHome, "policy", "user-policy.json"),
  diagnostics: join(ghostlightHome, "diagnostics")
};

function refuseUnlessInstalled() {
  if (process.env.GHOSTLIGHT_TEST_HOME) {
    console.error("Refusing to run: GHOSTLIGHT_TEST_HOME is set. This journey tests the installed service, not a second one.");
    process.exit(2);
  }
  for (const [what, path] of [["installed edge", installed.edge], ["installed service image", installed.service]]) {
    if (!existsSync(path)) {
      console.error(`Refusing to run: the ${what} is missing (${path}). Install Ghostlight first; this journey never starts a service of its own.`);
      process.exit(2);
    }
  }
}

// ---------------------------------------------------------------------------------------------
// The MCP client
// ---------------------------------------------------------------------------------------------

/** A small MCP stdio client over the installed MCP edge. */
class McpClient {
  constructor(command, args = []) {
    this.pending = new Map();
    this.nextId = 0;
    this.stderr = "";
    this.child = spawn(command, args, { stdio: ["pipe", "pipe", "pipe"], windowsHide: true, env: process.env });
    createInterface({ input: this.child.stdout }).on("line", (line) => this.#line(line));
    this.child.stderr.on("data", (chunk) => { this.stderr = (this.stderr + chunk).slice(-4000); });
    this.child.stdin.on("error", () => {});
    this.child.on("error", (error) => this.#fail(error));
    this.child.on("exit", (code, signal) => this.#fail(new Error(`MCP edge exited (${code ?? signal})${this.stderr ? `: ${this.stderr.trim()}` : ""}`)));
  }

  #line(line) {
    let message;
    try { message = JSON.parse(line); } catch { return; }
    const item = typeof message.id === "number" ? this.pending.get(message.id) : undefined;
    if (!item) return;
    this.pending.delete(message.id);
    clearTimeout(item.timer);
    item.done(message);
  }

  #fail(error) {
    for (const item of this.pending.values()) { clearTimeout(item.timer); item.reject(error); }
    this.pending.clear();
  }

  /** Send one JSON-RPC request and resolve with the whole response message. */
  request(method, params = {}, timeoutMs = CALL_TIMEOUT_MS) {
    const id = ++this.nextId;
    return new Promise((done, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Timed out after ${timeoutMs} ms: ${method} ${params.name ?? ""}`.trim()));
      }, timeoutMs);
      this.pending.set(id, { done, reject, timer });
      this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    });
  }

  /** Send a notification. */
  notify(method, params = {}) {
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
  }

  /** The MCP handshake. */
  async initialize() {
    const response = await this.request("initialize", { protocolVersion: MCP_PROTOCOL, capabilities: {}, clientInfo: { name: "installed-journey", version: "1" } });
    assert.equal(response.error, undefined, JSON.stringify(response.error));
    this.notify("notifications/initialized");
    return response.result;
  }

  /** End stdin and wait for the edge to leave; kill only this child when it will not. */
  async close() {
    this.child.stdin.end();
    const exited = await Promise.race([new Promise((done) => this.child.once("exit", () => done(true))), sleep(5000).then(() => false)]);
    if (!exited && this.child.exitCode === null) this.child.kill();
  }
}

// ---------------------------------------------------------------------------------------------
// The relay control stream (the journey asks the installed service to quit by itself)
// ---------------------------------------------------------------------------------------------

/** The installed service's answer to `status`, or null when none is running. */
async function serviceStatus() {
  const answer = await controlOnce(ghostlightHome, { op: controlOp.status });
  return answer?.reply.accepted ? answer.value : null;
}

async function quitService() {
  const answer = await controlOnce(ghostlightHome, { op: controlOp.quit, reason: quitReason.quit });
  assert.equal(answer?.reply.accepted, true, `quit refused: ${JSON.stringify(answer?.reply)}`);
  return answer.value;
}

/** The service image's process name; its command line is `<name> serve ...`. */
const SERVICE_PROCESS = "ghostlight-service";

function processCount(name) {
  return new Promise((done) => {
    const [command, args] = process.platform === "win32"
      ? ["tasklist", ["/FI", `IMAGENAME eq ${name}.exe`, "/FO", "CSV", "/NH"]]
      : ["pgrep", ["-f", `/${name} serve`]];
    execFile(command, args, { windowsHide: true }, (error, stdout) => {
      const lines = String(stdout).split(/\r?\n/).filter((line) => line.trim() && !/^INFO:/i.test(line));
      done(process.platform === "win32" ? lines.filter((line) => line.toLowerCase().includes(`${name}.exe`)).length : lines.length);
    });
  });
}

// ---------------------------------------------------------------------------------------------
// Local fixtures: two origins, so frames between them are cross-origin and cross-site
// ---------------------------------------------------------------------------------------------

const page = (title, body, script = "", head = "") =>
  `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title}</title>${head}</head><body>${body}${script ? `<script>${script}</script>` : ""}</body></html>`;

const PAD = "position:fixed;right:30px;bottom:30px;width:200px;height:100px;border:1px solid #333;background:#dde";

/** Every fixture page, keyed by path. A builder gets the two origins. */
const PAGES = {
  "/blank": () => page("Installed blank", "<p>blank</p>"),
  "/home": () => page("Installed home", `
    <h1>Installed home</h1>
    <nav><a href="/scroll">Scroll page link</a></nav>
    <article><h2>Article heading</h2><p>ARTICLE-SENTINEL alpha beta gamma delta epsilon.</p><p>A second paragraph gives the article enough words to count as one.</p></article>
    <aside>SIDEBAR-NOISE links and promotions</aside>
    <label>Display name <input id="name" aria-label="Display name"></label>
    <button id="inc">Increment</button> <output id="count">0</output>
    <div id="pad" role="button" tabindex="0" aria-label="Click pad" style="${PAD}">pad</div>`,
  `window.counter = 0; window.padClicks = [];
   document.getElementById("inc").addEventListener("click", () => { window.counter++; document.getElementById("count").textContent = String(window.counter); });
   document.getElementById("pad").addEventListener("click", (event) => window.padClicks.push([event.clientX, event.clientY]));`),
  "/dialogs": () => page("Installed dialogs", `
    <h1>Dialogs</h1>
    <button id="alert">Raise alert</button><button id="confirm">Raise confirm</button><button id="prompt">Raise prompt</button>
    <label>Trap field <input id="trap" aria-label="Trap field"></label>`,
  `window.alerts = 0; window.confirmed = "unset"; window.answer = "unset"; window.trapAlerts = 0;
   document.getElementById("trap").addEventListener("input", () => { alert("Raised by the trap field"); window.trapAlerts++; });
   document.getElementById("alert").addEventListener("click", () => { alert("Alert message"); window.alerts++; });
   document.getElementById("confirm").addEventListener("click", () => { window.confirmed = confirm("Confirm message"); });
   document.getElementById("prompt").addEventListener("click", () => { window.answer = prompt("Prompt message", "default answer"); });`),
  "/form": () => page("Installed form", `
    <h1>Installed form</h1>
    <form id="form">
      <label>Full name <input id="fullname" aria-label="Full name"></label>
      <label>Plan <select id="plan" aria-label="Plan"><option value="free-plan">Free</option><option value="team-plan">Team</option><option value="ent-plan">Enterprise</option></select></label>
      <label><input id="subscribe" type="checkbox"> Subscribe to updates</label>
      <label>Access code <input id="code" type="password" aria-label="Access code"></label>
      <button id="submit" type="submit">Submit form</button>
    </form>
    <p id="result"></p>`,
  `window.submitted = null;
   document.getElementById("form").addEventListener("submit", (event) => {
     event.preventDefault();
     window.submitted = { name: document.getElementById("fullname").value, plan: document.getElementById("plan").value,
       subscribe: document.getElementById("subscribe").checked, code: document.getElementById("code").value };
     document.getElementById("result").textContent = "Form submitted";
   });`),
  "/keys": () => page("Installed keys", `
    <h1>Keys</h1>
    <label>First key field <input id="k1" aria-label="First key field"></label>
    <label>Second key field <input id="k2" aria-label="Second key field"></label>`,
  `window.keys = []; window.enters = 0;
   document.addEventListener("keydown", (event) => { window.keys.push(event.key); });
   document.getElementById("k1").addEventListener("keydown", (event) => { if (event.key === "Enter") window.enters++; });`),
  "/waits": () => page("Installed waits", `
    <h1>Waits</h1>
    <button id="reveal">Reveal text</button><button id="hide">Hide text</button><button id="route">Change route</button><button id="animate">Animate box</button>
    <p id="vanishing">Vanishing line</p><div id="late"></div>
    <div id="box" style="position:relative;left:0;width:40px;height:40px;background:#a33;transition:left 0.8s"></div>`,
  `const later = (action) => setTimeout(action, 1500);
   document.getElementById("reveal").addEventListener("click", () => later(() => { document.getElementById("late").textContent = "Delayed line arrived"; }));
   document.getElementById("hide").addEventListener("click", () => later(() => document.getElementById("vanishing").remove()));
   document.getElementById("route").addEventListener("click", () => later(() => history.replaceState(null, "", "#ready")));
   document.getElementById("animate").addEventListener("click", () => { document.getElementById("box").style.left = "300px"; });`),
  "/tall": () => page("Installed tall", `
    <h1>Tall page</h1>
    <button id="capture">Capture target</button>
    ${[1, 2, 3, 4, 5, 6].map((n) => `<section style="height:500px;background:hsl(${n * 50},60%,70%)"><h2>Block ${n}</h2></section>`).join("")}`),
  "/scroll": () => page("Installed scroll", `
    <h1>Installed scroll page</h1><p>Scroll page marker</p>
    <button id="hover">Hover me</button>
    <div style="height:3500px"></div><p>Bottom marker</p>
    <div id="wheel" role="region" aria-label="Wheel pad" style="${PAD}">wheel</div>`,
  `window.hovered = false; window.padHover = false; window.wheelY = 0;
   document.getElementById("hover").addEventListener("mouseover", () => { window.hovered = true; });
   const pad = document.getElementById("wheel");
   pad.addEventListener("mouseenter", () => { window.padHover = true; });
   pad.addEventListener("wheel", (event) => { window.wheelY += event.deltaY; }, { passive: true });`),
  "/dnd": () => page("Installed drag", `
    <h1>Drag lists</h1>
    <ul id="list"><li><button id="alpha" draggable="true">Drag Alpha</button></li><li><button id="beta" draggable="true">Drag Beta</button></li></ul>
    <div id="zone" style="width:300px;height:80px;border:2px dashed #555">Drop here</div>
    <div id="track" style="${PAD}">track</div>`,
  `window.dropped = null; window.pointerDrag = null;
   for (const id of ["alpha", "beta"]) document.getElementById(id).addEventListener("dragstart", (event) => event.dataTransfer.setData("text/plain", id));
   const zone = document.getElementById("zone");
   zone.addEventListener("dragover", (event) => event.preventDefault());
   zone.addEventListener("drop", (event) => { event.preventDefault(); window.dropped = event.dataTransfer.getData("text/plain"); });
   const track = document.getElementById("track"); let start = null;
   track.addEventListener("pointerdown", (event) => { start = [event.clientX, event.clientY]; track.setPointerCapture(event.pointerId); });
   track.addEventListener("pointerup", (event) => { if (start) window.pointerDrag = { dx: event.clientX - start[0], dy: event.clientY - start[1] }; start = null; });`),
  "/upload": () => page("Installed upload", `
    <h1>Upload</h1>
    <label>Evidence file <input id="file" type="file" aria-label="Evidence file"></label>
    <div id="drop" role="region" aria-label="Drop pad" style="${PAD}">drop</div>`,
  `window.dropped = null;
   const drop = document.getElementById("drop");
   drop.addEventListener("dragover", (event) => event.preventDefault());
   drop.addEventListener("drop", (event) => { event.preventDefault(); window.dropped = [...event.dataTransfer.files].map((file) => ({ name: file.name, type: file.type, size: file.size })); });`),
  "/console": () => page("Installed console", `
    <h1>Console page</h1><button id="emit">Emit diagnostics</button>`,
  `document.getElementById("emit").addEventListener("click", async () => {
     console.warn("Installed sentinel warning");
     console.error("Installed sentinel error");
     await fetch("/ping-ok"); await fetch("/missing-asset");
   });`),
  "/editor": () => readFileSync(join(repository, "tests/fixtures/contenteditable.html"), "utf8"),
  "/shadow": () => page("Installed shadow", `<h1>Shadow page</h1><div id="host"></div>`,
  `window.shadowClicks = 0;
   const shadow = document.getElementById("host").attachShadow({ mode: "open" });
   shadow.innerHTML = '<label>Shadow field <input aria-label="Shadow field"></label><button>Shadow action</button>';
   shadow.querySelector("button").addEventListener("click", () => { window.shadowClicks++; });`),
  "/beforeunload": () => page("Installed beforeunload", `
    <h1>Unsaved work</h1><label>Draft <input id="draft" aria-label="Draft"></label><button id="touch">Touch page</button>`,
  `window.touched = 0;
   document.getElementById("touch").addEventListener("click", () => { window.touched++; });
   addEventListener("beforeunload", (event) => { event.preventDefault(); event.returnValue = ""; });`),
  "/frames-outer": ({ B }) => page("Installed outer frame", `<h1>Outer page</h1><iframe src="${B}/frames-mid" title="Mid frame" width="700" height="420"></iframe>`),
  "/frames-mid": ({ A }) => page("Installed mid frame", `<h2>Mid frame</h2>
    <iframe src="${A}/frames-inner" title="Inner embed" width="320" height="160"></iframe>
    <iframe src="${A}/frames-inner" title="Inner embed" width="320" height="160"></iframe>`),
  "/frames-inner": () => page("Installed inner frame", `<h3>Inner form</h3>
    <label>Inner note <input id="note" aria-label="Inner note"></label><button id="send" type="button">Inner send</button>`,
  `const frame = Math.random().toString(36).slice(2);
   const report = (event) => fetch("/events", { method: "POST", body: JSON.stringify(event), keepalive: true });
   document.getElementById("note").addEventListener("input", (event) => report({ kind: "inner-input", frame, value: event.target.value }));
   document.getElementById("send").addEventListener("click", () => report({ kind: "inner-send", frame }));`),
  "/csp": () => page("Installed strict page", `<h1>Strict page</h1><button id="count">Count</button><output id="out">0</output>`,
  "", `<script nonce="${NONCE}">window.violations = [];
     document.addEventListener("securitypolicyviolation", (event) => window.violations.push(event.violatedDirective + "|" + event.blockedURI));</script>
     <script nonce="${NONCE}">window.clicks = 0;
     addEventListener("DOMContentLoaded", () => { window.baseElements = document.getElementsByTagName("*").length;
       document.getElementById("count").addEventListener("click", () => { window.clicks++; document.getElementById("out").textContent = String(window.clicks); }); });</script>`)
};

const CSP_HEADER = `default-src 'none'; script-src 'nonce-${NONCE}'; style-src 'nonce-${NONCE}'; img-src 'self' data:; connect-src 'self'; require-trusted-types-for 'script'`;

function loadSylinSnapshot() {
  const snapshot = JSON.parse(readFileSync(join(repository, "tests/fixtures/sylin-iframe.json"), "utf8"));
  for (const item of snapshot.pages) assert.equal(createHash("sha256").update(item.body).digest("hex"), item.sha256, `snapshot page ${item.url}`);
  const assets = new Map();
  for (const asset of snapshot.assets) {
    const bytes = Buffer.from(asset.data_base64, "base64");
    assert.equal(createHash("sha256").update(bytes).digest("hex"), asset.sha256, `snapshot asset ${asset.path}`);
    assets.set(asset.path.split("?")[0], { type: asset.content_type, bytes });
  }
  return { parent: snapshot.pages[0].body, child: snapshot.pages[1].body, assets };
}

async function startFixtures() {
  const sylin = loadSylinSnapshot();
  const events = [];
  const origins = {};
  const servers = [];
  const handle = async (request, response) => {
    const path = request.url.split("?")[0];
    if (request.method === "POST" && path === "/events") {
      let body = "";
      for await (const chunk of request) body += chunk;
      try { events.push({ ...JSON.parse(body), at: Date.now() }); } catch { /* ignore */ }
      response.end("ok");
      return;
    }
    if (path === "/ping-ok") { response.setHeader("content-type", "text/plain"); response.end("pong"); return; }
    if (sylin.assets.has(path)) {
      const asset = sylin.assets.get(path);
      response.setHeader("content-type", asset.type);
      response.end(asset.bytes);
      return;
    }
    let html;
    if (path === "/sylin") html = sylin.parent.replace('src="./form/"', `src="${origins.B}/sylin-form"`);
    else if (path === "/sylin-form") html = sylin.child;
    else if (PAGES[path]) html = PAGES[path](origins);
    if (html === undefined) { response.statusCode = 404; response.end("not found"); return; }
    response.setHeader("content-type", "text/html; charset=utf-8");
    response.setHeader("cache-control", "no-store");
    if (path === "/csp") response.setHeader("content-security-policy", CSP_HEADER);
    response.end(html);
  };
  for (const role of ["A", "B"]) {
    const server = createServer((request, response) => { handle(request, response).catch(() => response.destroy()); });
    await new Promise((done) => server.listen(0, "127.0.0.1", done));
    servers.push(server);
    origins[role] = `http://${role === "A" ? "127.0.0.1" : "localhost"}:${server.address().port}`;
  }
  return {
    A: origins.A,
    B: origins.B,
    events,
    close: () => Promise.all(servers.map((server) => new Promise((done) => { server.close(done); server.closeAllConnections?.(); })))
  };
}

// ---------------------------------------------------------------------------------------------
// The check context: tool calls, tabs, and page ground truth read straight from the browser
// ---------------------------------------------------------------------------------------------

class Context {
  constructor({ mcp, pipe, fixtures, scratch }) {
    Object.assign(this, { mcp, pipe, fixtures, scratch });
    this.A = fixtures.A;
    this.B = fixtures.B;
    this.opened = [];
    this.browser = null;
    this.tagCounter = 0;
    this.tag = null;
  }

  /** Call one tool and return its structured outcome; the raw MCP result rides along unlisted. */
  async call(name, args = {}, { timeoutMs = CALL_TIMEOUT_MS } = {}) {
    const response = await this.mcp.request("tools/call", { name, arguments: args }, timeoutMs);
    assert.equal(response.error, undefined, `${name}: ${JSON.stringify(response.error)}`);
    const outcome = response.result?.structuredContent;
    assert.ok(outcome && typeof outcome === "object", `${name}: no structured content in ${JSON.stringify(response.result)?.slice(0, 400)}`);
    Object.defineProperty(outcome, "raw", { value: response.result, enumerable: false });
    return outcome;
  }

  /** Call a tool and require status succeeded. */
  async succeed(name, args = {}, options) {
    const outcome = await this.call(name, args, options);
    assert.equal(outcome.status, "succeeded", `${name} ${JSON.stringify(args).slice(0, 300)} -> ${JSON.stringify(outcome).slice(0, 900)}`);
    return outcome;
  }

  /** Open a URL in a new controlled tab of the scratch browser, wait for load, return the handle. */
  async open(url) {
    const outcome = await this.succeed("browser_navigate", { url, new_tab: true, ...(this.browser ? { browser: this.browser } : {}) });
    const tab = outcome.facts.tab;
    assert.match(tab, /^tab_/, JSON.stringify(outcome));
    this.opened.push(tab);
    await this.succeed("browser_wait", { tab, condition: "load_ready", timeout_ms: 20000 });
    return tab;
  }

  /** Open a fixture path on origin A, with a unique query so its CDP target is unambiguous. */
  openFixture(path, origin = this.A) {
    this.tag = `t${++this.tagCounter}x`;
    return this.open(`${origin}${path}?${this.tag}`);
  }

  /** Close every tab this check opened. */
  async closeOpened() {
    for (const tab of this.opened.splice(0)) await this.call("browser_tabs", { action: "close", tab }, { timeoutMs: 15000 }).catch(() => {});
  }

  /** Controls of a tab, as browser_inspect lists them. */
  async controls(tab, maxItems = 100) {
    return (await this.succeed("browser_inspect", { tab, scope: "controls", max_items: maxItems })).facts.items;
  }

  /** The handle of the control with exactly this name (or this name as a prefix). */
  async targetNamed(tab, name, role) {
    const items = await this.controls(tab);
    const pick = (match) => items.find((item) => match(item.name ?? "") && (!role || item.role === role));
    const item = pick((value) => value === name) ?? pick((value) => value.startsWith(name));
    assert.ok(item, `no control named ${name}: ${items.map((entry) => `${entry.role}:${entry.name}`).join(", ")}`);
    return item.target;
  }

  /** Ranked matches for visible text, each with a handle. */
  async matches(tab, text, role) {
    const found = await this.succeed("browser_find", { tab, text });
    const matches = found.facts.matches.filter((match) => !role || match.role === role);
    assert.ok(matches.length > 0, `nothing matched ${text}: ${JSON.stringify(found.facts).slice(0, 500)}`);
    return matches;
  }

  /** Evaluate in the page whose URL contains `urlPart`, over the scratch browser's own pipe. */
  async evalIn(urlPart, expression) {
    const { targetInfos } = await this.pipe.send("Target.getTargets");
    const matching = targetInfos.filter((target) => target.type === "page" && target.url.includes(urlPart));
    assert.ok(matching.length > 0, `no page target contains ${urlPart}: ${targetInfos.map((target) => target.url).join(", ")}`);
    const { sessionId } = await this.pipe.send("Target.attachToTarget", { targetId: matching.at(-1).targetId, flatten: true });
    try {
      const result = await this.pipe.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, sessionId, 20000);
      assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
      return result.result.value;
    } finally {
      await this.pipe.send("Target.detachFromTarget", { sessionId }).catch(() => {});
    }
  }

  /** A viewport screenshot, plus the CSS-to-image scale needed to aim coordinates at it. */
  async shot(tab, urlPart) {
    const shot = await this.succeed("browser_screenshot", { tab }, { timeoutMs: 90000 });
    assert.match(shot.facts.view, /^view_/, JSON.stringify(shot.facts));
    const innerWidth = await this.evalIn(urlPart, "innerWidth");
    return { shot, view: shot.facts.view, scale: shot.facts.width / innerWidth };
  }

  /** Image coordinates of the centre of a page element. */
  async centre(urlPart, selector, scale) {
    const [x, y] = await this.evalIn(urlPart, `(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`);
    return { x: Math.round(x * scale), y: Math.round(y * scale), cssX: x, cssY: y };
  }

  /** Server-side events the fixture pages reported, of one kind. */
  eventsOf(kind, since = 0) {
    return this.fixtures.events.filter((event) => event.kind === kind && event.at >= since);
  }

  /** The extension popup's text once it shows a connected state (lit light), read through the scratch browser. */
  async popupText() {
    const { targetId } = await this.pipe.send("Target.createTarget", { url: `chrome-extension://${EXTENSION_ID}/popup.html` });
    try {
      const { sessionId } = await this.pipe.send("Target.attachToTarget", { targetId, flatten: true });
      return await until(async () => {
        const result = await this.pipe.send("Runtime.evaluate", { expression: "document.getElementById('pop')?.dataset.light === 'lit' ? document.body.innerText : ''", returnByValue: true }, sessionId, 10000).catch(() => null);
        return result?.result?.value || null;
      }, "the popup light to be lit", 45000, 500);
    } finally {
      await this.pipe.send("Target.closeTarget", { targetId }).catch(() => {});
    }
  }
}

const within = (value, expected, tolerance) => Math.abs(value - expected) <= tolerance;
const imageOf = (outcome) => outcome.raw.content.find((item) => item.type === "image");

// ---------------------------------------------------------------------------------------------
// The checks (numbers refer to the validation plan)
// ---------------------------------------------------------------------------------------------

const CHECKS = [];
const check = (name, title, run) => CHECKS.push({ name, title, run });

check("catalog", "tools/list advertises the 23 tools and no output schema", async (t) => {
  const response = await t.mcp.request("tools/list", {});
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  const tools = response.result.tools;
  const names = tools.map((tool) => tool.name);
  for (const name of EXPECTED_TOOLS) assert.ok(names.includes(name), `missing tool ${name}`);
  assert.equal(names.length, EXPECTED_TOOLS.length, `unexpected tools: ${names.filter((name) => !EXPECTED_TOOLS.includes(name))}`);
  assert.ok(tools.every((tool) => tool.outputSchema === undefined), "a tool still advertises outputSchema");
});

check("tabs", "(1) browser_tabs lists the scratch browser; navigate opens a fixture", async (t) => {
  const listed = await t.succeed("browser_tabs", { action: "list" });
  assert.ok(listed.facts.browsers.some((browser) => browser.browser === t.browser), `scratch browser ${t.browser} not listed: ${JSON.stringify(listed.facts.browsers)}`);
  const tab = await t.openFixture("/home");
  assert.match(await t.evalIn(t.tag, "document.title"), /Installed home/);
  const after = await t.succeed("browser_tabs", { action: "list" });
  assert.ok(after.facts.tabs.some((entry) => entry.tab === tab), "the opened tab is not listed");
  await t.succeed("browser_tabs", { action: "focus", tab });
});

check("page-isolation", "(2) the page cannot see Ghostlight", async (t) => {
  const tab = await t.openFixture("/home");
  for (const script of ["typeof window.__ghostlight_dispatch__", "typeof globalThis.__ghostlightRuntime"]) {
    const result = await t.succeed("browser_execute", { tab, script });
    assert.equal(result.facts.value, "undefined", `${script} -> ${JSON.stringify(result.facts)}`);
  }
  const leaked = await t.succeed("browser_execute", { tab, script: "Object.getOwnPropertyNames(window).filter((name) => /ghostlight/i.test(name))" });
  assert.deepEqual(leaked.facts.value, []);
  assert.deepEqual(await t.evalIn(t.tag, "Object.getOwnPropertyNames(window).filter((name) => /ghostlight/i.test(name))"), []);
});

check("read", "(3) browser_read in visible and article modes", async (t) => {
  const tab = await t.openFixture("/home");
  const visible = await t.succeed("browser_read", { tab, mode: "visible", max_chars: 20000 });
  assert.match(visible.facts.text, /Installed home/);
  assert.match(visible.facts.text, /SIDEBAR-NOISE/);
  const article = await t.succeed("browser_read", { tab, mode: "article", max_chars: 20000 });
  assert.match(article.facts.text, /ARTICLE-SENTINEL/);
  assert.doesNotMatch(article.facts.text, /SIDEBAR-NOISE/);
});

check("find-inspect", "(4) browser_find and browser_inspect controls and tree", async (t) => {
  const tab = await t.openFixture("/home");
  const found = await t.matches(tab, "Increment", "button");
  assert.match(found[0].target, /^target_/);
  const items = await t.controls(tab);
  assert.ok(items.some((item) => item.name === "Display name" && item.role === "textbox"), JSON.stringify(items));
  assert.ok(items.some((item) => item.name === "Increment"));
  const document = await t.succeed("browser_inspect", { tab, scope: "document", max_depth: 6 });
  assert.ok(document.facts.nodes > 0, JSON.stringify(document.facts).slice(0, 400));
  assert.ok(Array.isArray(document.facts.tree?.children));
  const all = await t.succeed("browser_inspect", { tab, scope: "all", max_items: 100 });
  assert.ok(all.facts.items.length > 0);
});

check("click", "(5) click by target, by selector and by coordinates", async (t) => {
  const tab = await t.openFixture("/home");
  const tag = t.tag;
  await t.succeed("browser_click", { tab, target: await t.targetNamed(tab, "Increment", "button") });
  assert.equal(await t.evalIn(tag, "window.counter"), 1);
  await t.succeed("browser_click", { tab, selector: { name: "Increment", role: "button", exact: true } });
  assert.equal(await t.evalIn(tag, "window.counter"), 2);
  const { view, scale } = await t.shot(tab, tag);
  const pad = await t.centre(tag, "#pad", scale);
  await t.succeed("browser_click", { tab, view, x: pad.x, y: pad.y });
  const clicks = await t.evalIn(tag, "window.padClicks");
  assert.equal(clicks.length, 1, JSON.stringify(clicks));
  assert.ok(within(clicks[0][0], pad.cssX, 6) && within(clicks[0][1], pad.cssY, 6), `clicked ${clicks[0]} aimed at ${pad.cssX},${pad.cssY}`);
});

check("dialogs", "(6) alert, confirm and prompt: status, accept, dismiss, respond; an alert raised by a fill and by an evaluation", async (t) => {
  const tab = await t.openFixture("/dialogs");
  const tag = t.tag;
  const raise = async (name) => {
    const clicked = await t.call("browser_click", { tab, target: await t.targetNamed(tab, name, "button") });
    assert.equal(clicked.status, "succeeded", JSON.stringify(clicked));
    const status = await t.succeed("browser_dialog", { tab, action: "status" });
    assert.equal(status.facts.present, true, JSON.stringify(status));
    return status;
  };
  // The type is a fact; the bounded message is in the sentence the service says.
  const assertDialog = (status, type, message) => {
    assert.equal(status.facts.dialog_type, type, JSON.stringify(status));
    assert.match(status.summary, new RegExp(`visible: "${message}`), status.summary);
  };
  let status = await raise("Raise alert");
  assertDialog(status, "alert", "Alert message");
  await t.succeed("browser_dialog", { tab, action: "accept" });
  assert.equal(await t.evalIn(tag, "window.alerts"), 1);

  status = await raise("Raise confirm");
  assertDialog(status, "confirm", "Confirm message");
  await t.succeed("browser_dialog", { tab, action: "dismiss" });
  assert.equal(await t.evalIn(tag, "window.confirmed"), false);
  await raise("Raise confirm");
  await t.succeed("browser_dialog", { tab, action: "accept" });
  assert.equal(await t.evalIn(tag, "window.confirmed"), true);

  status = await raise("Raise prompt");
  assertDialog(status, "prompt", "Prompt message");
  await t.succeed("browser_dialog", { tab, action: "respond", text: "Typed answer" });
  assert.equal(await t.evalIn(tag, "window.answer"), "Typed answer");
  await raise("Raise prompt");
  await t.succeed("browser_dialog", { tab, action: "dismiss" });
  assert.equal(await t.evalIn(tag, "window.answer"), null);

  // A fill is a world call: the field's input listener raises an alert while the call is running.
  // The call must come back promptly with the dialog named, long before its own deadline.
  const trap = await t.targetNamed(tab, "Trap field", "textbox");
  const started = Date.now();
  const filled = await t.call("browser_fill_form", { tab, fields: [{ target: trap, value: "trigger" }], timeout_ms: 30000 }, { timeoutMs: 60000 });
  const elapsed = Date.now() - started;
  assert.ok(elapsed < 15000, `the fill waited ${elapsed} ms instead of answering when the dialog opened: ${JSON.stringify(filled)}`);
  assert.notEqual(filled.status, "succeeded", `a fill that raised a dialog cannot be reported as done: ${JSON.stringify(filled)}`);
  assert.doesNotMatch(JSON.stringify(filled), /deadline|timed out/i, `the fill ended on its deadline: ${JSON.stringify(filled)}`);
  assert.match(JSON.stringify(filled), /dialog/i, `the outcome does not point at the dialog: ${JSON.stringify(filled)}`);
  const raised = await t.succeed("browser_dialog", { tab, action: "status" });
  assert.equal(raised.facts.present, true, `the fill's alert is not reported: ${JSON.stringify(raised)}`);
  assertDialog(raised, "alert", "Raised by the trap field");
  await t.succeed("browser_dialog", { tab, action: "accept" });
  assert.equal(await t.evalIn(tag, "window.trapAlerts"), 1);
  assert.equal((await t.succeed("browser_dialog", { tab, action: "status" })).facts.present, false, "the dialog is still reported after accept");

  // An evaluation runs in the main world through its own path; its alert is a separate case.
  const evaluated = await t.call("browser_execute", { tab, script: "alert('Raised by an evaluation'); return 1;", timeout_ms: 8000 });
  assert.notEqual(evaluated.status, "failed", JSON.stringify(evaluated));
  const open = await t.succeed("browser_dialog", { tab, action: "status" });
  assert.equal(open.facts.present, true, `the evaluation's alert is not reported: ${JSON.stringify(evaluated)}`);
  assertDialog(open, "alert", "Raised by an evaluation");
  await t.succeed("browser_dialog", { tab, action: "accept" });
  return `fill answered in ${elapsed} ms with ${filled.status}; evaluation reported ${evaluated.status}`;
});

check("editor", "(7) contenteditable keeps retained values and the submission counter", async (t) => {
  const tab = await t.openFixture("/editor");
  const tag = t.tag;
  const reply = await t.targetNamed(tab, "Reply", "textbox");
  await t.succeed("browser_type_text", { tab, target: reply, text: "Native line", clear_first: true });
  let evidence = await t.evalIn(tag, "editorEvidence()");
  assert.equal(evidence.reply.value, "Native line", JSON.stringify(evidence));
  assert.equal(evidence.reply.rendered, "Native line");
  assert.ok(evidence.reply.native > 0 && evidence.reply.synthetic === 0, JSON.stringify(evidence.reply));
  assert.equal(evidence.ordinary, "Original input");
  assert.equal(evidence.submissions, 0);
  await t.succeed("browser_type_text", { tab, target: await t.targetNamed(tab, "Ordinary draft", "textbox"), text: "Typed ordinary", clear_first: true });
  evidence = await t.evalIn(tag, "editorEvidence()");
  assert.equal(evidence.ordinaryModel.value, "Typed ordinary", JSON.stringify(evidence));
  assert.equal(evidence.reply.rendered, "Native line");
  await t.succeed("browser_click", { tab, target: await t.targetNamed(tab, "Send", "button") });
  assert.equal((await t.evalIn(tag, "editorEvidence()")).submissions, 1);
});

check("fill-form", "(8) fill_form with text, select by label, checkbox, password; read masks the credential", async (t) => {
  const tab = await t.openFixture("/form");
  const tag = t.tag;
  const items = await t.controls(tab);
  const target = (name) => {
    const item = items.find((entry) => entry.name === name || entry.name?.startsWith(name));
    assert.ok(item, `no control ${name}: ${JSON.stringify(items.map((entry) => entry.name))}`);
    return item.target;
  };
  const filled = await t.succeed("browser_fill_form", {
    tab,
    fields: [
      { target: target("Full name"), value: "Ada Lovelace" },
      { target: target("Plan"), value: "Team" },
      { target: target("Subscribe to updates"), value: true },
      { target: target("Access code"), value: SECRET }
    ],
    user_authorized_credentials: true,
    submit_target: target("Submit form")
  }, { timeoutMs: 90000 });
  await t.succeed("browser_wait", { tab, condition: "text_present", value: "Form submitted", timeout_ms: 15000 });
  assert.deepEqual(await t.evalIn(tag, "window.submitted"), { name: "Ada Lovelace", plan: "team-plan", subscribe: true, code: SECRET }, JSON.stringify(filled.facts));
  const read = await t.succeed("browser_read", { tab, max_chars: 20000 });
  assert.ok(!JSON.stringify(read).includes(SECRET), "browser_read returned the credential");
  assert.match(read.facts.text, /\*{8,}/, "the credential field is not masked");
  assert.match(read.facts.text, /Subscribe to updates/);
  assert.match(read.facts.text, /(^|\s)checked(\s|$)/, "checkbox state is not reported");
});

check("press-key", "(9) press_key Tab and Enter, and Control+A dropping text", async (t) => {
  const tab = await t.openFixture("/keys");
  const tag = t.tag;
  const first = await t.targetNamed(tab, "First key field", "textbox");
  await t.succeed("browser_click", { tab, target: first });
  await t.succeed("browser_press_key", { tab, key: "Tab" });
  assert.equal(await t.evalIn(tag, "document.activeElement.id"), "k2");
  await t.succeed("browser_press_key", { tab, target: first, key: "Enter" });
  assert.ok(await t.evalIn(tag, "window.enters") >= 1, "Enter never reached the field");
  await t.succeed("browser_type_text", { tab, target: first, text: "dropme", clear_first: true });
  assert.equal(await t.evalIn(tag, "document.getElementById('k1').value"), "dropme");
  await t.succeed("browser_press_key", { tab, target: first, key: "a", modifiers: [MODIFIER] });
  await t.succeed("browser_press_key", { tab, target: first, key: "Backspace" });
  assert.equal(await t.evalIn(tag, "document.getElementById('k1').value"), "");
});

check("wait", "(10) wait: text_present, text_absent, url_contains, load_ready, visual_settle and more", async (t) => {
  const tab = await t.openFixture("/waits");
  await t.succeed("browser_wait", { tab, condition: "load_ready" });
  const clickQuiet = async (name) => t.succeed("browser_click", { tab, target: await t.targetNamed(tab, name, "button"), visual_settle: false });
  await clickQuiet("Reveal text");
  const started = Date.now();
  await t.succeed("browser_wait", { tab, condition: "text_present", value: "Delayed line arrived", timeout_ms: 15000 });
  assert.ok(Date.now() - started > 200, "text_present returned before the delay");
  await clickQuiet("Hide text");
  await t.succeed("browser_wait", { tab, condition: "text_absent", value: "Vanishing line", timeout_ms: 15000 });
  await clickQuiet("Change route");
  await t.succeed("browser_wait", { tab, condition: "url_contains", value: "#ready", timeout_ms: 15000 });
  await clickQuiet("Animate box");
  await t.succeed("browser_wait", { tab, condition: "visual_settle", timeout_ms: 15000 });
  await t.succeed("browser_wait", { tab, condition: "target_present", target: await t.targetNamed(tab, "Reveal text", "button") });
  await t.succeed("browser_wait", { tab, condition: "selector_present", selector: { name: "Animate box", role: "button", exact: true } });
  await t.succeed("browser_wait", { tab, condition: "duration", value: "300" });
  const absent = await t.call("browser_wait", { tab, condition: "text_present", value: "this text never appears", timeout_ms: 1500 });
  assert.equal(absent.status, "failed", `a false condition must be a decisive failure: ${JSON.stringify(absent)}`);
});

check("screenshot", "(11) screenshot: viewport, full page, target region within budget", async (t) => {
  const tab = await t.openFixture("/tall");
  const viewport = await t.succeed("browser_screenshot", { tab }, { timeoutMs: 90000 });
  const full = await t.succeed("browser_screenshot", { tab, full_page: true }, { timeoutMs: 90000 });
  const target = await t.succeed("browser_screenshot", { tab, target: await t.targetNamed(tab, "Capture target", "button") }, { timeoutMs: 90000 });
  for (const [label, shot] of [["viewport", viewport], ["full page", full], ["target", target]]) {
    const image = imageOf(shot);
    assert.ok(image, `${label} returned no image`);
    assert.equal(image.mimeType, "image/jpeg");
    assert.ok(image.data.length > 1000 && image.data.length <= IMAGE_BUDGET_CHARS, `${label} image is ${image.data.length} base64 characters`);
    assert.match(shot.facts.view, /^view_/);
  }
  assert.ok(full.facts.height > viewport.facts.height, `full page ${full.facts.height} is not taller than viewport ${viewport.facts.height}`);
  assert.ok(target.facts.width < viewport.facts.width && target.facts.height < viewport.facts.height, "the target capture is not a region");
  const region = await t.succeed("browser_screenshot", {
    tab, view: viewport.facts.view, x: 10, y: 10, width: Math.floor(viewport.facts.width / 2), height: Math.floor(viewport.facts.height / 2)
  }, { timeoutMs: 90000 });
  assert.notEqual(region.facts.view, viewport.facts.view, "a region capture must mint its own view");
});

check("scroll-hover", "(12) scroll, hover and a wheel at a point", async (t) => {
  const tab = await t.openFixture("/scroll");
  const tag = t.tag;
  await t.succeed("browser_scroll", { tab, direction: "down", amount: "page" });
  assert.ok(await t.evalIn(tag, "scrollY") > 0, "the page did not scroll");
  await t.succeed("browser_scroll", { tab, direction: "up", amount: "large" });
  await t.succeed("browser_hover", { tab, target: await t.targetNamed(tab, "Hover me", "button") });
  assert.equal(await t.evalIn(tag, "window.hovered"), true);
  const { view, scale } = await t.shot(tab, tag);
  const pad = await t.centre(tag, "#wheel", scale);
  await t.succeed("browser_hover", { tab, view, x: pad.x, y: pad.y });
  assert.equal(await t.evalIn(tag, "window.padHover"), true);
  await t.succeed("browser_scroll", { tab, direction: "down", view, x: pad.x, y: pad.y, ticks: 3 });
  assert.ok(await t.evalIn(tag, "window.wheelY") > 0, "the wheel never reached the pad");
});

check("drag", "(13) drag on an HTML5 list and a pointer drag", async (t) => {
  const tab = await t.openFixture("/dnd");
  const tag = t.tag;
  const destination = (await t.matches(tab, "Drop here"))[0].target;
  await t.succeed("browser_drag", { tab, source_target: await t.targetNamed(tab, "Drag Alpha", "button"), destination_target: destination });
  assert.equal(await t.evalIn(tag, "window.dropped"), "alpha");
  const { view, scale } = await t.shot(tab, tag);
  const track = await t.centre(tag, "#track", scale);
  const reach = Math.round(60 * scale);
  await t.succeed("browser_drag", { tab, view, start_x: track.x - reach, start_y: track.y, end_x: track.x + reach, end_y: track.y });
  const drag = await t.evalIn(tag, "window.pointerDrag");
  assert.ok(drag && drag.dx > 60, `the pointer drag moved ${JSON.stringify(drag)}`);
});

check("upload", "(14) upload into a file input and a drop at a point", async (t) => {
  const tab = await t.openFixture("/upload");
  const tag = t.tag;
  const input = await t.targetNamed(tab, "Evidence file");
  await t.succeed("browser_upload", { tab, target: input, files: [{ name: "inline.txt", media_type: "text/plain", data_base64: Buffer.from("inline journey bytes").toString("base64") }] });
  assert.equal(await t.evalIn(tag, "document.getElementById('file').files[0].text()"), "inline journey bytes");
  const path = join(repository, "tests/fixtures/contenteditable.html");
  await t.succeed("browser_upload", { tab, target: input, paths: [path] });
  assert.equal(await t.evalIn(tag, "document.getElementById('file').files[0].text()"), readFileSync(path, "utf8"));
  const { shot, view, scale } = await t.shot(tab, tag);
  assert.match(shot.facts.image, /^image_/, JSON.stringify(shot.facts));
  const pad = await t.centre(tag, "#drop", scale);
  await t.succeed("browser_upload", { tab, source_image: shot.facts.image, view, x: pad.x, y: pad.y });
  const dropped = await t.evalIn(tag, "window.dropped");
  assert.equal(dropped?.length, 1, JSON.stringify(dropped));
  assert.match(dropped[0].type, /^image\//);
});

check("execute", "(15) execute: a value, a throw as unknown effect, a loop ended by its deadline", async (t) => {
  const tab = await t.openFixture("/home");
  assert.equal((await t.succeed("browser_execute", { tab, script: "document.title" })).facts.value, "Installed home");
  assert.equal((await t.succeed("browser_execute", { tab, script: "return await Promise.resolve(42);" })).facts.value, 42);
  const thrown = await t.call("browser_execute", { tab, script: "throw new Error('journey boom');" });
  assert.notEqual(thrown.status, "succeeded", JSON.stringify(thrown));
  assert.equal(thrown.effect, "unknown", JSON.stringify(thrown));
  const started = Date.now();
  const looped = await t.call("browser_execute", { tab, script: "while (true) {}", timeout_ms: 3000 }, { timeoutMs: 45000 });
  assert.notEqual(looped.status, "succeeded", JSON.stringify(looped));
  assert.ok(Date.now() - started < 40000, "the loop outlived its deadline");
  await t.succeed("browser_read", { tab }, { timeoutMs: 30000 });
  return `throw -> ${thrown.status}/${thrown.effect}; loop -> ${looped.status}/${looped.effect}`;
});

check("record", "(16) record: start, status, stop, return to client, download, attach to page, discard", async (t) => {
  const tab = await t.openFixture("/upload");
  const tag = t.tag;
  const started = await t.succeed("browser_record", { action: "start", tab });
  const recording = started.facts.recording;
  assert.match(recording, /^recording_/, JSON.stringify(started.facts));
  const status = await t.succeed("browser_record", { action: "status", recording });
  assert.ok(status.facts, JSON.stringify(status));
  await t.succeed("browser_hover", { tab, target: await t.targetNamed(tab, "Evidence file") });
  await t.succeed("browser_wait", { tab, condition: "duration", value: "1200" });
  await t.succeed("browser_record", { action: "stop", recording });
  const returned = await t.succeed("browser_record", { action: "save", recording }, { timeoutMs: 90000 });
  assert.ok(returned.raw.content.some((item) => item.type === "image" && item.mimeType === "image/gif"), "no GIF came back to the client");
  const downloaded = await t.succeed("browser_record", { action: "save", recording, download: true }, { timeoutMs: 90000 });
  assert.ok(!downloaded.raw.content.some((item) => item.type === "image"), "a download must not return the image");
  await t.succeed("browser_record", { action: "save", recording, target: await t.targetNamed(tab, "Evidence file") }, { timeoutMs: 90000 });
  assert.equal(await t.evalIn(tag, "document.getElementById('file').files[0]?.type"), "image/gif");
  await t.succeed("browser_record", { action: "discard", recording });
});

check("diagnose", "(17) diagnose console and network on a logging page", async (t) => {
  const tab = await t.openFixture("/console");
  await t.succeed("browser_diagnose", { tab, source: "both", detail: "all" });
  await t.succeed("browser_click", { tab, target: await t.targetNamed(tab, "Emit diagnostics", "button") });
  await t.succeed("browser_wait", { tab, condition: "duration", value: "700" });
  const consoleLines = await t.succeed("browser_diagnose", { tab, source: "console", detail: "all" });
  assert.match(JSON.stringify(consoleLines.facts), /Installed sentinel warning/);
  assert.match(JSON.stringify(consoleLines.facts), /Installed sentinel error/);
  const network = await t.succeed("browser_diagnose", { tab, source: "network", detail: "all" });
  assert.match(JSON.stringify(network.facts), /ping-ok/);
  const problems = await t.succeed("browser_diagnose", { tab, source: "network", detail: "problems" });
  assert.match(JSON.stringify(problems.facts), /missing-asset/);
  assert.doesNotMatch(JSON.stringify(problems.facts), /ping-ok/);
});

check("window", "(18) window zoom 150 then 100, and resize", async (t) => {
  const tab = await t.openFixture("/home");
  const tag = t.tag;
  const original = await t.evalIn(tag, "devicePixelRatio");
  await t.succeed("browser_window", { tab, action: "zoom", percent: 150 });
  try { assert.ok(await t.evalIn(tag, "devicePixelRatio") > original, "zoom 150 did not change the page scale"); }
  finally { await t.succeed("browser_window", { tab, action: "zoom", percent: 100 }); }
  assert.equal(await t.evalIn(tag, "devicePixelRatio"), original);
  await t.succeed("browser_window", { tab, action: "resize", width: 1000, height: 760 });
  const outer = await t.evalIn(tag, "[outerWidth, outerHeight]");
  assert.ok(within(outer[0], 1000, 24) && within(outer[1], 760, 24), `outer size is ${outer}`);
});

const COMPOSED_PROOF = "Verify composed frame";
const COMPOSED_PROVEN = "Composed frame verified";

/** Find and press the proof control in the open shadow root of the composed Sylin stage, then read its new label. */
async function verifyComposedFrame(t, tab, urlPart) {
  const proof = (await t.matches(tab, COMPOSED_PROOF, "button"))[0];
  await t.succeed("browser_click", { tab, target: proof.target });
  const label = await t.evalIn(urlPart, "document.querySelector('composed-iframe-stage').shadowRoot.querySelector('frame-proof [slot=label]').textContent");
  assert.equal(label, COMPOSED_PROVEN, "the shadow-hosted proof control did not change its label");
  assert.ok((await t.matches(tab, COMPOSED_PROVEN)).some((match) => match.name?.includes(COMPOSED_PROVEN)), "the changed label is not found");
}

check("cross-origin-frames", "(19) two-level cross-origin iframes: read, find, click, type; identical embeds resolved", async (t) => {
  const tab = await t.openFixture("/frames-outer");
  await t.succeed("browser_wait", { tab, condition: "text_present", value: "Inner form", timeout_ms: 30000 });
  const read = await t.succeed("browser_read", { tab, mode: "visible", max_chars: 20000 });
  for (const text of ["Outer page", "Mid frame", "Inner form"]) assert.match(read.facts.text, new RegExp(text), `read lacks ${text}`);
  const notes = await t.matches(tab, "Inner note", "textbox");
  assert.ok(notes.length >= 2 && notes[0].target !== notes[1].target, `identical embeds were not told apart: ${JSON.stringify(notes)}`);
  const since = Date.now();
  await t.succeed("browser_type_text", { tab, target: notes[0].target, text: "first embed" });
  await t.succeed("browser_type_text", { tab, target: notes[1].target, text: "second embed" });
  const sends = await t.matches(tab, "Inner send", "button");
  await t.succeed("browser_click", { tab, target: sends[0].target });
  await sleep(500);
  const typed = new Map();
  for (const event of t.eventsOf("inner-input", since)) typed.set(event.frame, event.value);
  assert.equal(typed.size, 2, `two embeds should hold two values: ${JSON.stringify([...typed])}`);
  assert.deepEqual([...typed.values()].sort(), ["first embed", "second embed"]);
  assert.equal(t.eventsOf("inner-send", since).length, 1, "exactly one embed should have been clicked");

  const snapshot = await t.open(`${t.A}/sylin?${t.tag}`);
  await t.succeed("browser_wait", { tab: snapshot, condition: "text_present", value: "Project name", timeout_ms: 30000 });
  await t.succeed("browser_type_text", { tab: snapshot, target: (await t.matches(snapshot, "Project name", "textbox"))[0].target, text: "Snapshot project" });
  assert.match((await t.succeed("browser_read", { tab: snapshot, max_chars: 20000 })).facts.text, /Snapshot project/);
  await verifyComposedFrame(t, snapshot, `/sylin?${t.tag}`);
});

check("shadow-dom", "(20) open shadow DOM: find, click, type", async (t) => {
  const tab = await t.openFixture("/shadow");
  const tag = t.tag;
  await t.succeed("browser_click", { tab, target: (await t.matches(tab, "Shadow action", "button"))[0].target });
  assert.equal(await t.evalIn(tag, "window.shadowClicks"), 1);
  await t.succeed("browser_type_text", { tab, target: (await t.matches(tab, "Shadow field", "textbox"))[0].target, text: "inside the root" });
  assert.equal(await t.evalIn(tag, "document.getElementById('host').shadowRoot.querySelector('input').value"), "inside the root");
});

check("strict-csp", "(21) strict CSP and Trusted Types: overlay and click work with no Ghostlight violation", async (t) => {
  const tab = await t.openFixture("/csp");
  const tag = t.tag;
  await t.succeed("browser_diagnose", { tab, source: "console", detail: "all" });
  await t.succeed("browser_click", { tab, target: await t.targetNamed(tab, "Count", "button") });
  assert.equal(await t.evalIn(tag, "window.clicks"), 1);
  await until(() => t.evalIn(tag, "Boolean(document.getElementById('ghostlight-presentation-root'))"), "the presentation overlay to mount on the strict page", 15000, 250);
  await t.succeed("browser_screenshot", { tab }, { timeoutMs: 90000 });
  const violations = await t.evalIn(tag, "window.violations");
  assert.deepEqual(violations, [], `the page saw policy violations: ${JSON.stringify(violations)}`);
  const logged = await t.succeed("browser_diagnose", { tab, source: "console", detail: "all" });
  assert.doesNotMatch(JSON.stringify(logged.facts), /Content Security Policy|Trusted ?Type|Refused to/i);
  const extra = await t.evalIn(tag, "document.getElementsByTagName('*').length - window.baseElements");
  return `${extra} elements added to the page by the overlay or adapter`;
});

check("beforeunload", "(22) a beforeunload page, navigated away with the discard path", async (t) => {
  const tab = await t.openFixture("/beforeunload");
  const tag = t.tag;
  await t.succeed("browser_click", { tab, target: await t.targetNamed(tab, "Touch page", "button") });
  await t.succeed("browser_type_text", { tab, target: await t.targetNamed(tab, "Draft", "textbox"), text: "unsaved" });
  const away = `${t.A}/blank?away`;
  const stopped = await t.call("browser_navigate", { tab, url: away }, { timeoutMs: 45000 });
  assert.notEqual(stopped.status, "succeeded", `the unload prompt should stop the navigation: ${JSON.stringify(stopped)}`);
  assert.equal(await t.evalIn(tag, "location.pathname"), "/beforeunload");
  await t.succeed("browser_navigate", { tab, url: away, beforeunload: "discard" }, { timeoutMs: 45000 });
  assert.match(await t.evalIn("away", "location.href"), /\/blank\?away$/);
});

check("flow", "(23) flow with scroll and wait, and a flow_ref between steps", async (t) => {
  const tab = await t.openFixture("/scroll");
  const flow = await t.succeed("browser_flow", {
    tab,
    steps: [
      { tool: "browser_scroll", arguments: { direction: "down", amount: "page" } },
      { tool: "browser_wait", arguments: { condition: "text_present", value: "Bottom marker" } }
    ]
  }, { timeoutMs: 90000 });
  assert.equal(flow.facts.steps.length, 2);
  assert.ok(flow.facts.steps.every((step) => step.status === "succeeded"), JSON.stringify(flow.facts.steps));
  const chained = await t.succeed("browser_flow", {
    tab,
    steps: [
      { id: "title", tool: "browser_execute", arguments: { script: "document.title" } },
      { id: "find", tool: "browser_find", arguments: { text: { flow_ref: { step: "title", pointer: "/facts/value" } } } }
    ]
  }, { timeoutMs: 90000 });
  assert.equal(chained.facts.steps.length, 2);
});

check("history", "(24) history back and forward", async (t) => {
  const tab = await t.openFixture("/home");
  const tag = t.tag;
  await t.succeed("browser_navigate", { tab, url: `${t.A}/scroll?${tag}` });
  assert.equal(await t.evalIn(tag, "location.pathname"), "/scroll");
  await t.succeed("browser_history", { tab, action: "back" });
  assert.equal(await t.evalIn(tag, "location.pathname"), "/home");
  await t.succeed("browser_history", { tab, action: "forward" });
  assert.equal(await t.evalIn(tag, "location.pathname"), "/scroll");
  await t.succeed("browser_history", { tab, action: "reload", bypass_cache: true });
  assert.equal(await t.evalIn(tag, "location.pathname"), "/scroll");
});

check("policy", "(25) policy_explain, and a user policy refusing the second origin with honest wording", async (t) => {
  const previous = existsSync(installed.policy) ? readFileSync(installed.policy) : null;
  const backup = previous ? join(tmpdir(), `ghostlight-user-policy-backup-${process.pid}.json`) : null;
  if (backup) writeFileSync(backup, previous);
  const restore = () => { if (previous) writeFileSync(installed.policy, previous); else if (existsSync(installed.policy)) unlinkSync(installed.policy); };
  try {
    const explained = await t.succeed("policy_explain", {});
    assert.ok(JSON.stringify(explained.facts).length > 20, "policy_explain returned nothing");
    await t.open(`${t.B}/blank?policy-before`);
    mkdirSync(join(ghostlightHome, "policy"), { recursive: true });
    writeFileSync(installed.policy, JSON.stringify({
      schema: 4, name: "Installed journey", version: String(Date.now()),
      grants: [{ id: "first-origin", hosts: { allow: ["127.0.0.1"] }, allowed: ["read", "action", "write", "execute"] }]
    }));
    await policyRefreshed();
    const refused = await t.call("browser_navigate", { url: `${t.B}/blank?policy-denied`, new_tab: true, browser: t.browser });
    assert.equal(refused.status, "blocked", `the second origin should be refused: ${JSON.stringify(refused)}`);
    assert.match(refused.summary, /localhost/i, refused.summary);
    assert.match(refused.summary, /polic|not allowed|permitted|allow/i, refused.summary);
    await t.open(`${t.A}/blank?policy-allowed`);
    const governed = await t.succeed("policy_explain", {});
    assert.match(JSON.stringify(governed.facts), /127\.0\.0\.1/);
    restore();
    await policyRefreshed();
    await t.open(`${t.B}/blank?policy-after`);
    return refused.summary;
  } finally {
    restore();
    if (backup && existsSync(backup)) unlinkSync(backup);
  }
});

check("restart-continuity", "(26) a quit: the next call starts the service, the quit released the earlier tab, the extension reconnects", async (t) => {
  const tab = await t.openFixture("/home");
  const before = await serviceStatus();
  assert.ok(before, "a service is running before the quit");
  await quitService();
  await until(async () => !(await serviceStatus()), "the old service to stop", 30000, 250);
  const started = Date.now();
  // One call, which starts the service again. The quit released the session's tab and handed the
  // session a token without it, so the earlier handle names no open tab (ADR-0203 Decision 3).
  const released = await t.call("browser_read", { tab, max_chars: 2000 }, { timeoutMs: RESTART_TIMEOUT_MS });
  assert.notEqual(released.status, "succeeded", `the quit kept the earlier tab: ${JSON.stringify(released)}`);
  // A fresh tab opens and reads once the extension reconnects. Only a browser that has not yet
  // reconnected may need a repeat; a cut-off or expired answer is a failure of the hold.
  t.tag = `t${++t.tagCounter}x`;
  const fresh = { url: `${t.A}/home?${t.tag}`, new_tab: true, ...(t.browser ? { browser: t.browser } : {}) };
  let opened = await t.call("browser_navigate", fresh, { timeoutMs: RESTART_TIMEOUT_MS });
  let retries = 0;
  while (opened.status === "failed" && BROWSER_RECONNECTING.includes(opened.facts?.reason) && Date.now() - started < RESTART_TIMEOUT_MS) {
    retries++;
    await sleep(2000);
    opened = await t.call("browser_navigate", fresh, { timeoutMs: RESTART_TIMEOUT_MS });
  }
  assert.equal(opened.status, "succeeded", `no tab opened after the restart: ${JSON.stringify(opened)}`);
  t.opened.push(opened.facts.tab);
  await t.succeed("browser_read", { tab: opened.facts.tab, max_chars: 2000 });
  const after = await serviceStatus();
  assert.notEqual(after?.instance, before.instance, "the service instance is unchanged, so no restart happened");
  assert.equal(after.service, before.service);
  await t.popupText();
  return `restarted in ${Date.now() - started} ms after ${retries} reconnect retries`;
});

check("quit-holds", "(27) after a quit with nothing calling, the service stays down; a call starts it", async (t) => {
  assert.ok(await serviceStatus(), "a service is running before the quit");
  await quitService();
  await until(async () => !(await serviceStatus()), "the old service to stop", 30000, 250);
  await sleep(QUIT_HOLD_MS);
  assert.equal(await serviceStatus(), null, "a service is running again after a quit");
  const outcome = await t.call("browser_tabs", { action: "list" }, { timeoutMs: RESTART_TIMEOUT_MS });
  assert.equal(outcome.status, "succeeded", JSON.stringify(outcome));
  assert.ok(await serviceStatus(), "a call did not start the service");
});

check("one-service", "exactly one service process and no protocol errors in the diagnostics", async (t, run) => {
  const services = await processCount(SERVICE_PROCESS);
  assert.equal(services, 1, `${services} ghostlight-service processes are running`);
  if (!existsSync(installed.diagnostics)) return "diagnostics are off; nothing to scan";
  const recent = readdirSync(installed.diagnostics).filter((name) => name.endsWith(".jsonl"))
    .map((name) => join(installed.diagnostics, name)).filter((path) => statSync(path).mtimeMs >= run.startedAt);
  const bad = [];
  for (const path of recent) for (const line of readFileSync(path, "utf8").split(/\r?\n/)) if (/protocol[_ ]error/i.test(line)) bad.push(line.slice(0, 200));
  assert.deepEqual(bad, [], "protocol errors in the diagnostics");
  return `${recent.length} diagnostics files scanned`;
});

// ---------------------------------------------------------------------------------------------
// Live demo pages (--live). The public pages are not served by this repository, so these checks
// lean on the content the demo scripts and the iframe snapshot already depend on and otherwise
// exercise the page generically.
// ---------------------------------------------------------------------------------------------

const LIVE_CHECKS = [];
const live = (name, title, run) => LIVE_CHECKS.push({ name, title, run });

async function liveOpen(t, path) {
  return t.open(`${LIVE_ROOT}/${path}`);
}

live("live-desk", "live /demo/desk/: read, find the bell, click, dialog message, accept, answer, ring again, dismiss", async (t) => {
  const tab = await liveOpen(t, "desk/");
  assert.match((await t.succeed("browser_read", { tab, max_chars: 20000 })).facts.text, /bell/i);
  const bell = (await t.matches(tab, "Ring the bell", "button"))[0].target;
  let clicked = await t.call("browser_click", { tab, target: bell });
  assert.equal(clicked.status, "succeeded", JSON.stringify(clicked));
  const status = await t.succeed("browser_dialog", { tab, action: "status" });
  assert.equal(status.facts.present, true, JSON.stringify(status));
  assert.match(status.summary, /visible: ".+"/, "dialog status carries no message");
  await t.succeed("browser_dialog", { tab, action: "respond", text: "Ghostlight was here" });
  await t.succeed("browser_wait", { tab, condition: "text_present", value: "the bell says", timeout_ms: 15000 });
  clicked = await t.call("browser_click", { tab, target: bell });
  assert.equal(clicked.status, "succeeded", JSON.stringify(clicked));
  await t.succeed("browser_dialog", { tab, action: "dismiss" });
  await t.succeed("browser_wait", { tab, condition: "text_present", value: "dismissed without an answer", timeout_ms: 15000 });
});

live("live-form", "live /demo/form/: inspect controls, fill with select and checkbox, masked credentials, submit, completion", async (t) => {
  const tab = await liveOpen(t, "form/");
  const before = (await t.succeed("browser_read", { tab, max_chars: 20000 })).facts.text;
  const controls = await t.controls(tab, 200);
  assert.ok(controls.length > 0, "the form page exposes no controls");
  const fields = [];
  for (const control of controls) {
    if (control.role === "textbox" && !/password|secret|token|code/i.test(control.name ?? "")) fields.push({ target: control.target, value: /mail/i.test(control.name ?? "") ? "journey@example.com" : "Journey text" });
    else if (control.role === "checkbox") fields.push({ target: control.target, value: true });
  }
  assert.ok(fields.length > 0, "nothing fillable was found");
  const submit = controls.find((control) => control.role === "button" && /submit|send|create|complete|apply|save/i.test(control.name ?? ""));
  assert.ok(submit, `no submit-like button: ${controls.map((control) => `${control.role}:${control.name}`).join(", ")}`);
  await t.succeed("browser_fill_form", { tab, fields, submit_target: submit.target }, { timeoutMs: 90000 });
  await until(async () => (await t.succeed("browser_read", { tab, max_chars: 20000 })).facts.text !== before, "the form to show completion", 20000, 500);
  const after = (await t.succeed("browser_read", { tab, max_chars: 20000 })).facts.text;
  assert.ok(!after.includes("journey-secret"), "a credential reached read output");
});

live("live-signals", "live /demo/signals/: every wait condition, diagnose console and network", async (t) => {
  const tab = await liveOpen(t, "signals/");
  const text = (await t.succeed("browser_read", { tab, max_chars: 20000 })).facts.text;
  const word = text.match(/[A-Za-z]{6,}/)?.[0];
  assert.ok(word, "the signals page has no readable text");
  await t.succeed("browser_wait", { tab, condition: "load_ready" });
  await t.succeed("browser_wait", { tab, condition: "text_present", value: word, timeout_ms: 15000 });
  await t.succeed("browser_wait", { tab, condition: "text_absent", value: "text that is not on this page", timeout_ms: 15000 });
  await t.succeed("browser_wait", { tab, condition: "url_contains", value: "signals" });
  await t.succeed("browser_wait", { tab, condition: "visual_settle", timeout_ms: 15000 });
  await t.succeed("browser_wait", { tab, condition: "duration", value: "300" });
  const first = (await t.controls(tab))[0];
  if (first) await t.succeed("browser_wait", { tab, condition: "target_present", target: first.target });
  await t.succeed("browser_diagnose", { tab, source: "both", detail: "all", limit: 100 });
  await t.succeed("browser_history", { tab, action: "reload" });
  await t.succeed("browser_diagnose", { tab, source: "console", detail: "all", limit: 100 });
  await t.succeed("browser_diagnose", { tab, source: "network", detail: "all", limit: 100 });
});

live("live-reading", "live /demo/reading/: article read, find, scroll, full-page screenshot", async (t) => {
  const tab = await liveOpen(t, "reading/");
  const article = await t.succeed("browser_read", { tab, mode: "article", max_chars: 50000 });
  const word = article.facts.text.match(/[A-Za-z]{8,}/)?.[0];
  assert.ok(word, "no article text");
  await t.matches(tab, word);
  await t.succeed("browser_scroll", { tab, direction: "down", amount: "page" });
  const full = await t.succeed("browser_screenshot", { tab, full_page: true }, { timeoutMs: 90000 });
  assert.ok(imageOf(full), "no full-page image");
});

live("live-studio", "live /demo/studio/: hover, drag, zoom, resize, target screenshot", async (t) => {
  const tab = await liveOpen(t, "studio/");
  const controls = await t.controls(tab, 100);
  assert.ok(controls.length > 0, "the studio exposes no controls");
  await t.succeed("browser_hover", { tab, target: controls[0].target });
  const draggable = controls.filter((control) => /drag/i.test(control.name ?? ""));
  if (draggable.length > 0 && controls.length > 1) {
    const other = controls.find((control) => control.target !== draggable[0].target);
    await t.call("browser_drag", { tab, source_target: draggable[0].target, destination_target: other.target });
  }
  await t.succeed("browser_window", { tab, action: "zoom", percent: 150 });
  await t.succeed("browser_window", { tab, action: "zoom", percent: 100 });
  await t.succeed("browser_window", { tab, action: "resize", width: 1100, height: 800 });
  assert.ok(imageOf(await t.succeed("browser_screenshot", { tab, target: controls[0].target }, { timeoutMs: 90000 })), "no target image");
});

live("live-iframe", "live /demo/iframe/ and /demo/iframe/form/: cross-origin read, find, click and type inside the frame", async (t) => {
  const tab = await liveOpen(t, "iframe/");
  await t.succeed("browser_wait", { tab, condition: "text_present", value: "Project name", timeout_ms: 30000 });
  const project = (await t.matches(tab, "Project name", "textbox"))[0].target;
  await t.succeed("browser_type_text", { tab, target: project, text: "Live journey project" });
  assert.match((await t.succeed("browser_read", { tab, max_chars: 20000 })).facts.text, /Live journey project/);
  await verifyComposedFrame(t, tab, "/demo/iframe/");
  await t.succeed("browser_click", { tab, target: (await t.matches(tab, "Submit application", "button"))[0].target });
  const direct = await liveOpen(t, "iframe/form/");
  assert.match((await t.succeed("browser_read", { tab: direct, max_chars: 20000 })).facts.text, /Project name/);
});

live("live-shadow", "live /demo/shadow/: shadow-root find, click and type", async (t) => {
  const tab = await liveOpen(t, "shadow/");
  const controls = await t.controls(tab, 100);
  const field = controls.find((control) => control.role === "textbox");
  const button = controls.find((control) => control.role === "button");
  assert.ok(field && button, `the shadow page exposes ${controls.map((control) => `${control.role}:${control.name}`).join(", ")}`);
  await t.succeed("browser_type_text", { tab, target: field.target, text: "Shadow journey" });
  await t.succeed("browser_click", { tab, target: button.target });
  assert.match((await t.succeed("browser_read", { tab, max_chars: 20000 })).facts.text, /Shadow journey/);
});

live("live-brief", "live /demo/brief/: the page loads and reads (run scripts/demo-brief.ps1 against --hold)", async (t) => {
  const tab = await liveOpen(t, "brief/");
  const items = await t.controls(tab, 100);
  assert.ok(items.some((item) => /Project/i.test(item.name ?? "")) && items.some((item) => /Create brief/i.test(item.name ?? "")), `unexpected controls: ${items.map((item) => item.name)}`);
});

live("live-foundry", "live /demo/foundry/: the page loads and reads (run scripts/demo-foundry.ps1 against --hold)", async (t) => {
  const tab = await liveOpen(t, "foundry/");
  const items = await t.controls(tab, 200);
  assert.ok(items.some((item) => /Rotate foil proof/i.test(item.name ?? "")), `unexpected controls: ${items.map((item) => item.name)}`);
});

// ---------------------------------------------------------------------------------------------
// Running
// ---------------------------------------------------------------------------------------------

/** Whether a browser handle is the scratch Chrome: a probe page opened through it shows up there. */
async function isScratchBrowser(t, handle) {
  const probe = `${t.A}/blank?probe-${handle}-${Date.now()}`;
  const outcome = await t.call("browser_navigate", { url: probe, new_tab: true, browser: handle }).catch(() => null);
  if (outcome?.status !== "succeeded") return false;
  let mine = false;
  try {
    mine = Boolean(await until(async () => {
      const { targetInfos } = await t.pipe.send("Target.getTargets");
      return targetInfos.some((target) => target.url === probe);
    }, "the probe page to appear in the scratch browser", 3000, 250));
  } catch { /* the probe opened somewhere else */ }
  await t.call("browser_tabs", { action: "close", tab: outcome.facts.tab }).catch(() => {});
  return mine;
}

/** The handle of the scratch browser, proved by a probe even when it is the only browser listed. */
async function pickScratchBrowser(t) {
  const deadline = Date.now() + CONNECT_TIMEOUT_MS;
  const seen = new Set();
  for (;;) {
    const listed = await t.call("browser_tabs", { action: "list" }, { timeoutMs: 20000 }).catch(() => null);
    const handles = listed?.status === "succeeded" ? (listed.facts.browsers ?? []).map((browser) => browser.browser) : [];
    for (const handle of handles) {
      seen.add(handle);
      if (await isScratchBrowser(t, handle)) return handle;
    }
    if (Date.now() >= deadline) break;
    await sleep(1000);
  }
  throw new Error(seen.size === 0
    ? "No browser connected to the installed service in time."
    : `None of the connected browsers (${[...seen].join(", ")}) is the scratch browser. Remove the 2.0 extension from other profiles for the run.`);
}

async function runChecks(t, selected, run) {
  const results = [];
  for (const entry of selected) {
    const started = Date.now();
    let outcome;
    try {
      const detail = await entry.run(t, run);
      outcome = { name: entry.name, status: "pass", detail: typeof detail === "string" ? detail : undefined };
    } catch (error) {
      outcome = { name: entry.name, status: "fail", error: String(error?.message ?? error).slice(0, 1500) };
    }
    outcome.title = entry.title;
    outcome.ms = Date.now() - started;
    results.push(outcome);
    console.log(`${outcome.status === "pass" ? "PASS" : "FAIL"}  ${entry.name}  (${(outcome.ms / 1000).toFixed(1)} s)  ${outcome.status === "pass" ? (outcome.detail ?? entry.title) : outcome.error}`);
    await t.closeOpened();
  }
  return results;
}

function selectChecks(options) {
  const known = options.live ? [...CHECKS, ...LIVE_CHECKS] : CHECKS;
  if (options.only.length === 0) return known;
  const names = new Map(known.map((entry) => [entry.name, entry]));
  for (const name of options.only) {
    if (!names.has(name)) usageError(`unknown check ${name}${LIVE_CHECKS.some((entry) => entry.name === name) ? " (add --live)" : ""}; known: ${[...names.keys()].join(", ")}`);
  }
  return options.only.map((name) => names.get(name));
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  refuseUnlessInstalled();
  const chrome = options.chrome ?? defaultChrome();
  if (!chrome || !existsSync(chrome)) usageError("Chrome was not found; pass --chrome <path> to the Google Chrome executable");
  if (!existsSync(join(options.extension, "manifest.json"))) usageError(`no extension at ${options.extension}`);
  const selected = selectChecks(options);

  const run = { startedAt: Date.now(), summary: join(tmpdir(), `ghostlight-installed-journey-${new Date().toISOString().replace(/[:.]/g, "-")}.json`) };
  const scratch = createScratch();
  let fixtures, browser, mcp, results = [], setupError = null, extensionId = null;
  try {
    fixtures = await startFixtures();
    console.log(`Scratch profile: ${scratch}`);
    console.log(`Fixtures: ${fixtures.A} and ${fixtures.B}`);
    const copied = installScratchNativeHost(scratch);
    if (copied) console.log(`Native host manifest copied to ${copied}`);
    browser = launchChrome({ chrome, userDataDir: scratch, extraArgs: ["--window-size=1280,900"] });
    await browser.pipe.send("Browser.getVersion", {}, undefined, 60000);
    await browser.pipe.send("Browser.setDownloadBehavior", { behavior: "allow", downloadPath: join(scratch, "downloads") }).catch(() => {});
    extensionId = await browser.pipe.loadUnpacked(options.extension);
    assert.equal(extensionId, EXTENSION_ID, `the extension loaded as ${extensionId}; the pinned key should give ${EXTENSION_ID}`);
    console.log(`Extension ${extensionId} loaded from ${options.extension}`);

    mcp = new McpClient(installed.edge, ["mcp"]);
    await mcp.initialize();
    const t = new Context({ mcp, pipe: browser.pipe, fixtures, scratch });
    t.browser = await pickScratchBrowser(t);
    console.log(`Scratch browser handle: ${t.browser}`);
    results = await runChecks(t, selected, run);
    if (options.hold > 0) {
      console.log(`Holding the scratch browser for ${options.hold} s (Ctrl+C ends the hold). Run the demo scripts now.`);
      await new Promise((done) => { const timer = setTimeout(done, options.hold * 1000); process.once("SIGINT", () => { clearTimeout(timer); done(); }); });
    }
  } catch (error) {
    setupError = String(error?.stack ?? error);
    console.error(`SETUP FAILED: ${error?.message ?? error}`);
  } finally {
    await mcp?.close().catch(() => {});
    await browser?.pipe.close().catch(() => {});
    await fixtures?.close().catch(() => {});
    if (options.keepProfile) console.log(`Profile kept at ${scratch}`);
    else await removeScratch(scratch).catch((error) => console.error(`Could not remove ${scratch}: ${error.message}`));
  }

  const failed = results.filter((entry) => entry.status === "fail");
  const passed = !setupError && failed.length === 0 && results.length > 0;
  mkdirSync(tmpdir(), { recursive: true });
  writeFileSync(run.summary, JSON.stringify({
    started_at: new Date(run.startedAt).toISOString(), finished_at: new Date().toISOString(), chrome, extension: options.extension,
    extension_id: extensionId, live: options.live, passed, setup_error: setupError, results
  }, null, 2));
  console.log(`${results.length - failed.length} passed, ${failed.length} failed${setupError ? ", setup failed" : ""}. Summary: ${run.summary}`);
  process.exit(passed ? 0 : 1);
}

await main();
