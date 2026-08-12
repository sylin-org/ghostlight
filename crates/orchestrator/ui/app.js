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

/** Single channel the orchestrator publishes sequenced changes on. */
const CHANGE_EVENT = "ghostlight://change";
/** Slow safety pull for collections that have no change event of their own. */
const HEARTBEAT_MS = 10000;
/** Bound on the retained feed, matching the orchestrator's own bounded history. */
const FEED_LIMIT = 200;

/** Destinations this surface renders, keyed by the orchestrator's search vocabulary. */
const VIEWS = { monitor: "Monitor", integrations: "MCP integrations", status: "Status" };
const SEARCH_VIEWS = {
  home: "monitor",
  activity: "monitor",
  history: "monitor",
  checkup: "status",
  configuration: "status",
  install: "integrations"
};

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
  searchTimer: null
};

const el = {};
for (const id of [
  "lamp", "state-word", "state-facts", "wheel", "wheel-icon", "wheel-label",
  "main-content", "connections", "hero", "hero-med", "hero-body", "hero-right",
  "queue", "queue-count", "clear-monitor", "integration-grid", "diagnostic-grid", "authority-grid",
  "colophon", "palette", "palette-query", "palette-results", "toast",
  "confirm-dialog", "confirm-title", "confirm-detail"
]) {
  el[id] = document.getElementById(id);
}

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
  return String(value ?? "").replaceAll("_", " ");
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
  browser_record: "camera",
  browser_diagnose: "scan"
};

const glyphFor = entry =>
  GLYPHS[ACTIVITY_GLYPH[entry.activity] ?? TOOL_GLYPH[entry.tool] ?? "scan"];
const capabilityClass = entry => CAPABILITY_CLASS[entry.capability] ?? "cap-read";

/* -------------------------------- entries ------------------------------- */

function entryFromOperation(operation) {
  return {
    invocation: operation.invocation,
    workspace: operation.workspace,
    tool: operation.tool,
    activity: operation.activity,
    capability: operation.capability,
    startedAt: operation.started_at_ms ?? Date.now(),
    phase: operation.phase,
    settled: false
  };
}

function entryFromRecord(record, existing) {
  return {
    ...(existing ?? {}),
    invocation: record.invocation,
    workspace: record.workspace,
    tool: record.tool,
    // No activity when the record was restored rather than watched: the medallion then comes
    // from the tool. Defaulting to the quiet label is what made every row identical.
    activity: existing?.activity,
    capability: record.capability,
    startedAt: existing?.startedAt,
    endedAt: record.timestamp_ms,
    phase: record.allowed ? "completed" : "blocked",
    allowed: record.allowed,
    reason: record.reason,
    status: record.status,
    effect: record.effect,
    summary: record.summary,
    durationMs: record.duration_ms,
    observed: record.observed ?? null,
    channel: record.channel ?? null,
    settled: true
  };
}

const entryTime = entry => entry.endedAt ?? entry.startedAt ?? 0;

/**
 * How long the work took. The orchestrator measures this, so a record restored from the audit
 * file reports a real span instead of the blank left by never having watched it start.
 */
const settledMs = entry =>
  entry.durationMs || (entry.endedAt && entry.startedAt ? entry.endedAt - entry.startedAt : NaN);
const isRunning = entry => !entry.settled && (entry.phase === "running" || entry.phase === "held" || entry.phase === "attention");
const isBlocked = entry => entry.phase === "blocked" || entry.allowed === false;

function trimFeed() {
  while (state.feed.length > FEED_LIMIT) {
    const dropped = state.feed.pop();
    state.rowNodes.get(dropped.invocation)?.remove();
    state.rowNodes.delete(dropped.invocation);
  }
}

/* --------------------------- monitor rendering -------------------------- */

/** What the audit can honestly say happened to the page, with no payload to draw on. */
const EFFECT_STORY = {
  none: "left the page unchanged",
  applied: "changed the page",
  partial: "changed the page in part",
  unknown: "outcome could not be confirmed"
};

/**
 * What a readiness adds to a settled row.
 *
 * Complete is the quiet case and earns no words. The others are the difference between "2.5s",
 * which is reassurance, and "8.0s, never settled", which explains why an agent looked stuck.
 */
const READINESS_NOTE = {
  not_applicable: "",
  complete: "",
  interactive: "interactive",
  loading: "never settled",
  unknown: "readiness unknown"
};

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

function runtimeClass() {
  if (!state.connected) return "runtime-offline";
  if (state.runtime === "held" || state.runtime === "ended") return "runtime-held";
  if (state.runtime === "attention") return "runtime-attention";
  return state.feed.some(isRunning) ? "runtime-working" : "runtime-quiet";
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

function paintConnections(snapshot) {
  const chips = snapshot.sessions.map(session => {
    const busy = session.active_operations > 0;
    return `<span class="chip ${busy ? "busy" : "on"}"><span class="dot"></span>${escapeHtml(session.client_label)}`
      + `<small>${session.tab_count} tabs</small></span>`;
  });
  chips.push(...snapshot.browsers.map(browser =>
    `<span class="chip on"><span class="dot"></span>${escapeHtml(browser.family)}`
    + `<small>adapter ${escapeHtml(browser.adapter_version ?? "unknown")}</small></span>`));
  if (!chips.length) {
    chips.push('<span class="chip"><span class="dot"></span>Waiting for a client or a browser</span>');
  }
  el.connections.innerHTML = chips.join("");
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
function paintIfChanged(key, facts, paint) {
  const signature = JSON.stringify(facts);
  if (state.painted[key] === signature) return;
  state.painted[key] = signature;
  paint();
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
  paintIfChanged("integrations", [snapshot.harnesses, [...state.pendingHarnesses]], () => paintIntegrations(snapshot));
  paintIfChanged("status", [snapshot.diagnostics, snapshot.configuration, snapshot.service], () => paintStatus(snapshot));
  paintLamp();
}

async function resync({ rebuildFeed = false, quiet = true } = {}) {
  if (!invoke) {
    state.connected = false;
    paintLamp();
    return;
  }
  try {
    const snapshot = await invoke("workbench_snapshot");
    state.connected = true;
    applySnapshot(snapshot, rebuildFeed || snapshot.seq !== state.seq || !state.snapshot);
  } catch (error) {
    state.connected = false;
    paintLamp();
    if (!quiet) showToast(String(error), true);
  }
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
    case "operation_started": applyStarted(change.operation); break;
    case "operation_changed": applyChanged(change.operation); break;
    case "operation_settled": applySettled(change.record); break;
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

document.getElementById("refresh-status").addEventListener("click", () => {
  resync({ quiet: false }).then(() => showToast("Status refreshed."));
});

el["clear-monitor"].addEventListener("click", clearMonitorView);

document.getElementById("refresh-integrations").addEventListener("click", async event => {
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

document.getElementById("test-notification").addEventListener("click", async event => {
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

if (listen) listen(CHANGE_EVENT, message => applyChange(message.payload));

resync({ rebuildFeed: true });
setInterval(() => {
  if (!document.hidden) resync();
}, HEARTBEAT_MS);
