import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { createServer } from "node:http";

const repository = resolve(import.meta.dirname, "..");
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const binDir = process.env.GHOSTLIGHT_BIN_DIR || join(repository, "target", "release");
const connectorPath = join(binDir, `ghostlight-mcp-connector${executableSuffix}`);
const evidencePath = join(repository, ".tmp/installed-hardening-evidence.json");
const MAX_INVOCATION_RECORDS = 256;
const checks = [];
const evidence = [];
const report = {
  started_at: new Date().toISOString(),
  transport: "installed MCP -> service -> registered native host -> installed MV3 adapter",
  binaries: {}, passed: false, checks, invocations: evidence, preserved_tab: null,
  failure: { code: "run_incomplete" }
};

function writeEvidence() {
  report.recorded_at = new Date().toISOString();
  writeFileSync(evidencePath, JSON.stringify(report, null, 2) + "\n");
}

function failureCode(error) {
  return ["ERR_ASSERTION", "ETIMEDOUT", "EPIPE", "ENOENT", "ECONNRESET", "ERR_MCP_PROTOCOL", "ERR_EVIDENCE_LIMIT"]
    .includes(error?.code) ? error.code : "live_journey_failed";
}

function evidenceToken(value, limit) {
  return typeof value === "string" && value.length <= limit && /^[a-zA-Z0-9_-]+$/.test(value) ? value : null;
}

// Invalidate earlier success before startup, and keep bounded progress if this run is interrupted.
mkdirSync(join(repository, ".tmp"), { recursive: true });
writeEvidence();

let child;
const pending = new Map();
let nextId = 1;
let stderr = "";

function rejectPending(error) {
  for (const waiter of pending.values()) waiter.reject(error);
  pending.clear();
}

