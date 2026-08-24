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
  assert.equal(listed.result.tools.length, 24);
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

  const regionWidth = Math.max(1, Math.floor(screenshot.facts.width / 2));
  const regionHeight = Math.max(1, Math.floor(screenshot.facts.height / 2));
  const regionResponse = await request("tools/call", {
    name: "browser_screenshot",
    arguments: {
      view: screenshot.facts.view,
      x: Math.floor((screenshot.facts.width - regionWidth) / 2),
      y: Math.floor((screenshot.facts.height - regionHeight) / 2),
      width: regionWidth,
      height: regionHeight
    }
  });
  const region = structured(regionResponse);
  assert.equal(region.status, "succeeded", JSON.stringify(region));
  assert.match(region.facts.view, /^view_/);
  assert.notEqual(region.facts.view, screenshot.facts.view);
  assert.ok(region.facts.width > regionWidth);
  assert.ok(region.facts.height > regionHeight);
  assert.equal(regionResponse.result.content[1].type, "image");
  assert.ok(regionResponse.result.content[1].data.length > 1000);

  const chainedWidth = Math.max(1, Math.floor(region.facts.width / 2));
  const chainedHeight = Math.max(1, Math.floor(region.facts.height / 2));
  const chained = structured(await request("tools/call", {
    name: "browser_screenshot",
    arguments: {
      view: region.facts.view,
      x: Math.floor((region.facts.width - chainedWidth) / 2),
      y: Math.floor((region.facts.height - chainedHeight) / 2),
      width: chainedWidth,
      height: chainedHeight
    }
  }));
  assert.equal(chained.status, "succeeded", JSON.stringify(chained));
  assert.match(chained.facts.view, /^view_/);
  assert.notEqual(chained.facts.view, region.facts.view);
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 1600));

  const closed = structured(await request("tools/call", {
    name: "browser_tabs",
    arguments: { action: "close", tab }
  }));
  const preserved = closed.status === "blocked" && closed.facts.reason === "browser_local_interlock";
  assert.ok(closed.status === "succeeded" || preserved, JSON.stringify(closed));
  if (closed.status === "succeeded") assert.equal(closed.facts.closed, true);
  console.log(JSON.stringify({ live: true, catalog_tools: listed.result.tools.length, opened: true, read: true, screenshot: true, region_screenshot: true, chained_region_screenshot: true, closed: closed.status === "succeeded", preserved }));
} finally {
  child.stdin.end();
  child.kill();
}
