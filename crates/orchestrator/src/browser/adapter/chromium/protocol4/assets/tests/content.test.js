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
  assert.deepEqual(inspected.value.tree.children[0].children, []);
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

/** A control of the given native kind, labelled with `label`, appended to `parent`. */
function nativeControl(harness, parent, label, tag, attributes = {}) {
  const control = tag === "select" ? harness.select([{ value: "a", text: "A" }]) : harness.element(tag);
  control.setAttribute("aria-label", label);
  for (const [name, value] of Object.entries(attributes)) control.setAttribute(name, value);
  parent.append(control);
  return control;
}

test("native controls get their HTML-AAM roles", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  nativeControl(harness, body, "Volume", "input", { type: "range" });
  nativeControl(harness, body, "Quantity", "input", { type: "number" });
  nativeControl(harness, body, "Query", "input", { type: "search" });
  nativeControl(harness, body, "Email", "input", { type: "email" });
  nativeControl(harness, body, "Notes", "textarea");
  nativeControl(harness, body, "Country", "select");
  const multiple = nativeControl(harness, body, "Tags", "select");
  multiple.multiple = true;
  nativeControl(harness, body, "Rows", "select", { size: "4" });
  nativeControl(harness, body, "Single row", "select", { size: "1" });
  harness.setBody(body);

  const roles = Object.fromEntries((await harness.targets()).map((target) => [target.name, target.role]));

  for (const [name, role] of Object.entries({
    Volume: "slider", Quantity: "spinbutton", Query: "searchbox", Email: "textbox", Notes: "textbox",
    Country: "combobox", Tags: "listbox", Rows: "listbox", "Single row": "combobox"
  })) {
    assert.equal(roles[name], role, name);
  }
});

// The semantic roles a selector may name, read from the role enum of the frozen tool catalog. The
// catalog carries one copy per tool that takes a selector; the copies must be one list.
function catalogSemanticRoles() {
  const catalog = JSON.parse(fs.readFileSync(path.join(REPO, "docs", "1.0", "catalog.json"), "utf8"));
  const copies = [];
  const collect = (node) => {
    if (Array.isArray(node)) {
      node.forEach(collect);
    } else if (node && typeof node === "object") {
      for (const [key, value] of Object.entries(node)) {
        if (key === "role" && value && Array.isArray(value.enum)) {
          copies.push(value.enum);
        }
        collect(value);
      }
    }
  };
  collect(catalog);
  assert.ok(copies.length > 0, "the catalog names the semantic roles");
  for (const copy of copies) {
    assert.deepEqual(copy, copies[0]);
  }
  return copies[0];
}

test("every semantic role is producible", async () => {
  const SEMANTIC_ROLES = catalogSemanticRoles();

  const harness = contentHarness();
  const body = harness.element("body");
  const builders = {
    button: () => nativeControl(harness, body, "button", "button"),
    link: () => nativeControl(harness, body, "link", "a", { href: "/x" }),
    checkbox: () => nativeControl(harness, body, "checkbox", "input", { type: "checkbox" }),
    radio: () => nativeControl(harness, body, "radio", "input", { type: "radio" }),
    textbox: () => nativeControl(harness, body, "textbox", "input", { type: "text" }),
    searchbox: () => nativeControl(harness, body, "searchbox", "input", { type: "search" }),
    combobox: () => nativeControl(harness, body, "combobox", "select"),
    listbox: () => nativeControl(harness, body, "listbox", "select", { multiple: "" }),
    slider: () => nativeControl(harness, body, "slider", "input", { type: "range" }),
    spinbutton: () => nativeControl(harness, body, "spinbutton", "input", { type: "number" }),
    tab: () => nativeControl(harness, body, "tab", "div", { role: "tab" }),
    menuitem: () => nativeControl(harness, body, "menuitem", "div", { role: "menuitem" }),
    option: () => nativeControl(harness, body, "option", "div", { role: "option" })
  };
  for (const role of SEMANTIC_ROLES) {
    assert.equal(typeof builders[role], "function", `${role} has a builder`);
    builders[role]();
  }
  harness.setBody(body);

  for (const role of SEMANTIC_ROLES) {
    const found = await harness.send({ kind: "query_semantic", name: role, role, exact: true });
    assert.equal(found.value.targets.length, 1, role);
    assert.equal(found.value.targets[0].role, role);
  }
});

test("a hidden input is not a control", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  nativeControl(harness, body, "Csrf", "input", { type: "hidden" });
  nativeControl(harness, body, "Visible", "input", { type: "text" });
  harness.setBody(body);

  const names = (await harness.targets()).map((target) => target.name);
  const all = await harness.send({ kind: "inspect", inspect_kind: "all", max_items: 40, visual_settle: false });
  const found = await harness.send({ kind: "find", text: "Csrf", find_kind: "any", max_results: 5 });
  const semantic = await harness.send({ kind: "query_semantic", name: "Csrf", exact: true });

  assert.ok(names.includes("Visible"));
  assert.ok(!names.includes("Csrf"));
  assert.ok(!all.value.targets.some((target) => target.name === "Csrf"));
  assert.equal(found.value.targets.length, 0);
  assert.equal(semantic.value.targets.length, 0);
});

test("a control inside a disabled fieldset is disabled and not actionable", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const fieldset = harness.element("fieldset");
  fieldset.setAttribute("disabled", "");
  nativeControl(harness, fieldset, "Locked", "input", { type: "text" });
  nativeControl(harness, body, "Open", "input", { type: "text" });
  body.append(fieldset);
  harness.setBody(body);

  const byName = Object.fromEntries((await harness.targets()).map((target) => [target.name, target]));
  assert.ok(byName.Locked.state.includes("disabled"));
  assert.ok(!byName.Open.state.includes("disabled"));
  assert.equal(await harness.codeOf({ kind: "hover", locator: byName.Locked.locator }), "disabled");
  assert.equal(await harness.codeOf({ kind: "hover", locator: byName.Open.locator }), null);
});

test("credential names are recognized across spellings and innocent names keep their class", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const names = {
    credential: [
      "apiKey", "api_key", "api-key", "cardNumber", "card_number", "userPin", "pin", "ssn", "iban",
      "cc-exp", "cc-name", "currentPassword", "oneTimeCode", "authToken", "api_token"
    ],
    innocent: [
      "tokenizer", "shipping", "spinner", "pinned", "username", "keyword", "apikeyboard",
      "cardholder", "business", "ibanez"
    ]
  };
  for (const name of [...names.credential, ...names.innocent]) {
    nativeControl(harness, body, name, "input", { type: "text", name });
  }
  harness.setBody(body);

  const classes = Object.fromEntries((await harness.targets()).map((target) => [target.name, target.credential_class]));

  for (const name of names.credential) assert.equal(classes[name], true, name);
  for (const name of names.innocent) assert.equal(classes[name], false, name);
});

test("a dropdown is found by one of its option texts, unless it is a credential", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const role = harness.select([
    { value: "", text: "Choose a role" },
    { value: "admin", text: "Administrator" },
    { value: "viewer", text: "Read   only viewer" }
  ]);
  role.setAttribute("aria-label", "Role");
  const secret = harness.select([{ value: "a", text: "Zebra clearance" }]);
  secret.setAttribute("aria-label", "Level");
  secret.setAttribute("name", "userPin");
  body.append(role, secret);
  harness.setBody(body);

  for (const text of ["Administrator", "administrator", "read only"]) {
    const found = await harness.send({ kind: "find", text, find_kind: "control", max_results: 5 });
    assert.deepEqual(found.value.targets.map((target) => target.name), ["Role"], text);
    assert.deepEqual(found.value.targets[0].options, ["Administrator", "Read only viewer"]);
  }
  const hidden = await harness.send({ kind: "find", text: "Zebra", find_kind: "any", max_results: 5 });
  assert.equal(hidden.value.targets.length, 0);
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
  const [title, control] = section.children;
  assert.equal(section.children.length, 2);
  assert.deepEqual([title.kind, title.label], ["heading", "Invoices"]);
  assert.deepEqual([control.kind, control.label], ["control", "Pay now"]);
  assert.doesNotMatch(JSON.stringify(inspected.value.tree), /long paragraph/);
});

test("the document tree leaves out unlabeled containers that hold nothing", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const wrapper = harness.element("div");
  const textOnly = harness.element("div");
  textOnly.append(harness.text("Only text here."));
  const named = harness.element("section");
  named.setAttribute("aria-label", "Billing");
  const button = harness.element("button");
  button.append(harness.text("Pay now"));
  wrapper.append(textOnly, named, button);
  body.append(wrapper);
  harness.setBody(body);

  const inspected = await harness.send({ kind: "inspect_tree", max_depth: 5, max_nodes: 400 });

  const [kept] = inspected.value.tree.children;
  assert.deepEqual(kept.children.map((node) => node.label), ["Billing", "Pay now"]);
  assert.equal(inspected.value.nodes, 4);
  assert.equal(inspected.value.truncated, false);
});

test("the document tree keeps a container cut by depth and an empty root", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const outer = harness.element("div");
  const inner = harness.element("div");
  const button = harness.element("button");
  button.append(harness.text("Hidden by depth"));
  inner.append(button);
  outer.append(inner);
  body.append(outer);
  harness.setBody(body);

  const cut = await harness.send({ kind: "inspect_tree", max_depth: 2, max_nodes: 400 });
  assert.deepEqual(cut.value.tree.children.map((node) => node.kind), ["container"]);
  assert.equal(cut.value.nodes, 2);

  const empty = harness.element("body");
  harness.setBody(empty);
  const bare = await harness.send({ kind: "inspect_tree", max_depth: 5, max_nodes: 400 });
  assert.deepEqual(bare.value.tree.children, []);
  assert.equal(bare.value.nodes, 1);
});

test("the document tree stops after the visit cap and says it was cut", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  for (let index = 0; index < 4100; index += 1) body.append(harness.element("div"));
  harness.setBody(body);

  const inspected = await harness.send({ kind: "inspect_tree", max_depth: 5, max_nodes: 400 });

  assert.equal(inspected.value.truncated, true);
  assert.deepEqual(inspected.value.tree.children, []);
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

  const refused = await harness.send({ kind: "focus_text_field", field: { locator, value: "true" } });

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
  const prepared = await harness.send({ kind: "prepare_fill", fields, visual_settle: true, settle_ms: 1000 });
  const elapsed = harness.dom.context.performance.now() - started;

  assert.deepEqual(prepared.value, { prepared: true, field_kinds: ["browser_text", "browser_text", "local"] });
  assert.ok(elapsed >= 750 && elapsed < 1000, `waited for the form to settle (${elapsed} ms)`);
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

async function preparationTime(message) {
  const harness = contentHarness();
  textInput(harness);
  const locator = await harness.locatorOf("Name");
  const started = harness.dom.context.performance.now();
  const prepared = await harness.send({ kind: "prepare_fill", fields: [{ locator, value: "x" }], ...message });
  return { prepared, elapsed: harness.dom.context.performance.now() - started };
}

test("prepare_fill with the budget spent and the intent kept runs one 50 ms recheck", async () => {
  const { prepared, elapsed } = await preparationTime({ visual_settle: true, settle_ms: 0 });
  assert.equal(prepared.ok, true);
  assert.ok(elapsed >= 50 && elapsed < 200, `rechecked once (${elapsed} ms)`);
});

test("prepare_fill with an explicit false skips the stability wait whatever the budget", async () => {
  const { prepared, elapsed } = await preparationTime({ visual_settle: false, settle_ms: 800 });
  assert.equal(prepared.ok, true);
  assert.equal(elapsed, 0);
});

test("prepare_fill waits at most the budget it was given", async () => {
  const { prepared, elapsed } = await preparationTime({ visual_settle: true, settle_ms: 400 });
  assert.equal(prepared.ok, true);
  assert.ok(elapsed >= 400 && elapsed < 600, `stopped at the budget (${elapsed} ms)`);
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

  assert.deepEqual(answered, { ok: true, value: { retained: true, fields: [] } });
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
  harness.input.form = harness.element("form");
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

// ---- plain text: names and text conditions never read field values -----------------------------

/**
 * A form named from its content. The legend holds a typed input and a checked checkbox, and the
 * form holds a textarea, a password field and a select with a chosen option, so every control
 * sits inside content that a name or a text search reads.
 */
function typedForm(harness, { typed = "Ada" } = {}) {
  const body = harness.element("body");
  const form = harness.element("form");
  const legend = harness.element("legend");
  const field = harness.element("input");
  field.type = "text";
  field.value = typed;
  field.setAttribute("aria-label", "Name");
  const agreed = harness.element("input");
  agreed.type = "checkbox";
  agreed.checked = true;
  legend.append(harness.text("Contact details"), field, agreed);
  const area = harness.element("textarea");
  area.value = "private note";
  const secret = harness.element("input");
  secret.type = "password";
  secret.setAttribute("type", "password");
  secret.value = "hunter2";
  const choice = harness.element("select");
  choice.selectedOptions = [{ text: "Gold tier" }];
  form.append(legend, area, secret, choice);
  body.append(form);
  harness.setBody(body);
  return form;
}

async function textCondition(harness, condition, value) {
  const observed = await harness.send({ kind: "observe", condition, value, timeout_ms: 0 });
  return observed.value.satisfied;
}

test("a form named from its content never carries a typed value in its name", async () => {
  const harness = contentHarness();
  typedForm(harness);

  const structure = await harness.send({ kind: "inspect", inspect_kind: "structure", max_items: 20, visual_settle: false });
  const names = structure.value.targets.map((target) => target.name);

  assert.deepEqual(names, ["Contact details"]);
});

test("find does not match a typed value through its input or its form", async () => {
  const harness = contentHarness();
  typedForm(harness);

  for (const text of ["Ada", "private note", "hunter2", "[hidden]", "Gold tier", "checked", "unchecked"]) {
    const found = await harness.send({ kind: "find", text, find_kind: "any", max_results: 5 });
    assert.deepEqual(found.value.targets, [], `find ${text}`);
  }
  const named = await harness.send({ kind: "find", text: "Name", find_kind: "control", max_results: 5 });
  assert.equal(named.value.targets.length, 1);
});

test("a typed value alone never satisfies a text condition", async () => {
  const harness = contentHarness();
  typedForm(harness);

  assert.equal(await textCondition(harness, "text_present", "Ada"), false);
  assert.equal(await textCondition(harness, "text_absent", "Ada"), true);
  assert.equal(await textCondition(harness, "text_present", "[hidden]"), false);
  assert.equal(await textCondition(harness, "text_present", "unchecked"), false);
  assert.equal(await textCondition(harness, "text_present", "checked"), false);
  assert.equal(await textCondition(harness, "text_present", "Gold tier"), false);
  assert.equal(await textCondition(harness, "text_present", "Contact details"), true);
});

test("a wrapper whose only text is editable content has no name and is not found by it", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const wrapper = harness.element("section");
  const editor = harness.element("div");
  editor.isContentEditable = true;
  editor.append(harness.text("Ada"));
  editor.innerText = "Ada";
  wrapper.innerText = "Ada";
  wrapper.textContent = "Ada";
  wrapper.append(editor);
  body.append(wrapper);
  harness.setBody(body);

  const structure = await harness.send({ kind: "inspect", inspect_kind: "structure", max_items: 20, visual_settle: false });
  assert.deepEqual(structure.value.targets.map((target) => target.name), [""]);
  const found = await harness.send({ kind: "find", text: "Ada", find_kind: "any", max_results: 5 });
  assert.deepEqual(found.value.targets, []);
});

test("text conditions skip editable content but still read page text", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const editor = harness.element("div");
  editor.isContentEditable = true;
  editor.append(harness.text("draft body"));
  body.append(harness.text("Saved"), editor);
  harness.setBody(body);

  assert.equal(await textCondition(harness, "text_present", "draft body"), false);
  assert.equal(await textCondition(harness, "text_present", "Saved"), true);
});

test("the agent's text value is normalized to page text and matching stays exact", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const first = harness.element("p");
  first.append(harness.text("Order placed"));
  const second = harness.element("p");
  second.append(harness.text("Thank you"));
  const line = harness.element("div");
  line.append(harness.text("Total paid"), harness.element("br"), harness.text("Receipt sent"));
  body.append(first, second, line);
  harness.setBody(body);

  assert.equal(await textCondition(harness, "text_present", "Total paid\n  Receipt sent"), true);
  assert.equal(await textCondition(harness, "text_present", "  Total paid \r\n Receipt sent  "), true);
  assert.equal(await textCondition(harness, "text_present", "Order   placed"), true);
  assert.equal(await textCondition(harness, "text_present", "Order placed Thank you"), false);
  assert.equal(await textCondition(harness, "text_present", "order placed"), false);
});

test("browser_read keeps field values while plain text leaves them out", async () => {
  const harness = contentHarness();
  typedForm(harness);

  const read = await harness.send({ kind: "read_text", mode: "visible", max_chars: 500 });

  assert.match(read.value.text, /Ada/);
  assert.match(read.value.text, /private note/);
  assert.match(read.value.text, /Gold tier/);
  assert.match(read.value.text, /\bchecked\b/);
  assert.match(read.value.text, /\[hidden\]/);
  assert.doesNotMatch(read.value.text, /hunter2/);
});

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

// ---- quiet sampled reads -----------------------------------------------------------------------

