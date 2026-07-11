// SPDX-License-Identifier: Apache-2.0 OR MIT
// Headless smoke: real extension + real binary over native messaging, driven as an MCP server
// over stdio (ADR-0026 Decision 6). See docs/tasks/maturity-1/00-design.md "Headless smoke (m06)"
// for the pinned architecture this file implements.
//
// NOT wired into CI (2026-07): the former `e2e-smoke` job hung to the runner ceiling on every
// push and was retired (see .github/workflows/ci.yml). This harness is kept for MANUAL runs
// while the hang is diagnosed; when fixed, re-add a bounded CI job as its own reviewed commit.
// Real end-to-end coverage currently lives in the Rust `e2e` tier and the `lightbox` job.

import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import {
  readFileSync,
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  chmodSync,
  rmSync,
  existsSync,
} from "node:fs";
import { tmpdir, homedir } from "node:os";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const SCRIPT_DIR = path.dirname(SCRIPT_PATH);
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..");
const EXTENSION_DIR = path.join(REPO_ROOT, "extension");
const FIXTURE_PATH = path.join(SCRIPT_DIR, "fixture.html");
const EXTENSION_ID = "cjcmhepmagomefjggkcohdbfemacojoa";
const DRY_RUN = process.argv.includes("--dry-run");
const HEADED_RETRY = process.env.GHOSTLIGHT_E2E_HEADED_RETRY === "1";

function fail(reason, code) {
  console.error(reason);
  process.exit(code === undefined ? 1 : code);
}

// Step 1: resolve the repo root (done above) and locate the binary, building it if absent.
function resolveBinaryPath() {
  const exeName = process.platform === "win32" ? "ghostlight.exe" : "ghostlight";
  const binPath = path.join(REPO_ROOT, "target", "debug", exeName);
  if (existsSync(binPath)) return binPath;
  const build = spawnSync("cargo", ["build", "--workspace"], { cwd: REPO_ROOT, stdio: "inherit" });
  if (build.status !== 0 || !existsSync(binPath)) {
    fail(`cargo build did not produce ${binPath}`);
  }
  return binPath;
}

// Derive the sibling `ghostlight-relay` executable (ADR-0051 Phase 3) beside the resolved
// `ghostlight` binary: same dir, platform suffix. `cargo build --workspace` builds both bins into
// target/debug; role (agent vs. browser) is selected at launch, not by binary name.
function siblingBin(binaryPath, name) {
  const exe = process.platform === "win32" ? `${name}.exe` : name;
  return path.join(path.dirname(binaryPath), exe);
}

// Step 4: a plain static server for the one fixture page, on an OS-assigned loopback port.
function startFixtureServer() {
  const body = readFileSync(FIXTURE_PATH);
  const server = createServer((req, res) => {
    res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
    res.end(body);
  });
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({ server, url: `http://127.0.0.1:${port}/` });
    });
  });
}

// Step 5: a temp user-data-dir carrying the native-messaging host manifest and its wrapper
// script, so Chromium resolves org.sylin.ghostlight to a process that sets GHOSTLIGHT_ENDPOINT
// before exec'ing the real binary.
function buildProfile(endpoint, binaryPath) {
  const userDataDir = mkdtempSync(path.join(tmpdir(), "ghostlight-e2e-"));

  const wrapperPath = path.join(userDataDir, "ghostlight-wrapper.sh");
  const wrapperBody = `#!/bin/sh\nexport GHOSTLIGHT_ENDPOINT=${endpoint}\nexec "${binaryPath}" "$@"\n`;
  writeFileSync(wrapperPath, wrapperBody);
  try {
    chmodSync(wrapperPath, 0o755);
  } catch {
    // best-effort on platforms without POSIX permission bits (Windows dry-run plan only)
  }

  const manifest = {
    name: "org.sylin.ghostlight",
    description: "Ghostlight native messaging host",
    path: wrapperPath,
    type: "stdio",
    allowed_origins: [`chrome-extension://${EXTENSION_ID}/`],
  };
  const manifestJson = JSON.stringify(manifest, null, 2) + "\n";

  // Chromium on Linux/macOS looks up native-messaging host manifests in fixed
  // per-user config directories, NOT relative to --user-data-dir (unlike Windows,
  // which uses the registry). We therefore write the manifest to every plausible
  // location: the user-data-dir (harmless), plus the Chromium and Chrome per-user
  // dirs under $HOME/.config (Linux) and $HOME/Library/Application Support (macOS).
  const candidateDirs = [
    path.join(userDataDir, "NativeMessagingHosts"),
    path.join(homedir(), ".config", "chromium", "NativeMessagingHosts"),
    path.join(homedir(), ".config", "google-chrome", "NativeMessagingHosts"),
    path.join(homedir(), "Library", "Application Support", "Chromium", "NativeMessagingHosts"),
    path.join(homedir(), "Library", "Application Support", "Google", "Chrome", "NativeMessagingHosts"),
  ];
  const manifestPaths = [];
  for (const dir of candidateDirs) {
    try {
      mkdirSync(dir, { recursive: true });
      const p = path.join(dir, "org.sylin.ghostlight.json");
      writeFileSync(p, manifestJson);
      manifestPaths.push(p);
    } catch {
      // best-effort: a location we cannot write (e.g. wrong platform) is skipped
    }
  }

  return { userDataDir, wrapperPath, manifestPath: manifestPaths[0], manifestPaths };
}

