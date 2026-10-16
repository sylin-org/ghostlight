// Local resilience of the real service on relay protocol 4, in a test home: a private endpoint,
// peers that are silent, torn, malformed, oversized or of a revision it does not serve, an
// ordinary burst, human Pause, Resume and Stop from the browser, a repeated request id, queued
// work that is cancelled, expires, is refused for capacity or is stopped, and a client that stops
// reading. Synthetic clients and a synthetic browser speak bare relay 4 streams; nothing here
// touches the installed Ghostlight.
import assert from "node:assert/strict";
import { statSync } from "node:fs";
import { createHome, connectRelay, delay, opening, openRelay, refusalCode, role, startReason, until, vocabulary, within } from "./lib/home.mjs";
import { RelayMcp, SyntheticBrowser, World, structured } from "./lib/peers.mjs";

const RETIRE_LIMIT_MS = vocabulary.opening_deadline_ms + 3_000;
const BURST = 12;
const STALL_LIMIT_MS = 30_000;
const HELD_MS = 10_000;
const ANSWER_MS = 10_000;
const NO_CHANGE_MS = 500;
const QUEUED = 31;
const EXPIRING = 1;
const EXPIRY_MS = 100;
const QUEUED_PATIENCE_MS = 30_000;
const REASON_CAPACITY = "capacity";
const REASON_DEADLINE = "deadline";
const STATUS_CANCELLED = "cancelled";
const STATUS_BLOCKED = "blocked";
const EFFECT_NONE = "none";
const EFFECT_UNKNOWN = "unknown";
const UNIX_RUN_MODE = 0o700;
const UNIX_SOCKET_MODE = 0o600;
const MODE_MASK = 0o777;

const home = createHome({ label: "resilience" });
const world = new World();
const strangers = [];

/** Open a tab for `peer` on `url` and return its handle. */
async function tabFor(peer, url) {
  const opened = await peer.facts("browser_navigate", { url });
  assert.equal(opened.status, "succeeded", JSON.stringify(opened));
  return opened.facts.tab;
}

/** The parameters of a `tools/call` of `name` with `args`. */
const toolCall = (name, args) => ({ name, arguments: args });

/**
 * The world messages of the `call` requests `browser` received from request `from` on, in order.
 * A read is a preparation call that lets the page settle, then the read itself, so what one read
 * costs the browser is measured from a read, never assumed.
 */
