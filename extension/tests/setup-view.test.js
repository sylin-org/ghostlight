"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const shared = require("../lib/shared.js");
const setup = require("../lib/setup-view.js");

const { STATE, STEP, LIGHT, WORDS, COMMANDS, LINKS } = setup;

const WIN = "win";
const LINUX = "linux";

function states(model) {
  return model.steps.map((entry) => entry.state);
}

function words(model) {
  return model.steps.map((entry) => entry.word);
}

function headline(model) {
  const { lead, accent, trail } = model.headline;
  return `${lead}${accent}${trail}`;
}

test("a missing native host asks for the install command for the platform", () => {
  const windows = setup.view({ link_state: shared.LINK_HOST_ABSENT, connected: false, compatible: true, platform: WIN });
  assert.equal(windows.state, STATE.COMPUTER);
  assert.equal(headline(windows), "Install Ghostlight on this computer.");
  assert.equal(windows.headline.accent, "this computer");
  assert.equal(windows.eyebrow, "Set up Ghostlight");
  assert.equal(windows.command.label, "Paste into PowerShell");
  assert.equal(windows.command.prompt, false);
  assert.equal(windows.command.text, "irm https://sylin.org/ghostlight/get.ps1 | iex");
  assert.equal(windows.live, "Waiting for this computer.");
  assert.equal(windows.lead, null);
  assert.equal(windows.action, null);

  const linux = setup.view({ link_state: shared.LINK_HOST_ABSENT, connected: false, compatible: true, platform: LINUX });
  assert.equal(linux.state, STATE.COMPUTER);
  assert.equal(linux.command.label, "Paste into a terminal");
  assert.equal(linux.command.prompt, true);
  assert.equal(linux.command.text, "curl -fsSL https://sylin.org/ghostlight/get.sh | sh");
});

test("an unreachable service says where to open Ghostlight and offers no command", () => {
  const windows = setup.view({ link_state: shared.LINK_UNREACHABLE, connected: false, compatible: true, platform: WIN });
  assert.equal(windows.state, STATE.START);
  assert.equal(headline(windows), "Open Ghostlight on this computer.");
  assert.equal(windows.lead, "It is in the Start menu.");
  assert.equal(windows.command, null);
  assert.equal(windows.fallback, null);
  assert.equal(windows.live, "Waiting for this computer.");

  const linux = setup.view({ link_state: shared.LINK_UNREACHABLE, connected: false, compatible: true, platform: LINUX });
  assert.equal(linux.state, STATE.START);
  assert.equal(linux.lead, "It is in your applications.");
});

test("an outdated service asks for the update with the same command", () => {
  const model = setup.view({ link_state: shared.LINK_SERVICE_OUTDATED, connected: false, compatible: false, platform: LINUX });
  assert.equal(model.state, STATE.UPDATE);
  assert.equal(headline(model), "Update Ghostlight on this computer.");
  assert.equal(model.command, COMMANDS.linux.install);
  assert.equal(model.live, "Waiting for this computer.");
});

test("the fallback is the install's own finishing command, only where a command is asked for", () => {
  const windows = setup.view({ link_state: shared.LINK_HOST_ABSENT, platform: WIN });
  assert.equal(windows.fallback.title, "Ran it inside another app?");
  assert.equal(windows.fallback.text, "Run this once from a regular terminal.");
  assert.equal(windows.fallback.command.label, "Paste into PowerShell from the Start menu");
  assert.equal(windows.fallback.command.text, "~\\.ghostlight\\bin\\ghostlight.exe install");
  assert.equal(windows.fallback.command.prompt, false);

  const linux = setup.view({ link_state: shared.LINK_SERVICE_OUTDATED, platform: LINUX });
  assert.equal(linux.fallback.command.label, "Paste into a terminal");
  assert.equal(linux.fallback.command.text, "~/.ghostlight/bin/ghostlight install");
  assert.equal(linux.fallback.command.prompt, true);
});

test("a waiting state shows 01 current, 02 added, 03 to do, with the pilot light", () => {
  for (const link_state of [shared.LINK_HOST_ABSENT, shared.LINK_UNREACHABLE, shared.LINK_SERVICE_OUTDATED]) {
    const model = setup.view({ link_state, connected: false, compatible: true, platform: WIN });
    assert.deepEqual(states(model), [STEP.CURRENT, STEP.DONE, STEP.TODO]);
    assert.deepEqual(words(model), ["", "Added", ""]);
    assert.equal(model.light, LIGHT.WAITING);
  }
});