function cleanupProfile(userDataDir) {
  try {
    rmSync(userDataDir, { recursive: true, force: true });
  } catch {
    // best-effort cleanup; a leftover temp dir is not a test failure
  }
}

// A minimal newline-delimited JSON-RPC client over the spawned binary's stdio, matching the
// framing tests/mcp_protocol.rs's `drive` helper uses (one JSON object per line).
function createRpcClient(child) {
  const rl = readline.createInterface({ input: child.stdout, terminal: false });
  let nextId = 0;
  const pending = new Map();
  rl.on("line", (line) => {
    if (!line.trim()) return;
    let msg;
    try {
      msg = JSON.parse(line);
    } catch {
      return;
    }
    if (msg && msg.id !== undefined && pending.has(msg.id)) {
      const { resolve } = pending.get(msg.id);
      pending.delete(msg.id);
      resolve(msg);
    }
  });
  function call(method, params) {
    const id = ++nextId;
    const req = { jsonrpc: "2.0", id, method, params: params || {} };
    return new Promise((resolve) => {
      pending.set(id, { resolve });
      child.stdin.write(JSON.stringify(req) + "\n");
    });
  }
  function notify(method, params) {
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method, params: params || {} }) + "\n");
  }
  return { call, notify };
}

function toolResultText(response, label) {
  const content = response && response.result && response.result.content;
  if (!Array.isArray(content) || !content.length || typeof content[0].text !== "string") {
    throw new Error(
      `${label}: unexpected tools/call result shape: ${JSON.stringify(response)}`
    );
  }
  if (response.result.isError) {
    throw new Error(`${label}: tool call returned an error: ${content[0].text}`);
  }
  return content[0].text;
}

function extractRef(text, accessibleName) {
  const re = new RegExp(`"${accessibleName}"\\s*\\[(ref_\\d+)\\]`);
  const m = re.exec(text);
  if (!m) {
    throw new Error(
      `could not find a ref for accessible name "${accessibleName}" in read_page output; ` +
        `dumping the output verbatim for diagnosis:\n${text}`
    );
  }
  return m[1];
}

async function waitForServiceWorker(context, timeoutMs) {
  const existing = context.serviceWorkers();
  if (existing.length) return existing[0];
  try {
    return await context.waitForEvent("serviceworker", { timeout: timeoutMs });
  } catch {
    return null;
  }
}

async function launchContext(chromium, userDataDir, headless) {
  return chromium.launchPersistentContext(userDataDir, {
    channel: "chromium",
    headless,
    args: [
      `--disable-extensions-except=${EXTENSION_DIR}`,
      `--load-extension=${EXTENSION_DIR}`,
    ],
  });
}

// Re-exec this same script under xvfb-run for the one permitted headed retry, when no DISPLAY is
// available for a real headed launch. Guarded by GHOSTLIGHT_E2E_HEADED_RETRY so it can only ever
// happen once.
function reExecUnderXvfb() {
  const result = spawnSync(
    "xvfb-run",
    ["-a", process.execPath, SCRIPT_PATH, ...process.argv.slice(2)],
    {
      stdio: "inherit",
      env: { ...process.env, GHOSTLIGHT_E2E_HEADED_RETRY: "1" },
    }
  );
  process.exit(result.status === null ? 3 : result.status);
}

