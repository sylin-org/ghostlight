// The browser rig's pure parts: the extension id, the worker prelude and the loopback bridge. The
// bridge is driven with a stand-in edge, a node program that echoes native-messaging frames, so
// no Ghostlight program and no browser starts.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import test from "node:test";
import vm from "node:vm";
import { EdgeBridge, extensionId, extensionIdOf, extensionManifest, extensionOrigin, workerPrelude } from "./browser.mjs";

const ECHO = `
const frames = [];
let buffer = Buffer.alloc(0);
process.stdin.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (buffer.length >= 4 && buffer.length >= 4 + buffer.readUInt32LE(0)) {
    const size = buffer.readUInt32LE(0);
    const frame = JSON.parse(buffer.subarray(4, 4 + size).toString("utf8"));
    buffer = buffer.subarray(4 + size);
    const payload = Buffer.from(JSON.stringify({ kind: frame.kind === "hello" ? "hello_accepted" : "echo", frame }));
    const header = Buffer.alloc(4);
    header.writeUInt32LE(payload.length);
    process.stdout.write(Buffer.concat([header, payload]));
  }
});
process.stdin.on("end", () => process.exit(0));
`;

function standInHome() {
  const started = [];
  return {
    edge: "stand-in-edge",
    started,
    start(executable, args) {
      started.push({ executable, args });
      return spawn(process.execPath, ["-e", ECHO], { stdio: ["pipe", "pipe", "ignore"], windowsHide: true });
    }
  };
}

test("the extension id follows the pinned manifest key", () => {
  assert.equal(extensionId(), "cjcmhepmagomefjggkcohdbfemacojoa");
  assert.equal(extensionIdOf(extensionManifest().key), extensionId());
  assert.equal(extensionOrigin(), "chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/");
});

test("the worker prelude is a script that opens ports on the bridge", () => {
  const source = workerPrelude("http://127.0.0.1:9/token");
  assert.doesNotThrow(() => new vm.Script(source));
  assert.ok(!/[^\x00-\x7f]/.test(source));
});

test("a port carries frames in order through a started edge and reports its end", async () => {
  const home = standInHome();
  const bridge = new EdgeBridge(home, "chrome-extension://stand-in/");
  const base = await bridge.listen();
  const commands = [];
  const sandbox = {
    fetch,
    chrome: {
      debugger: { sendCommand: async (target, method) => { commands.push(method); return {}; } },
      runtime: {}
    }
  };
  vm.createContext(sandbox);
  try {
    vm.runInContext(workerPrelude(base), sandbox);
    const port = sandbox.chrome.runtime.connectNative("org.sylin.ghostlight");
    const received = [];
    let disconnected = 0;
    port.onMessage.addListener((frame) => received.push(frame));
    port.onDisconnect.addListener(() => { disconnected += 1; });
    port.postMessage({ kind: "hello" });
    port.postMessage({ kind: "event", n: 1 });
    port.postMessage({ kind: "event", n: 2 });
    const until = async (test) => { for (let tries = 0; tries < 200 && !test(); tries++) await new Promise((done) => setTimeout(done, 25)); };
    await until(() => received.length === 3);
    assert.deepEqual(received.map((frame) => frame.kind), ["hello_accepted", "echo", "echo"]);
    assert.deepEqual(received.slice(1).map((frame) => frame.frame.n), [1, 2]);
    assert.equal(bridge.accepted, 1);
    assert.deepEqual(home.started, [{ executable: "stand-in-edge", args: ["chrome-extension://stand-in/"] }]);
    await sandbox.chrome.debugger.sendCommand({ tabId: 1 }, "Runtime.evaluate");
    assert.deepEqual(commands, ["Runtime.evaluate"]);
    let held = 0;
    sandbox.__journeyBeforeCommand = async () => { held += 1; };
    await sandbox.chrome.debugger.sendCommand({ tabId: 1 }, "Page.getFrameTree");
    assert.equal(held, 1);
    port.disconnect();
    assert.throws(() => port.postMessage({ kind: "event" }), /disconnected port/);
    await until(() => bridge.ports.get(1).closed);
    assert.equal(bridge.ports.get(1).closed, true, "disconnect ends the edge behind the port");
    assert.equal(disconnected, 0, "a port closed from this side reports nothing, as in Chrome");

    const second = sandbox.chrome.runtime.connectNative("org.sylin.ghostlight");
    second.onDisconnect.addListener(() => { disconnected += 1; });
    second.postMessage({ kind: "hello" });
    await until(() => bridge.ports.size === 2 && bridge.accepted === 2);
    await fetch(`${base}/2/close`, { method: "POST" });
    await until(() => disconnected === 1);
    assert.equal(disconnected, 1, "an edge that ends reports a disconnect");
  } finally {
    await bridge.close();
  }
});

test("the bridge records the primitives and notices the service sends", () => {
  const bridge = new EdgeBridge(standInHome());
  bridge.observe({ kind: "request", correlation: "c1", primitive: { name: "capture", args: {} } });
  bridge.observe({ kind: "request", correlation: "c2", primitive: { name: "call", args: { calls: [{ message: { kind: "read_text" } }, { message: { kind: "inspect" } }] } } });
  bridge.observe({ kind: "notice", tab_id: 7, signal: { signal: "start" } });
  assert.deepEqual(bridge.sentSince(0), ["capture", "call:read_text", "call:inspect"]);
  assert.deepEqual(bridge.sentSince(1), ["call:read_text", "call:inspect"]);
  assert.equal(bridge.notices.length, 1);
});
