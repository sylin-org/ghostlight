"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");

function contentHarness() {
  let listener;
  let clock = 0;
  const delays = [];

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
    window: { scrollX: 0, scrollY: 0 },
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

  async function send(message) {
    return new Promise((resolve) => {
      const asynchronous = listener(message, {}, resolve);
      assert.equal(asynchronous, true);
    });
  }

  return { input, delays, send };
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
