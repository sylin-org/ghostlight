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

  function selectorMatches(element, selector) {
    const tag = String(element.tagName ?? "").toLowerCase();
    return selector.split(",").map((part) => part.trim()).some((part) => {
      if (part === "*") return true;
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
    }

    append(...nodes) {
      for (const node of nodes) {
        node.parentNode = this;
        this.childNodes.push(node);
        if (node.nodeType === 1) this.children.push(node);
      }
    }

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
    querySelectorAll(selector) { return descendantsOf(this, selector); }
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

    setAttribute(name, value) { this.attributes.set(name, String(value)); }

    getRootNode() { return document; }
    getBoundingClientRect() { return { left: 0, top: 0, width: this.hidden ? 0 : 100, height: this.hidden ? 0 : 30 }; }
    closest() { return null; }
    matches() { return true; }
    dispatchEvent(event) { this.events.push(event.type); return true; }
    click() { this.events.push("click"); }
    focus() {}
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
  const body = new HTMLElement("BODY");
  body.innerText = "nothing matching";
  body.append(new TextNode("nothing matching"));
  const document = {
    nodeType: 9,
    readyState: "complete",
    title: "Fixture",
    body,
    documentElement: {},
    querySelectorAll(selector) {
      const found = descendantsOf(this.body, selector);
      if (selectorMatches(input, selector) && !found.includes(input)) found.unshift(input);
      return found;
    },
    createRange() {
      let selected = null;
      return {
        selectNodeContents(node) { selected = node; },
        getClientRects() { return selected?.rendered === false ? [] : [{ width: 1, height: 1 }]; },
        detach() {}
      };
    },
    getElementById() { return null; }
  };
  input.getRootNode = () => document;

  function computedStyle(element) {
    return element?.hidden
      ? { display: "none", visibility: "hidden", contentVisibility: "hidden", opacity: "0", paddingLeft: "0", paddingRight: "0", paddingTop: "0", paddingBottom: "0" }
      : { display: "block", visibility: "visible", contentVisibility: "visible", opacity: "1", paddingLeft: "0", paddingRight: "0", paddingTop: "0", paddingBottom: "0" };
  }

  const context = {
    chrome: { runtime: { onMessage: { addListener(value) { listener = value; } } } },
    document,
    location: { href: "https://example.test/" },
    window: {
      scrollX: 0,
      scrollY: 0,
      getComputedStyle: computedStyle,
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
    getComputedStyle: computedStyle,
    innerWidth: 1024,
    innerHeight: 768,
    scrollX: 0,
    scrollY: 0,
    scrollBy() {},
    scrollTo() {},
    GhostlightShared: {
      ...sharedModule,
      bounded(value, maximum) { return String(value ?? "").slice(0, maximum); }
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
    document,
    send,
    element(tagName) { return new HTMLElement(tagName); },
    fragment() { return new DocumentFragment(); },
    text(value) { return new TextNode(value); },
    setBody(value) { document.body = value; },
    setScroll(x, y) {
      context.scrollX = x;
      context.scrollY = y;
    },
    dispatchWindowEvent(type, event) { windowListeners.get(type)?.(event); },
    hasWindowListener(type) { return windowListeners.has(type); }
  };
}

test("visible reads follow the composed tree without exposing hidden or editable text", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const host = harness.element("project-card");
  const shadow = harness.fragment();
  const shadowCopy = harness.element("p");
  shadowCopy.append(harness.text("Open shadow copy"));
  const nestedHost = harness.element("field-shell");
  const nestedShadow = harness.fragment();
  nestedShadow.append(harness.text("Nested shadow label"));
  nestedHost.shadowRoot = nestedShadow;
  const slot = harness.element("slot");
  const assigned = harness.text("Assigned label");
  slot.assignedNodes = () => [assigned];
  const editable = harness.element("textarea");
  editable.append(harness.text("private draft"));
  const hidden = harness.element("p");
  hidden.hidden = true;
  hidden.append(harness.text("hidden copy"));
  shadow.append(shadowCopy, nestedHost, slot, editable, hidden);
  host.shadowRoot = shadow;
  host.append(harness.text("unassigned light copy"));
  const closedHost = harness.element("sealed-card");
  const unrenderedText = harness.text("sealed unassigned copy");
  unrenderedText.rendered = false;
  const unrenderedLight = harness.element("p");
  unrenderedLight.rendered = false;
  unrenderedLight.append(harness.text("closed fallback copy"));
  closedHost.append(unrenderedText, unrenderedLight);
  body.append(harness.text("Outer shell"), host, closedHost);
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "visible", max_chars: 500 });

  assert.equal(read.ok, true);
  assert.match(read.result.text, /Outer shell/);
  assert.match(read.result.text, /Open shadow copy/);
  assert.match(read.result.text, /Nested shadow label/);
  assert.match(read.result.text, /Assigned label/);
  assert.doesNotMatch(read.result.text, /unassigned|private draft|hidden copy|closed fallback/);
  assert.equal(read.result.truncated, false);
});

