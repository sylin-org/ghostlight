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
    VIEWS, GLYPHS, EFFECT_STORY, READINESS_NOTE, DESTINATIONS, glyphFor, capabilityClass,
    CAPABILITY_ORDER, CAPABILITY_BADGE, CAPABILITY_TONE, SETTING_GROUPS, SACRED_KEY, settingWords,
    INTEGRATION_CATEGORIES, INTEGRATION_STATE_CATEGORY, INTEGRATION_CATEGORY_PRIORITY,
    hostReadback, patternCovers,
    escapeHtml, words, duration, stopwatch, ago, shortId
  } = globalThis.GhostlightWords;
  const { settledMs, isRunning, isBlocked } = globalThis.GhostlightEntries;

  const TOAST_MS = 4200;

  function create({ sessionFor = () => null, onFailure = () => {} } = {}) {
    /*
     * Every element with an id, looked up once.
     *
     * This was a hand-maintained list, and a node added to the markup without being added here
     * read as undefined and threw on first use. Deriving it from the document means a new id
     * cannot be forgotten, because there is nothing left to remember.
     */
    const el = Object.create(null);
    for (const node of document.querySelectorAll("[id]")) el[node.id] = node;

    const rowNodes = new Map();
    const painted = Object.create(null);
    let toastTimer = null;
    let confirmation = null;

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
     * The signature used to be stored before the paint ran, and this memo is never cleared, so a
     * panel that threw was remembered as finished and stayed blank for the life of the window.
     * Recording afterwards turns a failure into something the next change retries.
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
      if (!entry.settled) return entry.activity;
      if (entry.summary) return entry.summary;
      if (isBlocked(entry)) return entry.reason ? words(entry.reason) : "blocked";
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
     * A settled row carries it on the record. A running one has no record yet, so it resolves
     * through the session that admitted it, which is still connected while the work is in flight.
     */
    function channelFor(entry) {
      if (entry.channel) return entry.channel;
      return sessionFor(entry.workspace)?.channel ?? "";
    }

    /** The client that asked, resolved through the current sessions when it is still connected. */
    function clientFor(workspace) {
      const session = sessionFor(workspace);
      return session ? session.client_label : shortId(workspace);
    }

    /* -------------------------------- monitor ----------------------------- */

    function heroMarkup(entry) {
      // The sentence names the host itself now, so the hero carries no host chip: it would say the
      // same thing twice. Readiness is the one observed fact no sentence states.
      const observed = entry.settled ? entry.observed : null;
      const note = observed ? READINESS_NOTE[observed.readiness] ?? "" : "";
      const meta = [];
      if (entry.workspace) meta.push(`<span>${escapeHtml(clientFor(entry.workspace))}</span>`);
      // Only a non-default intake earns words. Labelling every agent row "mcp" is noise.
      if (entry.channel && entry.channel !== "mcp") meta.push(`<span><i></i>via ${escapeHtml(entry.channel)}</span>`);
      if (entry.capability) meta.push(`<span><i></i>${escapeHtml(entry.capability)} authority</span>`);
      if (entry.settled && entry.status) meta.push(`<span><i></i>${escapeHtml(words(entry.status))}</span>`);
      if (note) meta.push(`<span><i></i>${escapeHtml(note)}</span>`);
      if (entry.settled && entry.endedAt) meta.push(`<span><i></i>${escapeHtml(ago(entry.endedAt))} ago</span>`);

      const reason = isBlocked(entry) && entry.reason
        ? `<p class="hero-reason">${escapeHtml(words(entry.reason))}</p>${refusalMarkup(entry)}`
        : "";

      return `<div class="hero-tool">${escapeHtml(entry.tool)}<span class="cap-label">${escapeHtml(entry.capability ?? "read")}</span></div>`
        + `<p class="hero-activity">${escapeHtml(sentence(entry))}</p>`
        + reason
        + (meta.length ? `<div class="hero-meta">${meta.join("")}</div>` : "");
    }

    /**
     * Where a refusal leads.
     *
     * A blocked row used to end at the reason code, which tells a person that something stopped
     * them and nothing about what to do next. The deciding layer and rule are already recorded on
     * every enforced denial, and an organization that supplied contacts supplied them for exactly
     * this moment, so both belong here rather than on a page nobody had a reason to open.
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

    function heroRightMarkup(entry) {
      const running = isRunning(entry);
      const elapsed = running
        ? stopwatch(Date.now() - (entry.startedAt ?? Date.now()))
        : duration(settledMs(entry));
      const outcome = running
        ? words(entry.phase)
        : isBlocked(entry)
          ? "blocked"
          : words(entry.status ?? "completed");
      return `<div class="elapsed" id="elapsed">${escapeHtml(elapsed)}</div>`
        + `<div class="outcome">${escapeHtml(outcome)}</div>`;
    }

    function hero(entry, animate) {
      if (!entry) {
        el.hero.className = "hero";
        el["hero-med"].innerHTML = GLYPHS.scan;
        el["hero-body"].innerHTML = '<div class="hero-tool">Nothing yet</div>'
          + '<p class="hero-activity">The first browser action an agent takes will appear here.</p>';
        el["hero-right"].innerHTML = "";
        return;
      }
      el.hero.className = `hero ${capabilityClass(entry)}`;
      if (isRunning(entry)) el.hero.classList.add("live");
      if (isBlocked(entry)) el.hero.classList.add("blocked");
      el["hero-med"].innerHTML = glyphFor(entry);
      el["hero-body"].innerHTML = heroMarkup(entry);
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
      const running = isRunning(entry);
      const time = running
        ? stopwatch(Date.now() - (entry.startedAt ?? Date.now()))
        : duration(settledMs(entry));
      return `<div class="med-mini">${glyphFor(entry)}</div>`
        + `<div class="row-tool">${escapeHtml(entry.tool)}</div>`
        + `<div class="row-channel">${escapeHtml(channelFor(entry))}</div>`
        + `<div class="row-activity">${escapeHtml(describe(entry))}</div>`
        + `<div class="row-client">${escapeHtml(clientFor(entry.workspace))}</div>`
        + `<div class="row-cap">${escapeHtml(entry.capability ?? "")}</div>`
        + `<div class="row-dur">${escapeHtml(time)}</div>`
        + `<div class="row-when">${escapeHtml(entry.endedAt ? ago(entry.endedAt) : "")}</div>`;
    }

    function rowClass(entry) {
      let name = `row ${capabilityClass(entry)}`;
      if (isRunning(entry)) name += " running";
      if (isBlocked(entry)) name += " blocked";
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
      node.innerHTML = rowMarkup(entry);
    }

    function drop(entry) {
      rowNodes.get(entry.invocation)?.remove();
      rowNodes.delete(entry.invocation);
    }

    function queueCount(feed) {
      const rows = Math.max(0, feed.length - 1);
      el["queue-count"].textContent = rows ? `${rows} ${rows === 1 ? "action" : "actions"}` : "";
      el["clear-monitor"].disabled = !feed.some((entry) => !isRunning(entry));
    }

    /** Full rebuild. Only used on first paint and after a detected sequence gap. */
    function rebuildFeed(feed) {
      el.queue.replaceChildren();
      rowNodes.clear();
      hero(feed[0], false);
      const rows = document.createDocumentFragment();
      for (const entry of feed.slice(1)) rows.append(buildRow(entry, false));
      el.queue.append(rows);
      if (feed.length <= 1) {
        const empty = document.createElement("div");
        empty.className = "empty";
        empty.textContent = "Nothing earlier in this session.";
        el.queue.append(empty);
      }
      queueCount(feed);
    }

    function promote(entry, previous, feed) {
      if (previous) {
        el.queue.querySelector(".empty")?.remove();
        el.queue.prepend(buildRow(previous, true));
      }
      hero(entry, true);
      queueCount(feed);
    }

    /* --------------------------------- band ------------------------------- */

    // The answer and its words are the orchestrator's, not this window's. A surface that classifies
    // its own readiness is a second source of truth about the one thing that must have exactly one,
    // which is the same rule the Policy chip below already follows.
    function band(facts) {
      const readiness = facts.snapshot?.readiness;
      const name = readiness ? `runtime-${readiness.tone}` : "runtime-offline";
      document.body.className = name;
      el["state-word"].textContent = readiness ? readiness.word : "";
      el["state-word"].title = readiness ? readiness.detail : "";

      if (!facts.snapshot) {
        el["state-facts"].textContent = "";
      } else {
        el["state-facts"].innerHTML = `<b>${facts.snapshot.sessions.length}</b> sessions`
          + ` &middot; <b>${facts.snapshot.browsers.length}</b> browsers`
          + ` &middot; <b>${facts.running}</b> running`
          + ` &middot; <b>${facts.snapshot.history.length}</b> recorded`;
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

      const paused = facts.runtime !== "active";
      el.wheel.disabled = !(facts.snapshot?.readiness?.invites_control ?? false);
      el.wheel.dataset.intent = facts.runtime === "ended" ? "start_session" : paused ? "resume" : "hold";
      el["wheel-label"].textContent = facts.runtime === "ended" ? "Start session" : paused ? "Resume" : "Pause";
      el["wheel-icon"].innerHTML = paused
        ? '<path d="M7 4.5 19 12 7 19.5z"/>'
        : '<rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/>';
    }

    /* ----------------------------- collections ---------------------------- */

    /**
     * One chip per client, not per connection.
     *
     * A client that reconnects often -- an editor that starts a session per request, a shell
     * running `ghostlight call` in a loop -- is one thing the user recognizes, and a row of
     * identical names tells them nothing they did not already know. The sessions themselves stay
     * separate everywhere that matters: each keeps its own workspace, tabs, and attribution.
     */
    function connectionGroups(sessions) {
      const groups = new Map();
      for (const session of sessions) {
        const group = groups.get(session.client_label)
          ?? { label: session.client_label, count: 0, tabs: 0, busy: false };
        group.count += 1;
        group.tabs += session.tab_count;
        group.busy = group.busy || session.active_operations > 0;
        groups.set(session.client_label, group);
      }
      return [...groups.values()]
        .sort((left, right) => right.tabs - left.tabs || left.label.localeCompare(right.label));
    }

    function connections(snapshot) {
      const chips = connectionGroups(snapshot.sessions).map((group) => {
        // The count earns its place only when there is more than one, so the common case stays quiet.
        const many = group.count > 1 ? ` <span class="tally">${group.count}</span>` : "";
        return `<span class="chip ${group.busy ? "busy" : "on"}"><span class="dot"></span>${escapeHtml(group.label)}${many}`
          + `<small>${group.tabs} tabs</small></span>`;
      });
      chips.push(...snapshot.browsers.map((browser) =>
        `<span class="chip on"><span class="dot"></span>${escapeHtml(browser.family)}`
        + `<small>adapter ${escapeHtml(browser.adapter_version ?? "unknown")}</small></span>`));
      if (!chips.length) {
        chips.push('<span class="chip"><span class="dot"></span>Waiting for a client or a browser</span>');
      }
      el.connections.innerHTML = chips.join("");
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

    function about(snapshot) {
      const service = snapshot.service ?? {};
      const version = String(service.version ?? "");
      // The disc wears the short form the portfolio card wears; the exact build is in the facts.
      el["about-version"].textContent = version.split(".").slice(0, 2).join(".") || "1.0";
      const facts = [
        ["Version", version || "unknown"],
        ["Sessions", `${snapshot.sessions.length} connected`],
        ["Browsers", `${snapshot.browsers.length} attached`],
        ["Recorded", `${snapshot.history.length} actions on this device`],
        ["Engine", "Apache-2.0 OR MIT"],
        ["Governance", "Ghostlight Commercial License, source-available"]
      ];
      el["about-facts"].innerHTML = facts
        .map(([term, value]) => `<dt>${escapeHtml(term)}</dt><dd>${escapeHtml(value)}</dd>`)
        .join("");
    }

    /*
     * Master and detail: which clients exist on the left, everything about one of them on the right.
     *
     * The list is for finding, so a row carries identity and state and nothing else. The pane is
     * for acting, so every operation lives there with the facts that justify it -- the exact file,
     * what Ghostlight would write, and for a blocked target, what it actually found.
     *
     * Selection is view state and survives re-render by id. This surface redraws from every
     * sequenced snapshot, so a selection kept in the DOM would silently reset while someone read.
     */
    let selectedProduct = null;

    function integrations(snapshot, pending) {
      if (!snapshot.harnesses.length) {
        el["integration-grid"].innerHTML =
          '<div class="empty">No supported MCP client was found for this user.</div>';
        return;
      }

      const products = new Map();
      for (const harness of snapshot.harnesses) {
        const id = harness.product_id ?? harness.id;
        if (!products.has(id)) products.set(id, []);
        products.get(id).push(harness);
      }

      const entries = [...products.entries()].map(([productId, all]) => {
        const targets = [...all].sort((left, right) =>
          left.target.localeCompare(right.target) || left.id.localeCompare(right.id));
        const live = targets.filter((target) => target.state !== "not_detected");
        const categories = new Set((live.length ? live : [targets[0]])
          .map((target) => INTEGRATION_STATE_CATEGORY[target.state]));
        const categoryId = INTEGRATION_CATEGORY_PRIORITY.find((id) => categories.has(id))
          ?? "needs-attention";
        return {
          productId,
          targets,
          live,
          categoryId,
          name: targets[0].name,
          icon: targets[0].icon ?? "generic.svg"
        };
      }).sort((left, right) => left.name.localeCompare(right.name)
        || left.productId.localeCompare(right.productId));

      // Land on what needs a person; otherwise keep whatever was open, otherwise the first client.
      // A foreign entry outranks a merely stale path: one is someone else's file that Ghostlight
      // refused to touch, the other is a version number.
      const byId = new Map(entries.map((entry) => [entry.productId, entry]));
      const blocked = entries.find((entry) =>
        entry.targets.some((target) => target.state === "needs_attention"));
      const attention = blocked
        ?? entries.find((entry) => entry.categoryId === "needs-attention");
      if (!selectedProduct || !byId.has(selectedProduct)) {
        selectedProduct = (attention ?? entries[0]).productId;
      }
      const selected = byId.get(selectedProduct);

      el["integration-grid"].innerHTML =
        '<div class="integration-split">'
        + '<nav class="integration-list" aria-label="MCP clients">' + list(entries) + '</nav>'
        + '<section class="integration-detail" aria-live="polite">' + detail(selected, pending, snapshot)
        + '</section></div>';
    }

    function list(entries) {
      const groups = INTEGRATION_CATEGORIES
        .map((category) => [category, entries.filter((entry) => entry.categoryId === category.id)])
        .filter(([, members]) => members.length > 0);
      return groups.map(([category, members]) =>
        '<h2 class="integration-list-heading">' + escapeHtml(category.label)
        + '<span>' + members.length + '</span></h2>'
        + members.map((entry) =>
          '<button class="integration-list-row integration-' + category.id + '" type="button"'
          + ' data-harness-select="' + escapeHtml(entry.productId) + '"'
          + (entry.productId === selectedProduct ? ' aria-current="true"' : "")
          + '><img src="integrations/' + escapeHtml(entry.icon) + '" alt="" width="20" height="20">'
          + '<span class="integration-name">' + escapeHtml(entry.name) + '</span>'
          + '<span class="integration-pip" aria-hidden="true"></span></button>').join("")
      ).join("");
    }

    function detail(entry, pending, snapshot) {
      if (!entry) return "";
      const category = INTEGRATION_CATEGORIES.find((item) => item.id === entry.categoryId);
      const shown = entry.live.length ? entry.live : [entry.targets[0]];
      return '<header class="integration-detail-head">'
        + '<img src="integrations/' + escapeHtml(entry.icon) + '" alt="" width="34" height="34">'
        + '<div><h1>' + escapeHtml(entry.name) + '</h1>'
        + '<p class="integration-detail-state integration-' + escapeHtml(entry.categoryId) + '">'
        + escapeHtml(category?.label ?? "") + '</p></div></header>'
        + '<p class="integration-detail-sentence">' + escapeHtml(category?.sentence ?? "") + '</p>'
        + shown.map((target) => targetBlock(entry, target, pending)).join("")
        + connectorBlock(snapshot);
    }

    /* One block per concrete target: what it is, where it lives, and everything you may do to it. */
    function targetBlock(entry, target, pending) {
      const waiting = pending.has(target.id);
      const act = (action, label, kind) =>
        '<button class="' + kind + '" type="button" data-harness-operation="manage"'
        + ' data-harness-action="' + action + '" data-harness="' + escapeHtml(target.id) + '"'
        + ' data-harness-name="' + escapeHtml(target.name) + '"' + (waiting ? " disabled" : "") + '>'
        + (waiting ? "Working..." : label) + '</button>';
      const utility = (operation, label, extra = "") =>
        '<button class="link-button" type="button" data-harness-operation="' + operation + '"'
        + ' data-harness="' + escapeHtml(target.id) + '" data-harness-name="'
        + escapeHtml(target.name) + '"' + extra + '>' + label + '</button>';

      let actions = "";
      if (target.state === "installed" && target.can_uninstall) {
        actions = act("uninstall", "Remove", "danger-button");
      } else if (target.state === "updatable" && target.can_install) {
        actions = act("install", "Update", "ghost-button");
      } else if (target.state === "available" && target.can_install) {
        actions = act("install", "Set up", "ghost-button");
      } else if (target.state === "not_detected" && target.can_download) {
        actions = '<button class="ghost-button" type="button" data-harness-operation="download"'
          + ' data-product="' + escapeHtml(entry.productId) + '" data-harness="'
          + escapeHtml(target.id) + '" data-harness-name="' + escapeHtml(target.name)
          + '">Install ' + escapeHtml(entry.name) + '</button>';
      }
      // A foreign entry is never overwritten, so the routes offered are the ones that write
      // nothing. ADR-0125's ownership rule is what makes showing the evidence safe at all.
      const evidence = target.state === "needs_attention"
        ? '<div class="integration-evidence">'
          + '<div class="evidence"><span>Found</span><code>'
          + escapeHtml(target.found_command ?? target.detail) + '</code></div>'
          + '<div class="evidence"><span>Would write</span><code>'
          + escapeHtml(target.connector_command) + '</code></div>'
          + '<p class="integration-untouched">Ghostlight changed nothing.</p></div>'
        : "";
      const locate = target.can_locate ? utility("locate", "Open the file") : "";
      const copy = utility("copy", "Copy what it would write", ' data-copy-kind="setup"'
        + ' data-harness-manual="' + escapeHtml(target.id) + '"');

      return '<article class="integration-target-block" data-harness-target="'
        + escapeHtml(target.id) + '">'
        + '<div class="integration-target-head"><strong>' + escapeHtml(target.target) + '</strong>'
        + '<span class="integration-target-state">' + escapeHtml(stateWord(target.state))
        + '</span></div>'
        + '<p class="integration-target-detail">' + escapeHtml(target.detail) + '</p>'
        + evidence
        + '<div class="evidence"><span>File</span><code>' + escapeHtml(target.config_path)
        + '</code></div>'
        + '<details class="integration-manual"><summary>What Ghostlight writes</summary>'
        + '<pre>' + escapeHtml(target.manual_setup) + '</pre></details>'
        + '<div class="tile-actions">' + actions + locate + copy + '</div></article>';
    }

    function stateWord(state) {
      return {
        installed: "connected",
        updatable: "needs an update",
        available: "not connected",
        needs_attention: "needs attention",
        not_detected: "not installed here"
      }[state] ?? state;
    }

    /* The connector path is one string for every client, so the pane states it once. */
    function connectorBlock(snapshot) {
      const first = snapshot.harnesses[0];
      if (!first?.connector_command) return "";
      return '<div class="integration-connector"><span>Connector</span>'
        + '<code>' + escapeHtml(first.connector_command) + '</code>'
        + '<button class="link-button" type="button" data-harness-operation="copy"'
        + ' data-harness="' + escapeHtml(first.id) + '" data-harness-name="' + escapeHtml(first.name) + '"'
        + ' data-copy-kind="command">Copy</button></div>';
    }

    /* Selection is presentation state; the caller re-renders from the current snapshot. */
    function selectHarnessProduct(productId) {
      selectedProduct = productId;
    }

    function status(snapshot) {
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

    function policy(view) {
      applied = view;
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
      el["add-rule"].hidden = !user.editable;
      el["policy-blocked"].hidden = user.editable || !user.blocked_reason;
      if (user.blocked_reason) el["policy-blocked-reason"].textContent = user.blocked_reason;
      el["policy-remove"].hidden = !(user.editable && user.source === "workbench" && hasUserLayer(view));

      draft = user.editable ? draftFrom(view) : null;
      opened.clear();
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
        ? `<h2 class="subhead">Restrictions in force</h2><ul class="settings">`
          + settings.map((setting) => {
            let value = setting.value;
            try {
              value = JSON.parse(setting.value);
            } catch (error) {
              // A value the orchestrator could not render as JSON is shown as it arrived.
            }
            return `<li>${escapeHtml(settingWords(setting.key, value))}`
              + ` <span class="owner">${escapeHtml(setting.owner)}</span></li>`;
          }).join("")
          + `</ul>`
        : "";

      el["policy-documents"].innerHTML = layers.map((layer) => {
        if (!layer.document) return "";
        const where = layer.path ? `<span class="doc-path">${escapeHtml(layer.path)}</span>` : "";
        return `<details class="layer-doc"><summary>${escapeHtml(layer.title)}: `
          + `${escapeHtml(layer.policy_name)} ${escapeHtml(layer.version)}, `
          + `${escapeHtml(layer.mode === "observe" ? "watch only" : "enforced")}</summary>`
          + where + `<pre>${escapeHtml(layer.document)}</pre></details>`;
      }).join("");
    }

    /* ------------------------------ the editor ----------------------------- */

    /** Turn the applied user layer into an editable draft, or start an empty one. */
    function draftFrom(view) {
      const layer = view.layers.find((entry) => entry.kind === "user");
      if (!layer) return { rules: [], settings: emptySettings(), observe: false, dirty: false };
      return {
        observe: layer.mode === "observe",
        dirty: false,
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
      return { restricted: new Set(), sacred: "", startup: null };
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
        } else if (setting.key === "browser.startup") {
          try {
            const value = JSON.parse(setting.value);
            if (value === "on_demand" || value === "manual") settings.startup = value;
          } catch (error) {
            // The orchestrator validates the document. A malformed projected value stays absent.
          }
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
      el["setting-groups"].innerHTML = SETTING_GROUPS.map((group) =>
        `<div class="setting-group"><h3>${escapeHtml(group.title)}</h3>`
        + group.items.map(settingRow).join("")
        + `</div>`).join("");
      el["sacred-hosts"].value = draft.settings.sacred;
      refreshSacred();
    }

    function settingRow(item) {
      if (item.kind === "choice") return choiceRow(item);
      const forcedBy = organizationForces(item.key);
      const checked = !forcedBy && !draft.settings.restricted.has(item.key);
      const detail = forcedBy
        ? `${escapeHtml(forcedBy)} already turned this off.`
        : escapeHtml(checked ? item.on : item.off);
      const link = !forcedBy && item.link
        ? `<button class="link-button" type="button" `
          + (item.link.view
            ? `data-view="${escapeHtml(item.link.view)}"`
            : `data-destination="${escapeHtml(item.link.destination)}"`)
          + `>${escapeHtml(item.link.label)}</button>`
        : "";
      return `<div class="setting-row${forcedBy ? " setting-forced" : ""}">`
        + `<label class="toggle">`
        + `<input type="checkbox" data-restriction="${escapeHtml(item.key)}"`
        + `${checked ? " checked" : ""}${forcedBy ? " disabled" : ""}>`
        + `<span class="toggle-track"><span class="toggle-thumb"></span></span></label>`
        + `<div class="setting-body"><span class="setting-name">${escapeHtml(item.name)}</span>`
        + `<span class="setting-detail">${detail}</span>${link}</div></div>`;
    }

    /** Render the first closed string setting without turning it into a free-form field. */
    function choiceRow(item) {
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
        ? `${escapeHtml(forcedBy)} requires you to start the browser yourself.`
        : escapeHtml(choice.detail);
      return `<div class="setting-row${forcedBy ? " setting-forced" : ""}">`
        + `<div class="setting-body"><label class="setting-name" for="setting-browser-startup">`
        + `${escapeHtml(item.name)}</label>`
        + `<select class="setting-choice" id="setting-browser-startup" data-setting-choice="${escapeHtml(item.key)}"`
        + `${forcedBy ? " disabled" : ""}>${options}</select>`
        + `<span class="setting-detail">${detail}</span></div></div>`;
    }

    /**
     * Whether an organization has already turned this off, and if so who to name.
     *
     * A user's own switch cannot undo this, so it renders off and disabled rather than editable:
     * ADR-0122 A3's ceiling rule applied to settings the same way it already applies to capabilities.
     */
    function organizationForces(key) {
      const organization = applied?.layers?.find((layer) => layer.kind === "organization");
      const setting = organization?.settings.find((entry) => entry.key === key);
      if (setting?.value !== "false") return null;
      return applied?.organization?.name ?? organization?.title ?? "Your organization";
    }

    function refreshSacred() {
      const patterns = splitHosts(draft?.settings.sacred ?? "");
      const readback = patterns.map((pattern) => hostReadback(pattern)).join("; ");
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
      if (!draft || key !== "browser.startup") return;
      if (value !== "on_demand" && value !== "manual") return;
      draft.settings.startup = value;
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
      el["rule-list"].innerHTML = rows.concat(mine).join("")
        || `<p class="rules-empty">No rules anywhere. Agents may work on ordinary websites.</p>`;
      if (draft) {
        el["observe-mode"].checked = draft.observe;
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

    function organizationRow(rule, layer) {
      const key = `org:${layer.title}:${rule.id}`;
      const open = opened.has(key);
      const mode = rule.mode === "observe" ? "watch only" : "enforced";
      const detail = `<div class="rule-detail">`
        + (rule.description ? `<p>${escapeHtml(rule.description)}</p>` : "")
        + `<dl><dt>Sites</dt><dd>${rule.allow.map(escapeHtml).join(", ") || "none"}</dd>`
        + (rule.deny.length ? `<dt>Never</dt><dd>${rule.deny.map(escapeHtml).join(", ")}</dd>` : "")
        + `<dt>Named</dt><dd>${escapeHtml(rule.id)}</dd>`
        + `<dt>When it decides</dt><dd>${mode}</dd></dl></div>`;
      return `<article class="rule rule-theirs${open ? " open" : ""}" data-rule-key="${escapeHtml(key)}">`
        + `<button class="rule-row" type="button" data-rule-toggle="${escapeHtml(key)}"`
        + ` aria-expanded="${open}">`
        + `<span class="rule-sentence">${sentenceFor(rule.allow, rule.deny, rule.allowed)}</span>`
        + `<span class="rule-owner">${escapeHtml(layer.title)}</span></button>`
        + (open ? detail : "")
        + `</article>`;
    }

    function userRow(rule, index) {
      const key = `mine:${index}`;
      const open = opened.has(key);
      const capabilities = CAPABILITY_ORDER.filter((capability) => rule.allowed.has(capability));
      const sentence = sentenceFor(splitHosts(rule.hosts), [], capabilities);
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
     * A draft with no rules is a policy that refuses everything, so neither applying it nor asking
     * what it would have done is a question worth answering. Saying that once, plainly, beats
     * letting someone press a button and read a number that sounds like an accusation.
     */
    function editorReady() {
      if (!draft) return;
      // Rules are what make a policy decide anything. Restrictions alone still need one, because a
      // rule-less policy refuses everything regardless of what else it says.
      const empty = !draft.rules.length;
      el["apply-policy"].disabled = empty || !draft.dirty;
      el["check-policy"].disabled = empty;
      if (empty) {
        previewCleared();
        el["editor-status"].textContent =
          "No rules yet. A policy with none refuses everything, so add at least one.";
        return;
      }
      // Clearing matters as much as setting: the empty-draft warning outlived the empty draft and
      // sat under two perfectly good rules, contradicting them.
      if (el["editor-status"].textContent.startsWith("No rules yet")) {
        el["editor-status"].textContent = "";
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

    /** Compose the document the orchestrator will validate. The surface authors no semantics. */
    function draftDocument() {
      if (!draft) return null;
      return JSON.stringify({
        schema: 3,
        name: "Your rules",
        version: new Date().toISOString().slice(0, 10),
        mode: draft.observe ? "observe" : "enforce",
        grants: draft.rules.map((rule, index) => {
          const grant = {
            id: rule.id && /^[A-Za-z0-9._-]+$/.test(rule.id) ? rule.id : `rule-${index + 1}`,
            hosts: { allow: splitHosts(rule.hosts) },
            allowed: CAPABILITY_ORDER.filter((capability) => rule.allowed.has(capability))
          };
          if (rule.description.trim()) grant.description = rule.description.trim();
          return grant;
        }),
        config: settingEntries()
      }, null, 2);
    }

    /**
     * The settings the draft authors, in their registered typed shapes.
     *
     * `level` is not a choice offered here. Both levels only tighten in 1.0, and nothing sits below
     * this layer for a recommendation to be relaxed by, so asking would be a word without a
     * consequence.
     */
    function settingEntries() {
      const entries = [...draft.settings.restricted]
        .map((key) => ({ key, value: false, level: "mandatory" }));
      if (draft.settings.startup) {
        entries.push({ key: "browser.startup", value: draft.settings.startup, level: "mandatory" });
      }
      const sacred = splitHosts(draft.settings.sacred);
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

    function setObserve(on) {
      if (!draft) return;
      draft.observe = on;
      draft.dirty = true;
      el["discard-policy"].hidden = false;
      editorReady();
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
      passport = snapshot.configuration?.managed_policy ?? null;
      ifChanged("connections", [snapshot.sessions, snapshot.browsers], () => connections(snapshot));
      ifChanged("about", [snapshot.service, snapshot.sessions.length, snapshot.browsers.length, snapshot.history.length], () => about(snapshot));
      ifChanged("integrations", [snapshot.harnesses, [...pending]], () => integrations(snapshot, pending));
      ifChanged("status", [snapshot.diagnostics, snapshot.configuration, snapshot.service], () => status(snapshot));
    }

    /* -------------------------------- chrome ------------------------------ */

    function navigate(view) {
      if (!VIEWS[view]) return;
      for (const node of document.querySelectorAll(".view")) {
        node.classList.toggle("active", node.dataset.page === view);
      }
      for (const tab of document.querySelectorAll(".tab")) {
        const active = tab.dataset.view === view;
        tab.classList.toggle("active", active);
        if (active) tab.setAttribute("aria-current", "page");
        else tab.removeAttribute("aria-current");
      }
      el["main-content"].scrollTop = 0;
    }

    function toast(message, error = false) {
      clearTimeout(toastTimer);
      el.toast.textContent = message;
      el.toast.className = `toast${error ? " error" : ""}`;
      el.toast.hidden = false;
      toastTimer = setTimeout(() => { el.toast.hidden = true; }, TOAST_MS);
    }

    function openHarnessManual(id) {
      const manual = document.querySelector(`[data-harness-manual="${globalThis.CSS?.escape?.(id) ?? id}"]`);
      if (manual) manual.open = true;
    }

    function openPalette() {
      el.palette.hidden = false;
      el["palette-query"].value = "";
      el["palette-results"].innerHTML = "";
      el["palette-query"].focus();
    }

    const closePalette = () => { el.palette.hidden = true; };
    const paletteOpen = () => !el.palette.hidden;
    const paletteQuery = () => el["palette-query"].value;

    function searchResults(hits) {
      el["palette-results"].innerHTML = hits.length
        ? hits.map((hit) => `<button class="hit" type="button" data-search-view="${escapeHtml(hit.view)}">`
          + `<span class="hit-kind">${escapeHtml(words(hit.kind))}</span>`
          + `<span><strong>${escapeHtml(hit.title)}</strong><small>${escapeHtml(hit.detail)}</small></span></button>`).join("")
        : '<div class="palette-empty">No matching Ghostlight records.</div>';
    }

    const searchFailed = (error) =>
      { el["palette-results"].innerHTML = `<div class="palette-empty">${escapeHtml(String(error))}</div>`; };

    function confirmRemoval(name) {
      if (confirmation) return Promise.resolve(false);
      el["confirm-title"].textContent = `Remove Ghostlight from ${name}?`;
      el["confirm-detail"].textContent = "Only the entry Ghostlight owns will be removed.";
      return new Promise((resolve) => {
        const finish = (confirmed) => {
          el["confirm-dialog"].hidden = true;
          confirmation = null;
          document.removeEventListener("keydown", onKeyDown);
          resolve(confirmed);
        };
        const onKeyDown = (event) => { if (event.key === "Escape") finish(false); };
        confirmation = finish;
        el["confirm-dialog"].hidden = false;
        el["confirm-dialog"].querySelector('[data-confirm="cancel"]').focus();
        document.addEventListener("keydown", onKeyDown);
      });
    }

    const answerConfirmation = (confirmed) => {
      if (!confirmation) return false;
      confirmation(confirmed);
      return true;
    };

    /* ------------------------------ live clocks --------------------------- */

    function tickElapsed(feed) {
      const top = feed[0];
      if (top && isRunning(top)) {
        const node = document.getElementById("elapsed");
        if (node) node.textContent = stopwatch(Date.now() - (top.startedAt ?? Date.now()));
      }
      for (const entry of feed.slice(1)) {
        if (!isRunning(entry)) continue;
        const cell = rowNodes.get(entry.invocation)?.querySelector(".row-dur");
        if (cell) cell.textContent = stopwatch(Date.now() - (entry.startedAt ?? Date.now()));
      }
    }

    function tickAges(feed) {
      for (const entry of feed.slice(1)) {
        if (!entry.endedAt) continue;
        const cell = rowNodes.get(entry.invocation)?.querySelector(".row-when");
        if (cell) cell.textContent = ago(entry.endedAt);
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
      band, collections, navigate, toast, openHarnessManual, selectHarnessProduct,
      policy, draftDocument, draftIsDirty, editRule, toggleCapability, ruleAction, addRule, toggleRule,
      setPermission, setChoice, setSacred,
      setObserve, discardDraft, renderRules, previewResult, previewCleared, editorStatus,
      openPalette, closePalette, paletteOpen, paletteQuery, searchResults, searchFailed,
      confirmRemoval, answerConfirmation,
      tickElapsed, tickAges, armCard
    });
  }

  return Object.freeze({ create });
});
