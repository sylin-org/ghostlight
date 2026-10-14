"use strict";

// Test helper: a small fake DOM in a vm context, enough to run the protocol-4 page bundle.
//
// It provides what the isolated-world runtime needs: a manual timer queue, a mutation observer
// registry, element trees with classes, styles and removal, and shadow roots the test can reach. Setting `innerHTML` throws, the way a Trusted Types page does.

const vm = require("node:vm");
const { webcrypto } = require("node:crypto");

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";

function selectorMatches(element, selector) {
  const tag = String(element.tagName ?? "").toLowerCase();
  return selector.split(",").map((part) => part.trim()).some((part) => {
    if (part === "*") return true;
    if (part.startsWith("#")) return element.id === part.slice(1);
    if (part === tag || part.startsWith(`${tag}[`)) return true;
    const attribute = /^\[([^=\]]+)(?:=['"]?([^'"\]]+)['"]?)?\]$/.exec(part);
    if (!attribute) return false;
    const value = element.getAttribute?.(attribute[1]);
    return attribute[2] === undefined ? value !== null : value === attribute[2];
  });
}

function descendantsOf(root, selector) {
  const found = [];
  for (const node of root.childNodes ?? []) {
    if (node.nodeType !== 1) continue;
    if (selectorMatches(node, selector)) found.push(node);
    found.push(...descendantsOf(node, selector));
  }
  return found;
}

/**
 * Build a fake page and its vm context.
 *
 * @param {{top?: boolean}} [options] `top: false` makes the window a frame inside another window.
 */
