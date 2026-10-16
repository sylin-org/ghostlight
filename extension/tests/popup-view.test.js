"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const shared = require("../lib/shared.js");
const popup = require("../lib/popup-view.js");

const { INTENT, OPEN, LIGHT } = popup;
const WIN = "win";
const LINUX = "linux";
const SHORTCUT = "Alt+Shift+P";

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

function away(linkState, patch = {}) {
  return snapshot({ connected: false, link_state: linkState, ...patch });
}

function actions(view) {
  return { primary: view.primary, secondary: view.secondary, quiet: view.quiet, release: view.release };
}

test("connected and active is ready, with a pause and a quiet end", () => {
  const view = popup.popupView(snapshot(), { shortcut: SHORTCUT, platform: WIN });
  assert.equal(view.light, LIGHT.LIT);
  assert.equal(view.eyebrow, "Ghostlight");
  assert.equal(view.title, "Ready");
  assert.equal(view.line, "No agent is browsing.");
  assert.equal(view.line2, null);
  assert.equal(view.note, null);
  assert.deepEqual(actions(view), {
    primary: null,
    secondary: { label: "Pause agents", intent: INTENT.HOLD, key: SHORTCUT },
    quiet: { label: "End session", intent: INTENT.END_SESSION },
    release: null
  });
  assert.deepEqual(view.settings, { label: "Settings", open: OPEN.OPTIONS });
});

test("one client browsing is named, several clients are not", () => {
  const one = popup.popupView(snapshot({
    activity: [{ client_label: "Claude Code", label: "Reading" }, { client_label: "Claude Code", label: "Clicking" }]
  }));
  assert.equal(one.line, "Claude Code is browsing.");
  const several = popup.popupView(snapshot({
    activity: [{ client_label: "Claude Code", label: "Reading" }, { client_label: "Cursor", label: "Clicking" }]
  }));
  assert.equal(several.line, "Agents are browsing.");
  const unnamed = popup.popupView(snapshot({ activity: [{ client_label: "", label: "Reading" }] }));
  assert.equal(unnamed.line, "Agents are browsing.");
});

test("held offers a primary resume with the shortcut", () => {
  const view = popup.popupView(snapshot({ control_state: "held" }), { shortcut: SHORTCUT, platform: LINUX });
  assert.equal(view.light, LIGHT.LIT);
  assert.equal(view.title, "Paused");
  assert.equal(view.line, "Agents can't use this browser until you resume.");
  assert.deepEqual(actions(view), {
    primary: { label: "Resume agents", intent: INTENT.RESUME, key: SHORTCUT },
    secondary: null,
    quiet: { label: "End session", intent: INTENT.END_SESSION },
    release: null
  });
});

test("an ended session offers a new one and no end", () => {
  const view = popup.popupView(snapshot({ control_state: "ended" }), { shortcut: SHORTCUT });
  assert.equal(view.light, LIGHT.LIT);
  assert.equal(view.title, "Session ended");
  assert.equal(view.line, "Agents can't use this browser until you start a new session.");
  assert.deepEqual(actions(view), {
    primary: null,
    secondary: { label: "Start a new session", intent: INTENT.START_SESSION },
    quiet: null,
    release: null
  });
});

test("there is no key cap without a shortcut", () => {
  for (const shortcut of [null, undefined, ""]) {
    const active = popup.popupView(snapshot(), { shortcut });
    assert.deepEqual(active.secondary, { label: "Pause agents", intent: INTENT.HOLD });
    const held = popup.popupView(snapshot({ control_state: "held" }), { shortcut });
    assert.deepEqual(held.primary, { label: "Resume agents", intent: INTENT.RESUME });
  }
  assert.deepEqual(popup.popupView(snapshot()).secondary, { label: "Pause agents", intent: INTENT.HOLD });
});

test("the recording note is singular for one tab and plural otherwise", () => {
  assert.equal(popup.popupView(snapshot({ recording_tabs: 1 })).note, "Recording in 1 tab.");
  assert.equal(popup.popupView(snapshot({ recording_tabs: 3 })).note, "Recording in 3 tabs.");
  assert.equal(popup.popupView(snapshot({ recording_tabs: 0 })).note, null);
});

test("a missing native host or an incompatible service says Ghostlight is not answering", () => {
  for (const view of [
    popup.popupView(away(shared.LINK_HOST_ABSENT), { platform: WIN }),
    popup.popupView(snapshot({ connected: true, compatible: false, link_state: shared.LINK_UNREACHABLE }), { platform: WIN })
  ]) {
    assert.equal(view.light, LIGHT.WAITING);
    assert.equal(view.title, "Not connected");
    assert.equal(view.line, "Ghostlight on this computer isn't answering.");
    assert.equal(view.line2, null);
    assert.deepEqual(actions(view), {
      primary: { label: "Set up this computer", open: OPEN.SETUP },
      secondary: null,
      quiet: null,
      release: null
    });
  }
});

