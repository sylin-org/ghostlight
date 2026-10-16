// Local resilience of the real service on relay protocol 4, in a test home: a private endpoint,
// peers that are silent, torn, malformed or oversized, an ordinary burst, human Pause, Resume and
// Stop from the browser, and a client that stops reading. Synthetic clients and a synthetic
// browser speak bare relay 4 streams; nothing here touches the installed Ghostlight.
import assert from "node:assert/strict";
import { statSync } from "node:fs";
import { createHome, connectRelay, delay, opening, refusalCode, role, startReason, until, vocabulary, within } from "./lib/home.mjs";
import { RelayMcp, SyntheticBrowser, World } from "./lib/peers.mjs";

const RETIRE_LIMIT_MS = vocabulary.opening_deadline_ms + 3_000;
const BURST = 12;
const STALL_LIMIT_MS = 30_000;
const UNIX_RUN_MODE = 0o700;
const UNIX_SOCKET_MODE = 0o600;
const MODE_MASK = 0o777;

const home = createHome({ label: "resilience" });
const world = new World();
const strangers = [];
try {
  home.start(home.service, ["serve", "--reason", startReason.agent], { stdio: ["ignore", "ignore", "pipe"] });
  await home.serviceUp();
  if (process.platform !== "win32") {
    assert.equal(statSync(home.file(vocabulary.files.run_directory)).mode & MODE_MASK, UNIX_RUN_MODE, "the run directory is private");
    assert.equal(statSync(home.file(vocabulary.files.run_directory, vocabulary.files.relay_socket)).mode & MODE_MASK, UNIX_SOCKET_MODE,
      "the socket is private");
    console.log("PASS the endpoint is private to its owner");
  }

  const a = await RelayMcp.open(home);
  const b = await RelayMcp.open(home);
  await a.initialize("resilience a");
  await b.initialize("resilience b");
  const browser = await SyntheticBrowser.viaRelay(home, world);
  await browser.connect();
  const opened = await a.facts("browser_navigate", { url: "https://example.com/" });
  assert.equal(opened.status, "succeeded", JSON.stringify(opened));
  const handle = opened.facts.tab;

  // Peers that never finish an opening, or send what is not one, are retired.
  const silent = await connectRelay(home.path);
  const torn = await connectRelay(home.path);
  torn.sendRaw(Buffer.from([0x20, 0x00]));
  const partial = await connectRelay(home.path);
  partial.sendRaw(Buffer.concat([Buffer.from([0x64, 0x00, 0x00, 0x00]), Buffer.from("{\"relay\"")]));
  const malformed = await connectRelay(home.path);
  malformed.sendRaw(Buffer.concat([Buffer.from([0x07, 0x00, 0x00, 0x00]), Buffer.from("{broken")]));
  const oversized = await connectRelay(home.path);
  const tooLarge = Buffer.alloc(4);
  tooLarge.writeUInt32LE(vocabulary.frame.max_bytes + 1);
  oversized.sendRaw(tooLarge);
  const newer = await connectRelay(home.path);
  newer.send({ ...opening(role.mcp), relay: vocabulary.relay + 1 });
  strangers.push(silent, torn, partial, malformed, oversized, newer);
  const refusal = await newer.next((frame) => "accepted" in frame, RETIRE_LIMIT_MS, "the refusal of a newer relay");
  assert.equal(refusal.accepted, false);
  assert.equal(refusal.code, refusalCode.unsupported);
  assert.ok(refusal.say, "a refusal says why");
  assert.equal((await b.facts("browser_read", { tab: handle })).status, "succeeded");
  await within(Promise.all(strangers.map((stranger) => stranger.ended)), RETIRE_LIMIT_MS, "abandoned, torn, malformed and oversized peers retire");
  assert.equal((await a.facts("policy_explain")).status, "succeeded");
  console.log("PASS abandoned, torn, malformed and oversized peers retire; healthy clients stay served");

  const burst = await Promise.all(Array.from({ length: BURST }, () => a.facts("browser_read", { tab: handle })));
  assert.equal(burst.every((result) => result.status === "succeeded"), true);
  console.log("PASS an ordinary burst completes without intervention");

  // The person's controls reach the service through the browser.
  async function control(intent, state) {
    const seen = browser.controlStates().length;
    browser.requestControl(intent);
    await until(() => browser.controlStates().length > seen && browser.controlStates().at(-1) === state, `${intent} reaches ${state}`, 5_000);
  }
  async function refused(label) {
    const calls = browser.requested("call").length;
    const result = await b.facts("browser_read", { tab: handle });
    assert.notEqual(result.status, "succeeded", `${label}: work does not run`);
    assert.equal(result.effect, "none", `${label}: nothing happened`);
    assert.equal(browser.requested("call").length, calls, `${label}: nothing reached the browser`);
    return result;
  }
  await control("hold", "held");
  await refused("held");
  await control("resume", "active");
  assert.equal((await b.facts("browser_read", { tab: handle })).status, "succeeded");
  console.log("PASS Pause stops dispatch and Resume restores it");

  await control("end_session", "ended");
  const stopped = await refused("stopped");
  assert.equal(stopped.status, "blocked");
  await control("resume", "ended");
  await refused("resumed after Stop");
  assert.equal((await a.facts("policy_explain")).status, "succeeded", "diagnostics stay available after Stop");
  await control("start_session", "active");
  assert.equal((await b.facts("browser_read", { tab: handle })).status, "succeeded");
  console.log("PASS Stop is terminal until an explicit new session, and nothing is replayed");

  // A client that stops reading is retired alone.
  const slow = await RelayMcp.open(home);
  await slow.initialize("resilience slow");
  slow.stream.socket.pause();
  const listRequest = { kind: "message", line: JSON.stringify({ jsonrpc: "2.0", id: 1000, method: "tools/list" }) };
  await until(() => {
    if (slow.stream.closed) return true;
    slow.stream.send(listRequest);
    return false;
  }, "a stalled reader is retired", STALL_LIMIT_MS);
  await delay(100);
  assert.equal((await b.facts("browser_read", { tab: handle })).status, "succeeded");
  const fresh = await RelayMcp.open(home);
  await fresh.initialize("resilience fresh");
  assert.equal((await fresh.facts("policy_explain")).status, "succeeded");
  console.log("PASS a stalled reader is retired alone and later work succeeds");
  for (const peer of [a, b, fresh]) peer.close();
  browser.close();
  console.log("local resilience journey ok: relay 4, one service, synthetic peers");
} finally {
  for (const stranger of strangers) stranger.close();
  await home.cleanup();
}
