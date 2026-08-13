"use strict";

/*
 * Ghostlight workbench.
 *
 * The orchestrator is the only authority. This surface holds a cache it can always prove is
 * current: every change arrives with a monotonic sequence, and a gap means the cache is thrown
 * away and rebuilt from a fresh snapshot. Nothing here is ever the source of truth.
 */

const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;

/*
 * What this window is built from.
 *
 * Words is the fixed vocabulary and its number formatting. Entries is the shape one row of the
 * monitor takes. Both are pure -- no state, no document -- which is what lets them be read, and
 * tested, without a browser anywhere near them.
 */
const {
  CHANGE_EVENT, HEARTBEAT_MS, FEED_LIMIT, WORKING_LATCH_MS, VIEWS, SEARCH_VIEWS,
  GLYPHS, EFFECT_STORY, READINESS_NOTE, DESTINATIONS, glyphFor, capabilityClass,
  escapeHtml, words, duration, stopwatch, ago, shortId
} = globalThis.GhostlightWords;
const {
  entryFromOperation, entryFromRecord, entryTime, settledMs, isRunning, isBlocked
} = globalThis.GhostlightEntries;

const state = {
  seq: 0,
  connected: false,
  view: "monitor",
  runtime: "active",
  snapshot: null,
  feed: [],
  hiddenInvocations: new Set(),
  rowNodes: new Map(),
  pendingHarnesses: new Set(),
  painted: {},
  confirmation: null,
  toastTimer: null,
  searchTimer: null,
  interactionAt: 0,
  latchTimer: null,
  lastFailure: null
};

/*
 * Every element with an id, looked up once.
 *
 * This was a hand-maintained list, and a node added to the markup without being added here read
 * as undefined and threw on first use. Deriving it from the document means a new id cannot be
 * forgotten, because there is nothing left to remember.
 */
const el = Object.create(null);
for (const node of document.querySelectorAll("[id]")) el[node.id] = node;

function trimFeed() {
  while (state.feed.length > FEED_LIMIT) {
    const dropped = state.feed.pop();
    state.rowNodes.get(dropped.invocation)?.remove();
    state.rowNodes.delete(dropped.invocation);
  }
}

/* --------------------------- monitor rendering -------------------------- */

/**
 * What a readiness adds to a settled row.
 *
 * Complete is the quiet case and earns no words. The others are the difference between "2.5s",
 * which is reassurance, and "8.0s, never settled", which explains why an agent looked stuck.
 */

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
 * The orchestrator already chose the outcome's useful register and made every named measurement
 * agree with the structured observation. The surface renders that sentence without guessing.
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
 * A settled row carries it on the record. A running one has no record yet, so it resolves through
 * the session that admitted it, which is still connected while the work is in flight.
 */
function channelFor(entry) {
  if (entry.channel) return entry.channel;
  const session = state.snapshot?.sessions.find((item) => item.id === entry.workspace);
  return session?.channel ?? "";
}

/** The client that asked, resolved through the current sessions when it is still connected. */
function clientFor(workspace) {
  const session = state.snapshot?.sessions.find((item) => item.id === workspace);
  return session ? session.client_label : shortId(workspace);
}

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

function paintHero(entry, animate) {
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
  const row = document.createElement("div");
  row.className = rowClass(entry) + (landing ? " landing" : "");
  row.innerHTML = rowMarkup(entry);
  if (landing) {
    row.addEventListener("animationend", () => row.classList.remove("landing"), { once: true });
  }
  state.rowNodes.set(entry.invocation, row);
  return row;
}

function paintRow(entry) {
  const row = state.rowNodes.get(entry.invocation);
  if (!row) return;
  row.className = rowClass(entry);
  row.innerHTML = rowMarkup(entry);
}

function paintQueueCount() {
  const rows = Math.max(0, state.feed.length - 1);
  el["queue-count"].textContent = rows ? `${rows} ${rows === 1 ? "action" : "actions"}` : "";
  el["clear-monitor"].disabled = !state.feed.some(entry => !isRunning(entry));
}

