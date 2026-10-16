// Real Chromium with the protocol-4 extension, wired to a test home's service through a real
// browser edge.
//
// A browser journey copies `extension/` into its scratch area and puts a few lines in front of
// the worker. Those lines replace only `chrome.runtime.connectNative`: each port the worker opens
// starts a real `ghostlight chrome-extension://<id>/` of the test home and carries its frames over
// a loopback bridge, because a browser started by a test cannot use the machine's native-messaging
// registration. Everything else is the shipped extension: its worker, its modules, the page code
// the service sends and the debugger it attaches. The bridge records every frame it carries, so a
// journey can say which primitives the service sent and which notices it asked for.
//
// The journey also keeps its own DevTools connection to the browser. It reads pages through that
// connection, never through the extension, and reaches the worker the way a developer inspecting
// the extension would.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { decodeFrames, encodeFrame, repository, until } from "./home.mjs";
import { readDevToolsPort, removeBrowserScratch, waitForChromiumExit } from "./chromium.mjs";
import { McpEdge } from "./peers.mjs";

/** The extension under test. */
export const EXTENSION_DIRECTORY = join(repository, "extension");

/** The worker file the manifest names; the bridge lines go in front of it. */
export const WORKER_FILE = "worker.js";

/** The directory browser scratch areas live in; `removeBrowserScratch` guards it. */
export const SCRATCH_ROOT = join(repository, ".tmp");

/** Chrome for Testing, where the repository's tooling unpacks it. */
export const DEFAULT_TEST_BROWSER = join(SCRATCH_ROOT, "chrome-testing", "chrome-win64", "chrome.exe");

/** The name a journey sets to hook the worker's debugger commands; see `workerPrelude`. */
export const BEFORE_COMMAND_HOOK = "__journeyBeforeCommand";

/** How long a poll waits for a frame before it answers empty, so no request outlives the browser's patience. */
const POLL_HOLD_MS = 20_000;

/** Time the extension has to connect, say hello and have it accepted. */
const CONNECT_MS = 45_000;

/** The browser a journey uses; the extension needs a build that still loads unpacked extensions. */
export function testBrowser() {
  const path = process.env.GHOSTLIGHT_TEST_BROWSER || DEFAULT_TEST_BROWSER;
  assert.ok(existsSync(path), "Set GHOSTLIGHT_TEST_BROWSER to Chrome for Testing (unpacked extensions required).");
  return path;
}

/** The manifest of the extension under test. */
export function extensionManifest() {
  return JSON.parse(readFileSync(join(EXTENSION_DIRECTORY, "manifest.json"), "utf8"));
}

/** The id Chrome gives an extension whose manifest carries `key`: sha256 of the key, first 16 bytes, as a to p. */
export function extensionIdOf(key) {
  const digest = createHash("sha256").update(Buffer.from(key, "base64")).digest("hex").slice(0, 32);
  return Array.from(digest, (digit) => String.fromCharCode(97 + Number.parseInt(digit, 16))).join("");
}

/** The id of the extension under test, from its pinned manifest key. */
export function extensionId() {
  return extensionIdOf(extensionManifest().key);
}

/** The origin the browser edge is started with, as Chrome passes it to a native host. */
export function extensionOrigin(id = extensionId()) {
  return `chrome-extension://${id}/`;
}

/**
 * The lines placed in front of the worker.
 *
 * `connectNative` opens a port on the bridge at `base`. Sends keep their order, a port the bridge
 * reports closed fires `onDisconnect`, and `disconnect()` ends the edge behind the port the way
 * Chrome closes a native host. Debugger commands pass through `BEFORE_COMMAND_HOOK` when a
 * journey sets it, so a journey can count or hold the extension's own commands.
 */
