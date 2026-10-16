// H6 acceptance with the shipped Manifest V3 worker, page code, Chrome document APIs and CDP.
// Every call crosses the real MCP edge, the service of a test home and the real browser edge into
// the extension (tests/lib/browser.mjs replaces only native-port discovery). The pages are read
// through the journey's own DevTools session. No existing profile, native-host registration or
// installed extension is changed.
import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { BrowserRig, SCRATCH_ROOT } from "./lib/browser.mjs";
import { createHome, delay, repository, until as pollUntil } from "./lib/home.mjs";
import { structured } from "./lib/peers.mjs";
import { policyRefreshed } from "./lib/policy-refresh.mjs";

const scratchRoot = SCRATCH_ROOT;
const until = (check, label, timeout = 30000) => pollUntil(check, label, timeout);
// The primitives and world calls that change a page or type into it; none may be sent for a call
// the policy refuses.
const WRITING_WORK = new Set(["input", "call:prepare_fill", "call:fill_local", "call:submit_fill", "call:type_text", "call:clear_focused"]);
const HOST_RULES = "MAP * ~NOTFOUND, EXCLUDE localhost, EXCLUDE 127.0.0.1";
const policyFor = (child = "denied", screenshots = true) => ({
  schema: 4, name: "H6 isolated Sylin journey", version: randomUUID(),
  grants: [{ id: "parent", hosts: { allow: child === "unrestricted" ? ["*"] : ["sylin.org", "localhost"] }, allowed: ["read", "action", "write", "execute"] },
    ...(["denied", "unrestricted"].includes(child) ? [] : [{ id: "child", hosts: { allow: ["127.0.0.1"] }, allowed: child === "read" ? ["read"] : ["read", "action", "write", "execute"] }])],
  config: [{ key: "browser.startup", value: "manual", level: "mandatory" },
    { key: "browser.screenshots.enabled", value: screenshots, level: "mandatory" }]
});
const home = createHome({ label: "frame", policy: policyFor() });
async function policy(child = "denied", screenshots = true) {
  writeFileSync(home.policyFile, JSON.stringify(policyFor(child, screenshots)));
  await policyRefreshed();
}
function attributesOf(node) {
  const pairs = new Map();
  for (let index = 0; index + 1 < (node.attributes?.length ?? 0); index += 2) pairs.set(node.attributes[index], node.attributes[index + 1]);
  return pairs;
}
function descendants(node) {
  return [...(node.children ?? []), ...(node.shadowRoots ?? []), ...(node.contentDocument ? [node.contentDocument] : []),
    ...(node.pseudoElements ?? [])].flatMap((child) => [child, ...descendants(child)]);
}
// The runtime object's type in every ghostlight isolated world of a page, its out-of-process
// frames included. DevTools sees the worlds the extension created; the page's own code cannot.
const WORLD = "ghostlight";
async function isolatedRuntimeKinds(page) {
  const attached = [];
  rig.cdp.onEvent((message) => {
    if (message.sessionId === page.sessionId && message.method === "Target.attachedToTarget" && message.params.targetInfo.type === "iframe") {
      attached.push(message.params.sessionId);
    }
  });
  await page.send("Target.setAutoAttach", { autoAttach: true, waitForDebuggerOnStart: false, flatten: true });
  await delay(500);
  const kinds = [];
  for (const sessionId of [page.sessionId, ...attached]) {
    const created = [];
    const destroyed = new Set();
    rig.cdp.onEvent((message) => {
      if (message.sessionId !== sessionId) return;
      if (message.method === "Runtime.executionContextCreated") created.push(message.params.context);
      if (message.method === "Runtime.executionContextDestroyed") destroyed.add(message.params.executionContextId);
    });
    await rig.cdp.send("Runtime.enable", {}, sessionId);
    await delay(300);
    for (const context of created.filter((item) => item.name === WORLD && !destroyed.has(item.id))) {
      const reply = await rig.cdp.send("Runtime.evaluate",
        { expression: "typeof globalThis.__ghostlightRuntime", contextId: context.id, returnByValue: true }, sessionId);
      kinds.push(reply.result.value);
    }
    await rig.cdp.send("Runtime.disable", {}, sessionId);
  }
  await page.send("Target.setAutoAttach", { autoAttach: false, waitForDebuggerOnStart: false, flatten: true });
  return kinds;
}
const sourceUrls = ["https://sylin.org/ghostlight/demo/iframe/", "https://sylin.org/ghostlight/demo/iframe/form/"];
const liveSylin = process.env.GHOSTLIGHT_TEST_LIVE_SYLIN === "1";
const snapshot = JSON.parse(readFileSync(join(repository, "tests/fixtures/sylin-iframe.json"), "utf8"));
const source = liveSylin
  ? await Promise.all(sourceUrls.map(async (url) => { const response = await fetch(url, { signal: AbortSignal.timeout(20000) }); assert.equal(response.status, 200); return response.text(); }))
  : snapshot.pages.map((page, index) => {
    assert.equal(page.url, sourceUrls[index]);
    assert.equal(createHash("sha256").update(page.body).digest("hex"), page.sha256);
    return page.body;
  });
