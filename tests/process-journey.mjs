// The real executables on relay protocol 4: an MCP session through `ghostlight mcp`, a synthetic
// browser through `ghostlight chrome-extension://<id>/`, a service killed and restarted under
// both, Quit holding the next browser start back, ten racing starts leaving one service, and, on
// Linux, the edge and the service each leaving on SIGTERM, the session's end there.
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

async function racingStarts() {
  const race = createHome({ label: "race" });
  try {
    const contenders = Array.from({ length: CONTENDERS }, () =>
      race.start(race.service, ["serve", "--reason", startReason.agent], { stdio: ["ignore", "ignore", "pipe"] }));
    const first = await race.serviceUp();
    await until(() => contenders.filter((child) => child.exitCode === null && child.signalCode === null).length === 1,
      "all but one contender exits", 40_000);
    const survivors = contenders.filter((child) => child.exitCode === null && child.signalCode === null);
    for (const child of contenders.filter((candidate) => !survivors.includes(candidate))) {
      assert.equal(child.exitCode, 0, `a losing start exits quietly: ${child.output}`);
    }
    await delay(1500);
    const later = await race.status();
    assert.equal(later.instance, first.instance, "the instance is stable");
    assert.equal(later.pid, survivors[0].pid, "the survivor is the service");
    await race.quit();
    await within(survivors[0].exited, 10_000, "the surviving contender exits after quit");
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
    browsers.push(browser);
    const accepted = await browser.connect();
    assert.equal(accepted.control_state, "active");
    assert.ok(accepted.service_epoch);
    const tabs = await mcp.facts("browser_tabs", { action: "list" });
    assert.equal(tabs.status, "succeeded");
    assert.equal(browser.requested("list_tabs").length >= 1, true);
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

    // The service is killed under a call in flight.
    const probe = await RelayMcp.open(home);
    const cutOff = probe.reply.templates[templateKind.cut_off]["tools/call"].result.content;
    probe.close();
    browser.holdNext("call");
    const inFlight = mcp.begin("tools/call", { name: "browser_read", arguments: { tab: handle } });
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

    // Quit holds.
    const quitting = await home.control({ op: controlOp.quit, reason: quitReason.quit });
    assert.deepEqual(quitting.value, { quitting: true });
    await until(async () => !(await home.status()), "the service is gone after quit", 20_000);
    assert.equal(existsSync(home.file(...QUIT_LATCH)), true, "quit leaves its latch");
    await reborn.waitFor(BROWSER_ABSENT, 15_000, "the browser edge reports the quit");
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
    await within(reborn.ended, 20_000, "the browser edge that heard the quit exits when a service accepts");
    console.log("PASS Quit holds a browser start back and the next MCP request starts the service");

    // A browser and a heartbeat on the revived service.
    const last = SyntheticBrowser.viaEdge(home, world);
    browsers.push(last);
    await last.connect();
    const listedAgain = await mcp.facts("browser_tabs", { action: "list" });
    assert.equal(listedAgain.status, "succeeded");
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
console.log("process journey ok: relay 4, the edge, restart continuity, Quit, racing starts and the session's end");
