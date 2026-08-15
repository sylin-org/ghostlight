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
    CAPABILITY_ORDER, hostReadback, patternCovers,
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

    function bandClass(facts) {
      if (!facts.connected) return "runtime-offline";
      if (facts.runtime === "held" || facts.runtime === "ended") return "runtime-held";
      if (facts.runtime === "attention") return "runtime-attention";
      return facts.working ? "runtime-working" : "runtime-quiet";
    }

    function bandWord(name, runtime) {
      if (name === "runtime-offline") return "Not connected";
      if (name === "runtime-held") return runtime === "ended" ? "Session ended" : "Paused";
      if (name === "runtime-attention") return "Needs you";
      if (name === "runtime-working") return "Working";
      return "Quiet";
    }

    function band(facts) {
      const name = bandClass(facts);
      document.body.className = name;
      el["state-word"].textContent = bandWord(name, facts.runtime);

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
      el.wheel.disabled = !facts.connected;
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

    function integrations(snapshot, pending) {
      const order = { installed: 0, updatable: 1, needs_attention: 2, available: 3, not_detected: 4 };
      const harnesses = [...snapshot.harnesses].sort((left, right) =>
        (order[left.state] ?? 9) - (order[right.state] ?? 9) || left.name.localeCompare(right.name));

      if (!harnesses.length) {
        el["integration-grid"].innerHTML =
          '<div class="empty">No supported MCP client was found for this user.</div>';
        return;
      }

      el["integration-grid"].innerHTML = harnesses.map((harness) => {
        const installed = harness.state === "installed";
        const updatable = harness.state === "updatable";
        const waiting = pending.has(harness.id);
        const allowed = installed ? harness.can_uninstall : harness.can_install;
        const tone = installed ? " connected" : harness.state === "needs_attention" ? " attention" : "";
        const label = waiting ? "Working" : installed ? "Connected" : words(harness.state);
        const action = installed ? "uninstall" : "install";
        const verb = installed ? "Disconnect" : updatable ? "Update" : "Connect";
        const button = installed ? "danger-button" : "ghost-button";
        return `<article class="tile${tone}">`
          + `<div class="tile-top"><h2>${escapeHtml(harness.name)}</h2>`
          + `<span class="tile-state">${escapeHtml(label)}</span></div>`
          + `<p>${escapeHtml(harness.detail)}</p>`
          + `<div class="tile-actions"><button class="${button}" type="button" data-harness-action="${action}"`
          + ` data-harness="${escapeHtml(harness.id)}" data-harness-name="${escapeHtml(harness.name)}"`
          + `${waiting || !allowed ? " disabled" : ""}>${waiting ? "Working..." : verb}</button></div>`
          + `</article>`;
      }).join("");
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
        const tone = line.state === "available" ? "ok" : line.state === "sites" ? "some" : "no";
        const badge = line.state === "available" ? "Allowed"
          : line.state === "sites" ? "Some sites" : "Not allowed";
        return `<article class="cap cap-${tone}">`
          + `<div class="cap-top"><h3>${escapeHtml(line.label)}</h3>`
          + `<span class="cap-state">${badge}</span></div>`
          + `<p class="cap-covers">${escapeHtml(line.covers)}</p>`
          + `<p class="cap-detail">${escapeHtml(line.detail)}</p></article>`;
      }).join("");

      el["policy-ceilings"].innerHTML = view.ceilings
        .map((line) => `<li>${escapeHtml(line)}</li>`).join("");

      el["policy-layers"].innerHTML = view.layers.map(layerSection).join("");

      const user = view.user_layer;
      el["policy-editor"].hidden = !user.editable;
      el["policy-blocked"].hidden = user.editable || !user.blocked_reason;
      if (user.blocked_reason) el["policy-blocked-reason"].textContent = user.blocked_reason;
      el["policy-remove"].hidden = !(user.editable && user.source === "workbench" && hasUserLayer(view));

      if (user.editable) {
        draft = draftFrom(view);
        renderRules();
      }
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

    function layerSection(layer) {
      const rules = layer.rules.length
        ? layer.rules.map((rule) => {
          const note = rule.note === "unreachable"
            ? `<span class="rule-note">An earlier rule already covers this, so it never applies.</span>`
            : rule.note === "no_effect"
              ? `<span class="rule-note">Your organization already refuses this, so it changes nothing.</span>`
              : "";
          const hosts = rule.allow.length ? rule.allow.map(escapeHtml).join(", ") : "no sites";
          const except = rule.deny.length ? ` except ${rule.deny.map(escapeHtml).join(", ")}` : "";
          const verbs = rule.allowed.map((capability) => words(capability)).join(", ") || "nothing";
          return `<li class="rule-read${rule.note ? " rule-inert" : ""}">`
            + `<p><b>On ${hosts}</b>${except}, agents may ${escapeHtml(verbs)}.</p>`
            + (rule.description ? `<p class="rule-why">${escapeHtml(rule.description)}</p>` : "")
            + note
            + `<span class="rule-mode">${escapeHtml(rule.mode === "observe" ? "watch only" : "enforced")}</span></li>`;
        }).join("")
        : `<li class="rule-read"><p>No rules. Nothing is allowed by this policy.</p></li>`;
      const settings = layer.settings.length
        ? `<ul class="settings">${layer.settings.map((setting) =>
          `<li><code>${escapeHtml(setting.key)}</code> = ${escapeHtml(setting.value)}`
          + ` <span class="level">${escapeHtml(setting.level)}</span></li>`).join("")}</ul>`
        : "";
      const source = layer.path ? `<p class="layer-source">${escapeHtml(layer.path)}</p>` : "";
      const document = layer.document
        ? `<details class="layer-doc"><summary>Show the exact document</summary><pre>${escapeHtml(layer.document)}</pre></details>`
        : "";
      return `<section class="layer" data-layer="${escapeHtml(layer.kind)}">`
        + `<h2 class="subhead">${escapeHtml(layer.title)}</h2>`
        + `<p class="layer-meta">${escapeHtml(layer.policy_name)} ${escapeHtml(layer.version)}`
        + ` &middot; ${escapeHtml(layer.mode === "observe" ? "watch only" : "enforced")}</p>`
        + source
        + `<ul class="rules-read">${rules}</ul>`
        + settings
        + document
        + `</section>`;
    }

    /* ------------------------------ the editor ----------------------------- */

    /** Turn the applied user layer into an editable draft, or start an empty one. */
    function draftFrom(view) {
      const layer = view.layers.find((entry) => entry.kind === "user");
      if (!layer) return { rules: [], observe: false, dirty: false };
      return {
        observe: layer.mode === "observe",
        dirty: false,
        rules: layer.rules.map((rule) => ({
          id: rule.id,
          hosts: rule.allow.join(", "),
          description: rule.description || "",
          allowed: new Set(rule.allowed)
        }))
      };
    }

    /** Which capabilities an organization has already refused everywhere. */
    function ceilingFor(capability) {
      const line = applied?.capabilities?.find((entry) => entry.capability === capability);
      if (!line || line.state !== "unavailable") return null;
      return line.detail;
    }

    function renderRules() {
      if (!draft) return;
      el["rule-list"].innerHTML = draft.rules.map((rule, index) => {
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
        const readback = rule.hosts
          .split(",")
          .map((pattern) => pattern.trim())
          .filter(Boolean)
          .map((pattern) => hostReadback(pattern))
          .join("; ");
        const shadow = shadowedBy(index);
        return `<article class="rule-edit">`
          + `<div class="rule-line"><span>On</span>`
          + `<input class="hosts" type="text" data-rule="${index}" data-field="hosts"`
          + ` value="${escapeHtml(rule.hosts)}" placeholder="example.com, *.example.com" spellcheck="false">`
          + `<span>agents may</span></div>`
          + `<div class="cap-boxes">${boxes}</div>`
          + (readback ? `<p class="readback">Matches ${escapeHtml(readback)}.</p>` : `<p class="readback readback-empty">Add a site for this rule to do anything.</p>`)
          + (shadow ? `<p class="rule-note">Rule ${shadow} above already covers this, so this one never applies. Move it up to use it.</p>` : "")
          + `<div class="rule-foot">`
          + `<input class="why" type="text" data-rule="${index}" data-field="description"`
          + ` value="${escapeHtml(rule.description)}" placeholder="What is this rule for? (optional)" maxlength="200">`
          + `<button class="link-button" type="button" data-rule="${index}" data-rule-action="up"${index === 0 ? " disabled" : ""}>Move up</button>`
          + `<button class="link-button" type="button" data-rule="${index}" data-rule-action="remove">Remove</button>`
          + `</div></article>`;
      }).join("");
      el["observe-mode"].checked = draft.observe;
      el["apply-policy"].disabled = !draft.dirty;
      el["discard-policy"].hidden = !draft.dirty;
      if (!draft.rules.length) {
        el["editor-status"].textContent =
          "No rules yet. With none, this policy refuses everything, so add at least one before applying.";
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
        })
      }, null, 2);
    }

    function editRule(index, field, value) {
      if (!draft?.rules[index]) return;
      draft.rules[index][field] = value;
      draft.dirty = true;
      el["apply-policy"].disabled = false;
      el["discard-policy"].hidden = false;
      if (field === "hosts") refreshRuleHints(index);
    }

    /**
     * Update one rule's readback in place.
     *
     * Redrawing the whole list on every keystroke would take the caret with it, so the hints that
     * depend on what was just typed are the only thing that changes while a person is typing.
     */
    function refreshRuleHints(index) {
      const card = el["rule-list"].children[index];
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
      if (action === "remove") draft.rules.splice(index, 1);
      if (action === "up" && index > 0) {
        const [moved] = draft.rules.splice(index, 1);
        draft.rules.splice(index - 1, 0, moved);
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
      renderRules();
      const inputs = el["rule-list"].querySelectorAll(".hosts");
      inputs[inputs.length - 1]?.focus();
    }

    function setObserve(on) {
      if (!draft) return;
      draft.observe = on;
      draft.dirty = true;
      el["apply-policy"].disabled = false;
      el["discard-policy"].hidden = false;
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
      el["confirm-title"].textContent = `Disconnect Ghostlight from ${name}?`;
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
      band, collections, navigate, toast,
      policy, draftDocument, draftIsDirty, editRule, toggleCapability, ruleAction, addRule,
      setObserve, discardDraft, renderRules, previewResult, previewCleared, editorStatus,
      openPalette, closePalette, paletteOpen, paletteQuery, searchResults, searchFailed,
      confirmRemoval, answerConfirmation,
      tickElapsed, tickAges, armCard
    });
  }

  return Object.freeze({ create });
});
