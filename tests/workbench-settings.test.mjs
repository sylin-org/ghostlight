// Settings, the view and the window, exercised for real.
//
// Settings is a tab of its own: a rail of sections, each showing its current state, and the
// selected section beside it. The kit draws the service's controls part into the page's own
// markup and calls back into the page when a button is pressed; the view keeps the address. These
// tests run the real kit and the real view against the real markup from index.html, parsed into a
// minimal fake document, with fake callbacks and a fake localStorage. The window's tests run every
// script the page loads over the same markup against a fake service, and the stylesheet's tests
// read the rules styles.css and kit.css declare. No browser and no Ghostlight process is involved.
//
// Run with: node --test tests/workbench-settings.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import vm from "node:vm";
import { test } from "node:test";
import { join, resolve } from "node:path";

const require = createRequire(import.meta.url);
const ui = join(resolve(import.meta.dirname, ".."), "crates", "orchestrator", "ui");
const Settings = require(join(ui, "kit", "settings.js"));
const Words = require(join(ui, "lib", "words.js"));
require(join(ui, "lib", "entries.js"));
require(join(ui, "lib", "transport.js"));
const View = require(join(ui, "lib", "view.js"));
const markup = readFileSync(join(ui, "index.html"), "utf8");
const styles = readFileSync(join(ui, "styles.css"), "utf8") + readFileSync(join(ui, "kit", "kit.css"), "utf8");

const WHY = "Change this in the Ghostlight window.";
const WATCHING_OFF = "Watching is off on this computer. Turn it on in the Ghostlight window.";
const WATCHED = "Others on this computer can watch this view in a browser.";
const PRIVATE = "Watching is off: only this window shows Ghostlight.";
const WATCHER = "Anyone on this computer can open this view. Changes are made in the Ghostlight window.";
const SECTIONS = ["screen", "sharing", "browser-work", "notifications", "diagnostics"];

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
    you: { context, word: control ? "Full control" : "Watching", sentence: control ? (watch ? WATCHED : PRIVATE) : WATCHER }
  };
}

const VOID = new Set(["meta", "link", "img", "input", "br", "hr", "source", "wbr"]);

