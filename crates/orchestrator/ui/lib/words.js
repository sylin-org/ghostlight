// Ghostlight workbench -- the fixed vocabulary this window speaks, and how it renders a number.
(function installGhostlightWords(root, factory) {
  const api = factory();
  root.GhostlightWords = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightWords() {
  "use strict";

  /** Bound on the retained feed: the running card and the rows At a glance lists under it. */
  const FEED_LIMIT = 200;

  /**
   * How many recorded actions the service holds and searches (`HISTORY_LIMIT` in the service's
   * workbench), so the count the band and the end of the list say stays the service's as work
   * settles.
   */
  const HISTORY_HELD = 500;

  /** Said under the rule editor when the applied rules changed elsewhere while edits were unsaved. */
  const POLICY_CHANGED_ELSEWHERE = "The applied rules changed elsewhere. Discard your edits to see them.";

  /** The button that brings the Ghostlight application forward, where a change can be made. */
  const CONTINUE_LABEL = "Continue in the Ghostlight window";

  /** The one line a watching page's Policy shows above its read-only rules and settings. */
  const POLICY_IN_WINDOW = "Changes are made in the Ghostlight window.";

  /** Said after Copy setup puts an agent's entry on the clipboard. */
  const COPIED_SETUP = "Manual setup copied.";

  /** Said after Copy MCP command puts the command every agent runs on the clipboard. */
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

  /** Said after an agent's card copies the id of one of its connections. */
  const CONNECTION_ID_COPIED = "Connection id copied.";

  /**
   * Said in place of an exception's own text when part of this window could not be drawn. The
   * exception goes to the console, where a person debugging the window reads it.
   */
  const WINDOW_FAULT = "Part of this window could not be drawn. Reopening the window draws it again.";

  /** Said when a request to the service failed without the service saying why. */
  const NO_ANSWER = "Ghostlight did not answer. Try again in a moment.";

  /** Said when this screen's clipboard would not take what a Copy button put on it. */
  const COPY_FAILED = "This screen could not copy it. Select the text and copy it by hand.";

  /** What a destination says in place of its content when it could not be read, and its button. */
  const LOAD_FAILED = Object.freeze({
    policy: "Ghostlight could not read the policy.",
    status: "Ghostlight could not read its status.",
    integrations: "Ghostlight could not look for agents."
  });

  /** The button beside a destination that could not be read. */
  const TRY_AGAIN = "Try again";

  /**
   * The word a settled action that did not succeed carries on the running card and in what a screen
   * reader hears for its row, by the outcome it had.
   */
  const OUTCOME_WORD = Object.freeze({ failed: "Failed", unknown: "Unknown", kept: "Kept", blocked: "Blocked" });

  /** The phases of work that has not settled, in the service's wire words (workbench `OperationPhase`). */
  const PHASE = Object.freeze({ waiting: "waiting", running: "running", held: "held" });

  /**
   * The word unsettled work carries on the running card, and pending work in its row's duration
   * cell, by its phase. The band counts each phase under the same word, in lower case.
   */
  const PHASE_WORD = Object.freeze({ [PHASE.running]: "Running", [PHASE.waiting]: "Waiting", [PHASE.held]: "Held" });

  /**
   * What the band counts of the work in flight, as [count, word] pairs: what runs, then what waits
   * and what a pause holds, each only when there is some. While paused with nothing running, the
   * held work is the whole of it, so the band says "2 held" rather than "0 running, 2 held".
   */
  function workCounts({ running = 0, waiting = 0, held = 0, paused = false }) {
    const counts = [];
    if (running || !paused || !held) counts.push([running, PHASE.running]);
    if (waiting) counts.push([waiting, PHASE.waiting]);
    if (held) counts.push([held, PHASE.held]);
    return counts;
  }

  /**
   * The band's facts, each a whole that shows or gives way as one, in the order they give way from
   * the end: what has been recorded first, then the work in flight. Each is a kind and its
   * [count, word] parts; the work is what workCounts says.
   */
  const BAND_FACT = Object.freeze({ agents: "agents", browsers: "browsers", work: "work", recorded: "recorded" });

  /** The band's facts in order, as [kind, parts] pairs, from the counts the band shows. */
  function bandFacts({ agents, browsers, work, recorded }) {
    return [
      [BAND_FACT.agents, [[agents, noun(agents, "agent", "agents")]]],
      [BAND_FACT.browsers, [[browsers, noun(browsers, "browser", "browsers")]]],
      [BAND_FACT.work, work],
      [BAND_FACT.recorded, [[recorded, "recorded"]]]
    ];
  }

  /**
   * Who drove an action and how long ago it settled, as one phrase: "Claude Code, 4m" on the
   * narrow row's second line, "Claude Code, 4m ago" for a screen reader. A part it lacks is left out.
   */
  const rowBy = (...parts) => parts.filter(Boolean).join(", ");

  /** The eyebrow of a running card that has rested: nothing has run for a while. */
  const LAST_ACTION = "Last action";

  /**
   * The one sentence a screen reader hears when an action settles: the tool, the outcome word when
   * it did not succeed, and the row's own sentence, as in "navigate: Opened example.com.".
   */
  function actionSaid(tool, sentence, outcome = "") {
    const name = toolWords(tool);
    return outcome ? `${name}, ${outcome.toLowerCase()}: ${sentence}` : `${name}: ${sentence}`;
  }

  /** The intake that admitted a session, in the service's wire words. */
  const INTAKE = Object.freeze({ mcp: "mcp", cli: "cli" });

  /**
   * The intakes the activity list marks, and the words of each mark. Only the exception is marked:
   * nearly every action arrives over MCP, so an MCP mark on every row would say nothing.
   */
  const INTAKE_MARK = Object.freeze({ [INTAKE.cli]: "Command line" });

  /** The start every browser tool's name shares, which the activity list leaves out. */
  const SHARED_TOOL_PREFIX = "browser_";

  /**
   * A catalog tool as the activity list names it: the shared start left out and the rest in words,
   * so "browser_fill_form" reads "fill form" and "policy_explain" reads "policy explain". The
   * exact name stays one hover away, and everything an agent or a machine reads keeps it.
   */
  function toolWords(tool) {
    const name = String(tool ?? "");
    const rest = name.startsWith(SHARED_TOOL_PREFIX) ? name.slice(SHARED_TOOL_PREFIX.length) : name;
    return rest.replaceAll("_", " ");
  }

  /** The fixed words of the At a glance summary, its shelves and an agent's card. */
  const GLANCE = Object.freeze({
    details: "Details",
    hide: "Hide",
    agents: "Agents",
    browsers: "Browsers",
    noAgent: "No agent connected",
    copyId: "Copy id",
    working: "working",
    idle: "idle",
    fromCommandLine: "from the command line"
  });

  /**
   * The fixed words of an action's detail card. The connection says what Ghostlight can vouch for
   * in the service's words, and the intake only when it is the exception, the command line.
   */
  const CARD = Object.freeze({
    connection: "Connection details",
    notRecorded: "Connection details were not recorded.",
    commandLine: "Came through the command line.",
    permissions: "Permission details",
    permissionsOmitted: "Additional permission checks were omitted."
  });

  /** The hover of a browser's chip: the extension by its product name, and its version. */
  const browserChipTitle = (version) => `Ghostlight in Browser ${version}`;

  /** How many session lines an agent's card shows before it says how many more there are. */
  const GLANCE_CARD_LINES = 4;

  /** `count` of a thing, with the singular for one and `none` for zero. */
  function counted(count, one, many, none) {
    if (count === 0) return none;
    return count === 1 ? `1 ${one}` : `${count} ${many}`;
  }

  /** The noun after a count: `one` for exactly one, otherwise `many` ("1 session", "0 sessions"). */
  const noun = (count, one, many) => (count === 1 ? one : many);

  /** How many agents are connected, as the summary says it. */
  const agentsCount = (count) => counted(count, "agent", "agents", "No agents");

  /** How many browsers are connected, as the summary says it. */
  const browsersCount = (count) => counted(count, "browser", "browsers", "no browser");

  /** How many tabs a browser holds for the sessions working in it. */
  const tabsCount = (count) => counted(count, "tab", "tabs", "no tabs");

  /** How many sessions an agent holds, on its card. */
  const sessionsCount = (count) => counted(count, "session", "sessions", "no sessions");

  /** The line under a card's capped list of sessions. */
  const andMore = (count) => `and ${count} more`;

  /**
   * One session of an agent, in words: its tabs and the browser it works in, whether it is working
   * (running work now; waiting or held work is not), and the intake only when it is the command
   * line.
   */
  function sessionLine(session) {
    const tabs = session.tab_count ?? 0;
    const browser = session.browser ?? "";
    let where = tabsCount(tabs);
    if (tabs > 0 && browser) where = `${where} in ${browser}`;
    else if (browser) where = `In ${browser}, ${where}`;
    else where = where.charAt(0).toUpperCase() + where.slice(1);
    const parts = [where, session.running_operations > 0 ? GLANCE.working : GLANCE.idle];
    if (session.channel === INTAKE.cli) parts.push(GLANCE.fromCommandLine);
    return parts.join(", ");
  }

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
    textSize: "Text size",
    standardText: "Standard",
    largeText: "Large",
    motion: "Motion",
    followSystem: "Follow the system",
    reduced: "Reduced",
    pause: "Pause agents",
    resume: "Resume agents",
    startSession: "Start a new session",
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
  function screenState(density, text, motion) {
    const dense = density === "compact" ? SETTINGS.compact : SETTINGS.comfortable;
    const size = text === "large" ? "large text" : "standard text";
    const moving = motion === "reduced" ? "motion reduced" : "motion follows the system";
    return `${dense}, ${size}, ${moving}`;
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
    monitor: "At a glance", integrations: "Integrations", status: "Status", policy: "Policy",
    settings: "Settings", about: "About"
  };

  /** The destination a page opens on, and the section of Settings it opens on. */
  const DEFAULT_VIEW = "monitor";
  const DEFAULT_SECTION = "screen";

  /**
   * Every address a tab, a rail entry or a typed hash can name, and no other.
   *
   * Search destinations are not addresses: they are the service's words for what a hit is about,
   * and SEARCH_VIEWS turns them into a view. The setup view is the one address that is not a
   * destination.
   */
  const ADDRESSES = Object.freeze([
    "monitor", "integrations", "status", "policy", "settings", "settings/screen", "settings/sharing",
    "settings/browser-work", "settings/notifications", "settings/diagnostics", "about", "setup"
  ]);

  /**
   * The address of the setup view. It is an address but not a tab: while setup is under way the
   * view stands in for the tabs, and the service opens the window at it.
   */
  const SETUP_ADDRESS = "setup";

  /**
   * The words of the setup view that belong to this page: its buttons that are not the service's
   * rows. The service says everything else, in the setup part.
   */
  const SETUP = Object.freeze({ copy: "Copy", copied: "Copied", skip: "Skip for now" });

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
    settings: "settings",
    install: "integrations"
  };

  /** What each kind of search hit is, on the chip at its left, keyed by the service's words. */
  const HIT_KIND = Object.freeze({
    destination: "Page", operation: "Running", session: "Session", history: "Action",
    browser: "Browser", diagnostic: "Check", configuration: "Setting", installation: "Agent"
  });

  /** What the search palette says when nothing matches. */
  const SEARCH_FOUND_NOTHING = "No matching Ghostlight records.";

  /** The line that ends a list too long to show whole: every recorded action is a search away. */
  const searchAll = (count) => `Search all ${count} (Ctrl+K)`;

  /* ----------------------------- Integrations ----------------------------- */

  /**
   * The tone of an agent in use whose session is running work now. It is the only agent tone that
   * wears the accent, and it names the classes that draw it (`d-live`, `t-live`, `s-live`).
   */
  const WORKING_TONE = "live";

  /**
   * What each place an agent stands reads as, on the rail and in the pane: the service's states,
   * and `by_hand` for an agent found here whose file Ghostlight never reads (see agentStanding).
   */
  const AGENT_STATE_WORDS = Object.freeze({
    installed: "Connected",
    not_detected: "Not on this computer",
    available: "Found, not connected",
    by_hand: "Found, set up by hand",
    updatable: "Needs an update",
    needs_attention: "Needs your attention"
  });

  /**
   * The word for an agent's tone: "Working" for WORKING_TONE, a connected agent running work now,
   * so working and connected differ in words and not only in colour, and otherwise its standing's.
   */
  const agentStateWord = (tone) => (tone === WORKING_TONE ? "Working" : AGENT_STATE_WORDS[tone] ?? "");

  /** The standing, and the rail group, of a copy-only agent found here that is not in use. */
  const BY_HAND = "by_hand";

  /**
   * Where an agent stands, from the service's state and whether the service says it is in use.
   *
   * Ghostlight never reads a copy-only agent's file, so the service cannot say whether its entry is
   * there. Such an agent counts as connected while the service says it is in use (one of its
   * sessions is live), and one found here that is not stands `by_hand`: neither connected nor not
   * connected, because nothing observed says which. Every other agent stands where the service's
   * state puts it.
   */
  function agentStanding(agent, inUse) {
    if (!agent.copy_only) return agent.state;
    if (inUse) return "installed";
    return agent.state === "available" ? BY_HAND : agent.state;
  }

  /**
   * The rail's groups, in rail order, each with the standings it holds. A group with no agent is not
   * drawn, so Needs a look appears only when something does.
   */
  const AGENT_GROUPS = Object.freeze([
    Object.freeze({ key: "look", title: "Needs a look", states: Object.freeze(["needs_attention", "updatable"]) }),
    Object.freeze({ key: "connected", title: AGENT_STATE_WORDS.installed, states: Object.freeze(["installed"]) }),
    Object.freeze({ key: "found", title: AGENT_STATE_WORDS.available, states: Object.freeze(["available"]) }),
    Object.freeze({ key: BY_HAND, title: AGENT_STATE_WORDS[BY_HAND], states: Object.freeze([BY_HAND]) }),
    Object.freeze({ key: "absent", title: AGENT_STATE_WORDS.not_detected, states: Object.freeze(["not_detected"]) })
  ]);

  /** The key of the group that folds into one rail row. */
  const ABSENT_GROUP = "absent";

  /** The label on each button the pane offers, by the act it asks for. */
  const AGENT_ACTS = Object.freeze({
    install: "Install", prepare: "Prepare", connect: "Connect", update: "Update", fix: "Fix",
    locate: "Locate", copy: "Copy setup", recheck: "Re-check"
  });

  /** The fixed words of the Integrations page. */
  const AGENTS = Object.freeze({
    copyOnly: "Copy setup only",
    working: "Working...",
    evidence: "What Ghostlight found",
    settingsFile: "Settings file",
    entry: "Ghostlight's entry",
    factsNote: "Every agent finds Ghostlight through one entry like this in its own settings file, "
      + "and starts Ghostlight by itself when it needs your browser.",
    showSessions: "See them in At a glance",
    fixSteps: "How to fix it",
    copyStep: "Replace what Ghostlight found with the entry below",
    repairStep: "Correct the file yourself, so Ghostlight can read it",
    recheckStep: "Check again",
    noAgents: "No supported agent was found for this user.",
    rechecked: "Looked for agents again."
  });

  /** A list of names in a sentence: "A", "A and B", "A, B and C". */
  function namesList(names) {
    if (names.length <= 1) return names.join("");
    return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;
  }

  /** How many things, with the plural for anything but one. */
  const plural = (count, one, many) => `${count} ${count === 1 ? one : many}`;

  /** What the rail says when a search finds nothing. */
  const noAgentCalled = (query) => `No agent called "${query}".`;

  /** The words beside the count on the row the agents not on this computer fold into. */
  const moreAgents = (count) => (count === 1 ? "more agent Ghostlight can join" : "more agents Ghostlight can join");

  /**
   * The page's headline: how many of the agents found on this computer have Ghostlight, as a lead,
   * the emphasised part and a trail. Each agent carries its `standing`. An agent set up by hand is
   * left out of the count, since Ghostlight cannot tell whether it has the entry, and the headline
   * then says it counts only the agents Ghostlight can check.
   */
  function agentHeadline(agents) {
    const unseen = agents.filter((agent) => agent.standing === BY_HAND);
    const here = agents.filter((agent) => !["not_detected", BY_HAND].includes(agent.standing));
    const on = agents.filter((agent) => agent.standing === "installed");
    if (!here.length) {
      if (!unseen.length) return { lead: "Add Ghostlight to ", accent: "your agent", trail: "." };
      return unseen.length === 1
        ? { lead: "Ghostlight cannot check ", accent: unseen[0].label, trail: ", which is set up by hand." }
        : { lead: "Ghostlight cannot check ", accent: "your agents", trail: ", which are set up by hand." };
    }
    const kind = unseen.length ? " agents it can check" : " agents";
    const among = (count) => (unseen.length ? `of the ${count}` : `of your ${count}`);
    if (on.length === here.length) {
      if (on.length === 1) return { lead: "Ghostlight is in ", accent: on[0].label, trail: "." };
      return { lead: "Ghostlight is in ", accent: `all ${on.length}`, trail: unseen.length ? `${kind}.` : " of your agents." };
    }
    if (!on.length) {
      return here.length === 1
        ? { lead: "Ghostlight is not in ", accent: here[0].label, trail: " yet." }
        : { lead: "Ghostlight is in ", accent: "none", trail: ` ${among(here.length)}${kind} yet.` };
    }
    return { lead: "Ghostlight is in ", accent: `${on.length} ${among(here.length)}`, trail: `${kind}.` };
  }

  /** A short list of agents by name, or how many when there are more than three. */
  const fewOrCount = (names) => (names.length <= 3 ? namesList(names) : `${names.length} agents`);

  /**
   * What Set up everything will do, said before it is pressed: the agents it connects, the ones it
   * updates, and the ones that need the person ("Connects 5 agents and updates Cursor. Goose needs
   * you."). Empty when it would do nothing, since the button is not shown then.
   */
  function agentPlan({ connect, update, left }) {
    if (!connect.length && !update.length) return "";
    const parts = [];
    if (connect.length) parts.push(`connects ${fewOrCount(connect)}`);
    if (update.length) parts.push(`updates ${fewOrCount(update)}`);
    const said = parts.join(" and ");
    const need = left.length ? ` ${fewOrCount(left)} ${left.length === 1 ? "needs" : "need"} you.` : "";
    return `${said[0].toUpperCase()}${said.slice(1)}.${need}`;
  }

  /** An agent's sessions and tabs on its rail line, and whether one is working: "3 sessions, 1 tab, working". */
  function agentUse({ sessions, tabs, working }) {
    const said = [plural(sessions, "session", "sessions")];
    if (tabs) said.push(plural(tabs, "tab", "tabs"));
    if (working) said.push(GLANCE.working);
    return said.join(", ");
  }

  /** The pane's line for an agent in use now. */
  function agentLive(name, { sessions, tabs }) {
    const many = sessions === 1 ? `${name} session is` : `${name} sessions are`;
    const open = tabs ? `, with ${plural(tabs, "browser tab", "browser tabs")} open` : "";
    return `${sessions} ${many} connected right now${open}.`;
  }

  /**
   * The one sentence the pane says about an agent, from where it stands and the file it reads. A
   * blocked agent with no evidence says the service's own sentence, which names what Ghostlight
   * saw. A copy-only agent's sentence claims nothing about a file Ghostlight never reads.
   */
  function agentLead(agent, { prepare }) {
    const { name, file } = agent;
    switch (agent.standing) {
      case "installed":
        return agent.copy_only
          ? `${name} is using Ghostlight right now. Ghostlight never reads ${name}'s ${file}, so it knows ${name} has its entry only while ${name} is connected.`
          : `Ghostlight's entry is in ${name}'s ${file}, so ${name} can use your browser.`;
      case BY_HAND:
        return `${name} is on this computer. Ghostlight never reads ${name}'s ${file}, so it cannot tell whether ${name} has Ghostlight's entry until ${name} connects.`;
      case "not_detected":
        return prepare
          ? `${name} is not installed on this computer. You can install it, or prepare its settings first.`
          : `${name} is not installed on this computer.`;
      case "available":
        return `${name} is installed on this computer but does not know about Ghostlight yet.`;
      case "updatable":
        return `${name} has Ghostlight's entry, but an older Ghostlight wrote it. Update brings it to the current one.`;
      case "needs_attention":
        return agent.evidence
          ? `Something Ghostlight did not write sits where its entry belongs in ${name}'s ${file}, and Ghostlight left it as it is.`
          : agent.detail;
      default:
        return agent.detail;
    }
  }

  /** What one button of the pane does, written beside it. */
  function agentWhy(act, agent) {
    const { name, file } = agent;
    switch (act) {
      case "install": return `Opens the official ${name} download page in your browser. You install it from there.`;
      case "prepare": return `Writes Ghostlight's entry into ${file} now, so ${name} finds Ghostlight the first time it starts.`;
      case "connect": return `Adds Ghostlight's entry to ${file}. The rest of the file stays as it is. Restart ${name} afterwards.`;
      case "update": return `Rewrites Ghostlight's own entry in ${file}. Nothing else in the file changes. Restart ${name} afterwards.`;
      case "fix": return `Backs up ${file}, then puts Ghostlight's entry in place of what is there. Nothing else in the file changes. Asks first.`;
      case "locate": return `Lets you choose ${name}'s settings file or program yourself, if Ghostlight looks in the wrong place.`;
      case "repair": return `Ghostlight leaves ${file} as it is.`;
      case "recheck":
        return agent.evidence
          ? `Once the entry is right, ${name} moves to ${AGENT_STATE_WORDS.installed}.`
          : `Ghostlight reads ${file} again and says what it finds.`;
      case "copy":
        if (agent.standing === "installed") return `Copies this entry, to set ${name} up by hand on another computer.`;
        if (agent.copy_only) return `Ghostlight never edits ${file}. Copying this entry and pasting it in yourself is the only way to connect ${name}.`;
        if (agent.evidence) return "Copies the entry that belongs there, to paste over what Ghostlight found.";
        return `Copies this entry, to paste into ${file} yourself.`;
      default: return "";
    }
  }

  /** What the settings file is to this agent, under its label. */
  function agentFileLabel(agent) {
    if (agent.copy_only) return agent.standing === "installed" ? "Ghostlight never reads it" : "Where to paste it";
    return ({
      installed: "Ghostlight's entry is in here", not_detected: `Where ${agent.name} will look`,
      available: "Where Connect writes", updatable: "Where Update writes", needs_attention: "Where the problem is"
    })[agent.state] ?? "";
  }

  /**
   * What the pane says under the settings file when it is not simply the agent's usual one: the
   * Microsoft Store edition's own copy, and another copy of the same settings that exists beside
   * it. Empty for settings found only where the agent usually keeps them.
   */
  function agentCopyNote(agent) {
    const store = agent.packaged ? `${agent.name} from the Microsoft Store keeps its settings in this file, in its own folder.` : "";
    const other = agent.other_copy ? `Ghostlight's entry goes in this file. Another copy of these settings is at ${agent.other_copy}.` : "";
    return [store, other].filter(Boolean).join(" ");
  }

  /** What the entry shown is to this agent, under its label. */
  function agentEntryLabel(agent, { prepare }) {
    if (agent.copy_only) return "What to paste";
    return ({
      installed: "As Ghostlight added it", not_detected: prepare ? "What Prepare writes" : "What to paste",
      available: "What Connect adds", updatable: "After the update", needs_attention: "What belongs there"
    })[agent.state] ?? "";
  }

  /** The quiet line beside Remove, and its label. */
  const removeNote = (file) => `Deletes only Ghostlight's entry from ${file}. Asks first.`;
  const removeLabel = (name) => `Remove from ${name}`;

  /** The question Remove asks, naming the sessions that use Ghostlight now. */
  function confirmRemovalWords(name, file, sessions) {
    const live = sessions
      ? ` ${sessions} ${sessions === 1 ? `${name} session is` : `${name} sessions are`} using Ghostlight right now.` : "";
    return {
      title: `Remove Ghostlight from ${name}?`,
      detail: `Ghostlight deletes only its own entry from ${file}; everything else in it stays.${live}`
        + ` You can connect ${name} again from this page at any time.`,
      action: "Remove"
    };
  }

  /** The question Fix asks before it replaces what sits under Ghostlight's key. */
  const confirmFixWords = (name, file) => ({
    title: `Replace what sits in ${name}'s ${file}?`,
    detail: "Only the entry under Ghostlight's key is replaced, and the file is backed up first.",
    action: AGENT_ACTS.fix
  });

  /* --------------------------------------------------------------------------
   * The medallion vocabulary, keyed by the orchestrator's fixed activity labels.
   * The gear, the lens, the keyboard and the waiting lights are the shapes the
   * renderer draws inside the page (protocol4/assets/presentation.js), on this
   * window's 24-unit box, so the window and the browser tell the same story.
   * ----------------------------------------------------------------------- */
  const GLYPHS = {
    scan: '<svg viewBox="0 0 24 24"><rect x="3.5" y="4" width="17" height="16" rx="2.5"/><g class="scanline"><path d="M7 12h10"/></g></svg>',
    navigate: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5"/><path d="M15.5 8.5 10.6 10.6 8.5 15.5l4.9-2.1z"/></svg>',
    pointer: '<svg viewBox="0 0 24 24"><path d="M6 3.5 18.5 12 13 13.2l-2.2 5.6z"/><path d="M13.2 13.4 17.5 18"/></svg>',
    keyboard: '<svg viewBox="0 0 24 24"><rect x="2.5" y="6.5" width="19" height="11" rx="2"/><g class="keylight"><path d="M6.5 10.5h.01"/></g><g class="keylight"><path d="M10.5 10.5h.01"/></g><g class="keylight"><path d="M14.5 10.5h.01"/></g><path d="M8 14h8"/></svg>',
    gear: '<svg viewBox="0 0 24 24"><g class="spin"><circle cx="12" cy="12" r="6"/><circle cx="12" cy="12" r="1.6"/><path d="M12 2v3.5M12 18.5V22M2 12h3.5M18.5 12H22M4.95 4.95l2.5 2.5M16.55 16.55l2.5 2.5M19.05 4.95l-2.5 2.5M7.45 16.55l-2.5 2.5"/></g><circle class="particle" cx="15.6" cy="3.2" r="1.1" fill="currentColor" stroke="none"/><circle class="particle" cx="20.8" cy="15.6" r="1.1" fill="currentColor" stroke="none"/><circle class="particle" cx="3.2" cy="15.6" r="1.1" fill="currentColor" stroke="none"/></svg>',
    lens: '<svg viewBox="0 0 24 24"><g class="lens"><circle cx="10.2" cy="10.2" r="6.3"/><path d="M15 15l5.4 5.4"/><path d="M6.6 10.2h7.2" opacity=".45"/></g></svg>',
    camera: '<svg viewBox="0 0 24 24"><path d="M3.5 8.5h4l1.5-2.5h6L16.5 8.5h4v11h-17z"/><circle cx="12" cy="13.5" r="3.4"/><g class="glint"><path d="M9.8 11.4 11 10.4"/></g></svg>',
    wait: '<svg viewBox="0 0 24 24"><circle class="waitdot" cx="6" cy="12" r="1.7" fill="currentColor" stroke="none"/><circle class="waitdot" cx="12" cy="12" r="1.7" fill="currentColor" stroke="none"/><circle class="waitdot" cx="18" cy="12" r="1.7" fill="currentColor" stroke="none"/></svg>'
  };

  /**
   * The glyphs the service names for an agent with no image of its own, in its wire words: the
   * command line's prompt and the mark of an agent Ghostlight does not know.
   */
  const GLYPH = Object.freeze({ commandLine: "command_line", unknown: "unknown" });

  /** The drawing of each glyph in GLYPH. */
  const SYMBOL_GLYPHS = Object.freeze({
    [GLYPH.commandLine]: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 4.5 6 8l-3.5 3.5M7.5 11.5h6"/></svg>',
    [GLYPH.unknown]: '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="5.8"/><path d="M6.4 6.6a1.7 1.7 0 1 1 2.3 1.6c-.5.2-.7.5-.7 1v.3M8 11.3h.01"/></svg>'
  });

  /**
   * The sizes an agent's symbol is drawn at, in pixels: a face of the At a glance summary, a chip
   * on its shelf, the head of an agent's card, an Integrations rail entry and the pane's hero.
   */
  const SYMBOL_SIZES = Object.freeze({ face: 20, chip: 14, card: 16, rail: 22, pane: 46 });

  /**
   * The attribute that holds the address of an agent's image on an image that has no source yet:
   * the service answers the image only to a request the page makes with its key, so the icons
   * module fills the source in.
   */
  const ICON_ATTRIBUTE = "data-icon";

  /**
   * What the window shows for `agent` at `size`, one of SYMBOL_SIZES: the image at the address the
   * service names (whichever image the service serves there, filled in once the page has it), or
   * the glyph it names. Every image the service serves is a whole tile, an app's icon on the ground
   * the service chose for it or a monogram, so the page draws them all the same way and decides
   * nothing about an agent's looks; every place that draws an agent draws it through this.
   */
  function symbolMarkup(agent, size) {
    const symbol = agent?.symbol ?? {};
    const pixels = SYMBOL_SIZES[size];
    const shown = symbol.icon
      ? `<img ${ICON_ATTRIBUTE}="${escapeHtml(symbol.icon)}" alt="" width="${pixels}" height="${pixels}">`
      : SYMBOL_GLYPHS[symbol.glyph] ?? SYMBOL_GLYPHS[GLYPH.unknown];
    return `<span class="symbol symbol-${size}" aria-hidden="true">${shown}</span>`;
  }

  /** The sentence that says which installed app an agent's image came from, when the service says one. */
  const symbolAbout = (agent) => agent?.symbol?.about ?? "";

  /**
   * The glyphs At a glance draws for what is not an agent: a browser's window frame, and the arrow
   * from the agents toward the browsers.
   */
  const KIND_GLYPHS = Object.freeze({
    browser: '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="1.8" y="2.8" width="12.4" height="10.4" rx="1.8"/><path d="M1.8 6h12.4"/></svg>',
    toward: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 8h10M9.5 5l3 3-3 3"/></svg>'
  });

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
    "Finding on page": "lens",
    "Screenshot": "camera",
    "Zooming": "scan",
    "Filling form": "keyboard",
    "Uploading file": "gear",
    "Running JavaScript": "gear",
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
   * What a channel switch can and cannot promise, said wherever the switches are (ADR-0202
   * Decision 22a). The channel is what Ghostlight's own program declared when the agent connected,
   * and any program running as the same user could declare another.
   */
  const CHANNEL_LIMIT = "Ghostlight's own program says whether an agent connected through MCP or the command line, and another program running as you could claim otherwise. These switches hold agents that follow the rules; they do not stop harmful software on this account.";

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
      title: "Who may work",
      items: [
        {
          key: "channels.mcp.enabled",
          name: "MCP agents",
          on: "Agents connected through MCP may work in the browser.",
          off: "Work from MCP agents is refused.",
          limit: CHANNEL_LIMIT
        },
        {
          key: "channels.cli.enabled",
          name: "Command line",
          on: "Scripts and programs that use ghostlight call may work in the browser.",
          off: "Work from the command line is refused.",
          limit: CHANNEL_LIMIT
        }
      ]
    },
    { title: "History", items: [{
      key: "audit.availability", name: "When history cannot be saved", kind: "choice",
      authority: "audit", field: "mode", source: "source", default: "keep_working",
      choices: [
        { value: "keep_working", label: "Keep working", detail: "Continue browser work and show which history could not be saved." },
        { value: "require_audit", label: "Stop browser work", detail: "Stop new browser work during a known saving failure. Check storage automatically." }
      ]
    }] },
    {
      title: "In the browser",
      items: [
        {
          key: "browser.tabs.allow_close",
          name: "Closing tabs",
          on: "Agents may close a tab they control.",
          off: "Closing a tab stays something only you do.",
          // The browser's own setting, which keeps those tabs open whatever this one says. Its
          // value lives in the browser, so the line says how it starts and where it is changed.
          browser: Object.freeze({
            word: "Preserve Ghostlight tabs keeps them open while it is on. It starts on.",
            where: "Set in the browser"
          })
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
              detail: "Ghostlight tells the agent which eligible browser windows to ask you to open."
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

  /**
   * The rule that does what no rules do: agents may work on ordinary websites. A draft that starts
   * with no rules carries it, so turning one setting off applies that alone and changes nothing
   * else.
   */
  const OPEN_RULE = Object.freeze({
    id: "ordinary-websites",
    hosts: "*",
    description: "Agents may work on ordinary websites."
  });

  /** What the editor says of a draft that will carry the open rule. */
  const OPEN_RULE_NOTE = "Applying adds one rule that lets agents work on ordinary websites, as they do now.";

  /** What the editor says of a draft whose rules were all removed. */
  const NO_RULES_LEFT = "No rules left. A policy with none refuses everything, so add at least one.";

  /** What the rule list says while there is no rule anywhere. The headline says what that means. */
  const NO_RULES = "No rules yet.";

  /** Every setting item, in one flat list, for lookups that do not care about grouping. */
  const SETTING_ITEMS = SETTING_GROUPS.flatMap((group) => group.items);

  /** The limits the settings with these keys carry, each said once. */
  function settingLimits(keys) {
    const limits = keys.map((key) => SETTING_ITEMS.find((item) => item.key === key)?.limit).filter(Boolean);
    return [...new Set(limits)];
  }

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

  /**
   * How long work took, or has taken so far: the one time format of every clock the window shows.
   * Under a minute it counts tenths ("5.6s"), under an hour minutes and seconds ("1m 05s"), and
   * after that hours ("1:05:09"). Tenths are cut, not rounded, so a running clock never shows
   * "60.0s".
   */
  function duration(ms) {
    if (!Number.isFinite(ms) || ms < 0) return "";
    if (ms < 60_000) return `${(Math.floor(ms / 100) / 10).toFixed(1)}s`;
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const second = String(seconds % 60).padStart(2, "0");
    if (minutes < 60) return `${minutes}m ${second}s`;
    return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, "0")}:${second}`;
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

  /** How long ago something settled, as a phrase: "20m ago". */
  const agoSaid = (timestamp) => `${ago(timestamp)} ago`;

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
    browser_find: "lens",
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
    browser_upload: "gear",
    browser_execute: "gear",
    browser_flow: "gear",
    browser_record: "camera",
    browser_diagnose: "scan",
    policy_explain: "scan"
  };

  const glyphFor = entry =>
    GLYPHS[ACTIVITY_GLYPH[entry.activity] ?? TOOL_GLYPH[entry.tool] ?? "scan"];
  /** How the service joins a set of capabilities into one label, as in "read + action". */
  const CAPABILITY_JOIN = " + ";

  /**
   * The capabilities an action used, in canonical order. A composition such as a flow carries
   * none of its own, so it uses what its steps used. A label that names no capability, such as
   * the service's "local" for an empty set, adds nothing.
   */
  function capabilitiesOf(entry) {
    const labels = [entry?.capability, ...(entry?.steps ?? []).map((step) => step.record?.capability)];
    const used = new Set(labels.flatMap((label) => String(label ?? "").split(CAPABILITY_JOIN)));
    return CAPABILITY_ORDER.filter((capability) => used.has(capability));
  }

  /**
   * The one capability the activity list names for an action: the most consequential it used, or
   * "" when it used none. Consequence only: the full set stays in the detail card, in authority and
   * in audit.
   */
  const capabilityWord = entry => capabilitiesOf(entry).at(-1) ?? "";

  /** The class that colours an action's edge and word by its most consequential capability. */
  const capabilityClass = entry => CAPABILITY_CLASS[capabilityWord(entry)] ?? CAPABILITY_CLASS.read;

  /** An action's capabilities as the detail card shows them, as in "read + action". */
  const capabilityLabel = entry => capabilitiesOf(entry).join(CAPABILITY_JOIN);

  /** An action's capabilities as a screen reader hears them on its row, as in "read and action". */
  function capabilitiesSaid(entry) {
    const used = capabilitiesOf(entry);
    return used.length < 2 ? used.join("") : `${used.slice(0, -1).join(", ")} and ${used.at(-1)}`;
  }

  /** What a composition's steps used, in a person's words, as in "Steps: look at pages, click and type". */
  function stepsUsed(entry) {
    const used = capabilitiesOf(entry);
    return used.length ? `Steps: ${used.map((capability) => CAPABILITY_WORDS[capability]).join(", ")}` : "";
  }

  /** What the audit can honestly say happened to the page, with no payload to draw on. */
  const EFFECT_STORY = {
    none: "left the page unchanged",
    applied: "changed the page",
    partial: "changed the page in part",
    unknown: "outcome could not be confirmed"
  };

  /**
   * The note a settled row adds to its sentence for the readiness the service observed: only a
   * caution about the sentence. A page that was ready to use, loaded or not, needs no word.
   */
  const READINESS_NOTE = {
    not_applicable: "",
    complete: "",
    interactive: "",
    loading: "never settled",
    unknown: "readiness unknown"
  };

  /**
   * Which readiness values mark a settled row as worth finding while scrolling.
   *
   * A document that never settled, or whose readiness could not be observed, is a caution about
   * the sentence beside it. The duration cell carries the amber treatment so the row is found by
   * eye instead of read for.
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

  return Object.freeze({ FEED_LIMIT, HISTORY_HELD, WORKING_LATCH_MS, VIEWS, SEARCH_VIEWS, HIT_KIND, SEARCH_FOUND_NOTHING, searchAll,
    DEFAULT_VIEW, DEFAULT_SECTION, ADDRESSES, SETUP_ADDRESS, SETUP, parseAddress, addressOf,
    POLICY_CHANGED_ELSEWHERE, WINDOW_FAULT, NO_ANSWER, COPY_FAILED, LOAD_FAILED, TRY_AGAIN, OUTCOME_WORD, PHASE, PHASE_WORD, workCounts, BAND_FACT, bandFacts, rowBy, LAST_ACTION, actionSaid,
    CONTINUE_LABEL, POLICY_IN_WINDOW, COPIED_SETUP, COPIED_COMMAND, RECONNECTING,
    WATCHING_TURNED_ON, WATCHING_TURNED_OFF, TEST_NOTIFICATION_SENT, ADDRESS_COPIED, SETTINGS, SETTINGS_SECTIONS,
    CONNECTION_ID_COPIED, INTAKE, INTAKE_MARK, toolWords, GLANCE, GLANCE_CARD_LINES, CARD, browserChipTitle,
    agentsCount, browsersCount, tabsCount, sessionsCount, andMore, sessionLine, noun,
    screenState, byteSize,
    neverTouchedCount, leftOutCount,
    WORKING_TONE, AGENT_STATE_WORDS, agentStateWord, BY_HAND, agentStanding, AGENT_GROUPS, ABSENT_GROUP, AGENT_ACTS, AGENTS, noAgentCalled, moreAgents,
    agentHeadline, agentPlan, agentUse, agentLive, agentLead, agentWhy, agentFileLabel, agentCopyNote, agentEntryLabel,
    removeNote, removeLabel, confirmRemovalWords, confirmFixWords,
    GLYPHS, GLYPH, SYMBOL_GLYPHS, SYMBOL_SIZES, ICON_ATTRIBUTE, symbolMarkup, symbolAbout, KIND_GLYPHS, EFFECT_STORY, READINESS_NOTE, readinessNeedsAttention,
    DESTINATIONS, glyphFor, capabilitiesOf, capabilityWord, capabilityClass, capabilityLabel, capabilitiesSaid, stepsUsed,
    CAPABILITY_ORDER, CAPABILITY_WORDS,
    CAPABILITY_BADGE, CAPABILITY_TONE, SETTING_GROUPS, SACRED_KEY, settingWords, settingLimits,
    OPEN_RULE, OPEN_RULE_NOTE, NO_RULES_LEFT, NO_RULES,
    validHostPattern, hostReadback, patternCovers,
    escapeHtml, words, duration, ago, agoSaid });
});