/** Milliseconds of fake time a call took, driven the way `send` does. */
async function timed(harness, message) {
  const started = harness.dom.context.performance.now();
  const reply = await harness.send(message);
  return { reply, elapsed: harness.dom.context.performance.now() - started };
}

test("a sampled read on an idle page answers once it has been quiet, not at the ceiling", async () => {
  const harness = contentHarness();
  harness.newInput("Name");
  const { reply, elapsed } = await timed(harness, { kind: "inspect", inspect_kind: "controls", max_items: 40 });
  assert.ok(reply.value.targets.some((target) => target.name === "Name"));
  assert.ok(elapsed >= 300 && elapsed < 400, `answered after ${elapsed} ms`);
});

test("a form that hydrates in batches is returned whole", async () => {
  const harness = contentHarness();
  for (let index = 0; index < 3; index += 1) harness.newInput(`Field ${index}`);
  harness.dom.context.setTimeout(() => {
    for (let index = 3; index < 9; index += 1) harness.newInput(`Field ${index}`);
    harness.dom.mutate();
  }, 200);
  const { reply, elapsed } = await timed(harness, { kind: "inspect", inspect_kind: "controls", max_items: 40 });
  const names = reply.value.targets.map((target) => target.name);
  for (let index = 0; index < 9; index += 1) assert.ok(names.includes(`Field ${index}`), `Field ${index} is returned`);
  assert.ok(elapsed >= 500 && elapsed < 600, `answered after ${elapsed} ms`);
});

test("a find with no match on a complete idle page answers empty once it is quiet", async () => {
  const harness = contentHarness();
  harness.setBody(harness.element("body"));
  const { reply, elapsed } = await timed(harness, { kind: "find", text: "Nothing like this", find_kind: "any", max_results: 20 });
  assert.deepEqual(reply.value.targets, []);
  assert.ok(elapsed >= 300 && elapsed < 400, `answered after ${elapsed} ms`);
});

test("an empty page that is still loading waits for a control or the ceiling", async () => {
  const waiting = contentHarness();
  waiting.setBody(waiting.element("body"));
  waiting.document.readyState = "loading";
  const ceiling = await timed(waiting, { kind: "find", text: "Late", find_kind: "any", max_results: 20, settle_ms: 1000 });
  assert.deepEqual(ceiling.reply.value.targets, []);
  assert.ok(ceiling.elapsed >= 1000, `answered after ${ceiling.elapsed} ms`);

  const arriving = contentHarness();
  arriving.setBody(arriving.element("body"));
  arriving.document.readyState = "loading";
  arriving.dom.context.setTimeout(() => {
    arriving.newInput("Late");
    arriving.dom.mutate();
  }, 400);
  const late = await timed(arriving, { kind: "find", text: "Late", find_kind: "any", max_results: 20, settle_ms: 1000 });
  assert.equal(late.reply.value.targets.length, 1);
  assert.ok(late.elapsed >= 700 && late.elapsed < 800, `answered after ${late.elapsed} ms`);
});

/** The settle sensor alone, loaded into a fake page, for options no message sets. */
function sensorIn(dom) {
  const source = bundle.read("sensor.js");
  return dom.run(`(function () {
    class AssetError extends Error { constructor(code, message) { super(message); this.code = code; } }
    const world = { onDispose() {} };
    ${source}
    return sensor;
  })()`, "sensor.js");
}

test("a satisfied sample that a later mutation empties does not end a loading page's wait", async () => {
  const dom = createFakeDom();
  dom.document.readyState = "loading";
  const sensor = sensorIn(dom);
  let satisfied = false;
  const sample = () => ({ satisfied });
  let answeredAt = null;
  const pending = sensor.settle(sample, (candidate) => candidate.satisfied, { maxWaitMs: 400 }).then((value) => {
    answeredAt = dom.context.performance.now();
    return value;
  });
  const step = async (ms) => {
    await tick();
    dom.advance(ms);
    await tick();
  };
  satisfied = true;
  dom.mutate();
  await step(20);
  satisfied = false;
  dom.mutate();
  await step(100);
  assert.equal(answeredAt, null, "the stale satisfied sample did not resolve the wait");
  await step(250);
  assert.equal(answeredAt, null, "the quiet window ended on an empty sample of a loading page");
  await step(100);
  assert.deepEqual(plain(await pending), { satisfied: false });
  assert.ok(answeredAt >= 400, `answered at ${answeredAt} ms`);
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

/** The detail a refused call carries, or null when the call succeeded. */
async function detailOf(harness, message) {
  const reply = await harness.send(message);
  assert.equal(reply.ok, false);
  assert.equal(reply.error.code, "stale_locator");
  return reply.error.detail;
}

/** The stale word a refused call carries. */
async function staleWordOf(harness, message) {
  return (await detailOf(harness, message)).stale;
}

test("a locator from another world or an unminted one is stale_locator", async () => {
  const first = contentHarness();
  const second = contentHarness();
  const foreign = (await first.targets())[0].locator;
  const own = (await second.targets())[0].locator;
  assert.notEqual(foreign.split("_")[1], own.split("_")[1], "each world draws its own epoch");

  for (const kind of ["describe", "box", "activate", "focus", "hover", "scroll"]) {
    const message = kind === "describe" ? { kind, locators: [foreign] } : { kind, locator: foreign };
    assert.deepEqual(await detailOf(second, message), { stale: "unknown" }, kind);
  }
  assert.deepEqual(await detailOf(second, { kind: "describe", locators: ["locator_1"] }), { stale: "unknown" });
  const unminted = { kind: "describe", locators: [`locator_${own.split("_")[1]}_999`] };
  assert.deepEqual(await detailOf(second, unminted), { stale: "unknown" });
  assert.equal(await second.codeOf({ kind: "describe", locators: [own] }), null);
});

test("a locator for a removed element is stale_locator", async () => {
  const harness = contentHarness();
  const locator = (await harness.targets())[0].locator;
  harness.input.isConnected = false;
  assert.equal(await staleWordOf(harness, { kind: "describe", locators: [locator] }), "removed");
  assert.equal(await staleWordOf(harness, { kind: "box", locator }), "removed");
});

test("a removed locator says removed again on a second use", async () => {
  const harness = contentHarness();
  const locator = (await harness.targets())[0].locator;
  harness.input.isConnected = false;
  for (let use = 0; use < 3; use += 1) {
    assert.equal(await staleWordOf(harness, { kind: "activate", locator }), "removed", `use ${use}`);
  }
});

test("an evicted locator says unknown", async () => {
  const harness = contentHarness();
  const first = (await harness.targets())[0].locator;
  const body = harness.element("body");
  for (let index = 0; index < 520; index += 1) {
    const item = harness.element("button");
    item.setAttribute("aria-label", `Item ${index}`);
    body.append(item);
  }
  harness.setBody(body);
  const reply = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 520, visual_settle: false });
  assert.ok(reply.value.targets.length >= 500);
  assert.deepEqual(await detailOf(harness, { kind: "describe", locators: [first] }), { stale: "unknown" });
});

test("a detached element is released at the next read", async () => {
  const harness = contentHarness();
  const locator = (await harness.targets())[0].locator;
  harness.input.isConnected = false;
  await harness.send({ kind: "find", text: "anything", find_kind: "any", max_results: 5, visual_settle: false });
  harness.input.isConnected = true;
  assert.equal(
    await staleWordOf(harness, { kind: "describe", locators: [locator] }),
    "removed",
    "the registry no longer holds the node, so reattaching it does not revive the locator"
  );
});

test("live locators never exceed 500", async () => {
  const harness = contentHarness();
  const minted = [];
  for (let batch = 0; batch < 3; batch += 1) {
    const body = harness.element("body");
    for (let index = 0; index < 300; index += 1) {
      const item = harness.element("button");
      item.setAttribute("aria-label", `Item ${batch}-${index}`);
      body.append(item);
    }
    harness.setBody(body);
    const reply = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 300, visual_settle: false });
    minted.push(...reply.value.targets.map((target) => target.locator));
  }
  let live = 0;
  for (const locator of minted) {
    const reply = await harness.send({ kind: "describe", locators: [locator] });
    if (reply.ok) live += 1;
  }
  assert.ok(live > 0 && live <= 500, `${live} live locators`);
});

test("requireActionable says removed when the node vanishes mid-action", async () => {
  const harness = contentHarness();
  harness.input.hidden = false;
  harness.input.type = "submit";
  harness.input.setAttribute("value", "Save changes");
  const locator = (await harness.targets())[0].locator;
  harness.input.scrollIntoView = () => {
    harness.input.isConnected = false;
  };
  assert.equal(await staleWordOf(harness, { kind: "activate", locator }), "removed");
});

// ---- a refused fill lists what would succeed ----------------------------------------------------

const ROLES = [
  { value: "dev", text: "Developer" },
  { value: "adm", text: "Administrator" },
  { value: "oth", text: "Other" }
];

/** The refusal of a prepare_fill over `fields`, given as [label, value] pairs. */
async function prepareRefusal(harness, fields) {
  const wire = [];
  for (const [label, value] of fields) wire.push({ locator: await harness.locatorOf(label), value });
  return harness.send({ kind: "prepare_fill", fields: wire, visual_settle: false });
}

test("a fill with two bad fields reports both at once and edits nothing", async () => {
  const harness = contentHarness();
  const role = labelledSelect(harness, "Role", ROLES);
  textInput(harness, { label: "Name", value: "kept" });
  const team = labelledSelect(harness, "Team", [{ value: "a", text: "Alpha" }, { value: "b", text: "Beta" }]);

  const refused = await prepareRefusal(harness, [["Role", "Engineer"], ["Name", "Ada"], ["Team", "Gamma"]]);

  assert.equal(refused.ok, false);
  assert.equal(refused.error.code, "option_missing");
  assert.match(refused.error.message, /^select option not found/);
  assert.deepEqual(refused.error.detail.problems, [
    { field: 0, problem: "option_missing", options: ["Developer", "Administrator", "Other"] },
    { field: 2, problem: "option_missing", options: ["Alpha", "Beta"] }
  ]);
  assert.equal(role.value, "dev");
  assert.equal(team.value, "a");
  assert.equal(harness.input.value, "kept");
  assert.deepEqual(harness.input.events, []);
  assert.deepEqual(harness.edits, []);
});

test("the refusal never echoes the value the caller gave", async () => {
  const harness = contentHarness();
  labelledSelect(harness, "Role", ROLES);

  const refused = await prepareRefusal(harness, [["Role", "PRIVATE_GIVEN_VALUE"]]);

  assert.equal(refused.error.code, "option_missing");
  assert.ok(!JSON.stringify(refused).includes("PRIVATE_GIVEN_VALUE"));
});

test("the refusal lists an optgroup as a group and marks the options a fill would refuse", async () => {
  const harness = contentHarness();
  const retired = { label: "Retired", disabled: true };
  labelledSelect(harness, "Team", [
    { value: "all", text: "Everyone" },
    { value: "eng", text: "Engineering", group: { label: "Staff" } },
    { value: "old", text: "Archive", group: retired },
    { value: "solo", text: "Solo", disabled: true }
  ]);

  const refused = await prepareRefusal(harness, [["Team", "Nobody"]]);

  assert.deepEqual(refused.error.detail.problems, [{
    field: 0,
    problem: "option_missing",
    options: [
      "Everyone",
      { group: "Staff", options: ["Engineering"] },
      { group: "Retired", options: [{ text: "Archive", disabled: true }] },
      { text: "Solo", disabled: true }
    ]
  }]);
});

test("a value that names a disabled option is refused as disabled, in a group or alone", async () => {
  const harness = contentHarness();
  const retired = { label: "Retired", disabled: true };
  const team = labelledSelect(harness, "Team", [
    { value: "all", text: "Everyone" },
    { value: "old", text: "Archive", group: retired },
    { value: "solo", text: "Solo", disabled: true }
  ]);

  for (const value of ["Archive", "old", "Solo", "solo"]) {
    const refused = await prepareRefusal(harness, [["Team", value]]);
    assert.equal(refused.error.code, "option_missing", value);
    assert.match(refused.error.message, /^select option not found/);
    assert.deepEqual(refused.error.detail.problems, [{ field: 0, problem: "option_disabled" }], value);
  }
  assert.equal(team.value, "all");
  assert.equal((await prepareRefusal(harness, [["Team", "Everyone"]])).ok, true);
});

test("a long select reports its count and the options that contain the refused value", async () => {
  const harness = contentHarness();
  const options = Array.from({ length: 30 }, (_, index) => ({ value: `v${index}`, text: `Region ${index}` }));
  options.push({ value: "x", text: "Region retired", disabled: true });
  labelledSelect(harness, "Region", options);

  const refused = await prepareRefusal(harness, [["Region", "gion 1"]]);

  const [problem] = refused.error.detail.problems;
  assert.equal(problem.options.length, 25);
  assert.equal(problem.options_total, 31);
  assert.deepEqual(problem.containing, ["Region 1", "Region 10", "Region 11", "Region 12", "Region 13", "Region 14", "Region 15", "Region 16"]);
  const none = (await prepareRefusal(harness, [["Region", "Atlantis"]])).error.detail.problems[0];
  assert.equal("containing" in none, false);
});

test("a credential-class select lists no options and echoes nothing", async () => {
  const harness = contentHarness();
  const token = labelledSelect(harness, "Token kind", ROLES);
  token.setAttribute("name", "api_token");
  const locator = await harness.locatorOf("Token kind");
  const message = { kind: "prepare_fill", fields: [{ locator, value: "PRIVATE_GIVEN_VALUE" }], visual_settle: false };

  assert.equal((await harness.send(message)).error.code, "credential_authorization_required");
  const refused = await harness.send({ ...message, allow_credentials: true });

  assert.equal(refused.error.code, "option_missing");
  assert.deepEqual(refused.error.detail.problems, [{ field: 0, problem: "option_missing" }]);
  assert.ok(!JSON.stringify(refused).includes("PRIVATE_GIVEN_VALUE"));
  assert.ok(!JSON.stringify(refused).includes("Developer"));
});

test("the detail stays under 4 KB by dropping options first", async () => {
  const harness = contentHarness();
  const options = Array.from({ length: 25 }, (_, index) => ({ value: `v${index}`, text: `${"x".repeat(55)} ${index}` }));
  const labels = ["One", "Two", "Three", "Four", "Five"];
  for (const label of labels) labelledSelect(harness, label, options);

  const refused = await prepareRefusal(harness, labels.map((label) => [label, "missing"]));

  assert.ok(JSON.stringify(refused.error.detail).length <= 4096);
  assert.deepEqual(refused.error.detail.problems.map((problem) => problem.field), [0, 1, 2, 3, 4]);
  assert.ok(refused.error.detail.problems.every((problem) => problem.problem === "option_missing" && !("options" in problem)));
  assert.ok(refused.error.detail.problems.every((problem) => problem.options_total === 25));
});

test("single-field paths refuse with the code and message alone, with no detail", async () => {
  const harness = contentHarness();
  labelledSelect(harness, "Role", ROLES);
  const locator = await harness.locatorOf("Role");

  for (const message of [
    { kind: "fill_local", field: { locator, value: "Engineer" }, allow_credentials: false },
    { kind: "focus_text_field", field: { locator, value: "Engineer" }, allow_credentials: false }
  ]) {
    const refused = await harness.send(message);
    assert.equal(refused.error.code, "option_missing", message.kind);
    assert.match(refused.error.message, /^select option not found/);
    assert.equal("detail" in refused.error, false, message.kind);
  }
});

test("a select whose chosen option the page disabled afterwards is not retained", async () => {
  const harness = contentHarness();
  const team = labelledSelect(harness, "Team", [{ value: "a", text: "Alpha" }, { value: "b", text: "Beta" }]);
  const locator = await harness.locatorOf("Team");
  team.value = "b";
  team.options[1].disabled = true;

  const retained = await harness.send({ kind: "verify_fill_retained", fields: [{ locator, value: "Beta" }], stable_ms: 10, poll_ms: 5, limit_ms: 50 });

  assert.equal(retained.error.code, "not_retained");
});

// ---- choices match exactly, then uniquely --------------------------------------------------------

/** Fill one choice control and read back what the page reports it holds. */
async function fillChoice(harness, label, value) {
  const locator = await harness.locatorOf(label);
  const filled = await harness.send({ kind: "fill_local", field: { locator, value }, allow_credentials: false });
  const verified = await harness.send({ kind: "verify_fill_retained", fields: [{ locator, value }], stable_ms: 10, poll_ms: 5, limit_ms: 100 });
  return { filled, verified };
}

/**
 * A visible radio button labelled `label` in the radio group `group`, inside `parent` when given.
 * Checking it unchecks the others of its group, as a browser does.
 */
function groupedRadio(harness, label, group, props = {}, parent = null) {
  const radio = harness.newInput(label, { type: "radio", checked: false, ...props });
  radio.setAttribute("name", group);
  let held = radio.checked === true;
  Object.defineProperty(radio, "checked", {
    get: () => held,
    set: (value) => {
      held = value === true;
      if (!held) return;
      for (const other of harness.document.querySelectorAll("input")) {
        if (other !== radio && other.type === "radio" && other.getAttribute("name") === group) other.checked = false;
      }
    }
  });
  if (parent) {
    radio.remove();
    parent.appendChild(radio);
  }
  return radio;
}