export function workerPrelude(base) {
  return `(() => {
  const base = ${JSON.stringify(base)};
  const debuggerApi = chrome.debugger;
  const sendCommand = debuggerApi.sendCommand.bind(debuggerApi);
  debuggerApi.sendCommand = async (...command) => {
    await globalThis.${BEFORE_COMMAND_HOOK}?.(...command);
    return sendCommand(...command);
  };
  chrome.runtime.connectNative = () => {
    const messageListeners = [];
    const closeListeners = [];
    let live = true;
    let tail = Promise.resolve();
    const opened = fetch(base + "/open", { method: "POST" }).then((reply) => reply.json()).then((body) => body.port);
    const close = () => {
      if (!live) return;
      live = false;
      for (const listener of closeListeners) listener(port);
    };
    const port = {
      onMessage: { addListener: (listener) => messageListeners.push(listener) },
      onDisconnect: { addListener: (listener) => closeListeners.push(listener) },
      postMessage(frame) {
        if (!live) throw new Error("Attempting to use a disconnected port object");
        const body = JSON.stringify(frame);
        tail = tail.then(() => opened).then((id) => fetch(base + "/" + id + "/send", { method: "POST", body })).catch(close);
      },
      disconnect() {
        live = false;
        opened.then((id) => fetch(base + "/" + id + "/close", { method: "POST" })).catch(() => {});
      }
    };
    (async () => {
      try {
        const id = await opened;
        while (live) {
          const reply = await (await fetch(base + "/" + id + "/poll")).json();
          for (const frame of reply.frames) for (const listener of messageListeners) listener(frame, port);
          if (reply.closed) break;
        }
      } catch (_error) { /* the bridge is gone */ }
      close();
    })();
    return port;
  };
})();
`;
}

/** The primitives a world call carries, as `call:<message kind>` names beside the other primitives. */
function sentName(primitive) {
  return primitive.name === "call"
    ? primitive.args.calls.map((call) => `call:${call.message?.kind}`)
    : [primitive.name];
}

/**
 * The loopback bridge between the extension's ports and browser edges of one test home.
 *
 * `POST /open` starts an edge and answers `{ port }`; `POST /<port>/send` writes one frame to the
 * edge; `GET /<port>/poll` answers the frames the edge wrote, with `closed` once it has ended;
 * `POST /<port>/close` ends the edge's input. Every path starts with a secret token.
 */
export class EdgeBridge {
  /** Bridge to the service of `home` as the extension with `origin`. */
  constructor(home, origin = extensionOrigin()) {
    this.home = home;
    this.origin = origin;
    this.token = createHash("sha256").update(`${process.pid}:${Date.now()}:${Math.random()}`).digest("hex").slice(0, 32);
    this.ports = new Map();
    this.nextPort = 1;
    this.accepted = 0;
    this.sent = [];
    this.notices = [];
    this.received = [];
    this.server = createServer((request, response) => {
      this.serve(request, response).catch((error) => {
        response.writeHead(500);
        response.end(String(error?.message ?? error));
      });
    });
  }

  /** Listen on a free loopback port and return the base address the worker prelude uses. */
  async listen() {
    await new Promise((done) => this.server.listen(0, "127.0.0.1", done));
    this.base = `http://127.0.0.1:${this.server.address().port}/${this.token}`;
    return this.base;
  }

  /** Stop listening and end every edge's input. */
  async close() {
    for (const port of this.ports.values()) {
      if (!port.closed) port.child.stdin.end();
      this.wake(port);
    }
    this.server.closeAllConnections();
    await new Promise((done) => this.server.close(done));
  }

  /** The stderr tail of every edge this bridge started, for an error that would otherwise name nothing. */
  edgeOutput() {
    return [...this.ports.values()].map((port) => `edge ${port.id}: ${port.child.output?.trim() || "(no stderr)"}`).join("\n");
  }

  /** The names of the primitives the service sent after `index`, in order; world calls are `call:<kind>`. */
  sentSince(index = 0) {
    return this.sent.slice(index);
  }

  open() {
    const id = this.nextPort++;
    const child = this.home.start(this.home.edge, [this.origin]);
    const port = { id, child, queue: [], waiter: null, closed: false, buffer: Buffer.alloc(0) };
    child.stdout.on("data", (chunk) => {
      const { frames, rest } = decodeFrames(Buffer.concat([port.buffer, chunk]));
      port.buffer = rest;
      for (const frame of frames) {
        this.observe(frame);
        port.queue.push(frame);
      }
      this.wake(port);
    });
    child.once("close", () => {
      port.closed = true;
      this.wake(port);
    });
    this.ports.set(id, port);
    return port;
  }

  observe(frame) {
    this.received.push(frame);
    if (frame.kind === "hello_accepted") this.accepted += 1;
    if (frame.kind === "notice") this.notices.push(frame);
    if (frame.kind === "request") this.sent.push(...sentName(frame.primitive));
  }

