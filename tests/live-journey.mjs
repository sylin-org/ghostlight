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
    arguments: { url: "https://sylin.org/ghostlight/demo/iframe/" }
  }));
  assert.equal(opened.status, "succeeded", JSON.stringify(opened));
  const tab = opened.facts.tab;
  assert.match(tab, /^tab_/);

  const read = structured(await request("tools/call", {
    name: "browser_read",
    arguments: { tab }
  }));
  assert.equal(read.status, "succeeded", JSON.stringify(read));
  assert.match(read.facts.text, /Apply to the Sylin Foundry/i);
  assert.match(read.facts.text, /Project name/i);
  assert.match(read.facts.text, /Submit application/i);

  const inspected = structured(await request("tools/call", {
    name: "browser_inspect",
    arguments: { tab, scope: "document", max_depth: 8 }
  }));
  assert.equal(inspected.status, "succeeded", JSON.stringify(inspected));
  assert.ok(inspected.facts.nodes > 0, JSON.stringify(inspected));

  const projectMatches = structured(await request("tools/call", {
    name: "browser_find",
    arguments: { tab, text: "Project name", scope: "control", max_results: 5 }
  }));
  assert.equal(projectMatches.status, "succeeded", JSON.stringify(projectMatches));
  assert.ok(projectMatches.facts.matches.length > 0, JSON.stringify(projectMatches));
  const projectTarget = projectMatches.facts.matches.find((match) => match.role === "textbox")?.target;
  assert.match(projectTarget, /^target_/);

  const submitMatches = structured(await request("tools/call", {
    name: "browser_find",
    arguments: { tab, text: "Submit application", scope: "control", max_results: 5 }
  }));
  assert.equal(submitMatches.status, "succeeded", JSON.stringify(submitMatches));
  const submitTarget = submitMatches.facts.matches.find((match) => match.role === "button")?.target;
  assert.match(submitTarget, /^target_/);

  const waitedForForm = structured(await request("tools/call", {
    name: "browser_wait",
    arguments: { tab, condition: "text_present", value: "Submit application" }
  }));
  assert.equal(waitedForForm.status, "succeeded", JSON.stringify(waitedForForm));

  const hovered = structured(await request("tools/call", {
    name: "browser_hover",
    arguments: { tab, target: projectTarget }
  }));
  assert.equal(hovered.status, "succeeded", JSON.stringify(hovered));

  const targetScreenshotResponse = await request("tools/call", {
    name: "browser_screenshot",
    arguments: { tab, target: projectTarget }
  });
  const targetScreenshot = structured(targetScreenshotResponse);
  assert.equal(targetScreenshot.status, "succeeded", JSON.stringify(targetScreenshot));
  assert.equal(targetScreenshotResponse.result.content[1].type, "image");
  assert.ok(targetScreenshotResponse.result.content[1].data.length > 100);

  const filled = structured(await request("tools/call", {
    name: "browser_fill_form",
    arguments: {
      tab,
      fields: [
        { selector: { name: "Project name", role: "textbox", exact: true }, value: "Composed Lantern" },
        { selector: { name: "Contact email", role: "textbox", exact: true }, value: "test@example.com" },
        { selector: { name: "Repository URL", role: "textbox", exact: true }, value: "https://example.com/lantern" },
        { selector: { name: "Maintainer type", role: "combobox", exact: true }, value: "Individual" },
        { selector: { name: "Build system", role: "combobox", exact: true }, value: "GitHub Actions" },
        { selector: { name: "Notes", role: "textbox", exact: true }, value: "Full-page composed fixture" },
        {
          selector: {
            name: "I maintain this project and can answer questions about its releases.",
            role: "checkbox",
            exact: true
          },
          value: true
        }
      ],
      submit_target: submitTarget,
      expect: {
        condition: "text_present",
        value: "Application received. Nothing left your browser."
      }
    }
  }, 30000));
  assert.equal(filled.status, "succeeded", JSON.stringify(filled));

  const completed = structured(await request("tools/call", {
    name: "browser_wait",
    arguments: {
      tab,
      condition: "text_present",
      value: "Application received. Nothing left your browser."
    }
  }));
  assert.equal(completed.status, "succeeded", JSON.stringify(completed));

  const completedRead = structured(await request("tools/call", {
    name: "browser_read",
    arguments: { tab }
  }));
  assert.equal(completedRead.status, "succeeded", JSON.stringify(completedRead));
  assert.match(completedRead.facts.text, /Application received\. Nothing left your browser\./i);

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
  console.log(JSON.stringify({ live: true, catalog_tools: listed.result.tools.length, opened: true, composed_read: true, composed_inspect: true, composed_find: true, composed_wait: true, framed_hover: true, target_screenshot: true, composed_fill: true, composed_completion: true, screenshot: true, region_screenshot: true, chained_region_screenshot: true, closed: closed.status === "succeeded", preserved }));
} finally {
  child.stdin.end();
  child.kill();
}
