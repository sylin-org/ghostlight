// The shared Controls sheet, exercised for real.
//
// The kit draws the service's controls part into the sheet's own markup and calls back into the
// page when a button is pressed. These tests run the real kit module against the real sheet markup
// from index.html, parsed into a minimal fake document, with fake callbacks and a fake
// localStorage. No page, no transport and no Ghostlight process is involved.
//
// Run with: node --test tests/workbench-controls.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { join, resolve } from "node:path";

const require = createRequire(import.meta.url);
const ui = join(resolve(import.meta.dirname, ".."), "crates", "orchestrator", "ui");
const Controls = require(join(ui, "kit", "controls.js"));
const Words = require(join(ui, "lib", "words.js"));
const markup = readFileSync(join(ui, "index.html"), "utf8");

const WHY = "Change this in the Ghostlight window.";

/** The part as the service shapes it: the window may do every act, a watcher may only pause. */
function part({ context = "control", watch = true, runtime = "active", layer = "marker", url = "http://127.0.0.1:41234/" } = {}) {
  const control = context === "control";
  const act = (watcherMay = false) => (control || watcherMay
    ? { available: true, why_not: null } : { available: false, why_not: WHY });
  const on = layer !== "off";
  return {
    runtime: {
      label: "Browser work", state: runtime, word: { active: "Running", held: "Paused", ended: "Session ended" }[runtime],
      note: runtime === "held" ? "Paused from the tray." : null,
      acts: { hold: act(true), resume: act(), start_session: act(), end_session: act() }
    },
    viewing: {
      label: "Others can watch", watch, url, word: watch ? "On -- anyone on this computer can open this view" : "Off -- only this window shows Ghostlight",
      acts: { set_watching: act() }
    },
    diagnostics: {
      label: "Process diagnostics", on, layer, used_bytes: 2048, folder: "~/.ghostlight/logs",
      word: on ? `On (${layer})` : "Off", acts: { set_diagnostics: act(), reveal_diagnostics: act() }
    },
    notifications: { label: "Notifications", word: "Send one local checkup notification.", acts: { test_notification: act() } },
    service: { word: "Ghostlight service is up; 1 MCP client connected." },
    you: { context, word: control ? "Full control" : "Watching", sentence: "Someone can see this." }
  };
}

