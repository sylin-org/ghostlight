// The PowerShell deliverable, against real executables and a scripted browser adapter.
//
// scripts/browser-journey.ps1 is meant to run on a real machine with a real Chromium. This harness
// stands up the same process topology with a scripted adapter in the browser's place, so the script
// itself -- its session handling, its handle passing, its screenshot file, its exit code -- is
// proved rather than assumed.

import assert from "node:assert/strict";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";

const repository = resolve(import.meta.dirname, "..");
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const binDir = process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0", "debug");
const runtimeFile = join(repository, `tests/.ghostlight-ps-runtime-${process.pid}.json`);
const leaseFile = runtimeFile.replace(/\.json$/, ".lock");
const auditFile = join(repository, `tests/.ghostlight-ps-audit-${process.pid}.jsonl`);
const shotFile = join(repository, `tests/.ghostlight-ps-shot-${process.pid}.jpg`);
const environment = {
  ...process.env,
  GHOSTLIGHT_RUNTIME_FILE: runtimeFile,
  GHOSTLIGHT_AUDIT_FILE: auditFile
};
const children = [];

function executable(name) {
  const path = join(binDir, `${name}${executableSuffix}`);
  if (!existsSync(path)) throw new Error(`Missing ${path}; build the workspace first.`);
  return path;
}