const TEAMS = [{ value: "a", text: "Alpha" }, { value: "b", text: "Beta" }, { value: "c", text: "Gamma" }];

test("a value that is exactly an option's text, value or label picks it without a match note", async () => {
  const harness = contentHarness();
  const team = labelledSelect(harness, "Team", [{ value: "a", text: "Alpha" }, { value: "b", text: "Beta", label: "The second" }]);
  for (const [value, held] of [["Beta", "b"], ["a", "a"], ["The second", "b"]]) {
    const { filled, verified } = await fillChoice(harness, "Team", value);
    assert.equal(filled.ok, true, value);
    assert.equal(team.value, held, value);
    assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: held === "a" ? "Alpha" : "Beta" }] }, value);
  }
});

test("a value that differs only in case, spacing or Unicode form picks the one option and says it matched", async () => {
  const harness = contentHarness();
  const role = labelledSelect(harness, "Role", [{ value: "dev", text: "Developer" }, { value: "adm", text: "Administrator" }, { value: "caf", text: "Caf\u00e9 owner" }]);
  for (const [value, held, text] of [["administrator", "adm", "Administrator"], ["  ADMINISTRATOR\t", "adm", "Administrator"], ["cafe\u0301   OWNER", "caf", "Caf\u00e9 owner"]]) {
    role.value = "dev";
    const { filled, verified } = await fillChoice(harness, "Role", value);
    assert.equal(filled.ok, true, value);
    assert.equal(role.value, held, value);
    assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: text, matched: "normalized" }] }, value);
  }
});

test("a value that names no option exactly or after normalizing is never matched by part of it", async () => {
  const harness = contentHarness();
  const role = labelledSelect(harness, "Role", ROLES);
  for (const value of ["Admin", "istrator", "Administrato", "Administratr"]) {
    const refused = await prepareRefusal(harness, [["Role", value]]);
    assert.deepEqual(refused.error.detail.problems.map((problem) => problem.problem), ["option_missing"], value);
  }
  assert.equal(role.value, "dev");
});

test("two options that exactly name a value are refused with their values", async () => {
  const harness = contentHarness();
  const team = labelledSelect(harness, "Team", [{ value: "a", text: "Other" }, { value: "b", text: "Other" }, { value: "c", text: "Third" }]);

  const refused = await prepareRefusal(harness, [["Team", "Other"]]);

  assert.equal(refused.error.code, "option_missing");
  assert.deepEqual(refused.error.detail.problems, [{
    field: 0,
    problem: "option_ambiguous",
    options: [{ text: "Other", value: "a" }, { text: "Other", value: "b" }]
  }]);
  assert.equal(team.value, "a");
  assert.equal((await prepareRefusal(harness, [["Team", "b"]])).ok, true);
});

test("a value is ambiguous when two options match only after normalizing, and exact wins first", async () => {
  const harness = contentHarness();
  labelledSelect(harness, "Team", [{ value: "a", text: "Other" }, { value: "b", text: "OTHER" }]);

  const refused = await prepareRefusal(harness, [["Team", "other"]]);

  assert.equal(refused.error.detail.problems[0].problem, "option_ambiguous");
  assert.equal((await prepareRefusal(harness, [["Team", "OTHER"]])).ok, true);
});

test("a normalized match on a disabled option is refused as disabled and changes nothing", async () => {
  const harness = contentHarness();
  const team = labelledSelect(harness, "Team", [{ value: "a", text: "Alpha" }, { value: "b", text: "Beta", disabled: true }]);

  const refused = await prepareRefusal(harness, [["Team", "beta"]]);

  assert.deepEqual(refused.error.detail.problems, [{ field: 0, problem: "option_disabled" }]);
  assert.equal(team.value, "a");
});

test("an empty string picks the empty placeholder and no other text does", async () => {
  const harness = contentHarness();
  const team = labelledSelect(harness, "Team", [{ value: "", text: "Choose a team" }, ...TEAMS]);
  team.value = "b";

  const { filled, verified } = await fillChoice(harness, "Team", "");

  assert.equal(filled.ok, true);
  assert.equal(team.value, "");
  assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: "" }] });
  for (const value of ["Choose a team", "choose a team"]) {
    const refused = await prepareRefusal(harness, [["Team", value]]);
    assert.equal(refused.error.detail.problems[0].problem, "option_missing", value);
  }
  const plain = labelledSelect(harness, "Plain", TEAMS);
  assert.equal((await prepareRefusal(harness, [["Plain", ""]])).error.detail.problems[0].problem, "option_missing");
  assert.equal(plain.value, "a");
});

test("a multiple select takes a list and holds exactly that set", async () => {
  const harness = contentHarness();
  const tags = labelledSelect(harness, "Tags", [{ value: "a", text: "Alpha", selected: true }, { value: "b", text: "Beta" }, { value: "c", text: "Gamma" }], { multiple: true });

  const { filled, verified } = await fillChoice(harness, "Tags", ["Gamma", "beta"]);

  assert.equal(filled.ok, true);
  assert.deepEqual(tags.options.map((option) => option.selected === true), [false, true, true]);
  assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: "Beta, Gamma", matched: "normalized" }] });
  const cleared = await fillChoice(harness, "Tags", []);
  assert.equal(cleared.filled.ok, true);
  assert.deepEqual(tags.options.map((option) => option.selected === true), [false, false, false]);
});

test("a list with one bad entry is refused with that entry's position and nothing changes", async () => {
  const harness = contentHarness();
  const tags = labelledSelect(harness, "Tags", [{ value: "a", text: "Alpha", selected: true }, { value: "b", text: "Beta" }, { value: "c", text: "Gamma", disabled: true }], { multiple: true });

  const missing = await prepareRefusal(harness, [["Tags", ["Beta", "Delta"]]]);
  assert.deepEqual(missing.error.detail.problems.map(({ problem, value_index: index }) => [problem, index]), [["option_missing", 1]]);
  const disabled = await prepareRefusal(harness, [["Tags", ["Beta", "Gamma"]]]);
  assert.deepEqual(disabled.error.detail.problems, [{ field: 0, problem: "option_disabled", value_index: 1 }]);
  assert.deepEqual(tags.options.map((option) => option.selected === true), [true, false, false]);
});

test("a single string on a multiple select selects only that option", async () => {
  const harness = contentHarness();
  const tags = labelledSelect(harness, "Tags", [{ value: "a", text: "Alpha", selected: true }, { value: "b", text: "Beta" }], { multiple: true });

  const { filled, verified } = await fillChoice(harness, "Tags", "Beta");

  assert.equal(filled.ok, true);
  assert.deepEqual(tags.options.map((option) => option.selected === true), [false, true]);
  assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: "Beta" }] });
});

test("a list given to a control that takes one value is refused as multiple values", async () => {
  const harness = contentHarness();
  const team = labelledSelect(harness, "Team", TEAMS);
  const box = harness.newInput("Agree", { type: "checkbox", checked: false });
  textInput(harness, { label: "Name", value: "kept" });

  for (const label of ["Team", "Agree", "Name"]) {
    const refused = await prepareRefusal(harness, [[label, ["x", "y"]]]);
    assert.equal(refused.error.code, "not_fillable", label);
    assert.match(refused.error.message, /one value, not a list/, label);
    assert.deepEqual(refused.error.detail.problems, [{ field: 0, problem: "multiple_values" }], label);
  }
  assert.equal(team.value, "a");
  assert.equal(box.checked, false);
  assert.equal(harness.input.value, "kept");
});

test("a checkbox is set from a boolean or a closed word and never toggled", async () => {
  const harness = contentHarness();
  const box = harness.newInput("Agree", { type: "checkbox", checked: false });
  const checked = ["true", "TRUE", "Yes", "on", "1", "Checked", true];
  const unchecked = ["false", "No", "OFF", "0", "unchecked", false];
  for (const value of checked) {
    box.checked = true;
    const { filled, verified } = await fillChoice(harness, "Agree", value);
    assert.equal(filled.ok, true, String(value));
    assert.equal(box.checked, true, String(value));
    assert.deepEqual(verified.value.fields, [{ field: 0, now: "checked" }], String(value));
  }
  for (const value of unchecked) {
    box.checked = false;
    const { filled } = await fillChoice(harness, "Agree", value);
    assert.equal(filled.ok, true, String(value));
    assert.equal(box.checked, false, String(value));
  }
});

test("a checkbox given any other word is refused as not boolean and keeps its state", async () => {
  const harness = contentHarness();
  const box = harness.newInput("Agree", { type: "checkbox", checked: true });
  for (const value of ["maybe", "", "y", "enabled", " yes"]) {
    const refused = await prepareRefusal(harness, [["Agree", value]]);
    assert.equal(refused.error.code, "not_fillable", value);
    assert.deepEqual(refused.error.detail.problems, [{ field: 0, problem: "not_boolean" }], value);
    const local = await harness.send({ kind: "fill_local", field: { locator: await harness.locatorOf("Agree"), value }, allow_credentials: false });
    assert.equal(local.error.code, "not_fillable", value);
  }
  assert.equal(box.checked, true);
  assert.deepEqual(box.events, []);
});

test("a radio is chosen by the label of a sibling in its group", async () => {
  const harness = contentHarness();
  const free = groupedRadio(harness, "Free", "plan", { checked: true });
  const pro = groupedRadio(harness, "Pro", "plan");
  const other = groupedRadio(harness, "Pro", "other");

  const { filled, verified } = await fillChoice(harness, "Free", "pro");

  assert.equal(filled.ok, true);
  assert.equal(pro.checked, true);
  assert.equal(other.checked, false);
  assert.deepEqual(pro.events, ["input", "change"]);
  assert.deepEqual(free.events, []);
  assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: "unchecked", matched: "normalized", picked: "Pro" }] });
});

test("a radio picked by a sibling's label reports the group and not the addressed radio's state", async () => {
  const harness = contentHarness();
  const fieldset = harness.element("fieldset");
  const legend = harness.element("legend");
  legend.append(harness.text("Plan"));
  fieldset.append(legend);
  harness.document.body.append(fieldset);
  groupedRadio(harness, "Free", "plan", { checked: true }, fieldset);
  groupedRadio(harness, "Pro", "plan", {}, fieldset);

  const { verified } = await fillChoice(harness, "Free", "Pro");

  assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: "unchecked", picked: "Pro", group: "Plan" }] });
});

test("a sibling radio that cannot be acted on refuses the fill and nothing changes", async () => {
  const harness = contentHarness();
  const free = groupedRadio(harness, "Free", "plan", { checked: true });
  const pro = groupedRadio(harness, "Pro", "plan");
  pro.hidden = true;

  const refused = await prepareRefusal(harness, [["Free", "Pro"]]);
  const local = await harness.send({ kind: "fill_local", field: { locator: await harness.locatorOf("Free"), value: "Pro" }, allow_credentials: false });

  assert.equal(refused.error.code, "not_visible");
  assert.equal(local.error.code, "not_visible");
  assert.equal(free.checked, true);
  assert.equal(pro.checked, false);
  assert.deepEqual(pro.events, []);
  assert.deepEqual(free.events, []);
});

test("a radio given a boolean word that names no label applies it to the targeted radio", async () => {
  const harness = contentHarness();
  const free = groupedRadio(harness, "Free", "plan");
  const pro = groupedRadio(harness, "Pro", "plan", { checked: true });
  for (const value of ["yes", "true", true, "on"]) {
    free.checked = false;
    const { filled, verified } = await fillChoice(harness, "Free", value);
    assert.equal(filled.ok, true, String(value));
    assert.equal(free.checked, true, String(value));
    assert.deepEqual(verified.value.fields, [{ field: 0, now: "checked" }], String(value));
  }
  assert.equal(pro.checked, false);
});

test("a JSON boolean applies to the targeted radio even when a sibling is labelled with the word", async () => {
  const harness = contentHarness();
  const yes = groupedRadio(harness, "True", "flag");
  const no = groupedRadio(harness, "False", "flag");

  const { filled } = await fillChoice(harness, "False", true);

  assert.equal(filled.ok, true);
  assert.equal(no.checked, true);
  assert.equal(yes.checked, false);
  const named = await fillChoice(harness, "False", "true");
  assert.equal(named.filled.ok, true);
  assert.equal(yes.checked, true);
});

test("a radio given an unknown string is refused with its group and the labels that would work", async () => {
  const harness = contentHarness();
  const fieldset = harness.element("fieldset");
  const legend = harness.element("legend");
  legend.append(harness.text("Plan"));
  fieldset.append(legend);
  harness.document.body.append(fieldset);
  const free = groupedRadio(harness, "Free", "plan", {}, fieldset);
  groupedRadio(harness, "Pro", "plan", {}, fieldset);
  groupedRadio(harness, "Locked", "plan", { disabled: true }, fieldset);

  const refused = await prepareRefusal(harness, [["Free", "Enterprise"]]);

  assert.equal(refused.error.code, "option_missing");
  const [problem] = refused.error.detail.problems;
  assert.equal(problem.problem, "option_missing");
  assert.deepEqual(problem.options, ["Free", "Pro", { text: "Locked", disabled: true }]);
  assert.equal(problem.group, "Plan");
  assert.equal(free.checked, false);
  const locked = await prepareRefusal(harness, [["Free", "locked"]]);
  assert.deepEqual(locked.error.detail.problems.map((entry) => entry.problem), ["option_disabled"]);
});

test("a radio label shared by two radios of the group is ambiguous", async () => {
  const harness = contentHarness();
  groupedRadio(harness, "Same", "plan");
  groupedRadio(harness, "Same", "plan");

  const refused = await prepareRefusal(harness, [["Same", "same"]]);

  assert.deepEqual(refused.error.detail.problems.map((entry) => entry.problem), ["option_ambiguous"]);
});

test("the check before a fill, the fill and the retained check agree on every choice", async () => {
  const harness = contentHarness();
  labelledSelect(harness, "Role", ROLES);
  harness.newInput("Agree", { type: "checkbox", checked: false });
  const fields = [["Role", "ADMINISTRATOR"], ["Agree", "yes"]];
  const wire = [];
  for (const [label, value] of fields) wire.push({ locator: await harness.locatorOf(label), value });

  assert.equal((await harness.send({ kind: "prepare_fill", fields: wire, visual_settle: false })).ok, true);
  for (const field of wire) await harness.send({ kind: "fill_local", field, allow_credentials: false });
  const verified = await harness.send({ kind: "verify_fill_retained", fields: wire, stable_ms: 10, poll_ms: 5, limit_ms: 100 });

  assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: "Administrator", matched: "normalized" }, { field: 1, now: "checked" }] });
});

test("a refusal that mixes option problems with others is not fillable and says so in both code and message", async () => {
  const harness = contentHarness();
  labelledSelect(harness, "Role", ROLES);
  harness.newInput("Agree", { type: "checkbox", checked: false });

  const mixed = await prepareRefusal(harness, [["Role", "Engineer"], ["Agree", "maybe"]]);
  assert.equal(mixed.error.code, "not_fillable");
  assert.equal(mixed.error.message, "target is not fillable (2 fields refused)");
  assert.deepEqual(mixed.error.detail.problems.map((problem) => problem.problem), ["option_missing", "not_boolean"]);

  const lone = await prepareRefusal(harness, [["Agree", "maybe"]]);
  assert.equal(lone.error.code, "not_fillable");
  assert.equal(lone.error.message, "target is not fillable (takes true or false)");

  const options = await prepareRefusal(harness, [["Role", "Engineer"]]);
  assert.equal(options.error.code, "option_missing");
  assert.equal(options.error.message, "select option not found");
});

// ---- typed inputs and wrong tools are refused before any edit -----------------------------------

/** A visible typed input with the bounds a page declares. */
function typedInput(harness, label, type, bounds = {}, props = {}) {
  const element = harness.newInput(label, { type, ...props });
  for (const [name, value] of Object.entries(bounds)) element.setAttribute(name, value);
  return element;
}

/** The problems of a prepare_fill over one field, or an empty list when the fill is accepted. */
async function problemsOf(harness, label, value) {
  const reply = await prepareRefusal(harness, [[label, value]]);
  return reply.ok ? [] : reply.error.detail.problems;
}

test("a date the browser would sanitize away is refused with its shape, and the value stays out", async () => {
  const harness = contentHarness();
  const start = typedInput(harness, "Start", "date");

  const refused = await prepareRefusal(harness, [["Start", "10/14/2026"]]);

  assert.equal(refused.error.code, "not_fillable");
  assert.deepEqual(refused.error.detail.problems, [{ field: 0, problem: "format", type: "date", expected: "YYYY-MM-DD" }]);
  assert.ok(!JSON.stringify(refused).includes("10/14/2026"));
  assert.equal(start.value, "");
  assert.deepEqual(start.events, []);
  assert.deepEqual(await problemsOf(harness, "Start", "2026-10-14"), []);
  assert.deepEqual(await problemsOf(harness, "Start", "2026-02-30"), [{ field: 0, problem: "format", type: "date", expected: "YYYY-MM-DD" }]);
  assert.deepEqual(await problemsOf(harness, "Start", ""), [], "clearing a date is a valid value");
});