/** A fake element that knows its place in the tree, so `closest` answers like a browser's. */
class Element {
  constructor(tag, attributes = {}) {
    this.tag = tag;
    this.id = attributes.id ?? "";
    this.hidden = "hidden" in attributes;
    this.disabled = false;
    this.textContent = "";
    this.parent = null;
    this.children = [];
    this.attributes = {};
    this.dataset = {};
    this.classes = new Set(String(attributes.class ?? "").split(/\s+/).filter(Boolean));
    for (const [name, value] of Object.entries(attributes)) {
      if (!name.startsWith("data-")) continue;
      this.dataset[name.slice(5).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = value;
    }
    this.classList = {
      toggle: (name, on) => {
        if (on === undefined ? !this.classes.has(name) : on) this.classes.add(name);
        else this.classes.delete(name);
      },
      contains: (name) => this.classes.has(name)
    };
  }

  setAttribute(name, value) {
    this.attributes[name] = value;
  }

  focus() {
    Element.focused = this;
  }

  matches(selector) {
    if (selector.startsWith("#")) return this.id === selector.slice(1);
    const attribute = /^\[data-([a-z-]+)\]$/.exec(selector);
    if (attribute) return attribute[1].replace(/-([a-z])/g, (_, letter) => letter.toUpperCase()) in this.dataset;
    throw new Error(`the fake document does not know the selector ${selector}`);
  }

  closest(selector) {
    for (let node = this; node; node = node.parent) if (node.matches(selector)) return node;
    return null;
  }

  *descendants() {
    for (const child of this.children) {
      yield child;
      yield* child.descendants();
    }
  }
}

/** Parse the sheet and the band's button out of the real markup into fake elements. */
function parseSheet() {
  const start = markup.indexOf('<section class="controls-sheet"');
  const after = /<\/section>\s*<div class="palette-backdrop"/.exec(markup);
  assert.ok(start > 0 && after && after.index > start, "the sheet's markup moved");
  const end = after.index;
  const open = /<button class="controls-open"[\s\S]*?<\/button>/.exec(markup);
  assert.ok(open, "the band's Controls button moved");
  const root = new Element("root");
  const stack = [root];
  for (const source of [open[0], markup.slice(start, end + "</section>".length)]) {
    for (const [, closing, tag, rest] of source.matchAll(/<(\/?)([a-z0-9]+)([^>]*)>/g)) {
      if (closing) {
        stack.pop();
        continue;
      }
      const attributes = {};
      for (const [, name, value] of rest.matchAll(/([a-z-]+)(?:="([^"]*)")?/g)) attributes[name] = value ?? "";
      const element = new Element(tag, attributes);
      element.parent = stack.at(-1);
      stack.at(-1).children.push(element);
      stack.push(element);
    }
  }
  return root;
}

/** A page: the sheet's elements, a body, and the listeners the kit attaches to the document. */
function page() {
  const root = parseSheet();
  const byId = new Map([...root.descendants()].filter((node) => node.id).map((node) => [node.id, node]));
  const listeners = new Map();
  const body = new Element("body");
  const document = {
    body,
    getElementById: (id) => byId.get(id) ?? null,
    querySelectorAll: (selector) => [...root.descendants()].filter((node) => node.matches(selector)),
    addEventListener: (type, handler) => {
      if (!listeners.has(type)) listeners.set(type, []);
      listeners.get(type).push(handler);
    }
  };
  const dispatch = (type, event) => {
    for (const handler of listeners.get(type) ?? []) handler(event);
  };
  const el = (id) => {
    const found = byId.get(id);
    assert.ok(found, `the markup has no #${id}`);
    return found;
  };
  return {
    document, body, el,
    click: (target) => dispatch("click", { target }),
    key: (key) => dispatch("keydown", { key }),
    outside: () => dispatch("click", { target: new Element("p") })
  };
}

/** Callbacks that record what they were asked, and a kit over them. */
function kit(options = {}) {
  const shown = page();
  const calls = [];
  const failures = [];
  const record = (name) => async (...args) => {
    calls.push([name, ...args]);
  };
  const controls = Controls.create({
    document: shown.document,
    words: { ...Words.CONTROLS, byteSize: Words.byteSize },
    applyIntent: record("applyIntent"),
    setWatching: record("setWatching"),
    setDiagnostics: record("setDiagnostics"),
    revealDiagnostics: record("revealDiagnostics"),
    testNotification: record("testNotification"),
    continueInWindow: record("continueInWindow"),
    copyText: record("copyText"),
    onFailure: (what, error) => failures.push([what, String(error)]),
    ...options
  });
  controls.wire();
  return { ...shown, controls, calls, failures };
}

/** The ids of the sheet's action buttons that are showing. */
function showing(shown) {
  return [
    "controls-hold", "controls-resume", "controls-start", "controls-end", "controls-viewing-copy",
    "controls-viewing-toggle", "controls-diagnostics-reveal", "controls-diagnostics-toggle", "test-notification"
  ].filter((id) => !shown.el(id).hidden);
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

test("the button is hidden and the sheet cannot open until the service has said what it holds", () => {
  const shown = kit();
  shown.controls.open();
  assert.equal(shown.el("controls-sheet").hidden, true);
  shown.controls.render(part());
  assert.equal(shown.el("controls-open").hidden, false);
  shown.controls.render(null);
  assert.equal(shown.el("controls-open").hidden, true);
});

test("the sheet opens and closes by the button, by Escape and by a click outside", () => {
  const shown = kit();
  shown.controls.render(part());
  const sheet = shown.el("controls-sheet");
  const button = shown.el("controls-open");
  shown.click(button);
  assert.equal(sheet.hidden, false);
  assert.equal(button.attributes["aria-expanded"], "true");
  assert.equal(Element.focused, sheet);
  shown.click(button);
  assert.equal(sheet.hidden, true);
  assert.equal(button.attributes["aria-expanded"], "false");

  shown.click(button);
  shown.key("Enter");
  assert.equal(sheet.hidden, false);
  shown.key("Escape");
  assert.equal(sheet.hidden, true);
  assert.equal(Element.focused, button);

  shown.click(button);
  shown.click(shown.el("controls-service"));
  assert.equal(sheet.hidden, false, "a click inside the sheet leaves it open");
  shown.outside();
  assert.equal(sheet.hidden, true);
});

test("a screen with nothing to show takes the button and an open sheet away", () => {
  const shown = kit();
  shown.controls.render(part());
  shown.click(shown.el("controls-open"));
  shown.controls.hide();
  assert.equal(shown.el("controls-sheet").hidden, true);
  assert.equal(shown.el("controls-open").hidden, true);
});

test("the sheet says what the part says, in the service's words", () => {
  const shown = kit();
  shown.controls.render(part({ runtime: "held" }));
  const said = (id) => shown.el(id).textContent;
  assert.equal(said("controls-service"), "Ghostlight service is up; 1 MCP client connected.");
  assert.equal(said("controls-runtime-label"), "Browser work");
  assert.equal(said("controls-runtime-word"), "Paused");
  assert.equal(said("controls-runtime-note"), "Paused from the tray.");
  assert.equal(said("controls-viewing-label"), "Others can watch");
  assert.equal(said("controls-viewing-word"), "On -- anyone on this computer can open this view");
  assert.equal(said("controls-viewing-url"), "http://127.0.0.1:41234/");
  assert.equal(said("controls-diagnostics-word"), "On (marker)");
  assert.equal(said("controls-diagnostics-place"), "~/.ghostlight/logs (2.0 KB)");
  assert.equal(said("controls-notifications-label"), "Notifications");
});

test("the page's own labels fill the headings and the buttons", () => {
  const shown = kit();
  assert.equal(shown.el("controls-open").textContent, "Controls");
  assert.equal(shown.el("controls-hold").textContent, "Pause");
  assert.equal(shown.el("controls-end").textContent, "End session");
  assert.equal(shown.el("test-notification").textContent, "Send test");
  assert.equal(shown.el("controls-diagnostics-reveal").textContent, "Open log folder");
  assert.equal(shown.el("controls-continue-button").textContent, "Continue in the Ghostlight window");
});

test("in Control every row has its button, and pressing Turn off calls setWatching(false)", async () => {
  const shown = kit();
  shown.controls.render(part());
  assert.deepEqual(showing(shown), [
    "controls-hold", "controls-end", "controls-viewing-copy", "controls-viewing-toggle",
    "controls-diagnostics-reveal", "controls-diagnostics-toggle", "test-notification"
  ]);
  assert.equal(shown.el("controls-continue").hidden, true);
  assert.equal(shown.el("controls-viewing-toggle").textContent, "Turn off");
  assert.equal(shown.el("controls-diagnostics-toggle").textContent, "Turn off");

  shown.click(shown.el("controls-viewing-toggle"));
  shown.click(shown.el("controls-diagnostics-toggle"));
  shown.click(shown.el("controls-diagnostics-reveal"));
  shown.click(shown.el("test-notification"));
  shown.click(shown.el("controls-viewing-copy"));
  shown.click(shown.el("controls-hold"));
  shown.click(shown.el("controls-end"));
  await settle();
  assert.deepEqual(shown.calls, [
    ["setWatching", false],
    ["setDiagnostics", false],
    ["revealDiagnostics"],
    ["testNotification"],
    ["copyText", "http://127.0.0.1:41234/"],
    ["applyIntent", "hold"],
    ["applyIntent", "end_session"]
  ]);
});

const RUNTIME_BUTTONS = ["controls-hold", "controls-resume", "controls-start", "controls-end"];

/** The runtime row's buttons that are showing. */
const runtimeShowing = (shown) => showing(shown).filter((id) => RUNTIME_BUTTONS.includes(id));

test("each runtime state offers the acts that make sense in it", () => {
  const shown = kit();
  shown.controls.render(part({ runtime: "active" }));
  assert.deepEqual(runtimeShowing(shown), ["controls-hold", "controls-end"]);
  shown.controls.render(part({ runtime: "held" }));
  assert.deepEqual(runtimeShowing(shown), ["controls-resume", "controls-end"]);
  shown.controls.render(part({ runtime: "ended" }));
  assert.deepEqual(runtimeShowing(shown), ["controls-start"]);
});

test("a switch that is off offers Turn on", async () => {
  const shown = kit();
  shown.controls.render(part({ watch: false, layer: "off" }));
  assert.equal(shown.el("controls-viewing-toggle").textContent, "Turn on");
  assert.equal(shown.el("controls-diagnostics-toggle").textContent, "Turn on");
  assert.equal(shown.el("controls-diagnostics-word").textContent, "Off");
  shown.click(shown.el("controls-viewing-toggle"));
  shown.click(shown.el("controls-diagnostics-toggle"));
  await settle();
  assert.deepEqual(shown.calls, [["setWatching", true], ["setDiagnostics", true]]);
});

test("in Watch only Pause is a button while active, and the group has one Continue line", async () => {
  const shown = kit();
  shown.controls.render(part({ context: "watch" }));
  assert.deepEqual(showing(shown), ["controls-hold"]);
  assert.equal(shown.el("controls-continue").hidden, false);
  assert.equal(shown.el("controls-continue-why").textContent, WHY);
  assert.equal(shown.document.querySelectorAll("[data-controls-continue]").length, 1);
  // The rest is state: the words are still there, and nothing can be pressed to change them.
  assert.equal(shown.el("controls-viewing-word").textContent, "On -- anyone on this computer can open this view");
  assert.equal(shown.el("controls-diagnostics-word").textContent, "On (marker)");

  shown.click(shown.el("controls-continue-button"));
  await settle();
  assert.deepEqual(shown.calls, [["continueInWindow"]]);
});

test("while held, a watching screen shows the state and the Continue line, and no button for the runtime", () => {
  const shown = kit();
  shown.controls.render(part({ context: "watch", runtime: "held" }));
  assert.deepEqual(runtimeShowing(shown), []);
  assert.equal(shown.el("controls-runtime-word").textContent, "Paused");
  assert.equal(shown.el("controls-continue").hidden, false);
});

test("no copy button is drawn on a screen that cannot copy", () => {
  const shown = kit({ copyText: undefined });
  shown.controls.render(part());
  assert.equal(shown.el("controls-viewing-copy").hidden, true);
  assert.equal(shown.el("controls-viewing-url").hidden, false);
});

test("a button cannot be pressed twice while its act is still running", async () => {
  let release;
  const shown = kit({ setWatching: (watch) => new Promise((resolve) => { release = () => resolve(watch); }) });
  shown.controls.render(part());
  const toggle = shown.el("controls-viewing-toggle");
  shown.click(toggle);
  assert.equal(toggle.disabled, true);
  shown.click(toggle);
  release();
  await settle();
  assert.equal(toggle.disabled, false);
});

test("an act that fails is reported, and its button is free again", async () => {
  const shown = kit({ testNotification: async () => { throw new Error("no way to notify"); } });
  shown.controls.render(part());
  const send = shown.el("test-notification");
  shown.click(send);
  await settle();
  assert.deepEqual(shown.failures, [["using the controls", "Error: no way to notify"]]);
  assert.equal(send.disabled, false);
});

/** A localStorage the test can watch, break and refill. */
function memory(initial = {}) {
  const kept = { ...initial };
  return {
    kept,
    getItem: (key) => (key in kept ? kept[key] : null),
    setItem: (key, value) => { kept[key] = String(value); }
  };
}

const chosen = (shown, name) => shown.document.querySelectorAll("[data-personal]")
  .filter((button) => button.dataset.personal === name && button.attributes["aria-pressed"] === "true")
  .map((button) => button.dataset.value);

test("density and motion are kept in the browser and dress the page again after a reload", () => {
  const storage = memory();
  const first = kit({ storage });
  assert.deepEqual(first.controls.personal(), { density: "comfortable", motion: "system" });
  assert.deepEqual(chosen(first, "density"), ["comfortable"]);
  assert.equal(first.body.classes.has("density-compact"), false);

  const compact = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "density" && button.dataset.value === "compact");
  const reduced = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "motion" && button.dataset.value === "reduced");
  first.controls.render(part());
  first.click(first.el("controls-open"));
  first.click(compact);
  first.click(reduced);
  assert.equal(first.body.classes.has("density-compact"), true);
  assert.equal(first.body.classes.has("motion-reduced"), true);
  assert.deepEqual(chosen(first, "density"), ["compact"]);
  assert.equal(first.el("controls-sheet").hidden, false, "a personal setting leaves the sheet open");
  assert.deepEqual(JSON.parse(storage.kept[Controls.STORAGE_KEY]), { density: "compact", motion: "reduced" });
  assert.deepEqual(first.calls, [], "a personal setting is never sent anywhere");

  const reloaded = kit({ storage });
  assert.deepEqual(reloaded.controls.personal(), { density: "compact", motion: "reduced" });
  assert.equal(reloaded.body.classes.has("density-compact"), true);
  assert.equal(reloaded.body.classes.has("motion-reduced"), true);

  const comfortable = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "density" && button.dataset.value === "comfortable");
  first.click(comfortable);
  assert.equal(first.body.classes.has("density-compact"), false);
});

