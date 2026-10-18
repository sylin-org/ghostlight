// Ghostlight -- what the toolbar popup says. A pure function from the extension's connection
// snapshot, the toggle shortcut and the computer it runs on to the words and the actions; no DOM,
// no chrome.*. The words live here once, so the popup script only places them, the options page
// reads the same connection sentence, and a test can read every state.
(function installGhostlightPopupView(root, factory) {
  const required = typeof module !== "undefined" && module.exports;
  const shared = required ? require("./shared.js") : root.GhostlightShared;
  const setup = required ? require("./setup-view.js") : root.GhostlightSetupView;
  const api = factory(shared, setup);
  root.GhostlightPopupView = api;
  if (required) module.exports = api;
})(globalThis, function createGhostlightPopupView(shared, setup) {
  "use strict";

  // The popup's lantern: the setup page's lights, plus held, the waiting ghost with an amber ember
  // that the window and the tray also show for Paused.
  const LIGHT = Object.freeze({ ...setup.LIGHT, HELD: "held" });
  // The runtime_control intents the popup sends. The orchestrator owns the state each one moves.
  const INTENT = Object.freeze({
    HOLD: "hold",
    RESUME: "resume",
    END_SESSION: "end_session",
    START_SESSION: "start_session"
  });
  // Where an action that opens a page goes. The script maps each to the extension's own page.
  const OPEN = Object.freeze({ SETUP: "setup", OPTIONS: "options" });
  const CONTROL = Object.freeze({ HELD: "held", ENDED: "ended" });
  // How the popup reads the link: the one answer it needs from the snapshot.
  const LINK = Object.freeze({ CONNECTED: "connected", OUTDATED: "outdated", ABSENT: "absent", START: "start" });

  const WORDS = Object.freeze({
    eyebrow: "Ghostlight",
    ready: "Ready",
    idle: "No agent is browsing.",
    oneBrowsing: (client) => `${client} is browsing.`,
    manyBrowsing: "Agents are browsing.",
    pause: "Pause agents",
    held: Object.freeze({ title: "Paused", line: "Agents can't use this browser until you resume." }),
    resume: "Resume agents",
    ended: Object.freeze({
      title: "Session ended",
      line: "Agents can't use this browser until you start a new session."
    }),
    startSession: "Start a new session",
    endSession: "End session",
    recording: (tabs) => (tabs === 1 ? "Recording in 1 tab." : `Recording in ${tabs} tabs.`),
    notConnected: "Not connected",
    notAnswering: "Ghostlight on this computer isn't answering.",
    setUp: "Set up this computer",
    outdated: Object.freeze({
      title: "Update needed",
      line: "Update Ghostlight on this computer.",
      action: "Update this computer"
    }),
    release: (tabs) => (tabs === 1 ? "Release debugger session (1 tab)" : `Release debugger sessions (${tabs} tabs)`),
    settings: "Settings",
    show: setup.WORDS.met.action,
    connected: Object.freeze({ title: "Connected", line: "Ghostlight on this computer is answering." })
  });

  // The sentence that asks the person to start Ghostlight: "Open Ghostlight on this computer."
  const OPEN_LINE = `${setup.WORDS.open.lead}${setup.WORDS.open.accent}${setup.WORDS.open.trail}`;

  function link(snapshot) {
    if (snapshot.link_state === shared.LINK_SERVICE_OUTDATED) return LINK.OUTDATED;
    if (snapshot.connected && snapshot.compatible) return LINK.CONNECTED;
    if (snapshot.link_state === shared.LINK_HOST_ABSENT || !snapshot.compatible) return LINK.ABSENT;
    return LINK.START;
  }

  // The words for a link that is not connected; the popup and the options page both read these.
  function disconnected(kind, platform) {
    const place = setup.WORDS.openLead[setup.platformKey(platform)] ?? null;
    if (kind === LINK.OUTDATED) {
      return {
        title: WORDS.outdated.title,
        line: WORDS.outdated.line,
        line2: null,
        action: { label: WORDS.outdated.action, open: OPEN.SETUP }
      };
    }
    if (kind === LINK.ABSENT) {
      return {
        title: WORDS.notConnected,
        line: WORDS.notAnswering,
        line2: null,
        action: { label: WORDS.setUp, open: OPEN.SETUP }
      };
    }
    return { title: WORDS.notConnected, line: OPEN_LINE, line2: place, action: null };
  }

  function browsing(activity) {
    const entries = Array.isArray(activity) ? activity : [];
    if (entries.length === 0) return WORDS.idle;
    const first = entries[0]?.client_label;
    const same = typeof first === "string" && first !== "" && entries.every((entry) => entry?.client_label === first);
    return same ? WORDS.oneBrowsing(first) : WORDS.manyBrowsing;
  }

  // The quiet link to the Ghostlight window. The worker can only ask the service to show it over
  // a live link, so it exists only while connected.
  function showWindow(kind) {
    return kind === LINK.CONNECTED ? { label: WORDS.show, kind: setup.SHOW_GHOSTLIGHT } : null;
  }

  // A hold or resume button carries the toggle shortcut when the browser has one.
  function keyed(action, shortcut) {
    return typeof shortcut === "string" && shortcut !== "" ? { ...action, key: shortcut } : action;
  }

  function popupView(snapshot, { shortcut = null, platform = null } = {}) {
    const kind = link(snapshot);
    const view = {
      light: kind === LINK.CONNECTED ? LIGHT.LIT : LIGHT.WAITING,
      eyebrow: WORDS.eyebrow,
      title: "",
      line: "",
      line2: null,
      note: snapshot.recording_tabs > 0 ? WORDS.recording(snapshot.recording_tabs) : null,
      primary: null,
      secondary: null,
      quiet: null,
      release: null,
      show: showWindow(kind),
      settings: { label: WORDS.settings, open: OPEN.OPTIONS }
    };
    if (kind !== LINK.CONNECTED) {
      const words = disconnected(kind, platform);
      view.title = words.title;
      view.line = words.line;
      view.line2 = words.line2;
      view.primary = words.action;
      const attached = snapshot.attached_tabs || 0;
      if (attached > 0) view.release = { label: WORDS.release(attached), release: true };
      return view;
    }
    if (snapshot.control_state === CONTROL.ENDED) {
      view.light = LIGHT.WAITING;
      view.title = WORDS.ended.title;
      view.line = WORDS.ended.line;
      view.secondary = { label: WORDS.startSession, intent: INTENT.START_SESSION };
      return view;
    }
    view.quiet = { label: WORDS.endSession, intent: INTENT.END_SESSION };
    if (snapshot.control_state === CONTROL.HELD) {
      view.light = LIGHT.HELD;
      view.title = WORDS.held.title;
      view.line = WORDS.held.line;
      view.primary = keyed({ label: WORDS.resume, intent: INTENT.RESUME }, shortcut);
      return view;
    }
    view.title = WORDS.ready;
    view.line = browsing(snapshot.activity);
    view.secondary = keyed({ label: WORDS.pause, intent: INTENT.HOLD }, shortcut);
    return view;
  }

  // The connection sentence the options page shows. It agrees with the popup for every link state;
  // needsSetup says the person has a page to open, setupLabel is that button's name, and show is
  // the same quiet link to the Ghostlight window the popup offers.
  function connection(snapshot, { platform = null } = {}) {
    const kind = link(snapshot);
    if (kind === LINK.CONNECTED) {
      return {
        title: WORDS.connected.title,
        line: WORDS.connected.line,
        needsSetup: false,
        setupLabel: null,
        show: showWindow(kind)
      };
    }
    const words = disconnected(kind, platform);
    return {
      title: words.title,
      line: words.line2 ? `${words.line} ${words.line2}` : words.line,
      needsSetup: words.action !== null,
      setupLabel: words.action ? words.action.label : null,
      show: null
    };
  }

  return Object.freeze({ LIGHT, INTENT, OPEN, WORDS, popupView, connection });
});
