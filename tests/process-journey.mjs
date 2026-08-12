import assert from "node:assert/strict";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const repository = resolve(import.meta.dirname, "..");
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const binDir = process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0", "debug");
const runtimeFile = join(repository, `tests/.ghostlight-runtime-${process.pid}.json`);
const auditFile = join(repository, `tests/.ghostlight-audit-${process.pid}.jsonl`);
const deployLock = join(binDir, "deploy.lock");
const environment = { ...process.env, GHOSTLIGHT_RUNTIME_FILE: runtimeFile, GHOSTLIGHT_AUDIT_FILE: auditFile };
const children = [];
const physicalCommands = [];
let createdDeployLock = false;
// A real one-pixel GIF89a, the shape the extension now hands over already finished.
const ONE_PIXEL_GIF = "R0lGODlhAQABAPAAAAwiOAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQAZAAAACwAAAAAAQABAAAIBAABBAQAOw==";
const ONE_PIXEL_JPEG = "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAABAAEBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAACf/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKp//2Q==";

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
    if (frame.kind !== "request") continue;
    const request = frame.request;
    const command = request.command;
    physicalCommands.push(command.command);
    let result;
    if (command.command === "present") result = { outcome: "presented", rendered: true };
    else if (command.command === "open_tab") {
      tab = { ...tab, title: "Example Domain", url: "https://example.com/", readiness: "complete" };
      result = { outcome: "tab_opened", tab, committed_urls: [tab.url] };
    } else if (command.command === "read_text") {
      result = { outcome: "text", tab_id: tab.tab_id, text: "Example Domain", truncated: false, title: tab.title, url: tab.url };
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
    else throw new Error(`Unexpected physical primitive ${command.command}`);
    peer.send({ kind: "receipt", receipt: { correlation: request.correlation, result } });
  }
}

function structured(response) {
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  return response.result.structuredContent;
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

try {
  rmSync(runtimeFile, { force: true });
  rmSync(auditFile, { force: true });
  if (existsSync(deployLock)) throw new Error(`Refusing to replace existing deploy lock ${deployLock}`);
  writeFileSync(deployLock, "process journey quiesce", { flag: "wx" });
  createdDeployLock = true;
  const browserConnector = start(executable("ghostlight-browser-connector"));
  const native = new NativePeer(browserConnector);
  native.send({
    kind: "hello",
    major: 2,
    adapter_version: "1.0.0",
    browser_id: "browser_processjourney",
    adapter_epoch: "adapter_processjourney",
    capabilities: [
      "tabs", "atomic_tab_open", "navigation", "semantic_document", "capture", "pointer_input",
      "keyboard_input", "files", "script", "observation", "dialogs",
      "operation_recovery", "presentation", "window_geometry", "diagnostics", "recording",
      "chunked_commands"
    ].map((name) => ({ name, revision: 1 }))
  });
  assert.deepEqual(await native.next(), { kind: "backend_unavailable" });

  const connector = start(executable("ghostlight-mcp-connector"));
  const mcp = new McpPeer(connector);
  const initialize = mcp.beginRequest("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "acceptance", version: "1" } });
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  assert.equal(connector.exitCode, null);
  assert.equal(browserConnector.exitCode, null);

  let service = start(executable("ghostlight"), ["--headless"]);
  await waitForFile(runtimeFile);
  const endpoint = JSON.parse(readFileSync(runtimeFile, "utf8"));
  assert.equal(endpoint.service_bridge_major, 2);
  assert.equal(endpoint.browser_relay_major, 1);
  const browserHello = await native.next();
  assert.equal(browserHello.kind, "hello_accepted");
  assert.equal(browserHello.control_state, "active");
  native.send({ kind: "event", event: { event: "runtime_control_requested", intent: "hold" } });
  assert.deepEqual(await native.next(), { kind: "control_state", state: "held" });
  native.send({ kind: "event", event: { event: "runtime_control_requested", intent: "resume" } });
  assert.deepEqual(await native.next(), { kind: "control_state", state: "active" });
  void runAdapter(native);

  const initialized = await initialize.promise;
  assert.equal(initialized.result.serverInfo.name, "ghostlight");
  assert.equal(initialized.result.protocolVersion, "2025-11-25");
  assert.equal(initialized.result.capabilities.tools.listChanged, true);
  mcp.notify("notifications/initialized");

  const listed = await mcp.request("tools/list");
  assert.equal(listed.result.tools.length, 22);
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
  service = start(executable("ghostlight"), ["--headless"]);
  await waitForFile(runtimeFile);
  const secondBrowserHello = await reconnected;
  assert.equal(secondBrowserHello.kind, "hello_accepted");
  assert.notEqual(secondBrowserHello.service_epoch, browserHello.service_epoch);
  assert.equal(connector.exitCode, null);
  assert.equal(browserConnector.exitCode, null);
  const relisted = await waitForMcpReady(mcp);
  assert.equal(relisted.result.tools.length, 22);

  const reopened = structured(await mcp.request("tools/call", { name: "browser_navigate", arguments: { url: "https://example.com" } }));
  assert.equal(reopened.status, "succeeded");
  const restartedHandle = reopened.facts.tab;

  const read = structured(await mcp.request("tools/call", { name: "browser_read", arguments: { tab: restartedHandle } }));
  assert.equal(read.status, "succeeded");
  assert.equal(read.facts.text, "Example Domain");
  assert.equal(read.summary, "Read 2 words from example.com.");

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
  assert.deepEqual(cancelled.next_steps, []);

  const closed = structured(await mcp.request("tools/call", { name: "browser_tabs", arguments: { action: "close", tab: restartedHandle } }));
  assert.equal(closed.status, "succeeded");
  assert.equal(closed.facts.closed, true);
  assert.equal(existsSync(auditFile), true);

  // The real executable's audit file, not a fixture: what an action did, and none of what it saw.
  const records = readFileSync(auditFile, "utf8").trim().split("\n").map((line) => JSON.parse(line));
  const readRecord = records.findLast((record) => record.tool === "browser_read");
  assert.equal(readRecord.observed.host, "example.com");
  assert.equal(readRecord.observed.count, 2);
  assert.equal(readRecord.observed.readiness, null);
  const openRecord = records.findLast((record) => record.tool === "browser_navigate");
  assert.equal(openRecord.observed.host, "example.com");
  assert.equal(openRecord.observed.readiness, "complete");
  assert.equal(records.some((record) => JSON.stringify(record).includes("Example Domain")), false);
  console.log("process journey ok: reconnect -> open/read -> extension-owned recording save/discard -> close");
} finally {
  for (const child of children.reverse()) {
    if (!child.killed) child.kill();
  }
  rmSync(runtimeFile, { force: true });
  rmSync(auditFile, { force: true });
  if (createdDeployLock) rmSync(deployLock, { force: true });
}