test("a date outside its declared range is refused with the limits the page declares", async () => {
  const harness = contentHarness();
  typedInput(harness, "Start", "date", { min: "2026-10-01", max: "2026-12-31" });
  const limited = { field: 0, problem: "format", type: "date", expected: "YYYY-MM-DD", limits: { min: "2026-10-01", max: "2026-12-31" } };

  assert.deepEqual(await problemsOf(harness, "Start", "2027-01-15"), [limited]);
  assert.deepEqual(await problemsOf(harness, "Start", "2026-09-30"), [limited]);
  assert.deepEqual(await problemsOf(harness, "Start", "10/14/2026"), [limited]);
  assert.deepEqual(await problemsOf(harness, "Start", "2026-10-14"), []);
});

test("a number outside its bounds or not a number is refused, and one inside is accepted", async () => {
  const harness = contentHarness();
  typedInput(harness, "Quantity", "number", { min: "1", max: "10" });
  const limited = { field: 0, problem: "format", type: "number", expected: "a number", limits: { min: "1", max: "10" } };

  assert.deepEqual(await problemsOf(harness, "Quantity", "11"), [limited]);
  assert.deepEqual(await problemsOf(harness, "Quantity", "0"), [limited]);
  assert.deepEqual(await problemsOf(harness, "Quantity", "many"), [limited]);
  assert.deepEqual(await problemsOf(harness, "Quantity", "1,000"), [limited]);
  assert.deepEqual(await problemsOf(harness, "Quantity", "5"), []);
  assert.deepEqual(await problemsOf(harness, "Quantity", "10"), []);
});

test("a range the browser would clamp is refused instead of landing on another value", async () => {
  const harness = contentHarness();
  const volume = typedInput(harness, "Volume", "range", { min: "0", max: "10", step: "1" }, { value: "5" });

  assert.deepEqual(await problemsOf(harness, "Volume", "50"), [{ field: 0, problem: "format", type: "range", expected: "a number", limits: { min: "0", max: "10", step: "1" } }]);
  assert.deepEqual(await problemsOf(harness, "Volume", "7"), []);
  assert.equal(volume.value, "5");
});

test("a range that declares no bounds is refused with the bounds it applies", async () => {
  const harness = contentHarness();
  typedInput(harness, "Volume", "range", {}, { value: "50" });
  const applied = { field: 0, problem: "format", type: "range", expected: "a number", limits: { min: "0", max: "100", step: "1" } };

  assert.deepEqual(await problemsOf(harness, "Volume", "150"), [applied]);
  assert.deepEqual(await problemsOf(harness, "Volume", "-5"), [applied]);
  assert.deepEqual(await problemsOf(harness, "Volume", "loud"), [applied]);
  assert.deepEqual(await problemsOf(harness, "Volume", "100"), []);
});

test("a range value off its step is refused with the step, and an equal number in another form is not", async () => {
  const harness = contentHarness();
  typedInput(harness, "Level", "range", { min: "0", max: "100", step: "10" }, { value: "50" });
  const stepped = { field: 0, problem: "format", type: "range", expected: "a number", limits: { min: "0", max: "100", step: "10" } };

  assert.deepEqual(await problemsOf(harness, "Level", "15"), [stepped]);
  assert.deepEqual(await problemsOf(harness, "Level", "20"), []);
  assert.deepEqual(await problemsOf(harness, "Level", "20.0"), []);
  assert.deepEqual(await problemsOf(harness, "Level", "2e1"), []);
  assert.deepEqual(await problemsOf(harness, "Level", ""), [stepped], "a range has no empty value");
});

test("a range with the step any is clamped to its bounds and refused without a false step", async () => {
  const harness = contentHarness();
  typedInput(harness, "Volume", "range", { min: "0", max: "1", step: "any" }, { value: "1" });
  const bounded = { field: 0, problem: "format", type: "range", expected: "a number", limits: { min: "0", max: "1" } };

  assert.deepEqual(await problemsOf(harness, "Volume", "0.5"), []);
  assert.deepEqual(await problemsOf(harness, "Volume", "0.37"), []);
  assert.deepEqual(await problemsOf(harness, "Volume", "1.5"), [bounded]);
  assert.deepEqual(await problemsOf(harness, "Volume", "-1"), [bounded]);
});

test("a range given a number in another form is held as that number once filled", async () => {
  const harness = contentHarness();
  typedInput(harness, "Level", "range", { min: "0", max: "100", step: "10" }, { value: "20" });
  const field = { locator: await harness.locatorOf("Level"), value: "20.0" };

  const held = await harness.send({ kind: "verify_fill_retained", fields: [field], stable_ms: 0, poll_ms: 5, limit_ms: 50 });

  assert.equal(held.ok, true);
  assert.equal(held.value.retained, true);
  assert.deepEqual(held.value.fields, [{ field: 0, now: "20" }]);
});

test("a color takes any case, and a name that is not a hex color is refused", async () => {
  const harness = contentHarness();
  const accent = typedInput(harness, "Accent", "color", {}, { value: "#000000" });

  assert.deepEqual(await problemsOf(harness, "Accent", "#FF8800"), []);
  assert.deepEqual(await problemsOf(harness, "Accent", "#ff8800"), []);
  assert.deepEqual(await problemsOf(harness, "Accent", "orange"), [{ field: 0, problem: "format", type: "color", expected: "#rrggbb" }]);
  assert.deepEqual(await problemsOf(harness, "Accent", "#f80"), [{ field: 0, problem: "format", type: "color", expected: "#rrggbb" }]);

  const locator = await harness.locatorOf("Accent");
  accent.value = "#ff8800";
  const retained = await harness.send({ kind: "verify_fill_retained", fields: [{ locator, value: "#FF8800" }], stable_ms: 10, poll_ms: 5, limit_ms: 100 });
  assert.deepEqual(retained.value, { retained: true, fields: [{ field: 0, now: "#ff8800" }] });
});

test("time, datetime, month and week each name the one shape they take", async () => {
  const harness = contentHarness();
  const cases = [
    ["time", "Open", "HH:MM", "09:30", "9:30 AM"],
    ["datetime-local", "Meeting", "YYYY-MM-DDTHH:MM", "2026-10-14T09:30", "2026-10-14 9:30"],
    ["month", "Billing", "YYYY-MM", "2026-10", "October 2026"],
    ["week", "Sprint", "YYYY-Www", "2026-W42", "2026-42"]
  ];
  for (const [type, label] of cases) typedInput(harness, label, type);

  for (const [type, label, expected, good, bad] of cases) {
    assert.deepEqual(await problemsOf(harness, label, good), [], type);
    assert.deepEqual(await problemsOf(harness, label, bad), [{ field: 0, problem: "format", type, expected }], type);
  }
});

test("a typed input filled with a list is refused as one value, not probed", async () => {
  const harness = contentHarness();
  typedInput(harness, "Start", "date");

  assert.deepEqual(await problemsOf(harness, "Start", ["2026-10-14"]), [{ field: 0, problem: "multiple_values" }]);
});

test("a file input, a button and a page widget are the wrong tool, each with what it is", async () => {
  const harness = contentHarness();
  typedInput(harness, "Resume", "file");
  typedInput(harness, "Save", "submit");
  typedInput(harness, "Back", "button");
  typedInput(harness, "Clear", "reset");
  const pressed = harness.element("button");
  pressed.setAttribute("aria-label", "Send");
  harness.document.body.append(pressed);
  const widget = harness.element("div");
  widget.setAttribute("role", "combobox");
  widget.setAttribute("aria-label", "Country");
  harness.document.body.append(widget);
  const list = harness.element("div");
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", "Colors");
  harness.document.body.append(list);

  assert.deepEqual(await problemsOf(harness, "Resume", "cv.pdf"), [{ field: 0, problem: "wrong_tool", route: "file" }]);
  for (const label of ["Save", "Back", "Clear", "Send"]) {
    assert.deepEqual(await problemsOf(harness, label, "x"), [{ field: 0, problem: "wrong_tool", route: "button" }], label);
  }
  for (const label of ["Country", "Colors"]) {
    assert.deepEqual(await problemsOf(harness, label, "x"), [{ field: 0, problem: "wrong_tool", route: "widget" }], label);
  }
});

test("an image input and a button with a role of its own are still a button to click", async () => {
  const harness = contentHarness();
  typedInput(harness, "Go", "image");
  for (const [label, role] of [["Dark mode", "switch"], ["Overview", "tab"], ["Settings", "menuitem"]]) {
    const pressed = harness.element("button");
    pressed.setAttribute("role", role);
    pressed.setAttribute("aria-label", label);
    harness.document.body.append(pressed);
  }

  for (const label of ["Go", "Dark mode", "Overview", "Settings"]) {
    assert.deepEqual(await problemsOf(harness, label, "x"), [{ field: 0, problem: "wrong_tool", route: "button" }], label);
  }
});

test("a hidden file input is still named as the wrong tool", async () => {
  const harness = contentHarness();
  const upload = typedInput(harness, "Resume", "file");
  const locator = await harness.locatorOf("Resume");
  upload.hidden = true;

  const refused = await harness.send({ kind: "prepare_fill", fields: [{ locator, value: "cv.pdf" }], visual_settle: false });

  assert.equal(refused.error.code, "not_fillable");
  assert.deepEqual(refused.error.detail.problems, [{ field: 0, problem: "wrong_tool", route: "file" }]);
});

test("a call with a bad date, a bad checkbox word and a file input reports all three and edits nothing", async () => {
  const harness = contentHarness();
  const start = typedInput(harness, "Start", "date");
  const remember = harness.newInput("Remember me", { type: "checkbox", checked: false });
  typedInput(harness, "Resume", "file");
  textInput(harness, { label: "Name", value: "kept" });

  const refused = await prepareRefusal(harness, [["Start", "10/14/2026"], ["Remember me", "maybe"], ["Resume", "cv.pdf"], ["Name", "Ada"]]);

  assert.equal(refused.error.code, "not_fillable");
  assert.equal(refused.error.message, "target is not fillable (3 fields refused)");
  assert.deepEqual(refused.error.detail.problems, [
    { field: 0, problem: "format", type: "date", expected: "YYYY-MM-DD" },
    { field: 1, problem: "not_boolean" },
    { field: 2, problem: "wrong_tool", route: "file" }
  ]);
  assert.equal(start.value, "");
  assert.equal(remember.checked, false);
  assert.equal(harness.input.value, "kept");
  assert.deepEqual(harness.edits, []);
  assert.deepEqual(remember.events, []);
});