  wake(port) {
    const waiter = port.waiter;
    if (!waiter || (!port.queue.length && !port.closed)) return;
    port.waiter = null;
    clearTimeout(waiter.timer);
    this.answer(waiter.response, { frames: port.queue.splice(0), closed: port.closed });
  }

  answer(response, body) {
    response.writeHead(200, { "content-type": "application/json", "access-control-allow-origin": "*" });
    response.end(JSON.stringify(body));
  }

  async serve(request, response) {
    const [token, first, second] = new URL(request.url, "http://bridge").pathname.split("/").filter(Boolean);
    if (token !== this.token) {
      response.writeHead(404);
      response.end();
      return;
    }
    if (first === "open") {
      this.answer(response, { port: this.open().id });
      return;
    }
    const port = this.ports.get(Number(first));
    if (!port) {
      response.writeHead(404);
      response.end();
      return;
    }
    if (second === "poll") {
      if (port.waiter) {
        clearTimeout(port.waiter.timer);
        this.answer(port.waiter.response, { frames: [], closed: false });
      }
      const waiter = { response, timer: setTimeout(() => {
        port.waiter = null;
        this.answer(response, { frames: [], closed: false });
      }, POLL_HOLD_MS) };
      port.waiter = waiter;
      this.wake(port);
      return;
    }
    let body = "";
    for await (const chunk of request) body += chunk;
    if (second === "send" && !port.closed) {
      port.child.stdin.write(encodeFrame(JSON.parse(body)));
    } else if (second === "close") {
      port.child.stdin.end();
    }
    this.answer(response, { ok: true });
  }
}

/** A DevTools request channel: `write` sends one message and `receive` takes every message that comes back. */
export function cdpChannel(write, timeoutMs = 45_000) {
  let next = 0;
  const pending = new Map();
  const listeners = [];
  return {
    send(method, params = {}, sessionId) {
      const id = ++next;
      return new Promise((done, reject) => {
        const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, timeoutMs);
        pending.set(id, { done, reject, timer });
        write({ id, method, params, ...(sessionId ? { sessionId } : {}) });
      });
    },
    onEvent(listener) {
      listeners.push(listener);
    },
    receive(message) {
      if (message.id === undefined) {
        for (const listener of listeners) listener(message);
        return;
      }
      const item = pending.get(message.id);
      if (!item) return;
      pending.delete(message.id);
      clearTimeout(item.timer);
      if (message.error) item.reject(new Error(JSON.stringify(message.error)));
      else item.done(message.result);
    }
  };
}

/** One page of the browser, reached through the journey's own DevTools session. */
export class PageSession {
  constructor(rig, targetId, sessionId) {
    this.rig = rig;
    this.targetId = targetId;
    this.sessionId = sessionId;
  }

  /** Send one DevTools command to the page. */
  send(method, params = {}) {
    return this.rig.cdp.send(method, params, this.sessionId);
  }

  /** The value of `expression` in the page's main world, awaiting a promise it returns. */
  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  }

  /** The DevTools events this session received for `method`, collected from now on. */
  collect(method) {
    const events = [];
    this.rig.cdp.onEvent((message) => {
      if (message.sessionId === this.sessionId && message.method === method) events.push(message.params);
    });
    return events;
  }
}

/**
 * The browser of a journey: Chromium with the extension under test, its DevTools connection, the
 * bridge to the test home's service and the worker's inspector.
 */
export class BrowserRig {
  /**
   * Start Chromium on a fresh profile with the extension and wait until the service accepts the
   * extension's hello. `label` is a browser scratch prefix without its trailing dash; `hostRules`
   * is passed to `--host-resolver-rules`.
   */
  static async start({ home, label, hostRules = null }) {
    const rig = new BrowserRig(home, label);
    try {
      await rig.launch(hostRules);
    } catch (error) {
      await rig.stop().catch(() => {});
      throw error;
    }
    return rig;
  }

  constructor(home, label) {
    this.home = home;
    this.label = label;
    this.id = extensionId();
    this.bridge = new EdgeBridge(home, extensionOrigin(this.id));
    this.pages = new Map();
    this.mcps = [];
    mkdirSync(SCRATCH_ROOT, { recursive: true });
    this.scratch = mkdtempSync(join(SCRATCH_ROOT, `${label}-`));
  }

