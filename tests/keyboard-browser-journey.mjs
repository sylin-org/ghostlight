// Keyboard, credential-input and passive-presentation behavior of the shipped extension and page
// code in real Chromium. Every action goes through the real MCP edge, the service of a test home,
// the browser edge and the extension (tests/lib/browser.mjs replaces only native-port discovery).
// The page is read through the journey's own DevTools session. No installed browser profile,
// native-host registration or extension is used.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { randomUUID } from "node:crypto";
import { createServer } from "node:http";
import { BrowserRig } from "./lib/browser.mjs";
import { createHome } from "./lib/home.mjs";
import { policyRefreshed } from "./lib/policy-refresh.mjs";
import { structured } from "./lib/peers.mjs";

const FIXTURE_HOST = "127.0.0.1";
const ALL = ["read", "action", "write", "execute"];
const PRESENTATION_ROOT = "ghostlight-presentation-root";
const SECRET = "FIXTURE_CREDENTIAL_DO_NOT_RETAIN";
const CONTROL_ELEMENTS = new Set(["BUTTON", "A", "INPUT", "TEXTAREA", "SELECT"]);
const CONTROL_ATTRIBUTES = ["tabindex", "contenteditable", "href"];

const FIXTURE = `<!doctype html><title>Keyboard fixture</title>
<form id="f"><label>Draft<textarea id="draft"></textarea></label><label>Next<input id="next"></label><button>Submit</button></form>
<label>Password<input id="credential" type="password"></label><input id="after" aria-label="After">
<script>
  window.submits = 0; window.inputs = []; window.model = "";
  document.getElementById("f").addEventListener("submit", (event) => { event.preventDefault(); window.submits++; });
  draft.addEventListener("input", (event) => { window.model = draft.value; window.inputs.push({ trusted: event.isTrusted, type: event.inputType }); });
  credential.addEventListener("input", (event) => window.inputs.push({ trusted: event.isTrusted, type: event.inputType }));
</script>`;

const policy = (allowed) => ({
  schema: 4, name: "Keyboard journey", version: randomUUID(),
  grants: [{ id: "fixture", hosts: { allow: [FIXTURE_HOST] }, allowed }],
  config: [{ key: "browser.startup", value: "manual", level: "mandatory" }]
});

const server = createServer((_request, response) => {
  response.writeHead(200, { "content-type": "text/html" });
  response.end(FIXTURE);
});
const home = createHome({ label: "keyboard", policy: policy(ALL) });
let rig;

function attributesOf(node) {
  const pairs = new Map();
  for (let index = 0; index + 1 < (node.attributes?.length ?? 0); index += 2) pairs.set(node.attributes[index], node.attributes[index + 1]);
  return pairs;
}

function descendants(node) {
  return [...(node.children ?? []), ...(node.shadowRoots ?? []), ...(node.contentDocument ? [node.contentDocument] : [])]
    .flatMap((child) => [child, ...descendants(child)]);
}

// The presentation host and its closed shadow root, read through DevTools, which sees closed roots.
async function presentation(page) {
  await page.send("DOM.enable");
  const { root } = await page.send("DOM.getDocument", { depth: -1, pierce: true });
  const host = descendants(root).find((node) => attributesOf(node).get("id") === PRESENTATION_ROOT);
  return host ? { host, shadow: host.shadowRoots?.[0] ?? null } : { host: null, shadow: null };
}

const ribbonsOf = (shadow) => descendants(shadow ?? {}).filter((node) => attributesOf(node).get("class") === "denial-ribbon");

// Polls until a ribbon whose backend node id is not in `seen` is present. A denial replaces its
// ribbon and a later one removes it, so the journey waits for the render it asked for and never
// accepts an earlier one.
async function freshRibbons(page, seen) {
  for (let tries = 0; tries < 80; tries++) {
    const current = await presentation(page);
    const ribbons = ribbonsOf(current.shadow);
    if (ribbons.some((node) => !seen.has(node.backendNodeId))) return { ...current, ribbons };
    await new Promise((done) => setTimeout(done, 50));
  }
  return { host: null, shadow: null, ribbons: [] };
}

// Computed pointer-events of the host and every element in its shadow root, with each element's
// class, read in one round trip so no step holds a node id across a re-render. The read is null
// when the denial ribbon is no longer in the root, so a caller never mistakes an expired denial
// for an empty one.
async function pointerEventsInside(page, shadow) {
  const { object } = await page.send("DOM.resolveNode", { backendNodeId: shadow.backendNodeId });
  const { result } = await page.send("Runtime.callFunctionOn", {
    objectId: object.objectId,
    functionDeclaration: "function() { if (!this.querySelector('.denial-ribbon')) return null; return [this.host, ...this.querySelectorAll('*')].map((node) => [node.nodeName, getComputedStyle(node).pointerEvents, node.className]); }",
    returnByValue: true
  });
  return result.value;
}