test("a filled value is still retained when the page moved a bound after the fill", async () => {
  const harness = contentHarness();
  const end = typedInput(harness, "End", "date", { min: "2026-10-01" });
  const locator = await harness.locatorOf("End");
  await harness.send({ kind: "fill_local", field: { locator, value: "2026-10-14" }, allow_credentials: false });
  end.setAttribute("min", "2026-11-01");

  const verified = await harness.send({ kind: "verify_fill_retained", fields: [{ locator, value: "2026-10-14" }], stable_ms: 10, poll_ms: 5, limit_ms: 100 });

  assert.deepEqual(verified.value, { retained: true, fields: [{ field: 0, now: "2026-10-14" }] });
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

  const inForm = await textLocator({ label: "In form", props: { form: harness.element("form") } });
  const outside = await textLocator({ label: "Outside form" });
  await check("prepareFill submit_outside_form", { kind: "submit_fill", fields: [{ locator: inForm.locator, value: "x" }], submit_locator: outside.locator, allow_credentials: false }, "submit_outside_form", /not contained in the resolved form/);

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
        return { kind: "focus_text_field", field: { locator: await locatorFor("Check"), value: "true" }, allow_credentials: false };
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
  const form = harness.element("form");
  name.form = form;
  send.form = form;
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

// Controls carry their value and what they accept (ADR-0198 decisions 2 and 7). A key appears
// only when it applies, and a credential-class control carries its masked value and nothing else.

const IDENTITY_KEYS = ["credential_class", "locator", "name", "role", "state"];

async function controlNamed(harness, name) {
  return (await harness.targets()).find((target) => target.name === name);
}

/** A labelled select in the body. */
function labelledSelect(harness, label, options, props = {}) {
  const select = harness.select(options);
  select.setAttribute("aria-label", label);
  for (const [name, value] of Object.entries(props)) select[name] = value;
  harness.document.body.append(select);
  return select;
}

test("a select carries its chosen text, its options in order and its placeholder as a hint", async () => {
  const harness = contentHarness();
  const select = labelledSelect(harness, "Role", [
    { value: "", text: "Choose one..." },
    { value: "dev", text: "Developer" },
    { value: "adm", text: "Administrator" },
    { value: "oth", text: "Other" }
  ]);
  select.required = true;

  const placeholder = await controlNamed(harness, "Role");
  assert.equal(placeholder.value, "");
  assert.equal(placeholder.hint, "Choose one...");
  assert.deepEqual(placeholder.options, ["Developer", "Administrator", "Other"]);
  assert.deepEqual(placeholder.state, ["required"]);
  assert.equal("options_total" in placeholder, false);

  select.value = "adm";
  const chosen = await controlNamed(harness, "Role");
  assert.equal(chosen.value, "Administrator");
});

test("only the placeholder reads as empty: a later option with an empty value reads as its own text", async () => {
  const harness = contentHarness();
  const select = labelledSelect(harness, "Filter", [
    { value: "any", text: "Any" },
    { value: "", text: "None" },
    { value: "new", text: "Newest" }
  ]);
  // The fake select picks by value, so name the chosen option directly.
  Object.defineProperty(select, "selectedOptions", { configurable: true, get: () => [select.options[1]] });
  const locator = await harness.locatorOf("Filter");

  assert.equal((await controlNamed(harness, "Filter")).value, "None");
  const read = await harness.send({ kind: "read_text", mode: "visible", locator, max_chars: 500, visual_settle: false });
  assert.equal(read.value.text, "None\nOptions: Any; None; Newest");

  const placed = labelledSelect(harness, "Pick", [
    { value: "", text: "Choose..." },
    { value: "", text: "Nothing" },
    { value: "a", text: "Alpha" }
  ]);
  assert.equal((await controlNamed(harness, "Pick")).value, "");
  Object.defineProperty(placed, "selectedOptions", { configurable: true, get: () => [placed.options[1]] });
  assert.equal((await controlNamed(harness, "Pick")).value, "Nothing");
});

test("a select names an option by its value only when its text is not unique, and marks disabled ones", async () => {
  const harness = contentHarness();
  labelledSelect(harness, "Plan", [
    { value: "a1", text: "Team" },
    { value: "a2", text: "Team" },
    { value: "solo", text: "Solo", disabled: true },
    { value: "pro", text: "Pro" }
  ]);

  const plan = await controlNamed(harness, "Plan");
  assert.deepEqual(plan.options, [
    { text: "Team", value: "a1" },
    { text: "Team", value: "a2" },
    { text: "Solo", disabled: true },
    "Pro"
  ]);
});

test("a select lists an optgroup as a group and a disabled optgroup disables its options", async () => {
  const harness = contentHarness();
  const staff = { label: "Staff" };
  const retired = { label: "Retired", disabled: true };
  labelledSelect(harness, "Team", [
    { value: "all", text: "Everyone" },
    { value: "eng", text: "Engineering", group: staff },
    { value: "ops", text: "Operations", group: staff },
    { value: "old", text: "Archive", group: retired },
    { value: "end", text: "None" }
  ]);

  const team = await controlNamed(harness, "Team");
  assert.deepEqual(team.options, [
    "Everyone",
    { group: "Staff", options: ["Engineering", "Operations"] },
    { group: "Retired", options: [{ text: "Archive", disabled: true }] },
    "None"
  ]);
});

test("a select lists at most 25 options of 60 characters and reports the full count", async () => {
  const harness = contentHarness();
  const options = Array.from({ length: 30 }, (_, index) => ({ value: `v${index}`, text: `Option ${index} ${"x".repeat(80)}` }));
  labelledSelect(harness, "Many", options);

  const many = await controlNamed(harness, "Many");
  assert.equal(many.options.length, 25);
  assert.ok(many.options.every((text) => text.length === 60));
  assert.equal(many.options_total, 30);
  assert.equal(many.value.length, 80);
  assert.equal(many.value_chars, options[0].text.length);
});

test("a multiple select gives an array of its chosen texts and declares multiple", async () => {
  const harness = contentHarness();
  labelledSelect(harness, "Tags", [
    { value: "a", text: "Alpha", selected: true },
    { value: "b", text: "Beta" },
    { value: "c", text: "Gamma", selected: true }
  ], { multiple: true });

  const tags = await controlNamed(harness, "Tags");
  assert.deepEqual(tags.value, ["Alpha", "Gamma"]);
  assert.deepEqual(tags.limits, { multiple: true });
});

test("a field reports a long value cut to 80 characters with its length", async () => {
  const harness = contentHarness();
  harness.newInput("Bio", { value: "b".repeat(200) });
  harness.newInput("Nick", { value: "ok" });
  harness.newInput("Blank");

  const bio = await controlNamed(harness, "Bio");
  assert.equal(bio.value, "b".repeat(80));
  assert.equal(bio.value_chars, 200);
  const nick = await controlNamed(harness, "Nick");
  assert.equal(nick.value, "ok");
  assert.equal("value_chars" in nick, false);
  assert.equal((await controlNamed(harness, "Blank")).value, "");
});

test("a rich editor reports its text as a value and a link reports none", async () => {
  const harness = contentHarness();
  richEditor(harness, { label: "Reply", content: "Hello there" }).append(harness.text("Hello there"));
  const link = harness.element("a");
  link.setAttribute("href", "/x");
  link.setAttribute("aria-label", "Home");
  harness.document.body.append(link);

  assert.equal((await controlNamed(harness, "Reply")).value, "Hello there");
  assert.equal("value" in (await controlNamed(harness, "Home")), false);
});

test("typed inputs say their type and declare only the limits the page declares", async () => {
  const harness = contentHarness();
  const date = harness.newInput("Start", { type: "date" });
  date.setAttribute("min", "2026-01-01");
  date.setAttribute("max", "2026-12-31");
  const range = harness.newInput("Volume", { type: "range", value: "5" });
  range.setAttribute("min", "0");
  range.setAttribute("max", "10");
  range.setAttribute("step", "1");
  const code = harness.newInput("Code", { type: "text" });
  code.setAttribute("minlength", "3");
  code.setAttribute("maxlength", "8");
  code.setAttribute("pattern", "[A-Z]+");
  harness.newInput("Plain");

  const start = await controlNamed(harness, "Start");
  assert.equal(start.type, "date");
  assert.deepEqual(start.limits, { min: "2026-01-01", max: "2026-12-31" });
  const volume = await controlNamed(harness, "Volume");
  assert.equal(volume.type, "range");
  assert.equal(volume.value, "5");
  assert.deepEqual(volume.limits, { min: "0", max: "10", step: "1" });
  const entered = await controlNamed(harness, "Code");
  assert.equal("type" in entered, false);
  assert.deepEqual(entered.limits, { minlength: "3", maxlength: "8", pattern: "[A-Z]+" });
  const plain = await controlNamed(harness, "Plain");
  assert.equal("limits" in plain, false);
  assert.equal("type" in plain, false);
});

test("a hint is a placeholder that adds to the name, else the description, else the select placeholder", async () => {
  const harness = contentHarness();
  const email = harness.newInput("Email");
  email.setAttribute("placeholder", "you@example.com");
  const unlabeled = harness.element("input");
  unlabeled.type = "text";
  unlabeled.setAttribute("placeholder", "Search the docs");
  harness.document.body.append(unlabeled);
  const described = harness.newInput("Username");
  described.setAttribute("aria-describedby", "rules");
  const rules = harness.element("p");
  rules.id = "rules";
  rules.append(harness.text("Letters and digits only"));
  harness.document.body.append(rules);
  const both = harness.newInput("Both");
  both.setAttribute("placeholder", "from the placeholder");
  both.setAttribute("aria-describedby", "rules");
  harness.newInput("Neither");

  assert.equal((await controlNamed(harness, "Email")).hint, "you@example.com");
  assert.equal("hint" in (await controlNamed(harness, "Search the docs")), false);
  assert.equal((await controlNamed(harness, "Username")).hint, "Letters and digits only");
  assert.equal((await controlNamed(harness, "Both")).hint, "from the placeholder");
  assert.equal("hint" in (await controlNamed(harness, "Neither")), false);
});

test("a radio or checkbox names its fieldset legend or labelled group", async () => {
  const harness = contentHarness();
  const fieldset = harness.element("fieldset");
  const legend = harness.element("legend");
  legend.append(harness.text("Shipping speed"));
  const fast = harness.element("input");
  fast.type = "radio";
  fast.setAttribute("type", "radio");
  fast.setAttribute("aria-label", "Express");
  fieldset.append(legend, fast);
  const group = harness.element("div");
  group.setAttribute("role", "radiogroup");
  group.setAttribute("aria-label", "Gift wrap");
  const wrap = harness.element("input");
  wrap.type = "checkbox";
  wrap.setAttribute("type", "checkbox");
  wrap.setAttribute("aria-label", "Ribbon");
  group.append(wrap);
  const lone = harness.element("input");
  lone.type = "checkbox";
  lone.setAttribute("type", "checkbox");
  lone.setAttribute("aria-label", "Newsletter");
  harness.document.body.append(fieldset, group, lone);

  assert.equal((await controlNamed(harness, "Express")).group, "Shipping speed");
  assert.equal((await controlNamed(harness, "Ribbon")).group, "Gift wrap");
  assert.equal("group" in (await controlNamed(harness, "Newsletter")), false);
});

test("an invalid control says why in Ghostlight's words, never the browser's message", async () => {
  const harness = contentHarness();
  const cases = [
    ["Missing", { valueMissing: true }, {}, "is required"],
    ["Mail", { typeMismatch: true }, { type: "email" }, "is not a valid email address"],
    ["Link", { typeMismatch: true }, { type: "url" }, "is not a valid URL"],
    ["Phone", { typeMismatch: true }, { type: "tel" }, "is not valid"],
    ["Short", { tooShort: true }, { minlength: "5" }, "is too short (at least 5 characters)"],
    ["ShortBare", { tooShort: true }, {}, "is too short"],
    ["Long", { tooLong: true }, {}, "is too long"],
    ["Low", { rangeUnderflow: true }, { min: "18" }, "is below the minimum (min 18)"],
    ["High", { rangeOverflow: true }, { max: "99" }, "is above the maximum (max 99)"],
    ["Step", { stepMismatch: true }, {}, "does not match the allowed step"],
    ["Format", { patternMismatch: true }, {}, "does not match the required format"],
    ["Bad", { badInput: true }, { type: "number" }, "is not a valid number"],
    ["Custom", { customError: true }, {}, "Pick an unused handle"]
  ];
  for (const [label, flags, attributes] of cases) {
    const element = harness.newInput(label, { type: attributes.type ?? "text" });
    for (const [name, value] of Object.entries(attributes)) if (name !== "type") element.setAttribute(name, value);
    element.userInvalid = true;
    element.validity = { valid: false, ...flags };
    element.validationMessage = "Pick an unused handle";
  }
  const quoting = harness.newInput("Quoting", { value: "Ada" });
  quoting.userInvalid = true;
  quoting.validity = { valid: false, valueMissing: true };
  quoting.validationMessage = "Please fill out this field, Ada";

  for (const [label, , , expected] of cases) {
    const found = await controlNamed(harness, label);
    assert.equal(found.error, expected, label);
    assert.ok(found.state.includes("invalid"), label);
  }
  assert.equal((await controlNamed(harness, "Quoting")).error, "is required");
});

test("an error comes from aria-errormessage first, and aria-invalid alone reads 'is invalid'", async () => {
  const harness = contentHarness();
  const email = harness.newInput("Email");
  email.setAttribute("aria-invalid", "true");
  email.setAttribute("aria-errormessage", "email-error");
  email.validity = { valid: false, valueMissing: true };
  const message = harness.element("span");
  message.id = "email-error";
  message.append(harness.text("Enter the address we should write to"));
  harness.document.body.append(message);
  const bare = harness.newInput("Bare");
  bare.setAttribute("aria-invalid", "true");
  const fine = harness.newInput("Fine");
  fine.setAttribute("aria-invalid", "false");

  assert.equal((await controlNamed(harness, "Email")).error, "Enter the address we should write to");
  const bareFound = await controlNamed(harness, "Bare");
  assert.equal(bareFound.error, "is invalid");
  assert.deepEqual(bareFound.state, ["invalid"]);
  const fineFound = await controlNamed(harness, "Fine");
  assert.equal("error" in fineFound, false);
  assert.deepEqual(fineFound.state, []);
});

test("required and read-only controls say so in their state", async () => {
  const harness = contentHarness();
  harness.newInput("Must", { required: true });
  harness.newInput("Fixed", { readOnly: true });
  const aria = harness.newInput("Aria");
  aria.setAttribute("aria-required", "true");

  assert.deepEqual((await controlNamed(harness, "Must")).state, ["required"]);
  assert.deepEqual((await controlNamed(harness, "Fixed")).state, ["readonly"]);
  assert.deepEqual((await controlNamed(harness, "Aria")).state, ["required"]);
});

test("a credential-class control carries its masked value and no other new key", async () => {
  const harness = contentHarness();
  const filled = harness.newInput("Password", { type: "password", value: "hunter2hunter2" });
  filled.setAttribute("placeholder", "your secret");
  filled.setAttribute("maxlength", "20");
  filled.setAttribute("aria-describedby", "why");
  filled.userInvalid = true;
  filled.validity = { valid: false, tooShort: true, customError: true };
  filled.validationMessage = "hunter2hunter2 is wrong";
  harness.newInput("Code", { type: "password", value: "" });
  const why = harness.element("p");
  why.id = "why";
  why.append(harness.text("Use a strong one"));
  harness.document.body.append(why);
  const secretSelect = labelledSelect(harness, "Token kind", [
    { value: "", text: "Pick..." },
    { value: "a", text: "Alpha" },
    { value: "b", text: "Beta" }
  ]);
  secretSelect.setAttribute("name", "access_token");
  secretSelect.value = "b";
  const emptySelect = labelledSelect(harness, "Secret kind", [
    { value: "", text: "Pick..." },
    { value: "a", text: "Alpha" }
  ]);
  emptySelect.setAttribute("name", "secret_kind");

  const targets = await harness.targets();
  const byName = (name) => targets.find((target) => target.name === name);
  const allowed = [...IDENTITY_KEYS, "value"].sort();
  for (const name of ["Password", "Code", "Token kind", "Secret kind"]) {
    assert.deepEqual(Object.keys(byName(name)).sort(), allowed, name);
  }
  assert.equal(byName("Password").value, "[hidden]");
  assert.ok(byName("Password").state.includes("invalid"));
  assert.equal(byName("Code").value, "");
  assert.equal(byName("Token kind").value, "[hidden]");
  assert.equal(byName("Secret kind").value, "");
  assert.doesNotMatch(JSON.stringify(targets), /hunter2|Alpha|Beta|your secret|strong one|Pick/);
});

test("reading a select ends with a line naming every option and skips a credential select", async () => {
  const harness = contentHarness();
  const select = labelledSelect(harness, "Role", [
    { value: "", text: "Choose one..." },
    { value: "dev", text: "Developer" },
    { value: "adm", text: "Administrator", disabled: true },
    { value: "res", text: "Researcher" }
  ]);
  const locator = await harness.locatorOf("Role");
  const read = (target, maxChars = 500) => harness.send({ kind: "read_text", mode: "visible", locator: target, max_chars: maxChars, visual_settle: false });

  assert.equal((await read(locator)).value.text, "Options: Developer; Administrator (disabled); Researcher");

  select.value = "res";
  assert.equal((await read(locator)).value.text, "Researcher\nOptions: Developer; Administrator (disabled); Researcher");
  const cut = await read(locator, 21);
  assert.equal(cut.value.text, "Researcher\nOptions: D");
  assert.equal(cut.value.truncated, true);

  const secret = labelledSelect(harness, "Key kind", [{ value: "a", text: "Alpha" }, { value: "b", text: "Beta" }]);
  secret.setAttribute("name", "api_token");
  assert.equal((await read(await harness.locatorOf("Key kind"))).value.text, "[hidden]");
});

// ---- a fill says what the page now holds -----------------------------------------------------------

const verifyFill = (fields, extra = {}) => ({ kind: "verify_fill_retained", fields, stable_ms: 10, poll_ms: 5, limit_ms: 60, ...extra });

test("verify_fill_retained echoes what a choice or typed control holds and not plain text", async () => {
  const harness = contentHarness();
  const role = labelledSelect(harness, "Role", [
    { value: "", text: "Choose one..." },
    { value: "adm", text: "Administrator" }
  ]);
  const name = textInput(harness, { label: "Name", value: "Ada" });
  const roleLocator = await harness.locatorOf("Role");
  const nameLocator = await harness.locatorOf("Name");
  role.value = "adm";

  const reply = await harness.send(verifyFill([
    { locator: nameLocator, value: "Ada" },
    { locator: roleLocator, value: "Administrator" }
  ]));

  assert.deepEqual(reply, { ok: true, value: { retained: true, fields: [{ field: 1, now: "Administrator" }] } });
  assert.equal(name.value, "Ada");
});

test("verify_fill_retained echoes a checkbox by its state and a date by its value", async () => {
  const harness = contentHarness();
  const box = textInput(harness, { label: "Agree" });
  box.type = "checkbox";
  box.checked = true;
  const agree = await harness.locatorOf("Agree");
  assert.deepEqual((await harness.send(verifyFill([{ locator: agree, value: "yes" }]))).value.fields, [{ field: 0, now: "checked" }]);

  box.type = "date";
  box.value = "2026-10-15";
  assert.deepEqual((await harness.send(verifyFill([{ locator: agree, value: "2026-10-15" }]))).value.fields, [{ field: 0, now: "2026-10-15" }]);
});

test("a phone mask is held in another format, echoed and not a failure", async () => {
  const harness = contentHarness();
  textInput(harness, { label: "Phone", value: "(555) 123-4567" });
  const locator = await harness.locatorOf("Phone");

  const reply = await harness.send(verifyFill([{ locator, value: "5551234567" }]));

  assert.deepEqual(reply, { ok: true, value: { retained: true, fields: [{ field: 0, now: "(555) 123-4567", tolerated: true }] } });
});

test("formatting tolerance folds case and ignores spaces, brackets, hyphens and slashes only", async () => {
  const harness = contentHarness();
  const input = textInput(harness, { label: "Code", value: "ab-12/34 cd" });
  const locator = await harness.locatorOf("Code");

  assert.equal((await harness.send(verifyFill([{ locator, value: "AB123 4CD" }]))).ok, true);

  input.value = "1050";
  const currency = await harness.send(verifyFill([{ locator, value: "10.50" }]));
  assert.equal(currency.error.code, "not_retained", "a point is significant");
  assert.deepEqual(currency.error.detail, { problems: [{ field: 0, problem: "reverted", now: "1050" }] });

  input.value = "10,5";
  assert.equal((await harness.send(verifyFill([{ locator, value: "105" }]))).error.code, "not_retained", "a comma is significant");
});

test("a select the page resets after its change is named with what it holds now", async () => {
  const harness = contentHarness();
  const role = labelledSelect(harness, "Role", [
    { value: "", text: "Choose one..." },
    { value: "adm", text: "Administrator" }
  ]);
  const plan = labelledSelect(harness, "Plan", [{ value: "free", text: "Free" }, { value: "pro", text: "Pro" }]);
  const roleLocator = await harness.locatorOf("Role");
  const planLocator = await harness.locatorOf("Plan");
  role.value = "";
  plan.value = "pro";

  const reply = await harness.send(verifyFill([
    { locator: planLocator, value: "Pro" },
    { locator: roleLocator, value: "Administrator" }
  ]));

  assert.equal(reply.error.code, "not_retained");
  assert.deepEqual(reply.error.detail, { problems: [{ field: 1, problem: "reverted", now: "Choose one..." }] });
});

test("a credential-class field is never echoed, and a reverted one carries no value", async () => {
  const harness = contentHarness();
  const secret = textInput(harness, { label: "Secret", value: "hunter2" });
  secret.type = "password";
  const locator = await harness.locatorOf("Secret");
  const fields = [{ locator, value: "hunter2" }];

  assert.deepEqual((await harness.send(verifyFill(fields, { allow_credentials: true }))).value, { retained: true, fields: [] });

  secret.value = "other";
  const reply = await harness.send(verifyFill(fields, { allow_credentials: true }));
  assert.deepEqual(reply.error.detail, { problems: [{ field: 0, problem: "reverted" }] });
});

test("a credential-class field the page reformatted is a failure, not a tolerated format", async () => {
  const harness = contentHarness();
  const secret = textInput(harness, { label: "Key", value: "abc-123" });
  secret.type = "password";
  const locator = await harness.locatorOf("Key");
  const fields = [{ locator, value: "abc-123" }];

  secret.value = "ABC123";
  const reply = await harness.send(verifyFill(fields, { allow_credentials: true }));

  assert.equal(reply.error.code, "not_retained");
  assert.deepEqual(reply.error.detail, { problems: [{ field: 0, problem: "reverted" }] });
});

test("a field that is stable only part of the window still fails without naming a change", async () => {
  const harness = contentHarness();
  textInput(harness, { label: "Name", value: "Ada" });
  const locator = await harness.locatorOf("Name");

  const reply = await harness.send({ kind: "verify_fill_retained", fields: [{ locator, value: "Ada" }], stable_ms: 500, poll_ms: 5, limit_ms: 60 });

  assert.equal(reply.error.code, "not_retained");
  assert.equal("detail" in reply.error, false);
});

// ---- items name their page area and form ---------------------------------------------------------

/** The inspected items of the page, by name. */
async function itemsByName(harness, kind = "all") {
  const reply = await harness.send({ kind: "inspect", inspect_kind: kind, max_items: 100, visual_settle: false });
  return Object.fromEntries(reply.value.targets.filter((target) => target.name).map((target) => [target.name, target]));
}

/** A button named `label` appended to `parent`. */
function labelledButton(harness, parent, label) {
  const button = harness.element("button");
  button.setAttribute("aria-label", label);
  parent.append(button);
  return button;
}

/** An element of `tag`, with `attributes`, appended to `parent`. */
function wrapper(harness, parent, tag, attributes = {}) {
  const element = harness.element(tag);
  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value);
  parent.append(element);
  return element;
}

test("an item names the landmark that holds it, by HTML-AAM scoping", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const banner = wrapper(harness, body, "header");
  labelledButton(harness, banner, "Logo");
  const navigation = wrapper(harness, banner, "nav");
  labelledButton(harness, navigation, "Pricing");
  const main = wrapper(harness, body, "main");
  labelledButton(harness, main, "Content");
  const article = wrapper(harness, main, "article");
  labelledButton(harness, wrapper(harness, article, "header"), "Article head");
  labelledButton(harness, wrapper(harness, article, "footer"), "Article foot");
  labelledButton(harness, wrapper(harness, wrapper(harness, main, "section"), "header"), "Section head");
  labelledButton(harness, wrapper(harness, main, "aside"), "Related");
  labelledButton(harness, wrapper(harness, main, "dialog"), "Close");
  labelledButton(harness, wrapper(harness, main, "search"), "Find");
  labelledButton(harness, wrapper(harness, main, "div", { role: "search" }), "Query");
  labelledButton(harness, wrapper(harness, main, "div", { role: "alertdialog" }), "Dismiss");
  labelledButton(harness, wrapper(harness, body, "div", { role: "banner" }), "Role banner");
  labelledButton(harness, wrapper(harness, body, "div", { role: "contentinfo" }), "Role info");
  labelledButton(harness, wrapper(harness, body, "div", { role: "navigation" }), "Role nav");
  labelledButton(harness, wrapper(harness, body, "div", { role: "complementary" }), "Role aside");
  labelledButton(harness, wrapper(harness, body, "nav", { role: "presentation" }), "Not a landmark");
  labelledButton(harness, wrapper(harness, body, "footer"), "Page foot");
  harness.setBody(body);

  const items = await itemsByName(harness, "controls");
  const areas = Object.fromEntries(Object.entries(items).map(([name, item]) => [name, item.area ?? null]));

  assert.deepEqual(areas, {
    Logo: "header",
    Pricing: "nav",
    Content: null,
    "Article head": null,
    "Article foot": null,
    "Section head": null,
    Related: "aside",
    Close: "dialog",
    Find: "search",
    Query: "search",
    Dismiss: "dialog",
    "Role banner": "header",
    "Role info": "footer",
    "Role nav": "nav",
    "Role aside": "aside",
    "Not a landmark": null,
    "Page foot": "footer"
  });
  assert.ok(!Object.hasOwn(items.Content, "area"), "page content carries no area key");
});

