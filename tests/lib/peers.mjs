// The peers a process journey plays: an MCP client, and a browser that speaks adapter protocol 4.
//
// An MCP client reaches the service either through the real `ghostlight mcp` edge (McpEdge) or as
// a bare relay 4 `mcp` stream (RelayMcp). The synthetic browser stands where the extension
// stands: it sends the hello, answers each request with a receipt about a small fake world of
// tabs, acknowledges heartbeats and can hold an answer back. It reaches the service through the
// real browser edge (`ghostlight chrome-extension://<id>/`) or as a bare relay 4 `browser` stream.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { join } from "node:path";
import {
  buildVersion, decodeFrames, encodeFrame, mcpKind, openRelay, opening, program, repository, role, until, vocabulary
} from "./home.mjs";

/** The adapter protocol 4 vocabulary the Rust and extension goldens share. */
export const adapter = JSON.parse(readFileSync(join(repository, "tests", "golden", "protocol4", "vocabulary.json"), "utf8"));

/** The origin a synthetic extension presents. */
export const EXTENSION_ORIGIN = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";

/** The MCP protocol revision the journeys initialize with. */
export const MCP_REVISION = "2025-11-25";

/** Time a request may take when it can start a service. */
export const COLD_REQUEST_MS = 45_000;

/** A real one-pixel JPEG, the shape a capture hands over. */
export const ONE_PIXEL_JPEG = "/9j/4AAQSkZJRgABAQEAYABgAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/wAALCAABAAEBAREA/8QAFAABAAAAAAAAAAAAAAAAAAAACf/EABQQAQAAAAAAAAAAAAAAAAAAAAD/2gAIAQEAAD8AKp//2Q==";

/**
 * A frame id in the browser's alphabet. The service accepts only ASCII letters, digits and '-' in
 * a browser id (`is_browser_id` in protocol4/locator.rs), so an underscore here would make every
 * document-bound command fail admission. Real Chrome frame ids are uppercase hex.
 */
export const frameIdOf = (tabId) => `F${tabId}`;

/** A loader id in the browser's alphabet; see frameIdOf. */
export const loaderIdOf = (load) => `L${load}`;

const initializeParams = (name) => ({ protocolVersion: MCP_REVISION, capabilities: {}, clientInfo: { name, version: "1" } });

/** The structured facts of a tool call's response, after checking it is not a protocol error. */
export function structured(response) {
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  return response.result.structuredContent;
}

/** The first text block of a tool call's response. */
export function textOf(response) {
  assert.equal(response.error, undefined, JSON.stringify(response.error));
  const item = response.result.content.find((content) => content.type === "text");
  assert.equal(typeof item?.text, "string");
  return item.text;
}

/** An MCP client on the standard streams of a `ghostlight mcp` edge. */
export class McpEdge {
  constructor(child) {
    this.child = child;
    this.nextId = 1;
    this.pending = new Map();
    this.notifications = [];
    createInterface({ input: child.stdout }).on("line", (line) => {
      const message = JSON.parse(line);
      const key = JSON.stringify(message.id);
      const waiter = message.id === undefined ? null : this.pending.get(key);
      if (waiter) {
        this.pending.delete(key);
        waiter(message);
      } else {
        this.notifications.push(message);
      }
    });
  }

  /** Send a request now; `promise` resolves with the response. */
  begin(method, params = {}, timeoutMs = COLD_REQUEST_MS) {
    const id = this.nextId++;
    const promise = new Promise((done, fail) => {
      const timer = setTimeout(() => fail(new Error(`Timed out waiting for MCP ${method}`)), timeoutMs);
      this.pending.set(JSON.stringify(id), (message) => { clearTimeout(timer); done(message); });
    });
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    return { id, promise };
  }

  /** Send a request and wait for its response. */
  request(method, params = {}, timeoutMs = COLD_REQUEST_MS) {
    return this.begin(method, params, timeoutMs).promise;
  }

