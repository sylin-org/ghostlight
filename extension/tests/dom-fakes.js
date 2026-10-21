"use strict";

// A page script's world without a browser: just enough document and chrome.* for the popup and
// the options page to run in a vm context. Elements keep their text, flags and listeners; the
// extension message channel answers from a script and records every message it was sent.

const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

const ROOT = join(__dirname, "..");
const EXTENSION_ID = "ghostlight-extension";

function fakeElement(tag) {
  const listeners = new Map();
  const classes = new Set();
  const node = {
    tagName: tag.toUpperCase(),
    textContent: "",
    hidden: false,
    disabled: false,
    checked: false,
    type: "",
    href: "",
    dataset: {},
    attributes: {},
    children: [],
    get className() { return [...classes].join(" "); },
    set className(value) { classes.clear(); for (const name of String(value).split(/\s+/)) if (name) classes.add(name); },
    classList: {
      add: (...names) => names.forEach((name) => classes.add(name)),
      remove: (...names) => names.forEach((name) => classes.delete(name)),
      toggle: (name, on = !classes.has(name)) => { if (on) classes.add(name); else classes.delete(name); return on; },
      contains: (name) => classes.has(name)
    },
    setAttribute(name, value) { node.attributes[name] = String(value); },
    removeAttribute(name) { delete node.attributes[name]; },
    append(...children) { node.children.push(...children); },
    replaceChildren(...children) { node.children = [...children]; },
    addEventListener(type, listener) {
      if (!listeners.has(type)) listeners.set(type, []);
      listeners.get(type).push(listener);
    },
    dispatch(type) {
      const event = { type, target: node, preventDefault() {} };
      return (listeners.get(type) ?? []).map((listener) => listener(event));
    },
    click() { return node.dispatch("click"); },
    // Every element below this one, depth first.
    descendants() { return node.children.flatMap((child) => (typeof child === "object" ? [child, ...child.descendants()] : [])); }
  };
  if (tag === "template") {
    const content = { firstElementChild: null };
    Object.defineProperty(node, "innerHTML", { set() { content.firstElementChild = fakeElement("svg"); } });
    node.content = content;
  }
  return node;
}

function fakeDocument() {
  const byId = new Map();
  return {
    getElementById(id) {
      if (!byId.has(id)) byId.set(id, Object.assign(fakeElement("div"), { id }));
      return byId.get(id);
    },
    createElement: fakeElement,
    querySelectorAll: () => []
  };
}

// answers maps a message kind to the value the worker answers with, or to a function of the
// message. A kind with no answer is answered { ok: true, value: null }.
function fakeChrome(answers) {
  const sent = [];
  const onMessage = [];
  const chrome = {
    runtime: {
      id: EXTENSION_ID,
      getURL: (path) => `chrome-extension://${EXTENSION_ID}/${path}`,
      // A message is copied out of the page's realm, as the real channel serializes it.
      async sendMessage(sentMessage) {
        const message = JSON.parse(JSON.stringify(sentMessage));
        sent.push(message);
        const answer = answers[message.kind];
        return { ok: true, value: typeof answer === "function" ? answer(message) : (answer ?? null) };
      },
      getPlatformInfo: async () => ({ os: "win" }),
      openOptionsPage() {},
      onMessage: { addListener: (listener) => onMessage.push(listener) }
    },
    commands: { getAll: async () => [{ name: "toggle-hold", shortcut: "Alt+Shift+P" }] },
    tabs: { create: async () => ({}) },
    downloads: { download: async () => 1 }
  };
  return { chrome, sent };
}

// Runs one page script over the real view modules. The poll interval never fires.
function runPage(script, answers) {
  const document = fakeDocument();
  const { chrome, sent } = fakeChrome(answers);
  const context = vm.createContext({
    document,
    chrome,
    setInterval: () => 0,
    clearInterval() {},
    GhostlightMessages: require("../lib/messages.js"),
    GhostlightPopupView: require("../lib/popup-view.js")
  });
  vm.runInContext(readFileSync(join(ROOT, script), "utf8"), context, { filename: script });
  return { document, sent, byId: (id) => document.getElementById(id) };
}

// Lets every pending promise chain of the page settle.
async function settle(rounds = 8) {
  for (let round = 0; round < rounds; round += 1) await new Promise((resolve) => setImmediate(resolve));
}

module.exports = { runPage, settle };