/** A fake element that knows its place in the tree, so `closest` answers like a browser's. */
class Element {
  constructor(tag, attributes = {}) {
    this.tag = tag;
    this.id = attributes.id ?? "";
    this.hidden = "hidden" in attributes;
    this.disabled = false;
    this.textContent = "";
    this.title = "";
    this.scrollTop = 0;
    this.parent = null;
    this.children = [];
    this.attributes = {};
    this.dataset = {};
    this.listeners = {};
    this.style = { setProperty() {} };
    this.classes = new Set(String(attributes.class ?? "").split(/\s+/).filter(Boolean));
    for (const [name, value] of Object.entries(attributes)) {
      if (["aria-current", "aria-expanded", "open", "src", "title"].includes(name)) this.attributes[name] = value;
      if (!name.startsWith("data-")) continue;
      this.dataset[name.slice(5).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = value;
    }
    this.classList = {
      toggle: (name, on) => {
        if (on === undefined ? !this.classes.has(name) : on) this.classes.add(name);
        else this.classes.delete(name);
      },
      contains: (name) => this.classes.has(name),
      add: (name) => this.classes.add(name),
      remove: (name) => this.classes.delete(name)
    };
  }

  addEventListener(type, handler) {
    (this.listeners[type] ??= []).push(handler);
  }

  removeEventListener() {}

  querySelector() {
    return null;
  }

  querySelectorAll() {
    return [];
  }

  append(...nodes) {
    for (const node of nodes) {
      node.parent = this;
      this.children.push(node);
    }
  }

  replaceChildren(...nodes) {
    this.children = [];
    this.append(...nodes);
  }

  remove() {
    if (this.parent) this.parent.children = this.parent.children.filter((child) => child !== this);
  }

  contains(node) {
    for (let at = node; at; at = at.parent) if (at === this) return true;
    return false;
  }

  getBoundingClientRect() {
    return { left: 0, top: 0, width: 1, height: 1 };
  }

  setAttribute(name, value) {
    this.attributes[name] = value;
  }

  removeAttribute(name) {
    delete this.attributes[name];
  }

  focus() {
    Element.focused = this;
  }

  matches(selector) {
    if (selector === "[id]") return this.id !== "";
    if (selector.startsWith("#")) return this.id === selector.slice(1);
    if (selector.startsWith(".")) return this.classes.has(selector.slice(1));
    if (/^[a-z]+$/.test(selector)) return this.tag === selector;
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

/**
 * Parse markup into fake elements. With `text`, each element's textContent is the text it holds,
 * its children's included, as a browser reads it.
 */
function parse(html, { text = false } = {}) {
  const root = new Element("root");
  const stack = [root];
  let at = 0;
  for (const found of html.matchAll(/<(\/?)([a-z0-9]+)((?:[^>"]|"[^"]*")*)>/g)) {
    const [whole, closing, tag, rest] = found;
    if (text) for (const open of stack.slice(1)) open.textContent += html.slice(at, found.index);
    at = found.index + whole.length;
    if (closing) {
      stack.pop();
      continue;
    }
    const attributes = {};
    for (const [, name, value] of rest.matchAll(/([a-z-]+)(?:="([^"]*)")?/g)) attributes[name] = value ?? "";
    const element = new Element(tag, attributes);
    element.parent = stack.at(-1);
    stack.at(-1).children.push(element);
    if (!VOID.has(tag) && !rest.trimEnd().endsWith("/")) stack.push(element);
  }
  return root;
}

/** Parse the whole page's markup into fake elements. */
function parsePage() {
  return parse(markup.slice(markup.indexOf("<body")));
}

/** Markup a paint produced, parsed, with every element in it that wears `className`. */
function drawn(html) {
  const root = parse(html, { text: true });
  return { root, wearing: (className) => [...root.descendants()].filter((node) => node.classes.has(className)) };
}

/** A page: the real markup as fake elements, a body, and the listeners the modules attach. */
function page() {
  const root = parsePage();
  const everything = [...root.descendants()];
  const byId = new Map(everything.filter((node) => node.id).map((node) => [node.id, node]));
  const body = everything.find((node) => node.tag === "body");
  const listeners = new Map();
  const document = {
    body,
    activeElement: null,
    hasFocus: () => true,
    createElement: (tag) => new Element(tag),
    createDocumentFragment: () => new Element("#fragment"),
    getElementById: (id) => byId.get(id) ?? null,
    querySelector: () => null,
    querySelectorAll: (selector) => everything.filter((node) => node.matches(selector)),
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
  const address = { hash: "", replaced: [], pushed: [] };
  const window = {
    location: address,
    history: {
      replaceState: (_state, _title, url) => {
        address.replaced.push(url);
        address.hash = url;
      },
      pushState: (_state, _title, url) => address.pushed.push(url)
    }
  };
  return {
    document, window, body, el, address, dispatch,
    click: (target) => dispatch("click", { target }),
    outside: () => dispatch("click", { target: new Element("p") })
  };
}

/** Callbacks that record what they were asked, and Settings over them. */
function kit(options = {}) {
  const shown = page();
  const calls = [];
  const failures = [];
  const record = (name) => async (...args) => {
    calls.push([name, ...args]);
  };
  const settings = Settings.create({
    document: shown.document,
    words: { ...Words.SETTINGS, screenState: Words.screenState, byteSize: Words.byteSize, addressOf: Words.addressOf },
    applyIntent: record("applyIntent"),
    setWatching: record("setWatching"),
    setDiagnostics: record("setDiagnostics"),
    revealDiagnostics: record("revealDiagnostics"),
    testNotification: record("testNotification"),
    continueInWindow: record("continueInWindow"),
    quit: record("quit"),
    copyText: record("copyText"),
    onFailure: (what, error) => failures.push([what, String(error)]),
    ...options
  });
  settings.wire();
  return { ...shown, settings, calls, failures };
}

/** The view over the page's markup, with the globals it reads set to that page. */
function viewOver(shown) {
  globalThis.document = shown.document;
  globalThis.window = shown.window;
  return View.create({});
}

/** The ids of the action buttons that are showing. */
function showing(shown) {
  return [
    "controls-hold", "controls-resume", "controls-start", "controls-end", "controls-viewing-copy",
    "controls-viewing-toggle", "controls-diagnostics-reveal", "controls-diagnostics-toggle", "test-notification"
  ].filter((id) => !shown.el(id).hidden);
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

const said = (shown, id) => shown.el(id).textContent;

/* ------------------------------- stylesheet rules ------------------------------ */

/**
 * The rules of a stylesheet, each with the media condition it sits under (null at the top level),
 * its selectors and its declarations by property. Comments are dropped.
 */
function rulesOf(sheet) {
  const rules = [];
  const walk = (text, media) => {
    let at = 0;
    while (at < text.length) {
      const open = text.indexOf("{", at);
      if (open < 0) return;
      const prelude = text.slice(at, open).trim();
      let depth = 1;
      let close = open + 1;
      for (; depth > 0; close += 1) {
        if (text[close] === "{") depth += 1;
        if (text[close] === "}") depth -= 1;
      }
      const body = text.slice(open + 1, close - 1);
      if (prelude.startsWith("@media")) {
        walk(body, prelude.slice("@media".length).trim());
      } else if (!prelude.startsWith("@")) {
        const declarations = {};
        for (const line of body.split(";")) {
          const colon = line.indexOf(":");
          if (colon > 0) declarations[line.slice(0, colon).trim()] = line.slice(colon + 1).trim();
        }
        rules.push({ media, selectors: prelude.split(",").map((selector) => selector.trim()), declarations });
      }
      at = close;
    }
  };
  walk(sheet.replace(/\/\*[\s\S]*?\*\//g, ""), null);
  return rules;
}

const RULES = rulesOf(styles);

/** The rules that name `selector` exactly. */
const naming = (selector, rules = RULES) => rules.filter((rule) => rule.selectors.includes(selector));

/** The widest width a media condition applies up to, or null when it is not a width at all. */
const widthOf = (media) => Number(/^\(max-width: (\d+)px\)$/.exec(media ?? "")?.[1] ?? NaN) || null;

/** The rules that apply at `width`: the top level, and every width query that reaches down to it. */
const applyingAt = (width) => RULES.filter((rule) => rule.media === null
  || (widthOf(rule.media) !== null && width <= widthOf(rule.media)));

/** How many tracks a grid-template-columns value declares; minmax(0, 1fr) is one. */
const tracksIn = (value) => value.replace(/\([^)]*\)/g, "()").split(/\s+/).filter(Boolean).length;

/* --------------------------------- the markup -------------------------------- */

test("Settings is a tab before About, and the Controls button and sheet are gone", () => {
  const shown = page();
  const tabs = shown.document.querySelectorAll(".tab").map((tab) => tab.dataset.view);
  assert.deepEqual(tabs, ["monitor", "integrations", "status", "policy", "settings", "about"]);
  assert.ok(shown.el("view-settings"));
  assert.equal(shown.el("view-settings").dataset.page, "settings");
  for (const gone of ["controls-open", "controls-sheet", "controls-screen", "controls-computer", "controls-service"]) {
    assert.equal(shown.document.getElementById(gone), null, `#${gone} is still in the markup`);
  }
});

test("the rail has five entries, each with a state word, and each opens a section of its own", () => {
  const shown = page();
  const entries = shown.document.querySelectorAll(".rail-entry");
  assert.deepEqual(entries.map((entry) => entry.dataset.settingsSection), SECTIONS);
  assert.deepEqual(Words.SETTINGS_SECTIONS.map((section) => section.id), SECTIONS);
  for (const section of SECTIONS) {
    assert.ok(shown.el(`rail-${section}`).closest("#settings-rail"));
    assert.ok(shown.el(`rail-${section}-state`));
    assert.equal(shown.el(`settings-${section}`).dataset.section, section);
  }
  assert.equal(shown.el("rail-screen").classes.has("active"), true);
  assert.equal(shown.el("settings-screen").hidden, false);
  for (const section of SECTIONS.slice(1)) assert.equal(shown.el(`settings-${section}`).hidden, true, section);
});

test("the markup holds the ids and attributes the guards and the surface look for", () => {
  const shown = page();
  for (const id of ["test-notification", "settings-quit", "status-service", "controls-runtime", "controls-viewing",
    "controls-diagnostics", "controls-notifications"]) {
    assert.ok(shown.el(id));
  }
  for (const section of SECTIONS.slice(1)) {
    for (const id of [`settings-${section}-rows`, `settings-${section}-closed`, `settings-${section}-continue`,
      `settings-${section}-why`, `settings-${section}-continue-button`]) {
      assert.ok(shown.el(id));
    }
  }
  const attribute = (name, value) => shown.document.querySelectorAll(`[data-${name}]`)
    .map((node) => node.dataset[name.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())])
    .filter((found) => value === undefined || found === value);
  // The band's wheel keeps its intent beside the four in Browser work.
  assert.deepEqual(attribute("intent"), ["hold", "hold", "resume", "start_session", "end_session"]);
  assert.deepEqual(attribute("diagnostics-operation").sort(), ["reveal", "toggle"]);
  assert.deepEqual(attribute("quit"), ["ask", "yes", "no"]);
  assert.equal(shown.document.querySelectorAll("[data-settings-continue]").length, 4);
});

test("the Policy tab is a tab that opens Policy, with no label beside it", () => {
  const shown = page();
  const tab = shown.el("policy-state");
  assert.equal(tab.classes.has("tab"), true);
  assert.equal(tab.dataset.view, "policy");
  assert.equal(shown.document.getElementById("policy-state-label"), null);
});

test("the Status page holds no control that moved to Settings", () => {
  const shown = page();
  const status = [...shown.el("view-status").descendants()];
  assert.deepEqual(status.filter((node) => "intent" in node.dataset).map((node) => node.id), []);
  assert.equal(status.some((node) => node.id === "test-notification"), false);
  assert.deepEqual(status.filter((node) => node.classes.has("runtime-strip") || node.classes.has("notifications-strip")), []);
});

/* ---------------------------------- addresses ---------------------------------- */

test("the closed list of addresses names every destination, every section and the setup view, once", () => {
  assert.deepEqual([...Words.ADDRESSES], [
    "monitor", "integrations", "status", "policy", "settings", "settings/screen", "settings/sharing",
    "settings/browser-work", "settings/notifications", "settings/diagnostics", "about", "setup"
  ]);
  // The setup view is an address and not a destination: no tab opens it, and it stands in for them.
  const derived = Object.keys(Words.VIEWS).flatMap((name) => (name === "settings"
    ? ["settings", ...SECTIONS.map((section) => `settings/${section}`)] : [name]));
  assert.deepEqual([...Words.ADDRESSES], [...derived, Words.SETUP_ADDRESS]);
  assert.deepEqual(Words.parseAddress("setup"), { view: "setup", section: null });
  assert.equal(new Set(Words.ADDRESSES).size, Words.ADDRESSES.length);
  // A search destination is the service's word for a hit, never an address.
  for (const destination of Object.keys(Words.SEARCH_VIEWS)) {
    if (!Object.keys(Words.VIEWS).includes(destination)) assert.equal(Words.parseAddress(destination), null, destination);
  }
});

test("an address reads with or without its hash, and anything else is not one", () => {
  assert.deepEqual(Words.parseAddress("#settings/sharing"), { view: "settings", section: "sharing" });
  assert.deepEqual(Words.parseAddress("settings/browser-work"), { view: "settings", section: "browser-work" });
  assert.deepEqual(Words.parseAddress("#settings"), { view: "settings", section: null });
  assert.deepEqual(Words.parseAddress("#policy"), { view: "policy", section: null });
  for (const text of ["", "#", "#nowhere", "#settings/", "#settings/other", "#monitor/screen", "#Settings",
    "#settings/screen/more", "#__proto__", "#constructor", null, undefined]) {
    assert.equal(Words.parseAddress(text), null, String(text));
  }
  assert.equal(Words.addressOf("settings", "sharing"), "settings/sharing");
  assert.equal(Words.addressOf("settings", null), "settings");
  assert.equal(Words.addressOf("about", "sharing"), "about");
});

test("selecting a section or a tab writes its address in place and adds no history entry", () => {
  const shown = page();
  const view = viewOver(shown);
  view.navigate("settings", "sharing");
  assert.equal(shown.address.hash, "#settings/sharing");
  assert.equal(view.showing(), "settings");
  assert.equal(view.showingSection(), "sharing");
  assert.equal(shown.el("view-settings").classes.has("active"), true);
  assert.equal(shown.el("view-monitor").classes.has("active"), false);
  assert.deepEqual(shown.document.querySelectorAll(".rail-entry").filter((entry) => entry.classes.has("active"))
    .map((entry) => entry.dataset.settingsSection), ["sharing"]);
  assert.equal(shown.el("rail-sharing").attributes["aria-current"], "true");
  assert.equal(shown.el("rail-screen").attributes["aria-current"], undefined);
  assert.deepEqual(shown.document.querySelectorAll(".settings-section").filter((part) => !part.hidden)
    .map((part) => part.dataset.section), ["sharing"]);
  assert.equal(shown.el("settings-heading").closest("#view-settings") !== null, true);
  assert.equal(shown.document.querySelectorAll(".tab").find((tab) => tab.classes.has("active")).dataset.view, "settings");

  view.navigate("settings", "diagnostics");
  assert.equal(shown.address.hash, "#settings/diagnostics");
  // The Settings tab alone returns to the section shown last.
  view.navigate("about");
  assert.equal(shown.address.hash, "#about");
  view.navigate("settings");
  assert.equal(shown.address.hash, "#settings/diagnostics");
  view.navigate("monitor");
  assert.equal(shown.address.hash, "#monitor");
  assert.deepEqual(shown.address.pushed, [], "a tab or a rail entry never adds a history entry");
  assert.equal(shown.address.replaced.length, 5);
  // Saying the same thing twice writes once.
  view.navigate("monitor");
  assert.equal(shown.address.replaced.length, 5);
});

test("a name that is not a destination, or a section that is not one, changes nothing", () => {
  const shown = page();
  const view = viewOver(shown);
  view.navigate("settings", "sharing");
  const before = [...shown.address.replaced];
  view.navigate("nowhere");
  view.navigate("settings", "nowhere");
  view.navigate("settings", "__proto__");
  assert.equal(view.showing(), "settings");
  assert.equal(view.showingSection(), "sharing");
  assert.deepEqual(shown.address.replaced, before);
});

test("a page that cannot be addressed still navigates", () => {
  const shown = page();
  shown.window.history.replaceState = () => {
    throw new Error("sandboxed");
  };
  const view = viewOver(shown);
  view.navigate("policy");
  assert.equal(view.showing(), "policy");
});

/* ----------------------------------- the rail ----------------------------------- */

test("each rail entry shows the state of its section, in the service's words", () => {
  const shown = kit();
  shown.settings.render(part({ runtime: "held" }));
  assert.equal(said(shown, "rail-screen-state"), "Comfortable, motion follows the system");
  assert.equal(said(shown, "rail-sharing-state"), "On -- anyone on this computer can open this view");
  assert.equal(said(shown, "rail-browser-work-state"), "Paused");
  assert.equal(said(shown, "rail-notifications-state"), "Send one local checkup notification.");
  assert.equal(said(shown, "rail-diagnostics-state"), "On (marker)");
  assert.equal(shown.el("rail-diagnostics-state").title, "On (marker)");

  shown.settings.render(part({ watch: false, layer: "off", runtime: "ended" }));
  assert.equal(said(shown, "rail-sharing-state"), "Off -- only this window shows Ghostlight");
  assert.equal(said(shown, "rail-browser-work-state"), "Session ended");
  assert.equal(said(shown, "rail-diagnostics-state"), "Off");
});

test("the rail's word for This screen follows the personal settings", () => {
  const shown = kit();
  const choice = (name, value) => shown.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === name && button.dataset.value === value);
  shown.click(choice("density", "compact"));
  assert.equal(said(shown, "rail-screen-state"), "Compact, motion follows the system");
  shown.click(choice("motion", "reduced"));
  assert.equal(said(shown, "rail-screen-state"), "Compact, motion reduced");
  assert.equal(Words.screenState("comfortable", "reduced"), "Comfortable, motion reduced");
});

test("the sections say what the part says, in the service's words", () => {
  const shown = kit();
  shown.settings.render(part({ runtime: "held" }));
  assert.equal(said(shown, "controls-runtime-label"), "Browser work");
  assert.equal(said(shown, "controls-runtime-word"), "Paused");
  assert.equal(said(shown, "controls-runtime-note"), "Paused from the tray.");
  assert.equal(said(shown, "controls-viewing-label"), "Others can watch");
  assert.equal(said(shown, "controls-viewing-word"), "On -- anyone on this computer can open this view");
  assert.equal(said(shown, "controls-viewing-url"), "http://127.0.0.1:41234/");
  assert.equal(said(shown, "controls-diagnostics-word"), "On (marker)");
  assert.equal(said(shown, "controls-diagnostics-place"), "~/.ghostlight/logs (2.0 KB)");
  assert.equal(said(shown, "controls-notifications-label"), "Notifications");
});

test("the page's own labels fill the rail, the headings and the buttons", () => {
  const shown = kit();
  assert.deepEqual(SECTIONS.map((section) => shown.el(`rail-${section}`).children[0].textContent),
    ["This screen", "Sharing", "Browser work", "Notifications", "Diagnostics"]);
  assert.equal(said(shown, "settings-browser-work-title"), "Browser work");
  assert.equal(said(shown, "controls-hold"), "Pause");
  assert.equal(said(shown, "controls-end"), "End session");
  assert.equal(said(shown, "test-notification"), "Send test");
  assert.equal(said(shown, "controls-diagnostics-reveal"), "Open log folder");
  assert.equal(said(shown, "settings-sharing-continue-button"), "Continue in the Ghostlight window");
  assert.equal(said(shown, "settings-quit-ask"), "Quit Ghostlight");
  assert.equal(said(shown, "settings-quit-question"), "Quit now? Agents lose the browser until Ghostlight starts again.");
});

/* ------------------------------------ Control ----------------------------------- */

test("in Control every row has its button, and pressing Turn off calls setWatching(false)", async () => {
  const shown = kit();
  shown.settings.render(part());
  assert.deepEqual(showing(shown), [
    "controls-hold", "controls-end", "controls-viewing-copy", "controls-viewing-toggle",
    "controls-diagnostics-reveal", "controls-diagnostics-toggle", "test-notification"
  ]);
  for (const section of SECTIONS.slice(1)) assert.equal(shown.el(`settings-${section}-continue`).hidden, true, section);
  assert.equal(said(shown, "controls-viewing-toggle"), "Turn off");
  assert.equal(said(shown, "controls-diagnostics-toggle"), "Turn off");

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
  shown.settings.render(part({ runtime: "active" }));
  assert.deepEqual(runtimeShowing(shown), ["controls-hold", "controls-end"]);
  shown.settings.render(part({ runtime: "held" }));
  assert.deepEqual(runtimeShowing(shown), ["controls-resume", "controls-end"]);
  shown.settings.render(part({ runtime: "ended" }));
  assert.deepEqual(runtimeShowing(shown), ["controls-start"]);
});

test("a switch that is off offers Turn on", async () => {
  const shown = kit();
  shown.settings.render(part({ watch: false, layer: "off" }));
  assert.equal(said(shown, "controls-viewing-toggle"), "Turn on");
  assert.equal(said(shown, "controls-diagnostics-toggle"), "Turn on");
  assert.equal(said(shown, "controls-diagnostics-word"), "Off");
  shown.click(shown.el("controls-viewing-toggle"));
  shown.click(shown.el("controls-diagnostics-toggle"));
  await settle();
  assert.deepEqual(shown.calls, [["setWatching", true], ["setDiagnostics", true]]);
});

test("in Control with no watch view served, the state is told once and nothing sends the window to itself", () => {
  const shown = kit();
  const unserved = part();
  unserved.viewing = {
    ...unserved.viewing, watch: false, url: null, word: "Not serving a watch view.",
    acts: { set_watching: { available: false, why_not: "Not serving a watch view." } }
  };
  shown.settings.render(unserved);
  assert.equal(said(shown, "controls-viewing-word"), "Not serving a watch view.");
  assert.equal(shown.el("controls-viewing-toggle").hidden, true);
  assert.equal(shown.el("controls-viewing-copy").hidden, true);
  assert.equal(shown.el("settings-sharing-continue").hidden, true);
  assert.equal(shown.el("controls-hold").hidden, false);
});

test("no copy button is drawn on a screen that cannot copy", () => {
  const shown = kit({ copyText: undefined });
  shown.settings.render(part());
  assert.equal(shown.el("controls-viewing-copy").hidden, true);
  assert.equal(shown.el("controls-viewing-url").hidden, false);
});

test("a button cannot be pressed twice while its act is still running", async () => {
  let release;
  const shown = kit({ setWatching: (watch) => new Promise((resolve) => { release = () => resolve(watch); }) });
  shown.settings.render(part());
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
  shown.settings.render(part());
  const send = shown.el("test-notification");
  shown.click(send);
  await settle();
  assert.deepEqual(shown.failures, [["using Settings", "Error: no way to notify"]]);
  assert.equal(send.disabled, false);
});

test("the band's wheel and a press outside Settings are not Settings' to answer", () => {
  const shown = kit();
  shown.settings.render(part());
  shown.click(shown.el("wheel"));
  shown.outside();
  assert.deepEqual(shown.calls, []);
});

/* ------------------------------------- Watch ------------------------------------ */

test("in Watch each other section says where the change is made, with one Continue each, and Pause stays", async () => {
  const shown = kit();
  shown.settings.render(part({ context: "watch" }));
  assert.deepEqual(showing(shown), ["controls-hold"]);
  for (const section of SECTIONS.slice(1)) {
    assert.equal(shown.el(`settings-${section}-continue`).hidden, false, section);
    assert.equal(said(shown, `settings-${section}-why`), WHY, section);
    assert.equal(shown.el(`settings-${section}-closed`).hidden, true, section);
  }
  // The rest is state: the words are still there, and nothing can be pressed to change them.
  assert.equal(said(shown, "controls-viewing-word"), "On -- anyone on this computer can open this view");
  assert.equal(said(shown, "controls-diagnostics-word"), "On (marker)");
  assert.equal(said(shown, "rail-sharing-state"), "On -- anyone on this computer can open this view");

  shown.click(shown.el("settings-sharing-continue-button"));
  await settle();
  assert.deepEqual(shown.calls, [["continueInWindow", "settings/sharing"]]);
});

test("each section's Continue asks for the window at that section's address", async () => {
  const shown = kit();
  shown.settings.render(part({ context: "watch" }));
  for (const section of SECTIONS.slice(1)) {
    shown.click(shown.el(`settings-${section}-continue-button`));
    await settle();
  }
  assert.deepEqual(
    shown.calls,
    SECTIONS.slice(1).map((section) => ["continueInWindow", `settings/${section}`])
  );
  for (const [, address] of shown.calls) assert.ok(Words.parseAddress(address), address);
});

test("while held, a watching screen shows the state, no button for the runtime, and the Continue line", () => {
  const shown = kit();
  shown.settings.render(part({ context: "watch", runtime: "held" }));
  assert.deepEqual(runtimeShowing(shown), []);
  assert.equal(said(shown, "controls-runtime-word"), "Paused");
  assert.equal(shown.el("settings-browser-work-continue").hidden, false);
});

test("in Watch This screen works, and nothing it does is sent anywhere", () => {
  const storage = memory();
  const shown = kit({ storage });
  shown.settings.render(part({ context: "watch" }));
  const compact = shown.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "density" && button.dataset.value === "compact");
  shown.click(compact);
  assert.equal(shown.body.classes.has("density-compact"), true);
  assert.equal(said(shown, "rail-screen-state"), "Compact, motion follows the system");
  assert.deepEqual(shown.calls, []);
});

/* ------------------------------------- Closed ----------------------------------- */

test("with watching off This screen works and every other section says so in one line", () => {
  const shown = kit();
  shown.settings.render(part());
  shown.settings.closed(WATCHING_OFF);
  for (const section of SECTIONS.slice(1)) {
    assert.equal(shown.el(`settings-${section}-closed`).hidden, false, section);
    assert.equal(said(shown, `settings-${section}-closed`), WATCHING_OFF, section);
    assert.equal(shown.el(`settings-${section}-rows`).hidden, true, section);
    assert.equal(shown.el(`settings-${section}-continue`).hidden, true, section);
    assert.equal(said(shown, `rail-${section}-state`), "", section);
  }
  assert.equal(shown.el("settings-quit").hidden, true);
  assert.equal(said(shown, "rail-screen-state"), "Comfortable, motion follows the system");

  const compact = shown.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "density" && button.dataset.value === "compact");
  shown.click(compact);
  assert.equal(shown.body.classes.has("density-compact"), true);
  // A press on a section that shows nothing asks for nothing.
  shown.click(shown.el("controls-hold"));
  assert.deepEqual(shown.calls, []);
});

test("a part arriving again after a closed run draws the sections back", () => {
  const shown = kit();
  shown.settings.closed(WATCHING_OFF);
  shown.settings.render(part());
  for (const section of SECTIONS.slice(1)) {
    assert.equal(shown.el(`settings-${section}-closed`).hidden, true, section);
    assert.equal(shown.el(`settings-${section}-rows`).hidden, false, section);
  }
});

/* --------------------------------------- Quit ---------------------------------- */

test("Quit is in Control alone, at the end of Browser work", () => {
  for (const context of ["control", "watch"]) {
    const shown = kit();
    shown.settings.render(part({ context }));
    assert.equal(shown.el("settings-quit").hidden, context !== "control", context);
  }
  const shown = kit();
  shown.settings.closed(WATCHING_OFF);
  assert.equal(shown.el("settings-quit").hidden, true);
  assert.ok(shown.el("settings-quit").closest("#settings-browser-work"));
  const rows = shown.el("settings-browser-work-rows").children.map((row) => row.id);
  assert.equal(rows.at(-1), "settings-quit");
});

test("a window the service does not let quit shows no Quit", () => {
  const shown = kit({ mayQuit: () => false });
  shown.settings.render(part());
  assert.equal(shown.el("settings-quit").hidden, true);
});

test("Quit asks inline, Cancel puts the button back, and only the second press quits", async () => {
  const shown = kit();
  shown.settings.render(part());
  const ask = shown.el("settings-quit-ask");
  const yes = shown.el("settings-quit-yes");
  const no = shown.el("settings-quit-no");
  const question = shown.el("settings-quit-question");
  assert.equal(ask.hidden, false);
  assert.equal(question.hidden, true);
  assert.equal(yes.hidden, true);
  assert.equal(no.hidden, true);

  shown.click(ask);
  assert.equal(ask.hidden, true, "the button becomes the question");
  assert.equal(question.hidden, false);
  assert.equal(yes.hidden, false);
  assert.equal(no.hidden, false);
  assert.equal(Element.focused, no, "the safe answer has the focus");
  await settle();
  assert.deepEqual(shown.calls, [], "asking quits nothing");

  shown.click(no);
  assert.equal(ask.hidden, false);
  assert.equal(question.hidden, true);
  assert.equal(yes.hidden, true);
  assert.deepEqual(shown.calls, []);

  shown.click(ask);
  shown.settings.render(part({ runtime: "held" }));
  assert.equal(question.hidden, false, "a new part does not take the question away");
  shown.click(yes);
  await settle();
  assert.deepEqual(shown.calls, [["quit"]]);
});

test("a Quit that fails is reported and the button comes back", async () => {
  const shown = kit({ quit: async () => { throw new Error("no way to quit"); } });
  shown.settings.render(part());
  shown.click(shown.el("settings-quit-ask"));
  shown.click(shown.el("settings-quit-yes"));
  await settle();
  assert.deepEqual(shown.failures, [["using Settings", "Error: no way to quit"]]);
  assert.equal(shown.el("settings-quit-ask").hidden, false);
  assert.equal(shown.el("settings-quit-yes").hidden, true);
  assert.equal(shown.el("settings-quit-yes").disabled, false);
});

test("a screen that stops being the window takes the question away", () => {
  const shown = kit();
  shown.settings.render(part());
  shown.click(shown.el("settings-quit-ask"));
  shown.settings.render(part({ context: "watch" }));
  shown.settings.render(part());
  assert.equal(shown.el("settings-quit-ask").hidden, false);
  assert.equal(shown.el("settings-quit-question").hidden, true);
});

/* ---------------------------------- who can see --------------------------------- */

test("Sharing says who can see this view in every context", () => {
  const shown = kit();
  shown.settings.render(part());
  assert.equal(said(shown, "settings-sharing-who"), WATCHED);
  shown.settings.render(part({ watch: false }));
  assert.equal(said(shown, "settings-sharing-who"), PRIVATE);
  shown.settings.render(part({ context: "watch" }));
  assert.equal(said(shown, "settings-sharing-who"), WATCHER);
  assert.equal(shown.el("settings-sharing-rows").hidden, false);
  shown.settings.closed(WATCHING_OFF);
  assert.equal(shown.el("settings-sharing-closed").hidden, false);
  assert.equal(said(shown, "settings-sharing-closed"), WATCHING_OFF);
});

/* ------------------------------ This screen, kept here --------------------------- */

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
  assert.deepEqual(first.settings.personal(), { density: "comfortable", motion: "system" });
  assert.deepEqual(chosen(first, "density"), ["comfortable"]);
  assert.equal(first.body.classes.has("density-compact"), false);

  const compact = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "density" && button.dataset.value === "compact");
  const reduced = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "motion" && button.dataset.value === "reduced");
  first.settings.render(part());
  first.click(compact);
  first.click(reduced);
  assert.equal(first.body.classes.has("density-compact"), true);
  assert.equal(first.body.classes.has("motion-reduced"), true);
  assert.deepEqual(chosen(first, "density"), ["compact"]);
  assert.deepEqual(JSON.parse(storage.kept[Settings.STORAGE_KEY]), { density: "compact", motion: "reduced" });
  assert.deepEqual(first.calls, [], "a personal setting is never sent anywhere");

  const reloaded = kit({ storage });
  assert.deepEqual(reloaded.settings.personal(), { density: "compact", motion: "reduced" });
  assert.equal(reloaded.body.classes.has("density-compact"), true);
  assert.equal(reloaded.body.classes.has("motion-reduced"), true);
  assert.equal(said(reloaded, "rail-screen-state"), "Compact, motion reduced");

  const comfortable = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "density" && button.dataset.value === "comfortable");
  first.click(comfortable);
  assert.equal(first.body.classes.has("density-compact"), false);
});

