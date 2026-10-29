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
import { readdirSync, readFileSync } from "node:fs";
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
require(join(ui, "lib", "agents-view.js"));
require(join(ui, "lib", "policy-view.js"));
const View = require(join(ui, "lib", "view.js"));
/** The source of everything the view draws: the view and the panes it delegates to. */
const VIEW_SOURCE = ["view.js", "agents-view.js", "policy-view.js"]
  .map((name) => readFileSync(join(ui, "lib", name), "utf8")).join("\n");
const markup = readFileSync(join(ui, "index.html"), "utf8");
const styles = readFileSync(join(ui, "styles.css"), "utf8") + readFileSync(join(ui, "kit", "kit.css"), "utf8");

const WHY = "Change this in the Ghostlight window.";
const WATCHING_OFF = "Watching is off on this computer. Turn it on in the Ghostlight window.";
const WATCHED = "Others on this computer can watch this view in a browser.";
const PRIVATE = "Watching is off: only this window shows Ghostlight.";
const WATCHER = "Anyone on this computer can open this view. Changes are made in the Ghostlight window.";
const SECTIONS = ["screen", "sharing", "browser-work", "notifications", "diagnostics"];

/** The acts each runtime state accepts, in the order governance::offers names them. */
const OFFERS = Object.freeze({ active: ["hold", "end_session"], held: ["resume", "end_session"], ended: ["start_session"] });

/** The part as the service shapes it: the window may do every act, a watcher may only pause. */
function part({ context = "control", watch = true, runtime = "active", layer = "marker", url = "http://127.0.0.1:41234/" } = {}) {
  const control = context === "control";
  const act = (watcherMay = false) => (control || watcherMay
    ? { available: true, why_not: null } : { available: false, why_not: WHY });
  const on = layer !== "off";
  return {
    runtime: {
      label: "Browser work", state: runtime, word: { active: "Running", held: "Paused", ended: "Session ended" }[runtime],
      note: runtime === "held" ? "Paused from the tray." : null, offers: OFFERS[runtime],
      acts: { hold: act(true), resume: act(), start_session: act(), end_session: act() }
    },
    watching: {
      label: "Others can watch", watch, url, word: watch ? "On: anyone on this computer can open this view" : "Off: only this window shows Ghostlight",
      acts: { set_watching: act() }
    },
    diagnostics: {
      label: "Process diagnostics", on, layer, used_bytes: 2048, folder: "~/.ghostlight/logs",
      word: on ? `On (${layer})` : "Off", acts: { set_diagnostics: act(), reveal_diagnostics: act() }
    },
    notifications: { label: "Notifications", state: "On", word: "Send one local checkup notification.", acts: { test_notification: act() } },
    browser: {
      set_in: "Set in the browser",
      effects: { label: "Agent activity effects", word: "The cursor, click ripples and scan-lines on a page an agent works in. They start on." },
      captions: { label: "Action captions", word: "A caption at the bottom of the page names each action. They start off." }
    },
    service: { word: "Ghostlight is running; 1 agent connected." },
    you: { context, word: control ? (watch ? "Shared" : null) : "Watching", sentence: control ? (watch ? WATCHED : PRIVATE) : WATCHER }
  };
}

const VOID = new Set(["meta", "link", "img", "input", "br", "hr", "source", "wbr"]);

/** A fake element that knows its place in the tree, so `closest` answers like a browser's. */
class Element {
  constructor(tag, attributes = {}) {
    this.tag = tag;
    this.id = attributes.id ?? "";
    this.hidden = "hidden" in attributes;
    this.disabled = "disabled" in attributes;
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
      if (["aria-current", "aria-expanded", "aria-hidden", "aria-labelledby", "aria-describedby", "role", "open", "src", "title"]
        .includes(name)) this.attributes[name] = value;
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
      remove: (...names) => names.forEach((name) => this.classes.delete(name))
    };
  }

  /** The class attribute, read and written as one string, as a browser's is. */
  get className() {
    return [...this.classes].join(" ");
  }

  set className(value) {
    this.classes = new Set(String(value).split(/\s+/).filter(Boolean));
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

  prepend(...nodes) {
    for (const node of nodes) node.parent = this;
    this.children.unshift(...nodes);
  }

  replaceChildren(...nodes) {
    this.children = [];
    this.append(...nodes);
  }

  /** Leave the tree. As in a browser, focus held inside what leaves falls back to the body. */
  remove() {
    if (this.parent) this.parent.children = this.parent.children.filter((child) => child !== this);
    if (Element.focused && this.contains(Element.focused)) Element.focused = null;
  }

  get lastElementChild() {
    return this.children.at(-1) ?? null;
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

  /** Take focus, as a browser does for a control that is not disabled. */
  focus() {
    if (!this.disabled) Element.focused = this;
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
    },
    removeEventListener: (type, handler) => {
      listeners.set(type, (listeners.get(type) ?? []).filter((each) => each !== handler));
    }
  };
  const dispatch = (type, event) => {
    for (const handler of [...(listeners.get(type) ?? [])]) handler(event);
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
    "controls-hold", "controls-resume", "controls-start", "controls-end", "controls-watching-copy",
    "controls-watching-toggle", "controls-diagnostics-reveal", "controls-diagnostics-toggle", "test-notification"
  ].filter((id) => !shown.el(id).hidden);
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

const said = (shown, id) => shown.el(id).textContent;

/* ------------------------------- stylesheet rules ------------------------------ */

/**
 * The rules of a stylesheet, each with the media or container condition it sits under (null at the
 * top level), its selectors and its declarations by property. Comments are dropped.
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
      const condition = /^@(media|container)\s/.exec(prelude);
      if (condition) {
        walk(body, prelude.slice(condition[0].length).trim());
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

/**
 * The widest width of the window's `shell` container a condition applies up to, or null when it is
 * not a shell width at all. Every width rule asks the shell, so Large text moves the layout with the
 * room the shell has.
 */
const widthOf = (media) => Number(/^shell \(max-width: (\d+)px\)$/.exec(media ?? "")?.[1] ?? NaN) || null;

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
  for (const id of ["test-notification", "settings-quit", "status-service", "controls-runtime", "controls-watching",
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
  assert.equal(said(shown, "rail-screen-state"), "Comfortable, standard text, motion follows the system");
  assert.equal(said(shown, "rail-sharing-state"), "On: anyone on this computer can open this view");
  assert.equal(said(shown, "rail-browser-work-state"), "Paused");
  assert.equal(said(shown, "rail-notifications-state"), "On");
  assert.equal(said(shown, "rail-diagnostics-state"), "On (marker)");
  assert.equal(shown.el("rail-diagnostics-state").title, "On (marker)");

  shown.settings.render(part({ watch: false, layer: "off", runtime: "ended" }));
  assert.equal(said(shown, "rail-sharing-state"), "Off: only this window shows Ghostlight");
  assert.equal(said(shown, "rail-browser-work-state"), "Session ended");
  assert.equal(said(shown, "rail-diagnostics-state"), "Off");
});

test("This screen names the browser's effects and captions as set in the browser, and claims no value", () => {
  const shown = kit();
  shown.settings.render(part());
  for (const key of ["effects", "captions"]) {
    const row = shown.el(`controls-${key}`);
    assert.equal(row.hidden, false, key);
    assert.ok(row.closest("#settings-screen"), `${key} is not on This screen`);
    assert.equal(said(shown, `controls-${key}-where`), "Set in the browser");
    assert.ok(shown.el(`controls-${key}-where`).classes.has("context-chip"));
    assert.equal([...row.descendants()].some((node) => node.tag === "button" || node.tag === "input"), false,
      `${key} is a line here, never a control`);
  }
  assert.equal(said(shown, "controls-effects-label"), "Agent activity effects");
  assert.equal(said(shown, "controls-captions-label"), "Action captions");
  assert.match(said(shown, "controls-effects-word"), /They start on\.$/);
  assert.match(said(shown, "controls-captions-word"), /They start off\.$/);
  shown.settings.closed(WATCHING_OFF);
  assert.equal(shown.el("controls-effects").hidden, true, "a screen that shows nothing names nothing of the browser");
  assert.equal(shown.el("controls-captions").hidden, true);
});

test("the rail's word for This screen follows the personal settings", () => {
  const shown = kit();
  const choice = (name, value) => shown.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === name && button.dataset.value === value);
  shown.click(choice("density", "compact"));
  assert.equal(said(shown, "rail-screen-state"), "Compact, standard text, motion follows the system");
  shown.click(choice("motion", "reduced"));
  assert.equal(said(shown, "rail-screen-state"), "Compact, standard text, motion reduced");
  shown.click(choice("text", "large"));
  assert.equal(said(shown, "rail-screen-state"), "Compact, large text, motion reduced");
  assert.equal(Words.screenState("comfortable", "standard", "reduced"), "Comfortable, standard text, motion reduced");
});

test("the sections say what the part says, in the service's words", () => {
  const shown = kit();
  shown.settings.render(part({ runtime: "held" }));
  assert.equal(said(shown, "controls-runtime-label"), "Browser work");
  assert.equal(said(shown, "controls-runtime-word"), "Paused");
  assert.equal(said(shown, "controls-runtime-note"), "Paused from the tray.");
  assert.equal(said(shown, "controls-watching-label"), "Others can watch");
  assert.equal(said(shown, "controls-watching-word"), "On: anyone on this computer can open this view");
  assert.equal(said(shown, "controls-watching-url"), "http://127.0.0.1:41234/");
  assert.equal(said(shown, "controls-diagnostics-word"), "On (marker)");
  assert.equal(said(shown, "controls-diagnostics-place"), "~/.ghostlight/logs (2.0 KB)");
  assert.equal(said(shown, "controls-notifications-label"), "Notifications");
});

test("the page's own labels fill the rail, the headings and the buttons", () => {
  const shown = kit();
  assert.deepEqual(SECTIONS.map((section) => shown.el(`rail-${section}`).children[0].textContent),
    ["This screen", "Sharing", "Browser work", "Notifications", "Diagnostics"]);
  assert.equal(said(shown, "settings-browser-work-title"), "Browser work");
  assert.equal(said(shown, "controls-hold"), "Pause agents");
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
    "controls-hold", "controls-end", "controls-watching-copy", "controls-watching-toggle",
    "controls-diagnostics-reveal", "controls-diagnostics-toggle", "test-notification"
  ]);
  for (const section of SECTIONS.slice(1)) assert.equal(shown.el(`settings-${section}-continue`).hidden, true, section);
  assert.equal(said(shown, "controls-watching-toggle"), "Turn off");
  assert.equal(said(shown, "controls-diagnostics-toggle"), "Turn off");

  shown.click(shown.el("controls-watching-toggle"));
  shown.click(shown.el("controls-diagnostics-toggle"));
  shown.click(shown.el("controls-diagnostics-reveal"));
  shown.click(shown.el("test-notification"));
  shown.click(shown.el("controls-watching-copy"));
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

test("Settings shows only the runtime acts the controls part offers, never its own table's", () => {
  const shown = kit();
  // The page keeps no table of which act a state accepts: what the service offers is what shows.
  const ended = part({ runtime: "ended" });
  shown.settings.render({ ...ended, runtime: { ...ended.runtime, offers: ["resume", "start_session"] } });
  assert.deepEqual(runtimeShowing(shown), ["controls-resume", "controls-start"]);
  shown.settings.render({ ...ended, runtime: { ...ended.runtime, offers: [] } });
  assert.deepEqual(runtimeShowing(shown), []);
  // An offered act this screen may not do stays hidden.
  shown.settings.render(part({ context: "watch", runtime: "active" }));
  assert.deepEqual(runtimeShowing(shown), ["controls-hold"]);
});

test("a switch that is off offers Turn on", async () => {
  const shown = kit();
  shown.settings.render(part({ watch: false, layer: "off" }));
  assert.equal(said(shown, "controls-watching-toggle"), "Turn on");
  assert.equal(said(shown, "controls-diagnostics-toggle"), "Turn on");
  assert.equal(said(shown, "controls-diagnostics-word"), "Off");
  shown.click(shown.el("controls-watching-toggle"));
  shown.click(shown.el("controls-diagnostics-toggle"));
  await settle();
  assert.deepEqual(shown.calls, [["setWatching", true], ["setDiagnostics", true]]);
});

test("in Control with no watch view served, the state is told once and nothing sends the window to itself", () => {
  const shown = kit();
  const unserved = part();
  unserved.watching = {
    ...unserved.watching, watch: false, url: null, word: "Not serving a watch view.",
    acts: { set_watching: { available: false, why_not: "Not serving a watch view." } }
  };
  shown.settings.render(unserved);
  assert.equal(said(shown, "controls-watching-word"), "Not serving a watch view.");
  assert.equal(shown.el("controls-watching-toggle").hidden, true);
  assert.equal(shown.el("controls-watching-copy").hidden, true);
  assert.equal(shown.el("settings-sharing-continue").hidden, true);
  assert.equal(shown.el("controls-hold").hidden, false);
});

test("no copy button is drawn on a screen that cannot copy", () => {
  const shown = kit({ copyText: undefined });
  shown.settings.render(part());
  assert.equal(shown.el("controls-watching-copy").hidden, true);
  assert.equal(shown.el("controls-watching-url").hidden, false);
});

test("a button cannot be pressed twice while its act is still running", async () => {
  let release;
  const shown = kit({ setWatching: (watch) => new Promise((resolve) => { release = () => resolve(watch); }) });
  shown.settings.render(part());
  const toggle = shown.el("controls-watching-toggle");
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
  assert.equal(said(shown, "controls-watching-word"), "On: anyone on this computer can open this view");
  assert.equal(said(shown, "controls-diagnostics-word"), "On (marker)");
  assert.equal(said(shown, "rail-sharing-state"), "On: anyone on this computer can open this view");

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
  assert.equal(said(shown, "rail-screen-state"), "Compact, standard text, motion follows the system");
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
  assert.equal(said(shown, "rail-screen-state"), "Comfortable, standard text, motion follows the system");

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

test("density, text size and motion are kept in the browser and dress the page again after a reload", () => {
  const storage = memory();
  const first = kit({ storage });
  assert.deepEqual(first.settings.personal(), { density: "comfortable", text: "standard", motion: "system" });
  assert.deepEqual(chosen(first, "density"), ["comfortable"]);
  assert.equal(first.body.classes.has("density-compact"), false);

  const compact = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "density" && button.dataset.value === "compact");
  const reduced = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "motion" && button.dataset.value === "reduced");
  const large = first.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.personal === "text" && button.dataset.value === "large");
  first.settings.render(part());
  assert.equal(first.body.classes.has("text-large"), false);
  first.click(compact);
  first.click(large);
  first.click(reduced);
  assert.equal(first.body.classes.has("density-compact"), true);
  assert.equal(first.body.classes.has("text-large"), true);
  assert.equal(first.body.classes.has("motion-reduced"), true);
  assert.deepEqual(chosen(first, "density"), ["compact"]);
  assert.deepEqual(chosen(first, "text"), ["large"]);
  assert.deepEqual(JSON.parse(storage.kept[Settings.STORAGE_KEY]), { density: "compact", text: "large", motion: "reduced" });
  assert.deepEqual(first.calls, [], "a personal setting is never sent anywhere");

  const reloaded = kit({ storage });
  assert.deepEqual(reloaded.settings.personal(), { density: "compact", text: "large", motion: "reduced" });
  assert.equal(reloaded.body.classes.has("density-compact"), true);
  assert.equal(reloaded.body.classes.has("text-large"), true);
  assert.equal(reloaded.body.classes.has("motion-reduced"), true);
  assert.equal(said(reloaded, "rail-screen-state"), "Compact, large text, motion reduced");

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
  assert.equal(said(blocked, "controls-watching-label"), "Others can watch");
  assert.deepEqual(blocked.settings.personal(), { density: "comfortable", text: "standard", motion: "system" });
  const compact = blocked.document.querySelectorAll("[data-personal]")
    .find((button) => button.dataset.value === "compact");
  blocked.click(compact);
  assert.equal(blocked.body.classes.has("density-compact"), true, "the choice still applies to this screen");

  for (const kept of ["not json", '{"density":"huge","text":"huge","motion":3}', "null", "[]"]) {
    const rubbish = kit({ storage: memory({ [Settings.STORAGE_KEY]: kept }) });
    assert.deepEqual(rubbish.settings.personal(), { density: "comfortable", text: "standard", motion: "system" }, kept);
  }
  const none = kit({ storage: null });
  assert.deepEqual(none.settings.personal(), { density: "comfortable", text: "standard", motion: "system" });
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
  word: null,
  sentence: PRIVATE,
  product: { version: "2.0.0", license: "Apache-2.0 OR MIT" },
  acts: Object.fromEntries(Object.values(Transport.ACT).map((act) => [act, { available: true, why_not: null }]))
};

