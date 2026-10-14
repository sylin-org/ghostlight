// Unit tests for the DevTools pipe client with a fake child. No Chrome is started.
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { PassThrough } from "node:stream";
import test from "node:test";
import {
  CdpPipe, SCRATCH_PREFIX, chromeArguments, createScratch, installScratchNativeHost, nativeHostManifestSource, removeScratch
} from "./cdp-pipe.mjs";

const NUL = "\0";

// A child whose fd 3 records commands and whose fd 4 the test writes responses into.
function fakeChild() {
  const child = new EventEmitter();
  child.stdio = [null, null, null, new PassThrough(), new PassThrough()];
  child.exitCode = null;
  child.signalCode = null;
  child.sent = [];
  child.stdio[3].on("data", (chunk) => {
    for (const text of chunk.toString("utf8").split(NUL).filter(Boolean)) child.sent.push(JSON.parse(text));
  });
  child.reply = (message) => child.stdio[4].write(JSON.stringify(message) + NUL);
  child.exit = (code = 0) => { child.exitCode = code; child.emit("exit", code, null); };
  child.kill = () => child.exit(137);
  return child;
}
const tick = () => new Promise((done) => setImmediate(done));

test("commands are written as NUL-terminated JSON with increasing numeric ids", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const first = pipe.send("Browser.getVersion");
  const second = pipe.send("Target.getTargets", { filter: [] }, "session-1");
  await tick();
  assert.deepEqual(child.sent, [
    { id: 1, method: "Browser.getVersion", params: {} },
    { id: 2, method: "Target.getTargets", params: { filter: [] }, sessionId: "session-1" }
  ]);
  child.reply({ id: 1, result: { product: "Chrome" } });
  child.reply({ id: 2, result: { targetInfos: [] } });
  assert.deepEqual(await first, { product: "Chrome" });
  assert.deepEqual(await second, { targetInfos: [] });
});

test("responses match by id even when they arrive out of order", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const a = pipe.send("A");
  const b = pipe.send("B");
  child.reply({ id: 2, result: { name: "b" } });
  child.reply({ id: 1, result: { name: "a" } });
  assert.deepEqual(await a, { name: "a" });
  assert.deepEqual(await b, { name: "b" });
});

test("one message split across chunks and several messages in one chunk both decode", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const a = pipe.send("A");
  const b = pipe.send("B");
  const whole = JSON.stringify({ id: 1, result: { value: "x".repeat(50) } }) + NUL;
  child.stdio[4].write(whole.slice(0, 20));
  await tick();
  child.stdio[4].write(whole.slice(20) + JSON.stringify({ id: 2, result: { value: "y" } }) + NUL);
  assert.deepEqual(await a, { value: "x".repeat(50) });
  assert.deepEqual(await b, { value: "y" });
});

test("a multibyte character split between chunks decodes intact", async () => {
  const ACCENTED = "caf" + String.fromCharCode(0xe9);
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const a = pipe.send("A");
  const bytes = Buffer.from(JSON.stringify({ id: 1, result: { text: ACCENTED } }) + NUL, "utf8");
  const cut = bytes.indexOf(0xc3) + 1;
  child.stdio[4].write(bytes.subarray(0, cut));
  await tick();
  child.stdio[4].write(bytes.subarray(cut));
  assert.deepEqual(await a, { text: ACCENTED });
});

test("a protocol error rejects only its own command", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const bad = pipe.send("Nope");
  const good = pipe.send("Fine");
  child.reply({ id: 1, error: { code: -32601, message: "not found" } });
  child.reply({ id: 2, result: {} });
  await assert.rejects(bad, /Nope: .*-32601/);
  assert.deepEqual(await good, {});
});

test("events are emitted by method name and carry the session id", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const seen = new Promise((done) => pipe.once("Page.javascriptDialogOpening", (params, sessionId) => done({ params, sessionId })));
  child.reply({ method: "Page.javascriptDialogOpening", params: { message: "hi" }, sessionId: "s" });
  assert.deepEqual(await seen, { params: { message: "hi" }, sessionId: "s" });
});

test("a response with an unknown id and an unparseable message are ignored", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const malformed = new Promise((done) => pipe.once("malformed", done));
  const a = pipe.send("A");
  child.reply({ id: 99, result: {} });
  child.stdio[4].write("{not json" + NUL);
  assert.equal(await malformed, "{not json");
  child.reply({ id: 1, result: { ok: true } });
  assert.deepEqual(await a, { ok: true });
});

test("a command that is never answered times out and frees its id", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  await assert.rejects(pipe.send("Slow", {}, undefined, 20), /Timed out after 20 ms: Slow/);
  assert.equal(pipe.pending.size, 0);
});