test("a browser that keeps nothing, or keeps rubbish, still draws Settings with the defaults", () => {
  const broken = {
    getItem() { throw new Error("storage is blocked"); },
    setItem() { throw new Error("storage is blocked"); }
  };
  const blocked = kit({ storage: broken });
  blocked.settings.render(part());
  assert.equal(said(blocked, "controls-viewing-label"), "Others can watch");
  assert.deepEqual(blocked.settings.personal(), { density: "comfortable", motion: "system" });
  const compact = blocked.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.value === "compact");
  blocked.click(compact);
  assert.equal(blocked.body.classes.has("density-compact"), true, "the choice still applies to this screen");

  for (const kept of ["not json", '{"density":"huge","motion":3}', "null", "[]"]) {
    const rubbish = kit({ storage: memory({ [Settings.STORAGE_KEY]: kept }) });
    assert.deepEqual(rubbish.settings.personal(), { density: "comfortable", motion: "system" }, kept);
  }
  const none = kit({ storage: null });
  assert.deepEqual(none.settings.personal(), { density: "comfortable", motion: "system" });
});

test("the storage key of the personal settings is the one the reload test reads", () => {
  assert.equal(Settings.STORAGE_KEY, "ghostlight.screen");
});

/* ---------------------------------- the window ---------------------------------- */

const Transport = require(join(ui, "lib", "transport.js"));

/** The page's own scripts, in the order its markup loads them. */
const SCRIPTS = [...markup.matchAll(/<script src="([^"]+)"><\/script>/g)]
  .map(([, src]) => readFileSync(join(ui, src), "utf8"));

/** What the service tells the Ghostlight window it may do: everything. */
const WINDOW_CONTEXT = {
  context: "control",
  opener: Transport.OPENER_SERVICE,
  word: "Full control",
  sentence: PRIVATE,
  product: { version: "2.0.0", license: "Apache-2.0 OR MIT" },
  acts: Object.fromEntries(Object.values(Transport.ACT).map((act) => [act, { available: true, why_not: null }]))
};

