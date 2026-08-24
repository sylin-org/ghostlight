"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");
const sharedModule = require("../lib/shared.js");

function contentHarness() {
  let listener;
  let clock = 0;
  const delays = [];
  const windowListeners = new Map();

  class HTMLElement {}
  class HTMLInputElement extends HTMLElement {
    constructor() {
      super();
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
      this.attributes = new Map();
    }

    getAttribute(name) {
      if (name === "type") return this.type;
      if (name === "id") return this.id;
      return this.attributes.get(name) ?? null;
    }

    setAttribute(name, value) { this.attributes.set(name, String(value)); }

    getRootNode() { return document; }
    getBoundingClientRect() { return { left: 0, top: 0, width: this.hidden ? 0 : 100, height: this.hidden ? 0 : 30 }; }
    closest() { return null; }
    matches() { return true; }
    dispatchEvent(event) { this.events.push(event.type); return true; }
    click() { this.events.push("click"); }
    scrollIntoView() {}
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

  const input = new HTMLInputElement();
  const document = {
    readyState: "complete",
    title: "Fixture",
    body: { innerText: "nothing matching" },
    documentElement: {},
    querySelectorAll() { return [input]; },
    getElementById() { return null; }
  };
  input.getRootNode = () => document;

  const context = {
    chrome: { runtime: { onMessage: { addListener(value) { listener = value; } } } },
    document,
    location: { href: "https://example.test/" },
    window: {
      scrollX: 0,
      scrollY: 0,
      addEventListener(type, value) { windowListeners.set(type, value); },
      removeEventListener(type, value) {
        if (windowListeners.get(type) === value) windowListeners.delete(type);
      }
    },
    HTMLElement,
    HTMLInputElement,
    HTMLTextAreaElement,
    HTMLSelectElement,
    File,
    DataTransfer,
    Event,
    MouseEvent: Event,
    Uint8Array,
    WeakMap,
    Map,
    Set,
    Promise,
    queueMicrotask,
    Array,
    String,
    Number,
    Boolean,
    Math,
    Object,
    RegExp,
    atob,
    performance: { now: () => clock },
    setTimeout(callback, delay) {
      delays.push(delay);
      clock += delay;
      callback();
    },
    getComputedStyle(element) {
      return element?.hidden
        ? { display: "none", visibility: "hidden", opacity: "0" }
        : { display: "block", visibility: "visible", opacity: "1" };
    },
    innerWidth: 1024,
    innerHeight: 768,
    scrollX: 0,
    scrollY: 0,
    scrollBy() {},
    scrollTo() {},
    GhostlightShared: {
      ...sharedModule,
      bounded(value, maximum) { return String(value ?? "").slice(0, maximum); },
      isCredentialMetadata() { return false; }
    },
    GhostlightPresentation: {
      render() { return false; },
      setManaged() {},
      setHidden() {},
      setRecording() {},
      setRuntimeState() {}
    }
  };
  context.globalThis = context;
  vm.runInNewContext(
    readFileSync(join(__dirname, "..", "content.js"), "utf8"),
    context,
    { filename: "content.js" }
  );

  async function send(message, observe) {
    return new Promise((resolve) => {
      const asynchronous = listener(message, {}, (value) => {
        observe?.("reply");
        resolve(value);
      });
      // Activation answers synchronously through sendResponse and closes the channel; every
      // other primitive keeps the channel open and answers later.
      if (asynchronous !== true && asynchronous !== false) {
        throw new Error("listener returned an unexpected channel flag");
      }
    });
  }

  return {
    input,
    delays,
    send,
    dispatchWindowEvent(type, event) { windowListeners.get(type)?.(event); },
    hasWindowListener(type) { return windowListeners.has(type); }
  };
}

test("upload accepts a connected enabled file input even when it is hidden", async () => {
  const harness = contentHarness();
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const locator = inspected.result.targets[0].locator;

  const uploaded = await harness.send({
    kind: "upload_files",
    locator,
    files: [{ name: "fixture.txt", media_type: "text/plain", size: 5, data: "aGVsbG8=" }]
  });

  assert.equal(uploaded.result.uploaded_count, 1);
  assert.equal(uploaded.result.uploaded_bytes, 5);
  assert.equal(harness.input.files.length, 1);
  assert.equal(harness.input.files[0].name, "fixture.txt");
  assert.deepEqual(harness.input.events, ["input", "change"]);
});

test("upload still rejects a disabled hidden file input", async () => {
  const harness = contentHarness();
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const locator = inspected.result.targets[0].locator;
  harness.input.disabled = true;

  const uploaded = await harness.send({
    kind: "upload_files",
    locator,
    files: [{ name: "fixture.txt", media_type: "text/plain", size: 5, data: "aGVsbG8=" }]
  });

  assert.equal(uploaded.ok, false);
  assert.match(uploaded.error, /disabled for upload/);
});

test("action names use labels but never the current input value", async () => {
  const harness = contentHarness();
  harness.input.value = "patient-secret-42";
  let inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  assert.equal(inspected.result.targets[0].name, "");

  harness.input.setAttribute("aria-label", "Upload evidence");
  inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  assert.equal(inspected.result.targets[0].name, "Upload evidence");

  harness.input.setAttribute("aria-label", "");
  harness.input.type = "submit";
  harness.input.value = "changed runtime value";
  harness.input.setAttribute("value", "Save changes");
  inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  assert.equal(inspected.result.targets[0].role, "button");
  assert.equal(inspected.result.targets[0].name, "Save changes");
});

test("action names preserve rendered spacing inside a label", async () => {
  const harness = contentHarness();
  harness.input.labels = [{
    innerText: "Sylin back stamp verified\nSet seal and proof number match",
    textContent: "Sylin back stamp verifiedSet seal and proof number match"
  }];

  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });

  assert.equal(
    inspected.result.targets[0].name,
    "Sylin back stamp verified Set seal and proof number match"
  );
});