async function runDryRun(binaryPath, endpoint) {
  // Chrome launches the native host, so the manifest/wrapper wraps ghostlight-relay; the browser
  // role auto-detects from the chrome-extension:// origin Chrome passes (ADR-0051 Phase 3).
  const browserBin = siblingBin(binaryPath, "ghostlight-relay");
  const { server, url: fixtureUrl } = await startFixtureServer();
  const { userDataDir, wrapperPath, manifestPath } = buildProfile(endpoint, browserBin);
  const plan = {
    repoRoot: REPO_ROOT,
    binaryPath,
    endpoint,
    fixtureUrl,
    extensionDir: EXTENSION_DIR,
    extensionId: EXTENSION_ID,
    userDataDir,
    wrapperPath,
    manifestPath,
  };
  console.log(JSON.stringify(plan, null, 2));
  server.close();
  cleanupProfile(userDataDir);
  process.exit(0);
}

async function runLive(binaryPath, endpoint) {
  // ADR-0051 Phase 3: both roles are the same ghostlight-relay binary. Chrome launches it via the
  // native-messaging manifest (browser role auto-detected from the chrome-extension:// origin);
  // the MCP client launches it with an explicit `--role agent`. The `service` spawn below stays on
  // the separate `ghostlight` bin.
  const browserBin = siblingBin(binaryPath, "ghostlight-relay");
  const agentBin = siblingBin(binaryPath, "ghostlight-relay");
  const { server, url: fixtureUrl } = await startFixtureServer();
  const { userDataDir } = buildProfile(endpoint, browserBin);

  // The hub model (ADR-0030): a standalone SERVICE owns the browser link, and both the
  // extension's native-messaging host and this test's MCP client are thin ADAPTERS that dial it.
  // In production the installer registers the service to auto-start; CI has no OS supervisor, so
  // spawn it explicitly. Without it, an adapter's auto-start self-heal looks for a systemd unit
  // that does not exist and the connection fails.
  const service = spawn(binaryPath, ["service"], {
    stdio: ["ignore", "inherit", "inherit"],
    env: { ...process.env, GHOSTLIGHT_ENDPOINT: endpoint },
  });
  // Give the service a moment to claim its endpoint before the extension and adapter dial it.
  await new Promise((resolve) => setTimeout(resolve, 2000));

  let cleanup = async () => {
    try {
      service.kill();
    } catch {
      // already dead
    }
    server.close();
    cleanupProfile(userDataDir);
  };

  // Dynamic import: playwright is a devDependency of tests/e2e/, not needed for --dry-run.
  const { chromium } = await import("playwright");

  // Capture page + service-worker console so a native-messaging connect failure
  // (the extension logs chrome.runtime.lastError) is visible in the CI log.
  const browserLogs = [];
  const attachConsole = (ctx) => {
    try {
      ctx.on("console", (m) => browserLogs.push(`[${m.type()}] ${m.text()}`));
    } catch {
      // console events may not surface for service workers on this Playwright version
    }
  };

  let context = await launchContext(chromium, userDataDir, true);
  attachConsole(context);
  let sw = await waitForServiceWorker(context, 15000);
  if (!sw) {
    await context.close().catch(() => {});
    if (!process.env.DISPLAY && !HEADED_RETRY) {
      await cleanup();
      reExecUnderXvfb(); // never returns
    }
    context = await launchContext(chromium, userDataDir, false);
    attachConsole(context);
    sw = await waitForServiceWorker(context, 15000);
  }
  if (!sw) {
    await context.close().catch(() => {});
    await cleanup();
    fail("no extension service worker appeared within the retry budget", 3);
  }

  const child = spawn(agentBin, ["--role", "agent"], {
    stdio: ["pipe", "pipe", "inherit"],
    env: { ...process.env, GHOSTLIGHT_ENDPOINT: endpoint },
  });
  const rpc = createRpcClient(child);

  cleanup = async () => {
    try {
      child.kill();
    } catch {
      // already dead
    }
    try {
      service.kill();
    } catch {
      // already dead
    }
    await context.close().catch(() => {});
    server.close();
    cleanupProfile(userDataDir);
  };

  try {
    const init = await rpc.call("initialize", {});
    if (!init.result) throw new Error(`initialize did not return a result: ${JSON.stringify(init)}`);
    rpc.notify("notifications/initialized", {});

    const list = await rpc.call("tools/list", {});
    const names = (list.result && list.result.tools ? list.result.tools : []).map((t) => t.name);
    for (const required of ["navigate", "read_page", "computer", "form_input"]) {
      if (!names.includes(required)) {
        throw new Error(`tools/list missing "${required}"; got: ${names.join(", ")}`);
      }
    }

    // Bootstrap a tab: a fresh profile has no Ghostlight tab group yet, and navigate() (via
    // effectiveTabId) requires one to already exist -- it does not create tabs itself.
    const created = await rpc.call("tools/call", {
      name: "tabs_create_mcp",
      arguments: {},
    });
    const createdText = toolResultText(created, "tabs_create_mcp");
    const tabIdMatch = /Created tab (\d+)\./.exec(createdText);
    if (!tabIdMatch) {
      throw new Error(`could not parse a tab id from tabs_create_mcp output: ${createdText}`);
    }
    const tabId = Number(tabIdMatch[1]);

    await rpc.call("tools/call", {
      name: "navigate",
      arguments: { url: fixtureUrl, tabId },
    });

    const rp1Response = await rpc.call("tools/call", {
      name: "read_page",
      arguments: { tabId },
    });
    const rp1 = toolResultText(rp1Response, "read_page (before click)");
    if (!rp1.includes("Ghostlight smoke fixture")) {
      throw new Error(`read_page did not contain the expected fixture heading:\n${rp1}`);
    }
    const inputRef = extractRef(rp1, "Name input");
    const buttonRef = extractRef(rp1, "Click me");

    // The marker is a bare <p>, so it has neither a role nor an accessible name and read_page
    // (a structural/interactive tree) never surfaces it by design; get_page_text is the tool
    // for plain text, so it verifies the mutation the click below is meant to produce.
    const pt1Response = await rpc.call("tools/call", {
      name: "get_page_text",
      arguments: { tabId },
    });
    const pt1 = toolResultText(pt1Response, "get_page_text (before click)");
    if (!pt1.includes("marker-before-click")) {
      throw new Error(`get_page_text did not contain the expected marker text:\n${pt1}`);
    }

    const shotResponse = await rpc.call("tools/call", {
      name: "computer",
      arguments: { action: "screenshot", tabId },
    });
    const shotContent = shotResponse.result && shotResponse.result.content;
    const image =
      Array.isArray(shotContent) && shotContent.find((c) => c.type === "image");
    if (!image || !image.data) {
      throw new Error(
        `computer screenshot did not return an image content item: ${JSON.stringify(shotResponse)}`
      );
    }

    await rpc.call("tools/call", {
      name: "form_input",
      arguments: { ref: inputRef, value: "ghost", tabId },
    });

    await rpc.call("tools/call", {
      name: "computer",
      arguments: { action: "left_click", ref: buttonRef, tabId },
    });

    const pt2Response = await rpc.call("tools/call", {
      name: "get_page_text",
      arguments: { tabId },
    });
    const pt2 = toolResultText(pt2Response, "get_page_text (after click)");
    if (!pt2.includes("marker-after-click")) {
      throw new Error(`get_page_text after the click did not show marker-after-click:\n${pt2}`);
    }

    await cleanup();
    console.log("smoke: ok");
    process.exit(0);
  } catch (err) {
    if (browserLogs.length) {
      console.error("--- browser/extension console (last 40 lines) ---");
      for (const line of browserLogs.slice(-40)) console.error(line);
      console.error("--- end console ---");
    }
    await cleanup();
    fail(err && err.message ? err.message : String(err));
  }
}

async function main() {
  const binaryPath = resolveBinaryPath();
  const endpoint = `ghostlight-e2e-${process.pid}`;
  if (DRY_RUN) {
    await runDryRun(binaryPath, endpoint);
  } else {
    await runLive(binaryPath, endpoint);
  }
}

main().catch((err) => fail(err && err.message ? err.message : String(err)));
