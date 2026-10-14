"use strict";

// Pins the protocol-4 page behaviour of content.js: what each world message does, which coded
// error each refusal carries, that an effect kind refuses before it mutates anything, and that a
// cancel token ends every page loop. The bundle runs in the vm fake DOM through the world
// envelope, the way the extension calls it.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const bundle = require("./bundle.js");
const { createFakeDom, plain } = require("./fake-dom.js");

const REPO = path.join(__dirname, "..", "..", "..", "..", "..", "..", "..", "..", "..");
const GOLDEN = path.join(REPO, "tests", "golden", "protocol4");
const vocabulary = JSON.parse(fs.readFileSync(path.join(GOLDEN, "vocabulary.json"), "utf8"));

const SOURCE = bundle.expand();
const ID = bundle.sha256(SOURCE);
const STEP_MS = 25;
const MAX_STEPS = 800;

const tick = () => new Promise((resolve) => setImmediate(resolve));

/** Install the bundle in a fresh fake page and return the handles a test needs. */
function contentHarness({ top = true } = {}) {
  const dom = createFakeDom({ top });
  dom.run(bundle.registration(ID, SOURCE), "world.js");
  const runtime = dom.context.__ghostlightRuntime;
  const scrolls = [];
  dom.context.scrollBy = (options) => scrolls.push(options);
  dom.context.scrollTo = (options) => scrolls.push(options);

  /** Call the runtime and drive the fake clock until the call answers. */
  async function send(message, options = {}, observe) {
    let answered = false;
    const pending = runtime.call(message, options).then((reply) => {
      answered = true;
      observe?.("reply");
      return plain(reply);
    });
    for (let step = 0; step < MAX_STEPS && !answered; step += 1) {
      await tick();
      if (!answered) dom.advance(STEP_MS);
    }
    return pending;
  }

  /** The error code of a refused call, or null when the call succeeded. */
  async function codeOf(message, options = {}) {
    const reply = await send(message, options);
    return reply.ok ? null : reply.error.code;
  }

  async function targets() {
    const reply = await send({ kind: "inspect", inspect_kind: "controls", max_items: 40, visual_settle: false });
    return reply.value.targets;
  }

  async function locatorOf(name) {
    return (await targets()).find((target) => target.name === name).locator;
  }

  return {
    dom,
    runtime,
    scrolls,
    send,
    codeOf,
    targets,
    locatorOf,
    input: dom.input,
    edits: dom.edits,
    document: dom.document,
    element: (tagName) => dom.element(tagName),
    /** A visible, connected text input of the real input class, appended to the body. */
    newInput(label, props = {}) {
      const element = new dom.HTMLInputElement();
      element.id = "";
      element.hidden = false;
      element.type = "text";
      element.setAttribute("aria-label", label);
      for (const [name, value] of Object.entries(props)) element[name] = value;
      dom.document.body.append(element);
      return element;
    },
    select: (options) => dom.select(options),
    fragment: () => dom.fragment(),
    text: (value) => dom.text(value),
    setBody: (value) => dom.setBody(value),
    setScroll(x, y) {
      dom.context.scrollX = x;
      dom.context.scrollY = y;
    }
  };
}

/** A visible text input the tests can edit. */
function textInput(harness, { label = "Name", value = "" } = {}) {
  harness.input.hidden = false;
  harness.input.type = "text";
  harness.input.value = value;
  harness.input.setAttribute("aria-label", label);
  return harness.input;
}

/** A visible rich editor appended to the body. */
function richEditor(harness, { label = "Reply", content = "" } = {}) {
  const editor = harness.element("div");
  editor.isContentEditable = true;
  editor.textContent = content;
  editor.setAttribute("contenteditable", "true");
  editor.setAttribute("aria-label", label);
  harness.document.body.append(editor);
  return editor;
}

function editSummary(harness) {
  return harness.edits.map(({ command, value }) => ({ command, value }));
}

const FILE = Object.freeze({ name: "fixture.txt", media_type: "text/plain", size: 5, data: "aGVsbG8=" });

// ---- reads -------------------------------------------------------------------------------

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
  assert.match(read.value.text, /Outer shell/);
  assert.match(read.value.text, /Open shadow copy/);
  assert.match(read.value.text, /Nested shadow label/);
  assert.match(read.value.text, /Assigned label/);
  assert.doesNotMatch(read.value.text, /unassigned|private draft|hidden copy|closed fallback/);
  assert.equal(read.value.truncated, false);
});

test("composed reads apply one exact character ceiling", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  body.append(harness.text("abcdefghij"));
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "visible", max_chars: 5 });

  assert.equal(read.value.text, "abcde");
  assert.equal(read.value.truncated, true);
});

test("composed reads include form values and editable text while masking passwords", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const textField = harness.element("input");
  textField.type = "text";
  textField.value = "John Doe";
  const emptyField = harness.element("input");
  emptyField.type = "text";
  emptyField.placeholder = "Enter email";
  const passField = harness.element("input");
  passField.type = "password";
  passField.setAttribute("type", "password");
  passField.value = "secret123";
  const selectNode = harness.element("select");
  selectNode.selectedOptions = [{ text: "Option B" }];
  const editable = harness.element("div");
  editable.isContentEditable = true;
  editable.append(harness.text("Editable prose"));
  body.append(textField, emptyField, passField, selectNode, editable);
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "visible", max_chars: 500 });

  assert.equal(read.ok, true);
  assert.match(read.value.text, /John Doe/);
  assert.doesNotMatch(read.value.text, /Enter email/);
  assert.match(read.value.text, /\[hidden\]/);
  assert.doesNotMatch(read.value.text, /secret123|\*{8}/);
  assert.match(read.value.text, /Option B/);
  assert.match(read.value.text, /Editable prose/);
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

  assert.equal(read.value.article_found, true);
  assert.match(read.value.text, /composed article inside the component/);
  assert.equal(read.value.truncated, false);
});

test("an absent useful article reports article_found false", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const article = harness.element("article");
  article.append(harness.text("Too short."));
  body.append(article);
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "article", max_chars: 500, visual_settle: false });

  assert.deepEqual(
    { text: read.value.text, truncated: read.value.truncated, article_found: read.value.article_found },
    { text: "", truncated: false, article_found: false }
  );
});

test("reads mask every credential-class field, not only type password", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const pin = harness.element("input");
  pin.setAttribute("type", "text");
  pin.setAttribute("autocomplete", "one-time-code");
  pin.value = "482913";
  const token = harness.element("input");
  token.setAttribute("type", "text");
  token.setAttribute("name", "api_token");
  token.value = "tok-live-77";
  body.append(pin, token);
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "visible", max_chars: 500 });

  assert.doesNotMatch(read.value.text, /482913|tok-live-77/);
  assert.equal(read.value.text.match(/\[hidden\]/g).length, 2);
  assert.doesNotMatch(read.value.text, /\*{8}/);
});

test("reads report checkboxes and radios by state and never show a placeholder as a value", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const box = harness.element("input");
  box.type = "checkbox"; box.value = "on"; box.checked = true;
  const radio = harness.element("input");
  radio.type = "radio"; radio.value = "express"; radio.checked = false;
  const empty = harness.element("input");
  empty.type = "text"; empty.value = ""; empty.placeholder = "name@example.com";
  body.append(box, radio, empty);
  harness.setBody(body);

  const read = await harness.send({ kind: "read_text", mode: "visible", max_chars: 500 });

  assert.match(read.value.text, /checked/);
  assert.match(read.value.text, /unchecked/);
  assert.doesNotMatch(read.value.text, /express|name@example/);
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
  assert.equal(found.value.targets.length, 1);
  assert.equal(found.value.targets[0].name, "Approve garden task");

  const inspected = await harness.send({ kind: "inspect_tree", max_depth: 5, max_nodes: 400 });
  assert.equal(inspected.ok, true);
  assert.equal(inspected.value.tree.children[0].kind, "control");
  assert.equal(inspected.value.tree.children[0].label, "Approve garden task");
  assert.equal(inspected.value.tree.children[0].children[0].label, "");
  assert.doesNotMatch(JSON.stringify(inspected.value.tree), /unassigned decoy/);
});

test("elements with no role of their own report an accessibility role, never a tag name", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const plain = harness.element("div");
  plain.append(harness.text("plain container"));
  const navigation = harness.element("nav");
  navigation.setAttribute("aria-label", "Site");
  const list = harness.element("ul");
  list.setAttribute("aria-label", "Items");
  const banner = harness.element("header");
  banner.setAttribute("aria-label", "Top");
  const region = harness.element("section");
  region.setAttribute("aria-label", "Billing");
  body.append(plain, navigation, list, banner, region);
  harness.setBody(body);

  const found = await harness.send({ kind: "find", text: "plain container", find_kind: "any", max_results: 5 });
  assert.equal(found.value.targets[0].role, "generic");
  const structure = await harness.send({ kind: "inspect", inspect_kind: "structure", max_items: 20, visual_settle: false });
  const roles = Object.fromEntries(structure.value.targets.map((target) => [target.name, target.role]));
  assert.deepEqual(roles, { Site: "navigation", Items: "list", Top: "banner", Billing: "region" });
});

test("document tree containers carry a declared name only, never a dump of their text", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const panel = harness.element("section");
  panel.setAttribute("aria-label", "Billing");
  const paragraph = harness.element("div");
  paragraph.append(harness.text("A long paragraph of text that a container label must never repeat."));
  const heading = harness.element("h2");
  heading.append(harness.text("Invoices"));
  const button = harness.element("button");
  button.append(harness.text("Pay now"));
  panel.append(heading, paragraph, button);
  body.append(panel);
  harness.setBody(body);

  const inspected = await harness.send({ kind: "inspect_tree", max_depth: 5, max_nodes: 400 });

  const [section] = inspected.value.tree.children;
  assert.deepEqual(
    [section.kind, section.label],
    ["container", "Billing"]
  );
  const [title, text, control] = section.children;
  assert.deepEqual([title.kind, title.label], ["heading", "Invoices"]);
  assert.deepEqual([text.kind, text.label], ["container", ""]);
  assert.deepEqual([control.kind, control.label], ["control", "Pay now"]);
  assert.doesNotMatch(JSON.stringify(inspected.value.tree), /long paragraph/);
});