test("a landmark item names its own area, and find and describe agree with inspect", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const navigation = wrapper(harness, body, "nav", { "aria-label": "Site" });
  labelledButton(harness, navigation, "Pricing");
  harness.setBody(body);

  const structure = await itemsByName(harness, "structure");
  assert.equal(structure.Site.area, "nav");
  const found = await harness.send({ kind: "find", text: "Pricing", find_kind: "control", max_results: 5 });
  assert.equal(found.value.targets[0].area, "nav");
  const described = await harness.send({ kind: "describe", locators: [found.value.targets[0].locator] });
  assert.equal(described.value.targets[0].area, "nav");
  const semantic = await harness.send({ kind: "query_semantic", name: "Pricing", exact: true });
  assert.equal(semantic.value.targets[0].area, "nav");
});

test("a control belongs to the form its form attribute names", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const first = wrapper(harness, body, "form", { id: "first" });
  first.id = "first";
  const second = wrapper(harness, body, "form", { id: "second" });
  second.id = "second";
  nativeControl(harness, first, "Inside first", "input", { type: "text" });
  nativeControl(harness, body, "Pointed at second", "input", { type: "text", form: "second" });
  nativeControl(harness, first, "Pulled into second", "button", { form: "second" });
  nativeControl(harness, body, "Loose", "input", { type: "text" });
  nativeControl(harness, body, "Pointed at nothing", "input", { type: "text", form: "missing" });
  harness.setBody(body);

  const items = await itemsByName(harness, "controls");
  const forms = Object.fromEntries(Object.entries(items).map(([name, item]) => [name, item.form ?? null]));

  assert.deepEqual(forms, {
    "Inside first": 1,
    "Pointed at second": 2,
    "Pulled into second": 2,
    Loose: null,
    "Pointed at nothing": null
  });
});

test("a form numbers itself, and ordinals follow the order of the forms in the document", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  wrapper(harness, body, "form", { "aria-label": "Newsletter" });
  const search = wrapper(harness, body, "form", { "aria-label": "Account" });
  harness.setBody(body);
  nativeControl(harness, search, "Email", "input", { type: "text" });

  const structure = await itemsByName(harness, "all");

  assert.equal(structure.Newsletter.form, 1);
  assert.equal(structure.Account.form, 2);
  assert.equal(structure.Email.form, 2);
});

test("an input in a shadow root inside a light-DOM form belongs to that form", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const form = wrapper(harness, body, "form");
  const host = wrapper(harness, form, "email-field");
  const shadow = harness.fragment();
  shadow.host = host;
  const inner = harness.element("input");
  inner.setAttribute("aria-label", "Inner email");
  shadow.append(inner);
  host.shadowRoot = shadow;
  nativeControl(harness, body, "Outside", "input", { type: "text" });
  harness.setBody(body);

  const items = await itemsByName(harness, "controls");

  assert.equal(items["Inner email"].form, 1);
  assert.ok(!Object.hasOwn(items.Outside, "form"));
});

test("shadow content comes out where its host sits in the page", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  labelledButton(harness, body, "One");
  const host = wrapper(harness, body, "x-widget");
  const shadow = harness.fragment();
  shadow.host = host;
  const inner = harness.element("button");
  inner.setAttribute("aria-label", "Two");
  shadow.append(inner);
  host.shadowRoot = shadow;
  labelledButton(harness, body, "Three");
  const slotHost = wrapper(harness, body, "x-slotted");
  const slotShadow = harness.fragment();
  slotShadow.host = slotHost;
  const slot = harness.element("slot");
  const lead = harness.element("button");
  lead.setAttribute("aria-label", "Four");
  slotShadow.append(lead, slot);
  slotHost.shadowRoot = slotShadow;
  const slotted = labelledButton(harness, slotHost, "Five");
  slot.assignedNodes = () => [slotted];
  labelledButton(harness, body, "Six");
  harness.setBody(body);

  const reply = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 100, visual_settle: false });

  assert.deepEqual(reply.value.targets.map((target) => target.name).filter(Boolean), ["One", "Two", "Three", "Four", "Five", "Six"]);
  const cut = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 2, visual_settle: false });
  assert.deepEqual(cut.value.targets.map((target) => target.name), ["One", "Two"]);
});

test("a dialog scopes its own header and footer, so they are not the page's", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const modal = wrapper(harness, body, "dialog");
  labelledButton(harness, wrapper(harness, modal, "header"), "Modal close");
  labelledButton(harness, wrapper(harness, modal, "footer"), "Modal save");
  const alert = wrapper(harness, body, "div", { role: "alertdialog" });
  labelledButton(harness, wrapper(harness, alert, "header"), "Alert close");
  labelledButton(harness, wrapper(harness, alert, "footer"), "Alert ok");
  labelledButton(harness, wrapper(harness, body, "header"), "Page head");
  labelledButton(harness, wrapper(harness, body, "footer"), "Page foot");
  harness.setBody(body);

  const items = await itemsByName(harness, "controls");
  const areas = Object.fromEntries(Object.entries(items).map(([name, item]) => [name, item.area ?? null]));

  assert.deepEqual(areas, {
    "Modal close": "dialog",
    "Modal save": "dialog",
    "Alert close": "dialog",
    "Alert ok": "dialog",
    "Page head": "header",
    "Page foot": "footer"
  });
});

test("every targets answer reports how many forms the document holds, kept or not", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const first = wrapper(harness, body, "form");
  nativeControl(harness, first, "In first", "input", { type: "text" });
  nativeControl(harness, wrapper(harness, body, "form"), "In second", "input", { type: "text" });
  nativeControl(harness, wrapper(harness, body, "form"), "In third", "input", { type: "text" });
  harness.setBody(body);

  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 1, visual_settle: false });
  assert.equal(inspected.value.targets.length, 1);
  assert.equal(inspected.value.forms, 3);
  const found = await harness.send({ kind: "find", text: "In first", find_kind: "control", max_results: 5 });
  assert.equal(found.value.forms, 3);
  const semantic = await harness.send({ kind: "query_semantic", name: "In first", exact: true });
  assert.equal(semantic.value.forms, 3);
});

// ---- a capped answer is cut by tier and counts what it left out ----------------------------------

/** The names of an inspect answer's targets that have one. */
const namesOf = (reply) => reply.value.targets.map((target) => target.name).filter(Boolean);

/** A harness whose stray fixture input is a hidden input, so it never joins a pool. */
function pageHarness() {
  const harness = contentHarness();
  harness.input.type = "hidden";
  return harness;
}

/** Inspect the controls of the page with `max_items`. */
const inspectControls = (harness, maxItems) => harness.send({ kind: "inspect", inspect_kind: "controls", max_items: maxItems, visual_settle: false });

test("a navigation-heavy page keeps its form fields when the cap cuts, in document order", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  const banner = wrapper(harness, body, "header");
  const navigation = wrapper(harness, banner, "nav");
  for (let index = 0; index < 8; index += 1) labelledButton(harness, navigation, `Menu ${index}`);
  const main = wrapper(harness, body, "main");
  const form = wrapper(harness, main, "form");
  for (const label of ["Name", "Email", "Password"]) nativeControl(harness, form, label, "input", { type: "text" });
  labelledButton(harness, wrapper(harness, body, "footer"), "Terms");
  harness.setBody(body);

  const reply = await inspectControls(harness, 5);

  assert.deepEqual(namesOf(reply), ["Menu 0", "Menu 1", "Name", "Email", "Password"]);
  assert.deepEqual(reply.value.left_out, { chrome: 7 });
  assert.equal(reply.value.truncated, true);
  assert.equal(reply.value.modal_open, false);
});

test("hidden items are cut before chrome, chrome before asides, and asides before content", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  labelledButton(harness, wrapper(harness, body, "nav"), "Menu");
  const ghost = labelledButton(harness, body, "Ghost");
  ghost.hidden = true;
  labelledButton(harness, wrapper(harness, body, "aside"), "Related");
  labelledButton(harness, body, "Save");
  harness.setBody(body);

  const three = await inspectControls(harness, 3);
  assert.deepEqual(namesOf(three), ["Menu", "Related", "Save"]);
  assert.deepEqual(three.value.left_out, { hidden: 1 });

  const two = await inspectControls(harness, 2);
  assert.deepEqual(namesOf(two), ["Related", "Save"]);
  assert.deepEqual(two.value.left_out, { hidden: 1, chrome: 1 });

  const one = await inspectControls(harness, 1);
  assert.deepEqual(namesOf(one), ["Save"]);
  assert.deepEqual(one.value.left_out, { hidden: 1, chrome: 1, aside: 1 });
});

test("dialog and search items are page content, and a hidden item inside them is still cut first", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  const dialog = wrapper(harness, body, "dialog");
  labelledButton(harness, wrapper(harness, dialog, "header"), "Close");
  const search = wrapper(harness, body, "search");
  labelledButton(harness, search, "Go").hidden = true;
  labelledButton(harness, search, "Query");
  labelledButton(harness, wrapper(harness, body, "nav"), "Menu");
  harness.setBody(body);

  const reply = await inspectControls(harness, 2);

  assert.deepEqual(namesOf(reply), ["Close", "Query"]);
  assert.deepEqual(reply.value.left_out, { hidden: 1, chrome: 1 });
});

test("an answer that fits carries no counts and is not truncated", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  labelledButton(harness, wrapper(harness, body, "nav"), "Menu").hidden = true;
  labelledButton(harness, body, "Save");
  harness.setBody(body);

  const reply = await inspectControls(harness, 2);

  assert.deepEqual(namesOf(reply), ["Menu", "Save"]);
  assert.deepEqual(reply.value.left_out, {});
  assert.equal(reply.value.truncated, false);
});

test("controls behind an open modal dialog are dropped and counted apart from the cap", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  labelledButton(harness, wrapper(harness, body, "nav"), "Menu");
  labelledButton(harness, body, "Behind");
  const dialog = wrapper(harness, body, "dialog");
  dialog.open = true;
  dialog.modal = true;
  labelledButton(harness, dialog, "Confirm");
  nativeControl(harness, dialog, "Reason", "input", { type: "text" });
  harness.setBody(body);

  const reply = await inspectControls(harness, 80);

  assert.deepEqual(namesOf(reply), ["Confirm", "Reason"]);
  assert.deepEqual(reply.value.left_out, { behind_dialog: 2 });
  assert.equal(reply.value.modal_open, true);
  assert.equal(reply.value.truncated, false, "dropping unusable controls is not a size cut");
});

test("a dialog that is open but not modal leaves the page usable", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  labelledButton(harness, body, "Page");
  const dialog = wrapper(harness, body, "dialog");
  dialog.open = true;
  labelledButton(harness, dialog, "Inside");
  harness.setBody(body);

  const reply = await inspectControls(harness, 80);

  assert.deepEqual(namesOf(reply), ["Page", "Inside"]);
  assert.deepEqual(reply.value.left_out, {});
  assert.equal(reply.value.modal_open, false);
});

test("controls under an inert ancestor are dropped and counted like those behind a modal", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  const frozen = wrapper(harness, body, "div", { inert: "" });
  labelledButton(harness, frozen, "Frozen");
  labelledButton(harness, body, "Live");
  harness.setBody(body);

  const reply = await inspectControls(harness, 80);

  assert.deepEqual(namesOf(reply), ["Live"]);
  assert.deepEqual(reply.value.left_out, { behind_dialog: 1 });
  assert.equal(reply.value.modal_open, false);
});

test("structure items are not dropped for sitting behind a modal", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  wrapper(harness, body, "nav", { "aria-label": "Site" });
  labelledButton(harness, body, "Behind");
  const dialog = wrapper(harness, body, "dialog");
  dialog.open = true;
  dialog.modal = true;
  labelledButton(harness, dialog, "Confirm");
  harness.setBody(body);

  const reply = await harness.send({ kind: "inspect", inspect_kind: "all", max_items: 80, visual_settle: false });

  assert.deepEqual(namesOf(reply), ["Site", "Confirm"]);
  assert.deepEqual(reply.value.left_out, { behind_dialog: 1 });
});

test("a find cuts by the same tiers and counts what it left out", async () => {
  const harness = pageHarness();
  const body = harness.element("body");
  labelledButton(harness, wrapper(harness, body, "nav"), "Save draft");
  labelledButton(harness, body, "Save");
  labelledButton(harness, body, "Save as");
  harness.setBody(body);

  const reply = await harness.send({ kind: "find", text: "Save", find_kind: "control", max_results: 2 });

  assert.deepEqual(namesOf(reply), ["Save", "Save as"]);
  assert.deepEqual(reply.value.left_out, { chrome: 1 });
  assert.equal(reply.value.truncated, true);
});

// ---- repeated controls: row context and nth ----------------------------------------------------

/** An element with attributes and children, for building a fixture tree. */
function node(harness, tag, attributes = {}, ...children) {
  const element = harness.element(tag);
  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value);
  for (const child of children) element.append(typeof child === "string" ? harness.text(child) : child);
  return element;
}

/** A Suno-shaped song list: one named group per title, each with a song link and a menu button. */
function songRows(harness, titles) {
  const body = harness.element("body");
  titles.forEach((title, index) => {
    body.append(node(harness, "div", { role: "group", "aria-label": title },
      node(harness, "a", { href: `/song/${index}` }, title),
      node(harness, "button", { "aria-label": "More options", "aria-expanded": "false" })));
  });
  harness.setBody(body);
  return body;
}

/** What a result says about where each of its targets named `name` sits. */
function placesOf(targets, name) {
  return targets.filter((target) => target.name === name).map((target) => [target.context ?? null, target.nth ?? null]);
}

async function findControls(harness, text) {
  return (await harness.send({ kind: "find", text, find_kind: "control", max_results: 40 })).value.targets;
}

test("a control whose name repeats carries its row label, and nth when the row label repeats", async () => {
  const harness = contentHarness();
  songRows(harness, ["Alpha", "Alpha", "Beta", "Beta", "Gamma", "Gamma"]);

  const found = await findControls(harness, "More options");

  assert.deepEqual(placesOf(found, "More options"), [
    ["Alpha", 1], ["Alpha", 2], ["Beta", 1], ["Beta", 2], ["Gamma", 1], ["Gamma", 2]
  ]);

  songRows(harness, ["Alpha", "Beta", "Gamma"]);
  assert.deepEqual(placesOf(await findControls(harness, "More options"), "More options"), [
    ["Alpha", null], ["Beta", null], ["Gamma", null]
  ]);
});

test("a control with a unique name carries no context", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  body.append(node(harness, "div", { role: "group", "aria-label": "Alpha" },
    node(harness, "button", { "aria-label": "Play" }),
    node(harness, "button", { "aria-label": "More options" })));
  harness.setBody(body);

  const buttons = [...await findControls(harness, "Play"), ...await findControls(harness, "More options")];
  assert.equal(buttons.length, 2);
  for (const target of buttons) {
    assert.equal("context" in target, false, target.name);
    assert.equal("nth" in target, false, target.name);
  }
});