const callKinds = (browser, from) => browser.requests.slice(from)
  .filter((request) => request.primitive.name === "call")
  .map((request) => request.primitive.args.calls.map((entry) => entry.message.kind).join("+"));

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
  const handleA = await tabFor(a, "https://example.com/");
  const handleB = await tabFor(b, "https://example.org/");

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
  const older = await connectRelay(home.path);
  older.send({ ...opening(role.mcp), relay: vocabulary.relay - 1 });
  strangers.push(silent, torn, partial, malformed, oversized, older);
  const refusal = await older.next((frame) => "accepted" in frame, RETIRE_LIMIT_MS, "the refusal of an older relay");
  assert.equal(refusal.accepted, false);
  assert.equal(refusal.code, refusalCode.unsupported);
  assert.ok(refusal.say, "a refusal says why");
  assert.equal((await b.facts("browser_read", { tab: handleB })).status, "succeeded");
  await within(Promise.all(strangers.map((stranger) => stranger.ended)), RETIRE_LIMIT_MS,
    "abandoned, torn, malformed, oversized and older-revision peers retire");
  assert.equal((await a.facts("policy_explain")).status, "succeeded");
  // A later revision is served: the service accepts every revision from its own.
  const later = await openRelay(home.path, opening(role.mcp, { channel: "mcp", relay: vocabulary.relay + 1 }));
  strangers.push(later.stream);
  assert.equal(later.reply.accepted, true, JSON.stringify(later.reply));
  later.stream.close();
  console.log("PASS abandoned, torn, malformed, oversized and older-revision peers retire; a later revision and healthy clients stay served");

  const burst = await Promise.all(Array.from({ length: BURST }, () => a.facts("browser_read", { tab: handleA })));
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
    const result = await b.facts("browser_read", { tab: handleB });
    assert.notEqual(result.status, "succeeded", `${label}: work does not run`);
    assert.equal(result.effect, EFFECT_NONE, `${label}: nothing happened`);
    assert.equal(browser.requested("call").length, calls, `${label}: nothing reached the browser`);
    return result;
  }
  await control("hold", "held");
  await refused("held");
  await control("resume", "active");
  assert.equal((await b.facts("browser_read", { tab: handleB })).status, "succeeded");
  console.log("PASS Pause stops dispatch and Resume restores it");

  await control("end_session", "ended");
  const stopped = await refused("stopped");
  assert.equal(stopped.status, STATUS_BLOCKED);
  // Resume after Stop is not a change, so the service publishes no new state.
  const statesBeforeResume = browser.controlStates().length;
  browser.requestControl("resume");
  await delay(NO_CHANGE_MS);
  assert.equal(browser.controlStates().length, statesBeforeResume, "Resume after Stop publishes no state change");
  assert.equal(browser.controlStates().at(-1), "ended");
  await refused("resumed after Stop");
  assert.equal((await a.facts("policy_explain")).status, "succeeded", "diagnostics stay available after Stop");
  await control("start_session", "active");
  assert.equal((await b.facts("browser_read", { tab: handleB })).status, "succeeded");
  console.log("PASS Stop is terminal until an explicit new session, and nothing is replayed");

  // A repeated request id is refused and the original stays cancellable. Work that may have acted
  // ends uncertain and is not replayed.
  const c = await RelayMcp.open(home);
  await c.initialize("resilience c");
  const handleC = await tabFor(c, "https://example.net/");
  const navigations = browser.requested("navigate").length;
  browser.holdNext("navigate");
  const navigation = toolCall("browser_navigate", { tab: handleC, url: "https://example.net/second" });
  const original = c.begin("tools/call", navigation);
  await until(() => browser.held.length === 1, "the navigation reaches the browser", HELD_MS);
  c.begin("tools/call", navigation, original);
  const repeated = await c.response(original, ANSWER_MS);
  assert.notEqual(repeated.error, undefined, "a repeated id is refused");
  assert.equal(repeated.result, undefined);
  c.notify("notifications/cancelled", { requestId: original });
  const uncertain = structured(await c.response(original, ANSWER_MS));
  assert.equal(uncertain.effect, EFFECT_UNKNOWN);
  assert.equal(uncertain.repeat_safe, false);
  assert.equal(browser.requested("navigate").length, navigations + 1, "the repeated request never reached the browser");
  browser.held.splice(0);
  console.log("PASS a repeated id is refused, the original is cancelled, and uncertain work is not replayed");

  // Queued work behind a call in flight: refused for capacity, cancelled, expired, then stopped.
  const d = await RelayMcp.open(home);
  await d.initialize("resilience d");
  const handleD = await tabFor(d, "https://example.edu/");
  const healthyStart = browser.requests.length;
  assert.equal((await d.facts("browser_read", { tab: handleD })).status, "succeeded");
  const oneRead = callKinds(browser, healthyStart);
  assert.ok(oneRead.includes("read_text"), `a healthy read reaches the page: ${JSON.stringify(oneRead)}`);
  browser.holdNext("navigate");
  const inFlight = d.begin("tools/call", toolCall("browser_navigate", { tab: handleD, url: "https://example.edu/second" }));
  await until(() => browser.held.length === 1, "the call in flight reaches the browser", HELD_MS);
  const queued = Array.from({ length: QUEUED }, (_, index) => d.begin("tools/call",
    toolCall("browser_read", { tab: handleD, timeout_ms: index === EXPIRING ? EXPIRY_MS : QUEUED_PATIENCE_MS })));
  const overflow = structured(await d.request("tools/call", toolCall("browser_read", { tab: handleD })));
  assert.equal(overflow.facts.reason, REASON_CAPACITY);
  assert.equal(overflow.effect, EFFECT_NONE);
  assert.equal(overflow.repeat_safe, true);
  assert.equal((await d.facts("policy_explain")).status, "succeeded", "controls stay usable under saturation");
  assert.equal((await b.facts("browser_read", { tab: handleB })).status, "succeeded", "another workspace stays usable under saturation");
  d.notify("notifications/cancelled", { requestId: queued[0] });
  const cancelled = structured(await d.response(queued[0], ANSWER_MS));
  assert.equal(cancelled.status, STATUS_CANCELLED);
  assert.equal(cancelled.effect, EFFECT_NONE);
  const expired = structured(await d.response(queued[EXPIRING], ANSWER_MS));
  assert.equal(expired.facts.reason, REASON_DEADLINE);
  assert.equal(expired.effect, EFFECT_NONE);
  console.log("PASS a full queue refuses with no effect while controls and another workspace are served; queued cancellation and expiry have no effect");

  // Stop settles the queue quietly: queued work ends with no effect, the call in flight stays
  // uncertain, and nothing is replayed when a new session begins.
  const reads = browser.requested("call").length;
  const navigates = browser.requested("navigate").length;
  await control("end_session", "ended");
  d.notify("notifications/cancelled", { requestId: inFlight });
  const stoppedInFlight = structured(await d.response(inFlight, ANSWER_MS));
  assert.equal(stoppedInFlight.effect, EFFECT_UNKNOWN);
  assert.equal(stoppedInFlight.repeat_safe, false);
  for (const id of queued.filter((_, index) => index !== 0 && index !== EXPIRING)) {
    const settled = structured(await d.response(id, ANSWER_MS));
    assert.equal(settled.status, STATUS_BLOCKED);
    assert.equal(settled.effect, EFFECT_NONE);
  }
  assert.equal(browser.requested("call").length, reads, "Stop sends no queued work to the browser");
  browser.held.splice(0);
  await control("start_session", "active");
  assert.equal((await d.facts("browser_read", { tab: handleD })).status, "succeeded");
  assert.deepEqual(callKinds(browser, 0).slice(reads), oneRead, "a new session sends only what the one new read sends");
  assert.equal(browser.requested("navigate").length, navigates, "the call in flight is not sent again");
  console.log("PASS Stop ends queued work with no effect, leaves the call in flight uncertain and replays nothing");

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
  assert.equal((await b.facts("browser_read", { tab: handleB })).status, "succeeded");
  const fresh = await RelayMcp.open(home);
  await fresh.initialize("resilience fresh");
  assert.equal((await fresh.facts("policy_explain")).status, "succeeded");
  console.log("PASS a stalled reader is retired alone and later work succeeds");
  for (const peer of [a, b, c, d, fresh]) peer.close();
  browser.close();
  console.log("local resilience journey ok: relay 4, one service, synthetic peers");
} finally {
  for (const stranger of strangers) stranger.close();
  await home.cleanup();
}
