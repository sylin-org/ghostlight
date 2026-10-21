// The real executables on relay protocol 4: an MCP session through `ghostlight mcp`, a synthetic
// browser through `ghostlight chrome-extension://<id>/`, a service killed and restarted under
// both, a browser restart that hands the agent's tab id to the person's page, a replace that keeps
// the session's tab for the next service, Quit releasing that tab and holding the next browser
// start back, ten racing starts leaving one service, and, on Linux, the edge and the service each
// leaving on SIGTERM, the session's end there.
// Every process runs in a fresh test home under the temp directory; nothing touches the real one.
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { constants } from "node:os";
import { join } from "node:path";
import {
  QUIT_LATCH, buildVersion, controlOp, createHome, delay, imagePath, openPolicy, quitReason, repository, samePath,
  startReason, templateKind, until, vocabulary, within
} from "./lib/home.mjs";
import { COLD_REQUEST_MS, EXTENSION_ORIGIN, McpEdge, RelayMcp, SyntheticBrowser, World, structured } from "./lib/peers.mjs";

// docs/1.0/catalog.json is written on Windows. The catalog advertises an upload example path in the
// service's own platform form (EXAMPLE_UPLOAD_PATH in crates/orchestrator/src/language/catalog.rs),
// so on every other platform the golden's path is read in the form the service advertises there.
const GOLDEN_UPLOAD_PATH = "C:\\Users\\me\\notes.txt";
const PLATFORM_UPLOAD_PATH = "/home/me/notes.txt";
const goldenText = readFileSync(join(repository, "docs", "1.0", "catalog.json"), "utf8");
const catalog = JSON.parse(process.platform === "win32"
  ? goldenText
  : goldenText.replaceAll(JSON.stringify(GOLDEN_UPLOAD_PATH), JSON.stringify(PLATFORM_UPLOAD_PATH)));
const DENIED_HOST = "blocked.localhost";
// The person's own page that a restarted browser shows under the agent's old tab id.
const PERSONS_URL = "https://example.com/the-persons-own";
// The page of the tab a replace keeps bound and a quit releases.
const KEPT_URL = "https://example.com/kept";
// Where that page routes itself while the service is replaced, with no navigation the service sees.
const ROUTED_URL = "https://example.com/kept/inbox";
// The extension life of the browser run before its restart, and of the run after it.
const FIRST_LIFE = "life_first";
const RESTARTED_LIFE = "life_restarted";
const BROWSER_ABSENT = (frame) => frame.kind === "backend_unavailable" && frame.service === "absent";
const CONTENDERS = 10;
// How long each binary may take to leave once it hears the end of the session, the budgets of
// tests/windows-desktop-journey.ps1.
const EDGE_END_SESSION_MS = 4_000;
const SERVICE_END_SESSION_MS = 6_000;
// The signal that ends the session for a Ghostlight process on Linux.
const SESSION_END_SIGNAL = "SIGTERM";
// How long a process may take to install its session listener after it starts.
const LISTENER_READY_MS = 10_000;
// How long nothing of the home may start after the last process left: longer than the 2 s fallback
// exit and the 2 s presence launch delay, so a restart would show.
const RESTART_SETTLE_MS = 3_000;
// A starter that loses the race for the service lock polls it for the service's lock wait
// (LEAVING_LOCK_WAIT in crates/orchestrator/src/service/lifetime.rs) before it leaves, and one
// still waiting when the service leaves can take the lock and serve in its place.
const LEAVING_LOCK_WAIT_MS = 5_000;
const LEAVING_LOCK_MARGIN_MS = 10_000;

