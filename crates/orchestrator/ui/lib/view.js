// Ghostlight workbench -- everything that touches the document, and nothing else.
//
// This draws what it is handed. It cannot fetch, so it can never fail on a missing snapshot, and
// it cannot change the cache, so a rendering fault can never corrupt what the window believes.
// Where it needs a fact about a workspace it asks through a reader the composition root supplies,
// rather than reaching into the store itself.
(function installGhostlightView(root, factory) {
  const api = factory();
  root.GhostlightView = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightViewApi() {
  "use strict";

  const {
    VIEWS, SETTINGS_SECTIONS, DEFAULT_SECTION, addressOf, GLYPHS, EFFECT_STORY, READINESS_NOTE, readinessNeedsAttention,
    DESTINATIONS, glyphFor, capabilityClass, capabilityWord, capabilityLabel, capabilitiesSaid, stepsUsed,
    CAPABILITY_ORDER, CAPABILITY_BADGE, CAPABILITY_TONE, SETTING_GROUPS, SACRED_KEY, settingWords, settingLimits,
    OPEN_RULE, OPEN_RULE_NOTE, NO_RULES_LEFT, NO_RULES, SETTINGS,
    WORKING_TONE, AGENT_STATE_WORDS, agentStateWord, BY_HAND, agentStanding, AGENT_GROUPS, ABSENT_GROUP, AGENT_ACTS, AGENTS, noAgentCalled, moreAgents,
    agentHeadline, agentPlan, agentUse, agentLive, agentLead, agentWhy, agentFileLabel, agentCopyNote, agentEntryLabel,
    removeNote, removeLabel, confirmRemovalWords, confirmFixWords,
    hostReadback, patternCovers,
    escapeHtml, words, duration, ago, agoSaid,
    OUTCOME_WORD, PHASE_WORD, workCounts, bandFacts, rowBy, LAST_ACTION, actionSaid, LOAD_FAILED, TRY_AGAIN,
    CONTINUE_LABEL, POLICY_IN_WINDOW, RECONNECTING, neverTouchedCount, leftOutCount,
    KIND_GLYPHS, GLYPH, SYMBOL_GLYPHS, symbolMarkup, symbolAbout, INTAKE, INTAKE_MARK, toolWords, GLANCE, GLANCE_CARD_LINES,
    CARD, browserChipTitle,
    agentsCount, browsersCount, tabsCount, sessionsCount, andMore, sessionLine,
    FEED_LIMIT, HIT_KIND, SEARCH_FOUND_NOTHING, searchAll
  } = globalThis.GhostlightWords;
  const { settledMs, isRunning, isPending, isUnsettled, isBlocked, isFailed, isKept } = globalThis.GhostlightEntries;
  const { ACT, OPENER_SERVICE, COPY_KIND } = globalThis.GhostlightTransport;

  const TOAST_MS = 4200;

  /** What the band's wheel says for each act it can do, in the words of the tray and Settings. */
  const WHEEL_WORDS = Object.freeze({
    [ACT.hold]: SETTINGS.pause,
    [ACT.resume]: SETTINGS.resume,
    [ACT.startSession]: SETTINGS.startSession
  });

  /** The wheel's two faces: pause while it would pause, play while it would let agents work. */
  const PAUSE_ICON = '<rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/>';
  const PLAY_ICON = '<path d="M7 4.5 19 12 7 19.5z"/>';

  /**
   * The fewest milliseconds between two sentences the live region says, so a burst of quick actions
   * reaches a screen reader as its latest action, not as a stream.
   */
  const SAID_GAP_MS = 1000;

  /**
   * The fewest milliseconds a running card stays before the next action may swap in with motion:
   * the settle beat (--beat-settle, 420ms) and a short dwell. A burst faster than this replaces the
   * card in place, and the live look holds across the gaps between its actions.
   */
  const SWAP_DWELL_MS = 600;

  /**
   * How long a settled action leads the running card before it rests as the last action, without
   * its glow, so the band's word leads a window where nothing has run for a while.
   */
  const REST_AFTER_MS = 5 * 60_000;

  /** What can take focus inside a dialog, in the order Tab walks it. */
  const FOCUSABLE = "button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex=\"-1\"])";

  /** The page disclosure key of the At a glance summary, which opens its shelves. */
  const GLANCE_KEY = "glance";

  /** The id of the open agent's card, which its chip controls. */
  const GLANCE_CARD_ID = "glance-card";

  /** How many agents the closed summary shows a face for before it says how many more. */
  const GLANCE_FACES = 4;

  /**
   * The data keys a repaint gives focus back by: the control that held focus is found again by the
   * same keys with the same values, all of them together, so one agent's Remove is not its Copy.
   */
  const FOCUS_KEYS = Object.freeze([
    "actionDetails", "glanceAgent", "copyConnection", "agent", "agentFold",
    "harness", "harnessOperation", "harnessAction", "copyKind", "agentsRecheck", "showAgent"
  ]);

  /** What a control is, by its focus keys, or null for one a repaint cannot find again. */
  function controlIdentity(node) {
    const named = FOCUS_KEYS.filter((key) => node?.dataset?.[key] !== undefined);
    return named.length ? named.map((key) => `${key}=${node.dataset[key]}`).join("&") : null;
  }

  /** The chevron that marks a fold or a disclosure; it turns over once open. */
  const CHEVRON = '<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2 3.5l3 3 3-3"/></svg>';

  /** The panels that hold data, each with the line that says why it holds none and its section. */
  const DATA_PANELS = Object.freeze([
    Object.freeze({ line: "closed-monitor", section: "view-monitor" }),
    Object.freeze({ line: "closed-integrations", section: "view-integrations" }),
    Object.freeze({ line: "closed-status", section: "view-status" }),
    Object.freeze({ line: "closed-policy", section: "view-policy" })
  ]);

  function create({
    sessionFor = () => null, coverageFor = () => null, onFailure = () => {},
    now = () => Date.now(), schedule = (work, ms) => setTimeout(work, ms)
  } = {}) {
    // Every element with an id, looked up once from the document, so a new id needs no list.
    const el = Object.create(null);
    for (const node of document.querySelectorAll("[id]")) el[node.id] = node;

    const rowNodes = new Map();
    const painted = Object.create(null);
    let toastTimer = null;
    let confirmation = null;
    // The open dialog, with the control it gives focus back to; see holdFocus.
    let modal = null;

    /*
     * What this screen was told it may do.
     *
     * Until it is told, it may do everything, which is the Ghostlight window's own situation and what
     * a view built directly assumes. An act it may not do is drawn as the state it would have
     * changed, with one line saying where the change is made.
     */
    let access = null;
    let closedMode = false;
    let lost = false;
    let farewell = null;
    let lastFacts = null;
    const can = (act) => !closedMode && (access?.acts?.[act]?.available ?? true);
    const whyNot = (act) => access?.acts?.[act]?.why_not ?? "";
    const opensLinks = () => access !== null && access.opener !== OPENER_SERVICE;

    /**
     * The one line a panel shows for the acts it cannot offer: the reason, when the panel says
     * one, and the way on. A line that belongs to one screen names its address; the band's line
     * carries none and the page continues at the screen it is showing.
     */
    function continueMarkup(why, address) {
      const place = address ? ` data-address="${escapeHtml(address)}"` : "";
      return (why ? `<span>${escapeHtml(why)}</span>` : "")
        + `<button class="link-button" type="button" data-continue${place}>${escapeHtml(CONTINUE_LABEL)}</button>`;
    }

    /**
     * Name the body for the runtime state it shows, and keep every other class it wears.
     *
     * The page's personal settings are classes on the body too, so the runtime name replaces only
     * the runtime classes.
     */
    function runtimeClass(name) {
      const kept = String(document.body.className).split(/\s+/)
        .filter((className) => className && !className.startsWith("runtime-"));
      document.body.className = [name, ...kept].join(" ");
    }

    /** Run one fallible paint. Report what went wrong, and say it did not happen. */
    function attempt(what, step) {
      try {
        step();
        return true;
      } catch (error) {
        onFailure(what, error);
        return false;
      }
    }

    /**
     * Paint one panel when its facts changed, and never record a paint that did not happen.
     *
     * The signature is recorded after the paint succeeds, so a panel that threw is painted again
     * on the next change.
     */
    function ifChanged(key, facts, paint) {
      const signature = JSON.stringify(facts);
      if (painted[key] === signature) return;
      if (attempt(`painting ${key}`, paint)) painted[key] = signature;
    }

    /* ------------------------------ sentences ----------------------------- */

    /**
     * The sentence the orchestrator authored for an entry.
     *
     * A live operation names what it is doing. A settled one falls back to its governed outcome,
     * because the record is payload-free by design: no page text, no field value, and no URL past
     * the host the action landed on.
     */
    function sentence(entry) {
      if (entry.summary) return entry.summary;
      if (!entry.settled) return entry.activity;
      if (isBlocked(entry)) return OUTCOME_WORD.blocked;
      return EFFECT_STORY[entry.effect] || (entry.status ? words(entry.status) : "completed");
    }

    /**
     * What the row says happened.
     *
     * The orchestrator already chose the outcome's useful register and made every named
     * measurement agree with the structured observation. The surface renders that without guessing.
     */
    function describe(entry) {
      const observed = entry.settled ? entry.observed : null;
      const body = sentence(entry);
      const note = observed ? READINESS_NOTE[observed.readiness] ?? "" : "";
      return note ? `${body} (${note})` : body;
    }

    /**
     * Which intake drove this action.
     *
     * New work carries its own connection evidence. Older records must not borrow attribution
     * from a later connection that happens to share their workspace.
     */
    function channelFor(entry) {
      if (entry.channel) return entry.channel;
      if (entry.provenance?.channel) return entry.provenance.channel;
      if (entry.settled) return "";
      return sessionFor(entry.workspace)?.channel ?? "";
    }

    /**
     * The row's intake cell: a mark for the exception, the command line, and nothing for MCP.
     * The glyph is the mark; its words are for a hover and a screen reader.
     */
    function intakeMarkup(entry) {
      const mark = INTAKE_MARK[channelFor(entry)];
      if (!mark) return '<div class="row-channel"></div>';
      return `<div class="row-channel" title="${escapeHtml(mark)}">${SYMBOL_GLYPHS[GLYPH.commandLine]}`
        + `<span class="sr-only">${escapeHtml(mark)}</span></div>`;
    }

    /**
     * The agent's name as text, and the hover it wears. The service names the agent of every
     * action from the action's own connection evidence, including after disconnect or restart, and
     * gives the words of the hover where a name needs one more, as work recorded before names were
     * kept does. A cell whose words a screen reader already hears elsewhere (`heardElsewhere`) is
     * hidden from it.
     */
    function agentMarkup(tag, className, entry, heardElsewhere = false) {
      const agent = entry.agent;
      const hover = agent?.note ? ` title="${escapeHtml(agent.note)}"` : "";
      const hidden = heardElsewhere ? ' aria-hidden="true"' : "";
      return `<${tag} class="${className}"${hover}${hidden}>${escapeHtml(agent?.name ?? "")}</${tag}>`;
    }

    /**
     * The browser that performed or is performing this action. A settled action names only the
     * browser its record names: one refused before it reached a browser names none, and must not
     * borrow the browser its session works in now.
     */
    function browserFor(entry) {
      if (entry.browser) return entry.browser;
      if (entry.settled) return "";
      return sessionFor(entry.workspace)?.browser ?? "";
    }

    /* -------------------------------- monitor ----------------------------- */

    // Expansion is disposable view state; new receipts never force a panel open. Feed
    // disclosures belong to an activity entry and are forgotten with it; page disclosures, such
    // as the At a glance summary, belong to no entry, so no feed change forgets them.
    const expandedHistory = new Set();
    const openPage = new Set();

    /** The open-state set a `<details>` is remembered in and its key there, or null when it is not remembered. */
    function disclosure(details) {
      const feedKey = details?.dataset?.historyDetails;
      if (feedKey) return { remembered: expandedHistory, key: feedKey };
      const pageKey = details?.dataset?.pageDetails;
      if (pageKey) return { remembered: openPage, key: pageKey };
      return null;
    }

    document.addEventListener("click", (event) => {
      // Native toggle events are deferred. Remember the click before a receipt can repaint it.
      const details = event.target.closest?.("summary")?.parentElement;
      const clicked = disclosure(details);
      if (clicked) {
        if (details.open) clicked.remembered.delete(clicked.key); else clicked.remembered.add(clicked.key);
        details.dataset.historyOpening = String(!details.open);
      }
      const agent = event.target.closest?.("[data-glance-agent]");
      if (agent) {
        const id = agent.dataset.glanceAgent;
        glanceAgent = glanceAgent === id ? null : id;
        if (lastConnections) paintConnections(lastConnections);
        return;
      }
      const agentEntry = event.target.closest?.("[data-agent]");
      if (agentEntry) {
        showAgent(agentEntry.dataset.agent);
        return;
      }
      if (event.target.closest?.("[data-agent-fold]")) {
        toggleFold();
        return;
      }
      // "See them in At a glance" opens the summary on that agent's card as it goes there.
      const shown = event.target.closest?.("[data-show-agent]");
      if (shown) {
        glanceAgent = shown.dataset.showAgent;
        openPage.add(GLANCE_KEY);
        if (lastConnections) paintConnections(lastConnections);
        return;
      }
      const button = event.target.closest?.("[data-action-details]");
      if (button) showDetails(button, !expandedHistory.has(button.dataset.actionDetails));
    });
    document.addEventListener("toggle", (event) => {
      const details = event.target;
      const toggled = disclosure(details);
      if (!toggled || details.isConnected === false) return;
      const wasOpen = toggled.remembered.has(toggled.key);
      if (details.open) toggled.remembered.add(toggled.key); else toggled.remembered.delete(toggled.key);
      if (details.open && (!wasOpen || details.dataset.historyOpening === "true") && details.classList.contains("composition-details")) {
        details.querySelector('[data-step-problem="true"]')?.scrollIntoView({ block: "nearest" });
      }
      delete details.dataset.historyOpening;
    }, true);

    /** Open or close the details an action's row button controls, and remember which are open. */
    function showDetails(button, open) {
      const key = button.dataset.actionDetails;
      const panel = document.getElementById(button.getAttribute("aria-controls"));
      if (!panel) return;
      if (open) expandedHistory.add(key); else expandedHistory.delete(key);
      button.setAttribute("aria-expanded", String(open));
      panel.hidden = !open;
    }

    /**
     * Replace a panel's markup and keep the person's place in it: the scroll of its step lists, the
     * summary that held focus, and the control that held focus, found again by what it is. While a
     * dialog is open, the control the dialog gives focus back to is found again instead. When that
     * control is gone or is waiting on its own act, `fallback()` names where focus goes, so it
     * never drops to the page's body.
     */
    function replaceHistoryMarkup(node, markup, fallback = () => null) {
      const scroll = new Map([...node.querySelectorAll(".history-steps")].map((list) =>
        [list.closest("details")?.dataset.historyDetails, list.scrollTop]));
      const held = modal ? modal.opener : document.activeElement;
      const inside = Boolean(held) && node.contains(held);
      const focusKey = inside && held.tagName === "SUMMARY" ? disclosure(held.closest("details"))?.key : null;
      const identity = inside ? controlIdentity(held) : null;
      node.innerHTML = markup;
      for (const list of node.querySelectorAll(".history-steps")) {
        const key = list.closest("details")?.dataset.historyDetails;
        if (scroll.has(key)) list.scrollTop = scroll.get(key);
      }
      if (!inside) return;
      let back = null;
      if (focusKey) {
        back = [...node.querySelectorAll("details")].find((details) => disclosure(details)?.key === focusKey)
          ?.querySelector("summary") ?? null;
      } else if (identity) {
        const first = FOCUS_KEYS.find((key) => held.dataset[key] !== undefined);
        const selector = `[data-${first.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}]`;
        back = [...node.querySelectorAll(selector)]
          .find((found) => controlIdentity(found) === identity && !found.disabled) ?? null;
      }
      back ??= fallback();
      if (!back) return;
      if (modal) modal.opener = back;
      else back.focus({ preventScroll: true });
    }

    function historyDetails(key, title, body, className = "permission-details") {
      return `<details class="${className}" data-history-details="${escapeHtml(key)}"${expandedHistory.has(key) ? " open" : ""}>`
        + `<summary class="disclosure">${escapeHtml(title)}${CHEVRON}</summary>${body}</details>`;
    }

    /**
     * The permission checks of an action, one line per distinct sentence: checks the service
     * explained in the same words are said once, under every check they cover.
     */
    function permissionMarkup(entry, key) {
      const explanations = entry.permissionExplanations ?? entry.permission_explanations ?? [];
      const checks = entry.permissions?.checks ?? [];
      if (!explanations.length) return "";
      const lines = new Map();
      explanations.forEach((explanation, index) => {
        const check = checks[index];
        const context = check ? `${(check.requirements ?? []).join(" + ")}${check.host ? ` on ${check.host}` : ""}` : "";
        const line = lines.get(explanation) ?? [];
        if (context && !line.includes(context)) line.push(context);
        lines.set(explanation, line);
      });
      const rows = [...lines].map(([explanation, contexts]) =>
        `<li><span>${escapeHtml(contexts.join(", "))}</span><p>${escapeHtml(explanation)}</p></li>`).join("");
      const limited = entry.permissions?.truncated ? `<p>${escapeHtml(CARD.permissionsOmitted)}</p>` : "";
      return historyDetails(key, CARD.permissions, `<ul>${rows}</ul>${limited}`);
    }

    function storageMarkup(entry) {
      return entry.storage_detail ? `<p class="coverage-note">${escapeHtml(entry.storage_detail)}</p>` : "";
    }

    /**
     * An action's connection, as an agent's card says it: the agent is named once, in the card's
     * meta line, so this says only what Ghostlight can vouch for in the service's words (which
     * quote a client's own name only where it says more than the agent's), the intake only when it
     * is the command line, and the connection id behind Copy id. Empty when there is nothing to say.
     */
    function connectionFields(evidence) {
      let body = "";
      if (evidence.explanation) body += `<p class="connection-explanation">${escapeHtml(evidence.explanation)}</p>`;
      if (evidence.channel === INTAKE.cli) body += `<p class="connection-explanation">${escapeHtml(CARD.commandLine)}</p>`;
      if (evidence.connection_id) {
        body += `<button class="link-button connection-copy" type="button" data-copy-connection="${escapeHtml(evidence.connection_id)}"`
          + ` title="${escapeHtml(evidence.connection_id)}">${escapeHtml(GLANCE.copyId)}</button>`;
      }
      return body;
    }

    /** The connection disclosure of an action, once per card: a flow's steps share their flow's. */
    function connectionMarkup(entry, key) {
      if (!entry.provenance && !entry.settled && entry.complete !== true) return "";
      const body = entry.provenance ? connectionFields(entry.provenance)
        : `<p class="connection-explanation">${escapeHtml(CARD.notRecorded)}</p>`;
      return body ? historyDetails(key, CARD.connection, body, "connection-details") : "";
    }

    function compositionMarkup(entry) {
      const connection = connectionMarkup(entry, `${entry.invocation}:connection`);
      if (!entry.steps?.length) return storageMarkup(entry) + permissionMarkup(entry, `${entry.invocation}:permission`) + connection + coverageMarkup(entry);
      const rows = entry.steps.map((step) => {
        const receipt = step.record;
        const state = receipt?.status ?? step.state;
        const labels = { succeeded: "Completed", not_started: "Could not start", not_run: "Not run", pending: "Pending", unconfirmed: "Receipt unavailable" };
        const label = labels[state] ?? words(state);
        const problem = receipt?.storage === "unconfirmed" || !["succeeded", "not_run", "pending"].includes(state);
        const title = receipt?.summary ?? (step.tool ? toolWords(step.tool) : `Step ${step.position}`);
        const detail = receipt ? storageMarkup(receipt) + permissionMarkup(receipt, `${entry.invocation}:step:${step.position}:permission`) : "";
        return `<li class="history-step" data-step-problem="${problem}"><div class="step-line">`
          + `<span class="step-number">${step.position}</span><span>${escapeHtml(title)}</span><span class="step-status">${escapeHtml(label)}</span></div>`
          + (receipt ? `<div class="step-meta">${escapeHtml(receipt.capability)}${receipt.effect !== "none" ? `; ${escapeHtml(words(receipt.effect))} effects` : ""}</div>` : "")
          + detail + "</li>";
      }).join("");
      return storageMarkup(entry) + historyDetails(`${entry.invocation}:steps`, `View ${entry.steps.length} steps`, `<ol class="history-steps">${rows}</ol>`, "composition-details") + connection + coverageMarkup(entry);
    }

    function coverageMarkup(entry) {
      const details = coverageFor(entry.invocation);
      if (!details || (!details.coverage.page_excluded_documents && !details.coverage.unavailable_documents && !details.coverage.limited_by_size)) return "";
      const hosts = (details.excluded_hosts ?? []).map((host) => `<li>${escapeHtml(host)}</li>`).join("");
      // A screen that may not name the sites is told only how many there were.
      const unnamed = !hosts && details.excluded_host_count > 0
        ? `<p>${escapeHtml(leftOutCount(details.excluded_host_count))}</p>` : "";
      const indication = details.proactive ? `<p class="coverage-note">${escapeHtml(details.summary)}</p>` : "";
      return indication + historyDetails(`${entry.invocation}:coverage`, "Coverage details",
        `<p>${escapeHtml(details.summary)}</p>` + (hosts ? `<p>${escapeHtml(details.explanation)}</p><ul>${hosts}</ul>` : unnamed));
    }

    /**
     * The facts of one action, for the running card and a row's details. A resting card says its
     * age above. The running card says the outcome beside its clock (`outcomeBeside`); a row's
     * details say a settled action's outcome in the meta line when it did not succeed.
     */
    function heroMarkup(entry, resting = false, outcomeBeside = false) {
      // The sentence names the host itself, so the hero carries no host chip: it would say the
      // same thing twice. Readiness is the one observed fact no sentence states.
      const observed = entry.settled ? entry.observed : null;
      const note = observed ? READINESS_NOTE[observed.readiness] ?? "" : "";
      // The agent's name already says the command line, so the intake is not said again.
      const meta = [];
      if (entry.agent) meta.push(agentMarkup("span", "hero-agent", entry));
      const browser = browserFor(entry);
      if (browser) meta.push(`<span><i></i>${escapeHtml(browser)}</span>`);
      // A composition used what its steps used: said once, in a person's words, beside each step's own.
      const label = capabilityLabel(entry);
      if (entry.steps?.length) {
        const steps = stepsUsed(entry);
        if (steps) meta.push(`<span><i></i>${escapeHtml(steps)}</span>`);
      } else if (label) {
        meta.push(`<span><i></i>${escapeHtml(label)}</span>`);
      }
      // Success is what the sentence already says, a refusal has its own line, and the catalog
      // name is the tool's hover, so none of them is said again here.
      const outcome = entry.settled && !outcomeBeside ? outcomeWord(entry) : "";
      if (outcome) meta.push(`<span><i></i>${escapeHtml(outcome.toLowerCase())}</span>`);
      if (note) meta.push(`<span><i></i>${escapeHtml(note)}</span>`);
      if (entry.settled && entry.endedAt && !resting) meta.push(`<span><i></i>${escapeHtml(agoSaid(entry.endedAt))}</span>`);

      // The sentence already says why a refusal happened, so the reason is not said again as a
      // code; the refusal line says who decided and where to look.
      const reason = isBlocked(entry) ? refusalMarkup(entry) : "";

      return `<div class="hero-tool" title="${escapeHtml(entry.tool ?? "")}">${escapeHtml(toolWords(entry.tool))}`
        + (label ? `<span class="cap-label">${escapeHtml(label)}</span>` : "") + "</div>"
        + `<p class="hero-activity">${escapeHtml(sentence(entry))}</p>`
        + reason
        + compositionMarkup(entry)
        + (meta.length ? `<div class="hero-meta">${meta.join("")}</div>` : "");
    }

    /**
     * Where a refusal leads.
     *
     * A refusal row names the deciding layer and rule and the organization's contacts, so the
     * person knows what to do next.
     */
    function refusalMarkup(entry) {
      if (!entry.policyTier && !entry.grantId && !entry.denialId) return "";
      const managed = entry.policyTier === "managed";
      const who = managed ? (passportName() ?? "your organization") : "your own rules";
      const parts = [`Refused by ${who}`];
      if (entry.grantId) parts.push(`rule ${entry.grantId}`);
      const contacts = managed ? passportContacts() : [];
      const ask = contacts.length
        ? ` Ask ${contacts.map((contact) => `${contact.label || words(contact.kind)}: ${contact.value}`).join(", ")}.`
        : "";
      return `<p class="hero-refusal">${escapeHtml(`${parts.join(", ")}.${ask}`)}`
        + (entry.denialId ? `<span class="denial-id">${escapeHtml(entry.denialId)}</span>` : "")
        + `<button class="link-button" type="button" data-view="policy">See the policy</button></p>`;
    }

    /* The passport the last snapshot carried, so a refusal can name who to ask without a fetch. */
    let passport = null;
    const passportName = () => passport?.organization ?? null;
    const passportContacts = () => passport?.contacts ?? [];

    /** How long work unsettled has taken so far. */
    const elapsedSoFar = (entry) => duration(Math.max(0, now() - (entry.startedAt ?? now())));

    function heroRightMarkup(entry) {
      const unsettled = isUnsettled(entry);
      const elapsed = unsettled ? elapsedSoFar(entry) : duration(settledMs(entry));
      const outcome = unsettled
        ? PHASE_WORD[entry.phase]
        : isBlocked(entry)
          ? OUTCOME_WORD.blocked
          : outcomeWord(entry) || words(entry.status ?? "completed");
      // The stopwatch is for the eye: a screen reader hears the settled sentence instead.
      return `<div class="elapsed" id="elapsed" aria-hidden="true">${escapeHtml(elapsed)}</div>`
        + `<div class="outcome">${escapeHtml(outcome)}</div>`;
    }

    /**
     * The word a settled action that did not succeed carries, or "" for one that did: Failed, or
     * Unknown when what it did cannot be confirmed, for a permitted action that did not do what it
     * was asked, and Kept for one a browser setting held back. A refusal has its own treatment.
     */
    function outcomeWord(entry) {
      if (isFailed(entry)) return OUTCOME_WORD[entry.status];
      if (isKept(entry)) return OUTCOME_WORD.kept;
      return "";
    }

    /** The class a settled outcome adds to its row and to the running card. */
    function outcomeClass(entry) {
      if (isFailed(entry)) return "failed";
      if (isKept(entry)) return "kept";
      return "";
    }

    /*
     * The live region: one sentence per settled action, and never a running clock.
     *
     * The card and the rows are not live regions, because a stopwatch rewrites them ten times a
     * second. An action is said once, when it settles while this window watches; what the feed
     * already held when it was drawn is history and is not said. A burst of actions closer together
     * than SAID_GAP_MS is said as its latest.
     */
    const spoken = new Set();
    let toSay = null;
    let sayTimer = null;
    let saidAt = -Infinity;

    /** Say a settled action once, at most one sentence a second. */
    function say(entry) {
      if (!entry?.settled || spoken.has(entry.invocation)) return;
      spoken.add(entry.invocation);
      const outcome = isBlocked(entry) ? OUTCOME_WORD.blocked : outcomeWord(entry);
      toSay = actionSaid(entry.tool, describe(entry), outcome);
      if (sayTimer !== null) return;
      const wait = saidAt + SAID_GAP_MS - now();
      if (wait <= 0) speak();
      else sayTimer = schedule(speak, wait);
    }

    function speak() {
      sayTimer = null;
      if (toSay === null) return;
      el["action-said"].textContent = toSay;
      toSay = null;
      saidAt = now();
    }

    /** Draw the running card, and say the action when it has just settled. */
    function hero(entry, animate) {
      paintHero(entry, animate);
      say(entry);
    }

    /*
     * The running card's look over time.
     *
     * Only running work looks live. A burst of quick actions keeps the live look across the short
     * gaps between them, and swaps without motion, so the card does not jitter. A settled action
     * that has led for REST_AFTER_MS rests as the last action.
     */
    let heroEntry = null;
    let heroSince = -Infinity;
    let liveUntil = -Infinity;
    let liveHeld = false;
    let resting = false;

    /** Whether a settled action has led the card long enough to rest. */
    const rests = (entry) => Boolean(entry?.settled && entry.endedAt) && now() - entry.endedAt >= REST_AFTER_MS;

    /** Let the held live look go once the burst is over, unless running work took the card back. */
    function releaseLive() {
      const left = liveUntil - now();
      if (left > 0) {
        schedule(releaseLive, left);
        return;
      }
      liveHeld = false;
      if (heroEntry && !isRunning(heroEntry)) el.hero.classList.remove("live");
    }

    function paintHero(entry, animate) {
      const wasLive = heroEntry !== null && (isRunning(heroEntry) || liveHeld);
      heroEntry = entry ?? null;
      if (!entry) {
        liveHeld = false;
        resting = false;
        el.hero.className = "hero";
        el["hero-med"].innerHTML = GLYPHS.scan;
        el["hero-body"].innerHTML = '<div class="hero-tool">Nothing yet</div>'
          + '<p class="hero-activity">The first browser action an agent takes will appear here.</p>';
        el["hero-right"].innerHTML = "";
        return;
      }
      const outcome = outcomeClass(entry);
      resting = rests(entry);
      // A plain settle holds the live look for the dwell. Work that waits or is held, a refusal and
      // a failure show at once.
      const holding = Boolean(entry.settled) && wasLive && !isBlocked(entry) && !outcome && !resting;
      if (holding && !liveHeld) {
        liveHeld = true;
        liveUntil = now() + SWAP_DWELL_MS;
        schedule(releaseLive, SWAP_DWELL_MS);
      } else if (!holding) {
        liveHeld = false;
      }
      el.hero.className = `hero ${capabilityClass(entry)}`;
      if (isRunning(entry) || liveHeld) el.hero.classList.add("live");
      if (isPending(entry)) el.hero.classList.add(entry.phase);
      if (isBlocked(entry)) el.hero.classList.add("blocked");
      if (outcome) el.hero.classList.add(outcome);
      if (resting) el.hero.classList.add("rested");
      el["hero-med"].innerHTML = glyphFor(entry);
      const rest = resting
        ? `<div class="hero-rest"><span class="kicker">${escapeHtml(LAST_ACTION)}</span>`
          + `<span class="hero-age" id="hero-age">${escapeHtml(agoSaid(entry.endedAt))}</span></div>`
        : "";
      replaceHistoryMarkup(el["hero-body"], rest + heroMarkup(entry, resting, true));
      el["hero-right"].innerHTML = heroRightMarkup(entry);
      if (animate) {
        for (const node of [el["hero-med"], el["hero-body"], el["hero-right"]]) {
          node.classList.remove("swap-in");
          void node.offsetWidth;
          node.classList.add("swap-in");
        }
      }
    }

    function rowMarkup(entry) {
      const detailsKey = `${entry.invocation}:action`;
      const detailsId = `action-details-${entry.invocation}`;
      const open = expandedHistory.has(detailsKey);
      // The duration column holds the time alone, or for pending work the word that says why there
      // is none yet. An action that did not succeed says so in its own sentence and its row's look;
      // a screen reader hears the outcome word, or the pending one, on the button.
      const pending = isPending(entry);
      const said = escapeHtml(pending ? PHASE_WORD[entry.phase]
        : isRunning(entry) ? elapsedSoFar(entry) : duration(settledMs(entry)));
      const outcome = pending ? PHASE_WORD[entry.phase]
        : isBlocked(entry) ? OUTCOME_WORD.blocked : outcomeWord(entry);
      // The list names one capability, the most consequential, in its own column, or under the tool
      // where there is no room for the column. A screen reader hears the whole set on the button.
      const capability = escapeHtml(capabilityWord(entry));
      const heard = capabilitiesSaid(entry);
      // Who drove it and when it settled have columns of their own where the row has room, and a
      // line under the activity where it has not. A screen reader hears both on the button.
      return `<div class="med-mini" aria-hidden="true">${glyphFor(entry)}</div>`
        + `<button class="row-tool" type="button" title="${escapeHtml(entry.tool ?? "")}" data-action-details="${escapeHtml(detailsKey)}" aria-expanded="${open}" aria-controls="${escapeHtml(detailsId)}">`
        + `<span class="row-tool-name">${escapeHtml(toolWords(entry.tool))}</span>`
        + (capability ? `<span class="row-tool-cap" aria-hidden="true">${capability}</span>` : "")
        + (heard ? `<span class="sr-only">, ${escapeHtml(heard)}</span>` : "")
        + `<span class="sr-only row-by-said">${escapeHtml(bySaid(entry))}</span>`
        + (outcome ? `<span class="sr-only">, ${escapeHtml(outcome.toLowerCase())}</span>` : "")
        + "</button>"
        + intakeMarkup(entry)
        + `<div class="row-what"><div class="row-activity">${escapeHtml(describe(entry))}</div>`
        + `<div class="row-by" aria-hidden="true">${INTAKE_MARK[channelFor(entry)] ? SYMBOL_GLYPHS[GLYPH.commandLine] : ""}`
        + `<span class="row-by-words">${escapeHtml(byShown(entry))}</span></div></div>`
        + agentMarkup("div", "row-client", entry, true)
        + `<div class="row-browser">${escapeHtml(browserFor(entry))}</div>`
        + `<div class="row-cap" aria-hidden="true">${capability}</div>`
        + `<div class="row-dur${readinessNeedsAttention(entry) ? " unsettled" : ""}">${said}</div>`
        + `<div class="row-when" aria-hidden="true">${escapeHtml(entry.endedAt ? ago(entry.endedAt) : "")}</div>`
        + `<div class="row-history action-details" id="${escapeHtml(detailsId)}"${open ? "" : " hidden"}>${heroMarkup(entry)}</div>`;
    }

    /** A row's agent and age as its narrow second line shows them: "Claude Code, 4m". */
    function byShown(entry) {
      return rowBy(entry.agent?.name, entry.endedAt ? ago(entry.endedAt) : "");
    }

    /** A row's agent and age as a screen reader hears them on its button: ", Claude Code, 4m ago". */
    function bySaid(entry) {
      const said = rowBy(entry.agent?.name, entry.endedAt ? agoSaid(entry.endedAt) : "");
      return said ? `, ${said}` : "";
    }

    function rowClass(entry) {
      let name = `row ${capabilityClass(entry)}`;
      if (isRunning(entry)) name += " running";
      if (isPending(entry)) name += ` ${entry.phase}`;
      if (isBlocked(entry)) name += " blocked";
      const outcome = outcomeClass(entry);
      if (outcome) name += ` ${outcome}`;
      return name;
    }

    function buildRow(entry, landing) {
      const node = document.createElement("div");
      node.className = rowClass(entry) + (landing ? " landing" : "");
      node.innerHTML = rowMarkup(entry);
      if (landing) {
        node.addEventListener("animationend", () => node.classList.remove("landing"), { once: true });
      }
      rowNodes.set(entry.invocation, node);
      return node;
    }

    function row(entry) {
      const node = rowNodes.get(entry.invocation);
      if (!node) return;
      node.className = rowClass(entry);
      replaceHistoryMarkup(node, rowMarkup(entry));
      say(entry);
    }

    function drop(entry) {
      rowNodes.get(entry.invocation)?.remove();
      rowNodes.delete(entry.invocation);
      spoken.delete(entry.invocation);
      for (const key of expandedHistory) {
        if (key.startsWith(`${entry.invocation}:`)) expandedHistory.delete(key);
      }
    }

    /*
     * The end of a list too long to show whole. The list holds FEED_LIMIT actions and the service
     * holds more; search reaches every one, so the list ends by saying so, and opens the palette.
     */
    let listed = { full: false, settled: 0 };
    let recorded = 0;
    const queueMore = document.createElement("button");
    queueMore.className = "queue-more";
    queueMore.type = "button";
    queueMore.hidden = true;
    queueMore.dataset.openSearch = "";
    queueMore.setAttribute("aria-keyshortcuts", "Control+K");

    function queueCount(feed) {
      const rows = Math.max(0, feed.length - 1);
      el["queue-count"].textContent = rows ? `${rows} ${rows === 1 ? "action" : "actions"}` : "";
      el["clear-monitor"].disabled = !feed.some((entry) => !isUnsettled(entry));
      listed = { full: feed.length >= FEED_LIMIT, settled: feed.filter((entry) => !isUnsettled(entry)).length };
      // Rows arrive at the top, so the end stays last and keeps focus; a rebuild empties the list,
      // and only then is it put back.
      if (el.queue.lastElementChild !== queueMore) el.queue.append(queueMore);
      paintQueueMore();
    }

    function paintQueueMore() {
      const capped = listed.full && recorded > listed.settled;
      queueMore.hidden = !capped;
      queueMore.textContent = capped ? searchAll(recorded) : "";
    }

    /**
     * Show the action `invocation` where the list holds it: its row's details open, in view, with
     * focus on its button. Answers whether the list holds it; the running card shows its own.
     */
    function showAction(invocation) {
      const node = rowNodes.get(invocation);
      const button = node?.querySelector("[data-action-details]");
      if (!button) return false;
      showDetails(button, true);
      node.scrollIntoView?.({ block: "nearest" });
      button.focus();
      return true;
    }

    /** Full rebuild. Only used on first paint and after a detected sequence gap. */
    function rebuildFeed(feed) {
      el.queue.replaceChildren();
      rowNodes.clear();
      for (const key of expandedHistory) {
        if (!feed.some((entry) => key.startsWith(`${entry.invocation}:`))) expandedHistory.delete(key);
      }
      // What the feed already holds settled is history, and is not said.
      spoken.clear();
      for (const entry of feed) if (entry.settled) spoken.add(entry.invocation);
      paintHero(feed[0], false);
      const rows = document.createDocumentFragment();
      for (const entry of feed.slice(1)) rows.append(buildRow(entry, false));
      el.queue.append(rows);
      if (feed.length <= 1) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = "No other activity in this session.";
        el.queue.append(empty);
      }
      queueCount(feed);
    }

    /**
     * The conveyor: the new action takes the card and the one before lands as the top row. Both move
     * only when the card it replaces had time to settle; a burst is replaced in place.
     */
    function promote(entry, previous, feed) {
      const calm = now() - heroSince >= SWAP_DWELL_MS;
      heroSince = now();
      if (previous) {
        el.queue.querySelector(".empty")?.remove();
        el.queue.prepend(buildRow(previous, calm));
      }
      hero(entry, calm);
      queueCount(feed);
    }

    /* --------------------------------- band ------------------------------- */

    // The answer and its words are the orchestrator's, not this window's. A surface that classifies
    // its own readiness is a second source of truth about the one thing that must have exactly one,
    // which is the same rule the Policy chip below already follows.
    function band(facts) {
      lastFacts = facts;
      if (closedMode) {
        closedBand();
        return;
      }
      const readiness = facts.snapshot?.readiness;
      const name = lost || !readiness ? "runtime-offline" : `runtime-${readiness.tone}`;
      runtimeClass(name);
      // While the live stream is being opened again the page cannot vouch for what it shows, and
      // once Quit is accepted it says that instead.
      el["state-word"].textContent = farewell ?? (lost ? RECONNECTING : readiness ? readiness.word : "");
      // Where a pause came from is the better thing to read on the word while there is one.
      el["state-word"].title = facts.snapshot?.runtime_note ?? (readiness ? readiness.detail : "");

      if (!facts.snapshot) {
        el["state-facts"].textContent = "";
      } else {
        // Each fact is a whole of its own, its separator inside it, so the band drops whole facts as
        // it narrows and never cuts one. The agents are the ones At a glance shows, grouped the same
        // way, so the band and the pill agree. Only running work counts as running; what waits or is
        // held is counted under its own word.
        const told = bandFacts({
          agents: agentGroups(facts.snapshot.sessions).length,
          browsers: facts.snapshot.browsers.length,
          work: workCounts({ ...facts, paused: facts.runtime !== "active" }),
          recorded: facts.snapshot.history.length
        });
        recorded = facts.snapshot.history.length;
        paintQueueMore();
        el["state-facts"].innerHTML = told.map(([kind, parts], index) =>
          `<span class="fact fact-${kind}">${index ? " &middot; " : ""}`
          + parts.map(([count, word]) => `<b>${count}</b> ${escapeHtml(word)}`).join(", ")
          + "</span>").join("");
      }

      // The tab is called Policy and stays called Policy. What changes is the tone it carries and
      // the sentence behind it, and both are authored by the orchestrator: a surface that invents
      // its own policy words is a second source of truth about the one thing that must have one.
      const policy = facts.snapshot?.configuration?.policy;
      if (policy) {
        el["policy-state"].dataset.tone = policy.tone;
        el["policy-state"].title = policy.detail;
        el["policy-state"].setAttribute("aria-label", `Open Policy. ${policy.detail}`);
      }

      // The wheel does the first act the service offers in this state, from the controls part, so
      // it never offers what the state would refuse. The orchestrator offers runtime control in
      // every readiness state, browser or not. What it cannot say is that this window lost it: the
      // last snapshot outlives the connection, so a lost authority disables the wheel here rather
      // than letting a stale answer keep it enabled.
      const intent = facts.snapshot?.controls?.runtime?.offers?.[0] ?? null;
      el.wheel.disabled = lost || facts.connected === false || !(facts.snapshot?.readiness?.invites_control ?? false);
      el.wheel.dataset.intent = intent ?? "";
      el["wheel-label"].textContent = WHEEL_WORDS[intent] ?? "";
      el["wheel-icon"].innerHTML = intent === ACT.hold ? PAUSE_ICON : PLAY_ICON;

      // A screen that cannot press the wheel shows the state in its place, and where to change it.
      // Until the service has named an act, the band shows neither.
      const offered = intent !== null && can(intent);
      const withheld = intent !== null && !offered;
      el.wheel.hidden = !offered;
      el["wheel-state"].hidden = !withheld;
      el["band-continue"].hidden = !withheld;
      if (withheld) {
        el["wheel-state-word"].textContent = readiness ? readiness.word : "";
        el["wheel-state-note"].textContent = facts.snapshot?.runtime_note ?? "";
        el["band-continue"].innerHTML = continueMarkup(whyNot(intent));
      }
    }

    /** The band of a screen with nothing to show: no lamp state, no wheel, only its chip. */
    function closedBand() {
      runtimeClass("runtime-offline");
      el["state-word"].textContent = "";
      el["state-word"].title = "";
      el["state-facts"].textContent = "";
      el.wheel.hidden = true;
      el["wheel-state"].hidden = true;
      el["band-continue"].hidden = true;
    }

    /** The chip beside the state word: who can see this view, in the service's own words. */
    function chip(told) {
      el["context-chip"].textContent = told.word ?? "";
      el["context-chip"].title = told.sentence ?? "";
      el["context-chip"].hidden = !told.word;
    }

    /** Take what the service says this screen may do, and draw the controls to match. */
    function context(told) {
      access = told;
      closedMode = false;
      for (const panel of DATA_PANELS) {
        el[panel.line].hidden = true;
        el[panel.section].classList.remove("closed");
      }
      chip(told);
      el["refresh-integrations"].hidden = !can(ACT.rescan);
      el["setup-integrations"].hidden = !can(ACT.setupAll) || el["setup-integrations"].dataset.actionable !== "true";
      policyAccess();
    }

    /**
     * Draw Policy for what this screen may do with it. A screen that cannot write the rules shows
     * every draft control as the state it holds, read-only, under one line saying where changes
     * are made; it has no Apply, because it has nothing to apply.
     */
    function policyAccess() {
      const writes = can(ACT.applyPolicy);
      el["policy-continue"].hidden = writes;
      el["policy-continue"].innerHTML = writes ? "" : continueMarkup(POLICY_IN_WINDOW, addressOf("policy"));
      el["apply-policy"].hidden = !writes;
      el["sacred-hosts"].disabled = !writes;
      el["add-rule"].hidden = !(writes && applied?.user_layer?.editable);
      if (draft) renderRules();
    }

    /** Take the answer of a service that shows nothing: every data view says why, About shows in full. */
    function closed(told) {
      access = told;
      closedMode = true;
      chip(told);
      for (const panel of DATA_PANELS) {
        el[panel.line].textContent = told.sentence;
        el[panel.line].hidden = false;
        el[panel.section].classList.add("closed");
      }
      el["policy-state"].setAttribute("aria-label", "Open Policy.");
      closedBand();
      about({ version: told.product?.version, license: told.product?.license, counts: null });
    }

    /** Say that the live stream is being opened again, or that it is back. */
    function reconnecting(on) {
      lost = on || farewell !== null;
      if (!closedMode) band(lastFacts ?? { connected: false, snapshot: null, runtime: "active", running: 0 });
    }

    /** Say that Ghostlight is quitting: the band stops vouching for anything and says so, for good. */
    function leaving(sentence) {
      farewell = sentence;
      lost = true;
      if (!closedMode) band(lastFacts ?? { connected: false, snapshot: null, runtime: "active", running: 0 });
    }

    /* ----------------------------- collections ---------------------------- */

    /*
     * At a glance: one summary of who is connected and where they work.
     *
     * Agents are grouped by the agent id the service gives them, not per connection: an editor that
     * starts a session per request, or a shell running `ghostlight call` in a loop, is one agent
     * the user recognizes. The summary opens into two shelves, agents and browsers, and an agent
     * on its shelf opens its card, one line per session. The sessions themselves stay separate
     * everywhere that matters: each keeps its own workspace, tabs and attribution.
     */
    let glanceAgent = null;
    let lastConnections = null;

    /**
     * The agents among `sessions`, each with the agent the service names and its sessions, most
     * tabbed first. Sessions are one agent's when the service gives them one agent id: every host
     * of a roster agent shares one, and sessions whose client reported no name share the unknown
     * agent's.
     */
    function agentGroups(sessions) {
      const groups = new Map();
      for (const session of sessions) {
        const agent = session.agent;
        const group = groups.get(agent.id) ?? { id: agent.id, name: agent.name, agent, sessions: [], tabs: 0, busy: false };
        group.sessions.push(session);
        group.tabs += session.tab_count ?? 0;
        group.busy = group.busy || session.running_operations > 0;
        groups.set(agent.id, group);
      }
      return [...groups.values()]
        .sort((left, right) => right.tabs - left.tabs || left.name.localeCompare(right.name));
    }

    /** The face an agent wears in the closed summary: its symbol, ringed in the accent while it works. */
    function faceMarkup(group) {
      return `<span class="glance-face${group.busy ? " busy" : ""}" title="${escapeHtml(group.name)}">`
        + `${symbolMarkup(group.agent, "face")}</span>`;
    }

    /** The closed summary: who is connected, toward how many browsers, and the way to the details. */
    function pillMarkup(groups, browsers) {
      const faces = groups.slice(0, GLANCE_FACES).map(faceMarkup).join("");
      const hidden = groups.length - GLANCE_FACES;
      const rest = hidden > 0 ? `<span class="glance-face glance-rest">+${hidden}</span>` : "";
      const stack = groups.length ? `<span class="glance-faces" aria-hidden="true">${faces}${rest}</span>` : "";
      return '<summary class="glance-pill">'
        + `<span class="glance-side">${stack}${escapeHtml(agentsCount(groups.length))}</span>`
        + `<span class="glance-toward">${KIND_GLYPHS.toward}</span>`
        + `<span class="glance-side"><span class="dot${browsers.length ? " on" : ""}"></span>${escapeHtml(browsersCount(browsers.length))}</span>`
        + `<span class="glance-open disclosure"><span class="glance-closed-word">${escapeHtml(GLANCE.details)}</span>`
        + `<span class="glance-open-word">${escapeHtml(GLANCE.hide)}</span>${CHEVRON}</span>`
        + "</summary>";
    }

    /** One agent on its shelf: a button that opens its card. */
    function agentChipMarkup(group) {
      const open = glanceAgent === group.id;
      const many = group.sessions.length > 1 ? ` <span class="tally">${group.sessions.length}</span>` : "";
      return `<button class="chip glance-chip ${group.busy ? "busy" : "on"}" type="button"`
        + ` data-glance-agent="${escapeHtml(group.id)}" aria-expanded="${open}" aria-controls="${GLANCE_CARD_ID}">`
        + `${symbolMarkup(group.agent, "chip")}${escapeHtml(group.name)}${many}</button>`;
    }

    /** One browser on its shelf, with the tabs its sessions hold. */
    function browserChipMarkup(browser) {
      const version = browser.adapter_version ? ` title="${escapeHtml(browserChipTitle(browser.adapter_version))}"` : "";
      return `<span class="chip glance-chip on"${version}>${KIND_GLYPHS.browser}${escapeHtml(browser.family)}`
        + `<small>${escapeHtml(tabsCount(browser.tab_count ?? 0))}</small></span>`;
    }

    /** The chip a shelf shows when it holds nothing, saying so. */
    const missingMarkup = (text) => `<span class="chip glance-chip missing"><span class="dot"></span>${escapeHtml(text)}</span>`;

    /** The two shelves: agents, then the browsers they work in. */
    function shelvesMarkup(groups, browsers, noBrowser) {
      const agents = groups.length ? groups.map(agentChipMarkup).join("") : missingMarkup(GLANCE.noAgent);
      const places = browsers.length ? browsers.map(browserChipMarkup).join("") : missingMarkup(noBrowser ?? "");
      return '<div class="glance-shelves">'
        + `<div class="glance-shelf" role="group" aria-label="${escapeHtml(GLANCE.agents)}"><span class="kicker">${escapeHtml(GLANCE.agents)}</span>${agents}</div>`
        + '<span class="glance-rule" aria-hidden="true"></span>'
        + `<div class="glance-shelf" role="group" aria-label="${escapeHtml(GLANCE.browsers)}"><span class="kicker">${escapeHtml(GLANCE.browsers)}</span>${places}</div>`
        + "</div>";
    }

    /**
     * The open agent's card: one line per session, working ones first, at most a few, each with
     * its connection ids behind Copy id, and what Ghostlight can and cannot vouch for, said once.
     */
    function cardMarkup(group) {
      const sessions = [...group.sessions]
        .sort((left, right) => (right.running_operations > 0) - (left.running_operations > 0)
          || (right.tab_count ?? 0) - (left.tab_count ?? 0));
      const lines = sessions.slice(0, GLANCE_CARD_LINES).map((session) => {
        const ids = (session.connections ?? []).map((connection) => connection.connection_id).filter(Boolean);
        const copy = ids.length
          ? `<button class="link-button" type="button" data-copy-connection="${escapeHtml(ids.join("\n"))}" title="${escapeHtml(ids.join(" "))}">${escapeHtml(GLANCE.copyId)}</button>`
          : "";
        return `<li class="glance-line"><span>${escapeHtml(sessionLine(session))}</span>${copy}</li>`;
      }).join("");
      const more = sessions.length > GLANCE_CARD_LINES
        ? `<p class="glance-more">${escapeHtml(andMore(sessions.length - GLANCE_CARD_LINES))}</p>` : "";
      const notes = [...new Set(sessions.flatMap((session) => (session.connections ?? []).map((connection) => connection.explanation)))]
        .filter(Boolean).map((note) => `<p class="connection-explanation">${escapeHtml(note)}</p>`).join("");
      return `<section class="glance-card" id="${GLANCE_CARD_ID}" aria-label="${escapeHtml(group.name)}">`
        + `<header class="glance-card-head${group.busy ? " busy" : ""}">${symbolMarkup(group.agent, "card")}<h3>${escapeHtml(group.name)}</h3>`
        + `<small>${escapeHtml(sessionsCount(group.sessions.length))}</small></header>`
        + `<ul class="glance-lines">${lines}</ul>${more}${notes}</section>`;
    }

    function paintConnections(facts) {
      ifChanged("connections", [facts, glanceAgent], () => {
        const groups = agentGroups(facts.sessions);
        const open = groups.find((group) => group.id === glanceAgent);
        const body = shelvesMarkup(groups, facts.browsers, facts.no_browser) + (open ? cardMarkup(open) : "");
        const markup = `<details class="glance" data-page-details="${GLANCE_KEY}"${openPage.has(GLANCE_KEY) ? " open" : ""}>`
          + pillMarkup(groups, facts.browsers) + body + "</details>";
        replaceHistoryMarkup(el.connections, markup);
      });
    }

    function connections(snapshot) {
      lastConnections = { sessions: snapshot.sessions, browsers: snapshot.browsers, no_browser: snapshot.no_browser };
      paintConnections(lastConnections);
    }

    function links() {
      el["about-links"].innerHTML = DESTINATIONS.map(([heading, rows]) => {
        const items = rows.map(([destination, title, blurb]) =>
          `<button class="about-link" type="button" data-destination="${escapeHtml(destination)}">`
          + `<span class="about-link-title">${escapeHtml(title)}</span>`
          + `<span class="about-link-blurb">${escapeHtml(blurb)}</span></button>`).join("");
        return `<section class="about-group"><h2>${escapeHtml(heading)}</h2><div>${items}</div></section>`;
      }).join("");
    }

    /**
     * The About page. The product's version and license come from the service; the three counts
     * come from data, so a screen that is shown no data passes none.
     */
    function about({ version, license, counts }) {
      const exact = String(version ?? "");
      // The disc wears the short form the portfolio card wears; the exact build is in the facts.
      el["about-version"].textContent = exact.split(".").slice(0, 2).join(".") || "1.0";
      const facts = [["Version", exact || "unknown"]];
      if (counts) {
        facts.push(
          ["Agents", `${counts.agents} connected`],
          ["Browsers", `${counts.browsers} attached`],
          ["Recorded", `${counts.recorded} actions on this device`]
        );
      }
      if (license) facts.push(["License", license]);
      el["about-facts"].innerHTML = facts
        .map(([term, value]) => `<dt>${escapeHtml(term)}</dt><dd>${escapeHtml(value)}</dd>`)
        .join("");
    }

    /*
     * Integrations: every agent Ghostlight knows, on a rail grouped by state, and the one the
     * person picked shown in full beside it.
     *
     * Every fact comes from the service's roster: the state, what each act may do, the settings
     * file, the entry and the evidence. Which agents are in use is the service's word too
     * (`harnesses_in_use`, the rule the setup view reads); the page counts an in-use agent's
     * sessions and tabs for its words, and whether one of them is running work now, which is the
     * only time the agent wears the accent. It decides nothing.
     * The pick is kept by id across repaints, so an agent that moves group when someone acts on it
     * stays in the pane.
     */
    let agentPick = null;
    let foldOpen = false;
    let agentQuery = "";
    let lastAgents = null;
    let paneShows = null;

    /** The file an agent reads, from its settings path. */
    const fileOf = (path) => String(path ?? "").split(/[\\/]/).filter(Boolean).at(-1) ?? "";

    /**
     * The live sessions of `harness`'s agent, those with its agent id, the tabs they hold, and
     * whether any of them is running work now.
     */
    function useOf(harness, sessions) {
      const own = sessions.filter((session) => session.agent.id === harness.agent.id);
      return {
        sessions: own.length, tabs: own.reduce((sum, session) => sum + (session.tab_count ?? 0), 0),
        working: own.some((session) => session.running_operations > 0)
      };
    }

    /**
     * An agent as the page draws it: its roster row, the file it reads, whether the service says it
     * is in use (`inUseIds`, the rows it names), its live use and where it stands.
     */
    function agentOf(harness, sessions, inUseIds) {
      const used = inUseIds.includes(harness.id);
      return {
        ...harness, file: fileOf(harness.config_path), in_use: used, use: useOf(harness, sessions),
        standing: agentStanding(harness, used)
      };
    }

    const inUse = (agent) => agent.in_use;
    /** Whether an agent in use is running work now: the one state that wears the accent. */
    const working = (agent) => inUse(agent) && agent.use.working;
    const toneOf = (agent) => (working(agent) ? WORKING_TONE : agent.standing);
    const byLabel = (left, right) => left.label.localeCompare(right.label, undefined, { sensitivity: "base" });

    /** The rail's groups that hold an agent, each sorted the way a person scans it. */
    function railGroups(agents) {
      return AGENT_GROUPS.map((group) => {
        const members = agents.filter((agent) => group.states.includes(agent.standing));
        if (group.key === "look") {
          members.sort((left, right) => group.states.indexOf(left.state) - group.states.indexOf(right.state) || byLabel(left, right));
        } else if (group.key === "connected") {
          members.sort((left, right) => right.use.tabs - left.use.tabs || right.use.sessions - left.use.sessions || byLabel(left, right));
        } else {
          members.sort(byLabel);
        }
        return { ...group, agents: members };
      }).filter((group) => group.agents.length);
    }

    /**
     * The agent the page opens on: one that needs a look, then one found, then one with tabs, then
     * the first connected, then one set up by hand.
     */
    function mostRelevant(groups) {
      const members = (key) => groups.find((group) => group.key === key)?.agents ?? [];
      const connected = members("connected");
      return (members("look")[0] ?? members("found")[0] ?? connected.find((agent) => agent.use.tabs > 0)
        ?? connected[0] ?? members(BY_HAND)[0] ?? members(ABSENT_GROUP)[0])?.id ?? null;
    }

    /** What this screen may do for `agent`, from the roster's flags and what the screen was told. */
    function actsOf(agent) {
      const manage = can(ACT.harnessAct);
      const absent = agent.state === "not_detected";
      return {
        install: absent && agent.can_download && (!opensLinks() || Boolean(agent.download_url)),
        prepare: manage && absent && agent.can_install,
        connect: manage && agent.state === "available" && agent.can_install,
        update: manage && agent.state === "updatable" && agent.can_install,
        fix: manage && agent.state === "needs_attention" && agent.can_fix,
        remove: manage && agent.state === "installed" && agent.can_uninstall,
        locate: can(ACT.locate) && agent.can_locate,
        recheck: can(ACT.rescan) && agent.state === "needs_attention",
        copy: true
      };
    }

    /** The action each act asks of the service's client roster. */
    const MANAGE_ACTION = Object.freeze({ prepare: "install", connect: "install", update: "install", fix: "fix" });

    /** One button of the pane, wired to the act it names. */
    function actButton(act, agent, primary) {
      const kind = primary ? "primary-button" : "ghost-button";
      const label = escapeHtml(AGENT_ACTS[act]);
      const who = ` data-harness="${escapeHtml(agent.id)}" data-harness-name="${escapeHtml(agent.name)}"`;
      if (MANAGE_ACTION[act]) {
        const waiting = lastAgents.pending.has(agent.id);
        return `<button class="${kind}" type="button" data-harness-operation="manage"`
          + ` data-harness-action="${MANAGE_ACTION[act]}"${who} data-harness-state="${escapeHtml(agent.state)}"`
          + ` data-harness-file="${escapeHtml(agent.file)}"${waiting ? " disabled" : ""}>`
          + `${waiting ? escapeHtml(AGENTS.working) : label}</button>`;
      }
      if (act === "install") {
        // A tab follows the product's own link; the window asks the service to open it.
        return opensLinks()
          ? `<a class="${kind}" href="${escapeHtml(agent.download_url)}" target="_blank" rel="noopener noreferrer">${label}</a>`
          : `<button class="${kind}" type="button" data-harness-operation="download"${who}>${label}</button>`;
      }
      if (act === "locate") return `<button class="${kind}" type="button" data-harness-operation="locate"${who}>${label}</button>`;
      if (act === "copy") {
        return `<button class="${kind}" type="button" data-harness-operation="copy" data-copy-kind="${COPY_KIND.setup}"${who}>${label}</button>`;
      }
      return `<button class="${kind}" type="button" data-agents-recheck>${label}</button>`;
    }

    /** The second line of a rail entry, only when the agent has something of its own to say. */
    function subOf(agent) {
      if (working(agent)) return { tone: WORKING_TONE, text: agentUse(agent.use) };
      if (inUse(agent)) return { tone: "", text: agentUse(agent.use) };
      if (agent.state === "needs_attention") return { tone: "warn", text: AGENT_STATE_WORDS.needs_attention };
      if (agent.state === "updatable") return { tone: "offer", text: AGENT_STATE_WORDS.updatable };
      // An agent set up by hand sits under a heading that already says so.
      if (agent.copy_only && agent.standing === "not_detected") return { tone: "", text: AGENTS.copyOnly };
      return null;
    }

    function entryMarkup(agent) {
      const sub = subOf(agent);
      const second = sub
        ? `<span class="agent-entry-sub${sub.tone ? ` ${sub.tone}` : ""}">${escapeHtml(sub.text)}</span>`
        : `<span class="sr-only">, ${escapeHtml(agentStateWord(toneOf(agent)))}</span>`;
      return `<button class="agent-entry e-${escapeHtml(agent.standing)}" type="button" data-agent="${escapeHtml(agent.id)}"`
        + ` aria-controls="agents-pane" aria-current="${agent.id === agentPick}">`
        + `<span class="agent-dot d-${escapeHtml(toneOf(agent))}" aria-hidden="true"></span>`
        + symbolMarkup(agent.agent, "rail")
        + `<span class="agent-entry-text"><span class="agent-entry-name">${escapeHtml(agent.label)}</span>${second}</span></button>`;
    }

    /** The one row every agent not on this computer folds into, which opens in place. */
    function foldMarkup(group) {
      const count = group.agents.length;
      return `<div class="agents-group g-${ABSENT_GROUP}">`
        + `<button class="agents-fold" type="button" data-agent-fold aria-expanded="${foldOpen}" aria-controls="agents-absent">`
        + `<span class="agent-dot d-not_detected" aria-hidden="true"></span>`
        + `<span class="agent-entry-text"><span class="agent-entry-name"><b>${count}</b> ${escapeHtml(moreAgents(count))}</span>`
        + `<span class="agent-entry-sub">${escapeHtml(group.title)}</span></span>${CHEVRON}</button>`
        + `<div class="agents-fold-list" id="agents-absent"${foldOpen ? "" : " hidden"}>${group.agents.map(entryMarkup).join("")}</div></div>`;
    }

    function railMarkup(groups) {
      const query = agentQuery.trim().toLowerCase();
      const html = groups.map((group) => {
        const shown = query
          ? group.agents.filter((agent) => `${agent.label} ${agent.name}`.toLowerCase().includes(query)) : group.agents;
        if (!shown.length) return "";
        // While a search runs, the folded agents it reaches show under their own heading.
        if (group.key === ABSENT_GROUP && !query) return foldMarkup(group);
        return `<div class="agents-group g-${group.key}"><h2 class="agents-group-head"><span>${escapeHtml(group.title)}</span>`
          + `<span class="agents-count">${shown.length}</span></h2>${shown.map(entryMarkup).join("")}</div>`;
      }).join("");
      return html || `<p class="agents-empty">${escapeHtml(noAgentCalled(agentQuery.trim()))}</p>`;
    }

    /**
     * The agent's icon beside its name and its state, the dot and the word together, so the state
     * is never colour alone; and under the state, when the agent wears its app's own icon, the
     * service's line that says which app it came from. The icon is decoration.
     */
    function agentHeroMarkup(agent) {
      const tone = toneOf(agent);
      const about = symbolAbout(agent.agent);
      return `<header class="agent-hero s-${escapeHtml(tone)}">`
        + symbolMarkup(agent.agent, "pane")
        + `<div class="agent-hero-text"><h2 id="agents-pane-title">${escapeHtml(agent.label)}</h2>`
        + `<div class="agent-state-line"><span class="agent-dot d-${escapeHtml(tone)}" aria-hidden="true"></span>`
        + `<span class="agent-state t-${escapeHtml(tone)}">${escapeHtml(agentStateWord(tone))}</span></div>`
        + (about ? `<p class="agent-icon-about">${escapeHtml(about)}</p>` : "")
        + "</div></header>";
    }

    const actRow = (act, agent, primary) => `<div class="agent-act">${actButton(act, agent, primary)}`
      + `<span class="agent-why">${escapeHtml(agentWhy(act, agent))}</span></div>`;

    /**
     * The numbered steps for an agent that needs attention Ghostlight cannot fix, from what the
     * service observed: with evidence, an entry it found in the way, which the right entry replaces;
     * without, a file it could not read or parse, which only the person can correct. The repair
     * step has no button.
     */
    function stepsOf(agent, acts) {
      const steps = agent.evidence
        ? [{ act: "copy", what: AGENTS.copyStep, primary: true }]
        : [{ act: "repair", what: AGENTS.repairStep, primary: false }];
      // Re-check is never the view's primary: the heading's Set up everything is.
      steps.push({ act: "recheck", what: AGENTS.recheckStep, primary: false });
      return steps.filter(({ act }) => act === "repair" || acts[act]);
    }

    /** The next step: each button the agent's state allows, with what it does beside it. */
    function nextMarkup(agent, acts, used) {
      if (agent.state === "needs_attention" && !acts.fix) {
        const steps = stepsOf(agent, acts);
        for (const { act } of steps) used.add(act);
        return `<ol class="agent-steps" aria-label="${escapeHtml(AGENTS.fixSteps)}">`
          + steps.map(({ act, what, primary }) => `<li><span class="agent-step-what">${escapeHtml(what)}`
            + `<small>${escapeHtml(agentWhy(act, agent))}</small></span>`
            + `${act === "repair" ? "" : actButton(act, agent, primary)}</li>`).join("")
          + "</ol>";
      }
      const order = ["install", "connect", "update", "prepare", "fix"];
      if (agent.copy_only && agent.standing !== "installed") order.push("copy");
      const rows = order.filter((act) => acts[act]).map((act, index) => {
        used.add(act);
        return actRow(act, agent, index === 0);
      }).join("");
      return rows ? `<div class="agent-next">${rows}</div>` : "";
    }

    /** A path that breaks after its separators, so it wraps between folders. */
    const breakable = (text) => escapeHtml(text).replace(/([\\/]+)/g, "$1<wbr>");

    /**
     * The service's sentence with each program and path it marks in backticks set as code, the
     * marks themselves left out. The words are the service's, unchanged.
     */
    const codeSpans = (text) => String(text ?? "").split("`").map((part, index) =>
      (index % 2 ? `<code class="agent-code">${breakable(part)}</code>` : escapeHtml(part))).join("");

    /** The entry, whole, one block per line, each line's indent held by a class. */
    function docMarkup(doc) {
      return String(doc ?? "").split("\n").map((line) => {
        const level = Math.min(6, Math.floor(/^ */.exec(line)[0].length / 2));
        return `<span class="ln i${level}">${line ? breakable(line) : "<br>"}</span>`;
      }).join("");
    }

    function factsMarkup(agent, acts, used, prepare) {
      const locate = acts.locate && !used.has("locate");
      const copy = acts.copy && !used.has("copy");
      const copies = agentCopyNote(agent);
      return '<div class="agent-facts">'
        + `<p class="agent-facts-note">${escapeHtml(AGENTS.factsNote)}</p>`
        + `<div class="agent-fact"><span class="agent-fact-label">${escapeHtml(AGENTS.settingsFile)}<small>${escapeHtml(agentFileLabel(agent))}</small></span>`
        + `<div><div class="agent-path">${breakable(agent.config_path ?? "")}</div>`
        + (copies ? `<div class="agent-why agent-copies">${breakable(copies)}</div>` : "")
        + (locate ? `<div class="agent-why">${escapeHtml(agentWhy("locate", agent))}</div>` : "") + "</div>"
        + (locate ? actButton("locate", agent, false) : "<span></span>") + "</div>"
        + `<div class="agent-fact"><span class="agent-fact-label">${escapeHtml(AGENTS.entry)}<small>${escapeHtml(agentEntryLabel(agent, { prepare }))}</small></span>`
        + `<div><pre class="agent-doc">${docMarkup(agent.manual_setup)}</pre>`
        + (copy ? `<div class="agent-why">${escapeHtml(agentWhy("copy", agent))}</div>` : "") + "</div>"
        + (copy ? actButton("copy", agent, false) : "<span></span>") + "</div>"
        + "</div>";
    }

    function paneMarkup(agent, arriving) {
      const acts = actsOf(agent);
      const used = new Set();
      let html = agentHeroMarkup(agent);
      html += `<p class="agent-lead">${escapeHtml(agentLead(agent, { prepare: acts.prepare }))}</p>`;
      // ADR-0135: the orchestrator precomposed this proof for a blocked agent; it is rendered
      // verbatim and the page adds no words of its own to it.
      if (agent.evidence) {
        html += `<div class="agent-evidence"><span class="kicker">${escapeHtml(AGENTS.evidence)}</span>`
          + `<p>${codeSpans(agent.evidence)}</p></div>`;
      }
      if (inUse(agent)) {
        const tone = toneOf(agent);
        html += `<div class="agent-use${working(agent) ? ` ${WORKING_TONE}` : ""}">`
          + `<span class="agent-dot d-${escapeHtml(tone)}" aria-hidden="true"></span>`
          + `<span>${escapeHtml(agentLive(agent.name, agent.use))}</span>`
          + `<button class="link-button" type="button" data-view="monitor" data-show-agent="${escapeHtml(agent.agent.id)}">`
          + `${escapeHtml(AGENTS.showSessions)}</button></div>`;
      }
      html += nextMarkup(agent, acts, used);
      html += factsMarkup(agent, acts, used, acts.prepare);
      if (acts.remove) {
        html += `<div class="agent-quiet"><p>${escapeHtml(removeNote(agent.file))}</p>`
          + `<button class="agent-remove" type="button" data-harness-operation="manage" data-harness-action="uninstall"`
          + ` data-harness="${escapeHtml(agent.id)}" data-harness-name="${escapeHtml(agent.name)}"`
          + ` data-harness-state="${escapeHtml(agent.state)}" data-harness-file="${escapeHtml(agent.file)}"`
          + ` data-harness-sessions="${agent.use.sessions}"${lastAgents.pending.has(agent.id) ? " disabled" : ""}>`
          + `${escapeHtml(removeLabel(agent.name))}</button></div>`;
      }
      // Only a newly picked agent arrives; a repaint of the same one stays still.
      return `<div class="agent-pane-in${arriving ? " arriving" : ""}">${html}</div>`;
    }

    /** The headline, what Set up everything will do, and the one command every agent runs. */
    function agentsHeading(agents) {
      const headline = agentHeadline(agents);
      el["integrations-heading"].innerHTML = `${escapeHtml(headline.lead)}<em>${escapeHtml(headline.accent)}</em>`
        + escapeHtml(headline.trail);
      const plan = {
        connect: agents.filter((agent) => agent.can_install && agent.state === "available").map((agent) => agent.label),
        update: agents.filter((agent) => agent.can_install && agent.state === "updatable").map((agent) => agent.label),
        left: agents.filter((agent) => agent.state === "needs_attention" || agent.standing === BY_HAND)
          .map((agent) => agent.label)
      };
      // Set up everything is offered only while it would do something: a primary that can do
      // nothing is not shown, and neither is the caption that says what it would do.
      const actionable = plan.connect.length + plan.update.length > 0;
      const offered = actionable && can(ACT.setupAll);
      const setup = el["setup-integrations"];
      setup.dataset.actionable = String(actionable);
      setup.hidden = !offered;
      const said = agentPlan(plan);
      setup.title = said;
      el["integrations-caption"].textContent = said;
      el["integrations-caption"].hidden = !offered;
      const command = agents[0]?.connector_command ?? "";
      el["integrations-command"].textContent = command;
      el["integrations-command"].title = command;
      el["integrations-command"].hidden = !command;
      el["copy-command"].hidden = !command;
      el["copy-command"].dataset.harness = agents[0]?.id ?? "";
      el["copy-command"].dataset.harnessName = agents[0]?.name ?? "";
    }

    function drawAgents() {
      drawnOnce.add("integrations");
      const { harnesses, sessions, inUseIds } = lastAgents;
      const agents = harnesses.map((harness) => agentOf(harness, sessions, inUseIds));
      agentsHeading(agents);
      // One way on for the whole page while this screen cannot make a change: the controls it
      // leaves out already say that the change is made elsewhere.
      const withheld = [ACT.harnessAct, ACT.locate, ACT.setupAll].some((act) => !can(act));
      el["integrations-continue"].hidden = !withheld;
      el["integrations-continue"].innerHTML = withheld ? continueMarkup("", addressOf("integrations")) : "";
      if (!agents.length) {
        el["agents-list"].innerHTML = `<p class="agents-empty">${escapeHtml(AGENTS.noAgents)}</p>`;
        el["agents-pane"].innerHTML = "";
        return;
      }
      const groups = railGroups(agents);
      if (!agents.some((agent) => agent.id === agentPick)) {
        agentPick = mostRelevant(groups);
        if (agents.find((agent) => agent.id === agentPick)?.standing === "not_detected") foldOpen = true;
      }
      replaceHistoryMarkup(el["agents-list"], railMarkup(groups), () => el["main-content"]);
      const picked = agents.find((agent) => agent.id === agentPick);
      const arriving = paneShows !== agentPick;
      // A control the pane redraws away, such as Remove once it is pressed, hands focus to the
      // agent's entry in the rail, which every redraw keeps.
      const entry = () => [...el["agents-list"].querySelectorAll("[data-agent]")]
        .find((found) => found.dataset.agent === agentPick) ?? el["main-content"];
      replaceHistoryMarkup(el["agents-pane"], picked ? paneMarkup(picked, arriving) : "", entry);
      if (arriving) el["agents-pane"].scrollTop = 0;
      paneShows = agentPick;
    }

    /** Paint the page when its facts or the person's place on it changed. */
    function paintAgents() {
      if (!lastAgents) return;
      ifChanged("agents", [lastAgents.facts, agentPick, foldOpen, agentQuery], drawAgents);
    }

    function integrations(snapshot, pending) {
      const harnesses = snapshot.harnesses;
      const sessions = snapshot.sessions ?? [];
      const inUseIds = snapshot.harnesses_in_use ?? [];
      const uses = harnesses.map((harness) => useOf(harness, sessions));
      lastAgents = { harnesses, sessions, inUseIds, pending, facts: [harnesses, uses, inUseIds, [...pending], access?.acts] };
      paintAgents();
    }

    /** Whether the agent `id` sits in the fold of agents not on this computer. */
    function folded(id) {
      const harness = lastAgents?.harnesses.find((row) => row.id === id);
      return Boolean(harness) && agentOf(harness, lastAgents.sessions, lastAgents.inUseIds).standing === "not_detected";
    }

    /** Show `id` in the pane, and open the fold when it sits there and no search is running. */
    function showAgent(id) {
      if (!lastAgents?.harnesses.some((harness) => harness.id === id)) return;
      agentPick = id;
      if (folded(id) && !agentQuery.trim()) foldOpen = true;
      paintAgents();
    }

    /** The rail entries a person can reach now, in rail order. */
    const reachableEntries = () => [...el["agents-list"].querySelectorAll(".agent-entry")]
      .filter((entry) => entry.offsetParent !== null);

    function findAgent(text) {
      agentQuery = text;
      if (!text.trim() && folded(agentPick)) foldOpen = true;
      paintAgents();
    }

    function toggleFold() {
      foldOpen = !foldOpen;
      paintAgents();
      // Bring the opened list into view, so the press visibly lands when the rail is nearly full.
      if (foldOpen) el["agents-list"].querySelector("#agents-absent")?.scrollIntoView?.({ block: "nearest" });
    }

    el["agents-find"]?.addEventListener("input", (event) => findAgent(event.target.value));
    el["agents-find"]?.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.target.value = "";
        findAgent("");
        return;
      }
      if (event.key !== "ArrowDown" && event.key !== "Enter") return;
      const first = reachableEntries()[0];
      if (!first) return;
      event.preventDefault();
      if (event.key === "ArrowDown") first.focus();
      showAgent(first.dataset.agent);
    });
    el["agents-list"]?.addEventListener("keydown", (event) => {
      const entries = reachableEntries();
      const at = entries.indexOf(document.activeElement);
      if (at < 0) return;
      const to = { ArrowDown: entries[Math.min(at + 1, entries.length - 1)], ArrowUp: entries[Math.max(at - 1, 0)],
        Home: entries[0], End: entries.at(-1) }[event.key];
      if (!to) return;
      event.preventDefault();
      to.focus();
      showAgent(to.dataset.agent);
    });
    document.addEventListener("keydown", (event) => {
      if (event.key !== "/" || current !== "integrations" || !el["confirm-dialog"].hidden || !el.palette.hidden) return;
      if (/^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName ?? "")) return;
      event.preventDefault();
      el["agents-find"].focus();
    });

    function status(snapshot) {
      drawnOnce.add("status");
      // The service's own sentence about what is connected, read from the controls part it keeps.
      const service = snapshot.controls?.service?.word ?? "";
      el["status-service"].textContent = service;
      el["status-service"].hidden = !service;
      el["diagnostic-grid"].innerHTML = snapshot.diagnostics.map((item) =>
        `<article class="card"><span class="severity ${escapeHtml(item.severity)}"><span class="dot"></span>`
        + `${escapeHtml(item.severity)}</span><h2>${escapeHtml(item.label)}</h2>`
        + `<p>${escapeHtml(item.detail)}</p></article>`).join("");

      const started = snapshot.service.started_at_ms
        ? new Date(snapshot.service.started_at_ms).toLocaleString()
        : "unknown";
      el.colophon.textContent =
        `Ghostlight ${snapshot.service.version} - running since ${started} - everything on this page stays on this device.`;
    }

    /* -------------------------------- policy -------------------------------- */

    /*
     * The one page that answers "what may agents do here, and who decided".
     *
     * Everything drawn below arrives compiled from the orchestrator. This function chooses shapes
     * and order; it never decides what is allowed, never composes a policy sentence, and never
     * computes a decision. The draft the editor holds is disposable view state and stays that way
     * until the person applies it.
     */
    let draft = null;
    let applied = null;

    /**
     * Draw the compiled policy.
     *
     * The draft is replaced by the applied rules unless the caller says to keep it, which a screen
     * does when it learns that the applied rules moved on while the person's edits were unsent.
     */
    function policy(view, { keepDraft = false } = {}) {
      applied = view;
      drawnOnce.add("policy");
      el["policy-headline"].textContent = view.headline;

      organizationCard(view.organization, view.passport);

      el["capability-board"].innerHTML = view.capabilities.map((line) => {
        // The badge states polarity, because "some sites" is true of both an open baseline with
        // holes cut in it and a closed one with holes opened, and they are opposite situations.
        const tone = CAPABILITY_TONE[line.state] ?? "no";
        const badge = CAPABILITY_BADGE[line.state] ?? "Not allowed";
        return `<article class="cap cap-${tone}">`
          + `<div class="cap-top"><h3>${escapeHtml(line.label)}</h3>`
          + `<span class="cap-state">${badge}</span></div>`
          + `<p class="cap-covers">${escapeHtml(line.covers)}</p>`
          + `<p class="cap-detail">${escapeHtml(line.detail)}</p></article>`;
      }).join("");

      el["policy-ceilings"].innerHTML = view.ceilings
        .map((line) => `<li>${escapeHtml(line)}</li>`).join("");

      settingsAndDocuments(view.layers);

      const user = view.user_layer;
      el["policy-editor"].hidden = !user.editable;
      el["add-rule"].hidden = !(user.editable && can(ACT.applyPolicy));
      el["policy-blocked"].hidden = user.editable || !user.blocked_reason;
      if (user.blocked_reason) el["policy-blocked-reason"].textContent = user.blocked_reason;
      el["policy-remove"].hidden = !(user.editable && user.source === "workbench" && hasUserLayer(view)
        && can(ACT.removePolicy));

      if (!user.editable) {
        draft = null;
        opened.clear();
      } else if (!(keepDraft && draft)) {
        draft = draftFrom(view);
        opened.clear();
      }
      renderRules();
    }

    function hasUserLayer(view) {
      return view.layers.some((layer) => layer.kind === "user");
    }

    function organizationCard(organization, passport) {
      const card = el["policy-organization"];
      if (!organization && !passport?.configured) {
        card.hidden = true;
        card.innerHTML = "";
        return;
      }
      const name = organization?.name || passport?.organization || "Your organization";
      const statement = organization?.statement || passport?.rationale;
      const contacts = (organization?.contacts?.length ? organization.contacts : passport?.contacts || [])
        .map((contact) => `<li><span>${escapeHtml(contact.label || words(contact.kind))}</span>${escapeHtml(contact.value)}</li>`)
        .join("");
      const provenance = [];
      if (passport?.configured) {
        provenance.push(words(passport.freshness));
        if (passport.sequence != null) provenance.push(`verified sequence ${passport.sequence}`);
        if (passport.source_class && passport.source_class !== "none") {
          provenance.push(passport.source_class === "https" ? "from an HTTPS source" : `from a ${words(passport.source_class)} source`);
        }
        if (passport.last_success_ms) {
          provenance.push(`last checked ${new Date(passport.last_success_ms).toLocaleString()}`);
        }
      }
      card.hidden = false;
      card.innerHTML = `<h2>${escapeHtml(name)}</h2>`
        + (statement ? `<p class="org-statement">${escapeHtml(statement)}</p>` : "")
        + (organization?.url ? `<p class="org-url">${escapeHtml(organization.url)}</p>` : "")
        + (contacts ? `<ul class="org-contacts">${contacts}</ul>` : "")
        + (provenance.length ? `<p class="org-provenance">${escapeHtml(provenance.join(", "))}.</p>` : "");
    }

    /**
     * The settings a layer authors, and the exact document behind every layer.
     *
     * Settings only appear when a layer actually sets one, the way a policy page that lists every
     * possible setting teaches nothing. The documents stay reachable so this page never becomes the
     * only way to read the policy.
     */
    function settingsAndDocuments(layers) {
      const settings = layers.flatMap((layer) =>
        layer.settings.map((setting) => ({ ...setting, owner: layer.title, kind: layer.kind })));
      el["policy-settings"].innerHTML = settings.length
        ? `<h2 class="subhead">Restrictions in force</h2><ul class="layer-settings">`
          + settings.map((setting) => {
            let value = setting.value;
            try {
              value = JSON.parse(setting.value);
            } catch (error) {
              // A value the orchestrator could not render as JSON is shown as it arrived.
            }
            // A screen that may not name the never-touch sites is told how many there are.
            const counted = setting.key === SACRED_KEY && typeof setting.count === "number";
            return `<li>${escapeHtml(counted ? neverTouchedCount(setting.count) : settingWords(setting.key, value))}`
              + ` <span class="owner">${escapeHtml(setting.owner)}</span></li>`;
          }).join("")
          + `</ul>`
          + limitNotes(settingLimits(settings.filter((setting) => setting.value === "false").map((setting) => setting.key)))
        : "";

      el["policy-documents"].innerHTML = layers.map((layer) => {
        if (!layer.document) return "";
        const where = layer.path ? `<span class="doc-path">${escapeHtml(layer.path)}</span>` : "";
        return `<details class="layer-doc"><summary class="disclosure">${escapeHtml(layer.title)}: `
          + `${escapeHtml(layer.policy_name)} ${escapeHtml(layer.version)}${CHEVRON}</summary>`
          + where + `<pre>${escapeHtml(layer.document)}</pre></details>`;
      }).join("");
    }

    /* ------------------------------ the editor ----------------------------- */

    /**
     * Turn the applied user layer into an editable draft, or start an empty one.
     *
     * Only a draft with no user layer behind it is `open`: while it has no rules, its document
     * carries OPEN_RULE, so a setting turned off applies alone and agents keep the ordinary
     * websites they have now. An applied layer is never open, even with no rules: a policy with no
     * grants refuses everything today, and its draft says so instead of widening it.
     */
    function draftFrom(view) {
      const layer = view.layers.find((entry) => entry.kind === "user");
      if (!layer) return { rules: [], settings: emptySettings(), dirty: false, open: true };
      return {
        dirty: false,
        open: false,
        rules: layer.rules.map((rule) => ({
          id: rule.id,
          hosts: rule.allow.join(", "),
          description: rule.description || "",
          allowed: new Set(rule.allowed)
        })),
        settings: settingsFrom(layer.settings)
      };
    }

    /** No opinion on anything, which is what an absent setting means. */
    function emptySettings() {
      return { restricted: new Set(), sacred: "", sacredCount: null, startup: null, choices: {} };
    }

    /**
     * Read authored settings back into the draft.
     *
     * A boolean restriction is present in the document or it is not; the permissive value never
     * appears. The browser-startup setting is different by design: it is a closed operational
     * choice, so its authored string is read back directly.
     */
    function settingsFrom(authored) {
      const settings = emptySettings();
      for (const setting of authored) {
        if (setting.key === SACRED_KEY) {
          settings.sacred = parseHostList(setting.value).join(", ");
          if (typeof setting.count === "number") settings.sacredCount = setting.count;
        } else if (setting.key === "browser.startup") {
          try {
            const value = JSON.parse(setting.value);
            if (value === "on_demand" || value === "manual") settings.startup = value;
          } catch (error) {
            // The orchestrator validates the document. A malformed projected value stays absent.
          }
        } else if (SETTING_GROUPS.some((group) => group.items.some((item) => item.key === setting.key && item.kind === "choice"))) {
          try { settings.choices[setting.key] = JSON.parse(setting.value); } catch (_) { /* invalid authored choice remains absent */ }
        } else if (setting.value === "false") {
          settings.restricted.add(setting.key);
        }
      }
      return settings;
    }

    /** The compiled view renders values as JSON text, which is what the orchestrator sent. */
    function parseHostList(value) {
      try {
        const parsed = JSON.parse(value);
        return Array.isArray(parsed) ? parsed.map(String) : [];
      } catch (error) {
        return [];
      }
    }

    /**
     * The permission grid, grouped by what a person actually thinks about.
     *
     * Every boolean switch here defaults on, because absence is what "allowed" means. A switch
     * reads as the permission, not the internal flag --
     * "MCP clients" with a checked box, not "turn off MCP clients" with an unchecked one. Unchecking
     * one is the only thing this editor can do to it; the schema underneath is still a restriction
     * that is only ever authored as `false` (ADR-0122 A3), but a person should never have to hold
     * that inversion in their head to use the page. The startup choice branches to a closed select.
     */
    function renderSettings() {
      if (!draft) return;
      el["setting-groups"].innerHTML = SETTING_GROUPS.map((group, at) => {
        // What a group cannot promise leads it, under its heading, and each switch it covers is
        // described by it too.
        const limits = settingLimits(group.items.map((item) => item.key));
        const limitIds = new Map(limits.map((limit, index) => [limit, `setting-group-${at}-limit-${index}`]));
        return `<div class="setting-group"><h3>${escapeHtml(group.title)}</h3>`
          + limitNotes(limits, limitIds)
          + group.items.map((item) => settingRow(item, limitIds)).join("")
          + `</div>`;
      }).join("");
      el["sacred-hosts"].value = draft.settings.sacred;
      refreshSacred();
    }

    /**
     * Say each limit once. With `ids`, a map from a limit to the id it wears, the switches it
     * covers can point at it.
     */
    function limitNotes(limits, ids = new Map()) {
      return limits.map((limit) => {
        const id = ids.get(limit);
        return `<p class="setting-limit"${id ? ` id="${escapeHtml(id)}"` : ""}>${escapeHtml(limit)}</p>`;
      }).join("");
    }

    /** The id a setting's row parts are named by, made from its registered key. */
    function settingId(key) {
      return `setting-${String(key).replace(/[^A-Za-z0-9]+/g, "-")}`;
    }

    /**
     * One switch: the permission as the person sees it, named by its row's name and described by
     * its detail, the browser setting beside it and the limit its group states.
     */
    function settingRow(item, limitIds) {
      if (item.kind === "choice") return choiceRow(item);
      const forcedBy = organizationForces(item.key);
      const checked = !forcedBy && !draft.settings.restricted.has(item.key);
      const detail = forcedBy
        ? `${escapeHtml(forcedBy)} already turned this off.`
        : escapeHtml(checked ? item.on : item.off);
      const id = settingId(item.key);
      // A setting the browser keeps is read here and changed there, so it is a line, not a control.
      const also = item.browser
        ? `<span class="setting-also" id="${id}-also">${escapeHtml(item.browser.word)}`
          + `<span class="chip context-chip">${escapeHtml(item.browser.where)}</span></span>`
        : "";
      const described = [`${id}-detail`, item.browser ? `${id}-also` : null, limitIds.get(item.limit) ?? null]
        .filter(Boolean).join(" ");
      return `<div class="setting-row${forcedBy ? " setting-forced" : ""}">`
        + `<label class="toggle">`
        + `<input type="checkbox" data-restriction="${escapeHtml(item.key)}"`
        + `${checked ? " checked" : ""}${forcedBy || !can(ACT.applyPolicy) ? " disabled" : ""}`
        + ` role="switch" aria-labelledby="${id}-name" aria-describedby="${described}">`
        + `<span class="toggle-track"><span class="toggle-thumb"></span></span></label>`
        + `<div class="setting-body"><span class="setting-name" id="${id}-name">${escapeHtml(item.name)}</span>`
        + `<span class="setting-detail" id="${id}-detail">${detail}</span>${also}</div></div>`;
    }

    /** Render the first closed string setting without turning it into a free-form field. */
    function choiceRow(item) {
      if (item.key !== "browser.startup") return authoredChoiceRow(item);
      const ceiling = applied?.browser_startup?.organization_ceiling;
      const forcedBy = ceiling === "manual"
        ? applied?.organization?.name ?? "Your organization"
        : null;
      const selected = forcedBy
        ? "manual"
        : draft.settings.startup ?? applied?.browser_startup?.value ?? "manual";
      const choice = item.choices.find((entry) => entry.value === selected) ?? item.choices[0];
      const options = item.choices.map((entry) =>
        `<option value="${escapeHtml(entry.value)}"${entry.value === selected ? " selected" : ""}>`
        + `${escapeHtml(entry.label)}</option>`).join("");
      const detail = forcedBy
        ? `${escapeHtml(forcedBy)} requires browser auto-open to stay off.`
        : escapeHtml(choice.detail);
      return `<div class="setting-row${forcedBy ? " setting-forced" : ""}">`
        + `<div class="setting-body"><label class="setting-name" for="setting-browser-startup">`
        + `${escapeHtml(item.name)}</label>`
        + `<select class="setting-choice" id="setting-browser-startup" data-setting-choice="${escapeHtml(item.key)}"`
        + `${forcedBy || !can(ACT.applyPolicy) ? " disabled" : ""}>${options}</select>`
        + `<span class="setting-detail">${detail}</span></div></div>`;
    }

    // Closed choices share the effective policy's authored source and organization floor.
    function authoredChoiceRow(item) {
      const authority = applied?.[item.authority];
      const ceiling = authority?.[`organization_${item.field}`];
      const minimum = Math.max(0, item.choices.findIndex((choice) => choice.value === ceiling));
      const desired = draft.settings.choices[item.key] ?? authority?.[item.field] ?? item.default;
      const selected = item.choices[Math.max(minimum, item.choices.findIndex((choice) => choice.value === desired))];
      const options = item.choices.map((choice, index) => `<option value="${escapeHtml(choice.value)}"${choice === selected ? " selected" : ""}${index < minimum ? " disabled" : ""}>${escapeHtml(choice.label)}</option>`).join("");
      const source = authority?.[item.source ?? `${item.field}_source`];
      const author = source === "organization" ? applied?.organization?.name ?? "Your organization" : source === "user" ? "Your rules" : "Ghostlight default";
      return `<div class="setting-row"><div class="setting-body"><label class="setting-name">${escapeHtml(item.name)}`
        + `<select class="setting-choice" data-setting-choice="${escapeHtml(item.key)}"`
        + `${can(ACT.applyPolicy) ? "" : " disabled"}>${options}</select></label>`
        + `<span class="setting-detail">${escapeHtml(selected.detail)} ${escapeHtml(author)}.</span></div></div>`;
    }

    function organizationForces(key) {
      const organization = applied?.layers?.find((layer) => layer.kind === "organization");
      const setting = organization?.settings.find((entry) => entry.key === key);
      if (setting?.value !== "false") return null;
      return applied?.organization?.name ?? organization?.title ?? "Your organization";
    }

    function refreshSacred() {
      const patterns = splitHosts(draft?.settings.sacred ?? "");
      const readback = patterns.map((pattern) => hostReadback(pattern)).join("; ");
      // A screen that may not name the sites shows how many there are, and leaves them as they are.
      const counted = draft?.settings.sacredCount;
      if (typeof counted === "number" && !can(ACT.applyPolicy)) {
        el["sacred-readback"].textContent = neverTouchedCount(counted);
        el["sacred-readback"].classList.toggle("readback-empty", false);
        return;
      }
      el["sacred-readback"].textContent = readback
        ? `Never touched: ${readback}.`
        : "No sites beyond the ones Ghostlight always refuses.";
      el["sacred-readback"].classList.toggle("readback-empty", !readback);
    }

    /**
     * Switch one permission on or off, as the person sees it.
     *
     * `allowed` is the checkbox state, which is the opposite of what the schema stores: allowing
     * something removes it from the restricted set (no opinion authored), and disallowing it adds
     * the one value a user layer may ever write for that key. This is the seam where the delightful
     * vocabulary in words.js turns back into the tightening-only schema in ADR-0122 A3.
     */
    function setPermission(key, allowed) {
      if (!draft) return;
      if (allowed) draft.settings.restricted.delete(key);
      else draft.settings.restricted.add(key);
      draft.dirty = true;
      renderSettings();
      editorReady();
      el["discard-policy"].hidden = false;
    }

    /** Select one value from a closed string setting. */
    function setChoice(key, value) {
      const item = SETTING_GROUPS.flatMap((group) => group.items).find((item) => item.key === key && item.kind === "choice");
      if (!draft || !item?.choices.some((choice) => choice.value === value)) return;
      if (key === "browser.startup") draft.settings.startup = value;
      else draft.settings.choices[key] = value;
      draft.dirty = true;
      renderSettings();
      editorReady();
      el["discard-policy"].hidden = false;
    }

    function setSacred(value) {
      if (!draft) return;
      draft.settings.sacred = value;
      draft.dirty = true;
      refreshSacred();
      editorReady();
      el["discard-policy"].hidden = false;
    }

    /**
     * Which capabilities an organization has refused everywhere.
     *
     * Only an organization ceiling may disable a control. A capability that merely happens to be
     * missing from this person's own rules is exactly what they are here to change, and greying it
     * out would tell them they cannot grant themselves something they plainly can.
     */
    function ceilingFor(capability) {
      const line = applied?.capabilities?.find((entry) => entry.capability === capability);
      if (!line || line.state !== "unavailable") return null;
      if (!line.decided_by?.includes("organization")) return null;
      const name = applied?.organization?.name;
      return name ? `${name} does not allow this` : "your organization does not allow this";
    }

    /**
     * Every rule in force, in the order authority actually considers them.
     *
     * One list, not two. The organization's rules come first because they are checked first and
     * cannot be edited here, and this person's own follow. Each is one line: what it covers, and
     * whose it is. Opening a line is what reveals detail, so the common case -- reading what
     * applies -- stays a single glance instead of a page of repeated headings.
     */
    function renderRules() {
      const organization = (applied?.layers ?? []).filter((layer) => layer.kind === "organization");
      const rows = organization.flatMap((layer) =>
        layer.rules.map((rule) => organizationRow(rule, layer)));
      const mine = draft ? draft.rules.map((rule, index) => userRow(rule, index)) : [];
      // How rules are checked matters once there is one; before that the headline says it all.
      el["rules-note"].hidden = !(rows.length || mine.length);
      el["rule-list"].innerHTML = rows.concat(mine).join("")
        || `<p class="rules-empty">${escapeHtml(NO_RULES)}</p>`;
      if (draft) {
        el["discard-policy"].hidden = !draft.dirty;
        renderSettings();
      }
      editorReady();
    }

    /** The sentence a rule reads as, which is the whole row when it is closed. */
    function sentenceFor(hosts, deny, capabilities) {
      const where = hosts.length ? hosts.map(escapeHtml).join(", ") : "no sites";
      const except = deny.length ? ` except ${deny.map(escapeHtml).join(", ")}` : "";
      const verbs = capabilities.length
        ? capabilities.map((capability) => escapeHtml(words(capability))).join(", ")
        : "nothing";
      return `On <b>${where}</b>${except}, agents may ${verbs}.`;
    }

    /** What an open rule shows when it cannot be edited here: its purpose, its sites and its name. */
    function ruleDetail(description, allow, deny, id) {
      return `<div class="rule-detail">`
        + (description ? `<p>${escapeHtml(description)}</p>` : "")
        + `<dl><dt>Sites</dt><dd>${allow.map(escapeHtml).join(", ") || "none"}</dd>`
        + (deny.length ? `<dt>Never</dt><dd>${deny.map(escapeHtml).join(", ")}</dd>` : "")
        + `<dt>Named</dt><dd>${escapeHtml(id)}</dd></dl></div>`;
    }

    /** A rule that is read here and not edited: the sentence it reads as, and whose it is. */
    function readOnlyRow(key, sentence, owner, detail) {
      const open = opened.has(key);
      return `<article class="rule rule-theirs${open ? " open" : ""}" data-rule-key="${escapeHtml(key)}">`
        + `<button class="rule-row" type="button" data-rule-toggle="${escapeHtml(key)}"`
        + ` aria-expanded="${open}">`
        + `<span class="rule-sentence">${sentence}</span>`
        + `<span class="rule-owner">${escapeHtml(owner)}</span></button>`
        + (open ? detail : "")
        + `</article>`;
    }

    function organizationRow(rule, layer) {
      return readOnlyRow(`org:${layer.title}:${rule.id}`, sentenceFor(rule.allow, rule.deny, rule.allowed),
        layer.title, ruleDetail(rule.description, rule.allow, rule.deny, rule.id));
    }

    function userRow(rule, index) {
      const key = `mine:${index}`;
      const open = opened.has(key);
      const capabilities = CAPABILITY_ORDER.filter((capability) => rule.allowed.has(capability));
      const sentence = sentenceFor(splitHosts(rule.hosts), [], capabilities);
      // A screen that cannot write the rules reads this person's own as it reads an organization's.
      if (!can(ACT.applyPolicy)) {
        return readOnlyRow(key, sentence, applied?.layers?.find((layer) => layer.kind === "user")?.title ?? "",
          ruleDetail(rule.description, splitHosts(rule.hosts), [], rule.id ?? ""));
      }
      const shadow = shadowedBy(index);
      return `<article class="rule rule-mine${open ? " open" : ""}${shadow ? " rule-inert" : ""}"`
        + ` data-rule-key="${escapeHtml(key)}">`
        + `<button class="rule-row" type="button" data-rule-toggle="${escapeHtml(key)}"`
        + ` aria-expanded="${open}">`
        + `<span class="rule-sentence">${sentence}</span>`
        + `<span class="rule-edit-hint">${open ? "Close" : "Edit"}</span></button>`
        + (open ? userEditor(rule, index, shadow) : "")
        + `</article>`;
    }

    function userEditor(rule, index, shadow) {
      const boxes = CAPABILITY_ORDER.map((capability) => {
        const blocked = ceilingFor(capability);
        const checked = rule.allowed.has(capability) && !blocked;
        return `<label class="cap-box${blocked ? " cap-box-blocked" : ""}">`
          + `<input type="checkbox" data-rule="${index}" data-capability="${capability}"`
          + `${checked ? " checked" : ""}${blocked ? " disabled" : ""}>`
          + `<span>${escapeHtml(words(capability))}</span>`
          + (blocked ? `<em>${escapeHtml(blocked)}</em>` : "")
          + `</label>`;
      }).join("");
      const readback = splitHosts(rule.hosts).map((pattern) => hostReadback(pattern)).join("; ");
      return `<div class="rule-detail">`
        + `<div class="rule-line"><span>On</span>`
        + `<input class="hosts" type="text" data-rule="${index}" data-field="hosts"`
        + ` value="${escapeHtml(rule.hosts)}" placeholder="example.com, *.example.com" spellcheck="false">`
        + `<span>agents may</span></div>`
        + `<div class="cap-boxes">${boxes}</div>`
        + (readback
          ? `<p class="readback">Matches ${escapeHtml(readback)}.</p>`
          : `<p class="readback readback-empty">Add a site for this rule to do anything.</p>`)
        + (shadow
          ? `<p class="rule-note">Rule ${shadow} above already covers this, so this one never applies. Move it up to use it.</p>`
          : "")
        + `<div class="rule-foot">`
        + `<input class="why" type="text" data-rule="${index}" data-field="description"`
        + ` value="${escapeHtml(rule.description)}" placeholder="What is this rule for? (optional)" maxlength="200">`
        + `<button class="link-button" type="button" data-rule="${index}" data-rule-action="up"${index === 0 ? " disabled" : ""}>Move up</button>`
        + `<button class="link-button" type="button" data-rule="${index}" data-rule-action="remove">Remove</button>`
        + `</div></div>`;
    }

    /** Which rows are open. Disposable view state, and the only thing this list remembers. */
    const opened = new Set();

    function toggleRule(key) {
      if (opened.has(key)) opened.delete(key);
      else opened.add(key);
      renderRules();
      if (opened.has(key)) {
        el["rule-list"].querySelector(`[data-rule-key="${key}"] .hosts`)?.focus();
      }
    }

    /**
     * What the editor will let you do with the draft as it stands.
     *
     * A draft whose rules were all removed is a policy that refuses everything, so neither applying
     * it nor asking what it would have done is a question worth answering. Saying that once,
     * plainly, beats letting someone press a button and read a number that sounds like an
     * accusation. An open draft with no rules carries OPEN_RULE, and says so once it has a change.
     */
    function editorReady() {
      if (!draft) return;
      const bare = !draft.rules.length;
      const empty = bare && !draft.open;
      el["apply-policy"].disabled = empty || !draft.dirty;
      el["check-policy"].disabled = empty;
      // Clearing matters as much as setting: a note about the rules must not outlive the rules it
      // was about and sit under good ones, contradicting them.
      const said = el["editor-status"].textContent;
      if (said === NO_RULES_LEFT || said === OPEN_RULE_NOTE) el["editor-status"].textContent = "";
      if (empty) {
        previewCleared();
        el["editor-status"].textContent = NO_RULES_LEFT;
      } else if (bare && draft.dirty) {
        el["editor-status"].textContent = OPEN_RULE_NOTE;
      }
    }

    /**
     * Which earlier rule, if any, makes this one unreachable.
     *
     * Grants are answered in written order, so a later rule fully covered by an earlier one can
     * never fire. Hiding that would turn a known ordering hazard into a silent one.
     */
    function shadowedBy(index) {
      const rule = draft.rules[index];
      const patterns = splitHosts(rule.hosts);
      if (!patterns.length) return null;
      for (let earlier = 0; earlier < index; earlier += 1) {
        const above = draft.rules[earlier];
        const covers = [...rule.allowed].every((capability) => above.allowed.has(capability));
        const hosts = splitHosts(above.hosts);
        if (covers && patterns.every((pattern) => hosts.some((broad) => patternCovers(broad, pattern)))) {
          return earlier + 1;
        }
      }
      return null;
    }

    function splitHosts(value) {
      return String(value || "").split(",").map((pattern) => pattern.trim()).filter(Boolean);
    }

    /** What a draft says: its rules and its settings, and nothing about when it was written. */
    function documentBody(source) {
      const open = { ...OPEN_RULE, allowed: new Set(CAPABILITY_ORDER) };
      const rules = source.rules.length || !source.open ? source.rules : [open];
      return {
        grants: rules.map((rule, index) => {
          const grant = {
            id: rule.id && /^[A-Za-z0-9._-]+$/.test(rule.id) ? rule.id : `rule-${index + 1}`,
            hosts: { allow: splitHosts(rule.hosts) },
            allowed: CAPABILITY_ORDER.filter((capability) => rule.allowed.has(capability))
          };
          if (rule.description.trim()) grant.description = rule.description.trim();
          return grant;
        }),
        config: settingEntries(source.settings)
      };
    }

    /**
     * Compose the document the orchestrator will validate. The surface authors no semantics.
     *
     * The version is today's date only when the rules or settings differ from the applied ones;
     * a draft that says what is already applied keeps the applied version.
     */
    function draftDocument() {
      if (!draft) return null;
      const body = documentBody(draft);
      const user = applied?.layers?.find((layer) => layer.kind === "user");
      const unchanged = Boolean(user) && JSON.stringify(body) === JSON.stringify(documentBody(draftFrom(applied)));
      return JSON.stringify({
        schema: 4,
        name: "Your rules",
        version: unchanged ? user.version : new Date().toISOString().slice(0, 10),
        grants: body.grants,
        config: body.config
      }, null, 2);
    }

    /**
     * The settings the draft authors, in their registered typed shapes.
     *
     * `level` is not a choice offered here. Both levels only tighten in 1.0, and nothing sits below
     * this layer for a recommendation to be relaxed by, so asking would be a word without a
     * consequence.
     */
    function settingEntries(settings) {
      const entries = [...settings.restricted]
        .map((key) => ({ key, value: false, level: "mandatory" }));
      if (settings.startup) {
        entries.push({ key: "browser.startup", value: settings.startup, level: "mandatory" });
      }
      for (const [key, value] of Object.entries(settings.choices)) entries.push({ key, value, level: "mandatory" });
      const sacred = splitHosts(settings.sacred);
      if (sacred.length) entries.push({ key: SACRED_KEY, value: sacred, level: "mandatory" });
      return entries;
    }

    function editRule(index, field, value) {
      if (!draft?.rules[index]) return;
      draft.rules[index][field] = value;
      draft.dirty = true;
      el["discard-policy"].hidden = false;
      editorReady();
      if (field === "hosts") refreshRuleHints(index);
    }

    /**
     * Update one rule's readback in place.
     *
     * Redrawing the whole list on every keystroke would take the caret with it, so the hints that
     * depend on what was just typed are the only thing that changes while a person is typing.
     */
    function refreshRuleHints(index) {
      const card = el["rule-list"].querySelector(`[data-rule-key="mine:${index}"]`);
      if (!card) return;
      const rule = draft.rules[index];
      const patterns = splitHosts(rule.hosts);
      const readback = card.querySelector(".readback");
      if (readback) {
        const text = patterns.map((pattern) => hostReadback(pattern)).join("; ");
        readback.textContent = text ? `Matches ${text}.` : "Add a site for this rule to do anything.";
        readback.classList.toggle("readback-empty", !text);
      }
      const shadow = shadowedBy(index);
      let note = card.querySelector(".rule-note");
      if (shadow && !note) {
        note = document.createElement("p");
        note.className = "rule-note";
        readback?.insertAdjacentElement("afterend", note);
      }
      if (note) {
        note.textContent = shadow
          ? `Rule ${shadow} above already covers this, so this one never applies. Move it up to use it.`
          : "";
        note.hidden = !shadow;
      }
    }

    function toggleCapability(index, capability, on) {
      if (!draft?.rules[index]) return;
      if (on) draft.rules[index].allowed.add(capability);
      else draft.rules[index].allowed.delete(capability);
      draft.dirty = true;
      renderRules();
    }

    function ruleAction(index, action) {
      if (!draft) return;
      if (action === "remove") {
        draft.rules.splice(index, 1);
        opened.delete(`mine:${index}`);
      }
      if (action === "up" && index > 0) {
        const [moved] = draft.rules.splice(index, 1);
        draft.rules.splice(index - 1, 0, moved);
        opened.delete(`mine:${index}`);
        opened.add(`mine:${index - 1}`);
      }
      draft.dirty = true;
      renderRules();
    }

    function addRule(seed) {
      if (!draft) return;
      draft.rules.push({
        id: `rule-${draft.rules.length + 1}`,
        hosts: seed || "",
        description: "",
        allowed: new Set(["read"])
      });
      draft.dirty = true;
      // A rule you just asked for opens on its own; anything else is a second click for nothing.
      opened.add(`mine:${draft.rules.length - 1}`);
      renderRules();
      const inputs = el["rule-list"].querySelectorAll(".hosts");
      inputs[inputs.length - 1]?.focus();
    }

    function discardDraft() {
      if (!applied) return;
      draft = draftFrom(applied);
      renderRules();
      previewCleared();
      el["editor-status"].textContent = "Back to the rules that are applied.";
    }

    function previewResult(preview) {
      const box = el["policy-preview"];
      box.hidden = false;
      const rows = preview.refused.map((entry) =>
        `<li><b>${escapeHtml(entry.tool)}</b>${entry.host ? ` on ${escapeHtml(entry.host)}` : ""}`
        + ` <span>${entry.count}x</span></li>`).join("");
      box.innerHTML = `<p class="preview-summary">${escapeHtml(preview.summary)}</p>`
        + (rows ? `<ul class="preview-list">${rows}</ul>` : "");
    }

    function previewCleared() {
      el["policy-preview"].hidden = true;
      el["policy-preview"].innerHTML = "";
    }

    function editorStatus(message) {
      el["editor-status"].textContent = message;
    }

    function draftIsDirty() {
      return Boolean(draft?.dirty);
    }

    /** Repaint a section only when its own facts changed, so a safety pull never rewrites a
     * surface the user is pointing at. */
    function collections(snapshot, pending) {
      attempt("painting history health", () => {
        el["audit-health"].textContent = snapshot.audit_notice ?? "";
        el["audit-health"].hidden = !snapshot.audit_notice;
      });
      passport = snapshot.configuration?.managed_policy ?? null;
      connections(snapshot);
      const product = {
        version: access?.product?.version ?? snapshot.service?.version,
        license: access?.product?.license,
        counts: { agents: agentGroups(snapshot.sessions).length, browsers: snapshot.browsers.length, recorded: snapshot.history.length }
      };
      ifChanged("about", product, () => about(product));
      // The page's own memo decides; its facts hold each agent's live sessions, which change alone.
      attempt("painting integrations", () => integrations(snapshot, pending));
      ifChanged("status", [snapshot.diagnostics, snapshot.configuration, snapshot.service, snapshot.controls?.service],
        () => status(snapshot));
    }

    /* -------------------------------- chrome ------------------------------ */

    let current = "monitor";
    let currentSection = DEFAULT_SECTION;

    /**
     * Write where the page is into its address, in place, so a tab or a rail entry adds nothing
     * to the history. A page that cannot be addressed still navigates.
     */
    function writeAddress(address) {
      try {
        if (window.location.hash === `#${address}`) return;
        window.history.replaceState(null, "", `#${address}`);
      } catch (error) {
        // The address is a convenience; showing the page is the job.
      }
    }

    /**
     * Show one destination and, for Settings, one of its sections, and say where the page now is.
     *
     * A name that is not a destination, or a section that is not one of Settings, changes nothing.
     * Settings without a section keeps the one it showed last.
     */
    function navigate(view, section) {
      if (!VIEWS[view]) return;
      if (view === "settings" && section && !SETTINGS_SECTIONS.some((known) => known.id === section)) return;
      current = view;
      if (view === "settings" && section) currentSection = section;
      for (const node of document.querySelectorAll(".view")) {
        node.classList.toggle("active", node.dataset.page === view);
      }
      for (const tab of document.querySelectorAll(".tab")) {
        const active = tab.dataset.view === view;
        tab.classList.toggle("active", active);
        if (active) tab.setAttribute("aria-current", "page");
        else tab.removeAttribute("aria-current");
      }
      for (const entry of document.querySelectorAll(".rail-entry")) {
        const active = entry.dataset.settingsSection === currentSection;
        entry.classList.toggle("active", active);
        if (active) entry.setAttribute("aria-current", "true");
        else entry.removeAttribute("aria-current");
      }
      for (const part of document.querySelectorAll(".settings-section")) {
        part.hidden = part.dataset.section !== currentSection;
      }
      el["main-content"].scrollTop = 0;
      writeAddress(addressOf(view, currentSection));
    }

    /**
     * Say what an act did. A sentence of success leaves on its own after TOAST_MS; a failure is
     * an alert that stays until the person dismisses it, a later failure takes its place, or a
     * later success says the window is working again. The two stack in one column and never
     * cover each other.
     */
    function toast(message, error = false) {
      if (error) {
        el["alert-text"].textContent = message;
        el.alert.hidden = false;
        return;
      }
      el.alert.hidden = true;
      clearTimeout(toastTimer);
      el.toast.textContent = message;
      el.toast.hidden = false;
      toastTimer = setTimeout(() => { el.toast.hidden = true; }, TOAST_MS);
    }

    el["alert-dismiss"]?.addEventListener("click", () => { el.alert.hidden = true; });

    /* ---------------------------- load failures --------------------------- */

    /** The line a destination shows while it is being read, where it has one. */
    const LOADING_LINE = Object.freeze({ policy: "policy-headline" });

    /** The destinations that have been drawn at least once, by their LOAD_FAILED key. */
    const drawnOnce = new Set();

    /**
     * Say, in place of what a destination is waiting to show, that it could not be read, with a
     * button that reads it again. `destination` is a key of LOAD_FAILED.
     *
     * The view wears `load-failed`, so Try again stands in for its own Refresh, and `unread` while
     * it has never been drawn, so the headings of what it would show do not sit empty under the
     * failure.
     */
    function loadFailed(destination) {
      const line = el[`${destination}-failure`];
      const view = el[`view-${destination}`];
      // Try again stands in for the Refresh the failure hides and for the Try again it redraws, so
      // the keyboard focus on either moves to it rather than falling back to the page.
      const focused = document.activeElement;
      const standsIn = Boolean(focused) && view.contains(focused)
        && (focused.classList.contains("refresh") || focused.matches("[data-retry]"));
      line.innerHTML = `<span class="dot" aria-hidden="true"></span><span>${escapeHtml(LOAD_FAILED[destination])}</span>`
        + `<button class="ghost-button" type="button" data-retry="${escapeHtml(destination)}">${escapeHtml(TRY_AGAIN)}</button>`;
      line.hidden = false;
      view.classList.add("load-failed");
      view.classList.toggle("unread", !drawnOnce.has(destination));
      const loading = LOADING_LINE[destination];
      if (loading) el[loading].hidden = true;
      if (standsIn) line.querySelector("[data-retry]").focus();
    }

    /**
     * Take a destination's failure line away once it has been read. The view's Refresh comes back
     * in place of Try again, so the keyboard focus on Try again moves to it rather than falling
     * back to the page.
     */
    function loaded(destination) {
      const line = el[`${destination}-failure`];
      const view = el[`view-${destination}`];
      const retried = line.contains(document.activeElement);
      view.classList.remove("load-failed", "unread");
      if (retried) view.querySelector(".refresh")?.focus();
      line.hidden = true;
      const loading = LOADING_LINE[destination];
      if (loading) el[loading].hidden = false;
    }

    /* ------------------------------- dialogs ------------------------------ */

    /*
     * The window's two dialogs, the search palette and the confirmation, hold focus while open.
     *
     * Opening one remembers the control that held focus, makes the window behind it inert and moves
     * focus in; Tab and Shift+Tab stay inside it; closing it gives the window back and returns
     * focus to that control. A repaint while it is open finds that control again (see
     * replaceHistoryMarkup); the page takes focus only when nothing stands in for it.
     */

    function holdFocus(box, first) {
      const opener = document.activeElement;
      const keep = (event) => {
        if (event.key !== "Tab") return;
        const stops = [...box.querySelectorAll(FOCUSABLE)].filter((stop) => !stop.hidden);
        if (!stops.length) {
          event.preventDefault();
          return;
        }
        const at = stops.indexOf(document.activeElement);
        const to = event.shiftKey
          ? (at <= 0 ? stops.at(-1) : null)
          : (at < 0 || at === stops.length - 1 ? stops[0] : null);
        if (to) {
          event.preventDefault();
          to.focus();
        }
      };
      el["app-shell"].inert = true;
      document.addEventListener("keydown", keep);
      modal = { opener, keep };
      first.focus();
    }

    function releaseFocus() {
      if (!modal) return;
      const { opener, keep } = modal;
      modal = null;
      document.removeEventListener("keydown", keep);
      el["app-shell"].inert = false;
      const back = opener && opener !== document.body && opener.isConnected !== false && !opener.disabled
        ? opener : el["main-content"];
      back.focus?.({ preventScroll: true });
    }

    function openPalette() {
      if (modal) return;
      el.palette.hidden = false;
      el["palette-query"].value = "";
      el["palette-results"].innerHTML = "";
      holdFocus(el.palette, el["palette-query"]);
    }

    function closePalette() {
      if (el.palette.hidden) return;
      el.palette.hidden = true;
      releaseFocus();
    }

    const paletteOpen = () => !el.palette.hidden;
    const paletteQuery = () => el["palette-query"].value;

    // ArrowDown and ArrowUp walk the input and the hits below it.
    el.palette?.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      const stops = [el["palette-query"], ...el["palette-results"].querySelectorAll(".hit")];
      const at = stops.indexOf(document.activeElement);
      if (at < 0) return;
      const to = stops[event.key === "ArrowDown" ? Math.min(at + 1, stops.length - 1) : Math.max(at - 1, 0)];
      event.preventDefault();
      to.focus();
    });

    /**
     * Draw the palette's hits as the service worded them. A recorded action adds how long ago it
     * settled to its second line, and the outcome word its row wears when it did not succeed.
     */
    function searchResults(hits) {
      el["palette-results"].innerHTML = hits.length
        ? hits.map((hit) => {
          const second = rowBy(hit.detail, hit.timestamp_ms ? agoSaid(hit.timestamp_ms) : "");
          const outcome = OUTCOME_WORD[hit.outcome] ?? "";
          return `<button class="hit" type="button" data-search-view="${escapeHtml(hit.view)}" data-search-id="${escapeHtml(hit.id)}">`
            + `<span class="hit-kind">${escapeHtml(HIT_KIND[hit.kind] ?? words(hit.kind))}</span>`
            + `<span><strong>${escapeHtml(hit.title)}</strong><small>${escapeHtml(second)}</small></span>`
            + `<span class="hit-outcome${outcome ? ` ${escapeHtml(hit.outcome)}` : ""}">${escapeHtml(outcome)}</span></button>`;
        }).join("")
        : `<div class="palette-empty">${escapeHtml(SEARCH_FOUND_NOTHING)}</div>`;
    }

    /** Say in the palette that the search failed, in `sentence`, never an exception's text. */
    const searchFailed = (sentence) =>
      { el["palette-results"].innerHTML = `<div class="palette-empty">${escapeHtml(sentence)}</div>`; };

    function confirmHarnessChange(title, detail, action) {
      if (confirmation || modal) return Promise.resolve(false);
      el["confirm-title"].textContent = title;
      el["confirm-detail"].textContent = detail;
      const accept = el["confirm-dialog"].querySelector('[data-confirm="accept"]');
      accept.textContent = action;
      return new Promise((resolve) => {
        const finish = (confirmed) => {
          el["confirm-dialog"].hidden = true;
          confirmation = null;
          document.removeEventListener("keydown", onKeyDown);
          releaseFocus();
          resolve(confirmed);
        };
        const onKeyDown = (event) => { if (event.key === "Escape") finish(false); };
        confirmation = finish;
        el["confirm-dialog"].hidden = false;
        holdFocus(el["confirm-dialog"], el["confirm-dialog"].querySelector('[data-confirm="cancel"]'));
        document.addEventListener("keydown", onKeyDown);
      });
    }

    /** Ask before removing Ghostlight from an agent, naming the sessions that use it now. */
    function confirmRemoval(name, file, sessions) {
      const asked = confirmRemovalWords(name, file, sessions);
      return confirmHarnessChange(asked.title, asked.detail, asked.action);
    }

    /** Ask before Fix replaces what sits under Ghostlight's key in an agent's file. */
    function confirmFix(name, file) {
      const asked = confirmFixWords(name, file);
      return confirmHarnessChange(asked.title, asked.detail, asked.action);
    }

    const answerConfirmation = (confirmed) => {
      if (!confirmation) return false;
      confirmation(confirmed);
      return true;
    };

    /* ------------------------------ live clocks --------------------------- */

    /** Move the clocks of unsettled work: the card's for running or pending work, a row's for running. */
    function tickElapsed(feed) {
      const top = feed[0];
      if (top && isUnsettled(top)) {
        const node = document.getElementById("elapsed");
        if (node) node.textContent = elapsedSoFar(top);
      }
      for (const entry of feed.slice(1)) {
        if (!isRunning(entry)) continue;
        const cell = rowNodes.get(entry.invocation)?.querySelector(".row-dur");
        if (cell) cell.textContent = elapsedSoFar(entry);
      }
    }

    /** Move the ages of settled work, and rest the running card once it has led long enough. */
    function tickAges(feed) {
      const top = feed[0];
      if (top && rests(top) !== resting) paintHero(top, false);
      else if (resting) {
        const node = document.getElementById("hero-age");
        if (node) node.textContent = agoSaid(top.endedAt);
      }
      for (const entry of feed.slice(1)) {
        if (!entry.endedAt) continue;
        const node = rowNodes.get(entry.invocation);
        if (!node) continue;
        const cells = [[".row-when", ago(entry.endedAt)], [".row-by-words", byShown(entry)], [".row-by-said", bySaid(entry)]];
        for (const [selector, text] of cells) {
          const cell = node.querySelector(selector);
          if (cell) cell.textContent = text;
        }
      }
    }

    /**
     * The card's sheen and foil follow the pointer, and let go when it leaves.
     *
     * Everything the effect needs is a custom property, so the work here is two numbers per move
     * and the compositor does the rest.
     */
    function armCard() {
      const card = el["about-card"];
      if (!card) return;
      links();
      card.addEventListener("pointermove", (event) => {
        const box = card.getBoundingClientRect();
        const x = ((event.clientX - box.left) / box.width) * 100;
        const y = ((event.clientY - box.top) / box.height) * 100;
        card.style.setProperty("--mx", `${x.toFixed(1)}%`);
        card.style.setProperty("--my", `${y.toFixed(1)}%`);
        card.style.setProperty("--gx", `${((50 - x) / 6).toFixed(1)}px`);
        card.style.setProperty("--gy", `${((50 - y) / 6).toFixed(1)}px`);
        card.style.setProperty("--holo", "1");
      });
      card.addEventListener("pointerleave", () => card.style.setProperty("--holo", "0"));
    }

    return Object.freeze({
      el, attempt,
      hero, row, drop, promote, rebuildFeed, queueCount,
      context, closed, chip, reconnecting, leaving,
      band, collections, navigate, writeAddress, showing: () => current, showingSection: () => currentSection, toast,
      showAgent,
      policy, draftDocument, draftIsDirty, editRule, toggleCapability, ruleAction, addRule, toggleRule,
      setPermission, setChoice, setSacred,
      discardDraft, renderRules, previewResult, previewCleared, editorStatus,
      loadFailed, loaded,
      openPalette, closePalette, paletteOpen, paletteQuery, searchResults, searchFailed, showAction,
      confirmRemoval, confirmFix, answerConfirmation,
      tickElapsed, tickAges, armCard
    });
  }

  return Object.freeze({ create });
});
