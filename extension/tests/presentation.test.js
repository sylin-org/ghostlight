"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

// Execute the shipped renderer against a deterministic DOM/clock. The test observes its
// actual attached medallions, rather than searching the source for a cleanup function.
function rendererHarness() {
  let now = 0;
  let nextTimer = 0;
  const timers = new Map();
  class Element {
    constructor(tag) {
      this.tagName = tag;
      this.children = [];
      this.parentNode = null;
      this.style = {};
      this.className = "";
      this.attributes = new Map();
      this.classList = {
        contains: (name) => this.className.split(/\s+/).includes(name),
        add: (...names) => { this.className = [...new Set([...this.className.split(/\s+/).filter(Boolean), ...names])].join(" "); },
        remove: (...names) => { this.className = this.className.split(/\s+/).filter((name) => !names.includes(name)).join(" "); },
        toggle: (name, enabled) => {
          const next = enabled ?? !this.classList.contains(name);
          this.classList[next ? "add" : "remove"](name);
          return next;
        }
      };
    }
    get isConnected() { return this === document.documentElement || Boolean(this.parentNode?.isConnected); }
    append(...children) { for (const child of children) this.appendChild(child); }
    appendChild(child) { child.remove(); child.parentNode = this; this.children.push(child); return child; }
    remove() {
      if (this.parentNode) this.parentNode.children = this.parentNode.children.filter((child) => child !== this);
      this.parentNode = null;
    }
    replaceChildren(...children) { for (const child of [...this.children]) child.remove(); this.append(...children); }
    attachShadow() { const shadow = new Element("shadow"); this.appendChild(shadow); return shadow; }
    setAttribute(name, value) { this.attributes.set(name, value); }
    addEventListener() {}
  }
  const document = { createElement: (tag) => new Element(tag), documentElement: new Element("html") };
  const setTimer = (callback, delay = 0) => {
    const id = ++nextTimer;
    timers.set(id, { callback, at: now + delay });
    return id;
  };
  const context = vm.createContext({
    document,
    innerWidth: 1024,
    innerHeight: 768,
    setTimeout: setTimer,
    clearTimeout: (id) => timers.delete(id),
    requestAnimationFrame: (callback) => setTimer(callback, 16),
    GhostlightShared: require("../lib/shared.js")
  });
  vm.runInContext(readFileSync(join(__dirname, "../lib/presentation-css.js"), "utf8"), context);
  vm.runInContext(readFileSync(join(__dirname, "../lib/presentation.js"), "utf8"), context);
  const renderer = context.GhostlightPresentation;
  function find(className, root = document.documentElement) {
    return root.children.flatMap((child) => [...(child.classList.contains(className) ? [child] : []), ...find(className, child)]);
  }
  function advance(milliseconds) {
    const until = now + milliseconds;
    for (;;) {
      const entry = [...timers].filter(([, timer]) => timer.at <= until).sort((a, b) => a[1].at - b[1].at || a[0] - b[0])[0];
      if (!entry) break;
      const [id, timer] = entry;
      timers.delete(id);
      now = timer.at;
      timer.callback();
    }
    now = until;
  }
  return {
    renderer, find, advance,
    send(signal, activity = "script", invocation = "operation_a", preferences = { effects: true, captions: false }) {
      return renderer.render({ signal, activity, invocation, phase: "Fixed phase" }, preferences);
    }
  };
}

test("completion retires the owning script signature even when its final activity is quiet", () => {
  const page = rendererHarness();
  page.send("start");
  assert.equal(page.find("workwheel").length, 1, "positive control: the shipped renderer started the script wheel");
  page.send("completion", "quiet");
  page.advance(1000);
  assert.equal(page.find("signature").length, 0);
});

test("denial and changed effects preferences still terminate an active signature", () => {
  for (const [signal, effects] of [["denial", true], ["completion", false], ["denial", false]]) {
    const page = rendererHarness();
    page.send("start");
    assert.equal(page.find("signature").length, 1);
    page.send(signal, "script", "operation_a", { effects, captions: false });
    page.advance(1000);
    assert.equal(page.find("signature").length, 0, `${signal} with effects=${effects}`);
    if (signal === "denial") assert.equal(page.find("denial-ribbon").length, 1, "the refusal keeps its own notice lifetime");
  }
});

test("another invocation's terminal signal cannot retire the current script signature", () => {
  const page = rendererHarness();
  page.send("start", "script", "operation_a");
  page.send("start", "script", "operation_b");
  const current = page.find("signature")[0];
  page.send("completion", "script", "operation_a");
  page.advance(1000);
  assert.equal(page.find("signature")[0], current);
  assert.equal(current.classList.contains("completing"), false);
  page.send("completion", "script", "operation_b");
  page.advance(1000);
  assert.equal(page.find("signature").length, 0);
});

test("missing completion is bounded while normal maximum-duration work stays visible", () => {
  const page = rendererHarness();
  page.send("start");
  page.advance(29_000);
  page.send("progress");
  page.advance(1000);
  assert.equal(page.find("workwheel").length, 1, "the supported 30-second work lifetime remains represented");
  page.advance(6000);
  assert.equal(page.find("signature").length, 0, "duplicate progress cannot keep an abandoned wheel alive forever");
});

test("runtime stop, pause, attention, and disconnect clear activity without resurrecting it", () => {
  for (const state of ["held", "attention", "ended", "disconnected"]) {
    const page = rendererHarness();
    page.send("start");
    page.renderer.setRuntimeState(state);
    assert.equal(page.find("signature").length, 0, state);
    page.renderer.setRuntimeState("active");
    page.advance(1000);
    assert.equal(page.find("signature").length, 0, `resume from ${state}`);
  }
});

test("a new activity in a composition does not inherit its previous script wheel", () => {
  const page = rendererHarness();
  page.send("start");
  page.send("start", "read");
  assert.equal(page.find("workwheel").length, 0);
  assert.equal(page.find("read-scan").length, 1);
  page.send("completion", "read");
  page.advance(2000);
  assert.equal(page.find("signature").length, 0);
  assert.equal(page.find("read-scan").length, 0);
});

test("old completion timers cannot erase a replacement medallion", () => {
  const page = rendererHarness();
  page.send("start", "script", "operation_a");
  page.send("completion", "script", "operation_a");
  page.advance(200);
  page.send("start", "wait", "operation_b");
  const current = page.find("signature")[0];
  page.advance(1000);
  assert.equal(page.find("signature")[0], current);
  assert.equal(page.find("wait-lights").length, 1);
  page.send("completion", "quiet", "operation_b");
  page.advance(1000);
  assert.equal(page.find("signature").length, 0);
});
