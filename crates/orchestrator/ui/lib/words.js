// Ghostlight workbench -- the fixed vocabulary this window speaks, and how it renders a number.
(function installGhostlightWords(root, factory) {
  const api = factory();
  root.GhostlightWords = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightWords() {
  "use strict";

  /** Single channel the orchestrator publishes sequenced changes on. */
  const CHANGE_EVENT = "ghostlight://change";
  /** Slow safety pull for collections that have no change event of their own. */
  const HEARTBEAT_MS = 10000;
  /** Bound on the retained feed, matching the orchestrator's own bounded history. */
  const FEED_LIMIT = 200;

  /** Destinations this surface renders, keyed by the orchestrator's search vocabulary. */
  const VIEWS = { monitor: "At a glance", integrations: "MCP integrations", status: "Status", policy: "Policy", about: "About" };

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
    {
      title: "Where agents may connect",
      items: [
        {
          key: "channels.mcp.enabled",
          name: "MCP clients",
          on: "Claude Code, Codex, Cursor, and other MCP-compatible tools may open a session here.",
          off: "No MCP client may open a session here.",
          link: { view: "integrations", label: "See connected clients" }
        },
        {
          key: "channels.cli.enabled",
          name: "Command line",
          on: "ghostlight call may open a session, for scripts and automation.",
          off: "The command line may not open a session here.",
          link: { destination: "scripting_guide", label: "Scripting examples" }
        }
      ]
    },
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
          key: "browser.startup",
          kind: "choice",
          name: "When no browser is connected",
          choices: [
            {
              value: "on_demand",
              label: "Start my browser when needed",
              detail: "Ghostlight may make one bounded attempt to start the browser you normally use."
            },
            {
              value: "manual",
              label: "I will start it myself",
              detail: "Ghostlight names the missing browser and waits for you to start it."
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

  /** The one setting only an organization may author, named so its display is not a raw key. */
  const ORGANIZATION_SETTINGS = {
    "policy.user.enabled": "You may not set your own rules on this machine"
  };

  /** Registered key for the never-touch destination list, which is a list rather than a switch. */
  const SACRED_KEY = "content.security.sacred_domains";

  /** Every setting item, in one flat list, for lookups that do not care about grouping. */
  const SETTING_ITEMS = SETTING_GROUPS.flatMap((group) => group.items);

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
    if (ORGANIZATION_SETTINGS[key]) {
      return value === false ? ORGANIZATION_SETTINGS[key] : `${ORGANIZATION_SETTINGS[key]} (allowed)`;
    }
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

  return Object.freeze({ CHANGE_EVENT, HEARTBEAT_MS, FEED_LIMIT, WORKING_LATCH_MS, VIEWS, SEARCH_VIEWS,
    INTEGRATION_CATEGORIES, INTEGRATION_STATE_CATEGORY, INTEGRATION_CATEGORY_PRIORITY,
    GLYPHS, ACTIVITY_GLYPH, CAPABILITY_CLASS, TOOL_GLYPH, EFFECT_STORY, READINESS_NOTE,
    READINESS_ATTENTION, readinessNeedsAttention,
    DESTINATIONS, glyphFor, capabilityClass, CAPABILITY_ORDER, CAPABILITY_WORDS,
    CAPABILITY_BADGE, CAPABILITY_TONE, SETTING_GROUPS, SACRED_KEY, settingWords,
    validHostPattern, hostReadback, patternCovers,
    escapeHtml, words, duration, stopwatch, ago, shortId });
});