function start(command, args = []) {
  const child = spawn(command, args, { env: environment, stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
  child.stderr.on("data", (chunk) => process.stderr.write(`[${command.split(/[\\/]/).at(-1)}] ${chunk}`));
  children.push(child);
  return child;
}

const sleep = (ms) => new Promise((resolvePromise) => setTimeout(resolvePromise, ms));

/** Length-prefixed native messaging, the same framing the real extension speaks. */
class NativePeer {
  constructor(child) {
    this.child = child;
    this.buffer = Buffer.alloc(0);
    this.queue = [];
    this.waiters = [];
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
      const waiter = this.waiters.shift();
      if (waiter) waiter(value);
      else this.queue.push(value);
    }
  }

  next(timeoutMs = 10000) {
    const queued = this.queue.shift();
    if (queued) return Promise.resolve(queued);
    // The adapter loop waits indefinitely; only the handshake wants a deadline.
    if (timeoutMs <= 0) return new Promise((resolvePromise) => this.waiters.push(resolvePromise));
    return new Promise((resolvePromise, reject) => {
      const timer = setTimeout(() => reject(new Error("Timed out waiting for a native frame")), timeoutMs);
      this.waiters.push((value) => { clearTimeout(timer); resolvePromise(value); });
    });
  }
}

// A one-pixel JPEG, so the screenshot the script writes is real bytes rather than a placeholder.
const PIXEL_JPEG =
  "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAABAAEBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAACf/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKp//2Q==";

async function runAdapter(peer) {
  let tab = { tab_id: 41, title: "", url: "about:blank", active: true, readiness: "complete" };
  for (;;) {
    const frame = await peer.next(0);
    if (frame.kind !== "request") continue;
    const request = frame.request;
    const command = request.command;
    let result;
    switch (command.command) {
      case "present":
        result = { outcome: "presented", rendered: true };
        break;
      case "open_tab":
        tab = { ...tab, title: "Example Domain", url: "https://example.com/", readiness: "complete" };
        result = { outcome: "tab_opened", tab, committed_urls: [tab.url] };
        break;
      case "read_text":
        result = {
          outcome: "text",
          tab_id: tab.tab_id,
          text: "Example Domain. This domain is for use in documentation.",
          truncated: false,
          title: tab.title,
          url: tab.url
        };
        break;
      case "screenshot":
        result = {
          outcome: "screenshot",
          tab_id: tab.tab_id,
          mime_type: "image/jpeg",
          data: PIXEL_JPEG,
          width: 1280,
          height: 720,
          viewport: {
            scope: "viewport",
            page_x: 0, page_y: 0, css_width: 1280, css_height: 720,
            visual_page_x: 0, visual_page_y: 0, visual_css_width: 1280, visual_css_height: 720,
            device_scale: 1, zoom: 1, output_scale: 1
          }
        };
        break;
      case "close_tab":
        result = { outcome: "tab_closed", tab_id: command.tab_id };
        break;
      default:
        throw new Error(`Unexpected physical primitive ${command.command}`);
    }
    peer.send({ kind: "receipt", receipt: { correlation: request.correlation, result } });
  }
}

for (const file of [runtimeFile, leaseFile, auditFile, shotFile]) rmSync(file, { force: true });

try {
  const service = start(executable("ghostlight"), ["--headless"]);
  for (let attempt = 0; attempt < 100 && !existsSync(runtimeFile); attempt += 1) await sleep(50);
  assert.equal(existsSync(runtimeFile), true, "the service never published runtime discovery");
  assert.equal(service.exitCode, null);

  const browserConnector = start(executable("ghostlight-browser-connector"));
  const native = new NativePeer(browserConnector);
  native.send({
    kind: "hello",
    // The relay refuses a stale major outright, so this has to track ADAPTER_PROTOCOL_MAJOR.
    major: 2,
    adapter_version: "1.0.0",
    browser_id: "browser_psjourney",
    adapter_epoch: "adapter_psjourney",
    capabilities: [
      "tabs", "atomic_tab_open", "navigation", "semantic_document", "capture", "pointer_input",
      "keyboard_input", "files", "script", "observation", "dialogs",
      "operation_recovery", "presentation"
    ].map((name) => ({ name, revision: 1 }))
  });
  // Wait for the relay's own handshake rather than guessing: until the connector answers
  // hello_accepted there is no adapter, and every browser call would fail as disconnected.
  const accepted = await native.next();
  assert.equal(
    accepted.kind,
    "hello_accepted",
    `the relay never accepted the adapter: ${JSON.stringify(accepted)}`
  );
  runAdapter(native).catch((error) => process.stderr.write(`[adapter] ${error}\n`));

  // Never spawnSync here. The scripted adapter answers on this event loop, and a synchronous
  // child would block it, so every browser call would dispatch and then time out as unknown.
  const shell = process.platform === "win32" ? "pwsh.exe" : "pwsh";
  const journey = await new Promise((resolvePromise) => {
    const child = spawn(
      shell,
      [
        "-NoProfile", "-File", join(repository, "scripts", "browser-journey.ps1"),
        "-Ghostlight", executable("ghostlight"),
        "-OutputPath", shotFile
      ],
      { env: environment, windowsHide: true }
    );
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("close", (status) => resolvePromise({ status, stdout, stderr }));
  });
  process.stdout.write(journey.stdout ?? "");
  if (journey.status !== 0) process.stderr.write(journey.stderr ?? "");
  assert.equal(journey.status, 0, "the PowerShell journey must exit zero when every step succeeds");

  const output = journey.stdout ?? "";
  for (const expected of [
    "Opened example.com.",
    "Read 9 words from example.com.",
    "Captured the viewport at 1280x720.",
    "Closed the controlled tab."
  ]) {
    assert.ok(output.includes(expected), `the journey never reported: ${expected}`);
  }

  assert.equal(existsSync(shotFile), true, "the journey wrote no screenshot");
  const image = readFileSync(shotFile);
  assert.ok(image.length > 100, "the screenshot is too small to be an image");
  assert.equal(image[0], 0xff, "the screenshot is not JPEG bytes");
  assert.equal(image[1], 0xd8, "the screenshot is not JPEG bytes");

  await sleep(300);
  const records = readFileSync(auditFile, "utf8")
    .trim().split("\n").filter(Boolean).map((line) => JSON.parse(line));
  const tools = records.map((record) => record.tool);
  for (const tool of [
    "browser_navigate", "browser_tabs", "browser_read", "browser_screenshot"
  ]) {
    assert.ok(tools.includes(tool), `${tool} never reached the executor`);
  }
  for (const record of records) {
    assert.equal(record.channel, "cli", "a scripted step was not attributed to the cli channel");
    assert.equal(record.allowed, true, `${record.tool} was refused: ${record.reason}`);
  }
  // The audit stays payload-free even when a script drove the work.
  const encoded = JSON.stringify(records);
  assert.equal(encoded.includes("This domain is for use"), false, "page text reached the audit");
  assert.ok(encoded.includes('"host":"example.com"'), "the landing host was not recorded");

  // Every step of that journey was its own process, so one workspace across all of them is the
  // session marker working (ADR-0106). Before it, this number was five.
  assert.equal(
    new Set(records.map((record) => record.workspace)).size,
    1,
    "steps from one caller must share a workspace, whatever process each ran in"
  );

  const demoFailure = await new Promise((resolvePromise) => {
    const child = spawn(
      shell,
      [
        "-NoProfile", "-File", join(repository, "scripts", "demo-foundry.ps1"),
        // Node treats Ghostlight's first `call` argument as a missing script, exits nonzero, and
        // writes no JSON. That gives the demo a portable real-process transport failure.
        "-Ghostlight", process.execPath,
        "-Beat", "0"
      ],
      { env: environment, windowsHide: true }
    );
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("close", (status) => resolvePromise({ status, stdout, stderr }));
  });
  const failureOutput = `${demoFailure.stdout ?? ""}\n${demoFailure.stderr ?? ""}`;
  assert.notEqual(demoFailure.status, 0, "the demo must fail when Ghostlight returns no result");
  assert.ok(
    /open did not return a JSON result \(exit [1-9][0-9]*\)/.test(failureOutput),
    `the demo hid its transport failure: ${failureOutput}`
  );
  assert.equal(
    failureOutput.includes("The property 'status' cannot be found"),
    false,
    "the demo regressed to dereferencing an absent result"
  );

  console.log("\npowershell journey ok: separate processes, one session, open/list/read/capture/close");
} finally {
  for (const child of children.reverse()) if (!child.killed) child.kill();
  for (const file of [runtimeFile, leaseFile, auditFile, shotFile]) rmSync(file, { force: true });
}
