import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const repository = resolve(import.meta.dirname, "..");
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const binDir = process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0", "debug");
// ADR-0150: the runtime override elects the authority directory, so it points inside the build
// under test and the deploy.lock beside it keeps quiescing demand-start.
const runtimeFile = join(binDir, `.ghostlight-journey-runtime-${process.pid}.json`);
const runtimeLease = `${runtimeFile.replace(/\.json$/, "")}.lock`;
const auditFile = join(repository, `tests/.ghostlight-audit-${process.pid}.jsonl`);
const policyFile = join(repository, `tests/.ghostlight-policy-${process.pid}.json`);
const diagnosticsDir = join(repository, `tests/.ghostlight-diagnostics-${process.pid}`);
const nativeHostDir = join(repository, `tests/.ghostlight-native-host-${process.pid}`);
const deployLock = join(binDir, "deploy.lock");
const environment = {
  ...process.env,
  GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
  GHOSTLIGHT_AUDIT_FILE: auditFile,
  GHOSTLIGHT_POLICY_FILE: policyFile,
  GHOSTLIGHT_DIAGNOSTICS_DIR: diagnosticsDir,
  // No process this journey spawns may touch the machine's real native-host registration
  // (ADR-0149 makes recovery repair owned registrations toward the running tree).
  GHOSTLIGHT_NATIVE_HOST_DIR: nativeHostDir
};
const children = [];
const physicalCommands = [];
let queryCount = 0;
const physicalRequests = [];
let createdDeployLock = false;
// A real one-pixel GIF89a, the shape the extension now hands over already finished.
const ONE_PIXEL_GIF = "R0lGODlhAQABAPAAAAwiOAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQAZAAAACwAAAAAAQABAAAIBAABBAQAOw==";
const ONE_PIXEL_JPEG = "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAABAAEBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAACf/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKp//2Q==";
const PROCESS_BROWSER = "browser_processjourney";

function executable(name) {
  const path = join(binDir, `${name}${executableSuffix}`);
  if (!existsSync(path)) throw new Error(`Missing ${path}; build the workspace first.`);
  return path;
}

function start(command, args = [], options = {}) {
  const child = spawn(command, args, { env: environment, stdio: ["pipe", "pipe", "pipe"], windowsHide: true, ...options });
  child.stderr.on("data", (chunk) => process.stderr.write(`[${command.split(/[\\/]/).at(-1)}] ${chunk}`));
  child.on("exit", (code, signal) => {
    if (code && code !== 0) process.stderr.write(`[${command.split(/[\\/]/).at(-1)}] exited code=${code} signal=${signal}\n`);
  });
  children.push(child);
  return child;
}

function waitForExit(child, timeoutMs = 5000) {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
  return new Promise((resolvePromise, reject) => {
    const timer = setTimeout(() => reject(new Error("Timed out waiting for child process exit")), timeoutMs);
    child.once("exit", () => { clearTimeout(timer); resolvePromise(); });
  });
}

async function waitForFile(path, timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (existsSync(path)) return;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 25));
  }
  throw new Error(`Timed out waiting for ${path}`);
}

class NativePeer {
  constructor(child) {
    this.child = child;
    this.buffer = Buffer.alloc(0);
    this.queue = [];
    this.waiters = [];
    this.observers = [];
    child.stdout.on("data", (chunk) => this.receive(chunk));
  }

  send(value) {
    const payload = Buffer.from(JSON.stringify(value));
    const header = Buffer.alloc(4);
    header.writeUInt32LE(payload.length);
    this.child.stdin.write(Buffer.concat([header, payload]));
  }

  receive(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 4) {
      const length = this.buffer.readUInt32LE(0);
      if (this.buffer.length < length + 4) return;
      const value = JSON.parse(this.buffer.subarray(4, length + 4).toString("utf8"));
      this.buffer = this.buffer.subarray(length + 4);
      for (const observer of [...this.observers]) {
        if (!observer.predicate(value)) continue;
        this.observers.splice(this.observers.indexOf(observer), 1);
        clearTimeout(observer.timer);
        observer.resolve(value);
      }
      const waiter = this.waiters.shift();
      if (waiter) waiter(value); else this.queue.push(value);
    }
  }

  next(timeoutMs = 5000) {
    const queued = this.queue.shift();
    if (queued) return Promise.resolve(queued);
    if (timeoutMs <= 0) return new Promise((resolvePromise) => this.waiters.push(resolvePromise));
    return new Promise((resolvePromise, reject) => {
      const timer = setTimeout(() => reject(new Error("Timed out waiting for native frame")), timeoutMs);
      this.waiters.push((value) => { clearTimeout(timer); resolvePromise(value); });
    });
  }


  waitFor(predicate, timeoutMs = 5000) {
    return new Promise((resolvePromise, reject) => {
      const observer = { predicate, resolve: resolvePromise, timer: null };
      observer.timer = setTimeout(() => {
        this.observers.splice(this.observers.indexOf(observer), 1);
        reject(new Error("Timed out waiting for matching native frame"));
      }, timeoutMs);
      this.observers.push(observer);
    });
  }
}