test("a field with no label, aria-label or title is found by its placeholder", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const bare = harness.element("input");
  bare.setAttribute("type", "text");
  bare.setAttribute("placeholder", "Search the archive");
  const titled = harness.element("input");
  titled.setAttribute("type", "text");
  titled.setAttribute("title", "Order number");
  titled.setAttribute("placeholder", "ignored while a title exists");
  body.append(bare, titled);
  harness.setBody(body);

  const found = await harness.send({ kind: "find", text: "archive", find_kind: "control", max_results: 5 });
  assert.equal(found.value.targets.length, 1);
  assert.equal(found.value.targets[0].name, "Search the archive");
  assert.equal(found.value.targets[0].role, "textbox");
  const selected = await harness.send({ kind: "query_semantic", name: "search the archive", exact: true });
  assert.equal(selected.value.targets.length, 1);
  const titledName = await harness.send({ kind: "find", text: "order number", find_kind: "control", max_results: 5 });
  assert.equal(titledName.value.targets[0].name, "Order number");
});

test("text find reaches div containers and draggable, labelled and focusable elements", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const outer = harness.element("div");
  const card = harness.element("div");
  card.setAttribute("draggable", "true");
  card.append(harness.text("Move the blue card"));
  outer.append(card);
  const labelled = harness.element("section");
  labelled.setAttribute("aria-label", "Blue panel");
  const focusable = harness.element("section");
  focusable.setAttribute("tabindex", "0");
  focusable.append(harness.text("Blue focus ring"));
  body.append(outer, labelled, focusable);
  harness.setBody(body);

  const found = await harness.send({ kind: "find", text: "blue", find_kind: "any", max_results: 10 });

  assert.equal(found.ok, true);
  assert.equal(found.value.targets.length, 3);
  const names = found.value.targets.map((target) => target.name);
  assert.ok(names.includes("Blue panel"));
  assert.equal(names.filter((name) => name === "").length, 0);
});

test("text find prefers a shadow-root descendant over light wrappers around its host", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const outer = harness.element("div");
  const inner = harness.element("div");
  const host = harness.element("my-card");
  const shadow = harness.fragment();
  shadow.host = host;
  const label = harness.element("span");
  label.append(harness.text("Drag me"));
  shadow.append(label);
  host.shadowRoot = shadow;
  inner.append(host);
  outer.append(inner);
  body.append(outer);
  harness.setBody(body);

  const found = await harness.send({ kind: "find", text: "drag me", find_kind: "any", max_results: 1 });

  assert.equal(found.ok, true);
  assert.equal(found.value.targets.length, 1);
  assert.equal(found.value.targets[0].role, "generic");
  assert.equal(found.value.truncated, false);
});

test("text find gives locators only to the returned targets", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  for (let index = 0; index < 6; index += 1) {
    const item = harness.element("div");
    item.append(harness.text(`item ${index}`));
    body.append(item);
  }
  const marker = harness.element("div");
  marker.append(harness.text("unique marker"));
  body.append(marker);
  harness.setBody(body);

  const first = await harness.send({ kind: "find", text: "item", find_kind: "any", max_results: 1 });
  const second = await harness.send({ kind: "find", text: "unique marker", find_kind: "any", max_results: 1 });

  assert.equal(first.value.targets.length, 1);
  assert.equal(first.value.truncated, true);
  const number = (target) => Number(target.locator.split("_")[2]);
  assert.equal(number(second.value.targets[0]), number(first.value.targets[0]) + 1);
});

test("action names use labels but never the current input value", async () => {
  const harness = contentHarness();
  harness.input.value = "patient-secret-42";
  assert.equal((await harness.targets())[0].name, "");

  harness.input.setAttribute("aria-label", "Upload evidence");
  assert.equal((await harness.targets())[0].name, "Upload evidence");

  harness.input.setAttribute("aria-label", "");
  harness.input.type = "submit";
  harness.input.value = "changed runtime value";
  harness.input.setAttribute("value", "Save changes");
  const [target] = await harness.targets();
  assert.equal(target.role, "button");
  assert.equal(target.name, "Save changes");
});

test("action names preserve rendered spacing inside a label", async () => {
  const harness = contentHarness();
  harness.input.labels = [{
    innerText: "Sylin back stamp verified\nSet seal and proof number match",
    textContent: "Sylin back stamp verifiedSet seal and proof number match"
  }];

  assert.equal((await harness.targets())[0].name, "Sylin back stamp verified Set seal and proof number match");
});

test("inspection distinguishes styled hidden controls from a visible rich editor", async () => {
  const harness = contentHarness();
  textInput(harness, { label: "Hidden draft helper" });
  harness.input.computedStyle = { display: "none" };
  richEditor(harness);
  const found = await harness.targets();
  assert.ok(found.find((target) => target.name === "Hidden draft helper").state.includes("hidden"));
  assert.ok(!found.find((target) => target.name === "Reply").state.includes("hidden"));
});

// ---- geometry and routing ------------------------------------------------------------------

test("box reports the live viewport rectangle without scrolling", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.setAttribute("aria-label", "Embed field");
  const locator = await harness.locatorOf("Embed field");

  const boxed = await harness.send({ kind: "box", locator });

  assert.equal(boxed.ok, true);
  assert.equal(boxed.value.rectangle.width, 100);
  assert.equal(boxed.value.rectangle.height, 30);
  assert.equal(boxed.value.subject.role, "textbox");
  assert.equal(boxed.value.subject.name, "Embed field");
  assert.deepEqual(harness.scrolls, []);
});

test("scroll_offset reports the frame's own scroll position", async () => {
  const harness = contentHarness();
  harness.setScroll(40, 120);
  const offset = await harness.send({ kind: "scroll_offset" });
  assert.deepEqual(offset, { ok: true, value: { x: 40, y: 120 } });
});

function embedFixture(harness) {
  const frame = harness.element("iframe");
  frame.src = "https://example.test/inside";
  frame.name = "inside";
  frame.clientLeft = 2;
  frame.clientTop = 3;
  frame.clientWidth = 240;
  frame.clientHeight = 120;
  frame.getBoundingClientRect = () => ({ left: 10, top: 20, width: 244, height: 126 });
  return frame;
}

test("owner_box reports the content box of the supplied owner, border and padding included", async () => {
  const harness = contentHarness();
  const frame = embedFixture(harness);
  frame.computedStyle = { paddingLeft: "4", paddingRight: "6", paddingTop: "1", paddingBottom: "2" };

  const box = await harness.send({ kind: "owner_box" }, { owner: frame });

  assert.deepEqual(box, { ok: true, value: { left: 16, top: 24, width: 230, height: 117 } });
});

test("owner kinds without an owner are invalid requests", async () => {
  const harness = contentHarness();
  assert.equal(await harness.codeOf({ kind: "owner_box" }), "invalid_request");
  assert.equal(await harness.codeOf({ kind: "owner_probe", x: 1, y: 1 }), "invalid_request");
});

test("owner_probe hits only when the supplied owner is under the point", async () => {
  const harness = contentHarness();
  const frame = embedFixture(harness);
  const other = embedFixture(harness);
  harness.document.elementFromPoint = () => frame;

  const hit = await harness.send({ kind: "owner_probe", x: 30, y: 40 }, { owner: frame });
  assert.deepEqual(hit, { ok: true, value: { hit: true, left: 12, top: 23 } });

  const miss = await harness.send({ kind: "owner_probe", x: 30, y: 40 }, { owner: other });
  assert.equal(miss.value.hit, false);
  assert.equal(miss.value.left, 12);
});

test("owner_probe sees through an open shadow root to the owner", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const host = harness.element("frame-shell");
  const shadow = harness.fragment();
  const frame = embedFixture(harness);
  shadow.append(frame);
  shadow.elementFromPoint = () => frame;
  host.shadowRoot = shadow;
  body.append(host);
  harness.setBody(body);
  harness.document.elementFromPoint = () => host;

  const probe = await harness.send({ kind: "owner_probe", x: 30, y: 40 }, { owner: frame });
  assert.equal(probe.value.hit, true);
});

test("hit_point reports an embed without a subject and descends through an open shadow root", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const host = harness.element("frame-shell");
  const shadow = harness.fragment();
  const frame = embedFixture(harness);
  shadow.append(frame);
  shadow.elementFromPoint = () => frame;
  host.shadowRoot = shadow;
  body.append(host);
  harness.setBody(body);
  harness.document.elementFromPoint = () => host;

  const point = await harness.send({ kind: "hit_point", x: 30, y: 40, page: false });

  assert.deepEqual(point, { ok: true, value: { x: 30, y: 40, embed: true, subject: null } });
});

test("hit_point converts page coordinates and names the subject of a plain document", async () => {
  const harness = contentHarness();
  harness.setScroll(10, 200);
  harness.input.hidden = false;
  harness.input.setAttribute("aria-label", "Search");
  let seen = null;
  harness.document.elementFromPoint = (x, y) => { seen = [x, y]; return harness.input; };

  const page = await harness.send({ kind: "hit_point", x: 110, y: 260, page: true });
  assert.deepEqual(seen, [100, 60]);
  assert.deepEqual(page.value, { x: 100, y: 60, embed: false, subject: { role: "textbox", name: "Search" } });

  const viewport = await harness.send({ kind: "hit_point", x: 110, y: 260, page: false });
  assert.deepEqual(seen, [110, 260]);
  assert.equal(viewport.value.x, 110);
});

