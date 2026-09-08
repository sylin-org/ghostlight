// H4: real bundled UI in isolated Chromium, with synthetic workbench projection events.
// This proves rendering and interaction, not an installed Tauri/native-host deployment.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { readDevToolsPort, removeBrowserScratch, waitForChromiumExit } from "./lib/chromium.mjs";

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
    ...process.env, GHOSTLIGHT_PREVIEW_SCENARIO: "h5", GHOSTLIGHT_PREVIEW_PORT: "0"
  });
  let address = "";
  server.stdout.on("data", (data) => { address += data; });
  await until(() => address.includes("http://"), "preview server");
  const profile = join(scratch, "profile");
  chromium = start(browser, ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
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
  // Session review restores an explicitly cleared group, expands its problem, and recovers
  // only the incident handed to the UI. No browser job or global Resume is dispatched.
  await evaluate("document.querySelector('#clear-monitor').click()");
  await evaluate("document.querySelector('[data-review-session]').click()");
  await until(() => evaluate("!!document.querySelector('.composition-details')?.open"), "review restored group");
  assert.equal(await evaluate("document.querySelectorAll('.history-step').length"), 20);
  assert.equal(await evaluate(`(() => {
    const list = document.querySelector('.history-steps').getBoundingClientRect();
    const problem = document.querySelector('[data-step-problem="true"]').getBoundingClientRect();
    return problem.top >= list.top && problem.bottom <= list.bottom;
  })()`), true);
  assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth"), false);
  await capture("session-review");
  await evaluate("document.querySelector('[data-resume-session]').click()");
  await until(() => evaluate("!document.querySelector('[data-resume-session]')"), "scoped recovery");
  assert.deepEqual(await evaluate("window.__GHOSTLIGHT_RESUMED__"), { workspace: "workspace_codex", incident: "attention_test" });
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
  console.log("H4/H5 browser history: expansion, incremental scroll/focus, cleared-history review, scoped resume, and narrow layout passed.");
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
