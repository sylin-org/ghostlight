import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

const repository = resolve(import.meta.dirname, "..");
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const binDir = process.env.GHOSTLIGHT_BIN_DIR || join(repository, "target", "release");
const connectorPath = join(binDir, `ghostlight-mcp-connector${executableSuffix}`);
if (!existsSync(connectorPath)) throw new Error(`Repo-built MCP connector is missing ${connectorPath}`);

const child = spawn(connectorPath, [], {
  env: process.env,
  stdio: ["pipe", "pipe", "pipe"],
  windowsHide: true
});
const pending = new Map();
let nextId = 1;
let stderr = "";
child.stderr.on("data", (chunk) => { stderr += chunk.toString("utf8"); });
createInterface({ input: child.stdout }).on("line", (line) => {
  const message = JSON.parse(line);
  const waiter = pending.get(JSON.stringify(message.id));
  if (waiter) {
    pending.delete(JSON.stringify(message.id));
    waiter.resolve(message);
  }
});

function request(method, params = {}, timeoutMs = 15000) {
  const id = nextId++;
  const promise = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(JSON.stringify(id));
      reject(new Error(`Timed out waiting for MCP ${method}${stderr ? `: ${stderr.trim()}` : ""}`));
    }, timeoutMs);
    pending.set(JSON.stringify(id), {
      resolve(value) { clearTimeout(timer); resolve(value); }
    });
  });
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
  return promise;
}

function notify(method, params = {}) {
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
}

function structured(response) {
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  return response.result.structuredContent;
}

try {
  const initialized = await request("initialize", {
    protocolVersion: "2025-11-25",
    capabilities: {},
    clientInfo: { name: "Ghostlight live acceptance", version: "1" }
  });
  assert.equal(initialized.result.serverInfo.name, "ghostlight");
  notify("notifications/initialized");

  const listed = await request("tools/list");
  assert.equal(listed.result.tools.length, 22);
  assert.equal(listed.result.tools.every((tool) => tool.outputSchema && tool.annotations), true);

  const opened = structured(await request("tools/call", {
    name: "browser_navigate",
    arguments: { url: "https://example.com" }
  }));
  assert.equal(opened.status, "succeeded", JSON.stringify(opened));
  const tab = opened.facts.tab;
  assert.match(tab, /^tab_/);

  const read = structured(await request("tools/call", {
    name: "browser_read",
    arguments: { tab }
  }));
  assert.equal(read.status, "succeeded", JSON.stringify(read));
  assert.match(read.facts.text, /Example Domain/i);

  const screenshotResponse = await request("tools/call", {
    name: "browser_screenshot",
    arguments: { tab }
  });
  const screenshot = structured(screenshotResponse);
  assert.equal(screenshot.status, "succeeded", JSON.stringify(screenshot));
  assert.match(screenshot.facts.view, /^view_/);
  assert.equal(screenshot.facts.data, undefined);
  assert.equal(screenshotResponse.result.content[0].type, "text");
  assert.equal(screenshotResponse.result.content[1].type, "image");
  assert.equal(screenshotResponse.result.content[1].mimeType, "image/jpeg");
  assert.ok(screenshotResponse.result.content[1].data.length > 1000);
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 1600));

  const closed = structured(await request("tools/call", {
    name: "browser_tabs",
    arguments: { action: "close", tab }
  }));
  assert.equal(closed.status, "succeeded", JSON.stringify(closed));
  assert.equal(closed.facts.closed, true);
  console.log(JSON.stringify({ live: true, catalog_tools: listed.result.tools.length, opened: true, read: true, screenshot: true, closed: true }));
} finally {
  child.stdin.end();
  child.kill();
}
