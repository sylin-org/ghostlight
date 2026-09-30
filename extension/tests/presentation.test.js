// SPDX-License-Identifier: Apache-2.0 OR MIT
// Page presentation remains passive even when its stylesheet is unavailable.
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

function renderer() {
  const nodes = [];
  const timers = new Map();
  let now = 0;
  let nextTimer = 0;

  function element(tagName) {
    const classes = new Set();
    const node = {
      tagName,
      children: [],
      attributes: {},
      style: {},
      textContent: "",
      innerHTML: "",
      get isConnected() {
        return this === documentElement || Boolean(this.parentNode?.isConnected);
      },
      classList: {
        add(value) { classes.add(value); },
        remove(value) { classes.delete(value); },
        toggle(value, enabled) {
          if (enabled) classes.add(value);
          else classes.delete(value);
        }
      },
      setAttribute(name, value) { this.attributes[name] = value; },
      append(...children) {
        for (const child of children) {
          child.parentNode = this;
          this.children.push(child);
        }
      },
      appendChild(child) { this.append(child); },
      replaceChildren(...children) {
        for (const child of this.children) child.parentNode = null;
        this.children = [];
        this.append(...children);
      },
      remove() {
        if (this.parentNode) {
          this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
          this.parentNode = null;
        }
      },
      attachShadow() {
        this.shadow = element("#shadow-root");
        this.shadow.parentNode = this;
        return this.shadow;
      }
    };
    nodes.push(node);
    return node;
  }

  const documentElement = element("html");
  function descendants(node) {
    return [node, ...node.children.flatMap(descendants)];
  }
  const document = {
    createElement: element,
    documentElement,
    querySelectorAll(selector) {
      return descendants(documentElement).filter((node) => `#${node.id}` === selector);
    }
  };
  const context = vm.createContext({
    document,
    GhostlightShared: require("../lib/shared.js"),
    setTimeout(callback, delay) {
      const id = ++nextTimer;
      timers.set(id, { callback, deadline: now + delay });
      return id;
    },
    clearTimeout(id) { timers.delete(id); }
  });
  for (const file of ["presentation-css.js", "presentation.js"]) {
    vm.runInContext(readFileSync(join(__dirname, "..", "lib", file), "utf8"), context);
  }
  return {
    get api() { return context.GhostlightPresentation; },
    document,
    nodes,
    reinstall() {
      vm.runInContext(readFileSync(join(__dirname, "..", "lib", "presentation.js"), "utf8"), context);
    },
    byClass(name) { return nodes.find((node) => node.className === name); },
    advance(milliseconds) {
      now += milliseconds;
      for (const [id, timer] of timers) {
        if (timer.deadline > now) continue;
        timers.delete(id);
        timer.callback();
      }
    }
  };
}

function assertPassive(nodes) {
  for (const node of nodes) {
    assert.ok(["html", "#shadow-root", "style", "div", "section"].includes(node.tagName),
      `${node.tagName} must not introduce a page control`);
    assert.equal(node.attributes.tabindex, undefined);
    assert.equal(node.tabIndex, undefined);
    assert.equal(node.attributes.contenteditable, undefined);
    assert.notEqual(node.attributes.role, "dialog");
    assert.notEqual(node.attributes["aria-modal"], "true");
    assert.doesNotMatch(node.innerHTML, /<(?:button|input|select|textarea|a)\b|\btabindex=/i);
  }
}

test("mounting and runtime transitions cannot expose hidden page controls without CSS", () => {
  const view = renderer();
  view.api.setManaged(true);
  for (const state of ["active", "attention", "held", "ended", "disconnected", "active"]) {
    view.api.setRuntimeState(state);
    assertPassive(view.nodes);
    assert.equal(view.byClass("denials").children.length, 0);
  }
  const host = view.nodes.find((node) => node.id === "ghostlight-presentation-root");
  assert.equal(host.style.pointerEvents, "none");
  assert.equal(view.byClass("surface").style.pointerEvents, "none");
  assert.equal(view.nodes.filter((node) => node.tagName !== "style" && node.textContent).length, 0,
    "idle rendering has no hidden text for a failed stylesheet to expose");
});

test("installing a new renderer retires predecessor roots and preserves user controls", () => {
  const view = renderer();
  const userControl = view.document.createElement("button");
  userControl.id = "human-page-control";
  view.document.documentElement.appendChild(userControl);
  const predecessors = [0, 1].map(() => {
    const host = view.document.createElement("div");
    host.id = "ghostlight-presentation-root";
    host.attachShadow({ mode: "closed" }).appendChild(view.document.createElement("button"));
    view.document.documentElement.appendChild(host);
    return host;
  });
  const createdBeforeReplacement = view.nodes.length;
  view.reinstall();
  assert.equal(view.document.querySelectorAll("#ghostlight-presentation-root").length, 0,
    "installation removes old controls before any new presentation signal");
  assert.ok(predecessors.every((host) => !host.isConnected));
  assert.equal(userControl.isConnected, true);
  view.api.setManaged(true);
  view.api.setRuntimeState("attention");
  assert.equal(view.document.querySelectorAll("#ghostlight-presentation-root").length, 1);
  assertPassive(view.nodes.slice(createdBeforeReplacement));

  view.reinstall();
  view.api.setManaged(true);
  assert.equal(view.document.querySelectorAll("#ghostlight-presentation-root").length, 1,
    "repeated replacement cannot accumulate live roots");
  assert.equal(userControl.isConnected, true);
});

test("a legacy attention signal shows only a passive notice and expires", () => {
  const view = renderer();
  assert.equal(view.api.render({ signal: "attention" }, { effects: false, captions: false }), true);
  assertPassive(view.nodes);
  const layer = view.byClass("denials");
  assert.equal(layer.children.length, 1);
  assert.equal(layer.children[0].attributes.role, "status");
  assert.equal(view.byClass("denial-description").textContent, "Open the Ghostlight workbench for details.");
  view.advance(4999);
  assert.equal(layer.children.length, 1);
  view.advance(1);
  assert.equal(layer.children.length, 0);
});

test("all page styling preserves pointer transparency and passive notices replace one another", () => {
  const view = renderer();
  view.api.render({ signal: "denial", phase: "Action refused", detail: "Configured policy refused it." }, {});
  view.advance(3000);
  view.api.render({ signal: "attention", phase: "Runtime control changed", detail: "See the workbench." }, {});
  const layer = view.byClass("denials");
  assert.equal(layer.children.length, 1);
  assert.equal(layer.children[0].children[1].children[0].textContent, "Runtime control changed");
  view.advance(2000);
  assert.equal(layer.children.length, 1, "replacement owns a fresh notice lifetime");
  view.advance(3000);
  assert.equal(layer.children.length, 0);
  assertPassive(view.nodes);
  const css = view.nodes.find((node) => node.tagName === "style").textContent;
  assert.match(css, /:host,\*\{pointer-events:none!important\}/);
  for (const declaration of css.matchAll(/pointer-events:([^;!}]+)/g)) {
    assert.equal(declaration[1], "none");
  }
  assert.doesNotMatch(css, /backdrop-filter/);
});