function createFakeDom({ top = true } = {}) {
  let clock = 0;
  let nextTimer = 1;
  const timers = new Map();
  const observers = new Set();
  const created = { namespaces: [] };
  const edits = [];
  let selectedRange = null;
  const selection = {
    removeAllRanges() { selectedRange = null; },
    addRange(range) { selectedRange = range; }
  };

  class TextNode {
    constructor(value) {
      this.nodeType = 3;
      this.nodeValue = value;
      this.parentNode = null;
      this.rendered = true;
    }
  }

  class DocumentFragment {
    constructor() {
      this.nodeType = 11;
      this.childNodes = [];
    }

    append(...nodes) {
      for (const node of nodes) {
        node.parentNode = this;
        this.childNodes.push(node);
      }
    }

    querySelectorAll(selector) { return descendantsOf(this, selector); }
  }

  class HTMLElement {
    constructor(tagName = "DIV") {
      this.nodeType = 1;
      this.tagName = tagName.toUpperCase();
      this.childNodes = [];
      this.children = [];
      this.attributes = new Map();
      this.hidden = false;
      this.isContentEditable = false;
      this.rendered = true;
      this.isConnected = true;
      this.clientLeft = 0;
      this.clientTop = 0;
      this.clientWidth = 100;
      this.clientHeight = 30;
      this.events = [];
      this.listeners = new Map();
      this.style = {};
      this.id = "";
      this.textContent = "";
      this.parentNode = null;
      this.shadow = null;
      this.classes = new Set();
      this.classList = {
        add: (...names) => names.forEach((name) => this.classes.add(name)),
        remove: (...names) => names.forEach((name) => this.classes.delete(name)),
        contains: (name) => this.classes.has(name),
        toggle: (name, force) => {
          const on = force ?? !this.classes.has(name);
          if (on) this.classes.add(name); else this.classes.delete(name);
          return on;
        }
      };
    }

    get className() { return [...this.classes].join(" "); }
    set className(value) { this.classes = new Set(String(value).split(/\s+/).filter(Boolean)); }
    set innerHTML(_value) { throw new Error("This document requires 'TrustedHTML' assignment."); }

    append(...nodes) {
      for (const node of nodes) {
        node.parentNode = this;
        this.childNodes.push(node);
        if (node.nodeType === 1) this.children.push(node);
      }
    }

    appendChild(node) {
      this.append(node);
      node.isConnected = true;
      return node;
    }

    replaceChildren(...nodes) {
      for (const child of this.childNodes) child.parentNode = null;
      this.childNodes = [];
      this.children = [];
      this.append(...nodes);
    }

    remove() {
      const parent = this.parentNode;
      if (parent) {
        parent.childNodes = parent.childNodes.filter((node) => node !== this);
        if (parent.children) parent.children = parent.children.filter((node) => node !== this);
      }
      this.parentNode = null;
      this.isConnected = false;
    }

    attachShadow() {
      this.shadow = new DocumentFragment();
      return this.shadow;
    }

    addEventListener(type, listener) { this.listeners.set(type, listener); }
    getAttribute(name) { return this.attributes.get(name) ?? null; }
    setAttribute(name, value) { this.attributes.set(name, String(value)); }
    getClientRects() { return this.hidden || !this.rendered ? [] : [{ width: 1, height: 1 }]; }
    getBoundingClientRect() { return { left: 0, top: 0, width: this.clientWidth, height: this.clientHeight }; }
    getRootNode() {
      let node = this;
      while (node.parentNode) node = node.parentNode;
      return node;
    }
    matches(selector) { return selectorMatches(this, selector); }
    closest(selector) { return this.matches(selector) ? this : null; }
    contains(other) {
      for (let node = other; node; node = node.parentNode) if (node === this) return true;
      return false;
    }
    querySelectorAll(selector) { return descendantsOf(this, selector); }
    focus() { document.activeElement = this; }
    scrollIntoView() {}
    dispatchEvent(event) { this.events.push(event.type); return true; }
  }

  class HTMLInputElement extends HTMLElement {
    constructor() {
      super("INPUT");
      this.tagName = "INPUT";
      this.type = "file";
      this.id = "upload";
      this.isConnected = true;
      this.disabled = false;
      this.hidden = true;
      this.labels = [];
      this.files = [];
      this.events = [];
      this.value = "";
    }

    getAttribute(name) {
      if (name === "type") return this.type;
      if (name === "id") return this.id;
      return this.attributes.get(name) ?? null;
    }

    getRootNode() { return document; }
    getBoundingClientRect() { return { left: 0, top: 0, width: this.hidden ? 0 : 100, height: this.hidden ? 0 : 30 }; }
    closest() { return null; }
    matches() { return true; }
    click() { this.events.push("click"); }
    select() { selectedRange = { selected: this }; }
  }

  class HTMLTextAreaElement extends HTMLElement {}
  class HTMLSelectElement extends HTMLElement {}
  class File {
    constructor(chunks, name, options) {
      this.name = name;
      this.type = options.type;
      this.size = chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0);
    }
  }

  class DataTransfer {
    constructor() {
      this.files = [];
      this.items = { add: (file) => this.files.push(file) };
    }
  }

  class Event {
    constructor(type) { this.type = type; }
  }

  class DragEvent extends Event {}

  class MutationObserver {
    constructor(callback) {
      this.callback = callback;
    }

    observe() { observers.add(this); }
    disconnect() { observers.delete(this); }
  }

  const input = new HTMLInputElement();
  const body = new HTMLElement("BODY");
  body.innerText = "nothing matching";
  body.append(new TextNode("nothing matching"));
  const documentElement = new HTMLElement("HTML");
  const document = {
    nodeType: 9,
    readyState: "complete",
    title: "Fixture",
    body,
    documentElement,
    activeElement: null,
    hidden: false,
    querySelectorAll(selector) {
      const found = [...new Set([...descendantsOf(this.documentElement, selector), ...descendantsOf(this.body, selector)])];
      if (selectorMatches(input, selector) && !found.includes(input)) found.unshift(input);
      return found;
    },
    createElement(tag) { return new HTMLElement(tag); },
    createElementNS(namespace, tag) {
      created.namespaces.push(namespace);
      return new HTMLElement(tag);
    },
    createRange() {
      let selected = null;
      return {
        selectNodeContents(node) { selected = node; },
        get selected() { return selected; },
        getClientRects() { return selected?.rendered === false ? [] : [{ width: 1, height: 1 }]; },
        detach() {}
      };
    },
    getSelection() { return selection; },
    hasFocus() { return true; },
    execCommand(command, _showUi, value) {
      const element = selectedRange?.selected ?? document.activeElement;
      if (!element) return false;
      edits.push({ element, command, value });
      if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) element.value = command === "delete" ? "" : value;
      else element.textContent = command === "delete" ? "" : value;
      element.events.push("input");
      return true;
    },
    getElementById() { return null; }
  };
  input.getRootNode = () => document;

  function computedStyle(element) {
    const style = element?.hidden
      ? { display: "none", visibility: "hidden", contentVisibility: "hidden", opacity: "0", paddingLeft: "0", paddingRight: "0", paddingTop: "0", paddingBottom: "0" }
      : { display: "block", visibility: "visible", contentVisibility: "visible", opacity: "1", paddingLeft: "0", paddingRight: "0", paddingTop: "0", paddingBottom: "0" };
    return { ...style, ...element?.computedStyle };
  }

  function setTimeoutFake(callback, delay = 0) {
    const id = nextTimer++;
    timers.set(id, { at: clock + delay, callback });
    return id;
  }

  function clearTimeoutFake(id) { timers.delete(id); }

  const window = {
    scrollX: 0,
    scrollY: 0,
    getComputedStyle: computedStyle,
    addEventListener() {},
    removeEventListener() {}
  };
  window.self = window;
  window.top = top ? window : {};

  const context = {
    document,
    window,
    location: { href: "https://example.test/" },
    HTMLElement,
    HTMLInputElement,
    HTMLTextAreaElement,
    HTMLSelectElement,
    File,
    DataTransfer,
    Event,
    DragEvent,
    MouseEvent: Event,
    MutationObserver,
    atob,
    crypto: webcrypto,
    performance: { now: () => clock },
    setTimeout: setTimeoutFake,
    clearTimeout: clearTimeoutFake,
    requestAnimationFrame: (callback) => setTimeoutFake(callback, 16),
    cancelAnimationFrame: clearTimeoutFake,
    getComputedStyle: computedStyle,
    innerWidth: 1024,
    innerHeight: 768,
    scrollX: 0,
    scrollY: 0,
    scrollBy() {},
    scrollTo() {}
  };
  vm.createContext(context);

  /** Run every timer due within `ms` of fake time, in order, then move the clock to the end. */
  function advance(ms) {
    const end = clock + ms;
    for (;;) {
      let next = null;
      for (const [id, timer] of timers) {
        if (timer.at <= end && (!next || timer.at < next.timer.at || (timer.at === next.timer.at && id < next.id))) next = { id, timer };
      }
      if (!next) break;
      timers.delete(next.id);
      clock = Math.max(clock, next.timer.at);
      next.timer.callback();
    }
    clock = end;
  }

  return {
    context,
    document,
    window,
    input,
    edits,
    advance,
    pendingTimers: () => timers.size,
    activeObservers: () => observers.size,
    svgNamespacesCreated: () => created.namespaces.filter((namespace) => namespace === SVG_NAMESPACE).length,
    HTMLElement,
    HTMLInputElement,
    HTMLTextAreaElement,
    HTMLSelectElement,
    element(tagName) { return new HTMLElement(tagName); },
    /** A select holding `options` (objects with value and text); the first is selected. */
    select(options) {
      const element = new HTMLSelectElement("SELECT");
      element.options = options;
      element.value = options[0].value;
      return element;
    },
    text(value) { return new TextNode(value); },
    fragment() { return new DocumentFragment(); },
    setBody(value) { document.body = value; },
    /** Every top-level global the page script has defined, for the no-globals check. */
    globals() { return Object.keys(context); },
    /** Run a script in the page context. */
    run(script, filename = "script.js") { return vm.runInContext(script, context, { filename }); }
  };
}

/** Copy a value out of the vm realm so `assert.deepStrictEqual` can compare it. */
function plain(value) {
  return JSON.parse(JSON.stringify(value ?? null));
}

module.exports = { createFakeDom, plain, SVG_NAMESPACE };