test("hit_point without an element is no_subject", async () => {
  const harness = contentHarness();
  harness.document.elementFromPoint = () => null;
  assert.equal(await harness.codeOf({ kind: "hit_point", x: 1, y: 1, page: false }), "no_subject");
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
    dispatchEvent(event) { this.events.push(event.type); return true; },
    focus() {},
    scrollIntoView() {},
    getRootNode() { return {}; }
  };
}

test("focused-control discovery reaches the element living inside a shadow host", async () => {
  const harness = contentHarness();
  const inner = shadowFixture("Inner field");
  harness.document.activeElement = { shadowRoot: { activeElement: inner } };

  const described = await harness.send({ kind: "describe_focused" });
  assert.equal(described.ok, true);
  assert.equal(described.value.targets[0].role, "textbox");
  assert.equal(described.value.targets[0].name, "Inner field");

  const cleared = await harness.send({ kind: "clear_focused" });
  assert.equal(cleared.ok, true);
  assert.equal(cleared.value.cleared, true);
  assert.equal(cleared.value.subject.name, "Inner field");
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
  assert.equal(crossed.value.subject.role, "button");
  assert.equal(crossed.value.subject.name, "Seal");
  const routed = await harness.send({ kind: "hit_point", x: 30, y: 30, page: false });
  assert.deepEqual(routed.value.subject, { role: "button", name: "Seal" });

  const orphan = { tagName: "SPAN", id: "", labels: [], matches() { return false; }, getRootNode() { return {}; }, getAttribute() { return null; } };
  harness.document.elementFromPoint = () => orphan;
  const bare = await harness.send({ kind: "scroll_point", x: 30, y: 30 });
  assert.equal(bare.value.subject.role, "generic");
});

test("viewport_point converts a page point and refuses one outside the viewport", async () => {
  const harness = contentHarness();
  harness.setScroll(0, 100);
  harness.document.elementFromPoint = () => null;
  assert.deepEqual(await harness.send({ kind: "viewport_point", x: 50, y: 150 }), { ok: true, value: { x: 50, y: 50, subject: null } });
  assert.equal(await harness.codeOf({ kind: "viewport_point", x: 50, y: 50 }), "outside_viewport");
  assert.equal(await harness.codeOf({ kind: "viewport_point", x: 5000, y: 150 }), "outside_viewport");
});

test("focus_probe and require_focused agree on page focus versus an embedded frame", async () => {
  const harness = contentHarness();
  const field = textInput(harness);
  field.focus();
  assert.deepEqual(await harness.send({ kind: "focus_probe" }), { ok: true, value: { focused: true } });
  assert.deepEqual(await harness.send({ kind: "require_focused" }), { ok: true, value: { focused: true } });

  harness.document.activeElement = embedFixture(harness);
  assert.deepEqual(await harness.send({ kind: "focus_probe" }), { ok: true, value: { focused: false } });
  assert.equal(await harness.codeOf({ kind: "require_focused" }), "focus_lost");

  harness.document.activeElement = field;
  harness.document.hasFocus = () => false;
  assert.equal((await harness.send({ kind: "focus_probe" })).value.focused, false);
  assert.equal(await harness.codeOf({ kind: "require_focused" }), "focus_lost");
});

// ---- activation, preparation and uploads ------------------------------------------------------

test("upload accepts a connected enabled file input even when it is hidden", async () => {
  const harness = contentHarness();
  const [target] = await harness.targets();

  const uploaded = await harness.send({ kind: "upload_files", locator: target.locator, files: [FILE] });

  assert.equal(uploaded.value.uploaded_count, 1);
  assert.equal(uploaded.value.uploaded_bytes, 5);
  assert.equal(harness.input.files.length, 1);
  assert.equal(harness.input.files[0].name, "fixture.txt");
  assert.deepEqual(harness.input.events, ["input", "change"]);
});

test("upload still rejects a disabled hidden file input", async () => {
  const harness = contentHarness();
  const [target] = await harness.targets();
  harness.input.disabled = true;

  const uploaded = await harness.send({ kind: "upload_files", locator: target.locator, files: [FILE] });

  assert.equal(uploaded.ok, false);
  assert.equal(uploaded.error.code, "disabled");
  assert.match(uploaded.error.message, /disabled for upload/);
});

test("activation preparation names and locates the physical element without clicking", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "submit";
  harness.input.setAttribute("value", "Save changes");
  const [target] = await harness.targets();
  let clicked = false;
  harness.input.click = () => { clicked = true; };

  const activated = await harness.send({ kind: "activate", locator: target.locator });

  assert.equal(activated.value.activated, true);
  assert.equal(activated.value.subject.role, "button");
  assert.equal(activated.value.subject.name, "Save changes");
  assert.equal(typeof activated.value.rectangle.left, "number");
  assert.equal(typeof activated.value.rectangle.width, "number");
  assert.equal(clicked, false);
  assert.deepEqual(harness.input.events, []);
});

test("an unactionable activation target refuses with its code", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "submit";
  harness.input.setAttribute("value", "Save changes");
  const [target] = await harness.targets();

  harness.input.disabled = true;
  const refused = await harness.send({ kind: "activate", locator: target.locator });

  assert.equal(refused.ok, false);
  assert.equal(refused.error.code, "disabled");
  assert.match(refused.error.message, /disabled/);
});

test("invisibility refusals name the exact predicate", async () => {
  const harness = contentHarness();
  harness.input.type = "submit";
  harness.input.setAttribute("value", "Save changes");
  const [target] = await harness.targets();

  const hidden = await harness.send({ kind: "activate", locator: target.locator });
  assert.equal(hidden.error.code, "not_visible");
  assert.match(hidden.error.message, /target is not visible for activate \(display:none\)/);

  harness.input.hidden = false;
  harness.input.setAttribute("aria-hidden", "true");
  const ariaHidden = await harness.send({ kind: "activate", locator: target.locator });
  assert.equal(ariaHidden.error.code, "not_visible");
  assert.match(ariaHidden.error.message, /target is not visible for activate \(aria-hidden\)/);
});

test("hover, focus and scroll prepare the exact element", async () => {
  const harness = contentHarness();
  textInput(harness);
  const locator = await harness.locatorOf("Name");

  const hovered = await harness.send({ kind: "hover", locator });
  assert.equal(hovered.value.subject.name, "Name");
  const focused = await harness.send({ kind: "focus", locator });
  assert.equal(focused.value.focused, true);
  assert.equal(harness.document.activeElement, harness.input);
  const scrolled = await harness.send({ kind: "scroll", locator });
  assert.equal(scrolled.value.subject.name, "Name");
  const paged = await harness.send({ kind: "scroll", direction: "down", amount: "page" });
  assert.equal(paged.ok, true);
  assert.equal(harness.scrolls.length, 1);
});

test("attach_file reads the attachment and the file name from the message", async () => {
  const harness = contentHarness();
  const [target] = await harness.targets();

  const attached = await harness.send(
    { kind: "attach_file", locator: target.locator, file_name: "demo.gif" },
    { attachment: { media_type: "image/gif", data: "aGVsbG8=", size: 5 } }
  );

  assert.equal(attached.ok, true);
  assert.equal(attached.value.uploaded_count, 1);
  assert.equal(attached.value.uploaded_bytes, 5);
  assert.equal(harness.input.files[0].name, "demo.gif");
  assert.equal(harness.input.files[0].type, "image/gif");
  assert.deepEqual(harness.input.events, ["input", "change"]);
});

test("attach_file without an attachment or with a wrong size changes nothing", async () => {
  const harness = contentHarness();
  const [target] = await harness.targets();
  const message = { kind: "attach_file", locator: target.locator, file_name: "demo.gif" };

  assert.equal(await harness.codeOf(message), "invalid_request");
  assert.equal(await harness.codeOf(message, { attachment: { media_type: "image/gif", data: "aGVsbG8=", size: 99 } }), "payload_mismatch");
  assert.equal(await harness.codeOf(message, { attachment: { media_type: "image/gif", data: "***", size: 3 } }), "payload_mismatch");
  assert.equal(harness.input.files.length, 0);
  assert.deepEqual(harness.input.events, []);
});

test("drop_files delivers a drop event to the element under the point", async () => {
  const harness = contentHarness();
  const target = harness.element("div");
  harness.document.elementFromPoint = () => target;

  const dropped = await harness.send({ kind: "drop_files", x: 5, y: 6, files: [FILE] });

  assert.deepEqual(dropped.value, { uploaded_count: 1, uploaded_bytes: 5 });
  assert.deepEqual(target.events, ["drop"]);
});

// ---- typing ----------------------------------------------------------------------------------

test("focus_text_field focuses the exact control without editing or clicking", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "Existing draft" });
  const locator = await harness.locatorOf("Name");
  const field = { locator, value: "Replacement" };

  const prepared = await harness.send({ kind: "focus_text_field", field, allow_credentials: false });

  assert.equal(prepared.ok, true);
  assert.equal(prepared.value.focused, true);
  assert.equal(prepared.value.subject.name, "Name");
  assert.equal(harness.document.activeElement, harness.input);
  assert.equal(harness.input.value, "Existing draft");
  assert.deepEqual(harness.input.events, []);
  assert.deepEqual(harness.edits, []);

  harness.document.activeElement = harness.document.body;
  harness.input.focus = () => {};
  const refused = await harness.send({ kind: "focus_text_field", field, allow_credentials: false });
  assert.equal(refused.ok, false);
  assert.equal(refused.error.code, "focus_lost");
  assert.match(refused.error.message, /did not retain browser input focus/);
  assert.equal(harness.input.value, "Existing draft");
});

test("focus_text_field refuses a control that does not take browser text input", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "checkbox";
  harness.input.setAttribute("aria-label", "Agree");
  const locator = await harness.locatorOf("Agree");

  const refused = await harness.send({ kind: "focus_text_field", field: { locator, value: "x" } });

  assert.equal(refused.error.code, "not_editable");
  assert.equal(harness.document.activeElement, null);
});

