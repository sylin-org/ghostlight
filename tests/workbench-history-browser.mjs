// H4: real bundled UI in isolated Chromium, with synthetic workbench projection events.
// This proves rendering and interaction, not an installed Tauri/native-host deployment.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fixtureRenderingArguments, readDevToolsPort, removeBrowserScratch, waitForChromiumExit } from "./lib/chromium.mjs";

const repository = resolve(import.meta.dirname, "..");
const scratchRoot = join(repository, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "history-browser-"));
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || [
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
].find(existsSync);
assert.ok(browser, "Set GHOSTLIGHT_TEST_BROWSER to a Chromium executable.");
const children = [];
const delay = (ms) => new Promise((done) => setTimeout(done, ms));
async function until(check, label) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    const value = await check();
    if (value) return value;
    await delay(25);
  }
  throw new Error(`Timed out: ${label}`);
}
function start(executable, args, env = process.env) {
  const child = spawn(executable, args, { cwd: repository, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  child.stderr.on("data", () => {});
  child.on("error", (error) => { child.startError = error; });
  children.push(child);
  return child;
}
let socket;
let send;
let chromium;
try {
  const server = start(process.execPath, ["tests/workbench-preview-server.mjs"], {
    ...process.env, GHOSTLIGHT_PREVIEW_SCENARIO: "h4", GHOSTLIGHT_PREVIEW_PORT: "0"
  });
  let address = "";
  server.stdout.on("data", (data) => { address += data; });
  await until(() => address.includes("http://"), "preview server");
  const profile = join(scratch, "profile");
  chromium = start(browser, ["--remote-debugging-port=0", `--user-data-dir=${profile}`,
    ...fixtureRenderingArguments(),
    ...(process.env.GHOSTLIGHT_TEST_NO_SANDBOX === "1" ? ["--no-sandbox"] : []),
    "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
    "--disable-component-update", "--disable-sync", "about:blank"]);
  const [port, endpoint] = await readDevToolsPort(profile, chromium);
  socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
  await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
  let nextId = 0;
  const pending = new Map();
  send = (method, params = {}, sessionId) => new Promise((done, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 10000);
    pending.set(id, { done, reject, timer });
    socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
  });
  socket.onmessage = ({ data }) => {
    const message = JSON.parse(data);
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    clearTimeout(entry.timer);
    if (message.error) entry.reject(new Error(JSON.stringify(message.error)));
    else entry.done(message.result);
  };
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  const page = (method, params) => send(method, params, sessionId);
  const evaluate = async (expression) => {
    const result = await page("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const resize = (width, height) => page("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
  const capture = async (name) => {
    const { data } = await page("Page.captureScreenshot", { format: "png" });
    writeFileSync(join(scratchRoot, `h4-${name}.png`), Buffer.from(data, "base64"));
  };
  // Wrap the synthetic native fixture before transport captures invoke. This exercises real
  // DOM clicks through app/transport while retaining exact arguments and mutable policy truth.
  await page("Page.enable");
  await page("Page.addScriptToEvaluateOnNewDocument", { source: `(() => {
    window.__QUIET_CALLS__ = [];
    let native;
    Object.defineProperty(window, '__TAURI__', {
      get: () => native,
      set(value) {
        const invoke = value.core.invoke;
        value.core.invoke = async (command, args) => {
          window.__QUIET_CALLS__.push({ command, args });
          if (command === 'reveal_browser_tab') {
            if (window.__QUIET_REVEAL_FAILURE__) throw new Error(window.__QUIET_REVEAL_FAILURE__);
            return;
          }
          if (command === 'workbench_policy' && window.__QUIET_POLICY__) return window.__QUIET_POLICY__;
          if (command === 'apply_user_policy' && window.__QUIET_POLICY__) {
            const document = JSON.parse(args.document);
            const setting = document.config.find(setting => setting.key === 'browser.attention');
            window.__QUIET_POLICY__.browser_attention.value = setting?.value ?? 'background';
            window.__QUIET_POLICY__.browser_attention.decided_by = 'user';
            let layer = window.__QUIET_POLICY__.layers.find(layer => layer.kind === 'user');
            if (!layer) {
              layer = { kind: 'user', title: 'Your rules', policy_name: document.name, version: document.version,
                mode: document.mode, path: window.__QUIET_POLICY__.user_layer.path, document: args.document, rules: [] };
              window.__QUIET_POLICY__.layers.push(layer);
            }
            layer.rules = document.grants.map(grant => ({ id: grant.id, description: grant.description,
              allow: grant.hosts.allow, deny: grant.hosts.deny ?? [], allowed: grant.allowed, mode: document.mode, note: null }));
            layer.settings = document.config.map(setting => ({ ...setting, value: JSON.stringify(setting.value) }));
          }
          return invoke(command, args);
        };
        native = value;
      }
    });
  })()` });
  await resize(1280, 900);
  await page("Page.navigate", { url: address.trim() });
  await until(() => evaluate("!!document.querySelector('.composition-details')"), "grouped history");
  assert.equal(await evaluate("document.querySelector('.composition-details').open"), false);
  await capture("collapsed");
  await evaluate("document.querySelector('.composition-details > summary').click()");
  await until(() => evaluate("document.querySelector('.composition-details').open"), "expanded history");
  assert.equal(await evaluate("document.querySelectorAll('.history-step').length"), 4);
  await evaluate("document.querySelector('.history-step .permission-details > summary').click()");
  await capture("expanded");
  // Exercise an overflowing group; follow-up receipts must preserve both focus and scroll.
  await evaluate("window.__GHOSTLIGHT_HISTORY_FIXTURE__ = structuredClone(window.__GHOSTLIGHT_PREVIEW__.history[0])");
  await evaluate(`(() => {
    const record = structuredClone(window.__GHOSTLIGHT_HISTORY_FIXTURE__);
    record.steps = Array.from({ length: 20 }, (_, index) => ({ ...record.steps[index === 16 ? 2 : 0], position: index + 1 }));
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'composition_changed', record });
  })()`);
  await delay(80);
  await evaluate("document.querySelector('.composition-details > summary').click()");
  await delay(30);
  await evaluate("document.querySelector('.composition-details > summary').click()");
  await delay(30);
  assert.equal(await evaluate(`(() => {
    const list = document.querySelector('.history-steps').getBoundingClientRect();
    const problem = document.querySelector('[data-step-problem="true"]').getBoundingClientRect();
    return problem.top >= list.top && problem.bottom <= list.bottom;
  })()`), true);
  const scroll = await evaluate(`(() => {
    const list = document.querySelector('.history-steps'); list.scrollTop = 220;
    document.querySelector('.composition-details > summary').focus({ preventScroll: true });
    return list.scrollTop;
  })()`);
  assert.ok(scroll > 0);
  await evaluate(`(() => {
    const record = structuredClone(window.__GHOSTLIGHT_HISTORY_FIXTURE__);
    record.steps = Array.from({ length: 20 }, (_, index) => ({ ...record.steps[index === 16 ? 2 : 0], position: index + 1 }));
    record.summary = 'Completed 3 of 20 steps.';
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'composition_changed', record });
  })()`);
  await delay(80);
  assert.equal(await evaluate("document.querySelector('.history-steps').scrollTop"), scroll);
  assert.equal(await evaluate("document.activeElement.matches('.composition-details > summary')"), true);
  assert.equal(await evaluate("document.querySelector('.history-step .permission-details').open"), true);
  await resize(720, 900);
  await delay(80);
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  await capture("narrow");
  assert.equal(await evaluate("document.querySelectorAll('[data-review-session], [data-resume-session]').length"), 0,
    "policy refusals create no automatic session recovery controls");
  assert.equal(await evaluate("window.__GHOSTLIGHT_PREVIEW__.service.runtime_state"), "active");

  // H7 uses the actual bundled UI with a synthetic storage-failure projection.
  await evaluate(`(() => {
    const snapshot = window.__GHOSTLIGHT_PREVIEW__;
    snapshot.audit_notice = 'History cannot be saved. Browser work can continue. Ghostlight checks automatically.';
    snapshot.audit_health = { failure: 'write', unconfirmed_receipts: 1, unreadable_entries: 0, history_unavailable: false };
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'audit_health_changed', health: snapshot.audit_health });
  })()`);
  await until(() => evaluate("!document.querySelector('#audit-health').hidden"), "audit health notice");
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  await evaluate(`(() => {
    const record = structuredClone(window.__GHOSTLIGHT_HISTORY_FIXTURE__);
    record.storage = 'saved';
    record.storage_detail = 'Some step history could not be saved.';
    record.steps[0].record.storage = 'unconfirmed';
    record.steps[0].record.storage_detail = 'History could not be saved. Storage was not confirmed.';
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'composition_changed', record });
  })()`);
  await until(() => evaluate("document.querySelector('.history-step')?.textContent.includes('Storage was not confirmed')"), "child storage notice");
  assert.equal(await evaluate("document.querySelector('.composition-details').open"), true);
  await capture("audit-unavailable");
  await evaluate(`(() => {
    const snapshot = window.__GHOSTLIGHT_PREVIEW__;
    snapshot.audit_notice = 'History is saving. 1 earlier receipt was not confirmed saved.';
    snapshot.audit_health.failure = null;
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'audit_health_changed', health: snapshot.audit_health });
  })()`);
  await until(() => evaluate("document.querySelector('#audit-health').textContent.includes('1 earlier receipt')"), "recovery gap notice");
  assert.equal(await evaluate("document.querySelector('.history-step').textContent.includes('Storage was not confirmed')"), true);
  await capture("audit-recovered");
  await evaluate(`(() => {
    const snapshot = window.__GHOSTLIGHT_PREVIEW__;
    snapshot.audit_notice = 'History is saving. 2 unreadable entries were omitted.';
    snapshot.audit_health = { failure: null, unconfirmed_receipts: 1, unreadable_entries: 2, history_unavailable: false };
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'audit_health_changed', health: snapshot.audit_health });
  })()`);
  await until(() => evaluate("document.querySelector('#audit-health').textContent.includes('2 unreadable entries')"), "unreadable history stays explicit after writes recover");
  assert.equal(await evaluate("document.querySelector('.history-step').textContent.includes('Storage was not confirmed')"), true);
  // H8: delayed admission stays live below the running action, then promotes the same row.
  await evaluate(`(() => {
    const operation = { invocation: 'h8-active', workspace: 'workspace_codex', tool: 'browser_read',
      activity: 'Reading', capability: 'read', phase: 'running', started_at_ms: Date.now() };
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_started', operation });
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_started', operation: { ...operation,
      invocation: 'h8-waiting', phase: 'waiting', activity: 'Waiting for earlier browser work' } });
  })()`);
  await until(() => evaluate("document.body.textContent.includes('Waiting for earlier browser work')"), "waiting operation visible");
  assert.equal(await evaluate("document.querySelector('#hero-body').textContent.includes('Reading')"), true);
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  await capture("waiting");
  await evaluate(`window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_started', operation: {
    invocation: 'h8-waiting', workspace: 'workspace_codex', tool: 'browser_read', capability: 'read',
    activity: 'Reading', phase: 'running', started_at_ms: Date.now() } })`);
  await until(() => evaluate("!document.body.textContent.includes('Waiting for earlier browser work')"), "waiting operation started");
  // C1: a later connection cannot relabel a receipt, and details remain usable across refresh.
  await evaluate(`(() => {
    const provenance = structuredClone(window.__GHOSTLIGHT_PREVIEW__.sessions[0].connections[0]);
    const record = { invocation: 'c1-receipt', workspace: 'workspace_codex', tool: 'browser_read',
      capability: 'read', allowed: true, status: 'succeeded', effect: 'none', summary: 'Read 5 words.',
      complete: true, timestamp_ms: Date.now(), channel: 'mcp', provenance };
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_settled', record });
  })()`);
  const receiptDetails = '[data-history-details="c1-receipt:connection"]';
  await until(() => evaluate(`!!document.querySelector('${receiptDetails}')`), "recorded connection details");
  assert.equal(await evaluate(`document.querySelector('${receiptDetails}').open`), false);
  await evaluate(`document.querySelector('${receiptDetails} > summary').click()`);
  await evaluate("document.querySelector('.session-connections > summary').click()");
  await evaluate("document.querySelector('.session-connections > summary').focus({ preventScroll: true })");
  await evaluate(`(() => {
    const snapshot = window.__GHOSTLIGHT_PREVIEW__;
    snapshot.sessions[0].client_label = 'Later application';
    snapshot.sessions[0].connections = [
      { ...snapshot.sessions[0].connections[0], connection_id: 'connection_later',
        reported_application: 'Later application', observed_executable: 'later-peer.exe' },
      { ...snapshot.sessions[0].connections[0], connection_id: 'connection_unavailable',
        reported_application: 'Another application', observed_executable: null,
        observation: 'Could not identify this connection' }
    ];
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'audit_health_changed', health: snapshot.audit_health });
  })()`);
  await until(() => evaluate("document.querySelector('.session-connections').textContent.includes('later-peer.exe')"), "refreshed connection evidence");
  assert.equal(await evaluate("document.activeElement.matches('.session-connections > summary')"), true);
  assert.equal(await evaluate("document.querySelector('.session-connections').open"), true);
  assert.equal(await evaluate("document.querySelector('.session-connections').textContent.includes('Could not identify this connection')"), true);
  assert.equal(await evaluate(`document.querySelector('${receiptDetails}').open`), true);
  assert.equal(await evaluate(`document.querySelector('${receiptDetails}').textContent.includes('ghostlight-mcp-connector.exe')`), true);
  assert.equal(await evaluate(`document.querySelector('${receiptDetails}').textContent.includes('Later application')`), false);
  assert.equal(await evaluate("document.querySelector('#hero-body .hero-meta').textContent.includes('Codex')"), true);
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  await capture("connection-details");
  // Restored receipts have no retained application claim. Neither a current session nor a legacy
  // basename can silently supply missing evidence, and even live claims remain plain text.
  await evaluate(`(() => {
    const record = { invocation: 'c1-restored', workspace: 'workspace_codex', tool: 'browser_read',
      capability: 'read', allowed: true, status: 'succeeded', effect: 'none', summary: 'Read 5 words.',
      complete: true, timestamp_ms: Date.now(), channel: 'mcp', provenance: {
        ...window.__GHOSTLIGHT_PREVIEW__.sessions[0].connections[0],
        reported_application: null, observed_executable: 'original-restored-peer.exe' } };
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_settled', record });
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_settled', record: {
      ...record, invocation: 'c1-legacy', provenance: null, peer_image: 'legacy-unverified.exe' } });
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_settled', record: {
      ...record, invocation: 'c1-escaped', provenance: { ...record.provenance,
        reported_application: '<img src=x onerror="window.__C1_INJECTED__=true">' } } });
  })()`);
  const restoredDetails = '[data-history-details="c1-restored:connection"]';
  const legacyDetails = '[data-history-details="c1-legacy:connection"]';
  const escapedDetails = '[data-history-details="c1-escaped:connection"]';
  await until(() => evaluate(`!!document.querySelector('${restoredDetails}') && !!document.querySelector('${escapedDetails}')`), "restored and escaped connection records");
  assert.equal(await evaluate(`document.querySelector('${restoredDetails}').textContent.includes('Not retained in history')`), true);
  assert.equal(await evaluate(`document.querySelector('${restoredDetails}').textContent.includes('original-restored-peer.exe')`), true);
  assert.equal(await evaluate(`document.querySelector('${restoredDetails}').textContent.includes('Later application')`), false);
  assert.equal(await evaluate(`document.querySelector('${legacyDetails}').textContent.includes('Connection details were not recorded.')`), true);
  assert.equal(await evaluate(`document.querySelector('${legacyDetails}').textContent.includes('legacy-unverified.exe')`), false);
  assert.equal(await evaluate(`document.querySelector('${legacyDetails}').textContent.includes('Later application')`), false);
  assert.equal(await evaluate(`document.querySelector('${escapedDetails}').textContent.includes('<img src=x')`), true);
  assert.equal(await evaluate(`document.querySelector('${escapedDetails} img') !== null || window.__C1_INJECTED__ === true`), false);
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  console.log("C1 browser history: immutable attribution, plural connections, restored/legacy evidence, escaped claims, quiet details, focus retention, and narrow layout passed.");
  console.log("H8 browser history: waiting stays live beneath running work, then promotes without duplication.");
  console.log("H7 browser history: persistent health, independent child storage, recovery gaps and unreadable entries, preserved expansion, and narrow layout passed.");
  console.log("H4 browser history: expansion, incremental scroll/focus, no automatic session recovery controls, and narrow layout passed.");
  // Ordinary history rows disclose the same detail as the hero, without replacing live work.
  await evaluate(`(() => {
    for (const invocation of ['detail-row', 'detail-hero']) {
      window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_settled', record: {
        invocation, workspace: 'workspace_codex', tool: 'browser_read', capability: 'read',
        allowed: true, status: 'succeeded', effect: 'none', summary: 'Read 5 words.',
        complete: true, timestamp_ms: Date.now(), channel: 'mcp',
        permission_explanations: ['Allowed without configured policy.']
      }});
    }
  })()`);
  const detailButton = '[data-action-details="detail-row:action"]';
  const detailPanel = '#action-details-detail-row';
  await until(() => evaluate(`!!document.querySelector('${detailButton}')`), 'action detail toggle');
  assert.equal(await evaluate(`document.querySelector('${detailPanel}').hidden`), true);
  await resize(1280, 900);
  await delay(80);
  const point = await evaluate(`(() => { const button = document.querySelector('${detailButton}'); button.scrollIntoView({block:'center'}); const r = button.getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2}; })()`);
  await page('Input.dispatchMouseEvent', {type:'mousePressed',button:'left',clickCount:1,...point});
  await page('Input.dispatchMouseEvent', {type:'mouseReleased',button:'left',clickCount:1,...point});
  assert.equal(await evaluate(`document.querySelector('${detailPanel}').hidden`), false, JSON.stringify(await evaluate(`({point:${JSON.stringify(point)},hit:document.elementFromPoint(${point.x},${point.y})?.outerHTML})`)));
  assert.equal(await evaluate(`document.querySelector('${detailButton}').getAttribute('aria-expanded')`), 'true');
  assert.equal(await evaluate(`document.querySelector('${detailPanel}').textContent.includes('Allowed without configured policy.')`), true);
  await evaluate(`document.querySelector('${detailPanel} .permission-details > summary').click(); document.querySelector('${detailButton}').focus()`);
  await evaluate(`window.__GHOSTLIGHT_PUBLISH__({kind:'operation_settled',record:{
    invocation:'detail-row',workspace:'workspace_codex',tool:'browser_read',capability:'read',allowed:true,
    status:'succeeded',effect:'none',summary:'Read 6 words.',complete:true,timestamp_ms:Date.now(),
    permission_explanations:['Allowed without configured policy.']
  }})`);
  await until(() => evaluate(`document.querySelector('${detailPanel}')?.textContent.includes('Read 6 words.')`), 'open detail update');
  assert.equal(await evaluate(`document.activeElement.matches('${detailButton}')`), true);
  assert.equal(await evaluate(`document.querySelector('${detailPanel} .permission-details').open`), true);
  for (const [key, code, virtual, hidden] of [[' ', 'Space', 32, true], ['Enter', 'Enter', 13, false]]) {
    await page('Input.dispatchKeyEvent', {type:'keyDown',key,code,windowsVirtualKeyCode:virtual,...(key === 'Enter' ? {text:'\r'} : {})});
    await page('Input.dispatchKeyEvent', {type:'keyUp',key,code,windowsVirtualKeyCode:virtual});
    assert.equal(await evaluate(`document.querySelector('${detailPanel}').hidden`), hidden, `keyboard toggle: ${code}`);
  }
  await evaluate(`window.__GHOSTLIGHT_PUBLISH__({kind:'operation_settled',record:{
    invocation:'newer-action',workspace:'workspace_codex',tool:'browser_scroll',capability:'read',allowed:true,
    status:'succeeded',effect:'none',summary:'Scrolled down.',complete:true,timestamp_ms:Date.now()
  }})`);
  assert.equal(await evaluate(`document.querySelector('${detailPanel}').hidden`), false);
  assert.equal(await evaluate(`document.querySelector('#hero-body').textContent.includes('Scrolled down.')`), true);
  for (const width of [1280, 720]) {
    await resize(width, 900);
    assert.equal(await evaluate(`(() => { const pause=document.querySelector('#wheel'), tab=document.querySelector('[data-view="monitor"]'); return !!(pause.compareDocumentPosition(tab) & Node.DOCUMENT_POSITION_FOLLOWING) && pause.getBoundingClientRect().right <= tab.getBoundingClientRect().left; })()`), true);
    assert.equal(await evaluate('document.documentElement.scrollWidth > innerWidth'), false);
    assert.equal(await evaluate(`(() => { const panel=document.querySelector('${detailPanel}'); panel.scrollIntoView({block:'center'}); const p=panel.getBoundingClientRect(), r=panel.closest('.row').getBoundingClientRect(); return p.height>100 && p.top>=r.top && p.bottom<=r.bottom; })()`), true, 'expanded panel must be visible within its row');
    await capture(`action-details-${width}`);
  }
  console.log('PASS action name opens hero details; updates retain expansion and focus; mouse, Space, Enter, and Pause order work at both widths');

  // Quiet coexistence in the bundled UI with synthetic native truth. This lane proves DOM
  // behavior and transport fidelity; installed browser ownership/focus is verified separately.
  await evaluate(`(async () => {
    window.__QUIET_BASE_POLICY__ = structuredClone(await window.__TAURI__.core.invoke('workbench_policy'));
    window.__QUIET_POLICY__ = structuredClone(window.__QUIET_BASE_POLICY__);
    const policy = window.__QUIET_POLICY__;
    policy.layers = []; policy.organization = null; policy.passport = { configured: false };
    policy.headline = 'No configured restrictions. Agents may work on ordinary websites.';
    policy.capabilities.forEach(line => { line.state = 'available'; line.decided_by = []; line.detail = 'Available on ordinary websites.'; });
    policy.browser_attention = { value: 'background', decided_by: 'default', organization_ceiling: null };
    document.querySelector('[data-view="policy"]').click();
  })()`);
  await until(() => evaluate("!!document.querySelector('#setting-browser-attention')"), 'first preference without user policy');
  assert.equal(await evaluate("document.querySelector('#apply-policy').disabled"), true, 'untouched empty policy cannot be applied');
  await evaluate(`(() => {
    const select = document.querySelector('#setting-browser-attention');
    select.value = 'foreground'; select.dispatchEvent(new Event('change', { bubbles: true }));
  })()`);
  assert.equal(await evaluate("document.querySelector('#apply-policy').disabled"), false, 'preference is usable without permission-rule boilerplate');
  assert.equal(await evaluate("document.querySelector('#rule-list').textContent.includes('look at pages, click and type, fill in forms, run page code')"), true);
  assert.equal(await evaluate("document.querySelector('#policy-permissions-note').hidden"), false);
  assert.equal(await evaluate("document.querySelector('#setting-browser-attention').parentElement.textContent.includes('take control of and navigate tabs Ghostlight does not yet control on the same host')"), true);
  assert.equal(await evaluate("document.querySelector('#setting-browser-attention').parentElement.textContent.includes('repair or move matching Ghostlight tab groups')"), true);
  await evaluate(`(() => {
    const select = document.querySelector('#setting-browser-startup');
    select.value = 'on_demand'; select.dispatchEvent(new Event('change', { bubbles: true }));
  })()`);
  assert.equal(await evaluate("document.querySelector('#setting-browser-startup').parentElement.textContent.includes('Background mode overrides this: automatic launch stays off')"), true);
  assert.equal(await evaluate("document.querySelector('#setting-browser-startup').parentElement.textContent.includes('agent asks you to open an eligible browser')"), true);
  await evaluate("document.querySelector('#policy-permissions-note').scrollIntoView({ block: 'center' })");
  await capture('quiet-first-preference');
  await evaluate("document.querySelector('#apply-policy').click()");
  await until(() => evaluate("window.__QUIET_POLICY__.layers.some(layer => layer.kind === 'user')"), 'first preference applied');
  assert.deepEqual(await evaluate(`(() => {
    const document = JSON.parse(window.__QUIET_CALLS__.filter(call => call.command === 'apply_user_policy').at(-1).args.document);
    return { allow: document.grants[0].hosts.allow, allowed: document.grants[0].allowed,
      grants: document.grants.length, setting: document.config.find(setting => setting.key === 'browser.attention').value };
  })()`), { allow: ['*'], allowed: ['read', 'action', 'write', 'execute'], grants: 1, setting: 'foreground' });
  await until(() => evaluate("document.querySelector('#setting-browser-attention').value === 'foreground'"), 'first preference applied readback');
  console.log('PASS first preference without user policy preserves a visible wildcard RAWX rule and serializes foreground through normal Apply');
  await evaluate(`(async () => {
    window.__QUIET_POLICY__ = structuredClone(window.__QUIET_BASE_POLICY__);
    window.__QUIET_POLICY__.browser_attention = { value: 'background', decided_by: 'default', organization_ceiling: null };
    window.__GHOSTLIGHT_PREVIEW__.configuration.browser_attention = window.__QUIET_POLICY__.browser_attention;
    await resync();
    document.querySelector('[data-view="policy"]').click();
  })()`);
  const attentionSelect = '#setting-browser-attention';
  await until(() => evaluate(`!!document.querySelector('${attentionSelect}')`), 'background policy choice');
  assert.equal(await evaluate(`document.querySelector('${attentionSelect}').value`), 'background');
  assert.equal(await evaluate("document.querySelector('#policy-settings').textContent.includes('Ghostlight default')"), true,
    JSON.stringify(await evaluate("({ settings: document.querySelector('#policy-settings').innerHTML, calls: window.__QUIET_CALLS__?.slice(-4), attention: window.__QUIET_POLICY__?.browser_attention })")));
  assert.equal(await evaluate(`document.querySelector('${attentionSelect}').parentElement.textContent.includes('without activating or moving your tabs')`), true);
  await capture('quiet-policy-default');
  await evaluate(`(() => {
    const select = document.querySelector('${attentionSelect}');
    select.value = 'foreground'; select.dispatchEvent(new Event('change', { bubbles: true }));
    document.querySelector('#apply-policy').click();
  })()`);
  await until(() => evaluate("window.__QUIET_POLICY__.browser_attention.value === 'foreground'"), 'attention policy applied');
  await until(() => evaluate(`document.querySelector('${attentionSelect}').value === 'foreground'`), 'applied attention readback');
  assert.equal(await evaluate(`(() => {
    const call = window.__QUIET_CALLS__.filter(call => call.command === 'apply_user_policy').at(-1);
    return JSON.parse(call.args.document).config.some(setting => setting.key === 'browser.attention' && setting.value === 'foreground');
  })()`), true);
  await evaluate(`(() => {
    const select = document.querySelector('${attentionSelect}');
    select.value = 'background'; select.dispatchEvent(new Event('change', { bubbles: true }));
    document.querySelector('#apply-policy').click();
  })()`);
  await until(() => evaluate("window.__QUIET_POLICY__.browser_attention.value === 'background'"), 'background policy applied');
  await until(() => evaluate(`document.querySelector('${attentionSelect}').value === 'background'`), 'background applied readback');
  assert.equal(await evaluate(`(() => {
    const call = window.__QUIET_CALLS__.filter(call => call.command === 'apply_user_policy').at(-1);
    return JSON.parse(call.args.document).config.some(setting => setting.key === 'browser.attention' && setting.value === 'background');
  })()`), true);
  await evaluate(`(() => {
    window.__QUIET_POLICY__.browser_attention = { value: 'background', decided_by: 'organization', organization_ceiling: 'background' };
    document.querySelector('#refresh-policy').click();
  })()`);
  await until(() => evaluate(`document.querySelector('${attentionSelect}').disabled`), 'organization background ceiling');
  assert.equal(await evaluate(`document.querySelector('${attentionSelect}').value`), 'background');
  assert.equal(await evaluate(`document.querySelector('${attentionSelect}').parentElement.textContent.includes('requires background browser work')`), true);
  await resize(720, 900);
  assert.equal(await evaluate('document.documentElement.scrollWidth > innerWidth'), false);
  await evaluate(`document.querySelector('${attentionSelect}').scrollIntoView({ block: 'center' })`);
  await capture('quiet-policy-ceiling-720');
  await evaluate(`(() => {
    window.__QUIET_POLICY__.user_layer.editable = false;
    document.querySelector('#refresh-policy').click();
  })()`);
  await until(() => evaluate("document.querySelector('#policy-editor').hidden"), 'read-only policy');
  assert.equal(await evaluate("document.querySelector('#policy-settings').textContent.includes('Agents cannot request foreground focus')"), true);

  await evaluate(`(async () => {
    window.__GHOSTLIGHT_PREVIEW__.configuration.browser_attention = window.__QUIET_POLICY__.browser_attention;
    window.__GHOSTLIGHT_PREVIEW__.readiness = {
      state: 'ready', word: 'Ready', tone: 'quiet', detail: 'Synthetic connected browser.', invites_control: true
    };
    await resync();
    document.querySelector('[data-view="monitor"]').click();
    document.querySelector('#wheel').click();
    await resync();
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_started', operation: {
      invocation: 'quiet-live', workspace: 'workspace_codex', tab: 'tab_opaque_exact',
      tool: 'browser_read', activity: 'Reading', capability: 'read', phase: 'running', started_at_ms: Date.now()
    } });
  })()`);
  const showTab = '#hero-body [data-reveal-tab="tab_opaque_exact"]';
  await until(() => evaluate(`!!document.querySelector('${showTab}')`), 'live Show tab');
  assert.equal(await evaluate("document.querySelector('#browser-attention').textContent.includes('background')"), true);
  assert.equal(await evaluate("document.querySelector('#wheel').dataset.intent"), 'resume');
  const intentCount = await evaluate("window.__QUIET_CALLS__.filter(call => call.command === 'apply_runtime_intent').length");
  await evaluate(`document.querySelector('${showTab}').focus(); document.querySelector('${showTab}').click()`);
  await until(() => evaluate("window.__QUIET_CALLS__.some(call => call.command === 'reveal_browser_tab')"), 'native tab reveal');
  assert.deepEqual(await evaluate("window.__QUIET_CALLS__.filter(call => call.command === 'reveal_browser_tab').at(-1).args"),
    { workspace: 'workspace_codex', tab: 'tab_opaque_exact' });
  assert.equal(await evaluate("window.__QUIET_CALLS__.filter(call => call.command === 'apply_runtime_intent').length"), intentCount);
  await evaluate(`window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_settled', record: {
    invocation: 'quiet-live', workspace: 'workspace_codex', tab: 'tab_opaque_exact', tool: 'browser_read',
    capability: 'read', allowed: true, status: 'succeeded', effect: 'none', summary: 'Read 7 words.',
    complete: true, timestamp_ms: Date.now()
  } })`);
  await until(() => evaluate("document.querySelector('#hero-body').textContent.includes('Read 7 words.')"), 'settled tab destination');
  assert.equal(await evaluate(`document.activeElement.matches('${showTab}')`), true, 'Show tab focus survives receipt repaint');
  await evaluate(`window.__QUIET_REVEAL_FAILURE__ = 'This tab is no longer controlled.'; document.querySelector('${showTab}').click()`);
  await until(() => evaluate("document.querySelector('#toast').textContent.includes('no longer controlled')"), 'stale reveal error');
  assert.equal(await evaluate(`document.querySelector('${showTab}').disabled`), false);
  assert.equal(await evaluate("document.querySelector('#wheel').dataset.intent"), 'resume');
  for (const width of [1280, 720]) {
    await resize(width, 900);
    await evaluate(`document.querySelector('${showTab}').scrollIntoView({ block: 'center' })`);
    assert.equal(await evaluate('document.documentElement.scrollWidth > innerWidth'), false);
    await capture(`quiet-show-tab-${width}`);
  }
  await evaluate(`(() => {
    window.__QUIET_REVEAL_FAILURE__ = null;
    window.__GHOSTLIGHT_PUBLISH__({ kind: 'operation_settled', record: {
      invocation: 'quiet-newer', workspace: 'workspace_codex', tool: 'browser_read', capability: 'read',
      allowed: true, status: 'succeeded', effect: 'none', summary: 'Read 8 words.', complete: true, timestamp_ms: Date.now()
    } });
    document.querySelector('[data-action-details="quiet-live:action"]').click();
  })()`);
  const historyShowTab = '#action-details-quiet-live [data-reveal-tab="tab_opaque_exact"]';
  assert.equal(await evaluate("document.querySelector('#action-details-quiet-live').hidden"), false);
  await evaluate(`document.querySelector('${historyShowTab}').focus(); document.querySelector('${historyShowTab}').click()`);
  await until(() => evaluate("window.__QUIET_CALLS__.filter(call => call.command === 'reveal_browser_tab').length === 3"), 'history tab reveal');
  assert.equal(await evaluate("window.__QUIET_CALLS__.filter(call => call.command === 'apply_runtime_intent').length"), intentCount);
  assert.equal(await evaluate("document.querySelector('#hero-body [data-reveal-tab]') === null"), true);
  await evaluate(`document.querySelector('${historyShowTab}').scrollIntoView({ block: 'center' })`);
  assert.equal(await evaluate('document.documentElement.scrollWidth > innerWidth'), false);
  await capture('quiet-history-show-tab-720');
  console.log('PASS quiet Workbench: default/effective author, policy apply/readback, organization ceiling, read-only feedback, exact native reveal without resume, stale errors, focus retention, and 1280/720px layout');
  await evaluate(`(async () => {
    const snapshot = window.__GHOSTLIGHT_PREVIEW__;
    snapshot.browsers = [{ id: 'legacy-adapter', family: 'Chrome', adapter_version: '1.0.0', connected: false,
      detail: 'Update Ghostlight in Browser to continue. This adapter cannot enforce background work; its connection was retired without closing tabs.' }];
    snapshot.service.runtime_state = 'active'; snapshot.configuration.runtime_state = 'active';
    snapshot.diagnostics = [{ severity: 'warning', label: 'Browser adapter update needed',
      detail: 'An adapter cannot enforce background work and was retired without closing tabs. Update Ghostlight in Browser to continue.' }];
    await resync();
  })()`);
  assert.equal(await evaluate("document.querySelector('[data-browser=\"legacy-adapter\"]').classList.contains('on')"), false);
  assert.equal(await evaluate("document.querySelector('[data-browser=\"legacy-adapter\"]').textContent.includes('Unavailable')"), true);
  assert.equal(await evaluate("document.querySelector('[data-browser-detail=\"legacy-adapter\"]').textContent.includes('Update Ghostlight in Browser to continue')"), true);
  assert.equal(await evaluate("document.querySelector('#wheel').dataset.intent"), 'hold', 'adapter retirement never pauses active operator control');
  assert.equal(await evaluate("document.querySelector('#state-facts').textContent.includes('0 browsers')"), true);
  assert.equal(await evaluate("window.__GHOSTLIGHT_PREVIEW__.service.runtime_state"), 'active');
  await evaluate("document.querySelector('[data-browser-detail=\"legacy-adapter\"]').scrollIntoView({ block: 'center' })");
  assert.equal(await evaluate('document.documentElement.scrollWidth > innerWidth'), false);
  await capture('quiet-legacy-update-720');
  await evaluate("document.querySelector('[data-view=\"status\"]').click()");
  assert.equal(await evaluate("document.querySelector('#diagnostic-grid').textContent.includes('Browser adapter update needed')"), true);
  await capture('quiet-legacy-status-720');
  console.log('PASS incompatible adapter is visibly unavailable with update detail; browser count excludes it and operator runtime stays Active');
  // The delight slice uses the same bundled surface and snapshot/event paths. These are owned
  // projection fixtures, not installed native UX or physical-keyboard acceptance.
  await evaluate(`(async () => {
    const preview = window.__GHOSTLIGHT_PREVIEW__;
    preview.audit_notice = null;
    preview.browsers = [{id:'delight-browser',family:'Chromium',connected:true,adapter_version:preview.service.version}];
    preview.sessions = [
      {id:'delight-live-workspace',client_label:'delight MCP',channel:'mcp',connections:[{channel:'mcp'}],tab_count:2,active_operations:2,leased:true},
      {id:'delight-retained-workspace',client_label:'Draft CLI',channel:'cli',connections:[],tab_count:1,active_operations:0,leased:false},
      {id:'delight-dormant-workspace',client_label:'Empty CLI',channel:'cli',connections:[],tab_count:0,active_operations:0,leased:false}
    ];
    preview.readiness = {state:'ready',word:'Ready',tone:'quiet',detail:'Synthetic connected browser.',invites_control:true};
    preview.history = Array.from({length:500},(_,index)=>({
      invocation:'delight-history-'+index,workspace:'delight-live-workspace',tool:index===0?'browser_tabs':'browser_read',
      capability:index===0?'action':'read',allowed:true,status:index===0?'blocked':'succeeded',effect:'none',complete:true,
      timestamp_ms:Date.now()-index-1000,summary:index===0?"Kept the tab open: Ghostlight's preserve-tabs setting is on.":'Read 5 words.',
      presentation:{label:index===0?'Tab preserved':'Completed',tone:index===0?'controlled':'complete',
        summary:index===0?"Kept the tab open: Ghostlight's preserve-tabs setting is on.":'Read 5 words.',repeat_detail:''}
    }));
    preview.operations = [
      {invocation:'delight-reading',workspace:'delight-live-workspace',tab:'tab_owned_read',tool:'browser_read',activity:'Reading',phase:'running',started_at_ms:Date.now()},
      {invocation:'delight-waiting',workspace:'delight-live-workspace',tab:'tab_owned_wait',tool:'browser_wait',
        activity:'Waiting for the requested text to appear (up to 15000 ms remaining).',phase:'running',started_at_ms:Date.now()-1}
    ];
    await resync({rebuildFeed:true}); document.querySelector('[data-view="monitor"]').click();
  })()`);
  assert.equal(await evaluate("document.querySelector('#queue-count').textContent"), '200 shown / 500 retained groups');
  assert.equal(await evaluate("document.querySelector('#connections').textContent.includes('Empty CLI')"), false);
  assert.equal(await evaluate("document.querySelector('#connections').textContent.includes('1 tab retained')"), true);
  assert.equal(await evaluate("document.querySelector('#state-facts').textContent.includes('1 connections')"), true);
  for (const width of [1280,720]) {
    await resize(width,900); await delay(80);
    assert.equal(await evaluate('document.documentElement.scrollWidth > innerWidth'), false);
    assert.equal(await evaluate("document.querySelector('[data-action-details=\"delight-waiting:action\"]').textContent"), 'browser_wait');
    assert.equal(await evaluate("document.querySelector('[data-action-details=\"delight-waiting:action\"]').closest('.row').querySelector('.row-activity').textContent.includes('requested text to appear')"), true);
    await capture('delight-wait-history-'+width);
  }
  await evaluate("document.querySelector('#clear-monitor').click()");
  await evaluate("resync({rebuildFeed:true})");
  assert.equal(await evaluate("document.querySelector('#queue-count').textContent"), '2 shown / 500 retained groups');
  assert.equal(await evaluate("document.querySelector('#show-history').hidden"), false);
  assert.equal(await evaluate("document.querySelector('#toast').textContent.includes('Cleared 500 completed groups')"), true);
  await capture('delight-cleared-720');
  await evaluate("document.querySelector('#show-history').click()");
  assert.equal(await evaluate("document.querySelector('#queue-count').textContent"), '200 shown / 500 retained groups');
  await evaluate(`(() => {
    const operation = {invocation:'delight-composed-wait',workspace:'delight-live-workspace',tab:'tab_owned_wait',
      tool:'browser_flow',activity:'Waiting for the requested text to appear (up to 20000 ms remaining).',
      phase:'running',started_at_ms:Date.now()+1};
    const record = {invocation:operation.invocation,workspace:operation.workspace,tab:operation.tab,tool:operation.tool,
      capability:'read',allowed:true,status:'succeeded',effect:'partial',complete:false,timestamp_ms:Date.now(),
      summary:'Completed 2 of 4 steps.',presentation:{summary:'Completed 2 of 4 steps.',label:'Working',tone:'running'},
      steps:[{position:1,record:window.__GHOSTLIGHT_HISTORY_FIXTURE__.steps[0].record},
        {position:2,record:window.__GHOSTLIGHT_HISTORY_FIXTURE__.steps[1].record}]};
    window.__GHOSTLIGHT_PUBLISH__({kind:'operation_started',operation});
    window.__GHOSTLIGHT_PUBLISH__({kind:'composition_changed',record});
    window.__GHOSTLIGHT_PREVIEW__.operations.push(operation);
    window.__GHOSTLIGHT_PREVIEW__.history.unshift(record);
    window.__GHOSTLIGHT_PREVIEW__.history.length=500;
  })()`);
  for (const width of [1280,720]) {
    await resize(width,900);
    await until(() => evaluate("document.querySelector('#hero-body .hero-activity')?.textContent.includes('20000 ms remaining')"), 'composed current wait purpose');
    assert.equal(await evaluate("document.querySelector('#hero-body .hero-progress').textContent"), 'Completed 2 of 4 steps.');
    assert.equal(await evaluate("document.querySelector('#queue-count').textContent"), '200 shown / 500 retained groups');
    assert.equal(await evaluate("document.querySelector('#hero-body').textContent.includes('browser_flow')"), true);
    await capture('delight-composed-wait-live-'+width);
    await evaluate("resync({rebuildFeed:true})");
    assert.equal(await evaluate("document.querySelector('#hero-body .hero-activity').textContent.includes('20000 ms remaining')"), true);
    assert.equal(await evaluate("document.querySelector('#hero-body .hero-progress').textContent"), 'Completed 2 of 4 steps.');
    assert.equal(await evaluate('document.documentElement.scrollWidth > innerWidth'), false);
    await capture('delight-composed-wait-rebuilt-'+width);
  }
  await evaluate(`(() => {
    const operation=window.__GHOSTLIGHT_PREVIEW__.operations.find(item=>item.invocation==='delight-composed-wait');
    operation.activity='Reading';
    window.__GHOSTLIGHT_PUBLISH__({kind:'operation_changed',operation});
  })()`);
  await until(() => evaluate("document.querySelector('#hero-body .hero-activity')?.textContent==='Reading'"), 'next child clears wait purpose');
  assert.equal(await evaluate("document.querySelector('#hero-body').textContent.includes('ms remaining')"), false);
  await evaluate("resync({rebuildFeed:true})");
  assert.equal(await evaluate("document.querySelector('#hero-body .hero-activity').textContent"), 'Reading');
  await capture('delight-composed-next-child-720');
  await evaluate("document.querySelector('[data-view=\"about\"]').click()");
  await delay(4300);
  await resize(1280,900); await delay(80); await capture('delight-guardian-1280');
  await resize(720,900); await delay(80);
  assert.equal(await evaluate('document.documentElement.scrollWidth > innerWidth'), false);
  await capture('delight-guardian-720');
  console.log('PASS delight: exact wait tool/purpose/reveal, composed live/rebuilt purpose and secondary counts, next-child budget removal, 200 shown/500 retained, quiet dormant continuity, clear/restore, and guardian card at 1280/720');
} finally {
  if (send && socket?.readyState === WebSocket.OPEN) {
    try { await send("Browser.close"); } catch { /* shutdown can close the reply channel */ }
  }
  socket?.close();
  await waitForChromiumExit(chromium);
  for (const child of children.toReversed()) {
    if (child.exitCode === null && child.signalCode === null) child.kill();
  }
  await until(() => children.every((child) => child.exitCode !== null || child.signalCode !== null || child.startError), "owned child exit");
  await removeBrowserScratch(scratch, scratchRoot, "history-browser-");
}