/**
 * An agent as the service names it: an agent on the roster wears its packaged tile, and the
 * command line and an agent Ghostlight does not know wear their glyphs.
 */
function namedAgent(name) {
  if (name === "Command line") return { id: "command_line", name, symbol: { glyph: "command_line" } };
  if (name === "Unknown agent") return { id: "unknown", name, symbol: { glyph: "unknown" } };
  const id = name.toLowerCase().replaceAll(" ", "-");
  return { id, name, symbol: { src: `integrations/${id}.svg` } };
}

/** One settled read, as the service records it. */
function settledRead(overrides = {}) {
  return {
    invocation: "read-1", workspace: "w1", tool: "browser_read", capability: "read", requirements: ["read"],
    allowed: true, reason: "permitted", status: "succeeded", effect: "none", complete: true, timestamp_ms: 1,
    duration_ms: 1200, summary: "Read 1,240 words from example.com.", storage: "saved", storage_detail: "",
    steps: [], permissions: { checks: [], truncated: false }, permission_explanations: [],
    observed: { host: "example.com", readiness: "complete", count: 1240 }, channel: "mcp",
    ...overrides
  };
}

/** A snapshot answer from a service with one client, one browser and one settled read. */
function snapshotAnswer({ history = [settledRead()], sessions, service = { version: "2.0.0", started_at_ms: 1, runtime_state: "active" } } = {}) {
  return {
    seq: 1,
    instance: "i1",
    parts: {
      status: {
        service,
        readiness: { state: "ready", word: "Ready", detail: "A browser is connected.", tone: "ready", invites_control: true },
        audit_health: { state: "saved" },
        audit_notice: "",
        configuration: {
          policy: { tone: "applied", detail: "Your rules apply." },
          managed_policy: { configured: false, contacts: [] }
        }
      },
      connections: {
        sessions: sessions ?? [{ id: "w1", agent: namedAgent("Codex"), channel: "mcp", tab_count: 1, active_operations: 0 }],
        browsers: [{ id: "b1", family: "Chrome", adapter_version: "2.0.0" }]
      },
      activity: { operations: [], history, document_coverage: [] },
      integrations: { harnesses: [] },
      diagnostics: { diagnostics: [] },
      controls: part()
    }
  };
}

/** A response the fake service gives. */
function reply(body, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => null },
    text: async () => JSON.stringify(body)
  };
}

/**
 * The whole window: the page's scripts run over the real markup against a fake service.
 *
 * `answer` maps a path to the body the service gives, an Error to fail the fetch, or undefined
 * for a request that never answers (the live stream). `before` may break the page before the
 * scripts run. Timers never fire, so nothing outlives the test.
 */
async function windowOver({ answer = () => undefined, before = () => {} } = {}) {
  const shown = page();
  const requests = [];
  const sent = [];
  const opened = [];
  const logged = [];
  const copied = [];
  const windowListeners = {};
  const fetch = async (path, init = {}) => {
    requests.push(path);
    sent.push({ path, body: init.body });
    const body = answer(path, init);
    if (body === undefined) return new Promise(() => {});
    if (body instanceof Error) throw body;
    return reply(body);
  };
  before(shown);
  const sandbox = vm.createContext({
    document: shown.document,
    window: {
      location: shown.address,
      history: shown.window.history,
      fetch,
      open: (address) => {
        opened.push(address);
        return null;
      },
      matchMedia: () => ({ matches: false, addEventListener() {} }),
      addEventListener: (type, handler) => (windowListeners[type] ??= []).push(handler)
    },
    navigator: { clipboard: { writeText: async (text) => { copied.push(text); } } },
    GhostlightAccess: Object.freeze({ bearer: "key-1" }),
    console: { error: (detail) => logged.push(detail), warn() {}, log() {} },
    setTimeout: () => 0,
    clearTimeout() {},
    setInterval: () => 0,
    clearInterval() {},
    AbortController, TextDecoder, TextEncoder, URL
  });
  for (const source of SCRIPTS) vm.runInContext(source, sandbox);
  for (let turn = 0; turn < 5; turn += 1) await settle();
  return {
    ...shown,
    requests,
    sent,
    opened,
    logged,
    copied,
    evaluate: (code) => vm.runInContext(code, sandbox),
    fire: (type, event) => {
      for (const handler of windowListeners[type] ?? []) handler(event);
    }
  };
}

/** The fake service of a healthy window. */
const healthy = (snapshot = snapshotAnswer()) => (path) => {
  if (path === "/api/context") return WINDOW_CONTEXT;
  if (path.startsWith("/api/snapshot?parts=setup")) return { seq: 1, parts: {} };
  if (path.startsWith("/api/snapshot")) return snapshot;
  return undefined;
};

test("the window comes up over the real markup and reports nothing", async () => {
  const shown = await windowOver({ answer: healthy() });
  assert.deepEqual(shown.logged, [], "every element the window reaches for exists, and every paint ran");
  assert.equal(shown.el("toast").hidden, true);
  assert.ok(shown.requests.some((path) => path.startsWith("/api/snapshot?parts=status")));
  assert.equal(said(shown, "state-word"), "Ready");
  assert.equal(shown.evaluate("store.connected()"), true);
});

test("a snapshot the window cannot draw is reported as a drawing fault, not as a lost connection", async () => {
  // The answer reads, but it names no service, which the cache cannot fold in.
  const shown = await windowOver({ answer: healthy(snapshotAnswer({ service: null })) });
  assert.match(said(shown, "toast"), /^rendering the snapshot: /);
  assert.equal(shown.el("toast").hidden, false);
  assert.equal(shown.evaluate("store.connected()"), true, "the service answered, so the window is connected");
  assert.notEqual(said(shown, "state-word"), Words.RECONNECTING);
});

test("a snapshot the service never gives is a lost connection, and nothing else is said", async () => {
  const answer = (path) => (path.startsWith("/api/snapshot?parts=status")
    ? new Error("connection refused") : healthy()(path));
  const shown = await windowOver({ answer });
  assert.equal(shown.evaluate("store.connected()"), false);
  assert.deepEqual(shown.logged, []);
  assert.equal(shown.el("toast").hidden, true);
});

test("an error that escapes a listener or a promise reaches the person using the window", async () => {
  const shown = await windowOver({ answer: healthy() });
  shown.fire("error", { error: new Error("a listener broke") });
  assert.equal(said(shown, "toast"), "surface: a listener broke");
  assert.equal(shown.el("toast").hidden, false);
  shown.fire("unhandledrejection", { reason: new Error("a promise broke") });
  assert.equal(said(shown, "toast"), "surface: a promise broke");
  assert.equal(shown.logged.length, 2, "the console hears each failure too");
});

test("a listener that cannot attach costs its own wiring, not the first snapshot", async () => {
  const before = (shown) => {
    shown.el("palette-query").addEventListener = () => {
      throw new Error("no listener here");
    };
  };
  const shown = await windowOver({ answer: healthy(), before });
  assert.match(said(shown, "toast"), /^wiring the surface: no listener here/);
  assert.ok(shown.requests.some((path) => path.startsWith("/api/snapshot?parts=status")), "the snapshot was still read");
  assert.equal(said(shown, "state-word"), "Ready");
});

test("Copy id puts the connection id on the clipboard and says so", async () => {
  const sessions = [glanceSession("w1", "Codex", { tabs: 1, browser: "Google Chrome" })];
  const shown = await windowOver({ answer: healthy(snapshotAnswer({ sessions })) });
  const bar = drawn(shown.el("connections").innerHTML);
  shown.click(bar.wearing("glance-chip").find((node) => node.dataset.glanceAgent === "codex"));
  const card = drawn(shown.el("connections").innerHTML);
  shown.click(card.wearing("glance-line")[0].children.find((child) => child.tag === "button"));
  await settle();
  assert.deepEqual(shown.copied, ["connection_w1"]);
  assert.equal(said(shown, "toast"), Words.CONNECTION_ID_COPIED);
  assert.deepEqual(shown.logged, []);
});

test("pressing a destination in the window asks the service to open it by name", async () => {
  const shown = await windowOver({ answer: healthy() });
  shown.click(shown.el("about-card"));
  await settle();
  assert.deepEqual(shown.sent.filter((request) => request.path === "/api/open"),
    [{ path: "/api/open", body: '{"destination":"home"}' }]);
  assert.deepEqual(shown.opened, []);
});

test("a browser tab opens the address the service listed for a destination, and sends nothing", async () => {
  const tab = {
    ...WINDOW_CONTEXT,
    opener: "browser",
    links: { home: "https://sylin.org/ghostlight/" }
  };
  const answer = (path) => (path === "/api/context" ? tab : healthy()(path));
  const shown = await windowOver({ answer });
  shown.click(shown.el("about-card"));
  await settle();
  assert.deepEqual(shown.opened, ["https://sylin.org/ghostlight/"]);
  assert.equal(shown.sent.some((request) => request.path === "/api/open"), false);
});

/* ----------------------------------- the view ----------------------------------- */

const Entries = globalThis.GhostlightEntries;

/** The view over the page's markup, recording every paint that failed. `sessionFor` reads a session. */
function drawing({ sessionFor } = {}) {
  const shown = page();
  const failures = [];
  globalThis.document = shown.document;
  globalThis.window = shown.window;
  const view = View.create({ sessionFor, onFailure: (what, error) => failures.push(`${what}: ${error?.message ?? error}`) });
  return { ...shown, view, failures };
}

/** The rows of the queue below the hero, as the view built them. */
function queueRows(shown) {
  return shown.el("queue").children.flatMap((child) => (child.tag === "#fragment" ? child.children : [child]));
}

test("a paint that throws is tried again with the same facts, and one that drew is not redrawn", () => {
  const shown = drawing();
  let shownText = "";
  let writes = 0;
  let ready = false;
  Object.defineProperty(shown.el("about-version"), "textContent", {
    get: () => shownText,
    set: (value) => {
      writes += 1;
      if (!ready) {
        ready = true;
        throw new Error("the disc is not ready");
      }
      shownText = value;
    }
  });
  const snapshot = Transport.flatten(snapshotAnswer());
  shown.view.collections(snapshot, new Set());
  assert.deepEqual(shown.failures.filter((failure) => failure.startsWith("painting about")),
    ["painting about: the disc is not ready"]);
  shown.view.collections(snapshot, new Set());
  assert.equal(shownText, "2.0", "the failed panel is drawn on the next pass");
  shown.view.collections(snapshot, new Set());
  assert.equal(writes, 2, "a panel that drew is not drawn again for the same facts");
});

/** A session of the agent named `agent`, as the connections part carries it. */
function glanceSession(id, agent, { channel = "mcp", tabs = 0, busy = false, browser, reported = agent } = {}) {
  return {
    id, agent: namedAgent(agent), channel, tab_count: tabs, active_operations: busy ? 1 : 0,
    ...(browser ? { browser } : {}),
    connections: [{
      connection_id: `connection_${id}`, reported_application: reported, channel,
      explanation: channel === "cli" ? "A ghostlight call session is a local program."
        : `"${reported}" is what the agent calls itself; Ghostlight does not verify it.`
    }]
  };
}

/** The connections part of a glance: Claude Code at work, Codex twice, and the command line. */
const GLANCE_SESSIONS = [
  glanceSession("w1", "Claude Code", { tabs: 1, busy: true, browser: "Google Chrome", reported: "claude-code" }),
  glanceSession("w2", "Codex", { reported: "codex-mcp-client" }),
  glanceSession("w3", "Codex", { tabs: 2, browser: "Google Chrome" }),
  glanceSession("w4", "Command line", { channel: "cli" })
];

/** A view that has drawn the glance of `sessions` and `browsers`, with the parsed markup. */
function glance({ sessions = GLANCE_SESSIONS, browsers = [{ id: "b1", family: "Google Chrome", adapter_version: "2.0.0", tab_count: 3 }], noBrowser } = {}) {
  const shown = drawing();
  const answer = snapshotAnswer({ sessions });
  answer.parts.connections.browsers = browsers;
  if (noBrowser !== undefined) answer.parts.connections.no_browser = noBrowser;
  const snapshot = Transport.flatten(answer);
  shown.view.collections(snapshot, new Set());
  const read = () => drawn(shown.el("connections").innerHTML);
  return { ...shown, snapshot, read };
}