async function racingStarts() {
  const race = createHome({ label: "race" });
  try {
    const contenders = Array.from({ length: CONTENDERS }, () =>
      race.start(race.service, ["serve", "--reason", startReason.agent], { stdio: ["ignore", "ignore", "pipe"] }));
    const first = await race.serviceUp();
    // Each start relaunches itself once and leaves (ADR-0199 amendment of 2026-10-21), so every
    // contender exits quietly and the relaunched copies race for the lock.
    await until(() => contenders.every((child) => child.exitCode !== null || child.signalCode !== null),
      "every contender relaunches and exits", 40_000);
    for (const child of contenders) {
      assert.equal(child.exitCode, 0, `a start exits quietly once it has relaunched: ${child.output}`);
    }
    await until(() => race.serviceProcesses().length === 1, "all but one relaunched start exits", 40_000);
    await delay(1500);
    const later = await race.status();
    assert.equal(later.instance, first.instance, "the instance is stable");
    assert.deepEqual(race.serviceProcesses(), [later.pid], "the one left is the service");
    await race.quit();
    await until(() => race.serviceProcesses().length === 0, "the service exits after quit", 10_000);
    console.log("PASS ten racing starts leave exactly one service");
  } finally {
    await race.cleanup();
  }
}

async function mainFlow() {
  const home = createHome({ policy: openPolicy([DENIED_HOST]), label: "process" });
  const world = new World();
  const browsers = [];
  try {
    // An MCP session starts the service as an agent start.
    const mcp = new McpEdge(home.start(home.edge, ["mcp"]));
    const initialized = await mcp.initialize("process journey");
    assert.equal(initialized.serverInfo.name, "ghostlight");
    const status = await home.serviceUp();
    assert.equal(status.started_by, startReason.agent);
    assert.equal(status.service, buildVersion());
    const listed = await mcp.request("tools/list");
    assert.deepEqual(listed.result.tools, catalog, "tools/list is the catalog of docs/1.0/catalog.json");
    const explained = await mcp.facts("policy_explain");
    assert.equal(explained.status, "succeeded");
    const denied = await mcp.facts("browser_navigate", { url: `http://${DENIED_HOST}:3000/` });
    assert.equal(denied.status, "blocked");
    assert.equal(denied.effect, "none");
    console.log("PASS an MCP session through the edge lists the catalog and calls policy_explain");

    // A synthetic browser through the browser edge.
    const browser = SyntheticBrowser.viaEdge(home, world);
    browser.life = FIRST_LIFE;
    browsers.push(browser);
    const accepted = await browser.connect();
    assert.equal(accepted.control_state, "active");
    assert.ok(accepted.service_epoch);
    // The session holds no tab yet, so its list answers without asking the browser.
    const asked = browser.requested("list_tabs").length;
    const tabs = await mcp.facts("browser_tabs", { action: "list" });
    assert.equal(tabs.status, "succeeded");
    assert.deepEqual(tabs.facts.tabs, []);
    assert.equal(tabs.facts.browsers.length, 1);
    assert.equal(browser.requested("list_tabs").length, asked);
    const opened = await mcp.facts("browser_navigate", { url: "https://example.com/" });
    assert.equal(opened.status, "succeeded");
    const handle = opened.facts.tab;
    assert.match(handle, /^tab_/);
    assert.equal(world.tabs.length, 1);
    assert.equal(browser.requested("open_tab").some((request) => request.primitive.args.url.includes(DENIED_HOST)), false);
    const moved = await mcp.facts("browser_navigate", { tab: handle, url: "https://example.com/second" });
    assert.equal(moved.status, "succeeded");
    assert.equal(browser.requested("navigate").length, 1);
    const read = await mcp.facts("browser_read", { tab: handle });
    assert.equal(read.status, "succeeded");
    assert.equal(read.facts.text, "Example Domain");
    const shot = await mcp.call("browser_screenshot", { tab: handle });
    assert.equal(structured(shot).status, "succeeded");
    assert.match(structured(shot).facts.view, /^view_/);
    assert.equal(shot.result.content.some((item) => item.type === "image"), true);
    assert.equal(browser.requested("capture").length, 1);
    console.log("PASS a synthetic browser serves list_tabs, navigate, a call and a capture");

    // The service is killed under a call in flight. The read carries the longest timeout_ms a call
    // allows, so the kill lands before the read's own deadline however slow the runner is.
    const probe = await RelayMcp.open(home);
    const cutOff = probe.reply.templates[templateKind.cut_off]["tools/call"].result.content;
    probe.close();
    browser.holdNext("call");
    const inFlight = mcp.begin("tools/call", { name: "browser_read", arguments: { tab: handle, timeout_ms: 30_000 } });
    await until(() => browser.held.length === 1, "the read reaches the browser");
    const before = await home.status();
    await home.killService();
    const interrupted = await within(inFlight.promise, COLD_REQUEST_MS, "the interrupted call answers");
    assert.equal(interrupted.result.isError, true);
    assert.deepEqual(interrupted.result.content, cutOff, "the cut-off call answers with the service's own words");
    await browser.waitFor(BROWSER_ABSENT, 15_000, "the browser edge reports the service absent");
    console.log("PASS a call cut off by a killed service says it may have happened");

    // The next call starts a service, resumes the session, and the tab handle survives.
    const relisted = await mcp.request("tools/list");
    assert.equal(relisted.result.tools.length, catalog.length);
    await within(browser.ended, 20_000, "the browser edge exits when a service accepts again");
    const reborn = SyntheticBrowser.viaEdge(home, world);
    reborn.life = FIRST_LIFE;
    browsers.push(reborn);
    const again = await reborn.connect();
    assert.notEqual(again.service_epoch, accepted.service_epoch, "the new service has a new epoch");
    const after = await home.status();
    assert.notEqual(after.instance, before.instance);
    assert.equal(after.started_by, startReason.agent);
    const reread = await mcp.facts("browser_read", { tab: handle });
    assert.equal(reread.status, "succeeded");
    assert.equal(reread.facts.text, "Example Domain");
    console.log("PASS the next call resumes by token, the tab handle survives and the browser reconnects");

    // The browser restarts and hands the agent's old tab id to the person's own page.
    reborn.close();
    await within(reborn.ended, 20_000, "the browser edge exits with its browser");
    world.navigate(world.tabs[0].tab_id, PERSONS_URL);
    const restartedBrowser = SyntheticBrowser.viaEdge(home, world);
    restartedBrowser.life = RESTARTED_LIFE;
    browsers.push(restartedBrowser);
    await restartedBrowser.connect();
    const stale = await mcp.facts("browser_read", { tab: handle });
    assert.notEqual(stale.status, "succeeded");
    assert.equal(stale.summary, "That tab handle names no open tab in this session.");
    assert.equal(restartedBrowser.requested("call").length, 0, "nothing reads the person's tab");
    const afterRestart = await mcp.facts("browser_tabs", { action: "list" });
    assert.deepEqual(afterRestart.facts.tabs, [], "the person's tab is not listed under the agent's handle");
    console.log("PASS a browser restart never hands the agent's tab handle the person's tab");

    // A replace closes nothing of a live session, and the next service resumes its tab from the
    // session's token. The edge waits for the replacement, so the journey starts it, as an
    // update does.
    const keptOpen = await mcp.facts("browser_navigate", { url: KEPT_URL });
    assert.equal(keptOpen.status, "succeeded");
    const kept = keptOpen.facts.tab;
    const keptId = world.tabs.find((tab) => tab.url === KEPT_URL).tab_id;
    const closedBefore = restartedBrowser.requested("close_tab").length;
    const replaced = await home.status();
    const replacing = await home.control({ op: controlOp.quit, reason: quitReason.replace });
    assert.deepEqual(replacing.value, { quitting: true });
    await restartedBrowser.waitFor(BROWSER_ABSENT, 15_000, "the browser edge reports the replace");
    assert.equal(restartedBrowser.requested("close_tab").length, closedBefore, "a replace closes no tab of a live session");
    home.start(home.service, ["serve", "--reason", startReason.update], { stdio: ["ignore", "ignore", "pipe"] });
    await until(async () => {
      const next = await home.status();
      return next !== null && next.instance !== replaced.instance;
    }, "the replacing service serves", 40_000);
    await within(restartedBrowser.ended, 20_000, "the browser edge exits when the replacing service accepts");
    // The page routes itself meanwhile. The browser run goes on, so its life confirms the tab.
    world.navigate(keptId, ROUTED_URL);
    const resumedBrowser = SyntheticBrowser.viaEdge(home, world);
    resumedBrowser.life = RESTARTED_LIFE;
    browsers.push(resumedBrowser);
    await resumedBrowser.connect();
    const resumed = await mcp.facts("browser_read", { tab: kept });
    assert.equal(resumed.status, "succeeded", "the replacing service resumes the session's routed tab");
    assert.equal(resumed.facts.url, ROUTED_URL, "the resumed tab reads at its live address");
    console.log("PASS a replace closes nothing and the next service resumes the session's tab in the same life");

    // Quit releases the session's tab and holds.
    const quitting = await home.control({ op: controlOp.quit, reason: quitReason.quit });
    assert.deepEqual(quitting.value, { quitting: true });
    await until(async () => !(await home.status()), "the service is gone after quit", 20_000);
    assert.equal(existsSync(home.file(...QUIT_LATCH)), true, "quit leaves its latch");
    await resumedBrowser.waitFor(BROWSER_ABSENT, 15_000, "the browser edge reports the quit");
    assert.equal(resumedBrowser.requested("close_tab").some((request) => request.primitive.args.tab_id === keptId), true,
      "the quit closes the session's tab before its browser goodbye");
    const afterQuit = SyntheticBrowser.viaEdge(home, world);
    browsers.push(afterQuit);
    afterQuit.send(afterQuit.helloFrame());
    await afterQuit.waitFor(BROWSER_ABSENT, 20_000, "a new browser start is told the service is absent");
    await delay(1500);
    assert.equal(await home.status(), null, "a browser start after quit starts no service");
    afterQuit.close();
    const restarted = await mcp.request("tools/list");
    assert.equal(restarted.result.tools.length, catalog.length, "the next MCP request starts the service");
    const revived = await home.serviceUp();
    assert.equal(revived.started_by, startReason.agent);
    assert.equal(existsSync(home.file(...QUIT_LATCH)), false, "an agent start clears the latch");
    await within(resumedBrowser.ended, 20_000, "the browser edge that heard the quit exits when a service accepts");
    console.log("PASS Quit holds a browser start back and the next MCP request starts the service");

    // A browser and a heartbeat on the revived service.
    const last = SyntheticBrowser.viaEdge(home, world);
    browsers.push(last);
    await last.connect();
    const listedAgain = await mcp.facts("browser_tabs", { action: "list" });
    assert.equal(listedAgain.status, "succeeded");
    const forgotten = await mcp.facts("browser_read", { tab: kept });
    assert.notEqual(forgotten.status, "succeeded", "the quit handed the session a token without its tab");
    assert.equal(last.requested("call").length, 0, "nothing reads a tab the quit released");
    await until(() => last.heartbeats > 0, "the service sends a heartbeat", 40_000);
    const records = home.auditRecords();
    assert.equal(records.some((record) => record.tool === "browser_read" && record.allowed === true), true, "a read is in the audit");
    assert.equal(records.some((record) => record.tool === "browser_navigate" && record.allowed === false), true, "the refusal is in the audit");
    console.log("PASS a heartbeat is answered and the audit holds the work");
    mcp.child.stdin.end();
  } finally {
    for (const browser of browsers) browser.close();
    await home.cleanup();
  }
}