  /** Send a notification. */
  notify(method, params = {}) {
    this.child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`);
  }

  /** Initialize the session and announce it. */
  async initialize(name = "journey") {
    const response = await this.request("initialize", initializeParams(name));
    assert.equal(response.error, undefined, JSON.stringify(response.error));
    this.notify("notifications/initialized");
    return response.result;
  }

  /** Call a tool and return the whole response. */
  call(name, args = {}, timeoutMs = COLD_REQUEST_MS) {
    return this.request("tools/call", { name, arguments: args }, timeoutMs);
  }

  /** Call a tool and return its structured facts. */
  async facts(name, args = {}, timeoutMs = COLD_REQUEST_MS) {
    return structured(await this.call(name, args, timeoutMs));
  }
}

/** An MCP client on a bare relay 4 `mcp` stream. */
export class RelayMcp {
  constructor(stream, reply) {
    this.stream = stream;
    this.reply = reply;
    this.nextId = 1;
  }

  /** Open an `mcp` stream in `home`; `extra` adds opening fields such as `channel` and `session`. */
  static async open(home, extra = {}, patience = 0) {
    const { stream, reply } = await openRelay(home.path, opening(role.mcp, { channel: "mcp", ...extra }), patience);
    assert.equal(reply.accepted, true, JSON.stringify(reply));
    return new RelayMcp(stream, reply);
  }

  /** Send a request now and return its id; `response` takes its answer. `id` defaults to the next one. */
  begin(method, params = {}, id = this.nextId++) {
    this.stream.send({ kind: mcpKind.message, line: JSON.stringify({ jsonrpc: "2.0", id, method, params }) });
    return id;
  }

  /** The next response to the request with `id`; a second request with that id gets a second one. */
  async response(id, timeoutMs = COLD_REQUEST_MS) {
    const frame = await this.stream.next(
      (candidate) => candidate.kind === mcpKind.message && JSON.parse(candidate.line).id === id, timeoutMs, `the MCP response to request ${id}`);
    return JSON.parse(frame.line);
  }

  /** Send a request and wait for its response. */
  async request(method, params = {}, timeoutMs = COLD_REQUEST_MS) {
    return this.response(this.begin(method, params), timeoutMs);
  }

  /** Send a notification. */
  notify(method, params = {}) {
    this.stream.send({ kind: mcpKind.message, line: JSON.stringify({ jsonrpc: "2.0", method, params }) });
  }

  /** Initialize the session and announce it. */
  async initialize(name = "journey") {
    const response = await this.request("initialize", initializeParams(name));
    assert.equal(response.error, undefined, JSON.stringify(response.error));
    this.notify("notifications/initialized");
    return response.result;
  }

  /** Call a tool and return its structured facts. */
  async facts(name, args = {}, timeoutMs = COLD_REQUEST_MS) {
    return structured(await this.request("tools/call", { name, arguments: args }, timeoutMs));
  }

  /** The frames received and not yet taken. */
  pending() {
    return this.stream.pending();
  }

  /** Close the stream. */
  close() {
    this.stream.close();
  }
}

/** The tabs of a fake browser. It outlives any one connection, as a browser outlives its host. */
export class World {
  constructor(firstTab = 41) {
    this.nextTab = firstTab;
    this.tabs = [];
    this.loads = 0;
  }

  /** Open a tab on `url`. */
  open(url) {
    this.loads += 1;
    const tab = { tab_id: this.nextTab++, title: "Example Domain", url: new URL(url).href, active: true, readiness: "complete", load: this.loads };
    this.tabs.push(tab);
    return tab;
  }

  /** The tab with `id`, or undefined. */
  find(id) {
    return this.tabs.find((tab) => tab.tab_id === id);
  }

  /** Point the tab at `url`. */
  navigate(id, url) {
    const tab = this.find(id);
    this.loads += 1;
    tab.url = new URL(url).href;
    tab.load = this.loads;
    return tab;
  }

  /** The tabs as the wire reports them. */
  list(ids) {
    return this.tabs.filter((tab) => !ids || ids.includes(tab.tab_id)).map(wireTab);
  }
}

function wireTab({ tab_id, title, url, active, readiness }) {
  return { tab_id, title, url, active, readiness };
}

/** A connection to the service through the browser edge's standard streams. */
class EdgeTransport {
  constructor(child) {
    this.child = child;
    this.buffer = Buffer.alloc(0);
    this.listeners = [];
    this.ended = child.exited.then(() => undefined);
    child.stdout.on("data", (chunk) => {
      const { frames, rest } = decodeFrames(Buffer.concat([this.buffer, chunk]));
      this.buffer = rest;
      for (const frame of frames) for (const listener of this.listeners) listener(frame);
    });
  }

  write(frame) {
    this.child.stdin.write(encodeFrame(frame));
  }

  onFrame(listener) {
    this.listeners.push(listener);
  }

  close() {
    this.child.stdin.end();
    if (this.child.exitCode === null) this.child.kill();
  }
}

/** A connection to the service as a bare relay 4 `browser` stream. */
class RelayTransport {
  constructor(stream) {
    this.stream = stream;
    this.listeners = [];
    this.ended = stream.ended;
    (async () => {
      for (;;) {
        let frame;
        try {
          frame = await stream.next(() => true, 2_147_483_647, "a browser stream frame");
        } catch {
          return;
        }
        for (const listener of this.listeners) listener(frame);
      }
    })();
  }

  write(frame) {
    this.stream.send(frame);
  }

  onFrame(listener) {
    this.listeners.push(listener);
  }

  close() {
    this.stream.close();
  }
}

const receipt = (correlation, fields) => ({ kind: "receipt", correlation, dispatched: false, committed_urls: [], ...fields });

/**
 * A browser speaking adapter protocol 4 about a `World`. It answers every primitive it knows and
 * refuses the rest as unsupported, the way an extension answers a primitive it does not have.
 */
export class SyntheticBrowser {
  constructor(transport, world, browserId = "browser_journey") {
    this.transport = transport;
    this.world = world;
    this.browserId = browserId;
    this.frames = [];
    this.requests = [];
    this.held = [];
    this.holding = null;
    this.heartbeats = 0;
    this.answering = true;
    this.ended = transport.ended;
    transport.onFrame((frame) => this.receive(frame));
  }

  /** A browser behind a real `ghostlight chrome-extension://<id>/` edge of `home`. */
  static viaEdge(home, world, origin = EXTENSION_ORIGIN, browserId) {
    const child = home.start(home.edge, [origin]);
    const browser = new this(new EdgeTransport(child), world, browserId);
    browser.child = child;
    return browser;
  }

  /** A browser on a bare `browser` stream of `home`. */
  static async viaRelay(home, world, browserId, patience = 0) {
    const { stream, reply } = await openRelay(home.path, opening(role.browser, {
      edge: { program: program.edge, revision: vocabulary.relay, build: buildVersion() }, origin: EXTENSION_ORIGIN
    }), patience);
    assert.equal(reply.accepted, true, JSON.stringify(reply));
    return new this(new RelayTransport(stream), world, browserId);
  }

  /** The hello frame of this browser. */
  helloFrame() {
    return {
      kind: "hello",
      protocol: adapter.protocol,
      browser_id: this.browserId,
      browser_name: "Chrome",
      platform: adapter.platform,
      engine_version: "131.0.6778.86",
      adapter_version: buildVersion(),
      attended: false
    };
  }

  /** Send the hello and wait for the service to accept it; checks the assets it carries. */
  async connect(timeout = 30_000) {
    this.transport.write(this.helloFrame());
    const accepted = await this.waitFor((frame) => frame.kind === "hello_accepted", timeout, "hello_accepted");
    assert.equal(createHash("sha256").update(accepted.assets.source).digest("hex"), accepted.assets.id, "the assets carry their own digest");
    this.accepted = accepted;
    return accepted;
  }

  /** Send any frame to the service. */
  send(frame) {
    this.transport.write(frame);
  }

  /** Ask the service for a runtime control change, as the extension's popup does. */
  requestControl(intent) {
    this.send({ kind: "event", event: { event: "runtime_control_requested", intent } });
  }

  /** The first received frame `test` accepts, within `timeout` ms. */
  waitFor(test, timeout = 10_000, label = "a frame") {
    return until(() => this.frames.find(test), label, timeout);
  }

  /** The control state frames received, in order. */
  controlStates() {
    return this.frames.filter((frame) => frame.kind === "control_state").map((frame) => frame.state);
  }

  /** Hold back the answer to the next request of primitive `name`. */
  holdNext(name) {
    this.holding = name;
  }

  /** Answer every request held back. */
  release() {
    const held = this.held.splice(0);
    for (const request of held) this.transport.write(this.answer(request));
  }

  /** The requests received for primitive `name`, in order. */
  requested(name) {
    return this.requests.filter((request) => request.primitive.name === name);
  }

  /** Stop acknowledging heartbeats, or start again. */
  acknowledgeHeartbeats(on) {
    this.answering = on;
  }

  /** Close the connection. */
  close() {
    this.transport.close();
  }

  receive(frame) {
    this.frames.push(frame);
    if (frame.kind === "heartbeat") {
      this.heartbeats += 1;
      if (this.answering) this.transport.write({ kind: "heartbeat_ack", sequence: frame.sequence });
    } else if (frame.kind === "request") {
      this.requests.push(frame);
      if (this.holding === frame.primitive.name) {
        this.holding = null;
        this.held.push(frame);
      } else {
        this.transport.write(this.answer(frame));
      }
    }
  }

  failed(correlation, failure, message) {
    return receipt(correlation, { failure, message });
  }

  answer({ correlation, primitive: { name, args } }) {
    const missing = () => this.failed(correlation, "tab_missing", `No tab with id: ${args.tab_id}.`);
    const needsTab = ["navigate", "focus_tab", "close_tab", "frames", "view", "call", "capture", "dialog"];
    const tab = this.world.find(args.tab_id);
    if (needsTab.includes(name) && !tab) return missing();
    const done = (outcome, extra = {}) => receipt(correlation, { ok: outcome, ...extra });
    switch (name) {
      case "list_tabs":
        return done({ tabs: this.world.list(args.tab_ids) });
      case "open_tab": {
        const opened = this.world.open(args.url);
        return done({ tab: wireTab(opened), adopted: false }, { dispatched: true, committed_urls: [opened.url], tab: wireTab(opened) });
      }
      case "navigate": {
        const target = args.to;
        const moved = target.to === "url" ? this.world.navigate(tab.tab_id, target.url) : tab;
        return done({ tab: wireTab(moved) }, { dispatched: true, committed_urls: [moved.url], tab: wireTab(moved) });
      }
      case "focus_tab":
        return done({ tab_id: tab.tab_id, active: true, window_focused: true }, { dispatched: true });
      case "close_tab":
        this.world.tabs.splice(this.world.tabs.indexOf(tab), 1);
        return done({ tab_id: tab.tab_id }, { dispatched: true });
      case "frames":
        return done({
          frames: [{ frame_id: frameIdOf(tab.tab_id), loader_id: loaderIdOf(tab.load), url: tab.url, unreachable: false }],
          complete: true,
          unseen: []
        });
      case "view":
        return done(viewFacts());
      case "call":
        return done({ answers: this.callAnswers(args, tab) }, { dispatched: true, tab: wireTab(tab) });
      case "capture":
        return done({
          mime_type: "image/jpeg",
          data: ONE_PIXEL_JPEG,
          width: 400,
          height: 300,
          output_scale: 0.5,
          clip: { x: 0, y: 0, width: 800, height: 600 },
          view: viewFacts()
        });
      case "dialog":
        return done({ present: false, dialog_type: "unknown" });
      default:
        return receipt(correlation, { failure: "unsupported" });
    }
  }

  callAnswers(args, tab) {
    const answers = [];
    let stopped = false;
    for (const call of args.calls) {
      if (stopped && args.mode === "sequence") {
        answers.push({ status: "skipped" });
        continue;
      }
      const value = worldValue(call.message, tab);
      stopped = value === undefined;
      answers.push(value === undefined
        ? { status: "returned", envelope: { ok: false, error: { code: "unsupported", message: "" } } }
        : { status: "returned", envelope: { ok: true, value } });
    }
    return answers;
  }
}

function viewFacts() {
  return {
    visual_page_x: 10,
    visual_page_y: 20,
    visual_css_width: 800,
    visual_css_height: 600,
    content_css_width: 800,
    content_css_height: 1200,
    device_scale: 1,
    zoom: 1
  };
}

/** What the page asset answers to a world message, or undefined for a message this world lacks. */
function worldValue(message, tab) {
  if (message.kind === "read_text") {
    const base = { text: "Example Domain", truncated: false, title: tab.title, url: tab.url };
    return message.mode === "article" ? { ...base, text: "", article_found: false } : base;
  }
  return undefined;
}