try {
  await new Promise((done) => server.listen(0, FIXTURE_HOST, done));
  const url = `http://${FIXTURE_HOST}:${server.address().port}/`;
  rig = await BrowserRig.start({ home, label: "keyboard-browser" });
  const mcp = await rig.mcp("keyboard journey");
  let lastResponse;
  const call = async (name, args) => {
    lastResponse = await mcp.call(name, args);
    return structured(lastResponse);
  };
  const setPolicy = async (allowed) => {
    writeFileSync(home.policyFile, JSON.stringify(policy(allowed)));
    await policyRefreshed();
  };
  const opened = await call("browser_navigate", { url, new_tab: true });
  assert.equal(opened.status, "succeeded", JSON.stringify(opened));
  const tab = opened.facts.tab;
  const physical = await rig.physicalTab(url);
  const page = await rig.page(physical);
  const inspected = await call("browser_inspect", { tab, scope: "controls", max_items: 20 });
  assert.equal(inspected.status, "succeeded", JSON.stringify(inspected));
  const targetOf = (name) => inspected.facts.items.find((item) => item.name === name)?.target;
  const draft = targetOf("Draft");
  const credential = inspected.facts.items.find((item) => item.name === "Password");
  assert.ok(draft && credential?.target, JSON.stringify(inspected));
  assert.equal(credential.credential_class, true, "a real password input is classified by production observation");

  const cases = ["First job\nSecond job", "\nFirst\n\nLast\n", "First\r\nSecond\r\n", "First\rSecond\r", "Single line", ""];
  for (const value of cases) {
    await page.evaluate("draft.value = 'old draft'; draft.focus(); window.inputs = []; window.model = 'old draft'; true");
    const result = await call("browser_fill_form", { tab, fields: [{ target: draft, value }] });
    assert.equal(result.status, "succeeded", `${JSON.stringify(value)}: ${JSON.stringify(result)}`);
    assert.equal(result.facts.submitted, false);
    const observed = await page.evaluate("({ value: draft.value, model: window.model, inputs: window.inputs, submits: window.submits })");
    const expected = value.replace(/\r\n?/g, "\n");
    assert.equal(observed.value, expected, `DOM value for ${JSON.stringify(value)}`);
    assert.equal(observed.model, expected, "input listener model matches the requested text");
    assert.ok(observed.inputs.length > 0 && observed.inputs.every((event) => event.trusted), "the page sees trusted browser input");
    assert.equal(observed.submits, 0);
  }
  console.log(`PASS: ${cases.length} Chromium text replacements preserve DOM/model values and trusted input with no submission.`);

  const credentialCases = [
    ["fill", (extra) => call("browser_fill_form", { tab, fields: [{ target: credential.target, value: SECRET }], ...extra })],
    ["targeted typing", (extra) => call("browser_type_text", { tab, target: credential.target, text: SECRET, clear_first: true, ...extra })],
    ["focused typing", (extra) => call("browser_type_text", { tab, focused: true, text: SECRET, clear_first: true, ...extra })]
  ];
  const authorized = { user_authorized_credentials: true };
  const responses = [];
  const refused = (name, result) => {
    assert.notEqual(result.status, "succeeded", `${name} refuses without the explicit allowance: ${JSON.stringify(result)}`);
    assert.equal(result.effect, "none", name);
    assert.equal(JSON.stringify(lastResponse).includes(SECRET), false, `${name}: the refusal contains no input value`);
  };
  for (const [name, invoke] of credentialCases) {
    await page.evaluate("credential.value = 'existing'; credential.focus(); window.inputs = []; true");
    refused(name, await invoke({}));
    assert.equal(await page.evaluate("credential.value === 'existing' && inputs.length === 0"), true,
      `${name} refusal preserves the existing field without input`);
    const result = await invoke(authorized);
    assert.equal(result.status, "succeeded", `${name}: ${JSON.stringify(result)}`);
    responses.push(lastResponse);
    const observed = await page.evaluate(`({ retained: credential.value === ${JSON.stringify(SECRET)}, inputs: window.inputs, submits: window.submits })`);
    assert.equal(observed.retained, true, `${name} retains the explicitly authorized credential`);
    assert.ok(observed.inputs.length > 0, name);
    assert.ok(observed.inputs.every((event) => event.trusted), `${name} uses browser editing input`);
    assert.equal(observed.submits, 0, `${name} never implicitly submits`);
    await page.evaluate("credential.focus(); window.inputs = []; true");
    refused(name, await invoke({}));
    assert.equal(await page.evaluate(`credential.value === ${JSON.stringify(SECRET)} && inputs.length === 0`), true,
      `${name}: the allowance does not survive the request`);
  }
  assert.equal(JSON.stringify(responses).includes(SECRET), false, "results contain no credential value");
  assert.equal(readFileSync(home.auditFile, "utf8").includes(SECRET), false, "the audit contains no credential value");
  console.log(`PASS: ${credentialCases.length} credential input paths honor the per-request allowance, retain real password input and keep the value out of results and audit.`);

  // Denial feedback is passive: no controls, no pointer events and no focus taken, even when the
  // stylesheet is missing. The first denial mounts the presentation; its stylesheet is then removed
  // through DevTools and a second denial renders without it.
  await setPolicy(["read"]);
  const denialsBefore = rig.bridge.notices.length;
  const denied = await call("browser_click", { tab, target: targetOf("After") ?? credential.target });
  assert.equal(denied.status, "blocked", JSON.stringify(denied));
  const mounted = rig.bridge.notices.slice(denialsBefore);
  assert.ok(mounted.some((notice) => notice.tab_id === physical && notice.signal.signal === "denial"), "the service asked the tab to show a denial");
  let shadow = null;
  for (let tries = 0; tries < 80 && !shadow; tries++) {
    shadow = (await presentation(page)).shadow;
    if (!shadow) await new Promise((done) => setTimeout(done, 50));
  }
  assert.ok(shadow, "the presentation mounted a shadow root");
  const firstRibbons = new Set();
  for (let tries = 0; tries < 80 && firstRibbons.size === 0; tries++) {
    for (const node of ribbonsOf((await presentation(page)).shadow)) firstRibbons.add(node.backendNodeId);
    if (firstRibbons.size === 0) await new Promise((done) => setTimeout(done, 50));
  }
  assert.ok(firstRibbons.size > 0, "the first denial rendered");
  const { root: fresh } = await page.send("DOM.getDocument", { depth: -1, pierce: true });
  const freshHost = descendants(fresh).find((node) => attributesOf(node).get("id") === PRESENTATION_ROOT);
  const style = (freshHost?.shadowRoots?.[0]?.children ?? []).find((node) => node.nodeName === "STYLE");
  assert.ok(style, "the presentation carries a stylesheet");
  await page.send("DOM.removeNode", { nodeId: style.nodeId });
  await page.evaluate("credential.focus(); true");
  const second = await call("browser_click", { tab, target: credential.target });
  assert.equal(second.status, "blocked", JSON.stringify(second));
  // The denial lasts a few seconds, so the snapshot and the read happen right away.
  const inspectedRoot = await freshRibbons(page, firstRibbons);
  assert.ok(inspectedRoot.ribbons.length > 0, "the second denial rendered");
  const pointerEvents = await pointerEventsInside(page, inspectedRoot.shadow);
  assert.ok(pointerEvents, "the second denial was still showing when pointer input was read");
  const inside = descendants(inspectedRoot.shadow);
  const elements = inside.filter((node) => node.nodeType === 1);
  const controls = elements.filter((node) => CONTROL_ELEMENTS.has(node.nodeName)
    || CONTROL_ATTRIBUTES.some((name) => attributesOf(node).has(name)) || attributesOf(node).get("role") === "button");
  assert.equal(inside.some((node) => node.nodeName === "STYLE"), false, "the stylesheet stayed missing");
  assert.equal(controls.length, 0, "denial feedback has no controls");
  assert.ok(pointerEvents.some(([, , cls]) => cls === "denial-ribbon"), "the live read included the second denial");
  for (const [name, value] of pointerEvents) assert.equal(value, "none", `${name} takes no pointer input`);
  assert.ok(inside.filter((node) => node.nodeType === 3).some((node) => node.nodeValue.trim() !== ""), "the denial says why");
  assert.equal(await page.evaluate("document.activeElement === credential"), true, "the page keeps its focus");
  assert.equal(await page.evaluate(`(() => {
    const box = credential.getBoundingClientRect();
    return document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2) === credential;
  })()`), true, "the page still receives pointer input");
  console.log("PASS: denial feedback has no controls and preserves page focus and pointer input with its stylesheet absent.");
} finally {
  await rig?.stop();
  await new Promise((done) => { server.closeAllConnections(); server.close(done); });
  await home.cleanup();
}
