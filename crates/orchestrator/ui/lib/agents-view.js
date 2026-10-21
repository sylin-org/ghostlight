// Ghostlight workbench -- the Integrations view: every agent Ghostlight knows, on a rail grouped by
// state, and the one the person picked shown in full beside it.
//
// It draws the service's roster and decides nothing: what each act may do, the settings file and
// which agents are in use are the service's words. The view hands it the element table, what this
// screen may do (`can`, `access`, `opensLinks`), the page it is showing and the helpers every view
// shares; it gives back what the view calls: `integrations`, `showAgent` and `toggleFold`.
(function installGhostlightAgentsView(root, factory) {
  const api = factory();
  root.GhostlightAgentsView = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightAgentsViewApi() {
  "use strict";

  const {
    ABSENT_GROUP, AGENTS, AGENT_ACTS, AGENT_GROUPS, AGENT_STATE_WORDS, BY_HAND, CHEVRON,
    WORKING_TONE, addressOf, agentCopyNote, agentEntryLabel, agentFileLabel, agentHeadline,
    agentLead, agentLive, agentPlan, agentStanding, agentStateWord, agentUse, agentWhy, escapeHtml,
    moreAgents, noAgentCalled, removeLabel, removeNote, symbolAbout, symbolMarkup
  } = globalThis.GhostlightWords;
  const { ACT, COPY_KIND } = globalThis.GhostlightTransport;

  /**
   * The agents pane on the elements in `el`. `access()` is what this screen was last told it may do
   * and `showing()` the page it shows; `drawnOnce` is the view's set of destinations drawn at least
   * once. Registers the pane's listeners on creation.
   */
  function create({ el, can, opensLinks, continueMarkup, replaceHistoryMarkup, ifChanged, drawnOnce, access, showing }) {
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
        remove: manage && agent.can_uninstall,
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
      lastAgents = { harnesses, sessions, inUseIds, pending, facts: [harnesses, uses, inUseIds, [...pending], access()?.acts] };
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
      if (event.key !== "/" || showing() !== "integrations" || !el["confirm-dialog"].hidden || !el.palette.hidden) return;
      if (/^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName ?? "")) return;
      event.preventDefault();
      el["agents-find"].focus();
    });

    return Object.freeze({ integrations, showAgent, toggleFold });
  }

  return Object.freeze({ create });
});