// The public stage builds its form iframe inside the open shadow root of <composed-iframe-stage>
// and puts the <frame-proof role="button"> control in that same root. Page code reaches both here.
const STAGE_ROOT = "document.querySelector('composed-iframe-stage').shadowRoot";
const frameElement = `${STAGE_ROOT}.querySelector('iframe')`;
const PROOF_LABEL = "Verify composed frame";
const PROVEN_LABEL = "Composed frame verified";
// Site files the pages load from the public origin: the snapshot holds them, live mode fetches them.
const SITE_ORIGIN = "https://sylin.org";
const siteAssetPath = /^\/(?:assets|cdn-cgi)\//;
const referencedAssets = (html) => [...html.matchAll(/<(?:link|script|img)\b[^>]*?\s(?:href|src)="(\/(?:assets|cdn-cgi)\/[^"]+)"/g)].map((match) => match[1]);
if (!liveSylin) {
  const held = new Set(snapshot.assets.map((item) => item.path));
  for (const path of source.flatMap(referencedAssets)) assert.ok(held.has(path), `the snapshot is missing ${path}; recapture it with scripts/capture-sylin-iframe.mjs`);
  assert.ok(source[0].includes("<composed-iframe-stage"), "the snapshot predates the composed stage");
}
const assetFailures = [];
const liveAssets = new Map();
// The bytes and content type of a site file, or null when the mode cannot serve it.
async function siteAsset(url) {
  if (!liveSylin) {
    const item = snapshot.assets.find((entry) => entry.path.split("?")[0] === url.split("?")[0]);
    if (!item) return null;
    const bytes = Buffer.from(item.data_base64, "base64");
    assert.equal(createHash("sha256").update(bytes).digest("hex"), item.sha256);
    return { bytes, type: item.content_type };
  }
  if (!liveAssets.has(url)) {
    liveAssets.set(url, fetch(`${SITE_ORIGIN}${url}`, { signal: AbortSignal.timeout(20000) }).then(async (response) =>
      response.status === 200 ? { bytes: Buffer.from(await response.arrayBuffer()), type: response.headers.get("content-type") } : null));
  }
  return liveAssets.get(url);
}
const sourceEvidence = source.map((html, index) => ({ url: sourceUrls[index], sha256: createHash("sha256").update(html).digest("hex") }));
let port;
const server = createServer(async (request, response) => {
  response.setHeader("content-type", "text/html");
  if (siteAssetPath.test(request.url)) {
    const asset = await siteAsset(request.url).catch(() => null);
    if (!asset) { assetFailures.push(request.url); response.statusCode = 404; response.end(); return; }
    response.setHeader("content-type", asset.type); response.end(asset.bytes); return;
  }
  if (request.url === "/editor") {
    response.end(readFileSync(join(repository, "tests/fixtures/contenteditable.html"), "utf8"));
    return;
  }
  const child = request.url.startsWith("/form");
  let html = source[child ? 1 : 0].replace("<head>", `<head><base href="http://localhost:${port}/">`);
  if (child && request.url.includes("readonly-fixture")) html = html.replace("</body>", '<label>Embedded read only draft<input readonly aria-label="Embedded read only draft" value="Protected embedded input"></label></body>');
  if (child && request.url.includes("capture-pixel")) html = html.replace("</body>", '<div id="capture-pixel-fixture" style="position:fixed;left:0;top:0;width:100px;height:100px;background:rgb(255,0,255);z-index:2147483647;pointer-events:none"></div></body>');
  if (!child) html = html.replace('src="./form/"', `src="http://${request.url === "/same-origin" ? "localhost" : "127.0.0.1"}:${port}/form"`)
    .replace("</body>", '<label>Parent note<input aria-label="Parent note" id="h6-parent"></label></body>');
  response.end(html);
});
let rig;
try {
  await new Promise((done) => server.listen(0, "127.0.0.1", done)); port = server.address().port;
  rig = await BrowserRig.start({ home, label: "frame-browser", hostRules: liveSylin ? null : HOST_RULES });
  const version = { product: rig.version };
  const rawWorker = (expression) => rig.worker(expression);
  let mcp = await rig.mcp("h6-browser-journey");
  let lastResponse;
  const call = async (name, args) => {
    const response = await mcp.call(name, args);
    lastResponse = response.result;
    assert.ok(lastResponse?.structuredContent, JSON.stringify(response));
    return lastResponse.structuredContent;
  };
  const newSession = async () => { mcp = await rig.mcp("h6-case"); };
  const dispatchedSince = (index) => rig.bridge.sentSince(index);
  const refusedWork = (index) => dispatchedSince(index).filter((name) => WRITING_WORK.has(name));
  const passed = [];
  const check = (name) => { passed.push(name); console.log(`PASS ${name}`); };
  const live = await call("browser_navigate", { url: liveSylin ? sourceUrls[0] : `http://localhost:${port}/same-origin`, new_tab: true });
  assert.equal(live.status, "succeeded", JSON.stringify(live));
  let result = await call("browser_read", { tab: live.facts.tab, mode: "visible", max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage?.excluded_documents ?? 0, 0, JSON.stringify(result));
  assert.match(JSON.stringify(result), /Project name/); check(`${liveSylin ? "live" : "snapshot"} Sylin composed document read`);
  const opened = await call("browser_navigate", { url: `http://localhost:${port}/`, new_tab: true });
  assert.equal(opened.status, "succeeded", JSON.stringify(opened)); let tab = opened.facts.tab;
  let physical = await rig.physicalTab(`http://localhost:${port}/`);
  const freshFixture = async () => {
    await newSession();
    const next = await call("browser_navigate", { url: `http://localhost:${port}/`, new_tab: true });
    assert.equal(next.status, "succeeded", JSON.stringify(next)); tab = next.facts.tab;
    physical = await rig.physicalTab(`http://localhost:${port}/`);
  };
  const rawPage = async (expression) => (await rig.page(physical)).evaluate(expression);
  await until(async () => (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`)).length >= 2, "fixture frame")
    .catch(async (error) => { console.error(await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`));
      console.error(await (await rig.page(physical)).send("Page.getFrameTree")); throw error; });
  assert.deepEqual(await rawPage(`(() => {
    const root = document.querySelector("composed-iframe-stage")?.shadowRoot;
    return { open: Boolean(root), lightFrames: document.querySelectorAll("iframe").length,
      shadowFrames: root?.querySelectorAll("iframe").length ?? 0, proofRole: root?.querySelector("frame-proof")?.getAttribute("role") ?? null };
  })()`), { open: true, lightFrames: 0, shadowFrames: 1, proofRole: "button" });
  check("the form iframe and the proof control live in the open shadow root of the composed stage");
  await policy("denied");
  result = await call("browser_read", { tab, mode: "visible", max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage.excluded_documents, 1);
  assert.doesNotMatch(JSON.stringify(result), /127\.0\.0\.1|Project name|ifxf-project/);
  check("excluded child never extracted");
  await policy();
  await freshFixture();
  const beforeBlockedScript = rig.bridge.sent.length;
  result = await call("browser_execute", { tab, script: "document.title = 'must not execute'" });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.ok(!dispatchedSince(beforeBlockedScript).includes("evaluate_main"));
  assert.notEqual(await rawPage("document.title"), "must not execute"); check("script refused before execution");
  const embedAt = async (path) => {
    await rawPage(`${frameElement}.src = 'http://127.0.0.1:${port}/${path}'; true`);
    await delay(200);
    await until(async () => (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`)).some((frame) => frame.url.endsWith(`/${path}`)), `embedded ${path}`);
  };
  await embedAt("form?capture-pixel");
  result = await call("browser_screenshot", { tab, full_page: true });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage, undefined);
  const screenshot = lastResponse.content.find((item) => item.type === "image"); assert.ok(screenshot);
  writeFileSync(join(scratchRoot, "h6-complete-sylin.jpg"), Buffer.from(screenshot.data, "base64"));
  const embeddedPixel = await rawPage(`(async () => {
    const image = new Image(); image.src = ${JSON.stringify(`data:${screenshot.mimeType};base64,${screenshot.data}`)};
    await image.decode(); const canvas = document.createElement("canvas");
    canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext("2d"); context.drawImage(image, 0, 0);
    const box = ${frameElement}.getBoundingClientRect();
    const scale = image.width / document.documentElement.scrollWidth;
    return [...context.getImageData(Math.round((box.left + scrollX + 25) * scale),
      Math.round((box.top + scrollY + 25) * scale), 1, 1).data];
  })()`);
  assert.ok(embeddedPixel[0] > 200 && embeddedPixel[1] < 40 && embeddedPixel[2] > 200,
    `Complete capture must contain the actual embedded pixels: ${embeddedPixel}`);
  await embedAt("form");
  assert.equal(await rawPage(`getComputedStyle(${frameElement}).visibility`), "visible");
  check("complete screenshot remains available with embedded text access excluded");
  await policy("denied", false);
  const beforeDeniedCapture = rig.bridge.sent.length;
  for (const full_page of [false, true]) {
    result = await call("browser_screenshot", { tab, full_page });
    assert.equal(result.status, "blocked", JSON.stringify(result));
    assert.ok(!lastResponse.content.some(item => item.type === "image"));
  }
  assert.ok(!dispatchedSince(beforeDeniedCapture).includes("capture"));
  check("binary screenshot refusal dispatches no capture and returns no image");
  await policy("all");
  result = await call("browser_read", { tab, mode: "visible", max_chars: 20000 });
  const hidden = await rawPage("[typeof globalThis.__ghostlightRuntime, typeof window.__ghostlight_dispatch__, typeof window.__ghostlight_runtime__]");
  assert.deepEqual(hidden, ["undefined", "undefined", "undefined"], "the page cannot see Ghostlight's runtime");
  const worlds = await isolatedRuntimeKinds(await rig.page(physical));
  assert.ok(worlds.length >= 2 && worlds.every((kind) => kind === "object"), `every document has the runtime in its own world: ${JSON.stringify(worlds)}`);
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.match(JSON.stringify(result), /Project name/);
  check("same Sylin content fully available when allowed");
  await policy("all");
  result = await call("browser_read", { tab, mode: "visible", max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage?.excluded_documents ?? 0, 0);
  assert.equal(result.facts.coverage?.unavailable_documents ?? 0, 0);
  assert.match(JSON.stringify(result), /Project name/);
  check("fully permitted content remains available");
  result = await call("browser_find", { tab, text: PROOF_LABEL });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  const proof = result.facts.matches.find((match) => match.role === "button" && match.name?.includes(PROOF_LABEL));
  assert.ok(proof, JSON.stringify(result));
  check("the control inside the open shadow root is found by its label");
  result = await call("browser_click", { tab, target: proof.target });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(await rawPage(`${STAGE_ROOT}.querySelector('frame-proof [slot="label"]').textContent`), PROVEN_LABEL);
  result = await call("browser_find", { tab, text: PROVEN_LABEL });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.ok(result.facts.matches.some((match) => match.name?.includes(PROVEN_LABEL)), JSON.stringify(result));
  check("clicking the shadow-hosted control changes its label");
  result = await call("browser_execute", { tab, script: "document.body.dataset.executionCount=String(Number(document.body.dataset.executionCount||0)+1); await Promise.resolve(); return Number(document.body.dataset.executionCount);" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.value, 1);
  assert.equal(await rawPage("document.body.dataset.executionCount"), "1");
  check("permitted embedded page executes awaited bare return once through the shipped MV3 worker");
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 100 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  const project = result.facts.items.find((item) => item.name === "Project name");
  const parent = result.facts.items.find((item) => item.name === "Parent note");
  assert.ok(project && parent, JSON.stringify(result));
  await policy("read");
  result = await call("browser_fill_form", { tab, fields: [{ target: parent.target, value: "MUST_NOT_FILL" }, { target: project.target, value: "MUST_NOT_FILL" }] });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.equal(await rawPage("document.getElementById('h6-parent').value"), "");
  check("readable child cannot be written; known batch denial precedes parent write");
  await policy("all");
  await rawPage(`${frameElement}.src='http://127.0.0.1:${port}/form?readonly-fixture'; true`);
  await delay(200);
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 100 });
  const embeddedReadonly = result.facts.items.find((item) => item.name === "Embedded read only draft");
  assert.ok(embeddedReadonly, JSON.stringify(result));
  result = await call("browser_fill_form", { tab, fields: [
    { target: parent.target, value: "MUST_NOT_CHANGE_PARENT" },
    { target: embeddedReadonly.target, value: "MUST_NOT_CHANGE_EMBED" }
  ] });
  assert.equal(await rawPage("document.getElementById('h6-parent').value"), "");
  assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.effect, "none");
  check("later embedded readonly field refuses before editing an allowed parent document");
  await policy();
  result = await call("browser_fill_form", { tab, fields: [{ target: parent.target, value: "unrelated parent" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage?.excluded_documents ?? 0, 0); check("unrelated parent target remains usable");
  await policy();
  result = await call("browser_fill_form", { tab, fields: [
    { selector: { name: "Parent note", role: "textbox", exact: true }, value: "Standalone semantic draft" }
  ] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.submitted, false);
  assert.equal(await rawPage("document.getElementById('h6-parent').value"), "Standalone semantic draft");
  result = await call("browser_fill_form", { tab, fields: [{ target: parent.target, value: "Standalone handle draft" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.submitted, false);
  assert.equal(await rawPage("document.getElementById('h6-parent').value"), "Standalone handle draft");
  check("standalone permitted controls support equivalent semantic-selector and handle fills");
  await policy(); await freshFixture();
  result = await call("browser_find", { tab, text: "Project name", scope: "control" });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.matches.length, 0, JSON.stringify(result));
  assert.equal(result.facts.coverage.excluded_documents, 1); check("denied-only match yields scoped empty search");
  result = await call("browser_wait", { tab, condition: "text_absent", value: "Project name", timeout_ms: 100 });
  assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.facts.coverage?.excluded_documents, 1, JSON.stringify(result));
  check("absence across excluded content remains unproven");
  result = await call("browser_read", { tab, max_chars: 500 });
  assert.equal(result.facts.coverage.limited_by_size, true); assert.equal(result.facts.coverage.excluded_documents, 1);
  check("size ceiling and policy exclusion remain separate");
  result = await call("browser_screenshot", { tab, timeout_ms: 30000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.coverage, undefined);
  const view = result.facts.view;
  result = await call("browser_screenshot", { view, x: 300, y: 350, width: 400, height: 250, timeout_ms: 30000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.coverage, undefined);
  check("viewport and magnified captures return complete images");
  await policy("all");
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 100 });
  const oldProject = result.facts.items.find((item) => item.name === "Project name"); assert.ok(oldProject);
  const oldDocument = (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`)).find((frame) => frame.frameId !== 0);
  await rawPage(`${frameElement}.src = 'http://127.0.0.1:${port}/form?new-document'; true`);
  const newDocument = await until(async () => (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${physical}})`))
    .find((frame) => frame.frameId === oldDocument.frameId && frame.documentId !== oldDocument.documentId), "replacement embedded document");
  assert.equal(newDocument.frameId, oldDocument.frameId);
  assert.notEqual(newDocument.documentId, oldDocument.documentId);
  result = await call("browser_fill_form", { tab, fields: [{ target: oldProject.target, value: "STALE" }] });
  assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.effect, "none");
  check("navigation cannot revive a stale embedded target");
  result = await call("browser_record", { action: "start", tab });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.ok(result.facts.frame_count > 0, JSON.stringify(result));
  await rawPage(`${frameElement}.src = 'http://127.0.0.1:${port}/form?recording-boundary'; true`);
  await delay(300);
  result = await call("browser_record", { action: "status" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.stop_reason, "document_boundary", JSON.stringify(result));
  assert.ok(result.facts.frame_count > 0); const frozen = result.facts.frame_count;
  await delay(200);
  result = await call("browser_record", { action: "status" }); assert.equal(result.facts.frame_count, frozen);
  result = await call("browser_record", { action: "save" });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.ok(lastResponse.content.some((item) => item.type === "image"));
  check("recording stops at its admitted document boundary and prior frames remain exportable");
  await policy();
  result = await call("browser_record", { action: "save" });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.ok(!lastResponse.content.some((item) => item.type === "image"));
  check("replay export rechecks its captured embedded sources against current policy");
  await policy("all");
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 100 });
  const field = (name) => result.facts.items.find((item) => item.name === name)?.target;
  const values = [["Project name", "Ghostlight H6 fixture"], ["Contact email", "h6@example.invalid"],
    ["Repository URL", "https://example.invalid/fixture"], ["Maintainer type", "Individual"],
    ["Build system", "GitHub Actions"], ["Notes", "Isolated browser acceptance"]];
  const fields = values.map(([name, value]) => { const target = field(name); assert.ok(target, name); return { target, value }; });
  const submit = field("Submit application"); assert.ok(submit);
  result = await call("browser_fill_form", { tab, fields, submit_target: submit });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_read", { tab, max_chars: 20000 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.match(JSON.stringify(result), /The frame confirmed receipt locally/);
  check("permitted Sylin embedded form fills and submits its local simulation");
  await policy();
  await policy("denied", false);
  result = await call("browser_screenshot", { tab });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.ok(!lastResponse.content.some(item => item.type === "image"));
  await policy();
  result = await call("browser_screenshot", { tab });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.ok(lastResponse.content.some(item => item.type === "image"));
  check("policy toggle restores complete capture without iframe style mutation");
  result = await call("browser_inspect", { tab, scope: "structure", max_items: 100 });
  const heading = result.facts.items.find((item) => item.name === "Apply to the Sylin Foundry"); assert.ok(heading);
  result = await call("browser_screenshot", { tab, target: heading.target });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.coverage, undefined);
  check("target captures use the same binary screenshot permission");
  await rawPage("window.scrollTo(0,0); true");
  result = await call("browser_screenshot", { tab }); const pointView = result.facts.view;
  const point = await rawPage(`(()=>{const box=${frameElement}.getBoundingClientRect();return {x:box.left+60,y:box.top+60};})()`);
  result = await call("browser_click", { view: pointView, x: point.x, y: point.y });
  assert.equal(result.status, "blocked", JSON.stringify(result)); assert.equal(result.effect, "none");
  check("complete screenshot coordinates do not grant action inside an excluded document");
  await rawPage(`${frameElement}.src='about:blank'; true`); await delay(200);
  result = await call("browser_read", { tab });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.coverage.excluded_documents, 0); assert.equal(result.facts.coverage.unavailable_documents, 1);
  result = await call("browser_execute", { tab, script: "document.title='UNAVAILABLE_SCRIPT'" });
  assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.effect, "none");
  check("unavailable embedded documents are separate and bounded scripts refuse");
  await policy("unrestricted");
  result = await call("browser_execute", { tab, script: "document.body.dataset.allOpen='AVAILABLE'; return document.body.dataset.allOpen;" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.value, "AVAILABLE");
  assert.equal(await rawPage("document.body.dataset.allOpen"), "AVAILABLE");
  check("unrestricted scripts remain usable when an opaque embed is present");
  result = await call("browser_record", { action: "discard" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_record", { action: "start", tab });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  await rawPage(`${frameElement}.src = 'http://127.0.0.1:${port}/form?unrestricted-recording'; true`);
  await delay(300);
  result = await call("browser_record", { action: "status" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.state, "recording", JSON.stringify(result));
  assert.equal(result.facts.stop_reason, null);
  result = await call("browser_record", { action: "stop" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_record", { action: "save" });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.ok(lastResponse.content.some((item) => item.type === "image"));
  check("unrestricted recording survives document navigation and exports its retained replay");
  await policy("all");
  result = await call("browser_record", { action: "save" });
  assert.equal(result.status, "failed", JSON.stringify(result));
  assert.equal(result.facts.reason, "document_unavailable");
  assert.equal(result.facts.coverage.unavailable_documents, 1);
  assert.ok(!lastResponse.content.some((item) => item.type === "image"));
  check("incomplete recorded source evidence cannot later be exported under host restrictions");
  await policy(); await freshFixture();
  const beforeBlockedRecording = rig.bridge.sent.length;
  result = await call("browser_record", { action: "start", tab });
  assert.equal(result.status, "blocked", JSON.stringify(result));
  assert.equal(result.effect, "none");
  assert.ok(!dispatchedSince(beforeBlockedRecording).includes("recording"));
  assert.ok(!lastResponse.content.some((item) => item.type === "image"));
  check("recording cannot start across an excluded document boundary");
  await newSession();
  const editorPage = await call("browser_navigate", { url: `http://localhost:${port}/editor`, new_tab: true });
  assert.equal(editorPage.status, "succeeded", JSON.stringify(editorPage)); tab = editorPage.facts.tab;
  physical = await rawWorker(`(await chrome.tabs.query({})).find(tab => tab.url === 'http://localhost:${port}/editor').id`);
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 20 });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  const replyTarget = result.facts.items.find((item) => item.name === "Reply")?.target;
  const shadowTarget = result.facts.items.find((item) => item.name === "Shadow reply")?.target;
  assert.ok(replyTarget); assert.ok(shadowTarget);
  assert.ok(result.facts.items.find((item) => item.name === "Hidden draft helper")?.state.includes("hidden"));
  assert.ok(!(result.facts.items.find((item) => item.name === "Reply").state ?? []).includes("hidden"));
  // Prove the editor fixture rejects a DOM assignment plus a synthetic input event.
  await rawPage("document.querySelector('#reply').textContent='UNACCEPTED'; document.querySelector('#reply').dispatchEvent(new Event('input',{bubbles:true})); true");
  await delay(50);
  assert.equal((await rawPage("editorEvidence()")).reply.rendered, "Initial draft");
  for (const [target, name, value] of [[replyTarget, "reply", "Draft line one\nDraft line two"],
    [shadowTarget, "shadow", "Shadow draft\nSecond line"], [replyTarget, "reply", "Replacement draft"],
    [replyTarget, "reply", ""], [replyTarget, "reply", ""], [shadowTarget, "shadow", ""]]) {
    const before = await rawPage("editorEvidence()");
    result = await call("browser_fill_form", { tab, fields: [{ target, value }] });
    await delay(50);
    const after = await rawPage("editorEvidence()");
    assert.equal(result.status, "succeeded", JSON.stringify({ result, name, value, before, after }));
    assert.equal(result.facts.filled_count, 1); assert.equal(result.facts.submitted, false);
    assert.equal(after[name].value, value);
    assert.equal(after[name].rendered, value);
    assert.equal(after[name].synthetic, before[name].synthetic);
    assert.equal(after[name === "reply" ? "shadow" : "reply"].rendered, before[name === "reply" ? "shadow" : "reply"].rendered);
    assert.equal(after.submissions, 0);
  }
  check("controlled rich editors retain native multiline fills and clears without synthetic events or submission");
  const beforeSemanticShadow = await rawPage("editorEvidence()");
  result = await call("browser_fill_form", { tab, fields: [
    { selector: { name: "Shadow reply", role: "textbox", exact: true }, value: "Standalone semantic shadow draft" }
  ] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.submitted, false);
  const afterSemanticShadow = await rawPage("editorEvidence()");
  assert.equal(afterSemanticShadow.shadow.value, "Standalone semantic shadow draft");
  assert.equal(afterSemanticShadow.shadow.rendered, afterSemanticShadow.shadow.value);
  assert.equal(afterSemanticShadow.reply.rendered, beforeSemanticShadow.reply.rendered);
  assert.equal(afterSemanticShadow.submissions, 0);
  result = await call("browser_fill_form", { tab, fields: [{ target: shadowTarget, value: "Standalone handle shadow draft" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.submitted, false);
  const afterHandleShadow = await rawPage("editorEvidence()");
  assert.equal(afterHandleShadow.shadow.value, "Standalone handle shadow draft");
  assert.equal(afterHandleShadow.shadow.rendered, afterHandleShadow.shadow.value);
  assert.equal(afterHandleShadow.submissions, 0);
  check("open-shadow rich editors outside forms support equivalent semantic-selector and handle fills");
  await rawPage("(()=>{const input=document.createElement('input');input.type='file';input.id='standalone-upload';input.setAttribute('aria-label','Standalone upload');document.body.append(input);return true;})()");
  const uploadPath = join(rig.scratch, "standalone-upload.txt");
  const uploadedFile = () => rawPage("(async()=>{const files=document.getElementById('standalone-upload').files;return {count:files.length,name:files[0]?.name,text:await files[0]?.text()};})()");
  writeFileSync(uploadPath, "SYNTHETIC_UPLOAD_BY_SELECTOR");
  result = await call("browser_upload", { tab,
    selector: { name: "Standalone upload", exact: true }, paths: [uploadPath] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.uploaded_count, 1);
  assert.deepEqual(await uploadedFile(), { count: 1, name: "standalone-upload.txt", text: "SYNTHETIC_UPLOAD_BY_SELECTOR" });
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 30 });
  const uploadTarget = result.facts.items.find(item => item.name === "Standalone upload")?.target;
  assert.ok(uploadTarget, JSON.stringify(result));
  writeFileSync(uploadPath, "SYNTHETIC_UPLOAD_BY_HANDLE");
  result = await call("browser_upload", { tab, target: uploadTarget, paths: [uploadPath] });
  assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.uploaded_count, 1);
  assert.deepEqual(await uploadedFile(), { count: 1, name: "standalone-upload.txt", text: "SYNTHETIC_UPLOAD_BY_HANDLE" });
  assert.equal((await rawPage("editorEvidence()")).submissions, 0);
  check("standalone file inputs support equivalent selector and handle attachment with verified file contents");
  result = await call("browser_fill_form", { tab, fields: [{ target: replyTarget, value: "Old typed draft" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_type_text", { tab, target: replyTarget, text: "Typed replacement", clear_first: true });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  await delay(50);
  assert.equal((await rawPage("editorEvidence()")).reply.rendered, "Typed replacement");
  result = await call("browser_fill_form", { tab, fields: [{ target: shadowTarget, value: "Focused draft" }] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_click", { tab, target: shadowTarget });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  result = await call("browser_type_text", { tab, focused: true, text: "", clear_first: true });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  await delay(50);
  const clearedEditor = await rawPage("editorEvidence()");
  assert.equal(clearedEditor.shadow.rendered, "");
  assert.equal(clearedEditor.reply.rendered, "Typed replacement");
  assert.equal(clearedEditor.shadow.synthetic, 0); assert.equal(clearedEditor.submissions, 0);
  check("targeted typing replacement and focused shadow-editor clearing preserve native editor state");
  for (const [target, name] of [[replyTarget, "reply"], [shadowTarget, "shadow"]]) {
    for (const focused of [false, true]) {
      result = await call("browser_fill_form", { tab, fields: [{ target, value: "Existing draft" }] });
      assert.equal(result.status, "succeeded", JSON.stringify(result));
    const before = await rawPage("editorEvidence()");
    const replacement = `${name} ${focused ? "focused" : "targeted"} action draft`;
    if (focused) {
      result = await call("browser_click", { tab, target });
      assert.equal(result.status, "succeeded", JSON.stringify(result));
    }
    result = await call("browser_type_text", { tab, ...(focused ? { focused: true } : { target }),
        text: replacement, clear_first: true, });
      assert.equal(result.status, "succeeded", JSON.stringify(result));
      assert.equal(result.effect, "applied");
      await delay(50);
      const after = await rawPage("editorEvidence()");
      assert.equal(after[name].value, replacement);
      assert.equal(after[name].rendered, replacement);
      assert.equal(after[name].synthetic, before[name].synthetic);
      assert.equal(after[name === "reply" ? "shadow" : "reply"].rendered, before[name === "reply" ? "shadow" : "reply"].rendered);
      assert.equal(after.submissions, 0);
      check(`${name}/${focused ? "focused" : "targeted"}: Action-only typing retains its draft and succeeds`);
    }
  }
  result = await call("browser_inspect", { tab, scope: "controls", max_items: 20 });
  const editorTargets = new Map(result.facts.items.map((item) => [item.name, item.target]));
  await rawPage(`(() => {
    const element = document.querySelector("#ordinary");
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    setter.call(element, "DOM-only replacement");
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    window.renderOrdinaryModel();
    return true;
  })()`);
  const rejectedOrdinary = await rawPage("editorEvidence()");
  assert.equal(rejectedOrdinary.ordinary, "Original input");
  assert.equal(rejectedOrdinary.ordinaryModel.synthetic, 1);
  assert.equal(rejectedOrdinary.ordinaryModel.native, 0);
  check("generic setter events do not prove an ordinary controlled form retained its draft");
  result = await call("browser_fill_form", { tab, fields: [
    { target: editorTargets.get("Ordinary draft"), value: "Ordinary replacement" },
    { target: editorTargets.get("Multiline draft"), value: "Ordinary first line\nOrdinary second line" }
  ] });
  assert.equal(result.status, "succeeded", JSON.stringify(result));
  assert.equal(result.facts.filled_count, 2); assert.equal(result.facts.submitted, false);
  await rawPage("window.renderOrdinaryModel(); true");
  const ordinaryFilled = await rawPage("editorEvidence()");
  assert.equal(ordinaryFilled.ordinary, "Ordinary replacement");
  assert.equal(ordinaryFilled.multiline, "Ordinary first line\nOrdinary second line");
  assert.equal(ordinaryFilled.ordinaryModel.value, "Ordinary replacement");
  assert.equal(ordinaryFilled.multilineModel.value, "Ordinary first line\nOrdinary second line");
  assert.ok(ordinaryFilled.ordinaryModel.native > 0);
  assert.ok(ordinaryFilled.multilineModel.native > 0);
  assert.equal(ordinaryFilled.ordinaryModel.synthetic, 1);
  assert.equal(ordinaryFilled.multilineModel.synthetic, 0);
  assert.equal(ordinaryFilled.submissions, 0);
  check("Read+Write fills ordinary input and textarea without submitting");
  for (const name of ["Read only draft", "Disabled draft", "Hidden draft helper"]) {
    const before = await rawPage("editorEvidence()");
    result = await call("browser_fill_form", { tab, fields: [
      { target: replyTarget, value: "MUST_NOT_CHANGE_EARLIER_EDITOR" },
      { target: editorTargets.get(name), value: "MUST_NOT_CHANGE_PROTECTED_FIELD" }
    ] });
    assert.deepEqual(await rawPage("editorEvidence()"), before);
    assert.equal(result.status, "failed", JSON.stringify(result));
    assert.equal(result.effect, "none"); assert.equal(result.repeat_safe, true);
    check(`${name}: batch refusal preserves every existing draft without claiming success`);
  }
  for (const invalid of ["option", "submit"]) {
    const before = await rawPage("editorEvidence()");
    result = await call("browser_fill_form", { tab, fields: [
      { target: replyTarget, value: "MUST_NOT_CHANGE_BEFORE_VALIDATION" },
      ...(invalid === "option" ? [{ target: editorTargets.get("Draft category"), value: "Missing category" }] : [])
    ], ...(invalid === "submit" ? { submit_target: editorTargets.get("Other form send") } : {}) });
    assert.deepEqual(await rawPage("editorEvidence()"), before);
    assert.equal(result.status, "failed", JSON.stringify(result)); assert.equal(result.effect, "none");
    check(`invalid ${invalid}: form preparation refuses before the first edit or submission`);
  }
  // Page code can invalidate a later field after preparation. Preserve the first edit and
  // uncertainty in that case; preflight is not a transaction or permission to replay a batch.
  const beforeChangedField = await rawPage("editorEvidence()");
  assert.notEqual(beforeChangedField.multiline, "MUST_NOT_CHANGE_LATER_FIELD");
  await rawPage("document.querySelector('#ordinary').addEventListener('input',()=>{document.querySelector('#multiline').readOnly=true;},{once:true}); true");
  result = await call("browser_fill_form", { tab, fields: [
    { target: editorTargets.get("Ordinary draft"), value: "PARTIAL_DRAFT_EFFECT" },
    { target: editorTargets.get("Multiline draft"), value: "MUST_NOT_CHANGE_LATER_FIELD" }
  ] });
  const afterChangedField = await rawPage("editorEvidence()");
  assert.equal(afterChangedField.ordinary, "PARTIAL_DRAFT_EFFECT");
  assert.equal(afterChangedField.multiline, beforeChangedField.multiline);
  assert.equal(afterChangedField.submissions, 0);
  assert.equal(result.status, "unknown", JSON.stringify({ result, beforeChangedField, afterChangedField }));
  assert.equal(result.effect, "unknown"); assert.equal(result.repeat_safe, false);
  await rawPage("document.querySelector('#multiline').readOnly=false; true");
  check("a page change after the first edit preserves the partial draft and refuses replay");
  const editorPolicy = readFileSync(home.policyFile, "utf8");
  for (const capabilities of [["read"], ["write"], ["read", "write"]]) {
    const configured = JSON.parse(editorPolicy);
    for (const grant of configured.grants) grant.allowed = capabilities;
    writeFileSync(home.policyFile, JSON.stringify(configured));
    await policyRefreshed();
    const before = await rawPage("editorEvidence()");
    const beforeDispatch = rig.bridge.sent.length;
    result = await call("browser_fill_form", { tab,
      fields: [{ target: replyTarget, value: "Complete allowlist draft" }] });
    if (capabilities.length === 1) {
      assert.equal(result.status, "blocked", JSON.stringify(result)); assert.equal(result.effect, "none");
      assert.equal(result.facts.policy_rule, "capability");
      assert.equal(refusedWork(beforeDispatch).length, 0);
      assert.deepEqual(await rawPage("editorEvidence()"), before);
    } else {
      assert.equal(result.status, "succeeded", JSON.stringify(result)); assert.equal(result.facts.submitted, false);
      await delay(50);
      const after = await rawPage("editorEvidence()");
      assert.equal(after.reply.rendered, "Complete allowlist draft");
      assert.equal(after.reply.value, "Complete allowlist draft"); assert.equal(after.submissions, 0);
    }
    check(`${capabilities.join("+")}: draft fill enforces the configured capability grant`);
  }
  writeFileSync(home.policyFile, editorPolicy);
  await policyRefreshed();
  // Exercise the shipped renderer in its closed shadow tree. Holding an observation or script
  // Exercise the shipped renderer in its closed shadow tree. Holding an observation or script
  // gives us time to inspect the actual live elements while another controlled tab is active.
  await policy("all"); await newSession();
  const openVisualTab = async name => {
    const url = `http://localhost:${port}/editor#${name}`;
    const opened = await call("browser_navigate", { url, new_tab: true });
    assert.equal(opened.status, "succeeded", JSON.stringify(opened));
    const physical = await rig.physicalTab(url);
    return { handle: opened.facts.tab, physical, page: await rig.page(physical) };
  };
  const targetVisual = await openVisualTab("visual-target");
  const unrelatedVisual = await openVisualTab("visual-unrelated");
  const firstVisualPeer = mcp;
  await rawWorker(`chrome.tabs.update(${unrelatedVisual.physical}, {active:true})`);
  const visualState = async target => {
    const { root } = await target.page.send("DOM.getDocument", { depth: -1, pierce: true });
    const counts = { wheel: 0, read: 0, signature: 0 };
    for (const node of descendants(root)) {
      const classes = String(attributesOf(node).get("class") ?? "").split(/\s+/);
      if (classes.includes("workwheel")) counts.wheel++;
      if (classes.includes("read-scan")) counts.read++;
      if (classes.includes("signature")) counts.signature++;
    }
    return counts;
  };
  const visualPage = (target, expression) => target.page.evaluate(expression);
  const captureVisual = async (target, name) => {
    // The product screenshot path intentionally hides presentation. This test observes the
    // actual Chrome surface directly, retaining the wheel for human comparison with cleanup.
    const { data } = await target.page.send("Page.captureScreenshot", { format: "png" });
    writeFileSync(join(scratchRoot, `h6-visual-${name}.png`), Buffer.from(data, "base64"));
  };
  // The worker holds the page call that reads one tab's text until the journey releases it. The
  // held work shows the presentation of work in flight.
  await rawWorker(`(() => {
    globalThis.visualReadTarget = ${targetVisual.physical};
    globalThis.__journeyBeforeCommand = async function (target, method, params) {
      const message = params?.arguments?.[1]?.value;
      if (method === 'Runtime.callFunctionOn' && params.functionDeclaration.includes('rt.call(') && message?.kind === 'read_text'
        && target.tabId === visualReadTarget) {
        await new Promise(resolve => { globalThis.visualReleaseRead = resolve; });
      }
    };
    return true;
  })()`);
  const visualRequests = [];
  const beginVisual = (peer, name, args) => {
    const request = peer.begin("tools/call", { name, arguments: args });
    request.promise.catch(() => {}); // The main assertion or cleanup awaits this same promise.
    visualRequests.push({ peer, ...request });
    return request;
  };
  const resultOf = async request => (await request.promise).result.structuredContent;
  try {
    const noticesBeforeRead = rig.bridge.notices.length;
    const reading = beginVisual(firstVisualPeer, "browser_read", { tab: targetVisual.handle });
    await until(async () => (await visualState(targetVisual)).read > 0, "read scan on its background target", 5000);
    assert.equal((await visualState(unrelatedVisual)).read, 0);
    await until(() => rawWorker("typeof globalThis.visualReleaseRead==='function'"), "blocked visual read", 5000);
    await rawWorker("(visualReadTarget=null,visualReleaseRead(),true)");
    assert.equal((await resultOf(reading)).status, "succeeded");
    const readDeliveries = rig.bridge.notices.slice(noticesBeforeRead)
      .filter(item => item.signal.activity === "read" && item.signal.signal === "start").map(item => item.tab_id);
    assert.ok(readDeliveries.length > 0); assert.ok(readDeliveries.every(id => id === targetVisual.physical));
    check("read animation renders only on the requested background tab");

    const heldScript = (peer, target, finish) => beginVisual(peer, "browser_execute", {
      tab: target.handle, timeout_ms: 15000,
      script: `await new Promise(resolve=>{globalThis.__visualRelease=resolve;}); ${finish}`
    });
    const firstScript = heldScript(firstVisualPeer, targetVisual, "return 'visual success';");
    await until(async () => (await visualState(targetVisual)).wheel === 1, "script wheel on its background target", 5000);
    assert.equal((await visualState(unrelatedVisual)).wheel, 0);
    await until(() => visualPage(targetVisual, "typeof globalThis.__visualRelease==='function'"), "first held script");
    await captureVisual(targetVisual, "script-running");
    await newSession();
    const secondVisual = await openVisualTab("visual-overlap");
    const secondScript = heldScript(mcp, secondVisual, "throw new Error('visual failure');");
    await until(async () => (await visualState(secondVisual)).wheel === 1, "independent overlapping script wheel", 5000);
    await until(() => visualPage(secondVisual, "typeof globalThis.__visualRelease==='function'"), "second held script");
    assert.equal((await visualState(targetVisual)).wheel, 1);
    await visualPage(targetVisual, "(__visualRelease(),true)");
    assert.equal((await resultOf(firstScript)).status, "succeeded");
    await until(async () => (await visualState(targetVisual)).wheel === 0, "successful script wheel cleanup", 3000);
    await captureVisual(targetVisual, "script-complete");
    assert.equal((await visualState(secondVisual)).wheel, 1);
    assert.equal((await visualState(unrelatedVisual)).wheel, 0);
    check("overlapping script wheels stay on their own tabs and success cleans only its invocation");
    await visualPage(secondVisual, "(__visualRelease(),true)");
    const failedScript = await resultOf(secondScript);
    assert.equal(failedScript.status, "unknown"); assert.equal(failedScript.repeat_safe, false);
    await until(async () => (await visualState(secondVisual)).wheel === 0, "failed script wheel cleanup", 3000);
    check("runtime script failure clears the actual work wheel");

    await rawWorker(`chrome.tabs.update(${unrelatedVisual.physical},{active:true})`);
    await visualPage(targetVisual, "(delete globalThis.__visualRelease,true)");
    const cancelledScript = heldScript(firstVisualPeer, targetVisual, "return 'cancelled script completed physically';");
    await until(async () => (await visualState(targetVisual)).wheel === 1, "cancelled script starts its wheel", 5000);
    await until(() => visualPage(targetVisual, "typeof globalThis.__visualRelease==='function'"), "script ready to cancel");
    firstVisualPeer.notify("notifications/cancelled", { requestId: cancelledScript.id, reason: "visual acceptance cancellation" });
    const cancelled = await resultOf(cancelledScript);
    assert.equal(cancelled.status, "unknown"); assert.equal(cancelled.repeat_safe, false);
    await until(async () => (await visualState(targetVisual)).wheel === 0, "cancelled script wheel cleanup", 3000);
    assert.equal((await visualState(unrelatedVisual)).wheel, 0);
    await visualPage(targetVisual, "(__visualRelease(),true)");
    check("cancellation clears its work wheel without leaving a spinner on another tab");

    // Hold document discovery after the real targeted start. Its denied child is known before
    // any script dispatch; the hidden-tab notice may wait, but its live wheel must stop now.
    await visualPage(targetVisual, `(()=>{const frame=document.createElement('iframe');frame.src='http://127.0.0.1:${port}/form?visual-denial';document.body.append(frame);return true;})()`);
    await until(async () => (await rawWorker(`chrome.webNavigation.getAllFrames({tabId:${targetVisual.physical}})`))
      .some(frame => frame.url.includes("visual-denial")), "denied visual fixture document");
    await policy();
    await rawWorker(`(() => {
      globalThis.visualOriginalFrames=chrome.webNavigation.getAllFrames.bind(chrome.webNavigation);
      globalThis.visualInventoryTarget=${targetVisual.physical};
      chrome.webNavigation.getAllFrames=async function(details) {
        if(details.tabId===visualInventoryTarget) await new Promise(resolve=>{globalThis.visualReleaseInventory=resolve;});
        return visualOriginalFrames(details);
      }; return true;
    })()`);
    const beforeDeniedVisual = rig.bridge.sent.length;
    const deniedVisual = beginVisual(firstVisualPeer, "browser_execute", {
      tab: targetVisual.handle, script: "document.title='MUST_NOT_EXECUTE_VISUAL_DENIED';"
    });
    await until(async () => (await visualState(targetVisual)).wheel === 1, "script wheel before document admission", 5000);
    assert.equal((await visualState(unrelatedVisual)).wheel, 0);
    await until(() => rawWorker("typeof visualReleaseInventory==='function'"), "held document discovery");
    await rawWorker("(visualInventoryTarget=null,visualReleaseInventory(),true)");
    const deniedResult = await resultOf(deniedVisual);
    assert.equal(deniedResult.status, "blocked", JSON.stringify(deniedResult)); assert.equal(deniedResult.effect, "none");
    assert.ok(!dispatchedSince(beforeDeniedVisual).includes("evaluate_main"));
    await until(async () => (await visualState(targetVisual)).wheel === 0, "hidden denied script wheel cleanup", 3000);
    const queuedDenials = await rawWorker("(await chrome.storage.session.get('ghostlight.presentations'))['ghostlight.presentations'] || []");
    assert.ok(queuedDenials.some(item => item.tabId === targetVisual.physical && item.signal.invocation === deniedResult.invocation && item.signal.signal === "denial"));
    assert.equal((await visualState(unrelatedVisual)).wheel, 0);
    check("hidden denied script clears its wheel while preserving its queued human notice");
  } finally {
    await rawWorker("(()=>{globalThis.__journeyBeforeCommand=undefined;globalThis.visualReleaseRead?.();if(globalThis.visualOriginalFrames)chrome.webNavigation.getAllFrames=visualOriginalFrames;globalThis.visualReleaseInventory?.();return true;})()");
    for (const request of visualRequests) request.peer.notify("notifications/cancelled", { requestId: request.id, reason: "visual fixture cleanup" });
    await Promise.allSettled(visualRequests.map(request => request.promise));
  }
  assert.deepEqual(assetFailures, [], "every site file the pages load was served");
  const audit = readFileSync(home.auditFile, "utf8");
  assert.doesNotMatch(audit, /127\.0\.0\.1|h6@example\.invalid|Ghostlight H6 fixture|ifxf-project|Draft line one|Shadow draft|Replacement draft|action draft|Complete allowlist draft|Ordinary replacement|MUST_NOT_CHANGE_|PARTIAL_DRAFT_EFFECT|Standalone semantic|Standalone handle|standalone-upload\.txt|SYNTHETIC_UPLOAD_/);
  check("durable audit excludes embedded origins, values, and selectors");
  writeFileSync(join(scratchRoot, "h6-browser-evidence.json"), JSON.stringify({ browser: version.product,
    source_kind: liveSylin ? "live_html" : "checked_in_snapshot", source: sourceEvidence, passed,
    transport: "MV3 native-port bridge -> real browser edge -> service -> real MCP edge" }, null, 2));
  console.log(`H6 browser journey: ${passed.length} checks passed with ${version.product}.`);
} finally {
  await rig?.stop();
  server.closeAllConnections(); await new Promise((done) => server.close(done));
  await home.cleanup();
}