test("type_text replaces an unfocused ordinary or rich draft with one browser edit", async () => {
  for (const rich of [false, true]) {
    const harness = contentHarness();
    harness.document.hasFocus = () => false;
    const element = rich ? harness.element("div") : harness.input;
    element.hidden = false; element.type = "text";
    element.isContentEditable = rich;
    element.setAttribute("aria-label", "Draft");
    if (rich) {
      element.setAttribute("contenteditable", "true");
      element.textContent = "Original draft";
      harness.document.body.append(element);
    } else element.value = "Original draft";
    const locator = await harness.locatorOf("Draft");

    const result = await harness.send({ kind: "type_text", locator, text: "Replacement", clear_first: true });

    assert.equal(result.ok, true);
    assert.equal(rich ? element.textContent : element.value, "Replacement");
    assert.deepEqual(editSummary(harness), [{ command: "insertText", value: "Replacement" }], "there is no separate destructive clear");
    assert.deepEqual(element.events, ["input"], "typing does not fabricate input/change events");
    const cleared = await harness.send({ kind: "type_text", locator, text: "", clear_first: true });
    assert.equal(cleared.ok, true);
    assert.equal(rich ? element.textContent : element.value, "");
    assert.equal(harness.edits.at(-1).command, "delete");
  }
});

test("type_text preserves drafts when the target or browser refuses before editing", async () => {
  const expected = { disabled: "disabled", readonly: "read_only", credential: "credential_authorization_required", focus: "focus_lost", selection: "page_exception", browser: "page_exception" };
  for (const [refusal, code] of Object.entries(expected)) {
    const harness = contentHarness();
    const element = harness.input;
    textInput(harness, { value: "Retained draft" });
    const locator = await harness.locatorOf("Name");
    if (refusal === "disabled") element.disabled = true;
    if (refusal === "readonly") element.readOnly = true;
    if (refusal === "credential") element.type = "password";
    if (refusal === "focus") element.focus = () => { harness.document.activeElement = harness.document.body; };
    if (refusal === "selection") element.select = () => { throw new Error("selection unsupported"); };
    if (refusal === "browser") harness.document.execCommand = () => false;

    const result = await harness.send({ kind: "type_text", locator, text: "Replacement", clear_first: true });

    assert.equal(result.ok, false, refusal);
    assert.equal(result.error.code, code, refusal);
    assert.equal(element.value, "Retained draft", refusal);
    assert.equal(harness.edits.length, 0, refusal);
    assert.deepEqual(element.events, [], refusal);
  }
});

test("type_text rechecks the exact target after page focus or selection handlers run", async () => {
  for (const trigger of ["focus", "select"]) {
    const harness = contentHarness();
    const element = textInput(harness, { value: "Retained draft" });
    const locator = await harness.locatorOf("Name");
    const original = element[trigger].bind(element);
    element[trigger] = () => { original(); element.type = "password"; };

    const result = await harness.send({ kind: "type_text", locator, text: "Replacement", clear_first: true });

    assert.equal(result.error.code, "credential_authorization_required");
    assert.match(result.error.message, /authorization/);
    assert.equal(element.value, "Retained draft");
    assert.equal(harness.edits.length, 0);
  }
});

test("empty typing without explicit clear cannot dispatch a deletion", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "Retained draft" });
  const locator = await harness.locatorOf("Name");

  const result = await harness.send({ kind: "type_text", locator, text: "", clear_first: false });

  assert.equal(result.ok, true);
  assert.equal(harness.input.value, "Retained draft");
  assert.equal(harness.edits.length, 0);
  assert.deepEqual(harness.input.events, []);
});

test("typing without clear preserves the browser's existing input selection", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "Existing draft" });
  harness.input.select = () => { throw new Error("typing must not select all without explicit clear"); };
  const locator = await harness.locatorOf("Name");

  const result = await harness.send({ kind: "type_text", locator, text: "suffix", clear_first: false });

  assert.equal(result.ok, true);
  assert.deepEqual(editSummary(harness), [{ command: "insertText", value: "suffix" }]);
});

test("non-text input controls cannot route typing into a retained document selection", async () => {
  for (const type of ["checkbox", "radio", "submit", "file", "date", "color"]) {
    const harness = contentHarness();
    harness.input.hidden = false; harness.input.type = type; harness.input.value = "Retained value";
    const [target] = await harness.targets();

    const result = await harness.send({ kind: "type_text", locator: target.locator, text: "Replacement", clear_first: true });

    assert.equal(result.error.code, "not_editable", type);
    assert.match(result.error.message, /not text-editable/);
    assert.equal(harness.input.value, "Retained value", type);
    assert.equal(harness.edits.length, 0, type);
  }
});

test("a rich editor refusal preserves the existing draft", async () => {
  const cases = [["aria-readonly", "true", "read_only"], ["aria-disabled", "true", "disabled"], ["autocomplete", "current-password", "credential_authorization_required"]];
  for (const [attribute, value, code] of cases) {
    const harness = contentHarness();
    const editor = richEditor(harness, { content: "untouched" });
    editor.setAttribute(attribute, value);
    const locator = await harness.locatorOf("Reply");

    const result = await harness.send({ kind: "type_text", locator, text: "replacement", clear_first: true });

    assert.equal(result.error.code, code);
    assert.equal(editor.textContent, "untouched");
    assert.equal(harness.edits.length, 0);
  }
});

test("clear_focused clears the focused control and names it", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "stale" });
  harness.input.focus();

  const cleared = await harness.send({ kind: "clear_focused", allow_credentials: false });

  assert.equal(cleared.value.cleared, true);
  assert.equal(cleared.value.subject.name, "Name");
  assert.equal(harness.input.value, "");
  assert.equal(harness.edits.at(-1).command, "delete");
});

test("an empty focus is no_focused_control for describe, verify and clear", async () => {
  const harness = contentHarness();
  harness.document.activeElement = harness.document.body;
  assert.equal(await harness.codeOf({ kind: "describe_focused" }), "no_focused_control");
  assert.equal(await harness.codeOf({ kind: "clear_focused" }), "no_focused_control");
  harness.document.hasFocus = () => false;
  assert.equal(await harness.codeOf({ kind: "verify_focused_text", allow_credentials: false }), "no_focused_control");
  harness.document.activeElement = null;
  assert.equal(await harness.codeOf({ kind: "clear_focused" }), "no_focused_control");
});

test("focused password verification and clear honor only the current allowance", async () => {
  const harness = contentHarness();
  harness.input.hidden = false; harness.input.type = "password"; harness.input.value = "existing";
  harness.input.focus();
  for (const kind of ["verify_focused_text", "clear_focused"]) {
    assert.equal(await harness.codeOf({ kind }), "credential_authorization_required");
    assert.equal(harness.input.value, "existing");
    assert.equal(await harness.codeOf({ kind, allow_credentials: true }), null, kind);
  }
  assert.equal(harness.input.value, "");
  harness.document.hasFocus = () => false;
  assert.equal(await harness.codeOf({ kind: "verify_focused_text", allow_credentials: true }), "no_focused_control");
});

test("password typing requires an exact per-request allowance without retaining it", async () => {
  const harness = contentHarness();
  harness.input.hidden = false; harness.input.type = "password"; harness.input.value = "existing";
  const [target] = await harness.targets();
  const locator = target.locator;
  for (const allow_credentials of [undefined, false, "true", 1]) {
    const refused = await harness.send({ kind: "type_text", locator, text: "replacement", clear_first: true, allow_credentials });
    assert.equal(refused.error.code, "credential_authorization_required");
    assert.equal(harness.input.value, "existing");
  }
  const typed = await harness.send({ kind: "type_text", locator, text: "replacement", clear_first: true, allow_credentials: true });
  assert.equal(typed.ok, true);
  assert.equal(harness.input.value, "replacement");
  assert.equal(JSON.stringify(typed).includes("replacement"), false);
  const later = await harness.send({ kind: "type_text", locator, text: "unacknowledged", clear_first: true });
  assert.equal(later.ok, false);
  assert.equal(harness.input.value, "replacement");
});

test("credential guidance names the exact control", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "password";
  harness.input.setAttribute("aria-label", "Master password");
  const locator = await harness.locatorOf("Master password");

  const refused = await harness.send({ kind: "focus_text_field", field: { locator, value: "hunter2" } });

  assert.equal(refused.error.code, "credential_authorization_required");
  assert.match(refused.error.message, /requires user authorization: the textbox "Master password"/);
});

test("credential allowance survives every staged fill check and does not weaken actionability", async () => {
  const harness = contentHarness();
  harness.input.hidden = false; harness.input.type = "password"; harness.input.value = "existing";
  const [target] = await harness.targets();
  const field = { locator: target.locator, value: "existing" };
  const messages = [
    { kind: "prepare_fill", fields: [field], visual_settle: false },
    { kind: "focus_text_field", field },
    { kind: "verify_fill_retained", fields: [field], stable_ms: 0, poll_ms: 5, limit_ms: 50 }
  ];
  for (const message of messages) {
    assert.equal(await harness.codeOf(message), "credential_authorization_required", message.kind);
    assert.equal(await harness.codeOf({ ...message, allow_credentials: true }), null, message.kind);
    assert.equal(harness.input.value, "existing");
  }
  harness.input.readOnly = true;
  assert.equal(await harness.codeOf({ kind: "focus_text_field", field, allow_credentials: true }), "read_only");
});

// ---- fill --------------------------------------------------------------------------------------

