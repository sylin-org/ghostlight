// Exercise production keyboard, credential-input, and passive-presentation mechanisms in Chromium.
// This component test starts no Ghostlight service and touches no installed browser profile.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { basename, dirname, join, resolve } from "node:path";
import vm from "node:vm";
import { readDevToolsPort, waitForChromiumExit } from "./lib/chromium.mjs";

const root = resolve(import.meta.dirname, "..");
const scratchRoot = join(root, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "keyboard-browser-"));
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || join(root, ".tmp/chrome-testing/chrome-win64/chrome.exe");
const child = spawn(browser, ["--remote-debugging-port=0",
  `--user-data-dir=${scratch}`, "--no-first-run", "--no-default-browser-check", "about:blank"],
{ windowsHide: true, stdio: "ignore" });
child.on("error", error => { child.startError = error; });
let socket;
let send;
try {
  const [port, endpoint] = await readDevToolsPort(scratch, child);
  socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
  await new Promise((resolveOpen, reject) => {
    socket.addEventListener("open", resolveOpen, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  let next = 0;
  const pending = new Map();
  socket.addEventListener("message", event => {
    const response = JSON.parse(event.data);
    const request = pending.get(response.id);
    if (!request) return;
    pending.delete(response.id);
    clearTimeout(request.timer);
    if (response.error) request.reject(new Error(JSON.stringify(response.error)));
    else request.resolve(response.result);
  });
  send = (method, params = {}, sessionId) => new Promise((resolveCall, reject) => {
    const id = ++next;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timeout: ${method}`)); }, 10000);
    pending.set(id, { resolve: resolveCall, reject, timer });
    socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
  });
  const { targetInfos } = await send("Target.getTargets");
  const target = targetInfos.find(info => info.type === "page");
  const { sessionId } = await send("Target.attachToTarget", { targetId: target.targetId, flatten: true });
  const evaluate = async expression => {
    const response = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, sessionId);
    assert.equal(response.exceptionDetails, undefined);
    return response.result.value;
  };
  await evaluate(`document.body.innerHTML = '<form><textarea id="draft"></textarea><input id="next"><button>Submit</button></form>';
    window.submits = 0; window.inputs = []; window.model = '';
    document.querySelector('form').addEventListener('submit', e => { e.preventDefault(); window.submits++; });
    draft.addEventListener('input', e => { window.model = draft.value; window.inputs.push({trusted:e.isTrusted,type:e.inputType}); });`);
  const require = createRequire(import.meta.url);
  const sandbox = { shared: require("../extension/lib/shared.js"),
    sendDebugger: (_target, method, params) => send(method, params, sessionId) };
  vm.createContext(sandbox);
  const worker = readFileSync(join(root, "extension/service-worker.js"), "utf8");
  const start = worker.indexOf("function requireFillBudget(");
  const end = worker.indexOf("async function fill(", start);
  assert.ok(start >= 0 && end > start);
  vm.runInContext(worker.slice(start, end), sandbox);
  const content = readFileSync(join(root, "crates/orchestrator/src/page_runtime/content.js"), "utf8");
  const expectedStart = content.indexOf("  function expectedFillValue(");
  const expectedEnd = content.indexOf("  function fillValuesRetained(", expectedStart);
  assert.ok(expectedStart >= 0 && expectedEnd > expectedStart);
  await evaluate(content.slice(expectedStart, expectedEnd));
  const cases = ["First job\nSecond job", "\nFirst\n\nLast\n", "First\r\nSecond\r\n", "First\rSecond\r", "Single line", ""];
  for (const value of cases) {
    await evaluate("draft.value = 'old draft'; draft.focus(); window.inputs = []; window.model = 'old draft';");
    await sandbox.replaceFocusedText(1, 0, value);
    const observed = await evaluate("({value:draft.value, model:window.model, inputs:window.inputs, submits:window.submits, focused:document.activeElement.id})");
    const expected = value.replace(/\r\n?/g, "\n");
    assert.equal(observed.value, expected, `DOM value for ${JSON.stringify(value)}`);
    assert.equal(observed.model, expected, "input listener model matches the requested text");
    assert.equal(await evaluate(`expectedFillValue(draft, ${JSON.stringify(value)})`), observed.value,
      "production readback accepts Chromium's normalized textarea value");
    assert.ok(observed.inputs.length > 0 && observed.inputs.every(event => event.trusted));
    assert.equal(observed.submits, 0);
    assert.equal(observed.focused, "next");
  }
  console.log(`PASS: ${cases.length} Chromium text replacements preserve DOM/model values, trusted input, blur, and no submission.`);

  // Load the same source the service bundles. Only Chrome transport is replaced by this
  // disposable page's CDP connection; validators and worker input sequencing are production.
  await evaluate(`document.body.innerHTML = '<form><label>Password<input id="credential" type="password"></label><input id="after"><button>Submit</button></form>';
    window.submits = 0; window.inputs = []; window.diagnosticMessages = [];
    document.querySelector('form').addEventListener('submit', e => { e.preventDefault(); window.submits++; });
    credential.addEventListener('input', e => window.inputs.push({trusted:e.isTrusted,type:e.inputType}));
    globalThis.chrome = { runtime: { sendMessage: async message => {
      if (message.kind === 'form_diagnostics') window.diagnosticMessages.push(message.row);
      return {ok:true,value:{enabled:true}};
    } } };`);
  for (const relative of ["extension/lib/shared.js", "extension/lib/form-diagnostics.js",
    "crates/orchestrator/src/page_runtime/sensor.js", "extension/lib/presentation-css.js",
    "extension/lib/presentation.js", "crates/orchestrator/src/page_runtime/content.js"]) {
    await evaluate(readFileSync(join(root, relative), "utf8"));
  }
  const runtime = async message => {
    const response = await evaluate(`window.__ghostlight_dispatch__(${JSON.stringify(message)})
      .then(result => ({ok:true,result})).catch(error => ({ok:false,error:String(error.message)}))`);
    if (!response.ok) throw new Error(response.error);
    return response.result;
  };
  const inspected = await runtime({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const credential = inspected.targets.find(target => target.name === "Password");
  assert.ok(credential?.credential_class, "real password input is classified by production observation");
  Object.assign(sandbox, {
    navigationWatchers: new Map(), cancelled: new Set(), setTimeout,
    frames: { frameOf: () => 0, localOf: locator => locator,
      groupLocators: locators => new Map([[0, locators]]) },
    contentIn: (_tab, _frame, message) => runtime(message),
    content: (_tab, message) => runtime(message),
    firstFrameAnswer: (_tab, message) => runtime(message),
    documents: { verify: async () => {}, verifyInput: async () => {} },
    ensureDebugger: async () => {}, detachDebugger: async () => {},
    chrome: { tabs: { get: async id => ({ id, url: "about:blank", status: "complete" }) } },
    physicalTab: tab => tab
  });
  const constants = worker.match(/^const FILL_RETAINED_(?:STABLE|LIMIT|POLL)_MS = [\d_]+;$/gm);
  assert.equal(constants?.length, 3);
  vm.runInContext(constants.join("\n"), sandbox);
  const fillStart = worker.indexOf("async function fill(");
  const fillEnd = worker.indexOf("async function dispatchDrag(", fillStart);
  const focusedStart = worker.indexOf("async function typeFocused(");
  const focusedEnd = worker.indexOf("async function inspectDialog(", focusedStart);
  assert.ok(fillStart >= 0 && fillEnd > fillStart && focusedStart >= 0 && focusedEnd > focusedStart);
  vm.runInContext(worker.slice(fillStart, fillEnd), sandbox);
  vm.runInContext(worker.slice(focusedStart, focusedEnd), sandbox);
  const secret = "FIXTURE_CREDENTIAL_DO_NOT_RETAIN";
  const credentialCases = [
    ["fill", allow_credentials => sandbox.fill("credential-fill", {
      tab_id: 1, timeout_ms: 5_000, allow_credentials,
      fields: [{ locator: credential.locator, value: secret }]
    })],
    ["targeted typing", allow_credentials => sandbox.typeText("credential-type", {
      tab_id: 1, locator: credential.locator, text: secret, clear_first: true, allow_credentials
    })],
    ["focused typing", allow_credentials => sandbox.typeFocused("credential-focused", {
      tab_id: 1, text: secret, clear_first: true, allow_credentials
    })]
  ];
  const receipts = [];
  const authorizationRefusal = error => {
    assert.match(error.message, /requires user authorization/);
    assert.equal(error.message.includes(secret), false, "credential refusal contains no input value");
    return true;
  };
  for (const [name, invoke] of credentialCases) {
    await evaluate("credential.value = 'existing'; credential.focus(); window.inputs = [];");
    await assert.rejects(invoke(false), authorizationRefusal, `${name} refuses without the service allowance`);
    assert.equal(await evaluate("credential.value === 'existing' && inputs.length === 0"), true,
      `${name} refusal preserves the existing field without input`);
    receipts.push(await invoke(true));
    const observed = await evaluate(`({retained:credential.value === ${JSON.stringify(secret)}, inputs:window.inputs, submits:window.submits})`);
    assert.equal(observed.retained, true, `${name} retains the explicitly authorized credential`);
    assert.ok(observed.inputs.length > 0, name);
    assert.ok(observed.inputs.every(event => event.trusted), `${name} uses browser editing input`);
    assert.equal(observed.submits, 0, `${name} never implicitly submits`);
    await evaluate("credential.focus(); window.inputs = [];");
    await assert.rejects(invoke(false), authorizationRefusal, `${name} allowance does not survive the request`);
    assert.equal(await evaluate(`credential.value === ${JSON.stringify(secret)} && inputs.length === 0`), true);
  }
  await new Promise(resolveDelay => setTimeout(resolveDelay, 350));
  assert.equal(JSON.stringify(receipts).includes(secret), false, "effect receipts contain no credential value");
  const diagnostics = await evaluate("window.diagnosticMessages");
  assert.ok(diagnostics.some(row => row.event === "trace_started"), "diagnostics were enabled during real credential entry");
  assert.ok(diagnostics.every(row => row.control_count === 1 && row.nonempty_count === 0),
    "the credential control and its changing value stay outside diagnostic measurements");
  assert.equal(JSON.stringify(diagnostics).includes(secret), false, "enabled diagnostics contain no credential value");

  // Capture the real closed shadow root only for inspection. Simulate a missing stylesheet,
  // then prove that legacy attention feedback has no controls and cannot take page input.
  const passive = await evaluate(`(() => {
    const attach = Element.prototype.attachShadow;
    let shadow;
    Element.prototype.attachShadow = function (options) { shadow = attach.call(this, options); return shadow; };
    try { GhostlightPresentation.setManaged(true); }
    finally { Element.prototype.attachShadow = attach; }
    shadow.querySelector('style').remove();
    credential.focus();
    GhostlightPresentation.render({signal:'attention'}, {effects:true,captions:true});
    const box = credential.getBoundingClientRect();
    const host = document.getElementById('ghostlight-presentation-root');
    return {
      controls: shadow.querySelectorAll('button,a[href],input,textarea,select,[tabindex],[contenteditable="true"],[role="button"]').length,
      styleMissing: shadow.querySelector('style') === null,
      pointerPassive: [host, ...shadow.querySelectorAll('*')].every(node => getComputedStyle(node).pointerEvents === 'none'),
      focusRetained: document.activeElement === credential,
      pageReceivesPointer: document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2) === credential,
      guidanceVisible: shadow.textContent.includes('workbench')
    };
  })()`);
  assert.deepEqual(passive, { controls: 0, styleMissing: true, pointerPassive: true,
    focusRetained: true, pageReceivesPointer: true, guidanceVisible: true });
  console.log(`PASS: ${credentialCases.length} production credential input paths honor per-request allowance, retain real password input, and exclude values from receipts/diagnostics.`);
  console.log("PASS: production attention feedback has no controls and preserves page focus/pointer input with its stylesheet absent.");
} finally {
  if (send && socket?.readyState === WebSocket.OPEN) await send("Browser.close").catch(() => {});
  socket?.close();
  if (!await waitForChromiumExit(child)) { child.kill(); await waitForChromiumExit(child); }
  assert.equal(dirname(resolve(scratch)), scratchRoot);
  assert.ok(basename(scratch).startsWith("keyboard-browser-"));
  rmSync(scratch, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}