test("the glance is one closed summary: who is connected, toward how many browsers, and Details", () => {
  const shown = glance();
  const bar = shown.read();
  const details = bar.wearing("glance");
  assert.equal(details.length, 1);
  assert.equal(details[0].tag, "details");
  assert.equal("open" in details[0].attributes, false, "closed by default");
  assert.deepEqual(bar.wearing("glance-face").map((face) => face.attributes.title), ["Codex", "Claude Code", "Command line"],
    "a face for each agent, named on hover");
  assert.deepEqual(bar.wearing("glance-face").map((face) => face.textContent), ["", "", ""], "no initials");
  assert.equal(bar.wearing("glance-face").filter((face) => face.classes.has("busy")).length, 1);
  assert.deepEqual(bar.wearing("glance-side").map((side) => side.textContent), ["3 agents", "1 browser"]);
  assert.equal(bar.wearing("glance-closed-word")[0].textContent, "Details");
  assert.equal(bar.wearing("glance-open-word")[0].textContent, "Hide");
  assert.deepEqual(shown.failures, []);
});

test("an agent wears the symbol the service names, drawn by one renderer in the pill, the shelves, its card and the rail", () => {
  const shown = drawing();
  const answer = snapshotAnswer({ sessions: GLANCE_SESSIONS });
  answer.parts.integrations.harnesses = [agent("claude-code", "Claude Code", "installed"), agent("codex", "Codex", "installed")];
  const snapshot = Transport.flatten(answer);
  shown.view.collections(snapshot, new Set());
  const codex = namedAgent("Codex");
  const html = () => shown.el("connections").innerHTML;
  const faces = drawn(html()).wearing("glance-face");
  assert.ok(html().includes(Words.symbolMarkup(codex, "face")), "the pill");
  assert.ok(html().includes(Words.symbolMarkup(namedAgent("Claude Code"), "face")));
  assert.ok(html().includes(Words.symbolMarkup(namedAgent("Command line"), "face")), "the command line wears its prompt");
  assert.ok(html().includes(Words.symbolMarkup(codex, "chip")), "the shelf");
  shown.click(drawn(html()).wearing("glance-chip").find((node) => node.dataset.glanceAgent === "codex"));
  assert.ok(html().includes(Words.symbolMarkup(codex, "card")), "the card");
  assert.ok(shown.el("agents-list").innerHTML.includes(Words.symbolMarkup(codex, "rail")), "the rail");
  assert.ok(html().includes('<img src="integrations/codex.svg" alt="" width="20" height="20">'));
  assert.ok(Words.symbolMarkup(codex, "rail").includes('src="integrations/codex.svg"'), "the same tile at every size");
  assert.ok(!("agent" in Words.KIND_GLYPHS) && !("initials" in Words), "no spark and no initials for an agent");
  assert.equal(faces.filter((face) => face.classes.has("busy")).length, 1, "the working agent still shows it");
  assert.deepEqual(shown.failures, []);
});

test("the command line and an unknown agent are drawn from the glyph their symbol names", () => {
  assert.equal(Words.symbolMarkup(namedAgent("Command line"), "chip"),
    `<span class="symbol symbol-chip" aria-hidden="true">${Words.SYMBOL_GLYPHS[Words.GLYPH.commandLine]}</span>`);
  assert.equal(Words.symbolMarkup(namedAgent("Unknown agent"), "chip"),
    `<span class="symbol symbol-chip" aria-hidden="true">${Words.SYMBOL_GLYPHS[Words.GLYPH.unknown]}</span>`);
  assert.equal(Words.symbolMarkup(namedAgent("Codex"), "pane"),
    '<span class="symbol symbol-pane" aria-hidden="true"><img src="integrations/codex.svg" alt="" width="46" height="46"></span>');
  for (const size of Object.keys(Words.SYMBOL_SIZES)) {
    assert.ok(naming(`.symbol-${size} img`).length > 0, `the stylesheet sizes a ${size} tile`);
  }
});

test("the summary counts in words, singular for one and plain for none", () => {
  assert.deepEqual([0, 1, 3].map(Words.agentsCount), ["No agents", "1 agent", "3 agents"]);
  assert.deepEqual([0, 1, 2].map(Words.browsersCount), ["no browser", "1 browser", "2 browsers"]);
  const lone = glance({ sessions: [GLANCE_SESSIONS[0]], browsers: [], noBrowser: "No browser connected" }).read();
  assert.deepEqual(lone.wearing("glance-side").map((side) => side.textContent), ["1 agent", "no browser"]);
});

test("the summary stays open across a repaint once a person opened it", () => {
  const shown = glance();
  const details = shown.read().wearing("glance")[0];
  details.open = true;
  details.isConnected = true;
  shown.dispatch("toggle", { target: details });
  shown.view.collections({ ...shown.snapshot, sessions: [GLANCE_SESSIONS[1]] }, new Set());
  assert.equal("open" in shown.read().wearing("glance")[0].attributes, true);
});

test("the opened summary stays open when the activity feed is rebuilt", () => {
  const shown = glance();
  const details = shown.read().wearing("glance")[0];
  details.open = true;
  details.isConnected = true;
  shown.dispatch("toggle", { target: details });
  shown.view.rebuildFeed([Entries.entryFromRecord(settledRead({ invocation: "newest" }), null)]);
  shown.view.rebuildFeed([]);
  shown.view.collections({ ...shown.snapshot, sessions: [GLANCE_SESSIONS[2]] }, new Set());
  assert.equal("open" in shown.read().wearing("glance")[0].attributes, true,
    "a queued call, Clear view or a reconnect rebuilds the feed, which owns no page disclosure");
});

test("the shelves name each agent once with its glyph, a count past one session, and a working agent lit", () => {
  const shown = glance();
  const bar = shown.read();
  const shelves = bar.wearing("glance-shelf");
  assert.deepEqual(shelves.map((shelf) => shelf.children[0].textContent), ["Agents", "Browsers"]);
  const agents = bar.wearing("glance-chip").filter((chip) => chip.tag === "button");
  assert.deepEqual(agents.map((chip) => chip.textContent), ["Codex 2", "Claude Code", "Command line"]);
  assert.deepEqual(agents.map((chip) => chip.classes.has("busy")), [false, true, false]);
  assert.deepEqual(agents.map((chip) => chip.attributes["aria-expanded"]), ["false", "false", "false"]);
  const chipMarkup = (id) => shown.el("connections").innerHTML.split(`data-glance-agent="${id}"`)[1].split("</button>")[0];
  assert.ok(chipMarkup("codex").includes(Words.symbolMarkup(namedAgent("Codex"), "chip")), "an agent wears its tile");
  assert.ok(chipMarkup("command_line").includes(Words.SYMBOL_GLYPHS[Words.GLYPH.commandLine]), "the command line wears the prompt");
  const browsers = bar.wearing("glance-chip").filter((chip) => chip.tag === "span");
  assert.deepEqual(browsers.map((chip) => chip.textContent), ["Google Chrome3 tabs"]);
});

test("the shelves group sessions by the agent id the service gives, never by name", () => {
  const cline = { id: "cline", name: "Cline", symbol: { src: "integrations/cline.svg" } };
  const stranger = (name) => ({ id: `unknown:${name}`, name, symbol: { glyph: "unknown" } });
  const sessions = [
    { ...glanceSession("on-cli", "Cline"), agent: cline },
    { ...glanceSession("in-cursor", "Cline"), agent: cline },
    { ...glanceSession("one", "Helper"), agent: stranger("Helper") },
    { ...glanceSession("two", "Other helper"), agent: stranger("Other helper") }
  ];
  const agents = glance({ sessions, browsers: [] }).read().wearing("glance-chip").filter((chip) => chip.tag === "button");
  assert.deepEqual(agents.map((chip) => [chip.dataset.glanceAgent, chip.textContent]),
    [["cline", "Cline 2"], ["unknown:Helper", "Helper"], ["unknown:Other helper", "Other helper"]],
    "an agent on two hosts is one chip, and two agents Ghostlight does not know stay two");
});

test("a working agent's symbol breathes in the accent, and reduced motion keeps it still", () => {
  const breathing = naming(".glance-chip.busy .symbol").filter((rule) => rule.media === null);
  assert.equal(breathing.length, 1);
  assert.match(breathing[0].declarations.animation, /^glyph-breathe /);
  assert.equal(breathing[0].declarations.color, "var(--a)");
  const still = naming("*").filter((rule) => rule.media === "(prefers-reduced-motion: reduce)");
  assert.ok(still.some((rule) => /^none/.test(rule.declarations.animation)), "reduced motion stops every animation");
});

test("with no browser the Browsers shelf says what the service says happens next", () => {
  for (const said of ["Starts Google Chrome on request", "No browser connected"]) {
    const bar = glance({ browsers: [], noBrowser: said }).read();
    const missing = bar.wearing("missing");
    assert.deepEqual(missing.map((chip) => chip.textContent), [said]);
  }
  const empty = glance({ sessions: [], browsers: [], noBrowser: "No browser connected" }).read();
  assert.deepEqual(empty.wearing("missing").map((chip) => chip.textContent), ["No agent connected", "No browser connected"]);
});

test("an agent on its shelf opens its card: a few sessions in words, and the trust note once", () => {
  const many = [
    ...Array.from({ length: 5 }, (_, at) => glanceSession(`c${at}`, "Codex", { reported: "codex-mcp-client" })),
    glanceSession("c9", "Codex", { tabs: 2, busy: true, browser: "Google Chrome", reported: "codex-mcp-client" })
  ];
  const shown = glance({ sessions: many });
  const chip = shown.read().wearing("glance-chip").find((node) => node.dataset.glanceAgent === "codex");
  shown.click(chip);
  const card = shown.read().wearing("glance-card");
  assert.equal(card.length, 1);
  assert.equal(shown.read().wearing("glance-chip").find((node) => node.dataset.glanceAgent === "codex").attributes["aria-expanded"], "true");
  const lines = shown.read().wearing("glance-line");
  assert.equal(lines.length, Words.GLANCE_CARD_LINES);
  assert.equal(lines[0].children[0].textContent, "2 tabs in Google Chrome, working", "working sessions first");
  assert.equal(lines[1].children[0].textContent, "No tabs, idle");
  assert.deepEqual(shown.read().wearing("glance-more").map((more) => more.textContent), ["and 2 more"]);
  const notes = card[0].children.filter((child) => child.classes.has("connection-explanation"));
  assert.deepEqual(notes.map((note) => note.textContent), ["&quot;codex-mcp-client&quot; is what the agent calls itself; Ghostlight does not verify it."]);
  assert.ok(!shown.el("connections").innerHTML.includes("Intake"), "the intake is not a field of the card");
  shown.click(shown.read().wearing("glance-chip").find((node) => node.dataset.glanceAgent === "codex"));
  assert.equal(shown.read().wearing("glance-card").length, 0, "the same chip closes it");
});

test("a session line says the command line only for the command line", () => {
  assert.equal(Words.sessionLine({ tab_count: 1, browser: "Edge", active_operations: 0, channel: "cli" }),
    "1 tab in Edge, idle, from the command line");
  assert.equal(Words.sessionLine({ tab_count: 0, browser: "Edge", active_operations: 1, channel: "mcp" }),
    "In Edge, no tabs, working");
});

test("each line of a card keeps its connection ids behind Copy id", () => {
  const shown = glance();
  shown.click(shown.read().wearing("glance-chip").find((node) => node.dataset.glanceAgent === "codex"));
  const copies = shown.read().wearing("glance-line").map((line) => line.children.find((child) => child.tag === "button"));
  assert.deepEqual(copies.map((button) => button.textContent), ["Copy id", "Copy id"]);
  assert.deepEqual(copies.map((button) => button.dataset.copyConnection), ["connection_w3", "connection_w2"]);
  assert.ok(!shown.read().wearing("glance-line").some((line) => line.textContent.includes("connection_")),
    "no id is printed in the line");
});

/* ------------------------------- Integrations -------------------------------- */