async function request(method, params = {}, timeoutMs = 15000) {
  let record;
  if (method === "tools/call") {
    if (evidence.length >= MAX_INVOCATION_RECORDS) {
      throw Object.assign(new Error("Installed invocation evidence limit reached"), { code: "ERR_EVIDENCE_LIMIT" });
    }
    record = { tool: evidenceToken(params.name, 80), invocation: null, status: null, effect: null };
    evidence.push(record);
    writeEvidence();
  }
  const id = nextId++;
  try {
    const response = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(JSON.stringify(id));
        reject(Object.assign(new Error(`Timed out waiting for MCP ${method}${stderr ? `: ${stderr.trim()}` : ""}`), { code: "ETIMEDOUT" }));
      }, timeoutMs);
      const waiter = {
        resolve(value) { clearTimeout(timer); resolve(value); },
        reject(error) { clearTimeout(timer); reject(error); }
      };
      pending.set(JSON.stringify(id), waiter);
      try {
        child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`, error => {
          if (!error || !pending.delete(JSON.stringify(id))) return;
          waiter.reject(error);
        });
      } catch (error) {
        pending.delete(JSON.stringify(id));
        waiter.reject(error);
      }
    });
    if (record) {
      const result = response.result?.structuredContent;
      record.invocation = evidenceToken(result?.invocation, 128);
      record.status = ["succeeded", "blocked", "failed", "cancelled", "attention_required", "unknown"].includes(result?.status) ? result.status : null;
      record.effect = ["none", "applied", "partial", "unknown"].includes(result?.effect) ? result.effect : null;
      if (response.error) record.failure = { code: "mcp_error", rpc_code: Number.isSafeInteger(response.error.code) ? response.error.code : null };
      else if (!record.tool || !record.invocation || !record.status || !record.effect) record.failure = { code: "invalid_tool_receipt" };
      if (record.failure) throw Object.assign(new Error("Invalid installed tool receipt"), { code: "ERR_MCP_PROTOCOL" });
    }
    return response;
  } catch (error) {
    if (record) record.failure ??= { code: failureCode(error) };
    throw error;
  } finally {
    if (record) writeEvidence();
  }
}

function notify(method, params = {}) {
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
}

function structured(response) {
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  return response.result.structuredContent;
}

function check(name) { checks.push(name); console.log(`PASS installed: ${name}`); }
async function call(name, args) {
  const response = await request("tools/call", { name, arguments: args }, 45000);
  return structured(response);
}

let capturedMask;
// Serve only a disposable fixture and its captured test image, never arbitrary machine files.
const localFixture = createServer((incoming, response) => {
  if (incoming.url === "/captured-mask.jpg" && capturedMask) {
    response.writeHead(200, { "Content-Type": "image/jpeg", "Cache-Control": "no-store" });
    response.end(capturedMask); return;
  }
  response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
  if (incoming.url === "/editor") {
    response.end(readFileSync(join(repository, "tests/fixtures/contenteditable.html"), "utf8")); return;
  }
  if (incoming.url === "/frames") {
    response.end(`<!doctype html><title>Installed document boundaries</title><h1>Permitted parent</h1>
      <label>Parent field<input aria-label="Parent field" id="parent"></label>
      <iframe title="Separate origin" src="http://127.0.0.1:${localFixture.address().port}/child" style="width:700px;height:200px"></iframe>`); return;
  }
  if (incoming.url === "/child") {
    response.end('<!doctype html><title>Child fixture</title><label>Excluded child sentinel<input aria-label="Excluded child sentinel"></label>'); return;
  }
  response.end("<!doctype html><html><head><title>Ghostlight local acceptance</title></head><body><h1>Local development works</h1></body></html>");
});

try {
  if (!existsSync(connectorPath)) throw Object.assign(new Error(`Repo-built MCP connector is missing ${connectorPath}`), { code: "ENOENT" });
  report.binaries = Object.fromEntries(["ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"].map(name => {
    const path = join(binDir, name + executableSuffix);
    return [name, { path, sha256: createHash("sha256").update(readFileSync(path)).digest("hex") }];
  }));
  writeEvidence();
  child = spawn(connectorPath, [], { env: process.env, stdio: ["pipe", "pipe", "pipe"], windowsHide: true });
  child.on("error", rejectPending);
  child.stdin.on("error", rejectPending);
  child.stderr.on("data", chunk => { stderr = (stderr + chunk.toString("utf8")).slice(-4000); });
  createInterface({ input: child.stdout }).on("line", line => {
    let message;
    try { message = JSON.parse(line); }
    catch { rejectPending(Object.assign(new Error("Invalid MCP protocol frame"), { code: "ERR_MCP_PROTOCOL" })); return; }
    const waiter = pending.get(JSON.stringify(message?.id));
    if (waiter) { pending.delete(JSON.stringify(message.id)); waiter.resolve(message); }
  });
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
  const authority = await call("policy_explain", {});
  assert.equal(authority.status, "succeeded", JSON.stringify(authority));
  assert.equal(authority.facts.layers.length, 0, "This installed acceptance lane requires all-open authority; it never changes policy.");

  await new Promise((resolve, reject) => {
    localFixture.once("error", reject);
    localFixture.listen(0, "127.0.0.1", resolve);
  });
  const localPort = localFixture.address().port;
  let localTab;
  for (const host of ["localhost", "127.0.0.1"]) {
    const localOpened = structured(await request("tools/call", {
      name: "browser_navigate",
      arguments: { url: `http://${host}:${localPort}/`, ...(localTab ? { tab: localTab } : { new_tab: true }) }
    }));
    assert.equal(localOpened.status, "succeeded", JSON.stringify(localOpened));
    localTab = localOpened.facts.tab;
    const localRead = structured(await request("tools/call", {
      name: "browser_read", arguments: { tab: localTab }
    }));
    assert.equal(localRead.status, "succeeded", JSON.stringify(localRead));
    assert.match(localRead.facts.text, /Local development works/);
  }
  const restrictedLocal = structured(await request("tools/call", {
    name: "browser_read", arguments: { tab: localTab, restrict_hosts: ["example.com"] }
  }));
  assert.equal(restrictedLocal.status, "blocked", JSON.stringify(restrictedLocal));
  assert.equal(restrictedLocal.facts.reason, "host_denied");
  console.log(JSON.stringify({ localhost: true, loopback: true, local_policy_denial: true }));

  const editor = await call("browser_navigate", { tab: localTab, url: `http://localhost:${localPort}/editor` });
  assert.equal(editor.status, "succeeded", JSON.stringify(editor));
  const tabEvidence = async script => {
    const result = await call("browser_execute", { tab: localTab, script });
    assert.equal(result.status, "succeeded", JSON.stringify(result)); return result.facts.value;
  };
  const inspectedEditor = await call("browser_inspect", { tab: localTab, scope: "controls", max_items: 100 });
  assert.equal(inspectedEditor.status, "succeeded", JSON.stringify(inspectedEditor));
  const reply = inspectedEditor.facts.items.find(item => item.name === "Reply");
  const shadow = inspectedEditor.facts.items.find(item => item.name === "Shadow reply");
  assert.ok(reply && shadow, JSON.stringify(inspectedEditor));
  const hidden = inspectedEditor.facts.items.find(item => item.name === "Hidden draft helper");
  if (hidden) assert.ok(hidden.state.includes("hidden"));
  const fill = await call("browser_fill_form", { tab: localTab, restrict_capabilities: ["read", "write"], fields: [
    { target: reply.target, value: "Installed unsent draft\nSecond line" }, { target: shadow.target, value: "Installed shadow draft" }
  ] });
  assert.equal(fill.status, "succeeded", JSON.stringify(fill)); assert.equal(fill.facts.submitted, false);
  let editorValue = await tabEvidence("editorEvidence()");
  assert.equal(editorValue.reply.value, "Installed unsent draft\nSecond line");
  assert.equal(editorValue.reply.rendered, editorValue.reply.value);
  assert.equal(editorValue.shadow.value, "Installed shadow draft"); assert.equal(editorValue.submissions, 0);
  check("ordinary and shadow rich-editor drafts are retained without submission");
  const readOnly = inspectedEditor.facts.items.find(item => item.name === "Read only draft"); assert.ok(readOnly);
  const refusedBatch = await call("browser_fill_form", { tab: localTab, fields: [
    { target: reply.target, value: "MUST_NOT_CHANGE_EARLIER_DRAFT" }, { target: readOnly.target, value: "MUST_NOT_CHANGE_READONLY" }
  ] });
  assert.notEqual(refusedBatch.status, "succeeded", JSON.stringify(refusedBatch));
  const afterRefusal = await tabEvidence("editorEvidence()");
  assert.equal(afterRefusal.reply.value, editorValue.reply.value);
  assert.equal(afterRefusal.readonly, "Protected input"); assert.equal(afterRefusal.submissions, 0);
  check("known ineligible batch field leaves the earlier draft unchanged");
  const typed = await call("browser_type_text", { tab: localTab, target: reply.target, text: "Action-only typing", clear_first: true,
    restrict_capabilities: ["action"] });
  assert.equal(typed.status, "succeeded", JSON.stringify(typed));
  const afterTyping = await tabEvidence("editorEvidence()");
  assert.equal(afterTyping.reply.value, "Action-only typing"); assert.equal(afterTyping.reply.rendered, "Action-only typing");
  assert.equal(afterTyping.shadow.value, "Installed shadow draft"); assert.equal(afterTyping.submissions, 0);
  const cleared = await call("browser_type_text", { tab: localTab, focused: true, text: "", clear_first: true, restrict_capabilities: ["action"] });
  assert.equal(cleared.status, "succeeded", JSON.stringify(cleared));
  editorValue = await tabEvidence("editorEvidence()");
  assert.equal(editorValue.reply.value, ""); assert.equal(editorValue.reply.rendered, "");
  assert.equal(editorValue.shadow.value, "Installed shadow draft"); assert.equal(editorValue.submissions, 0);
  check("Action-only targeted typing and focused clearing preserve their actual effects");

  for (const [onError, expected] of [["stop", 1], ["continue", 2]]) {
    await tabEvidence("window.installedEffects = 0; 0");
    const flow = await call("browser_flow", { on_error: onError, steps: [
      { id: "failed", tool: "browser_execute", arguments: { tab: localTab,
        script: "window.installedEffects++; throw new SyntaxError('Illegal return statement');" } },
      { id: "later", tool: "browser_execute", arguments: { tab: localTab, script: "window.installedEffects++; return 2;" } }
    ] });
    assert.equal(flow.status, "unknown", JSON.stringify(flow)); assert.equal(flow.effect, "unknown");
    assert.equal(flow.repeat_safe, false);
    assert.equal(flow.facts.steps[1].status, onError === "stop" ? "not_run" : "succeeded");
    assert.equal(await tabEvidence("window.installedEffects"), expected);
    check(`${onError}: script exceptions execute once and composition keeps truthful progress`);
  }
  const invalid = await call("browser_execute", { tab: localTab, script: "window.installedEffects++; const broken = ();" });
  assert.equal(invalid.status, "failed", JSON.stringify(invalid)); assert.equal(invalid.effect, "none");
  assert.equal(await tabEvidence("window.installedEffects"), 2);
  const awaited = await tabEvidence("await Promise.resolve(); return 7;"); assert.equal(awaited, 7);
  check("syntax refusal has no effect and awaited bare returns retain their value");

  const framed = await call("browser_navigate", { tab: localTab, url: `http://localhost:${localPort}/frames` });
  assert.equal(framed.status, "succeeded", JSON.stringify(framed));
  const unrestricted = await call("browser_read", { tab: localTab });
  assert.equal(unrestricted.status, "succeeded", JSON.stringify(unrestricted));
  assert.match(unrestricted.facts.text, /Excluded child sentinel/);
  const restricted = await call("browser_read", { tab: localTab, restrict_hosts: ["localhost"] });
  assert.equal(restricted.status, "succeeded", JSON.stringify(restricted));
  assert.equal(restricted.facts.coverage.excluded_documents, 1);
  assert.match(restricted.facts.text, /Permitted parent/);
  assert.doesNotMatch(JSON.stringify(restricted), /127\.0\.0\.1|Excluded child sentinel/);
  const parentFill = await call("browser_fill_form", { tab: localTab, restrict_hosts: ["localhost"], fields: [
    { selector: { name: "Parent field", role: "textbox", exact: true }, value: "Permitted parent draft" }
  ] });
  assert.equal(parentFill.status, "succeeded", JSON.stringify(parentFill));
  assert.equal(await tabEvidence("document.getElementById('parent').value"), "Permitted parent draft");
  const maskedResponse = await request("tools/call", { name: "browser_screenshot", arguments: { tab: localTab, restrict_hosts: ["localhost"] } }, 30000);
  const masked = structured(maskedResponse); assert.equal(masked.status, "succeeded", JSON.stringify(masked));
  assert.equal(masked.facts.coverage.masked_regions, 1);
  const maskedImage = maskedResponse.result.content.find(item => item.type === "image"); assert.ok(maskedImage);
  mkdirSync(join(repository, ".tmp"), { recursive: true });
  capturedMask = Buffer.from(maskedImage.data, "base64");
  writeFileSync(join(repository, ".tmp/installed-hardening-mask.jpg"), capturedMask);
  const pixels = await tabEvidence(`
    const image = new Image(); image.src = '/captured-mask.jpg';
    await image.decode(); const canvas = document.createElement('canvas');
    canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext('2d'); context.drawImage(image,0,0);
    const rectangle = document.querySelector('iframe').getBoundingClientRect();
    const scale = image.width / innerWidth;
    return [0.2,0.8].flatMap(x => [0.2,0.8].map(y => [...context.getImageData(
      Math.round((rectangle.left + rectangle.width*x)*scale),
      Math.round((rectangle.top + rectangle.height*y)*scale),1,1).data]));
  `);
  assert.ok(Array.isArray(pixels), JSON.stringify(pixels));
  for (const pixel of pixels) for (const [index, expected] of [32, 36, 43, 255].entries()) {
    assert.ok(Math.abs(pixel[index] - expected) <= 5, `Installed exclusion pixel: ${pixel}`);
  }
  assert.equal(await tabEvidence("getComputedStyle(document.querySelector('iframe')).visibility"), "visible");
  const refusedScript = await call("browser_execute", { tab: localTab, restrict_hosts: ["localhost"], script: "document.title = 'MUST_NOT_RUN'" });
  assert.equal(refusedScript.status, "blocked", JSON.stringify(refusedScript));
  assert.equal(refusedScript.effect, "none"); assert.notEqual(await tabEvidence("document.title"), "MUST_NOT_RUN");
  check("installed native host preserves permitted work, excludes child content, masks captures, and refuses unbounded scripts");

  const opened = structured(await request("tools/call", {
    name: "browser_navigate",
    arguments: { tab: localTab, url: "https://sylin.org/ghostlight/demo/iframe/" }
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
  check("live Sylin framed form, local-only submission, and target/viewport/magnified captures");
  report.preserved_tab = preserved ? tab : null;
  report.passed = true;
  report.failure = null;
} catch (error) {
  report.passed = false;
  report.failure = { code: failureCode(error) };
  throw error;
} finally {
  try {
    if (child) { child.stdin.end(); child.kill(); }
    localFixture.closeAllConnections();
    if (localFixture.listening) await new Promise((resolve) => localFixture.close(resolve));
  } catch (error) {
    report.passed = false;
    report.failure = { code: failureCode(error) };
    throw error;
  } finally {
    writeEvidence();
  }
}