test("fill preparation checks every field before any native edit or input event", async () => {
  for (const [attribute, value] of [["aria-readonly", "true"], ["aria-disabled", "true"], ["aria-hidden", "true"], ["autocomplete", "current-password"]]) {
    const harness = contentHarness();
    textInput(harness, { label: "Ordinary", value: "Existing ordinary draft" });
    const editor = richEditor(harness, { label: "Protected reply", content: "Existing controlled draft" });
    editor.setAttribute(attribute, value);
    const ordinary = await harness.locatorOf("Ordinary");
    const protectedTarget = await harness.locatorOf("Protected reply");

    const result = await harness.send({
      kind: "prepare_fill",
      fields: [{ locator: ordinary, value: "MUST_NOT_CHANGE" }, { locator: protectedTarget, value: "MUST_NOT_CHANGE" }],
      visual_settle: false
    });

    assert.equal(result.ok, false, attribute);
    assert.equal(harness.input.value, "Existing ordinary draft");
    assert.equal(editor.textContent, "Existing controlled draft");
    assert.deepEqual(harness.input.events, []);
    assert.deepEqual(harness.edits, []);
  }
});

test("successful fill preparation changes nothing and reports each field's input path", async () => {
  const harness = contentHarness();
  textInput(harness, { label: "Ordinary", value: "Original input" });
  const editor = richEditor(harness, { content: "Original rich draft" });
  const select = harness.select([{ value: "us", text: "United States" }]);
  select.setAttribute("aria-label", "Country");
  harness.document.body.append(select);
  const fields = [];
  for (const name of ["Ordinary", "Reply", "Country"]) fields.push({ locator: await harness.locatorOf(name), value: "x" });
  fields[2].value = "us";

  const started = harness.dom.context.performance.now();
  const prepared = await harness.send({ kind: "prepare_fill", fields });
  const elapsed = harness.dom.context.performance.now() - started;

  assert.deepEqual(prepared.value, { prepared: true, field_kinds: ["browser_text", "browser_text", "local"] });
  assert.ok(elapsed >= 750 && elapsed < 6000, `waited for the form to settle (${elapsed} ms)`);
  assert.equal(harness.input.value, "Original input");
  assert.equal(editor.textContent, "Original rich draft");
  assert.deepEqual(harness.edits, []);
  assert.deepEqual(harness.input.events, []);
});

test("prepare_fill with visual_settle false answers at once", async () => {
  const harness = contentHarness();
  textInput(harness);
  const locator = await harness.locatorOf("Name");
  const before = harness.dom.context.performance.now();
  const prepared = await harness.send({ kind: "prepare_fill", fields: [{ locator, value: "x" }], visual_settle: false });
  assert.equal(prepared.ok, true);
  assert.equal(harness.dom.context.performance.now(), before);
});

test("fill_local sets a select by its visible label or value and verify_fill_retained confirms it", async () => {
  const harness = contentHarness();
  const select = harness.select([{ value: "us", text: "United States" }, { value: "de", text: "Germany" }]);
  select.setAttribute("aria-label", "Country");
  harness.document.body.append(select);
  const locator = await harness.locatorOf("Country");
  const verify = (value) => ({ kind: "verify_fill_retained", fields: [{ locator, value }], stable_ms: 50, poll_ms: 10, limit_ms: 200 });

  for (const value of ["Germany", "de"]) {
    select.value = "us";
    const filled = await harness.send({ kind: "fill_local", field: { locator, value }, allow_credentials: false });
    assert.deepEqual(filled, { ok: true, value: { filled: true } });
    assert.equal(select.value, "de");
    assert.deepEqual(select.events, value === "Germany" ? ["input", "change"] : ["input", "change", "input", "change"]);
    assert.equal((await harness.send(verify(value))).ok, true);
  }
  select.value = "us";
  assert.equal(await harness.codeOf(verify("Germany")), "not_retained");
});

test("fill_local toggles checkboxes and sets non-text inputs, firing input then change", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "checkbox";
  harness.input.checked = false;
  harness.input.setAttribute("aria-label", "Agree");
  const locator = await harness.locatorOf("Agree");

  await harness.send({ kind: "fill_local", field: { locator, value: "yes" }, allow_credentials: false });
  assert.equal(harness.input.checked, true);
  assert.deepEqual(harness.input.events, ["input", "change"]);
  await harness.send({ kind: "fill_local", field: { locator, value: "false" }, allow_credentials: false });
  assert.equal(harness.input.checked, false);

  harness.input.type = "date";
  harness.input.setAttribute("aria-label", "Birthday");
  await harness.send({ kind: "fill_local", field: { locator, value: "2000-01-02" }, allow_credentials: false });
  assert.equal(harness.input.value, "2000-01-02");
});

test("fill_local refuses text controls, which belong to the browser's trusted input", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "keep" });
  const locator = await harness.locatorOf("Name");
  const refused = await harness.send({ kind: "fill_local", field: { locator, value: "x" }, allow_credentials: false });
  assert.equal(refused.error.code, "invalid_request");
  assert.equal(harness.input.value, "keep");
});

test("verify_fill_retained is ok once every field has held its value for stable_ms", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "nope" });
  const locator = await harness.locatorOf("Name");
  const clock = () => harness.dom.context.performance.now();
  const pending = harness.runtime.call({
    kind: "verify_fill_retained",
    fields: [{ locator, value: "Ada" }],
    allow_credentials: false,
    stable_ms: 250,
    poll_ms: 25,
    limit_ms: 3000
  }, {});
  let answered = null;
  pending.then((reply) => { answered = plain(reply); });
  let matched = null;
  for (let step = 0; step < 400 && !answered; step += 1) {
    await tick();
    harness.dom.advance(STEP_MS);
    if (matched === null && clock() >= 100) {
      harness.input.value = "Ada";
      matched = clock();
    }
  }
  await tick();

  assert.deepEqual(answered, { ok: true, value: { retained: true } });
  const held = clock() - matched;
  assert.ok(held >= 250 && held < 350, `held for ${held} ms`);
});

test("verify_fill_retained restarts the stable window when a value flips back", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "Ada" });
  const locator = await harness.locatorOf("Name");
  const clock = () => harness.dom.context.performance.now();
  let answered = null;
  harness.runtime.call({
    kind: "verify_fill_retained",
    fields: [{ locator, value: "Ada" }],
    stable_ms: 250,
    poll_ms: 25,
    limit_ms: 3000
  }, {}).then((reply) => { answered = plain(reply); });
  let flipped = null;
  for (let step = 0; step < 400 && !answered; step += 1) {
    await tick();
    harness.dom.advance(STEP_MS);
    if (flipped === null && clock() >= 125) {
      harness.input.value = "rolled back";
      flipped = clock();
    } else if (flipped !== null && clock() >= flipped + STEP_MS * 2 && harness.input.value !== "Ada") {
      harness.input.value = "Ada";
    }
  }
  await tick();

  assert.equal(answered.ok, true);
  assert.ok(clock() > 125 + 250, "the window restarted after the rollback");
});

test("verify_fill_retained fails with not_retained at its limit", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "other" });
  const locator = await harness.locatorOf("Name");
  const started = harness.dom.context.performance.now();

  const reply = await harness.send({ kind: "verify_fill_retained", fields: [{ locator, value: "Ada" }], stable_ms: 250, poll_ms: 25, limit_ms: 200 });

  assert.equal(reply.error.code, "not_retained");
  const elapsed = harness.dom.context.performance.now() - started;
  assert.ok(elapsed >= 200 && elapsed < 300, `gave up after ${elapsed} ms`);
});

test("verify_fill_retained validates its time budget and its fields", async () => {
  const harness = contentHarness();
  textInput(harness);
  const locator = await harness.locatorOf("Name");
  const fields = [{ locator, value: "" }];
  assert.equal(await harness.codeOf({ kind: "verify_fill_retained", fields }), "invalid_request");
  assert.equal(await harness.codeOf({ kind: "verify_fill_retained", fields: [{ locator: "locator_ffffffff_1", value: "" }], limit_ms: 10 }), "stale_locator");
});

test("an empty rich editor counts as retained when its text is blank", async () => {
  const harness = contentHarness();
  const editor = richEditor(harness);
  const locator = await harness.locatorOf("Reply");
  editor.innerText = "\n";
  const verified = await harness.send({ kind: "verify_fill_retained", fields: [{ locator, value: "" }], stable_ms: 0, poll_ms: 5, limit_ms: 50 });
  assert.equal(verified.ok, true);
});

test("submit_fill answers before it clicks, and only the verified submit control is clicked", async () => {
  const harness = contentHarness();
  textInput(harness);
  harness.input.closest = (selector) => (selector === "form" ? { contains: () => true } : null);
  const locator = await harness.locatorOf("Name");
  const order = [];
  harness.input.click = () => order.push("submit");

  const reply = await harness.send(
    { kind: "submit_fill", fields: [{ locator, value: "x" }], submit_locator: locator, allow_credentials: false },
    {},
    (phase) => order.push(phase)
  );
  assert.deepEqual(reply, { ok: true, value: { submitted: true } });
  assert.deepEqual(order, ["reply"], "the click is the tail of the call");
  harness.dom.advance(0);
  assert.deepEqual(order, ["reply", "submit"]);
});

test("a submit control outside the resolved form refuses before any click", async () => {
  const harness = contentHarness();
  textInput(harness);
  const locator = await harness.locatorOf("Name");
  let clicked = false;
  harness.input.click = () => { clicked = true; };

  const refused = await harness.send({ kind: "submit_fill", fields: [{ locator, value: "x" }], submit_locator: locator, allow_credentials: false });
  harness.dom.advance(10);

  assert.equal(refused.error.code, "submit_outside_form");
  assert.match(refused.error.message, /not contained/);
  assert.equal(clicked, false);
  assert.equal(harness.input.value, "");
  assert.deepEqual(harness.input.events, []);
});

test("submit_fill without a submit locator is an invalid request", async () => {
  const harness = contentHarness();
  textInput(harness);
  const locator = await harness.locatorOf("Name");
  assert.equal(await harness.codeOf({ kind: "submit_fill", fields: [{ locator, value: "x" }], allow_credentials: false }), "invalid_request");
});