/** One agent of the roster, as the integrations part carries it. */
function agent(id, name, state, extra = {}) {
  return {
    id, product_id: id, name, label: name, target: "User", agent: { id, name, symbol: { src: `integrations/${id}.svg` } },
    state, detail: "Said by the service.",
    can_install: ["available", "updatable", "not_detected"].includes(state) && !extra.copy_only,
    can_fix: false, can_uninstall: state === "installed", can_download: true, can_locate: true, copy_only: false,
    config_path: `/home/ann/.${id}/mcp.json`, connector_command: "/home/ann/.ghostlight/bin/ghostlight mcp",
    manual_setup: '{\n  "mcpServers": {\n    "ghostlight": {\n      "command": "/home/ann/.ghostlight/bin/ghostlight"\n    }\n  }\n}',
    ...extra
  };
}

/**
 * A view that has drawn the Integrations page of `harnesses` beside `sessions`, in `context`, with
 * the rows the service says are in use (`inUse`, their ids).
 */
function agentsPage(harnesses, { sessions = [], inUse = [], context } = {}) {
  const shown = drawing();
  if (context) shown.view.context(context);
  const answer = snapshotAnswer({ sessions });
  answer.parts.integrations.harnesses = harnesses;
  answer.parts.connections.harnesses_in_use = inUse;
  const snapshot = Transport.flatten(answer);
  shown.view.collections(snapshot, new Set());
  const rail = () => drawn(shown.el("agents-list").innerHTML);
  const pane = () => drawn(shown.el("agents-pane").innerHTML);
  const title = () => [...pane().root.descendants()].find((node) => node.id === "agents-pane-title")?.textContent;
  const pick = (id) => shown.click(rail().wearing("agent-entry").find((entry) => entry.dataset.agent === id));
  return { ...shown, snapshot, rail, pane, title, pick };
}

const ROSTER = [
  agent("zed", "Zed", "installed"),
  agent("codex", "Codex", "installed"),
  agent("cursor", "Cursor", "available"),
  agent("junie", "Junie", "needs_attention", { can_install: false, evidence: "Found another program, `npx` with 2 arguments, where Ghostlight's entry belongs; Ghostlight maintains `/home/ann/.ghostlight/bin/ghostlight` there and changed nothing." }),
  agent("crush", "Crush", "not_detected"),
  agent("goose", "goose", "not_detected", { copy_only: true }),
  agent("kiro", "Kiro", "updatable")
];

test("Integrations opens on what needs attention, then what was found, then an agent with tabs, then the first connected", () => {
  const working = [glanceSession("w1", "Zed", { tabs: 2 })];
  assert.equal(agentsPage(ROSTER, { sessions: working }).title(), "Junie");
  const found = ROSTER.filter((row) => !["junie", "kiro"].includes(row.id));
  assert.equal(agentsPage(found, { sessions: working }).title(), "Cursor");
  const connected = found.filter((row) => row.id !== "cursor");
  assert.equal(agentsPage(connected, { sessions: working }).title(), "Zed", "the agent with tabs");
  assert.equal(agentsPage(connected).title(), "Codex", "the first connected, by name");
  assert.equal(agentsPage(ROSTER.filter((row) => row.state === "not_detected")).title(), "Crush");
});

test("the pick is kept by id when the agent moves group, and the page draws nothing it was not given", () => {
  const shown = agentsPage(ROSTER);
  shown.pick("cursor");
  assert.equal(shown.title(), "Cursor");
  const connected = ROSTER.map((row) => (row.id === "cursor" ? { ...row, state: "installed", can_install: false, can_uninstall: true } : row));
  shown.view.collections({ ...shown.snapshot, harnesses: connected }, new Set());
  assert.equal(shown.title(), "Cursor");
  assert.deepEqual(shown.failures, []);
  const empty = agentsPage([]);
  assert.equal(empty.rail().wearing("agents-empty")[0].textContent, Words.AGENTS.noAgents);
  assert.equal(empty.el("agents-pane").innerHTML, "");
  assert.equal(empty.el("copy-command").hidden, true, "no command to copy without an agent");
});

test("the agents not on this computer fold into one closed row that opens in place, and search reaches them", () => {
  const shown = agentsPage(ROSTER);
  const fold = () => shown.rail().wearing("agents-fold")[0];
  assert.equal(fold().attributes["aria-expanded"], "false");
  assert.equal(fold().textContent.replace(/\s+/g, " ").trim(), "2 more agents Ghostlight can joinNot on this computer");
  assert.equal([...shown.rail().root.descendants()].find((node) => node.id === "agents-absent").hidden, true);
  shown.click(fold());
  assert.equal(fold().attributes["aria-expanded"], "true");
  assert.equal([...shown.rail().root.descendants()].find((node) => node.id === "agents-absent").hidden, false);
  shown.click(fold());
  shown.el("agents-find").listeners.input[0]({ target: { value: "GOO" } });
  assert.equal(shown.rail().wearing("agents-fold").length, 0, "a search shows the folded agents it reaches under their heading");
  assert.deepEqual(shown.rail().wearing("agent-entry").map((entry) => entry.dataset.agent), ["goose"]);
  assert.deepEqual(shown.rail().wearing("agents-group-head").map((head) => head.textContent), ["Not on this computer1"]);
  shown.pick("goose");
  shown.el("agents-find").listeners.input[0]({ target: { value: "" } });
  assert.equal(fold().attributes["aria-expanded"], "true", "the folded agent the person picked stays in view");
  shown.el("agents-find").listeners.input[0]({ target: { value: "nobody" } });
  assert.equal(shown.rail().wearing("agents-empty")[0].textContent, Words.noAgentCalled("nobody").replaceAll('"', "&quot;"));
});

test("each state reads as its own words, and a group shows only when it holds an agent", () => {
  const shown = agentsPage(ROSTER, { sessions: [glanceSession("w1", "Zed", { tabs: 2 }), glanceSession("w2", "Zed")], inUse: ["zed"] });
  assert.deepEqual(shown.rail().wearing("agents-group-head").map((head) => head.children[0].textContent),
    ["Needs a look", "Connected", "Found, not connected"]);
  const sub = (id) => [...shown.rail().wearing("agent-entry").find((entry) => entry.dataset.agent === id).descendants()].find((node) => node.classes.has("agent-entry-sub"))?.textContent ?? null;
  assert.equal(sub("zed"), "2 sessions, 2 tabs");
  assert.equal(sub("codex"), null, "a plain connected agent has no second line");
  assert.equal(sub("kiro"), "Needs an update");
  assert.equal(sub("junie"), "Needs your attention");
  assert.equal(sub("goose"), "Copy setup only");
  const dot = (id) => shown.rail().wearing("agent-entry").find((entry) => entry.dataset.agent === id)
    .children.find((node) => node.classes.has("agent-dot"));
  assert.ok(dot("zed").classes.has("d-live"), "an agent in use wears the teal dot");
  assert.ok(dot("codex").classes.has("d-installed"));
  const agentsAgain = agentsPage(ROSTER.filter((row) => row.state === "installed"));
  assert.deepEqual(agentsAgain.rail().wearing("agents-group-head").map((head) => head.children[0].textContent), ["Connected"]);
});

test("a pane for each state: its sentence, the step it allows with what it does, and the entry whole", () => {
  const shown = agentsPage(ROSTER);
  const acts = () => shown.pane().wearing("agent-act").map((row) => [row.children[0].textContent, row.children[1].textContent]);
  shown.pick("kiro");
  assert.deepEqual(acts(), [["Update", "Rewrites Ghostlight's own entry in mcp.json. Nothing else in the file changes. Restart Kiro afterwards."]]
    .map(([label, why]) => [label, why.replaceAll("'", "&#039;")]));
  shown.pick("crush");
  assert.deepEqual(acts().map(([label]) => label), ["Install", "Prepare"]);
  assert.match(shown.pane().wearing("agent-lead")[0].textContent, /^Crush is not installed on this computer\. You can install it, or prepare its settings first\.$/);
  shown.pick("goose");
  assert.deepEqual(acts().map(([label]) => label), ["Install", "Copy setup"], "copy-only is never offered Prepare");
  assert.equal(shown.pane().wearing("agent-lead")[0].textContent, "goose is not installed on this computer.");
  shown.pick("codex");
  assert.deepEqual(acts(), [], "a connected agent has nothing to do next");
  const lines = shown.pane().wearing("agent-doc")[0].children.map((line) => [...line.classes].join(" "));
  assert.deepEqual(lines, ["ln i0", "ln i1", "ln i2", "ln i3", "ln i2", "ln i1", "ln i0"], "one block per line, its indent a class");
  assert.equal(shown.pane().wearing("agent-remove")[0].textContent, "Remove from Codex");
});

test("an agent that needs attention shows the service's evidence and numbered steps when it cannot be fixed", () => {
  const shown = agentsPage(ROSTER);
  assert.equal(shown.title(), "Junie");
  assert.equal(shown.pane().wearing("agent-evidence")[0].children[1].textContent.includes("`npx` with 2 arguments"), true);
  const steps = shown.pane().wearing("agent-steps")[0].children;
  assert.deepEqual(steps.map((step) => step.children[1].textContent), ["Copy setup", "Re-check"]);
  const fixable = agentsPage(ROSTER.map((row) => (row.id === "junie" ? { ...row, can_fix: true } : row)));
  assert.equal(fixable.pane().wearing("agent-steps").length, 0);
  assert.deepEqual(fixable.pane().wearing("agent-act").map((row) => row.children[0].textContent), ["Fix"]);
  const unreadable = agentsPage([agent("zed", "Zed", "needs_attention", { detail: "This configuration could not be read; it was left untouched." })]);
  assert.equal(unreadable.pane().wearing("agent-lead")[0].textContent, "This configuration could not be read; it was left untouched.",
    "without evidence the service's own sentence names what it saw");
  const bare = unreadable.pane().wearing("agent-steps")[0].children;
  assert.equal(bare.length, 2);
  assert.ok(bare[0].children[0].textContent.startsWith(Words.AGENTS.repairStep), "the person corrects the file");
  assert.equal(bare[0].children.length, 1, "the repair step has no button");
  assert.equal(bare[1].children[1].textContent, "Re-check");
  const html = unreadable.el("agents-pane").innerHTML;
  assert.ok(!html.includes("what Ghostlight found"), "Ghostlight found no entry, so nothing says it did");
  assert.ok(!html.includes(Words.AGENTS.copyStep));
});

test("a session that arrives after the first paint reaches the rail, the pane and Remove", () => {
  const shown = agentsPage(ROSTER);
  shown.pick("codex");
  const entry = () => shown.rail().wearing("agent-entry").find((node) => node.dataset.agent === "codex");
  const sub = () => [...entry().descendants()].find((node) => node.classes.has("agent-entry-sub"))?.textContent ?? null;
  assert.equal(sub(), null);
  assert.equal(shown.pane().wearing("agent-live").length, 0);
  assert.equal(shown.pane().wearing("agent-remove")[0].dataset.harnessSessions, "0");
  shown.view.collections({ ...shown.snapshot, sessions: [glanceSession("w1", "Codex", { tabs: 3 })], harnesses_in_use: ["codex"] },
    new Set());
  assert.equal(sub(), "1 session, 3 tabs");
  assert.ok(entry().children.find((node) => node.classes.has("agent-dot")).classes.has("d-live"), "the dot pulses teal");
  assert.equal(shown.pane().wearing("agent-live")[0].children[1].textContent,
    "1 Codex session is connected right now, with 3 browser tabs open.");
  assert.equal(shown.pane().wearing("agent-remove")[0].dataset.harnessSessions, "1");
  shown.view.collections({ ...shown.snapshot, sessions: [], harnesses_in_use: [] }, new Set());
  assert.equal(sub(), null, "and leaves when the session ends");
  assert.equal(shown.pane().wearing("agent-remove")[0].dataset.harnessSessions, "0");
  assert.deepEqual(shown.failures, []);
});

test("only the rows the service says are in use pulse, so one session never lights every host of its agent", () => {
  const cline = (id, label, state) => ({ ...agent(id, "Cline", state), label, agent: namedAgent("Cline") });
  const rows = [cline("cline-cli", "Cline (CLI)", "installed"), cline("cline-vscode", "Cline (Visual Studio Code)", "available"),
    cline("cline-cursor", "Cline (Cursor)", "installed")];
  const shown = agentsPage(rows, { sessions: [glanceSession("w1", "Cline", { tabs: 1 })], inUse: ["cline-cli"] });
  const dot = (id) => shown.rail().wearing("agent-entry").find((entry) => entry.dataset.agent === id)
    .children.find((node) => node.classes.has("agent-dot"));
  assert.ok(dot("cline-cli").classes.has("d-live"));
  assert.ok(dot("cline-cursor").classes.has("d-installed"), "a session the service did not put on this host leaves it still");
  assert.ok(dot("cline-vscode").classes.has("d-available"));
  assert.deepEqual(shown.rail().wearing("agents-group-head").map((head) => head.children[0].textContent),
    ["Connected", "Found, not connected"]);
});

