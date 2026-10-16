// Ghostlight workbench -- the fixed vocabulary this window speaks, and how it renders a number.
(function installGhostlightWords(root, factory) {
  const api = factory();
  root.GhostlightWords = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightWords() {
  "use strict";

  /** Bound on the retained feed, matching the orchestrator's own bounded history. */
  const FEED_LIMIT = 200;

  /** Said under the rule editor when the applied rules changed elsewhere while edits were unsaved. */
  const POLICY_CHANGED_ELSEWHERE = "The applied rules changed elsewhere. Discard your edits to see them.";

  /** The button that brings the Ghostlight application forward, where a change can be made. */
  const CONTINUE_LABEL = "Continue in the Ghostlight window";

  /** The button that takes a draft of the rules to the Ghostlight application to be applied. */
  const APPLY_IN_WINDOW_LABEL = "Apply in the Ghostlight window";

  /** Said after Copy setup puts a client's manual setup on the clipboard. */
  const COPIED_SETUP = "Manual setup copied.";

  /** Said after Copy MCP command puts a client's command on the clipboard. */
  const COPIED_COMMAND = "MCP command copied.";

  /** The band's word while the live stream is lost and being opened again. */
  const RECONNECTING = "Reconnecting";

  /** Said after Settings turns "Others can watch" on. */
  const WATCHING_TURNED_ON = "Others can watch again.";

  /** Said after Settings turns "Others can watch" off. */
  const WATCHING_TURNED_OFF = "Only this window shows Ghostlight now.";

  /** Said after Settings sends the notification test. */
  const TEST_NOTIFICATION_SENT = "Test notification sent.";

  /** Said after Settings copies the address of the watch view. */
  const ADDRESS_COPIED = "Address copied.";

  /**
   * The sections of Settings, in the order of its rail.
   *
   * `id` is the section's address (`#settings/<id>`) and `word` names its label in SETTINGS.
   */
  const SETTINGS_SECTIONS = Object.freeze([
    Object.freeze({ id: "screen", word: "sectionScreen" }),
    Object.freeze({ id: "sharing", word: "sectionSharing" }),
    Object.freeze({ id: "browser-work", word: "sectionBrowserWork" }),
    Object.freeze({ id: "notifications", word: "sectionNotifications" }),
    Object.freeze({ id: "diagnostics", word: "sectionDiagnostics" })
  ]);

  /**
   * The labels of Settings that belong to this page: the rail, the personal settings, the verb on
   * each button and the question Quit asks. The service says everything else, in its own part.
   */
  const SETTINGS = Object.freeze({
    sectionScreen: "This screen",
    sectionSharing: "Sharing",
    sectionBrowserWork: "Browser work",
    sectionNotifications: "Notifications",
    sectionDiagnostics: "Diagnostics",
    density: "Density",
    comfortable: "Comfortable",
    compact: "Compact",
    motion: "Motion",
    followSystem: "Follow the system",
    reduced: "Reduced",
    pause: "Pause",
    resume: "Resume",
    startSession: "Start session",
    endSession: "End session",
    turnOn: "Turn on",
    turnOff: "Turn off",
    copyAddress: "Copy address",
    openLogFolder: "Open log folder",
    sendTest: "Send test",
    quitLabel: "Ghostlight",
    quit: "Quit Ghostlight",
    quitQuestion: "Quit now? Agents lose the browser until Ghostlight starts again.",
    quitYes: "Quit",
    quitNo: "Cancel",
    continueInWindow: CONTINUE_LABEL
  });

  /** How the personal settings read together, as the rail's state word for This screen. */
  function screenState(density, motion) {
    const dense = density === "compact" ? SETTINGS.compact : SETTINGS.comfortable;
    const moving = motion === "reduced" ? "motion reduced" : "motion follows the system";
    return `${dense}, ${moving}`;
  }

  /** How a count of bytes reads in Settings. */
  function byteSize(bytes) {
    if (!Number.isFinite(bytes) || bytes < 1024) return `${Math.max(0, Math.floor(bytes || 0))} bytes`;
    const units = ["KB", "MB", "GB"];
    let value = bytes / 1024;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit += 1;
    }
    return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
  }

  /** Destinations this surface renders, keyed by the orchestrator's search vocabulary. */
  const VIEWS = {
    monitor: "At a glance", integrations: "MCP integrations", status: "Status", policy: "Policy",
    settings: "Settings", about: "About"
  };

  /** The destination a page opens on, and the section of Settings it opens on. */
  const DEFAULT_VIEW = "monitor";
  const DEFAULT_SECTION = "screen";

  /**
   * Every address a tab, a rail entry or a typed hash can name, and no other.
   *
   * Search destinations are not addresses: they are the service's words for what a hit is about,
   * and SEARCH_VIEWS turns them into a view.
   */
  const ADDRESSES = Object.freeze([
    "monitor", "integrations", "status", "policy", "settings", "settings/screen", "settings/sharing",
    "settings/browser-work", "settings/notifications", "settings/diagnostics", "about"
  ]);

  /**
   * Read an address, with or without its leading `#`.
   *
   * Answers `{ view, section }`, or null when the text is not an address. `section` is null for
   * every view but Settings, and for the bare `settings` address.
   */
  function parseAddress(text) {
    const address = String(text ?? "").replace(/^#/, "");
    if (!ADDRESSES.includes(address)) return null;
    const [view, section] = address.split("/");
    return { view, section: section ?? null };
  }

  /** The address of a view, and of a section when the view is Settings. */
  function addressOf(view, section) {
    return view === "settings" && section ? `settings/${section}` : view;
  }

  /**
   * How long the band keeps saying "Working" after the last thing happened.
   *
   * Per-operation truth flickers: most calls settle in well under a second, so a label tied to
   * them strobes between two words and reads as a fault. What a person wants from that corner is
   * coarser and more useful -- whether anything is currently working through Ghostlight -- so the
   * word latches, and every new action pushes the deadline back.
   */
  const WORKING_LATCH_MS = 10_000;
  const SEARCH_VIEWS = {
    home: "monitor",
    activity: "monitor",
    history: "monitor",
    checkup: "status",
    configuration: "policy",
    install: "integrations"
  };

  /**
   * The integration roster's card categories, in presentation order.
   *
   * Product state and presentation order are separate facts: an available product is shown before
   * one that needs attention, while attention still outranks availability when several concrete
   * targets share one product card.
   */
  const INTEGRATION_CATEGORIES = Object.freeze([
    Object.freeze({ id: "ready", label: "Ready" }),
    Object.freeze({ id: "available", label: "Available" }),
    Object.freeze({ id: "needs-attention", label: "Needs Attention" }),
    Object.freeze({ id: "not-detected", label: "Not Detected" })
  ]);

  /** Map every closed harness state to the one category a product card may occupy. */
  const INTEGRATION_STATE_CATEGORY = Object.freeze({
    installed: "ready",
    available: "available",
    updatable: "needs-attention",
    needs_attention: "needs-attention",
    not_detected: "not-detected"
  });

  /** Resolve a plural product card from its strongest concrete target state. */
  const INTEGRATION_CATEGORY_PRIORITY = Object.freeze([
    "ready", "needs-attention", "available", "not-detected"
  ]);

  /* --------------------------------------------------------------------------
   * The medallion vocabulary, keyed by the orchestrator's fixed activity labels.
   * These are the same four shapes the renderer draws inside the page, so the
   * window and the browser tell the same story.
   * ----------------------------------------------------------------------- */
  const GLYPHS = {
    scan: '<svg viewBox="0 0 24 24"><rect x="3.5" y="4" width="17" height="16" rx="2.5"/><g class="scanline"><path d="M7 12h10"/></g></svg>',
    navigate: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5"/><path d="M15.5 8.5 10.6 10.6 8.5 15.5l4.9-2.1z"/></svg>',
    pointer: '<svg viewBox="0 0 24 24"><path d="M6 3.5 18.5 12 13 13.2l-2.2 5.6z"/><path d="M13.2 13.4 17.5 18"/></svg>',
    keyboard: '<svg viewBox="0 0 24 24"><rect x="2.5" y="6.5" width="19" height="11" rx="2"/><g class="keylight"><path d="M6.5 10.5h.01"/></g><g class="keylight"><path d="M10.5 10.5h.01"/></g><g class="keylight"><path d="M14.5 10.5h.01"/></g><path d="M8 14h8"/></svg>',
    workwheel: '<svg viewBox="0 0 24 24"><g class="spin"><circle cx="12" cy="12" r="7.5" stroke-dasharray="5 4"/></g><circle class="particle" cx="12" cy="3.4" r="1.25" fill="currentColor" stroke="none"/><circle class="particle" cx="19.4" cy="16" r="1.25" fill="currentColor" stroke="none"/><circle class="particle" cx="4.6" cy="16" r="1.25" fill="currentColor" stroke="none"/></svg>',
    camera: '<svg viewBox="0 0 24 24"><path d="M3.5 8.5h4l1.5-2.5h6L16.5 8.5h4v11h-17z"/><circle cx="12" cy="13.5" r="3.4"/><g class="glint"><path d="M9.8 11.4 11 10.4"/></g></svg>',
    wait: '<svg viewBox="0 0 24 24"><circle class="waitdot" cx="6" cy="12" r="1.7" fill="currentColor" stroke="none"/><circle class="waitdot" cx="12" cy="12" r="1.7" fill="currentColor" stroke="none"/><circle class="waitdot" cx="18" cy="12" r="1.7" fill="currentColor" stroke="none"/></svg>'
  };

  const ACTIVITY_GLYPH = {
    "Ghostlight": "scan",
    "Navigating": "navigate",
    "Clicking": "pointer",
    "Hovering": "pointer",
    "Dragging": "pointer",
    "Typing": "keyboard",
    "Keyboard": "keyboard",
    "Scrolling": "navigate",
    "Reading page": "scan",
    "Finding on page": "scan",
    "Screenshot": "camera",
    "Zooming": "scan",
    "Filling form": "keyboard",
    "Uploading file": "workwheel",
    "Running JavaScript": "workwheel",
    "Waiting": "wait",
    "Browser dialog": "wait"
  };

  const CAPABILITY_CLASS = {
    read: "cap-read",
    action: "cap-action",
    write: "cap-write",
    execute: "cap-execute"
  };

  /* ------------------------------ formatting ------------------------------ */

  function escapeHtml(value) {
    return String(value ?? "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&#039;");
  }

  function words(value) {
    return CAPABILITY_WORDS[value] ?? String(value ?? "").replaceAll("_", " ");
  }

  /* --------------------------- the policy grammar -------------------------- */

  /** Canonical capability order, matching the orchestrator's own. */
  const CAPABILITY_ORDER = ["read", "action", "write", "execute"];

  /**
   * What each capability does, in the words a person would use.
   *
   * The policy vocabulary is the orchestrator's; these are the labels the editor puts on a
   * checkbox, so nobody has to learn what "action" means as a policy word before using the editor.
   */
  const CAPABILITY_WORDS = {
    read: "look at pages",
    action: "click and type",
    write: "fill in forms",
    execute: "run page code"
  };

  /**
   * How each compiled capability state reads on the board, and how it is coloured.
   *
   * Two of these are the same shape of answer pointing opposite ways: an open baseline with sites
   * blocked, and a closed one with sites allowed. Naming them the same thing would hide the only
   * part a person needs in order to act.
   */
  const CAPABILITY_BADGE = {
    available: "Allowed",
    some_blocked: "Some sites blocked",
    some_allowed: "Some sites allowed",
    unavailable: "Not allowed"
  };

  const CAPABILITY_TONE = {
    available: "ok",
    some_blocked: "most",
    some_allowed: "some",
    unavailable: "no"
  };

  /**
   * The settings a policy may author, grouped and worded the way a person actually thinks about
   * them -- not the way the schema stores them.
   *
   * Boolean entries are restrictions: the registered key is only ever authored as `false`, because
   * a user layer cannot hand back authority a higher one removed. But "a grid of things to turn
   * off" is the wrong mental model to hand a person. What a person actually has is a small number
   * of permissions that are on by default, plus one closed browser-startup choice, grouped by what
   * they are about.
   *
   * So the surface owns a second vocabulary here, built for reading rather than for storage: each
   * A boolean item is `on` (checked, the default) and `off` (what unchecking it does), and the
   * toggle is the permission itself, not a restriction wearing a costume. The startup item keeps
   * its two schema values behind direct human choices. `setPermission` and `setChoice` in view.js
   * are the seams that translate those controls back into the closed schema.
   */
  const SETTING_GROUPS = [
    { title: "History", items: [{
      key: "audit.availability", name: "When history cannot be saved", kind: "choice",
      authority: "audit", field: "mode", source: "source", default: "keep_working",
      choices: [
        { value: "keep_working", label: "Keep working", detail: "Continue browser work and show which history could not be saved." },
        { value: "require_audit", label: "Require audit", detail: "Stop new browser work during a known saving failure. Check storage automatically." }
      ]
    }] },
    {
      title: "In the browser",
      items: [
        {
          key: "browser.tabs.allow_close",
          name: "Closing tabs",
          on: "Agents may close a tab they control.",
          off: "Closing a tab stays something only you do."
        },
        {
          key: "browser.screenshots.enabled",
          name: "Screenshots",
          on: "Agents may capture complete images, including embedded content.",
          off: "Agents cannot take screenshots."
        },
        {
          key: "browser.startup",
          kind: "choice",
          name: "Auto-open browser on request",
          choices: [
            {
              value: "on_demand",
              label: "On",
              detail: "Ghostlight may make one bounded attempt to start the browser you normally use."
            },
            {
              value: "manual",
              label: "Off",
              detail: "Ghostlight tells the model which eligible browser windows to ask you to open."
            }
          ]
        }
      ]
    },
    {
      title: "Privacy",
      items: [
        {
          key: "privacy.preserve_target_names",
          name: "Page-authored names",
          on: "Names a page chose for its own elements appear in results and the audit record.",
          off: "Page-authored names are kept out of results and the audit record."
        }
      ]
    }
  ];

  /** Registered key for the never-touch destination list, which is a list rather than a switch. */
  const SACRED_KEY = "content.security.sacred_domains";

  /** Every setting item, in one flat list, for lookups that do not care about grouping. */
  const SETTING_ITEMS = SETTING_GROUPS.flatMap((group) => group.items);

  /** How a view that may not name the never-touch sites says how many there are. */
  function neverTouchedCount(count) {
    return count === 1 ? "1 site is never touched." : `${count} sites are never touched.`;
  }

  /** How a view that may not name the sites a document left out says how many there were. */
  function leftOutCount(count) {
    return count === 1 ? "1 site was left out." : `${count} sites were left out.`;
  }

  /** How one authored setting reads once it is in force, for the read-only summary. */
  function settingWords(key, value) {
    if (key === SACRED_KEY) {
      const hosts = Array.isArray(value) ? value : [];
      return `Never touched: ${hosts.join(", ")}`;
    }
    const known = SETTING_ITEMS.find((item) => item.key === key);
    if (known?.kind === "choice") {
      const choice = known.choices.find((entry) => entry.value === value);
      return choice ? `${known.name}: ${choice.label}` : `${known.name}: ${JSON.stringify(value)}`;
    }
    if (known) return value === false ? known.off : `${known.on} (not restricted)`;
    return `${key} = ${JSON.stringify(value)}`;
  }

  /**
   * Whether a host pattern is one Ghostlight accepts.
   *
   * Mirrors the production matcher exactly: `*`, an exact hostname, or one leading `*.` suffix.
   * The editor checks here so a person learns about a bad pattern while typing rather than when
   * they press Apply, but the orchestrator still validates every document it is handed.
   */
  function validHostPattern(pattern) {
    const value = String(pattern ?? "");
    if (value === "*") return true;
    const host = value.startsWith("*.") ? value.slice(2) : value;
    if (!host || host.length > 253) return false;
    return host.split(".").every((label) =>
      label.length > 0 && label.length <= 63
      && !label.startsWith("-") && !label.endsWith("-")
      && /^[A-Za-z0-9-]+$/.test(label));
  }

  /**
   * Say in plain words what a pattern matches.
   *
   * Ghostlight's suffix wildcard covers subdomains only, which is its own choice and differs from
   * what a person may expect from other products. Stating the result removes the need to know.
   */
  function hostReadback(pattern) {
    const value = String(pattern ?? "").trim();
    if (!value) return "";
    if (!validHostPattern(value)) return `${value} (not a site Ghostlight can match)`;
    if (value === "*") return "any website";
    if (value.startsWith("*.")) {
      const suffix = value.slice(2);
      return `anything under ${suffix}, but not ${suffix} itself`;
    }
    return `${value} exactly, and none of its subdomains`;
  }

  /** Whether every host matched by `narrow` is also matched by `broad`. */
  function patternCovers(broad, narrow) {
    if (broad === "*") return true;
    if (String(broad).toLowerCase() === String(narrow).toLowerCase()) return true;
    if (!String(broad).startsWith("*.")) return false;
    const suffix = String(broad).slice(2).toLowerCase();
    const candidate = String(narrow).startsWith("*.")
      ? String(narrow).slice(2).toLowerCase()
      : String(narrow).toLowerCase();
    return candidate.length > suffix.length && candidate.endsWith(`.${suffix}`);
  }

  function duration(ms) {
    if (!Number.isFinite(ms) || ms < 0) return "";
    const seconds = ms / 1000;
    if (seconds < 60) return `${seconds.toFixed(1)}s`;
    const minutes = Math.floor(seconds / 60);
    return `${minutes}m ${String(Math.floor(seconds % 60)).padStart(2, "0")}s`;
  }

  function stopwatch(ms) {
    const seconds = Math.max(0, ms) / 1000;
    if (seconds < 60) return seconds.toFixed(1).padStart(4, "0");
    const minutes = Math.floor(seconds / 60);
    return `${minutes}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;
  }

  function ago(timestamp) {
    if (!timestamp) return "";
    const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1000));
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h`;
    return `${Math.floor(hours / 24)}d`;
  }

  function shortId(value) {
    const text = String(value ?? "");
    return text.length <= 20 ? text : `${text.slice(0, 10)}...${text.slice(-6)}`;
  }

  /**
   * A settled record has no presentation activity, so its medallion comes from the tool.
   * Without this every historical row wears the same glyph and the queue reads as one texture.
   */
  const TOOL_GLYPH = {
    browser_tabs: "navigate",
    browser_navigate: "navigate",
    browser_history: "navigate",
    browser_window: "scan",
    browser_read: "scan",
    browser_inspect: "scan",
    browser_find: "scan",
    browser_screenshot: "camera",
    browser_click: "pointer",
    browser_scroll: "navigate",
    browser_hover: "pointer",
    browser_fill_form: "keyboard",
    browser_type_text: "keyboard",
    browser_press_key: "keyboard",
    browser_drag: "pointer",
    browser_wait: "wait",
    browser_dialog: "wait",
    browser_upload: "workwheel",
    browser_execute: "workwheel",
    browser_sequence: "workwheel",
    browser_flow: "workwheel",
    browser_record: "camera",
    browser_diagnose: "scan",
    browser_workspace: "scan",
    policy_explain: "scan"
  };

  const glyphFor = entry =>
    GLYPHS[ACTIVITY_GLYPH[entry.activity] ?? TOOL_GLYPH[entry.tool] ?? "scan"];
  const capabilityClass = entry => {
    const facts = String(entry.capability ?? "").split(" + ");
    // Consequence colour only. RAWX remains an independent set in authority and audit.
    for (const capability of ["execute", "write", "action", "read"]) {
      if (facts.includes(capability)) return CAPABILITY_CLASS[capability];
    }
    return "cap-read";
  };

  /** What the audit can honestly say happened to the page, with no payload to draw on. */
  const EFFECT_STORY = {
    none: "left the page unchanged",
    applied: "changed the page",
    partial: "changed the page in part",
    unknown: "outcome could not be confirmed"
  };

  const READINESS_NOTE = {
    not_applicable: "",
    complete: "",
    interactive: "interactive",
    loading: "never settled",
    unknown: "readiness unknown"
  };

  /**
   * Which readiness values mark a settled row as worth finding while scrolling.
   *
   * A document that never settled, or whose readiness could not be observed, is a caution about
   * the sentence beside it; "interactive" is only extra information. The duration cell carries
   * the amber treatment so the row is found by eye instead of read for.
   */
  const READINESS_ATTENTION = new Set(["loading", "unknown"]);

  /** Whether a settled entry's observation deserves the attention treatment. */
  function readinessNeedsAttention(entry) {
    return Boolean(entry.settled && entry.observed
      && READINESS_ATTENTION.has(entry.observed.readiness));
  }

  /**
   * Where the About page will send you, in the orchestrator's own closed vocabulary.
   *
   * Each row names a destination rather than an address: the surface has no URLs in it at all, so
   * this list cannot grow a link the product did not choose. The sentences are the point -- a bare
   * list of names makes a reader guess, and a guess is a worse experience than a plain sentence.
   */
  const DESTINATIONS = [
    ["Ghostlight", [
      ["home", "Project page", "What it is, who it is for, and how it behaves."],
      ["demo", "Watch a run", "A recorded browser job you can follow end to end."],
      ["decision_aid", "Which mode fits you", "Ungoverned, governed, or somewhere between."],
      ["source", "Source", "Every line of the engine, and the governance module beside it."]
    ]],
    ["Get it working", [
      ["install_guide", "Install and connect", "Set up the browser side and point a client at it."],
      ["scripting_guide", "Drive it from a script", "The command line reaches the same catalog a client does."],
      ["governance_guide", "Write a policy", "Hosts, capabilities, and the runtime controls around them."],
      ["audit_guide", "Ship the audit", "The record's exact shape, and what it deliberately omits."]
    ]],
    ["Read the reasoning", [
      ["trust_center", "What it promises", "Each public claim, and where the code keeps it."],
      ["decision_records", "Why it is built this way", "Every architectural decision, kept as written."],
      ["licensing_guide", "What you may do with it", "The whole product is Apache-2.0 OR MIT."],
      ["sylin_tools", "The rest of the toolkit", "The other Sylin tools this one grew up beside."]
    ]]
  ];

  return Object.freeze({ FEED_LIMIT, WORKING_LATCH_MS, VIEWS, SEARCH_VIEWS,
    DEFAULT_VIEW, DEFAULT_SECTION, ADDRESSES, parseAddress, addressOf,
    POLICY_CHANGED_ELSEWHERE, CONTINUE_LABEL, APPLY_IN_WINDOW_LABEL, COPIED_SETUP, COPIED_COMMAND, RECONNECTING,
    WATCHING_TURNED_ON, WATCHING_TURNED_OFF, TEST_NOTIFICATION_SENT, ADDRESS_COPIED, SETTINGS, SETTINGS_SECTIONS,
    screenState, byteSize,
    neverTouchedCount, leftOutCount,
    INTEGRATION_CATEGORIES, INTEGRATION_STATE_CATEGORY, INTEGRATION_CATEGORY_PRIORITY,
    GLYPHS, ACTIVITY_GLYPH, CAPABILITY_CLASS, TOOL_GLYPH, EFFECT_STORY, READINESS_NOTE,
    READINESS_ATTENTION, readinessNeedsAttention,
    DESTINATIONS, glyphFor, capabilityClass, CAPABILITY_ORDER, CAPABILITY_WORDS,
    CAPABILITY_BADGE, CAPABILITY_TONE, SETTING_GROUPS, SACRED_KEY, settingWords,
    validHostPattern, hostReadback, patternCovers,
    escapeHtml, words, duration, stopwatch, ago, shortId });
});