// ---- observe -----------------------------------------------------------------------------------

test("observation polling stops at its physical timeout without overshooting", async () => {
  const harness = contentHarness();
  let answered = null;
  harness.runtime.call({ kind: "observe", condition: "text_present", value: "never present", timeout_ms: 250 }, {})
    .then((reply) => { answered = plain(reply); });
  for (const step of [100, 100, 50]) {
    await tick();
    harness.dom.advance(step);
  }
  await tick();

  assert.deepEqual(answered, { ok: true, value: { satisfied: false, elapsed_ms: 250, readiness: "complete" } });
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

  const observed = await harness.send({ kind: "observe", condition: "text_present", value: "Shadow task complete", timeout_ms: 0 });

  assert.equal(observed.value.satisfied, true);
  assert.equal(observed.value.elapsed_ms, 0);
});

test("until_satisfied turns an unmet condition into the unsatisfied failure", async () => {
  const harness = contentHarness();
  const message = { kind: "observe", condition: "text_present", value: "never present", timeout_ms: 100 };

  const plainReply = await harness.send(message);
  assert.equal(plainReply.value.satisfied, false);

  const raced = await harness.send({ ...message, until_satisfied: true });
  assert.equal(raced.ok, false);
  assert.equal(raced.error.code, "unsatisfied");

  const met = await harness.send({ kind: "observe", condition: "load_ready", timeout_ms: 100, until_satisfied: true });
  assert.equal(met.value.satisfied, true);
});

test("until_satisfied also covers a visual settle that never settles", async () => {
  const harness = contentHarness();
  textInput(harness);
  const locator = await harness.locatorOf("Name");
  let width = 1;
  harness.input.getBoundingClientRect = () => ({ x: 0, y: 0, width: (width += 1), height: 1 });
  const reply = await harness.send({ kind: "observe", condition: "visual_settle", locator, timeout_ms: 200, until_satisfied: true });
  assert.equal(reply.error.code, "unsatisfied");
});

// ---- cancel tokens -------------------------------------------------------------------------------

async function cancelled(harness, message, token) {
  const pending = harness.runtime.call(message, { token });
  assert.equal(harness.runtime.cancel(token), true, "the call registered its token");
  for (let step = 0; step < 40; step += 1) {
    await tick();
    harness.dom.advance(STEP_MS);
  }
  return plain(await pending);
}

test("a cancel token ends observe", async () => {
  const harness = contentHarness();
  const reply = await cancelled(harness, { kind: "observe", condition: "text_present", value: "never", timeout_ms: 60_000 }, "t-observe");
  assert.equal(reply.error?.code, "cancelled");
  assert.equal(harness.dom.pendingTimers(), 0);
});

test("a cancel token ends a visual observe", async () => {
  const harness = contentHarness();
  const reply = await cancelled(harness, { kind: "observe", condition: "visual_settle", timeout_ms: 60_000 }, "t-visual");
  assert.equal(reply.error?.code, "cancelled");
  assert.equal(harness.dom.pendingTimers(), 0);
});

test("a cancel token ends prepare_fill while it waits for the form to settle", async () => {
  const harness = contentHarness();
  textInput(harness);
  const locator = await harness.locatorOf("Name");
  const reply = await cancelled(harness, { kind: "prepare_fill", fields: [{ locator, value: "x" }] }, "t-prepare");
  assert.equal(reply.error?.code, "cancelled");
  assert.equal(harness.dom.pendingTimers(), 0);
  assert.deepEqual(harness.edits, []);
});

test("a cancel token ends verify_fill_retained", async () => {
  const harness = contentHarness();
  textInput(harness, { value: "other" });
  const locator = await harness.locatorOf("Name");
  const reply = await cancelled(harness, { kind: "verify_fill_retained", fields: [{ locator, value: "Ada" }], stable_ms: 250, poll_ms: 25, limit_ms: 60_000 }, "t-retained");
  assert.equal(reply.error?.code, "cancelled");
  assert.equal(harness.dom.pendingTimers(), 0);
});

test("a cancel token ends a sensor settle", async () => {
  const harness = contentHarness();
  harness.setBody(harness.element("body"));
  const reply = await cancelled(harness, { kind: "read_text", mode: "visible", max_chars: 100, settle_ms: 60_000 }, "t-settle");
  assert.equal(reply.error?.code, "cancelled");
  assert.equal(harness.dom.activeObservers(), 0);
  assert.equal(harness.dom.pendingTimers(), 0);
});

// ---- locators ------------------------------------------------------------------------------------

test("locators carry the world's epoch and are minted in order", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  for (let index = 0; index < 3; index += 1) {
    const item = harness.element("button");
    item.setAttribute("aria-label", `Item ${index}`);
    body.append(item);
  }
  harness.setBody(body);
  const found = await harness.targets();
  const pattern = /^locator_[0-9a-f]{8}_(\d+)$/;
  assert.ok(found.every((target) => pattern.test(target.locator)));
  const epochs = new Set(found.map((target) => target.locator.split("_")[1]));
  assert.equal(epochs.size, 1);
  const numbers = found.map((target) => Number(pattern.exec(target.locator)[1]));
  assert.deepEqual(numbers, numbers.map((_, index) => numbers[0] + index));
});

test("a locator from another world or an unminted one is stale_locator", async () => {
  const first = contentHarness();
  const second = contentHarness();
  const foreign = (await first.targets())[0].locator;
  const own = (await second.targets())[0].locator;
  assert.notEqual(foreign.split("_")[1], own.split("_")[1], "each world draws its own epoch");

  for (const kind of ["describe", "box", "activate", "focus", "hover", "scroll"]) {
    const message = kind === "describe" ? { kind, locators: [foreign] } : { kind, locator: foreign };
    assert.equal(await second.codeOf(message), "stale_locator", kind);
  }
  assert.equal(await second.codeOf({ kind: "describe", locators: ["locator_1"] }), "stale_locator");
  assert.equal(await second.codeOf({ kind: "describe", locators: [`locator_${own.split("_")[1]}_999`] }), "stale_locator");
  assert.equal(await second.codeOf({ kind: "describe", locators: [own] }), null);
});

test("a locator for a removed element is stale_locator", async () => {
  const harness = contentHarness();
  const locator = (await harness.targets())[0].locator;
  harness.input.isConnected = false;
  assert.equal(await harness.codeOf({ kind: "describe", locators: [locator] }), "stale_locator");
  assert.equal(await harness.codeOf({ kind: "box", locator }), "stale_locator");
});

// ---- coded throw sites ---------------------------------------------------------------------------

test("every throw site carries its code and keeps its message", async () => {
  const cases = [];
  const harness = contentHarness();
  const reply = async (message, options) => (await harness.send(message, options)).error;
  const check = async (label, message, code, pattern, options) => {
    const error = await reply(message, options);
    cases.push(label);
    assert.equal(error?.code, code, label);
    assert.match(error.message, pattern, label);
  };

  const textLocator = async (attributes) => {
    const element = harness.newInput(attributes.label, attributes.props);
    for (const [name, value] of Object.entries(attributes.set ?? {})) element.setAttribute(name, value);
    return { element, locator: await harness.locatorOf(attributes.label) };
  };

  await check("resolve", { kind: "describe", locators: ["locator_aaaaaaaa_1"] }, "stale_locator", /stale browser locator/);

  const hiddenOne = await textLocator({ label: "Hidden", props: { hidden: true } });
  await check("requireActionable not_visible", { kind: "activate", locator: hiddenOne.locator }, "not_visible", /not visible for activate \(display:none\)/);
  const disabledOne = await textLocator({ label: "Disabled", props: { disabled: true } });
  await check("requireActionable disabled", { kind: "activate", locator: disabledOne.locator }, "disabled", /disabled for activate/);
  const readOnlyOne = await textLocator({ label: "Readonly", props: { readOnly: true } });
  await check("requireActionable read_only", { kind: "type_text", locator: readOnlyOne.locator, text: "x", clear_first: true }, "read_only", /read-only for type/);

  const secret = await textLocator({ label: "Secret", set: { autocomplete: "current-password" } });
  await check("credentialAuthorizationError", { kind: "type_text", locator: secret.locator, text: "x", clear_first: true }, "credential_authorization_required", /requires user authorization/);

  const generic = harness.element("div");
  generic.setAttribute("role", "button");
  generic.setAttribute("aria-label", "Generic");
  harness.document.body.append(generic);
  const genericLocator = await harness.locatorOf("Generic");
  await check("validateFillElement not_fillable", { kind: "fill_local", field: { locator: genericLocator, value: "x" }, allow_credentials: false }, "not_fillable", /not fillable/);
  await check("validateTextEditElement not_editable", { kind: "type_text", locator: genericLocator, text: "x", clear_first: true }, "not_editable", /not text-editable/);

  const select = harness.select([{ value: "a", text: "A" }]);
  select.setAttribute("aria-label", "Pick");
  harness.document.body.append(select);
  await check("validateFillElement option_missing", { kind: "fill_local", field: { locator: await harness.locatorOf("Pick"), value: "zzz" }, allow_credentials: false }, "option_missing", /select option not found/);

  await check("requireUploadTarget not_file_input", { kind: "upload_files", locator: genericLocator, files: [FILE] }, "not_file_input", /not a file input/);
  harness.input.disabled = true;
  await check("requireUploadTarget disabled", { kind: "upload_files", locator: (await harness.targets()).find((t) => t.name === "").locator, files: [FILE] }, "disabled", /disabled for upload/);
  harness.input.disabled = false;

  const form = { contains: () => false };
  const inForm = await textLocator({ label: "In form", props: { closest: (selector) => (selector === "form" ? form : null) } });
  await check("prepareFill submit_outside_form", { kind: "submit_fill", fields: [{ locator: inForm.locator, value: "x" }], submit_locator: inForm.locator, allow_credentials: false }, "submit_outside_form", /not contained in the resolved form/);

  harness.document.activeElement = harness.document.body;
  await check("describe_focused", { kind: "describe_focused" }, "no_focused_control", /no editable control is focused/);
  harness.document.hasFocus = () => false;
  await check("verify_focused_text", { kind: "verify_focused_text" }, "no_focused_control", /no editable control is focused/);
  harness.document.hasFocus = () => true;

  const slippery = await textLocator({ label: "Slippery", props: { focus: () => {} } });
  await check("verifyBrowserTextFocus", { kind: "focus_text_field", field: { locator: slippery.locator, value: "x" } }, "focus_lost", /did not retain browser input focus/);

  harness.setScroll(0, 0);
  await check("viewport_point", { kind: "viewport_point", x: -5, y: 5 }, "outside_viewport", /outside the current viewport/);
  harness.document.elementFromPoint = () => null;
  await check("hit_point", { kind: "hit_point", x: 1, y: 1, page: false }, "no_subject", /no current document subject/);
  await check("drop_files no_subject", { kind: "drop_files", x: 1, y: 1, files: [FILE] }, "no_subject", /no element is at the drop point/);
  harness.document.elementFromPoint = () => harness.element("div");
  await check("decodeFile", { kind: "drop_files", x: 1, y: 1, files: [{ ...FILE, size: 6 }] }, "payload_mismatch", /size mismatch/);

  await check("unknown kind", { kind: "no_such_kind" }, "unknown_kind", /unknown content primitive/);
  assert.equal(cases.length, 19);
});

