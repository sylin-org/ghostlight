"use strict";

// The popup and the options page scripts, run over a fake document: what they draw from the
// worker's answers and the messages they send back.

const test = require("node:test");
const assert = require("node:assert/strict");
const shared = require("../lib/shared.js");
const { runPage, settle } = require("./dom-fakes.js");

const SHOW = "show_ghostlight";

function snapshot(patch = {}) {
  return {
    connected: true,
    compatible: true,
    control_state: "active",
    link_state: shared.LINK_CONNECTED,
    attached_tabs: 0,
    recording_tabs: 0,
    activity: [],
    ...patch
  };
}

const PREFERENCES = Object.freeze({ effects: true, captions: true, diagnostics: false, preserveTabs: true });

test("the popup draws each connected state with its own lantern", async () => {
  const states = {
    idle: {},
    browsing: { activity: [{ client_label: "Claude Code", label: "Reading" }] },
    held: { control_state: "held" },
    ended: { control_state: "ended" }
  };
  const lights = {};
  for (const [name, patch] of Object.entries(states)) {
    const page = runPage("popup.js", { ui_snapshot: snapshot(patch) });
    await settle();
    lights[name] = page.byId("pop").dataset.light;
  }
  assert.deepEqual(lights, { idle: "waiting", browsing: "lit", held: "held", ended: "held" });
});

test("the popup's ended state offers Start a new session as its primary step", async () => {
  const page = runPage("popup.js", { ui_snapshot: snapshot({ control_state: "ended" }) });
  await settle();
  const [start, ...rest] = page.byId("actions").children;
  assert.equal(start.textContent, "Start a new session");
  assert.equal(start.className, "gl-btn gl-btn--primary");
  assert.deepEqual(rest, []);
});

test("the popup's Open Ghostlight link sends show_ghostlight", async () => {
  const page = runPage("popup.js", { ui_snapshot: snapshot(), [SHOW]: { queued: true } });
  await settle();
  const show = page.byId("show");
  assert.equal(show.hidden, false);
  assert.equal(show.textContent, "Open Ghostlight");
  show.click();
  await settle();
  assert.deepEqual(page.sent.filter((message) => message.kind === SHOW), [{ kind: SHOW }]);
});

test("the popup offers no window link while Ghostlight is not connected", async () => {
  const page = runPage("popup.js", {
    ui_snapshot: snapshot({ connected: false, link_state: shared.LINK_UNREACHABLE })
  });
  await settle();
  assert.equal(page.byId("show").hidden, true);
  page.byId("show").click();
  await settle();
  assert.equal(page.sent.some((message) => message.kind === SHOW), false);
});

test("the options page keeps captions available with effects off", async () => {
  const saved = [];
  const page = runPage("options.js", {
    get_preferences: { ...PREFERENCES, effects: false },
    ui_snapshot: snapshot(),
    set_preferences: (message) => { saved.push(message.preferences); return null; }
  });
  await settle();
  const effects = page.byId("effects");
  const captions = page.byId("captions");
  assert.equal(effects.checked, false);
  assert.equal(captions.checked, true);
  assert.equal(captions.disabled, false);

  effects.checked = true;
  effects.dispatch("change");
  await settle();
  effects.checked = false;
  effects.dispatch("change");
  await settle();
  assert.equal(captions.disabled, false);
  assert.deepEqual(saved.at(-1), { ...PREFERENCES, effects: false });
});

test("the options page's Open Ghostlight link sends show_ghostlight while connected", async () => {
  const page = runPage("options.js", { get_preferences: PREFERENCES, ui_snapshot: snapshot(), [SHOW]: { queued: true } });
  await settle();
  const show = page.byId("show-window");
  assert.equal(show.hidden, false);
  assert.equal(show.textContent, "Open Ghostlight");
  show.click();
  await settle();
  assert.deepEqual(page.sent.filter((message) => message.kind === SHOW), [{ kind: SHOW }]);

  const away = runPage("options.js", {
    get_preferences: PREFERENCES,
    ui_snapshot: snapshot({ connected: false, link_state: shared.LINK_HOST_ABSENT })
  });
  await settle();
  assert.equal(away.byId("show-window").hidden, true);
});
