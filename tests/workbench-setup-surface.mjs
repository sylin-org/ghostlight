// The window's setup view, actually executed.
//
// workbench-surface.mjs runs the page against a stub document that cannot hold a tree, which is
// enough to prove a broken panel costs only itself and not enough to see a row appear, a disc fill
// or a timer fire. This module runs the real modules against a small fake document built from the
// real markup, with a clock and a list of timers the test moves by hand, and asserts what a person
// would see: which view is on screen, the words in it, what a click sends, and what happens when
// the browser connects, the person skips, or setup finishes.
//
// It exports `setupChecks()`, which answers the same `[name, passed, detail]` rows the surface
// test prints. Nothing here starts Ghostlight, a browser or a server.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import vm from "node:vm";
import { join, resolve } from "node:path";
import { setupStages } from "./workbench-setup-fixtures.mjs";

const require = createRequire(import.meta.url);
const repository = resolve(import.meta.dirname, "..");
const ui = join(repository, "crates", "orchestrator", "ui");
const markup = readFileSync(join(ui, "index.html"), "utf8");
const ORDER = [...markup.matchAll(/<script src="([^"]+)"><\/script>/g)].map(([, src]) => src);
const SetupView = require(join(ui, "lib", "setup-view.js"));
const Words = require(join(ui, "lib", "words.js"));

const VOID = new Set(["meta", "link", "img", "input", "br", "hr", "source"]);

/** A fake element: enough tree, classes, attributes and events for the page to run on. */
class Node {
  constructor(tag, attributes = {}) {
    this.tag = tag;
    this.id = attributes.id ?? "";
    this.hidden = "hidden" in attributes;
    this.disabled = false;
    this.title = "";
    this.type = "";
    this.value = "";
    this.scrollTop = 0;
    this.offsetWidth = 0;
    this.parent = null;
    this.children = [];
    this.text = "";
    this.markup = "";
    this.attributes = {};
    this.dataset = {};
    this.handlers = new Map();
    this.style = { setProperty() {} };
    this.classes = new Set(String(attributes.class ?? "").split(/\s+/).filter(Boolean));
    for (const [name, value] of Object.entries(attributes)) {
      if (!["id", "class", "hidden"].includes(name)) this.setAttribute(name, value);
    }
    this.classList = {
      add: (...names) => names.forEach((name) => this.classes.add(name)),
      remove: (...names) => names.forEach((name) => this.classes.delete(name)),
      toggle: (name, on) => {
        if (on === undefined ? !this.classes.has(name) : on) this.classes.add(name);
        else this.classes.delete(name);
      },
      contains: (name) => this.classes.has(name)
    };
  }

  get className() {
    return [...this.classes].join(" ");
  }

  set className(value) {
    this.classes = new Set(String(value).split(/\s+/).filter(Boolean));
  }

  get textContent() {
    return this.text + this.children.map((child) => child.textContent).join("");
  }

  set textContent(value) {
    this.detach();
    this.text = String(value);
  }

  get innerHTML() {
    return this.markup;
  }

  set innerHTML(value) {
    this.detach();
    this.text = "";
    this.markup = String(value);
  }

  /** A template's content is the one icon its markup holds, as a node that remembers the markup. */
  get content() {
    const icon = new Node("svg");
    icon.markup = this.markup;
    return { firstElementChild: icon };
  }

  detach() {
    for (const child of this.children) child.parent = null;
    this.children = [];
  }

  adopt(items) {
    return items.map((item) => {
      const child = typeof item === "string" ? Object.assign(new Node("#text"), { text: item }) : item;
      child.parent = this;
      return child;
    });
  }

  append(...items) {
    this.children.push(...this.adopt(items));
  }

  replaceChildren(...items) {
    this.detach();
    this.text = "";
    this.append(...items);
  }

