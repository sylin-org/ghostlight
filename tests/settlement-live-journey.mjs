// Installed-browser acceptance for bounded settlement and unrelated active-frame churn.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { randomUUID, createHash } from "node:crypto";

const root = resolve(import.meta.dirname, "..");
const run = randomUUID();
const directory = join(root, ".tmp", "settlement-live", run);
mkdirSync(directory, { recursive: true });
const attachment = join(directory, "attachment.txt");
writeFileSync(attachment, "bounded settlement attachment");
const records = [], checks = [];
const report = { run, installed: true, records, checks, passed: false, owned_tab: null };
const evidence = join(directory, "evidence.json");
const save = () => writeFileSync(evidence, JSON.stringify(report, null, 2));
save();

const html = `<!doctype html><meta charset="utf-8"><title>Ghostlight bounded settlement acceptance</title>
<style>body{font:18px system-ui;background:#102025;color:#eee;margin:25px}button,input{margin:8px;padding:12px}
@keyframes busy{from{opacity:.5}to{opacity:1}}#motion{animation:busy 60s linear}#churn{width:80px;height:20px}
#source,#destination{display:inline-block;background:#267078;padding:25px;margin:10px}#destination{background:#614479}
main{min-height:2600px}</style><main><h1>Mutation fixture</h1><p id="motion">Finite animation remains running during every tool call.</p>
<button id="click">Click once</button><label>Draft<input id="draft" aria-label="Draft"></label>
<button id="hover1">Hover one</button><button id="hover2">Hover two</button>
<div id="source" role="button" aria-label="Drag source" draggable="true">Drag source</div>
<div id="destination" role="button" aria-label="Drop destination">Drop destination</div>
<label>Attachment<input id="upload" aria-label="Attachment" type="file"></label>
<p>Changing embedded document: <iframe id="churn" src="/child?initial"></iframe></p><output id="counter">0</output></main>
<script>
window.acceptance={clicks:0,keys:0,hovers:0,drags:0,drops:0,uploads:0,uploadedText:'',scripts:0,churn:0};
click.onclick=()=>{acceptance.clicks++;counter.textContent=acceptance.clicks};
draft.addEventListener('keydown',e=>{if(e.key==='ArrowRight')acceptance.keys++});
for(const el of [hover1,hover2])el.addEventListener('mouseenter',()=>acceptance.hovers++);
source.addEventListener('dragstart',e=>{acceptance.drags++;e.dataTransfer.setData('text/plain','owned fixture')});
destination.addEventListener('dragover',e=>e.preventDefault());destination.addEventListener('drop',e=>{e.preventDefault();acceptance.drops++});
upload.addEventListener('change',async()=>{acceptance.uploads++;acceptance.uploadedText=await upload.files[0].text()});
setInterval(()=>{acceptance.churn++;churn.src='/child?generation='+acceptance.churn},130);
</script>`;
const server = createServer((req, res) => {
  res.setHeader("Content-Type", "text/html");
  res.end(req.url.startsWith("/child") ? "<!doctype html><p>Changing auxiliary frame</p>" : html);
});
await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
const url = `http://127.0.0.1:${server.address().port}/${run}`;
report.url = url;
const connector = join(root, "target", "release", `ghostlight-mcp-connector${process.platform === "win32" ? ".exe" : ""}`);
report.authority_sha256 = createHash("sha256").update(readFileSync(join(root, "target/release/ghostlight.exe"))).digest("hex");
const child = spawn(connector, [], { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
let next = 0, stderr = "";
const pending = new Map();
child.stderr.on("data", chunk => { stderr = (stderr + chunk).slice(-2000); });
createInterface({ input: child.stdout }).on("line", line => {
  const message = JSON.parse(line), waiter = pending.get(message.id);
  if (waiter) { pending.delete(message.id); clearTimeout(waiter.timer); waiter.resolve(message); }
});
const request = (method, params) => new Promise((resolve, reject) => {
  const id = ++next;
  const timer = setTimeout(() => { pending.delete(id); reject(new Error(`MCP timeout: ${method} ${stderr}`)); }, 20000);
  pending.set(id, { resolve, timer });
  child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
});
async function call(tool, args, expected = "succeeded") {
  const start = performance.now();
  const response = await request("tools/call", { name: tool, arguments: args });
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  const result = response.result.structuredContent;
  if (tool === "browser_screenshot") {
    const image = response.result.content.find(item => item.type === "image");
    if (image) {
      const path = join(directory, "fixture.jpg");
      writeFileSync(path, Buffer.from(image.data, "base64"));
      report.screenshot = path;
    }
  }
  records.push({ tool, opted_out: args.visual_settle === false, duration_ms: Math.round(performance.now() - start),
    invocation: result.invocation, status: result.status, effect: result.effect, summary: result.summary });
  save();
  assert.equal(result.status, expected, `${tool}: ${JSON.stringify(result)}`);
  return result;
}
let tab;
try {
  await request("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "settlement-installed-acceptance", version: "1" } });
  child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
  const catalog = await request("tools/list", {});
  for (const name of ["browser_read", "browser_inspect", "browser_find", "browser_screenshot", "browser_click", "browser_scroll", "browser_hover", "browser_fill_form", "browser_type_text", "browser_press_key", "browser_drag", "browser_upload", "browser_execute"]) {
    assert.equal(catalog.result.tools.find(tool => tool.name === name).inputSchema.properties.visual_settle.default, true, name);
  }
  const opened = await call("browser_navigate", { url, new_tab: true });
  tab = opened.facts.tab;
  report.owned_tab = tab;
  const inspect = await call("browser_inspect", { tab, visual_settle: false, max_items: 50 });
  const items = inspect.facts.items;
  const target = name => {
    const matches = items.filter(item => item.name === name);
    assert.equal(matches.length, 1, `${name}: ${JSON.stringify(items)}`);
    return matches[0].target;
  };
  const snapshot = async () => (await call("browser_execute", { tab, visual_settle: false,
    script: "return {...window.acceptance,draft:document.getElementById('draft').value,animationRunning:document.getAnimations().some(a=>a.playState==='running')};" })).facts.value;
  for (const enabled of [true, false]) {
    const args = { tab, visual_settle: enabled, timeout_ms: 5000 };
    const before = await snapshot();
    await call("browser_execute", { tab, visual_settle: false,
      script: "document.getElementById('upload').value=''; return true;" });
    await call("browser_read", { tab, visual_settle: enabled });
    await call("browser_inspect", { tab, visual_settle: enabled });
    await call("browser_find", { tab, visual_settle: enabled, text: "Click once" });
    await call("browser_screenshot", args);
    await call("browser_click", { ...args, selector: { name: "Click once", role: "button", exact: true } });
    await call("browser_fill_form", { ...args, fields: [{ target: target("Draft"), value: "filled" }] });
    await call("browser_type_text", { ...args, target: target("Draft"), text: "typed", clear_first: true });
    await call("browser_press_key", { tab, visual_settle: enabled, target: target("Draft"), key: "ArrowRight" });
    await call("browser_hover", { ...args, target: target(enabled ? "Hover one" : "Hover two") });
    await call("browser_drag", { ...args, source_target: target("Drag source"), destination_target: target("Drop destination") });
    await call("browser_upload", { ...args, target: target("Attachment"), paths: [attachment] });
    await call("browser_execute", { ...args, script: "window.acceptance.scripts++; return window.acceptance.scripts;" });
    await call("browser_scroll", { ...args, direction: "down", amount: "small" });
    const after = await snapshot();
    assert.equal(after.clicks, before.clicks + 1, "click handler ran exactly once");
    assert.equal(after.keys, before.keys + 1, "native key reached the actual control");
    assert.equal(after.draft, "typed", "trusted edits remain in the actual control");
    assert.equal(after.hovers, before.hovers + 1, "hover reached the actual control exactly once");
    assert.equal(after.drags, before.drags + 1, "drag handler ran exactly once");
    assert.equal(after.drops, before.drops + 1, "drop handler ran exactly once");
    assert.equal(after.uploads, before.uploads + 1, "file change handler ran exactly once");
    assert.equal(after.uploadedText, "bounded settlement attachment");
    assert.equal(after.scripts, before.scripts + 1);
    assert.equal(after.animationRunning, true, "work completed while finite animation continued");
    assert.ok(after.churn > before.churn, "auxiliary document kept navigating");
    checks.push({ enabled, counters: after }); save();
  }
  await call("browser_wait", { tab, condition: "load_ready", timeout_ms: 5000 });
  await call("browser_wait", { tab, condition: "visual_settle", timeout_ms: 150 }, "failed");
  const settled = await call("browser_wait", { tab, condition: "load_ready", visual_settle: false });
  assert.equal(settled.facts.satisfied, true);
  for (const name of ["browser_read", "browser_inspect", "browser_find", "browser_screenshot", "browser_click", "browser_scroll", "browser_hover", "browser_fill_form", "browser_type_text", "browser_press_key", "browser_drag", "browser_upload", "browser_execute"]) {
    const slow = records.find(row => row.tool === name && !row.opted_out);
    const fast = records.find(row => row.tool === name && row.opted_out);
    assert.ok(slow.duration_ms < 5000, `${name} stays within its command budget`);
    assert.ok(slow.duration_ms - fast.duration_ms >= 450, `${name} opt-out removes bounded waiting: ${JSON.stringify({slow,fast})}`);
  }
  // Return the evidence image to the controls after the scrolling checks.
  await call("browser_scroll", { tab, visual_settle: false, direction: "up", amount: "page" });
  await call("browser_scroll", { tab, visual_settle: false, direction: "up", amount: "page" });
  await call("browser_screenshot", { tab, visual_settle: false });
  report.passed = true;
  console.log(JSON.stringify({ passed: true, evidence, records: records.length, checks }));
} catch (error) {
  report.failure = error.message; console.error(error.stack); process.exitCode = 1;
} finally {
  save();
  child.stdin.end(); child.kill();
  server.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  console.log(`Owned fixture tab is retained for inspection: ${tab ?? "not opened"}`);
}