class McpPeer {
  constructor(child) {
    this.child = child;
    this.nextId = 1;
    this.pending = new Map();
    createInterface({ input: child.stdout }).on("line", (line) => {
      const message = JSON.parse(line);
      const waiter = this.pending.get(JSON.stringify(message.id));
      if (waiter) {
        this.pending.delete(JSON.stringify(message.id));
        waiter(message);
      }
    });
  }

  request(method, params = {}) {
    return this.beginRequest(method, params).promise;
  }

  beginRequest(method, params = {}) {
    const id = this.nextId++;
    const promise = new Promise((resolvePromise, reject) => {
      const timer = setTimeout(() => reject(new Error(`Timed out waiting for MCP ${method}`)), 10000);
      this.pending.set(JSON.stringify(id), (value) => { clearTimeout(timer); resolvePromise(value); });
    });
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    return { id, promise };
  }

  notify(method, params = {}) {
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
  }
}

async function runAdapter(peer) {
  let tab = { tab_id: 41, title: "", url: "about:blank", active: true, readiness: "complete" };
  const recordings = new Map();
  function selectedRecording(request) {
    const requested = request.command.recording_id;
    if (requested) return recordings.get(requested)?.workspace === request.workspace ? recordings.get(requested) : null;
    const owned = Array.from(recordings.values()).filter((recording) => recording.workspace === request.workspace);
    return owned.length === 1 ? owned[0] : null;
  }
  function recordingSummary(recording) {
    return {
      recording_id: recording.id,
      tab_id: tab.tab_id,
      state: recording.state,
      frame_count: recording.frames.length,
      bytes_held: recording.frames.length ? 166 : 0,
      duration_ms: 1000,
      ...(recording.state === "recording"
        ? { hard_expires_unix_ms: Date.now() + 120000 }
        : { retention_expires_unix_ms: Date.now() + 300000, stop_reason: "explicit" }),
      source_urls: [tab.url]
    };
  }
  for (;;) {
    const frame = await peer.next(0);
    if (frame.kind === "heartbeat") {
      peer.send({ kind: "heartbeat_ack", sequence: frame.sequence });
      continue;
    }
    if (frame.kind !== "request") continue;
    const request = frame.request;
    const command = request.command;
    physicalCommands.push(command.command);
    physicalRequests.push(command);
    let result;
    if (command.command === "present") result = { outcome: "presented", rendered: true };
    else if (command.command === "open_tab") {
      tab = { ...tab, title: "Example Domain", url: "https://example.com/", readiness: "complete" };
      result = { outcome: "tab_opened", tab, committed_urls: [tab.url] };
    } else if (command.command === "read_text") {
      result = { outcome: "text", tab_id: tab.tab_id, text: "Example Domain", truncated: false, title: tab.title, url: tab.url };
    } else if (command.command === "find") {
      result = {
        outcome: "targets",
        tab_id: tab.tab_id,
        targets: [{ locator: "heading-1", role: "heading", name: "Example Domain", state: [], credential_class: false }]
      };
    } else if (command.command === "evaluate_script") {
      result = {
        outcome: "script_evaluated",
        tab,
        value: JSON.stringify({ title: "Example Domain", visible: true }),
        truncated: false,
        committed_urls: []
      };
    } else if (command.command === "screenshot") {
      result = {
        outcome: "screenshot",
        tab_id: tab.tab_id,
        mime_type: "image/jpeg",
        data: ONE_PIXEL_JPEG,
        width: 400,
        height: 300,
        viewport: {
          scope: "viewport",
          page_x: 10,
          page_y: 20,
          css_width: 800,
          css_height: 600,
          visual_page_x: 10,
          visual_page_y: 20,
          visual_css_width: 800,
          visual_css_height: 600,
          device_scale: 1,
          zoom: 1,
          output_scale: 0.5
        }
      };
    } else if (command.command === "screenshot_region") {
      const { region, expected_viewport: expected } = command;
      const scale = Math.min(
        2400 / region.width,
        2400 / region.height,
        Math.sqrt(4_000_000 / (region.width * region.height))
      );
      result = {
        outcome: "screenshot",
        tab_id: tab.tab_id,
        mime_type: "image/jpeg",
        data: ONE_PIXEL_JPEG,
        width: Math.round(region.width * scale),
        height: Math.round(region.height * scale),
        viewport: {
          scope: "region",
          page_x: region.x,
          page_y: region.y,
          css_width: region.width,
          css_height: region.height,
          visual_page_x: expected.visual_page_x,
          visual_page_y: expected.visual_page_y,
          visual_css_width: expected.visual_css_width,
          visual_css_height: expected.visual_css_height,
          device_scale: expected.device_scale,
          zoom: expected.zoom,
          output_scale: scale
        }
      };
    } else if (command.command === "observe") {
      setTimeout(() => peer.send({ kind: "receipt", receipt: { correlation: request.correlation, result: { outcome: "observed", tab_id: command.tab_id, satisfied: true, elapsed_ms: 1000, readiness: "complete" } } }), 1000);
      continue;
    } else if (command.command === "start_recording") {
      const recording = { id: `recording_${recordings.size + 1}`, workspace: request.workspace, state: "recording", frames: [] };
      recordings.set(recording.id, recording);
      result = { outcome: "recording_started", summary: recordingSummary(recording), existing: false };
    } else if (command.command === "stop_recording") {
      const recording = selectedRecording(request);
      if (!recording) result = { outcome: "recording_not_found" };
      else {
        const changed = recording.state === "recording";
        recording.state = "frozen";
        if (recording.frames.length === 0) {
          recording.frames.push({ frame_kind: "final", duration_ms: 1_000, mime_type: "image/jpeg", data: ONE_PIXEL_JPEG });
        }
        result = { outcome: "recording_stopped", summary: recordingSummary(recording), changed };
      }
    } else if (command.command === "export_recording") {
      // The browser encodes and delivers. Only a client return carries bytes back; a target or
      // download save finishes here, exactly as the extension does it.
      const recording = selectedRecording(request);
      if (!recording) result = { outcome: "recording_not_found" };
      else {
        const kind = command.destination.destination;
        result = {
          outcome: "recording_exported",
          summary: recordingSummary(recording),
          encoded: {
            frame_count: recording.frames.length,
            captured_frame_count: recording.frames.length,
            duration_ms: 1_000,
            width: 1,
            height: 1,
            byte_count: 69
          },
          delivery: kind === "client"
            ? { delivery: "returned", mime_type: "image/gif", data: ONE_PIXEL_GIF }
            : kind === "download"
              ? { delivery: "downloaded" }
              : { delivery: "attached", tab_id: command.destination.tab_id }
        };
      }
    } else if (command.command === "discard_recording") {
      const recording = selectedRecording(request);
      if (!recording) result = { outcome: "recording_not_found" };
      else {
        recordings.delete(recording.id);
        result = { outcome: "recording_discarded", recording_id: recording.id, released_bytes: 166 };
      }
    } else if (command.command === "close_tab") result = { outcome: "tab_closed", tab_id: command.tab_id };
    else if (command.command === "cancel") result = { outcome: "cancelled" };
    else if (command.command === "read_document") {
      result = {
        outcome: "text",
        tab_id: command.tab_id,
        text: command.mode === "article"
          ? "Article body describing Example Domain and its purpose in depth."
          : "Example Domain",
        truncated: false,
        title: "Example Domain",
        url: "https://example.com/"
      };
    } else if (command.command === "inspect_tree") {
      result = { outcome: "document_tree", tab_id: command.tab_id, tree: JSON.stringify({ kind: "container", label: "Example Domain", children: [{ kind: "heading", label: "Example Domain", children: [] }] }), truncated: false };
    } else if (command.command === "query_semantic") {
      queryCount += 1;
      const one = [{ locator: `locator_${queryCount}`, role: "link", name: "More information...", state: [], credential_class: false }];
      result = { outcome: "targets", tab_id: command.tab_id, targets: queryCount === 2 ? [...one, { ...one[0], locator: `${one[0].locator}b` }] : one };
    } else if (command.command === "describe_targets") {
      result = { outcome: "targets_described", tab_id: command.tab_id, targets: command.locators.map((locator) => ({ locator, role: "textbox", name: "Notes", state: [], credential_class: false })) };
    } else if (command.command === "activate" || command.command === "activate_modified") {
      tab = { ...tab, title: "Example Domain", url: "https://example.com/", readiness: "complete" };
      result = { outcome: "activated", tab, subject: { role: "link", name: "More information..." }, committed_urls: [] };
    } else if (command.command === "press_key") {
      tab = { ...tab, readiness: "complete" };
      result = { outcome: "key_pressed", tab, key: command.key, subject: null, committed_urls: [] };
    } else if (command.command === "wheel_at") {
      result = { outcome: "scrolled", tab_id: command.tab_id, x: 100, y: 50, subject: null };
    } else if (command.command === "navigate_discarding_before_unload") {
      tab = { ...tab, title: "Submitted", url: "https://example.com/submitted", readiness: "complete" };
      result = { outcome: "navigated", tab, committed_urls: ["https://example.com/submitted"] };
    } else if (command.command === "upload_files" || command.command === "drop_image_at") {
      const files = command.files ?? [command.file];
      result = { outcome: "files_uploaded", tab_id: command.tab_id, uploaded_count: files.length, uploaded_bytes: files.reduce((sum, file) => sum + file.size, 0), subject: null };
    }
    else throw new Error(`Unexpected physical primitive ${command.command}`);
    peer.send({ kind: "receipt", receipt: { correlation: request.correlation, result } });
  }
}

