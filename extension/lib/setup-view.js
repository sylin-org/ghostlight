// Ghostlight -- what the setup page says. A pure function from the extension's connection snapshot
// and the computer it runs on to the words, the command and the three steps; no DOM, no chrome.*.
// The words live here, once, so the page script only places them and a test can read them all.
(function installGhostlightSetupView(root, factory) {
  const shared = typeof module !== "undefined" && module.exports ? require("./shared.js") : root.GhostlightShared;
  const api = factory(shared);
  root.GhostlightSetupView = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightSetupView(shared) {
  "use strict";

  const STATE = Object.freeze({
    COMPUTER: "computer",
    START: "start",
    UPDATE: "update",
    MET: "met",
    UNSUPPORTED: "unsupported"
  });
  const STEP = Object.freeze({ DONE: "done", CURRENT: "current", TODO: "todo", OFF: "off" });
  const LIGHT = Object.freeze({ WAITING: "waiting", LIT: "lit", OFF: "off" });
  const PLATFORM = Object.freeze({ WIN: "win", LINUX: "linux" });
  // The one page message the met state sends. The worker answers it by asking the service to show
  // its window.
  const SHOW_GHOSTLIGHT = "show_ghostlight";

  // A fallback appears only when Ghostlight still does not answer after the person had time to
  // run the command. The get script downloads three images, so a cold run is not instant.
  const FALLBACK_AFTER_COPY_MS = 90000;
  const FALLBACK_AFTER_OPEN_MS = 120000;

  const BROWSER_SHORT = Object.freeze({
    "google chrome": "Chrome",
    "microsoft edge": "Edge",
    "brave": "Brave"
  });
  const BROWSER_FALLBACK = "Chromium";

  const WORDS = Object.freeze({
    title: "Set up Ghostlight",
    eyebrow: "Set up Ghostlight",
    steps: Object.freeze(["This computer", "Your browser", "Your agent"]),
    added: "Added",
    installed: "Installed",
    notSupported: "Not supported",
    connectedIn: (browser) => `Connected in ${browser}`,
    // Appended to a step name for a screen reader, in this order of meaning.
    stepSuffix: Object.freeze({
      [STEP.DONE]: ", done",
      [STEP.CURRENT]: ", current step",
      [STEP.OFF]: ", not supported",
      [STEP.TODO]: ""
    }),
    waiting: "Waiting for this computer.",
    install: Object.freeze({ lead: "Install Ghostlight on ", accent: "this computer", trail: "." }),
    update: Object.freeze({ lead: "Update Ghostlight on ", accent: "this computer", trail: "." }),
    open: Object.freeze({ lead: "Open Ghostlight on ", accent: "this computer", trail: "." }),
    openLead: Object.freeze({
      [PLATFORM.WIN]: "It is in the Start menu.",
      [PLATFORM.LINUX]: "It is in your applications."
    }),
    met: Object.freeze({
      headline: Object.freeze({ lead: "Your browser and this computer are ", accent: "connected", trail: "." }),
      lead: "One step left: your agent.",
      announcement: "Your browser and this computer are connected. One step left: your agent.",
      action: "Open Ghostlight"
    }),
    unsupported: Object.freeze({
      headline: Object.freeze({ lead: "Ghostlight runs on Windows and Linux.", accent: "", trail: "" }),
      lead: "It does not run on this computer yet."
    }),
    copy: "Copy",
    copied: "Copied",
    copyName: "Copy command",
    fallback: Object.freeze({
      title: "Ran it inside another app?",
      text: "Run this once from a regular terminal."
    })
  });

  const LINKS = Object.freeze({
    documentation: Object.freeze({ label: "Documentation", url: "https://sylin.org/ghostlight/" })
  });

  // Each command is shown in a panel. install is the get script one-liner; finish is the install's
  // own finishing command by full path, for a person who ran the first one inside another app.
  const COMMANDS = Object.freeze({
    [PLATFORM.WIN]: Object.freeze({
      install: Object.freeze({
        label: "Paste into PowerShell",
        text: "irm https://sylin.org/ghostlight/get.ps1 | iex",
        prompt: false
      }),
      finish: Object.freeze({
        label: "Paste into PowerShell from the Start menu",
        text: "~\\.ghostlight\\bin\\ghostlight.exe install",
        prompt: false
      })
    }),
    [PLATFORM.LINUX]: Object.freeze({
      install: Object.freeze({
        label: "Paste into a terminal",
        text: "curl -fsSL https://sylin.org/ghostlight/get.sh | sh",
        prompt: true
      }),
      finish: Object.freeze({
        label: "Paste into a terminal",
        text: "~/.ghostlight/bin/ghostlight install",
        prompt: true
      })
    })
  });

  // chrome.runtime.getPlatformInfo().os -> the two computers Ghostlight runs on, or null.
  function platformKey(os) {
    if (os === "win") return PLATFORM.WIN;
    if (os === "linux") return PLATFORM.LINUX;
    return null;
  }

  function shortBrowser(name) {
    const key = typeof name === "string" ? name.trim().toLowerCase() : "";
    return BROWSER_SHORT[key] ?? BROWSER_FALLBACK;
  }

  function step(state, word = "") {
    return { state, word };
  }

  // The steps a person sees while this computer is the missing half: Ghostlight is not answering,
  // the browser side is already in place.
  function waitingSteps() {
    return [step(STEP.CURRENT), step(STEP.DONE, WORDS.added), step(STEP.TODO)];
  }

  function view({ link_state, connected, compatible, platform, browser } = {}) {
    const key = platformKey(platform);
    if (!key) {
      return {
        state: STATE.UNSUPPORTED,
        eyebrow: WORDS.eyebrow,
        headline: WORDS.unsupported.headline,
        lead: WORDS.unsupported.lead,
        command: null,
        fallback: null,
        live: null,
        steps: [step(STEP.OFF, WORDS.notSupported), step(STEP.OFF), step(STEP.OFF)],
        light: LIGHT.OFF,
        action: null
      };
    }
    if (connected && compatible) {
      return {
        state: STATE.MET,
        eyebrow: WORDS.eyebrow,
        headline: WORDS.met.headline,
        lead: WORDS.met.lead,
        command: null,
        fallback: null,
        live: null,
        steps: [
          step(STEP.DONE, WORDS.installed),
          step(STEP.DONE, WORDS.connectedIn(shortBrowser(browser))),
          step(STEP.CURRENT)
        ],
        light: LIGHT.LIT,
        action: { label: WORDS.met.action, kind: SHOW_GHOSTLIGHT }
      };
    }
    const waiting = {
      eyebrow: WORDS.eyebrow,
      live: WORDS.waiting,
      steps: waitingSteps(),
      light: LIGHT.WAITING,
      action: null
    };
    if (link_state === shared.LINK_HOST_ABSENT || link_state === shared.LINK_SERVICE_OUTDATED) {
      const update = link_state === shared.LINK_SERVICE_OUTDATED;
      return {
        ...waiting,
        state: update ? STATE.UPDATE : STATE.COMPUTER,
        headline: update ? WORDS.update : WORDS.install,
        lead: null,
        command: COMMANDS[key].install,
        fallback: { ...WORDS.fallback, command: COMMANDS[key].finish }
      };
    }
    return {
      ...waiting,
      state: STATE.START,
      headline: WORDS.open,
      lead: WORDS.openLead[key],
      command: null,
      fallback: null
    };
  }

  // True when the fallback should show: Ghostlight still does not answer, the page is in front of
  // the person, and enough time passed. The clock is the copy when there was one, otherwise the
  // moment the page opened.
  function fallbackDue({ openedAt, copiedAt, now, visible, connected }) {
    if (connected || !visible) return false;
    if (typeof copiedAt === "number") return now - copiedAt >= FALLBACK_AFTER_COPY_MS;
    return now - openedAt >= FALLBACK_AFTER_OPEN_MS;
  }

  return Object.freeze({
    STATE,
    STEP,
    LIGHT,
    PLATFORM,
    SHOW_GHOSTLIGHT,
    FALLBACK_AFTER_COPY_MS,
    FALLBACK_AFTER_OPEN_MS,
    WORDS,
    LINKS,
    COMMANDS,
    platformKey,
    shortBrowser,
    view,
    fallbackDue
  });
});