function clearMonitorView() {
  const completed = state.feed.filter(entry => !isRunning(entry));
  if (!completed.length) return;
  for (const entry of completed) state.hiddenInvocations.add(entry.invocation);
  state.feed = state.feed.filter(isRunning);
  rebuildFeedDom();
  paintLamp();
  const count = completed.length;
  showToast(`Cleared ${count} ${count === 1 ? "entry" : "entries"} from this view. Audit history is unchanged.`);
}

/** Full rebuild. Only used on first paint and after a detected sequence gap. */
function rebuildFeedDom() {
  el.queue.replaceChildren();
  state.rowNodes.clear();
  paintHero(state.feed[0], false);
  const rows = document.createDocumentFragment();
  for (const entry of state.feed.slice(1)) rows.append(buildRow(entry, false));
  el.queue.append(rows);
  if (state.feed.length <= 1) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "Nothing earlier in this session.";
    el.queue.append(empty);
  }
  paintQueueCount();
}

/** The conveyor: whatever held the hero slides down, the new action rises. */
function promote(entry) {
  const previous = state.feed[0];
  state.feed.unshift(entry);
  if (previous && previous.invocation !== entry.invocation) {
    el.queue.querySelector(".empty")?.remove();
    el.queue.prepend(buildRow(previous, true));
  }
  trimFeed();
  paintHero(entry, true);
  paintQueueCount();
}

function applyStarted(operation) {
  if (state.hiddenInvocations.has(operation.invocation)) return;
  const entry = entryFromOperation(operation);
  const index = state.feed.findIndex(item => item.invocation === entry.invocation);
  if (index === 0) {
    state.feed[0] = { ...state.feed[0], ...entry };
    paintHero(state.feed[0], false);
    return;
  }
  if (index > 0) {
    state.rowNodes.get(entry.invocation)?.remove();
    state.rowNodes.delete(entry.invocation);
    state.feed.splice(index, 1);
  }
  promote(entry);
}

function applyChanged(operation) {
  if (state.hiddenInvocations.has(operation.invocation)) return;
  const index = state.feed.findIndex(item => item.invocation === operation.invocation);
  if (index < 0) return applyStarted(operation);
  state.feed[index] = { ...state.feed[index], ...entryFromOperation(operation) };
  if (index === 0) paintHero(state.feed[0], false);
  else paintRow(state.feed[index]);
}

function applySettled(record) {
  if (state.hiddenInvocations.has(record.invocation)) return;
  const index = state.feed.findIndex(item => item.invocation === record.invocation);
  if (index < 0) {
    promote(entryFromRecord(record, null));
    return;
  }
  state.feed[index] = entryFromRecord(record, state.feed[index]);
  if (index === 0) paintHero(state.feed[0], false);
  else paintRow(state.feed[index]);
}

/* ------------------------------- lamp band ------------------------------ */

function working() {
  if (state.feed.some(isRunning)) return true;
  return Date.now() - state.interactionAt < WORKING_LATCH_MS;
}

/**
 * Mark that something interacted with Ghostlight, and arrange to notice when that stops.
 *
 * Nothing else will wake the band once the last operation settles, so the latch has to schedule
 * its own expiry or the word would stay lit until the next unrelated repaint.
 */
function touchInteraction() {
  state.interactionAt = Date.now();
  clearTimeout(state.latchTimer);
  state.latchTimer = setTimeout(paintLamp, WORKING_LATCH_MS + 60);
}

function runtimeClass() {
  if (!state.connected) return "runtime-offline";
  if (state.runtime === "held" || state.runtime === "ended") return "runtime-held";
  if (state.runtime === "attention") return "runtime-attention";
  return working() ? "runtime-working" : "runtime-quiet";
}

function runtimeWord(name) {
  if (name === "runtime-offline") return "Not connected";
  if (name === "runtime-held") return state.runtime === "ended" ? "Session ended" : "Paused";
  if (name === "runtime-attention") return "Needs you";
  if (name === "runtime-working") return "Working";
  return "Quiet";
}