/**
 * An agent as the service names it: an agent on the roster has its image address, and the
 * command line and an agent Ghostlight does not know wear their glyphs.
 */
function namedAgent(name) {
  if (name === "Command line") return { id: "command_line", name, symbol: { glyph: "command_line" } };
  if (name === "Unknown agent") return { id: "unknown", name, symbol: { glyph: "unknown" } };
  const id = name.toLowerCase().replaceAll(" ", "-");
  return { id, name, symbol: { icon: `agent-icons/${id}?v=fedcba987654` } };
}

/** One settled read, as the service records it. */
function settledRead(overrides = {}) {
  return {
    invocation: "read-1", workspace: "w1", tool: "browser_read", capability: "read", requirements: ["read"],
    allowed: true, reason: "permitted", status: "succeeded", outcome: "succeeded", effect: "none", complete: true,
    timestamp_ms: 1, duration_ms: 1200, summary: "Read 1,240 words from example.com.", storage: "saved", storage_detail: "",
    steps: [], permissions: { checks: [], truncated: false }, permission_explanations: [],
    observed: { host: "example.com", readiness: "complete", count: 1240 }, channel: "mcp",
    ...overrides
  };
}

/**
 * What the service counts of `sessions` and `browsers` (workbench `ConnectionCounts`): one agent
 * per agent id, the connections by intake, and the browsers.
 */
function countsOf(sessions, browsers) {
  const connections = sessions.flatMap((session) => session.connections ?? []);
  return {
    agents: new Set(sessions.map((session) => session.agent.id)).size,
    mcp_clients: connections.filter((connection) => connection.channel === "mcp").length,
    command_line: connections.filter((connection) => connection.channel === "cli").length,
    browsers: browsers.length
  };
}

/** The organization as the service names one that no policy names (workbench `OrganizationView`). */
const UNNAMED_ORGANIZATION = Object.freeze({ identity: null, title: "Your organization", in_sentence: "your organization" });

/** A snapshot answer from a service with one client, one browser and one settled read. */
function snapshotAnswer({ history = [settledRead()], sessions, service = { version: "2.0.0", started_at_ms: 1, runtime_state: "active" } } = {}) {
  const connected = sessions ?? [{ id: "w1", agent: namedAgent("Codex"), channel: "mcp", tab_count: 1, running_operations: 0 }];
  const browsers = [{ id: "b1", family: "Chrome", adapter_version: "2.0.0" }];
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
          managed_policy: { configured: false },
          organization: UNNAMED_ORGANIZATION
        }
      },
      connections: { sessions: connected, browsers, counts: countsOf(connected, browsers) },
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
    AbortController, TextDecoder, TextEncoder, URL, MutationObserver: class { observe() {} }
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
  assert.equal(said(shown, "alert-text"), Words.WINDOW_FAULT);
  assert.equal(shown.el("alert").hidden, false);
  assert.ok(shown.logged.some((line) => /^rendering the snapshot: /.test(line)), "the console keeps the cause");
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
  assert.equal(said(shown, "alert-text"), Words.WINDOW_FAULT, "the person reads the window's words, not the exception's");
  assert.equal(shown.el("alert").hidden, false);
  shown.fire("unhandledrejection", { reason: new Error("a promise broke") });
  assert.equal(said(shown, "alert-text"), Words.WINDOW_FAULT);
  assert.deepEqual(shown.logged, ["surface: a listener broke", "surface: a promise broke"],
    "the console hears each failure, in its own words");
});

test("a listener that cannot attach costs its own wiring, not the first snapshot", async () => {
  const before = (shown) => {
    shown.el("palette-query").addEventListener = () => {
      throw new Error("no listener here");
    };
  };
  const shown = await windowOver({ answer: healthy(), before });
  assert.equal(said(shown, "alert-text"), Words.WINDOW_FAULT);
  assert.ok(shown.logged.some((line) => /^wiring the surface: no listener here/.test(line)));
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
    id, agent: namedAgent(agent), channel, tab_count: tabs, running_operations: busy ? 1 : 0,
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
  answer.parts.connections.counts = countsOf(sessions, browsers);
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
  assert.ok(html().includes('<img data-icon="agent-icons/codex?v=fedcba987654" alt="" width="20" height="20">'));
  assert.ok(Words.symbolMarkup(codex, "rail").includes('data-icon="agent-icons/codex?v=fedcba987654"'), "the same address at every size");
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
    '<span class="symbol symbol-pane" aria-hidden="true"><img data-icon="agent-icons/codex?v=fedcba987654" alt="" width="46" height="46"></span>');
  for (const size of Object.keys(Words.SYMBOL_SIZES)) {
    assert.ok(naming(`.symbol-${size} img`).length > 0, `the stylesheet sizes a ${size} tile`);
  }
});

test("an agent's image is a whole tile the service made, and the page paints only a placeholder under one still loading", () => {
  assert.ok(naming(".symbol img:not([src])").some((rule) => rule.declarations.background === "var(--icon-placeholder)"),
    "an image still loading, or one that failed, shows the quiet placeholder");
  const appIcon = Words.symbolMarkup({ symbol: { icon: "agent-icons/zed?v=0123456789ab", about: "Icon from the installed Zed app." } }, "rail");
  assert.equal(appIcon, Words.symbolMarkup(namedAgent("Codex"), "rail").replace("agent-icons/codex?v=fedcba987654", "agent-icons/zed?v=0123456789ab"),
    "an app's icon and a monogram are drawn the same way");
  const grounds = RULES.filter((rule) => ["background", "background-color", "padding"].some((name) => name in rule.declarations)
    && rule.selectors.some((selector) => selector.includes(".symbol")));
  assert.deepEqual(grounds.flatMap((rule) => rule.selectors), [".symbol img:not([src])"],
    "no rule paints a ground or room around a drawn agent image: the service's tile is whole");
  const symbolClasses = new Set(RULES.flatMap((rule) => rule.selectors.join(" ").match(/\.symbol-[a-z-]+/g) ?? []));
  const drawn = new Set(Object.keys(Words.SYMBOL_SIZES).map((size) => `.symbol-${size}`));
  assert.deepEqual([...symbolClasses].filter((name) => !drawn.has(name)), [],
    "the stylesheet styles no symbol class the page no longer draws");
});

test("the summary counts in words, singular for one and plain for none", () => {
  assert.deepEqual([0, 1, 3].map(Words.agentsCount), ["No agents", "1 agent", "3 agents"]);
  assert.deepEqual([0, 1, 2].map(Words.browsersCount), ["no browser", "1 browser", "2 browsers"]);
  assert.deepEqual([0, 1, 2].map((count) => Words.noun(count, "session", "sessions")),
    ["sessions", "session", "sessions"], "the band says 1 session, not 1 sessions");
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
  assert.ok(chipMarkup("codex").includes(Words.symbolMarkup(namedAgent("Codex"), "chip")), "an agent wears its image");
  assert.ok(chipMarkup("command_line").includes(Words.SYMBOL_GLYPHS[Words.GLYPH.commandLine]), "the command line wears the prompt");
  const browsers = bar.wearing("glance-chip").filter((chip) => chip.tag === "span");
  assert.deepEqual(browsers.map((chip) => chip.textContent), ["Google Chrome3 tabs"]);
  assert.equal(browsers[0].attributes.title, "Ghostlight in Browser 2.0.0", "the extension by its product name");
});

test("the shelves group sessions by the agent id the service gives, never by name", () => {
  const cline = { id: "cline", name: "Cline", symbol: { icon: "agent-icons/cline?v=fedcba987654" } };
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
  assert.equal(Words.sessionLine({ tab_count: 1, browser: "Edge", running_operations: 0, channel: "cli" }),
    "1 tab in Edge, idle, from the command line");
  assert.equal(Words.sessionLine({ tab_count: 0, browser: "Edge", running_operations: 1, channel: "mcp" }),
    "In Edge, no tabs, working");
});