test("a copy-only agent found here claims nothing about a file Ghostlight never reads", () => {
  const goose = agent("goose", "goose", "available", { copy_only: true, config_path: "/home/ann/.config/goose/config.yaml" });
  const rows = [agent("zed", "Zed", "installed"), goose];
  const shown = agentsPage(rows);
  assert.deepEqual(shown.rail().wearing("agents-group-head").map((head) => head.children[0].textContent),
    ["Connected", "Found, set up by hand"]);
  assert.ok(!shown.el("agents-list").innerHTML.includes("Found, not connected"));
  assert.equal(shown.el("integrations-heading").innerHTML, "Ghostlight is in <em>Zed</em>.");
  assert.equal(Words.agentHeadline([{ standing: "installed", label: "Zed" }, { standing: "installed", label: "Codex" },
    { standing: "by_hand", label: "goose" }]).trail, " agents it can check.");
  shown.pick("goose");
  assert.equal(shown.pane().wearing("agent-state")[0].textContent, "Found, set up by hand");
  const lead = shown.pane().wearing("agent-lead")[0].textContent;
  assert.match(lead, /cannot tell whether goose has Ghostlight&#039;s entry/);
  assert.ok(!/does not know about Ghostlight|not connected/.test(lead), lead);
  assert.deepEqual(shown.pane().wearing("agent-act").map((row) => row.children[0].textContent), ["Copy setup"]);
  assert.equal(shown.el("integrations-caption").textContent, "Nothing Ghostlight can set up. goose is left for you.");
  const alone = agentsPage([goose]);
  assert.equal(alone.el("integrations-heading").innerHTML, "Ghostlight cannot check <em>goose</em>, which is set up by hand.");
});

test("a copy-only agent with a live session is connected, and is never called not connected", () => {
  const goose = agent("goose", "goose", "available", { copy_only: true, config_path: "/home/ann/.config/goose/config.yaml" });
  const shown = agentsPage([agent("zed", "Zed", "installed"), goose],
    { sessions: [glanceSession("w1", "goose", { tabs: 1 })], inUse: ["goose"] });
  assert.deepEqual(shown.rail().wearing("agents-group-head").map((head) => head.children[0].textContent), ["Connected"]);
  assert.equal(shown.el("integrations-heading").innerHTML, "Ghostlight is in <em>all 2</em> of your agents.");
  assert.equal(shown.title(), "goose", "the agent with tabs");
  assert.equal(shown.pane().wearing("agent-state")[0].textContent, "Connected");
  assert.match(shown.pane().wearing("agent-lead")[0].textContent, /^goose is using Ghostlight right now\./);
  assert.equal(shown.pane().wearing("agent-live").length, 1);
  assert.deepEqual(shown.pane().wearing("agent-act"), [], "nothing to do next");
  assert.equal(shown.pane().wearing("agent-remove").length, 0, "Ghostlight never edits its file, so it offers no Remove");
  assert.ok(!shown.el("agents-pane").innerHTML.includes("not connected"));
  assert.equal(shown.el("integrations-caption").textContent, "Nothing to set up.");
});

test("Remove asks first, naming the sessions that use Ghostlight now", () => {
  const shown = agentsPage(ROSTER, { sessions: [glanceSession("w1", "Codex"), glanceSession("w2", "Codex", { tabs: 1 })] });
  shown.pick("codex");
  const remove = shown.pane().wearing("agent-remove")[0];
  assert.equal(remove.dataset.harnessSessions, "2");
  assert.equal(remove.dataset.harnessAction, "uninstall");
  const asked = Words.confirmRemovalWords("Codex", "config.toml", 2);
  assert.equal(asked.title, "Remove Ghostlight from Codex?");
  assert.match(asked.detail, / 2 Codex sessions are using Ghostlight right now\. /);
  assert.ok(!Words.confirmRemovalWords("Codex", "config.toml", 0).detail.includes("session"));
});

test("the headline counts the agents with Ghostlight, and Set up everything says beforehand what it will do", () => {
  const head = (rows) => {
    const said = Words.agentHeadline(rows.map((row) => ({ ...row, standing: Words.agentStanding(row, false) })));
    return `${said.lead}${said.accent}${said.trail}`;
  };
  const on = (count) => Array.from({ length: count }, (_, at) => agent(`a${at}`, `A${at}`, "installed"));
  assert.equal(head(on(7)), "Ghostlight is in all 7 of your agents.");
  assert.equal(head(on(1)), "Ghostlight is in A0.");
  assert.equal(head([...on(2), agent("c", "Cursor", "available")]), "Ghostlight is in 2 of your 3 agents.");
  assert.equal(head([agent("c", "Cursor", "available")]), "Ghostlight is not in Cursor yet.");
  assert.equal(head([agent("c", "C", "available"), agent("d", "D", "available")]), "Ghostlight is in none of your 2 agents yet.");
  assert.equal(head([agent("c", "C", "not_detected")]), "Add Ghostlight to your agent.");
  const shown = agentsPage(ROSTER);
  assert.equal(shown.el("integrations-heading").innerHTML, "Ghostlight is in <em>2 of your 5</em> agents.");
  assert.equal(shown.el("integrations-caption").textContent, "Will connect Cursor and update Kiro. Junie is left for you.");
  assert.equal(shown.el("setup-integrations").disabled, false);
  assert.equal(Words.agentPlan({ connect: [], update: [], left: [] }), "Nothing to set up.");
  assert.equal(Words.agentPlan({ connect: ["A", "B", "C", "D"], update: [], left: ["goose"] }),
    "Will connect 4 agents. goose is left for you.");
  const done = agentsPage(on(2));
  assert.equal(done.el("setup-integrations").disabled, true);
  assert.equal(done.el("integrations-caption").textContent, "Nothing to set up.");
});

test("the page offers one Copy MCP command, in its header, with the command shown small", () => {
  const shown = agentsPage(ROSTER);
  assert.equal(shown.el("copy-command").hidden, false);
  assert.equal(shown.el("copy-command").dataset.harnessOperation, "copy");
  assert.equal(shown.el("copy-command").dataset.copyKind, Transport.COPY_KIND.command);
  assert.equal(shown.el("copy-command").dataset.harness, "zed");
  assert.equal(shown.el("integrations-command").textContent, "/home/ann/.ghostlight/bin/ghostlight mcp");
  for (const row of ROSTER) {
    shown.pick(row.id);
    assert.ok(!shown.el("agents-pane").innerHTML.includes("Copy MCP command"), row.id);
  }
  assert.ok(!shown.el("agents-list").innerHTML.includes("Copy MCP command"));
});

test("a watching screen sees every agent's state and copies, and is told once where changes are made", () => {
  const watcher = {
    context: "watch", opener: "anchor", word: "Watching", sentence: WATCHER, product: { version: "2.0.0" },
    acts: Object.fromEntries(Object.values(Transport.ACT).map((act) => [act,
      [Transport.ACT.rescan, Transport.ACT.hold].includes(act) ? { available: true, why_not: null } : { available: false, why_not: WHY }]))
  };
  const rows = ROSTER.map((row) => ({ ...row, download_url: `https://example.test/${row.id}` }));
  const shown = agentsPage(rows, { context: watcher });
  assert.equal(shown.el("integrations-continue").hidden, false);
  assert.ok(shown.el("integrations-continue").innerHTML.includes(WHY));
  for (const row of rows) {
    shown.pick(row.id);
    const html = shown.el("agents-pane").innerHTML;
    assert.ok(!/data-harness-operation="(manage|locate|download)"/.test(html), `${row.id}: ${html}`);
    assert.ok(html.includes(">Copy setup</button>"), row.id);
  }
  shown.pick("crush");
  assert.ok(shown.el("agents-pane").innerHTML.includes('href="https://example.test/crush"'), "Install is the product's own link");
});

test("only the rail and the pane scroll, the entry has no scroll of its own, and agents in use pulse unless motion is reduced", () => {
  const top = (selector) => naming(selector).filter((rule) => rule.media === null);
  assert.equal(top(".agents-list")[0].declarations["overflow-y"], "auto");
  assert.equal(top(".agents-pane")[0].declarations["overflow-y"], "auto");
  const doc = top(".agent-doc")[0].declarations;
  assert.equal(doc.overflow, undefined);
  assert.equal(doc["max-height"], undefined);
  assert.equal(doc["white-space"], "pre-wrap");
  assert.match(top(".agent-dot.d-live")[0].declarations.animation, /^glyph-breathe /);
  assert.match(top(".agent-dot.d-live::after")[0].declarations.animation, /^halo-breathe /);
  const still = naming("*").filter((rule) => rule.media === "(prefers-reduced-motion: reduce)");
  assert.ok(still.some((rule) => /^none/.test(rule.declarations.animation)));
  assert.ok(naming(".agent-entry[aria-current=\"true\"]").some((rule) => /forced-colors/.test(rule.media ?? "")),
    "the picked agent takes the system highlight under forced colours");
});

test("a settled action says the service's sentence, the readiness note beside it, and the host once", () => {
  const shown = drawing();
  const loading = Entries.entryFromRecord(settledRead({
    observed: { host: "example.com", readiness: "loading", count: 1240 }
  }), null);
  shown.view.hero(loading, false);
  const hero = drawn(shown.el("hero-body").innerHTML);
  assert.deepEqual(hero.wearing("hero-activity").map((line) => line.textContent), ["Read 1,240 words from example.com."]);
  assert.ok(hero.wearing("hero-meta")[0].textContent.includes(Words.READINESS_NOTE.loading));
  assert.equal(hero.root.children.map((node) => node.textContent).join("").split("example.com").length - 1, 1,
    "the sentence names the host, so nothing else repeats it");
});

test("every readiness the service reports reads beside the row's sentence, and the doubtful ones mark its duration", () => {
  // Readiness in work/result.rs, as it serializes, and the note a settled row carries for each.
  const notes = { not_applicable: "", complete: "", interactive: "interactive", loading: "never settled", unknown: "readiness unknown" };
  assert.deepEqual(Object.keys(Words.READINESS_NOTE).sort(), Object.keys(notes).sort());
  const sentence = "Read 1,240 words from example.com.";
  for (const [readiness, note] of Object.entries(notes)) {
    const shown = drawing();
    const entry = Entries.entryFromRecord(settledRead({
      invocation: `read-${readiness}`, observed: { host: "example.com", readiness }
    }), null);
    const newest = Entries.entryFromRecord(settledRead({ invocation: "newest" }), null);
    shown.view.rebuildFeed([newest, entry]);
    const [row] = queueRows(shown);
    const cells = drawn(row.innerHTML);
    assert.equal(cells.wearing("row-activity")[0].textContent, note ? `${sentence} (${note})` : sentence, readiness);
    assert.equal(cells.wearing("row-dur")[0].classes.has("unsettled"), readiness === "loading" || readiness === "unknown",
      readiness);
  }
  assert.ok(naming(".row-dur.unsettled").length > 0, "the stylesheet gives the marked duration no treatment");
});

/** The cells of the one row below the hero, for `record` as the service sends it. */
function rowOf(record, options) {
  const shown = drawing(options);
  shown.view.rebuildFeed([
    Entries.entryFromRecord(settledRead({ invocation: "newest" }), null),
    Entries.entryFromRecord(record, null)
  ]);
  const [row] = queueRows(shown);
  const cells = drawn(row.innerHTML);
  const cell = (className) => cells.wearing(className)[0];
  return { shown, cells, cell, text: cells.root.children.map((node) => node.textContent).join(" ") };
}

test("a tool reads without the shared browser_ prefix, in words, and a tool without it in words too", () => {
  assert.equal(Words.toolWords("browser_navigate"), "navigate");
  assert.equal(Words.toolWords("browser_fill_form"), "fill form");
  assert.equal(Words.toolWords("browser_press_key"), "press key");
  assert.equal(Words.toolWords("policy_explain"), "policy explain");
  const catalog = JSON.parse(readFileSync(join(ui, "..", "..", "..", "docs", "1.0", "catalog.json"), "utf8"));
  for (const { name } of catalog) {
    const shown = Words.toolWords(name);
    assert.ok(shown && !shown.startsWith("browser") && !shown.includes("_"), `${name} reads ${shown}`);
  }
});

test("a row names its tool in words, the exact name on hover and among the detail card's facts", () => {
  const { cell, cells } = rowOf(settledRead({ invocation: "fill-1", tool: "browser_fill_form", capability: "write" }));
  assert.equal(cell("row-tool").textContent, "fill form");
  assert.equal(cell("row-tool").attributes.title, "browser_fill_form");
  const meta = cells.wearing("hero-meta")[0];
  assert.ok(meta.textContent.includes("browser_fill_form"), meta.textContent);
  assert.equal(cells.wearing("hero-tool")[0].attributes.title, "browser_fill_form");
});

test("an action the service names no agent for says Unknown agent, its workspace id only on hover", () => {
  const workspace = "workspace_0123456789abcdef825";
  const { cell, text } = rowOf(settledRead({ invocation: "unknown-1", workspace, provenance: null, agent: namedAgent("Unknown agent") }));
  assert.equal(cell("row-client").textContent, "Unknown agent");
  assert.equal(cell("row-client").attributes.title, workspace);
  assert.ok(!text.includes("workspace_"), `a raw id is printed in the row: ${text}`);
});

test("an action names the agent its record names, with no hover id", () => {
  const { cell } = rowOf(settledRead({ invocation: "codex-1", agent: namedAgent("Codex"), provenance: {
    connection_id: "connection_1", reported_application: "codex-mcp-client", channel: "mcp",
    explanation: "\"codex-mcp-client\" is what the agent calls itself; Ghostlight does not verify it."
  } }));
  assert.equal(cell("row-client").textContent, "Codex");
  assert.equal(cell("row-client").attributes.title, undefined);
});

test("the intake is marked only for the command line, never for MCP", () => {
  const mcp = rowOf(settledRead({ invocation: "mcp-1", channel: "mcp" }));
  assert.equal(mcp.cell("row-channel").textContent, "");
  assert.equal(mcp.cell("row-channel").children.length, 0);
  const cli = rowOf(settledRead({ invocation: "cli-1", channel: "cli" }));
  assert.equal(cli.cell("row-channel").textContent, "Command line");
  assert.equal(cli.cell("row-channel").attributes.title, "Command line");
  assert.ok(cli.cells.wearing("sr-only").length > 0, "the mark's words are for a screen reader");
  assert.ok(naming(".row-channel svg").length > 0, "the stylesheet sizes the mark");
});

test("a call refused before it reached a browser shows no browser, though its session works in one", () => {
  const session = { id: "w1", agent: namedAgent("Codex"), channel: "mcp", browser: "Chromium" };
  const refused = rowOf(settledRead({
    invocation: "refused-1", allowed: false, reason: "browser_disconnected", status: "failed",
    summary: "The browser this session works in is not connected. Nothing was sent."
  }), { sessionFor: () => session });
  assert.equal(refused.cell("row-browser").textContent, "");
  const reached = rowOf(settledRead({ invocation: "reached-1", browser: "Google Chrome" }), { sessionFor: () => session });
  assert.equal(reached.cell("row-browser").textContent, "Google Chrome");
});

test("the hero names the command line once", () => {
  const shown = drawing();
  shown.view.hero(Entries.entryFromRecord(settledRead({ invocation: "call-1", channel: "cli", agent: namedAgent("Command line"), provenance: {
    connection_id: "connection_1", reported_application: null, channel: "cli",
    explanation: "A ghostlight call session is a local program."
  } }), null), false);
  const meta = drawn(shown.el("hero-body").innerHTML).wearing("hero-meta")[0].textContent;
  assert.equal(meta.split("Command line").length - 1, 1, meta);
  assert.ok(!/via|cli/.test(meta), meta);
});

test("a managed policy's passport shows every signed fact", () => {
  const shown = drawing();
  shown.view.policy({
    headline: "Example Org sets the rules.",
    organization: null,
    capabilities: [],
    ceilings: [],
    layers: [],
    user_layer: { source: "managed", editable: false, path: null, blocked_reason: null },
    passport: {
      configured: true,
      organization: "Example Org",
      rationale: "Support work only.",
      contacts: [{ kind: "email", label: "Service desk", value: "desk@example.test" }],
      sequence: 42,
      freshness: "fresh",
      source_class: "https",
      last_success_ms: 1_700_000_000_000
    }
  });
  assert.deepEqual(shown.failures, []);
  const card = shown.el("policy-organization");
  assert.equal(card.hidden, false);
  const text = drawn(card.innerHTML).root.children.map((node) => node.textContent).join(" ");
  for (const fact of ["Example Org", "Support work only.", "Service desk", "desk@example.test", "fresh",
    "verified sequence 42", "from an HTTPS source", "last checked"]) {
    assert.ok(text.includes(fact), `the passport does not show ${fact}: ${text}`);
  }
});

// WorkbenchDestination in workbench/mod.rs, less get_chrome, which only the setup view offers. The
// Rust table's own test pins the same names.
const PRODUCT_DESTINATIONS = [
  "home", "demo", "decision_aid", "source", "install_guide", "governance_guide", "scripting_guide",
  "audit_guide", "trust_center", "decision_records", "licensing_guide", "sylin_tools"
];

test("About offers every destination the product chose, by name, and holds no address", () => {
  const shown = drawing();
  shown.view.armCard();
  const links = shown.el("about-links").innerHTML;
  const named = [
    ...drawn(links).root.descendants(),
    ...shown.document.querySelectorAll("[data-destination]")
  ].map((node) => node.dataset.destination).filter(Boolean);
  assert.deepEqual([...new Set(named)].sort(), [...PRODUCT_DESTINATIONS].sort());
  assert.ok(!/href=|https?:/.test(links), "a destination is a name, never an address");
});

test("each catalog tool wears the medallion chosen for it", () => {
  // The tools of docs/1.0/catalog.json and the medallion a person sees on each one's row.
  const chosen = {
    browser_tabs: "navigate", browser_navigate: "navigate", browser_history: "navigate", browser_window: "scan",
    browser_read: "scan", browser_inspect: "scan", browser_find: "scan", browser_screenshot: "camera",
    browser_click: "pointer", browser_scroll: "navigate", browser_hover: "pointer", browser_fill_form: "keyboard",
    browser_type_text: "keyboard", browser_press_key: "keyboard", browser_drag: "pointer", browser_wait: "wait",
    browser_dialog: "wait", browser_upload: "workwheel", browser_execute: "workwheel", browser_flow: "workwheel",
    browser_record: "camera", browser_diagnose: "scan", policy_explain: "scan"
  };
  const catalog = JSON.parse(readFileSync(join(ui, "..", "..", "..", "docs", "1.0", "catalog.json"), "utf8"));
  assert.deepEqual(catalog.map((tool) => tool.name).sort(), Object.keys(chosen).sort(),
    "a new tool needs a medallion chosen for it");
  for (const [tool, glyph] of Object.entries(chosen)) {
    assert.ok(Words.GLYPHS[glyph], glyph);
    assert.equal(Words.glyphFor({ tool }), Words.GLYPHS[glyph], tool);
  }
});

/* -------------------------------- the stylesheets -------------------------------- */

test("every cell of a row has a column at every width, and the details span the row below them", () => {
  const shown = drawing();
  shown.view.rebuildFeed([
    Entries.entryFromRecord(settledRead({ invocation: "newest" }), null),
    Entries.entryFromRecord(settledRead(), null)
  ]);
  const [row] = queueRows(shown);
  const children = drawn(row.innerHTML).root.children;
  const details = children.filter((cell) => cell.classes.has("row-history"));
  const cells = children.filter((cell) => !cell.classes.has("row-history"));
  assert.equal(details.length, 1);
  assert.ok(cells.length >= 6, `a row of cells, saw ${cells.length}`);

  const widths = [Infinity, ...new Set(naming(".row").map((rule) => widthOf(rule.media)).filter(Boolean))];
  assert.ok(widths.length >= 3, "the row is laid out again at narrower widths");
  for (const width of widths) {
    const rules = applyingAt(width);
    const grid = naming(".row", rules).filter((rule) => rule.declarations["grid-template-columns"]).at(-1);
    const hidden = new Set(rules.filter((rule) => rule.declarations.display === "none")
      .flatMap((rule) => rule.selectors).filter((selector) => /^\.row-[a-z-]+$/.test(selector)).map((selector) => selector.slice(1)));
    const showing = cells.filter((cell) => ![...cell.classes].some((name) => hidden.has(name)));
    assert.equal(tracksIn(grid.declarations["grid-template-columns"]), showing.length,
      `at ${width}px the row declares ${grid.declarations["grid-template-columns"]} for ${showing.length} cells`);
    const span = naming(".row-history", rules).map((rule) => rule.declarations["grid-column"]).filter(Boolean).at(-1);
    assert.match(span, /\/ -1$/, `at ${width}px the details do not span to the row's end`);
  }
});

test("every capability class has its own treatment, and a mixed one takes the most consequential", () => {
  // Capability in governance/mod.rs, as it serializes.
  for (const capability of ["read", "action", "write", "execute"]) {
    const name = Words.capabilityClass({ capability });
    assert.equal(name, `cap-${capability}`);
    assert.ok(naming(`.${name}`).length > 0, `the ${capability} capability has no treatment`);
  }
  assert.equal(Words.capabilityClass({ capability: "read + write" }), "cap-write");
  assert.equal(Words.capabilityClass({}), "cap-read");
});

test("with forced colours the band, the tabs and the rail keep their edges, and a selection takes the system highlight", () => {
  const forced = RULES.filter((rule) => rule.media === "(forced-colors: active)");
  const declared = (selector, property) => naming(selector, forced).map((rule) => rule.declarations[property] ?? "").join(" ");
  for (const selector of [".lampband", ".state-word", ".tab", ".tab.active", ".rail-entry", ".rail-entry.active", ".rail-state"]) {
    assert.ok(naming(selector, forced).length > 0, `forced colours leave ${selector} unstyled`);
  }
  for (const selector of [".tab", ".rail-entry"]) assert.match(declared(selector, "border"), /ButtonBorder/, selector);
  for (const selector of [".tab.active", ".rail-entry.active"]) {
    assert.match(declared(selector, "background"), /^Highlight$/, selector);
    assert.match(declared(selector, "color"), /^HighlightText$/, selector);
  }
});

test("About wears the published card and the product's own artwork, at a whole multiple of its pixels", () => {
  const shown = page();
  const card = shown.el("about-card");
  assert.equal(card.tag, "button");
  assert.ok(card.classes.has("tcg"), "the card keeps its own root, apart from the diagnostic cards");
  assert.ok(card.closest("#view-about"));
  const parts = [...card.descendants()];
  for (const name of ["card-art", "card-divider", "card-disc", "card-notch", "card-pane", "card-title", "card-rules",
    "card-foot", "holo2"]) {
    assert.ok(parts.some((node) => node.classes.has(name)), `the card is missing ${name}`);
  }
  const mascot = parts.find((node) => node.classes.has("mascot"));
  assert.equal(mascot.attributes.src, "ghostlight.png");
  assert.ok(mascot.classes.has("px"));
  assert.ok(RULES.some((rule) => rule.selectors.some((selector) => selector.startsWith(".tcg"))
    && rule.declarations["mix-blend-mode"] === "color-dodge"), "the foil layer lost its blend");

  // The size comes from the artwork itself: a different sprite fails here until the card matches.
  const sprite = readFileSync(join(ui, "ghostlight.png"));
  assert.equal(sprite.subarray(1, 4).toString("latin1"), "PNG");
  const native = sprite.readUInt32BE(16);
  assert.equal(sprite.readUInt32BE(20), native, "the card art assumes a square sprite");
  const rendered = Number(/^(\d+)px$/.exec(naming(".tcg .card-art .mascot")[0].declarations.width)[1]);
  assert.ok(rendered >= native, "the sprite is drawn below its own size");
  assert.equal(rendered % native, 0, `${rendered}px from a ${native}px sprite splits its pixels unevenly`);
  assert.equal(naming(".tcg img.px")[0].declarations["image-rendering"], "pixelated", "the sprite is smoothed");
});