test("a connected link names the browser by its short name and offers the window", () => {
  const cases = [
    ["Google Chrome", "Connected in Chrome"],
    ["Microsoft Edge", "Connected in Edge"],
    ["Brave", "Connected in Brave"],
    ["Opera", "Connected in Chromium"],
    [null, "Connected in Chromium"]
  ];
  for (const [browser, word] of cases) {
    const model = setup.view({ link_state: shared.LINK_CONNECTED, connected: true, compatible: true, platform: LINUX, browser });
    assert.equal(model.state, STATE.MET);
    assert.equal(headline(model), "Your browser and this computer are connected.");
    assert.equal(model.headline.accent, "connected");
    assert.equal(model.lead, "One step left: your agent.");
    assert.deepEqual(states(model), [STEP.DONE, STEP.DONE, STEP.CURRENT]);
    assert.deepEqual(words(model), ["Installed", word, ""]);
    assert.equal(model.light, LIGHT.LIT);
    assert.equal(model.live, null);
    assert.equal(model.command, null);
    assert.deepEqual(model.action, { label: "Open Ghostlight", kind: "show_ghostlight" });
  }
});

test("a connection that is not compatible is not met", () => {
  const model = setup.view({ link_state: shared.LINK_UNREACHABLE, connected: true, compatible: false, platform: WIN });
  assert.notEqual(model.state, STATE.MET);
});

test("a computer Ghostlight does not run on says so and turns every step off", () => {
  for (const platform of ["mac", "cros", "android", undefined]) {
    const model = setup.view({ link_state: shared.LINK_HOST_ABSENT, connected: false, compatible: true, platform });
    assert.equal(model.state, STATE.UNSUPPORTED);
    assert.equal(headline(model), "Ghostlight runs on Windows and Linux.");
    assert.equal(model.lead, "It does not run on this computer yet.");
    assert.deepEqual(states(model), [STEP.OFF, STEP.OFF, STEP.OFF]);
    assert.deepEqual(words(model), ["Not supported", "", ""]);
    assert.equal(model.light, LIGHT.OFF);
    assert.equal(model.action, null);
    assert.equal(model.command, null);
    assert.equal(model.live, null);
  }
});

test("platformKey accepts only the two supported computers", () => {
  assert.equal(setup.platformKey("win"), "win");
  assert.equal(setup.platformKey("linux"), "linux");
  assert.equal(setup.platformKey("mac"), null);
  assert.equal(setup.platformKey("cros"), null);
});

test("shortBrowser maps the four names", () => {
  assert.equal(setup.shortBrowser("Google Chrome"), "Chrome");
  assert.equal(setup.shortBrowser("Microsoft Edge"), "Edge");
  assert.equal(setup.shortBrowser("Brave"), "Brave");
  assert.equal(setup.shortBrowser("Vivaldi"), "Chromium");
  assert.equal(setup.shortBrowser(undefined), "Chromium");
});

test("the fallback waits ninety seconds after a copy and two minutes after opening", () => {
  const openedAt = 1_000_000;
  const base = { openedAt, visible: true, connected: false };
  const copiedAt = openedAt + 10_000;
  assert.equal(setup.fallbackDue({ ...base, copiedAt, now: copiedAt + 89_999 }), false);
  assert.equal(setup.fallbackDue({ ...base, copiedAt, now: copiedAt + 90_000 }), true);
  assert.equal(setup.fallbackDue({ ...base, copiedAt: null, now: openedAt + 119_999 }), false);
  assert.equal(setup.fallbackDue({ ...base, copiedAt: null, now: openedAt + 120_000 }), true);
});

test("the fallback never shows while the page is hidden or Ghostlight answers", () => {
  const late = { openedAt: 0, copiedAt: 0, now: 600_000 };
  assert.equal(setup.fallbackDue({ ...late, visible: true, connected: false }), true);
  assert.equal(setup.fallbackDue({ ...late, visible: false, connected: false }), false);
  assert.equal(setup.fallbackDue({ ...late, visible: true, connected: true }), false);
});

test("the documentation link is the product's own page", () => {
  assert.equal(LINKS.documentation.label, "Documentation");
  assert.equal(LINKS.documentation.url, "https://sylin.org/ghostlight/");
});

test("every word on the page is ASCII", () => {
  const seen = [];
  const collect = (value) => {
    if (typeof value === "string") seen.push(value);
    else if (typeof value === "function") seen.push(value("Chrome"));
    else if (value && typeof value === "object") Object.values(value).forEach(collect);
  };
  collect(WORDS);
  collect(COMMANDS);
  collect(LINKS);
  assert.ok(seen.length > 30);
  for (const text of seen) assert.match(text, /^[\x20-\x7e]*$/, text);
});

test("an extension the service refused asks to update Ghostlight in Browser and offers no command", () => {
  const refusal = "Ghostlight in Browser is out of date.";
  const model = setup.view({ link_state: shared.LINK_ADAPTER_OUTDATED, connected: false, compatible: false, platform: WIN, refusal });
  assert.equal(model.state, STATE.UPDATE_BROWSER);
  assert.equal(headline(model), "Update Ghostlight in Browser.");
  assert.equal(model.lead, refusal);
  assert.equal(model.command, null);
  assert.equal(model.fallback, null);
  assert.equal(model.live, null);
  assert.deepEqual(states(model), [STEP.DONE, STEP.CURRENT, STEP.TODO]);
  assert.deepEqual(words(model), ["Installed", "", ""]);
  assert.equal(setup.view({ link_state: shared.LINK_ADAPTER_OUTDATED, platform: LINUX }).lead, null);
});