test("a radio whose row label equals its group shows only group", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  for (const [legend, group] of [["Plan", "plan"], ["Billing", "billing"]]) {
    const fieldset = node(harness, "fieldset", {}, node(harness, "legend", {}, legend));
    fieldset.append(node(harness, "input", { type: "radio", name: group, "aria-label": "Yearly" }));
    body.append(fieldset);
  }
  harness.setBody(body);

  const yearly = (await findControls(harness, "Yearly")).filter((target) => target.name === "Yearly");

  assert.deepEqual(yearly.map((target) => target.group), ["Plan", "Billing"]);
  for (const target of yearly) {
    assert.equal("context" in target, false);
    assert.equal("nth" in target, false);
  }
});

test("a row named by aria-labelledby of a field never carries the field value", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const field = node(harness, "input", { id: "title-field", type: "text", "aria-label": "Title" });
  field.value = "Ada Lovelace";
  body.append(field);
  for (let index = 0; index < 2; index += 1) {
    body.append(node(harness, "li", { "aria-labelledby": "title-field" }, node(harness, "button", { "aria-label": "Edit" })));
  }
  harness.setBody(body);

  const found = await findControls(harness, "Edit");

  assert.doesNotMatch(JSON.stringify(found), /Ada Lovelace/);
  assert.deepEqual(placesOf(found, "Edit"), [[null, 1], [null, 2]]);
});

test("a row with no declared name takes its heading, else its first link name", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const play = () => node(harness, "button", { "aria-label": "Play" });
  body.append(
    node(harness, "li", {}, node(harness, "a", { href: "/one" }, "Link one"), node(harness, "h3", {}, "Heading one"), play()),
    node(harness, "li", {}, node(harness, "span", {}, "no name here"), node(harness, "a", { href: "/two" }, "Link two"), play()),
    node(harness, "li", {}, play()));
  harness.setBody(body);

  assert.deepEqual(placesOf(await findControls(harness, "Play"), "Play"), [
    ["Heading one", null], ["Link two", null], [null, null]
  ]);
});

test("a control in a dialog takes no item from outside the dialog", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const close = () => node(harness, "button", { "aria-label": "Close" });
  body.append(node(harness, "li", {}, node(harness, "h3", {}, "Outer row"), close(),
    node(harness, "div", { role: "dialog", "aria-label": "Settings" }, close())));
  harness.setBody(body);

  assert.deepEqual(placesOf(await findControls(harness, "Close"), "Close"), [["Outer row", null], ["Settings", null]]);
});

test("a control in an open shadow root takes its host row label", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  const host = harness.element("open-menu");
  const shadow = harness.fragment();
  shadow.host = host;
  shadow.append(node(harness, "button", { "aria-label": "Open" }));
  host.shadowRoot = shadow;
  body.append(
    node(harness, "li", {}, node(harness, "h3", {}, "Hosted row"), host),
    node(harness, "li", {}, node(harness, "h3", {}, "Plain row"), node(harness, "button", { "aria-label": "Open" })));
  harness.setBody(body);

  assert.deepEqual(placesOf(await findControls(harness, "Open"), "Open"), [["Hosted row", null], ["Plain row", null]]);
});

test("a credential-class control never carries context or nth", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  for (const title of ["First", "Second"]) {
    body.append(node(harness, "li", {}, node(harness, "h3", {}, title), node(harness, "input", { type: "password", "aria-label": "Code" })));
  }
  harness.setBody(body);

  const codes = (await findControls(harness, "Code")).filter((target) => target.name === "Code");

  assert.equal(codes.length, 2);
  for (const target of codes) {
    assert.equal(target.credential_class, true);
    assert.equal("context" in target, false);
    assert.equal("nth" in target, false);
  }
});

test("find, inspect and the semantic query agree on context", async () => {
  const harness = contentHarness();
  songRows(harness, ["Alpha", "Alpha", "Beta"]);

  const found = placesOf(await findControls(harness, "More options"), "More options");
  const inspected = await harness.send({ kind: "inspect", inspect_kind: "controls", max_items: 40, visual_settle: false });
  const queried = await harness.send({ kind: "query_semantic", name: "More options", exact: true });

  assert.deepEqual(found, [["Alpha", 1], ["Alpha", 2], ["Beta", null]]);
  assert.deepEqual(placesOf(inspected.value.targets, "More options"), found);
  assert.deepEqual(placesOf(queried.value.targets, "More options"), found);
});

// ---- a stale handle's lookup: the one trusted replacement --------------------------------------

/** Mark a subtree as left the document, the way a re-render drops the old nodes. */
function detach(element) {
  element.isConnected = false;
  for (const child of element.childNodes ?? []) detach(child);
}

/**
 * A song library: one named group per [title, id], each with its own song link and a menu button.
 * With `named` false each row is a list item that only its link names, which a handle never
 * follows through a re-render.
 */
function libraryRows(harness, entries, { before = [], region = null, named = true } = {}) {
  const body = harness.element("body");
  for (const extra of before) body.append(extra);
  const rows = entries.map(([title, id]) => node(harness, named ? "div" : "li", named ? { role: "group", "aria-label": title } : {},
    node(harness, "a", { href: `/song/${id}` }, title),
    node(harness, "button", { "aria-label": "More options", "aria-expanded": "false" })));
  if (region) body.append(node(harness, "main", { "aria-label": region }, ...rows));
  else for (const row of rows) body.append(row);
  harness.setBody(body);
  return body;
}

/** Observe the named buttons, drop the old page the way a re-render does, and build the new one. */
async function rerendered(harness, build, name = "More options") {
  const handles = (await findControls(harness, name)).filter((target) => target.name === name);
  detach(harness.document.body);
  build();
  return handles.map((target) => target.locator);
}

/** The lookup a stale locator carries. */
async function lookupOf(harness, locator) {
  const detail = await detailOf(harness, { kind: "activate", locator });
  assert.equal(detail.stale, "removed");
  return detail;
}

test("a re-rendered row button is the one same match when role, name, credential class, row label and row link agree", async () => {
  const harness = contentHarness();
  const rows = [["Alpha", 1], ["Beta", 2], ["Gamma", 3]];
  libraryRows(harness, rows, { named: false });
  const handles = await rerendered(harness, () => libraryRows(harness, rows, { named: false }));

  const lookup = await lookupOf(harness, handles[1]);

  assert.equal(lookup.matched, 3);
  assert.equal(lookup.same_count, 1);
  assert.equal(lookup.truncated, false);
  assert.deepEqual(lookup.candidates.map((candidate) => [candidate.same, candidate.target.context]), [
    [true, "Beta"], [false, "Alpha"], [false, "Gamma"]
  ]);
  const replacement = lookup.candidates[0].target;
  assert.equal(await harness.codeOf({ kind: "describe", locators: [replacement.locator] }), null);
  assert.equal(JSON.stringify(lookup).includes("/song/"), false, "no link leaves the page");
});

test("two same-title takes give same only to the take whose link matches", async () => {
  const harness = contentHarness();
  const rows = [["Neon Rain", 11], ["Neon Rain", 12]];
  libraryRows(harness, rows, { named: false });
  const handles = await rerendered(harness, () => libraryRows(harness, rows, { named: false }));

  const second = await lookupOf(harness, handles[1]);

  assert.equal(second.same_count, 1);
  assert.deepEqual(second.candidates.map((candidate) => [candidate.same, candidate.target.context, candidate.target.nth]), [
    [true, "Neon Rain", 2], [false, "Neon Rain", 1]
  ]);
  const first = await lookupOf(harness, handles[0]);
  assert.equal(first.candidates[0].same, true);
  assert.equal(first.candidates[0].target.nth, 1);
});

test("a deleted take leaves the other take not same", async () => {
  const harness = contentHarness();
  libraryRows(harness, [["Neon Rain", 11], ["Neon Rain", 12]]);
  const handles = await rerendered(harness, () => libraryRows(harness, [["Neon Rain", 11]]));

  const lookup = await lookupOf(harness, handles[1]);

  assert.equal(lookup.matched, 1);
  assert.equal(lookup.same_count, 0);
  assert.deepEqual(lookup.candidates.map((candidate) => [candidate.same, candidate.target.context]), [[false, "Neon Rain"]]);
});

test("a row link that also appears outside the row is not an anchor", async () => {
  const harness = contentHarness();
  const rows = [["Neon Rain", 11], ["Neon Rain", 12]];
  const nowPlaying = () => [node(harness, "a", { href: "/song/11" }, "Now playing")];
  libraryRows(harness, rows, { before: nowPlaying(), named: false });
  const handles = await rerendered(harness, () => libraryRows(harness, rows, { before: nowPlaying(), named: false }));

  assert.equal((await lookupOf(harness, handles[0])).same_count, 0, "the shared link proves nothing, and the title repeats");
  assert.equal((await lookupOf(harness, handles[1])).same_count, 1, "the other take still has a link of its own");
});

test("a link past the 2000-link cap gives no anchor", async () => {
  const harness = contentHarness();
  const rows = [["Neon Rain", 11], ["Neon Rain", 12]];
  const filler = () => Array.from({ length: 2001 }, (_, index) => node(harness, "a", { href: `/page/${index}` }, `Page ${index}`));
  libraryRows(harness, rows, { before: filler() });
  const handles = await rerendered(harness, () => libraryRows(harness, rows, { before: filler() }));

  assert.equal((await lookupOf(harness, handles[1])).same_count, 0);
});

test("without a link a row label counts only if it was unique at mint", async () => {
  const harness = contentHarness();
  const headed = (titles) => {
    const body = harness.element("body");
    for (const title of titles) {
      body.append(node(harness, "li", {}, node(harness, "h3", {}, title), node(harness, "button", { "aria-label": "More options" })));
    }
    harness.setBody(body);
  };

  headed(["Alpha", "Beta"]);
  let handles = await rerendered(harness, () => headed(["Alpha", "Beta"]));
  assert.equal((await lookupOf(harness, handles[1])).same_count, 1, "unique when minted and unique now");

  headed(["Alpha", "Beta"]);
  handles = await rerendered(harness, () => headed(["Alpha", "Alpha"]));
  assert.equal((await lookupOf(harness, handles[0])).same_count, 0, "unique when minted, repeated now");

  headed(["Alpha", "Alpha"]);
  handles = await rerendered(harness, () => headed(["Alpha", "Alpha"]));
  assert.equal((await lookupOf(harness, handles[0])).same_count, 0, "repeated when minted");

  const linked = () => {
    const body = harness.element("body");
    body.append(node(harness, "li", {}, node(harness, "a", { href: "/one" }, "Link one"), node(harness, "button", { "aria-label": "More options" })));
    body.append(node(harness, "li", {}, node(harness, "a", { href: "/two" }, "Link two"), node(harness, "button", { "aria-label": "More options" })));
    harness.setBody(body);
  };
  linked();
  handles = await rerendered(harness, linked);
  assert.equal((await lookupOf(harness, handles[0])).same_count, 1, "a row's own link is proof even when its name is link text");
});

test("a control that sat in no row is never the same control", async () => {
  const harness = contentHarness();
  const toolbar = (inRow) => {
    const body = harness.element("body");
    const save = node(harness, "button", { "aria-label": "Save" });
    body.append(inRow ? node(harness, "li", {}, node(harness, "h3", {}, "Row"), save) : save);
    harness.setBody(body);
  };

  toolbar(false);
  let handles = await rerendered(harness, () => toolbar(false), "Save");
  let lookup = await lookupOf(harness, handles[0]);
  assert.equal(lookup.matched, 1, "the node is still a candidate");
  assert.equal(lookup.same_count, 0, "a name alone is not proof");

  toolbar(false);
  handles = await rerendered(harness, () => toolbar(true), "Save");
  assert.equal((await lookupOf(harness, handles[0])).same_count, 0);
});

test("a deleted message leaves the other message's button not same", async () => {
  const harness = contentHarness();
  const messages = (texts) => {
    const body = harness.element("body");
    for (const text of texts) body.append(node(harness, "div", {}, node(harness, "span", {}, text), node(harness, "button", { "aria-label": "Delete" })));
    harness.setBody(body);
  };

  messages(["First", "Second"]);
  const handles = await rerendered(harness, () => messages(["Second"]), "Delete");
  const lookup = await lookupOf(harness, handles[0]);

  assert.equal(lookup.matched, 1);
  assert.equal(lookup.same_count, 0, "one control with the same name in no row is not the deleted one");
  assert.equal(lookup.candidates[0].same, false);
});

test("a changed view (URL, title or region) gives no same", async () => {
  const rows = [["Alpha", 1], ["Beta", 2]];
  const cases = {
    "the title": (harness) => { harness.document.title = "Playlists"; },
    "the address": (harness) => { harness.dom.context.location.href = "https://example.test/playlists"; },
    "the region": () => {}
  };
  for (const [what, change] of Object.entries(cases)) {
    const harness = contentHarness();
    libraryRows(harness, rows, { region: "Songs", named: false });
    const handles = await rerendered(harness, () => {
      change(harness);
      libraryRows(harness, rows, { region: what === "the region" ? "Playlists" : "Songs", named: false });
    });
    const lookup = await lookupOf(harness, handles[1]);
    assert.equal(lookup.matched, 2, what);
    assert.equal(lookup.same_count, 0, what);
  }

  const harness = contentHarness();
  libraryRows(harness, rows, { named: false });
  const handles = await rerendered(harness, () => {
    harness.dom.context.location.href = "https://example.test/#details";
    libraryRows(harness, rows, { named: false });
  });
  assert.equal((await lookupOf(harness, handles[1])).same_count, 1, "a fragment is not a different view");
});

test("the latest view the agent saw is the one a handle is held to", async () => {
  const harness = contentHarness();
  const rows = [["Alpha", 1], ["Beta", 2]];
  libraryRows(harness, rows, { named: false });
  await findControls(harness, "More options");
  harness.document.title = "Library, page two";
  const handles = await rerendered(harness, () => libraryRows(harness, rows, { named: false }));
  assert.equal((await lookupOf(harness, handles[1])).same_count, 1);
});

test("a credential field is never same with a plain one", async () => {
  const harness = contentHarness();
  const field = (type) => {
    const body = harness.element("body");
    body.append(node(harness, "li", {}, node(harness, "h3", {}, "Account"), node(harness, "input", { type, "aria-label": "Code" })));
    harness.setBody(body);
  };

  field("password");
  let handles = await rerendered(harness, () => field("text"), "Code");
  let lookup = await lookupOf(harness, handles[0]);
  assert.equal(lookup.matched, 0, "a plain field is not a candidate for a credential field");

  field("text");
  handles = await rerendered(harness, () => field("password"), "Code");
  lookup = await lookupOf(harness, handles[0]);
  assert.equal(lookup.matched, 0, "nor the other way round");

  field("password");
  handles = await rerendered(harness, () => field("password"), "Code");
  lookup = await lookupOf(harness, handles[0]);
  assert.equal(lookup.matched, 1);
  assert.equal(lookup.same_count, 0, "a credential field is named but never trusted");
  assert.equal("context" in lookup.candidates[0].target, false);
});

test("a recycled connected row whose link changed refuses as removed", async () => {
  const harness = contentHarness();
  const body = libraryRows(harness, [["Alpha", 1], ["Beta", 2]]);
  const [alpha, beta] = (await findControls(harness, "More options")).map((target) => target.locator);

  body.children[0].children[0].setAttribute("href", "/song/99");

  assert.equal(await staleWordOf(harness, { kind: "activate", locator: alpha }), "removed");
  assert.equal(await staleWordOf(harness, { kind: "describe", locators: [alpha] }), "removed", "and it stays removed");
  assert.equal(await harness.codeOf({ kind: "describe", locators: [beta] }), null, "the other row is untouched");

  body.children[1].setAttribute("aria-label", "Renamed");
  assert.equal(await staleWordOf(harness, { kind: "describe", locators: [beta] }), "removed", "a changed row name recycles it too");
});

test("a recycled row refuses its old handle even after another read sees the node", async () => {
  const harness = contentHarness();
  const body = libraryRows(harness, [["Alpha", 1], ["Beta", 2]]);
  const [alpha] = (await findControls(harness, "More options")).map((target) => target.locator);

  const row = body.children[0];
  row.setAttribute("aria-label", "Gamma");
  row.children[0].setAttribute("href", "/song/3");
  const reread = (await findControls(harness, "More options")).map((target) => target.locator);

  assert.equal(reread.includes(alpha), false, "the recycled node gets a new handle");
  assert.equal(await staleWordOf(harness, { kind: "describe", locators: [alpha] }), "removed");
  assert.equal(await harness.codeOf({ kind: "describe", locators: [reread[0]] }), null, "the new handle works");
});

test("a row without a recorded link or declared name never reads as recycled", async () => {
  const harness = contentHarness();
  const body = harness.element("body");
  body.append(node(harness, "li", {}, node(harness, "span", {}, "text"), node(harness, "button", { "aria-label": "Play" })));
  harness.setBody(body);
  const [play] = (await findControls(harness, "Play")).map((target) => target.locator);
  body.children[0].append(node(harness, "a", { href: "/elsewhere" }, "Elsewhere"));
  assert.equal(await harness.codeOf({ kind: "describe", locators: [play] }), null);
});