test("pending commands fail when Chrome exits, and later commands are refused", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const pending = pipe.send("A");
  child.exit(1);
  await assert.rejects(pending, /Chrome exited \(1\)/);
  await assert.rejects(pipe.send("B"), /closed/);
});

test("loadUnpacked returns the extension id and refuses a result without one", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  const loaded = pipe.loadUnpacked("/repo/extension");
  await tick();
  assert.deepEqual(child.sent[0], { id: 1, method: "Extensions.loadUnpacked", params: { path: "/repo/extension" } });
  child.reply({ id: 1, result: { id: "cjcmhepmagomefjggkcohdbfemacojoa" } });
  assert.equal(await loaded, "cjcmhepmagomefjggkcohdbfemacojoa");
  const empty = pipe.loadUnpacked("/repo/extension");
  child.reply({ id: 2, result: {} });
  await assert.rejects(empty, /returned no id/);
});

test("close sends Browser.close and waits for the exit without killing", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child);
  let killed = false;
  child.kill = () => { killed = true; child.exit(137); };
  const closing = pipe.close({ exitTimeoutMs: 1000 });
  await tick();
  assert.equal(child.sent[0].method, "Browser.close");
  child.reply({ id: 1, result: {} });
  child.exit(0);
  assert.equal(await closing, true);
  assert.equal(killed, false);
});

test("close kills only its own child when Chrome ignores Browser.close", async () => {
  const child = fakeChild();
  const pipe = new CdpPipe(child, { timeoutMs: 20 });
  let killed = false;
  child.kill = () => { killed = true; child.exit(137); };
  assert.equal(await pipe.close({ exitTimeoutMs: 60, commandTimeoutMs: 20 }), true);
  assert.equal(killed, true);
});

test("chrome arguments never include --load-extension and end at about:blank", () => {
  const args = chromeArguments("/tmp/installed-x", ["--window-size=1280,900"]);
  assert.ok(args.includes("--user-data-dir=/tmp/installed-x"));
  assert.ok(args.includes("--remote-debugging-pipe"));
  assert.ok(args.includes("--enable-unsafe-extension-debugging"));
  assert.ok(!args.some((arg) => arg.startsWith("--load-extension")));
  assert.equal(args.at(-1), "about:blank");
});

test("scratch directories carry the installed prefix and removal refuses anything else", async () => {
  const scratch = createScratch();
  assert.ok(scratch.includes(SCRATCH_PREFIX));
  writeFileSync(join(scratch, "marker"), "x");
  await assert.rejects(removeScratch(tmpdir()), /Refusing/);
  await assert.rejects(removeScratch(join(tmpdir(), "other-dir")), /Refusing/);
  await removeScratch(scratch);
  assert.equal(existsSync(scratch), false);
});

test("the Linux manifest is found under the ghostlight home first and copied into the scratch profile", () => {
  const home = mkdtempSync(join(tmpdir(), "cdp-pipe-home-"));
  const manifests = join(home, ".ghostlight", "manifests");
  mkdirSync(manifests, { recursive: true });
  writeFileSync(join(manifests, "org.sylin.ghostlight.json"), '{"name":"org.sylin.ghostlight"}');
  const scratch = createScratch();
  const copy = installScratchNativeHost(scratch, { platform: "linux", home, env: {} });
  assert.equal(copy, join(scratch, "NativeMessagingHosts", "org.sylin.ghostlight.json"));
  assert.equal(readFileSync(copy, "utf8"), '{"name":"org.sylin.ghostlight"}');
  rmSync(home, { recursive: true, force: true });
  rmSync(scratch, { recursive: true, force: true });
});

test("the browser config directory is the fallback, a missing manifest is an error, Windows copies nothing", () => {
  const home = mkdtempSync(join(tmpdir(), "cdp-pipe-home-"));
  assert.throws(() => nativeHostManifestSource({ platform: "linux", home, env: {} }), /No user-level native host manifest/);
  const config = join(home, "xdg", "google-chrome", "NativeMessagingHosts");
  mkdirSync(config, { recursive: true });
  writeFileSync(join(config, "org.sylin.ghostlight.json"), "{}");
  assert.equal(nativeHostManifestSource({ platform: "linux", home, env: { XDG_CONFIG_HOME: join(home, "xdg") } }),
    join(config, "org.sylin.ghostlight.json"));
  assert.equal(nativeHostManifestSource({ platform: "win32", home, env: {} }), null);
  assert.equal(installScratchNativeHost(join(home, "unused"), { platform: "win32", home, env: {} }), null);
  rmSync(home, { recursive: true, force: true });
});