function paintLamp() {
  const name = runtimeClass();
  document.body.className = name;
  el["state-word"].textContent = runtimeWord(name);

  const snapshot = state.snapshot;
  if (!snapshot) {
    el["state-facts"].textContent = "";
  } else {
    const running = state.feed.filter(isRunning).length;
    el["state-facts"].innerHTML = `<b>${snapshot.sessions.length}</b> sessions`
      + ` &middot; <b>${snapshot.browsers.length}</b> browsers`
      + ` &middot; <b>${running}</b> running`
      + ` &middot; <b>${snapshot.history.length}</b> recorded`;
  }

  const paused = state.runtime !== "active";
  el.wheel.disabled = !state.connected;
  el.wheel.dataset.intent = state.runtime === "ended" ? "start_session" : paused ? "resume" : "hold";
  el["wheel-label"].textContent = state.runtime === "ended" ? "Start session" : paused ? "Resume" : "Pause";
  el["wheel-icon"].innerHTML = paused
    ? '<path d="M7 4.5 19 12 7 19.5z"/>'
    : '<rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/>';
}

/* ------------------------------ collections ----------------------------- */

/**
 * One chip per client, not per connection.
 *
 * A client that reconnects often -- an editor that starts a session per request, a shell running
 * `ghostlight call` in a loop -- is one thing the user recognizes, and a row of identical names
 * tells them nothing they did not already know. The sessions themselves stay separate everywhere
 * that matters: each keeps its own workspace, its own tabs, and its own history attribution.
 */
function connectionGroups(sessions) {
  const groups = new Map();
  for (const session of sessions) {
    const group = groups.get(session.client_label) ?? { label: session.client_label, count: 0, tabs: 0, busy: false };
    group.count += 1;
    group.tabs += session.tab_count;
    group.busy = group.busy || session.active_operations > 0;
    groups.set(session.client_label, group);
  }
  return [...groups.values()].sort((left, right) => right.tabs - left.tabs || left.label.localeCompare(right.label));
}

function paintConnections(snapshot) {
  const chips = connectionGroups(snapshot.sessions).map(group => {
    // The count earns its place only when there is more than one, so the common case stays quiet.
    const many = group.count > 1 ? ` <span class="tally">${group.count}</span>` : "";
    return `<span class="chip ${group.busy ? "busy" : "on"}"><span class="dot"></span>${escapeHtml(group.label)}${many}`
      + `<small>${group.tabs} tabs</small></span>`;
  });
  chips.push(...snapshot.browsers.map(browser =>
    `<span class="chip on"><span class="dot"></span>${escapeHtml(browser.family)}`
    + `<small>adapter ${escapeHtml(browser.adapter_version ?? "unknown")}</small></span>`));
  if (!chips.length) {
    chips.push('<span class="chip"><span class="dot"></span>Waiting for a client or a browser</span>');
  }
  el.connections.innerHTML = chips.join("");
}

/* --------------------------------- about -------------------------------- */

function paintLinks() {
  el["about-links"].innerHTML = DESTINATIONS.map(([heading, rows]) => {
    const items = rows.map(([destination, title, blurb]) =>
      `<button class="about-link" type="button" data-destination="${escapeHtml(destination)}">`
      + `<span class="about-link-title">${escapeHtml(title)}</span>`
      + `<span class="about-link-blurb">${escapeHtml(blurb)}</span></button>`).join("");
    return `<section class="about-group"><h2>${escapeHtml(heading)}</h2><div>${items}</div></section>`;
  }).join("");
}

/** Every destination opens in the browser you already use, which is the one Ghostlight drives. */
async function openDestination(destination) {
  try {
    await invoke("open_destination", { destination });
  } catch (error) {
    showToast(String(error), true);
  }
}