test("a browser that keeps nothing, or keeps rubbish, still draws the sheet with the defaults", () => {
  const broken = {
    getItem() { throw new Error("storage is blocked"); },
    setItem() { throw new Error("storage is blocked"); }
  };
  const blocked = kit({ storage: broken });
  blocked.controls.render(part());
  assert.equal(blocked.el("controls-service").textContent, "Ghostlight service is up; 1 MCP client connected.");
  assert.deepEqual(blocked.controls.personal(), { density: "comfortable", motion: "system" });
  const compact = blocked.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.value === "compact");
  blocked.click(compact);
  assert.equal(blocked.body.classes.has("density-compact"), true, "the choice still applies to this screen");

  for (const kept of ["not json", '{"density":"huge","motion":3}', "null", "[]"]) {
    const rubbish = kit({ storage: memory({ [Controls.STORAGE_KEY]: kept }) });
    assert.deepEqual(rubbish.controls.personal(), { density: "comfortable", motion: "system" }, kept);
  }
  const none = kit({ storage: null });
  assert.deepEqual(none.controls.personal(), { density: "comfortable", motion: "system" });
});

test("the sheet's markup holds the ids and attributes the guards and the surface look for", () => {
  const shown = page();
  for (const id of ["controls-open", "controls-sheet", "test-notification", "controls-continue"]) {
    assert.ok(shown.el(id));
  }
  const attribute = (name, value) => shown.document.querySelectorAll(`[data-${name}]`)
    .map((node) => node.dataset[name.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())])
    .filter((found) => value === undefined || found === value);
  assert.deepEqual(attribute("intent"), ["hold", "resume", "start_session", "end_session"]);
  assert.deepEqual(attribute("diagnostics-operation").sort(), ["reveal", "toggle"]);
  assert.ok(/role="dialog" aria-modal="true"/.test(markup), "the sheet is a modal dialog");
});
