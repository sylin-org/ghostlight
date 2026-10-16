// The real executables on relay protocol 4: an MCP session through `ghostlight mcp`, a synthetic
// browser through `ghostlight chrome-extension://<id>/`, a service killed and restarted under
// both, Quit holding the next browser start back, and ten racing starts leaving one service.
// Every process runs in a fresh test home under the temp directory; nothing touches the real one.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import {
  QUIT_LATCH, buildVersion, controlOp, createHome, delay, openPolicy, quitReason, repository, startReason, templateKind,
  until, vocabulary, within
} from "./lib/home.mjs";
import { COLD_REQUEST_MS, McpEdge, RelayMcp, SyntheticBrowser, World, structured } from "./lib/peers.mjs";

const catalog = JSON.parse(readFileSync(join(repository, "docs", "1.0", "catalog.json"), "utf8"));
const DENIED_HOST = "blocked.localhost";
const BROWSER_ABSENT = (frame) => frame.kind === "backend_unavailable" && frame.service === "absent";
const CONTENDERS = 10;

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

assert.equal(vocabulary.relay, 4);
await racingStarts();
await mainFlow();
console.log("process journey ok: relay 4, the edge, restart continuity, Quit and racing starts");