test("candidates are at most 3, same first, and carry context", async () => {
  const harness = contentHarness();
  const rows = [["One", 1], ["Two", 2], ["Three", 3], ["Four", 4], ["Five", 5]];
  libraryRows(harness, rows, { named: false });
  const handles = await rerendered(harness, () => libraryRows(harness, rows, { named: false }));

  const lookup = await lookupOf(harness, handles[3]);

  assert.equal(lookup.matched, 5);
  assert.equal(lookup.same_count, 1);
  assert.equal(lookup.candidates.length, 3);
  assert.deepEqual(lookup.candidates.map((candidate) => [candidate.same, candidate.target.context]), [
    [true, "Four"], [false, "One"], [false, "Two"]
  ]);
  const locators = new Set(lookup.candidates.map((candidate) => candidate.target.locator));
  assert.equal(locators.size, 3);
  for (const candidate of lookup.candidates) {
    assert.equal(candidate.target.role, "button");
    assert.equal(candidate.target.name, "More options");
    assert.deepEqual(Object.keys(candidate).sort(), ["same", "target"]);
  }
});

test("the lookup counts at most 50 matches and says it was cut", async () => {
  const harness = contentHarness();
  const many = () => {
    const body = harness.element("body");
    for (let index = 0; index < 60; index += 1) body.append(node(harness, "button", { "aria-label": "More options" }));
    harness.setBody(body);
  };
  many();
  const handles = await rerendered(harness, many);
  const lookup = await lookupOf(harness, handles[0]);
  assert.equal(lookup.matched, 50);
  assert.equal(lookup.truncated, true);
});

test("the stale detail is computed without painting", async () => {
  const harness = contentHarness();
  const rows = [["Alpha", 1], ["Beta", 2]];
  libraryRows(harness, rows, { named: false });
  const handles = await rerendered(harness, () => libraryRows(harness, rows, { named: false }));
  let created = 0;
  const counting = (make) => (...args) => {
    created += 1;
    return make(...args);
  };
  harness.document.createElement = counting(harness.document.createElement);
  harness.document.createElementNS = counting(harness.document.createElementNS);
  const timers = harness.dom.pendingTimers();

  await lookupOf(harness, handles[0]);

  assert.equal(created, 0, "no element is made");
  assert.equal(harness.dom.pendingTimers(), timers, "nothing is scheduled");
  assert.deepEqual(harness.scrolls, []);
});

test("a vanished element in the middle of an action carries the same lookup", async () => {
  const harness = contentHarness();
  const rows = [["Alpha", 1], ["Beta", 2]];
  const body = libraryRows(harness, rows);
  const [alpha] = (await findControls(harness, "More options")).map((target) => target.locator);
  const button = body.children[0].children[1];
  button.scrollIntoView = () => {
    detach(button);
    libraryRows(harness, rows);
  };
  const detail = await detailOf(harness, { kind: "activate", locator: alpha });
  assert.equal(detail.stale, "removed");
  assert.equal(detail.same_count, 1);
  assert.equal(detail.candidates[0].target.context, "Alpha");
});

test("an unknown locator carries no lookup", async () => {
  const harness = contentHarness();
  libraryRows(harness, [["Alpha", 1]]);
  assert.deepEqual(await detailOf(harness, { kind: "describe", locators: ["locator_1"] }), { stale: "unknown" });
});

// ---- a handle follows its control through a re-render the page proves ---------------------------

/** The Suno library: nine titles twice, 18 rows, each with its own /song/<uuid> link. */
function sunoEntries() {
  return Array.from({ length: 18 }, (_, index) => [`Title ${Math.floor(index / 2) + 1}`, `uuid-${index}`]);
}

/** Build the library the way React does: every button carries an id that changes per render. */
function sunoLibrary(harness, entries, { render = 0, ...options } = {}) {
  const body = libraryRows(harness, entries, options);
  let index = 0;
  const label = (element) => {
    if (element.getAttribute?.("aria-label") === "More options") {
      element.id = `:r${render}${index.toString(36)}:`;
      index += 1;
    }
    (element.childNodes ?? []).forEach(label);
  };
  label(body);
  return body;
}

/** The reply to an activation, which must have succeeded. */
async function activated(harness, locator) {
  const reply = await harness.send({ kind: "activate", locator });
  assert.equal(reply.ok, true, JSON.stringify(reply));
  return reply;
}

/** The handles of every "More options" button on the page. */
async function optionHandles(harness) {
  return (await findControls(harness, "More options")).filter((target) => target.name === "More options").map((target) => target.locator);
}

test("a re-rendered row control keeps its locator when its row link is unique", async () => {
  const harness = contentHarness();
  sunoLibrary(harness, sunoEntries(), { render: 1 });
  const before = await optionHandles(harness);
  assert.equal(before.length, 18);
  detach(harness.document.body);
  sunoLibrary(harness, sunoEntries(), { render: 2 });

  for (const locator of before) {
    const reply = await activated(harness, locator);
    assert.deepEqual(reply.rebound, [locator]);
    assert.equal(reply.value.subject.rebound, true);
    assert.deepEqual(Object.keys(reply.value.subject).sort(), ["name", "rebound", "role"]);
  }
  const again = await activated(harness, before[0]);
  assert.equal("rebound" in again, false, "the control is live again, so the next use follows nothing");
  assert.equal("rebound" in again.value.subject, false);
});

test("find after a re-render returns the same locators", async () => {
  const harness = contentHarness();
  sunoLibrary(harness, sunoEntries(), { render: 1 });
  const before = await optionHandles(harness);
  detach(harness.document.body);
  sunoLibrary(harness, sunoEntries(), { render: 2 });

  const after = await optionHandles(harness);

  assert.deepEqual(after, before);
  assert.equal("rebound" in await activated(harness, before[5]), false, "the read already pointed it at the new node");
});

test("two same-title takes each follow their own link", async () => {
  const harness = contentHarness();
  libraryRows(harness, [["Neon Rain", 11], ["Neon Rain", 12]]);
  const [first, second] = await optionHandles(harness);
  detach(harness.document.body);
  const body = libraryRows(harness, [["Neon Rain", 12], ["Neon Rain", 11]]);
  const reached = [];
  body.children.forEach((row, position) => { row.children[1].scrollIntoView = () => reached.push(position); });

  await activated(harness, first);
  await activated(harness, second);

  assert.deepEqual(reached, [1, 0], "each went to the row of its own link, not to its place in the list");
});

test("a deleted take does not follow onto the other take", async () => {
  const harness = contentHarness();
  libraryRows(harness, [["Neon Rain", 11], ["Neon Rain", 12]]);
  const [first, second] = await optionHandles(harness);
  detach(harness.document.body);
  libraryRows(harness, [["Neon Rain", 12]]);

  assert.deepEqual((await activated(harness, second)).rebound, [second], "the take that stayed follows");
  const detail = await detailOf(harness, { kind: "activate", locator: first });

  assert.equal(detail.stale, "removed");
  assert.equal(detail.same_count, 0);
  assert.deepEqual(detail.candidates.map((candidate) => [candidate.same, candidate.target.locator]), [[false, second]]);
});

test("a row link that also appears outside the row is not followed", async () => {
  const harness = contentHarness();
  const playing = () => [node(harness, "a", { href: "/song/11" }, "Now playing")];
  libraryRows(harness, [["Neon Rain", 11], ["Paper Moon", 12]], { before: playing() });
  const [shared, own] = await optionHandles(harness);
  detach(harness.document.body);
  libraryRows(harness, [["Neon Rain", 11], ["Paper Moon", 12]], { before: playing() });

  assert.deepEqual((await activated(harness, own)).rebound, [own]);
  assert.equal(await staleWordOf(harness, { kind: "activate", locator: shared }), "removed");
});

test("a changed URL, title or region is not followed", async () => {
  const rows = [["Alpha", 1], ["Beta", 2]];
  const changes = {
    "the title": [(harness) => { harness.document.title = "Playlists"; }, "Songs"],
    "the address": [(harness) => { harness.dom.context.location.href = "https://example.test/playlists"; }, "Songs"],
    "the region": [() => {}, "Playlists"]
  };
  for (const [what, [change, region]] of Object.entries(changes)) {
    const harness = contentHarness();
    libraryRows(harness, rows, { region: "Songs" });
    const [locator] = await optionHandles(harness);
    detach(harness.document.body);
    change(harness);
    libraryRows(harness, rows, { region });
    assert.equal(await staleWordOf(harness, { kind: "activate", locator }), "removed", what);
  }

  const harness = contentHarness();
  libraryRows(harness, rows, { region: "Songs" });
  const [locator] = await optionHandles(harness);
  detach(harness.document.body);
  harness.dom.context.location.href = "https://example.test/#details";
  libraryRows(harness, rows, { region: "Songs" });
  assert.deepEqual((await activated(harness, locator)).rebound, [locator], "a fragment is not another view");
});

test("a changed row name, control name, role or in-row position is not followed", async () => {
  const buildRow = (harness, { title = "Alpha", control = { "aria-label": "More options" }, extra = 0 } = {}) => {
    const body = harness.element("body");
    body.append(node(harness, "div", { role: "group", "aria-label": title },
      node(harness, "a", { href: "/song/1" }, "Alpha"),
      node(harness, "button", control),
      ...Array.from({ length: extra }, () => node(harness, "button", control))));
    harness.setBody(body);
  };
  const changes = {
    "the row name": { title: "Renamed" },
    "the control name": { control: { "aria-label": "Options" } },
    "the control role": { control: { "aria-label": "More options", role: "menuitem" } },
    "the position in the row": { extra: 1 }
  };
  for (const [what, change] of Object.entries(changes)) {
    const harness = contentHarness();
    buildRow(harness);
    const [locator] = await optionHandles(harness);
    detach(harness.document.body);
    buildRow(harness, change);
    assert.equal(await staleWordOf(harness, { kind: "activate", locator }), "removed", what);
  }
});

test("a row with no declared name is not followed, even with a unique link", async () => {
  const harness = contentHarness();
  libraryRows(harness, [["Alpha", 1], ["Beta", 2]], { named: false });
  const [locator] = await optionHandles(harness);
  detach(harness.document.body);
  libraryRows(harness, [["Alpha", 1], ["Beta", 2]], { named: false });

  assert.equal(await staleWordOf(harness, { kind: "activate", locator }), "removed");
});

test("an ordinal alone, a page id or a data attribute never follows", async () => {
  const harness = contentHarness();
  const rows = () => {
    const body = harness.element("body");
    for (const title of ["Alpha", "Beta"]) {
      body.append(node(harness, "div", { role: "group", "aria-label": title },
        node(harness, "button", { "aria-label": "More options", id: `menu-${title}`, "data-testid": `menu-${title}` })));
    }
    harness.setBody(body);
  };
  rows();
  const handles = await optionHandles(harness);
  detach(harness.document.body);
  rows();

  for (const locator of handles) assert.equal(await staleWordOf(harness, { kind: "activate", locator }), "removed");
});

test("a credential-class control is never followed or re-adopted", async () => {
  const harness = contentHarness();
  const row = () => {
    const body = harness.element("body");
    body.append(node(harness, "div", { role: "group", "aria-label": "Account" },
      node(harness, "a", { href: "/account/1" }, "Account"),
      node(harness, "input", { type: "password", "aria-label": "Code" })));
    harness.setBody(body);
  };
  const codes = async () => (await findControls(harness, "Code")).filter((target) => target.name === "Code").map((target) => target.locator);
  row();
  const [old] = await codes();
  detach(harness.document.body);
  row();

  const [reread] = await codes();

  assert.notEqual(reread, old, "the new field gets a new locator");
  assert.equal(await staleWordOf(harness, { kind: "describe", locators: [old] }), "removed");
});

test("a control in a dialog is never followed", async () => {
  const harness = contentHarness();
  const dialog = () => {
    const body = harness.element("body");
    body.append(node(harness, "div", { role: "dialog", "aria-label": "Share" },
      node(harness, "div", { role: "group", "aria-label": "Alpha" },
        node(harness, "a", { href: "/song/1" }, "Alpha"),
        node(harness, "button", { "aria-label": "More options" }))));
    harness.setBody(body);
  };
  dialog();
  const [locator] = await optionHandles(harness);
  detach(harness.document.body);
  dialog();

  assert.equal(await staleWordOf(harness, { kind: "activate", locator }), "removed");
});

test("a control in an open shadow root follows through its host row", async () => {
  const harness = contentHarness();
  const shadows = [];
  const row = () => {
    const body = harness.element("body");
    const host = harness.element("open-menu");
    const shadow = harness.fragment();
    shadow.host = host;
    shadow.append(node(harness, "button", { "aria-label": "More options" }));
    host.shadowRoot = shadow;
    shadows.push(shadow);
    body.append(node(harness, "div", { role: "group", "aria-label": "Alpha" }, node(harness, "a", { href: "/song/1" }, "Alpha"), host));
    harness.setBody(body);
  };
  row();
  const [locator] = await optionHandles(harness);
  detach(harness.document.body);
  shadows.forEach(detach);
  row();

  assert.deepEqual((await activated(harness, locator)).rebound, [locator]);
});

test("a node that vanishes during an action's wait refuses as removed", async () => {
  const harness = contentHarness();
  const rows = [["Alpha", 1], ["Beta", 2]];
  const body = libraryRows(harness, rows);
  const [alpha] = await optionHandles(harness);
  const button = body.children[0].children[1];
  button.scrollIntoView = () => {
    detach(button);
    libraryRows(harness, rows);
  };

  const reply = await harness.send({ kind: "activate", locator: alpha });

  assert.equal(reply.ok, false);
  assert.equal(reply.error.detail.stale, "removed");
  assert.equal("rebound" in reply, false);
  assert.equal(reply.error.detail.candidates[0].target.locator, alpha, "the lookup names the control by its own handle");
});

test("a new node already holding a locator is not taken over", async () => {
  const harness = contentHarness();
  const rows = [["Alpha", 1], ["Beta", 2]];
  libraryRows(harness, rows);
  const [old] = await optionHandles(harness);
  detach(harness.document.body);
  harness.document.title = "Playlists";
  libraryRows(harness, rows);
  const [other] = await optionHandles(harness);
  harness.document.title = "Fixture";

  assert.notEqual(other, old, "in another view the node is a new control");
  assert.equal(await staleWordOf(harness, { kind: "activate", locator: old }), "removed");
  await activated(harness, other);
});

test("rebound appears in the envelope only when non-empty", async () => {
  const harness = contentHarness();
  libraryRows(harness, [["Alpha", 1]]);
  const [locator] = await optionHandles(harness);

  assert.equal("rebound" in await harness.send({ kind: "describe", locators: [locator] }), false);
  detach(harness.document.body);
  libraryRows(harness, [["Alpha", 1]]);
  const followed = await harness.send({ kind: "describe", locators: [locator] });
  assert.deepEqual(followed.rebound, [locator]);
  assert.equal("rebound" in await harness.send({ kind: "describe", locators: [locator] }), false);
  assert.equal("rebound" in await harness.send({ kind: "describe", locators: ["locator_1"] }), false, "a refusal carries none");
});

test("the presentation anchor neither follows nor adopts, so the action still reports rebound", async () => {
  const harness = contentHarness();
  const local = { preferences: { effects: true, captions: false } };
  sunoLibrary(harness, sunoEntries(), { render: 1 });
  const before = await optionHandles(harness);
  detach(harness.document.body);
  sunoLibrary(harness, sunoEntries(), { render: 2 });

  const drawn = harness.runtime.present({ signal: "target", activity: "click", locator: before[3], invocation: "i-1" }, local);
  const reply = await activated(harness, before[3]);

  assert.equal(drawn, false, "a detached node draws nothing");
  assert.deepEqual(reply.rebound, [before[3]]);
  assert.equal(reply.value.subject.rebound, true);
});

test("the call that follows a handle reports it and the measurement after it finds the control live", async () => {
  const harness = contentHarness();
  sunoLibrary(harness, sunoEntries(), { render: 1 });
  const before = await optionHandles(harness);
  detach(harness.document.body);
  sunoLibrary(harness, sunoEntries(), { render: 2 });

  const hovered = await harness.send({ kind: "hover", locator: before[3] });
  assert.deepEqual(hovered.rebound, [before[3]]);
  assert.equal(hovered.value.subject.rebound, true);
  const boxed = await harness.send({ kind: "box", locator: before[3] });
  assert.equal(boxed.ok, true, JSON.stringify(boxed));
  assert.equal("rebound" in boxed, false, "a later call finds the control live and follows nothing");
  assert.equal("rebound" in boxed.value.subject, false);
});

test("a fill step after the first call is never followed", async () => {
  const steps = (locator) => [
    { kind: "fill_local", field: { locator, value: "x" }, allow_credentials: false },
    { kind: "focus_text_field", field: { locator, value: "x" }, allow_credentials: false },
    { kind: "submit_fill", fields: [{ locator, value: "x" }], submit_locator: locator, allow_credentials: false },
  ];
  for (let step = 0; step < steps("").length; step += 1) {
    const harness = contentHarness();
    libraryRows(harness, [["Alpha", 1]]);
    const [locator] = await optionHandles(harness);
    detach(harness.document.body);
    libraryRows(harness, [["Alpha", 1]]);

    const refused = await harness.send(steps(locator)[step]);

    assert.equal(refused.error.detail.stale, "removed", steps(locator)[step].kind);
    assert.equal("rebound" in refused, false);
  }
});