  async launch(hostRules) {
    const base = await this.bridge.listen();
    const extension = join(this.scratch, "extension");
    cpSync(EXTENSION_DIRECTORY, extension, { recursive: true });
    const worker = join(extension, WORKER_FILE);
    writeFileSync(worker, workerPrelude(base) + readFileSync(worker, "utf8"));
    const profile = join(this.scratch, "profile");
    this.chromium = spawn(testBrowser(), [
      "--remote-debugging-port=0", `--user-data-dir=${profile}`,
      ...(process.env.GHOSTLIGHT_TEST_NO_SANDBOX === "1" ? ["--no-sandbox"] : []),
      ...(hostRules ? [`--host-resolver-rules=${hostRules}`] : []),
      `--load-extension=${extension}`, "--no-first-run", "--no-default-browser-check", "--disable-background-networking",
      "--disable-component-update", "--disable-sync", "--window-size=1280,900", "about:blank"
    ], { windowsHide: true, stdio: ["ignore", "ignore", "pipe"] });
    this.chromium.stderr.on("data", (data) => { this.chromium.logs = ((this.chromium.logs || "") + data).slice(-4000); });
    this.chromium.on("error", (error) => { this.chromium.startError = error; });
    const [port, endpoint] = await readDevToolsPort(profile, this.chromium);
    this.socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
    await new Promise((done, reject) => { this.socket.onopen = done; this.socket.onerror = reject; });
    this.cdp = cdpChannel((message) => this.socket.send(JSON.stringify(message)));
    this.socket.onmessage = ({ data }) => this.cdp.receive(JSON.parse(data));
    this.version = (await this.cdp.send("Browser.getVersion")).product;
    const workerUrl = `${extensionOrigin(this.id)}${WORKER_FILE}`;
    const target = await until(async () => (await this.cdp.send("Target.getTargets")).targetInfos
      .find((info) => info.type === "service_worker" && info.url === workerUrl), "the extension's service worker", CONNECT_MS);
    const { sessionId } = await this.cdp.send("Target.attachToTarget", { targetId: target.targetId, flatten: true });
    this.workerSession = sessionId;
    try {
      await until(() => this.bridge.accepted > 0, "the service accepts the extension's hello", CONNECT_MS);
    } catch (error) {
      throw new Error(`${error.message}\n${this.bridge.edgeOutput() || "no edge was started"}`);
    }
  }

  /** The value of `expression` in the extension's worker, awaiting a promise it returns. */
  async worker(expression) {
    const result = await this.cdp.send("Runtime.evaluate",
      { expression: `await (${expression})`, returnByValue: true, awaitPromise: true, replMode: true }, this.workerSession);
    assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  }

  /** The id of the newest tab, in tab order, whose URL is exactly `url`. */
  async physicalTab(url) {
    return this.worker(`(await chrome.tabs.query({})).filter((tab) => tab.url === ${JSON.stringify(url)}).at(-1)?.id`);
  }

  /** The page of the tab with the browser's id `tabId`, attached once and then reused. */
  async page(tabId) {
    const targetId = await until(
      () => this.worker(`(await chrome.debugger.getTargets()).find((target) => target.tabId === ${Number(tabId)})?.id`),
      `a page target for tab ${tabId}`, 15_000);
    if (!this.pages.has(targetId)) {
      const { sessionId } = await this.cdp.send("Target.attachToTarget", { targetId, flatten: true });
      this.pages.set(targetId, new PageSession(this, targetId, sessionId));
    }
    return this.pages.get(targetId);
  }

  /** Start `ghostlight mcp` in the home and initialize the session. */
  async mcp(name = "browser journey") {
    const edge = new McpEdge(this.home.start(this.home.edge, ["mcp"]));
    await edge.initialize(name);
    this.mcps.push(edge);
    return edge;
  }

  /** Close the browser, the bridge and the scratch area. The home's own processes are the home's to stop. */
  async stop() {
    for (const edge of this.mcps) edge.child.stdin.end();
    if (this.cdp && this.socket?.readyState === WebSocket.OPEN) {
      try { await this.cdp.send("Browser.close"); } catch { /* the browser may close the reply channel first */ }
    }
    this.socket?.close();
    if (this.chromium && !await waitForChromiumExit(this.chromium)) {
      this.chromium.kill();
      await waitForChromiumExit(this.chromium);
    }
    await this.bridge.close();
    await removeBrowserScratch(this.scratch, SCRATCH_ROOT, `${this.label}-`);
  }
}