function paintAbout(snapshot) {
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

/**
 * The card's sheen and foil follow the pointer, and let go when it leaves.
 *
 * Everything the effect needs is a custom property, so the work here is two numbers per move and
 * the compositor does the rest.
 */
function armAboutCard() {
  const card = el["about-card"];
  if (!card) return;
  paintLinks();
  document.addEventListener("click", event => {
    const target = event.target.closest("[data-destination]");
    if (target) openDestination(target.dataset.destination);
  });
  card.addEventListener("pointermove", event => {
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

function paintIntegrations(snapshot) {
  const order = { installed: 0, needs_attention: 1, available: 2, not_detected: 3 };
  const harnesses = [...snapshot.harnesses].sort((left, right) =>
    (order[left.state] ?? 9) - (order[right.state] ?? 9) || left.name.localeCompare(right.name));

  if (!harnesses.length) {
    el["integration-grid"].innerHTML = '<div class="empty">No supported MCP client was found for this user.</div>';
    return;
  }

  el["integration-grid"].innerHTML = harnesses.map(harness => {
    const installed = harness.state === "installed";
    const pending = state.pendingHarnesses.has(harness.id);
    const allowed = installed ? harness.can_uninstall : harness.can_install;
    const tone = installed ? " connected" : harness.state === "needs_attention" ? " attention" : "";
    const label = pending ? "Working" : installed ? "Connected" : words(harness.state);
    const action = installed ? "uninstall" : "install";
    const verb = installed ? "Disconnect" : "Connect";
    const button = installed ? "danger-button" : "ghost-button";
    return `<article class="tile${tone}">`
      + `<div class="tile-top"><h2>${escapeHtml(harness.name)}</h2>`
      + `<span class="tile-state">${escapeHtml(label)}</span></div>`
      + `<p>${escapeHtml(harness.detail)}</p>`
      + `<div class="tile-actions"><button class="${button}" type="button" data-harness-action="${action}"`
      + ` data-harness="${escapeHtml(harness.id)}" data-harness-name="${escapeHtml(harness.name)}"`
      + `${pending || !allowed ? " disabled" : ""}>${pending ? "Working..." : verb}</button></div>`
      + `</article>`;
  }).join("");
}

function paintStatus(snapshot) {
  el["diagnostic-grid"].innerHTML = snapshot.diagnostics.map(item =>
    `<article class="card"><span class="severity ${escapeHtml(item.severity)}"><span class="dot"></span>`
    + `${escapeHtml(item.severity)}</span><h2>${escapeHtml(item.label)}</h2>`
    + `<p>${escapeHtml(item.detail)}</p></article>`).join("");

  const config = snapshot.configuration;
  const sources = [
    ["Local policy", config.local_policy_configured, config.local_policy_valid, "Authority rules you own."],
    ["Managed authority", config.managed_authority_configured, config.managed_authority_valid, "A monotonic managed restriction layer."],
    ["Runtime control file", config.runtime_control_file_configured, true, "A local final-boundary control source."]
  ];
  el["authority-grid"].innerHTML = sources.map(([title, configured, valid, detail]) => {
    const severity = !configured ? "" : valid ? "passing" : "failing";
    const label = !configured ? "not configured" : valid ? "valid" : "invalid, failing closed";
    return `<article class="card"><span class="severity ${severity}"><span class="dot"></span>${escapeHtml(label)}</span>`
      + `<h2>${escapeHtml(title)}</h2><p>${escapeHtml(detail)}</p></article>`;
  }).join("");

  const started = snapshot.service.started_at_ms ? new Date(snapshot.service.started_at_ms).toLocaleString() : "unknown";
  el.colophon.textContent = `Ghostlight ${snapshot.service.version} - running since ${started} - everything on this page stays on this device.`;
}

/* ------------------------------ synchronizing --------------------------- */

function seedFeed(snapshot) {
  const live = snapshot.operations.map(entryFromOperation);
  const settled = snapshot.history.map(record => entryFromRecord(record, null));
  const byInvocation = new Map();
  for (const entry of [...live, ...settled]) {
    if (!byInvocation.has(entry.invocation)) byInvocation.set(entry.invocation, entry);
  }
  state.feed = [...byInvocation.values()]
    .filter(entry => !state.hiddenInvocations.has(entry.invocation))
    .sort((left, right) => entryTime(right) - entryTime(left));
  trimFeed();
}

/**
 * Repaint a section only when its own facts changed, so the safety pull never
 * rewrites a surface the user is pointing at.
 */
/*
 * Nothing in this window is allowed to fail quietly.
 *
 * A surface that throws where nobody is looking is indistinguishable from a surface that is
 * merely slow, and the person waiting has no way to tell which. Every failure gets a visible
 * notice; identical failures are stated once so a repeating fault does not bury the screen.
 */
function reportFailure(what, error) {
  const detail = `${what}: ${error?.message ?? error}`;
  if (state.lastFailure === detail) return;
  state.lastFailure = detail;
  // The console is the last channel that still works when the surface itself is broken, so a
  // failure goes there whether or not there is anything left to render a notice with.
  console.error(detail, error);
  if (el.toast) showToast(detail, true);
}

/** Run one fallible step. Report what went wrong, and tell the caller it did not happen. */
function attempt(what, step) {
  try {
    step();
    return true;
  } catch (error) {
    reportFailure(what, error);
    return false;
  }
}

/**
 * Paint one panel when its facts changed, and never record a paint that did not happen.
 *
 * The signature used to be stored before the paint ran, and `painted` is never cleared, so a
 * panel that threw was remembered as finished and stayed blank for the life of the window.
 * Recording afterwards turns a failure into something the next change retries. Isolating the
 * paint keeps one bad panel from abandoning the rest of the pass.
 */
function paintIfChanged(key, facts, paint) {
  const signature = JSON.stringify(facts);
  if (state.painted[key] === signature) return;
  if (attempt(`painting ${key}`, paint)) state.painted[key] = signature;
}

function applySnapshot(snapshot, rebuildFeed) {
  state.snapshot = snapshot;
  state.seq = snapshot.seq;
  state.runtime = snapshot.service.runtime_state;
  if (rebuildFeed) {
    seedFeed(snapshot);
    rebuildFeedDom();
  }
  paintIfChanged("connections", [snapshot.sessions, snapshot.browsers], () => paintConnections(snapshot));
  paintIfChanged("about", [snapshot.service, snapshot.sessions.length, snapshot.browsers.length, snapshot.history.length], () => paintAbout(snapshot));
  paintIfChanged("integrations", [snapshot.harnesses, [...state.pendingHarnesses]], () => paintIntegrations(snapshot));
  paintIfChanged("status", [snapshot.diagnostics, snapshot.configuration, snapshot.service], () => paintStatus(snapshot));
  attempt("painting the band", paintLamp);
}

async function resync({ rebuildFeed = false, quiet = true } = {}) {
  if (!invoke) {
    state.connected = false;
    attempt("painting the band", paintLamp);
    return;
  }
  let snapshot;
  try {
    snapshot = await invoke("workbench_snapshot");
  } catch (error) {
    // Losing the orchestrator is an ordinary condition with a state of its own to show.
    state.connected = false;
    attempt("painting the band", paintLamp);
    if (!quiet) showToast(String(error), true);
    return;
  }
  state.connected = true;
  // A surface that failed to draw is not a surface that lost its connection. One catch around
  // both said "Not connected" for either, which sends the reader looking at the wrong thing.
  attempt("rendering the snapshot", () =>
    applySnapshot(snapshot, rebuildFeed || snapshot.seq !== state.seq || !state.snapshot));
}

function applyChange(event) {
  if (!event || typeof event.seq !== "number") return;
  if (event.seq !== state.seq + 1) {
    // A gap means this cache can no longer be trusted. Rebuild rather than guess.
    resync({ rebuildFeed: true });
    return;
  }
  state.seq = event.seq;
  const change = event.change;
  switch (change.kind) {
    case "operation_started": touchInteraction(); applyStarted(change.operation); break;
    case "operation_changed": touchInteraction(); applyChanged(change.operation); break;
    case "operation_settled": touchInteraction(); applySettled(change.record); break;
    case "runtime_changed": state.runtime = change.runtime_state; break;
    default: return;
  }
  paintLamp();
}

/* ------------------------------ live stopwatch -------------------------- */

setInterval(() => {
  if (document.hidden) return;
  const hero = state.feed[0];
  if (hero && isRunning(hero)) {
    const node = document.getElementById("elapsed");
    if (node) node.textContent = stopwatch(Date.now() - (hero.startedAt ?? Date.now()));
  }
  for (const entry of state.feed.slice(1)) {
    if (!isRunning(entry)) continue;
    const cell = state.rowNodes.get(entry.invocation)?.querySelector(".row-dur");
    if (cell) cell.textContent = stopwatch(Date.now() - (entry.startedAt ?? Date.now()));
  }
}, 100);

setInterval(() => {
  if (document.hidden) return;
  for (const entry of state.feed.slice(1)) {
    if (!entry.endedAt) continue;
    const cell = state.rowNodes.get(entry.invocation)?.querySelector(".row-when");
    if (cell) cell.textContent = ago(entry.endedAt);
  }
}, 15000);

/* -------------------------------- surfaces ------------------------------ */

function navigate(view) {
  if (!VIEWS[view]) return;
  state.view = view;
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

function showToast(message, error = false) {
  clearTimeout(state.toastTimer);
  el.toast.textContent = message;
  el.toast.className = `toast${error ? " error" : ""}`;
  el.toast.hidden = false;
  state.toastTimer = setTimeout(() => { el.toast.hidden = true; }, 4200);
}

function openPalette() {
  el.palette.hidden = false;
  el["palette-query"].value = "";
  el["palette-results"].innerHTML = "";
  el["palette-query"].focus();
}

function closePalette() {
  el.palette.hidden = true;
}

async function search(query) {
  const trimmed = query.trim();
  if (!trimmed) {
    el["palette-results"].innerHTML = "";
    return;
  }
  try {
    const hits = await invoke("workbench_search", { query: trimmed });
    el["palette-results"].innerHTML = hits.length
      ? hits.map(hit => `<button class="hit" type="button" data-search-view="${escapeHtml(hit.view)}">`
        + `<span class="hit-kind">${escapeHtml(words(hit.kind))}</span>`
        + `<span><strong>${escapeHtml(hit.title)}</strong><small>${escapeHtml(hit.detail)}</small></span></button>`).join("")
      : '<div class="palette-empty">No matching Ghostlight records.</div>';
  } catch (error) {
    el["palette-results"].innerHTML = `<div class="palette-empty">${escapeHtml(String(error))}</div>`;
  }
}

function confirmRemoval(name) {
  if (state.confirmation) return Promise.resolve(false);
  el["confirm-title"].textContent = `Disconnect Ghostlight from ${name}?`;
  el["confirm-detail"].textContent = "Only the entry Ghostlight owns will be removed.";
  return new Promise(resolve => {
    const finish = confirmed => {
      el["confirm-dialog"].hidden = true;
      state.confirmation = null;
      document.removeEventListener("keydown", onKeyDown);
      resolve(confirmed);
    };
    const onKeyDown = event => { if (event.key === "Escape") finish(false); };
    state.confirmation = finish;
    el["confirm-dialog"].hidden = false;
    el["confirm-dialog"].querySelector('[data-confirm="cancel"]').focus();
    document.addEventListener("keydown", onKeyDown);
  });
}

async function handleHarnessAction(button) {
  if (!invoke || button.disabled) return;
  const { harness: id, harnessAction: action, harnessName: name } = button.dataset;
  if (action === "uninstall" && !(await confirmRemoval(name))) return;
  state.pendingHarnesses.add(id);
  if (state.snapshot) paintIntegrations(state.snapshot);
  try {
    const result = await invoke("manage_harness", { id, action });
    showToast(result.message);
  } catch (error) {
    showToast(String(error), true);
  } finally {
    state.pendingHarnesses.delete(id);
    await resync();
  }
}

async function applyIntent(intent) {
  if (!invoke) return;
  try {
    const result = await invoke("apply_runtime_intent", { intent });
    showToast(result.message);
    await resync();
  } catch (error) {
    showToast(String(error), true);
  }
}

/* -------------------------------- wiring -------------------------------- */

/**
 * Attach every listener the surface needs.
 *
 * These ran as loose top-level statements, which put them ahead of boot: one listener failing to
 * attach took the first snapshot, the change subscription and the heartbeat down with it, because
 * nothing after the throw ever ran. They are one isolated step now, and they reach nodes through
 * the derived table so a missing id fails the build rather than the window.
 */
function wire() {
  document.addEventListener("click", event => {
    const confirmation = event.target.closest("[data-confirm]");
    if (confirmation && state.confirmation) {
      state.confirmation(confirmation.dataset.confirm === "remove");
      return;
    }
    const tab = event.target.closest("[data-view]");
    if (tab) navigate(tab.dataset.view);
    const intent = event.target.closest("[data-intent]");
    if (intent && !intent.disabled) applyIntent(intent.dataset.intent);
    const harness = event.target.closest("[data-harness-action]");
    if (harness) handleHarnessAction(harness);
    const hit = event.target.closest("[data-search-view]");
    if (hit) {
      navigate(SEARCH_VIEWS[hit.dataset.searchView] ?? "monitor");
      closePalette();
    }
    if (event.target === el.palette) closePalette();
  });

  el["palette-query"].addEventListener("input", () => {
    clearTimeout(state.searchTimer);
    state.searchTimer = setTimeout(() => search(el["palette-query"].value), 140);
  });

  document.addEventListener("keydown", event => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      if (el.palette.hidden) openPalette();
      else closePalette();
      return;
    }
    if (event.key === "Escape" && !el.palette.hidden) closePalette();
  });

  el["refresh-status"].addEventListener("click", () => {
    resync({ quiet: false }).then(() => showToast("Status refreshed."));
  });

  el["clear-monitor"].addEventListener("click", clearMonitorView);

  el["refresh-integrations"].addEventListener("click", async event => {
    event.currentTarget.disabled = true;
    try {
      await invoke("refresh_harnesses");
      await resync();
      showToast("MCP clients re-checked.");
    } catch (error) {
      showToast(String(error), true);
    } finally {
      event.currentTarget.disabled = false;
    }
  });

  el["test-notification"].addEventListener("click", async event => {
    event.currentTarget.disabled = true;
    try {
      await invoke("test_notification");
      showToast("Test notification sent.");
    } catch (error) {
      showToast(String(error), true);
    } finally {
      event.currentTarget.disabled = false;
    }
  });

  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) resync();
  });
}

/*
 * Start the window, in the order that matters.
 *
 * These were loose top-level statements, so a throw in any one of them silently abandoned every
 * statement after it -- which is how one missing element id left the band reading "Starting"
 * forever with no snapshot, no change subscription, and no heartbeat to recover with.
 *
 * The rule now: the live surface is brought up first, and anything decorative goes last, where
 * failing cannot cost the window its connection to the truth.
 */
function boot() {
  // The heartbeat is this surface's own recovery, so it is installed before anything that can
  // fail. A bad subscription or a bad first snapshot then costs one cycle rather than the window.
  setInterval(() => {
    if (!document.hidden) resync();
  }, HEARTBEAT_MS);
  attempt("wiring the surface", wire);
  if (listen) {
    attempt("subscribing to changes", () => listen(CHANGE_EVENT, message => applyChange(message.payload)));
  }
  attempt("first snapshot", () => resync({ rebuildFeed: true }));
  attempt("about card", armAboutCard);
}

// Anything that escapes a listener or a promise still has to reach the person using the window.
window.addEventListener("error", event => reportFailure("surface", event.error ?? event.message));
window.addEventListener("unhandledrejection", event => reportFailure("surface", event.reason));

boot();