test("an unreachable link asks to open Ghostlight and says where it is", () => {
  const windows = popup.popupView(away(shared.LINK_UNREACHABLE), { platform: WIN });
  assert.equal(windows.light, LIGHT.WAITING);
  assert.equal(windows.title, "Not connected");
  assert.equal(windows.line, "Open Ghostlight on this computer.");
  assert.equal(windows.line2, "It is in the Start menu.");
  assert.deepEqual(actions(windows), { primary: null, secondary: null, quiet: null, release: null });

  const linux = popup.popupView(away(shared.LINK_UNREACHABLE), { platform: LINUX });
  assert.equal(linux.line, "Open Ghostlight on this computer.");
  assert.equal(linux.line2, "It is in your applications.");
  assert.deepEqual(actions(linux), { primary: null, secondary: null, quiet: null, release: null });

  const elsewhere = popup.popupView(away(shared.LINK_UNREACHABLE), { platform: "mac" });
  assert.equal(elsewhere.line2, null);
});

test("an outdated service offers an update, and outranks a missing host", () => {
  const view = popup.popupView(
    away(shared.LINK_SERVICE_OUTDATED, { compatible: false, last_error: "Specified native messaging host not found." }),
    { platform: WIN }
  );
  assert.equal(view.light, LIGHT.WAITING);
  assert.equal(view.title, "Update needed");
  assert.equal(view.line, "Update Ghostlight on this computer.");
  assert.deepEqual(view.primary, { label: "Update this computer", open: OPEN.SETUP });
  assert.equal(view.secondary, null);
});

test("a debugger release is offered only when not connected and tabs are attached", () => {
  const one = popup.popupView(away(shared.LINK_UNREACHABLE, { attached_tabs: 1 }), { platform: WIN });
  assert.deepEqual(one.release, { label: "Release debugger session (1 tab)", release: true });
  const many = popup.popupView(away(shared.LINK_HOST_ABSENT, { attached_tabs: 4 }), { platform: WIN });
  assert.deepEqual(many.release, { label: "Release debugger sessions (4 tabs)", release: true });
  assert.equal(popup.popupView(away(shared.LINK_UNREACHABLE, { attached_tabs: 0 })).release, null);
  assert.equal(popup.popupView(snapshot({ attached_tabs: 2 })).release, null);
  assert.equal(popup.popupView(snapshot({ attached_tabs: 2, control_state: "held" })).release, null);
});

test("the recording note shows beside every state", () => {
  assert.equal(popup.popupView(snapshot({ control_state: "held", recording_tabs: 2 })).note, "Recording in 2 tabs.");
  assert.equal(popup.popupView(away(shared.LINK_UNREACHABLE, { recording_tabs: 1 })).note, "Recording in 1 tab.");
});

test("the connection sentence agrees with the popup for each link state", () => {
  const cases = [
    away(shared.LINK_HOST_ABSENT),
    away(shared.LINK_SERVICE_OUTDATED, { compatible: false }),
    away(shared.LINK_UNREACHABLE),
    snapshot({ compatible: false, link_state: shared.LINK_UNREACHABLE })
  ];
  for (const platform of [WIN, LINUX]) {
    for (const entry of cases) {
      const view = popup.popupView(entry, { platform });
      const connection = popup.connection(entry, { platform });
      assert.equal(connection.title, view.title);
      assert.ok(connection.line.startsWith(view.line));
      assert.equal(connection.line, view.line2 ? `${view.line} ${view.line2}` : view.line);
      assert.equal(connection.needsSetup, view.primary !== null);
      assert.equal(connection.setupLabel, view.primary ? view.primary.label : null);
    }
  }
  const connected = popup.connection(snapshot(), { platform: WIN });
  assert.deepEqual(connected, {
    title: "Connected",
    line: "Ghostlight on this computer is answering.",
    needsSetup: false,
    setupLabel: null
  });
});

test("an unreachable connection names where Ghostlight is and offers no setup", () => {
  const windows = popup.connection(away(shared.LINK_UNREACHABLE), { platform: WIN });
  assert.equal(windows.line, "Open Ghostlight on this computer. It is in the Start menu.");
  assert.equal(windows.needsSetup, false);
  const linux = popup.connection(away(shared.LINK_UNREACHABLE), { platform: LINUX });
  assert.equal(linux.line, "Open Ghostlight on this computer. It is in your applications.");
});

test("the options connection is Connected for a live link whatever the session says", () => {
  for (const controlState of ["active", "held", "ended"]) {
    const connection = popup.connection(snapshot({ control_state: controlState }), { platform: WIN });
    assert.equal(connection.title, popup.WORDS.connected.title);
    assert.equal(connection.needsSetup, false);
  }
});

test("an outdated service offers its update on the options connection", () => {
  const connection = popup.connection(away(shared.LINK_SERVICE_OUTDATED, { compatible: false }), { platform: LINUX });
  assert.equal(connection.title, "Update needed");
  assert.equal(connection.needsSetup, true);
  assert.equal(connection.setupLabel, "Update this computer");
});