  setAttribute(name, value) {
    this.attributes[name] = String(value);
    if (name.startsWith("data-")) {
      this.dataset[name.slice(5).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = String(value);
    }
  }

  getAttribute(name) {
    return name in this.attributes ? this.attributes[name] : null;
  }

  removeAttribute(name) {
    delete this.attributes[name];
  }

  focus() {}

  addEventListener(type, handler) {
    if (!this.handlers.has(type)) this.handlers.set(type, []);
    this.handlers.get(type).push(handler);
  }

  removeEventListener() {}

  getBoundingClientRect() {
    return { left: 0, top: 0, width: 1, height: 1 };
  }

  matches(selector) {
    if (/[\s>+~,]/.test(selector)) throw new Error(`the fake document does not know the selector ${selector}`);
    const head = /^[a-z0-9]+/i.exec(selector);
    if (head && head[0] !== this.tag) return false;
    for (const [, kind, name, attribute, wanted] of selector.matchAll(/([#.])([\w-]+)|\[([\w-]+)(?:="([^"]*)")?\]/g)) {
      if (kind === "#" && this.id !== name) return false;
      if (kind === "." && !this.classes.has(name)) return false;
      if (attribute) {
        const have = attribute === "id" ? (this.id === "" ? null : this.id) : this.getAttribute(attribute);
        if (have === null || (wanted !== undefined && have !== wanted)) return false;
      }
    }
    return true;
  }

  closest(selector) {
    for (let node = this; node; node = node.parent) if (node.matches(selector)) return node;
    return null;
  }

  * descendants() {
    for (const child of this.children) {
      yield child;
      yield* child.descendants();
    }
  }

  querySelectorAll(selector) {
    return [...this.descendants()].filter((node) => node.matches(selector));
  }

  querySelector(selector) {
    return this.querySelectorAll(selector)[0] ?? null;
  }

  /** Press this element: its handlers run, then each ancestor's, then the document's. */
  click(document) {
    const event = { target: this, preventDefault() {} };
    for (let node = this; node; node = node.parent) for (const handler of node.handlers.get("click") ?? []) handler(event);
    for (const handler of document.handlers.get("click") ?? []) handler(event);
  }
}

/** The page's markup as a tree of fake elements. */
function parsePage() {
  const root = new Node("root");
  const stack = [root];
  const body = markup.slice(markup.indexOf("<body"));
  for (const [, closing, tag, rest] of body.matchAll(/<(\/?)([a-z0-9]+)((?:[^>"]|"[^"]*")*)>/g)) {
    if (closing) {
      stack.pop();
      continue;
    }
    const attributes = {};
    for (const [, name, value] of rest.matchAll(/([a-z-]+)(?:="([^"]*)")?/g)) attributes[name] = value ?? "";
    const element = new Node(tag, attributes);
    element.parent = stack.at(-1);
    stack.at(-1).children.push(element);
    if (!VOID.has(tag) && !rest.trimEnd().endsWith("/")) stack.push(element);
  }
  return root;
}

/** A document over the page's markup. */
function fakeDocument({ focused = true } = {}) {
  const root = parsePage();
  const body = [...root.descendants()].find((node) => node.tag === "body");
  const handlers = new Map();
  const document = {
    state: { focused },
    handlers,
    root,
    body,
    hidden: false,
    getElementById: (id) => [...root.descendants()].find((node) => node.id === id) ?? null,
    querySelectorAll: (selector) => root.querySelectorAll(selector),
    querySelector: (selector) => root.querySelector(selector),
    createElement: (tag) => new Node(tag),
    addEventListener: (type, handler) => {
      if (!handlers.has(type)) handlers.set(type, []);
      handlers.get(type).push(handler);
    },
    removeEventListener() {},
    hasFocus: () => document.state.focused
  };
  return document;
}

/** A clock and a list of timers that only move when the test moves them. */
function fakeClock() {
  const clock = { now: 1_000_000, timers: [], next: 1 };
  clock.schedule = (work, ms) => {
    const timer = { id: clock.next, work, ms, due: clock.now + ms };
    clock.next += 1;
    clock.timers.push(timer);
    return timer.id;
  };
  clock.cancel = (id) => {
    clock.timers = clock.timers.filter((timer) => timer.id !== id);
  };
  /** Move time forward, running each timer that comes due, in order. */
  clock.advance = async (ms) => {
    const until = clock.now + ms;
    for (;;) {
      const due = clock.timers.filter((timer) => timer.due <= until).sort((a, b) => a.due - b.due || a.id - b.id)[0];
      if (!due) break;
      clock.timers = clock.timers.filter((timer) => timer !== due);
      clock.now = Math.max(clock.now, due.due);
      due.work();
      await flush();
    }
    clock.now = until;
    await flush();
  };
  /** The delays of the timers waiting, in the order they will fire. */
  clock.waiting = () => clock.timers.slice().sort((a, b) => a.due - b.due || a.id - b.id).map((timer) => timer.ms);
  return clock;
}

/** Text for a value, with an element shown as its tag so a row of the page can be printed. */
const show = (value) => JSON.stringify(value, (_key, each) => (each instanceof Node ? `<${each.tag}>` : each));

const flush = async () => {
  for (let turn = 0; turn < 6; turn += 1) await new Promise((resolve) => setImmediate(resolve));
};

/* ------------------------------ the view on its own ------------------------------ */

/** The setup view over the real markup, with callbacks that record what they were asked. */
function viewRig({ reduced = false, focused = true, copyFails = false } = {}) {
  const document = fakeDocument({ focused });
  const clock = fakeClock();
  const calls = { actions: [], skips: 0, forwards: 0, finished: 0, copies: [], failures: [] };
  const screen = SetupView.create({
    document,
    words: Words.SETUP,
    schedule: clock.schedule,
    cancel: clock.cancel,
    now: () => clock.now,
    hasFocus: () => document.state.focused,
    reducedMotion: () => reduced,
    onAction: (action, row) => calls.actions.push([action, row.id]),
    onSkip: () => { calls.skips += 1; },
    copyText: async (text) => {
      if (copyFails) throw new Error("no clipboard");
      calls.copies.push(text);
    },
    bringForward: () => { calls.forwards += 1; },
    onFinished: () => { calls.finished += 1; },
    onFailure: (what, error) => calls.failures.push(`${what}: ${error.message}`)
  });
  const el = (id) => document.getElementById(id);
  const page = {
    el,
    document,
    text: (id) => el(id).textContent,
    rows: () => el("setup-rows").children.map((row) => ({
      id: row.getAttribute("data-row"),
      classes: row.className,
      name: row.querySelector(".gl-row__name").textContent,
      detail: row.querySelector(".gl-row__detail").textContent,
      button: row.querySelector("button")
    })),
    discs: () => el("setup-discs").querySelectorAll(".gl-stepper__disc").map((disc) => disc.className),
    segments: () => el("setup-discs").querySelectorAll(".gl-stepper__seg").map((seg) => seg.classes.has("is-lit")),
    companion: el("setup-companion"),
    live: () => ({ text: el("setup-live").textContent, done: el("setup-live").classes.has("is-ok") }),
    headline: () => `${page.text("setup-h1-lead")}${page.text("setup-h1-accent")}${page.text("setup-h1-trail")}`
  };
  return { screen, clock, calls, page, document };
}

const stage = (name) => structuredClone(setupStages[name]);

/** Whether any element of the setup root carries one of the classes that play an animation. */
function animates(page) {
  const motion = ["is-pop", "is-celebrating", "is-flip", "gl-swap", "is-new"];
  const nodes = [...page.document.getElementById("setup-root").descendants(), page.document.getElementById("setup-root")];
  return nodes.some((node) => motion.some((name) => node.classes.has(name)))
    || nodes.some((node) => node.markup.includes("gl-draw"));
}

async function viewChecks() {
  const checks = [];
  const add = (name, pass, detail = "") => checks.push([name, Boolean(pass), detail]);

  // ---- which view, and what changing views plays ----
  const { visible, transition, STAGE, PLAY, MEET_AFTER_MS } = SetupView;
  const part = (name) => ({ stage: name });
  add("setup shows for a stage with something to do, unless this window skipped it",
    visible(part("computer"), { skipped: false, shown: false })
      && visible(part("browser"), { skipped: false, shown: true })
      && visible(part("agent"), { skipped: false, shown: false })
      && !visible(part("browser"), { skipped: true, shown: false })
      && !visible(part("agent"), { skipped: true, shown: true }));
  add("a finished setup shows only to a window that was showing setup, and a retired one never shows",
    visible(part("done"), { skipped: false, shown: true })
      && !visible(part("done"), { skipped: false, shown: false })
      && !visible(part("done"), { skipped: true, shown: false })
      && !visible(part("retired"), { skipped: false, shown: true })
      && !visible(null, { skipped: false, shown: true }));
  add("a browser that connects after the browser step has been on screen for two seconds is a meeting",
    MEET_AFTER_MS === 2000
      && transition(part("browser"), part("agent"), 2000) === PLAY.meet
      && transition(part("browser"), part("agent"), 1999) === null
      && transition(part("computer"), part("agent"), 60_000) === null
      && transition(part("agent"), part("agent"), 60_000) === null);
  add("completion wins: setup finishing plays the completion alone, even beside a connection",
    transition(part("agent"), part("done"), 0) === PLAY.finish
      && transition(part("browser"), part("done"), 60_000) === PLAY.finish
      && transition(part("computer"), part("done"), 0) === PLAY.finish
      && transition(part("done"), part("retired"), 0) === null
      && transition(null, part("done"), 0) === null);
  add("the stage names are the service's", Object.values(STAGE).join() === "computer,browser,agent,done,retired");

  // ---- the stages at rest, verbatim ----
  {
    const { screen, page } = viewRig();
    screen.open(stage("browser"));
    const rows = page.rows();
    add("the browser stage is drawn word for word: eyebrow, headline parts, lead, rows and live line",
      page.text("setup-eyebrow") === "Set up Ghostlight"
        && page.text("setup-h1-lead") === "Add Ghostlight to "
        && page.text("setup-h1-accent") === "your browser"
        && page.text("setup-h1-trail") === "."
        && page.text("setup-lead") === "One browser is enough."
        && rows.map((row) => `${row.name}|${row.detail}`).join(";")
          === "Google Chrome|Not added;Microsoft Edge|Added in your Work profile. Open Edge to finish."
        && page.live().text === "Waiting for a browser." && !page.live().done
        && !page.el("setup-root").hidden,
      show({ rows, live: page.live() }));
    add("a row wears its tone, its detail word and a button named for what it does",
      rows[0].classes === "gl-row is-todo"
        && rows[0].button.className === "gl-btn gl-btn--primary gl-btn--sm"
        && rows[0].button.getAttribute("aria-label") === "Add to Google Chrome"
        && rows[0].button.textContent === "Add"
        && rows[0].button.children.some((child) => child.markup.includes("M14 5h5v5"))
        && rows[1].button.className === "gl-btn gl-btn--ghost gl-btn--sm"
        && rows[1].button.getAttribute("aria-label") === "Open Microsoft Edge"
        && rows[1].button.getAttribute("data-kind") === "open_browser"
        && page.el("setup-rows").getAttribute("aria-live") === "polite",
      show(rows.map((row) => [row.classes, row.button.className])));
    add("the steps carry the service's names, words and screen-reader text, and the discs show the state",
      page.el("setup-steps").children.map((item) => item.querySelector(".gl-sr").textContent).join(";")
        === "This computer, done;Your browser, current step;Your agent"
        && page.el("setup-steps").children[0].querySelector(".gl-stepper__w").textContent === "Installed"
        && page.el("setup-steps").children[1].getAttribute("aria-current") === "step"
        && page.discs()[0] === "gl-stepper__disc gl-stepper__disc--0 is-done"
        && page.discs()[1] === "gl-stepper__disc gl-stepper__disc--1 is-current"
        && page.discs()[2] === "gl-stepper__disc gl-stepper__disc--2"
        && page.el("setup-discs").querySelectorAll(".gl-stepper__disc")[2].textContent === "03"
        && page.companion.getAttribute("data-light") === "waiting"
        && page.segments().join() === "false,false",
      show({ discs: page.discs(), segments: page.segments() }));
    add("the skip button says its words and shows while there is something to do",
      page.text("setup-skip") === "Skip for now" && !page.el("setup-skip").hidden);
  }
  {
    const { screen, page } = viewRig();
    screen.open(stage("agent"));
    const code = page.el("setup-code").children[0];
    add("the agent stage draws the prompt panel, the agents and the live line",
      page.headline() === "Ask your agent."
        && code.querySelector(".gl-code__label").textContent === "Paste into your agent"
        && code.querySelector(".gl-code__text").textContent === "Open sylin.org/ghostlight/demo/desk and ring the bell."
        && code.querySelector("button").getAttribute("aria-label") === "Copy prompt"
        && code.querySelector(".gl-copy__lbl").textContent === "Copy"
        && page.text("setup-rows-label") === "Agents on this computer"
        && page.rows().map((row) => row.classes).join(";") === "gl-row is-done;gl-row is-todo;gl-row is-attention;gl-row is-todo;gl-row is-todo"
        && page.live().text === "Waiting for your agent."
        && page.companion.getAttribute("data-light") === "lit"
        && page.segments().join() === "true,false",
      show({ rows: page.rows().map((row) => row.classes) }));
  }
  {
    const { screen, page } = viewRig();
    screen.open(stage("computer"));
    add("the computer stage asks for the command on the computer itself",
      page.headline() === "Finish installing on this computer."
        && page.text("setup-lead") === "Run this once from a regular terminal."
        && page.el("setup-code").children[0].querySelector(".gl-code__text").textContent
          === "~/.ghostlight/bin/ghostlight install"
        && page.rows().length === 0 && page.el("setup-agents").hidden);
  }

  // ---- copying ----
  {
    const { screen, clock, calls, page } = viewRig();
    screen.open(stage("agent"));
    const button = page.el("setup-code").children[0].querySelector("button");
    button.click(page.document);
    await flush();
    const label = button.querySelector(".gl-copy__lbl");
    const copiedAt = clock.waiting();
    add("Copy puts the panel's text on the clipboard, says Copied, announces it and takes it back after 1600 ms",
      calls.copies.join() === "Open sylin.org/ghostlight/demo/desk and ring the bell."
        && label.textContent === "Copied" && button.classes.has("is-copied")
        && copiedAt.includes(SetupView.COPIED_MS),
      show(copiedAt));
    await clock.advance(SetupView.ANNOUNCE_DELAY_MS);
    const said = page.text("setup-say");
    await clock.advance(SetupView.COPIED_MS);
    add("Copied is announced, and Copy comes back",
      said === "Copied" && label.textContent === "Copy" && !button.classes.has("is-copied"),
      show({ said, label: label.textContent }));
  }
  {
    const { screen, calls, page } = viewRig({ copyFails: true });
    screen.open(stage("agent"));
    page.el("setup-code").children[0].querySelector("button").click(page.document);
    await flush();
    add("a clipboard that refuses is reported and the label stays Copy",
      calls.failures.join() === "copying: no clipboard"
        && page.el("setup-code").children[0].querySelector(".gl-copy__lbl").textContent === "Copy");
  }

  // ---- a click: pending words until the row changes ----
  {
    const { screen, calls, page } = viewRig();
    screen.open(stage("browser"));
    page.rows()[0].button.click(page.document);
    const clicked = page.rows()[0];
    add("a click on Add asks for the step and the row says what is happening in place of its button",
      show(calls.actions) === show([[{ kind: "add_in_browser", browser: "chrome" }, "chrome"]])
        && clicked.classes === "gl-row is-working" && clicked.detail === "Finish in the Chrome Web Store."
        && clicked.button === null && page.live().text === "Waiting for Chrome." && !page.live().done,
      show({ clicked: [clicked.classes, clicked.detail], live: page.live() }));
    screen.update(stage("browser"));
    add("an update that leaves the row as it was keeps the pending words",
      page.rows()[0].detail === "Finish in the Chrome Web Store." && page.live().text === "Waiting for Chrome.");
    const changed = stage("browser");
    changed.rows[0].detail = "Added. Open Chrome to finish.";
    screen.update(changed);
    add("an update that changes the row replaces the pending words with the service's",
      page.rows()[0].detail === "Added. Open Chrome to finish." && page.live().text === "Waiting for a browser.",
      show({ row: page.rows()[0].detail, live: page.live() }));
  }
  {
    const { screen, page } = viewRig();
    screen.open(stage("agent"));
    page.rows()[1].button.click(page.document);
    const waiting = page.rows()[1];
    add("a Connect click shows Connecting and leaves the live line alone",
      waiting.detail === "Connecting" && waiting.classes === "gl-row is-working"
        && page.live().text === "Waiting for your agent.");
    screen.clearPending("claude-desktop");
    add("a step that failed puts its row back",
      page.rows()[1].detail === "Found" && page.rows()[1].button !== null);
  }
  {
    const { screen, calls, page } = viewRig();
    screen.open(stage("agent"));
    page.rows()[2].button.click(page.document);
    add("a Review click asks for the step and shows no pending words",
      calls.actions[0][0].kind === "review_agent" && page.rows()[2].detail === "Needs attention"
        && page.rows()[2].button !== null);
    page.el("setup-skip").click(page.document);
    add("Skip for now asks the page to leave", calls.skips === 1);
  }
  {
    const { screen, calls, page } = viewRig();
    screen.open(stage("agent"));
    const update = page.rows().find((row) => row.id === "zed");
    add("an agent with an older entry reads Needs an update and offers Update",
      update.detail === "Needs an update" && update.button.textContent === "Update"
        && update.button.getAttribute("aria-label") === "Update Zed");
    update.button.click(page.document);
    const updating = page.rows().find((row) => row.id === "zed");
    add("an Update click asks to write the entry and shows Updating",
      calls.actions.at(-1)[0].kind === "connect_agent" && updating.detail === "Updating"
        && updating.classes === "gl-row is-working");
    const goose = page.rows().find((row) => row.id === "goose");
    add("a copy-only agent offers Copy setup and never Connect",
      goose.detail === "Found, set up by hand" && goose.button.textContent === "Copy setup");
    goose.button.click(page.document);
    add("a Copy setup click asks for the copy and shows no pending words",
      show(calls.actions.at(-1)[0]) === show({ kind: "copy_setup", agent: "goose" })
        && page.rows().find((row) => row.id === "goose").detail === "Found, set up by hand");
  }

  // ---- the meeting ----
  {
    const { screen, clock, calls, page } = viewRig();
    screen.open(stage("browser"));
    await clock.advance(2500);
    screen.update(stage("agent"));
    const rows = page.rows();
    add("the meeting: the browser row turns done and draws its check, and the live line says it connected",
      rows.length === 2 && rows[0].classes === "gl-row is-done is-flip"
        && rows[0].detail === "Connected in Chrome" && rows[0].button === null && rows[1].button === null
        && page.el("setup-rows").children[0].children[0].children[0].markup.includes("gl-draw")
        && page.live().text === "Chrome is connected." && page.live().done
        && page.headline() === "Add Ghostlight to your browser.",
      show({ rows: rows.map((row) => [row.classes, row.detail]), live: page.live() }));
    add("the meeting: disc 02 pops and fills, the segment lights, the companion is lit and celebrating",
      page.discs()[1] === "gl-stepper__disc gl-stepper__disc--1 is-done is-pop"
        && page.segments().join() === "true,false" && page.companion.classes.has("is-celebrating")
        && page.companion.getAttribute("data-light") === "lit",
      show(page.discs()));
    add("the meeting: the agent step arrives after 1900 ms and not before",
      clock.waiting().includes(SetupView.MEET_SWAP_MS) && SetupView.MEET_SWAP_MS === 1900);
    await clock.advance(1899);
    const before = page.headline();
    await clock.advance(1);
    add("the agent step replaces the browser step at 1900 ms, drawn at rest with its entrance",
      before === "Add Ghostlight to your browser." && page.headline() === "Ask your agent."
        && page.el("setup-main").classes.has("gl-swap") && page.el("setup-after").classes.has("gl-swap")
        && page.rows().map((row) => row.name).join() === "Claude Code,Claude Desktop,Cursor,Zed,goose"
        && page.live().text === "Waiting for your agent." && !page.live().done
        && page.discs()[1] === "gl-stepper__disc gl-stepper__disc--1 is-done",
      show({ before, now: page.headline() }));
    add("a meeting in a window the person is looking at does not ask to come forward", calls.forwards === 0);
  }
  {
    const { screen, clock, calls } = viewRig({ focused: false });
    screen.open(stage("browser"));
    await clock.advance(3000);
    screen.update(stage("agent"));
    add("a meeting in a window that does not have the person's attention asks to come forward, once",
      calls.forwards === 1);
  }
  {
    const { screen, clock, calls, page } = viewRig();
    screen.open(stage("browser"));
    await clock.advance(1999);
    screen.update(stage("agent"));
    add("a connection before the browser step has been on screen for two seconds draws the agent step at rest",
      page.headline() === "Ask your agent." && !page.companion.classes.has("is-celebrating")
        && !animates(page) && clock.waiting().every((ms) => ms !== SetupView.MEET_SWAP_MS)
        && calls.forwards === 0 && page.live().text === "Waiting for your agent.");
  }
  {
    const { screen, clock, page } = viewRig();
    screen.open(stage("agent"));
    await clock.advance(10_000);
    screen.update(stage("agent"));
    add("a window opened on the agent step (the extension came first) never sees the meeting",
      !page.companion.classes.has("is-celebrating") && !animates(page));
  }
  {
    const { screen, clock, calls, page } = viewRig();
    screen.open(stage("browser"));
    await clock.advance(3000);
    screen.update(stage("agent"));
    await clock.advance(500);
    screen.update(stage("done"));
    add("completion arriving during the meeting cancels the swap and plays the completion",
      page.headline() === "Claude Code just used your browser. You're set." && page.text("setup-eyebrow") === "Ready"
        && !clock.waiting().includes(SetupView.MEET_SWAP_MS) && calls.finished === 0);
  }

  // ---- completion ----
  {
    const { screen, clock, calls, page } = viewRig();
    screen.open(stage("agent"));
    await clock.advance(3000);
    screen.update(stage("done"));
    add("completion: the eyebrow says Ready, the heading swaps, disc 03 pops and fills, no second celebration",
      page.text("setup-eyebrow") === "Ready"
        && page.headline() === "Claude Code just used your browser. You're set."
        && page.text("setup-h1-accent") === "You're set."
        && page.discs()[2] === "gl-stepper__disc gl-stepper__disc--2 is-done is-pop"
        && page.segments().join() === "true,true" && !page.companion.classes.has("is-celebrating")
        && page.el("setup-main").classes.has("gl-swap"),
      show({ discs: page.discs(), segments: page.segments() }));
    add("completion: the code, rows and live line go, the tagline arrives, the region says the heading",
      page.el("setup-code").children.length === 0 && page.rows().length === 0 && page.live().text === ""
        && page.el("setup-agents").hidden && page.el("setup-skip").hidden
        && page.text("setup-tagline") === "A light left burning, so the halls stay safe."
        && !page.el("setup-tagline").hidden && page.el("setup-tagline").classes.has("is-new"));
    await clock.advance(SetupView.ANNOUNCE_DELAY_MS);
    add("completion: the screen-reader region says the new heading",
      page.text("setup-say") === "Claude Code just used your browser. You're set.", page.text("setup-say"));
    await clock.advance(5000);
    const early = calls.finished;
    await clock.advance(1000);
    add("completion: after 6000 ms the page is told to move on, once",
      early === 0 && calls.finished === 1 && SetupView.FINISHED_HOLD_MS === 6000);
    screen.update(stage("done"));
    add("an update after the completion changes nothing", page.text("setup-eyebrow") === "Ready");
  }

  // ---- reduced motion: no animation, the same content at the same moments ----
  {
    const { screen, clock, calls, page } = viewRig({ reduced: true });
    screen.open(stage("browser"));
    await clock.advance(2500);
    screen.update(stage("agent"));
    const rows = page.rows();
    add("reduced motion: the meeting changes the same content with no animation classes",
      rows[0].classes === "gl-row is-done" && rows[0].detail === "Connected in Chrome"
        && page.live().text === "Chrome is connected." && page.live().done
        && page.discs()[1] === "gl-stepper__disc gl-stepper__disc--1 is-done"
        && page.companion.getAttribute("data-light") === "lit" && !animates(page),
      show({ rows: rows.map((row) => row.classes), discs: page.discs() }));
    await clock.advance(1900);
    add("reduced motion: the agent step still arrives at 1900 ms, without its entrance",
      page.headline() === "Ask your agent." && !animates(page));
    screen.update(stage("done"));
    add("reduced motion: the completion changes the heading and fills disc 03 with no animation",
      page.headline() === "Claude Code just used your browser. You're set."
        && page.discs()[2] === "gl-stepper__disc gl-stepper__disc--2 is-done" && !animates(page));
    add("reduced motion: the tagline arrives at 1050 ms",
      page.el("setup-tagline").hidden && clock.waiting().includes(SetupView.TAGLINE_MS));
    await clock.advance(SetupView.TAGLINE_MS);
    add("reduced motion: the tagline is there once 1050 ms have passed, still with no animation",
      !page.el("setup-tagline").hidden && page.text("setup-tagline") === "A light left burning, so the halls stay safe."
        && !animates(page));
    await clock.advance(6000);
    add("reduced motion: the page is told to move on after the same 6000 ms", calls.finished === 1);
  }

  // ---- closing and reopening ----
  {
    const { screen, clock, page } = viewRig();
    screen.open(stage("browser"));
    await clock.advance(3000);
    screen.update(stage("agent"));
    screen.close();
    add("closing stops the meeting and hides the view", page.el("setup-root").hidden && clock.waiting().length === 0
      && !screen.isOpen());
    screen.open(stage("browser"));
    await clock.advance(1000);
    screen.update(stage("agent"));
    add("a reopened view starts its two seconds again",
      page.headline() === "Ask your agent." && !page.companion.classes.has("is-celebrating"));
  }
  return checks;
}

/* ------------------------------ the page around it ------------------------------ */

/** A snapshot, shaped as the service's parts, with the clients the setup rows act on. */
function snapshotParts() {
  return {
    status: {
      service: { version: "2.0.0", started_at_ms: 0, runtime_state: "active" },
      readiness: { state: "ready", word: "Ready", detail: "Connected and idle.", tone: "quiet", invites_control: true },
      audit_health: {}, audit_notice: "",
      configuration: { managed_policy: { configured: false }, policy: { situation: "open", detail: "Open.", tone: "open" } }
    },
    connections: { sessions: [], browsers: [] },
    activity: { operations: [], history: [], document_coverage: [] },
    integrations: {
      harnesses: [{
        id: "claude-desktop", product_id: "claude-desktop", name: "Claude Desktop", target: "User",
        agent: { id: "claude-desktop", name: "Claude Desktop", symbol: { icon: "agent-icons/claude-desktop?v=fedcba987654" } },
        state: "available", detail: "Detected.", can_install: true, can_uninstall: false, can_download: true, can_locate: true,
        config_path: "/home/test/claude.json", connector_command: "ghostlight mcp", manual_setup: "{}"
      }, {
        id: "zed", product_id: "zed", name: "Zed", target: "User",
        agent: { id: "zed", name: "Zed", symbol: { icon: "agent-icons/zed?v=fedcba987654" } },
        state: "updatable", detail: "Needs an update.", can_install: true, can_uninstall: false, can_download: true, can_locate: true,
        config_path: "/home/test/.config/zed/settings.json", connector_command: "ghostlight mcp", manual_setup: "{}"
      }, {
        id: "goose", product_id: "goose", name: "goose", target: "CLI and desktop",
        agent: { id: "goose", name: "goose", symbol: { icon: "agent-icons/goose?v=fedcba987654" } },
        state: "available", detail: "Set up by hand.", can_install: false, can_uninstall: false, can_download: true, can_locate: true,
        copy_only: true, config_path: "/home/test/.config/goose/config.yaml", connector_command: "ghostlight mcp",
        manual_setup: "extensions: { ghostlight: { cmd: ghostlight } }"
      }]
    },
    diagnostics: { diagnostics: [] }
  };
}

const CONTROL_CONTEXT_ACTS = ["search", "check_policy", "rescan", "continue_in_window", "hold", "resume", "start_session",
  "end_session", "apply_policy", "remove_policy", "set_diagnostics", "set_watching", "setup_all", "harness_act",
  "test_notification", "open", "reveal_diagnostics", "locate", "setup_act", "quit"];

/** A stream the fake service holds open, which the test writes events into. */
function openStream() {
  const queued = [];
  const encoder = new TextEncoder();
  let waiting = null;
  return {
    write(text) {
      const value = encoder.encode(text);
      if (waiting) {
        const resolve = waiting;
        waiting = null;
        resolve({ done: false, value });
      } else {
        queued.push(value);
      }
    },
    reader: {
      read: () => (queued.length ? Promise.resolve({ done: false, value: queued.shift() })
        : new Promise((resolve) => { waiting = resolve; }))
    }
  };
}

/**
 * The real page, running against a fake service.
 *
 * `setup` is the setup part the service gives (null leaves the part out, as it does for a watcher),
 * `hash` the address the page loads at, `context` the kind of screen. The service's setup part is
 * `world.setup`; set it and call `announce()` to tell the page, as the service's stream would.
 */
async function bootPage({ setup, hash = "", context = "control", focused = true, reduced = false } = {}) {
  const document = fakeDocument({ focused });
  const clock = fakeClock();
  const world = { setup: setup ?? null, requests: [], seq: 10, stream: null, hidden: [], refusals: new Map(), copied: [] };
  const address = {
    hash,
    replaced: [],
    pushed: []
  };
  const hashHandlers = [];
  const history = {
    replaceState: (_state, _title, url) => {
      address.replaced.push(url);
      address.hash = url;
    },
    pushState: (_state, _title, url) => address.pushed.push(url)
  };
  const said = (status, body) => ({
    ok: status >= 200 && status < 300, status, headers: { get: () => null },
    text: async () => (body === undefined ? "" : show(body))
  });
  const sandboxClock = Object.assign(class extends Date {}, { now: () => clock.now });
  const sandbox = {
    console: { error: (detail) => world.hidden.push(String(detail)), log() {}, warn() {} },
    document,
    window: {
      addEventListener: (kind, handler) => { if (kind === "hashchange") hashHandlers.push(handler); },
      location: address,
      history,
      matchMedia: () => ({ matches: reduced }),
      fetch: async (path, init = {}) => {
        const url = new URL(path, "http://127.0.0.1:41000");
        const method = init.method ?? "GET";
        const body = init.body ? JSON.parse(init.body) : undefined;
        world.requests.push({ method, path: url.pathname + url.search, body });
        const route = `${method} ${url.pathname}`;
        const refusal = world.refusals.get(route);
        if (refusal) return said(refusal.status, { error: { code: "refused", message: refusal.message } });
        if (route === "GET /api/context") {
          return said(200, {
            context, word: "Full control", sentence: "Others can watch.", instance: "i1", opener: "service",
            acts: Object.fromEntries(CONTROL_CONTEXT_ACTS.map((act) => [act, { available: true, why_not: null }])),
            product: { version: "2.0.0", license: "Apache-2.0 OR MIT" }, links: {}
          });
        }
        if (route === "GET /api/snapshot") {
          // The setup part is read apart from the sequence of changes, as the service reads it.
          if (url.searchParams.get("parts") === "setup") {
            return said(200, { seq: world.seq, instance: "i1", parts: world.setup ? { setup: world.setup } : {} });
          }
          world.seq += 1;
          return said(200, { seq: world.seq, instance: "i1", parts: snapshotParts() });
        }
        if (route === "POST /api/setup/act") return said(200, world.setup);
        if (route === "POST /api/integrations/rescan") return said(200, []);
        if (route === "POST /api/integrations/act") return said(200, { changed: true, message: "Set up Claude Desktop." });
        if (route === "POST /api/window") return said(200, {});
        if (route === "GET /api/events") {
          world.stream = openStream();
          return { ok: true, status: 200, headers: { get: () => null }, body: { getReader: () => world.stream.reader } };
        }
        return said(200, {});
      }
    },
    navigator: { clipboard: { writeText: async (text) => { world.copied.push(text); } } },
    GhostlightAccess: Object.freeze({ bearer: "key-1" }),
    AbortController, TextDecoder, TextEncoder, MutationObserver: class { observe() {} },
    setTimeout: clock.schedule,
    clearTimeout: clock.cancel,
    setInterval: () => 0,
    clearInterval() {},
    Date: sandboxClock, JSON, Math, String, Number, Boolean, Array, Object, Set, Map, Promise, Error,
    isNaN, parseInt, parseFloat, performance: { now: () => 0 }
  };
  sandbox.globalThis = sandbox;
  vm.createContext(sandbox);
  const sources = ORDER.map((name) => readFileSync(join(ui, name), "utf8"));
  vm.runInContext(sources.join("\n"), sandbox);
  await flush();

  const el = (id) => document.getElementById(id);
  const page = {
    document, clock, world, address, sandbox,
    el,
    shellShown: () => !el("app-shell").hidden,
    setupShown: () => !el("setup-root").hidden,
    headline: () => `${el("setup-h1-lead").textContent}${el("setup-h1-accent").textContent}${el("setup-h1-trail").textContent}`,
    rows: () => el("setup-rows").children.map((row) => ({
      id: row.getAttribute("data-row"),
      classes: row.className,
      detail: row.querySelector(".gl-row__detail").textContent,
      button: row.querySelector("button")
    })),
    row: (id) => page.rows().find((row) => row.id === id),
    live: () => ({ text: el("setup-live").textContent, done: el("setup-live").classes.has("is-ok") }),
    discs: () => el("setup-discs").querySelectorAll(".gl-stepper__disc").map((disc) => disc.className),
    count: (method, path) => world.requests.filter((request) => request.method === method && request.path === path).length,
    posted: (path) => world.requests.filter((request) => request.method === "POST" && request.path === path),
    /** Tell the page, as the service's stream does, that the setup part changed. */
    async announce(next) {
      if (next !== undefined) world.setup = next;
      world.seq += 1;
      world.stream.write(`data: ${show({ seq: world.seq, change: { kind: "part_changed", part: "setup", etag: `t${world.seq}` } })}\n\n`);
      await flush();
    },
    /** Change the address of the page, as the shell does for a window it brings forward at one. */
    async navigateTo(hash) {
      address.hash = hash;
      for (const handler of hashHandlers) handler();
      await flush();
    },
    async click(node) {
      node.click(document);
      await flush();
    }
  };
  // The stream is opened a turn after the context is read; wait for it.
  for (let turn = 0; turn < 20 && !world.stream; turn += 1) await flush();
  return page;
}

async function pageChecks() {
  const checks = [];
  const add = (name, pass, detail = "") => checks.push([name, Boolean(pass), detail]);
  const stages = (name) => structuredClone(setupStages[name]);
  const retired = { ...stages("agent"), stage: "retired", eyebrow: "", code: null, rows: [], live: null };

  // ---- the browser stage opens the view in place of the tabs ----
  {
    const page = await bootPage({ setup: stages("browser") });
    add("the browser stage shows the setup view and hides the shell",
      page.setupShown() && !page.shellShown() && page.headline() === "Add Ghostlight to your browser."
        && page.rows().length === 2 && page.live().text === "Waiting for a browser.",
      show({ requests: page.world.requests.map((request) => request.path) }));
    add("the setup part is read on its own, and opening looks for the agents once",
      page.count("GET", "/api/snapshot?parts=setup") === 1 && page.count("POST", "/api/integrations/rescan") === 1,
      show(page.world.requests.map((request) => `${request.method} ${request.path}`)));
    add("the address says where the window is", page.address.hash === "#setup", page.address.hash);

    // Add: the step is posted and the row says what is happening.
    await page.click(page.row("chrome").button);
    add("an Add click posts the step and shows the pending words",
      show(page.posted("/api/setup/act").map((request) => request.body))
        === show([{ kind: "add_in_browser", browser: "chrome" }])
        && page.row("chrome").detail === "Finish in the Chrome Web Store."
        && page.live().text === "Waiting for Chrome.",
      show(page.row("chrome")));
    add("the words that came back with the step do not end the wait while the row is unchanged",
      page.row("chrome").detail === "Finish in the Chrome Web Store.");

    // The browser connects after two seconds on the step: the meeting.
    await page.clock.advance(2500);
    const agent = stages("agent");
    await page.announce(agent);
    add("a part update that shows a connected browser plays the meeting after two seconds on the step",
      page.live().text === "Chrome is connected." && page.live().done
        && page.row("chrome").classes === "gl-row is-done is-flip"
        && page.discs()[1].endsWith("is-done is-pop")
        && page.el("setup-companion").classes.has("is-celebrating")
        && page.headline() === "Add Ghostlight to your browser.",
      show({ live: page.live(), row: page.row("chrome") }));
    await page.clock.advance(1900);
    add("1900 ms later the agent step takes over with the agent prompt",
      page.headline() === "Ask your agent."
        && page.el("setup-code").children[0].querySelector(".gl-code__text").textContent
          === "Open sylin.org/ghostlight/demo/desk and ring the bell."
        && page.count("POST", "/api/window") === 0);

    // Connect an agent: the integrations act, the pending word, then the service's own.
    await page.click(page.row("claude-desktop").button);
    add("a Connect click posts the integrations act with the state the agent was drawn in",
      show(page.posted("/api/integrations/act").map((request) => request.body))
        === show([{ id: "claude-desktop", action: "install", expected_state: "available" }])
        && page.row("claude-desktop").detail === "Connecting",
      show({ posted: page.posted("/api/integrations/act"), row: page.row("claude-desktop") }));
    const restart = stages("agent");
    Object.assign(restart.rows[1], { detail: "Restart Claude Desktop to finish.", tone: "attention", action: null });
    await page.announce(restart);
    add("the next update replaces Connecting with the service's words",
      page.row("claude-desktop").detail === "Restart Claude Desktop to finish."
        && page.row("claude-desktop").classes === "gl-row is-attention");

    // Completion, observed while open.
    await page.announce(stages("done"));
    add("a finished setup observed while open shows Ready and the new heading",
      page.el("setup-eyebrow").textContent === "Ready" && page.headline() === "Claude Code just used your browser. You're set."
        && page.setupShown());
    await page.clock.advance(5999);
    const stillThere = page.setupShown() && page.address.hash === "#setup";
    await page.clock.advance(1);
    add("after 6000 ms the window goes to the monitor and gives the tabs back",
      stillThere && !page.setupShown() && page.shellShown() && page.address.hash === "#monitor",
      show({ hash: page.address.hash, shell: page.shellShown() }));
    await page.announce(retired);
    add("nothing brings setup back once it is finished", !page.setupShown());
  }

  // ---- a connection before two seconds ----
  {
    const page = await bootPage({ setup: stages("browser") });
    await page.clock.advance(1500);
    await page.announce(stages("agent"));
    add("a connection before two seconds on the browser step draws the agent step at rest",
      page.headline() === "Ask your agent." && !page.el("setup-companion").classes.has("is-celebrating")
        && page.live().text === "Waiting for your agent." && page.row("claude-code") !== undefined);
  }

  // ---- a window the person is not looking at ----
  {
    const page = await bootPage({ setup: stages("browser"), focused: false });
    await page.clock.advance(2500);
    await page.announce(stages("agent"));
    add("a meeting in a window without focus posts the window route once, at setup",
      show(page.posted("/api/window").map((request) => request.body))
        === show([{ view: "workbench", address: "setup" }]),
      show(page.posted("/api/window")));
  }

  // ---- skipping ----
  {
    const page = await bootPage({ setup: stages("agent") });
    await page.click(page.el("setup-skip"));
    add("Skip for now goes to the monitor and gives the tabs back",
      !page.setupShown() && page.shellShown() && page.address.hash === "#monitor");
    await page.announce(Object.assign(stages("agent"), { live: { text: "Still waiting.", done: false } }));
    add("a later update does not bring setup back to a window that skipped", !page.setupShown() && page.shellShown());
    await page.navigateTo("#setup");
    add("a reveal at #setup shows setup again and undoes the skip",
      page.setupShown() && !page.shellShown() && page.live().text === "Still waiting.");
    await page.announce(stages("done"));
    await page.clock.advance(6000);
    add("a finished setup takes a window that was showing it to the monitor", !page.setupShown() && page.address.hash === "#monitor");
  }
  {
    const page = await bootPage({ setup: stages("agent") });
    await page.click(page.row("zed").button);
    add("an Update click posts the integrations act with the state the agent was drawn in",
      show(page.posted("/api/integrations/act").map((request) => request.body))
        === show([{ id: "zed", action: "install", expected_state: "updatable" }])
        && page.row("zed").detail === "Updating",
      show({ posted: page.posted("/api/integrations/act"), row: page.row("zed") }));
    await page.click(page.row("goose").button);
    add("a Copy setup click puts the agent's own setup on the clipboard and says so, and stays in setup",
      show(page.world.copied) === show(["extensions: { ghostlight: { cmd: ghostlight } }"])
        && page.el("toast").textContent === "Manual setup copied." && page.setupShown()
        && page.posted("/api/setup/act").length === 0,
      show({ copied: page.world.copied, toast: page.el("toast").textContent }));
  }
  {
    const page = await bootPage({ setup: stages("agent") });
    await page.click(page.row("cursor").button);
    add("Review leaves setup for the integrations tab, and counts as a skip",
      !page.setupShown() && page.shellShown() && page.address.hash === "#integrations");
    await page.announce(stages("agent"));
    add("a window that went to Review is not pulled back by the next update", !page.setupShown());
  }
  {
    const page = await bootPage({ setup: stages("agent") });
    await page.click(page.el("setup-skip"));
    await page.announce(stages("done"));
    add("a window that skipped is not interrupted by the completion", !page.setupShown() && page.address.hash === "#monitor");
  }

  // ---- addresses at load ----
  {
    const page = await bootPage({ setup: stages("browser"), hash: "#setup" });
    add("a window that opens at #setup shows it once the part is read",
      page.setupShown() && !page.shellShown() && page.address.hash === "#setup");
  }
  {
    const page = await bootPage({ setup: retired, hash: "#setup" });
    add("a window that opens at #setup with nothing to do opens at the monitor",
      !page.setupShown() && page.shellShown() && page.address.hash === "#monitor");
  }
  {
    const page = await bootPage({ setup: stages("browser"), hash: "#settings/diagnostics" });
    add("a window that opens at another place is not shown setup",
      !page.setupShown() && page.shellShown() && page.address.hash === "#settings/diagnostics");
  }

  // ---- who is shown it ----
  {
    const page = await bootPage({ setup: retired });
    add("a retired setup never shows the view, and opening it does not look for the agents",
      !page.setupShown() && page.shellShown() && page.count("POST", "/api/integrations/rescan") === 0);
  }
  {
    const page = await bootPage({ setup: stages("done") });
    add("a window that opens after setup finished in this run shows the workbench",
      !page.setupShown() && page.shellShown());
  }
  {
    const page = await bootPage({ setup: null, context: "watch" });
    add("a screen the service gives no setup part shows the ordinary workbench",
      !page.setupShown() && page.shellShown());
  }

  // ---- failures ----
  {
    const page = await bootPage({ setup: stages("browser") });
    page.world.refusals.set("POST /api/setup/act", { status: 422, message: "Chrome is no longer offered." });
    await page.click(page.row("chrome").button);
    add("a step the service refuses puts the row back and says why",
      page.row("chrome").detail === "Not added" && page.row("chrome").button !== null
        && page.el("toast").textContent === "Chrome is no longer offered.",
      show({ row: page.row("chrome"), toast: page.el("toast").textContent }));
  }
  return checks;
}

/** Every check of the setup view, in order. */
export async function setupChecks() {
  const rows = [...await viewChecks(), ...await pageChecks()];
  return rows.map(([name, pass, detail]) => [`setup view: ${name}`, pass, detail]);
}