/** Whether the process `pid` has a handler of its own for `signal`, as its SigCgt mask says. Linux only. */
function catchesSignal(pid, signal) {
  try {
    const mask = readFileSync(`/proc/${Number(pid)}/status`, "utf8").match(/^SigCgt:\s*([0-9a-f]+)$/m)?.[1];
    return mask !== undefined && ((BigInt(`0x${mask}`) >> BigInt(signal - 1)) & 1n) === 1n;
  } catch {
    return false;
  }
}

/** The ids of the running processes whose image is the edge or the service of `home`. Linux only. */
function homeProcesses(home) {
  return readdirSync("/proc").filter((name) => /^\d+$/.test(name)).map(Number).filter((pid) => {
    const image = imagePath(pid);
    return image !== null && [home.edge, home.service].some((own) => samePath(own, image));
  });
}

/**
 * Send SIGTERM to `pid` once its session listener is installed. A signal sent earlier would meet
 * the default action and prove nothing.
 */
async function signalSessionEnd(pid, label) {
  await until(() => catchesSignal(pid, constants.signals[SESSION_END_SIGNAL]), `${label} installs its session listener`, LISTENER_READY_MS);
  process.kill(pid, SESSION_END_SIGNAL);
}

// SIGTERM is the end of the session on Linux (ADR-0199 Decision 21); Windows sends the end-session
// messages instead, and tests/windows-desktop-journey.ps1 checks them. The home holds no shell, so
// the shell's own SIGTERM is checked live.
async function sessionEnd() {
  const home = createHome({ label: "session-end" });
  try {
    const edgeLeaves = async (edge, label) => {
      await signalSessionEnd(edge.pid, label);
      const ended = await within(edge.exited, EDGE_END_SESSION_MS, `${label} leaves on SIGTERM`);
      assert.deepEqual(ended, { code: 0, signal: null }, `${label} leaves cleanly on SIGTERM: ${edge.output}`);
    };

    // A browser relay starts the service only after the extension's hello, so nothing is left.
    await edgeLeaves(home.start(home.edge, [EXTENSION_ORIGIN]), "the browser edge");
    await delay(RESTART_SETTLE_MS);
    assert.deepEqual(homeProcesses(home), [], "the browser edge starts nothing once the session ends");
    console.log("PASS the browser edge leaves on SIGTERM and starts nothing");

    // The MCP relay starts the service at its own start; its end leaves that service as it was.
    const mcp = home.start(home.edge, ["mcp"]);
    const status = await home.serviceUp();
    // The MCP edge retries its connection and starts the service on each failed attempt, so a
    // slow first start leaves losing starters behind; they must be gone before the signal.
    await until(() => {
      const running = homeProcesses(home).filter((pid) => pid !== mcp.pid);
      return running.length === 1 && running[0] === status.pid;
    }, "the losing starters leave", LEAVING_LOCK_WAIT_MS + LEAVING_LOCK_MARGIN_MS);
    await edgeLeaves(mcp, "the MCP edge");
    await delay(RESTART_SETTLE_MS);
    assert.deepEqual(homeProcesses(home), [status.pid], "the MCP edge restarts nothing once the session ends");
    assert.equal((await home.status())?.instance, status.instance, "the service the MCP edge started runs on");
    console.log("PASS the MCP edge leaves on SIGTERM and restarts nothing");

    // The service hears SIGTERM itself and leaves, and nothing of the home starts again.
    const image = imagePath(status.pid);
    assert.ok(image && samePath(home.service, image), `process ${status.pid} is this home's service, not ${image}`);
    await signalSessionEnd(status.pid, "the service");
    await until(() => !homeProcesses(home).includes(status.pid), "the service leaves on SIGTERM", SERVICE_END_SESSION_MS);
    await delay(RESTART_SETTLE_MS);
    assert.deepEqual(homeProcesses(home), [], "the service restarts nothing once the session ends");
    assert.equal(await home.status(), null, "no service answers after the session ended");
    console.log("PASS the service leaves on SIGTERM and nothing starts again");
  } finally {
    await home.cleanup();
  }
}

assert.equal(vocabulary.relay, 4);
await racingStarts();
await mainFlow();
if (process.platform === "linux") await sessionEnd();
console.log("process journey ok: relay 4, the edge, restart continuity, replace, Quit, racing starts and the session's end");