test("work held while paused is not working: nothing at a glance or in the pane wears the live look", () => {
  const shown = drawing();
  const answer = snapshotAnswer({
    sessions: [glanceSession("w1", "Codex", { tabs: 3, browser: "Chrome" })],
    service: { version: "2.0.0", started_at_ms: 1, runtime_state: "held" }
  });
  answer.parts.activity.operations = ["h1", "h2"].map((invocation) => ({
    invocation, workspace: "w1", tool: "browser_read", activity: "Reading page", capability: "read",
    started_at_ms: 1, phase: "held"
  }));
  answer.parts.integrations.harnesses = [agent("codex", "Codex", "installed")];
  answer.parts.connections.harnesses_in_use = ["codex"];
  shown.view.collections(Transport.flatten(answer), new Set());
  const glanced = () => drawn(shown.el("connections").innerHTML);
  shown.click(glanced().wearing("glance-chip").find((node) => node.dataset.glanceAgent === "codex"));
  const busy = [...glanced().root.descendants()].filter((node) => node.classes?.has("busy"));
  assert.deepEqual(busy, [], "no face, chip or card head is busy");
  assert.equal(glanced().wearing("glance-line")[0].children[0].textContent, "3 tabs in Chrome, idle");
  const entry = drawn(shown.el("agents-list").innerHTML).wearing("agent-entry").find((node) => node.dataset.agent === "codex");
  assert.ok(!entry.children.find((node) => node.classes.has("agent-dot")).classes.has("d-live"), "the rail is not live");
  assert.deepEqual(shown.failures, []);
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
    id, product_id: id, name, label: name, target: "User", agent: { id, name, symbol: { icon: `agent-icons/${id}?v=fedcba987654` } },
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
  assert.ok(dot("zed").classes.has("d-installed"), "an idle agent in use keeps the green dot");
  assert.ok(dot("codex").classes.has("d-installed"));
  const busy = agentsPage(ROSTER, { sessions: [glanceSession("w1", "Zed", { tabs: 2, busy: true })], inUse: ["zed"] });
  const busyEntry = busy.rail().wearing("agent-entry").find((entry) => entry.dataset.agent === "zed");
  assert.ok(busyEntry.children.find((node) => node.classes.has("agent-dot")).classes.has("d-live"),
    "an agent running work wears the teal dot");
  assert.equal([...busyEntry.descendants()].find((node) => node.classes.has("agent-entry-sub")).textContent,
    "1 session, 2 tabs, working", "and says it is working, not only in colour");
  busy.pick("zed");
  assert.equal(busy.pane().wearing("agent-state")[0].textContent, "Working");
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

test("Remove follows the service's answer alone, so an updatable entry it can remove shows Remove", () => {
  const removable = agentsPage(ROSTER.map((row) => (row.id === "kiro" ? { ...row, can_uninstall: true } : row)));
  removable.pick("kiro");
  assert.deepEqual(removable.pane().wearing("agent-act").map((row) => row.children[0].textContent), ["Update"]);
  const remove = removable.pane().wearing("agent-remove")[0];
  assert.equal(remove.textContent, "Remove from Kiro");
  assert.equal(remove.dataset.harnessState, "updatable");
  const kept = agentsPage(ROSTER.map((row) => (row.id === "codex" ? { ...row, can_uninstall: false } : row)));
  kept.pick("codex");
  assert.equal(kept.pane().wearing("agent-remove").length, 0, "an installed entry the service cannot remove shows no Remove");
});

test("an agent that needs attention shows the service's evidence and numbered steps when it cannot be fixed", () => {
  const shown = agentsPage(ROSTER);
  assert.equal(shown.title(), "Junie");
  const evidence = shown.pane().wearing("agent-evidence")[0].children[1];
  assert.ok(evidence.textContent.includes("npx with 2 arguments"), evidence.textContent);
  assert.ok(!evidence.textContent.includes("`"), "the service's marks are drawn as code, not printed");
  assert.equal(shown.pane().wearing("agent-code")[0].textContent, "npx");
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
  assert.equal(shown.pane().wearing("agent-use").length, 0);
  assert.equal(shown.pane().wearing("agent-remove")[0].dataset.harnessSessions, "0");
  const arrive = (busy) => shown.view.collections({
    ...shown.snapshot, sessions: [glanceSession("w1", "Codex", { tabs: 3, busy })], harnesses_in_use: ["codex"]
  }, new Set());
  const dotOf = () => entry().children.find((node) => node.classes.has("agent-dot"));
  arrive(false);
  assert.equal(sub(), "1 session, 3 tabs");
  assert.ok(dotOf().classes.has("d-installed"), "an idle session keeps the green dot");
  assert.equal(shown.pane().wearing("agent-use")[0].classes.has("live"), false);
  arrive(true);
  assert.equal(sub(), "1 session, 3 tabs, working");
  assert.ok(dotOf().classes.has("d-live"), "the dot pulses teal while work runs");
  assert.ok(shown.pane().wearing("agent-use")[0].classes.has("live"));
  assert.equal(shown.pane().wearing("agent-use")[0].children[1].textContent,
    "1 Codex session is connected right now, with 3 browser tabs open.");
  assert.equal(shown.pane().wearing("agent-remove")[0].dataset.harnessSessions, "1");
  shown.view.collections({ ...shown.snapshot, sessions: [], harnesses_in_use: [] }, new Set());
  assert.equal(sub(), null, "and leaves when the session ends");
  assert.equal(shown.pane().wearing("agent-remove")[0].dataset.harnessSessions, "0");
  assert.deepEqual(shown.failures, []);
});

test("only the rows the service says are in use pulse, and the page never puts a session on a host itself", () => {
  const cline = (id, label, state) => ({ ...agent(id, "Cline", state), label, agent: namedAgent("Cline") });
  const rows = [cline("cline-cli", "Cline (CLI)", "installed"), cline("cline-vscode", "Cline (Visual Studio Code)", "available"),
    cline("cline-cursor", "Cline (Cursor)", "updatable")];
  const shown = agentsPage(rows, { sessions: [glanceSession("w1", "Cline", { tabs: 1, busy: true })], inUse: ["cline-cli"] });
  const dot = (id) => shown.rail().wearing("agent-entry").find((entry) => entry.dataset.agent === id)
    .children.find((node) => node.classes.has("agent-dot"));
  assert.ok(dot("cline-cli").classes.has("d-live"));
  assert.ok(dot("cline-cursor").classes.has("d-updatable"), "the page draws only the rows the service names in use");
  assert.ok(dot("cline-vscode").classes.has("d-available"), "a host without the entry stays found");
  assert.deepEqual(shown.rail().wearing("agents-group-head").map((head) => head.children[0].textContent),
    ["Needs a look", "Connected", "Found, not connected"]);
});

test("the pane names a Store edition's own settings file and another copy beside it", () => {
  const store = "~\\AppData\\Local\\Packages\\Claude_pzs8sxrjxfjjc\\LocalCache\\Roaming\\Claude\\claude_desktop_config.json";
  const plain = "~\\AppData\\Roaming\\Claude\\claude_desktop_config.json";
  const desktop = agent("claude-desktop", "Claude Desktop", "available", { config_path: store, packaged: true });
  const shown = agentsPage([desktop, agent("zed", "Zed", "installed")]);
  shown.pick("claude-desktop");
  const text = (node) => node.textContent.replace(/<wbr>/g, "").replace(/&#039;/g, "'");
  assert.equal(text(shown.pane().wearing("agent-path")[0]), store);
  assert.equal(text(shown.pane().wearing("agent-copies")[0]),
    "Claude Desktop from the Microsoft Store keeps its settings in this file, in its own folder.");
  assert.match(shown.pane().wearing("agent-lead")[0].textContent, /^Claude Desktop is installed on this computer/);

  const beside = agentsPage([{ ...desktop, other_copy: plain }]);
  assert.equal(text(beside.pane().wearing("agent-copies")[0]),
    "Claude Desktop from the Microsoft Store keeps its settings in this file, in its own folder. "
    + `Ghostlight's entry goes in this file. Another copy of these settings is at ${plain}.`);

  const plainOnly = agentsPage([agent("zed", "Zed", "installed")]);
  assert.equal(plainOnly.pane().wearing("agent-copies").length, 0, "a plain settings file gets no note");
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
  assert.equal(shown.el("setup-integrations").hidden, true, "nothing for Set up everything to do, so it is not offered");
  assert.equal(shown.el("integrations-caption").hidden, true);
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
  assert.equal(shown.pane().wearing("agent-use").length, 1);
  assert.deepEqual(shown.pane().wearing("agent-act"), [], "nothing to do next");
  assert.equal(shown.pane().wearing("agent-remove").length, 0, "Ghostlight never edits its file, so it offers no Remove");
  assert.ok(!shown.el("agents-pane").innerHTML.includes("not connected"));
  assert.equal(shown.el("setup-integrations").hidden, true);
  assert.equal(shown.el("integrations-caption").hidden, true);
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
  assert.equal(shown.el("integrations-caption").textContent, "Connects Cursor and updates Kiro. Junie needs you.");
  assert.equal(shown.el("setup-integrations").hidden, false);
  assert.equal(shown.el("integrations-caption").hidden, false);
  assert.equal(Words.agentPlan({ connect: [], update: [], left: [] }), "");
  assert.equal(Words.agentPlan({ connect: ["A", "B", "C", "D"], update: [], left: ["goose", "Junie"] }),
    "Connects 4 agents. goose and Junie need you.");
  assert.equal(Words.agentPlan({ connect: [], update: ["Cursor"], left: [] }), "Updates Cursor.");
  const done = agentsPage(on(2));
  assert.equal(done.el("setup-integrations").hidden, true, "a primary that can do nothing is not shown");
  assert.equal(done.el("integrations-caption").hidden, true);
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

test("a watching screen sees every agent's state and copies, and is shown once the way to the window", () => {
  const watcher = {
    context: "watch", opener: "anchor", word: "Watching", sentence: WATCHER, product: { version: "2.0.0" },
    acts: Object.fromEntries(Object.values(Transport.ACT).map((act) => [act,
      [Transport.ACT.rescan, Transport.ACT.hold].includes(act) ? { available: true, why_not: null } : { available: false, why_not: WHY }]))
  };
  const rows = ROSTER.map((row) => ({ ...row, download_url: `https://example.test/${row.id}` }));
  const shown = agentsPage(rows, { context: watcher });
  assert.equal(shown.el("integrations-continue").hidden, false);
  assert.ok(shown.el("integrations-continue").innerHTML.includes(`>${Words.CONTINUE_LABEL}</button>`));
  assert.ok(!shown.el("integrations-continue").innerHTML.includes(WHY), "the link alone says where changes are made");
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
  const notes = { not_applicable: "", complete: "", interactive: "", loading: "never settled", unknown: "readiness unknown" };
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
  assert.equal(cell("row-tool-name").textContent, "fill form");
  assert.equal(cell("row-tool").attributes.title, "browser_fill_form");
  const meta = cells.wearing("hero-meta")[0];
  assert.ok(!meta.textContent.includes("browser_fill_form"), "the exact name is the tool's hover, not a fact to read");
  assert.ok(!meta.textContent.includes("succeeded"), "success is what the sentence says");
  assert.equal(cells.wearing("hero-tool")[0].attributes.title, "browser_fill_form");
});

test("a row names one capability, the most consequential, and a screen reader hears the whole set", () => {
  const { cell, cells } = rowOf(settledRead({ invocation: "fill-2", tool: "browser_fill_form", capability: "read + write" }));
  assert.equal(cell("row-cap").textContent, "write");
  assert.equal(cell("row-tool-cap").textContent, "write", "the word under the tool, for the narrowest row");
  for (const shown of [cell("row-cap"), cell("row-tool-cap")]) assert.equal(shown.attributes["aria-hidden"], "true");
  const heard = cell("row-tool").children.filter((node) => node.classes.has("sr-only") && !node.classes.has("row-by-said"));
  assert.deepEqual(heard.map((node) => node.textContent), [", read and write"]);
  assert.equal(cells.wearing("cap-label")[0].textContent, "read + write", "the detail card shows the whole set");
  assert.ok(cells.wearing("hero-meta")[0].textContent.includes("read + write"));
  assert.ok(!cells.wearing("hero-meta")[0].textContent.includes("authority"), "the meta line says the capability plainly");

  for (const [capability, word, said] of [["read", "read", "read"], ["read + action", "action", "read and action"],
    ["read + action + execute", "execute", "read, action and execute"]]) {
    const entry = { capability };
    assert.equal(Words.capabilityWord(entry), word, capability);
    assert.equal(Words.capabilityClass(entry), `cap-${word}`, capability);
    assert.equal(Words.capabilitiesSaid(entry), said, capability);
  }
});

/** A flow that clicked, typed and filled a form, with a fourth step it never ran. */
function flowRecord() {
  const step = (position, tool, capability) => ({
    position, tool, state: "recorded", record: settledRead({ invocation: "flow-1", tool, capability })
  });
  return settledRead({
    invocation: "flow-1", tool: "browser_flow", capability: "local", summary: "Completed 3 of 4 steps.",
    steps: [step(1, "browser_click", "read + action"), step(2, "browser_type_text", "read + action"),
      step(3, "browser_fill_form", "read + write"), { position: 4, tool: "browser_read", state: "not_run", record: null }]
  });
}

test("a flow takes the most consequential capability its steps used, and never says local", () => {
  const entry = Entries.entryFromRecord(flowRecord(), null);
  assert.deepEqual(Words.capabilitiesOf(entry), ["read", "action", "write"]);
  assert.equal(Words.capabilityClass(entry), "cap-write", "the edge is the write colour, not the read grey");
  const { cell, cells, text } = rowOf(flowRecord());
  assert.equal(cell("row-cap").textContent, "write");
  assert.equal(cells.wearing("cap-label")[0].textContent, "read + action + write");
  assert.ok(cells.wearing("hero-meta")[0].textContent.includes("Steps: look at pages, click and type, fill in forms"));
  assert.deepEqual(cells.wearing("step-meta").map((node) => node.textContent.split(";")[0]),
    ["read + action", "read + action", "read + write"], "each step keeps its own label");
  assert.doesNotMatch(text, /\blocal\b|per step/i);

  // A flow still running has no step receipts yet: it names no capability rather than a place.
  const running = { capability: "local", steps: [] };
  assert.equal(Words.capabilityWord(running), "");
  assert.equal(Words.capabilityLabel(running), "");
  assert.equal(Words.capabilityClass(running), "cap-read");
});

test("earlier work that kept no agent says Earlier session and that no name was kept, never a workspace id", () => {
  const workspace = "workspace_0123456789abcdef825";
  const earlier = {
    id: "earlier", name: "Earlier session", symbol: { glyph: "unknown" }, note: "Recorded in an earlier session. Ghostlight kept no agent name for it."
  };
  const { cell, cells, text } = rowOf(settledRead({ invocation: "earlier-1", workspace, agent: earlier, provenance: {
    connection_id: "connection_old", reported_application: null, channel: "mcp", explanation: ""
  } }));
  assert.equal(cell("row-client").textContent, "Earlier session");
  assert.equal(cell("row-client").attributes.title, "Recorded in an earlier session. Ghostlight kept no agent name for it.");
  assert.ok(!text.includes("workspace_"), `a raw id is printed in the row: ${text}`);
  assert.ok(![...cells.root.descendants()].some((node) => node.attributes?.title === workspace), "no hover holds the workspace id");
  assert.equal(cells.wearing("connection-explanation").length, 0, "no caveat about a name the card does not show");

  const live = rowOf(settledRead({ invocation: "unknown-1", workspace, agent: namedAgent("Unknown agent") }));
  assert.equal(live.cell("row-client").textContent, "Unknown agent");
  assert.equal(live.cell("row-client").attributes.title, undefined);
});

test("an action names the agent its record names, with no hover id", () => {
  const { cell } = rowOf(settledRead({ invocation: "codex-1", agent: namedAgent("Codex"), provenance: {
    connection_id: "connection_1", reported_application: "codex-mcp-client", channel: "mcp",
    explanation: "\"codex-mcp-client\" is what the agent calls itself; Ghostlight does not verify it."
  } }));
  assert.equal(cell("row-client").textContent, "Codex");
  assert.equal(cell("row-client").attributes.title, undefined);
});

test("an action's card names its agent once and keeps the connection to what Ghostlight can vouch for", () => {
  const mcp = rowOf(settledRead({ invocation: "card-mcp", agent: namedAgent("Codex"), provenance: {
    connection_id: "connection_1", reported_application: "codex-mcp-client", channel: "mcp",
    explanation: "\"codex-mcp-client\" is what the agent calls itself; Ghostlight does not verify it."
  } }));
  const connection = mcp.cells.wearing("connection-details")[0];
  assert.equal(mcp.cells.wearing("hero-meta")[0].textContent.split("Codex").length - 1, 1, "the agent is named in the meta line");
  assert.ok(!connection.textContent.includes("Codex"), "and not again under the connection");
  assert.doesNotMatch(connection.textContent, /Reported application|Not retained|Intake/);
  assert.ok(connection.textContent.includes("is what the agent calls itself"), "the trust note stays");
  const copy = mcp.cells.wearing("connection-copy")[0];
  assert.equal(copy.dataset.copyConnection, "connection_1");
  assert.equal(copy.textContent, "Copy id");
  assert.ok(!connection.textContent.includes("connection_1"), "the id is behind Copy id, not printed");

  const cli = rowOf(settledRead({ invocation: "card-cli", channel: "cli", agent: namedAgent("Command line"), provenance: {
    connection_id: "connection_2", reported_application: null, channel: "cli",
    explanation: "A ghostlight call session is a local program."
  } }));
  assert.ok(cli.cells.wearing("connection-details")[0].textContent.includes("Came through the command line."));
  assert.ok(!mcp.cells.wearing("connection-details")[0].textContent.includes("command line"));

  const nothing = rowOf(settledRead({ invocation: "card-none", agent: namedAgent("Unknown agent"), provenance: {
    connection_id: null, reported_application: null, channel: "mcp", explanation: ""
  } }));
  assert.equal(nothing.cells.wearing("connection-details").length, 0, "a connection with nothing to say draws no disclosure");
});

test("permission checks the service explained in the same words are said once", () => {
  const check = (host) => ({ requirements: ["read"], host, allowed: true, reason: "permitted", layers: [] });
  const { cells } = rowOf(settledRead({ invocation: "same-1",
    permissions: { checks: [check("example.com"), check("example.com"), check("example.org")], truncated: false },
    permission_explanations: ["Allowed: no rules are set.", "Allowed: no rules are set.", "Allowed: no rules are set."] }));
  const lines = cells.wearing("permission-details")[0].descendants();
  const items = [...lines].filter((node) => node.tag === "li");
  assert.equal(items.length, 1);
  assert.equal(items[0].children[0].textContent, "read on example.com, read on example.org");
  assert.equal(items[0].children[1].textContent, "Allowed: no rules are set.");
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
    invocation: "refused-1", allowed: false, reason: "browser_disconnected", status: "failed", outcome: "failed",
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

/** The organization as the service names one whose policy gives a name and somewhere to ask. */
const EXAMPLE_ORG = Object.freeze({
  identity: {
    name: "Example Org",
    statement: "Support work only.",
    url: "https://example.test/policy",
    contacts: [{ kind: "email", label: "Service desk", value: "desk@example.test" }]
  },
  title: "Example Org",
  in_sentence: "Example Org"
});

/** A compiled policy that `organization` sets, with a signed passport. */
function managedPolicy(organization) {
  return {
    headline: `${organization.title} sets the rules.`,
    organization,
    capabilities: [],
    ceilings: [],
    layers: [],
    user_layer: { source: "managed", editable: false, path: null, blocked_reason: null },
    passport: {
      configured: true,
      sequence: 42,
      freshness: "fresh",
      source_class: "https",
      last_success_ms: 1_700_000_000_000
    }
  };
}

/** The text of the drawn organization card. */
function cardText(shown) {
  return drawn(shown.el("policy-organization").innerHTML).root.children.map((node) => node.textContent).join(" ");
}

test("a managed policy's passport shows every signed fact", () => {
  const shown = drawing();
  shown.view.policy(managedPolicy(EXAMPLE_ORG));
  assert.deepEqual(shown.failures, []);
  assert.equal(shown.el("policy-organization").hidden, false);
  const text = cardText(shown);
  for (const fact of ["Example Org", "Support work only.", "https://example.test/policy", "Service desk",
    "desk@example.test", "fresh", "verified sequence 42", "from an HTTPS source", "last checked"]) {
    assert.ok(text.includes(fact), `the passport does not show ${fact}: ${text}`);
  }
});

test("the refusal row and the card name one organization, in the service's words", () => {
  const refusal = (shown) => {
    shown.view.hero(Entries.entryFromRecord(settledRead({ invocation: "refused-1", tool: "browser_execute",
      capability: "execute", allowed: false, reason: "denied_by_policy", status: "blocked", outcome: "refused",
      policy_tier: "managed", grant_id: "support-sites", denial_id: "deny-1", summary: "Refused." }), null), false);
    return drawn(shown.el("hero-body").innerHTML).wearing("hero-refusal")[0].textContent;
  };
  const named = drawing();
  named.view.collections(Transport.flatten({
    ...snapshotAnswer(),
    parts: { ...snapshotAnswer().parts, status: { ...snapshotAnswer().parts.status,
      configuration: { ...snapshotAnswer().parts.status.configuration, organization: EXAMPLE_ORG } } }
  }), new Set());
  named.view.policy(managedPolicy(EXAMPLE_ORG));
  assert.deepEqual(named.failures, []);
  const said = refusal(named);
  assert.ok(said.startsWith("Refused by Example Org, rule support-sites."), said);
  assert.ok(said.includes("Service desk: desk@example.test"), said);
  assert.ok(cardText(named).includes("Example Org"));

  // An organization that names itself nowhere is named by the service's words, on both.
  const unnamed = drawing();
  unnamed.view.collections(Transport.flatten(snapshotAnswer()), new Set());
  unnamed.view.policy(managedPolicy(UNNAMED_ORGANIZATION));
  assert.deepEqual(unnamed.failures, []);
  assert.ok(refusal(unnamed).startsWith("Refused by your organization, rule support-sites."), refusal(unnamed));
  assert.equal(drawn(unnamed.el("policy-organization").innerHTML).root.children[0].textContent, "Your organization");
  assert.doesNotMatch(VIEW_SOURCE, /[Yy]our organization/, "the page names no organization of its own");
});

// WorkbenchDestination in workbench/destination.rs, less get_chrome, which only the setup view offers. The
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
    browser_read: "scan", browser_inspect: "scan", browser_find: "lens", browser_screenshot: "camera",
    browser_click: "pointer", browser_scroll: "navigate", browser_hover: "pointer", browser_fill_form: "keyboard",
    browser_type_text: "keyboard", browser_press_key: "keyboard", browser_drag: "pointer", browser_wait: "wait",
    browser_dialog: "wait", browser_upload: "gear", browser_execute: "gear", browser_flow: "gear",
    browser_record: "camera", browser_diagnose: "scan", policy_explain: "scan"
  };
  const catalog = JSON.parse(readFileSync(join(ui, "..", "..", "..", "docs", "1.0", "catalog.json"), "utf8"));
  assert.deepEqual(catalog.map((tool) => tool.name).sort(), Object.keys(chosen).sort(),
    "a new tool needs a medallion chosen for it");
  for (const [tool, glyph] of Object.entries(chosen)) {
    assert.ok(Words.GLYPHS[glyph], glyph);
    assert.equal(Words.glyphFor({ tool }), Words.GLYPHS[glyph], tool);
  }
  // Watched work wears its activity's medallion, keyed by the service's code and not its words,
  // waiting or running alike (SURF-4): a waiting browser_tabs call's quiet work is the scan it
  // shows once it runs, no longer its tool's navigate, and a reworded label changes nothing.
  for (const activity of ["Waiting for earlier browser work", "Ghostlight", "Something else"]) {
    assert.equal(Words.glyphFor({ tool: "browser_tabs", activity, activityCode: "quiet" }), Words.GLYPHS.scan, activity);
  }
  assert.equal(Words.glyphFor({ tool: "browser_flow", activityCode: "read" }), Words.GLYPHS.scan);
  assert.equal(Words.glyphFor({ tool: "browser_click", activityCode: "click" }), Words.GLYPHS.pointer);
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

/** The display the last rule naming `selector` sets at a shell `width`, or "" when none sets one. */
const displayAt = (width, selector) =>
  naming(selector, applyingAt(width)).map((rule) => rule.declarations.display).filter(Boolean).at(-1) ?? "";

test("the capability word keeps a column until the row is narrowest, then sits under the tool", () => {
  assert.ok(naming(".row-cap").some((rule) => rule.declarations["white-space"] === "nowrap"), "one word never wraps");
  for (const cell of [".row-cap", ".row-client", ".row-browser", ".row-when"]) assert.notEqual(displayAt(Infinity, cell), "none", cell);
  assert.equal(displayAt(Infinity, ".row-tool-cap"), "none");
  // A half-snapped window: the browser gives way first, since the card names it.
  for (const width of [1040, 960]) assert.equal(displayAt(width, ".row-browser"), "none", `${width}px`);
  for (const width of [1040, 821]) {
    assert.notEqual(displayAt(width, ".row-cap"), "none", `the capability is hidden at ${width}px`);
    assert.equal(displayAt(width, ".row-tool-cap"), "none", `the capability shows twice at ${width}px`);
  }
  for (const width of [820, 720, 600]) {
    assert.equal(displayAt(width, ".row-cap"), "none", `${width}px`);
    assert.equal(displayAt(width, ".row-tool-cap"), "block", `the capability is gone at ${width}px`);
  }
});

test("a row names its agent at every width: in its column, then on a second line under the activity", () => {
  assert.equal(displayAt(Infinity, ".row-by"), "none", "the agent and the age are not said twice");
  for (const width of [Infinity, 1280, 1040, 960, 821]) {
    assert.notEqual(displayAt(width, ".row-client"), "none", `the agent's column is gone at ${width}px`);
    assert.notEqual(displayAt(width, ".row-when"), "none", `the age's column is gone at ${width}px`);
    assert.equal(displayAt(width, ".row-by"), "none", `the agent shows twice at ${width}px`);
  }
  for (const width of [820, 720, 600]) {
    assert.equal(displayAt(width, ".row-client"), "none", `${width}px`);
    assert.equal(displayAt(width, ".row-by"), "flex", `the agent is gone at ${width}px`);
  }
  // The second line is the quiet mono of a value.
  const by = naming(".row-by")[0].declarations;
  assert.equal(by.color, "var(--ink-4)");
  assert.match(by.font, /11\.5px\/1\.3 var\(--mono\)/);
});

test("a row's agent and age are drawn on its second line and said on its button", () => {
  const record = settledRead({ invocation: "by-1", timestamp_ms: Date.now() - 4 * 60_000,
    channel: "cli", agent: { id: "command_line", name: "Command line", symbol: { glyph: "command_line" } } });
  const { cell } = rowOf(record);
  assert.equal(cell("row-by-words").textContent, "Command line, 4m");
  assert.equal(cell("row-by").attributes["aria-hidden"], "true", "a screen reader hears it on the button");
  assert.ok(cell("row-by").children.some((node) => node.tag === "svg"), "the command line keeps its mark on the second line");
  assert.equal(cell("row-by-said").textContent, ", Command line, 4m ago");
  for (const shownTwice of [cell("row-client"), cell("row-when")]) assert.equal(shownTwice.attributes["aria-hidden"], "true");
  const mcp = rowOf(settledRead({ invocation: "by-2", timestamp_ms: Date.now() - 4 * 60_000,
    agent: { id: "codex", name: "Codex", symbol: { glyph: "codex" } } }));
  assert.ok(!mcp.cell("row-by").children.some((node) => node.tag === "svg"), "only the command line is marked");
  assert.equal(mcp.cell("row-by-words").textContent, "Codex, 4m");
  assert.equal(Words.rowBy("Codex", ""), "Codex", "running work has no age yet");
});

test("Status's checks and Policy's capabilities sit four in a row, or two and two, never three and one alone", () => {
  // The last rule that applies wins, as in the cascade: a narrow rule must follow the rule it narrows.
  const columns = (selector, width) => naming(selector, applyingAt(width))
    .map((rule) => rule.declarations["grid-template-columns"]).filter(Boolean).at(-1);
  for (const selector of [".card-grid", ".cap-board"]) {
    for (const width of [Infinity, 1280, 1001]) assert.equal(columns(selector, width), "repeat(4, minmax(0, 1fr))", `${selector} at ${width}px`);
    for (const width of [1000, 960, 820, 720, 600]) assert.equal(columns(selector, width), "repeat(2, minmax(0, 1fr))", `${selector} at ${width}px`);
  }
});

test("Settings keeps its rail beside the section down to the window's 720px minimum", () => {
  const kitRules = rulesOf(readFileSync(join(ui, "kit", "kit.css"), "utf8"));
  const stack = kitRules.filter((rule) => rule.selectors.includes(".settings") && rule.declarations["grid-template-columns"] === "minmax(0, 1fr)");
  assert.equal(stack.length, 1);
  assert.ok(widthOf(stack[0].media) < 720, `the rail stacks at ${stack[0].media}`);
});

test("at 600px the narrowest row leaves the activity room to read", () => {
  const columns = naming(".row", applyingAt(600)).map((rule) => rule.declarations["grid-template-columns"]).filter(Boolean).at(-1);
  const fixed = [...columns.matchAll(/(?:^|\s)(\d+)px/g)].reduce((sum, [, px]) => sum + Number(px), 0);
  const gaps = (tracksIn(columns) - 1) * pxOf(naming(".row")[0].declarations.gap);
  // The content's padding, its scrollbar, and the row's own padding and edge.
  const around = 18 * 2 + 10 + 12 + 11 + 3;
  const activity = 600 - around - fixed - gaps;
  assert.ok(activity >= 200, `the activity has ${activity}px at 600px`);
});

test("the band's facts give way whole and from the end, all below 1000px, and below 900px the tabs take a line of their own", () => {
  assert.notEqual(displayAt(1001, ".state-facts"), "none");
  assert.equal(displayAt(1000, ".state-facts"), "none");
  // A fact is never cut: no ellipsis, and one with no room wraps onto a line the band never shows.
  const facts = naming(".state-facts")[0].declarations;
  assert.equal(facts["text-overflow"], undefined);
  assert.equal(facts["flex-wrap"], "wrap");
  assert.equal(facts.overflow, "hidden");
  assert.equal(facts.height, "1.4em");
  assert.match(facts.font, /\/1\.4 /, "the line shown is the one line the facts take");
  for (const width of [Infinity, 1280]) {
    for (const fact of [".fact-agents", ".fact-browsers", ".fact-work", ".fact-recorded"]) {
      assert.notEqual(displayAt(width, fact), "none", `${fact} at ${width}px`);
    }
  }
  // What was recorded goes first, then the work in flight; the agents and the browsers stay longest.
  for (const width of [1279, 1200, 1180]) assert.equal(displayAt(width, ".fact-recorded"), "none", `${width}px`);
  assert.notEqual(displayAt(1160, ".fact-work"), "none");
  assert.equal(displayAt(1159, ".fact-work"), "none");
  for (const fact of [".fact-agents", ".fact-browsers"]) assert.notEqual(displayAt(1001, fact), "none", fact);
  // Any state other than Ready or Working is a word to read: Session ended, with the Shared chip
  // beside it, at the default 1180px window, in Standard and in Large text, shows the word alone.
  const large = Number(naming(".text-large .app-shell")[0].declarations.zoom);
  for (const tone of ["held", "attention", "offline"]) {
    for (const width of [1180, Math.floor(1180 / large)]) {
      assert.equal(displayAt(width, `body.runtime-${tone} .state-facts`), "none", `${tone} at ${width}px`);
    }
    assert.equal(displayAt(Infinity, `body.runtime-${tone} .state-facts`), "", `${tone} keeps its facts where there is room`);
  }
  for (const tone of ["quiet", "working"]) {
    assert.equal(naming(`body.runtime-${tone} .state-facts`).length, 0, `${tone} keeps its facts at 1180px`);
  }
  const tabs = (width) => Object.assign({}, ...naming(".tabs", applyingAt(width)).map((rule) => rule.declarations));
  assert.equal(tabs(901)["grid-row"], undefined);
  assert.equal(tabs(900)["grid-row"], "2");
  assert.equal(tabs(900)["grid-column"], "1 / -1");
  assert.equal(naming(".lampband")[0].declarations["min-height"], "var(--band)", "the band grows to hold the second line");
  assert.match(naming(".app-shell")[0].declarations["grid-template-rows"], /^auto /);
});

test("every width rule asks the shell, which Large text draws larger, and the page holds at 600px", () => {
  assert.equal(naming(".app-shell")[0].declarations.container, "shell / inline-size");
  assert.equal(naming("html")[0].declarations["min-width"], "600px");
  // The one viewport width left is the setup view's, which follows the extension kit's own
  // one-column breakpoint, read from the kit so the two cannot drift apart.
  const viewport = (sheet) => [...sheet.matchAll(/@media([^{]*)\{/g)].map(([, condition]) => condition.trim())
    .filter((condition) => /width/.test(condition));
  const kitSheet = readFileSync(join(ui, "..", "..", "..", "extension", "kit.css"), "utf8");
  const column = Number(/One column below \d+px[\s\S]*?@media \(max-width: (\d+)px\)/.exec(kitSheet)?.[1]);
  assert.ok(column > 0, "the kit names its one-column breakpoint");
  assert.deepEqual(viewport(readFileSync(join(ui, "styles.css"), "utf8")),
    [`(min-width: ${column + 1}px)`, `(max-width: ${column}px)`]);
  assert.deepEqual(viewport(readFileSync(join(ui, "kit", "kit.css"), "utf8")), []);
  const large = naming(".text-large .app-shell")[0];
  assert.equal(large.declarations.zoom, "1.15");
  for (const place of ["#setup-root", ".palette-backdrop", ".notices", ".dialog-backdrop"]) {
    assert.ok(large.selectors.includes(`.text-large ${place}`), `Large text leaves ${place} small`);
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

/* ------------------------- a screen reader and a failure ------------------------- */

/** The fewest milliseconds the view leaves between two sentences it says (view.js SAID_GAP_MS). */
const SAID_GAP = 1000;

/** The view over the page's markup with a clock this test moves, and the work it was asked to do later. */
function clocked() {
  const shown = page();
  const later = [];
  let clock = 10_000;
  globalThis.document = shown.document;
  globalThis.window = shown.window;
  const view = View.create({ now: () => clock, schedule: (work, ms) => later.push({ work, ms }) });
  const run = () => {
    for (const { work } of later.splice(0)) work();
  };
  return { ...shown, view, later, run, pass: (ms) => { clock += ms; } };
}

/** A read the window watched start, as the live stream names it. */
const runningRead = (invocation) => Entries.entryFromOperation({
  invocation, workspace: "w1", tool: "browser_read", activity: "Reading page", capability: "read",
  phase: "running", started_at_ms: 1
});

test("the running card is not a live region; one quiet status line outside it is", () => {
  const hero = /<article[^>]*\bid="hero"[^>]*>/.exec(markup)[0];
  assert.doesNotMatch(hero, /aria-live|role=/, "a stopwatch rewrites the card ten times a second");
  const line = /<([a-z]+)[^>]*\bid="action-said"[^>]*>/.exec(markup)[0];
  assert.match(line, /role="status"/);
  assert.match(line, /class="sr-only"/);
  const shown = page();
  assert.equal(shown.el("action-said").closest("#hero"), null);
  assert.ok(shown.el("action-said").closest("#view-monitor"), "it speaks only while At a glance is shown");
  assert.match(/<div[^>]*\bid="hero-med"[^>]*>/.exec(markup)[0], /aria-hidden="true"/);
});

test("a running action says nothing, and its clock and medallions are hidden from a screen reader", () => {
  const shown = clocked();
  const running = runningRead("live-1");
  shown.view.promote(running, null, [running]);
  shown.run();
  assert.equal(said(shown, "action-said"), "");
  assert.match(shown.el("hero-right").innerHTML, /id="elapsed" aria-hidden="true"/);
  const newest = Entries.entryFromRecord(settledRead({ invocation: "newest" }), null);
  shown.view.rebuildFeed([newest, running]);
  assert.match(queueRows(shown)[0].innerHTML, /^<div class="med-mini" aria-hidden="true">/);
});

test("a settled action is said once, in the row's own words, and what the feed already held is not", () => {
  const shown = clocked();
  const history = Entries.entryFromRecord(settledRead({ invocation: "old" }), null);
  shown.view.rebuildFeed([history]);
  shown.view.hero(history, false);
  assert.equal(said(shown, "action-said"), "", "history drawn on arrival is not news");

  const running = runningRead("read-2");
  shown.view.promote(running, history, [running, history]);
  const done = Entries.entryFromRecord(settledRead({ invocation: "read-2" }), running);
  shown.view.hero(done, false);
  assert.equal(said(shown, "action-said"), "read: Read 1,240 words from example.com.");
  shown.el("action-said").textContent = "";
  shown.view.hero(done, false);
  assert.equal(said(shown, "action-said"), "", "a repaint of the same action says nothing again");
});

test("a burst of settled actions reaches a screen reader as at most one sentence a second, the latest", () => {
  const shown = clocked();
  const settle = (invocation, summary) => {
    const entry = Entries.entryFromRecord(settledRead({ invocation, summary }), null);
    shown.view.promote(entry, null, [entry]);
  };
  settle("a", "Read 10 words from a.example.");
  assert.equal(said(shown, "action-said"), "read: Read 10 words from a.example.");
  shown.pass(200);
  settle("b", "Read 20 words from b.example.");
  settle("c", "Read 30 words from c.example.");
  assert.equal(said(shown, "action-said"), "read: Read 10 words from a.example.", "nothing more inside the second");
  assert.equal(shown.later.length, 1, "one sentence waits, not two");
  assert.equal(shown.later[0].ms, SAID_GAP - 200);
  shown.pass(SAID_GAP - 200);
  shown.run();
  assert.equal(said(shown, "action-said"), "read: Read 30 words from c.example.");
});

/* ---------------------------- only running work glows ---------------------------- */

/** Work the window watched start in `phase`, as the live stream names it, `ago` milliseconds ago. */
const startedRead = (invocation, phase, startedAt = 1) => Entries.entryFromOperation({
  invocation, workspace: "w1", tool: "browser_read", activity: "Reading page", capability: "read",
  phase, started_at_ms: startedAt
});

test("waiting and held work is still: it never wears the live look, and its row says why it has no time", () => {
  const shown = clocked();
  const running = startedRead("run", "running");
  const waiting = startedRead("wait", "waiting");
  const held = startedRead("hold", "held");
  shown.view.rebuildFeed([running, waiting, held]);
  assert.ok(shown.el("hero").classes.has("live"), "running work looks live");
  const [waitRow, heldRow] = queueRows(shown);
  for (const [row, phase, word] of [[waitRow, "waiting", "Waiting"], [heldRow, "held", "Held"]]) {
    assert.ok(row.className.split(" ").includes(phase), row.className);
    assert.ok(!row.className.split(" ").includes("running"), `${phase} work is not painted as running`);
    const cells = drawn(row.innerHTML);
    assert.equal(cells.wearing("row-dur")[0].textContent, word);
    assert.ok(cells.wearing("sr-only").some((node) => node.textContent === `, ${phase}`), `a screen reader hears ${phase}`);
  }
  for (const phase of ["waiting", "held"]) {
    const card = clocked();
    card.view.hero(startedRead(phase, phase), false);
    assert.ok(!card.el("hero").classes.has("live"), `a ${phase} card has no glow`);
    assert.ok(card.el("hero").classes.has(phase));
    assert.equal(drawn(card.el("hero-right").innerHTML).wearing("outcome")[0].textContent, Words.PHASE_WORD[phase]);
  }
  assert.equal(naming(".row.waiting .row-dur")[0].declarations.color, "var(--ink-3)");
  assert.equal(naming(".row.held .row-dur")[0].declarations.color, "var(--held)");
  assert.equal(naming(".hero.held .outcome")[0].declarations.color, "var(--held)");
  assert.ok(naming(".hero.waiting .elapsed").length && naming(".hero.held .elapsed").length, "a pending clock is in ink");
});

test("the band counts only running work as running, and what waits or is held under its own word", () => {
  assert.deepEqual(Words.workCounts({ running: 0, waiting: 2 }), [[0, "running"], [2, "waiting"]]);
  assert.deepEqual(Words.workCounts({ running: 0, held: 2, paused: true }), [[2, "held"]]);
  assert.deepEqual(Words.workCounts({ running: 1, held: 2, paused: true }), [[1, "running"], [2, "held"]]);
  assert.deepEqual(Words.workCounts({ running: 0, paused: true }), [[0, "running"]]);
  const shown = glance();
  shown.view.band({ connected: true, snapshot: shown.snapshot, runtime: "active", running: 0, waiting: 2, held: 0 });
  assert.match(shown.el("state-facts").innerHTML, /<span class="fact fact-work"> &middot; <b>0<\/b> running, <b>2<\/b> waiting<\/span>/);
  shown.view.band({ connected: true, snapshot: shown.snapshot, runtime: "held", running: 0, waiting: 0, held: 2 });
  assert.match(shown.el("state-facts").innerHTML, /<span class="fact fact-work"> &middot; <b>2<\/b> held<\/span>/);
});

test("every clock in the window reads one way: tenths, then minutes and seconds, then hours", () => {
  assert.equal(Words.duration(5_600), "5.6s");
  assert.equal(Words.duration(59_990), "59.9s", "a running clock never shows 60.0s");
  assert.equal(Words.duration(65_000), "1m 05s");
  assert.equal(Words.duration(3_909_000), "1:05:09");
  assert.equal(Words.stopwatch, undefined, "there is no second clock format");
  const shown = clocked();
  const running = startedRead("run", "running", 10_000 - 65_000);
  const row = startedRead("row", "running", 10_000 - 5_600);
  shown.view.rebuildFeed([running, row]);
  assert.equal(drawn(shown.el("hero-right").innerHTML).wearing("elapsed")[0].textContent, "1m 05s");
  assert.equal(drawn(queueRows(shown)[0].innerHTML).wearing("row-dur")[0].textContent, "5.6s");
});

test("a burst of quick actions replaces the card in place and keeps it live between them", () => {
  const shown = clocked();
  const first = startedRead("a", "running");
  shown.view.promote(first, null, [first]);
  assert.ok(shown.el("hero-body").classes.has("swap-in"), "the first action after a quiet card swaps in");
  shown.el("hero-body").classes.delete("swap-in");
  shown.pass(150);
  const done = Entries.entryFromRecord(settledRead({ invocation: "a" }), first);
  shown.view.hero(done, false);
  assert.ok(shown.el("hero").classes.has("live"), "a quick settle holds the live look");
  shown.pass(150);
  const second = startedRead("b", "running");
  shown.view.promote(second, done, [second, done]);
  assert.ok(!shown.el("hero-body").classes.has("swap-in"), "a card shown for less than the dwell is replaced in place");
  assert.doesNotMatch(queueRows(shown)[0].className, /\blanding\b/, "and the row below does not drop");
  const settled = Entries.entryFromRecord(settledRead({ invocation: "b" }), second);
  shown.view.hero(settled, false);
  assert.ok(shown.el("hero").classes.has("live"));
  shown.pass(600);
  shown.run();
  assert.ok(!shown.el("hero").classes.has("live"), "the live look goes once the burst is over");
  shown.pass(600);
  const third = startedRead("c", "running");
  shown.view.promote(third, settled, [third, settled]);
  assert.ok(shown.el("hero-body").classes.has("swap-in"), "after a calm card the next action swaps in again");
  const refused = clocked();
  const asked = startedRead("r", "running");
  refused.view.hero(asked, false);
  refused.view.hero(Entries.entryFromRecord(settledRead({ invocation: "r", status: "failed", outcome: "failed", summary: "Could not." }), asked), false);
  assert.ok(!refused.el("hero").classes.has("live"), "a failure shows at once");
  const paused = clocked();
  const reading = startedRead("p", "running");
  paused.view.hero(reading, false);
  paused.view.hero({ ...reading, phase: "held" }, false);
  assert.ok(!paused.el("hero").classes.has("live"), "running work that is held shows it at once");
  assert.ok(paused.el("hero").classes.has("held"));
  const queued = clocked();
  const runningFirst = startedRead("q", "running");
  queued.view.rebuildFeed([runningFirst]);
  queued.pass(100);
  queued.view.rebuildFeed([startedRead("w", "waiting")]);
  assert.ok(!queued.el("hero").classes.has("live"), "a waiting card that replaces a running one shows it at once");
  assert.ok(queued.el("hero").classes.has("waiting"));
});

test("a card that has led for five minutes with nothing running rests as the last action", () => {
  const shown = clocked();
  const entry = Entries.entryFromRecord(settledRead({ invocation: "old", timestamp_ms: 10_000, status: "failed", outcome: "failed", summary: "Could not." }), null);
  shown.view.rebuildFeed([entry]);
  assert.ok(!shown.el("hero").classes.has("rested"));
  shown.pass(5 * 60_000);
  shown.view.tickAges([entry]);
  assert.ok(shown.el("hero").classes.has("rested"));
  const body = drawn(shown.el("hero-body").innerHTML);
  assert.equal(body.wearing("kicker")[0].textContent, Words.LAST_ACTION);
  assert.equal(body.wearing("hero-age").length, 1, "it says its age");
  assert.ok(!body.wearing("hero-meta")[0].textContent.includes(" ago"), "and says it once");
  assert.ok(shown.el("hero").classes.has("failed"), "a failure keeps its edge and its word");
  assert.equal(naming(".hero.rested::after")[0].declarations.opacity, "0", "a resting card has no glow");
  const later = RULES.findIndex((rule) => rule.selectors.includes(".hero.rested::after"));
  assert.ok(later > RULES.findIndex((rule) => rule.selectors.includes(".hero.failed::after")), "even a failed one");
});

test("a failed, a kept and a refused action each say their outcome in the sentence", () => {
  const shown = clocked();
  const cases = [
    [{ invocation: "f", tool: "browser_click", status: "failed", outcome: "failed", summary: "Could not find that control." },
      "click, failed: Could not find that control."],
    [{ invocation: "k", tool: "browser_tabs", status: "blocked", outcome: "kept", summary: "Kept the tab open." },
      "tabs, kept: Kept the tab open."],
    [{ invocation: "r", tool: "browser_execute", allowed: false, status: "blocked", outcome: "refused", reason: "channel_denied",
      summary: "Your policy turns off work from MCP agents." },
      "execute, blocked: Your policy turns off work from MCP agents."],
    // A refusal with no sentence says its outcome word, never its reason code.
    [{ invocation: "e", tool: "browser_execute", allowed: false, status: "blocked", outcome: "refused", reason: "denied_by_policy", summary: "" },
      "execute, blocked: Blocked"]
  ];
  for (const [record, sentence] of cases) {
    shown.pass(SAID_GAP);
    const entry = Entries.entryFromRecord(settledRead(record), null);
    shown.view.promote(entry, null, [entry]);
    assert.equal(said(shown, "action-said"), sentence);
  }
});

test("a failed row reads like a refused one: its sentence in the soft red on the red edge, its time alone", () => {
  const { shown, cell } = rowOf(settledRead({ invocation: "fail-1", tool: "browser_click", capability: "action",
    status: "failed", outcome: "failed", effect: "none", summary: "Could not find that control." }));
  const [row] = queueRows(shown);
  assert.match(row.className, /\bfailed\b/);
  assert.equal(cell("row-dur").textContent, "1.2s", "the duration column holds the time alone");
  assert.equal(cell("row-activity").textContent, "Could not find that control.");
  for (const selector of [".row.failed .row-activity", ".row.blocked .row-activity"]) {
    assert.equal(naming(selector)[0].declarations.color, "var(--danger-soft)", selector);
  }
  for (const selector of [".row.failed", ".row.blocked"]) {
    assert.equal(naming(selector)[0].declarations["border-left-color"], "var(--danger)", selector);
  }
  assert.equal(naming(":root")[0].declarations["--danger-soft"], "#fca5a5");
  assert.ok(!RULES.some((rule) => rule.selectors.some((selector) => selector.includes("outcome-glyph"))),
    "no mark sits beside the time");
});

test("a call that did not match the catalog is drawn as the failure it is, not as a policy block", () => {
  const record = settledRead({ invocation: "invalid-1", tool: "browser_click", capability: "action",
    allowed: false, reason: "invalid_request", status: "failed", outcome: "failed", effect: "none",
    summary: "The call does not match the Ghostlight catalog." });
  const entry = Entries.entryFromRecord(record, null);
  assert.equal(Entries.isBlocked(entry), false);
  const { shown, cell, cells: details } = rowOf(record);
  const [row] = queueRows(shown);
  assert.match(row.className, /\bfailed\b/);
  assert.doesNotMatch(row.className, /\bblocked\b/);
  assert.ok(cell("row-tool").textContent.endsWith(", failed"), cell("row-tool").textContent);
  assert.ok(!details.wearing("hero-refusal").length, "no policy line under a failure");
  assert.ok(details.wearing("hero-meta")[0].textContent.includes("failed"), "the details say the failure in words");
});

test("a refusal says why once, in its sentence, and who decided on its refusal line, never as a code", () => {
  const record = settledRead({ invocation: "off-1", tool: "browser_read", allowed: false, reason: "channel_denied",
    status: "blocked", outcome: "refused", policy_tier: "user", denial_id: "deny-1",
    summary: "Your policy turns off work from MCP agents." });
  const { shown, cells: details } = rowOf(record);
  const [row] = queueRows(shown);
  assert.equal(details.wearing("hero-activity")[0].textContent, "Your policy turns off work from MCP agents.");
  assert.equal(details.wearing("hero-reason").length, 0, "no second line repeats the reason");
  assert.ok(details.wearing("hero-refusal")[0].textContent.startsWith("Refused by your own rules"));
  assert.doesNotMatch(row.innerHTML, /channel[ _]denied/);
});

test("a screen reader hears each row's outcome on its button", () => {
  const cases = [
    [{ invocation: "f", status: "failed", outcome: "failed", summary: "Could not find that control." }, ", failed"],
    [{ invocation: "u", status: "unknown", outcome: "unknown", effect: "unknown" }, ", unknown"],
    [{ invocation: "k", status: "blocked", outcome: "kept", summary: "Kept the tab open." }, ", kept"],
    [{ invocation: "r", allowed: false, reason: "denied_by_policy", status: "blocked", outcome: "refused" }, ", blocked"],
  ];
  for (const [record, heard] of cases) {
    const { cell } = rowOf(settledRead(record));
    assert.ok(cell("row-tool").textContent.endsWith(heard), `${record.invocation}: ${cell("row-tool").textContent}`);
  }
  const { cell } = rowOf(settledRead({ invocation: "fine-1" }));
  assert.doesNotMatch(cell("row-tool").textContent, /, (failed|unknown|kept|blocked)$/);
});

test("an action whose effect cannot be confirmed wears the failed look", () => {
  const { shown, cell } = rowOf(settledRead({ invocation: "unknown-1", status: "unknown", outcome: "unknown", effect: "unknown" }));
  assert.match(queueRows(shown)[0].className, /\bfailed\b/);
  assert.equal(cell("row-dur").textContent, "1.2s");
});

test("a kept row stays in ink with its time alone, and a refusal keeps the shared look", () => {
  const kept = rowOf(settledRead({ invocation: "kept-1", tool: "browser_tabs", capability: "action",
    status: "blocked", outcome: "kept", effect: "none", summary: "Kept the tab open: Preserve Ghostlight tabs is on." }));
  const keptClass = queueRows(kept.shown)[0].className;
  assert.match(keptClass, /\bkept\b/);
  assert.doesNotMatch(keptClass, /\b(failed|blocked)\b/);
  assert.equal(kept.cell("row-dur").textContent, "1.2s");

  const refused = rowOf(settledRead({ invocation: "refused-1", allowed: false, reason: "denied_by_policy", status: "blocked", outcome: "refused" }));
  const className = queueRows(refused.shown)[0].className;
  assert.match(className, /\bblocked\b/);
  assert.doesNotMatch(className, /\b(failed|kept)\b/);
  assert.equal(refused.cell("row-dur").textContent, "1.2s");

  const plain = rowOf(settledRead({ invocation: "fine-1" }));
  assert.doesNotMatch(queueRows(plain.shown)[0].className, /\b(failed|kept|blocked)\b/);
  assert.equal(plain.cell("row-dur").textContent, "1.2s");
});

test("a credential refusal row says needs you, in ink, and is never drawn as kept or blocked", () => {
  // The service classes a refused secret as needs you; its status says blocked and authority
  // admitted it, which is all the page once read, and it then said Kept.
  const record = settledRead({ invocation: "secret-1", tool: "browser_type_text", capability: "write",
    status: "blocked", outcome: "needs_you", effect: "none",
    summary: "Ghostlight did not type your password: you have not allowed Ghostlight to type it." });
  const { shown, cell } = rowOf(record);
  const className = queueRows(shown)[0].className;
  assert.match(className, /\bneeds-you\b/);
  assert.doesNotMatch(className, /\b(failed|kept|blocked)\b/);
  assert.ok(cell("row-tool").textContent.endsWith(", needs you"), cell("row-tool").textContent);
  const hero = drawing();
  hero.view.hero(Entries.entryFromRecord(record, null), false);
  assert.ok(hero.el("hero").classes.has("needs-you"));
  assert.equal(drawn(hero.el("hero-right").innerHTML).wearing("outcome")[0].textContent, "Needs you");
  assert.equal(naming(".hero.needs-you .outcome")[0].declarations.color, "var(--ink-3)");

  // A flow's step says the same of its own record, and its other steps say theirs.
  const flow = flowRecord();
  flow.steps[1].record = { ...record, invocation: "flow-1" };
  const steps = rowOf(flow).cells.wearing("step-status").map((node) => node.textContent);
  assert.deepEqual(steps, ["Completed", "Needs you", "Completed", "Not run"]);
});

test("a failed hero wears the refusal's edge and red word, and a kept one stays in ink", () => {
  const shown = drawing();
  shown.view.hero(Entries.entryFromRecord(settledRead({ status: "failed", outcome: "failed", summary: "Could not find that control." }), null), false);
  assert.ok(shown.el("hero").classes.has("failed"));
  assert.equal(drawn(shown.el("hero-right").innerHTML).wearing("outcome")[0].textContent, "Failed");
  for (const selector of [".hero.failed::before", ".hero.blocked::before"]) {
    assert.equal(naming(selector)[0].declarations.background, "var(--danger)", selector);
  }
  for (const selector of [".hero.failed .outcome", ".hero.blocked .outcome"]) {
    assert.equal(naming(selector)[0].declarations.color, "var(--danger)", selector);
  }

  const kept = drawing();
  kept.view.hero(Entries.entryFromRecord(settledRead({ status: "blocked", outcome: "kept", summary: "Kept the tab open." }), null), false);
  assert.ok(kept.el("hero").classes.has("kept"));
  assert.equal(drawn(kept.el("hero-right").innerHTML).wearing("outcome")[0].textContent, "Kept");
});

/** A window whose policy read fails `failures` times and then answers. */
async function policyWindow(failures) {
  let left = failures;
  const answer = (path) => {
    if (path === "/api/policy") {
      if (left > 0) {
        left -= 1;
        return new Error("ReferenceError: policyView is not defined");
      }
      return { headline: "No rules apply.", capabilities: [], ceilings: [], layers: [], etag: "tag-1" };
    }
    return healthy()(path);
  };
  const shown = await windowOver({ answer });
  shown.evaluate("go(\"policy\")");
  for (let turn = 0; turn < 3; turn += 1) await settle();
  return shown;
}

/** The Try again button a failure line holds, as drawn. */
const retryIn = (line) => [...drawn(line.innerHTML).root.descendants()].find((node) => node.tag === "button");

test("a policy that cannot be read says so in place of its loading line, with Try again, and no exception", async () => {
  const shown = await policyWindow(2);
  const failure = shown.el("policy-failure");
  assert.equal(failure.hidden, false);
  assert.equal(shown.el("policy-headline").hidden, true, "the loading sentence does not stay");
  const line = drawn(failure.innerHTML);
  assert.ok(line.root.children.some((node) => node.textContent === Words.LOAD_FAILED.policy));
  assert.ok(line.root.children.some((node) => node.classes.has("dot")), "the failing tone's dot");
  const retry = retryIn(failure);
  assert.equal(retry.textContent, Words.TRY_AGAIN);
  assert.ok(retry.classes.has("ghost-button"));
  assert.equal(shown.el("toast").hidden, true);
  assert.equal(shown.el("alert").hidden, true, "the page says it once, in its place");
  assert.doesNotMatch(failure.innerHTML, /ReferenceError|policyView/);
  assert.ok(shown.logged.some((detail) => String(detail).startsWith("reading the policy")), "the console keeps the cause");
  assert.equal(naming(".load-failure")[0].declarations.color, "var(--danger)");

  const reads = () => shown.requests.filter((path) => path === "/api/policy").length;
  const before = reads();
  shown.click(retry);
  for (let turn = 0; turn < 3; turn += 1) await settle();
  assert.equal(reads(), before + 1, "Try again reads the policy once more");
  assert.equal(failure.hidden, false, "a second failure still says so");

  shown.click(retryIn(failure));
  for (let turn = 0; turn < 3; turn += 1) await settle();
  assert.equal(failure.hidden, true, "a policy that reads takes the failure away");
  assert.equal(shown.el("policy-headline").hidden, false);
  assert.equal(said(shown, "policy-headline"), "No rules apply.");
});

test("Status and Integrations each have a failure line of their own", () => {
  const shown = drawing();
  for (const destination of ["status", "integrations"]) {
    const line = shown.el(`${destination}-failure`);
    assert.equal(line.hidden, true);
    assert.ok(line.closest(`#view-${destination}`));
    shown.view.loadFailed(destination);
    assert.equal(line.hidden, false);
    assert.equal(retryIn(line).dataset.retry, destination);
    assert.ok(drawn(line.innerHTML).root.children.some((node) => node.textContent === Words.LOAD_FAILED[destination]));
    shown.view.loaded(destination);
    assert.equal(line.hidden, true);
  }
});

test("a failure the service did not word is said in the window's words, as an alert that stays until dismissed", async () => {
  const shown = await windowOver({ answer: healthy() });
  shown.evaluate("sayFailure(new Error(\"TypeError: Failed to fetch\"))");
  assert.equal(said(shown, "alert-text"), Words.NO_ANSWER);
  assert.equal(shown.el("alert").hidden, false);
  assert.match(/<div[^>]*\bid="alert"[^>]*>/.exec(markup)[0], /role="alert"/);
  for (const handler of shown.el("alert-dismiss").listeners.click) handler({});
  assert.equal(shown.el("alert").hidden, true);
});

test("a success after a failure takes the alert away, and the two notices never share a spot", () => {
  const shown = drawing();
  shown.view.toast(Words.NO_ANSWER, true);
  assert.equal(shown.el("alert").hidden, false);
  shown.view.toast(Words.AGENTS.rechecked);
  assert.equal(shown.el("toast").hidden, false);
  assert.equal(shown.el("alert").hidden, true, "the window no longer says something failed once it worked");
  shown.view.toast(Words.NO_ANSWER, true);
  assert.equal(shown.el("toast").hidden, false, "a failure while a success shows leaves both to read");

  const column = shown.el("alert").parent;
  assert.equal(column, shown.el("toast").parent, "the alert and the toast sit in one column");
  assert.ok(column.classes.has("notices"));
  const top = (selector) => naming(selector).filter((rule) => rule.media === null);
  assert.equal(top(".notices")[0].declarations.position, "fixed");
  assert.equal(top(".notices")[0].declarations["flex-direction"], "column");
  for (const selector of [".toast", ".toast.error"]) {
    for (const rule of top(selector)) assert.equal(rule.declarations.position, undefined, `${selector} is placed by its column`);
  }
});

/* --------------------------------- dialog focus -------------------------------- */

/** The view over the page, with the document's focus following the fake elements' own. */
function focusable() {
  const shown = drawing();
  Object.defineProperty(shown.document, "activeElement", { get: () => Element.focused ?? null, configurable: true });
  const key = (name, shiftKey = false) => {
    let prevented = false;
    shown.dispatch("keydown", { key: name, shiftKey, preventDefault: () => { prevented = true; } });
    return prevented;
  };
  return { ...shown, key };
}

test("the search palette takes focus, keeps Tab inside, and gives focus back to the control that opened it", () => {
  const shown = focusable();
  const opener = shown.el("refresh-status");
  opener.focus();
  shown.view.openPalette();
  assert.equal(Element.focused, shown.el("palette-query"));
  assert.equal(shown.el("app-shell").inert, true, "the window behind the palette is inert");

  const hit = new Element("button", { class: "hit" });
  shown.el("palette").querySelectorAll = () => [shown.el("palette-query"), hit];
  hit.focus();
  assert.equal(shown.key("Tab"), true);
  assert.equal(Element.focused, shown.el("palette-query"), "Tab from the last stop wraps to the first");
  assert.equal(shown.key("Tab", true), true);
  assert.equal(Element.focused, hit, "Shift+Tab from the first stop wraps to the last");

  shown.view.closePalette();
  assert.equal(shown.el("app-shell").inert, false);
  assert.equal(Element.focused, opener);
  assert.equal(shown.key("Tab"), false, "once closed, Tab is the page's again");
});

test("a search hit says what it found in words: its kind, its second line with the age, and how an action ended", () => {
  const shown = drawing();
  shown.view.searchResults([
    { kind: "history", id: "read-1", title: "navigate: Opened sylin.org.", detail: "Claude Code, Google Chrome",
      timestamp_ms: Date.now() - 4 * 60_000, outcome: "refused", view: "monitor" },
    { kind: "installation", id: "codex", title: "Codex", detail: "Connected", timestamp_ms: null, view: "integrations" }
  ]);
  const hits = drawn(shown.el("palette-results").innerHTML);
  assert.deepEqual(hits.wearing("hit-kind").map((node) => node.textContent), ["Action", "Agent"]);
  const lines = [...hits.root.descendants()].filter((node) => node.tag === "small").map((node) => node.textContent);
  assert.deepEqual(lines, ["Claude Code, Google Chrome, 4m ago", "Connected"]);
  const outcomes = hits.wearing("hit-outcome");
  assert.equal(outcomes[0].textContent, "Blocked");
  assert.ok(outcomes[0].classes.has("refused"), "a refusal says so in its row's colour");
  assert.equal(naming(".hit-outcome.refused")[0].declarations.color, "var(--danger)");
  assert.equal(outcomes[1].textContent, "");
  assert.deepEqual(hits.wearing("hit").map((node) => node.dataset.searchId), ["read-1", "codex"]);
  // Every kind of hit the service words (workbench SearchKind) has a word of its own here.
  assert.deepEqual(Object.keys(Words.HIT_KIND).sort(),
    ["browser", "configuration", "destination", "diagnostic", "history", "installation", "operation", "session"]);
  // Work in flight is worded with its phase on the second line, so its chip claims none.
  // The service's phase words: waiting, running, held, blocked, finished (workbench OperationPhase).
  for (const phase of ["waiting", "running", "held", "blocked", "finished"]) {
    assert.notEqual(Words.HIT_KIND.operation.toLowerCase(), phase, phase);
  }

  shown.view.searchResults([]);
  assert.equal(drawn(shown.el("palette-results").innerHTML).wearing("palette-empty")[0].textContent, Words.SEARCH_FOUND_NOTHING);
});

test("search is offered beside the list, and a list too long to show whole ends with a way to all of it", () => {
  assert.match(markup, /<button class="ghost-button queue-search"[^>]*data-open-search[^>]*>[\s\S]*?Search<kbd aria-hidden="true">Ctrl K<\/kbd>/);
  assert.match(markup, /placeholder="Search agents and history"/);
  const shown = drawing();
  const answer = (count) => ({ ...Transport.flatten(snapshotAnswer()), history: Array.from({ length: count }, () => settledRead()) });
  const end = () => shown.el("queue").children.at(-1);
  const full = Array.from({ length: Words.FEED_LIMIT }, (_, index) => Entries.entryFromRecord(settledRead({ invocation: `r${index}` }), null));
  shown.view.rebuildFeed(full);
  shown.view.band({ connected: true, snapshot: answer(410), runtime: "active", running: 0 });
  assert.equal(end().hidden, false);
  assert.equal(end().textContent, "Search all 410 (Ctrl+K)");
  assert.ok("openSearch" in end().dataset, "the line opens the palette");
  assert.ok(end().classes.has("queue-more"));

  shown.view.band({ connected: true, snapshot: answer(Words.FEED_LIMIT), runtime: "active", running: 0 });
  assert.equal(end().hidden, true, "a list that shows every recorded action has nothing more to offer");
  shown.view.band({ connected: true, snapshot: answer(410), runtime: "active", running: 0 });
  shown.view.rebuildFeed(full.slice(0, 3));
  assert.equal(end().hidden, true, "a list that is not full is the whole of what this view shows");
  assert.ok(naming(".queue-more").some((rule) => rule.declarations.color === "var(--ink-4)"), "the line is quiet");
});

test("the line that ends a full list keeps focus while work arrives at the top", () => {
  const shown = focusable();
  const full = Array.from({ length: Words.FEED_LIMIT }, (_, index) => Entries.entryFromRecord(settledRead({ invocation: `r${index}` }), null));
  shown.view.rebuildFeed(full);
  shown.view.band({ connected: true, snapshot: { ...Transport.flatten(snapshotAnswer()), history: Array.from({ length: 410 }, () => settledRead()) },
    runtime: "active", running: 0 });
  const end = shown.el("queue").children.at(-1);
  assert.ok(end.classes.has("queue-more"));
  end.focus();
  const arriving = Entries.entryFromRecord(settledRead({ invocation: "arriving" }), null);
  shown.view.promote(arriving, full[0], [arriving, ...full]);
  assert.equal(Element.focused, end, "a row arriving above does not take the person's place");
  assert.equal(shown.el("queue").children.at(-1), end);
  assert.equal(shown.el("queue").children.filter((child) => child === end).length, 1);
});

test("a hit for an action the list holds opens that row's details and gives its button focus", () => {
  const shown = focusable();
  shown.view.rebuildFeed([
    Entries.entryFromRecord(settledRead({ invocation: "newest" }), null),
    Entries.entryFromRecord(settledRead(), null)
  ]);
  const [row] = queueRows(shown);
  const button = new Element("button", { "data-action-details": "read-1:action" });
  button.setAttribute("aria-controls", "action-details-read-1");
  button.getAttribute = (name) => button.attributes[name];
  const panel = new Element("div", { hidden: "" });
  row.querySelector = (selector) => (selector === "[data-action-details]" ? button : null);
  const byId = shown.document.getElementById;
  shown.document.getElementById = (id) => (id === "action-details-read-1" ? panel : byId(id));
  assert.equal(shown.view.showAction("read-1"), true);
  assert.equal(panel.hidden, false);
  assert.equal(button.attributes["aria-expanded"], "true");
  assert.equal(Element.focused, button);
  assert.equal(shown.view.showAction("not-listed"), false, "an action past the list is found, not opened");
});

test("ArrowDown and ArrowUp walk from the search box through the hits, and a focused hit wears the ring", () => {
  const shown = focusable();
  shown.view.openPalette();
  const first = new Element("button", { class: "hit" });
  const second = new Element("button", { class: "hit" });
  shown.el("palette-results").querySelectorAll = () => [first, second];
  const press = (name) => {
    for (const handler of shown.el("palette").listeners.keydown ?? []) handler({ key: name, preventDefault() {} });
  };
  press("ArrowDown");
  assert.equal(Element.focused, first);
  press("ArrowDown");
  press("ArrowDown");
  assert.equal(Element.focused, second, "the last hit stays");
  press("ArrowUp");
  press("ArrowUp");
  assert.equal(Element.focused, shown.el("palette-query"));
  assert.ok(naming(".hit:focus-visible").some((rule) => /^2px solid/.test(rule.declarations.outline ?? "")));
});

test("the confirmation takes focus on Cancel, and Escape gives it back to the control that opened it", async () => {
  const shown = focusable();
  const cancel = new Element("button");
  const accept = new Element("button");
  shown.el("confirm-dialog").querySelector = (selector) => (selector === "[data-confirm=\"cancel\"]" ? cancel : accept);
  const opener = new Element("button");
  opener.focus();
  const asked = shown.view.confirmRemoval("Zed", "settings.json", 0);
  assert.equal(Element.focused, cancel);
  assert.equal(shown.el("app-shell").inert, true);
  shown.key("Escape");
  assert.equal(await asked, false);
  assert.equal(shown.el("app-shell").inert, false);
  assert.equal(Element.focused, opener);
});

/** Let `node` take markup as a browser does: parsed into children, the old ones leaving the page. */
function live(node) {
  let html = "";
  Object.defineProperty(node, "innerHTML", {
    configurable: true,
    get: () => html,
    set: (value) => {
      for (const old of node.descendants()) old.isConnected = false;
      html = value;
      node.replaceChildren(...parse(value, { text: true }).children);
    }
  });
  node.querySelectorAll = (selector) => [...node.descendants()].filter((found) => found.matches(selector));
  node.querySelector = (selector) => node.querySelectorAll(selector)[0] ?? null;
}

/**
 * The Integrations page of Zed, connected, and Codex, drawn into a rail and a pane that parse their
 * markup, with the document's focus following the fake elements' own and the confirmation's buttons.
 */
function liveAgents() {
  const shown = drawing();
  Object.defineProperty(shown.document, "activeElement", { get: () => Element.focused ?? null, configurable: true });
  const cancel = new Element("button");
  shown.el("confirm-dialog").querySelector = (selector) => (selector === "[data-confirm=\"cancel\"]" ? cancel : new Element("button"));
  live(shown.el("agents-list"));
  live(shown.el("agents-pane"));
  const roster = (zed) => {
    const answer = snapshotAnswer();
    answer.parts.integrations.harnesses = [agent("zed", "Zed", zed), agent("codex", "Codex", "available")];
    return Transport.flatten(answer);
  };
  shown.view.collections(roster("installed"), new Set());
  shown.view.showAgent("zed");
  const control = (action) => [...shown.el("agents-pane").descendants()].find((node) => node.dataset.harnessAction === action);
  const key = (name) => shown.dispatch("keydown", { key: name, preventDefault() {} });
  return { ...shown, roster, control, key, cancel };
}

test("after a confirmed Remove the redrawn pane hands focus to the agent's rail entry, never to the page", async () => {
  const shown = liveAgents();
  const remove = shown.control("uninstall");
  assert.ok(remove, "Zed, connected, offers Remove");
  remove.focus();
  const asked = shown.view.confirmRemoval("Zed", "settings.json", 0);
  assert.equal(Element.focused, shown.cancel);
  shown.view.answerConfirmation(true);
  assert.equal(await asked, true);
  assert.equal(Element.focused, remove, "the answer gives focus back to Remove, which is still on the page");

  // The app then marks the agent busy, which redraws the pane with Remove waiting, then settles it.
  shown.view.collections(shown.roster("installed"), new Set(["zed"]));
  assert.equal(remove.isConnected, false, "the redraw replaced the pressed Remove");
  assert.equal(shown.control("uninstall").disabled, true);
  const entry = () => Element.focused;
  assert.notEqual(entry().isConnected, false, "focus is on an element the page still holds");
  assert.equal(entry().dataset.agent, "zed", "focus waits on Zed's rail entry while Remove works");
  assert.ok(shown.el("agents-list").contains(entry()));

  shown.view.collections(shown.roster("available"), new Set());
  assert.equal(shown.control("uninstall"), undefined, "Zed, removed, no longer offers Remove");
  assert.equal(entry().dataset.agent, "zed", "the settled redraw keeps focus on Zed's entry");
  assert.ok(shown.el("agents-list").contains(entry()));
  assert.notEqual(entry(), shown.document.body);
});

test("a pane redrawn while the confirmation is open gives focus back to the redrawn control", async () => {
  const shown = liveAgents();
  shown.control("uninstall").focus();
  const asked = shown.view.confirmRemoval("Zed", "settings.json", 0);
  const session = glanceSession("w1", "Zed", { tabs: 1 });
  shown.view.collections({ ...shown.roster("installed"), sessions: [session], harnesses_in_use: ["zed"] }, new Set());
  const redrawn = shown.control("uninstall");
  assert.equal(Element.focused, shown.cancel, "the dialog keeps focus while the pane behind it redraws");
  shown.key("Escape");
  assert.equal(await asked, false);
  assert.equal(Element.focused, redrawn, "Cancel lands on the Remove the person sees, not the one the redraw replaced");
});

/* --------------------------- one legible type and colour --------------------------- */

const SHEETS = Object.freeze({
  "styles.css": rulesOf(readFileSync(join(ui, "styles.css"), "utf8")),
  "kit/kit.css": rulesOf(readFileSync(join(ui, "kit", "kit.css"), "utf8"))
});

/** The tokens the kit keeps for strokes and borders (--muted) and for decoration (--faint). */
const DECORATION = /var\(--(muted|faint)\)/;

/** The px size a font or font-size declaration sets, or null when it names a token instead. */
const pxOf = (value) => Number(/(?:^|\s)(\d+(?:\.\d+)?)px/.exec(value ?? "")?.[1] ?? NaN) || null;

test("no word in the window is set in a token kept for strokes and decoration", () => {
  for (const [file, rules] of Object.entries(SHEETS)) {
    const offenders = rules.filter((rule) => DECORATION.test(rule.declarations.color ?? ""))
      .map((rule) => rule.selectors.join(", "));
    assert.deepEqual(offenders, [], `${file} sets text in --muted or --faint`);
  }
  // The two tokens keep their work as strokes, borders and decoration, so the rule above measures
  // a real choice and not a palette that lost them.
  const decorating = SHEETS["styles.css"].filter((rule) => Object.entries(rule.declarations)
    .some(([property, value]) => property !== "color" && DECORATION.test(value)));
  assert.ok(decorating.length > 0, "nothing draws in --muted or --faint any longer");

  const root = naming(":root")[0].declarations;
  const kitInk = /--ink-4:\s*(#[0-9a-f]{6})/.exec(readFileSync(join(ui, "..", "..", "..", "extension", "kit.css"), "utf8"))[1];
  assert.equal(root["--ink-4"], kitInk, "the window's quietest text is the kit's");
});

/**
 * Teal means live: the selectors allowed to draw in the accent, by why. Everything else at rest is
 * ink. The About card's trading card (.tcg) is artwork and keeps its teal.
 */
const ACCENT = Object.freeze({
  "work running now": ["body.runtime-working .lamp-halo", "body.runtime-working .lamp-core", "body.runtime-working .state-word",
    ".glance-face.busy", ".glance-chip.busy", ".glance-card-head.busy", ".glance-chip.busy .symbol", ".glance-card-head.busy .symbol",
    ".hero::after", ".hero.live .elapsed", ".hero.live .med", ".hero.live .med svg", ".row.running .row-dur",
    ".row.running .med-mini svg", ".agent-entry-sub.live", ".agent-dot.d-live", ".agent-dot.d-live::after",
    ".agent-state.t-live", ".agent-use.live"],
  "a control with focus": [":focus-visible", ".hit:focus-visible", ".about-link:focus-visible", ".agents-search input:focus",
    ".rule-detail input[type=\"text\"]:focus", "#sacred-hosts:focus", ".toggle input:focus-visible ~ .toggle-track",
    ".rule-row:focus-visible .rule-edit-hint"],
  "the pointer on a control": [".about-link:hover .about-link-title", ".rule-row:hover .rule-edit-hint"],
  "an enabled primary": [".primary-button:not(:disabled)", ".primary-button:hover:not(:disabled)"]
});

test("teal means live: only running work, focus, the pointer and an enabled primary draw in the accent", () => {
  const accent = /var\(--(a|al|argb)\)|#5eead4|94,\s*234,\s*212/i;
  const allowed = new Set(Object.values(ACCENT).flat());
  const drawn = [];
  for (const [file, rules] of Object.entries(SHEETS)) {
    for (const rule of rules.filter((each) => Object.values(each.declarations).some((value) => accent.test(value)))) {
      for (const selector of rule.selectors.filter((each) => each !== ":root" && !each.startsWith(".tcg"))) {
        assert.ok(allowed.has(selector), `${file}: ${selector} draws in the accent at rest`);
        drawn.push(selector);
      }
    }
  }
  assert.deepEqual([...allowed].filter((selector) => !drawn.includes(selector)), [], "an allowed selector is gone");
});

/** Artwork and shapes keep their own geometry: the trading card, monograms, icon tiles and scrollbars. */
const SHAPED = /^(\.tcg|\.glance-face|\.symbol|.*scrollbar)/;

test("every word is set on one type scale and every corner on the kit's radii", () => {
  const sizes = new Set([10, 11.5, 12, 12.5, 13, 14.5, 17, 22, 25]);
  const radii = new Set(["4px", "6px", "8px", "12px", "999px", "50%"]);
  for (const [file, rules] of Object.entries(SHEETS)) {
    for (const rule of rules.filter((each) => !each.selectors.every((selector) => SHAPED.test(selector)))) {
      const size = pxOf(rule.declarations.font) ?? pxOf(rule.declarations["font-size"]);
      if (size !== null) assert.ok(sizes.has(size), `${file}: ${rule.selectors.join(", ")} is set at ${size}px`);
      const radius = rule.declarations["border-radius"];
      if (radius) {
        for (const corner of radius.split(/\s+/).filter((each) => each !== "0")) {
          assert.ok(radii.has(corner), `${file}: ${rule.selectors.join(", ")} rounds at ${radius}`);
        }
      }
    }
  }
});

test("the read capability keeps its quiet edge while its word reads in ink, and the settled clock reads", () => {
  const read = naming(".cap-read")[0].declarations;
  assert.equal(read["--cap"], "var(--cap-read)");
  assert.equal(read["--cap-ink"], "var(--ink-3)");
  assert.equal(naming(":root")[0].declarations["--cap-read"], "#52525b");
  for (const capability of ["action", "write", "execute"]) {
    assert.equal(naming(`.cap-${capability}`)[0].declarations["--cap-ink"], `var(--cap-${capability})`, capability);
  }
  for (const selector of [".row-cap", ".hero-tool .cap-label"]) {
    assert.match(naming(selector).map((rule) => rule.declarations.color ?? "").join(" "), /var\(--cap-ink/, selector);
  }
  const clock = naming(".elapsed")[0].declarations;
  assert.equal(clock.color, "var(--ink-4)");
  assert.match(clock.font, /^400 /, "the settled stopwatch is not set in a hairline weight");
});

test("only a data tag, in capitals, is as small as 10px, and every other word is larger", () => {
  // A monogram inside a 20px face and the sylin.org trading card, ported whole, are artwork.
  const artwork = (selector) => selector === ".glance-face" || selector.startsWith(".tcg");
  for (const [file, rules] of Object.entries(SHEETS)) {
    for (const rule of rules) {
      if (rule.selectors.every(artwork)) continue;
      const size = pxOf(rule.declarations.font) ?? pxOf(rule.declarations["font-size"]);
      if (size === null) continue;
      const tag = rule.declarations["text-transform"] === "uppercase";
      assert.ok(size >= (tag ? 10 : 10.5), `${file}: ${rule.selectors.join(", ")} is set at ${size}px`);
    }
  }
});

const EYEBROWS = [".kicker", ".queue-head", ".agents-group-head", ".about-group h2", ".about-facts dt"];
const TAGS = [".cap-label", ".outcome", ".row-cap", ".row-tool-cap", ".agent-state", ".severity", ".hit-kind", ".cap-state",
  ".rule-owner", ".rule-edit-hint", ".rule-detail dt"];

test("a place is named by one eyebrow and a value by one data tag, on every tab", () => {
  const voice = (selectors, font) => {
    const rule = naming(selectors[0]).find((each) => each.declarations.font);
    assert.ok(rule, `${selectors[0]} has no voice`);
    assert.equal(rule.declarations.font, font);
    assert.equal(rule.declarations["text-transform"], "uppercase");
    assert.equal(rule.declarations.color, "var(--ink-4)");
    for (const selector of selectors) {
      assert.ok(rule.selectors.includes(selector), `${selector} speaks in a voice of its own`);
      for (const other of naming(selector).filter((each) => each !== rule)) {
        for (const property of ["font", "font-size", "font-family", "font-weight", "letter-spacing", "text-transform"]) {
          assert.equal(other.declarations[property], undefined, `${selector} changes its ${property}`);
        }
      }
    }
    return rule;
  };
  const eyebrow = voice(EYEBROWS, "500 11.5px/1.4 var(--sans)");
  assert.equal(eyebrow.declarations["letter-spacing"], ".18em");
  const tag = voice(TAGS, "500 10px/1 var(--mono)");
  assert.equal(tag.declarations["letter-spacing"], ".14em");

  // Nothing else in the window speaks in capitals, and the setup view's eyebrow stays the kit's.
  const capitals = SHEETS["styles.css"].filter((rule) => rule.declarations["text-transform"] === "uppercase"
    && rule.selectors.join() !== eyebrow.selectors.join() && rule.selectors.join() !== tag.selectors.join()
    && !rule.selectors.every((selector) => selector.startsWith(".tcg")));
  assert.deepEqual(capitals.map((rule) => rule.selectors.join(", ")), []);
  assert.deepEqual(SHEETS["styles.css"].filter((rule) => rule.selectors.some((selector) => selector.includes("gl-eyebrow"))), []);
});

test("every destination's title wears the one page title", () => {
  const view = parsePage();
  const titles = [...view.descendants()].filter((node) => node.tag === "h1" && node.closest("#main-content"));
  assert.ok(titles.length >= 5, `saw ${titles.length} page titles`);
  const title = naming(".page-heading h1").find((rule) => rule.declarations.font);
  assert.equal(title.declarations.font, "300 25px/1.25 var(--sans)");
  assert.equal(title.declarations.color, "var(--ink)");
  for (const node of titles) {
    const place = node.closest(".page-heading") ? ".page-heading h1" : node.closest(".about-copy") ? ".about-copy h1" : null;
    assert.ok(place && title.selectors.includes(place), `the title #${node.id} has no page title`);
  }
  const resized = RULES.filter((rule) => rule !== title && rule.selectors.some((selector) => / h1$/.test(selector))
    && ["font", "font-size", "font-weight", "letter-spacing"].some((property) => rule.declarations[property]));
  assert.deepEqual(resized.map((rule) => rule.selectors.join(", ")), [], "a destination resizes its title");
});

/** The kit's layout containers: the page draws them in index.html, and only there. */
const KIT_LAYOUT = ["settings", "settings-rail", "settings-body", "settings-section"];

test("Policy's restrictions are a plain list, never the kit's Settings grid", () => {
  const shown = drawing();
  shown.view.policy({
    headline: "Example Org sets the rules.", organization: null, capabilities: [], ceilings: [],
    layers: [{
      kind: "organization", title: "Example Org", rules: [], policy_name: "support", version: "3",
      path: "/etc/ghostlight/policy.json", document: "{}",
      settings: [
        { key: "browser.startup", value: "\"manual\"" },
        { key: "example.one", value: "false" },
        { key: "example.two", value: "true" }
      ]
    }],
    user_layer: { source: "managed", editable: false, path: null, blocked_reason: null }
  });
  assert.deepEqual(shown.failures, []);
  const list = [...drawn(shown.el("policy-settings").innerHTML).root.descendants()].find((node) => node.tag === "ul");
  assert.equal(list.children.length, 3);
  assert.deepEqual([...list.classes], ["layer-settings"]);
  for (const name of list.classes) {
    for (const rule of naming(`.${name}`)) {
      assert.notEqual(rule.declarations.display, "grid", `.${name} lays the list out as a grid`);
      assert.equal(rule.declarations["grid-template-columns"], undefined, `.${name} lays the list out in columns`);
    }
  }
  // The kit's Settings grid is a grid, and the window's sheet leaves it where its heading is.
  assert.ok(naming(".settings").some((rule) => rule.declarations.display === "grid"));
  assert.deepEqual(naming(".settings", SHEETS["styles.css"]), [], "styles.css restyles the kit's Settings");
});

test("the view draws none of the kit's layout containers", () => {
  const source = VIEW_SOURCE;
  const drawnClasses = [...source.matchAll(/class="([^"]*)"/g)].flatMap(([, names]) => names.split(/\s+/));
  assert.ok(drawnClasses.length > 0, "the view draws no classes at all");
  for (const name of KIT_LAYOUT) assert.equal(drawnClasses.includes(name), false, `view.js draws .${name}`);
});

test("a disclosure is quiet ink with the agents fold's chevron, and opens to ink", () => {
  const shown = drawing();
  shown.view.policy({
    headline: "", organization: null, capabilities: [], ceilings: [],
    layers: [{ kind: "organization", title: "Example Org", rules: [], settings: [], policy_name: "support", version: "3", path: null, document: "{}" }],
    user_layer: { source: "managed", editable: false, path: null, blocked_reason: null }
  });
  const summary = [...drawn(shown.el("policy-documents").innerHTML).root.descendants()].find((node) => node.tag === "summary");
  assert.ok(summary.classes.has("disclosure"));
  assert.ok(summary.children.some((node) => node.tag === "svg"), "the disclosure has no chevron");

  const source = VIEW_SOURCE;
  const summaries = [...source.matchAll(/<summary class="([^"]*)"/g)].map(([, names]) => names);
  assert.equal(summaries.length, [...source.matchAll(/<summary/g)].length, "a summary is drawn with no class");
  assert.deepEqual(summaries.filter((names) => names !== "disclosure" && names !== "glance-pill"), []);

  const rest = naming(".disclosure")[0].declarations;
  assert.equal(rest.color, "var(--ink-3)");
  assert.equal(rest["font-size"], "12px");
  assert.equal(naming("details[open] > .disclosure")[0].declarations.color, "var(--ink)");
  assert.match(naming("details[open] > .disclosure svg")[0].declarations.transform, /rotate\(180deg\)/);
});

/** Every stylesheet, script and page under ui/, by its path below it. */
function uiFiles(directory = ui, below = "") {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    const named = below ? `${below}/${entry.name}` : entry.name;
    if (entry.isDirectory()) return uiFiles(path, named);
    return /\.(css|js|html)$/.test(entry.name) ? [[named, readFileSync(path, "utf8")]] : [];
  });
}

test("the page's sky blue never reaches the window, and nothing is coloured inline", () => {
  const files = uiFiles();
  assert.ok(files.some(([name]) => name === "lib/view.js"), "the walk missed the view");
  for (const [name, text] of files) {
    assert.equal(text.includes("--sky"), false, `${name} names --sky`);
    // The window's policy allows no inline style, so one is dropped without a word.
    assert.equal(/\sstyle\s*=/.test(text), false, `${name} carries an inline style`);
  }
  const orbs = [...parsePage().descendants()].filter((node) => node.classes.has("orb"));
  assert.deepEqual(orbs.map((orb) => [...orb.classes].find((name) => name !== "orb")), ["orb-agent", "orb-trust"]);
  assert.equal(naming(".tcg .orb-agent")[0].declarations.color, "var(--ok)");
  assert.equal(naming(".tcg .orb-trust")[0].declarations.color, "var(--a)");
});

test("the agent pane's header is the agent's icon beside its name and state, with no mascot and no wire", () => {
  const shown = agentsPage(ROSTER, { sessions: [glanceSession("w1", "Zed", { busy: true })], inUse: ["zed"] });
  shown.pick("zed");
  const header = shown.pane().wearing("agent-hero")[0];
  assert.deepEqual(header.children.map((node) => [...node.classes][0]), ["symbol", "agent-hero-text"]);
  assert.equal(header.children[0].attributes["aria-hidden"], "true", "the icon is decoration");
  assert.equal(shown.title(), "Zed", "the pane keeps its accessible name");
  assert.equal(shown.pane().wearing("agent-state")[0].textContent, "Working", "the state is said in words beside its dot");
  assert.doesNotMatch(shown.el("agents-pane").innerHTML, /mascot|agent-wire|agent-ghost|agent-link/);
  assert.ok(!RULES.some((rule) => rule.selectors.some((selector) => /agent-(wire|ghost|link)/.test(selector))),
    "nothing in the stylesheet serves the wire or the mascot");
  assert.equal(naming(".agent-hero")[0].declarations["align-items"], "center");
  assert.ok(!RULES.some((rule) => rule.selectors.includes(".agent-hero") && rule.declarations["flex-direction"] === "column"),
    "the icon stays beside the text at every width");
});

/* ------------------------------ one vocabulary ------------------------------ */

test("the band and About count what the tray counts", () => {
  // Four sessions of three agents: Claude Code, Codex twice, and the command line. The service
  // counts them as At a glance groups them, so the band, the pill and About agree.
  const shown = glance();
  shown.view.band({ connected: true, snapshot: shown.snapshot, runtime: "active", running: 0 });
  assert.match(shown.el("state-facts").innerHTML,
    /^<span class="fact fact-agents"><b>3<\/b> agents<\/span><span class="fact fact-browsers"> &middot; <b>1<\/b> browser<\/span>/);
  assert.equal(shown.read().wearing("glance-side")[0].textContent, "3 agents");
  const facts = () => drawn(shown.el("about-facts").innerHTML).root.children.map((node) => node.textContent);
  assert.deepEqual(facts().slice(2, 6), ["Agents", "3 connected", "Browsers", "1 attached"]);
  assert.equal(facts().includes("Sessions"), false);
  // The numbers are the connections part's, the ones the tray says: the page never counts the
  // sessions or the browsers again.
  const counted = { ...shown.snapshot, counts: { agents: 7, mcp_clients: 6, command_line: 1, browsers: 2 } };
  shown.view.band({ connected: true, snapshot: counted, runtime: "active", running: 0 });
  assert.match(shown.el("state-facts").innerHTML,
    /^<span class="fact fact-agents"><b>7<\/b> agents<\/span><span class="fact fact-browsers"> &middot; <b>2<\/b> browsers<\/span>/);
  shown.view.collections(counted, new Set());
  assert.deepEqual(facts().slice(2, 6), ["Agents", "7 connected", "Browsers", "2 attached"]);
});

/** The glance's snapshot with the controls part the service publishes for `runtime`. */
const inState = (snapshot, runtime) => ({ ...snapshot, controls: part({ runtime }) });

test("the band's wheel says Pause agents, Resume agents and Start a new session, the words of the tray and the toolbar", () => {
  const shown = glance();
  shown.view.band({ connected: true, snapshot: inState(shown.snapshot, "active"), runtime: "active", running: 0 });
  assert.equal(shown.el("wheel-label").textContent, "Pause agents");
  shown.view.band({ connected: true, snapshot: inState(shown.snapshot, "held"), runtime: "held", running: 0 });
  assert.equal(shown.el("wheel-label").textContent, "Resume agents");
  shown.view.band({ connected: true, snapshot: inState(shown.snapshot, "ended"), runtime: "ended", running: 0 });
  assert.equal(shown.el("wheel-label").textContent, "Start a new session");
  assert.equal(said(kit(), "controls-resume"), "Resume agents");
});

test("the band's wheel does the first act the controls part offers, and offers no Resume once the session ended", () => {
  const shown = glance();
  const wheel = shown.el("wheel");
  for (const [runtime, intent] of [["active", "hold"], ["held", "resume"], ["ended", "start_session"]]) {
    shown.view.band({ connected: true, snapshot: inState(shown.snapshot, runtime), runtime, running: 0 });
    assert.equal(wheel.dataset.intent, intent, runtime);
    assert.equal(wheel.hidden, false, runtime);
  }
  // The wheel reads the part, not the runtime word beside it: an ended session's part offers a new
  // session, never the Resume a paused one would.
  shown.view.band({ connected: true, snapshot: inState(shown.snapshot, "ended"), runtime: "held", running: 0 });
  assert.equal(wheel.dataset.intent, "start_session");
  assert.equal(shown.el("wheel-label").textContent, "Start a new session");
  // Before the service has named an act, the band shows neither the wheel nor a way on.
  const unnamed = inState(shown.snapshot, "active");
  delete unnamed.controls;
  shown.view.band({ connected: true, snapshot: unnamed, runtime: "active", running: 0 });
  assert.equal(wheel.hidden, true);
  assert.equal(shown.el("wheel-state").hidden, true);
  assert.equal(shown.el("band-continue").hidden, true);
});

test("Status asks whether Ghostlight is working", () => {
  const status = [...drawn(markup).root.descendants()].find((node) => node.id === "view-status");
  const kicker = [...status.descendants()].find((node) => node.classes.has("kicker"));
  assert.equal(kicker.textContent.trim(), "Is Ghostlight working");
});

/** The compiled policy of a machine with no rules anywhere, where the person may write their own. */
function openPolicy(layers = []) {
  return {
    headline: "No rules apply. Agents may work on ordinary websites, within the limits below.",
    organization: null, capabilities: [], ceilings: [], layers,
    user_layer: { source: layers.length ? "workbench" : "none", editable: true, path: "/home/ann/.ghostlight/policy.json", blocked_reason: null }
  };
}

test("with no rules the open case is said once, and how rules are checked waits for a rule", () => {
  const shown = drawing();
  shown.view.policy(openPolicy());
  assert.deepEqual(shown.failures, []);
  assert.equal(shown.el("rules-note").hidden, true);
  assert.equal(shown.el("rule-list").innerHTML, '<p class="rules-empty">No rules yet.</p>');
  assert.equal(shown.el("editor-status").textContent, "");
  assert.equal(shown.el("apply-policy").disabled, true, "nothing changed yet");
  const hint = [...drawn(markup).root.descendants()].find((node) => node.classes.has("editor-hint"));
  assert.equal(hint.textContent.trim(), "Each starts at its most open setting; you can only narrow it.");

  shown.view.addRule("example.com");
  assert.equal(shown.el("rules-note").hidden, false);
});

test("with no rules, turning one setting off applies alone and keeps the ordinary websites", () => {
  const shown = drawing();
  shown.view.policy(openPolicy());
  shown.view.setPermission("browser.tabs.allow_close", false);
  assert.equal(shown.el("apply-policy").disabled, false);
  assert.equal(shown.el("check-policy").disabled, false);
  assert.equal(shown.el("editor-status").textContent, Words.OPEN_RULE_NOTE);
  const document = JSON.parse(shown.view.draftDocument());
  assert.deepEqual(document.grants, [{
    id: "ordinary-websites", hosts: { allow: ["*"] }, allowed: ["read", "action", "write", "execute"],
    description: "Agents may work on ordinary websites."
  }]);
  assert.deepEqual(document.config, [{ key: "browser.tabs.allow_close", value: false, level: "mandatory" }]);
});

test("a draft whose rules were all removed refuses everything, and says so only then", () => {
  const shown = drawing();
  shown.view.policy(openPolicy([{
    kind: "user", title: "Your rules", policy_name: "Your rules", version: "2026-10-18", path: null, document: "{}",
    settings: [], rules: [{ id: "rule-1", allow: ["example.com"], deny: [], allowed: ["read"], description: "" }]
  }]));
  assert.equal(shown.el("rules-note").hidden, false);
  assert.equal(shown.el("editor-status").textContent, "");
  shown.view.ruleAction(0, "remove");
  assert.equal(shown.el("editor-status").textContent, Words.NO_RULES_LEFT);
  assert.equal(shown.el("apply-policy").disabled, true);
  assert.equal(shown.el("rules-note").hidden, true);
});

test("an applied policy with no rules refuses everything, and a setting never widens it", () => {
  const shown = drawing();
  shown.view.policy(openPolicy([{
    kind: "user", title: "Your rules", policy_name: "Your rules", version: "2026-10-18", path: null, document: "{}",
    settings: [{ key: "browser.startup", value: "\"manual\"", level: "mandatory" }], rules: []
  }]));
  assert.deepEqual(shown.failures, []);
  assert.equal(shown.el("editor-status").textContent, Words.NO_RULES_LEFT);
  assert.equal(shown.el("apply-policy").disabled, true);
  shown.view.setPermission("browser.tabs.allow_close", false);
  assert.equal(shown.el("editor-status").textContent, Words.NO_RULES_LEFT);
  assert.equal(shown.el("apply-policy").disabled, true);
  assert.deepEqual(JSON.parse(shown.view.draftDocument()).grants, []);
});

test("Closing tabs names the browser's Preserve Ghostlight tabs, which is set in the browser", () => {
  const shown = drawing();
  shown.view.policy(openPolicy());
  const rows = drawn(shown.el("setting-groups").innerHTML).wearing("setting-row");
  const closing = rows.find((row) => row.textContent.includes("Closing tabs"));
  const also = [...closing.descendants()].find((node) => node.classes.has("setting-also"));
  assert.ok(also, "Closing tabs does not name the browser's setting");
  assert.match(also.textContent, /^Preserve Ghostlight tabs keeps them open while it is on\. It starts on\./);
  const chip = [...also.descendants()].find((node) => node.classes.has("context-chip"));
  assert.equal(chip.textContent, "Set in the browser");
  assert.equal([...also.descendants()].some((node) => node.tag === "input" || node.tag === "button"), false,
    "the browser's setting is a line here, never a control");
});

/** The text an id names in drawn markup, or "" when nothing wears that id. */
const textOf = (root, id) => [...root.descendants()].find((node) => node.id === id)?.textContent.trim() ?? "";

test("every Policy switch is a named switch, described by its detail and, for a channel, by its limit", () => {
  const shown = drawing();
  shown.view.policy(openPolicy());
  const root = drawn(shown.el("setting-groups").innerHTML).root;
  const switches = [...root.descendants()].filter((node) => node.tag === "input" && "restriction" in node.dataset);
  assert.equal(switches.length, 6, "MCP agents, Command line, Closing tabs, Screenshots, Update check and Page-authored names");
  for (const box of switches) {
    const key = box.dataset.restriction;
    assert.equal(box.attributes.role, "switch", key);
    const name = (box.attributes["aria-labelledby"] ?? "").split(/\s+/).map((id) => textOf(root, id)).join(" ").trim();
    assert.notEqual(name, "", `${key} has no accessible name`);
    const described = (box.attributes["aria-describedby"] ?? "").split(/\s+/).filter(Boolean);
    assert.ok(described.length > 0, `${key} has no description`);
    for (const id of described) assert.notEqual(textOf(root, id), "", `${key} is described by #${id}, which says nothing`);
  }
  const limit = Words.settingLimits(["channels.mcp.enabled"])[0];
  for (const key of ["channels.mcp.enabled", "channels.cli.enabled"]) {
    const box = switches.find((node) => node.dataset.restriction === key);
    const texts = box.attributes["aria-describedby"].split(/\s+/).map((id) => textOf(root, id));
    assert.ok(texts.includes(Words.escapeHtml(limit)), `${key} is not described by the channel limit`);
  }
  const names = switches.map((box) => textOf(root, box.attributes["aria-labelledby"]));
  assert.deepEqual(names, ["MCP agents", "Command line", "Closing tabs", "Screenshots", "Update check", "Page-authored names"]);
});

test("the channel limit leads Who may work, under its heading, said plainly and kept to a reading width", () => {
  const shown = drawing();
  shown.view.policy(openPolicy());
  const group = drawn(shown.el("setting-groups").innerHTML).wearing("setting-group")[0];
  assert.deepEqual(group.children.slice(0, 2).map((node) => node.tag), ["h3", "p"]);
  assert.equal(group.children[0].textContent, "Who may work");
  const lead = group.children[1];
  assert.ok(lead.classes.has("setting-limit"));
  assert.equal(lead.textContent, Words.escapeHtml("Ghostlight's own program says whether an agent connected through MCP or the command line, "
    + "and another program running as you could claim otherwise. These switches hold agents that follow the rules; "
    + "they do not stop harmful software on this account."));
  assert.equal(drawn(shown.el("setting-groups").innerHTML).wearing("setting-limit").length, 1, "the limit is said once");
  assert.equal(naming(".setting-limit")[0].declarations["max-width"], "72ch");
  assert.equal(naming(".setting-group h3 + .setting-limit")[0].declarations.margin, "0 2px 8px");
});

test("the history choice says what it does, and auto-open speaks of the agent", () => {
  const items = Words.SETTING_GROUPS.flatMap((group) => group.items);
  const history = items.find((item) => item.key === "audit.availability");
  assert.deepEqual(history.choices.map((choice) => [choice.value, choice.label]),
    [["keep_working", "Keep working"], ["require_audit", "Stop browser work"]]);
  const manual = items.find((item) => item.key === "browser.startup").choices.find((choice) => choice.value === "manual");
  assert.match(manual.detail, /tells the agent /);
  assert.doesNotMatch(JSON.stringify(Words.SETTING_GROUPS), /\bthe model\b/);
});

/** What a watching screen is told: it may look, search, check a draft and pause, and change nothing. */
function watcherContext() {
  const open = [Transport.ACT.search, Transport.ACT.checkPolicy, Transport.ACT.rescan, Transport.ACT.continueInWindow, Transport.ACT.hold];
  return {
    context: "watch", opener: "anchor", word: "Watching", sentence: WATCHER, product: { version: "2.0.0" },
    acts: Object.fromEntries(Object.values(Transport.ACT).map((act) => [act,
      open.includes(act) ? { available: true, why_not: null } : { available: false, why_not: WHY }]))
  };
}

test("a watching page reads Policy: every draft control holds its state, under one line saying where changes are made", () => {
  const user = {
    kind: "user", title: "Your rules", policy_name: "Your rules", version: "2026-10-18", path: null, document: "{}",
    settings: [{ key: "audit.availability", value: "\"require_audit\"", level: "mandatory" }],
    rules: [{ id: "rule-1", allow: ["example.com"], deny: [], allowed: ["read"], description: "Docs" }]
  };
  assert.equal(Words.POLICY_IN_WINDOW, "Changes are made in the Ghostlight window.");
  for (const order of ["context first", "policy first"]) {
    const shown = drawing();
    if (order === "context first") shown.view.context(watcherContext());
    shown.view.policy(openPolicy([user]));
    if (order === "policy first") shown.view.context(watcherContext());
    assert.deepEqual(shown.failures, [], order);
    const groups = drawn(shown.el("setting-groups").innerHTML).root;
    const controls = [...groups.descendants()].filter((node) => node.tag === "input" || node.tag === "select");
    assert.equal(controls.length, 8, `${order}: six switches, the history choice and the startup choice`);
    for (const control of controls) {
      assert.equal(control.disabled, true, `${order}: ${control.dataset.restriction ?? control.dataset.settingChoice}`);
    }
    assert.equal(shown.el("sacred-hosts").disabled, true, order);
    assert.equal(shown.el("add-rule").hidden, true, order);
    assert.equal(shown.el("apply-policy").hidden, true, `${order}: nothing to apply`);
    const line = shown.el("policy-continue");
    assert.equal(line.hidden, false, order);
    const said = drawn(line.innerHTML).root.children.map((node) => node.textContent);
    assert.deepEqual(said, [Words.POLICY_IN_WINDOW, Words.CONTINUE_LABEL], order);
    // The person's own rule opens to be read, as an organization's does, with nothing to type into.
    const row = drawn(shown.el("rule-list").innerHTML).wearing("rule")[0];
    assert.ok(row.classes.has("rule-theirs"), order);
    shown.view.toggleRule("mine:0");
    const opened = shown.el("rule-list").innerHTML;
    assert.match(opened, /<dt>Sites<\/dt><dd>example\.com<\/dd>/, order);
    assert.doesNotMatch(opened, /<input|data-rule-action/, order);
  }

  const window = drawing();
  window.view.policy(openPolicy([user]));
  assert.equal(window.el("policy-continue").hidden, true);
  assert.equal(window.el("apply-policy").hidden, false);
  assert.equal(window.el("add-rule").hidden, false);
  const live = [...drawn(window.el("setting-groups").innerHTML).root.descendants()].filter((node) => node.tag === "input");
  assert.ok(live.every((box) => !box.disabled), "the window's switches stay switches");
});

test("a destination that cannot be read lets Try again stand in for Refresh, and hides the headings it never filled", () => {
  const shown = drawing();
  const policy = shown.el("view-policy");
  shown.view.loadFailed("policy");
  assert.ok(policy.classes.has("load-failed"));
  assert.ok(policy.classes.has("unread"), "a policy never drawn hides its empty headings");
  for (const id of ["refresh-policy", "refresh-status", "refresh-integrations"]) {
    assert.ok(shown.el(id).classes.has("refresh"), `#${id} is not a Refresh that Try again stands in for`);
  }
  const part = [...parsePage().descendants()].find((node) => node.classes.has("read-part"));
  assert.ok(part.closest("#view-policy"));
  assert.equal([...part.descendants()].filter((node) => node.tag === "h2").length, 3,
    "Right now, Always off limits and Rules sit in the part a failure hides");
  for (const selector of [".view.load-failed .refresh", ".view.load-failed.unread .read-part"]) {
    assert.equal(naming(selector)[0]?.declarations.display, "none", selector);
  }
  shown.view.loaded("policy");
  assert.equal(policy.classes.has("load-failed"), false);
  assert.equal(policy.classes.has("unread"), false);

  shown.view.policy(openPolicy());
  shown.view.loadFailed("policy");
  assert.ok(policy.classes.has("load-failed"));
  assert.equal(policy.classes.has("unread"), false, "a policy drawn before keeps what it showed");
});

test("the keyboard focus on a Refresh the failure hides, or on the Try again it redraws, moves to the new Try again", () => {
  const shown = focusable();
  const line = shown.el("policy-failure");
  let retry;
  line.querySelector = (selector) => {
    assert.equal(selector, "[data-retry]");
    retry = new Element("button", { class: "ghost-button", "data-retry": "policy" });
    line.append(retry);
    return retry;
  };

  shown.el("refresh-policy").focus();
  shown.view.loadFailed("policy");
  assert.equal(shown.document.activeElement, retry, "Refresh hands its focus to Try again");

  const before = retry;
  shown.view.loadFailed("policy");
  assert.notEqual(retry, before);
  assert.equal(shown.document.activeElement, retry, "a Try again redrawn by a second failure keeps the focus");

  const elsewhere = shown.el("refresh-status");
  elsewhere.focus();
  shown.view.loadFailed("policy");
  assert.equal(shown.document.activeElement, elsewhere, "focus outside the destination stays where it is");
});

test("the keyboard focus on Try again moves to the Refresh that comes back once the read works", () => {
  const shown = focusable();
  live(shown.el("policy-failure"));
  const view = shown.el("view-policy");
  view.querySelector = (selector) => [...view.descendants()].find((found) => found.matches(selector)) ?? null;
  shown.view.loadFailed("policy");
  shown.el("policy-failure").querySelector("[data-retry]").focus();
  shown.view.loaded("policy");
  assert.equal(shown.document.activeElement, shown.el("refresh-policy"), "Try again hands its focus to Refresh");

  const elsewhere = shown.el("refresh-status");
  shown.view.loadFailed("policy");
  elsewhere.focus();
  shown.view.loaded("policy");
  assert.equal(shown.document.activeElement, elsewhere, "focus outside the failure line stays where it is");
});