// ---- effect kinds refuse before they mutate -------------------------------------------------------

/**
 * One refusal per scenario. Each effect kind appears at least once; the page must be untouched
 * afterwards: no native edit, no event, no value or checked write, no file, no scroll.
 */
function effectScenarios(harness) {
  const watched = [];
  const track = (element) => { watched.push(element); return element; };
  const writes = [];
  const spyOn = (element, property) => {
    let stored = element[property];
    Object.defineProperty(element, property, {
      get: () => stored,
      set: (next) => { writes.push(`${property}=${next}`); stored = next; },
      configurable: true
    });
  };
  const textField = (label, props = {}) => {
    const element = track(harness.newInput(label, { value: "kept", ...props }));
    spyOn(element, "value");
    spyOn(element, "checked");
    return element;
  };
  const locatorFor = (label) => harness.locatorOf(label);
  const missing = "locator_ffffffff_1";
  const plainFile = { ...FILE };
  return {
    watched,
    writes,
    list: [
      ["fill_local", "stale_locator", async () => ({ kind: "fill_local", field: { locator: missing, value: "x" }, allow_credentials: false })],
      ["fill_local", "not_fillable", async () => {
        const div = track(harness.element("div")); div.setAttribute("role", "button"); div.setAttribute("aria-label", "Div"); harness.document.body.append(div);
        return { kind: "fill_local", field: { locator: await locatorFor("Div"), value: "x" }, allow_credentials: false };
      }],
      ["fill_local", "option_missing", async () => {
        const select = track(harness.select([{ value: "a", text: "A" }])); select.setAttribute("aria-label", "Sel"); harness.document.body.append(select); spyOn(select, "value");
        return { kind: "fill_local", field: { locator: await locatorFor("Sel"), value: "nope" }, allow_credentials: false };
      }],
      ["fill_local", "disabled", async () => {
        textField("Box", { type: "checkbox", disabled: true });
        return { kind: "fill_local", field: { locator: await locatorFor("Box"), value: "yes" }, allow_credentials: false };
      }],
      ["fill_local", "credential_authorization_required", async () => {
        const date = textField("Token date", { type: "date" }); date.setAttribute("name", "api_token");
        return { kind: "fill_local", field: { locator: await locatorFor("Token date"), value: "2000-01-01" }, allow_credentials: false };
      }],
      ["focus_text_field", "stale_locator", async () => ({ kind: "focus_text_field", field: { locator: missing, value: "x" }, allow_credentials: false })],
      ["focus_text_field", "not_visible", async () => {
        textField("Ghost", { hidden: true });
        return { kind: "focus_text_field", field: { locator: await locatorFor("Ghost"), value: "x" }, allow_credentials: false };
      }],
      ["focus_text_field", "not_editable", async () => {
        textField("Check", { type: "checkbox" });
        return { kind: "focus_text_field", field: { locator: await locatorFor("Check"), value: "x" }, allow_credentials: false };
      }],
      ["focus_text_field", "credential_authorization_required", async () => {
        textField("Pass", { type: "password" });
        return { kind: "focus_text_field", field: { locator: await locatorFor("Pass"), value: "x" }, allow_credentials: false };
      }],
      ["focus_text_field", "focus_lost", async () => {
        textField("Slip", { focus: () => {} });
        return { kind: "focus_text_field", field: { locator: await locatorFor("Slip"), value: "x" }, allow_credentials: false };
      }],
      ["submit_fill", "stale_locator", async () => ({ kind: "submit_fill", fields: [{ locator: missing, value: "x" }], submit_locator: missing, allow_credentials: false })],
      ["submit_fill", "submit_outside_form", async () => {
        textField("Alone");
        const locator = await locatorFor("Alone");
        return { kind: "submit_fill", fields: [{ locator, value: "x" }], submit_locator: locator, allow_credentials: false };
      }],
      ["submit_fill", "credential_authorization_required", async () => {
        textField("Secret", { type: "password" });
        const locator = await locatorFor("Secret");
        return { kind: "submit_fill", fields: [{ locator, value: "x" }], submit_locator: locator, allow_credentials: false };
      }],
      ["submit_fill", "invalid_request", async () => {
        textField("Plain");
        return { kind: "submit_fill", fields: [{ locator: await locatorFor("Plain"), value: "x" }], allow_credentials: false };
      }],
      ["type_text", "stale_locator", async () => ({ kind: "type_text", locator: missing, text: "x", clear_first: true, allow_credentials: false })],
      ["type_text", "not_editable", async () => {
        textField("Radio", { type: "radio" });
        return { kind: "type_text", locator: await locatorFor("Radio"), text: "x", clear_first: true, allow_credentials: false };
      }],
      ["type_text", "disabled", async () => {
        textField("Off", { disabled: true });
        return { kind: "type_text", locator: await locatorFor("Off"), text: "x", clear_first: true, allow_credentials: false };
      }],
      ["type_text", "read_only", async () => {
        textField("Locked", { readOnly: true });
        return { kind: "type_text", locator: await locatorFor("Locked"), text: "x", clear_first: true, allow_credentials: false };
      }],
      ["type_text", "credential_authorization_required", async () => {
        textField("Code", { type: "password" });
        return { kind: "type_text", locator: await locatorFor("Code"), text: "x", clear_first: true, allow_credentials: false };
      }],
      ["type_text", "not_visible", async () => {
        textField("Gone", { hidden: true });
        return { kind: "type_text", locator: await locatorFor("Gone"), text: "x", clear_first: true, allow_credentials: false };
      }],
      ["type_text", "focus_lost", async () => {
        textField("Wander", { focus: () => { harness.document.activeElement = harness.document.body; } });
        return { kind: "type_text", locator: await locatorFor("Wander"), text: "x", clear_first: true, allow_credentials: false };
      }],
      ["clear_focused", "no_focused_control", async () => {
        harness.document.activeElement = harness.document.body;
        return { kind: "clear_focused", allow_credentials: false };
      }],
      ["clear_focused", "credential_authorization_required", async () => {
        const field = textField("Focused secret", { type: "password" });
        field.focus();
        return { kind: "clear_focused", allow_credentials: false };
      }],
      ["clear_focused", "read_only", async () => {
        const field = textField("Focused locked", { readOnly: true });
        field.focus();
        return { kind: "clear_focused", allow_credentials: false };
      }],
      ["scroll", "stale_locator", async () => ({ kind: "scroll", locator: missing })],
      ["scroll", "not_visible", async () => {
        textField("Offscreen", { hidden: true });
        return { kind: "scroll", locator: await locatorFor("Offscreen") };
      }],
      ["upload_files", "stale_locator", async () => ({ kind: "upload_files", locator: missing, files: [plainFile] })],
      ["upload_files", "not_file_input", async () => {
        textField("Not a file");
        return { kind: "upload_files", locator: await locatorFor("Not a file"), files: [plainFile] };
      }],
      ["upload_files", "disabled", async () => {
        harness.input.disabled = true;
        return { kind: "upload_files", locator: (await harness.targets())[0].locator, files: [plainFile] };
      }],
      ["upload_files", "payload_mismatch", async () => ({ kind: "upload_files", locator: (await harness.targets())[0].locator, files: [{ ...plainFile, size: 9 }] })],
      ["drop_files", "no_subject", async () => {
        harness.document.elementFromPoint = () => null;
        return { kind: "drop_files", x: 1, y: 1, files: [plainFile] };
      }],
      ["drop_files", "payload_mismatch", async () => {
        harness.document.elementFromPoint = () => track(harness.element("div"));
        return { kind: "drop_files", x: 1, y: 1, files: [{ ...plainFile, size: 9 }] };
      }],
      ["attach_file", "invalid_request", async () => ({ kind: "attach_file", locator: (await harness.targets())[0].locator, file_name: "a.gif" })],
      ["attach_file", "stale_locator", async () => ({ kind: "attach_file", locator: missing, file_name: "a.gif" }), { attachment: { media_type: "image/gif", data: "aGVsbG8=", size: 5 } }],
      ["attach_file", "not_file_input", async () => {
        textField("Not a file either");
        return { kind: "attach_file", locator: await locatorFor("Not a file either"), file_name: "a.gif" };
      }, { attachment: { media_type: "image/gif", data: "aGVsbG8=", size: 5 } }],
      ["attach_file", "payload_mismatch", async () => ({ kind: "attach_file", locator: (await harness.targets())[0].locator, file_name: "a.gif" }), { attachment: { media_type: "image/gif", data: "aGVsbG8=", size: 1 } }]
    ]
  };
}