test("the activation receipt names the physical element it used", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "submit";
  harness.input.setAttribute("value", "Save changes");
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });

  const activated = await harness.send({
    kind: "activate",
    locator: inspected.result.targets[0].locator,
    button: "primary",
    click_count: 1
  });

  assert.equal(activated.result.subject.role, "button");
  assert.equal(activated.result.subject.name, "Save changes");
  assert.deepEqual(harness.input.events, ["click"]);
});

test("the activation reply crosses to the worker before the dispatch runs", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "submit";
  harness.input.setAttribute("value", "Save changes");
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });

  const order = [];
  harness.input.click = () => order.push("dispatch");
  const response = await harness.send(
    { kind: "activate", locator: inspected.result.targets[0].locator, button: "primary", click_count: 1 },
    (phase) => order.push(phase)
  );

  assert.deepEqual(order, ["reply", "dispatch"]);
  assert.equal(response.result.activated, true);
  assert.equal(response.result.subject.name, "Save changes");
});

test("an unactionable activation target still refuses before any reply", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "submit";
  harness.input.setAttribute("value", "Save changes");
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });

  harness.input.disabled = true;
  const refused = await harness.send({
    kind: "activate",
    locator: inspected.result.targets[0].locator,
    button: "primary",
    click_count: 1
  });

  assert.equal(refused.ok, false);
  assert.match(refused.error, /disabled/);
});

test("observation polling stops at its physical timeout without overshooting", async () => {
  const harness = contentHarness();
  const observed = await harness.send({
    kind: "observe",
    condition: "text_present",
    value: "never present",
    timeout_ms: 250
  });

  assert.equal(observed.result.satisfied, false);
  assert.equal(observed.result.elapsed_ms, 250);
  assert.equal(observed.result.readiness, "complete");
  assert.deepEqual(harness.delays, [100, 100, 50]);
});

test("drag observation retains only native lifecycle booleans and cleans up", async () => {
  const harness = contentHarness();
  assert.equal((await harness.send({ kind: "drag_observation_arm" })).result.armed, true);
  assert.equal(harness.hasWindowListener("dragstart"), true);

  const event = { defaultPrevented: true, dataTransfer: { secret: "never retained" } };
  harness.dispatchWindowEvent("dragstart", event);
  await new Promise((resolve) => queueMicrotask(resolve));
  const status = await harness.send({ kind: "drag_observation_status" });
  assert.equal(status.ok, true);
  assert.equal(status.result.started, true);
  assert.equal(status.result.cancelled, true);
  const finished = await harness.send({ kind: "drag_observation_finish" });
  assert.equal(finished.ok, true);
  assert.equal(finished.result.started, true);
  assert.equal(finished.result.cancelled, true);
  assert.equal(harness.hasWindowListener("dragstart"), false);
});