test("composed reads apply one exact character ceiling", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  body.append(harness.text("abcdefghij"));
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "visible", max_chars: 5 });

  assert.equal(read.result.text, "abcde");
  assert.equal(read.result.truncated, true);
});

test("explicit article reading can select useful prose inside an open shadow root", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const host = harness.element("news-shell");
  const shadow = harness.fragment();
  const article = harness.element("article");
  article.append(harness.text("A composed article inside the component contains enough useful prose to satisfy the article threshold without exposing the host's unrendered light children."));
  shadow.append(article);
  host.shadowRoot = shadow;
  body.append(host);
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "article", max_chars: 500 });

  assert.equal(read.result.article_found, true);
  assert.match(read.result.text, /composed article inside the component/);
  assert.equal(read.result.truncated, false);
});

test("an absent useful article asks the worker for the full-page fallback", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const article = harness.element("article");
  article.append(harness.text("Too short."));
  body.append(article);
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "article", max_chars: 500 });

  assert.deepEqual(
    { text: read.result.text, truncated: read.result.truncated, article_found: read.result.article_found },
    { text: "", truncated: false, article_found: false }
  );
});

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

test("form fill reports the verified fields without a submit", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "text";
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const locator = inspected.result.targets[0].locator;

  const filled = await harness.send({ kind: "fill", fields: [{ locator, value: "Aurora Drop 01" }] });

  assert.equal(filled.result.filled_count, 1);
  assert.equal(filled.result.submitted, false);
});

test("the fill reply crosses to the worker before the verified submit fires", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "text";
  harness.input.closest = (selector) => (selector === "form" ? { contains: () => true } : null);
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const locator = inspected.result.targets[0].locator;
  const order = [];
  harness.input.click = () => order.push("submit");

  const response = await harness.send(
    { kind: "fill", fields: [{ locator, value: "Aurora Drop 01" }], submit_locator: locator },
    (phase) => order.push(phase)
  );

  assert.deepEqual(order, ["reply", "submit"]);
  assert.equal(response.result.filled_count, 1);
  assert.equal(response.result.submitted, true);
  assert.ok(harness.input.events.includes("input"), "the field value change still fired its input event");
});

test("a submit control outside the resolved form refuses before any reply", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "text";
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const locator = inspected.result.targets[0].locator;

  const refused = await harness.send({
    kind: "fill",
    fields: [{ locator, value: "x" }],
    submit_locator: locator
  });

  assert.equal(refused.ok, false);
  assert.match(refused.error, /not contained/);
});

test("invisibility refusals name the exact predicate", async () => {
  const harness = contentHarness();
  harness.input.type = "submit";
  harness.input.setAttribute("value", "Save changes");
  let inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const locator = inspected.result.targets[0].locator;

  // Hidden by the hidden attribute -> display:none per computed style.
  const hidden = await harness.send({ kind: "activate", locator, button: "primary", click_count: 1 });
  assert.equal(hidden.ok, false);
  assert.match(hidden.error, /target is not visible for activate \(display:none\)/);

  // Hidden by aria-hidden even when rendered.
  harness.input.hidden = false;
  harness.input.setAttribute("aria-hidden", "true");
  const ariaHidden = await harness.send({ kind: "activate", locator, button: "primary", click_count: 1 });
  assert.equal(ariaHidden.ok, false);
  assert.match(ariaHidden.error, /target is not visible for activate \(aria-hidden\)/);
});

test("credential handoff names the exact control", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "password";
  harness.input.setAttribute("aria-label", "Master password");
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const locator = inspected.result.targets[0].locator;

  const refused = await harness.send({ kind: "fill", fields: [{ locator, value: "hunter2" }] });

  assert.equal(refused.ok, false);
  assert.match(refused.error, /requires user handoff: the textbox "Master password"/);
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

test("text observation sees rendered text inside an open shadow root", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const host = harness.element("status-card");
  const shadow = harness.fragment();
  shadow.append(harness.text("Shadow task complete"));
  host.shadowRoot = shadow;
  body.append(host);
  harness.setBody(body);

  const observed = await harness.send({
    kind: "observe",
    condition: "text_present",
    value: "Shadow task complete",
    timeout_ms: 0
  });

  assert.equal(observed.result.satisfied, true);
  assert.equal(observed.result.elapsed_ms, 0);
});

test("find and document trees use composed shadow content", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const host = harness.element("task-control");
  host.setAttribute("role", "button");
  const shadow = harness.fragment();
  const label = harness.element("span");
  label.append(harness.text("Approve garden task"));
  shadow.append(label);
  host.shadowRoot = shadow;
  host.append(harness.text("unassigned decoy"));
  body.append(host);
  harness.setBody(body);

  const found = await harness.send({ kind: "find", text: "garden task", find_kind: "control", max_results: 5 });
  assert.equal(found.ok, true);
  assert.equal(found.result.targets.length, 1);
  assert.equal(found.result.targets[0].name, "Approve garden task");

  const inspected = await harness.send({ kind: "inspect_tree", max_depth: 5, max_nodes: 400 });
  assert.equal(inspected.ok, true);
  assert.equal(inspected.result.tree.children[0].kind, "control");
  assert.equal(inspected.result.tree.children[0].label, "Approve garden task");
  assert.equal(inspected.result.tree.children[0].children[0].label, "Approve garden task");
  assert.doesNotMatch(JSON.stringify(inspected.result.tree), /unassigned decoy/);
});

