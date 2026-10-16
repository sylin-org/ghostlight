// Settings, exercised for real.
//
// Settings is a tab of its own: a rail of sections, each showing its current state, and the
// selected section beside it. The kit draws the service's controls part into the page's own
// markup and calls back into the page when a button is pressed; the view keeps the address. These
// tests run the real kit and the real view against the real markup from index.html, parsed into a
// minimal fake document, with fake callbacks and a fake localStorage. No page, no transport and no
// Ghostlight process is involved.
//
// Run with: node --test tests/workbench-settings.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
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

const VOID = new Set(["meta", "link", "img", "input", "br", "hr", "source"]);

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
    this.classes = new Set(String(attributes.class ?? "").split(/\s+/).filter(Boolean));
    for (const [name, value] of Object.entries(attributes)) {
      if (name === "aria-current") this.attributes[name] = value;
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

/** Parse the whole page's markup into fake elements. */
function parsePage() {
  const root = new Element("root");
  const stack = [root];
  const body = markup.slice(markup.indexOf("<body"));
  for (const [, closing, tag, rest] of body.matchAll(/<(\/?)([a-z0-9]+)((?:[^>"]|"[^"]*")*)>/g)) {
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

/** A page: the real markup as fake elements, a body, and the listeners the modules attach. */
function page() {
  const root = parsePage();
  const everything = [...root.descendants()];
  const byId = new Map(everything.filter((node) => node.id).map((node) => [node.id, node]));
  const body = everything.find((node) => node.tag === "body");
  const listeners = new Map();
  const document = {
    body,
    getElementById: (id) => byId.get(id) ?? null,
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
    document, window, body, el, address,
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
  assert.ok(!/controls-sheet|controls-open|Controls sheet/.test(markup));
  assert.ok(!/\.controls-(sheet|open)/.test(styles), "the sheet's styles are still in the stylesheets");
  assert.ok(!/role="dialog"[^>]*controls/.test(markup));
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
  assert.ok(!/role="dialog" aria-modal="true"[^>]*aria-labelledby="controls/.test(markup));
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