function structured(response) {
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  return response.result.structuredContent;
}

function textual(response) {
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  const item = response.result.content.find((content) => content.type === "text");
  assert.equal(typeof item?.text, "string");
  return item.text;
}

async function waitForMcpReady(mcp, timeoutMs = 10000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const response = await mcp.request("tools/list");
    if (!response.error) return response;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  }
  throw new Error("Timed out waiting for MCP service reconnection");
}

async function waitForNoBrowsers(mcp, timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs;
  let lastSeen = "no response";
  while (Date.now() < deadline) {
    const listed = structured(await mcp.request("tools/call", {
      name: "browser_tabs",
      arguments: { action: "list" }
    }));
    // Listing reads live state through the adapter, so once the adapter is disconnected the
    // read refuses -- and that refusal is exactly the registry-empty signal.
    lastSeen = listed.facts?.browsers
      ? `tabs=${JSON.stringify(listed.facts.browsers)}`
      : `${listed.status}: ${listed.summary ?? ""}`;
    if (listed.status === "failed") return;
    if (Array.isArray(listed.facts?.browsers) && listed.facts.browsers.length === 0) return;
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 25));
  }
  throw new Error(`Timed out waiting for the browser registry to empty: ${lastSeen}`);
}

try {
  rmSync(runtimeFile, { force: true });
  rmSync(auditFile, { force: true });
  rmSync(policyFile, { force: true });
  writeFileSync(policyFile, JSON.stringify({
    schema: 3,
    name: "Process journey",
    version: "1",
    grants: [{
      id: "ordinary-web",
      hosts: { allow: ["*"] },
      allowed: ["read", "action", "write", "execute"]
    }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" }]
  }));
  if (existsSync(deployLock)) throw new Error(`Refusing to replace existing deploy lock ${deployLock}`);
  writeFileSync(deployLock, "process journey quiesce", { flag: "wx" });
  createdDeployLock = true;
  const browserConnector = start(executable("ghostlight-browser-connector"));
  const native = new NativePeer(browserConnector);
  native.send({
    kind: "hello",
    major: 2,
    adapter_version: "1.0.0",
    browser_id: PROCESS_BROWSER,
    adapter_epoch: "adapter_processjourney",
    capabilities: [
      "tabs", "atomic_tab_open", "navigation", "semantic_document", "capture", "pointer_input",
      "keyboard_input", "files", "script", "observation", "dialogs",
      "operation_recovery", "presentation", "window_geometry", "diagnostics", "recording",
      "chunked_commands", "adapter_liveness"
    ].map((name) => ({ name, revision: { script: 2, pointer_input: 3, keyboard_input: 2, semantic_document: 4, capture: 2, navigation: 2, files: 3, observation: 2 }[name] ?? 1 }))
  });
  assert.deepEqual(await native.next(), { kind: "backend_unavailable" });

  const connector = start(executable("ghostlight-mcp-connector"));
  const mcp = new McpPeer(connector);
  const discovery = mcp.beginRequest("server/discover", {
    _meta: {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientInfo": { name: "acceptance", version: "1" },
      "io.modelcontextprotocol/clientCapabilities": {}
    }
  });
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  assert.equal(connector.exitCode, null);
  assert.equal(browserConnector.exitCode, null);

  let service = start(executable("ghostlight"));
  await waitForFile(runtimeFile);
  const endpoint = JSON.parse(readFileSync(runtimeFile, "utf8"));
  assert.equal(endpoint.service_bridge_major, 2);
  assert.equal(endpoint.browser_relay_major, 1);
  const browserHello = await native.next();
  assert.equal(browserHello.kind, "hello_accepted");
  assert.equal(browserHello.control_state, "active");
  native.send({ kind: "event", event: { event: "runtime_control_requested", intent: "hold" } });
  assert.deepEqual(await native.next(), { kind: "control_state", state: "held", diagnostics: { layer: "explicit" } });
  native.send({ kind: "event", event: { event: "runtime_control_requested", intent: "resume" } });
  assert.deepEqual(await native.next(), { kind: "control_state", state: "active", diagnostics: { layer: "explicit" } });
  void runAdapter(native);

  const discovered = await discovery.promise;
  assert.equal(discovered.result.resultType, "complete");
  assert.deepEqual(discovered.result.supportedVersions,
    ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"]);
  assert.equal(discovered.result._meta["io.modelcontextprotocol/serverInfo"].name, "ghostlight");
  assert.equal(discovered.result.cacheScope, "private");
  assert.equal(discovered.result.ttlMs, 0);

  const initialize = mcp.beginRequest("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "acceptance", version: "1" } });
  const initialized = await initialize.promise;
  assert.equal(initialized.result.serverInfo.name, "ghostlight");
  assert.equal(initialized.result.protocolVersion, "2025-11-25");
  assert.equal(initialized.result.capabilities.tools.listChanged, true);
  mcp.notify("notifications/initialized");

  const listed = await mcp.request("tools/list");
  assert.equal(listed.result.tools.length, 24);
  assert.equal(listed.result.tools.every((tool) => tool.outputSchema && tool.annotations), true);
  assert.equal(listed.result.tools.some((tool) => tool.name === "browser_execute"), true);
  assert.equal(listed.result.tools.some((tool) => tool.name === "browser_evaluate"), false);

  const opened = structured(await mcp.request("tools/call", { name: "browser_navigate", arguments: { url: "https://example.com" } }));
  assert.equal(opened.status, "succeeded");
  const handle = opened.facts.tab;
  assert.match(handle, /^tab_/);
  assert.equal(physicalCommands.filter((command) => command === "open_tab").length, 1);
  assert.equal(physicalCommands.includes("create_tab"), false);

  const observedDispatch = native.waitFor(
    (frame) => frame.kind === "request" && frame.request.command.command === "observe",
    10000
  );
  const interrupted = mcp.beginRequest("tools/call", { name: "browser_wait", arguments: { tab: handle, condition: "load_ready" } });
  await observedDispatch;
  const unavailable = native.waitFor((frame) => frame.kind === "backend_unavailable", 10000);
  service.kill();
  await waitForExit(service);
  assert.deepEqual(await unavailable, { kind: "backend_unavailable" });
  const interruptedResult = await interrupted.promise;
  assert.match(interruptedResult.error.message, /outcome is unavailable/);

  const reconnected = native.waitFor((frame) => frame.kind === "hello_accepted", 10000);
  service = start(executable("ghostlight"));
  await waitForFile(runtimeFile);
  const secondBrowserHello = await reconnected;
  assert.equal(secondBrowserHello.kind, "hello_accepted");
  assert.notEqual(secondBrowserHello.service_epoch, browserHello.service_epoch);
  assert.equal(connector.exitCode, null);
  assert.equal(browserConnector.exitCode, null);
  const relisted = await waitForMcpReady(mcp);
  assert.equal(relisted.result.tools.length, 24);

  const reopened = structured(await mcp.request("tools/call", { name: "browser_navigate", arguments: { url: "https://example.com" } }));
  assert.equal(reopened.status, "succeeded");
  const restartedHandle = reopened.facts.tab;

  const read = structured(await mcp.request("tools/call", { name: "browser_read", arguments: { tab: restartedHandle } }));
  assert.equal(read.status, "succeeded");
  assert.equal(read.facts.text, "Example Domain");
  assert.equal(read.summary, "Read 2 words from example.com.");
  assert.equal(physicalRequests.findLast((command) => command.command === "read_document").mode, "visible");

  const foundResponse = await mcp.request("tools/call", {
    name: "browser_find",
    arguments: { tab: restartedHandle, text: "Example Domain", max_results: 3 }
  });
  const found = structured(foundResponse);
  assert.equal(found.status, "succeeded");
  assert.equal(found.facts.matches[0].name, "Example Domain");
  assert.match(textual(foundResponse), /\"matches\":\[\{[^\n]*\"name\":\"Example Domain\"/);

  const executedResponse = await mcp.request("tools/call", {
    name: "browser_execute",
    arguments: { tab: restartedHandle, script: "({ title: document.title, visible: true })" }
  });
  const executed = structured(executedResponse);
  assert.deepEqual(executed.facts.value, { title: "Example Domain", visible: true });
  assert.match(textual(executedResponse), /\"value\":\{\"title\":\"Example Domain\",\"visible\":true\}/);

  const screenshotResponse = await mcp.request("tools/call", {
    name: "browser_screenshot",
    arguments: { tab: restartedHandle }
  });
  const screenshot = structured(screenshotResponse);
  assert.equal(screenshot.status, "succeeded");
  assert.match(screenshot.facts.view, /^view_/);
  assert.equal(screenshotResponse.result.content.some((item) => item.type === "image"), true);

  const firstRegion = structured(await mcp.request("tools/call", {
    name: "browser_screenshot",
    arguments: { view: screenshot.facts.view, x: 100, y: 50, width: 100, height: 50 }
  }));
  assert.equal(firstRegion.status, "succeeded");
  assert.equal(firstRegion.summary, "Captured the magnified region at 2400x1200.");
  const firstRegionCommand = physicalRequests.findLast((command) => command.command === "screenshot_region");
  assert.deepEqual(firstRegionCommand.region, { x: 210, y: 120, width: 200, height: 100 });
  assert.equal(firstRegionCommand.expected_viewport.output_scale, 0.5);

  const secondRegion = structured(await mcp.request("tools/call", {
    name: "browser_screenshot",
    arguments: { view: firstRegion.facts.view, x: 1200, y: 600, width: 600, height: 300 }
  }));
  assert.equal(secondRegion.status, "succeeded");
  const secondRegionCommand = physicalRequests.findLast((command) => command.command === "screenshot_region");
  assert.deepEqual(secondRegionCommand.region, { x: 310, y: 170, width: 50, height: 25 });
  assert.equal(secondRegionCommand.expected_viewport.scope, "region");

  // R4: explicit article reading and document-tree snapshots with a diff.
  const article = structured(await mcp.request("tools/call", {
    name: "browser_read",
    arguments: { mode: "article", max_chars: 5000 }
  }));
  assert.equal(article.status, "succeeded");
  assert.match(article.facts.text, /Article body/);
  const treeFirst = structured(await mcp.request("tools/call", {
    name: "browser_inspect",
    arguments: { scope: "document", max_depth: 4 }
  }));
  assert.equal(treeFirst.status, "succeeded");
  assert.match(treeFirst.facts.snapshot, /^snapshot_/);
  const treeSecond = structured(await mcp.request("tools/call", {
    name: "browser_inspect",
    arguments: { scope: "document", max_depth: 4 }
  }));
  assert.deepEqual(treeSecond.facts.diff, { added: 0, removed: 0, changed: 0, paths: [] });

  // R3: semantic selector click, ambiguity refusal without effect, modified click.
  queryCount = 0;
  const selectorClick = structured(await mcp.request("tools/call", {
    name: "browser_click",
    arguments: { tab: restartedHandle, selector: { name: "More information..." } }
  }));
  if (selectorClick.status !== "succeeded") console.log("SELECTOR_CLICK", JSON.stringify(selectorClick));
  assert.equal(selectorClick.status, "succeeded");
  const ambiguous = structured(await mcp.request("tools/call", {
    name: "browser_click",
    arguments: { tab: restartedHandle, selector: { name: "More information..." } }
  }));
  assert.equal(ambiguous.status, "failed");
  assert.match(ambiguous.summary, /none was chosen/);
  assert.equal(ambiguous.facts.selector_matched, 2);
  const modifiedClick = structured(await mcp.request("tools/call", {
    name: "browser_click",
    arguments: { tab: restartedHandle, selector: { name: "More information..." }, modifiers: ["Control"] }
  }));
  assert.equal(modifiedClick.status, "succeeded");
  assert.equal(physicalCommands.findLast((command) => command.startsWith("activate")), "activate_modified");

  // R2: stroke sequences with repeats and a duration wait.
  const strokes = structured(await mcp.request("tools/call", {
    name: "browser_press_key",
    arguments: { tab: restartedHandle, strokes: ["a", "b"], repeat: 2 }
  }));
  assert.equal(strokes.status, "succeeded");
  assert.equal(physicalCommands.filter((command) => command === "press_key").length >= 4, true);
  const durationWait = structured(await mcp.request("tools/call", {
    name: "browser_wait",
    arguments: { condition: "duration", value: "50" }
  }));
  assert.equal(durationWait.status, "succeeded");

  // R5: inline upload to a found target; captured-image attach and view drop.
  const inlineUpload = structured(await mcp.request("tools/call", {
    name: "browser_upload",
    arguments: {
      target: found.facts.matches[0].target,
      files: [{ name: "notes.txt", media_type: "text/plain", data_base64: Buffer.from("hello").toString("base64") }]
    }
  }));
  if (inlineUpload.status !== "succeeded") console.log("UPLOAD", JSON.stringify(inlineUpload));
  assert.equal(inlineUpload.status, "succeeded");
  assert.equal(inlineUpload.facts.uploaded_bytes, 5);
  const freshShot = structured(await mcp.request("tools/call", {
    name: "browser_screenshot",
    arguments: { tab: restartedHandle }
  }));
  assert.match(freshShot.facts.image, /^image_/);
  const imageAttach = structured(await mcp.request("tools/call", {
    name: "browser_upload",
    arguments: { target: found.facts.matches[0].target, source_image: freshShot.facts.image }
  }));
  assert.equal(imageAttach.status, "succeeded");
  const imageDrop = structured(await mcp.request("tools/call", {
    name: "browser_upload",
    arguments: { source_image: freshShot.facts.image, view: freshShot.facts.view, x: 100, y: 50 }
  }));
  if (imageDrop.status !== "succeeded") console.log("DROP", JSON.stringify(imageDrop));
  assert.equal(imageDrop.status, "succeeded");

  // R2 coordinate wheel through the governed view transform.
  const wheel = structured(await mcp.request("tools/call", {
    name: "browser_scroll",
    arguments: { view: freshShot.facts.view, x: 100, y: 50, direction: "down", ticks: 2 }
  }));
  assert.equal(wheel.status, "succeeded");

  // R7: guarded navigation discarding only its own beforeunload prompt.
  const guarded = structured(await mcp.request("tools/call", {
    name: "browser_navigate",
    arguments: { url: "https://example.com/submitted", beforeunload: "discard" }
  }));
  assert.equal(guarded.status, "succeeded");

  // R6: flow dry run plus a referenced three-step flow over ordinary results.
  const flowDry = structured(await mcp.request("tools/call", {
    name: "browser_flow",
    arguments: {
      dry_run: true,
      steps: [
        { id: "list", tool: "browser_tabs", arguments: { action: "list" } },
        { id: "read", tool: "browser_read", arguments: { mode: "article" } }
      ]
    }
  }));
  assert.equal(flowDry.status, "succeeded");
  assert.equal(flowDry.facts.steps[0].capabilities.includes("read"), true);
  assert.equal(flowDry.facts.steps[1].capabilities.includes("execute"), false);
  const flowResponse = await mcp.request("tools/call", {
    name: "browser_flow",
    arguments: {
      steps: [
        { id: "find", tool: "browser_find", arguments: { text: "More information...", max_results: 1 } },
        { id: "click", tool: "browser_click", arguments: { target: { flow_ref: { step: "find", pointer: "/facts/matches/0/target" } } } },
        { id: "read", tool: "browser_read", arguments: { max_chars: 500 } }
      ]
    }
  });
  const flow = structured(flowResponse);
  if (flow.status !== "succeeded") console.log("FLOW", JSON.stringify(flow));
  assert.equal(flow.status, "succeeded");
  assert.equal(flow.facts.completed, 3);
  assert.match(textual(flowResponse), /Completed 3 flow steps\./);

  const startedRecording = structured(await mcp.request("tools/call", {
    name: "browser_record",
    arguments: { action: "start", tab: restartedHandle }
  }));
  assert.equal(startedRecording.status, "succeeded");
  assert.match(startedRecording.facts.recording, /^recording_/);
  const savedRecordingResponse = await mcp.request("tools/call", {
    name: "browser_record",
    arguments: { action: "save", recording: startedRecording.facts.recording }
  });
  const savedRecording = structured(savedRecordingResponse);
  assert.equal(savedRecording.status, "succeeded");
  assert.equal(savedRecording.facts.delivery, "returned_to_client");
  assert.equal(savedRecording.summary, "Saved a replay of 1 second of page changes.");
  assert.equal(
    savedRecordingResponse.result.content.some((item) => item.type === "image" && item.mimeType === "image/gif"),
    true
  );

  // The same recording saved to a file. This is the claim ADR-0109 exists for: a replay that
  // stays inside the browser reaches the user without one byte crossing back through here.
  const downloadedResponse = await mcp.request("tools/call", {
    name: "browser_record",
    arguments: { action: "save", recording: startedRecording.facts.recording, download: true }
  });
  const downloaded = structured(downloadedResponse);
  assert.equal(downloaded.status, "succeeded");
  assert.equal(downloaded.facts.delivery, "downloaded_by_browser");
  assert.equal(downloaded.summary, "Downloaded a replay of 1 second of page changes.");
  assert.deepEqual(downloadedResponse.result.content.filter((item) => item.type === "image"), []);
  const discardedRecording = structured(await mcp.request("tools/call", {
    name: "browser_record",
    arguments: { action: "discard", recording: startedRecording.facts.recording }
  }));
  assert.equal(discardedRecording.status, "succeeded");

  const delayed = mcp.beginRequest("tools/call", { name: "browser_wait", arguments: { tab: restartedHandle, condition: "load_ready" } });
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 50));
  mcp.notify("notifications/cancelled", { requestId: delayed.id, reason: "acceptance cancellation" });
  const cancelled = structured(await delayed.promise);
  assert.equal(cancelled.status, "unknown");
  assert.equal(cancelled.effect, "unknown");
  assert.equal(cancelled.repeat_safe, false);
  // An effect whose fate is unknown names the open-dialog hypothesis and the observation
  // route (d5a8c5de); it never suggests replaying the interrupted call.
  assert.deepEqual(cancelled.next_steps, [
    "If a JavaScript dialog may be open on the page, handle it with browser_dialog; handling checks the page directly.",
    "Then observe the page with browser_read or browser_inspect to learn what happened.",
  ]);

  const closed = structured(await mcp.request("tools/call", { name: "browser_tabs", arguments: { action: "close", tab: restartedHandle } }));
  assert.equal(closed.status, "succeeded");
  assert.equal(closed.facts.closed, true);
  assert.equal(existsSync(auditFile), true);

  // The real executable's audit file, not a fixture: what an action did, and none of what it saw.
  const records = readFileSync(auditFile, "utf8").trim().split("\n").map((line) => JSON.parse(line));
  const readRecord = records.findLast((record) => record.tool === "browser_read");
  assert.equal(readRecord.observed.host, "example.com");
  // Flow children share one invocation audit row; the last read on file is the
  // journey's article-mode read.
  assert.equal(readRecord.observed.count, 10);
  assert.equal(readRecord.observed.readiness, null);
  const openRecord = records.findLast((record) => record.tool === "browser_navigate");
  assert.equal(openRecord.observed.host, "example.com");
  assert.equal(openRecord.observed.readiness, "complete");
  assert.equal(records.some((record) => JSON.stringify(record).includes("Example Domain")), false);

  // ADR-0145: the explicit layer pinned every process at birth, so all three wrote bounded,
  // content-free operational logs into the one journey directory.
  const diagnosticNames = readdirSync(diagnosticsDir).filter((name) => name.endsWith(".jsonl"));
  assert.equal(diagnosticNames.some((name) => name.includes("-orchestrator-")), true);
  assert.equal(diagnosticNames.some((name) => name.includes("-mcp-connector-")), true);
  assert.equal(diagnosticNames.some((name) => name.includes("-browser-connector-")), true);
  const diagnosticRecords = diagnosticNames
    .flatMap((name) => readFileSync(join(diagnosticsDir, name), "utf8").trim().split("\n"))
    .map((line) => JSON.parse(line))
    .filter((record) => record.event !== undefined);
  for (const name of diagnosticNames) {
    const header = JSON.parse(readFileSync(join(diagnosticsDir, name), "utf8").split("\n")[0]);
    assert.equal(header.schema, "ghostlight-diagnostics-1");
  }
  const readOperation = diagnosticRecords.findLast(
    (record) => record.event === "operation_completed" && record.detail.includes("browser_read")
  );
  assert.equal(readOperation.component, "orchestrator");
  assert.equal(typeof readOperation.op, "string");
  assert.match(readOperation.detail, /browser_read succeeded/);
  assert.equal(diagnosticRecords.some((record) => JSON.stringify(record).includes("Example Domain")), false);
  assert.equal(diagnosticRecords.some((record) => record.event === "harness_attached" && record.detail.includes("acceptance")), true);
  assert.equal(diagnosticRecords.some((record) => record.event === "adapter_attached" && record.detail.includes(PROCESS_BROWSER)), true);
  assert.equal(diagnosticRecords.some((record) => record.component === "mcp-connector" && record.event === "service_connected"), true);
  assert.equal(diagnosticRecords.some((record) => record.component === "browser-connector" && record.event === "service_connected"), true);

  const runDiagnosticsCli = (args) =>
    new Promise((resolvePromise, reject) => {
      const child = spawn(executable("ghostlight"), ["diagnostics", ...args], { env: environment, stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
      let out = "";
      child.stdout.on("data", (chunk) => { out += chunk; });
      child.on("exit", (code) => (code === 0 ? resolvePromise(out) : reject(new Error(`diagnostics ${args.join(" ")} exited ${code}`))));
    });
  const shown = JSON.parse(await runDiagnosticsCli(["show", "--json"]));
  assert.equal(shown.some((record) => record.event === "operation_completed" && record.detail.includes("browser_read")), true);
  assert.match(await runDiagnosticsCli(["path"]), /explicit/);
  await runDiagnosticsCli(["on"]);
  assert.equal(existsSync(join(dirname(runtimeFile), "diagnostics.on")), true, "on creates the marker");
  await runDiagnosticsCli(["off"]);
  assert.equal(existsSync(join(dirname(runtimeFile), "diagnostics.on")), false, "off removes the marker");

  // This workspace stays pinned to the fake browser after its last tab closes. Once that adapter
  // disconnects, recovery must preserve the profile binding and stop before repair, launch, or
  // adapter dispatch. Injected Rust tests own the exact unpinned startup behavior matrix.
  const physicalCommandCountBeforeRecovery = physicalCommands.length;
  browserConnector.kill();
  await waitForExit(browserConnector);
  await waitForNoBrowsers(mcp);
  const disconnected = structured(await mcp.request("tools/call", {
    name: "browser_navigate",
    arguments: {
      url: "https://example.com",
      new_tab: true
    }
  }));
  assert.equal(disconnected.status, "failed");
  assert.equal(disconnected.effect, "none");
  assert.equal(disconnected.repeat_safe, true);
  assert.equal(disconnected.facts.reason, "browser_wrong_profile");
  assert.deepEqual(disconnected.facts.details, [PROCESS_BROWSER]);
  assert.equal(physicalCommands.length, physicalCommandCountBeforeRecovery);
  assert.equal(disconnected.next_steps.length, 1);

  console.log("process journey ok: reconnect -> open/read/find/flow(execute/article/tree/wheel/upload/drop/guarded) -> screenshot/region/chain -> recording -> close -> pinned no-adapter refusal");
} finally {
  for (const child of children.reverse()) {
    if (!child.killed) child.kill();
  }
  rmSync(runtimeFile, { force: true });
  rmSync(nativeHostDir, { force: true, recursive: true });
  rmSync(auditFile, { force: true });
  rmSync(policyFile, { force: true });
  rmSync(diagnosticsDir, { recursive: true, force: true });
  if (createdDeployLock) rmSync(deployLock, { force: true });
  rmSync(runtimeLease, { force: true });
}
