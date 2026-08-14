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
        ? `<p class="hero-reason">${escapeHtml(words(entry.reason))}</p>`
        : "";

      return `<div class="hero-tool">${escapeHtml(entry.tool)}<span class="cap-label">${escapeHtml(entry.capability ?? "read")}</span></div>`
        + `<p class="hero-activity">${escapeHtml(sentence(entry))}</p>`
        + reason
        + (meta.length ? `<div class="hero-meta">${meta.join("")}</div>` : "");
    }

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

    function policyState(config = {}) {
      const sources = [
        {
          name: "Local",
          configured: Boolean(config.local_policy_configured),
          active: Boolean(config.local_policy_active),
          valid: Boolean(config.local_policy_valid)
        },
        {
          name: "Managed",
          configured: Boolean(config.managed_authority_configured),
          active: Boolean(config.managed_authority_active),
          valid: Boolean(config.managed_authority_valid)
        }
      ];
      const active = sources.filter((source) => source.active);
      const unavailable = sources.filter((source) => source.configured && !source.active);
      const stale = sources.filter((source) => source.active && !source.valid);

      if (unavailable.length) {
        return {
          label: "Policy issue",
          tone: "failing",
          detail: `${unavailable.map((source) => source.name).join(" and ")} policy is unavailable; work fails closed.`
        };
      }
      if (!active.length) {
        return { label: "All open", tone: "open", detail: "No authored policy is applied." };
      }
      const label = active.length === 1 ? "Policy applied" : `${active.length} policies applied`;
      const names = active.map((source) => source.name).join(" and ");
      return stale.length
        ? { label, tone: "warning", detail: `${names} policy remains applied; its latest reload needs attention.` }
        : { label, tone: "applied", detail: `${names} policy is applied.` };
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

      const policy = policyState(facts.snapshot?.configuration);
      el["policy-state-label"].textContent = policy.label;
      el["policy-state"].dataset.tone = policy.tone;
      el["policy-state"].title = policy.detail;
      el["policy-state"].setAttribute("aria-label", `Open Status. ${policy.detail}`);

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

      const config = snapshot.configuration;
      const sources = [
        ["Local policy", config.local_policy_configured, config.local_policy_active, config.local_policy_valid, "Authority rules you own."],
        ["Managed authority", config.managed_authority_configured, config.managed_authority_active, config.managed_authority_valid, "A monotonic managed restriction layer."],
        ["Runtime control file", config.runtime_control_file_configured, config.runtime_control_file_configured, true, "A local final-boundary control source."]
      ];
      el["authority-grid"].innerHTML = sources.map(([title, configured, active, valid, detail]) => {
        const severity = !configured ? "" : active && valid ? "passing" : active ? "warning" : "failing";
        const label = !configured ? "not configured" : active && valid ? "applied" : active
          ? "applied; latest reload invalid" : "invalid, failing closed";
        return `<article class="card"><span class="severity ${severity}"><span class="dot"></span>${escapeHtml(label)}</span>`
          + `<h2>${escapeHtml(title)}</h2><p>${escapeHtml(detail)}</p></article>`;
      }).join("");

      const passport = config.managed_policy;
      if (passport?.configured) {
        const organization = passport.organization || "Managed policy";
        const freshness = words(passport.freshness);
        const sequence = passport.sequence == null ? "No verified sequence" : `Verified sequence ${passport.sequence}`;
        const source = passport.source_class === "https" ? "HTTPS" : words(passport.source_class);
        const checked = passport.last_success_ms
          ? ` Last verified ${new Date(passport.last_success_ms).toLocaleString()}.`
          : "";
        const contacts = (passport.contacts || []).map((contact) => {
          const label = contact.label || words(contact.kind);
          return `${label}: ${contact.value}`;
        });
        const detail = [passport.rationale, `${sequence} from ${source}. ${freshness}.${checked}`, ...contacts]
          .filter(Boolean)
          .join(" ");
        const severity = passport.verified ? (passport.freshness === "fresh" ? "passing" : "") : "failing";
        el["authority-grid"].insertAdjacentHTML("beforeend",
          `<article class="card"><span class="severity ${severity}"><span class="dot"></span>${escapeHtml(freshness)}</span>`
          + `<h2>${escapeHtml(organization)}</h2><p>${escapeHtml(detail)}</p></article>`);
      }

      const started = snapshot.service.started_at_ms
        ? new Date(snapshot.service.started_at_ms).toLocaleString()
        : "unknown";
      el.colophon.textContent =
        `Ghostlight ${snapshot.service.version} - running since ${started} - everything on this page stays on this device.`;
    }

    /** Repaint a section only when its own facts changed, so a safety pull never rewrites a
     * surface the user is pointing at. */
    function collections(snapshot, pending) {
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
      openPalette, closePalette, paletteOpen, paletteQuery, searchResults, searchFailed,
      confirmRemoval, answerConfirmation,
      tickElapsed, tickAges, armCard
    });
  }

  return Object.freeze({ create });
});