const EFFECT_KINDS = ["fill_local", "focus_text_field", "submit_fill", "type_text", "clear_focused", "scroll", "upload_files", "drop_files", "attach_file"];

test("every effect kind has refusal scenarios, and each refusal is a pre-effect code", () => {
  const scenarios = effectScenarios(contentHarness()).list;
  for (const kind of EFFECT_KINDS) {
    assert.ok(scenarios.some(([name]) => name === kind), `${kind} has a scenario`);
  }
  for (const [kind, code] of scenarios) {
    assert.ok(vocabulary.pre_effect_codes.includes(code), `${kind}/${code} is a pre-effect code`);
  }
});

test("a pre-effect code is thrown before any mutation for every effect kind", async () => {
  const names = effectScenarios(contentHarness()).list.map(([kind, code]) => `${kind}/${code}`);
  for (let index = 0; index < names.length; index += 1) {
    const harness = contentHarness();
    const { list, watched, writes } = effectScenarios(harness);
    const [kind, code, build, options] = list[index];
    harness.input.hidden = harness.input.hidden;
    watched.push(harness.input);
    const message = await build();
    assert.equal(message.kind, kind);
    const filesBefore = harness.input.files.length;
    const reply = await harness.send(message, options ?? {});
    harness.dom.advance(10);

    const label = names[index];
    assert.equal(reply.ok, false, label);
    assert.equal(reply.error.code, code, `${label}: ${reply.error?.message}`);
    assert.deepEqual(harness.edits, [], `${label}: no execCommand`);
    assert.deepEqual(writes, [], `${label}: no value or checked write`);
    assert.deepEqual(harness.scrolls, [], `${label}: no scroll`);
    assert.equal(harness.input.files.length, filesBefore, `${label}: no file attached`);
    for (const element of watched) assert.deepEqual(element.events, [], `${label}: no event`);
  }
});

test("a pre-effect refusal in a multi-field fill leaves the earlier fields alone", async () => {
  const harness = contentHarness();
  textInput(harness, { label: "First", value: "one" });
  const second = harness.newInput("Second", { type: "password", value: "two" });
  const fields = [
    { locator: await harness.locatorOf("First"), value: "A" },
    { locator: await harness.locatorOf("Second"), value: "B" }
  ];
  const reply = await harness.send({ kind: "submit_fill", fields, submit_locator: fields[0].locator, allow_credentials: false });
  assert.equal(reply.error.code, "credential_authorization_required");
  assert.equal(harness.input.value, "one");
  assert.equal(second.value, "two");
  assert.deepEqual(harness.edits, []);
});

// ---- the vocabulary ---------------------------------------------------------------------------------

test("the dispatched kinds equal the vocabulary's world kinds exactly", () => {
  const source = bundle.read("content.js");
  const dispatched = [...source.matchAll(/message\.kind === "([a-z_]+)"/g)].map((match) => match[1]);
  assert.equal(new Set(dispatched).size, dispatched.length, "no kind is dispatched twice");
  assert.deepEqual([...dispatched].sort(), [...vocabulary.world_message_kinds].sort());
});

// The locator the golden fixtures carry is minted for a page that no longer exists; a message
// that kept it would stop at stale_locator before any other field was read. Each page below mints
// live locators for the roles the fixtures give their locators, and `bind` swaps them in.
const GOLDEN_LOCATOR = /^locator_a1b2c3d4_(\d+)$/;

/** A page holding one live element for every locator role the golden messages use. */
async function goldenPage() {
  const harness = contentHarness();
  const owner = embedFixture(harness);
  harness.document.elementFromPoint = () => owner;
  const panel = harness.newInput("Panel");
  const name = harness.newInput("Name", { value: "Ada Lovelace" });
  const email = harness.newInput("Email", { value: "ada@example.com" });
  const send = harness.newInput("Send", { type: "submit" });
  const country = harness.select([{ value: "us", text: "United States" }, { value: "ca", text: "Canada" }]);
  country.setAttribute("aria-label", "Country");
  harness.document.body.append(country);
  harness.document.body.append(harness.text("Saved"));
  const form = { contains: (element) => element === send };
  name.closest = (selector) => (selector === "form" ? form : null);
  const selects = [];
  const select = name.select.bind(name);
  name.select = () => { selects.push(name); select(); };

  const [file] = await harness.targets();
  const minted = {
    1: panel, 2: panel, 3: name, 4: email, 5: send, 6: name, 7: email, 8: send, 9: name, 10: country, 11: panel
  };
  const locators = { 12: file.locator };
  for (const [role, element] of Object.entries(minted)) {
    const label = element.getAttribute("aria-label");
    locators[role] = (await harness.targets()).find((target) => target.name === label).locator;
  }
  return {
    harness, owner, name, email, send, panel, selects,
    /** The message with every golden locator replaced by the live locator of the same role. */
    bind(message) {
      return JSON.parse(JSON.stringify(message), (_key, value) => {
        const match = typeof value === "string" ? GOLDEN_LOCATOR.exec(value) : null;
        if (!match) return value;
        assert.ok(locators[match[1]], `golden locator role ${match[1]} has a live element`);
        return locators[match[1]];
      });
    },
    locatorOf: (role) => locators[role]
  };
}

/** The message with `key` replaced, after proving the golden message carries that key. */
function withKey(message, key, value) {
  assert.ok(Object.hasOwn(message, key), `${message.kind} golden carries ${key}`);
  return { ...message, [key]: value };
}

/** Kinds that read the focused control, so the page focuses one first. */
const FOCUSED_KINDS = new Set(["clear_focused", "describe_focused", "verify_focused_text", "require_focused", "focus_probe"]);

/** Kinds whose reply carries a value the fixture's field names decide; each is checked by name. */
const GOLDEN_PROBES = {
  async attach_file(page, message, options) {
    const reply = await page.harness.send(page.bind(message), options);
    assert.equal(reply.ok, true);
    assert.equal(page.harness.input.files[0].name, message.file_name, "file_name names the attached file");
  },
  async type_text(page, message) {
    page.name.value = "Existing";
    const reply = await page.harness.send(page.bind(message));
    assert.equal(reply.ok, true);
    assert.equal(page.selects.length, 1, "clear_first selects the existing text");
    const unchanged = await page.harness.send(page.bind(withKey(message, "clear_first", false)));
    assert.equal(unchanged.ok, true);
    assert.equal(page.selects.length, 1, "clear_first false keeps the existing text");
  },
  async prepare_fill(page, message) {
    const elsewhere = page.locatorOf(11);
    const reply = await page.harness.send(page.bind(withKey(message, "submit_locator", elsewhere)));
    assert.equal(reply.error?.code, "submit_outside_form", "submit_locator names the control that must sit in the form");
  },
  async verify_fill_retained(page, message) {
    assert.equal(message.stable_ms, 250);
    const clock = () => page.harness.dom.context.performance.now();
    const started = clock();
    const reply = await page.harness.send(page.bind(withKey(message, "stable_ms", 400)));
    assert.equal(reply.ok, true);
    assert.ok(clock() - started >= 400, "stable_ms is the time every field must hold its value");
    page.name.value = "Other";
    const lost = await page.harness.send(page.bind(withKey(message, "limit_ms", 100)));
    assert.equal(lost.error?.code, "not_retained", "limit_ms bounds the wait for the values to hold");
  }
};

test("every world kind has a golden message that content.js reads and accepts", async () => {
  const directory = path.join(GOLDEN, "world");
  const files = fs.readdirSync(directory).filter((name) => name.endsWith(".json")).sort();
  assert.deepEqual(files.map((name) => name.replace(/\.json$/, "")), [...vocabulary.world_message_kinds].sort());
  const source = bundle.read("content.js");
  const attachment = { media_type: "image/gif", data: "aGVsbG8=", size: 5 };
  for (const file of files) {
    const message = JSON.parse(fs.readFileSync(path.join(directory, file), "utf8"));
    assert.equal(`${message.kind}.json`, file);
    for (const key of Object.keys(message)) {
      if (key !== "kind") assert.ok(source.includes(`message.${key}`), `${file}: content.js reads ${key}`);
    }
    const page = await goldenPage();
    if (FOCUSED_KINDS.has(message.kind)) page.name.focus();
    const options = { owner: page.owner, attachment };
    const reply = await page.harness.send(page.bind(message), options);
    page.harness.dom.advance(10);
    assert.equal(reply.ok, true, `${file} was answered ${reply.error?.code}: ${reply.error?.message}`);
    const probe = GOLDEN_PROBES[message.kind];
    if (probe) await probe(await goldenPage(), message, options);
  }
});

test("the retired kinds answer unknown_kind", async () => {
  const harness = contentHarness();
  for (const kind of ["fill", "clear", "prepare_text_fill", "verify_text_fill_focus", "verify_fill_values", "frame_boxes", "point_context", "document_route", "present"]) {
    assert.equal(await harness.codeOf({ kind }), "unknown_kind", kind);
  }
});

test("present renders only target-anchored signals in a frame other than the top", async () => {
  const harness = contentHarness({ top: false });
  const local = { preferences: { effects: true, captions: false } };
  assert.equal(harness.runtime.present({ signal: "start", activity: "script", invocation: "i-1" }, local), false);
  textInput(harness);
  const locator = await harness.locatorOf("Name");
  assert.equal(harness.runtime.present({ signal: "target", activity: "click", locator, invocation: "i-2" }, local), true);
  assert.equal(harness.runtime.present({ signal: "target", activity: "click", locator: "locator_ffffffff_1", invocation: "i-2" }, local), false);
});
