"use strict";

// Test doubles for the protocol 4 world, inventory and input modules: a scripted chrome.debugger
// with per-session frame trees, chrome.webNavigation, and the wiring the worker will do.

const crypto = require("node:crypto");
const sessions = require("../lib/sessions.js");
const World = require("../lib/world.js");
const Inventory = require("../lib/inventory.js");
const Input = require("../lib/input.js");

const ROOT = "root";
const TOP = "TOP";
const TOP_LOADER = "L-top";
const CHILD = "CHILD";
const CHILD_LOADER = "L-child";

/** A frame tree node: `{ frame: { id, loaderId, url, parentId? }, childFrames }`. */
function node(id, loaderId, url, parentId, childFrames = []) {
  const frame = { id, loaderId, url };
  if (parentId) frame.parentId = parentId;
  return { frame, childFrames };
}

/** The default tab: a top document with one same-process child frame. */
function sameProcessTree() {
  return node(TOP, TOP_LOADER, "https://example.com/account", null,
    [node(CHILD, CHILD_LOADER, "https://example.com/embed", TOP)]);
}

/**
 * A scripted Chrome debugger. `trees` maps a session id (or "root") to its frame tree.
 * `handlers[method](target, params)` overrides one method; `calls` records every command.
 */
function fakeChrome({ trees = { [ROOT]: sameProcessTree() }, frames = [] } = {}) {
  const listeners = [];
  const calls = [];
  const handlers = {};
  let counter = 0;
  const contexts = new Map();
  const api = {
    calls,
    handlers,
    trees,
    frames,
    onEvent: { addListener(listener) { listeners.push(listener); } },
    emit(source, method, params) { for (const listener of listeners) listener(source, method, params); },
    async attach() {},
    async detach() {},
    async sendCommand(target, method, params = {}) {
      calls.push({ target, method, params });
      if (handlers[method]) return handlers[method](target, params);
      switch (method) {
        case "Page.getFrameTree": return { frameTree: trees[target.sessionId ?? ROOT] };
        case "Page.addScriptToEvaluateOnNewDocument": counter += 1; return { identifier: `script-${counter}` };
        case "Page.createIsolatedWorld":
          counter += 1;
          contexts.set(counter, params.frameId);
          return { executionContextId: counter };
        case "Runtime.evaluate": return { result: { objectId: `global:${contexts.get(params.contextId)}` } };
        case "DOM.getFrameOwner": return { backendNodeId: 77 };
        case "DOM.resolveNode": return { object: { objectId: `owner:${params.backendNodeId}` } };
        case "Runtime.callFunctionOn":
          if (params.functionDeclaration === World.SHIMS.CANCEL) return { result: { value: true } };
          if (params.functionDeclaration === World.SHIMS.CALL) {
            return { result: { value: { ok: true, value: { kind: params.arguments[1].value.kind, on: params.objectId } } } };
          }
          return { result: { value: true } };
        default: return {};
      }
    },
    webNavigation: { async getAllFrames() { return frames; } }
  };
  return api;
}

/** The current assets for a tab: a tiny source and its sha256 id. */
function assets(source = "\"use strict\";\nglobalThis.__ghostlightRuntime = { id: __ghostlightAssetId };") {
  return { id: crypto.createHash("sha256").update(source, "utf8").digest("hex"), source };
}

/** Wires sessions, inventory, world and input the way the worker does. `now` and `sleep` replace the clock of the world and input. */
function wire(chrome, options = {}) {
  const box = {};
  const lifecycle = sessions.create(chrome, {
    installer: (session) => box.world.register(session),
    onRelease: (tabId) => box.world.disposeTab(tabId),
    onSessionGone: (tabId, sessionId) => box.world.sessionGone(tabId, sessionId)
  });
  const inventory = Inventory.create({ lifecycle, webNavigation: chrome.webNavigation });
  const clock = { now: options.now, sleep: options.sleep };
  box.world = World.create({ lifecycle, inventory, isCancelled: options.isCancelled, local: options.local, ...clock });
  const input = Input.create({ lifecycle, inventory, worldCalls: box.world, isCancelled: options.isCancelled, ...clock });
  return { lifecycle, inventory, world: box.world, input };
}

/** An effect watch like the dispatcher's: records watched tabs and the dispatched flag. */
function effects() {
  return { watched: [], dispatched: false, watch(tabId) { this.watched.push(tabId); } };
}

/** The methods sent, in order, optionally only for one method. */
function methods(chrome, only) {
  return chrome.calls.map((call) => call.method).filter((method) => !only || method === only);
}

/** The world call shim invocations, as their parameters. */
function worldCalls(chrome) {
  return chrome.calls.filter((call) => call.method === "Runtime.callFunctionOn" && call.params.functionDeclaration === World.SHIMS.CALL);
}

module.exports = {
  ROOT, TOP, TOP_LOADER, CHILD, CHILD_LOADER,
  node, sameProcessTree, fakeChrome, assets, wire, effects, methods, worldCalls
};