test("frame geometry and point subjects descend through an open shadow root", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const host = harness.element("frame-shell");
  const shadow = harness.fragment();
  const frame = harness.element("iframe");
  frame.src = "https://example.test/inside";
  frame.name = "inside";
  frame.clientLeft = 2;
  frame.clientTop = 3;
  frame.clientWidth = 240;
  frame.clientHeight = 120;
  frame.getBoundingClientRect = () => ({ left: 10, top: 20, width: 244, height: 126 });
  shadow.append(frame);
  shadow.elementFromPoint = () => frame;
  host.shadowRoot = shadow;
  body.append(host);
  harness.setBody(body);
  harness.document.elementFromPoint = () => host;

  const boxes = await harness.send({ kind: "frame_boxes" });
  assert.equal(boxes.result.boxes.length, 1);
  assert.equal(boxes.result.boxes[0].left, 12);
  assert.equal(boxes.result.boxes[0].top, 23);

  const point = await harness.send({ kind: "point_context", x: 30, y: 40 });
  assert.equal(point.result.subject.role, "iframe");
  assert.equal(point.result.embed.src, "https://example.test/inside");
  assert.equal(point.result.embed.left, 12);
  assert.equal(point.result.embed.top, 23);
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

test("box reports the live viewport rectangle without scrolling", async () => {
  const harness = contentHarness();
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 10 });
  const locator = inspected.result.targets[0].locator;

  harness.input.hidden = false;
  harness.input.setAttribute("aria-label", "Embed field");
  const boxed = await harness.send({ kind: "box", locator });

  assert.equal(boxed.ok, true);
  assert.equal(boxed.result.rectangle.width, 100);
  assert.equal(boxed.result.rectangle.height, 30);
  assert.equal(boxed.result.subject.role, "textbox");
  assert.equal(boxed.result.subject.name, "Embed field");
});

test("scroll_offset reports the frame's own scroll position", async () => {
  const harness = contentHarness();
  harness.setScroll(40, 120);
  const offset = await harness.send({ kind: "scroll_offset" });
  assert.equal(offset.ok, true);
  assert.equal(offset.result.x, 40);
  assert.equal(offset.result.y, 120);
});

function shadowFixture(name) {
  return {
    tagName: "INPUT",
    id: "",
    labels: [],
    isConnected: true,
    disabled: false,
    hidden: false,
    isContentEditable: true,
    textContent: "stale draft",
    events: [],
    getAttribute(key) { return key === "aria-label" ? name : null; },
    getBoundingClientRect() { return { left: 4, top: 4, width: 40, height: 20 }; },
    dispatchEvent(event) { this.events.push(event.type); return true; }
  };
}

test("focused-control discovery reaches the element living inside a shadow host", async () => {
  const harness = contentHarness();
  const inner = shadowFixture("Inner field");
  harness.document.activeElement = { shadowRoot: { activeElement: inner } };

  const described = await harness.send({ kind: "describe_focused" });
  assert.equal(described.ok, true);
  assert.equal(described.result.targets[0].role, "textbox");
  assert.equal(described.result.targets[0].name, "Inner field");

  const cleared = await harness.send({ kind: "clear_focused" });
  assert.equal(cleared.ok, true);
  assert.equal(cleared.result.cleared, true);
  assert.equal(cleared.result.subject.name, "Inner field");
  assert.equal(inner.textContent, "");
  assert.deepEqual(inner.events, ["input"]);
});

test("point subjects cross the shadow boundary to the nearest actionable host", async () => {
  const harness = contentHarness();
  const inner = shadowFixture("Inner field");
  const button = {
    tagName: "BUTTON",
    id: "",
    labels: [],
    closest(selector) { return selector.includes("button") ? this : null; },
    getAttribute(key) { return key === "aria-label" ? "Seal" : null; },
    getRootNode() { return {}; }
  };
  inner.getRootNode = () => ({ host: button });
  harness.document.elementFromPoint = () => inner;

  const crossed = await harness.send({ kind: "scroll_point", x: 30, y: 30 });
  assert.equal(crossed.result.subject.role, "button");
  assert.equal(crossed.result.subject.name, "Seal");

  const orphan = { tagName: "SPAN", id: "", labels: [], matches() { return false; }, getRootNode() { return {}; }, getAttribute() { return null; } };
  harness.document.elementFromPoint = () => orphan;
  const plain = await harness.send({ kind: "scroll_point", x: 30, y: 30 });
  assert.equal(plain.result.subject.role, "span");
});
